use apirunner::configuration::{Configuration, ConfigurationManager, ExecutionConfig, RetryConfig, RateLimitConfig, ConfigPluginConfig, ReportingConfig, ConfigAuthConfig, ConfigDataSourceConfig, EnvironmentConfig};
use proptest::prelude::*;
use std::collections::HashMap;
use std::path::PathBuf;

#[tokio::test]
async fn test_configuration_manager_creation() {
    let config_manager = ConfigurationManager::new().unwrap();
    let config = config_manager.get_configuration();
    
    // Test default configuration values
    assert_eq!(config.execution.default_strategy, "serial");
    assert!(config.execution.max_concurrency > 0);
}

#[tokio::test]
async fn test_configuration_structure() {
    let config_manager = ConfigurationManager::new().unwrap();
    
    // Test valid configuration structure
    let valid_config = Configuration {
        execution: ExecutionConfig {
            default_strategy: "serial".to_string(),
            max_concurrency: 4,
            request_timeout: 30,
            retry: RetryConfig {
                max_attempts: 3,
                base_delay_ms: 1000,
                max_delay_ms: 10000,
                backoff_multiplier: 2.0,
            },
            rate_limit: RateLimitConfig {
                requests_per_second: 100.0,
                burst_capacity: 10,
            },
        },
        plugins: ConfigPluginConfig {
            plugin_dir: PathBuf::from("plugins"),
            auto_load: true,
            hot_reload: false,
            plugins: HashMap::new(),
        },
        reporting: ReportingConfig {
            formats: vec!["json".to_string()],
            output_dir: std::env::temp_dir(),
            templates: HashMap::new(),
        },
        auth: ConfigAuthConfig {
            default_method: Some("none".to_string()),
            methods: HashMap::new(),
        },
        data_sources: ConfigDataSourceConfig {
            default_type: "file".to_string(),
            sources: HashMap::new(),
        },
        environments: HashMap::new(),
        custom: HashMap::new(),
    };
    
    // Basic structure validation
    assert_eq!(valid_config.execution.default_strategy, "serial");
    assert_eq!(valid_config.execution.max_concurrency, 4);
    assert_eq!(valid_config.data_sources.default_type, "file");
}

#[tokio::test]
async fn test_environment_specific_configuration() {
    let config_manager = ConfigurationManager::new().unwrap();
    
    // Create configuration with environment-specific settings
    let mut config = Configuration {
        execution: ExecutionConfig {
            default_strategy: "serial".to_string(),
            max_concurrency: 4,
            request_timeout: 30,
            retry: RetryConfig {
                max_attempts: 3,
                base_delay_ms: 1000,
                max_delay_ms: 10000,
                backoff_multiplier: 2.0,
            },
            rate_limit: RateLimitConfig {
                requests_per_second: 100.0,
                burst_capacity: 10,
            },
        },
        plugins: ConfigPluginConfig {
            plugin_dir: PathBuf::from("plugins"),
            auto_load: true,
            hot_reload: false,
            plugins: HashMap::new(),
        },
        reporting: ReportingConfig {
            formats: vec!["json".to_string()],
            output_dir: std::env::temp_dir(),
            templates: HashMap::new(),
        },
        auth: ConfigAuthConfig {
            default_method: Some("none".to_string()),
            methods: HashMap::new(),
        },
        data_sources: ConfigDataSourceConfig {
            default_type: "file".to_string(),
            sources: HashMap::new(),
        },
        environments: HashMap::new(),
        custom: HashMap::new(),
    };
    
    // Add environment-specific configuration
    config.environments.insert("test".to_string(), EnvironmentConfig {
        name: "test".to_string(),
        base_urls: HashMap::from([("api".to_string(), "http://localhost:8080".to_string())]),
        auth_overrides: HashMap::new(),
        variables: HashMap::new(),
    });
    
    config.environments.insert("production".to_string(), EnvironmentConfig {
        name: "production".to_string(),
        base_urls: HashMap::from([("api".to_string(), "https://api.example.com".to_string())]),
        auth_overrides: HashMap::new(),
        variables: HashMap::new(),
    });
    
    // Test environment resolution
    assert_eq!(config.environments.len(), 2);
    assert!(config.environments.contains_key("test"));
    assert!(config.environments.contains_key("production"));
}

#[tokio::test]
async fn test_configuration_defaults() {
    let config_manager = ConfigurationManager::new().unwrap();
    let config = config_manager.get_configuration();
    
    // Test default values
    assert_eq!(config.execution.default_strategy, "serial");
    assert!(config.execution.max_concurrency > 0);
    assert!(config.execution.request_timeout > 0);
    assert!(config.execution.retry.max_attempts > 0);
    assert!(config.execution.rate_limit.requests_per_second > 0.0);
}

