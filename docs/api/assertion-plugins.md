# Assertion Plugins

Assertion plugins validate response data and system state. They provide flexible validation capabilities that can be extended for custom business logic.

## AssertionPlugin Trait

```rust
#[async_trait]
pub trait AssertionPlugin: Plugin {
    /// Return the type of assertion this plugin handles
    fn assertion_type(&self) -> AssertionType;
    
    /// Execute the assertion against actual response data
    async fn execute(&self, actual: &ResponseData, expected: &ExpectedValue) -> AssertionResult;
    
    /// Return the priority of this assertion (higher numbers execute first)
    fn priority(&self) -> u8;
}
```

## Assertion Types

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum AssertionType {
    /// HTTP status code assertion
    StatusCode,
    /// HTTP header assertion
    Header,
    /// JSON path assertion
    JsonPath,
    /// XML XPath assertion
    XPath,
    /// Response time assertion
    ResponseTime,
    /// Custom assertion type
    Custom(String),
}
```

## Response Data

```rust
#[derive(Debug, Clone)]
pub struct ResponseData {
    /// HTTP status code
    pub status_code: u16,
    /// Response headers
    pub headers: HashMap<String, String>,
    /// Response body as bytes
    pub body: Vec<u8>,
    /// Response body as string (if valid UTF-8)
    pub body_text: Option<String>,
    /// Parsed JSON (if response is JSON)
    pub json: Option<serde_json::Value>,
    /// Request duration
    pub duration: Duration,
    /// Additional metadata
    pub metadata: HashMap<String, String>,
}
```

## Expected Values

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExpectedValue {
    /// Exact value match
    Exact(serde_json::Value),
    /// Regular expression match (for strings)
    Regex(String),
    /// Numeric range (min, max)
    Range(f64, f64),
    /// List of acceptable values
    OneOf(Vec<serde_json::Value>),
    /// JSONPath expression with expected value
    JsonPath { path: String, value: serde_json::Value },
    /// XPath expression with expected value
    XPath { path: String, value: String },
    /// Custom expected value
    Custom(HashMap<String, serde_json::Value>),
}
```

## Assertion Result

```rust
#[derive(Debug)]
pub struct AssertionResult {
    /// Whether the assertion passed
    pub success: bool,
    /// Human-readable message describing the result
    pub message: String,
    /// The actual value that was tested
    pub actual_value: Option<serde_json::Value>,
    /// The expected value
    pub expected_value: Option<serde_json::Value>,
    /// Additional context or details
    pub details: Option<HashMap<String, serde_json::Value>>,
}
```

## Built-in Assertion Plugins

### HTTP Status Code Assertion

```rust
pub struct HttpStatusAssertion;

#[async_trait]
impl AssertionPlugin for HttpStatusAssertion {
    fn assertion_type(&self) -> AssertionType {
        AssertionType::StatusCode
    }
    
    async fn execute(&self, actual: &ResponseData, expected: &ExpectedValue) -> AssertionResult {
        let actual_status = actual.status_code;
        
        match expected {
            ExpectedValue::Exact(status) => {
                let expected_status = status.as_u64().unwrap() as u16;
                AssertionResult {
                    success: actual_status == expected_status,
                    message: format!("Expected status {}, got {}", expected_status, actual_status),
                    actual_value: Some(json!(actual_status)),
                    expected_value: Some(json!(expected_status)),
                    details: None,
                }
            }
            ExpectedValue::OneOf(statuses) => {
                let expected_statuses: Vec<u16> = statuses.iter()
                    .filter_map(|s| s.as_u64().map(|n| n as u16))
                    .collect();
                    
                AssertionResult {
                    success: expected_statuses.contains(&actual_status),
                    message: format!("Expected status in {:?}, got {}", expected_statuses, actual_status),
                    actual_value: Some(json!(actual_status)),
                    expected_value: Some(json!(expected_statuses)),
                    details: None,
                }
            }
            _ => AssertionResult {
                success: false,
                message: "Invalid expected value type for status code assertion".to_string(),
                actual_value: Some(json!(actual_status)),
                expected_value: None,
                details: None,
            }
        }
    }
    
    fn priority(&self) -> u8 { 100 }
}
```

### JSON Path Assertion

```rust
pub struct JsonPathAssertion {
    json_path_engine: JsonPathEngine,
}

#[async_trait]
impl AssertionPlugin for JsonPathAssertion {
    fn assertion_type(&self) -> AssertionType {
        AssertionType::JsonPath
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
        
        match expected {
            ExpectedValue::JsonPath { path, value } => {
                match self.json_path_engine.query(json_data, path) {
                    Ok(actual_values) => {
                        let matches = actual_values.iter().any(|v| v == value);
                        AssertionResult {
                            success: matches,
                            message: format!("JSONPath '{}' assertion", path),
                            actual_value: Some(json!(actual_values)),
                            expected_value: Some(value.clone()),
                            details: Some(HashMap::from([
                                ("json_path".to_string(), json!(path)),
                            ])),
                        }
                    }
                    Err(e) => AssertionResult {
                        success: false,
                        message: format!("JSONPath query failed: {}", e),
                        actual_value: None,
                        expected_value: Some(value.clone()),
                        details: Some(HashMap::from([
                            ("json_path".to_string(), json!(path)),
                            ("error".to_string(), json!(e.to_string())),
                        ])),
                    }
                }
            }
            _ => AssertionResult {
                success: false,
                message: "Invalid expected value type for JSONPath assertion".to_string(),
                actual_value: None,
                expected_value: None,
                details: None,
            }
        }
    }
    
    fn priority(&self) -> u8 { 80 }
}
```

