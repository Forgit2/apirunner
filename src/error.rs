use thiserror::Error;
use std::time::Duration;

/// Core error types for the API Test Runner
#[derive(Error, Debug)]
pub enum ApiTestError {
    #[error("Plugin error: {0}")]
    Plugin(#[from] PluginError),
    
    #[error("Protocol error: {0}")]
    Protocol(#[from] ProtocolError),
    
    #[error("Assertion error: {0}")]
    Assertion(#[from] AssertionError),
    
    #[error("Data source error: {0}")]
    DataSource(#[from] DataSourceError),
    
    #[error("Execution error: {0}")]
    Execution(#[from] ExecutionError),
    
    #[error("Configuration error: {0}")]
    Configuration(String),
    
    #[error("Metrics error: {0}")]
    Metrics(#[from] MetricsError),
    
    #[error("Test case error: {0}")]
    TestCase(#[from] TestCaseError),
    
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

/// Plugin-specific errors
#[derive(Error, Debug)]
pub enum PluginError {
    #[error("Plugin not found: {0}")]
    NotFound(String),
    
    #[error("Plugin initialization failed: {0}")]
    InitializationFailed(String),
    
    #[error("Plugin dependency not satisfied: {0}")]
    DependencyNotSatisfied(String),
    
    #[error("Plugin version mismatch: expected {expected}, found {found}")]
    VersionMismatch { expected: String, found: String },
    
    #[error("Plugin loading failed: {0}")]
    LoadingFailed(String),
    
    #[error("Plugin execution failed: {0}")]
    ExecutionFailed(String),
}

/// Protocol-specific errors
#[derive(Error, Debug)]
pub enum ProtocolError {
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),
    
    #[error("Request timeout after {timeout:?}")]
    Timeout { timeout: Duration },
    
    #[error("Invalid URL: {0}")]
    InvalidUrl(String),
    
    #[error("Authentication failed: {0}")]
    AuthenticationFailed(String),
    
    #[error("TLS error: {0}")]
    TlsError(String),
    
    #[error("Protocol version not supported: {0}")]
    UnsupportedVersion(String),
    
    #[error("Network error: {0}")]
    NetworkError(String),
}

/// Assertion-specific errors
#[derive(Error, Debug)]
pub enum AssertionError {
    #[error("Assertion failed: {message}")]
    Failed { message: String },
    
    #[error("Invalid assertion configuration: {0}")]
    InvalidConfiguration(String),
    
    #[error("Assertion timeout: {0}")]
    Timeout(String),
    
    #[error("Type mismatch in assertion: expected {expected}, got {actual}")]
    TypeMismatch { expected: String, actual: String },
}

/// Data source-specific errors
#[derive(Error, Debug)]
pub enum DataSourceError {
    #[error("Data source not found: {0}")]
    NotFound(String),
    
    #[error("Connection error: {0}")]
    ConnectionError(String),
    
    #[error("Query error: {0}")]
    QueryError(String),
    
    #[error("Data transformation failed: {0}")]
    TransformationFailed(String),
    
    #[error("Invalid data format: {0}")]
    InvalidFormat(String),
    
    #[error("File error: {0}")]
    FileError(String),
    
    #[error("Parse error: {0}")]
    ParseError(String),
    
    #[error("Configuration error: {0}")]
    ConfigError(String),
    
    #[error("Transaction error: {0}")]
    TransactionError(String),
}

/// Execution-specific errors
#[derive(Error, Debug)]
pub enum ExecutionError {
    #[error("Test case execution failed: {0}")]
    TestCaseFailed(String),
    
    #[error("Execution strategy not supported: {0}")]
    UnsupportedStrategy(String),
    
    #[error("Resource exhausted: {0}")]
    ResourceExhausted(String),
    
    #[error("Execution cancelled by user")]
    Cancelled,
    
    #[error("Distributed execution node failure: {0}")]
    NodeFailure(String),
}

/// Metrics-specific errors
#[derive(Error, Debug)]
pub enum MetricsError {
    #[error("Metrics collection failed: {0}")]
    CollectionFailed(String),
    
    #[error("Prometheus registry error: {0}")]
    RegistryError(String),
    
    #[error("Metrics export failed: {0}")]
    ExportFailed(String),
    
    #[error("Invalid metrics configuration: {0}")]
    InvalidConfiguration(String),
    
    #[error("System metrics unavailable: {0}")]
    SystemMetricsUnavailable(String),
}

/// Test case management errors
#[derive(Error, Debug)]
pub enum TestCaseError {
    #[error("Test case not found: {0}")]
    NotFound(String),
    
    #[error("Test case already exists: {0}")]
    AlreadyExists(String),
    
    #[error("Validation error: {0}")]
    ValidationError(String),
    
    #[error("Storage error: {0}")]
    StorageError(String),
    
    #[error("Serialization error: {0}")]
    SerializationError(String),
    
    #[error("Import error: {0}")]
    ImportError(String),
    
    #[error("Export error: {0}")]
    ExportError(String),
    
    #[error("Template error: {0}")]
    TemplateError(String),
    
    #[error("Search error: {0}")]
    SearchError(String),
}

impl From<serde_json::Error> for TestCaseError {
    fn from(err: serde_json::Error) -> Self {
        TestCaseError::SerializationError(err.to_string())
    }
}

impl From<serde_yaml::Error> for TestCaseError {
    fn from(err: serde_yaml::Error) -> Self {
        TestCaseError::SerializationError(err.to_string())
    }
}

/// Result type alias for convenience
pub type Result<T> = std::result::Result<T, ApiTestError>;

// Conversion implementations for external crate errors
impl From<prometheus::Error> for MetricsError {
    fn from(err: prometheus::Error) -> Self {
        MetricsError::RegistryError(err.to_string())
    }
}

impl From<std::string::FromUtf8Error> for MetricsError {
    fn from(err: std::string::FromUtf8Error) -> Self {
        MetricsError::ExportFailed(err.to_string())
    }
}

impl From<prometheus::Error> for ApiTestError {
    fn from(err: prometheus::Error) -> Self {
        ApiTestError::Metrics(MetricsError::RegistryError(err.to_string()))
    }
}

impl From<std::string::FromUtf8Error> for ApiTestError {
    fn from(err: std::string::FromUtf8Error) -> Self {
        ApiTestError::Metrics(MetricsError::ExportFailed(err.to_string()))
    }
}