#[tokio::test]
async fn test_configuration_loading() {
    use tempfile::Builder;
    use std::io::Write;
    
    // Create a temporary configuration file with .yaml extension
    let mut temp_file = Builder::new()
        .suffix(".yml")
        .tempfile()
        .unwrap();
    let config_content = r#"
execution:
  default_strategy: "parallel"
  max_concurrency: 8
  request_timeout: 60
  retry:
    max_attempts: 5
    base_delay_ms: 2000
    max_delay_ms: 30000
    backoff_multiplier: 1.5
  rate_limit:
    requests_per_second: 50.0
    burst_capacity: 20

plugins:
  plugin_dir: "custom_plugins"
  auto_load: true
  hot_reload: true
  plugins: {}

reporting:
  formats: ["json", "html"]
  output_dir: "/tmp/reports"
  templates: {}

auth:
  default_method: "basic"
  methods: {}

data_sources:
  default_type: "csv"
  sources: {}

environments: {}
custom: {}
"#;
    
    temp_file.write_all(config_content.as_bytes()).unwrap();
    temp_file.flush().unwrap();
    
    // Create configuration manager and add the config path
    let mut config_manager = ConfigurationManager::new().unwrap();
    config_manager.add_config_path(temp_file.path(), 1).unwrap();
    
    // Load configuration from file
    config_manager.load_configuration().await.unwrap();
    
    let config = config_manager.get_configuration();
    
    // Verify loaded configuration values
    assert_eq!(config.execution.default_strategy, "parallel");
    assert_eq!(config.execution.max_concurrency, 8);
    assert_eq!(config.execution.request_timeout, 60);
    assert_eq!(config.execution.retry.max_attempts, 5);
    assert_eq!(config.execution.retry.base_delay_ms, 2000);
    assert_eq!(config.execution.retry.max_delay_ms, 30000);
    assert_eq!(config.execution.retry.backoff_multiplier, 1.5);
    assert_eq!(config.execution.rate_limit.requests_per_second, 50.0);
    assert_eq!(config.execution.rate_limit.burst_capacity, 20);
    
    assert_eq!(config.plugins.plugin_dir, PathBuf::from("custom_plugins"));
    assert_eq!(config.plugins.auto_load, true);
    assert_eq!(config.plugins.hot_reload, true);
    
    assert_eq!(config.reporting.formats, vec!["json", "html"]);
    assert_eq!(config.reporting.output_dir, PathBuf::from("/tmp/reports"));
    
    assert_eq!(config.auth.default_method, Some("basic".to_string()));
    assert_eq!(config.data_sources.default_type, "csv");
}

// Property-based tests
proptest! {
    #[test]
    fn test_configuration_concurrency_limits(
        max_concurrency in 1u32..100,
        request_timeout in 1u64..300
    ) {
        let config = Configuration {
            execution: ExecutionConfig {
                default_strategy: "parallel".to_string(),
                max_concurrency: max_concurrency as usize,
                request_timeout,
                retry: RetryConfig {
                    max_attempts: 3,
                    base_delay_ms: 1000,
                    max_delay_ms: 10000,
                    backoff_multiplier: 2.0,
                },
                rate_limit: RateLimitConfig {
                    requests_per_second: 100.0,
                    burst_capacity: 10,
                },
            },
            plugins: ConfigPluginConfig {
                plugin_dir: PathBuf::from("plugins"),
                auto_load: true,
                hot_reload: false,
                plugins: HashMap::new(),
            },
            reporting: ReportingConfig {
                formats: vec!["json".to_string()],
                output_dir: std::env::temp_dir(),
                templates: HashMap::new(),
            },
            auth: ConfigAuthConfig {
                default_method: Some("none".to_string()),
                methods: HashMap::new(),
            },
            data_sources: ConfigDataSourceConfig {
                default_type: "file".to_string(),
                sources: HashMap::new(),
            },
            environments: HashMap::new(),
            custom: HashMap::new(),
        };
        
        prop_assert_eq!(config.execution.max_concurrency, max_concurrency as usize);
        prop_assert_eq!(config.execution.request_timeout, request_timeout);
    }
    
    #[test]
    fn test_retry_configuration_validation(
        max_attempts in 1u32..10,
        base_delay_ms in 100u64..5000,
        max_delay_ms in 5000u64..60000
    ) {
        let retry_config = RetryConfig {
            max_attempts,
            base_delay_ms,
            max_delay_ms,
            backoff_multiplier: 2.0,
        };
        
        prop_assert!(retry_config.max_attempts >= 1);
        prop_assert!(retry_config.base_delay_ms < retry_config.max_delay_ms);
        prop_assert!(retry_config.backoff_multiplier > 0.0);
    }
}