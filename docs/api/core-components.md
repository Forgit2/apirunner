# Core Components

This document describes the core system components and APIs that form the foundation of the API Test Runner architecture.

## Plugin Manager

The Plugin Manager handles dynamic loading, registration, and lifecycle management of plugins.

```rust
pub struct PluginManager {
    plugins: HashMap<String, Box<dyn Plugin>>,
    protocol_plugins: HashMap<String, Box<dyn ProtocolPlugin>>,
    assertion_plugins: HashMap<String, Box<dyn AssertionPlugin>>,
    data_source_plugins: HashMap<String, Box<dyn DataSourcePlugin>>,
    auth_plugins: HashMap<String, Box<dyn AuthPlugin>>,
    reporter_plugins: HashMap<String, Box<dyn ReporterPlugin>>,
    plugin_directory: PathBuf,
    dependency_graph: DependencyGraph,
}

impl PluginManager {
    /// Create a new plugin manager
    pub fn new(plugin_directory: PathBuf) -> Self {
        Self {
            plugins: HashMap::new(),
            protocol_plugins: HashMap::new(),
            assertion_plugins: HashMap::new(),
            data_source_plugins: HashMap::new(),
            auth_plugins: HashMap::new(),
            reporter_plugins: HashMap::new(),
            plugin_directory,
            dependency_graph: DependencyGraph::new(),
        }
    }
    
    /// Discover and load all plugins from the plugin directory
    pub async fn discover_plugins(&mut self) -> Result<Vec<String>, PluginError> {
        let mut loaded_plugins = Vec::new();
        
        for entry in std::fs::read_dir(&self.plugin_directory)? {
            let entry = entry?;
            let path = entry.path();
            
            if path.extension().and_then(|s| s.to_str()) == Some("so") ||
               path.extension().and_then(|s| s.to_str()) == Some("dll") {
                match self.load_plugin(&path).await {
                    Ok(plugin_name) => loaded_plugins.push(plugin_name),
                    Err(e) => eprintln!("Failed to load plugin {:?}: {}", path, e),
                }
            }
        }
        
        Ok(loaded_plugins)
    }
    
    /// Load a specific plugin from a library file
    pub async fn load_plugin(&mut self, library_path: &Path) -> Result<String, PluginError> {
        unsafe {
            let lib = libloading::Library::new(library_path)?;
            let create_plugin: libloading::Symbol<unsafe extern "C" fn() -> *mut dyn Plugin> =
                lib.get(b"create_plugin")?;
            
            let plugin_ptr = create_plugin();
            let mut plugin = Box::from_raw(plugin_ptr);
            
            let plugin_name = plugin.name().to_string();
            
            // Add to dependency graph
            self.dependency_graph.add_plugin(&plugin_name, plugin.dependencies());
            
            // Register plugin by type
            self.register_plugin_by_type(plugin).await?;
            
            // Keep the library loaded
            std::mem::forget(lib);
            
            Ok(plugin_name)
        }
    }
    
    /// Register a plugin based on its type
    async fn register_plugin_by_type(&mut self, plugin: Box<dyn Plugin>) -> Result<(), PluginError> {
        let name = plugin.name().to_string();
        
        // Try to cast to specific plugin types
        if let Ok(protocol_plugin) = plugin.downcast::<dyn ProtocolPlugin>() {
            self.protocol_plugins.insert(name.clone(), protocol_plugin);
        } else if let Ok(assertion_plugin) = plugin.downcast::<dyn AssertionPlugin>() {
            self.assertion_plugins.insert(name.clone(), assertion_plugin);
        } else if let Ok(data_source_plugin) = plugin.downcast::<dyn DataSourcePlugin>() {
            self.data_source_plugins.insert(name.clone(), data_source_plugin);
        } else if let Ok(auth_plugin) = plugin.downcast::<dyn AuthPlugin>() {
            self.auth_plugins.insert(name.clone(), auth_plugin);
        } else if let Ok(reporter_plugin) = plugin.downcast::<dyn ReporterPlugin>() {
            self.reporter_plugins.insert(name.clone(), reporter_plugin);
        }
        
        self.plugins.insert(name, plugin);
        Ok(())
    }
    
    /// Initialize all plugins in dependency order
    pub async fn initialize_plugins(&mut self, config: &PluginConfig) -> Result<(), PluginError> {
        let initialization_order = self.dependency_graph.resolve_dependencies()?;
        
        for plugin_name in initialization_order {
            if let Some(plugin) = self.plugins.get_mut(&plugin_name) {
                plugin.initialize(config).await?;
            }
        }
        
        Ok(())
    }
    
    /// Shutdown all plugins
    pub async fn shutdown_plugins(&mut self) -> Result<(), PluginError> {
        for plugin in self.plugins.values_mut() {
            if let Err(e) = plugin.shutdown().await {
                eprintln!("Error shutting down plugin {}: {}", plugin.name(), e);
            }
        }
        Ok(())
    }
    
    /// Get a protocol plugin by name
    pub fn get_protocol_plugin(&self, name: &str) -> Option<&dyn ProtocolPlugin> {
        self.protocol_plugins.get(name).map(|p| p.as_ref())
    }
    
    /// Get an assertion plugin by type
    pub fn get_assertion_plugin(&self, assertion_type: &AssertionType) -> Option<&dyn AssertionPlugin> {
        self.assertion_plugins.values()
            .find(|p| &p.assertion_type() == assertion_type)
            .map(|p| p.as_ref())
    }
    
    /// Get all assertion plugins sorted by priority
    pub fn get_assertion_plugins(&self) -> Vec<&dyn AssertionPlugin> {
        let mut plugins: Vec<&dyn AssertionPlugin> = self.assertion_plugins.values()
            .map(|p| p.as_ref())
            .collect();
        plugins.sort_by(|a, b| b.priority().cmp(&a.priority()));
        plugins
    }
}
```

