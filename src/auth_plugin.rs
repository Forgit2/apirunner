use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Duration, SystemTime};

use crate::error::{PluginError, ProtocolError};
use crate::plugin::{Plugin, ProtocolRequest};

/// Authentication token with expiration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthToken {
    pub token: String,
    pub token_type: String,
    pub expires_at: Option<SystemTime>,
    pub refresh_token: Option<String>,
    pub scope: Option<String>,
}

impl AuthToken {
    /// Check if the token is expired or will expire within the given buffer
    pub fn is_expired(&self, buffer: Duration) -> bool {
        if let Some(expires_at) = self.expires_at {
            let now = SystemTime::now();
            let buffer_time = now + buffer;
            expires_at <= buffer_time
        } else {
            false // No expiration means token is valid
        }
    }
}

/// Authentication configuration for different auth types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AuthConfig {
    OAuth2 {
        client_id: String,
        client_secret: String,
        token_url: String,
        scope: Option<String>,
        grant_type: String,
        username: Option<String>,
        password: Option<String>,
    },
    Jwt {
        secret: String,
        algorithm: String,
        issuer: Option<String>,
        audience: Option<String>,
        expiration: Option<Duration>,
        custom_claims: HashMap<String, serde_json::Value>,
    },
    BasicAuth {
        username: String,
        password: String,
    },
    ApiKey {
        key: String,
        header_name: String,
        prefix: Option<String>,
    },
    Custom {
        auth_type: String,
        parameters: HashMap<String, serde_json::Value>,
    },
}

/// Authentication result containing token and metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthResult {
    pub token: AuthToken,
    pub metadata: HashMap<String, String>,
}

/// Authentication failure information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthFailure {
    pub error_code: String,
    pub error_description: String,
    pub retry_after: Option<Duration>,
    pub should_retry: bool,
}

/// Authentication plugin trait
#[async_trait]
pub trait AuthPlugin: Plugin {
    /// Get the authentication type this plugin handles
    fn auth_type(&self) -> String;
    
    /// Authenticate and return a token
    async fn authenticate(&self, config: &AuthConfig) -> Result<AuthResult, AuthFailure>;
    
    /// Refresh an existing token if supported
    async fn refresh_token(&self, token: &AuthToken, config: &AuthConfig) -> Result<AuthResult, AuthFailure>;
    
    /// Apply authentication to a request
    async fn apply_auth(&self, request: &mut ProtocolRequest, token: &AuthToken) -> Result<(), ProtocolError>;
    
    /// Validate authentication configuration
    fn validate_config(&self, config: &AuthConfig) -> Result<(), PluginError>;
    
    /// Check if this plugin supports token refresh
    fn supports_refresh(&self) -> bool;
}

/// Authentication chain configuration for multi-step auth
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthChainConfig {
    pub steps: Vec<AuthStepConfig>,
    pub failure_strategy: FailureStrategy,
    pub retry_config: RetryConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthStepConfig {
    pub plugin_name: String,
    pub config: AuthConfig,
    pub required: bool,
    pub depends_on: Vec<String>, // Names of previous steps this depends on
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FailureStrategy {
    FailFast,        // Stop on first failure
    ContinueOnError, // Continue even if non-required steps fail
    RetryAll,        // Retry entire chain on any failure
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryConfig {
    pub max_attempts: u32,
    pub initial_delay: Duration,
    pub max_delay: Duration,
    pub backoff_multiplier: f64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            initial_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(30),
            backoff_multiplier: 2.0,
        }
    }
}

/// Authentication chain result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthChainResult {
    pub tokens: HashMap<String, AuthToken>,
    pub metadata: HashMap<String, String>,
    pub failed_steps: Vec<String>,
}

/// Authentication manager for handling plugin chains and token management
pub struct AuthManager {
    plugins: HashMap<String, Box<dyn AuthPlugin>>,
    token_cache: HashMap<String, AuthToken>,
}

impl AuthManager {
    pub fn new() -> Self {
        Self {
            plugins: HashMap::new(),
            token_cache: HashMap::new(),
        }
    }
    
    /// Register an authentication plugin
    pub fn register_plugin(&mut self, plugin: Box<dyn AuthPlugin>) {
        let auth_type = plugin.auth_type();
        self.plugins.insert(auth_type, plugin);
    }
    
    /// Execute authentication chain
    pub async fn execute_chain(&mut self, chain_config: &AuthChainConfig) -> Result<AuthChainResult, AuthFailure> {
        let mut result = AuthChainResult {
            tokens: HashMap::new(),
            metadata: HashMap::new(),
            failed_steps: Vec::new(),
        };
        
        let mut attempt = 0;
        while attempt < chain_config.retry_config.max_attempts {
            match self.try_execute_chain(chain_config, &mut result).await {
                Ok(_) => return Ok(result),
                Err(failure) => {
                    if !failure.should_retry || attempt == chain_config.retry_config.max_attempts - 1 {
                        return Err(failure);
                    }
                    
                    // Calculate backoff delay
                    let delay = std::cmp::min(
                        chain_config.retry_config.initial_delay.mul_f64(
                            chain_config.retry_config.backoff_multiplier.powi(attempt as i32)
                        ),
                        chain_config.retry_config.max_delay,
                    );
                    
                    tokio::time::sleep(delay).await;
                    attempt += 1;
                }
            }
        }
        
        Err(AuthFailure {
            error_code: "MAX_RETRIES_EXCEEDED".to_string(),
            error_description: "Maximum retry attempts exceeded".to_string(),
            retry_after: None,
            should_retry: false,
        })
    }
    
