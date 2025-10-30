use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::time::Duration;
use tokio::time::sleep;

use crate::auth_plugin::{
    AuthPlugin, AuthConfig, AuthResult, AuthFailure, AuthChainConfig, 
    AuthStepConfig, FailureStrategy, RetryConfig, AuthChainResult
};

/// Enhanced authentication chain executor with advanced failure handling
pub struct AuthChainExecutor {
    plugins: HashMap<String, Box<dyn AuthPlugin>>,
    execution_history: Vec<ExecutionAttempt>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionAttempt {
    pub attempt_number: u32,
    pub started_at: std::time::SystemTime,
    pub completed_at: Option<std::time::SystemTime>,
    pub steps_executed: Vec<StepExecution>,
    pub final_result: Option<ExecutionOutcome>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepExecution {
    pub step_name: String,
    pub started_at: std::time::SystemTime,
    pub completed_at: Option<std::time::SystemTime>,
    pub success: bool,
    pub error_message: Option<String>,
    pub retry_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExecutionOutcome {
    Success,
    PartialSuccess { failed_optional_steps: Vec<String> },
    Failure { error: String },
}

impl AuthChainExecutor {
    pub fn new() -> Self {
        Self {
            plugins: HashMap::new(),
            execution_history: Vec::new(),
        }
    }
    
    /// Register an authentication plugin
    pub fn register_plugin(&mut self, plugin: Box<dyn AuthPlugin>) {
        let auth_type = plugin.auth_type();
        self.plugins.insert(auth_type, plugin);
    }
    
    /// Execute authentication chain with advanced error handling and retry logic
    pub async fn execute_chain(&mut self, chain_config: &AuthChainConfig) -> Result<AuthChainResult, AuthFailure> {
        let mut attempt = ExecutionAttempt {
            attempt_number: 1,
            started_at: std::time::SystemTime::now(),
            completed_at: None,
            steps_executed: Vec::new(),
            final_result: None,
        };
        
        // Validate chain configuration
        self.validate_chain_config(chain_config)?;
        
        let mut retry_count = 0;
        while retry_count < chain_config.retry_config.max_attempts {
            attempt.attempt_number = retry_count + 1;
            attempt.started_at = std::time::SystemTime::now();
            attempt.steps_executed.clear();
            
            match self.try_execute_chain_with_tracking(chain_config, &mut attempt).await {
                Ok(result) => {
                    attempt.completed_at = Some(std::time::SystemTime::now());
                    attempt.final_result = Some(if result.failed_steps.is_empty() {
                        ExecutionOutcome::Success
                    } else {
                        ExecutionOutcome::PartialSuccess { 
                            failed_optional_steps: result.failed_steps.clone() 
                        }
                    });
                    
                    self.execution_history.push(attempt);
                    return Ok(result);
                }
                Err(failure) => {
                    attempt.completed_at = Some(std::time::SystemTime::now());
                    attempt.final_result = Some(ExecutionOutcome::Failure { 
                        error: failure.error_description.clone() 
                    });
                    
                    if !failure.should_retry || retry_count == chain_config.retry_config.max_attempts - 1 {
                        self.execution_history.push(attempt);
                        return Err(failure);
                    }
                    
                    // Calculate exponential backoff delay
                    let delay = self.calculate_backoff_delay(&chain_config.retry_config, retry_count);
                    
                    // Add some jitter to prevent thundering herd
                    let jitter = Duration::from_millis(fastrand::u64(0..=100));
                    let total_delay = delay + jitter;
                    
                    sleep(total_delay).await;
                    retry_count += 1;
                }
            }
        }
        
        self.execution_history.push(attempt);
        Err(AuthFailure {
            error_code: "MAX_RETRIES_EXCEEDED".to_string(),
            error_description: format!("Maximum retry attempts ({}) exceeded", chain_config.retry_config.max_attempts),
            retry_after: None,
            should_retry: false,
        })
    }
    
    async fn try_execute_chain_with_tracking(&mut self, chain_config: &AuthChainConfig, attempt: &mut ExecutionAttempt) -> Result<AuthChainResult, AuthFailure> {
        let mut result = AuthChainResult {
            tokens: HashMap::new(),
            metadata: HashMap::new(),
            failed_steps: Vec::new(),
        };
        
        // Execute steps in dependency order
        let execution_order = self.calculate_execution_order(&chain_config.steps)?;
        
        for step_name in execution_order {
            let step = chain_config.steps.iter()
                .find(|s| s.plugin_name == step_name)
                .unwrap(); // Safe because we validated the order
                
            let mut step_execution = StepExecution {
                step_name: step.plugin_name.clone(),
                started_at: std::time::SystemTime::now(),
                completed_at: None,
                success: false,
                error_message: None,
                retry_count: 0,
            };
            
            match self.execute_step_with_retry(step, &result, &chain_config.retry_config).await {
                Ok(auth_result) => {
                    result.tokens.insert(step.plugin_name.clone(), auth_result.token);
                    result.metadata.extend(auth_result.metadata);
                    step_execution.success = true;
                }
                Err(failure) => {
                    step_execution.error_message = Some(failure.error_description.clone());
                    result.failed_steps.push(step.plugin_name.clone());
                    
                    if step.required {
                        match chain_config.failure_strategy {
                            FailureStrategy::FailFast => {
                                step_execution.completed_at = Some(std::time::SystemTime::now());
                                attempt.steps_executed.push(step_execution);
                                return Err(failure);
                            }
                            FailureStrategy::RetryAll => {
                                step_execution.completed_at = Some(std::time::SystemTime::now());
                                attempt.steps_executed.push(step_execution);
                                return Err(failure);
                            }
                            FailureStrategy::ContinueOnError => {
                                // Log error but continue with next steps
                                eprintln!("Required auth step '{}' failed but continuing due to ContinueOnError strategy: {}", 
                                    step.plugin_name, failure.error_description);
                            }
                        }
                    }
                }
            }
            
            step_execution.completed_at = Some(std::time::SystemTime::now());
            attempt.steps_executed.push(step_execution);
        }
        
        Ok(result)
    }
    
    async fn execute_step_with_retry(&mut self, step: &AuthStepConfig, result: &AuthChainResult, retry_config: &RetryConfig) -> Result<AuthResult, AuthFailure> {
        let mut retry_count = 0;
        
        while retry_count < retry_config.max_attempts {
            match self.execute_single_step(step, result).await {
                Ok(auth_result) => return Ok(auth_result),
                Err(failure) => {
                    if !failure.should_retry || retry_count == retry_config.max_attempts - 1 {
                        return Err(failure);
                    }
                    
                    let delay = self.calculate_backoff_delay(retry_config, retry_count);
                    sleep(delay).await;
                    retry_count += 1;
                }
            }
        }
        
        Err(AuthFailure {
            error_code: "STEP_MAX_RETRIES_EXCEEDED".to_string(),
            error_description: format!("Step '{}' exceeded maximum retry attempts", step.plugin_name),
            retry_after: None,
            should_retry: false,
        })
    }
    
    async fn execute_single_step(&mut self, step: &AuthStepConfig, result: &AuthChainResult) -> Result<AuthResult, AuthFailure> {
        // Check dependencies
        for dependency in &step.depends_on {
            if !result.tokens.contains_key(dependency) {
                return Err(AuthFailure {
                    error_code: "DEPENDENCY_NOT_SATISFIED".to_string(),
                    error_description: format!("Step '{}' depends on '{}' which has not completed successfully", step.plugin_name, dependency),
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
        match config {
            AuthConfig::Custom { auth_type, parameters } => {
                let mut enhanced_parameters = parameters.clone();
                // Inject dependency tokens into custom auth parameters
                for dep in dependencies {
                    if let Some(token) = result.tokens.get(dep) {
                        enhanced_parameters.insert(format!("dep_{}_token", dep), serde_json::Value::String(token.token.clone()));
                        enhanced_parameters.insert(format!("dep_{}_type", dep), serde_json::Value::String(token.token_type.clone()));
                        if let Some(expires_at) = token.expires_at {
                            if let Ok(duration) = expires_at.duration_since(std::time::UNIX_EPOCH) {
                                enhanced_parameters.insert(format!("dep_{}_expires_at", dep), serde_json::Value::Number(serde_json::Number::from(duration.as_secs())));
                            }
                        }
                    }
                }
                Ok(AuthConfig::Custom { 
                    auth_type: auth_type.clone(), 
                    parameters: enhanced_parameters 
                })
            }
            AuthConfig::OAuth2 { client_id, client_secret, token_url, scope, grant_type, username, password } => {
                // For OAuth2, we might inject tokens from previous steps as additional parameters
                for dep in dependencies {
                    if let Some(_token) = result.tokens.get(dep) {
                        // This could be used for token exchange flows
                        if grant_type == "urn:ietf:params:oauth:grant-type:token-exchange" {
                            // Inject the subject token from dependency
                            // This would require extending the OAuth2 config to support token exchange
                        }
                    }
                }
                Ok(config.clone()) // For now, return as-is
            }
            _ => Ok(config.clone()) // Other auth types don't need dependency injection
        }
    }
    
    fn validate_chain_config(&self, chain_config: &AuthChainConfig) -> Result<(), AuthFailure> {
        // Check for circular dependencies
        if self.has_circular_dependencies(&chain_config.steps) {
            return Err(AuthFailure {
                error_code: "CIRCULAR_DEPENDENCY".to_string(),
                error_description: "Circular dependency detected in authentication chain".to_string(),
                retry_after: None,
                should_retry: false,
            });
        }
        
        // Check that all referenced plugins exist
        for step in &chain_config.steps {
            if !self.plugins.contains_key(&step.plugin_name) {
                return Err(AuthFailure {
                    error_code: "PLUGIN_NOT_FOUND".to_string(),
                    error_description: format!("Plugin '{}' not found", step.plugin_name),
                    retry_after: None,
                    should_retry: false,
                });
            }
        }
        
        // Check that all dependencies reference valid steps
        let step_names: HashSet<String> = chain_config.steps.iter().map(|s| s.plugin_name.clone()).collect();
        for step in &chain_config.steps {
            for dep in &step.depends_on {
                if !step_names.contains(dep) {
                    return Err(AuthFailure {
                        error_code: "INVALID_DEPENDENCY".to_string(),
                        error_description: format!("Step '{}' depends on '{}' which is not defined in the chain", step.plugin_name, dep),
                        retry_after: None,
                        should_retry: false,
                    });
                }
            }
        }
        
        Ok(())
    }
    
    fn has_circular_dependencies(&self, steps: &[AuthStepConfig]) -> bool {
        let mut visited = HashSet::new();
        let mut rec_stack = HashSet::new();
        
        for step in steps {
            if self.has_cycle_util(&step.plugin_name, steps, &mut visited, &mut rec_stack) {
                return true;
            }
        }
        
        false
    }
    
    fn has_cycle_util(&self, step_name: &str, steps: &[AuthStepConfig], visited: &mut HashSet<String>, rec_stack: &mut HashSet<String>) -> bool {
        visited.insert(step_name.to_string());
        rec_stack.insert(step_name.to_string());
        
        if let Some(step) = steps.iter().find(|s| s.plugin_name == step_name) {
            for dep in &step.depends_on {
                if !visited.contains(dep) {
                    if self.has_cycle_util(dep, steps, visited, rec_stack) {
                        return true;
                    }
                } else if rec_stack.contains(dep) {
                    return true;
                }
            }
        }
        
        rec_stack.remove(step_name);
        false
    }
    
    fn calculate_execution_order(&self, steps: &[AuthStepConfig]) -> Result<Vec<String>, AuthFailure> {
        let mut order = Vec::new();
        let mut visited = HashSet::new();
        let mut temp_visited = HashSet::new();
        
        for step in steps {
            if !visited.contains(&step.plugin_name) {
                self.topological_sort(&step.plugin_name, steps, &mut visited, &mut temp_visited, &mut order)?;
            }
        }
        
        order.reverse(); // Reverse to get correct dependency order
        Ok(order)
    }
    
    fn topological_sort(&self, step_name: &str, steps: &[AuthStepConfig], visited: &mut HashSet<String>, temp_visited: &mut HashSet<String>, order: &mut Vec<String>) -> Result<(), AuthFailure> {
        if temp_visited.contains(step_name) {
            return Err(AuthFailure {
                error_code: "CIRCULAR_DEPENDENCY".to_string(),
                error_description: format!("Circular dependency detected involving step '{}'", step_name),
                retry_after: None,
                should_retry: false,
            });
        }
        
        if visited.contains(step_name) {
            return Ok(());
        }
        
        temp_visited.insert(step_name.to_string());
        
        if let Some(step) = steps.iter().find(|s| s.plugin_name == step_name) {
            for dep in &step.depends_on {
                self.topological_sort(dep, steps, visited, temp_visited, order)?;
            }
        }
        
        temp_visited.remove(step_name);
        visited.insert(step_name.to_string());
        order.push(step_name.to_string());
        
        Ok(())
    }
    
    fn calculate_backoff_delay(&self, retry_config: &RetryConfig, retry_count: u32) -> Duration {
        let delay = retry_config.initial_delay.as_millis() as f64 * retry_config.backoff_multiplier.powi(retry_count as i32);
        let delay_ms = delay as u64;
        let capped_delay = Duration::from_millis(delay_ms).min(retry_config.max_delay);
        capped_delay
    }
    
    /// Get execution history for debugging and monitoring
    pub fn get_execution_history(&self) -> &[ExecutionAttempt] {
        &self.execution_history
    }
    
    /// Clear execution history
    pub fn clear_history(&mut self) {
        self.execution_history.clear();
    }
}