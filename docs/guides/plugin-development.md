# Plugin Development Guide

This guide will walk you through creating custom plugins for the API Test Runner. Plugins extend the framework's capabilities by adding support for new protocols, assertion types, data sources, authentication methods, and report formats.

## Table of Contents

1. [Plugin Architecture Overview](#plugin-architecture-overview)
2. [Development Environment Setup](#development-environment-setup)
3. [Creating Your First Plugin](#creating-your-first-plugin)
4. [Protocol Plugins](#protocol-plugins)
5. [Assertion Plugins](#assertion-plugins)
6. [Data Source Plugins](#data-source-plugins)
7. [Authentication Plugins](#authentication-plugins)
8. [Reporter Plugins](#reporter-plugins)
9. [Testing Your Plugin](#testing-your-plugin)
10. [Packaging and Distribution](#packaging-and-distribution)
11. [Best Practices](#best-practices)
12. [Advanced Topics](#advanced-topics)

## Plugin Architecture Overview

The API Test Runner uses a trait-based plugin system that allows for runtime loading of dynamic libraries. All plugins must implement the base `Plugin` trait and one or more specialized traits depending on their functionality.

### Plugin Types

- **Protocol Plugins**: Handle communication with specific protocols (HTTP, GraphQL, gRPC, etc.)
- **Assertion Plugins**: Validate response data and system state
- **Data Source Plugins**: Provide test data from various sources
- **Authentication Plugins**: Handle authentication mechanisms
- **Reporter Plugins**: Generate test reports in various formats

### Plugin Lifecycle

1. **Discovery**: Plugins are discovered by scanning the plugin directory
2. **Loading**: Plugin libraries are dynamically loaded using `libloading`
3. **Registration**: Plugins register themselves with the plugin manager
4. **Initialization**: Plugins are initialized with their configuration
5. **Operation**: Plugins handle requests during test execution
6. **Shutdown**: Plugins are gracefully shutdown when the system stops

## Development Environment Setup

### Prerequisites

- Rust 1.70 or later
- Cargo package manager
- Git for version control

### Project Structure

Create a new Rust library project for your plugin:

```bash
cargo new --lib my-custom-plugin
cd my-custom-plugin
```

Update your `Cargo.toml`:

```toml
[package]
name = "my-custom-plugin"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]  # Required for dynamic loading

[dependencies]
api-test-runner = { path = "../api-test-runner" }  # Or from crates.io
async-trait = "0.1"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
tokio = { version = "1.0", features = ["full"] }
thiserror = "1.0"

# Add other dependencies as needed
reqwest = { version = "0.11", features = ["json"] }
```

### Plugin Entry Point

Every plugin must export a function to create an instance:

```rust
// src/lib.rs
use api_test_runner::plugin::Plugin;

mod my_plugin;
pub use my_plugin::MyCustomPlugin;

#[no_mangle]
pub extern "C" fn create_plugin() -> *mut dyn Plugin {
    Box::into_raw(Box::new(MyCustomPlugin::new()))
}
```

## Creating Your First Plugin

Let's create a simple "Hello World" plugin to understand the basics:

```rust
// src/my_plugin.rs
use api_test_runner::plugin::{Plugin, PluginConfig, PluginError};
use async_trait::async_trait;

pub struct MyCustomPlugin {
    name: String,
    initialized: bool,
}

impl MyCustomPlugin {
    pub fn new() -> Self {
        Self {
            name: "my-custom-plugin".to_string(),
            initialized: false,
        }
    }
}

#[async_trait]
impl Plugin for MyCustomPlugin {
    fn name(&self) -> &str {
        &self.name
    }
    
    fn version(&self) -> &str {
        "0.1.0"
    }
    
    fn dependencies(&self) -> Vec<String> {
        vec![] // No dependencies
    }
    
    async fn initialize(&mut self, config: &PluginConfig) -> Result<(), PluginError> {
        println!("Initializing {} plugin", self.name);
        
        // Validate configuration
        if let Some(setting) = config.settings.get("required_setting") {
            println!("Required setting: {}", setting);
        } else {
            return Err(PluginError::InvalidConfiguration(
                "required_setting is missing".to_string()
            ));
        }
        
        self.initialized = true;
        Ok(())
    }
    
    async fn shutdown(&mut self) -> Result<(), PluginError> {
        println!("Shutting down {} plugin", self.name);
        self.initialized = false;
        Ok(())
    }
}
```

Build your plugin:

```bash
cargo build --release
```

The compiled plugin will be at `target/release/libmy_custom_plugin.so` (Linux) or `target/release/my_custom_plugin.dll` (Windows).

## Protocol Plugins

Protocol plugins handle communication with specific protocols. Here's an example of a simple HTTP client plugin:

```rust
// src/http_plugin.rs
use api_test_runner::plugin::{
    Plugin, PluginConfig, PluginError,
    ProtocolPlugin, ProtocolRequest, ProtocolResponse, ProtocolError,
    HttpMethod, ProtocolVersion
};
use async_trait::async_trait;
use reqwest::Client;
use std::collections::HashMap;
use std::time::{Duration, Instant};

pub struct SimpleHttpPlugin {
    client: Option<Client>,
}

impl SimpleHttpPlugin {
    pub fn new() -> Self {
        Self { client: None }
    }
}

#[async_trait]
impl Plugin for SimpleHttpPlugin {
    fn name(&self) -> &str {
        "simple-http"
    }
    
    fn version(&self) -> &str {
        "1.0.0"
    }
    
    fn dependencies(&self) -> Vec<String> {
        vec![]
    }
    
    async fn initialize(&mut self, config: &PluginConfig) -> Result<(), PluginError> {
        let timeout = config.settings.get("timeout")
            .and_then(|v| v.as_u64())
            .map(Duration::from_secs)
            .unwrap_or(Duration::from_secs(30));
            
        self.client = Some(
            Client::builder()
                .timeout(timeout)
                .build()
                .map_err(|e| PluginError::InitializationFailed(e.to_string()))?
        );
        
        Ok(())
    }
    
    async fn shutdown(&mut self) -> Result<(), PluginError> {
        self.client = None;
        Ok(())
    }
}

#[async_trait]
impl ProtocolPlugin for SimpleHttpPlugin {
    async fn execute_request(&self, request: ProtocolRequest) -> Result<ProtocolResponse, ProtocolError> {
        let client = self.client.as_ref()
            .ok_or_else(|| ProtocolError::ProtocolError("Plugin not initialized".to_string()))?;
            
        let method = match request.method {
            HttpMethod::GET => reqwest::Method::GET,
            HttpMethod::POST => reqwest::Method::POST,
            HttpMethod::PUT => reqwest::Method::PUT,
            HttpMethod::DELETE => reqwest::Method::DELETE,
            HttpMethod::PATCH => reqwest::Method::PATCH,
            HttpMethod::HEAD => reqwest::Method::HEAD,
            HttpMethod::OPTIONS => reqwest::Method::OPTIONS,
            _ => return Err(ProtocolError::ProtocolError("Unsupported HTTP method".to_string())),
        };
        
        let mut req_builder = client.request(method, &request.url);
        
        // Add headers
        for (key, value) in &request.headers {
            req_builder = req_builder.header(key, value);
        }
        
        // Add body if present
        if let Some(body) = &request.body {
            req_builder = req_builder.body(body.clone());
        }
        
        // Set timeout
        req_builder = req_builder.timeout(request.timeout);
        
        let start_time = Instant::now();
        let response = req_builder.send().await
            .map_err(|e| ProtocolError::ConnectionFailed(e.to_string()))?;
        let duration = start_time.elapsed();
        
        let status_code = response.status().as_u16();
        let headers: HashMap<String, String> = response.headers()
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
            .collect();
            
        let body = response.bytes().await
            .map_err(|e| ProtocolError::ProtocolError(e.to_string()))?
            .to_vec();
            
        Ok(ProtocolResponse {
            status_code,
            headers,
            body,
            duration,
            metadata: HashMap::new(),
        })
    }
    
    async fn validate_connection(&self, config: &ConnectionConfig) -> Result<(), ProtocolError> {
        let client = self.client.as_ref()
            .ok_or_else(|| ProtocolError::ProtocolError("Plugin not initialized".to_string()))?;
            
        // Perform a HEAD request to validate connectivity
        client.head(&config.base_url)
            .timeout(Duration::from_secs(5))
            .send()
            .await
            .map_err(|e| ProtocolError::ConnectionFailed(e.to_string()))?;
            
        Ok(())
    }
    
    fn supported_schemes(&self) -> Vec<String> {
        vec!["http".to_string(), "https".to_string()]
    }
    
    fn supported_versions(&self) -> Vec<ProtocolVersion> {
        vec![ProtocolVersion::Http1_1, ProtocolVersion::Http2]
    }
}
```

## Assertion Plugins

Assertion plugins validate response data. Here's an example of a custom business logic assertion:

```rust
// src/business_assertion.rs
use api_test_runner::plugin::{
    Plugin, PluginConfig, PluginError,
    AssertionPlugin, AssertionType, ResponseData, ExpectedValue, AssertionResult
};
use async_trait::async_trait;
use serde_json::json;
use std::collections::HashMap;

pub struct BusinessLogicAssertion {
    rules: HashMap<String, BusinessRule>,
}

#[derive(Debug, Clone)]
struct BusinessRule {
    field: String,
    validation_type: ValidationType,
    parameters: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone)]
enum ValidationType {
    EmailFormat,
    PhoneFormat,
    CreditCardFormat,
    CustomRegex(String),
}

impl BusinessLogicAssertion {
    pub fn new() -> Self {
        Self {
            rules: HashMap::new(),
        }
    }
    
    fn validate_email(&self, email: &str) -> bool {
        let email_regex = regex::Regex::new(
            r"^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$"
        ).unwrap();
        email_regex.is_match(email)
    }
    
    fn validate_phone(&self, phone: &str) -> bool {
        let phone_regex = regex::Regex::new(
            r"^\+?[1-9]\d{1,14}$"
        ).unwrap();
        phone_regex.is_match(phone)
    }
    
    fn validate_credit_card(&self, card: &str) -> bool {
        // Simplified Luhn algorithm check
        let digits: Vec<u32> = card.chars()
            .filter(|c| c.is_ascii_digit())
            .map(|c| c.to_digit(10).unwrap())
            .collect();
            
        if digits.len() < 13 || digits.len() > 19 {
            return false;
        }
        
        let checksum: u32 = digits.iter().rev().enumerate()
            .map(|(i, &digit)| {
                if i % 2 == 1 {
                    let doubled = digit * 2;
                    if doubled > 9 { doubled - 9 } else { doubled }
                } else {
                    digit
                }
            })
            .sum();
            
        checksum % 10 == 0
    }
}

#[async_trait]
impl Plugin for BusinessLogicAssertion {
    fn name(&self) -> &str {
        "business-logic-assertion"
    }
    
    fn version(&self) -> &str {
        "1.0.0"
    }
    
    fn dependencies(&self) -> Vec<String> {
        vec![]
    }
    
    async fn initialize(&mut self, config: &PluginConfig) -> Result<(), PluginError> {
        // Load business rules from configuration
        if let Some(rules_config) = config.settings.get("rules") {
            if let Some(rules_array) = rules_config.as_array() {
                for rule_config in rules_array {
                    if let Some(rule_obj) = rule_config.as_object() {
                        let name = rule_obj.get("name")
                            .and_then(|v| v.as_str())
                            .ok_or_else(|| PluginError::InvalidConfiguration("Rule name required".to_string()))?;
                            
                        let field = rule_obj.get("field")
                            .and_then(|v| v.as_str())
                            .ok_or_else(|| PluginError::InvalidConfiguration("Rule field required".to_string()))?;
                            
                        let validation_type = match rule_obj.get("type").and_then(|v| v.as_str()) {
                            Some("email") => ValidationType::EmailFormat,
                            Some("phone") => ValidationType::PhoneFormat,
                            Some("credit_card") => ValidationType::CreditCardFormat,
                            Some("regex") => {
                                let pattern = rule_obj.get("pattern")
                                    .and_then(|v| v.as_str())
                                    .ok_or_else(|| PluginError::InvalidConfiguration("Regex pattern required".to_string()))?;
                                ValidationType::CustomRegex(pattern.to_string())
                            }
                            _ => return Err(PluginError::InvalidConfiguration("Invalid validation type".to_string())),
                        };
                        
                        let rule = BusinessRule {
                            field: field.to_string(),
                            validation_type,
                            parameters: HashMap::new(),
                        };
                        
                        self.rules.insert(name.to_string(), rule);
                    }
                }
            }
        }
        
        Ok(())
    }
    
    async fn shutdown(&mut self) -> Result<(), PluginError> {
        self.rules.clear();
        Ok(())
    }
}

#[async_trait]
impl AssertionPlugin for BusinessLogicAssertion {
    fn assertion_type(&self) -> AssertionType {
        AssertionType::Custom("business-logic".to_string())
    }
    
    async fn execute(&self, actual: &ResponseData, expected: &ExpectedValue) -> AssertionResult {
        let json_data = match &actual.json {
            Some(json) => json,
            None => return AssertionResult {
                success: false,
                message: "Response is not valid JSON".to_string(),
                actual_value: None,
                expected_value: None,
                details: None,
            }
        };
        
        if let ExpectedValue::Custom(params) = expected {
            if let Some(rule_name) = params.get("rule").and_then(|v| v.as_str()) {
                if let Some(rule) = self.rules.get(rule_name) {
                    if let Some(field_value) = json_data.get(&rule.field).and_then(|v| v.as_str()) {
                        let is_valid = match &rule.validation_type {
                            ValidationType::EmailFormat => self.validate_email(field_value),
                            ValidationType::PhoneFormat => self.validate_phone(field_value),
                            ValidationType::CreditCardFormat => self.validate_credit_card(field_value),
                            ValidationType::CustomRegex(pattern) => {
                                if let Ok(regex) = regex::Regex::new(pattern) {
                                    regex.is_match(field_value)
                                } else {
                                    false
                                }
                            }
                        };
                        
                        return AssertionResult {
                            success: is_valid,
                            message: format!("Business logic validation for field '{}' using rule '{}'", rule.field, rule_name),
                            actual_value: Some(json!(field_value)),
                            expected_value: Some(json!(true)),
                            details: Some(HashMap::from([
                                ("rule".to_string(), json!(rule_name)),
                                ("field".to_string(), json!(rule.field)),
                            ])),
                        };
                    }
                }
            }
        }
        
        AssertionResult {
            success: false,
            message: "Invalid business logic assertion configuration".to_string(),
            actual_value: None,
            expected_value: None,
            details: None,
        }
    }
    
    fn priority(&self) -> u8 {
        50 // Medium priority
    }
}
```

## Data Source Plugins

Data source plugins provide test data from various sources. Here's an example of a REST API data source:

```rust
// src/api_data_source.rs
use api_test_runner::plugin::{
    Plugin, PluginConfig, PluginError,
    DataSourcePlugin, DataSourceType, DataSourceConfig, DataQuery, DataSet, DataRow, HealthStatus, DataSourceError
};
use async_trait::async_trait;
use reqwest::Client;
use serde_json::json;
use std::collections::HashMap;
use std::time::{Duration, Instant};

pub struct ApiDataSource {
    client: Option<Client>,
    base_url: String,
    auth_token: Option<String>,
}

impl ApiDataSource {
    pub fn new() -> Self {
        Self {
            client: None,
            base_url: String::new(),
            auth_token: None,
        }
    }
}

#[async_trait]
impl Plugin for ApiDataSource {
    fn name(&self) -> &str {
        "api-data-source"
    }
    
    fn version(&self) -> &str {
        "1.0.0"
    }
    
    fn dependencies(&self) -> Vec<String> {
        vec![]
    }
    
    async fn initialize(&mut self, config: &PluginConfig) -> Result<(), PluginError> {
        let timeout = config.settings.get("timeout")
            .and_then(|v| v.as_u64())
            .map(Duration::from_secs)
            .unwrap_or(Duration::from_secs(30));
            
        self.client = Some(
            Client::builder()
                .timeout(timeout)
                .build()
                .map_err(|e| PluginError::InitializationFailed(e.to_string()))?
        );
        
        Ok(())
    }
    
    async fn shutdown(&mut self) -> Result<(), PluginError> {
        self.client = None;
        Ok(())
    }
}

#[async_trait]
impl DataSourcePlugin for ApiDataSource {
    fn source_type(&self) -> DataSourceType {
        DataSourceType::Api
    }
    
    async fn connect(&mut self, config: &DataSourceConfig) -> Result<(), DataSourceError> {
        self.base_url = config.connection.clone();
        
        // Extract auth token from credentials
        if let Some(credentials) = &config.credentials {
            self.auth_token = credentials.token.clone();
        }
        
        Ok(())
    }
    
    async fn fetch_data(&self, query: &DataQuery) -> Result<DataSet, DataSourceError> {
        let client = self.client.as_ref()
            .ok_or_else(|| DataSourceError::NotConnected)?;
            
        let url = if query.query.starts_with("http") {
            query.query.clone()
        } else {
            format!("{}/{}", self.base_url.trim_end_matches('/'), query.query.trim_start_matches('/'))
        };
        
        let mut request = client.get(&url);
        
        // Add authentication if available
        if let Some(token) = &self.auth_token {
            request = request.bearer_auth(token);
        }
        
        // Add query parameters
        for (key, value) in &query.parameters {
            if let Some(param_value) = value.as_str() {
                request = request.query(&[(key, param_value)]);
            }
        }
        
        // Add limit and offset as query parameters
        if let Some(limit) = query.limit {
            request = request.query(&[("limit", limit.to_string())]);
        }
        if let Some(offset) = query.offset {
            request = request.query(&[("offset", offset.to_string())]);
        }
        
        let response = request.send().await
            .map_err(|e| DataSourceError::QueryFailed(e.to_string()))?;
            
        if !response.status().is_success() {
            return Err(DataSourceError::QueryFailed(
                format!("HTTP {}: {}", response.status(), response.status().canonical_reason().unwrap_or("Unknown"))
            ));
        }
        
        let json_data: serde_json::Value = response.json().await
            .map_err(|e| DataSourceError::QueryFailed(e.to_string()))?;
            
        // Convert JSON response to DataSet
        match json_data {
            serde_json::Value::Array(items) => {
                let mut columns = std::collections::HashSet::new();
                let mut rows = Vec::new();
                
                // First pass: collect all possible column names
                for item in &items {
                    if let serde_json::Value::Object(obj) = item {
                        for key in obj.keys() {
                            columns.insert(key.clone());
                        }
                    }
                }
                
                let columns: Vec<String> = columns.into_iter().collect();
                
                // Second pass: create rows
                for item in items {
                    if let serde_json::Value::Object(obj) = item {
                        let mut values = HashMap::new();
                        for column in &columns {
                            let value = obj.get(column).cloned().unwrap_or(json!(null));
                            values.insert(column.clone(), value);
                        }
                        rows.push(DataRow { values });
                    }
                }
                
                Ok(DataSet {
                    columns,
                    rows,
                    total_count: None,
                    metadata: HashMap::new(),
                })
            }
            serde_json::Value::Object(obj) => {
                // Single object response
                let columns: Vec<String> = obj.keys().cloned().collect();
                let values: HashMap<String, serde_json::Value> = obj.into_iter().collect();
                let rows = vec![DataRow { values }];
                
                Ok(DataSet {
                    columns,
                    rows,
                    total_count: Some(1),
                    metadata: HashMap::new(),
                })
            }
            _ => Err(DataSourceError::QueryFailed("Response is not a JSON object or array".to_string()))
        }
    }
    
    async fn health_check(&self) -> Result<HealthStatus, DataSourceError> {
        let start = Instant::now();
        
        let client = self.client.as_ref()
            .ok_or_else(|| DataSourceError::NotConnected)?;
            
        let health_url = format!("{}/health", self.base_url.trim_end_matches('/'));
        let mut request = client.get(&health_url);
        
        if let Some(token) = &self.auth_token {
            request = request.bearer_auth(token);
        }
        
        let healthy = match request.send().await {
            Ok(response) => response.status().is_success(),
            Err(_) => false,
        };
        
        let response_time = start.elapsed();
        
        Ok(HealthStatus {
            healthy,
            response_time,
            details: HashMap::from([
                ("base_url".to_string(), json!(self.base_url)),
                ("health_endpoint".to_string(), json!(health_url)),
            ]),
        })
    }
    
    async fn disconnect(&mut self) -> Result<(), DataSourceError> {
        self.auth_token = None;
        Ok(())
    }
}
```

## Authentication Plugins

Authentication plugins handle various authentication mechanisms. Here's an example of a custom API key authentication plugin:

```rust
// src/api_key_auth.rs
use api_test_runner::plugin::{
    Plugin, PluginConfig, PluginError,
    AuthPlugin, AuthRequest, AuthResponse, AuthError, AuthToken
};
use async_trait::async_trait;
use std::collections::HashMap;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub struct ApiKeyAuthPlugin {
    api_keys: HashMap<String, ApiKeyConfig>,
}

#[derive(Debug, Clone)]
struct ApiKeyConfig {
    key: String,
    header_name: String,
    prefix: Option<String>,
}

impl ApiKeyAuthPlugin {
    pub fn new() -> Self {
        Self {
            api_keys: HashMap::new(),
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
    
    async fn initialize(&mut self, config: &PluginConfig) -> Result<(), PluginError> {
        if let Some(keys_config) = config.settings.get("api_keys") {
            if let Some(keys_obj) = keys_config.as_object() {
                for (name, key_config) in keys_obj {
                    if let Some(config_obj) = key_config.as_object() {
                        let key = config_obj.get("key")
                            .and_then(|v| v.as_str())
                            .ok_or_else(|| PluginError::InvalidConfiguration("API key required".to_string()))?;
                            
                        let header_name = config_obj.get("header_name")
                            .and_then(|v| v.as_str())
                            .unwrap_or("X-API-Key");
                            
                        let prefix = config_obj.get("prefix")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string());
                            
                        let api_key_config = ApiKeyConfig {
                            key: key.to_string(),
                            header_name: header_name.to_string(),
                            prefix,
                        };
                        
                        self.api_keys.insert(name.clone(), api_key_config);
                    }
                }
            }
        }
        
        Ok(())
    }
    
    async fn shutdown(&mut self) -> Result<(), PluginError> {
        self.api_keys.clear();
        Ok(())
    }
}

#[async_trait]
impl AuthPlugin for ApiKeyAuthPlugin {
    async fn authenticate(&self, request: &AuthRequest) -> Result<AuthResponse, AuthError> {
        let config_name = request.config.get("config_name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AuthError::InvalidConfiguration("config_name required".to_string()))?;
            
        let api_key_config = self.api_keys.get(config_name)
            .ok_or_else(|| AuthError::InvalidConfiguration(format!("API key config '{}' not found", config_name)))?;
            
        let header_value = if let Some(prefix) = &api_key_config.prefix {
            format!("{} {}", prefix, api_key_config.key)
        } else {
            api_key_config.key.clone()
        };
        
        let mut headers = HashMap::new();
        headers.insert(api_key_config.header_name.clone(), header_value);
        
        // API keys typically don't expire, but we'll set a far future expiration
        let expires_at = SystemTime::now() + Duration::from_secs(365 * 24 * 60 * 60); // 1 year
        
        let token = AuthToken {
            token_type: "api_key".to_string(),
            access_token: api_key_config.key.clone(),
            refresh_token: None,
            expires_at: Some(expires_at),
            scope: None,
        };
        
        Ok(AuthResponse {
            token,
            headers,
            metadata: HashMap::new(),
        })
    }
    
    async fn refresh_token(&self, _token: &AuthToken) -> Result<AuthToken, AuthError> {
        // API keys don't need refreshing
        Err(AuthError::RefreshNotSupported)
    }
    
    async fn validate_token(&self, token: &AuthToken) -> Result<bool, AuthError> {
        // Simple validation - check if token exists in our configs
        Ok(self.api_keys.values().any(|config| config.key == token.access_token))
    }
    
    fn supported_auth_types(&self) -> Vec<String> {
        vec!["api_key".to_string()]
    }
}
```

## Testing Your Plugin

Create comprehensive tests for your plugin:

```rust
// tests/integration_tests.rs
use my_custom_plugin::MyCustomPlugin;
use api_test_runner::plugin::{Plugin, PluginConfig};
use std::collections::HashMap;
use serde_json::json;

#[tokio::test]
async fn test_plugin_initialization() {
    let mut plugin = MyCustomPlugin::new();
    
    let config = PluginConfig {
        settings: HashMap::from([
            ("required_setting".to_string(), json!("test_value")),
        ]),
        environment: "test".to_string(),
    };
    
    let result = plugin.initialize(&config).await;
    assert!(result.is_ok());
    
    assert_eq!(plugin.name(), "my-custom-plugin");
    assert_eq!(plugin.version(), "0.1.0");
}

#[tokio::test]
async fn test_plugin_missing_config() {
    let mut plugin = MyCustomPlugin::new();
    
    let config = PluginConfig {
        settings: HashMap::new(),
        environment: "test".to_string(),
    };
    
    let result = plugin.initialize(&config).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_plugin_shutdown() {
    let mut plugin = MyCustomPlugin::new();
    
    let config = PluginConfig {
        settings: HashMap::from([
            ("required_setting".to_string(), json!("test_value")),
        ]),
        environment: "test".to_string(),
    };
    
    plugin.initialize(&config).await.unwrap();
    let result = plugin.shutdown().await;
    assert!(result.is_ok());
}
```

Run your tests:

```bash
cargo test
```

## Packaging and Distribution

### Building for Release

Build your plugin for release:

```bash
cargo build --release
```

### Creating a Plugin Package

Create a plugin package with metadata:

```yaml
# plugin.yaml
name: "my-custom-plugin"
version: "0.1.0"
description: "A custom plugin for the API Test Runner"
author: "Your Name <your.email@example.com>"
license: "MIT"
homepage: "https://github.com/yourusername/my-custom-plugin"

# Plugin binary information
binary:
  linux: "target/release/libmy_custom_plugin.so"
  windows: "target/release/my_custom_plugin.dll"
  macos: "target/release/libmy_custom_plugin.dylib"

# Dependencies
dependencies:
  api-test-runner: ">=1.0.0"

# Configuration schema
configuration:
  type: "object"
  properties:
    required_setting:
      type: "string"
      description: "A required configuration setting"
    optional_setting:
      type: "integer"
      description: "An optional configuration setting"
      default: 42
  required:
    - "required_setting"

# Usage examples
examples:
  - name: "Basic Usage"
    description: "Basic plugin configuration"
    configuration:
      required_setting: "example_value"
      optional_setting: 100
```

### Distribution

You can distribute your plugin in several ways:

1. **GitHub Releases**: Upload the compiled binaries and plugin.yaml
2. **Plugin Registry**: Submit to the official plugin registry
3. **Direct Distribution**: Share the compiled library files directly

## Best Practices

### Code Quality

1. **Use proper error handling** with the provided error types
2. **Implement comprehensive logging** for debugging
3. **Follow Rust best practices** for memory safety and performance
4. **Use async/await** properly for non-blocking operations
5. **Write comprehensive tests** for all functionality

### Configuration

1. **Validate configuration** during initialization
2. **Provide sensible defaults** for optional settings
3. **Document all configuration options** clearly
4. **Use environment variables** for sensitive data
5. **Support multiple environments** (dev, staging, prod)

### Performance

1. **Use connection pooling** for network resources
2. **Implement proper caching** where appropriate
3. **Handle timeouts** gracefully
4. **Optimize for concurrent usage**
5. **Monitor resource usage**

### Security

1. **Validate all inputs** to prevent injection attacks
2. **Handle sensitive data** securely (don't log secrets)
3. **Use secure communication** (TLS) for network operations
4. **Implement proper authentication** and authorization
5. **Follow security best practices** for your domain

### Documentation

1. **Document all public APIs** with clear examples
2. **Provide usage examples** for common scenarios
3. **Include troubleshooting guides** for common issues
4. **Keep documentation up to date** with code changes
5. **Use clear, concise language**

## Advanced Topics

### Plugin Communication

Plugins can communicate with each other through the event system:

```rust
use api_test_runner::event::{EventBus, Event, EventType};

// In your plugin
async fn handle_event(&self, event: &Event) -> Result<(), PluginError> {
    match event.event_type {
        EventType::TestStarted => {
            // Handle test started event
            println!("Test started: {}", event.test_case_id);
        }
        EventType::TestCompleted => {
            // Handle test completed event
            println!("Test completed: {}", event.test_case_id);
        }
        _ => {}
    }
    Ok(())
}
```

### Custom Metrics

Plugins can expose custom metrics:

```rust
use api_test_runner::metrics::{MetricsCollector, Metric, MetricType};

// In your plugin
async fn record_custom_metric(&self, metrics: &MetricsCollector) {
    let metric = Metric {
        name: "custom_plugin_operations".to_string(),
        metric_type: MetricType::Counter,
        value: 1.0,
        labels: HashMap::from([
            ("plugin".to_string(), self.name().to_string()),
            ("operation".to_string(), "custom_operation".to_string()),
        ]),
        timestamp: SystemTime::now(),
    };
    
    metrics.record(metric).await;
}
```

### Plugin Hooks

Implement hooks for lifecycle events:

```rust
#[async_trait]
pub trait PluginHooks {
    async fn before_test_execution(&self, test_case: &TestCase) -> Result<(), PluginError> {
        Ok(())
    }
    
    async fn after_test_execution(&self, test_case: &TestCase, result: &TestResult) -> Result<(), PluginError> {
        Ok(())
    }
    
    async fn before_assertion(&self, assertion: &AssertionDefinition) -> Result<(), PluginError> {
        Ok(())
    }
    
    async fn after_assertion(&self, assertion: &AssertionDefinition, result: &AssertionResult) -> Result<(), PluginError> {
        Ok(())
    }
}
```

### Dynamic Configuration

Support dynamic configuration updates:

```rust
#[async_trait]
pub trait DynamicConfiguration {
    async fn update_configuration(&mut self, config: &PluginConfig) -> Result<(), PluginError>;
    
    async fn get_configuration_schema(&self) -> Result<serde_json::Value, PluginError>;
    
    async fn validate_configuration(&self, config: &PluginConfig) -> Result<Vec<String>, PluginError>;
}
```

This completes the comprehensive plugin development guide. With these examples and best practices, you should be able to create powerful, reliable plugins that extend the API Test Runner's capabilities.