## Configuration Manager

The Configuration Manager handles hierarchical configuration loading and management.

```rust
pub struct ConfigurationManager {
    config: Configuration,
    watchers: HashMap<PathBuf, FileWatcher>,
    reload_callbacks: Vec<Box<dyn Fn(&Configuration) -> Result<(), ConfigError> + Send + Sync>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Configuration {
    /// Global settings
    pub global: GlobalConfig,
    /// Environment-specific configurations
    pub environments: HashMap<String, EnvironmentConfig>,
    /// Plugin configurations
    pub plugins: HashMap<String, PluginConfig>,
    /// Data source configurations
    pub data_sources: HashMap<String, DataSourceConfig>,
    /// Authentication configurations
    pub auth: HashMap<String, AuthConfig>,
    /// Reporting configurations
    pub reporting: ReportingConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalConfig {
    pub default_environment: String,
    pub plugin_directory: PathBuf,
    pub log_level: String,
    pub max_concurrent_tests: Option<usize>,
    pub default_timeout: Duration,
}

impl ConfigurationManager {
    /// Create a new configuration manager
    pub fn new() -> Self {
        Self {
            config: Configuration::default(),
            watchers: HashMap::new(),
            reload_callbacks: Vec::new(),
        }
    }
    
    /// Load configuration from multiple sources
    pub async fn load_configuration(&mut self, config_paths: &[PathBuf]) -> Result<(), ConfigError> {
        let mut merged_config = Configuration::default();
        
        for config_path in config_paths {
            let config_content = std::fs::read_to_string(config_path)?;
            let config: Configuration = match config_path.extension().and_then(|s| s.to_str()) {
                Some("yaml") | Some("yml") => serde_yaml::from_str(&config_content)?,
                Some("json") => serde_json::from_str(&config_content)?,
                Some("toml") => toml::from_str(&config_content)?,
                _ => return Err(ConfigError::UnsupportedFormat(config_path.clone())),
            };
            
            merged_config = self.merge_configurations(merged_config, config)?;
        }
        
        // Override with environment variables
        self.apply_environment_overrides(&mut merged_config)?;
        
        // Validate configuration
        self.validate_configuration(&merged_config)?;
        
        self.config = merged_config;
        Ok(())
    }
    
    /// Merge two configurations with the second taking precedence
    fn merge_configurations(&self, base: Configuration, override_config: Configuration) -> Result<Configuration, ConfigError> {
        // Implementation would merge configurations recursively
        // This is a simplified version
        Ok(Configuration {
            global: if override_config.global != GlobalConfig::default() {
                override_config.global
            } else {
                base.global
            },
            environments: {
                let mut merged = base.environments;
                merged.extend(override_config.environments);
                merged
            },
            plugins: {
                let mut merged = base.plugins;
                merged.extend(override_config.plugins);
                merged
            },
            data_sources: {
                let mut merged = base.data_sources;
                merged.extend(override_config.data_sources);
                merged
            },
            auth: {
                let mut merged = base.auth;
                merged.extend(override_config.auth);
                merged
            },
            reporting: if override_config.reporting != ReportingConfig::default() {
                override_config.reporting
            } else {
                base.reporting
            },
        })
    }
    
    /// Apply environment variable overrides
    fn apply_environment_overrides(&self, config: &mut Configuration) -> Result<(), ConfigError> {
        // Check for environment variable overrides
        if let Ok(env) = std::env::var("API_TEST_RUNNER_ENVIRONMENT") {
            config.global.default_environment = env;
        }
        
        if let Ok(log_level) = std::env::var("API_TEST_RUNNER_LOG_LEVEL") {
            config.global.log_level = log_level;
        }
        
        if let Ok(timeout) = std::env::var("API_TEST_RUNNER_DEFAULT_TIMEOUT") {
            if let Ok(timeout_secs) = timeout.parse::<u64>() {
                config.global.default_timeout = Duration::from_secs(timeout_secs);
            }
        }
        
        Ok(())
    }
    
    /// Validate configuration
    fn validate_configuration(&self, config: &Configuration) -> Result<(), ConfigError> {
        // Validate that default environment exists
        if !config.environments.contains_key(&config.global.default_environment) {
            return Err(ConfigError::InvalidConfiguration(
                format!("Default environment '{}' not found", config.global.default_environment)
            ));
        }
        
        // Validate plugin directory exists
        if !config.global.plugin_directory.exists() {
            return Err(ConfigError::InvalidConfiguration(
                format!("Plugin directory '{}' does not exist", config.global.plugin_directory.display())
            ));
        }
        
        Ok(())
    }
    
    /// Get current configuration
    pub fn get_configuration(&self) -> &Configuration {
        &self.config
    }
    
    /// Get environment-specific configuration
    pub fn get_environment_config(&self, environment: &str) -> Option<&EnvironmentConfig> {
        self.config.environments.get(environment)
    }
    
    /// Watch configuration files for changes
    pub async fn watch_configuration_files(&mut self, config_paths: &[PathBuf]) -> Result<(), ConfigError> {
        for config_path in config_paths {
            let watcher = FileWatcher::new(config_path.clone(), {
                let config_paths = config_paths.to_vec();
                move || {
                    // Reload configuration when file changes
                    tokio::spawn(async move {
                        // Implementation would reload configuration
                    });
                }
            })?;
            
            self.watchers.insert(config_path.clone(), watcher);
        }
        
        Ok(())
    }
    
    /// Register a callback for configuration reloads
    pub fn on_configuration_reload<F>(&mut self, callback: F)
    where
        F: Fn(&Configuration) -> Result<(), ConfigError> + Send + Sync + 'static,
    {
        self.reload_callbacks.push(Box::new(callback));
    }
}
```

