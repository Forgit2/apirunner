# Authentication Plugins

Authentication plugins handle various authentication mechanisms and token management. They provide a unified interface for different authentication methods while supporting custom enterprise authentication systems.

## AuthPlugin Trait

```rust
#[async_trait]
pub trait AuthPlugin: Plugin {
    /// Authenticate and return authentication tokens/headers
    async fn authenticate(&self, request: &AuthRequest) -> Result<AuthResponse, AuthError>;
    
    /// Refresh an expired authentication token
    async fn refresh_token(&self, token: &AuthToken) -> Result<AuthToken, AuthError>;
    
    /// Validate if a token is still valid
    async fn validate_token(&self, token: &AuthToken) -> Result<bool, AuthError>;
    
    /// Return the list of authentication types this plugin supports
    fn supported_auth_types(&self) -> Vec<String>;
}
```

## Authentication Request

```rust
#[derive(Debug, Clone)]
pub struct AuthRequest {
    /// Authentication type (oauth2, jwt, basic, etc.)
    pub auth_type: String,
    /// Authentication configuration
    pub config: HashMap<String, serde_json::Value>,
    /// Target URL for context
    pub target_url: Option<String>,
    /// Additional metadata
    pub metadata: HashMap<String, String>,
}
```

## Authentication Response

```rust
#[derive(Debug, Clone)]
pub struct AuthResponse {
    /// Authentication token information
    pub token: AuthToken,
    /// Headers to add to requests
    pub headers: HashMap<String, String>,
    /// Additional metadata
    pub metadata: HashMap<String, String>,
}
```

## Authentication Token

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthToken {
    /// Type of token (bearer, api_key, etc.)
    pub token_type: String,
    /// The actual token value
    pub access_token: String,
    /// Refresh token (if applicable)
    pub refresh_token: Option<String>,
    /// Token expiration time
    pub expires_at: Option<SystemTime>,
    /// Token scope (if applicable)
    pub scope: Option<String>,
}

impl AuthToken {
    /// Check if the token is expired
    pub fn is_expired(&self) -> bool {
        if let Some(expires_at) = self.expires_at {
            SystemTime::now() > expires_at
        } else {
            false
        }
    }
    
    /// Get time until expiration
    pub fn time_until_expiry(&self) -> Option<Duration> {
        self.expires_at.and_then(|expires_at| {
            expires_at.duration_since(SystemTime::now()).ok()
        })
    }
}
```

## Error Types

```rust
#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    #[error("Authentication failed: {0}")]
    AuthenticationFailed(String),
    
    #[error("Token refresh failed: {0}")]
    RefreshFailed(String),
    
    #[error("Token validation failed: {0}")]
    ValidationFailed(String),
    
    #[error("Invalid configuration: {0}")]
    InvalidConfiguration(String),
    
    #[error("Token expired")]
    TokenExpired,
    
    #[error("Refresh not supported for this authentication type")]
    RefreshNotSupported,
    
    #[error("Network error: {0}")]
    NetworkError(String),
}
```

## Built-in Authentication Plugins

### OAuth 2.0 Plugin

```rust
pub struct OAuth2AuthPlugin {
    client: Arc<reqwest::Client>,
    token_cache: Arc<Mutex<HashMap<String, AuthToken>>>,
}

#[async_trait]
impl AuthPlugin for OAuth2AuthPlugin {
    async fn authenticate(&self, request: &AuthRequest) -> Result<AuthResponse, AuthError> {
        let client_id = request.config.get("client_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AuthError::InvalidConfiguration("client_id required".to_string()))?;
            
        let client_secret = request.config.get("client_secret")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AuthError::InvalidConfiguration("client_secret required".to_string()))?;
            
        let token_url = request.config.get("token_url")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AuthError::InvalidConfiguration("token_url required".to_string()))?;
            
        let scope = request.config.get("scope")
            .and_then(|v| v.as_str())
            .unwrap_or("read");
            
        let grant_type = request.config.get("grant_type")
            .and_then(|v| v.as_str())
            .unwrap_or("client_credentials");
        
        // Check cache first
        let cache_key = format!("{}:{}:{}", client_id, scope, grant_type);
        if let Ok(cache) = self.token_cache.lock() {
            if let Some(cached_token) = cache.get(&cache_key) {
                if !cached_token.is_expired() {
                    let mut headers = HashMap::new();
                    headers.insert("Authorization".to_string(), format!("Bearer {}", cached_token.access_token));
                    
                    return Ok(AuthResponse {
                        token: cached_token.clone(),
                        headers,
                        metadata: HashMap::new(),
                    });
                }
            }
        }
        
        // Request new token
        let mut form_data = HashMap::new();
        form_data.insert("grant_type", grant_type);
        form_data.insert("client_id", client_id);
        form_data.insert("client_secret", client_secret);
        form_data.insert("scope", scope);
        
        let response = self.client
            .post(token_url)
            .form(&form_data)
            .send()
            .await
            .map_err(|e| AuthError::NetworkError(e.to_string()))?;
            
