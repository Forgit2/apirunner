use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

use crate::error::{PluginError, ProtocolError, DataSourceError};

/// Configuration for plugin initialization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginConfig {
    pub settings: HashMap<String, serde_json::Value>,
    pub environment: String,
}

/// Base trait that all plugins must implement
#[async_trait]
pub trait Plugin: Send + Sync {
    /// Returns the unique name of the plugin
    fn name(&self) -> &str;
    
    /// Returns the version of the plugin
    fn version(&self) -> &str;
    
    /// Returns list of plugin dependencies (by name)
    fn dependencies(&self) -> Vec<String>;
    
    /// Initialize the plugin with configuration
    async fn initialize(&mut self, config: &PluginConfig) -> Result<(), PluginError>;
    
    /// Shutdown the plugin and cleanup resources
    async fn shutdown(&mut self) -> Result<(), PluginError>;
}

/// Protocol versions supported by the system
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ProtocolVersion {
    Http1_0,
    Http1_1,
    Http2,
    Http3,
}

/// HTTP methods supported
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum HttpMethod {
    GET,
    POST,
    PUT,
    DELETE,
    PATCH,
    HEAD,
    OPTIONS,
    TRACE,
    CONNECT,
}

impl std::fmt::Display for HttpMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HttpMethod::GET => write!(f, "GET"),
            HttpMethod::POST => write!(f, "POST"),
            HttpMethod::PUT => write!(f, "PUT"),
            HttpMethod::DELETE => write!(f, "DELETE"),
            HttpMethod::PATCH => write!(f, "PATCH"),
            HttpMethod::HEAD => write!(f, "HEAD"),
            HttpMethod::OPTIONS => write!(f, "OPTIONS"),
            HttpMethod::TRACE => write!(f, "TRACE"),
            HttpMethod::CONNECT => write!(f, "CONNECT"),
        }
    }
}

/// TLS configuration for secure connections
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TlsConfig {
    pub verify_certificates: bool,
    pub client_cert: Option<ClientCertificate>,
    pub ca_certificates: Vec<Certificate>,
    pub min_tls_version: TlsVersion,
    pub max_tls_version: TlsVersion,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientCertificate {
    pub cert_pem: Vec<u8>,
    pub key_pem: Vec<u8>,
    pub password: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Certificate {
    pub pem_data: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TlsVersion {
    Tls1_0,
    Tls1_1,
    Tls1_2,
    Tls1_3,
}

/// Connection configuration for protocol plugins
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionConfig {
    pub base_url: String,
    pub preferred_version: Option<ProtocolVersion>,
    pub tls_config: Option<TlsConfig>,
    pub connection_pool_size: Option<usize>,
    pub keep_alive_timeout: Option<Duration>,
    pub follow_redirects: bool,
    pub max_redirects: u32,
}

/// Request structure for protocol plugins
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolRequest {
    pub method: HttpMethod,
    pub url: String,
    pub headers: HashMap<String, String>,
    pub body: Option<Vec<u8>>,
    pub timeout: Duration,
    pub protocol_version: Option<ProtocolVersion>,
    pub tls_config: Option<TlsConfig>,
}

/// Response structure from protocol plugins
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolResponse {
    pub status_code: u16,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
    pub duration: Duration,
    pub metadata: HashMap<String, String>,
}

/// Protocol plugin trait for handling different API protocols
#[async_trait]
pub trait ProtocolPlugin: Plugin {
    /// Execute a protocol request and return response
    async fn execute_request(&self, request: ProtocolRequest) -> Result<ProtocolResponse, ProtocolError>;
    
    /// Validate connection configuration
    async fn validate_connection(&self, config: &ConnectionConfig) -> Result<(), ProtocolError>;
    
    /// Return supported URL schemes (e.g., "http", "https")
    fn supported_schemes(&self) -> Vec<String>;
    
    /// Return supported protocol versions
    fn supported_versions(&self) -> Vec<ProtocolVersion>;
}

/// Types of assertions supported
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AssertionType {
    StatusCode,
    Header,
    JsonPath,
    XPath,
    ResponseTime,
    Custom(String),
}

/// Expected value for assertions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExpectedValue {
    Exact(serde_json::Value),
    Range { min: f64, max: f64 },
    Pattern(String),
    Contains(String),
    NotEmpty,
}

/// Response data for assertion validation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseData {
    pub status_code: u16,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
    pub duration: Duration,
    pub metadata: HashMap<String, String>,
}

/// Result of assertion execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssertionResult {
    pub success: bool,
    pub message: String,
    pub actual_value: Option<serde_json::Value>,
    pub expected_value: Option<serde_json::Value>,
}

/// Assertion plugin trait for validating responses
#[async_trait]
pub trait AssertionPlugin: Plugin {
    /// Return the type of assertion this plugin handles
    fn assertion_type(&self) -> AssertionType;
    
    /// Execute the assertion against response data
    async fn execute(&self, actual: &ResponseData, expected: &ExpectedValue) -> AssertionResult;
    
    /// Return execution priority (higher numbers execute first)
    fn priority(&self) -> u8;
}

/// Data source types supported
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum DataSourceType {
    Csv,
    Json,
    Yaml,
    Database,
    Api,
    Custom(String),
}

/// Configuration for data sources
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataSourceConfig {
    pub source_type: DataSourceType,
    pub connection_string: String,
    pub parameters: HashMap<String, serde_json::Value>,
    pub batch_size: Option<usize>,
    pub timeout: Option<Duration>,
}

/// Data record from a data source
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataRecord {
    pub id: String,
    pub fields: HashMap<String, serde_json::Value>,
    pub metadata: HashMap<String, String>,
}

/// Data transformation configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataTransformation {
    pub transformation_type: String,
    pub parameters: HashMap<String, serde_json::Value>,
}

/// Data source plugin trait for providing test data
#[async_trait]
pub trait DataSourcePlugin: Plugin {
    /// Return supported data source types
    fn supported_types(&self) -> Vec<DataSourceType>;
    
    /// Connect to the data source
    async fn connect(&mut self, config: &DataSourceConfig) -> Result<(), DataSourceError>;
    
    /// Disconnect from the data source
    async fn disconnect(&mut self) -> Result<(), DataSourceError>;
    
    /// Read data records from the source
    async fn read_data(&self, query: Option<&str>) -> Result<Vec<DataRecord>, DataSourceError>;
    
    /// Apply transformations to data
    async fn transform_data(&self, data: Vec<DataRecord>, transformations: &[DataTransformation]) -> Result<Vec<DataRecord>, DataSourceError>;
    
    /// Validate data source configuration
    fn validate_config(&self, config: &DataSourceConfig) -> Result<(), DataSourceError>;
}