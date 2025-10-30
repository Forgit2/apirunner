use super::*;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Mock HTTP client that simulates various network conditions
pub struct MockHttpClientWithConditions {
    responses: Arc<Mutex<Vec<HttpResponse>>>,
    errors: Arc<Mutex<Vec<HttpError>>>,
    latency: Duration,
    failure_rate: f64,
}

impl MockHttpClientWithConditions {
    pub fn new() -> Self {
        Self {
            responses: Arc::new(Mutex::new(Vec::new())),
            errors: Arc::new(Mutex::new(Vec::new())),
            latency: Duration::from_millis(100),
            failure_rate: 0.0,
        }
    }
    
    pub fn with_latency(mut self, latency: Duration) -> Self {
        self.latency = latency;
        self
    }
    
    pub fn with_failure_rate(mut self, rate: f64) -> Self {
        self.failure_rate = rate.clamp(0.0, 1.0);
        self
    }
    
    pub async fn add_response(&self, response: HttpResponse) {
        self.responses.lock().await.push(response);
    }
    
    pub async fn add_error(&self, error: HttpError) {
        self.errors.lock().await.push(error);
    }
    
    pub async fn simulate_network_partition(&self, duration: Duration) {
        let errors = self.errors.clone();
        tokio::spawn(async move {
            tokio::time::sleep(duration).await;
            errors.lock().await.clear();
        });
        
        self.add_error(HttpError::ConnectionFailed("Network partition".to_string())).await;
    }
}

#[async_trait]
impl HttpClientTrait for MockHttpClientWithConditions {
    async fn send_request(&self, request: HttpRequest) -> Result<HttpResponse, HttpError> {
        // Simulate latency
        tokio::time::sleep(self.latency).await;
        
        // Simulate random failures
        if fastrand::f64() < self.failure_rate {
            return Err(HttpError::ConnectionFailed("Random failure".to_string()));
        }
        
        // Check for queued errors first
        {
            let mut errors = self.errors.lock().await;
            if !errors.is_empty() {
                return Err(errors.remove(0));
            }
        }
        
        // Return queued responses
        {
            let mut responses = self.responses.lock().await;
            if !responses.is_empty() {
                let mut response = responses.remove(0);
                response.duration = self.latency;
                return Ok(response);
            }
        }
        
        // Default response based on request
        let status_code = match request.method.as_str() {
            "GET" => 200,
            "POST" => 201,
            "PUT" => 200,
            "DELETE" => 204,
            _ => 200,
        };
        
        let body = match request.method.as_str() {
            "GET" => r#"{"data": "sample_data"}"#,
            "POST" => r#"{"id": 123, "created": true}"#,
            "PUT" => r#"{"id": 123, "updated": true}"#,
            "DELETE" => "",
            _ => r#"{"status": "ok"}"#,
        };
        
        Ok(HttpResponse {
            status_code,
            headers: HashMap::from([
                ("content-type".to_string(), "application/json".to_string()),
                ("server".to_string(), "mock-server/1.0".to_string()),
            ]),
            body: body.as_bytes().to_vec(),
            duration: self.latency,
        })
    }
    
    async fn configure_tls(&mut self, _config: TlsConfig) -> Result<(), HttpError> {
        // Simulate TLS configuration
        tokio::time::sleep(Duration::from_millis(50)).await;
        Ok(())
    }
    
    fn set_timeout(&mut self, timeout: Duration) {
        // Update internal timeout (could affect latency simulation)
        if timeout < self.latency {
            self.latency = timeout;
        }
    }
    
    fn set_connection_pool_size(&mut self, _size: usize) {
        // Mock implementation - no actual pool management needed
    }
}

/// Mock HTTP client that records all requests for verification
pub struct RecordingHttpClient {
    requests: Arc<Mutex<Vec<HttpRequest>>>,
    default_response: HttpResponse,
}

impl RecordingHttpClient {
    pub fn new() -> Self {
        Self {
            requests: Arc::new(Mutex::new(Vec::new())),
            default_response: HttpResponse {
                status_code: 200,
                headers: HashMap::new(),
                body: b"{}".to_vec(),
                duration: Duration::from_millis(10),
            },
        }
    }
    
    pub fn with_default_response(mut self, response: HttpResponse) -> Self {
        self.default_response = response;
        self
    }
    
    pub async fn get_recorded_requests(&self) -> Vec<HttpRequest> {
        self.requests.lock().await.clone()
    }
    
    pub async fn clear_recorded_requests(&self) {
        self.requests.lock().await.clear();
    }
    
    pub async fn request_count(&self) -> usize {
        self.requests.lock().await.len()
    }
}

#[async_trait]
impl HttpClientTrait for RecordingHttpClient {
    async fn send_request(&self, request: HttpRequest) -> Result<HttpResponse, HttpError> {
        // Record the request
        self.requests.lock().await.push(request.clone());
        
        // Return default response
        Ok(self.default_response.clone())
    }
    
    async fn configure_tls(&mut self, _config: TlsConfig) -> Result<(), HttpError> {
        Ok(())
    }
    
    fn set_timeout(&mut self, _timeout: Duration) {
        // No-op for recording client
    }
    
    fn set_connection_pool_size(&mut self, _size: usize) {
        // No-op for recording client
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_mock_http_client_with_conditions() {
        let mut client = MockHttpClientWithConditions::new()
            .with_latency(Duration::from_millis(50))
            .with_failure_rate(0.0);
        
        let request = HttpRequest {
            method: "GET".to_string(),
            url: "https://api.example.com/test".to_string(),
            headers: HashMap::new(),
            body: None,
        };
        
        let response = client.send_request(request).await.unwrap();
        assert_eq!(response.status_code, 200);
        assert!(response.duration >= Duration::from_millis(50));
    }
    
    #[tokio::test]
    async fn test_recording_http_client() {
        let client = RecordingHttpClient::new();
        
        let request1 = HttpRequest {
            method: "GET".to_string(),
            url: "https://api.example.com/users".to_string(),
            headers: HashMap::new(),
            body: None,
        };
        
        let request2 = HttpRequest {
            method: "POST".to_string(),
            url: "https://api.example.com/users".to_string(),
            headers: HashMap::from([("content-type".to_string(), "application/json".to_string())]),
            body: Some(b"{}".to_vec()),
        };
        
        client.send_request(request1.clone()).await.unwrap();
        client.send_request(request2.clone()).await.unwrap();
        
        let recorded = client.get_recorded_requests().await;
        assert_eq!(recorded.len(), 2);
        assert_eq!(recorded[0].method, "GET");
        assert_eq!(recorded[1].method, "POST");
    }
    
    #[tokio::test]
    async fn test_network_partition_simulation() {
        let client = MockHttpClientWithConditions::new();
        
        // Simulate network partition for 100ms
        client.simulate_network_partition(Duration::from_millis(100)).await;
        
        let request = HttpRequest {
            method: "GET".to_string(),
            url: "https://api.example.com/test".to_string(),
            headers: HashMap::new(),
            body: None,
        };
        
        // Should fail due to network partition
        let result = client.send_request(request).await;
        assert!(result.is_err());
        
        // Wait for partition to end
        tokio::time::sleep(Duration::from_millis(150)).await;
        
        // Should succeed now
        let request2 = HttpRequest {
            method: "GET".to_string(),
            url: "https://api.example.com/test".to_string(),
            headers: HashMap::new(),
            body: None,
        };
        
        let result2 = client.send_request(request2).await;
        assert!(result2.is_ok());
    }
}