use async_trait::async_trait;
use reqwest::{Client, ClientBuilder, Method, RequestBuilder, Response};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::error::{PluginError, ProtocolError};
use crate::plugin::{
    AssertionPlugin, AssertionResult, AssertionType, ConnectionConfig, ExpectedValue, HttpMethod, Plugin, PluginConfig,
    ProtocolPlugin, ProtocolRequest, ProtocolResponse, ProtocolVersion, ResponseData, TlsConfig,
};

/// REST API Protocol Plugin with connection pooling and advanced HTTP features
#[derive(Debug)]
pub struct RestApiPlugin {
    client: Option<Arc<Client>>,
    connection_pool_size: usize,
    default_timeout: Duration,
    retry_config: RetryConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryConfig {
    pub max_retries: u32,
    pub initial_delay: Duration,
    pub max_delay: Duration,
    pub backoff_multiplier: f64,
    pub retry_on_status: Vec<u16>,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            initial_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(30),
            backoff_multiplier: 2.0,
            retry_on_status: vec![429, 502, 503, 504],
        }
    }
}

impl Default for RestApiPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl RestApiPlugin {
    /// Create a new REST API plugin with default configuration
    pub fn new() -> Self {
        Self {
            client: None,
            connection_pool_size: 10,
            default_timeout: Duration::from_secs(30),
            retry_config: RetryConfig::default(),
        }
    }

    /// Create a new REST API plugin with custom configuration
    pub fn with_config(
        connection_pool_size: usize,
        default_timeout: Duration,
        retry_config: RetryConfig,
    ) -> Self {
        Self {
            client: None,
            connection_pool_size,
            default_timeout,
            retry_config,
        }
    }

    /// Get the connection pool size
    pub fn connection_pool_size(&self) -> usize {
        self.connection_pool_size
    }

    /// Get the default timeout
    pub fn default_timeout(&self) -> Duration {
        self.default_timeout
    }

    /// Get the retry configuration
    pub fn retry_config(&self) -> &RetryConfig {
        &self.retry_config
    }

    /// Build the HTTP client with connection pooling and configuration
    fn build_client(&self, config: &PluginConfig) -> Result<Client, PluginError> {
        let mut builder = ClientBuilder::new()
            .pool_max_idle_per_host(self.connection_pool_size)
            .pool_idle_timeout(Duration::from_secs(30))
            .timeout(self.default_timeout)
            .tcp_keepalive(Duration::from_secs(60))
            .http2_keep_alive_interval(Duration::from_secs(30))
            .http2_keep_alive_timeout(Duration::from_secs(10))
            .http2_keep_alive_while_idle(true);

        // Configure user agent
        builder = builder.user_agent(format!("API-Test-Runner/{}", env!("CARGO_PKG_VERSION")));

        // Configure redirect policy
        builder = builder.redirect(reqwest::redirect::Policy::limited(10));

        // Configure TLS settings from plugin config
        if let Some(tls_settings) = config.settings.get("tls") {
            builder = self.configure_client_tls(builder, tls_settings)?;
        }

        // Configure HTTP version preferences
        if let Some(http_version) = config.settings.get("http_version") {
            builder = self.configure_http_version(builder, http_version)?;
        }

        // Build client
        builder
            .build()
            .map_err(|e| PluginError::InitializationFailed(format!("Failed to build HTTP client: {}", e)))
    }

