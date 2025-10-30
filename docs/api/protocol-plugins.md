# Protocol Plugins

Protocol plugins handle communication with specific protocols and services. They extend the base `Plugin` trait with protocol-specific functionality.

## ProtocolPlugin Trait

```rust
#[async_trait]
pub trait ProtocolPlugin: Plugin {
    /// Execute a protocol request and return the response
    async fn execute_request(&self, request: ProtocolRequest) -> Result<ProtocolResponse, ProtocolError>;
    
    /// Validate that a connection can be established with the given configuration
    async fn validate_connection(&self, config: &ConnectionConfig) -> Result<(), ProtocolError>;
    
    /// Return the list of URL schemes this plugin supports (http, https, ws, etc.)
    fn supported_schemes(&self) -> Vec<String>;
    
    /// Return the list of protocol versions this plugin supports
    fn supported_versions(&self) -> Vec<ProtocolVersion>;
}
```

## Protocol Request

```rust
#[derive(Debug, Clone)]
pub struct ProtocolRequest {
    /// HTTP method (GET, POST, etc.)
    pub method: HttpMethod,
    /// Target URL
    pub url: String,
    /// Request headers
    pub headers: HashMap<String, String>,
    /// Request body (optional)
    pub body: Option<Vec<u8>>,
    /// Request timeout
    pub timeout: Duration,
    /// Preferred protocol version
    pub protocol_version: Option<ProtocolVersion>,
    /// TLS configuration for HTTPS
    pub tls_config: Option<TlsConfig>,
}
```

## Protocol Response

```rust
#[derive(Debug, Clone)]
pub struct ProtocolResponse {
    /// HTTP status code
    pub status_code: u16,
    /// Response headers
    pub headers: HashMap<String, String>,
    /// Response body
    pub body: Vec<u8>,
    /// Request duration
    pub duration: Duration,
    /// Additional metadata
    pub metadata: HashMap<String, String>,
}
```

## HTTP Methods

```rust
#[derive(Debug, Clone, PartialEq)]
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
```

## Protocol Versions

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum ProtocolVersion {
    Http1_0,
    Http1_1,
    Http2,
    Http3,
}
```

## TLS Configuration

```rust
#[derive(Debug, Clone)]
pub struct TlsConfig {
    /// Whether to verify server certificates
    pub verify_certificates: bool,
    /// Client certificate for mutual TLS
    pub client_cert: Option<ClientCertificate>,
    /// Additional CA certificates to trust
    pub ca_certificates: Vec<Certificate>,
    /// Minimum TLS version to accept
    pub min_tls_version: TlsVersion,
    /// Maximum TLS version to use
    pub max_tls_version: TlsVersion,
}

#[derive(Debug, Clone)]
pub enum TlsVersion {
    Tls1_0,
    Tls1_1,
    Tls1_2,
    Tls1_3,
}

#[derive(Debug, Clone)]
pub struct ClientCertificate {
    pub cert_pem: Vec<u8>,
    pub key_pem: Vec<u8>,
    pub password: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Certificate {
    pub pem_data: Vec<u8>,
}
```

## Connection Configuration

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionConfig {
    /// Base URL for the service
    pub base_url: String,
    /// Preferred protocol version
    pub preferred_version: Option<ProtocolVersion>,
    /// TLS configuration
    pub tls_config: Option<TlsConfig>,
    /// Connection pool size
    pub connection_pool_size: Option<usize>,
    /// Keep-alive timeout
    pub keep_alive_timeout: Option<Duration>,
    /// Whether to follow redirects
    pub follow_redirects: bool,
    /// Maximum number of redirects to follow
    pub max_redirects: u32,
}
```

## Error Types

```rust
#[derive(Debug, thiserror::Error)]
pub enum ProtocolError {
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),
    
    #[error("Request timeout")]
    Timeout,
    
    #[error("Invalid URL: {0}")]
    InvalidUrl(String),
    
    #[error("TLS error: {0}")]
    TlsError(String),
    
    #[error("Protocol error: {0}")]
    ProtocolError(String),
    
    #[error("Serialization error: {0}")]
    SerializationError(String),
}
```

## Built-in Protocol Plugins

### REST API Plugin

The built-in REST API plugin supports:
- HTTP/1.1 and HTTP/2
- All standard HTTP methods
- TLS with client certificates
- Connection pooling
- Automatic retries
- Request/response compression

Example usage:
```rust
let rest_plugin = RestApiPlugin::new();
let request = ProtocolRequest {
    method: HttpMethod::GET,
    url: "https://api.example.com/users".to_string(),
    headers: HashMap::from([
        ("Accept".to_string(), "application/json".to_string()),
    ]),
    body: None,
    timeout: Duration::from_secs(30),
    protocol_version: Some(ProtocolVersion::Http2),
    tls_config: None,
};

let response = rest_plugin.execute_request(request).await?;
```

## Implementing a Protocol Plugin

1. **Create the Plugin Structure**:
```rust
pub struct MyProtocolPlugin {
    client: Arc<MyClient>,
    config: PluginConfig,
}
```

2. **Implement the Plugin Trait**:
```rust
#[async_trait]
impl Plugin for MyProtocolPlugin {
    fn name(&self) -> &str { "my-protocol" }
    fn version(&self) -> &str { "1.0.0" }
    fn dependencies(&self) -> Vec<String> { vec![] }
    
    async fn initialize(&mut self, config: &PluginConfig) -> Result<(), PluginError> {
        // Initialize client with configuration
        Ok(())
    }
    
    async fn shutdown(&mut self) -> Result<(), PluginError> {
        // Clean up resources
        Ok(())
    }
}
```

3. **Implement the ProtocolPlugin Trait**:
```rust
#[async_trait]
impl ProtocolPlugin for MyProtocolPlugin {
    async fn execute_request(&self, request: ProtocolRequest) -> Result<ProtocolResponse, ProtocolError> {
        // Execute the protocol-specific request
        todo!()
    }
    
    async fn validate_connection(&self, config: &ConnectionConfig) -> Result<(), ProtocolError> {
        // Test connectivity
        todo!()
    }
    
    fn supported_schemes(&self) -> Vec<String> {
        vec!["myprotocol".to_string()]
    }
    
    fn supported_versions(&self) -> Vec<ProtocolVersion> {
        vec![ProtocolVersion::Http1_1]
    }
}
```

## Best Practices

### Performance
- Use connection pooling for network requests
- Implement proper timeout handling
- Support request/response streaming for large payloads
- Cache connection metadata when possible

### Error Handling
- Provide detailed error messages with context
- Distinguish between retryable and non-retryable errors
- Handle network failures gracefully
- Implement circuit breaker patterns for reliability

### Security
- Validate TLS certificates by default
- Support client certificate authentication
- Handle sensitive data (passwords, tokens) securely
- Implement proper authentication token refresh

### Configuration
- Provide sensible defaults for all settings
- Validate configuration during initialization
- Support environment-specific overrides
- Document all configuration options