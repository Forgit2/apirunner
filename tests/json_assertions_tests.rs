use apirunner::json_assertions::{JsonPathAssertion, JsonSchemaAssertion, JsonStructureAssertion};
use apirunner::{
    Plugin, PluginConfig, AssertionPlugin, AssertionType, ExpectedValue, ResponseData,
};
use serde_json::json;
use std::collections::HashMap;
use std::time::Duration;

#[tokio::test]
async fn test_jsonpath_assertion_initialization() {
    let mut assertion = JsonPathAssertion::new("$.user.name".to_string());
    
    let config = PluginConfig {
        settings: HashMap::from([
            ("json_path".to_string(), json!("$.user.name"))
        ]),
        environment: "test".to_string(),
    };
    
    let result = assertion.initialize(&config).await;
    assert!(result.is_ok());
    assert_eq!(assertion.assertion_type(), AssertionType::JsonPath);
    assert_eq!(assertion.priority(), 80);
}

#[tokio::test]
async fn test_jsonpath_assertion_exact_match_success() {
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
        body: r#"{"name": "John Doe", "age": 30}"#.as_bytes().to_vec(),
        duration: Duration::from_millis(100),
        metadata: HashMap::new(),
    };

    let expected = ExpectedValue::Exact(json!("John Doe"));
    let result = assertion.execute(&response_data, &expected).await;

    assert!(result.success);
    assert!(result.message.contains("matches expected value"));
    assert_eq!(result.actual_value, Some(json!("John Doe")));
}

#[tokio::test]
async fn test_jsonpath_assertion_exact_match_failure() {
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
        body: r#"{"name": "Jane Doe", "age": 25}"#.as_bytes().to_vec(),
        duration: Duration::from_millis(100),
        metadata: HashMap::new(),
    };

    let expected = ExpectedValue::Exact(json!("John Doe"));
    let result = assertion.execute(&response_data, &expected).await;

    assert!(!result.success);
    assert!(result.message.contains("mismatch"));
    assert_eq!(result.actual_value, Some(json!("Jane Doe")));
}

#[tokio::test]
async fn test_jsonpath_assertion_contains_success() {
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
        body: r#"{"message": "Hello World from API"}"#.as_bytes().to_vec(),
        duration: Duration::from_millis(100),
        metadata: HashMap::new(),
    };

    let expected = ExpectedValue::Contains("World".to_string());
    let result = assertion.execute(&response_data, &expected).await;

    assert!(result.success);
    assert!(result.message.contains("contains"));
}

#[tokio::test]
async fn test_jsonpath_assertion_pattern_match() {
    let mut assertion = JsonPathAssertion::new("$.email".to_string());
    
    let config = PluginConfig {
        settings: HashMap::from([
            ("json_path".to_string(), json!("$.email"))
        ]),
        environment: "test".to_string(),
    };
    
    assertion.initialize(&config).await.unwrap();

    let response_data = ResponseData {
        status_code: 200,
        headers: HashMap::new(),
        body: r#"{"email": "user@example.com"}"#.as_bytes().to_vec(),
        duration: Duration::from_millis(100),
        metadata: HashMap::new(),
    };

    let expected = ExpectedValue::Pattern(r"^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$".to_string());
    let result = assertion.execute(&response_data, &expected).await;

    assert!(result.success);
    assert!(result.message.contains("matches pattern"));
}

#[tokio::test]
async fn test_jsonpath_assertion_not_empty() {
    let mut assertion = JsonPathAssertion::new("$.data".to_string());
    
    let config = PluginConfig {
        settings: HashMap::from([
            ("json_path".to_string(), json!("$.data"))
        ]),
        environment: "test".to_string(),
    };
    
    assertion.initialize(&config).await.unwrap();

    let response_data = ResponseData {
        status_code: 200,
        headers: HashMap::new(),
        body: r#"{"data": [1, 2, 3]}"#.as_bytes().to_vec(),
        duration: Duration::from_millis(100),
        metadata: HashMap::new(),
    };

    let expected = ExpectedValue::NotEmpty;
    let result = assertion.execute(&response_data, &expected).await;

    assert!(result.success);
    assert!(result.message.contains("not empty"));
}

#[tokio::test]
async fn test_jsonpath_assertion_range() {
    let mut assertion = JsonPathAssertion::new("$.score".to_string());
    
    let config = PluginConfig {
        settings: HashMap::from([
            ("json_path".to_string(), json!("$.score"))
        ]),
        environment: "test".to_string(),
    };
    
    assertion.initialize(&config).await.unwrap();

    let response_data = ResponseData {
        status_code: 200,
        headers: HashMap::new(),
        body: r#"{"score": 85.5}"#.as_bytes().to_vec(),
        duration: Duration::from_millis(100),
        metadata: HashMap::new(),
    };

    let expected = ExpectedValue::Range { min: 80.0, max: 100.0 };
    let result = assertion.execute(&response_data, &expected).await;

    assert!(result.success);
    assert!(result.message.contains("within range"));
}