    /// Configure TLS settings for the client builder
    fn configure_client_tls(
        &self,
        mut builder: ClientBuilder,
        tls_settings: &serde_json::Value,
    ) -> Result<ClientBuilder, PluginError> {
        // Disable certificate verification if requested
        if let Some(verify_certs) = tls_settings.get("verify_certificates") {
            if let Some(false) = verify_certs.as_bool() {
                builder = builder.danger_accept_invalid_certs(true);
                log::warn!("TLS certificate verification disabled - use only for testing!");
            }
        }

        // Configure client certificate authentication
        if let Some(client_cert) = tls_settings.get("client_certificate") {
            if let Some(cert_path) = client_cert.get("cert_path") {
                if let Some(key_path) = client_cert.get("key_path") {
                    if let (Some(cert_str), Some(key_str)) = (cert_path.as_str(), key_path.as_str()) {
                        match self.load_client_certificate(cert_str, key_str) {
                            Ok(identity) => {
                                builder = builder.identity(identity);
                                log::info!("Client certificate loaded successfully");
                            }
                            Err(e) => {
                                return Err(PluginError::InitializationFailed(format!(
                                    "Failed to load client certificate: {}", e
                                )));
                            }
                        }
                    }
                }
            }
        }

        // Configure custom CA certificates
        if let Some(ca_certs) = tls_settings.get("ca_certificates") {
            if let Some(ca_array) = ca_certs.as_array() {
                for ca_cert in ca_array {
                    if let Some(ca_path) = ca_cert.as_str() {
                        match self.load_ca_certificate(ca_path) {
                            Ok(cert) => {
                                builder = builder.add_root_certificate(cert);
                                log::info!("CA certificate loaded: {}", ca_path);
                            }
                            Err(e) => {
                                log::warn!("Failed to load CA certificate {}: {}", ca_path, e);
                            }
                        }
                    }
                }
            }
        }

        // Configure TLS version - prefer minimum version setting
        if let Some(min_tls) = tls_settings.get("min_tls_version") {
            if let Some(version_str) = min_tls.as_str() {
                match version_str {
                    "1.2" => {
                        builder = builder.min_tls_version(reqwest::tls::Version::TLS_1_2);
                    }
                    "1.3" => {
                        builder = builder.min_tls_version(reqwest::tls::Version::TLS_1_3);
                    }
                    _ => {
                        log::warn!("Unsupported minimum TLS version: {}", version_str);
                    }
                }
            }
        } else if let Some(max_tls) = tls_settings.get("max_tls_version") {
            // Only set max version if min version is not set to avoid conflicts
            if let Some(version_str) = max_tls.as_str() {
                match version_str {
                    "1.2" => {
                        builder = builder.max_tls_version(reqwest::tls::Version::TLS_1_2);
                    }
                    "1.3" => {
                        builder = builder.max_tls_version(reqwest::tls::Version::TLS_1_3);
                    }
                    _ => {
                        log::warn!("Unsupported maximum TLS version: {}", version_str);
                    }
                }
            }
        }

        Ok(builder)
    }

    /// Load client certificate from file paths
    fn load_client_certificate(&self, cert_path: &str, key_path: &str) -> Result<reqwest::Identity, PluginError> {
        use std::fs;
        
        // Read certificate and key files
        let cert_pem = fs::read(cert_path)
            .map_err(|e| PluginError::InitializationFailed(format!("Failed to read certificate file {}: {}", cert_path, e)))?;
        
        let key_pem = fs::read(key_path)
            .map_err(|e| PluginError::InitializationFailed(format!("Failed to read key file {}: {}", key_path, e)))?;

        // Combine certificate and key
        let mut combined_pem = cert_pem;
        combined_pem.extend_from_slice(&key_pem);

        // Create identity
        reqwest::Identity::from_pem(&combined_pem)
            .map_err(|e| PluginError::InitializationFailed(format!("Failed to create identity from PEM: {}", e)))
    }

    /// Load CA certificate from file path
    fn load_ca_certificate(&self, ca_path: &str) -> Result<reqwest::Certificate, PluginError> {
        use std::fs;
        
        let ca_pem = fs::read(ca_path)
            .map_err(|e| PluginError::InitializationFailed(format!("Failed to read CA certificate file {}: {}", ca_path, e)))?;

        reqwest::Certificate::from_pem(&ca_pem)
            .map_err(|e| PluginError::InitializationFailed(format!("Failed to create certificate from PEM: {}", e)))
    }

