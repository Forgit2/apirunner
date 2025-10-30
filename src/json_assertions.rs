use async_trait::async_trait;
use jsonpath_rust::JsonPathQuery;
use serde_json::{Value as JsonValue};

use crate::error::PluginError;
use crate::plugin::{
    AssertionPlugin, AssertionResult, AssertionType, ExpectedValue, Plugin, PluginConfig,
    ResponseData,
};

/// JSONPath Assertion Plugin for validating JSON responses using JSONPath expressions
#[derive(Debug, Default)]
pub struct JsonPathAssertion {
    json_path: String,
}

impl JsonPathAssertion {
    pub fn new(json_path: String) -> Self {
        Self { json_path }
    }

    /// Parse response body as JSON
    fn parse_json_body(&self, body: &[u8]) -> Result<JsonValue, String> {
        let body_str = std::str::from_utf8(body)
            .map_err(|_| "Response body is not valid UTF-8".to_string())?;
        
        serde_json::from_str(body_str)
            .map_err(|e| format!("Failed to parse JSON: {}", e))
    }

    /// Execute JSONPath query on JSON data
    fn execute_jsonpath(&self, json_data: &JsonValue, path: &str) -> Result<JsonValue, String> {
        match json_data.clone().path(path) {
            Ok(result) => {
                // JSONPath library returns an array of results, extract single value if array has one element
                match &result {
                    JsonValue::Array(arr) if arr.len() == 1 => Ok(arr[0].clone()),
                    _ => Ok(result),
                }
            },
            Err(e) => Err(format!("JSONPath query failed: {}", e)),
        }
    }

    /// Compare actual and expected values with type-aware comparison
    fn compare_values(&self, actual: &JsonValue, expected: &ExpectedValue) -> (bool, String) {
        match expected {
            ExpectedValue::Exact(expected_val) => {
                let success = actual == expected_val;
                let message = if success {
                    format!("JSONPath result matches expected value: {}", expected_val)
                } else {
                    format!(
                        "JSONPath result mismatch: expected {}, got {}",
                        expected_val, actual
                    )
                };
                (success, message)
            }
            ExpectedValue::Contains(substring) => {
                if let Some(actual_str) = actual.as_str() {
                    let success = actual_str.contains(substring);
                    let message = if success {
                        format!("JSONPath result '{}' contains '{}'", actual_str, substring)
                    } else {
                        format!("JSONPath result '{}' does not contain '{}'", actual_str, substring)
                    };
                    (success, message)
                } else {
                    (false, format!("JSONPath result is not a string, cannot check contains: {}", actual))
                }
            }
            ExpectedValue::Pattern(regex_pattern) => {
                if let Some(actual_str) = actual.as_str() {
                    match regex::Regex::new(regex_pattern) {
                        Ok(regex) => {
                            let success = regex.is_match(actual_str);
                            let message = if success {
                                format!("JSONPath result '{}' matches pattern '{}'", actual_str, regex_pattern)
                            } else {
                                format!("JSONPath result '{}' does not match pattern '{}'", actual_str, regex_pattern)
                            };
                            (success, message)
                        }
                        Err(e) => (false, format!("Invalid regex pattern '{}': {}", regex_pattern, e))
                    }
                } else {
                    (false, format!("JSONPath result is not a string, cannot match pattern: {}", actual))
                }
            }
            ExpectedValue::NotEmpty => {
                let success = match actual {
                    JsonValue::Null => false,
                    JsonValue::String(s) => !s.is_empty(),
                    JsonValue::Array(arr) => !arr.is_empty(),
                    JsonValue::Object(obj) => !obj.is_empty(),
                    _ => true, // Numbers and booleans are considered non-empty
                };
                let message = if success {
                    "JSONPath result is not empty".to_string()
                } else {
                    "JSONPath result is empty".to_string()
                };
                (success, message)
            }
            ExpectedValue::Range { min, max } => {
                if let Some(num) = actual.as_f64() {
                    let success = num >= *min && num <= *max;
                    let message = if success {
                        format!("JSONPath result {} is within range [{}, {}]", num, min, max)
                    } else {
                        format!("JSONPath result {} is outside range [{}, {}]", num, min, max)
                    };
                    (success, message)
                } else {
                    (false, format!("JSONPath result is not a number, cannot check range: {}", actual))
                }
            }
        }
    }
}

