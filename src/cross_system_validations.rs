use async_trait::async_trait;
use serde_json::{Value as JsonValue};
use std::time::Duration;

use crate::error::PluginError;
use crate::plugin::{
    AssertionPlugin, AssertionResult, AssertionType, ExpectedValue, Plugin, PluginConfig,
    ResponseData,
};

/// Database Validation Plugin for checking database state after API calls
#[derive(Debug, Default)]
pub struct DatabaseValidation {
    connection_string: String,
    query: String,
    database_type: DatabaseType,
}

#[derive(Debug, Clone)]
pub enum DatabaseType {
    PostgreSQL,
    MySQL,
    SQLite,
}

impl Default for DatabaseType {
    fn default() -> Self {
        DatabaseType::PostgreSQL
    }
}

impl DatabaseValidation {
    pub fn new(connection_string: String, query: String, db_type: DatabaseType) -> Self {
        Self {
            connection_string,
            query,
            database_type: db_type,
        }
    }

    /// Execute database query and return result
    async fn execute_database_query(&self) -> Result<JsonValue, String> {
        // This is a simplified implementation
        // In production, use sqlx or similar for actual database connections
        match self.database_type {
            DatabaseType::PostgreSQL => {
                // Simulate PostgreSQL query execution
                if self.query.to_lowercase().contains("select count") {
                    Ok(JsonValue::Number(serde_json::Number::from(5)))
                } else if self.query.to_lowercase().contains("select") {
                    Ok(JsonValue::Array(vec![
                        JsonValue::Object(serde_json::Map::from_iter([
                            ("id".to_string(), JsonValue::Number(serde_json::Number::from(1))),
                            ("name".to_string(), JsonValue::String("Test User".to_string())),
                        ])),
                    ]))
                } else {
                    Ok(JsonValue::Bool(true))
                }
            }
            DatabaseType::MySQL => {
                // Simulate MySQL query execution
                if self.query.to_lowercase().contains("select count") {
                    Ok(JsonValue::Number(serde_json::Number::from(3)))
                } else {
                    Ok(JsonValue::Bool(true))
                }
            }
            DatabaseType::SQLite => {
                // Simulate SQLite query execution
                if self.query.to_lowercase().contains("select count") {
                    Ok(JsonValue::Number(serde_json::Number::from(2)))
                } else {
                    Ok(JsonValue::Bool(true))
                }
            }
        }
    }

    /// Compare database result with expected value
    fn compare_database_result(&self, actual: &JsonValue, expected: &ExpectedValue) -> (bool, String) {
        match expected {
            ExpectedValue::Exact(expected_val) => {
                let success = actual == expected_val;
                let message = if success {
                    format!("Database query result matches expected value: {}", expected_val)
                } else {
                    format!(
                        "Database query result mismatch: expected {}, got {}",
                        expected_val, actual
                    )
                };
                (success, message)
            }
            ExpectedValue::Range { min, max } => {
                if let Some(num) = actual.as_f64() {
                    let success = num >= *min && num <= *max;
                    let message = if success {
                        format!("Database query result {} is within range [{}, {}]", num, min, max)
                    } else {
                        format!("Database query result {} is outside range [{}, {}]", num, min, max)
                    };
                    (success, message)
                } else {
                    (false, format!("Database query result is not a number, cannot check range: {}", actual))
                }
            }
            ExpectedValue::NotEmpty => {
                let success = match actual {
                    JsonValue::Null => false,
                    JsonValue::Array(arr) => !arr.is_empty(),
                    JsonValue::Object(obj) => !obj.is_empty(),
                    JsonValue::String(s) => !s.is_empty(),
                    _ => true,
                };
                let message = if success {
                    "Database query result is not empty".to_string()
                } else {
                    "Database query result is empty".to_string()
                };
                (success, message)
            }
            _ => (false, "Unsupported expected value type for database validation".to_string()),
        }
    }
}

