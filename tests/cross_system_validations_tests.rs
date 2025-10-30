use apirunner::cross_system_validations::{
    DatabaseValidation, MessageQueueValidation, CacheValidation,
    DatabaseType, QueueType, CacheType,
};
use apirunner::{
    Plugin, PluginConfig, AssertionPlugin, AssertionType, ExpectedValue, ResponseData,
};
use serde_json::json;
use std::collections::HashMap;
use std::time::Duration;

#[tokio::test]
async fn test_database_validation_initialization() {
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

    let result = validation.initialize(&config).await;
    assert!(result.is_ok());
    assert_eq!(validation.assertion_type(), AssertionType::Custom("database-validation".to_string()));
    assert_eq!(validation.priority(), 50);
}

#[tokio::test]
async fn test_database_validation_count_query_success() {
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
    assert_eq!(result.actual_value, Some(json!(5)));
}

#[tokio::test]
async fn test_database_validation_range_query() {
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

    let expected = ExpectedValue::Range { min: 1.0, max: 10.0 };
    let result = validation.execute(&response_data, &expected).await;

    assert!(result.success);
    assert!(result.message.contains("within range"));
}

#[tokio::test]
async fn test_database_validation_mysql() {
    let mut validation = DatabaseValidation::new(
        "mysql://localhost/test".to_string(),
        "SELECT COUNT(*) FROM products".to_string(),
        DatabaseType::MySQL,
    );

    let config = PluginConfig {
        settings: HashMap::from([
            ("connection_string".to_string(), json!("mysql://localhost/test")),
            ("query".to_string(), json!("SELECT COUNT(*) FROM products")),
            ("database_type".to_string(), json!("mysql")),
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

    let expected = ExpectedValue::Exact(json!(3));
    let result = validation.execute(&response_data, &expected).await;

    assert!(result.success);
}

#[tokio::test]
async fn test_database_validation_sqlite() {
    let mut validation = DatabaseValidation::new(
        "sqlite://test.db".to_string(),
        "SELECT COUNT(*) FROM orders".to_string(),
        DatabaseType::SQLite,
    );

    let config = PluginConfig {
        settings: HashMap::from([
            ("connection_string".to_string(), json!("sqlite://test.db")),
            ("query".to_string(), json!("SELECT COUNT(*) FROM orders")),
            ("database_type".to_string(), json!("sqlite")),
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

    let expected = ExpectedValue::Exact(json!(2));
    let result = validation.execute(&response_data, &expected).await;

    assert!(result.success);
}

#[tokio::test]
async fn test_message_queue_validation_initialization() {
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

    let result = validation.initialize(&config).await;
    assert!(result.is_ok());
    assert_eq!(validation.assertion_type(), AssertionType::Custom("message-queue-validation".to_string()));
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
async fn test_message_queue_validation_redis() {
    let mut validation = MessageQueueValidation::new(
        "redis://localhost:6379".to_string(),
        "task_queue".to_string(),
        QueueType::Redis,
    );

    let config = PluginConfig {
        settings: HashMap::from([
            ("queue_url".to_string(), json!("redis://localhost:6379")),
            ("queue_name".to_string(), json!("task_queue")),
            ("queue_type".to_string(), json!("redis")),
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
}

#[tokio::test]
async fn test_message_queue_validation_sqs() {
    let mut validation = MessageQueueValidation::new(
        "https://sqs.us-east-1.amazonaws.com/123456789012/test-queue".to_string(),
        "test-queue".to_string(),
        QueueType::AmazonSQS,
    );

    let config = PluginConfig {
        settings: HashMap::from([
            ("queue_url".to_string(), json!("https://sqs.us-east-1.amazonaws.com/123456789012/test-queue")),
            ("queue_name".to_string(), json!("test-queue")),
            ("queue_type".to_string(), json!("sqs")),
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
}

#[tokio::test]
async fn test_message_queue_validation_kafka() {
    let mut validation = MessageQueueValidation::new(
        "localhost:9092".to_string(),
        "events-topic".to_string(),
        QueueType::ApacheKafka,
    );

    let config = PluginConfig {
        settings: HashMap::from([
            ("queue_url".to_string(), json!("localhost:9092")),
            ("queue_name".to_string(), json!("events-topic")),
            ("queue_type".to_string(), json!("kafka")),
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
}

#[tokio::test]
async fn test_cache_validation_initialization() {
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

    let result = validation.initialize(&config).await;
    assert!(result.is_ok());
    assert_eq!(validation.assertion_type(), AssertionType::Custom("cache-validation".to_string()));
}

#[tokio::test]
async fn test_cache_validation_redis_contains() {
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

#[tokio::test]
async fn test_cache_validation_redis_not_empty() {
    let mut validation = CacheValidation::new(
        "redis://localhost:6379".to_string(),
        "count:active_users".to_string(),
        CacheType::Redis,
    );

    let config = PluginConfig {
        settings: HashMap::from([
            ("cache_url".to_string(), json!("redis://localhost:6379")),
            ("cache_key".to_string(), json!("count:active_users")),
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

    let expected = ExpectedValue::NotEmpty;
    let result = validation.execute(&response_data, &expected).await;

    assert!(result.success);
    assert!(result.message.contains("not empty"));
}

#[tokio::test]
async fn test_cache_validation_memcached() {
    let mut validation = CacheValidation::new(
        "localhost:11211".to_string(),
        "session:abc123".to_string(),
        CacheType::Memcached,
    );

    let config = PluginConfig {
        settings: HashMap::from([
            ("cache_url".to_string(), json!("localhost:11211")),
            ("cache_key".to_string(), json!("session:abc123")),
            ("cache_type".to_string(), json!("memcached")),
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
}

#[tokio::test]
async fn test_cache_validation_in_memory() {
    let mut validation = CacheValidation::new(
        "".to_string(),
        "temp:data".to_string(),
        CacheType::InMemory,
    );

    let config = PluginConfig {
        settings: HashMap::from([
            ("cache_key".to_string(), json!("temp:data")),
            ("cache_type".to_string(), json!("in_memory")),
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
}

#[tokio::test]
async fn test_cache_validation_exact_match() {
    let mut validation = CacheValidation::new(
        "redis://localhost:6379".to_string(),
        "config:feature_flags".to_string(),
        CacheType::Redis,
    );

    let config = PluginConfig {
        settings: HashMap::from([
            ("cache_url".to_string(), json!("redis://localhost:6379")),
            ("cache_key".to_string(), json!("config:feature_flags")),
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

    let expected = ExpectedValue::Exact(json!("cached_value"));
    let result = validation.execute(&response_data, &expected).await;

    assert!(result.success);
    assert!(result.message.contains("matches expected value"));
}