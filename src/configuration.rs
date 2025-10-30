//! Configuration Management System
//! 
//! Provides hierarchical configuration loading with environment-specific overrides,
//! secure storage for sensitive data, and dynamic reloading capabilities.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use std::time::SystemTime;
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

use anyhow::{Result, Context};
use thiserror::Error;
use aes_gcm::{Aes256Gcm, Key, Nonce, aead::{Aead, KeyInit, OsRng}};
use argon2::{Argon2, PasswordHasher, password_hash::{rand_core::RngCore, SaltString}};
use zeroize::ZeroizeOnDrop;
use keyring::Entry;
use base64::{Engine as _, engine::general_purpose};

/// Configuration management errors
#[derive(Error, Debug)]
pub enum ConfigurationError {
    #[error("Configuration file not found: {path}")]
    FileNotFound { path: String },
    
    #[error("Invalid configuration format: {message}")]
    InvalidFormat { message: String },
    
    #[error("Configuration validation failed: {errors:?}")]
    ValidationFailed { errors: Vec<String> },
    
    #[error("Environment variable not found: {name}")]
    EnvironmentVariableNotFound { name: String },
    
    #[error("Encryption/decryption error: {message}")]
    EncryptionError { message: String },
    
    #[error("Configuration reload failed: {message}")]
    ReloadFailed { message: String },
    
    #[error("Secret not found: {name}")]
    SecretNotFound { name: String },
    
    #[error("Keyring error: {message}")]
    KeyringError { message: String },
    
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Configuration change event types
#[derive(Debug, Clone)]
pub enum ConfigurationEvent {
    /// Configuration was successfully reloaded
    Reloaded {
        path: PathBuf,
        timestamp: SystemTime,
        changes: Vec<ConfigurationChange>,
    },
    /// Configuration reload failed
    ReloadFailed {
        path: PathBuf,
        error: String,
        timestamp: SystemTime,
    },
    /// Configuration file was modified
    FileModified {
        path: PathBuf,
        timestamp: SystemTime,
    },
    /// Configuration was rolled back due to validation failure
    RolledBack {
        path: PathBuf,
        error: String,
        timestamp: SystemTime,
    },
    /// Configuration validation started
    ValidationStarted {
        path: PathBuf,
        timestamp: SystemTime,
    },
    /// Configuration validation completed successfully
    ValidationCompleted {
        path: PathBuf,
        timestamp: SystemTime,
    },
}

/// Represents a specific configuration change
#[derive(Debug, Clone)]
pub struct ConfigurationChange {
    /// Path to the changed configuration value (dot-separated)
    pub path: String,
    /// Previous value (if any)
    pub old_value: Option<serde_json::Value>,
    /// New value
    pub new_value: serde_json::Value,
    /// Type of change
    pub change_type: ChangeType,
}

/// Type of configuration change
#[derive(Debug, Clone, PartialEq)]
pub enum ChangeType {
    /// Value was added
    Added,
    /// Value was modified
    Modified,
    /// Value was removed
    Removed,
}

/// Secure configuration value that can be encrypted
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SecureValue {
    /// Plain text value
    Plain(String),
    /// Encrypted value with metadata
    Encrypted {
        /// Base64 encoded encrypted data
        encrypted_data: String,
        /// Base64 encoded nonce/IV
        nonce: String,
        /// Encryption algorithm used
        algorithm: String,
    },
    /// Reference to environment variable
    EnvVar {
        /// Environment variable name
        env_var: String,
        /// Optional default value if env var is not set
        default: Option<String>,
    },
    /// Reference to system keyring/secret store
    Keyring {
        /// Service name in keyring
        service: String,
        /// Username/key name in keyring
        username: String,
    },
}

/// Types of secure storage available
#[derive(Debug, Clone, Copy)]
pub enum SecureStorageType {
    /// Store encrypted in configuration files
    Encrypted,
    /// Store in system keyring/credential store
    Keyring,
    /// Reference environment variable
    EnvVar,
}

/// Secure configuration manager for handling sensitive data
#[derive(Clone, ZeroizeOnDrop)]
pub struct SecureConfigManager {
    /// Master key for encryption/decryption
    #[zeroize(skip)]
    master_key: Option<Key<Aes256Gcm>>,
    /// Keyring service name
    keyring_service: String,
}

/// Configuration with secure value support
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecureConfiguration {
    /// Base configuration
    #[serde(flatten)]
    pub base: Configuration,
    /// Secure values that may be encrypted
    #[serde(default)]
    pub secure_values: HashMap<String, SecureValue>,
}

/// Core configuration structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Configuration {
    /// Test execution settings
    pub execution: ExecutionConfig,
    /// Plugin configuration
    pub plugins: ConfigPluginConfig,
    /// Reporting configuration
    pub reporting: ReportingConfig,
    /// Authentication settings
    pub auth: ConfigAuthConfig,
    /// Data source configuration
    pub data_sources: ConfigDataSourceConfig,
    /// Environment-specific overrides
    pub environments: HashMap<String, EnvironmentConfig>,
    /// Custom configuration values
    #[serde(flatten)]
    pub custom: HashMap<String, serde_json::Value>,
}

/// Execution configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionConfig {
    /// Default execution strategy (serial, parallel, distributed)
    pub default_strategy: String,
    /// Maximum concurrent executions for parallel strategy
    pub max_concurrency: usize,
    /// Request timeout in seconds
    pub request_timeout: u64,
    /// Retry configuration
    pub retry: RetryConfig,
    /// Rate limiting configuration
    pub rate_limit: RateLimitConfig,
}

/// Retry configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryConfig {
    /// Maximum number of retries
    pub max_attempts: u32,
    /// Base delay between retries in milliseconds
    pub base_delay_ms: u64,
    /// Maximum delay between retries in milliseconds
    pub max_delay_ms: u64,
    /// Backoff multiplier
    pub backoff_multiplier: f64,
}

/// Rate limiting configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitConfig {
    /// Requests per second limit
    pub requests_per_second: f64,
    /// Burst capacity
    pub burst_capacity: u32,
}

/// Plugin configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigPluginConfig {
    /// Plugin directory path
    pub plugin_dir: PathBuf,
    /// Auto-load plugins on startup
    pub auto_load: bool,
    /// Hot reload enabled
    pub hot_reload: bool,
    /// Plugin-specific configurations
    pub plugins: HashMap<String, serde_json::Value>,
}

/// Reporting configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportingConfig {
    /// Output directory for reports
    pub output_dir: PathBuf,
    /// Enabled report formats
    pub formats: Vec<String>,
    /// Report template configurations
    pub templates: HashMap<String, serde_json::Value>,
}

/// Authentication configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigAuthConfig {
    /// Default authentication method
    pub default_method: Option<String>,
    /// Authentication configurations by name
    pub methods: HashMap<String, serde_json::Value>,
}

/// Data source configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigDataSourceConfig {
    /// Default data source type
    pub default_type: String,
    /// Data source configurations by name
    pub sources: HashMap<String, serde_json::Value>,
}

/// Environment-specific configuration overrides
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvironmentConfig {
    /// Environment name
    pub name: String,
    /// Base URL overrides
    pub base_urls: HashMap<String, String>,
    /// Authentication overrides
    pub auth_overrides: HashMap<String, serde_json::Value>,
    /// Custom environment variables
    pub variables: HashMap<String, String>,
}

/// Configuration manager with hierarchical loading and hot reload support
pub struct ConfigurationManager {
    /// Current configuration
    config: Arc<RwLock<Configuration>>,
    /// Configuration file paths in order of precedence (highest to lowest)
    config_paths: Vec<PathBuf>,
    /// File watcher for hot reload
    _watcher: Option<notify::RecommendedWatcher>,
    /// Event broadcaster for configuration changes
    event_sender: broadcast::Sender<ConfigurationEvent>,
    /// Last modification times for config files
    file_timestamps: HashMap<PathBuf, SystemTime>,
    /// Secure configuration manager
    secure_manager: SecureConfigManager,
    /// Configuration backup for rollback
    config_backup: Arc<RwLock<Option<Configuration>>>,
    /// Maximum number of configuration backups to keep
    max_backups: usize,
    /// Configuration history for rollback
    config_history: Arc<RwLock<Vec<ConfigurationSnapshot>>>,
}

/// Configuration snapshot for rollback functionality
#[derive(Debug, Clone)]
pub struct ConfigurationSnapshot {
    /// The configuration at this point in time
    pub config: Configuration,
    /// Timestamp when this snapshot was created
    pub timestamp: SystemTime,
    /// Path that triggered this snapshot
    pub trigger_path: PathBuf,
    /// Reason for creating this snapshot
    pub reason: String,
}

impl SecureConfigManager {
    /// Create a new secure configuration manager
    pub fn new(keyring_service: &str) -> Self {
        Self {
            master_key: None,
            keyring_service: keyring_service.to_string(),
        }
    }

    /// Initialize with a master password for encryption
    pub fn initialize_with_password(&mut self, password: &str) -> Result<()> {
        let salt = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::default();
        
        // Derive key from password using Argon2
        let password_hash = argon2.hash_password(password.as_bytes(), &salt)
            .map_err(|e| ConfigurationError::EncryptionError { 
                message: format!("Failed to hash password: {}", e) 
            })?;
        
        // Use the hash as our encryption key (first 32 bytes)
        let hash_output = password_hash.hash.unwrap();
        let hash_bytes = hash_output.as_bytes();
        let key_bytes = &hash_bytes[..32];
        let key = Key::<Aes256Gcm>::from_slice(key_bytes);
        
        self.master_key = Some(*key);
        Ok(())
    }