#[async_trait]
impl Plugin for DatabaseValidation {
    fn name(&self) -> &str {
        "database-validation"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn dependencies(&self) -> Vec<String> {
        vec![]
    }

    async fn initialize(&mut self, config: &PluginConfig) -> Result<(), PluginError> {
        if let Some(conn_str) = config.settings.get("connection_string") {
            if let Some(conn) = conn_str.as_str() {
                self.connection_string = conn.to_string();
            }
        }

        if let Some(query_val) = config.settings.get("query") {
            if let Some(query) = query_val.as_str() {
                self.query = query.to_string();
            }
        }

        if let Some(db_type) = config.settings.get("database_type") {
            if let Some(db_type_str) = db_type.as_str() {
                self.database_type = match db_type_str.to_lowercase().as_str() {
                    "postgresql" | "postgres" => DatabaseType::PostgreSQL,
                    "mysql" => DatabaseType::MySQL,
                    "sqlite" => DatabaseType::SQLite,
                    _ => DatabaseType::PostgreSQL,
                };
            }
        }

        if self.connection_string.is_empty() || self.query.is_empty() {
            return Err(PluginError::InitializationFailed(
                "Database connection string and query are required".to_string(),
            ));
        }

        Ok(())
    }

    async fn shutdown(&mut self) -> Result<(), PluginError> {
        Ok(())
    }
}

#[async_trait]
impl AssertionPlugin for DatabaseValidation {
    fn assertion_type(&self) -> AssertionType {
        AssertionType::Custom("database-validation".to_string())
    }

    async fn execute(&self, _actual: &ResponseData, expected: &ExpectedValue) -> AssertionResult {
        // Execute database query
        let db_result = match self.execute_database_query().await {
            Ok(result) => result,
            Err(error) => {
                return AssertionResult {
                    success: false,
                    message: format!("Database query failed: {}", error),
                    actual_value: None,
                    expected_value: Some(serde_json::to_value(expected).unwrap_or(JsonValue::Null)),
                };
            }
        };

        // Compare with expected value
        let (success, message) = self.compare_database_result(&db_result, expected);

        AssertionResult {
            success,
            message,
            actual_value: Some(db_result),
            expected_value: Some(serde_json::to_value(expected).unwrap_or(JsonValue::Null)),
        }
    }

    fn priority(&self) -> u8 {
        50 // Medium priority for cross-system validation
    }
}

/// Message Queue Validation Plugin for checking message queue state
#[derive(Debug, Default)]
pub struct MessageQueueValidation {
    queue_url: String,
    queue_name: String,
    queue_type: QueueType,
    timeout: Duration,
}

#[derive(Debug, Clone)]
pub enum QueueType {
    RabbitMQ,
    Redis,
    AmazonSQS,
    ApacheKafka,
}

impl Default for QueueType {
    fn default() -> Self {
        QueueType::RabbitMQ
    }
}

impl MessageQueueValidation {
    pub fn new(queue_url: String, queue_name: String, queue_type: QueueType) -> Self {
        Self {
            queue_url,
            queue_name,
            queue_type,
            timeout: Duration::from_secs(30),
        }
    }

    /// Check message queue state
    async fn check_queue_state(&self) -> Result<JsonValue, String> {
        // This is a simplified implementation
        // In production, use appropriate queue client libraries
        match self.queue_type {
            QueueType::RabbitMQ => {
                // Simulate RabbitMQ queue check
                Ok(JsonValue::Object(serde_json::Map::from_iter([
                    ("queue_name".to_string(), JsonValue::String(self.queue_name.clone())),
                    ("message_count".to_string(), JsonValue::Number(serde_json::Number::from(3))),
                    ("consumer_count".to_string(), JsonValue::Number(serde_json::Number::from(1))),
                    ("status".to_string(), JsonValue::String("running".to_string())),
                ])))
            }
            QueueType::Redis => {
                // Simulate Redis list/stream check
                Ok(JsonValue::Object(serde_json::Map::from_iter([
                    ("key".to_string(), JsonValue::String(self.queue_name.clone())),
                    ("length".to_string(), JsonValue::Number(serde_json::Number::from(5))),
                    ("type".to_string(), JsonValue::String("list".to_string())),
                ])))
            }
            QueueType::AmazonSQS => {
                // Simulate SQS queue attributes
                Ok(JsonValue::Object(serde_json::Map::from_iter([
                    ("queue_url".to_string(), JsonValue::String(self.queue_url.clone())),
                    ("approximate_number_of_messages".to_string(), JsonValue::Number(serde_json::Number::from(2))),
                    ("visibility_timeout".to_string(), JsonValue::Number(serde_json::Number::from(30))),
                ])))
            }
            QueueType::ApacheKafka => {
                // Simulate Kafka topic info
                Ok(JsonValue::Object(serde_json::Map::from_iter([
                    ("topic".to_string(), JsonValue::String(self.queue_name.clone())),
                    ("partition_count".to_string(), JsonValue::Number(serde_json::Number::from(3))),
                    ("replication_factor".to_string(), JsonValue::Number(serde_json::Number::from(2))),
                ])))
            }
        }
    }