    /// Configure HTTP version preferences for the client builder
    fn configure_http_version(
        &self,
        mut builder: ClientBuilder,
        http_version: &serde_json::Value,
    ) -> Result<ClientBuilder, PluginError> {
        if let Some(version_str) = http_version.as_str() {
            match version_str {
                "1.1" => {
                    builder = builder.http1_only();
                }
                "2" => {
                    builder = builder.http2_prior_knowledge();
                }
                "auto" => {
                    // Default behavior - let reqwest negotiate
                }
                _ => {
                    log::warn!("Unsupported HTTP version preference: {}", version_str);
                }
            }
        }

        Ok(builder)
    }

    /// Configure TLS settings for the request builder
    fn configure_tls(
        &self,
        builder: RequestBuilder,
        tls_config: &TlsConfig,
    ) -> Result<RequestBuilder, ProtocolError> {
        // Note: Most TLS configuration is done at the client level during initialization
        // This method handles per-request TLS settings that can be configured
        
        let request_builder = builder;

        // Log TLS configuration being used
        if !tls_config.verify_certificates {
            log::debug!("Request using TLS with certificate verification disabled");
        }

        if tls_config.client_cert.is_some() {
            log::debug!("Request using client certificate authentication");
        }

        if !tls_config.ca_certificates.is_empty() {
            log::debug!("Request using custom CA certificates");
        }

        // TLS version preferences are handled at the client level
        log::debug!("TLS version range: {:?} to {:?}", tls_config.min_tls_version, tls_config.max_tls_version);

        Ok(request_builder)
    }

    /// Convert HttpMethod to reqwest Method
    fn convert_method(&self, method: &HttpMethod) -> Method {
        match method {
            HttpMethod::GET => Method::GET,
            HttpMethod::POST => Method::POST,
            HttpMethod::PUT => Method::PUT,
            HttpMethod::DELETE => Method::DELETE,
            HttpMethod::PATCH => Method::PATCH,
            HttpMethod::HEAD => Method::HEAD,
            HttpMethod::OPTIONS => Method::OPTIONS,
            HttpMethod::TRACE => Method::TRACE,
            HttpMethod::CONNECT => Method::CONNECT,
        }
    }

    /// Convert ProtocolVersion to reqwest Version
    pub fn convert_version(&self, version: &ProtocolVersion) -> reqwest::Version {
        match version {
            ProtocolVersion::Http1_0 => reqwest::Version::HTTP_10,
            ProtocolVersion::Http1_1 => reqwest::Version::HTTP_11,
            ProtocolVersion::Http2 => reqwest::Version::HTTP_2,
            ProtocolVersion::Http3 => reqwest::Version::HTTP_3,
        }
    }

    /// Extract headers from response
    fn extract_headers(&self, response: &Response) -> HashMap<String, String> {
        let mut headers = HashMap::new();
        for (name, value) in response.headers() {
            if let Ok(value_str) = value.to_str() {
                headers.insert(name.to_string(), value_str.to_string());
            }
        }
        headers
    }

    /// Extract metadata from response
    fn extract_metadata(&self, response: &Response) -> HashMap<String, String> {
        let mut metadata = HashMap::new();
        
        metadata.insert("version".to_string(), format!("{:?}", response.version()));
        metadata.insert("status".to_string(), response.status().to_string());
        
        if let Some(remote_addr) = response.remote_addr() {
            metadata.insert("remote_addr".to_string(), remote_addr.to_string());
        }
        
        metadata
    }