        if !response.status().is_success() {
            return Err(AuthError::AuthenticationFailed(
                format!("HTTP {}: {}", response.status(), response.status().canonical_reason().unwrap_or("Unknown"))
            ));
        }
        
        let token_response: serde_json::Value = response.json().await
            .map_err(|e| AuthError::AuthenticationFailed(e.to_string()))?;
            
        let access_token = token_response.get("access_token")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AuthError::AuthenticationFailed("No access_token in response".to_string()))?;
            
        let expires_in = token_response.get("expires_in")
            .and_then(|v| v.as_u64())
            .unwrap_or(3600); // Default to 1 hour
            
        let refresh_token = token_response.get("refresh_token")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
            
        let expires_at = SystemTime::now() + Duration::from_secs(expires_in);
        
        let token = AuthToken {
            token_type: "bearer".to_string(),
            access_token: access_token.to_string(),
            refresh_token,
            expires_at: Some(expires_at),
            scope: Some(scope.to_string()),
        };
        
        // Cache the token
        if let Ok(mut cache) = self.token_cache.lock() {
            cache.insert(cache_key, token.clone());
        }
        
        let mut headers = HashMap::new();
        headers.insert("Authorization".to_string(), format!("Bearer {}", token.access_token));
        
        Ok(AuthResponse {
            token,
            headers,
            metadata: HashMap::new(),
        })
    }
    
    async fn refresh_token(&self, token: &AuthToken) -> Result<AuthToken, AuthError> {
        let refresh_token = token.refresh_token.as_ref()
            .ok_or_else(|| AuthError::RefreshNotSupported)?;
            
        // Implementation would make refresh token request
        // This is a simplified version
        todo!("Implement refresh token logic")
    }
    
    async fn validate_token(&self, token: &AuthToken) -> Result<bool, AuthError> {
        // Simple expiration check
        Ok(!token.is_expired())
    }
    
    fn supported_auth_types(&self) -> Vec<String> {
        vec!["oauth2".to_string()]
    }
}
```

### JWT Authentication Plugin

```rust
pub struct JwtAuthPlugin {
    public_keys: Arc<Mutex<HashMap<String, jsonwebtoken::DecodingKey>>>,
    client: Arc<reqwest::Client>,
}

#[async_trait]
impl AuthPlugin for JwtAuthPlugin {
    async fn authenticate(&self, request: &AuthRequest) -> Result<AuthResponse, AuthError> {
        let token = request.config.get("token")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AuthError::InvalidConfiguration("JWT token required".to_string()))?;
            
        // Validate token if public key is provided
        if let Some(public_key_url) = request.config.get("public_key_url").and_then(|v| v.as_str()) {
            self.validate_jwt_token(token, public_key_url).await?;
        }
        
        // Extract expiration from token
        let header = jsonwebtoken::decode_header(token)
            .map_err(|e| AuthError::InvalidConfiguration(format!("Invalid JWT header: {}", e)))?;
            
        let claims: serde_json::Value = jsonwebtoken::decode::<serde_json::Value>(
            token,
            &jsonwebtoken::DecodingKey::from_secret(&[]), // We'll validate separately
            &jsonwebtoken::Validation::default(),
        )
        .map_err(|_| AuthError::InvalidConfiguration("Invalid JWT token".to_string()))?
        .claims;
        
        let expires_at = claims.get("exp")
            .and_then(|v| v.as_u64())
            .map(|exp| UNIX_EPOCH + Duration::from_secs(exp));
            
        let auth_token = AuthToken {
            token_type: "bearer".to_string(),
            access_token: token.to_string(),
            refresh_token: None,
            expires_at,
            scope: claims.get("scope").and_then(|v| v.as_str()).map(|s| s.to_string()),
        };
        
        let mut headers = HashMap::new();
        headers.insert("Authorization".to_string(), format!("Bearer {}", token));
        
        Ok(AuthResponse {
            token: auth_token,
            headers,
            metadata: HashMap::new(),
        })
    }
    
    async fn refresh_token(&self, _token: &AuthToken) -> Result<AuthToken, AuthError> {
        Err(AuthError::RefreshNotSupported)
    }
    
    async fn validate_token(&self, token: &AuthToken) -> Result<bool, AuthError> {
        // Check expiration
        if token.is_expired() {
            return Ok(false);
        }
        
        // Additional JWT-specific validation could be added here
        Ok(true)
    }
    
    fn supported_auth_types(&self) -> Vec<String> {
        vec!["jwt".to_string()]
    }
}

impl JwtAuthPlugin {
    async fn validate_jwt_token(&self, token: &str, public_key_url: &str) -> Result<(), AuthError> {
        // Fetch public key if not cached
        let public_key = self.get_public_key(public_key_url).await?;
        
        // Validate token signature
        let validation = jsonwebtoken::Validation::default();
        jsonwebtoken::decode::<serde_json::Value>(token, &public_key, &validation)
            .map_err(|e| AuthError::ValidationFailed(e.to_string()))?;
            
        Ok(())
    }
    
