use apirunner::rest_api_plugin::{RestApiPlugin, HttpStatusAssertion, HttpHeaderAssertion, ResponseTimeAssertion, RetryConfig};
use apirunner::{
    Plugin, PluginConfig, ProtocolPlugin, AssertionPlugin,
    ProtocolRequest, HttpMethod, ProtocolVersion, ConnectionConfig,
    ResponseData, ExpectedValue, AssertionType,
};
use std::collections::HashMap;
use std::time::Duration;
use tokio;

#[tokio::test]
async fn test_rest_api_plugin_initialization() {
    let mut plugin = RestApiPlugin::new();
    
    let config = PluginConfig {
        settings: HashMap::from([
            ("connection_pool_size".to_string(), serde_json::Value::Number(20.into())),
            ("default_timeout_ms".to_string(), serde_json::Value::Number(5000.into())),
        ]),
        environment: "test".to_string(),
    };
    
    let result = plugin.initialize(&config).await;
    assert!(result.is_ok(), "Plugin initialization should succeed");
    
    assert_eq!(plugin.name(), "rest-api");
    assert_eq!(plugin.version(), "1.0.0");
    assert!(plugin.dependencies().is_empty());
    
    let shutdown_result = plugin.shutdown().await;
    assert!(shutdown_result.is_ok(), "Plugin shutdown should succeed");
}

#[tokio::test]
async fn test_rest_api_plugin_supported_features() {
    let plugin = RestApiPlugin::new();
    
    let schemes = plugin.supported_schemes();
    assert!(schemes.contains(&"http".to_string()));
    assert!(schemes.contains(&"https".to_string()));
    
    let versions = plugin.supported_versions();
    assert!(versions.contains(&ProtocolVersion::Http1_1));
    assert!(versions.contains(&ProtocolVersion::Http2));
}

#[tokio::test]
async fn test_rest_api_plugin_with_custom_config() {
    let retry_config = RetryConfig {
        max_retries: 5,
        initial_delay: Duration::from_millis(50),
        max_delay: Duration::from_secs(10),
        backoff_multiplier: 1.5,
        retry_on_status: vec![429, 502, 503],
    };
    
    let plugin = RestApiPlugin::with_config(15, Duration::from_secs(45), retry_config);
    assert_eq!(plugin.connection_pool_size(), 15);
    assert_eq!(plugin.default_timeout(), Duration::from_secs(45));
}

#[tokio::test]
async fn test_http_status_assertion() {
    let mut assertion = HttpStatusAssertion::default();
    
    let config = PluginConfig {
        settings: HashMap::new(),
        environment: "test".to_string(),
    };
    
    let init_result = assertion.initialize(&config).await;
    assert!(init_result.is_ok());
    
    assert_eq!(assertion.assertion_type(), AssertionType::StatusCode);
    assert_eq!(assertion.priority(), 100);
    
    // Test exact match
    let response_data = ResponseData {
        status_code: 200,
        headers: HashMap::new(),
        body: vec![],
        duration: Duration::from_millis(100),
        metadata: HashMap::new(),
    };
    
    let expected = ExpectedValue::Exact(serde_json::Value::Number(200.into()));
    let result = assertion.execute(&response_data, &expected).await;
    
    assert!(result.success);
    assert!(result.message.contains("Status code matches"));
    
    // Test range match
    let expected_range = ExpectedValue::Range { min: 200.0, max: 299.0 };
    let result_range = assertion.execute(&response_data, &expected_range).await;
    
    assert!(result_range.success);
    assert!(result_range.message.contains("within range"));
    
    // Test mismatch
    let expected_fail = ExpectedValue::Exact(serde_json::Value::Number(404.into()));
    let result_fail = assertion.execute(&response_data, &expected_fail).await;
    
    assert!(!result_fail.success);
    assert!(result_fail.message.contains("mismatch"));
}

