pub mod http_client;
pub mod database;
pub mod file_system;
pub mod notification_service;

use async_trait::async_trait;
use mockall::mock;
use serde_json::Value;
use std::collections::HashMap;
use std::time::Duration;

// Mock HTTP client for testing REST API plugin
mock! {
    pub HttpClient {}
    
    #[async_trait]
    impl HttpClientTrait for HttpClient {
        async fn send_request(&self, request: HttpRequest) -> Result<HttpResponse, HttpError>;
        async fn configure_tls(&mut self, config: TlsConfig) -> Result<(), HttpError>;
        fn set_timeout(&mut self, timeout: Duration);
        fn set_connection_pool_size(&mut self, size: usize);
    }
}

#[derive(Debug, Clone)]
pub struct HttpRequest {
    pub method: String,
    pub url: String,
    pub headers: HashMap<String, String>,
    pub body: Option<Vec<u8>>,
}

#[derive(Debug, Clone)]
pub struct HttpResponse {
    pub status_code: u16,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
    pub duration: Duration,
}

#[derive(Debug, Clone)]
pub struct TlsConfig {
    pub verify_certificates: bool,
    pub client_cert: Option<Vec<u8>>,
    pub ca_certificates: Vec<Vec<u8>>,
}

#[derive(Debug, thiserror::Error)]
pub enum HttpError {
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),
    #[error("Timeout occurred")]
    Timeout,
    #[error("TLS error: {0}")]
    TlsError(String),
    #[error("Invalid request: {0}")]
    InvalidRequest(String),
}

#[async_trait]
pub trait HttpClientTrait: Send + Sync {
    async fn send_request(&self, request: HttpRequest) -> Result<HttpResponse, HttpError>;
    async fn configure_tls(&mut self, config: TlsConfig) -> Result<(), HttpError>;
    fn set_timeout(&mut self, timeout: Duration);
    fn set_connection_pool_size(&mut self, size: usize);
}

// Mock database connection for testing data source plugins
mock! {
    pub DatabaseConnection {}
    
    #[async_trait]
    impl DatabaseConnectionTrait for DatabaseConnection {
        async fn execute_query(&self, query: &str, params: Vec<Value>) -> Result<Vec<HashMap<String, Value>>, DatabaseError>;
        async fn execute_transaction(&self, queries: Vec<(String, Vec<Value>)>) -> Result<(), DatabaseError>;
        async fn test_connection(&self) -> Result<(), DatabaseError>;
        fn get_connection_info(&self) -> ConnectionInfo;
    }
}

#[derive(Debug, Clone)]
pub struct ConnectionInfo {
    pub host: String,
    pub port: u16,
    pub database: String,
    pub username: String,
}

#[derive(Debug, thiserror::Error)]
pub enum DatabaseError {
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),
    #[error("Query failed: {0}")]
    QueryFailed(String),
    #[error("Transaction failed: {0}")]
    TransactionFailed(String),
    #[error("Invalid query: {0}")]
    InvalidQuery(String),
}

#[async_trait]
pub trait DatabaseConnectionTrait: Send + Sync {
    async fn execute_query(&self, query: &str, params: Vec<Value>) -> Result<Vec<HashMap<String, Value>>, DatabaseError>;
    async fn execute_transaction(&self, queries: Vec<(String, Vec<Value>)>) -> Result<(), DatabaseError>;
    async fn test_connection(&self) -> Result<(), DatabaseError>;
    fn get_connection_info(&self) -> ConnectionInfo;
}

// Mock file system for testing file operations
mock! {
    pub FileSystem {}
    
    #[async_trait]
    impl FileSystemTrait for FileSystem {
        async fn read_file(&self, path: &str) -> Result<Vec<u8>, FileSystemError>;
        async fn write_file(&self, path: &str, content: &[u8]) -> Result<(), FileSystemError>;
        async fn delete_file(&self, path: &str) -> Result<(), FileSystemError>;
        async fn list_directory(&self, path: &str) -> Result<Vec<String>, FileSystemError>;
        async fn create_directory(&self, path: &str) -> Result<(), FileSystemError>;
        async fn file_exists(&self, path: &str) -> Result<bool, FileSystemError>;
        async fn get_file_metadata(&self, path: &str) -> Result<FileMetadata, FileSystemError>;
    }
}