#[async_trait]
impl Plugin for JsonPathAssertion {
    fn name(&self) -> &str {
        "jsonpath-assertion"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn dependencies(&self) -> Vec<String> {
        vec![]
    }

    async fn initialize(&mut self, config: &PluginConfig) -> Result<(), PluginError> {
        if let Some(json_path) = config.settings.get("json_path") {
            if let Some(path) = json_path.as_str() {
                self.json_path = path.to_string();
            }
        }

        if self.json_path.is_empty() {
            return Err(PluginError::InitializationFailed(
                "JSONPath expression is required".to_string(),
            ));
        }

        Ok(())
    }

    async fn shutdown(&mut self) -> Result<(), PluginError> {
        Ok(())
    }
}

#[async_trait]
impl AssertionPlugin for JsonPathAssertion {
    fn assertion_type(&self) -> AssertionType {
        AssertionType::JsonPath
    }

    async fn execute(&self, actual: &ResponseData, expected: &ExpectedValue) -> AssertionResult {
        // Parse JSON from response body
        let json_data = match self.parse_json_body(&actual.body) {
            Ok(data) => data,
            Err(error) => {
                return AssertionResult {
                    success: false,
                    message: error,
                    actual_value: None,
                    expected_value: Some(serde_json::to_value(expected).unwrap_or(JsonValue::Null)),
                };
            }
        };

        // Execute JSONPath query
        let jsonpath_result = match self.execute_jsonpath(&json_data, &self.json_path) {
            Ok(result) => result,
            Err(error) => {
                return AssertionResult {
                    success: false,
                    message: error,
                    actual_value: Some(json_data),
                    expected_value: Some(serde_json::to_value(expected).unwrap_or(JsonValue::Null)),
                };
            }
        };

        // Compare with expected value
        let (success, message) = self.compare_values(&jsonpath_result, expected);

        AssertionResult {
            success,
            message,
            actual_value: Some(jsonpath_result),
            expected_value: Some(serde_json::to_value(expected).unwrap_or(JsonValue::Null)),
        }
    }

    fn priority(&self) -> u8 {
        80 // Medium-high priority for JSON validation
    }
}

/// JSON Schema Assertion Plugin for validating JSON responses against JSON Schema
#[derive(Debug, Default)]
pub struct JsonSchemaAssertion {
    schema: Option<JsonValue>,
}

impl JsonSchemaAssertion {
    pub fn new() -> Self {
        Self { schema: None }
    }

    pub fn with_schema(schema: JsonValue) -> Self {
        Self {
            schema: Some(schema),
        }
    }

    /// Parse response body as JSON
    fn parse_json_body(&self, body: &[u8]) -> Result<JsonValue, String> {
        let body_str = std::str::from_utf8(body)
            .map_err(|_| "Response body is not valid UTF-8".to_string())?;
        
        serde_json::from_str(body_str)
            .map_err(|e| format!("Failed to parse JSON: {}", e))
    }