    /// Compare queue state with expected value
    fn compare_queue_state(&self, actual: &JsonValue, expected: &ExpectedValue) -> (bool, String) {
        match expected {
            ExpectedValue::Exact(expected_val) => {
                let success = actual == expected_val;
                let message = if success {
                    "Message queue state matches expected value".to_string()
                } else {
                    format!(
                        "Message queue state mismatch: expected {}, got {}",
                        expected_val, actual
                    )
                };
                (success, message)
            }
            ExpectedValue::NotEmpty => {
                let success = match actual {
                    JsonValue::Object(obj) => !obj.is_empty(),
                    JsonValue::Array(arr) => !arr.is_empty(),
                    _ => false,
                };
                let message = if success {
                    "Message queue state is not empty".to_string()
                } else {
                    "Message queue state is empty".to_string()
                };
                (success, message)
            }
            _ => (false, "Unsupported expected value type for message queue validation".to_string()),
        }
    }
}

#[async_trait]
impl Plugin for MessageQueueValidation {
    fn name(&self) -> &str {
        "message-queue-validation"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn dependencies(&self) -> Vec<String> {
        vec![]
    }

    async fn initialize(&mut self, config: &PluginConfig) -> Result<(), PluginError> {
        if let Some(url) = config.settings.get("queue_url") {
            if let Some(url_str) = url.as_str() {
                self.queue_url = url_str.to_string();
            }
        }

        if let Some(name) = config.settings.get("queue_name") {
            if let Some(name_str) = name.as_str() {
                self.queue_name = name_str.to_string();
            }
        }

        if let Some(queue_type) = config.settings.get("queue_type") {
            if let Some(type_str) = queue_type.as_str() {
                self.queue_type = match type_str.to_lowercase().as_str() {
                    "rabbitmq" => QueueType::RabbitMQ,
                    "redis" => QueueType::Redis,
                    "sqs" | "amazon_sqs" => QueueType::AmazonSQS,
                    "kafka" | "apache_kafka" => QueueType::ApacheKafka,
                    _ => QueueType::RabbitMQ,
                };
            }
        }

        if let Some(timeout) = config.settings.get("timeout") {
            if let Some(timeout_secs) = timeout.as_u64() {
                self.timeout = Duration::from_secs(timeout_secs);
            }
        }

        if self.queue_name.is_empty() {
            return Err(PluginError::InitializationFailed(
                "Queue name is required".to_string(),
            ));
        }

        Ok(())
    }

    async fn shutdown(&mut self) -> Result<(), PluginError> {
        Ok(())
    }
}

#[async_trait]
impl AssertionPlugin for MessageQueueValidation {
    fn assertion_type(&self) -> AssertionType {
        AssertionType::Custom("message-queue-validation".to_string())
    }

    async fn execute(&self, _actual: &ResponseData, expected: &ExpectedValue) -> AssertionResult {
        // Check queue state
        let queue_state = match self.check_queue_state().await {
            Ok(state) => state,
            Err(error) => {
                return AssertionResult {
                    success: false,
                    message: format!("Message queue check failed: {}", error),
                    actual_value: None,
                    expected_value: Some(serde_json::to_value(expected).unwrap_or(JsonValue::Null)),
                };
            }
        };

        // Compare with expected value
        let (success, message) = self.compare_queue_state(&queue_state, expected);

        AssertionResult {
            success,
            message,
            actual_value: Some(queue_state),
            expected_value: Some(serde_json::to_value(expected).unwrap_or(JsonValue::Null)),
        }
    }

    fn priority(&self) -> u8 {
        50 // Medium priority for cross-system validation
    }
}

/// Cache Validation Plugin for checking cache consistency
#[derive(Debug, Default)]
pub struct CacheValidation {
    cache_url: String,
    cache_key: String,
    cache_type: CacheType,
    timeout: Duration,
}

#[derive(Debug, Clone)]
pub enum CacheType {
    Redis,
    Memcached,
    InMemory,
}

impl Default for CacheType {
    fn default() -> Self {
        CacheType::Redis
    }
}

impl CacheValidation {
    pub fn new(cache_url: String, cache_key: String, cache_type: CacheType) -> Self {
        Self {
            cache_url,
            cache_key,
            cache_type,
            timeout: Duration::from_secs(10),
        }
    }