## Test Case Manager

The Test Case Manager handles CRUD operations for test cases and test suites.

```rust
pub struct TestCaseManager {
    storage_backend: Box<dyn StorageBackend>,
    indexer: TestCaseIndexer,
    validator: TestCaseValidator,
    template_manager: Arc<TemplateManager>,
}

impl TestCaseManager {
    /// Create a new test case manager
    pub fn new(
        storage_backend: Box<dyn StorageBackend>,
        template_manager: Arc<TemplateManager>,
    ) -> Self {
        Self {
            storage_backend,
            indexer: TestCaseIndexer::new(),
            validator: TestCaseValidator::new(),
            template_manager,
        }
    }
    
    /// Create a new test case
    pub async fn create_test_case(&mut self, test_case: &TestCase) -> Result<String, TestCaseError> {
        // Validate test case
        self.validator.validate(test_case)?;
        
        // Store test case
        let test_case_id = self.storage_backend.create_test_case(test_case).await?;
        
        // Update index
        self.indexer.add_test_case(&test_case_id, test_case).await?;
        
        Ok(test_case_id)
    }
    
    /// Get a test case by ID
    pub async fn get_test_case(&self, id: &str) -> Result<TestCase, TestCaseError> {
        self.storage_backend.read_test_case(id).await
    }
    
    /// Update an existing test case
    pub async fn update_test_case(&mut self, id: &str, test_case: &TestCase) -> Result<(), TestCaseError> {
        // Validate test case
        self.validator.validate(test_case)?;
        
        // Update storage
        self.storage_backend.update_test_case(id, test_case).await?;
        
        // Update index
        self.indexer.update_test_case(id, test_case).await?;
        
        Ok(())
    }
    
    /// Delete a test case
    pub async fn delete_test_case(&mut self, id: &str) -> Result<(), TestCaseError> {
        // Remove from storage
        self.storage_backend.delete_test_case(id).await?;
        
        // Remove from index
        self.indexer.remove_test_case(id).await?;
        
        Ok(())
    }
    
    /// Search for test cases
    pub async fn search_test_cases(&self, query: &SearchQuery) -> Result<Vec<TestCaseMetadata>, TestCaseError> {
        self.indexer.search(query).await
    }
    
    /// List all test cases with optional filtering
    pub async fn list_test_cases(&self, filter: &TestCaseFilter) -> Result<Vec<TestCaseMetadata>, TestCaseError> {
        self.storage_backend.list_test_cases(filter).await
    }
    
    /// Create test case from template
    pub async fn create_from_template(
        &mut self,
        template_id: &str,
        variables: HashMap<String, String>,
        name: String,
    ) -> Result<String, TestCaseError> {
        let test_case = self.template_manager
            .create_from_template(template_id, variables)
            .await?;
            
        let mut test_case = test_case;
        test_case.name = name;
        test_case.id = Uuid::new_v4().to_string();
        
        self.create_test_case(&test_case).await
    }
    
    /// Import test cases from external format
    pub async fn import_test_cases(
        &mut self,
        source: &ImportSource,
        format: ImportFormat,
    ) -> Result<Vec<String>, TestCaseError> {
        let importer = self.get_importer(format)?;
        let test_cases = importer.import(source).await?;
        
        let mut imported_ids = Vec::new();
        for test_case in test_cases {
            let id = self.create_test_case(&test_case).await?;
            imported_ids.push(id);
        }
        
        Ok(imported_ids)
    }
    
    /// Export test cases to external format
    pub async fn export_test_cases(
        &self,
        test_case_ids: &[String],
        format: ExportFormat,
        output_path: &Path,
    ) -> Result<(), TestCaseError> {
        let mut test_cases = Vec::new();
        for id in test_case_ids {
            let test_case = self.get_test_case(id).await?;
            test_cases.push(test_case);
        }
        
        let exporter = self.get_exporter(format)?;
        exporter.export(&test_cases, output_path).await?;
        
        Ok(())
    }
}
```

