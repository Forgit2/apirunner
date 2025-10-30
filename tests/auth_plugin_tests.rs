use std::collections::HashMap;
use std::time::Duration;
use tokio;

use apirunner::{
    Plugin, AuthPlugin, AuthConfig, AuthPluginToken as AuthToken, AuthChainConfig, AuthStepConfig,
    FailureStrategy, AuthRetryConfig, AuthManager, JwtAuthPlugin, 
    BasicAuthPlugin, ApiKeyAuthPlugin, PluginConfig,
};

#[tokio::test]
async fn test_basic_auth_plugin() {
    let mut plugin = BasicAuthPlugin::new();
    let config = PluginConfig {
        settings: HashMap::new(),
        environment: "test".to_string(),
    };
    
    // Initialize plugin
    assert!(plugin.initialize(&config).await.is_ok());
    
    // Test authentication
    let auth_config = AuthConfig::BasicAuth {
        username: "testuser".to_string(),
        password: "testpass".to_string(),
    };
    
    let result = plugin.authenticate(&auth_config).await;
    assert!(result.is_ok());
    
    let auth_result = result.unwrap();
    assert_eq!(auth_result.token.token_type, "Basic");
    assert!(auth_result.token.expires_at.is_none()); // Basic auth doesn't expire
    
    // Verify the token is properly base64 encoded
    let expected_token = base64::encode("testuser:testpass");
    assert_eq!(auth_result.token.token, expected_token);
}

#[tokio::test]
async fn test_api_key_auth_plugin() {
    let mut plugin = ApiKeyAuthPlugin::new();
    let config = PluginConfig {
        settings: HashMap::new(),
        environment: "test".to_string(),
    };
    
    // Initialize plugin
    assert!(plugin.initialize(&config).await.is_ok());
    
    // Test authentication with prefix
    let auth_config = AuthConfig::ApiKey {
        key: "secret-api-key-123".to_string(),
        header_name: "X-API-Key".to_string(),
        prefix: Some("Bearer".to_string()),
    };
    
    let result = plugin.authenticate(&auth_config).await;
    assert!(result.is_ok());
    
    let auth_result = result.unwrap();
    assert_eq!(auth_result.token.token_type, "ApiKey");
    assert_eq!(auth_result.token.token, "Bearer secret-api-key-123");
    
    // Test authentication without prefix
    let auth_config_no_prefix = AuthConfig::ApiKey {
        key: "secret-api-key-456".to_string(),
        header_name: "Authorization".to_string(),
        prefix: None,
    };
    
    let result_no_prefix = plugin.authenticate(&auth_config_no_prefix).await;
    assert!(result_no_prefix.is_ok());
    
    let auth_result_no_prefix = result_no_prefix.unwrap();
    assert_eq!(auth_result_no_prefix.token.token, "secret-api-key-456");
}

#[tokio::test]
async fn test_jwt_auth_plugin() {
    let mut plugin = JwtAuthPlugin::new();
    let config = PluginConfig {
        settings: HashMap::new(),
        environment: "test".to_string(),
    };
    
    // Initialize plugin
    assert!(plugin.initialize(&config).await.is_ok());
    
    // Test JWT generation
    let mut custom_claims = HashMap::new();
    custom_claims.insert("user_id".to_string(), serde_json::Value::String("12345".to_string()));
    custom_claims.insert("role".to_string(), serde_json::Value::String("admin".to_string()));
    
    let auth_config = AuthConfig::Jwt {
        secret: "my-secret-key".to_string(),
        algorithm: "HS256".to_string(),
        issuer: Some("test-issuer".to_string()),
        audience: Some("test-audience".to_string()),
        expiration: Some(Duration::from_secs(3600)),
        custom_claims,
    };
    
    let result = plugin.authenticate(&auth_config).await;
    assert!(result.is_ok());
    
    let auth_result = result.unwrap();
    assert_eq!(auth_result.token.token_type, "Bearer");
    assert!(auth_result.token.expires_at.is_some());
    
    // JWT should have 3 parts separated by dots
    let token_parts: Vec<&str> = auth_result.token.token.split('.').collect();
    assert_eq!(token_parts.len(), 3);
}

#[tokio::test]
async fn test_auth_manager_single_plugin() {
    let mut auth_manager = AuthManager::new();
    
    // Register plugins
    auth_manager.register_plugin(Box::new(BasicAuthPlugin::new()));
    auth_manager.register_plugin(Box::new(ApiKeyAuthPlugin::new()));
    
    // Create a simple chain with one step
    let chain_config = AuthChainConfig {
        steps: vec![
            AuthStepConfig {
                plugin_name: "basic".to_string(),
                config: AuthConfig::BasicAuth {
                    username: "user".to_string(),
                    password: "pass".to_string(),
                },
                required: true,
                depends_on: vec![],
            }
        ],
        failure_strategy: FailureStrategy::FailFast,
        retry_config: AuthRetryConfig::default(),
    };
    
    let result = auth_manager.execute_chain(&chain_config).await;
    assert!(result.is_ok());
    
    let chain_result = result.unwrap();
    assert_eq!(chain_result.tokens.len(), 1);
    assert!(chain_result.tokens.contains_key("basic"));
    assert!(chain_result.failed_steps.is_empty());
}

