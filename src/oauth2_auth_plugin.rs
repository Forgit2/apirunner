use async_trait::async_trait;
use serde::Deserialize;
use std::collections::HashMap;
use std::time::{Duration, SystemTime};

use crate::auth_plugin::{AuthPlugin, AuthConfig, AuthResult, AuthFailure, AuthToken};
use crate::error::{PluginError, ProtocolError};
use crate::plugin::{Plugin, PluginConfig, ProtocolRequest};

/// OAuth2 authentication plugin
pub struct OAuth2AuthPlugin {
    client: reqwest::Client,
}

impl OAuth2AuthPlugin {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }
    
    async fn request_token(&self, config: &AuthConfig) -> Result<AuthResult, AuthFailure> {
        if let AuthConfig::OAuth2 { 
            client_id, 
            client_secret, 
            token_url, 
            scope, 
            grant_type,
            username,
            password,
        } = config {
            let mut form_data = HashMap::new();
            form_data.insert("client_id", client_id.as_str());
            form_data.insert("client_secret", client_secret.as_str());
            form_data.insert("grant_type", grant_type.as_str());
            
            if let Some(scope) = scope {
                form_data.insert("scope", scope.as_str());
            }
            
            // Handle different grant types
            match grant_type.as_str() {
                "password" => {
                    if let (Some(username), Some(password)) = (username, password) {
                        form_data.insert("username", username.as_str());
                        form_data.insert("password", password.as_str());
                    } else {
                        return Err(AuthFailure {
                            error_code: "MISSING_CREDENTIALS".to_string(),
                            error_description: "Username and password required for password grant".to_string(),
                            retry_after: None,
                            should_retry: false,
                        });
                    }
                }
                "client_credentials" => {
                    // Client credentials are already included
                }
                _ => {
                    return Err(AuthFailure {
                        error_code: "UNSUPPORTED_GRANT_TYPE".to_string(),
                        error_description: format!("Grant type '{}' not supported", grant_type),
                        retry_after: None,
                        should_retry: false,
                    });
                }
            }
            
            let response = self.client
                .post(token_url)
                .form(&form_data)
                .send()
                .await
                .map_err(|e| AuthFailure {
                    error_code: "REQUEST_FAILED".to_string(),
                    error_description: format!("Failed to request token: {}", e),
                    retry_after: Some(Duration::from_secs(5)),
                    should_retry: true,
                })?;
                
            if !response.status().is_success() {
                let status = response.status();
                let error_text = response.text().await.unwrap_or_default();
                
                return Err(AuthFailure {
                    error_code: format!("HTTP_{}", status.as_u16()),
                    error_description: format!("Token request failed with status {}: {}", status, error_text),
                    retry_after: if status.as_u16() == 429 { Some(Duration::from_secs(60)) } else { None },
                    should_retry: status.is_server_error() || status.as_u16() == 429,
                });
            }
            
            let token_response: OAuth2TokenResponse = response.json().await
                .map_err(|e| AuthFailure {
                    error_code: "PARSE_ERROR".to_string(),
                    error_description: format!("Failed to parse token response: {}", e),
                    retry_after: None,
                    should_retry: false,
                })?;
                
            let expires_at = if let Some(expires_in) = token_response.expires_in {
                Some(SystemTime::now() + Duration::from_secs(expires_in))
            } else {
                None
            };
            
            let token = AuthToken {
                token: token_response.access_token,
                token_type: token_response.token_type.unwrap_or_else(|| "Bearer".to_string()),
                expires_at,
                refresh_token: token_response.refresh_token,
                scope: token_response.scope,
            };
            
            let mut metadata = HashMap::new();
            if let Some(expires_in) = token_response.expires_in {
                metadata.insert("expires_in".to_string(), expires_in.to_string());
            }
            
            Ok(AuthResult { token, metadata })
        } else {
            Err(AuthFailure {
                error_code: "INVALID_CONFIG".to_string(),
                error_description: "Expected OAuth2 configuration".to_string(),
                retry_after: None,
                should_retry: false,
            })
        }
    }
    
    async fn refresh_access_token(&self, token: &AuthToken, config: &AuthConfig) -> Result<AuthResult, AuthFailure> {
        if let AuthConfig::OAuth2 { 
            client_id, 
            client_secret, 
            token_url, 
            .. 
        } = config {
            let refresh_token = token.refresh_token.as_ref()
                .ok_or_else(|| AuthFailure {
                    error_code: "NO_REFRESH_TOKEN".to_string(),
                    error_description: "No refresh token available".to_string(),
                    retry_after: None,
                    should_retry: false,
                })?;
                
            let mut form_data = HashMap::new();
            form_data.insert("client_id", client_id.as_str());
            form_data.insert("client_secret", client_secret.as_str());
            form_data.insert("grant_type", "refresh_token");
            form_data.insert("refresh_token", refresh_token.as_str());
            
            let response = self.client
                .post(token_url)
                .form(&form_data)
                .send()
                .await
                .map_err(|e| AuthFailure {
                    error_code: "REFRESH_FAILED".to_string(),
                    error_description: format!("Failed to refresh token: {}", e),
                    retry_after: Some(Duration::from_secs(5)),
                    should_retry: true,
                })?;
                
            if !response.status().is_success() {
                let status = response.status();
                let error_text = response.text().await.unwrap_or_default();
                
                return Err(AuthFailure {
                    error_code: format!("REFRESH_HTTP_{}", status.as_u16()),
                    error_description: format!("Token refresh failed with status {}: {}", status, error_text),
                    retry_after: None,
                    should_retry: false, // Refresh failures usually require re-authentication
                });
            }
            
            let token_response: OAuth2TokenResponse = response.json().await
                .map_err(|e| AuthFailure {
                    error_code: "REFRESH_PARSE_ERROR".to_string(),
                    error_description: format!("Failed to parse refresh response: {}", e),
                    retry_after: None,
                    should_retry: false,
                })?;
                
            let expires_at = if let Some(expires_in) = token_response.expires_in {
                Some(SystemTime::now() + Duration::from_secs(expires_in))
            } else {
                None
            };
            
            let new_token = AuthToken {
                token: token_response.access_token,
                token_type: token_response.token_type.unwrap_or_else(|| "Bearer".to_string()),
                expires_at,
                refresh_token: token_response.refresh_token.or_else(|| token.refresh_token.clone()),
                scope: token_response.scope.or_else(|| token.scope.clone()),
            };
            
            let mut metadata = HashMap::new();
            if let Some(expires_in) = token_response.expires_in {
                metadata.insert("expires_in".to_string(), expires_in.to_string());
            }
            metadata.insert("refreshed".to_string(), "true".to_string());
            
            Ok(AuthResult { token: new_token, metadata })
        } else {
            Err(AuthFailure {
                error_code: "INVALID_CONFIG".to_string(),
                error_description: "Expected OAuth2 configuration".to_string(),
                retry_after: None,
                should_retry: false,
            })
        }
    }
}