    /// Basic JSON schema validation (simplified implementation)
    /// In a production system, you would use a proper JSON schema validation library
    fn validate_against_schema(&self, data: &JsonValue, schema: &JsonValue) -> Result<(), String> {
        // This is a simplified schema validation - in production use jsonschema crate
        match schema.get("type") {
            Some(JsonValue::String(expected_type)) => {
                let actual_type = match data {
                    JsonValue::Null => "null",
                    JsonValue::Bool(_) => "boolean",
                    JsonValue::Number(_) => "number",
                    JsonValue::String(_) => "string",
                    JsonValue::Array(_) => "array",
                    JsonValue::Object(_) => "object",
                };

                if actual_type != expected_type {
                    return Err(format!(
                        "Type mismatch: expected {}, got {}",
                        expected_type, actual_type
                    ));
                }
            }
            _ => {}
        }

        // Validate required properties for objects
        if let (JsonValue::Object(data_obj), JsonValue::Object(schema_obj)) = (data, schema) {
            if let Some(JsonValue::Array(required)) = schema_obj.get("required") {
                for req_field in required {
                    if let JsonValue::String(field_name) = req_field {
                        if !data_obj.contains_key(field_name) {
                            return Err(format!("Required field '{}' is missing", field_name));
                        }
                    }
                }
            }

            // Validate properties
            if let Some(JsonValue::Object(properties)) = schema_obj.get("properties") {
                for (prop_name, prop_schema) in properties {
                    if let Some(prop_value) = data_obj.get(prop_name) {
                        self.validate_against_schema(prop_value, prop_schema)?;
                    }
                }
            }
        }

        // Validate array items
        if let (JsonValue::Array(data_arr), JsonValue::Object(schema_obj)) = (data, schema) {
            if let Some(items_schema) = schema_obj.get("items") {
                for item in data_arr {
                    self.validate_against_schema(item, items_schema)?;
                }
            }
        }

        Ok(())
    }
}

#[async_trait]
impl Plugin for JsonSchemaAssertion {
    fn name(&self) -> &str {
        "json-schema-assertion"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn dependencies(&self) -> Vec<String> {
        vec![]
    }

    async fn initialize(&mut self, config: &PluginConfig) -> Result<(), PluginError> {
        if let Some(schema_value) = config.settings.get("schema") {
            self.schema = Some(schema_value.clone());
        }

        if self.schema.is_none() {
            return Err(PluginError::InitializationFailed(
                "JSON schema is required".to_string(),
            ));
        }

        Ok(())
    }

    async fn shutdown(&mut self) -> Result<(), PluginError> {
        Ok(())
    }
}

#[async_trait]
impl AssertionPlugin for JsonSchemaAssertion {
    fn assertion_type(&self) -> AssertionType {
        AssertionType::Custom("json-schema".to_string())
    }

    async fn execute(&self, actual: &ResponseData, _expected: &ExpectedValue) -> AssertionResult {
        let schema = match &self.schema {
            Some(s) => s,
            None => {
                return AssertionResult {
                    success: false,
                    message: "No JSON schema configured".to_string(),
                    actual_value: None,
                    expected_value: None,
                };
            }
        };

        // Parse JSON from response body
        let json_data = match self.parse_json_body(&actual.body) {
            Ok(data) => data,
            Err(error) => {
                return AssertionResult {
                    success: false,
                    message: error,
                    actual_value: None,
                    expected_value: Some(schema.clone()),
                };
            }
        };

        // Validate against schema
        match self.validate_against_schema(&json_data, schema) {
            Ok(()) => AssertionResult {
                success: true,
                message: "JSON response validates against schema".to_string(),
                actual_value: Some(json_data),
                expected_value: Some(schema.clone()),
            },
            Err(error) => AssertionResult {
                success: false,
                message: format!("JSON schema validation failed: {}", error),
                actual_value: Some(json_data),
                expected_value: Some(schema.clone()),
            },
        }
    }

    fn priority(&self) -> u8 {
        70 // Medium priority for schema validation
    }
}

/// JSON Structure Comparison Assertion for complex JSON comparisons
#[derive(Debug, Default)]
pub struct JsonStructureAssertion {
    ignore_order: bool,
    ignore_extra_fields: bool,
}

impl JsonStructureAssertion {
    pub fn new() -> Self {
        Self {
            ignore_order: false,
            ignore_extra_fields: false,
        }
    }

    pub fn ignore_array_order(mut self) -> Self {
        self.ignore_order = true;
        self
    }

    pub fn ignore_extra_fields(mut self) -> Self {
        self.ignore_extra_fields = true;
        self
    }

    /// Parse response body as JSON
    fn parse_json_body(&self, body: &[u8]) -> Result<JsonValue, String> {
        let body_str = std::str::from_utf8(body)
            .map_err(|_| "Response body is not valid UTF-8".to_string())?;
        
        serde_json::from_str(body_str)
            .map_err(|e| format!("Failed to parse JSON: {}", e))
    }

