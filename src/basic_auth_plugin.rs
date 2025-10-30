use async_trait::async_trait;
use std::collections::HashMap;

use crate::auth_plugin::{AuthPlugin, AuthConfig, AuthResult, AuthFailure, AuthToken};
use crate::error::{PluginError, ProtocolError};
use crate::plugin::{Plugin, PluginConfig, ProtocolRequest};

/// Basic Authentication plugin
pub struct BasicAuthPlugin;

impl BasicAuthPlugin {
    pub fn new() -> Self {
        Self
    }
    
    fn create_basic_auth_token(&self, config: &AuthConfig) -> Result<AuthResult, AuthFailure> {
        if let AuthConfig::BasicAuth { username, password } = config {
            // Create Basic Auth token: base64(username:password)
            let credentials = format!("{}:{}", username, password);
            let encoded = base64::encode(credentials.as_bytes());
            
            let token = AuthToken {
                token: encoded,
                token_type: "Basic".to_string(),
                expires_at: None, // Basic auth doesn't expire
                refresh_token: None,
                scope: None,
            };
            
            let metadata = HashMap::from([
                ("username".to_string(), username.clone()),
                ("auth_type".to_string(), "basic".to_string()),
            ]);
            
            Ok(AuthResult { token, metadata })
        } else {
            Err(AuthFailure {
                error_code: "INVALID_CONFIG".to_string(),
                error_description: "Expected BasicAuth configuration".to_string(),
                retry_after: None,
                should_retry: false,
            })
        }
    }
}

#[async_trait]
impl Plugin for BasicAuthPlugin {
    fn name(&self) -> &str {
        "basic-auth"
    }
    
    fn version(&self) -> &str {
        "1.0.0"
    }
    
    fn dependencies(&self) -> Vec<String> {
        vec![]
    }
    
    async fn initialize(&mut self, _config: &PluginConfig) -> Result<(), PluginError> {
        Ok(())
    }
    
    async fn shutdown(&mut self) -> Result<(), PluginError> {
        Ok(())
    }
}

#[async_trait]
impl AuthPlugin for BasicAuthPlugin {
    fn auth_type(&self) -> String {
        "basic".to_string()
    }
    
    async fn authenticate(&self, config: &AuthConfig) -> Result<AuthResult, AuthFailure> {
        self.create_basic_auth_token(config)
    }
    
    async fn refresh_token(&self, token: &AuthToken, _config: &AuthConfig) -> Result<AuthResult, AuthFailure> {
        // Basic auth tokens don't need refreshing, return the same token
        Ok(AuthResult {
            token: token.clone(),
            metadata: HashMap::from([
                ("refreshed".to_string(), "false".to_string()),
                ("reason".to_string(), "basic_auth_no_refresh_needed".to_string()),
            ]),
        })
    }
    
    async fn apply_auth(&self, request: &mut ProtocolRequest, token: &AuthToken) -> Result<(), ProtocolError> {
        let auth_header = format!("{} {}", token.token_type, token.token);
        request.headers.insert("Authorization".to_string(), auth_header);
        Ok(())
    }
    
    fn validate_config(&self, config: &AuthConfig) -> Result<(), PluginError> {
        if let AuthConfig::BasicAuth { username, password } = config {
            if username.is_empty() {
                return Err(PluginError::InitializationFailed("username cannot be empty".to_string()));
            }
            if password.is_empty() {
                return Err(PluginError::InitializationFailed("password cannot be empty".to_string()));
            }
            Ok(())
        } else {
            Err(PluginError::InitializationFailed("Expected BasicAuth configuration".to_string()))
        }
    }
    
    fn supports_refresh(&self) -> bool {
        false // Basic auth doesn't need refresh
    }
}

/// API Key authentication plugin
pub struct ApiKeyAuthPlugin;

impl ApiKeyAuthPlugin {
    pub fn new() -> Self {
        Self
    }
    
    fn create_api_key_token(&self, config: &AuthConfig) -> Result<AuthResult, AuthFailure> {
        if let AuthConfig::ApiKey { key, header_name, prefix } = config {
            let token_value = if let Some(prefix) = prefix {
                format!("{} {}", prefix, key)
            } else {
                key.clone()
            };
            
            let token = AuthToken {
                token: token_value,
                token_type: "ApiKey".to_string(),
                expires_at: None, // API keys typically don't expire automatically
                refresh_token: None,
                scope: None,
            };
            
            let metadata = HashMap::from([
                ("header_name".to_string(), header_name.clone()),
                ("has_prefix".to_string(), prefix.is_some().to_string()),
                ("auth_type".to_string(), "api_key".to_string()),
            ]);
            
            Ok(AuthResult { token, metadata })
        } else {
            Err(AuthFailure {
                error_code: "INVALID_CONFIG".to_string(),
                error_description: "Expected ApiKey configuration".to_string(),
                retry_after: None,
                should_retry: false,
            })
        }
    }
}

#[async_trait]
impl Plugin for ApiKeyAuthPlugin {
    fn name(&self) -> &str {
        "api-key-auth"
    }
    
    fn version(&self) -> &str {
        "1.0.0"
    }
    
    fn dependencies(&self) -> Vec<String> {
        vec![]
    }
    
    async fn initialize(&mut self, _config: &PluginConfig) -> Result<(), PluginError> {
        Ok(())
    }
    
    async fn shutdown(&mut self) -> Result<(), PluginError> {
        Ok(())
    }
}

#[async_trait]
impl AuthPlugin for ApiKeyAuthPlugin {
    fn auth_type(&self) -> String {
        "api_key".to_string()
    }
    
    async fn authenticate(&self, config: &AuthConfig) -> Result<AuthResult, AuthFailure> {
        self.create_api_key_token(config)
    }
    
    async fn refresh_token(&self, token: &AuthToken, _config: &AuthConfig) -> Result<AuthResult, AuthFailure> {
        // API keys don't need refreshing, return the same token
        Ok(AuthResult {
            token: token.clone(),
            metadata: HashMap::from([
                ("refreshed".to_string(), "false".to_string()),
                ("reason".to_string(), "api_key_no_refresh_needed".to_string()),
            ]),
        })
    }
    
    async fn apply_auth(&self, request: &mut ProtocolRequest, token: &AuthToken) -> Result<(), ProtocolError> {
        // For API key auth, we need to get the header name from the original config
        // Since we don't have access to the config here, we'll use a default or 
        // store it in the token metadata during authentication
        
        // Try to get header name from metadata, fallback to common defaults
        let header_name = if let Some(metadata) = request.headers.get("X-Auth-Header-Name") {
            metadata.clone()
        } else {
            // Common API key header names
            "X-API-Key".to_string()
        };
        
        request.headers.insert(header_name, token.token.clone());
        Ok(())
    }
    
    fn validate_config(&self, config: &AuthConfig) -> Result<(), PluginError> {
        if let AuthConfig::ApiKey { key, header_name, .. } = config {
            if key.is_empty() {
                return Err(PluginError::InitializationFailed("API key cannot be empty".to_string()));
            }
            if header_name.is_empty() {
                return Err(PluginError::InitializationFailed("header_name cannot be empty".to_string()));
            }
            Ok(())
        } else {
            Err(PluginError::InitializationFailed("Expected ApiKey configuration".to_string()))
        }
    }
    
    fn supports_refresh(&self) -> bool {
        false // API keys don't need refresh
    }
}