## Metrics Collector

The Metrics Collector gathers and aggregates performance and execution metrics.

```rust
pub struct MetricsCollector {
    metrics: Arc<Mutex<HashMap<String, MetricValue>>>,
    histograms: Arc<Mutex<HashMap<String, Histogram>>>,
    counters: Arc<Mutex<HashMap<String, Counter>>>,
    gauges: Arc<Mutex<HashMap<String, Gauge>>>,
    exporters: Vec<Box<dyn MetricsExporter>>,
}

#[derive(Debug, Clone)]
pub enum MetricValue {
    Counter(u64),
    Gauge(f64),
    Histogram(Vec<f64>),
    Timer(Duration),
}

impl MetricsCollector {
    /// Create a new metrics collector
    pub fn new() -> Self {
        Self {
            metrics: Arc::new(Mutex::new(HashMap::new())),
            histograms: Arc::new(Mutex::new(HashMap::new())),
            counters: Arc::new(Mutex::new(HashMap::new())),
            gauges: Arc::new(Mutex::new(HashMap::new())),
            exporters: Vec::new(),
        }
    }
    
    /// Record a counter metric
    pub fn increment_counter(&self, name: &str, value: u64) {
        if let Ok(mut counters) = self.counters.lock() {
            let counter = counters.entry(name.to_string()).or_insert_with(Counter::new);
            counter.increment(value);
        }
    }
    
    /// Record a gauge metric
    pub fn set_gauge(&self, name: &str, value: f64) {
        if let Ok(mut gauges) = self.gauges.lock() {
            let gauge = gauges.entry(name.to_string()).or_insert_with(Gauge::new);
            gauge.set(value);
        }
    }
    
    /// Record a histogram metric
    pub fn record_histogram(&self, name: &str, value: f64) {
        if let Ok(mut histograms) = self.histograms.lock() {
            let histogram = histograms.entry(name.to_string()).or_insert_with(Histogram::new);
            histogram.record(value);
        }
    }
    
    /// Record a timer metric
    pub fn record_timer(&self, name: &str, duration: Duration) {
        self.record_histogram(name, duration.as_secs_f64() * 1000.0); // Convert to milliseconds
    }
    
    /// Get current metric values
    pub fn get_metrics(&self) -> HashMap<String, MetricValue> {
        let mut all_metrics = HashMap::new();
        
        if let Ok(counters) = self.counters.lock() {
            for (name, counter) in counters.iter() {
                all_metrics.insert(name.clone(), MetricValue::Counter(counter.value()));
            }
        }
        
        if let Ok(gauges) = self.gauges.lock() {
            for (name, gauge) in gauges.iter() {
                all_metrics.insert(name.clone(), MetricValue::Gauge(gauge.value()));
            }
        }
        
        if let Ok(histograms) = self.histograms.lock() {
            for (name, histogram) in histograms.iter() {
                all_metrics.insert(name.clone(), MetricValue::Histogram(histogram.values()));
            }
        }
        
        all_metrics
    }
    
    /// Export metrics to registered exporters
    pub async fn export_metrics(&self) -> Result<(), MetricsError> {
        let metrics = self.get_metrics();
        
        for exporter in &self.exporters {
            exporter.export(&metrics).await?;
        }
        
        Ok(())
    }
    
    /// Add a metrics exporter
    pub fn add_exporter(&mut self, exporter: Box<dyn MetricsExporter>) {
        self.exporters.push(exporter);
    }
    
    /// Get performance summary
    pub fn get_performance_summary(&self) -> PerformanceMetrics {
        let metrics = self.get_metrics();
        
        let response_times: Vec<f64> = metrics.get("response_time")
            .and_then(|m| match m {
                MetricValue::Histogram(values) => Some(values.clone()),
                _ => None,
            })
            .unwrap_or_default();
            
        let avg_response_time = if !response_times.is_empty() {
            Duration::from_secs_f64(response_times.iter().sum::<f64>() / response_times.len() as f64 / 1000.0)
        } else {
            Duration::ZERO
        };
        
        let min_response_time = response_times.iter()
            .min_by(|a, b| a.partial_cmp(b).unwrap())
            .map(|&t| Duration::from_secs_f64(t / 1000.0))
            .unwrap_or(Duration::ZERO);
            
        let max_response_time = response_times.iter()
            .max_by(|a, b| a.partial_cmp(b).unwrap())
            .map(|&t| Duration::from_secs_f64(t / 1000.0))
            .unwrap_or(Duration::ZERO);
        
        let mut percentiles = HashMap::new();
        if !response_times.is_empty() {
            let mut sorted_times = response_times.clone();
            sorted_times.sort_by(|a, b| a.partial_cmp(b).unwrap());
            
            percentiles.insert("p50".to_string(), 
                Duration::from_secs_f64(percentile(&sorted_times, 0.5) / 1000.0));
            percentiles.insert("p95".to_string(), 
                Duration::from_secs_f64(percentile(&sorted_times, 0.95) / 1000.0));
            percentiles.insert("p99".to_string(), 
                Duration::from_secs_f64(percentile(&sorted_times, 0.99) / 1000.0));
        }
        
        let total_requests = metrics.get("total_requests")
            .and_then(|m| match m {
                MetricValue::Counter(count) => Some(*count as f64),
                _ => None,
            })
            .unwrap_or(0.0);
            
        let failed_requests = metrics.get("failed_requests")
            .and_then(|m| match m {
                MetricValue::Counter(count) => Some(*count as f64),
                _ => None,
            })
            .unwrap_or(0.0);
        
        let error_rate = if total_requests > 0.0 {
            failed_requests / total_requests
        } else {
            0.0
        };
        
        let execution_duration = metrics.get("execution_duration")
            .and_then(|m| match m {
                MetricValue::Timer(duration) => Some(*duration),
                _ => None,
            })
            .unwrap_or(Duration::ZERO);
            
        let throughput = if execution_duration.as_secs_f64() > 0.0 {
            total_requests / execution_duration.as_secs_f64()
        } else {
            0.0
        };
        
        PerformanceMetrics {
            avg_response_time,
            min_response_time,
            max_response_time,
            percentiles,
            throughput,
            error_rate,
        }
    }
}

fn percentile(sorted_values: &[f64], p: f64) -> f64 {
    if sorted_values.is_empty() {
        return 0.0;
    }
    
    let index = (p * (sorted_values.len() - 1) as f64).round() as usize;
    sorted_values[index.min(sorted_values.len() - 1)]
}
```