    /// Deep comparison of JSON structures with configurable options
    fn deep_compare(&self, actual: &JsonValue, expected: &JsonValue) -> Result<(), String> {
        match (actual, expected) {
            (JsonValue::Object(actual_obj), JsonValue::Object(expected_obj)) => {
                // Check that all expected fields are present and match
                for (key, expected_val) in expected_obj {
                    match actual_obj.get(key) {
                        Some(actual_val) => {
                            self.deep_compare(actual_val, expected_val)
                                .map_err(|e| format!("Field '{}': {}", key, e))?;
                        }
                        None => {
                            return Err(format!("Missing required field: '{}'", key));
                        }
                    }
                }

                // If not ignoring extra fields, check for unexpected fields
                if !self.ignore_extra_fields {
                    for key in actual_obj.keys() {
                        if !expected_obj.contains_key(key) {
                            return Err(format!("Unexpected field: '{}'", key));
                        }
                    }
                }

                Ok(())
            }
            (JsonValue::Array(actual_arr), JsonValue::Array(expected_arr)) => {
                if self.ignore_order {
                    // Check that all expected items are present (order-independent)
                    if actual_arr.len() != expected_arr.len() {
                        return Err(format!(
                            "Array length mismatch: expected {}, got {}",
                            expected_arr.len(),
                            actual_arr.len()
                        ));
                    }

                    for (i, expected_item) in expected_arr.iter().enumerate() {
                        let found = actual_arr.iter().any(|actual_item| {
                            self.deep_compare(actual_item, expected_item).is_ok()
                        });

                        if !found {
                            return Err(format!("Expected array item at index {} not found", i));
                        }
                    }
                } else {
                    // Order-dependent comparison
                    if actual_arr.len() != expected_arr.len() {
                        return Err(format!(
                            "Array length mismatch: expected {}, got {}",
                            expected_arr.len(),
                            actual_arr.len()
                        ));
                    }

                    for (i, (actual_item, expected_item)) in
                        actual_arr.iter().zip(expected_arr.iter()).enumerate()
                    {
                        self.deep_compare(actual_item, expected_item)
                            .map_err(|e| format!("Array index {}: {}", i, e))?;
                    }
                }

                Ok(())
            }
            (actual, expected) => {
                if actual == expected {
                    Ok(())
                } else {
                    Err(format!("Value mismatch: expected {}, got {}", expected, actual))
                }
            }
        }
    }
}

#[async_trait]
impl Plugin for JsonStructureAssertion {
    fn name(&self) -> &str {
        "json-structure-assertion"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn dependencies(&self) -> Vec<String> {
        vec![]
    }

    async fn initialize(&mut self, config: &PluginConfig) -> Result<(), PluginError> {
        if let Some(ignore_order) = config.settings.get("ignore_order") {
            self.ignore_order = ignore_order.as_bool().unwrap_or(false);
        }

        if let Some(ignore_extra) = config.settings.get("ignore_extra_fields") {
            self.ignore_extra_fields = ignore_extra.as_bool().unwrap_or(false);
        }

        Ok(())
    }

    async fn shutdown(&mut self) -> Result<(), PluginError> {
        Ok(())
    }
}

#[async_trait]
impl AssertionPlugin for JsonStructureAssertion {
    fn assertion_type(&self) -> AssertionType {
        AssertionType::Custom("json-structure".to_string())
    }

    async fn execute(&self, actual: &ResponseData, expected: &ExpectedValue) -> AssertionResult {
        // Parse JSON from response body
        let actual_json = match self.parse_json_body(&actual.body) {
            Ok(data) => data,
            Err(error) => {
                return AssertionResult {
                    success: false,
                    message: error,
                    actual_value: None,
                    expected_value: Some(serde_json::to_value(expected).unwrap_or(JsonValue::Null)),
                };
            }
        };

        // Extract expected JSON from ExpectedValue
        let expected_json = match expected {
            ExpectedValue::Exact(json_val) => json_val,
            _ => {
                return AssertionResult {
                    success: false,
                    message: "JSON structure assertion requires exact JSON value".to_string(),
                    actual_value: Some(actual_json),
                    expected_value: Some(serde_json::to_value(expected).unwrap_or(JsonValue::Null)),
                };
            }
        };

        // Perform deep comparison
        match self.deep_compare(&actual_json, expected_json) {
            Ok(()) => AssertionResult {
                success: true,
                message: "JSON structure matches expected structure".to_string(),
                actual_value: Some(actual_json),
                expected_value: Some(expected_json.clone()),
            },
            Err(error) => AssertionResult {
                success: false,
                message: format!("JSON structure mismatch: {}", error),
                actual_value: Some(actual_json),
                expected_value: Some(expected_json.clone()),
            },
        }
    }

