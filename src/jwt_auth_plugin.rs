use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::auth_plugin::{AuthPlugin, AuthConfig, AuthResult, AuthFailure, AuthToken};
use crate::error::{PluginError, ProtocolError};
use crate::plugin::{Plugin, PluginConfig, ProtocolRequest};

/// JWT authentication plugin
pub struct JwtAuthPlugin {
    // In a real implementation, you'd use a JWT library like jsonwebtoken
}

impl JwtAuthPlugin {
    pub fn new() -> Self {
        Self {}
    }
    
    fn generate_jwt(&self, config: &AuthConfig) -> Result<AuthResult, AuthFailure> {
        if let AuthConfig::Jwt { 
            secret, 
            algorithm, 
            issuer, 
            audience, 
            expiration,
            custom_claims,
        } = config {
            // Create JWT header
            let header = JwtHeader {
                alg: algorithm.clone(),
                typ: "JWT".to_string(),
            };
            
            // Create JWT payload
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs();
                
            let exp = if let Some(expiration) = expiration {
                Some(now + expiration.as_secs())
            } else {
                Some(now + 3600) // Default 1 hour
            };
            
            let mut payload = JwtPayload {
                iss: issuer.clone(),
                aud: audience.clone(),
                exp,
                iat: Some(now),
                nbf: Some(now),
                sub: None,
                jti: Some(uuid::Uuid::new_v4().to_string()),
                custom: custom_claims.clone(),
            };
            
            // In a real implementation, you would:
            // 1. Encode header and payload as base64url
            // 2. Create signature using the specified algorithm and secret
            // 3. Combine header.payload.signature
            
            // For this example, we'll create a mock JWT
            let token = self.create_mock_jwt(&header, &payload, secret, algorithm)?;
            
            let expires_at = exp.map(|exp_timestamp| {
                UNIX_EPOCH + Duration::from_secs(exp_timestamp)
            });
            
            let auth_token = AuthToken {
                token,
                token_type: "Bearer".to_string(),
                expires_at,
                refresh_token: None, // JWT typically doesn't use refresh tokens
                scope: None,
            };
            
            let mut metadata = HashMap::new();
            metadata.insert("algorithm".to_string(), algorithm.clone());
            if let Some(iss) = issuer {
                metadata.insert("issuer".to_string(), iss.clone());
            }
            if let Some(aud) = audience {
                metadata.insert("audience".to_string(), aud.clone());
            }
            
            Ok(AuthResult { 
                token: auth_token, 
                metadata 
            })
        } else {
            Err(AuthFailure {
                error_code: "INVALID_CONFIG".to_string(),
                error_description: "Expected JWT configuration".to_string(),
                retry_after: None,
                should_retry: false,
            })
        }
    }
    
    fn create_mock_jwt(&self, header: &JwtHeader, payload: &JwtPayload, secret: &str, algorithm: &str) -> Result<String, AuthFailure> {
        // In a real implementation, you would use a proper JWT library
        // This is a simplified mock implementation for demonstration
        
        let header_json = serde_json::to_string(header)
            .map_err(|e| AuthFailure {
                error_code: "HEADER_ENCODING_ERROR".to_string(),
                error_description: format!("Failed to encode JWT header: {}", e),
                retry_after: None,
                should_retry: false,
            })?;
            
        let payload_json = serde_json::to_string(payload)
            .map_err(|e| AuthFailure {
                error_code: "PAYLOAD_ENCODING_ERROR".to_string(),
                error_description: format!("Failed to encode JWT payload: {}", e),
                retry_after: None,
                should_retry: false,
            })?;
        
        // Base64url encode (simplified - in real implementation use proper base64url encoding)
        let header_b64 = base64::encode(header_json.as_bytes());
        let payload_b64 = base64::encode(payload_json.as_bytes());
        
        // Create signature (simplified - in real implementation use proper HMAC/RSA signing)
        let signing_input = format!("{}.{}", header_b64, payload_b64);
        let signature = match algorithm {
            "HS256" => {
                // In real implementation: HMAC-SHA256(signing_input, secret)
                let mock_signature = format!("mock_signature_{}_{}", 
                    signing_input.len(), 
                    secret.len()
                );
                base64::encode(mock_signature.as_bytes())
            }
            "RS256" => {
                // In real implementation: RSA-SHA256 signature
                let mock_signature = format!("mock_rsa_signature_{}", signing_input.len());
                base64::encode(mock_signature.as_bytes())
            }
            _ => {
                return Err(AuthFailure {
                    error_code: "UNSUPPORTED_ALGORITHM".to_string(),
                    error_description: format!("Algorithm '{}' not supported", algorithm),
                    retry_after: None,
                    should_retry: false,
                });
            }
        };
        
        Ok(format!("{}.{}.{}", header_b64, payload_b64, signature))
    }
    