## Error Types

```rust
#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    #[error("Plugin error: {0}")]
    PluginError(#[from] PluginError),
    
    #[error("Configuration error: {0}")]
    ConfigError(#[from] ConfigError),
    
    #[error("Test case error: {0}")]
    TestCaseError(#[from] TestCaseError),
    
    #[error("Metrics error: {0}")]
    MetricsError(#[from] MetricsError),
    
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    
    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("Invalid configuration: {0}")]
    InvalidConfiguration(String),
    
    #[error("Unsupported format: {0}")]
    UnsupportedFormat(PathBuf),
    
    #[error("File not found: {0}")]
    FileNotFound(PathBuf),
    
    #[error("Parse error: {0}")]
    ParseError(String),
}

#[derive(Debug, thiserror::Error)]
pub enum TestCaseError {
    #[error("Validation error: {0}")]
    ValidationError(String),
    
    #[error("Storage error: {0}")]
    StorageError(String),
    
    #[error("Import error: {0}")]
    ImportError(String),
    
    #[error("Export error: {0}")]
    ExportError(String),
    
    #[error("Template error: {0}")]
    TemplateError(String),
}

#[derive(Debug, thiserror::Error)]
pub enum MetricsError {
    #[error("Export error: {0}")]
    ExportError(String),
    
    #[error("Collection error: {0}")]
    CollectionError(String),
}
```