    fn priority(&self) -> u8 {
        60 // Medium priority for structure validation
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::collections::HashMap;

    #[tokio::test]
    async fn test_jsonpath_assertion_exact_match() {
        let mut assertion = JsonPathAssertion::new("$.name".to_string());
        
        let config = PluginConfig {
            settings: HashMap::from([
                ("json_path".to_string(), json!("$.name"))
            ]),
            environment: "test".to_string(),
        };
        
        assertion.initialize(&config).await.unwrap();

        let response_data = ResponseData {
            status_code: 200,
            headers: HashMap::new(),
            body: r#"{"name": "John", "age": 30}"#.as_bytes().to_vec(),
            duration: std::time::Duration::from_millis(100),
            metadata: HashMap::new(),
        };

        let expected = ExpectedValue::Exact(json!("John"));
        let result = assertion.execute(&response_data, &expected).await;

        assert!(result.success);
        assert_eq!(result.actual_value, Some(json!("John")));
    }

    #[tokio::test]
    async fn test_jsonpath_assertion_contains() {
        let mut assertion = JsonPathAssertion::new("$.message".to_string());
        
        let config = PluginConfig {
            settings: HashMap::from([
                ("json_path".to_string(), json!("$.message"))
            ]),
            environment: "test".to_string(),
        };
        
        assertion.initialize(&config).await.unwrap();

        let response_data = ResponseData {
            status_code: 200,
            headers: HashMap::new(),
            body: r#"{"message": "Hello World"}"#.as_bytes().to_vec(),
            duration: std::time::Duration::from_millis(100),
            metadata: HashMap::new(),
        };

        let expected = ExpectedValue::Contains("World".to_string());
        let result = assertion.execute(&response_data, &expected).await;

        assert!(result.success);
        assert!(result.message.contains("contains"));
    }

    #[tokio::test]
    async fn test_json_schema_assertion_valid() {
        let schema = json!({
            "type": "object",
            "required": ["name", "age"],
            "properties": {
                "name": {"type": "string"},
                "age": {"type": "number"}
            }
        });

        let mut assertion = JsonSchemaAssertion::with_schema(schema.clone());
        
        let config = PluginConfig {
            settings: HashMap::from([
                ("schema".to_string(), schema)
            ]),
            environment: "test".to_string(),
        };
        
        assertion.initialize(&config).await.unwrap();

        let response_data = ResponseData {
            status_code: 200,
            headers: HashMap::new(),
            body: r#"{"name": "John", "age": 30}"#.as_bytes().to_vec(),
            duration: std::time::Duration::from_millis(100),
            metadata: HashMap::new(),
        };

        let expected = ExpectedValue::NotEmpty; // Schema validation doesn't use expected value
        let result = assertion.execute(&response_data, &expected).await;

        assert!(result.success);
        assert!(result.message.contains("validates against schema"));
    }

    #[tokio::test]
    async fn test_json_structure_assertion_match() {
        let mut assertion = JsonStructureAssertion::new();
        
        let config = PluginConfig {
            settings: HashMap::new(),
            environment: "test".to_string(),
        };
        
        assertion.initialize(&config).await.unwrap();

        let response_data = ResponseData {
            status_code: 200,
            headers: HashMap::new(),
            body: r#"{"name": "John", "age": 30, "active": true}"#.as_bytes().to_vec(),
            duration: std::time::Duration::from_millis(100),
            metadata: HashMap::new(),
        };

        let expected = ExpectedValue::Exact(json!({
            "name": "John",
            "age": 30,
            "active": true
        }));
        
        let result = assertion.execute(&response_data, &expected).await;

        assert!(result.success);
        assert!(result.message.contains("matches expected structure"));
    }
}