#[tokio::test]
async fn test_http_header_assertion() {
    let mut assertion = HttpHeaderAssertion::new("Content-Type".to_string());
    
    let config = PluginConfig {
        settings: HashMap::from([
            ("header_name".to_string(), serde_json::Value::String("Content-Type".to_string())),
        ]),
        environment: "test".to_string(),
    };
    
    let init_result = assertion.initialize(&config).await;
    assert!(init_result.is_ok());
    
    assert_eq!(assertion.assertion_type(), AssertionType::Header);
    assert_eq!(assertion.priority(), 80);
    
    // Test exact match
    let mut headers = HashMap::new();
    headers.insert("Content-Type".to_string(), "application/json".to_string());
    
    let response_data = ResponseData {
        status_code: 200,
        headers,
        body: vec![],
        duration: Duration::from_millis(100),
        metadata: HashMap::new(),
    };
    
    let expected = ExpectedValue::Exact(serde_json::Value::String("application/json".to_string()));
    let result = assertion.execute(&response_data, &expected).await;
    
    assert!(result.success);
    assert!(result.message.contains("matches expected value"));
    
    // Test contains
    let expected_contains = ExpectedValue::Contains("json".to_string());
    let result_contains = assertion.execute(&response_data, &expected_contains).await;
    
    assert!(result_contains.success);
    assert!(result_contains.message.contains("contains expected substring"));
    
    // Test not empty
    let expected_not_empty = ExpectedValue::NotEmpty;
    let result_not_empty = assertion.execute(&response_data, &expected_not_empty).await;
    
    assert!(result_not_empty.success);
    assert!(result_not_empty.message.contains("is not empty"));
}

#[tokio::test]
async fn test_response_time_assertion() {
    let mut assertion = ResponseTimeAssertion::default();
    
    let config = PluginConfig {
        settings: HashMap::new(),
        environment: "test".to_string(),
    };
    
    let init_result = assertion.initialize(&config).await;
    assert!(init_result.is_ok());
    
    assert_eq!(assertion.assertion_type(), AssertionType::ResponseTime);
    assert_eq!(assertion.priority(), 60);
    
    // Test range assertion
    let response_data = ResponseData {
        status_code: 200,
        headers: HashMap::new(),
        body: vec![],
        duration: Duration::from_millis(150),
        metadata: HashMap::new(),
    };
    
    let expected_range = ExpectedValue::Range { min: 100.0, max: 200.0 };
    let result = assertion.execute(&response_data, &expected_range).await;
    
    assert!(result.success);
    assert!(result.message.contains("within range"));
    
    // Test out of range
    let expected_fast = ExpectedValue::Range { min: 10.0, max: 50.0 };
    let result_fail = assertion.execute(&response_data, &expected_fast).await;
    
    assert!(!result_fail.success);
    assert!(result_fail.message.contains("outside range"));
}

#[tokio::test]
async fn test_connection_validation_failure() {
    let mut plugin = RestApiPlugin::new();
    
    let config = PluginConfig {
        settings: HashMap::new(),
        environment: "test".to_string(),
    };
    
    let init_result = plugin.initialize(&config).await;
    assert!(init_result.is_ok());
    
    // Test with invalid URL
    let connection_config = ConnectionConfig {
        base_url: "http://invalid-host-that-does-not-exist.local".to_string(),
        preferred_version: None,
        tls_config: None,
        connection_pool_size: Some(5),
        keep_alive_timeout: Some(Duration::from_secs(30)),
        follow_redirects: true,
        max_redirects: 5,
    };
    
    let validation_result = plugin.validate_connection(&connection_config).await;
    assert!(validation_result.is_err(), "Connection validation should fail for invalid host");
}

#[tokio::test]
async fn test_protocol_request_creation() {
    let request = ProtocolRequest {
        method: HttpMethod::GET,
        url: "https://httpbin.org/get".to_string(),
        headers: HashMap::from([
            ("Accept".to_string(), "application/json".to_string()),
            ("User-Agent".to_string(), "test-client".to_string()),
        ]),
        body: None,
        timeout: Duration::from_secs(10),
        protocol_version: Some(ProtocolVersion::Http1_1),
        tls_config: None,
    };
    
    assert_eq!(request.method, HttpMethod::GET);
    assert_eq!(request.url, "https://httpbin.org/get");
    assert_eq!(request.headers.len(), 2);
    assert!(request.body.is_none());
    assert_eq!(request.timeout, Duration::from_secs(10));
}