    async fn get_public_key(&self, url: &str) -> Result<jsonwebtoken::DecodingKey, AuthError> {
        // Check cache first
        if let Ok(keys) = self.public_keys.lock() {
            if let Some(key) = keys.get(url) {
                return Ok(key.clone());
            }
        }
        
        // Fetch public key
        let response = self.client.get(url).send().await
            .map_err(|e| AuthError::NetworkError(e.to_string()))?;
            
        let jwks: serde_json::Value = response.json().await
            .map_err(|e| AuthError::NetworkError(e.to_string()))?;
            
        // Extract first key (simplified)
        let key_data = jwks.get("keys")
            .and_then(|keys| keys.as_array())
            .and_then(|keys| keys.first())
            .ok_or_else(|| AuthError::ValidationFailed("No keys found in JWKS".to_string()))?;
            
        let public_key = jsonwebtoken::DecodingKey::from_rsa_components(
            key_data.get("n").and_then(|v| v.as_str()).unwrap_or(""),
            key_data.get("e").and_then(|v| v.as_str()).unwrap_or(""),
        ).map_err(|e| AuthError::ValidationFailed(e.to_string()))?;
        
        // Cache the key
        if let Ok(mut keys) = self.public_keys.lock() {
            keys.insert(url.to_string(), public_key.clone());
        }
        
        Ok(public_key)
    }
}
```

### Basic Authentication Plugin

```rust
pub struct BasicAuthPlugin;

#[async_trait]
impl AuthPlugin for BasicAuthPlugin {
    async fn authenticate(&self, request: &AuthRequest) -> Result<AuthResponse, AuthError> {
        let username = request.config.get("username")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AuthError::InvalidConfiguration("username required".to_string()))?;
            
        let password = request.config.get("password")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AuthError::InvalidConfiguration("password required".to_string()))?;
            
        let credentials = format!("{}:{}", username, password);
        let encoded = base64::encode(credentials);
        
        let token = AuthToken {
            token_type: "basic".to_string(),
            access_token: encoded.clone(),
            refresh_token: None,
            expires_at: None, // Basic auth doesn't expire
            scope: None,
        };
        
        let mut headers = HashMap::new();
        headers.insert("Authorization".to_string(), format!("Basic {}", encoded));
        
        Ok(AuthResponse {
            token,
            headers,
            metadata: HashMap::new(),
        })
    }
    
    async fn refresh_token(&self, _token: &AuthToken) -> Result<AuthToken, AuthError> {
        Err(AuthError::RefreshNotSupported)
    }
    
    async fn validate_token(&self, _token: &AuthToken) -> Result<bool, AuthError> {
        // Basic auth is always valid (no expiration)
        Ok(true)
    }
    
    fn supported_auth_types(&self) -> Vec<String> {
        vec!["basic".to_string()]
    }
}
```

## Authentication Chain Executor

```rust
pub struct AuthChainExecutor {
    auth_plugins: HashMap<String, Arc<dyn AuthPlugin>>,
}

impl AuthChainExecutor {
    pub async fn execute_chain(&self, chain: &[AuthRequest]) -> Result<AuthResponse, AuthError> {
        let mut final_headers = HashMap::new();
        let mut final_token = None;
        let mut final_metadata = HashMap::new();
        
        for (index, auth_request) in chain.iter().enumerate() {
            let plugin = self.auth_plugins.get(&auth_request.auth_type)
                .ok_or_else(|| AuthError::InvalidConfiguration(
                    format!("Auth plugin '{}' not found", auth_request.auth_type)
                ))?;
                
            let response = plugin.authenticate(auth_request).await?;
            
            // Merge headers
            for (key, value) in response.headers {
                final_headers.insert(key, value);
            }
            
            // Use the last token as the final token
            final_token = Some(response.token);
            
            // Merge metadata
            for (key, value) in response.metadata {
                final_metadata.insert(format!("step_{}_{}", index, key), value);
            }
        }
        
        Ok(AuthResponse {
            token: final_token.unwrap(),
            headers: final_headers,
            metadata: final_metadata,
        })
    }
}
```

## Best Practices

### Token Management
- **Cache tokens** to avoid unnecessary authentication requests
- **Implement automatic refresh** for expiring tokens
- **Handle token expiration** gracefully with retry logic
- **Secure token storage** in memory and configuration

### Error Handling
- **Provide detailed error messages** for authentication failures
- **Distinguish between** authentication and authorization errors
- **Implement retry logic** for transient network failures
- **Log authentication attempts** for debugging (without sensitive data)

### Security
- **Never log sensitive data** (passwords, tokens, secrets)
- **Use secure communication** (HTTPS) for token requests
- **Validate tokens** before use when possible
- **Implement proper token lifecycle** management
- **Support token revocation** when available

### Performance
- **Cache authentication results** to reduce latency
- **Use connection pooling** for token requests
- **Implement concurrent token refresh** for multiple requests
- **Monitor authentication performance** and success rates