    async fn try_execute_chain(&mut self, chain_config: &AuthChainConfig, result: &mut AuthChainResult) -> Result<(), AuthFailure> {
        for step in &chain_config.steps {
            match self.execute_step(step, result).await {
                Ok(auth_result) => {
                    result.tokens.insert(step.plugin_name.clone(), auth_result.token);
                    result.metadata.extend(auth_result.metadata);
                }
                Err(failure) => {
                    result.failed_steps.push(step.plugin_name.clone());
                    
                    if step.required {
                        match chain_config.failure_strategy {
                            FailureStrategy::FailFast => return Err(failure),
                            FailureStrategy::RetryAll => return Err(failure),
                            FailureStrategy::ContinueOnError => {
                                // Log error but continue
                                eprintln!("Required auth step '{}' failed: {}", step.plugin_name, failure.error_description);
                            }
                        }
                    }
                }
            }
        }
        
        Ok(())
    }
    
    async fn execute_step(&mut self, step: &AuthStepConfig, result: &AuthChainResult) -> Result<AuthResult, AuthFailure> {
        // Check dependencies
        for dependency in &step.depends_on {
            if !result.tokens.contains_key(dependency) {
                return Err(AuthFailure {
                    error_code: "DEPENDENCY_NOT_SATISFIED".to_string(),
                    error_description: format!("Authentication step '{}' depends on '{}' which has not completed successfully", step.plugin_name, dependency),
                    retry_after: None,
                    should_retry: false,
                });
            }
        }
        
        let plugin = self.plugins.get(&step.plugin_name)
            .ok_or_else(|| AuthFailure {
                error_code: "PLUGIN_NOT_FOUND".to_string(),
                error_description: format!("Authentication plugin '{}' not found", step.plugin_name),
                retry_after: None,
                should_retry: false,
            })?;
        
        // Create enhanced config with dependency tokens if needed
        let enhanced_config = self.enhance_config_with_dependencies(&step.config, &step.depends_on, result)?;
        
        // Validate configuration before attempting authentication
        plugin.validate_config(&enhanced_config).map_err(|e| AuthFailure {
            error_code: "CONFIG_VALIDATION_FAILED".to_string(),
            error_description: format!("Configuration validation failed: {}", e),
            retry_after: None,
            should_retry: false,
        })?;
        
        plugin.authenticate(&enhanced_config).await
    }
    
    fn enhance_config_with_dependencies(&self, config: &AuthConfig, dependencies: &[String], result: &AuthChainResult) -> Result<AuthConfig, AuthFailure> {
        // For certain auth types, we might need to inject tokens from dependencies
        match config {
            AuthConfig::Custom { auth_type, parameters } => {
                let mut enhanced_parameters = parameters.clone();
                // Inject dependency tokens into custom auth parameters
                for dep in dependencies {
                    if let Some(token) = result.tokens.get(dep) {
                        enhanced_parameters.insert(format!("dep_{}_token", dep), serde_json::Value::String(token.token.clone()));
                        enhanced_parameters.insert(format!("dep_{}_type", dep), serde_json::Value::String(token.token_type.clone()));
                    }
                }
                Ok(AuthConfig::Custom { 
                    auth_type: auth_type.clone(), 
                    parameters: enhanced_parameters 
                })
            }
            _ => Ok(config.clone()) // Other auth types don't need dependency injection
        }
    }
    
    /// Apply authentication to a request using cached tokens
    pub async fn apply_auth_to_request(&self, request: &mut ProtocolRequest, auth_configs: &[AuthStepConfig]) -> Result<(), ProtocolError> {
        for config in auth_configs {
            if let Some(token) = self.token_cache.get(&config.plugin_name) {
                if let Some(plugin) = self.plugins.get(&config.plugin_name) {
                    plugin.apply_auth(request, token).await?;
                }
            }
        }
        Ok(())
    }
    
    /// Refresh expired tokens
    pub async fn refresh_expired_tokens(&mut self, configs: &[AuthStepConfig]) -> Result<(), AuthFailure> {
        for config in configs {
            if let Some(token) = self.token_cache.get(&config.plugin_name).cloned() {
                if token.is_expired(Duration::from_secs(60)) { // 1 minute buffer
                    if let Some(plugin) = self.plugins.get(&config.plugin_name) {
                        if plugin.supports_refresh() {
                            match plugin.refresh_token(&token, &config.config).await {
                                Ok(auth_result) => {
                                    self.token_cache.insert(config.plugin_name.clone(), auth_result.token);
                                }
                                Err(failure) => {
                                    // Remove expired token and re-authenticate
                                    self.token_cache.remove(&config.plugin_name);
                                    let auth_result = plugin.authenticate(&config.config).await?;
                                    self.token_cache.insert(config.plugin_name.clone(), auth_result.token);
                                }
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }
}