#[tokio::test]
async fn test_tls_and_http_version_configuration() {
    let mut plugin = RestApiPlugin::new();
    
    let config = PluginConfig {
        settings: HashMap::from([
            ("tls".to_string(), serde_json::json!({
                "verify_certificates": false,
                "min_tls_version": "1.2"
            })),
            ("http_version".to_string(), serde_json::Value::String("2".to_string())),
        ]),
        environment: "test".to_string(),
    };
    
    let init_result = plugin.initialize(&config).await;
    if let Err(e) = &init_result {
        println!("Initialization error: {:?}", e);
    }
    assert!(init_result.is_ok(), "Plugin initialization with TLS config should succeed");
    
    // Verify the plugin is properly initialized
    assert_eq!(plugin.name(), "rest-api");
    assert!(plugin.supported_versions().contains(&ProtocolVersion::Http2));
}

#[tokio::test]
async fn test_http_version_support() {
    let plugin = RestApiPlugin::new();
    
    let versions = plugin.supported_versions();
    assert!(versions.contains(&ProtocolVersion::Http1_0));
    assert!(versions.contains(&ProtocolVersion::Http1_1));
    assert!(versions.contains(&ProtocolVersion::Http2));
    
    // Test version conversion
    assert_eq!(plugin.convert_version(&ProtocolVersion::Http1_1), reqwest::Version::HTTP_11);
    assert_eq!(plugin.convert_version(&ProtocolVersion::Http2), reqwest::Version::HTTP_2);
}

#[tokio::test]
async fn test_tls_configuration_validation() {
    let mut plugin = RestApiPlugin::new();
    
    // Test with invalid TLS version
    let config = PluginConfig {
        settings: HashMap::from([
            ("tls".to_string(), serde_json::json!({
                "min_tls_version": "invalid_version"
            })),
        ]),
        environment: "test".to_string(),
    };
    
    // Should still initialize successfully but log warnings
    let init_result = plugin.initialize(&config).await;
    assert!(init_result.is_ok(), "Plugin should initialize even with invalid TLS config");
}

#[tokio::test]
async fn test_client_certificate_configuration() {
    let mut plugin = RestApiPlugin::new();
    
    // Test with client certificate configuration (files don't exist, should fail gracefully)
    let config = PluginConfig {
        settings: HashMap::from([
            ("tls".to_string(), serde_json::json!({
                "verify_certificates": true,
                "client_certificate": {
                    "cert_path": "/nonexistent/client.crt",
                    "key_path": "/nonexistent/client.key"
                },
                "ca_certificates": ["/nonexistent/ca.crt"]
            })),
        ]),
        environment: "test".to_string(),
    };
    
    // Should fail to initialize due to missing certificate files
    let init_result = plugin.initialize(&config).await;
    assert!(init_result.is_err(), "Plugin should fail to initialize with missing certificate files");
    
    if let Err(e) = init_result {
        assert!(e.to_string().contains("Failed to read certificate file"), 
                "Error should mention certificate file reading failure");
    }
}

#[tokio::test]
async fn test_tls_version_constraints() {
    let mut plugin = RestApiPlugin::new();
    
    // Test TLS 1.2 minimum version configuration
    let config = PluginConfig {
        settings: HashMap::from([
            ("tls".to_string(), serde_json::json!({
                "min_tls_version": "1.2"
            })),
        ]),
        environment: "test".to_string(),
    };
    
    let init_result = plugin.initialize(&config).await;
    assert!(init_result.is_ok(), "Plugin should initialize with TLS 1.2 minimum version");
}

// Integration test that requires network access - marked as ignored by default
#[tokio::test]
#[ignore]
async fn test_real_http_request() {
    let mut plugin = RestApiPlugin::new();
    
    let config = PluginConfig {
        settings: HashMap::new(),
        environment: "test".to_string(),
    };
    
    let init_result = plugin.initialize(&config).await;
    assert!(init_result.is_ok());
    
    let request = ProtocolRequest {
        method: HttpMethod::GET,
        url: "https://httpbin.org/get".to_string(),
        headers: HashMap::from([
            ("Accept".to_string(), "application/json".to_string()),
        ]),
        body: None,
        timeout: Duration::from_secs(10),
        protocol_version: Some(ProtocolVersion::Http1_1),
        tls_config: None,
    };
    
    let response = plugin.execute_request(request).await;
    assert!(response.is_ok(), "HTTP request should succeed");
    
    let response = response.unwrap();
    assert_eq!(response.status_code, 200);
    assert!(!response.body.is_empty());
    assert!(response.duration > Duration::from_millis(0));
}