#[derive(Debug, Clone)]
pub struct FileMetadata {
    pub size: u64,
    pub created: chrono::DateTime<chrono::Utc>,
    pub modified: chrono::DateTime<chrono::Utc>,
    pub is_directory: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum FileSystemError {
    #[error("File not found: {0}")]
    FileNotFound(String),
    #[error("Permission denied: {0}")]
    PermissionDenied(String),
    #[error("IO error: {0}")]
    IoError(String),
    #[error("Invalid path: {0}")]
    InvalidPath(String),
}

#[async_trait]
pub trait FileSystemTrait: Send + Sync {
    async fn read_file(&self, path: &str) -> Result<Vec<u8>, FileSystemError>;
    async fn write_file(&self, path: &str, content: &[u8]) -> Result<(), FileSystemError>;
    async fn delete_file(&self, path: &str) -> Result<(), FileSystemError>;
    async fn list_directory(&self, path: &str) -> Result<Vec<String>, FileSystemError>;
    async fn create_directory(&self, path: &str) -> Result<(), FileSystemError>;
    async fn file_exists(&self, path: &str) -> Result<bool, FileSystemError>;
    async fn get_file_metadata(&self, path: &str) -> Result<FileMetadata, FileSystemError>;
}

// Mock notification service for testing alerting
mock! {
    pub NotificationService {}
    
    #[async_trait]
    impl NotificationServiceTrait for NotificationService {
        async fn send_email(&self, to: &str, subject: &str, body: &str) -> Result<(), NotificationError>;
        async fn send_slack_message(&self, channel: &str, message: &str) -> Result<(), NotificationError>;
        async fn send_webhook(&self, url: &str, payload: Value) -> Result<(), NotificationError>;
        async fn send_sms(&self, phone: &str, message: &str) -> Result<(), NotificationError>;
    }
}

#[derive(Debug, thiserror::Error)]
pub enum NotificationError {
    #[error("Send failed: {0}")]
    SendFailed(String),
    #[error("Invalid recipient: {0}")]
    InvalidRecipient(String),
    #[error("Rate limited")]
    RateLimited,
    #[error("Service unavailable")]
    ServiceUnavailable,
}

#[async_trait]
pub trait NotificationServiceTrait: Send + Sync {
    async fn send_email(&self, to: &str, subject: &str, body: &str) -> Result<(), NotificationError>;
    async fn send_slack_message(&self, channel: &str, message: &str) -> Result<(), NotificationError>;
    async fn send_webhook(&self, url: &str, payload: Value) -> Result<(), NotificationError>;
    async fn send_sms(&self, phone: &str, message: &str) -> Result<(), NotificationError>;
}

// Test utilities for creating mock data
pub struct MockDataGenerator;

impl MockDataGenerator {
    pub fn create_sample_http_response(status: u16, body: &str) -> HttpResponse {
        HttpResponse {
            status_code: status,
            headers: HashMap::from([
                ("content-type".to_string(), "application/json".to_string()),
                ("content-length".to_string(), body.len().to_string()),
            ]),
            body: body.as_bytes().to_vec(),
            duration: Duration::from_millis(100),
        }
    }
    
    pub fn create_sample_database_row(id: i32, name: &str, active: bool) -> HashMap<String, Value> {
        HashMap::from([
            ("id".to_string(), Value::Number(id.into())),
            ("name".to_string(), Value::String(name.to_string())),
            ("active".to_string(), Value::Bool(active)),
            ("created_at".to_string(), Value::String("2023-01-01T00:00:00Z".to_string())),
        ])
    }
    
    pub fn create_sample_file_metadata(size: u64, is_dir: bool) -> FileMetadata {
        FileMetadata {
            size,
            created: chrono::Utc::now() - chrono::Duration::days(30),
            modified: chrono::Utc::now() - chrono::Duration::days(1),
            is_directory: is_dir,
        }
    }
}

// Test fixtures for common test scenarios
pub struct TestFixtures;

impl TestFixtures {
    pub fn setup_mock_http_client_success() -> MockHttpClient {
        let mut mock = MockHttpClient::new();
        mock.expect_send_request()
            .returning(|_| Ok(MockDataGenerator::create_sample_http_response(200, r#"{"status": "ok"}"#)));
        mock.expect_set_timeout()
            .returning(|_| ());
        mock.expect_set_connection_pool_size()
            .returning(|_| ());
        mock
    }
    
    pub fn setup_mock_http_client_failure() -> MockHttpClient {
        let mut mock = MockHttpClient::new();
        mock.expect_send_request()
            .returning(|_| Err(HttpError::ConnectionFailed("Connection refused".to_string())));
        mock
    }
    
    pub fn setup_mock_database_success() -> MockDatabaseConnection {
        let mut mock = MockDatabaseConnection::new();
        mock.expect_execute_query()
            .returning(|_, _| Ok(vec![
                MockDataGenerator::create_sample_database_row(1, "Test User 1", true),
                MockDataGenerator::create_sample_database_row(2, "Test User 2", false),
            ]));
        mock.expect_test_connection()
            .returning(|| Ok(()));
        mock.expect_get_connection_info()
            .returning(|| ConnectionInfo {
                host: "localhost".to_string(),
                port: 5432,
                database: "testdb".to_string(),
                username: "testuser".to_string(),
            });
        mock
    }
    
    pub fn setup_mock_file_system() -> MockFileSystem {
        let mut mock = MockFileSystem::new();
        mock.expect_read_file()
            .returning(|path| {
                if path.ends_with(".json") {
                    Ok(br#"{"test": "data"}"#.to_vec())
                } else if path.ends_with(".csv") {
                    Ok(b"id,name,active\n1,Test User,true\n2,Another User,false".to_vec())
                } else {
                    Err(FileSystemError::FileNotFound(path.to_string()))
                }
            });
        mock.expect_file_exists()
            .returning(|path| Ok(!path.contains("nonexistent")));
        mock.expect_get_file_metadata()
            .returning(|path| {
                if path.contains("directory") {
                    Ok(MockDataGenerator::create_sample_file_metadata(0, true))
                } else {
                    Ok(MockDataGenerator::create_sample_file_metadata(1024, false))
                }
            });
        mock
    }
    
    pub fn setup_mock_notification_service() -> MockNotificationService {
        let mut mock = MockNotificationService::new();
        mock.expect_send_email()
            .returning(|_, _, _| Ok(()));
        mock.expect_send_slack_message()
            .returning(|_, _| Ok(()));
        mock.expect_send_webhook()
            .returning(|_, _| Ok(()));
        mock.expect_send_sms()
            .returning(|_, _| Ok(()));
        mock
    }
}