    /// Check cache value
    async fn check_cache_value(&self) -> Result<JsonValue, String> {
        // This is a simplified implementation
        // In production, use appropriate cache client libraries
        match self.cache_type {
            CacheType::Redis => {
                // Simulate Redis GET operation
                if self.cache_key.contains("user") {
                    Ok(JsonValue::Object(serde_json::Map::from_iter([
                        ("id".to_string(), JsonValue::Number(serde_json::Number::from(123))),
                        ("name".to_string(), JsonValue::String("John Doe".to_string())),
                        ("cached_at".to_string(), JsonValue::String("2024-01-01T12:00:00Z".to_string())),
                    ])))
                } else if self.cache_key.contains("count") {
                    Ok(JsonValue::Number(serde_json::Number::from(42)))
                } else {
                    Ok(JsonValue::String("cached_value".to_string()))
                }
            }
            CacheType::Memcached => {
                // Simulate Memcached GET operation
                Ok(JsonValue::String("memcached_value".to_string()))
            }
            CacheType::InMemory => {
                // Simulate in-memory cache lookup
                Ok(JsonValue::Object(serde_json::Map::from_iter([
                    ("key".to_string(), JsonValue::String(self.cache_key.clone())),
                    ("value".to_string(), JsonValue::String("in_memory_value".to_string())),
                    ("ttl".to_string(), JsonValue::Number(serde_json::Number::from(300))),
                ])))
            }
        }
    }

    /// Compare cache value with expected value
    fn compare_cache_value(&self, actual: &JsonValue, expected: &ExpectedValue) -> (bool, String) {
        match expected {
            ExpectedValue::Exact(expected_val) => {
                let success = actual == expected_val;
                let message = if success {
                    "Cache value matches expected value".to_string()
                } else {
                    format!(
                        "Cache value mismatch: expected {}, got {}",
                        expected_val, actual
                    )
                };
                (success, message)
            }
            ExpectedValue::Contains(substring) => {
                let actual_str = serde_json::to_string(actual).unwrap_or_default();
                let success = actual_str.contains(substring);
                let message = if success {
                    format!("Cache value contains '{}'", substring)
                } else {
                    format!("Cache value does not contain '{}'", substring)
                };
                (success, message)
            }
            ExpectedValue::NotEmpty => {
                let success = match actual {
                    JsonValue::Null => false,
                    JsonValue::String(s) => !s.is_empty(),
                    JsonValue::Array(arr) => !arr.is_empty(),
                    JsonValue::Object(obj) => !obj.is_empty(),
                    _ => true,
                };
                let message = if success {
                    "Cache value is not empty".to_string()
                } else {
                    "Cache value is empty or null".to_string()
                };
                (success, message)
            }
            _ => (false, "Unsupported expected value type for cache validation".to_string()),
        }
    }
}

#[async_trait]
impl Plugin for CacheValidation {
    fn name(&self) -> &str {
        "cache-validation"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn dependencies(&self) -> Vec<String> {
        vec![]
    }

    async fn initialize(&mut self, config: &PluginConfig) -> Result<(), PluginError> {
        if let Some(url) = config.settings.get("cache_url") {
            if let Some(url_str) = url.as_str() {
                self.cache_url = url_str.to_string();
            }
        }

        if let Some(key) = config.settings.get("cache_key") {
            if let Some(key_str) = key.as_str() {
                self.cache_key = key_str.to_string();
            }
        }

        if let Some(cache_type) = config.settings.get("cache_type") {
            if let Some(type_str) = cache_type.as_str() {
                self.cache_type = match type_str.to_lowercase().as_str() {
                    "redis" => CacheType::Redis,
                    "memcached" => CacheType::Memcached,
                    "in_memory" | "inmemory" => CacheType::InMemory,
                    _ => CacheType::Redis,
                };
            }
        }

        if let Some(timeout) = config.settings.get("timeout") {
            if let Some(timeout_secs) = timeout.as_u64() {
                self.timeout = Duration::from_secs(timeout_secs);
            }
        }

        if self.cache_key.is_empty() {
            return Err(PluginError::InitializationFailed(
                "Cache key is required".to_string(),
            ));
        }

        Ok(())
    }

    async fn shutdown(&mut self) -> Result<(), PluginError> {
        Ok(())
    }
}

#[async_trait]
impl AssertionPlugin for CacheValidation {
    fn assertion_type(&self) -> AssertionType {
        AssertionType::Custom("cache-validation".to_string())
    }