    fn validate_jwt(&self, token: &str, config: &AuthConfig) -> Result<bool, AuthFailure> {
        if let AuthConfig::Jwt { secret, algorithm, .. } = config {
            // In a real implementation, you would:
            // 1. Split the token into header, payload, and signature
            // 2. Verify the signature using the secret and algorithm
            // 3. Check expiration and other claims
            
            // For this mock implementation, we'll do basic validation
            let parts: Vec<&str> = token.split('.').collect();
            if parts.len() != 3 {
                return Err(AuthFailure {
                    error_code: "INVALID_TOKEN_FORMAT".to_string(),
                    error_description: "JWT must have 3 parts separated by dots".to_string(),
                    retry_after: None,
                    should_retry: false,
                });
            }
            
            // Decode and validate payload
            let payload_b64 = parts[1];
            let payload_bytes = base64::decode(payload_b64)
                .map_err(|e| AuthFailure {
                    error_code: "PAYLOAD_DECODE_ERROR".to_string(),
                    error_description: format!("Failed to decode JWT payload: {}", e),
                    retry_after: None,
                    should_retry: false,
                })?;
                
            let payload: JwtPayload = serde_json::from_slice(&payload_bytes)
                .map_err(|e| AuthFailure {
                    error_code: "PAYLOAD_PARSE_ERROR".to_string(),
                    error_description: format!("Failed to parse JWT payload: {}", e),
                    retry_after: None,
                    should_retry: false,
                })?;
                
            // Check expiration
            if let Some(exp) = payload.exp {
                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs();
                    
                if now >= exp {
                    return Err(AuthFailure {
                        error_code: "TOKEN_EXPIRED".to_string(),
                        error_description: "JWT token has expired".to_string(),
                        retry_after: None,
                        should_retry: false,
                    });
                }
            }
            
            Ok(true)
        } else {
            Err(AuthFailure {
                error_code: "INVALID_CONFIG".to_string(),
                error_description: "Expected JWT configuration".to_string(),
                retry_after: None,
                should_retry: false,
            })
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct JwtHeader {
    alg: String,
    typ: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct JwtPayload {
    iss: Option<String>, // Issuer
    aud: Option<String>, // Audience
    exp: Option<u64>,    // Expiration time
    iat: Option<u64>,    // Issued at
    nbf: Option<u64>,    // Not before
    sub: Option<String>, // Subject
    jti: Option<String>, // JWT ID
    #[serde(flatten)]
    custom: HashMap<String, serde_json::Value>, // Custom claims
}

#[async_trait]
impl Plugin for JwtAuthPlugin {
    fn name(&self) -> &str {
        "jwt-auth"
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
impl AuthPlugin for JwtAuthPlugin {
    fn auth_type(&self) -> String {
        "jwt".to_string()
    }
    
    async fn authenticate(&self, config: &AuthConfig) -> Result<AuthResult, AuthFailure> {
        self.generate_jwt(config)
    }
    
    async fn refresh_token(&self, token: &AuthToken, config: &AuthConfig) -> Result<AuthResult, AuthFailure> {
        // JWT tokens are typically not refreshed, but regenerated
        // Check if current token is still valid
        match self.validate_jwt(&token.token, config) {
            Ok(true) => {
                // Token is still valid, return it as-is
                Ok(AuthResult {
                    token: token.clone(),
                    metadata: HashMap::from([
                        ("refreshed".to_string(), "false".to_string()),
                        ("reason".to_string(), "token_still_valid".to_string()),
                    ]),
                })
            }
            Ok(false) | Err(_) => {
                // Token is invalid or expired, generate a new one
                let mut result = self.generate_jwt(config)?;
                result.metadata.insert("refreshed".to_string(), "true".to_string());
                result.metadata.insert("reason".to_string(), "token_regenerated".to_string());
                Ok(result)
            }
        }
    }
    
    async fn apply_auth(&self, request: &mut ProtocolRequest, token: &AuthToken) -> Result<(), ProtocolError> {
        let auth_header = format!("{} {}", token.token_type, token.token);
        request.headers.insert("Authorization".to_string(), auth_header);
        Ok(())
    }
    
    fn validate_config(&self, config: &AuthConfig) -> Result<(), PluginError> {
        if let AuthConfig::Jwt { 
            secret, 
            algorithm, 
            .. 
        } = config {
            if secret.is_empty() {
                return Err(PluginError::InitializationFailed("secret cannot be empty".to_string()));
            }
            
            match algorithm.as_str() {
                "HS256" | "HS384" | "HS512" | "RS256" | "RS384" | "RS512" => {
                    // Supported algorithms
                }
                _ => {
                    return Err(PluginError::InitializationFailed(
                        format!("Unsupported JWT algorithm: {}", algorithm)
                    ));
                }
            }
            
            Ok(())
        } else {
            Err(PluginError::InitializationFailed("Expected JWT configuration".to_string()))
        }
    }
    
    fn supports_refresh(&self) -> bool {
        true // JWT can be regenerated
    }
}