### Response Time Assertion

```rust
pub struct ResponseTimeAssertion;

#[async_trait]
impl AssertionPlugin for ResponseTimeAssertion {
    fn assertion_type(&self) -> AssertionType {
        AssertionType::ResponseTime
    }
    
    async fn execute(&self, actual: &ResponseData, expected: &ExpectedValue) -> AssertionResult {
        let actual_duration_ms = actual.duration.as_millis() as f64;
        
        match expected {
            ExpectedValue::Range(min, max) => {
                let success = actual_duration_ms >= *min && actual_duration_ms <= *max;
                AssertionResult {
                    success,
                    message: format!("Response time {}ms should be between {}ms and {}ms", 
                                   actual_duration_ms, min, max),
                    actual_value: Some(json!(actual_duration_ms)),
                    expected_value: Some(json!({"min": min, "max": max})),
                    details: None,
                }
            }
            ExpectedValue::Exact(max_time) => {
                let max_ms = max_time.as_f64().unwrap();
                let success = actual_duration_ms <= max_ms;
                AssertionResult {
                    success,
                    message: format!("Response time {}ms should be <= {}ms", 
                                   actual_duration_ms, max_ms),
                    actual_value: Some(json!(actual_duration_ms)),
                    expected_value: Some(json!(max_ms)),
                    details: None,
                }
            }
            _ => AssertionResult {
                success: false,
                message: "Invalid expected value type for response time assertion".to_string(),
                actual_value: Some(json!(actual_duration_ms)),
                expected_value: None,
                details: None,
            }
        }
    }
    
    fn priority(&self) -> u8 { 90 }
}
```

## Implementing Custom Assertion Plugins

### 1. Define the Plugin Structure

```rust
pub struct CustomBusinessLogicAssertion {
    database_client: Arc<DatabaseClient>,
    config: AssertionConfig,
}

#[derive(Debug, Clone)]
pub struct AssertionConfig {
    pub database_url: String,
    pub timeout: Duration,
}
```

### 2. Implement the Plugin Trait

```rust
#[async_trait]
impl Plugin for CustomBusinessLogicAssertion {
    fn name(&self) -> &str {
        "custom-business-logic"
    }
    
    fn version(&self) -> &str {
        "1.0.0"
    }
    
    fn dependencies(&self) -> Vec<String> {
        vec!["database-client".to_string()]
    }
    
    async fn initialize(&mut self, config: &PluginConfig) -> Result<(), PluginError> {
        // Initialize database connection
        let db_url = config.settings.get("database_url")
            .and_then(|v| v.as_str())
            .ok_or_else(|| PluginError::InvalidConfiguration("database_url required".to_string()))?;
            
        self.database_client = Arc::new(DatabaseClient::connect(db_url).await?);
        Ok(())
    }
    
    async fn shutdown(&mut self) -> Result<(), PluginError> {
        // Clean up database connections
        Ok(())
    }
}
```

### 3. Implement the AssertionPlugin Trait

```rust
#[async_trait]
impl AssertionPlugin for CustomBusinessLogicAssertion {
    fn assertion_type(&self) -> AssertionType {
        AssertionType::Custom("business-logic".to_string())
    }
    
    async fn execute(&self, actual: &ResponseData, expected: &ExpectedValue) -> AssertionResult {
        // Extract user ID from response
        let user_id = match &actual.json {
            Some(json) => json.get("user_id").and_then(|v| v.as_str()),
            None => None,
        };
        
        let user_id = match user_id {
            Some(id) => id,
            None => return AssertionResult {
                success: false,
                message: "No user_id found in response".to_string(),
                actual_value: None,
                expected_value: None,
                details: None,
            }
        };
        
        // Validate business logic against database
        match self.validate_user_state(user_id).await {
            Ok(is_valid) => AssertionResult {
                success: is_valid,
                message: format!("Business logic validation for user {}", user_id),
                actual_value: Some(json!({"user_id": user_id, "valid": is_valid})),
                expected_value: Some(json!(true)),
                details: None,
            },
            Err(e) => AssertionResult {
                success: false,
                message: format!("Business logic validation failed: {}", e),
                actual_value: Some(json!({"user_id": user_id})),
                expected_value: Some(json!(true)),
                details: Some(HashMap::from([
                    ("error".to_string(), json!(e.to_string())),
                ])),
            }
        }
    }
    
    fn priority(&self) -> u8 {
        50 // Lower priority than basic assertions
    }
}

impl CustomBusinessLogicAssertion {
    async fn validate_user_state(&self, user_id: &str) -> Result<bool, DatabaseError> {
        // Custom business logic validation
        let user = self.database_client.get_user(user_id).await?;
        Ok(user.is_active && user.email_verified)
    }
}
```

## Assertion Execution Order

Assertions are executed in priority order (highest to lowest):
1. **Priority 100**: Basic HTTP assertions (status code, headers)
2. **Priority 90**: Performance assertions (response time)
3. **Priority 80**: Content assertions (JSON, XML)
4. **Priority 50**: Business logic assertions
5. **Priority 10**: Custom assertions

## Best Practices

### Error Handling
- Provide clear, actionable error messages
- Include both actual and expected values in results
- Handle edge cases gracefully (null values, malformed data)

### Performance
- Cache expensive operations when possible
- Use async operations for external validations
- Implement timeouts for long-running assertions

### Extensibility
- Use the Custom assertion type for domain-specific validations
- Provide configuration options for assertion behavior
- Support multiple expected value formats

### Testing
- Write comprehensive unit tests for assertion logic
- Test edge cases and error conditions
- Provide mock implementations for external dependencies