#[derive(Debug, Deserialize)]
struct OAuth2TokenResponse {
    access_token: String,
    token_type: Option<String>,
    expires_in: Option<u64>,
    refresh_token: Option<String>,
    scope: Option<String>,
}

#[async_trait]
impl Plugin for OAuth2AuthPlugin {
    fn name(&self) -> &str {
        "oauth2-auth"
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
impl AuthPlugin for OAuth2AuthPlugin {
    fn auth_type(&self) -> String {
        "oauth2".to_string()
    }
    
    async fn authenticate(&self, config: &AuthConfig) -> Result<AuthResult, AuthFailure> {
        self.request_token(config).await
    }
    
    async fn refresh_token(&self, token: &AuthToken, config: &AuthConfig) -> Result<AuthResult, AuthFailure> {
        self.refresh_access_token(token, config).await
    }
    
    async fn apply_auth(&self, request: &mut ProtocolRequest, token: &AuthToken) -> Result<(), ProtocolError> {
        let auth_header = format!("{} {}", token.token_type, token.token);
        request.headers.insert("Authorization".to_string(), auth_header);
        Ok(())
    }
    
    fn validate_config(&self, config: &AuthConfig) -> Result<(), PluginError> {
        if let AuthConfig::OAuth2 { 
            client_id, 
            client_secret, 
            token_url, 
            grant_type,
            username,
            password,
            .. 
        } = config {
            if client_id.is_empty() {
                return Err(PluginError::InitializationFailed("client_id cannot be empty".to_string()));
            }
            if client_secret.is_empty() {
                return Err(PluginError::InitializationFailed("client_secret cannot be empty".to_string()));
            }
            if token_url.is_empty() {
                return Err(PluginError::InitializationFailed("token_url cannot be empty".to_string()));
            }
            
            match grant_type.as_str() {
                "client_credentials" => {
                    // No additional validation needed
                }
                "password" => {
                    if username.is_none() || password.is_none() {
                        return Err(PluginError::InitializationFailed(
                            "username and password required for password grant".to_string()
                        ));
                    }
                }
                _ => {
                    return Err(PluginError::InitializationFailed(
                        format!("Unsupported grant type: {}", grant_type)
                    ));
                }
            }
            
            Ok(())
        } else {
            Err(PluginError::InitializationFailed("Expected OAuth2 configuration".to_string()))
        }
    }
    
    fn supports_refresh(&self) -> bool {
        true
    }
}