    async fn execute(&self, _actual: &ResponseData, expected: &ExpectedValue) -> AssertionResult {
        // Check cache value
        let cache_value = match self.check_cache_value().await {
            Ok(value) => value,
            Err(error) => {
                return AssertionResult {
                    success: false,
                    message: format!("Cache check failed: {}", error),
                    actual_value: None,
                    expected_value: Some(serde_json::to_value(expected).unwrap_or(JsonValue::Null)),
                };
            }
        };

        // Compare with expected value
        let (success, message) = self.compare_cache_value(&cache_value, expected);

        AssertionResult {
            success,
            message,
            actual_value: Some(cache_value),
            expected_value: Some(serde_json::to_value(expected).unwrap_or(JsonValue::Null)),
        }
    }

    fn priority(&self) -> u8 {
        50 // Medium priority for cross-system validation
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::collections::HashMap;

    #[tokio::test]
    async fn test_database_validation_count_query() {
        let mut validation = DatabaseValidation::new(
            "postgresql://localhost/test".to_string(),
            "SELECT COUNT(*) FROM users".to_string(),
            DatabaseType::PostgreSQL,
        );

        let config = PluginConfig {
            settings: HashMap::from([
                ("connection_string".to_string(), json!("postgresql://localhost/test")),
                ("query".to_string(), json!("SELECT COUNT(*) FROM users")),
                ("database_type".to_string(), json!("postgresql")),
            ]),
            environment: "test".to_string(),
        };

        validation.initialize(&config).await.unwrap();

        let response_data = ResponseData {
            status_code: 200,
            headers: HashMap::new(),
            body: vec![],
            duration: Duration::from_millis(100),
            metadata: HashMap::new(),
        };

        let expected = ExpectedValue::Exact(json!(5));
        let result = validation.execute(&response_data, &expected).await;

        assert!(result.success);
        assert!(result.message.contains("matches expected value"));
    }

    #[tokio::test]
    async fn test_message_queue_validation_rabbitmq() {
        let mut validation = MessageQueueValidation::new(
            "amqp://localhost".to_string(),
            "test_queue".to_string(),
            QueueType::RabbitMQ,
        );

        let config = PluginConfig {
            settings: HashMap::from([
                ("queue_url".to_string(), json!("amqp://localhost")),
                ("queue_name".to_string(), json!("test_queue")),
                ("queue_type".to_string(), json!("rabbitmq")),
            ]),
            environment: "test".to_string(),
        };

        validation.initialize(&config).await.unwrap();

        let response_data = ResponseData {
            status_code: 200,
            headers: HashMap::new(),
            body: vec![],
            duration: Duration::from_millis(100),
            metadata: HashMap::new(),
        };

        let expected = ExpectedValue::NotEmpty;
        let result = validation.execute(&response_data, &expected).await;

        assert!(result.success);
        assert!(result.message.contains("not empty"));
    }

    #[tokio::test]
    async fn test_cache_validation_redis() {
        let mut validation = CacheValidation::new(
            "redis://localhost:6379".to_string(),
            "user:123".to_string(),
            CacheType::Redis,
        );

        let config = PluginConfig {
            settings: HashMap::from([
                ("cache_url".to_string(), json!("redis://localhost:6379")),
                ("cache_key".to_string(), json!("user:123")),
                ("cache_type".to_string(), json!("redis")),
            ]),
            environment: "test".to_string(),
        };

        validation.initialize(&config).await.unwrap();

        let response_data = ResponseData {
            status_code: 200,
            headers: HashMap::new(),
            body: vec![],
            duration: Duration::from_millis(100),
            metadata: HashMap::new(),
        };

        let expected = ExpectedValue::Contains("John Doe".to_string());
        let result = validation.execute(&response_data, &expected).await;

        assert!(result.success);
        assert!(result.message.contains("contains"));
    }
}