#[tokio::test]
async fn test_auth_chain_with_dependencies() {
    let mut auth_manager = AuthManager::new();
    
    // Register plugins
    auth_manager.register_plugin(Box::new(BasicAuthPlugin::new()));
    auth_manager.register_plugin(Box::new(ApiKeyAuthPlugin::new()));
    
    // Create a chain where second step depends on first
    let chain_config = AuthChainConfig {
        steps: vec![
            AuthStepConfig {
                plugin_name: "basic".to_string(),
                config: AuthConfig::BasicAuth {
                    username: "user".to_string(),
                    password: "pass".to_string(),
                },
                required: true,
                depends_on: vec![],
            },
            AuthStepConfig {
                plugin_name: "api_key".to_string(),
                config: AuthConfig::ApiKey {
                    key: "api-key-123".to_string(),
                    header_name: "X-API-Key".to_string(),
                    prefix: None,
                },
                required: true,
                depends_on: vec!["basic".to_string()],
            }
        ],
        failure_strategy: FailureStrategy::FailFast,
        retry_config: AuthRetryConfig::default(),
    };
    
    let result = auth_manager.execute_chain(&chain_config).await;
    assert!(result.is_ok());
    
    let chain_result = result.unwrap();
    assert_eq!(chain_result.tokens.len(), 2);
    assert!(chain_result.tokens.contains_key("basic"));
    assert!(chain_result.tokens.contains_key("api_key"));
    assert!(chain_result.failed_steps.is_empty());
}

#[tokio::test]
async fn test_auth_chain_failure_strategies() {
    let mut auth_manager = AuthManager::new();
    
    // Register plugins
    auth_manager.register_plugin(Box::new(BasicAuthPlugin::new()));
    
    // Create a chain with invalid config to test failure handling
    let chain_config = AuthChainConfig {
        steps: vec![
            AuthStepConfig {
                plugin_name: "basic".to_string(),
                config: AuthConfig::BasicAuth {
                    username: "".to_string(), // Invalid empty username
                    password: "pass".to_string(),
                },
                required: true,
                depends_on: vec![],
            }
        ],
        failure_strategy: FailureStrategy::FailFast,
        retry_config: AuthRetryConfig {
            max_attempts: 1,
            initial_delay: Duration::from_millis(10),
            max_delay: Duration::from_millis(100),
            backoff_multiplier: 2.0,
        },
    };
    
    let result = auth_manager.execute_chain(&chain_config).await;
    assert!(result.is_err());
    
    let failure = result.unwrap_err();
    assert!(failure.error_description.contains("username"));
}

#[tokio::test]
async fn test_auth_chain_retry_logic() {
    let mut auth_manager = AuthManager::new();
    
    // Register plugins
    auth_manager.register_plugin(Box::new(BasicAuthPlugin::new()));
    
    // Create a chain with retry configuration
    let chain_config = AuthChainConfig {
        steps: vec![
            AuthStepConfig {
                plugin_name: "basic".to_string(),
                config: AuthConfig::BasicAuth {
                    username: "user".to_string(),
                    password: "pass".to_string(),
                },
                required: true,
                depends_on: vec![],
            }
        ],
        failure_strategy: FailureStrategy::FailFast,
        retry_config: AuthRetryConfig {
            max_attempts: 3,
            initial_delay: Duration::from_millis(10),
            max_delay: Duration::from_millis(100),
            backoff_multiplier: 2.0,
        },
    };
    
    let result = auth_manager.execute_chain(&chain_config).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_plugin_validation() {
    let basic_plugin = BasicAuthPlugin::new();
    let api_key_plugin = ApiKeyAuthPlugin::new();
    let jwt_plugin = JwtAuthPlugin::new();
    
    // Test valid configurations
    let valid_basic_config = AuthConfig::BasicAuth {
        username: "user".to_string(),
        password: "pass".to_string(),
    };
    assert!(basic_plugin.validate_config(&valid_basic_config).is_ok());
    
    let valid_api_key_config = AuthConfig::ApiKey {
        key: "key123".to_string(),
        header_name: "X-API-Key".to_string(),
        prefix: None,
    };
    assert!(api_key_plugin.validate_config(&valid_api_key_config).is_ok());
    
    let valid_jwt_config = AuthConfig::Jwt {
        secret: "secret".to_string(),
        algorithm: "HS256".to_string(),
        issuer: None,
        audience: None,
        expiration: None,
        custom_claims: HashMap::new(),
    };
    assert!(jwt_plugin.validate_config(&valid_jwt_config).is_ok());
    
    // Test invalid configurations
    let invalid_basic_config = AuthConfig::BasicAuth {
        username: "".to_string(), // Empty username
        password: "pass".to_string(),
    };
    assert!(basic_plugin.validate_config(&invalid_basic_config).is_err());
    
    let invalid_api_key_config = AuthConfig::ApiKey {
        key: "".to_string(), // Empty key
        header_name: "X-API-Key".to_string(),
        prefix: None,
    };
    assert!(api_key_plugin.validate_config(&invalid_api_key_config).is_err());
    
    let invalid_jwt_config = AuthConfig::Jwt {
        secret: "".to_string(), // Empty secret
        algorithm: "HS256".to_string(),
        issuer: None,
        audience: None,
        expiration: None,
        custom_claims: HashMap::new(),
    };
    assert!(jwt_plugin.validate_config(&invalid_jwt_config).is_err());
}

#[tokio::test]
async fn test_token_refresh_capabilities() {
    let basic_plugin = BasicAuthPlugin::new();
    let api_key_plugin = ApiKeyAuthPlugin::new();
    let jwt_plugin = JwtAuthPlugin::new();
    
    // Basic auth and API key don't support refresh
    assert!(!basic_plugin.supports_refresh());
    assert!(!api_key_plugin.supports_refresh());
    
    // JWT supports refresh (regeneration)
    assert!(jwt_plugin.supports_refresh());
}