    /// Encrypt a sensitive value
    pub fn encrypt_value(&self, plaintext: &str) -> Result<SecureValue> {
        let key = self.master_key.as_ref()
            .ok_or_else(|| ConfigurationError::EncryptionError { 
                message: "Master key not initialized".to_string() 
            })?;

        let cipher = Aes256Gcm::new(key);
        let mut nonce_bytes = [0u8; 12];
        OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        let ciphertext = cipher.encrypt(nonce, plaintext.as_bytes())
            .map_err(|e| ConfigurationError::EncryptionError { 
                message: format!("Encryption failed: {}", e) 
            })?;

        Ok(SecureValue::Encrypted {
            encrypted_data: general_purpose::STANDARD.encode(&ciphertext),
            nonce: general_purpose::STANDARD.encode(&nonce_bytes),
            algorithm: "AES-256-GCM".to_string(),
        })
    }

    /// Decrypt a secure value
    pub fn decrypt_value(&self, secure_value: &SecureValue) -> Result<String> {
        match secure_value {
            SecureValue::Plain(value) => Ok(value.clone()),
            
            SecureValue::Encrypted { encrypted_data, nonce, algorithm } => {
                if algorithm != "AES-256-GCM" {
                    return Err(ConfigurationError::EncryptionError { 
                        message: format!("Unsupported encryption algorithm: {}", algorithm) 
                    }.into());
                }

                let key = self.master_key.as_ref()
                    .ok_or_else(|| ConfigurationError::EncryptionError { 
                        message: "Master key not initialized".to_string() 
                    })?;

                let cipher = Aes256Gcm::new(key);
                let ciphertext = general_purpose::STANDARD.decode(encrypted_data)
                    .map_err(|e| ConfigurationError::EncryptionError { 
                        message: format!("Failed to decode encrypted data: {}", e) 
                    })?;
                
                let nonce_bytes = general_purpose::STANDARD.decode(nonce)
                    .map_err(|e| ConfigurationError::EncryptionError { 
                        message: format!("Failed to decode nonce: {}", e) 
                    })?;
                
                let nonce = Nonce::from_slice(&nonce_bytes);
                let plaintext = cipher.decrypt(nonce, ciphertext.as_ref())
                    .map_err(|e| ConfigurationError::EncryptionError { 
                        message: format!("Decryption failed: {}", e) 
                    })?;

                String::from_utf8(plaintext)
                    .map_err(|e| ConfigurationError::EncryptionError { 
                        message: format!("Invalid UTF-8 in decrypted data: {}", e) 
                    }.into())
            },
            
            SecureValue::EnvVar { env_var, default } => {
                std::env::var(env_var)
                    .or_else(|_| {
                        default.clone().ok_or_else(|| {
                            ConfigurationError::EnvironmentVariableNotFound { 
                                name: env_var.clone() 
                            }
                        })
                    })
                    .map_err(|e| e.into())
            },
            
            SecureValue::Keyring { service, username } => {
                let entry = Entry::new(service, username)
                    .map_err(|e| ConfigurationError::KeyringError { 
                        message: format!("Failed to create keyring entry: {}", e) 
                    })?;
                
                entry.get_password()
                    .map_err(|e| ConfigurationError::SecretNotFound { 
                        name: format!("{}:{}", service, username) 
                    }.into())
            },
        }
    }

    /// Store a secret in the system keyring
    pub fn store_secret(&self, key: &str, value: &str) -> Result<()> {
        let entry = Entry::new(&self.keyring_service, key)
            .map_err(|e| ConfigurationError::KeyringError { 
                message: format!("Failed to create keyring entry: {}", e) 
            })?;
        
        entry.set_password(value)
            .map_err(|e| ConfigurationError::KeyringError { 
                message: format!("Failed to store secret: {}", e) 
            }.into())
    }

    /// Retrieve a secret from the system keyring
    pub fn get_secret(&self, key: &str) -> Result<String> {
        let entry = Entry::new(&self.keyring_service, key)
            .map_err(|e| ConfigurationError::KeyringError { 
                message: format!("Failed to create keyring entry: {}", e) 
            })?;
        
        entry.get_password()
            .map_err(|e| ConfigurationError::SecretNotFound { 
                name: key.to_string() 
            }.into())
    }

    /// Delete a secret from the system keyring
    pub fn delete_secret(&self, key: &str) -> Result<()> {
        let entry = Entry::new(&self.keyring_service, key)
            .map_err(|e| ConfigurationError::KeyringError { 
                message: format!("Failed to create keyring entry: {}", e) 
            })?;
        
        entry.delete_password()
            .map_err(|e| ConfigurationError::KeyringError { 
                message: format!("Failed to delete secret: {}", e) 
            }.into())
    }

    /// Resolve all secure values in a configuration
    pub fn resolve_secure_values(&self, secure_config: &SecureConfiguration) -> Result<Configuration> {
        let mut config = secure_config.base.clone();
        
        // Resolve secure values and apply them to the configuration
        for (path, secure_value) in &secure_config.secure_values {
            let resolved_value = self.decrypt_value(secure_value)?;
            self.apply_secure_value_to_config(&mut config, path, &resolved_value)?;
        }
        
        Ok(config)
    }

    /// Apply a resolved secure value to the configuration at the specified path
    fn apply_secure_value_to_config(&self, config: &mut Configuration, path: &str, value: &str) -> Result<()> {
        let parts: Vec<&str> = path.split('.').collect();
        
        match parts.as_slice() {
            ["auth", "methods", method_name, "client_secret"] => {
                if let Some(method_config) = config.auth.methods.get_mut(*method_name) {
                    if let Some(obj) = method_config.as_object_mut() {
                        obj.insert("client_secret".to_string(), serde_json::Value::String(value.to_string()));
                    }
                }
            },
            ["auth", "methods", method_name, "api_key"] => {
                if let Some(method_config) = config.auth.methods.get_mut(*method_name) {
                    if let Some(obj) = method_config.as_object_mut() {
                        obj.insert("api_key".to_string(), serde_json::Value::String(value.to_string()));
                    }
                }
            },
            ["data_sources", "sources", source_name, "password"] => {
                if let Some(source_config) = config.data_sources.sources.get_mut(*source_name) {
                    if let Some(obj) = source_config.as_object_mut() {
                        obj.insert("password".to_string(), serde_json::Value::String(value.to_string()));
                    }
                }
            },
            ["data_sources", "sources", source_name, "connection_string"] => {
                if let Some(source_config) = config.data_sources.sources.get_mut(*source_name) {
                    if let Some(obj) = source_config.as_object_mut() {
                        obj.insert("connection_string".to_string(), serde_json::Value::String(value.to_string()));
                    }
                }
            },
            _ => {
                // For custom paths, store in the custom section
                config.custom.insert(path.to_string(), serde_json::Value::String(value.to_string()));
            }
        }
        
        Ok(())
    }
}

impl ConfigurationManager {
    /// Create a new configuration manager
    pub fn new() -> Result<Self> {
        let (event_sender, _) = broadcast::channel(100);
        
        Ok(Self {
            config: Arc::new(RwLock::new(Self::default_configuration())),
            config_paths: Vec::new(),
            _watcher: None,
            event_sender,
            file_timestamps: HashMap::new(),
            secure_manager: SecureConfigManager::new("api-test-runner"),
            config_backup: Arc::new(RwLock::new(None)),
            max_backups: 10,
            config_history: Arc::new(RwLock::new(Vec::new())),
        })
    }

    /// Create a configuration snapshot for rollback
    fn create_snapshot(&self, trigger_path: PathBuf, reason: String) -> Result<()> {
        let current_config = self.get_configuration();
        let snapshot = ConfigurationSnapshot {
            config: current_config,
            timestamp: SystemTime::now(),
            trigger_path,
            reason,
        };

        let mut history = self.config_history.write().unwrap();
        history.push(snapshot);

        // Keep only the most recent snapshots
        if history.len() > self.max_backups {
            history.remove(0);
        }

        Ok(())
    }

    /// Rollback to the most recent valid configuration
    pub fn rollback_to_last_valid(&mut self) -> Result<()> {
        let history = self.config_history.read().unwrap();
        
        if let Some(last_snapshot) = history.last() {
            let backup_config = last_snapshot.config.clone();
            
            // Validate the backup configuration before rolling back
            self.validate_configuration(&backup_config)?;
            
            // Update the current configuration
            {
                let mut config = self.config.write().unwrap();
                *config = backup_config;
            }

            // Notify about rollback
            let _ = self.event_sender.send(ConfigurationEvent::RolledBack {
                path: last_snapshot.trigger_path.clone(),
                error: "Rolled back to last valid configuration".to_string(),
                timestamp: SystemTime::now(),
            });

            Ok(())
        } else {
            Err(ConfigurationError::ReloadFailed {
                message: "No valid configuration snapshots available for rollback".to_string(),
            }.into())
        }
    }

    /// Get configuration history
    pub fn get_configuration_history(&self) -> Vec<ConfigurationSnapshot> {
        self.config_history.read().unwrap().clone()
    }

    /// Compare two configurations and return the differences
    fn compare_configurations(&self, old_config: &Configuration, new_config: &Configuration) -> Vec<ConfigurationChange> {
        let mut changes = Vec::new();
        
        let old_json = serde_json::to_value(old_config).unwrap_or_default();
        let new_json = serde_json::to_value(new_config).unwrap_or_default();
        
        self.compare_json_values("", &old_json, &new_json, &mut changes);
        
        changes
    }