    /// Execute HTTP request with retry logic
    async fn execute_with_retry(
        &self,
        request: &ProtocolRequest,
    ) -> Result<ProtocolResponse, ProtocolError> {
        let client = self.client.as_ref().ok_or_else(|| {
            ProtocolError::ConnectionFailed("Client not initialized".to_string())
        })?;

        let mut attempt = 0;
        let mut delay = self.retry_config.initial_delay;

        loop {
            match self.execute_single_request(client, request).await {
                Ok(response) => return Ok(response),
                Err(e) => {
                    attempt += 1;
                    
                    // Check if we should retry
                    if attempt > self.retry_config.max_retries {
                        return Err(e);
                    }

                    // Check if error is retryable
                    let should_retry = match &e {
                        ProtocolError::ConnectionFailed(msg) => {
                            // Check if it's a network error or timeout
                            msg.contains("timeout") || msg.contains("connection")
                        }
                        ProtocolError::NetworkError(msg) => {
                            // Network errors are generally retryable
                            true
                        }
                        _ => false,
                    };

                    if !should_retry {
                        return Err(e);
                    }

                    // Wait before retry
                    tokio::time::sleep(delay).await;
                    
                    // Exponential backoff
                    delay = std::cmp::min(
                        Duration::from_millis(
                            (delay.as_millis() as f64 * self.retry_config.backoff_multiplier) as u64
                        ),
                        self.retry_config.max_delay,
                    );
                }
            }
        }
    }

    /// Execute a single HTTP request
    async fn execute_single_request(
        &self,
        client: &Client,
        request: &ProtocolRequest,
    ) -> Result<ProtocolResponse, ProtocolError> {
        let method = self.convert_method(&request.method);
        let mut req_builder = client.request(method, &request.url);

        // Set timeout
        req_builder = req_builder.timeout(request.timeout);

        // Set HTTP version if specified
        if let Some(version) = &request.protocol_version {
            req_builder = req_builder.version(self.convert_version(version));
        }

        // Configure TLS if present
        if let Some(tls_config) = &request.tls_config {
            req_builder = self.configure_tls(req_builder, tls_config)?;
        }

        // Add headers
        for (key, value) in &request.headers {
            req_builder = req_builder.header(key, value);
        }

        // Add body if present
        if let Some(body) = &request.body {
            req_builder = req_builder.body(body.clone());
        }

        // Execute request and measure time
        let start_time = Instant::now();
        let response = req_builder.send().await.map_err(|e| {
            if e.is_timeout() {
                ProtocolError::Timeout { timeout: request.timeout }
            } else if e.is_connect() {
                ProtocolError::ConnectionFailed(e.to_string())
            } else {
                ProtocolError::NetworkError(e.to_string())
            }
        })?;

        let duration = start_time.elapsed();
        let status_code = response.status().as_u16();

        // Check for HTTP error status codes
        if response.status().is_client_error() || response.status().is_server_error() {
            // Still process the response but note the error status
            log::debug!("HTTP error status: {}", status_code);
        }

        // Extract response data
        let headers = self.extract_headers(&response);
        let metadata = self.extract_metadata(&response);
        
        let body = response.bytes().await.map_err(|e| {
            ProtocolError::NetworkError(format!("Failed to read response body: {}", e))
        })?.to_vec();

        Ok(ProtocolResponse {
            status_code,
            headers,
            body,
            duration,
            metadata,
        })
    }
}

#[async_trait]
impl Plugin for RestApiPlugin {
    fn name(&self) -> &str {
        "rest-api"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn dependencies(&self) -> Vec<String> {
        vec![]
    }

    async fn initialize(&mut self, config: &PluginConfig) -> Result<(), PluginError> {
        // Extract configuration from settings
        if let Some(pool_size) = config.settings.get("connection_pool_size") {
            if let Some(size) = pool_size.as_u64() {
                self.connection_pool_size = size as usize;
            }
        }

        if let Some(timeout) = config.settings.get("default_timeout_ms") {
            if let Some(timeout_ms) = timeout.as_u64() {
                self.default_timeout = Duration::from_millis(timeout_ms);
            }
        }

        // Build and store the HTTP client
        let client = self.build_client(config)?;
        self.client = Some(Arc::new(client));

        log::info!(
            "REST API plugin initialized with pool size: {}, timeout: {:?}",
            self.connection_pool_size,
            self.default_timeout
        );

        Ok(())
    }

    async fn shutdown(&mut self) -> Result<(), PluginError> {
        self.client = None;
        log::info!("REST API plugin shutdown completed");
        Ok(())
    }
}

#[async_trait]
impl ProtocolPlugin for RestApiPlugin {
    async fn execute_request(
        &self,
        request: ProtocolRequest,
    ) -> Result<ProtocolResponse, ProtocolError> {
        self.execute_with_retry(&request).await
    }