#[tokio::test]
async fn test_jsonpath_assertion_invalid_json() {
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
        body: b"invalid json".to_vec(),
        duration: Duration::from_millis(100),
        metadata: HashMap::new(),
    };

    let expected = ExpectedValue::Exact(json!("John"));
    let result = assertion.execute(&response_data, &expected).await;

    assert!(!result.success);
    assert!(result.message.contains("Failed to parse JSON"));
}

#[tokio::test]
async fn test_json_schema_assertion_valid_schema() {
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
        duration: Duration::from_millis(100),
        metadata: HashMap::new(),
    };

    let expected = ExpectedValue::NotEmpty;
    let result = assertion.execute(&response_data, &expected).await;

    assert!(result.success);
    assert!(result.message.contains("validates against schema"));
}

#[tokio::test]
async fn test_json_schema_assertion_missing_required_field() {
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
        body: r#"{"name": "John"}"#.as_bytes().to_vec(),
        duration: Duration::from_millis(100),
        metadata: HashMap::new(),
    };

    let expected = ExpectedValue::NotEmpty;
    let result = assertion.execute(&response_data, &expected).await;

    assert!(!result.success);
    assert!(result.message.contains("Required field 'age' is missing"));
}

#[tokio::test]
async fn test_json_schema_assertion_type_mismatch() {
    let schema = json!({
        "type": "object",
        "properties": {
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
        body: r#"{"age": "thirty"}"#.as_bytes().to_vec(),
        duration: Duration::from_millis(100),
        metadata: HashMap::new(),
    };

    let expected = ExpectedValue::NotEmpty;
    let result = assertion.execute(&response_data, &expected).await;

    assert!(!result.success);
    assert!(result.message.contains("Type mismatch"));
}

#[tokio::test]
async fn test_json_structure_assertion_exact_match() {
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
        duration: Duration::from_millis(100),
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

#[tokio::test]
async fn test_json_structure_assertion_ignore_extra_fields() {
    let mut assertion = JsonStructureAssertion::new().ignore_extra_fields();
    
    let config = PluginConfig {
        settings: HashMap::from([
            ("ignore_extra_fields".to_string(), json!(true))
        ]),
        environment: "test".to_string(),
    };
    
    assertion.initialize(&config).await.unwrap();

    let response_data = ResponseData {
        status_code: 200,
        headers: HashMap::new(),
        body: r#"{"name": "John", "age": 30, "extra": "field"}"#.as_bytes().to_vec(),
        duration: Duration::from_millis(100),
        metadata: HashMap::new(),
    };

    let expected = ExpectedValue::Exact(json!({
        "name": "John",
        "age": 30
    }));
    
    let result = assertion.execute(&response_data, &expected).await;

    assert!(result.success);
}

#[tokio::test]
async fn test_json_structure_assertion_array_order_independent() {
    let mut assertion = JsonStructureAssertion::new().ignore_array_order();
    
    let config = PluginConfig {
        settings: HashMap::from([
            ("ignore_order".to_string(), json!(true))
        ]),
        environment: "test".to_string(),
    };
    
    assertion.initialize(&config).await.unwrap();

    let response_data = ResponseData {
        status_code: 200,
        headers: HashMap::new(),
        body: r#"{"items": [3, 1, 2]}"#.as_bytes().to_vec(),
        duration: Duration::from_millis(100),
        metadata: HashMap::new(),
    };

    let expected = ExpectedValue::Exact(json!({
        "items": [1, 2, 3]
    }));
    
    let result = assertion.execute(&response_data, &expected).await;

    assert!(result.success);
}

#[tokio::test]
async fn test_json_structure_assertion_nested_objects() {
    let mut assertion = JsonStructureAssertion::new();
    
    let config = PluginConfig {
        settings: HashMap::new(),
        environment: "test".to_string(),
    };
    
    assertion.initialize(&config).await.unwrap();

    let response_data = ResponseData {
        status_code: 200,
        headers: HashMap::new(),
        body: r#"{"user": {"name": "John", "profile": {"age": 30}}}"#.as_bytes().to_vec(),
        duration: Duration::from_millis(100),
        metadata: HashMap::new(),
    };

    let expected = ExpectedValue::Exact(json!({
        "user": {
            "name": "John",
            "profile": {
                "age": 30
            }
        }
    }));
    
    let result = assertion.execute(&response_data, &expected).await;

    assert!(result.success);
}

#[tokio::test]
async fn test_json_structure_assertion_field_mismatch() {
    let mut assertion = JsonStructureAssertion::new();
    
    let config = PluginConfig {
        settings: HashMap::new(),
        environment: "test".to_string(),
    };
    
    assertion.initialize(&config).await.unwrap();

    let response_data = ResponseData {
        status_code: 200,
        headers: HashMap::new(),
        body: r#"{"name": "Jane", "age": 25}"#.as_bytes().to_vec(),
        duration: Duration::from_millis(100),
        metadata: HashMap::new(),
    };

    let expected = ExpectedValue::Exact(json!({
        "name": "John",
        "age": 30
    }));
    
    let result = assertion.execute(&response_data, &expected).await;

    assert!(!result.success);
    assert!(result.message.contains("Value mismatch"));
}