    /// Recursively compare JSON values to find differences
    fn compare_json_values(&self, path: &str, old: &serde_json::Value, new: &serde_json::Value, changes: &mut Vec<ConfigurationChange>) {
        match (old, new) {
            (serde_json::Value::Object(old_obj), serde_json::Value::Object(new_obj)) => {
                // Check for modified and removed values
                for (key, old_value) in old_obj {
                    let current_path = if path.is_empty() { key.clone() } else { format!("{}.{}", path, key) };
                    
                    if let Some(new_value) = new_obj.get(key) {
                        if old_value != new_value {
                            if old_value.is_object() && new_value.is_object() {
                                self.compare_json_values(&current_path, old_value, new_value, changes);
                            } else {
                                changes.push(ConfigurationChange {
                                    path: current_path,
                                    old_value: Some(old_value.clone()),
                                    new_value: new_value.clone(),
                                    change_type: ChangeType::Modified,
                                });
                            }
                        }
                    } else {
                        changes.push(ConfigurationChange {
                            path: current_path,
                            old_value: Some(old_value.clone()),
                            new_value: serde_json::Value::Null,
                            change_type: ChangeType::Removed,
                        });
                    }
                }
                
                // Check for added values
                for (key, new_value) in new_obj {
                    if !old_obj.contains_key(key) {
                        let current_path = if path.is_empty() { key.clone() } else { format!("{}.{}", path, key) };
                        changes.push(ConfigurationChange {
                            path: current_path,
                            old_value: None,
                            new_value: new_value.clone(),
                            change_type: ChangeType::Added,
                        });
                    }
                }
            }
            _ => {
                if old != new {
                    changes.push(ConfigurationChange {
                        path: path.to_string(),
                        old_value: Some(old.clone()),
                        new_value: new.clone(),
                        change_type: ChangeType::Modified,
                    });
                }
            }
        }
    }

    /// Initialize secure configuration with a master password
    pub fn initialize_secure_config(&mut self, password: &str) -> Result<()> {
        self.secure_manager.initialize_with_password(password)
    }

    /// Store a secure configuration value
    pub fn store_secure_value(&self, key: &str, value: &str, storage_type: SecureStorageType) -> Result<()> {
        match storage_type {
            SecureStorageType::Encrypted => {
                // For encrypted storage, we would typically save this to a secure config file
                // For now, we'll store it in the keyring as a fallback
                self.secure_manager.store_secret(&format!("encrypted_{}", key), value)
            },
            SecureStorageType::Keyring => {
                self.secure_manager.store_secret(key, value)
            },
            SecureStorageType::EnvVar => {
                // Environment variables are set externally, so we just validate they exist
                std::env::var(key)
                    .map(|_| ())
                    .map_err(|_| ConfigurationError::EnvironmentVariableNotFound { 
                        name: key.to_string() 
                    }.into())
            },
        }
    }

    /// Get a secure configuration value
    pub fn get_secure_value(&self, key: &str) -> Result<String> {
        // Try different sources in order of preference
        
        // First try environment variables
        if let Ok(value) = std::env::var(key) {
            return Ok(value);
        }
        
        // Then try keyring
        if let Ok(value) = self.secure_manager.get_secret(key) {
            return Ok(value);
        }
        
        // Finally try encrypted storage (keyring with prefix)
        if let Ok(value) = self.secure_manager.get_secret(&format!("encrypted_{}", key)) {
            return Ok(value);
        }
        
        Err(ConfigurationError::SecretNotFound { 
            name: key.to_string() 
        }.into())
    }

    /// Load secure configuration from file
    pub async fn load_secure_configuration(&mut self, path: &Path) -> Result<()> {
        let content = tokio::fs::read_to_string(path).await
            .with_context(|| format!("Failed to read secure config file: {:?}", path))?;

        let secure_config: SecureConfiguration = match path.extension().and_then(|ext| ext.to_str()) {
            Some("yaml") | Some("yml") => {
                serde_yaml::from_str(&content)
                    .with_context(|| "Failed to parse YAML secure configuration")?
            }
            Some("json") => {
                serde_json::from_str(&content)
                    .with_context(|| "Failed to parse JSON secure configuration")?
            }
            _ => {
                return Err(ConfigurationError::InvalidFormat {
                    message: format!("Unsupported secure config file format: {:?}", path),
                }.into());
            }
        };

        // Resolve secure values
        let resolved_config = self.secure_manager.resolve_secure_values(&secure_config)?;

        // Validate the resolved configuration
        self.validate_configuration(&resolved_config)?;

        // Update the current configuration
        {
            let mut config = self.config.write().unwrap();
            *config = resolved_config;
        }

        Ok(())
    }