    async fn validate_connection(&self, config: &ConnectionConfig) -> Result<(), ProtocolError> {
        let client = self.client.as_ref().ok_or_else(|| {
            ProtocolError::ConnectionFailed("Client not initialized".to_string())
        })?;

        // Perform a HEAD request to validate connectivity
        let test_request = ProtocolRequest {
            method: HttpMethod::HEAD,
            url: config.base_url.clone(),
            headers: HashMap::new(),
            body: None,
            timeout: Duration::from_secs(5),
            protocol_version: config.preferred_version.clone(),
            tls_config: config.tls_config.clone(),
        };

        match self.execute_single_request(client, &test_request).await {
            Ok(_) => Ok(()),
            Err(e) => Err(ProtocolError::ConnectionFailed(format!(
                "Connection validation failed: {}",
                e
            ))),
        }
    }

    fn supported_schemes(&self) -> Vec<String> {
        vec!["http".to_string(), "https".to_string()]
    }

    fn supported_versions(&self) -> Vec<ProtocolVersion> {
        vec![
            ProtocolVersion::Http1_0,
            ProtocolVersion::Http1_1,
            ProtocolVersion::Http2,
            // HTTP/3 support is experimental in reqwest
        ]
    }
}

/// HTTP Status Code Assertion Plugin
#[derive(Debug, Default)]
pub struct HttpStatusAssertion;

#[async_trait]
impl Plugin for HttpStatusAssertion {
    fn name(&self) -> &str {
        "http-status-assertion"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn dependencies(&self) -> Vec<String> {
        vec![]
    }

    async fn initialize(&mut self, _config: &PluginConfig) -> Result<(), PluginError> {
        Ok(())
    }

    async fn shutdown(&mut self) -> Result<(), PluginError> {
        Ok(())
    }
}

#[async_trait]
impl AssertionPlugin for HttpStatusAssertion {
    fn assertion_type(&self) -> AssertionType {
        AssertionType::StatusCode
    }

    async fn execute(&self, actual: &ResponseData, expected: &ExpectedValue) -> AssertionResult {
        let actual_status = actual.status_code;

        match expected {
            ExpectedValue::Exact(value) => {
                if let Some(expected_status) = value.as_u64() {
                    let expected_status = expected_status as u16;
                    let success = actual_status == expected_status;
                    
                    AssertionResult {
                        success,
                        message: if success {
                            format!("Status code matches expected value: {}", expected_status)
                        } else {
                            format!(
                                "Status code mismatch: expected {}, got {}",
                                expected_status, actual_status
                            )
                        },
                        actual_value: Some(serde_json::Value::Number(actual_status.into())),
                        expected_value: Some(value.clone()),
                    }
                } else {
                    AssertionResult {
                        success: false,
                        message: "Expected value is not a valid status code number".to_string(),
                        actual_value: Some(serde_json::Value::Number(actual_status.into())),
                        expected_value: Some(IntoJsonValue::into(expected.clone())),
                    }
                }
            }
            ExpectedValue::Range { min, max } => {
                let status_f64 = actual_status as f64;
                let success = status_f64 >= *min && status_f64 <= *max;
                
                AssertionResult {
                    success,
                    message: if success {
                        format!("Status code {} is within range [{}, {}]", actual_status, min, max)
                    } else {
                        format!(
                            "Status code {} is outside range [{}, {}]",
                            actual_status, min, max
                        )
                    },
                    actual_value: Some(serde_json::Value::Number(actual_status.into())),
                    expected_value: Some(serde_json::json!({"min": min, "max": max})),
                }
            }
            _ => AssertionResult {
                success: false,
                message: "Unsupported expected value type for status code assertion".to_string(),
                actual_value: Some(serde_json::Value::Number(actual_status.into())),
                expected_value: Some(IntoJsonValue::into(expected.clone())),
            },
        }
    }