## Usage Examples

### Plugin Manager Usage

```rust
use api_test_runner::core::{PluginManager, PluginConfig};

let mut plugin_manager = PluginManager::new(PathBuf::from("./plugins"));

// Discover and load plugins
let loaded_plugins = plugin_manager.discover_plugins().await?;
println!("Loaded plugins: {:?}", loaded_plugins);

// Initialize plugins
let config = PluginConfig {
    settings: HashMap::new(),
    environment: "development".to_string(),
};
plugin_manager.initialize_plugins(&config).await?;

// Use plugins
if let Some(http_plugin) = plugin_manager.get_protocol_plugin("http") {
    let response = http_plugin.execute_request(request).await?;
}
```

### Configuration Manager Usage

```rust
use api_test_runner::core::ConfigurationManager;

let mut config_manager = ConfigurationManager::new();

// Load configuration from multiple sources
let config_paths = vec![
    PathBuf::from("config/base.yaml"),
    PathBuf::from("config/development.yaml"),
];
config_manager.load_configuration(&config_paths).await?;

// Watch for configuration changes
config_manager.watch_configuration_files(&config_paths).await?;

// Register reload callback
config_manager.on_configuration_reload(|config| {
    println!("Configuration reloaded: {:?}", config.global);
    Ok(())
});
```

### Test Case Manager Usage

```rust
use api_test_runner::core::TestCaseManager;

let mut test_manager = TestCaseManager::new(
    Box::new(FileSystemStorage::new(PathBuf::from("./tests"))),
    Arc::new(template_manager),
);

// Create a test case
let test_case = TestCase {
    id: Uuid::new_v4().to_string(),
    name: "Get Users Test".to_string(),
    // ... other fields
};
let test_id = test_manager.create_test_case(&test_case).await?;

// Search for test cases
let query = SearchQuery {
    text: Some("users".to_string()),
    tags: vec!["api".to_string()],
    // ... other criteria
};
let results = test_manager.search_test_cases(&query).await?;
```

### Metrics Collector Usage

```rust
use api_test_runner::core::MetricsCollector;

let metrics = MetricsCollector::new();

// Record metrics during test execution
metrics.increment_counter("total_requests", 1);
metrics.record_timer("response_time", response_duration);
metrics.set_gauge("active_connections", 5.0);

// Get performance summary
let summary = metrics.get_performance_summary();
println!("Average response time: {:?}", summary.avg_response_time);

// Export metrics
metrics.export_metrics().await?;
```