    /// Enable hot reload for configuration files
    pub fn enable_hot_reload(&mut self) -> Result<()> {
        use notify::{Watcher, RecommendedWatcher, RecursiveMode, Event, Config};
        use std::sync::mpsc;
        
        let (tx, rx) = mpsc::channel();
        let config = Config::default();
        let mut watcher: RecommendedWatcher = RecommendedWatcher::new(tx, config)
            .map_err(|e| ConfigurationError::ReloadFailed { 
                message: format!("Failed to create file watcher: {}", e) 
            })?;

        // Watch all configuration file paths
        for config_path in &self.config_paths {
            if config_path.exists() {
                watcher.watch(config_path, RecursiveMode::NonRecursive)
                    .map_err(|e| ConfigurationError::ReloadFailed { 
                        message: format!("Failed to watch config file {:?}: {}", config_path, e) 
                    })?;
            }
        }

        // Spawn a task to handle file change events
        let config_paths = self.config_paths.clone();
        let event_sender = self.event_sender.clone();
        let config_arc = self.config.clone();
        let secure_manager = self.secure_manager.clone();
        let config_history = self.config_history.clone();
        let max_backups = self.max_backups;
        
        tokio::spawn(async move {
            while let Ok(event) = rx.recv() {
                match event {
                    Ok(Event { kind: notify::EventKind::Modify(_), paths, .. }) => {
                        for path in paths {
                            if config_paths.contains(&path) {
                                let timestamp = SystemTime::now();
                                
                                // Notify about file modification
                                let _ = event_sender.send(ConfigurationEvent::FileModified { 
                                    path: path.clone(),
                                    timestamp,
                                });
                                
                                // Create a backup before attempting reload
                                let backup_config = {
                                    config_arc.read().unwrap().clone()
                                };
                                
                                // Notify validation started
                                let _ = event_sender.send(ConfigurationEvent::ValidationStarted {
                                    path: path.clone(),
                                    timestamp: SystemTime::now(),
                                });
                                
                                // Attempt to reload configuration
                                match Self::reload_configuration_internal(
                                    &config_paths, 
                                    &config_arc, 
                                    &secure_manager,
                                    &backup_config,
                                ).await {
                                    Ok(changes) => {
                                        // Create snapshot of successful configuration
                                        let snapshot = ConfigurationSnapshot {
                                            config: backup_config,
                                            timestamp,
                                            trigger_path: path.clone(),
                                            reason: "Hot reload backup".to_string(),
                                        };
                                        
                                        {
                                            let mut history = config_history.write().unwrap();
                                            history.push(snapshot);
                                            if history.len() > max_backups {
                                                history.remove(0);
                                            }
                                        }
                                        
                                        let _ = event_sender.send(ConfigurationEvent::ValidationCompleted {
                                            path: path.clone(),
                                            timestamp: SystemTime::now(),
                                        });
                                        
                                        let _ = event_sender.send(ConfigurationEvent::Reloaded {
                                            path: path.clone(),
                                            timestamp: SystemTime::now(),
                                            changes,
                                        });
                                    }
                                    Err(e) => {
                                        // Rollback to previous configuration on error
                                        {
                                            let mut config = config_arc.write().unwrap();
                                            *config = backup_config;
                                        }
                                        
                                        let _ = event_sender.send(ConfigurationEvent::RolledBack {
                                            path: path.clone(),
                                            error: e.to_string(),
                                            timestamp: SystemTime::now(),
                                        });
                                    }
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
        });

        self._watcher = Some(watcher);
        Ok(())
    }

    /// Reload configuration from all configured paths
    pub async fn reload_configuration(&mut self) -> Result<()> {
        let trigger_path = self.config_paths.first().cloned().unwrap_or_default();
        let timestamp = SystemTime::now();
        
        // Create a backup of the current configuration for rollback
        let backup_config = self.get_configuration();
        
        // Create snapshot before reload
        self.create_snapshot(trigger_path.clone(), "Manual reload backup".to_string())?;
        
        // Notify validation started
        let _ = self.event_sender.send(ConfigurationEvent::ValidationStarted {
            path: trigger_path.clone(),
            timestamp,
        });
        
        match self.load_configuration().await {
            Ok(_) => {
                // Compare configurations to detect changes
                let new_config = self.get_configuration();
                let changes = self.compare_configurations(&backup_config, &new_config);
                
                // Notify validation completed
                let _ = self.event_sender.send(ConfigurationEvent::ValidationCompleted {
                    path: trigger_path.clone(),
                    timestamp: SystemTime::now(),
                });
                
                // Notify about successful reload
                let _ = self.event_sender.send(ConfigurationEvent::Reloaded {
                    path: trigger_path,
                    timestamp: SystemTime::now(),
                    changes,
                });
                Ok(())
            }
            Err(e) => {
                // Rollback to previous configuration on error
                {
                    let mut config = self.config.write().unwrap();
                    *config = backup_config;
                }
                
                // Notify about rollback
                let _ = self.event_sender.send(ConfigurationEvent::RolledBack {
                    path: trigger_path,
                    error: e.to_string(),
                    timestamp: SystemTime::now(),
                });
                
                Err(ConfigurationError::ReloadFailed { 
                    message: format!("Configuration reload failed, rolled back to previous version: {}", e) 
                }.into())
            }
        }
    }

    /// Internal method for reloading configuration (used by hot reload)
    async fn reload_configuration_internal(
        config_paths: &[PathBuf],
        config_arc: &Arc<RwLock<Configuration>>,
        secure_manager: &SecureConfigManager,
        backup_config: &Configuration,
    ) -> Result<Vec<ConfigurationChange>> {
        // Load and merge configurations
        let mut merged_config = Self::default_configuration();
        
        for config_path in config_paths.iter().rev() {
            if config_path.exists() {
                let file_config = Self::load_config_file_static(config_path).await?;
                merged_config = Self::merge_configurations_static(merged_config, file_config)?;
            }
        }

        // Apply environment variable overrides
        merged_config = Self::apply_environment_overrides_static(merged_config)?;

        // Validate the new configuration
        Self::validate_configuration_static(&merged_config)?;

        // Compare configurations to detect changes
        let changes = Self::compare_configurations_static(backup_config, &merged_config);

        // Update the configuration atomically
        {
            let mut config = config_arc.write().unwrap();
            *config = merged_config;
        }

        Ok(changes)
    }

    /// Static version of compare_configurations for use in hot reload
    fn compare_configurations_static(old_config: &Configuration, new_config: &Configuration) -> Vec<ConfigurationChange> {
        let mut changes = Vec::new();
        
        let old_json = serde_json::to_value(old_config).unwrap_or_default();
        let new_json = serde_json::to_value(new_config).unwrap_or_default();
        
        Self::compare_json_values_static("", &old_json, &new_json, &mut changes);
        
        changes
    }

    /// Static version of compare_json_values for use in hot reload
    fn compare_json_values_static(path: &str, old: &serde_json::Value, new: &serde_json::Value, changes: &mut Vec<ConfigurationChange>) {
        match (old, new) {
            (serde_json::Value::Object(old_obj), serde_json::Value::Object(new_obj)) => {
                // Check for modified and removed values
                for (key, old_value) in old_obj {
                    let current_path = if path.is_empty() { key.clone() } else { format!("{}.{}", path, key) };
                    
                    if let Some(new_value) = new_obj.get(key) {
                        if old_value != new_value {
                            if old_value.is_object() && new_value.is_object() {
                                Self::compare_json_values_static(&current_path, old_value, new_value, changes);
                            } else {
                                changes.push(ConfigurationChange {
                                    path: current_path,
                                    old_value: Some(old_value.clone()),
                                    new_value: new_value.clone(),
                                    change_type: ChangeType::Modified,
                                });
                            }
                        }
                    } else {
                        changes.push(ConfigurationChange {
                            path: current_path,
                            old_value: Some(old_value.clone()),
                            new_value: serde_json::Value::Null,
                            change_type: ChangeType::Removed,
                        });
                    }
                }
                
                // Check for added values
                for (key, new_value) in new_obj {
                    if !old_obj.contains_key(key) {
                        let current_path = if path.is_empty() { key.clone() } else { format!("{}.{}", path, key) };
                        changes.push(ConfigurationChange {
                            path: current_path,
                            old_value: None,
                            new_value: new_value.clone(),
                            change_type: ChangeType::Added,
                        });
                    }
                }
            }
            _ => {
                if old != new {
                    changes.push(ConfigurationChange {
                        path: path.to_string(),
                        old_value: Some(old.clone()),
                        new_value: new.clone(),
                        change_type: ChangeType::Modified,
                    });
                }
            }
        }
    }

    /// Static version of load_config_file for use in hot reload
    async fn load_config_file_static(path: &Path) -> Result<Configuration> {
        let content = tokio::fs::read_to_string(path).await
            .with_context(|| format!("Failed to read config file: {:?}", path))?;

        let config: Configuration = match path.extension().and_then(|ext| ext.to_str()) {
            Some("yaml") | Some("yml") => {
                serde_yaml::from_str(&content)
                    .with_context(|| "Failed to parse YAML configuration")?
            }
            Some("json") => {
                serde_json::from_str(&content)
                    .with_context(|| "Failed to parse JSON configuration")?
            }
            Some("toml") => {
                toml::from_str(&content)
                    .with_context(|| "Failed to parse TOML configuration")?
            }
            _ => {
                return Err(ConfigurationError::InvalidFormat {
                    message: format!("Unsupported config file format: {:?}", path),
                }.into());
            }
        };

        Ok(config)
    }

    /// Static version of merge_configurations for use in hot reload
    fn merge_configurations_static(mut base: Configuration, override_config: Configuration) -> Result<Configuration> {
        // Merge execution config
        if override_config.execution.default_strategy != base.execution.default_strategy && !override_config.execution.default_strategy.is_empty() {
            base.execution.default_strategy = override_config.execution.default_strategy;
        }
        if override_config.execution.max_concurrency != base.execution.max_concurrency && override_config.execution.max_concurrency > 0 {
            base.execution.max_concurrency = override_config.execution.max_concurrency;
        }
        if override_config.execution.request_timeout != base.execution.request_timeout && override_config.execution.request_timeout > 0 {
            base.execution.request_timeout = override_config.execution.request_timeout;
        }

        // Merge plugin config
        if !override_config.plugins.plugin_dir.as_os_str().is_empty() {
            base.plugins.plugin_dir = override_config.plugins.plugin_dir;
        }
        base.plugins.auto_load = override_config.plugins.auto_load;
        base.plugins.hot_reload = override_config.plugins.hot_reload;
        
        // Merge plugin-specific configurations
        for (key, value) in override_config.plugins.plugins {
            base.plugins.plugins.insert(key, value);
        }

        // Merge reporting config
        if !override_config.reporting.output_dir.as_os_str().is_empty() {
            base.reporting.output_dir = override_config.reporting.output_dir;
        }
        if !override_config.reporting.formats.is_empty() {
            base.reporting.formats = override_config.reporting.formats;
        }
        for (key, value) in override_config.reporting.templates {
            base.reporting.templates.insert(key, value);
        }

        // Merge auth config
        if override_config.auth.default_method.is_some() {
            base.auth.default_method = override_config.auth.default_method;
        }
        for (key, value) in override_config.auth.methods {
            base.auth.methods.insert(key, value);
        }

        // Merge data source config
        if !override_config.data_sources.default_type.is_empty() {
            base.data_sources.default_type = override_config.data_sources.default_type;
        }
        for (key, value) in override_config.data_sources.sources {
            base.data_sources.sources.insert(key, value);
        }

        // Merge environments
        for (key, value) in override_config.environments {
            base.environments.insert(key, value);
        }

        // Merge custom values
        for (key, value) in override_config.custom {
            base.custom.insert(key, value);
        }

        Ok(base)
    }

    /// Static version of apply_environment_overrides for use in hot reload
    fn apply_environment_overrides_static(mut config: Configuration) -> Result<Configuration> {
        // Override execution settings from environment variables
        if let Ok(strategy) = std::env::var("API_TEST_EXECUTION_STRATEGY") {
            config.execution.default_strategy = strategy;
        }
        
        if let Ok(concurrency) = std::env::var("API_TEST_MAX_CONCURRENCY") {
            config.execution.max_concurrency = concurrency.parse()
                .with_context(|| "Invalid API_TEST_MAX_CONCURRENCY value")?;
        }
        
        if let Ok(timeout) = std::env::var("API_TEST_REQUEST_TIMEOUT") {
            config.execution.request_timeout = timeout.parse()
                .with_context(|| "Invalid API_TEST_REQUEST_TIMEOUT value")?;
        }

        // Override plugin directory from environment
        if let Ok(plugin_dir) = std::env::var("API_TEST_PLUGIN_DIR") {
            config.plugins.plugin_dir = PathBuf::from(plugin_dir);
        }

        // Override output directory from environment
        if let Ok(output_dir) = std::env::var("API_TEST_OUTPUT_DIR") {
            config.reporting.output_dir = PathBuf::from(output_dir);
        }

        Ok(config)
    }

    /// Static version of validate_configuration for use in hot reload
    fn validate_configuration_static(config: &Configuration) -> Result<()> {
        let mut errors = Vec::new();

        // Validate execution strategy
        match config.execution.default_strategy.as_str() {
            "serial" | "parallel" | "distributed" => {}
            _ => errors.push(format!("Invalid execution strategy: {}", config.execution.default_strategy)),
        }

        // Validate concurrency limits
        if config.execution.max_concurrency == 0 {
            errors.push("max_concurrency must be greater than 0".to_string());
        }

        // Validate timeout values
        if config.execution.request_timeout == 0 {
            errors.push("request_timeout must be greater than 0".to_string());
        }

        // Validate retry configuration
        if config.execution.retry.max_attempts == 0 {
            errors.push("retry.max_attempts must be greater than 0".to_string());
        }
        
        if config.execution.retry.base_delay_ms == 0 {
            errors.push("retry.base_delay_ms must be greater than 0".to_string());
        }

        // Validate rate limiting
        if config.execution.rate_limit.requests_per_second <= 0.0 {
            errors.push("rate_limit.requests_per_second must be greater than 0".to_string());
        }

        if !errors.is_empty() {
            return Err(ConfigurationError::ValidationFailed { errors }.into());
        }

        Ok(())
    }

    /// Rollback to a previous configuration
    pub fn rollback_configuration(&mut self, backup_config: Configuration) -> Result<()> {
        // Validate the backup configuration before rolling back
        self.validate_configuration(&backup_config)?;
        
        // Update the current configuration
        {
            let mut config = self.config.write().unwrap();
            *config = backup_config;
        }

        // Notify about rollback
        let _ = self.event_sender.send(ConfigurationEvent::Reloaded {
            path: PathBuf::from("rollback"),
            timestamp: SystemTime::now(),
            changes: vec![],
        });

        Ok(())
    }

    /// Add a configuration file path with specified precedence
    /// Higher precedence files override lower precedence ones
    pub fn add_config_path<P: AsRef<Path>>(&mut self, path: P, precedence: u32) -> Result<()> {
        let path = path.as_ref().to_path_buf();
        
        // Insert at the correct position based on precedence
        let insert_pos = self.config_paths
            .iter()
            .position(|_| true) // For now, just append - we'll sort by precedence later
            .unwrap_or(self.config_paths.len());
            
        self.config_paths.insert(insert_pos, path);
        Ok(())
    }

    /// Load configuration from all configured paths
    pub async fn load_configuration(&mut self) -> Result<()> {
        // Get current configuration for comparison
        let old_config = self.get_configuration();
        
        let mut merged_config = Self::default_configuration();
        
        // Load and merge configurations in order of precedence (lowest to highest)
        for config_path in self.config_paths.iter().rev() {
            if config_path.exists() {
                let file_config = self.load_config_file(config_path).await
                    .with_context(|| format!("Failed to load config from {:?}", config_path))?;
                
                merged_config = self.merge_configurations(merged_config, file_config)?;
                
                // Update file timestamp
                if let Ok(metadata) = std::fs::metadata(config_path) {
                    if let Ok(modified) = metadata.modified() {
                        self.file_timestamps.insert(config_path.clone(), modified);
                    }
                }
            }
        }

        // Apply environment variable overrides
        merged_config = self.apply_environment_overrides(merged_config)?;

        // Validate the final configuration
        self.validate_configuration(&merged_config)?;

        // Update the current configuration
        {
            let mut config = self.config.write().unwrap();
            *config = merged_config;
        }

        Ok(())
    }

    /// Load configuration from a single file
    async fn load_config_file(&self, path: &Path) -> Result<Configuration> {
        let content = tokio::fs::read_to_string(path).await
            .with_context(|| format!("Failed to read config file: {:?}", path))?;

        let config: Configuration = match path.extension().and_then(|ext| ext.to_str()) {
            Some("yaml") | Some("yml") => {
                serde_yaml::from_str(&content)
                    .with_context(|| "Failed to parse YAML configuration")?
            }
            Some("json") => {
                serde_json::from_str(&content)
                    .with_context(|| "Failed to parse JSON configuration")?
            }
            Some("toml") => {
                toml::from_str(&content)
                    .with_context(|| "Failed to parse TOML configuration")?
            }
            _ => {
                return Err(ConfigurationError::InvalidFormat {
                    message: format!("Unsupported config file format: {:?}", path),
                }.into());
            }
        };

        Ok(config)
    }

    /// Merge two configurations, with the second taking precedence
    fn merge_configurations(&self, mut base: Configuration, override_config: Configuration) -> Result<Configuration> {
        // Merge execution config
        if override_config.execution.default_strategy != base.execution.default_strategy && !override_config.execution.default_strategy.is_empty() {
            base.execution.default_strategy = override_config.execution.default_strategy;
        }
        if override_config.execution.max_concurrency != base.execution.max_concurrency && override_config.execution.max_concurrency > 0 {
            base.execution.max_concurrency = override_config.execution.max_concurrency;
        }
        if override_config.execution.request_timeout != base.execution.request_timeout && override_config.execution.request_timeout > 0 {
            base.execution.request_timeout = override_config.execution.request_timeout;
        }

        // Merge retry config
        if override_config.execution.retry.max_attempts != base.execution.retry.max_attempts && override_config.execution.retry.max_attempts > 0 {
            base.execution.retry.max_attempts = override_config.execution.retry.max_attempts;
        }
        if override_config.execution.retry.base_delay_ms != base.execution.retry.base_delay_ms && override_config.execution.retry.base_delay_ms > 0 {
            base.execution.retry.base_delay_ms = override_config.execution.retry.base_delay_ms;
        }
        if override_config.execution.retry.max_delay_ms != base.execution.retry.max_delay_ms && override_config.execution.retry.max_delay_ms > 0 {
            base.execution.retry.max_delay_ms = override_config.execution.retry.max_delay_ms;
        }
        if override_config.execution.retry.backoff_multiplier != base.execution.retry.backoff_multiplier && override_config.execution.retry.backoff_multiplier > 0.0 {
            base.execution.retry.backoff_multiplier = override_config.execution.retry.backoff_multiplier;
        }

        // Merge rate limit config
        if override_config.execution.rate_limit.requests_per_second != base.execution.rate_limit.requests_per_second && override_config.execution.rate_limit.requests_per_second > 0.0 {
            base.execution.rate_limit.requests_per_second = override_config.execution.rate_limit.requests_per_second;
        }
        if override_config.execution.rate_limit.burst_capacity != base.execution.rate_limit.burst_capacity && override_config.execution.rate_limit.burst_capacity > 0 {
            base.execution.rate_limit.burst_capacity = override_config.execution.rate_limit.burst_capacity;
        }

        // Merge plugin config
        if !override_config.plugins.plugin_dir.as_os_str().is_empty() {
            base.plugins.plugin_dir = override_config.plugins.plugin_dir;
        }
        base.plugins.auto_load = override_config.plugins.auto_load;
        base.plugins.hot_reload = override_config.plugins.hot_reload;
        
        // Merge plugin-specific configurations
        for (key, value) in override_config.plugins.plugins {
            base.plugins.plugins.insert(key, value);
        }

        // Merge reporting config
        if !override_config.reporting.output_dir.as_os_str().is_empty() {
            base.reporting.output_dir = override_config.reporting.output_dir;
        }
        if !override_config.reporting.formats.is_empty() {
            base.reporting.formats = override_config.reporting.formats;
        }
        for (key, value) in override_config.reporting.templates {
            base.reporting.templates.insert(key, value);
        }

        // Merge auth config
        if override_config.auth.default_method.is_some() {
            base.auth.default_method = override_config.auth.default_method;
        }
        for (key, value) in override_config.auth.methods {
            base.auth.methods.insert(key, value);
        }

        // Merge data source config
        if !override_config.data_sources.default_type.is_empty() {
            base.data_sources.default_type = override_config.data_sources.default_type;
        }
        for (key, value) in override_config.data_sources.sources {
            base.data_sources.sources.insert(key, value);
        }

        // Merge environments
        for (key, value) in override_config.environments {
            base.environments.insert(key, value);
        }

        // Merge custom values
        for (key, value) in override_config.custom {
            base.custom.insert(key, value);
        }

        Ok(base)
    }

    /// Apply environment variable overrides
    fn apply_environment_overrides(&self, mut config: Configuration) -> Result<Configuration> {
        // Override execution settings from environment variables
        if let Ok(strategy) = std::env::var("API_TEST_EXECUTION_STRATEGY") {
            config.execution.default_strategy = strategy;
        }
        
        if let Ok(concurrency) = std::env::var("API_TEST_MAX_CONCURRENCY") {
            config.execution.max_concurrency = concurrency.parse()
                .with_context(|| "Invalid API_TEST_MAX_CONCURRENCY value")?;
        }
        
        if let Ok(timeout) = std::env::var("API_TEST_REQUEST_TIMEOUT") {
            config.execution.request_timeout = timeout.parse()
                .with_context(|| "Invalid API_TEST_REQUEST_TIMEOUT value")?;
        }

        // Override plugin directory from environment
        if let Ok(plugin_dir) = std::env::var("API_TEST_PLUGIN_DIR") {
            config.plugins.plugin_dir = PathBuf::from(plugin_dir);
        }

        // Override output directory from environment
        if let Ok(output_dir) = std::env::var("API_TEST_OUTPUT_DIR") {
            config.reporting.output_dir = PathBuf::from(output_dir);
        }

        Ok(config)
    }

    /// Validate configuration for consistency and required values
    fn validate_configuration(&self, config: &Configuration) -> Result<()> {
        let mut errors = Vec::new();

        // Validate execution strategy
        match config.execution.default_strategy.as_str() {
            "serial" | "parallel" | "distributed" => {}
            _ => errors.push(format!("Invalid execution strategy: {}", config.execution.default_strategy)),
        }

        // Validate concurrency limits
        if config.execution.max_concurrency == 0 {
            errors.push("max_concurrency must be greater than 0".to_string());
        }

        // Validate timeout values
        if config.execution.request_timeout == 0 {
            errors.push("request_timeout must be greater than 0".to_string());
        }

        // Validate retry configuration
        if config.execution.retry.max_attempts == 0 {
            errors.push("retry.max_attempts must be greater than 0".to_string());
        }
        
        if config.execution.retry.base_delay_ms == 0 {
            errors.push("retry.base_delay_ms must be greater than 0".to_string());
        }

        // Validate rate limiting
        if config.execution.rate_limit.requests_per_second <= 0.0 {
            errors.push("rate_limit.requests_per_second must be greater than 0".to_string());
        }

        // Validate plugin directory exists or can be created
        if !config.plugins.plugin_dir.exists() {
            if let Err(_) = std::fs::create_dir_all(&config.plugins.plugin_dir) {
                errors.push(format!("Cannot create plugin directory: {:?}", config.plugins.plugin_dir));
            }
        }

        // Validate output directory exists or can be created
        if !config.reporting.output_dir.exists() {
            if let Err(_) = std::fs::create_dir_all(&config.reporting.output_dir) {
                errors.push(format!("Cannot create output directory: {:?}", config.reporting.output_dir));
            }
        }

        if !errors.is_empty() {
            return Err(ConfigurationError::ValidationFailed { errors }.into());
        }

        Ok(())
    }

    /// Get the current configuration
    pub fn get_configuration(&self) -> Configuration {
        self.config.read().unwrap().clone()
    }

    /// Get a configuration value by path (dot-separated)
    pub fn get_value(&self, path: &str) -> Option<serde_json::Value> {
        let config = self.config.read().unwrap();
        let config_json = serde_json::to_value(&*config).ok()?;
        
        let mut current = &config_json;
        for part in path.split('.') {
            current = current.get(part)?;
        }
        
        Some(current.clone())
    }

    /// Subscribe to configuration change events
    pub fn subscribe_to_changes(&self) -> broadcast::Receiver<ConfigurationEvent> {
        self.event_sender.subscribe()
    }

    /// Force a configuration reload without file system changes
    pub async fn force_reload(&mut self) -> Result<()> {
        self.reload_configuration().await
    }

    /// Check if configuration files have been modified since last load
    pub fn check_for_modifications(&self) -> Vec<PathBuf> {
        let mut modified_files = Vec::new();
        
        for config_path in &self.config_paths {
            if config_path.exists() {
                if let Ok(metadata) = std::fs::metadata(config_path) {
                    if let Ok(modified) = metadata.modified() {
                        if let Some(last_modified) = self.file_timestamps.get(config_path) {
                            if modified > *last_modified {
                                modified_files.push(config_path.clone());
                            }
                        } else {
                            // File wasn't tracked before, consider it modified
                            modified_files.push(config_path.clone());
                        }
                    }
                }
            }
        }
        
        modified_files
    }

    /// Validate current configuration without reloading
    pub fn validate_current_configuration(&self) -> Result<()> {
        let config = self.get_configuration();
        self.validate_configuration(&config)
    }

    /// Get configuration change summary
    pub fn get_change_summary(&self, changes: &[ConfigurationChange]) -> String {
        if changes.is_empty() {
            return "No changes detected".to_string();
        }

        let mut summary = Vec::new();
        let mut added_count = 0;
        let mut modified_count = 0;
        let mut removed_count = 0;

        for change in changes {
            match change.change_type {
                ChangeType::Added => {
                    added_count += 1;
                    summary.push(format!("+ {}: {}", change.path, change.new_value));
                }
                ChangeType::Modified => {
                    modified_count += 1;
                    summary.push(format!("~ {}: {} -> {}", 
                        change.path, 
                        change.old_value.as_ref().unwrap_or(&serde_json::Value::Null), 
                        change.new_value
                    ));
                }
                ChangeType::Removed => {
                    removed_count += 1;
                    summary.push(format!("- {}: {}", 
                        change.path, 
                        change.old_value.as_ref().unwrap_or(&serde_json::Value::Null)
                    ));
                }
            }
        }

        let header = format!("Configuration changes: {} added, {} modified, {} removed", 
            added_count, modified_count, removed_count);
        
        if summary.len() <= 10 {
            format!("{}\n{}", header, summary.join("\n"))
        } else {
            format!("{}\n{}... and {} more changes", 
                header, 
                summary[..10].join("\n"), 
                summary.len() - 10
            )
        }
    }

    /// Create default configuration
    fn default_configuration() -> Configuration {
        Configuration {
            execution: ExecutionConfig {
                default_strategy: "serial".to_string(),
                max_concurrency: 10,
                request_timeout: 30,
                retry: RetryConfig {
                    max_attempts: 3,
                    base_delay_ms: 1000,
                    max_delay_ms: 30000,
                    backoff_multiplier: 2.0,
                },
                rate_limit: RateLimitConfig {
                    requests_per_second: 10.0,
                    burst_capacity: 20,
                },
            },
            plugins: ConfigPluginConfig {
                plugin_dir: PathBuf::from("plugins"),
                auto_load: true,
                hot_reload: false,
                plugins: HashMap::new(),
            },
            reporting: ReportingConfig {
                output_dir: PathBuf::from("reports"),
                formats: vec!["junit".to_string(), "html".to_string()],
                templates: HashMap::new(),
            },
            auth: ConfigAuthConfig {
                default_method: None,
                methods: HashMap::new(),
            },
            data_sources: ConfigDataSourceConfig {
                default_type: "file".to_string(),
                sources: HashMap::new(),
            },
            environments: HashMap::new(),
            custom: HashMap::new(),
        }
    }
}

impl Default for ConfigurationManager {
    fn default() -> Self {
        Self::new().expect("Failed to create default ConfigurationManager")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use tokio::fs;

    #[tokio::test]
    async fn test_default_configuration() {
        let manager = ConfigurationManager::new().unwrap();
        let config = manager.get_configuration();
        
        assert_eq!(config.execution.default_strategy, "serial");
        assert_eq!(config.execution.max_concurrency, 10);
        assert_eq!(config.execution.request_timeout, 30);
    }

    #[tokio::test]
    async fn test_load_yaml_configuration() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.yaml");
        
        let yaml_content = r#"
execution:
  default_strategy: "parallel"
  max_concurrency: 20
  request_timeout: 60
  retry:
    max_attempts: 3
    base_delay_ms: 1000
    max_delay_ms: 30000
    backoff_multiplier: 2.0
  rate_limit:
    requests_per_second: 10.0
    burst_capacity: 20
plugins:
  plugin_dir: "custom_plugins"
  auto_load: false
  hot_reload: false
  plugins: {}
reporting:
  output_dir: "reports"
  formats: ["junit", "html"]
  templates: {}
auth:
  default_method: null
  methods: {}
data_sources:
  default_type: "file"
  sources: {}
environments: {}
custom: {}
"#;
        
        fs::write(&config_path, yaml_content).await.unwrap();
        
        let mut manager = ConfigurationManager::new().unwrap();
        manager.add_config_path(&config_path, 1).unwrap();
        manager.load_configuration().await.unwrap();
        
        let config = manager.get_configuration();
        assert_eq!(config.execution.default_strategy, "parallel");
        assert_eq!(config.execution.max_concurrency, 20);
        assert_eq!(config.execution.request_timeout, 60);
        assert_eq!(config.plugins.plugin_dir, PathBuf::from("custom_plugins"));
        assert_eq!(config.plugins.auto_load, false);
    }

    #[tokio::test]
    async fn test_configuration_merging() {
        let temp_dir = TempDir::new().unwrap();
        
        // Base configuration
        let base_config_path = temp_dir.path().join("base.yaml");
        let base_yaml = r#"
execution:
  default_strategy: "serial"
  max_concurrency: 10
  request_timeout: 30
  retry:
    max_attempts: 3
    base_delay_ms: 1000
    max_delay_ms: 30000
    backoff_multiplier: 2.0
  rate_limit:
    requests_per_second: 10.0
    burst_capacity: 20
plugins:
  plugin_dir: "plugins"
  auto_load: true
  hot_reload: false
  plugins: {}
reporting:
  output_dir: "reports"
  formats: ["junit", "html"]
  templates: {}
auth:
  default_method: null
  methods: {}
data_sources:
  default_type: "file"
  sources: {}
environments: {}
custom: {}
"#;
        fs::write(&base_config_path, base_yaml).await.unwrap();
        
        // Override configuration
        let override_config_path = temp_dir.path().join("override.yaml");
        let override_yaml = r#"
execution:
  default_strategy: "serial"
  max_concurrency: 20
  request_timeout: 30
  retry:
    max_attempts: 3
    base_delay_ms: 1000
    max_delay_ms: 30000
    backoff_multiplier: 2.0
  rate_limit:
    requests_per_second: 10.0
    burst_capacity: 20
plugins:
  plugin_dir: "plugins"
  auto_load: false
  hot_reload: true
  plugins: {}
reporting:
  output_dir: "reports"
  formats: ["junit", "html"]
  templates: {}
auth:
  default_method: null
  methods: {}
data_sources:
  default_type: "file"
  sources: {}
environments: {}
custom: {}
"#;
        fs::write(&override_config_path, override_yaml).await.unwrap();
        
        let mut manager = ConfigurationManager::new().unwrap();
        manager.add_config_path(&base_config_path, 1).unwrap();
        manager.add_config_path(&override_config_path, 2).unwrap();
        manager.load_configuration().await.unwrap();
        
        let config = manager.get_configuration();
        assert_eq!(config.execution.default_strategy, "serial"); // From base
        assert_eq!(config.execution.max_concurrency, 20); // Overridden
        assert_eq!(config.plugins.auto_load, false); // Overridden
        assert_eq!(config.plugins.hot_reload, true); // From override
    }

    #[tokio::test]
    async fn test_environment_variable_overrides() {
        std::env::set_var("API_TEST_EXECUTION_STRATEGY", "distributed");
        std::env::set_var("API_TEST_MAX_CONCURRENCY", "50");
        
        let mut manager = ConfigurationManager::new().unwrap();
        manager.load_configuration().await.unwrap();
        
        let config = manager.get_configuration();
        assert_eq!(config.execution.default_strategy, "distributed");
        assert_eq!(config.execution.max_concurrency, 50);
        
        // Clean up
        std::env::remove_var("API_TEST_EXECUTION_STRATEGY");
        std::env::remove_var("API_TEST_MAX_CONCURRENCY");
    }

    #[tokio::test]
    async fn test_configuration_validation() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("invalid.yaml");
        
        let invalid_yaml = r#"
execution:
  default_strategy: "invalid_strategy"
  max_concurrency: 0
  request_timeout: 0
  retry:
    max_attempts: 3
    base_delay_ms: 1000
    max_delay_ms: 30000
    backoff_multiplier: 2.0
  rate_limit:
    requests_per_second: 10.0
    burst_capacity: 20
plugins:
  plugin_dir: "plugins"
  auto_load: true
  hot_reload: false
  plugins: {}
reporting:
  output_dir: "reports"
  formats: ["junit", "html"]
  templates: {}
auth:
  default_method: null
  methods: {}
data_sources:
  default_type: "file"
  sources: {}
environments: {}
custom: {}
"#;
        
        fs::write(&config_path, invalid_yaml).await.unwrap();
        
        let mut manager = ConfigurationManager::new().unwrap();
        manager.add_config_path(&config_path, 1).unwrap();
        
        let result = manager.load_configuration().await;
        assert!(result.is_err());
        
        if let Err(e) = result {
            let error_msg = e.to_string();
            assert!(error_msg.contains("Invalid execution strategy"));
        }
    }

    #[test]
    fn test_get_configuration_value() {
        let manager = ConfigurationManager::new().unwrap();
        
        let strategy = manager.get_value("execution.default_strategy");
        assert_eq!(strategy, Some(serde_json::Value::String("serial".to_string())));
        
        let concurrency = manager.get_value("execution.max_concurrency");
        assert_eq!(concurrency, Some(serde_json::Value::Number(10.into())));
        
        let nonexistent = manager.get_value("nonexistent.path");
        assert_eq!(nonexistent, None);
    }

    #[test]
    fn test_secure_config_manager_encryption() {
        let mut secure_manager = SecureConfigManager::new("test-service");
        secure_manager.initialize_with_password("test-password-123").unwrap();
        
        let plaintext = "super-secret-api-key";
        let encrypted = secure_manager.encrypt_value(plaintext).unwrap();
        
        // Verify it's encrypted
        match &encrypted {
            SecureValue::Encrypted { algorithm, .. } => {
                assert_eq!(algorithm, "AES-256-GCM");
            }
            _ => panic!("Expected encrypted value"),
        }
        
        // Verify we can decrypt it back
        let decrypted = secure_manager.decrypt_value(&encrypted).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_secure_value_env_var() {
        std::env::set_var("TEST_SECRET_VALUE", "env-secret-123");
        
        let secure_manager = SecureConfigManager::new("test-service");
        let env_value = SecureValue::EnvVar {
            env_var: "TEST_SECRET_VALUE".to_string(),
            default: None,
        };
        
        let resolved = secure_manager.decrypt_value(&env_value).unwrap();
        assert_eq!(resolved, "env-secret-123");
        
        std::env::remove_var("TEST_SECRET_VALUE");
    }

    #[test]
    fn test_secure_value_env_var_with_default() {
        let secure_manager = SecureConfigManager::new("test-service");
        let env_value = SecureValue::EnvVar {
            env_var: "NONEXISTENT_VAR".to_string(),
            default: Some("default-value".to_string()),
        };
        
        let resolved = secure_manager.decrypt_value(&env_value).unwrap();
        assert_eq!(resolved, "default-value");
    }

    #[test]
    fn test_secure_value_plain() {
        let secure_manager = SecureConfigManager::new("test-service");
        let plain_value = SecureValue::Plain("plain-text-value".to_string());
        
        let resolved = secure_manager.decrypt_value(&plain_value).unwrap();
        assert_eq!(resolved, "plain-text-value");
    }

    #[tokio::test]
    async fn test_secure_configuration_loading() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("secure_config.yaml");
        
        let secure_yaml = r#"
execution:
  default_strategy: "serial"
  max_concurrency: 10
  request_timeout: 30
  retry:
    max_attempts: 3
    base_delay_ms: 1000
    max_delay_ms: 30000
    backoff_multiplier: 2.0
  rate_limit:
    requests_per_second: 10.0
    burst_capacity: 20
plugins:
  plugin_dir: "plugins"
  auto_load: true
  hot_reload: false
  plugins: {}
reporting:
  output_dir: "reports"
  formats: ["junit", "html"]
  templates: {}
auth:
  default_method: null
  methods: {}
data_sources:
  default_type: "file"
  sources: {}
environments: {}
custom: {}
secure_values:
  "auth.methods.oauth2.client_secret":
    env_var: "OAUTH2_CLIENT_SECRET"
    default: "default-secret"
  "data_sources.sources.db.password":
    env_var: "DB_PASSWORD"
    default: null
"#;
        
        fs::write(&config_path, secure_yaml).await.unwrap();
        
        // Set environment variables
        std::env::set_var("OAUTH2_CLIENT_SECRET", "oauth-secret-123");
        std::env::set_var("DB_PASSWORD", "db-password-456");
        
        let mut manager = ConfigurationManager::new().unwrap();
        manager.load_secure_configuration(&config_path).await.unwrap();
        
        let config = manager.get_configuration();
        assert_eq!(config.execution.default_strategy, "serial");
        
        // Clean up
        std::env::remove_var("OAUTH2_CLIENT_SECRET");
        std::env::remove_var("DB_PASSWORD");
    }

    #[test]
    fn test_configuration_manager_secure_value_storage() {
        let mut manager = ConfigurationManager::new().unwrap();
        manager.initialize_secure_config("test-password").unwrap();
        
        // Test environment variable storage (just validation)
        std::env::set_var("TEST_API_KEY", "test-key-123");
        let result = manager.store_secure_value("TEST_API_KEY", "test-key-123", SecureStorageType::EnvVar);
        assert!(result.is_ok());
        
        // Test retrieval
        let retrieved = manager.get_secure_value("TEST_API_KEY").unwrap();
        assert_eq!(retrieved, "test-key-123");
        
        std::env::remove_var("TEST_API_KEY");
    }

    #[test]
    fn test_secure_config_validation_errors() {
        let secure_manager = SecureConfigManager::new("test-service");
        
        // Test decryption without master key
        let encrypted_value = SecureValue::Encrypted {
            encrypted_data: "dummy".to_string(),
            nonce: "dummy".to_string(),
            algorithm: "AES-256-GCM".to_string(),
        };
        
        let result = secure_manager.decrypt_value(&encrypted_value);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Master key not initialized"));
        
        // Test unsupported algorithm
        let mut secure_manager = SecureConfigManager::new("test-service");
        secure_manager.initialize_with_password("test").unwrap();
        
        let unsupported_value = SecureValue::Encrypted {
            encrypted_data: "dummy".to_string(),
            nonce: "dummy".to_string(),
            algorithm: "UNSUPPORTED".to_string(),
        };
        
        let result = secure_manager.decrypt_value(&unsupported_value);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Unsupported encryption algorithm"));
    }

    #[tokio::test]
    async fn test_configuration_reload() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.yaml");
        
        // Initial configuration
        let initial_yaml = r#"
execution:
  default_strategy: "serial"
  max_concurrency: 10
  request_timeout: 30
  retry:
    max_attempts: 3
    base_delay_ms: 1000
    max_delay_ms: 30000
    backoff_multiplier: 2.0
  rate_limit:
    requests_per_second: 10.0
    burst_capacity: 20
plugins:
  plugin_dir: "plugins"
  auto_load: true
  hot_reload: false
  plugins: {}
reporting:
  output_dir: "reports"
  formats: ["junit", "html"]
  templates: {}
auth:
  default_method: null
  methods: {}
data_sources:
  default_type: "file"
  sources: {}
environments: {}
custom: {}
"#;
        
        fs::write(&config_path, initial_yaml).await.unwrap();
        
        let mut manager = ConfigurationManager::new().unwrap();
        manager.add_config_path(&config_path, 1).unwrap();
        manager.load_configuration().await.unwrap();
        
        let initial_config = manager.get_configuration();
        assert_eq!(initial_config.execution.default_strategy, "serial");
        assert_eq!(initial_config.execution.max_concurrency, 10);
        
        // Update configuration file
        let updated_yaml = r#"
execution:
  default_strategy: "parallel"
  max_concurrency: 20
  request_timeout: 60
  retry:
    max_attempts: 3
    base_delay_ms: 1000
    max_delay_ms: 30000
    backoff_multiplier: 2.0
  rate_limit:
    requests_per_second: 10.0
    burst_capacity: 20
plugins:
  plugin_dir: "plugins"
  auto_load: true
  hot_reload: false
  plugins: {}
reporting:
  output_dir: "reports"
  formats: ["junit", "html"]
  templates: {}
auth:
  default_method: null
  methods: {}
data_sources:
  default_type: "file"
  sources: {}
environments: {}
custom: {}
"#;
        
        fs::write(&config_path, updated_yaml).await.unwrap();
        
        // Reload configuration
        manager.reload_configuration().await.unwrap();
        
        let updated_config = manager.get_configuration();
        assert_eq!(updated_config.execution.default_strategy, "parallel");
        assert_eq!(updated_config.execution.max_concurrency, 20);
        assert_eq!(updated_config.execution.request_timeout, 60);
    }

    #[tokio::test]
    async fn test_configuration_reload_with_rollback() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.yaml");
        
        // Initial valid configuration
        let initial_yaml = r#"
execution:
  default_strategy: "serial"
  max_concurrency: 10
  request_timeout: 30
  retry:
    max_attempts: 3
    base_delay_ms: 1000
    max_delay_ms: 30000
    backoff_multiplier: 2.0
  rate_limit:
    requests_per_second: 10.0
    burst_capacity: 20
plugins:
  plugin_dir: "plugins"
  auto_load: true
  hot_reload: false
  plugins: {}
reporting:
  output_dir: "reports"
  formats: ["junit", "html"]
  templates: {}
auth:
  default_method: null
  methods: {}
data_sources:
  default_type: "file"
  sources: {}
environments: {}
custom: {}
"#;
        
        fs::write(&config_path, initial_yaml).await.unwrap();
        
        let mut manager = ConfigurationManager::new().unwrap();
        manager.add_config_path(&config_path, 1).unwrap();
        manager.load_configuration().await.unwrap();
        
        let initial_config = manager.get_configuration();
        assert_eq!(initial_config.execution.default_strategy, "serial");
        
        // Write invalid configuration
        let invalid_yaml = r#"
execution:
  default_strategy: "invalid_strategy"
  max_concurrency: 0
  request_timeout: 0
  retry:
    max_attempts: 3
    base_delay_ms: 1000
    max_delay_ms: 30000
    backoff_multiplier: 2.0
  rate_limit:
    requests_per_second: 10.0
    burst_capacity: 20
plugins:
  plugin_dir: "plugins"
  auto_load: true
  hot_reload: false
  plugins: {}
reporting:
  output_dir: "reports"
  formats: ["junit", "html"]
  templates: {}
auth:
  default_method: null
  methods: {}
data_sources:
  default_type: "file"
  sources: {}
environments: {}
custom: {}
"#;
        
        fs::write(&config_path, invalid_yaml).await.unwrap();
        
        // Attempt to reload - should fail and rollback
        let result = manager.reload_configuration().await;
        assert!(result.is_err());
        
        // Configuration should be rolled back to previous valid state
        let current_config = manager.get_configuration();
        assert_eq!(current_config.execution.default_strategy, "serial");
        assert_eq!(current_config.execution.max_concurrency, 10);
    }

    #[test]
    fn test_configuration_rollback() {
        let mut manager = ConfigurationManager::new().unwrap();
        let original_config = manager.get_configuration();
        
        // Create a modified configuration
        let mut modified_config = original_config.clone();
        modified_config.execution.default_strategy = "parallel".to_string();
        modified_config.execution.max_concurrency = 50;
        
        // Update configuration
        {
            let mut config = manager.config.write().unwrap();
            *config = modified_config;
        }
        
        // Verify configuration was changed
        let current_config = manager.get_configuration();
        assert_eq!(current_config.execution.default_strategy, "parallel");
        assert_eq!(current_config.execution.max_concurrency, 50);
        
        // Rollback to original configuration
        manager.rollback_configuration(original_config.clone()).unwrap();
        
        // Verify rollback worked
        let rolled_back_config = manager.get_configuration();
        assert_eq!(rolled_back_config.execution.default_strategy, original_config.execution.default_strategy);
        assert_eq!(rolled_back_config.execution.max_concurrency, original_config.execution.max_concurrency);
    }

    #[test]
    fn test_configuration_event_subscription() {
        let manager = ConfigurationManager::new().unwrap();
        let mut receiver = manager.subscribe_to_changes();
        
        // Send a test event
        let test_event = ConfigurationEvent::Reloaded {
            path: PathBuf::from("test.yaml"),
            timestamp: SystemTime::now(),
            changes: vec![],
        };
        
        let _ = manager.event_sender.send(test_event.clone());
        
        // Receive the event
        let received_event = receiver.try_recv().unwrap();
        match received_event {
            ConfigurationEvent::Reloaded { path, .. } => {
                assert_eq!(path, PathBuf::from("test.yaml"));
            }
            _ => panic!("Expected Reloaded event"),
        }
    }

    #[test]
    fn test_configuration_change_detection() {
        let manager = ConfigurationManager::new().unwrap();
        
        let mut old_config = ConfigurationManager::default_configuration();
        old_config.execution.default_strategy = "serial".to_string();
        old_config.execution.max_concurrency = 10;
        
        let mut new_config = old_config.clone();
        new_config.execution.default_strategy = "parallel".to_string();
        new_config.execution.max_concurrency = 20;
        new_config.execution.request_timeout = 60;
        
        let changes = manager.compare_configurations(&old_config, &new_config);
        
        assert_eq!(changes.len(), 3);
        
        // Check for strategy change
        let strategy_change = changes.iter().find(|c| c.path == "execution.default_strategy").unwrap();
        assert_eq!(strategy_change.change_type, ChangeType::Modified);
        assert_eq!(strategy_change.old_value, Some(serde_json::Value::String("serial".to_string())));
        assert_eq!(strategy_change.new_value, serde_json::Value::String("parallel".to_string()));
        
        // Check for concurrency change
        let concurrency_change = changes.iter().find(|c| c.path == "execution.max_concurrency").unwrap();
        assert_eq!(concurrency_change.change_type, ChangeType::Modified);
        assert_eq!(concurrency_change.old_value, Some(serde_json::Value::Number(10.into())));
        assert_eq!(concurrency_change.new_value, serde_json::Value::Number(20.into()));
    }

    #[test]
    fn test_configuration_snapshot_creation() {
        let manager = ConfigurationManager::new().unwrap();
        
        let trigger_path = PathBuf::from("test.yaml");
        let reason = "Test snapshot".to_string();
        
        manager.create_snapshot(trigger_path.clone(), reason.clone()).unwrap();
        
        let history = manager.get_configuration_history();
        assert_eq!(history.len(), 1);
        
        let snapshot = &history[0];
        assert_eq!(snapshot.trigger_path, trigger_path);
        assert_eq!(snapshot.reason, reason);
    }

    #[test]
    fn test_configuration_rollback_to_last_valid() {
        let mut manager = ConfigurationManager::new().unwrap();
        let original_config = manager.get_configuration();
        
        // Create a snapshot
        manager.create_snapshot(PathBuf::from("test.yaml"), "Test backup".to_string()).unwrap();
        
        // Modify current configuration
        {
            let mut config = manager.config.write().unwrap();
            config.execution.default_strategy = "parallel".to_string();
            config.execution.max_concurrency = 50;
        }
        
        // Verify configuration was changed
        let modified_config = manager.get_configuration();
        assert_eq!(modified_config.execution.default_strategy, "parallel");
        assert_eq!(modified_config.execution.max_concurrency, 50);
        
        // Rollback to last valid
        manager.rollback_to_last_valid().unwrap();
        
        // Verify rollback worked
        let rolled_back_config = manager.get_configuration();
        assert_eq!(rolled_back_config.execution.default_strategy, original_config.execution.default_strategy);
        assert_eq!(rolled_back_config.execution.max_concurrency, original_config.execution.max_concurrency);
    }

    #[test]
    fn test_configuration_change_summary() {
        let manager = ConfigurationManager::new().unwrap();
        
        let changes = vec![
            ConfigurationChange {
                path: "execution.default_strategy".to_string(),
                old_value: Some(serde_json::Value::String("serial".to_string())),
                new_value: serde_json::Value::String("parallel".to_string()),
                change_type: ChangeType::Modified,
            },
            ConfigurationChange {
                path: "execution.max_concurrency".to_string(),
                old_value: Some(serde_json::Value::Number(10.into())),
                new_value: serde_json::Value::Number(20.into()),
                change_type: ChangeType::Modified,
            },
            ConfigurationChange {
                path: "new_setting".to_string(),
                old_value: None,
                new_value: serde_json::Value::String("new_value".to_string()),
                change_type: ChangeType::Added,
            },
        ];
        
        let summary = manager.get_change_summary(&changes);
        assert!(summary.contains("Configuration changes: 1 added, 2 modified, 0 removed"));
        assert!(summary.contains("execution.default_strategy"));
        assert!(summary.contains("execution.max_concurrency"));
        assert!(summary.contains("new_setting"));
    }

    #[test]
    fn test_check_for_modifications() {
        let manager = ConfigurationManager::new().unwrap();
        
        // Since we don't have any config paths, should return empty
        let modifications = manager.check_for_modifications();
        assert!(modifications.is_empty());
    }

    #[test]
    fn test_validate_current_configuration() {
        let manager = ConfigurationManager::new().unwrap();
        
        // Default configuration should be valid
        let result = manager.validate_current_configuration();
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_force_reload() {
        let mut manager = ConfigurationManager::new().unwrap();
        
        // Force reload should work even without config files
        let result = manager.force_reload().await;
        assert!(result.is_ok());
    }

    #[test]
    fn test_configuration_events_with_changes() {
        let manager = ConfigurationManager::new().unwrap();
        let mut receiver = manager.subscribe_to_changes();
        
        let changes = vec![
            ConfigurationChange {
                path: "test.path".to_string(),
                old_value: None,
                new_value: serde_json::Value::String("test".to_string()),
                change_type: ChangeType::Added,
            },
        ];
        
        // Send events with different types
        let events = vec![
            ConfigurationEvent::FileModified {
                path: PathBuf::from("test.yaml"),
                timestamp: SystemTime::now(),
            },
            ConfigurationEvent::ValidationStarted {
                path: PathBuf::from("test.yaml"),
                timestamp: SystemTime::now(),
            },
            ConfigurationEvent::ValidationCompleted {
                path: PathBuf::from("test.yaml"),
                timestamp: SystemTime::now(),
            },
            ConfigurationEvent::Reloaded {
                path: PathBuf::from("test.yaml"),
                timestamp: SystemTime::now(),
                changes: changes.clone(),
            },
            ConfigurationEvent::RolledBack {
                path: PathBuf::from("test.yaml"),
                error: "Test error".to_string(),
                timestamp: SystemTime::now(),
            },
        ];
        
        for event in events {
            let _ = manager.event_sender.send(event);
        }
        
        // Verify we can receive all events
        for _ in 0..5 {
            let received = receiver.try_recv();
            assert!(received.is_ok());
        }
    }
}