    fn priority(&self) -> u8 {
        100 // High priority for basic HTTP validation
    }
}

/// HTTP Header Assertion Plugin
#[derive(Debug, Default)]
pub struct HttpHeaderAssertion {
    header_name: String,
}

impl HttpHeaderAssertion {
    pub fn new(header_name: String) -> Self {
        Self { header_name }
    }
}

#[async_trait]
impl Plugin for HttpHeaderAssertion {
    fn name(&self) -> &str {
        "http-header-assertion"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn dependencies(&self) -> Vec<String> {
        vec![]
    }

    async fn initialize(&mut self, config: &PluginConfig) -> Result<(), PluginError> {
        if let Some(header_name) = config.settings.get("header_name") {
            if let Some(name) = header_name.as_str() {
                self.header_name = name.to_string();
            }
        }
        Ok(())
    }

    async fn shutdown(&mut self) -> Result<(), PluginError> {
        Ok(())
    }
}

#[async_trait]
impl AssertionPlugin for HttpHeaderAssertion {
    fn assertion_type(&self) -> AssertionType {
        AssertionType::Header
    }

    async fn execute(&self, actual: &ResponseData, expected: &ExpectedValue) -> AssertionResult {
        let actual_value = actual.headers.get(&self.header_name);

        match expected {
            ExpectedValue::Exact(value) => {
                if let Some(expected_str) = value.as_str() {
                    let success = actual_value.map(|v| v == expected_str).unwrap_or(false);
                    
                    AssertionResult {
                        success,
                        message: if success {
                            format!("Header '{}' matches expected value", self.header_name)
                        } else {
                            format!(
                                "Header '{}' mismatch: expected '{}', got '{}'",
                                self.header_name,
                                expected_str,
                                actual_value.unwrap_or(&"<missing>".to_string())
                            )
                        },
                        actual_value: actual_value.map(|v| serde_json::Value::String(v.clone())),
                        expected_value: Some(value.clone()),
                    }
                } else {
                    AssertionResult {
                        success: false,
                        message: "Expected value is not a string".to_string(),
                        actual_value: actual_value.map(|v| serde_json::Value::String(v.clone())),
                        expected_value: Some(value.clone()),
                    }
                }
            }
            ExpectedValue::Contains(substring) => {
                let success = actual_value
                    .map(|v| v.contains(substring))
                    .unwrap_or(false);
                
                AssertionResult {
                    success,
                    message: if success {
                        format!("Header '{}' contains expected substring", self.header_name)
                    } else {
                        format!(
                            "Header '{}' does not contain '{}', got '{}'",
                            self.header_name,
                            substring,
                            actual_value.unwrap_or(&"<missing>".to_string())
                        )
                    },
                    actual_value: actual_value.map(|v| serde_json::Value::String(v.clone())),
                    expected_value: Some(serde_json::Value::String(substring.clone())),
                }
            }
            ExpectedValue::NotEmpty => {
                let success = actual_value.map(|v| !v.is_empty()).unwrap_or(false);
                
                AssertionResult {
                    success,
                    message: if success {
                        format!("Header '{}' is not empty", self.header_name)
                    } else {
                        format!("Header '{}' is empty or missing", self.header_name)
                    },
                    actual_value: actual_value.map(|v| serde_json::Value::String(v.clone())),
                    expected_value: Some(serde_json::Value::String("not_empty".to_string())),
                }
            }
            _ => AssertionResult {
                success: false,
                message: "Unsupported expected value type for header assertion".to_string(),
                actual_value: actual_value.map(|v| serde_json::Value::String(v.clone())),
                expected_value: Some(IntoJsonValue::into(expected.clone())),
            },
        }
    }

    fn priority(&self) -> u8 {
        80 // Medium-high priority
    }
}

/// Response Time Assertion Plugin
#[derive(Debug, Default)]
pub struct ResponseTimeAssertion;

#[async_trait]
impl Plugin for ResponseTimeAssertion {
    fn name(&self) -> &str {
        "response-time-assertion"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn dependencies(&self) -> Vec<String> {
        vec![]
    }

    async fn initialize(&mut self, _config: &PluginConfig) -> Result<(), PluginError> {
        Ok(())
    }

    async fn shutdown(&mut self) -> Result<(), PluginError> {
        Ok(())
    }
}

#[async_trait]
impl AssertionPlugin for ResponseTimeAssertion {
    fn assertion_type(&self) -> AssertionType {
        AssertionType::ResponseTime
    }

    async fn execute(&self, actual: &ResponseData, expected: &ExpectedValue) -> AssertionResult {
        let actual_ms = actual.duration.as_millis() as f64;

        match expected {
            ExpectedValue::Exact(value) => {
                if let Some(expected_ms) = value.as_f64() {
                    let success = (actual_ms - expected_ms).abs() < 1.0; // 1ms tolerance
                    
                    AssertionResult {
                        success,
                        message: if success {
                            format!("Response time matches expected value: {}ms", expected_ms)
                        } else {
                            format!(
                                "Response time mismatch: expected {}ms, got {}ms",
                                expected_ms, actual_ms
                            )
                        },
                        actual_value: Some(serde_json::Value::Number(
                            serde_json::Number::from_f64(actual_ms).unwrap()
                        )),
                        expected_value: Some(value.clone()),
                    }
                } else {
                    AssertionResult {
                        success: false,
                        message: "Expected value is not a valid number".to_string(),
                        actual_value: Some(serde_json::Value::Number(
                            serde_json::Number::from_f64(actual_ms).unwrap()
                        )),
                        expected_value: Some(value.clone()),
                    }
                }
            }
            ExpectedValue::Range { min, max } => {
                let success = actual_ms >= *min && actual_ms <= *max;
                
                AssertionResult {
                    success,
                    message: if success {
                        format!("Response time {}ms is within range [{}, {}]ms", actual_ms, min, max)
                    } else {
                        format!(
                            "Response time {}ms is outside range [{}, {}]ms",
                            actual_ms, min, max
                        )
                    },
                    actual_value: Some(serde_json::Value::Number(
                        serde_json::Number::from_f64(actual_ms).unwrap()
                    )),
                    expected_value: Some(serde_json::json!({"min": min, "max": max})),
                }
            }
            _ => AssertionResult {
                success: false,
                message: "Unsupported expected value type for response time assertion".to_string(),
                actual_value: Some(serde_json::Value::Number(
                    serde_json::Number::from_f64(actual_ms).unwrap()
                )),
                expected_value: Some(IntoJsonValue::into(expected.clone())),
            },
        }
    }

    fn priority(&self) -> u8 {
        60 // Medium priority
    }
}

// Helper trait to convert ExpectedValue to serde_json::Value
trait IntoJsonValue {
    fn into(self) -> serde_json::Value;
}

impl IntoJsonValue for ExpectedValue {
    fn into(self) -> serde_json::Value {
        match self {
            ExpectedValue::Exact(value) => value,
            ExpectedValue::Range { min, max } => serde_json::json!({"min": min, "max": max}),
            ExpectedValue::Pattern(pattern) => serde_json::Value::String(pattern),
            ExpectedValue::Contains(substring) => serde_json::Value::String(substring),
            ExpectedValue::NotEmpty => serde_json::Value::String("not_empty".to_string()),
        }
    }
}