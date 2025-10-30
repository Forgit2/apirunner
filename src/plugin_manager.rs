use crate::error::{PluginError, Result};
use crate::plugin::{Plugin, PluginConfig};
use async_trait::async_trait;
use libloading::{Library, Symbol};
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::{mpsc, RwLock};

/// Plugin registry entry containing metadata and loaded plugin
pub struct PluginEntry {
    pub metadata: PluginMetadata,
    pub plugin: Box<dyn Plugin>,
    pub library: Option<Library>,
    pub status: PluginStatus,
}

/// Plugin metadata for registration and discovery
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginMetadata {
    pub name: String,
    pub version: String,
    pub description: Option<String>,
    pub author: Option<String>,
    pub dependencies: Vec<String>,
    pub plugin_type: PluginType,
    pub file_path: PathBuf,
}

/// Types of plugins supported by the system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PluginType {
    Protocol,
    Assertion,
    DataSource,
    Reporter,
    Authentication,
    Monitor,
}

/// Current status of a plugin
#[derive(Debug, Clone, PartialEq)]
pub enum PluginStatus {
    Discovered,
    Loading,
    Loaded,
    Initialized,
    Failed(String),
    Unloading,
}

/// Plugin factory function signature for dynamic loading
pub type PluginFactory = unsafe fn() -> *mut dyn Plugin;

/// Plugin manager responsible for discovery, loading, and lifecycle management
pub struct PluginManager {
    plugins: Arc<RwLock<HashMap<String, PluginEntry>>>,
    plugin_directories: Vec<PathBuf>,
    config: PluginManagerConfig,
    file_watcher: Option<RecommendedWatcher>,
    reload_sender: Option<mpsc::UnboundedSender<PluginReloadEvent>>,
}

/// Configuration for the plugin manager
#[derive(Debug, Clone)]
pub struct PluginManagerConfig {
    pub auto_discover: bool,
    pub auto_load: bool,
    pub isolation_enabled: bool,
    pub crash_recovery: bool,
    pub hot_reload_enabled: bool,
    pub reload_debounce_ms: u64,
}

/// Plugin reload event for hot reload functionality
#[derive(Debug, Clone)]
pub struct PluginReloadEvent {
    pub plugin_name: String,
    pub file_path: PathBuf,
    pub event_type: ReloadEventType,
    pub timestamp: SystemTime,
}

/// Types of reload events
#[derive(Debug, Clone, PartialEq)]
pub enum ReloadEventType {
    Modified,
    Created,
    Deleted,
}

impl Default for PluginManagerConfig {
    fn default() -> Self {
        Self {
            auto_discover: true,
            auto_load: true,
            isolation_enabled: true,
            crash_recovery: true,
            hot_reload_enabled: true,
            reload_debounce_ms: 500,
        }
    }
}

impl PluginManager {
    /// Create a new plugin manager with default configuration
    pub fn new() -> Self {
        Self {
            plugins: Arc::new(RwLock::new(HashMap::new())),
            plugin_directories: vec![PathBuf::from("plugins")],
            config: PluginManagerConfig::default(),
            file_watcher: None,
            reload_sender: None,
        }
    }

    /// Create a new plugin manager with custom configuration
    pub fn with_config(config: PluginManagerConfig) -> Self {
        Self {
            plugins: Arc::new(RwLock::new(HashMap::new())),
            plugin_directories: vec![PathBuf::from("plugins")],
            config,
            file_watcher: None,
            reload_sender: None,
        }
    }

    /// Add a directory to scan for plugins
    pub fn add_plugin_directory<P: AsRef<Path>>(&mut self, path: P) {
        self.plugin_directories.push(path.as_ref().to_path_buf());
    }

    /// Discover all plugins in configured directories
    pub async fn discover_plugins(&self) -> Result<Vec<PluginMetadata>> {
        let mut discovered = Vec::new();

        for directory in &self.plugin_directories {
            if !directory.exists() {
                continue;
            }

            let entries = std::fs::read_dir(directory)
                .map_err(|e| PluginError::LoadingFailed(format!("Failed to read directory {}: {}", directory.display(), e)))?;

            for entry in entries {
                let entry = entry.map_err(|e| PluginError::LoadingFailed(format!("Failed to read directory entry: {}", e)))?;
                let path = entry.path();

                // Look for dynamic libraries (platform-specific extensions)
                if self.is_plugin_file(&path) {
                    match self.extract_plugin_metadata(&path).await {
                        Ok(metadata) => {
                            discovered.push(metadata);
                        }
                        Err(e) => {
                            eprintln!("Warning: Failed to extract metadata from {}: {}", path.display(), e);
                        }
                    }
                }
            }
        }

        Ok(discovered)
    }

    /// Check if a file is a potential plugin based on extension
    fn is_plugin_file(&self, path: &Path) -> bool {
        if let Some(extension) = path.extension() {
            match extension.to_str() {
                Some("so") => true,  // Linux
                Some("dylib") => true, // macOS
                Some("dll") => true, // Windows
                _ => false,
            }
        } else {
            false
        }
    }

    /// Extract plugin metadata from a plugin file
    async fn extract_plugin_metadata(&self, path: &Path) -> Result<PluginMetadata> {
        // For now, we'll derive metadata from the filename and attempt to load the plugin
        // In a real implementation, you might have a separate metadata file or embedded metadata
        let name = path.file_stem()
            .and_then(|s| s.to_str())
            .ok_or_else(|| PluginError::LoadingFailed("Invalid plugin filename".to_string()))?
            .to_string();

        // Try to load the library temporarily to get metadata
        let library = unsafe {
            Library::new(path)
                .map_err(|e| PluginError::LoadingFailed(format!("Failed to load library {}: {}", path.display(), e)))?
        };

        // Look for metadata function
        let metadata = unsafe {
            match library.get::<Symbol<unsafe fn() -> PluginMetadata>>(b"plugin_metadata") {
                Ok(metadata_fn) => metadata_fn(),
                Err(_) => {
                    // Fallback: create basic metadata
                    PluginMetadata {
                        name: name.clone(),
                        version: "unknown".to_string(),
                        description: None,
                        author: None,
                        dependencies: vec![],
                        plugin_type: PluginType::Protocol, // Default type
                        file_path: path.to_path_buf(),
                    }
                }
            }
        };

        Ok(metadata)
    }

    /// Load a plugin by name
    pub async fn load_plugin(&self, name: &str) -> Result<()> {
        let mut plugins = self.plugins.write().await;
        
        if let Some(entry) = plugins.get_mut(name) {
            if entry.status == PluginStatus::Loaded || entry.status == PluginStatus::Initialized {
                return Ok(()); // Already loaded
            }

            entry.status = PluginStatus::Loading;
        } else {
            return Err(PluginError::NotFound(name.to_string()).into());
        }

        // Get the file path
        let file_path = plugins.get(name).unwrap().metadata.file_path.clone();
        drop(plugins); // Release the lock before loading

        // Load the plugin
        match self.load_plugin_from_file(&file_path).await {
            Ok((plugin, library)) => {
                let mut plugins = self.plugins.write().await;
                if let Some(entry) = plugins.get_mut(name) {
                    entry.plugin = plugin;
                    entry.library = Some(library);
                    entry.status = PluginStatus::Loaded;
                }
                Ok(())
            }
            Err(e) => {
                let mut plugins = self.plugins.write().await;
                if let Some(entry) = plugins.get_mut(name) {
                    entry.status = PluginStatus::Failed(e.to_string());
                }
                Err(e)
            }
        }
    }

    /// Load a plugin from a specific file
    async fn load_plugin_from_file(&self, path: &Path) -> Result<(Box<dyn Plugin>, Library)> {
        let library = unsafe {
            Library::new(path)
                .map_err(|e| PluginError::LoadingFailed(format!("Failed to load library {}: {}", path.display(), e)))?
        };

        let plugin = unsafe {
            let factory: Symbol<PluginFactory> = library
                .get(b"create_plugin")
                .map_err(|e| PluginError::LoadingFailed(format!("Plugin factory function not found: {}", e)))?;

            let plugin_ptr = factory();
            if plugin_ptr.is_null() {
                return Err(PluginError::LoadingFailed("Plugin factory returned null".to_string()).into());
            }

            Box::from_raw(plugin_ptr)
        };

        Ok((plugin, library))
    }

    /// Register a discovered plugin
    pub async fn register_plugin(&self, metadata: PluginMetadata) -> Result<()> {
        let mut plugins = self.plugins.write().await;
        
        if plugins.contains_key(&metadata.name) {
            return Err(PluginError::LoadingFailed(format!("Plugin {} already registered", metadata.name)).into());
        }

        // Create a placeholder plugin entry
        let entry = PluginEntry {
            metadata: metadata.clone(),
            plugin: Box::new(PlaceholderPlugin::new(&metadata.name)),
            library: None,
            status: PluginStatus::Discovered,
        };

        plugins.insert(metadata.name.clone(), entry);
        Ok(())
    }

    /// Initialize a loaded plugin
    pub async fn initialize_plugin(&self, name: &str, config: &PluginConfig) -> Result<()> {
        let mut plugins = self.plugins.write().await;
        
        if let Some(entry) = plugins.get_mut(name) {
            if entry.status != PluginStatus::Loaded {
                return Err(PluginError::InitializationFailed(format!("Plugin {} is not loaded", name)).into());
            }

            match entry.plugin.initialize(config).await {
                Ok(()) => {
                    entry.status = PluginStatus::Initialized;
                    Ok(())
                }
                Err(e) => {
                    entry.status = PluginStatus::Failed(e.to_string());
                    Err(e.into())
                }
            }
        } else {
            Err(PluginError::NotFound(name.to_string()).into())
        }
    }

    /// Unload a plugin
    pub async fn unload_plugin(&self, name: &str) -> Result<()> {
        let mut plugins = self.plugins.write().await;
        
        if let Some(mut entry) = plugins.remove(name) {
            entry.status = PluginStatus::Unloading;
            
            // Shutdown the plugin
            if let Err(e) = entry.plugin.shutdown().await {
                eprintln!("Warning: Plugin {} shutdown failed: {}", name, e);
            }

            // The library will be dropped automatically, unloading the plugin
            Ok(())
        } else {
            Err(PluginError::NotFound(name.to_string()).into())
        }
    }

    /// Get plugin by name (returns whether plugin exists and is initialized)
    pub async fn has_plugin(&self, name: &str) -> bool {
        let plugins = self.plugins.read().await;
        plugins.get(name)
            .map(|entry| entry.status == PluginStatus::Initialized)
            .unwrap_or(false)
    }

    /// List all registered plugins
    pub async fn list_plugins(&self) -> Vec<(String, PluginStatus)> {
        let plugins = self.plugins.read().await;
        plugins.iter()
            .map(|(name, entry)| (name.clone(), entry.status.clone()))
            .collect()
    }

    /// Get plugin metadata
    pub async fn get_plugin_metadata(&self, name: &str) -> Option<PluginMetadata> {
        let plugins = self.plugins.read().await;
        plugins.get(name).map(|entry| entry.metadata.clone())
    }

    /// Auto-discover and optionally auto-load plugins
    pub async fn auto_discover_and_load(&self) -> Result<()> {
        if !self.config.auto_discover {
            return Ok(());
        }

        let discovered = self.discover_plugins().await?;
        
        for metadata in discovered {
            self.register_plugin(metadata.clone()).await?;
        }
        
        if self.config.auto_load {
            // Load plugins in dependency order
            let load_order = self.resolve_load_order().await?;
            for plugin_name in load_order {
                if let Err(e) = self.load_plugin(&plugin_name).await {
                    eprintln!("Warning: Failed to auto-load plugin {}: {}", plugin_name, e);
                }
            }
        }

        Ok(())
    }

    /// Resolve the correct loading order for plugins based on dependencies
    pub async fn resolve_load_order(&self) -> Result<Vec<String>> {
        let plugins = self.plugins.read().await;
        let mut dependency_graph: HashMap<String, Vec<String>> = HashMap::new();
        let mut in_degree: HashMap<String, usize> = HashMap::new();
        
        // Build dependency graph and initialize in-degrees
        for (name, entry) in plugins.iter() {
            dependency_graph.insert(name.clone(), entry.metadata.dependencies.clone());
            in_degree.insert(name.clone(), 0);
        }
        
        // Validate all dependencies exist
        for (name, dependencies) in dependency_graph.iter() {
            for dep in dependencies {
                if !plugins.contains_key(dep) {
                    return Err(PluginError::DependencyNotSatisfied(format!("Plugin {} depends on {}, but {} is not registered", name, dep, dep)).into());
                }
            }
        }
        
        // Calculate in-degrees (count how many dependencies each plugin has)
        for (name, dependencies) in dependency_graph.iter() {
            for _dep in dependencies {
                if let Some(degree) = in_degree.get_mut(name) {
                    *degree += 1;
                }
            }
        }
        
        // Topological sort using Kahn's algorithm
        let mut queue: VecDeque<String> = VecDeque::new();
        let mut result: Vec<String> = Vec::new();
        
        // Find all nodes with no dependencies (in-degree = 0)
        for (name, &degree) in in_degree.iter() {
            if degree == 0 {
                queue.push_back(name.clone());
            }
        }
        
        while let Some(current) = queue.pop_front() {
            result.push(current.clone());
            
            // For each plugin that depends on the current plugin, reduce its in-degree
            for (plugin_name, dependencies) in dependency_graph.iter() {
                if dependencies.contains(&current) {
                    if let Some(degree) = in_degree.get_mut(plugin_name) {
                        *degree -= 1;
                        if *degree == 0 {
                            queue.push_back(plugin_name.clone());
                        }
                    }
                }
            }
        }
        
        // Check for circular dependencies
        if result.len() != plugins.len() {
            return Err(PluginError::DependencyNotSatisfied("Circular dependency detected".to_string()).into());
        }
        
        Ok(result)
    }

    /// Load plugins in dependency order
    pub async fn load_plugins_with_dependencies(&self, plugin_names: &[String]) -> Result<()> {
        // First, ensure all requested plugins and their dependencies are registered
        self.validate_dependencies(plugin_names).await?;
        
        // Get the correct loading order
        let load_order = self.resolve_load_order().await?;
        
        // Filter to only load requested plugins and their dependencies
        let mut plugins_to_load = HashSet::new();
        for name in plugin_names {
            self.collect_dependencies(name, &mut plugins_to_load).await?;
        }
        
        // Load in dependency order
        for plugin_name in load_order {
            if plugins_to_load.contains(&plugin_name) {
                self.load_plugin(&plugin_name).await?;
            }
        }
        
        Ok(())
    }

    /// Validate that all dependencies are satisfied
    async fn validate_dependencies(&self, plugin_names: &[String]) -> Result<()> {
        let plugins = self.plugins.read().await;
        
        for name in plugin_names {
            if let Some(entry) = plugins.get(name) {
                for dep in &entry.metadata.dependencies {
                    if !plugins.contains_key(dep) {
                        return Err(PluginError::DependencyNotSatisfied(format!("Plugin {} requires {}, but {} is not registered", name, dep, dep)).into());
                    }
                }
            } else {
                return Err(PluginError::NotFound(name.clone()).into());
            }
        }
        
        Ok(())
    }

    /// Recursively collect all dependencies for a plugin
    fn collect_dependencies<'a>(&'a self, plugin_name: &'a str, collected: &'a mut HashSet<String>) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<()>> + 'a>> {
        Box::pin(async move {
            if collected.contains(plugin_name) {
                return Ok(()); // Already processed
            }
            
            let plugins = self.plugins.read().await;
            if let Some(entry) = plugins.get(plugin_name) {
                collected.insert(plugin_name.to_string());
                let dependencies = entry.metadata.dependencies.clone();
                drop(plugins); // Release the lock before recursive calls
                
                for dep in &dependencies {
                    self.collect_dependencies(dep, collected).await?;
                }
            } else {
                return Err(PluginError::NotFound(plugin_name.to_string()).into());
            }
            
            Ok(())
        })
    }

    /// Check if a plugin's dependencies are satisfied
    pub async fn check_dependencies(&self, plugin_name: &str) -> Result<bool> {
        let plugins = self.plugins.read().await;
        
        if let Some(entry) = plugins.get(plugin_name) {
            for dep in &entry.metadata.dependencies {
                if let Some(dep_entry) = plugins.get(dep) {
                    if dep_entry.status != PluginStatus::Initialized && dep_entry.status != PluginStatus::Loaded {
                        return Ok(false);
                    }
                } else {
                    return Ok(false);
                }
            }
            Ok(true)
        } else {
            Err(PluginError::NotFound(plugin_name.to_string()).into())
        }
    }

    /// Load plugin with crash protection and isolation
    pub async fn load_plugin_safe(&self, name: &str) -> Result<()> {
        if !self.config.isolation_enabled {
            return self.load_plugin(name).await;
        }

        // In a real implementation, this would use process isolation or sandboxing
        // For now, we'll use panic catching
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            // This is a simplified version - in practice you'd use async-compatible panic handling
            tokio::runtime::Handle::current().block_on(async {
                self.load_plugin(name).await
            })
        }));

        match result {
            Ok(load_result) => load_result,
            Err(_) => {
                // Mark plugin as failed due to crash
                let mut plugins = self.plugins.write().await;
                if let Some(entry) = plugins.get_mut(name) {
                    entry.status = PluginStatus::Failed("Plugin crashed during loading".to_string());
                }
                Err(PluginError::LoadingFailed(format!("Plugin {} crashed during loading", name)).into())
            }
        }
    }

    /// Initialize plugin with crash protection
    pub async fn initialize_plugin_safe(&self, name: &str, config: &PluginConfig) -> Result<()> {
        if !self.config.isolation_enabled {
            return self.initialize_plugin(name, config).await;
        }

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            tokio::runtime::Handle::current().block_on(async {
                self.initialize_plugin(name, config).await
            })
        }));

        match result {
            Ok(init_result) => init_result,
            Err(_) => {
                // Mark plugin as failed due to crash
                let mut plugins = self.plugins.write().await;
                if let Some(entry) = plugins.get_mut(name) {
                    entry.status = PluginStatus::Failed("Plugin crashed during initialization".to_string());
                }
                Err(PluginError::InitializationFailed(format!("Plugin {} crashed during initialization", name)).into())
            }
        }
    }

    /// Start hot reload monitoring for plugin directories
    pub async fn start_hot_reload(&mut self) -> Result<mpsc::UnboundedReceiver<PluginReloadEvent>> {
        if !self.config.hot_reload_enabled {
            return Err(PluginError::LoadingFailed("Hot reload is disabled".to_string()).into());
        }

        let (tx, rx) = mpsc::unbounded_channel();
        self.reload_sender = Some(tx.clone());

        let mut watcher = notify::recommended_watcher(move |res: notify::Result<Event>| {
            match res {
                Ok(event) => {
                    if let Err(e) = Self::handle_file_event(event, &tx) {
                        eprintln!("Error handling file event: {}", e);
                    }
                }
                Err(e) => eprintln!("File watcher error: {}", e),
            }
        }).map_err(|e| PluginError::LoadingFailed(format!("Failed to create file watcher: {}", e)))?;

        // Watch all plugin directories
        for dir in &self.plugin_directories {
            if dir.exists() {
                watcher.watch(dir, RecursiveMode::NonRecursive)
                    .map_err(|e| PluginError::LoadingFailed(format!("Failed to watch directory {}: {}", dir.display(), e)))?;
            }
        }

        self.file_watcher = Some(watcher);
        Ok(rx)
    }

    /// Handle file system events for hot reload
    fn handle_file_event(event: Event, sender: &mpsc::UnboundedSender<PluginReloadEvent>) -> Result<()> {
        match event.kind {
            EventKind::Modify(_) => {
                for path in event.paths {
                    if Self::is_plugin_file_static(&path) {
                        let plugin_name = Self::extract_plugin_name_from_path(&path)?;
                        let reload_event = PluginReloadEvent {
                            plugin_name,
                            file_path: path,
                            event_type: ReloadEventType::Modified,
                            timestamp: SystemTime::now(),
                        };
                        sender.send(reload_event).map_err(|e| PluginError::LoadingFailed(format!("Failed to send reload event: {}", e)))?;
                    }
                }
            }
            EventKind::Create(_) => {
                for path in event.paths {
                    if Self::is_plugin_file_static(&path) {
                        let plugin_name = Self::extract_plugin_name_from_path(&path)?;
                        let reload_event = PluginReloadEvent {
                            plugin_name,
                            file_path: path,
                            event_type: ReloadEventType::Created,
                            timestamp: SystemTime::now(),
                        };
                        sender.send(reload_event).map_err(|e| PluginError::LoadingFailed(format!("Failed to send reload event: {}", e)))?;
                    }
                }
            }
            EventKind::Remove(_) => {
                for path in event.paths {
                    if Self::is_plugin_file_static(&path) {
                        let plugin_name = Self::extract_plugin_name_from_path(&path)?;
                        let reload_event = PluginReloadEvent {
                            plugin_name,
                            file_path: path,
                            event_type: ReloadEventType::Deleted,
                            timestamp: SystemTime::now(),
                        };
                        sender.send(reload_event).map_err(|e| PluginError::LoadingFailed(format!("Failed to send reload event: {}", e)))?;
                    }
                }
            }
            _ => {} // Ignore other event types
        }
        Ok(())
    }

    /// Static version of is_plugin_file for use in static context
    fn is_plugin_file_static(path: &Path) -> bool {
        if let Some(extension) = path.extension() {
            match extension.to_str() {
                Some("so") => true,  // Linux
                Some("dylib") => true, // macOS
                Some("dll") => true, // Windows
                _ => false,
            }
        } else {
            false
        }
    }

    /// Extract plugin name from file path
    fn extract_plugin_name_from_path(path: &Path) -> Result<String> {
        path.file_stem()
            .and_then(|s| s.to_str())
            .map(|s| s.to_string())
            .ok_or_else(|| PluginError::LoadingFailed("Invalid plugin filename".to_string()).into())
    }

    /// Process a reload event
    pub async fn process_reload_event(&self, event: PluginReloadEvent) -> Result<()> {
        match event.event_type {
            ReloadEventType::Modified => {
                self.reload_plugin(&event.plugin_name).await
            }
            ReloadEventType::Created => {
                // Discover and register the new plugin
                if let Ok(metadata) = self.extract_plugin_metadata(&event.file_path).await {
                    self.register_plugin(metadata).await?;
                    if self.config.auto_load {
                        self.load_plugin(&event.plugin_name).await?;
                    }
                }
                Ok(())
            }
            ReloadEventType::Deleted => {
                self.unload_plugin(&event.plugin_name).await
            }
        }
    }

    /// Reload a specific plugin
    pub async fn reload_plugin(&self, name: &str) -> Result<()> {
        // First, check if the plugin exists and get its metadata
        let (metadata, was_initialized) = {
            let plugins = self.plugins.read().await;
            if let Some(entry) = plugins.get(name) {
                (entry.metadata.clone(), entry.status == PluginStatus::Initialized)
            } else {
                return Err(PluginError::NotFound(name.to_string()).into());
            }
        };

        // Unload the current plugin
        self.unload_plugin(name).await?;

        // Wait a bit to ensure file operations are complete
        tokio::time::sleep(Duration::from_millis(self.config.reload_debounce_ms)).await;

        // Re-register the plugin (in case metadata changed)
        let new_metadata = self.extract_plugin_metadata(&metadata.file_path).await?;
        self.register_plugin(new_metadata).await?;

        // Reload the plugin
        self.load_plugin(name).await?;

        // If it was previously initialized, initialize it again
        if was_initialized {
            let default_config = PluginConfig {
                settings: HashMap::new(),
                environment: "default".to_string(),
            };
            self.initialize_plugin(name, &default_config).await?;
        }

        Ok(())
    }

    /// Create a hot reload processor task
    pub async fn create_hot_reload_processor(&mut self) -> Result<tokio::task::JoinHandle<()>> {
        let mut receiver = self.start_hot_reload().await?;
        let _plugins = self.plugins.clone();
        let debounce_ms = self.config.reload_debounce_ms;
        
        let handle = tokio::spawn(async move {
            let mut pending_events: HashMap<String, PluginReloadEvent> = HashMap::new();
            let mut debounce_timer = tokio::time::interval(Duration::from_millis(100));
            
            loop {
                tokio::select! {
                    event = receiver.recv() => {
                        if let Some(event) = event {
                            // Store the latest event for each plugin (debouncing)
                            pending_events.insert(event.plugin_name.clone(), event);
                        } else {
                            break; // Channel closed
                        }
                    }
                    _ = debounce_timer.tick() => {
                        // Process pending events that are old enough
                        let now = SystemTime::now();
                        let mut to_process = Vec::new();
                        
                        pending_events.retain(|_, event| {
                            if let Ok(elapsed) = now.duration_since(event.timestamp) {
                                if elapsed.as_millis() >= debounce_ms as u128 {
                                    to_process.push(event.clone());
                                    false // Remove from pending
                                } else {
                                    true // Keep in pending
                                }
                            } else {
                                false // Remove invalid timestamps
                            }
                        });
                        
                        // Process the events (simplified for this example)
                        for event in to_process {
                            println!("Processing reload event for plugin: {}", event.plugin_name);
                            // In a real implementation, you'd call the actual reload methods
                        }
                    }
                }
            }
        });
        
        Ok(handle)
    }

    /// Stop hot reload monitoring
    pub fn stop_hot_reload(&mut self) {
        self.file_watcher = None;
        self.reload_sender = None;
    }
}

/// Placeholder plugin implementation for discovered but not loaded plugins
struct PlaceholderPlugin {
    name: String,
}

impl PlaceholderPlugin {
    fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
        }
    }
}

#[async_trait]
impl Plugin for PlaceholderPlugin {
    fn name(&self) -> &str {
        &self.name
    }

    fn version(&self) -> &str {
        "placeholder"
    }

    fn dependencies(&self) -> Vec<String> {
        vec![]
    }

    async fn initialize(&mut self, _config: &PluginConfig) -> std::result::Result<(), PluginError> {
        Err(PluginError::InitializationFailed("Placeholder plugin cannot be initialized".to_string()))
    }

    async fn shutdown(&mut self) -> std::result::Result<(), PluginError> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_plugin_manager_creation() {
        let manager = PluginManager::new();
        assert_eq!(manager.plugin_directories.len(), 1);
        assert_eq!(manager.plugin_directories[0], PathBuf::from("plugins"));
    }

    #[tokio::test]
    async fn test_add_plugin_directory() {
        let mut manager = PluginManager::new();
        manager.add_plugin_directory("/custom/plugins");
        assert_eq!(manager.plugin_directories.len(), 2);
        assert_eq!(manager.plugin_directories[1], PathBuf::from("/custom/plugins"));
    }

    #[tokio::test]
    async fn test_discover_plugins_empty_directory() {
        let temp_dir = TempDir::new().unwrap();
        let mut manager = PluginManager::new();
        manager.plugin_directories = vec![temp_dir.path().to_path_buf()];
        
        let discovered = manager.discover_plugins().await.unwrap();
        assert!(discovered.is_empty());
    }

    #[tokio::test]
    async fn test_register_plugin() {
        let manager = PluginManager::new();
        let metadata = PluginMetadata {
            name: "test_plugin".to_string(),
            version: "1.0.0".to_string(),
            description: Some("Test plugin".to_string()),
            author: Some("Test Author".to_string()),
            dependencies: vec![],
            plugin_type: PluginType::Protocol,
            file_path: PathBuf::from("test_plugin.so"),
        };

        manager.register_plugin(metadata.clone()).await.unwrap();
        
        let plugins = manager.list_plugins().await;
        assert_eq!(plugins.len(), 1);
        assert_eq!(plugins[0].0, "test_plugin");
        assert_eq!(plugins[0].1, PluginStatus::Discovered);
    }

    #[tokio::test]
    async fn test_register_duplicate_plugin() {
        let manager = PluginManager::new();
        let metadata = PluginMetadata {
            name: "test_plugin".to_string(),
            version: "1.0.0".to_string(),
            description: None,
            author: None,
            dependencies: vec![],
            plugin_type: PluginType::Protocol,
            file_path: PathBuf::from("test_plugin.so"),
        };

        manager.register_plugin(metadata.clone()).await.unwrap();
        let result = manager.register_plugin(metadata).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_unload_nonexistent_plugin() {
        let manager = PluginManager::new();
        let result = manager.unload_plugin("nonexistent").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_get_plugin_metadata() {
        let manager = PluginManager::new();
        let metadata = PluginMetadata {
            name: "test_plugin".to_string(),
            version: "1.0.0".to_string(),
            description: Some("Test plugin".to_string()),
            author: Some("Test Author".to_string()),
            dependencies: vec!["dep1".to_string()],
            plugin_type: PluginType::Assertion,
            file_path: PathBuf::from("test_plugin.so"),
        };

        manager.register_plugin(metadata.clone()).await.unwrap();
        
        let retrieved = manager.get_plugin_metadata("test_plugin").await.unwrap();
        assert_eq!(retrieved.name, metadata.name);
        assert_eq!(retrieved.version, metadata.version);
        assert_eq!(retrieved.dependencies, metadata.dependencies);
    }

    #[test]
    fn test_is_plugin_file() {
        let manager = PluginManager::new();
        
        assert!(manager.is_plugin_file(Path::new("plugin.so")));
        assert!(manager.is_plugin_file(Path::new("plugin.dylib")));
        assert!(manager.is_plugin_file(Path::new("plugin.dll")));
        assert!(!manager.is_plugin_file(Path::new("plugin.txt")));
        assert!(!manager.is_plugin_file(Path::new("plugin")));
    }

    #[tokio::test]
    async fn test_dependency_resolution_simple() {
        let manager = PluginManager::new();
        
        // Register plugins with dependencies: A -> B -> C
        let plugin_c = PluginMetadata {
            name: "plugin_c".to_string(),
            version: "1.0.0".to_string(),
            description: None,
            author: None,
            dependencies: vec![],
            plugin_type: PluginType::Protocol,
            file_path: PathBuf::from("plugin_c.so"),
        };
        
        let plugin_b = PluginMetadata {
            name: "plugin_b".to_string(),
            version: "1.0.0".to_string(),
            description: None,
            author: None,
            dependencies: vec!["plugin_c".to_string()],
            plugin_type: PluginType::Protocol,
            file_path: PathBuf::from("plugin_b.so"),
        };
        
        let plugin_a = PluginMetadata {
            name: "plugin_a".to_string(),
            version: "1.0.0".to_string(),
            description: None,
            author: None,
            dependencies: vec!["plugin_b".to_string()],
            plugin_type: PluginType::Protocol,
            file_path: PathBuf::from("plugin_a.so"),
        };
        
        manager.register_plugin(plugin_c).await.unwrap();
        manager.register_plugin(plugin_b).await.unwrap();
        manager.register_plugin(plugin_a).await.unwrap();
        
        let load_order = manager.resolve_load_order().await.unwrap();
        
        // C should be loaded before B, B before A
        let c_pos = load_order.iter().position(|x| x == "plugin_c").unwrap();
        let b_pos = load_order.iter().position(|x| x == "plugin_b").unwrap();
        let a_pos = load_order.iter().position(|x| x == "plugin_a").unwrap();
        
        assert!(c_pos < b_pos);
        assert!(b_pos < a_pos);
    }

    #[tokio::test]
    async fn test_dependency_resolution_circular() {
        let manager = PluginManager::new();
        
        // Register plugins with circular dependencies: A -> B -> A
        let plugin_a = PluginMetadata {
            name: "plugin_a".to_string(),
            version: "1.0.0".to_string(),
            description: None,
            author: None,
            dependencies: vec!["plugin_b".to_string()],
            plugin_type: PluginType::Protocol,
            file_path: PathBuf::from("plugin_a.so"),
        };
        
        let plugin_b = PluginMetadata {
            name: "plugin_b".to_string(),
            version: "1.0.0".to_string(),
            description: None,
            author: None,
            dependencies: vec!["plugin_a".to_string()],
            plugin_type: PluginType::Protocol,
            file_path: PathBuf::from("plugin_b.so"),
        };
        
        manager.register_plugin(plugin_a).await.unwrap();
        manager.register_plugin(plugin_b).await.unwrap();
        
        let result = manager.resolve_load_order().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_dependency_resolution_missing_dependency() {
        let manager = PluginManager::new();
        
        // Register plugin with missing dependency
        let plugin_a = PluginMetadata {
            name: "plugin_a".to_string(),
            version: "1.0.0".to_string(),
            description: None,
            author: None,
            dependencies: vec!["missing_plugin".to_string()],
            plugin_type: PluginType::Protocol,
            file_path: PathBuf::from("plugin_a.so"),
        };
        
        manager.register_plugin(plugin_a).await.unwrap();
        
        let result = manager.resolve_load_order().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_dependency_validation() {
        let manager = PluginManager::new();
        
        let plugin_a = PluginMetadata {
            name: "plugin_a".to_string(),
            version: "1.0.0".to_string(),
            description: None,
            author: None,
            dependencies: vec!["plugin_b".to_string()],
            plugin_type: PluginType::Protocol,
            file_path: PathBuf::from("plugin_a.so"),
        };
        
        let plugin_b = PluginMetadata {
            name: "plugin_b".to_string(),
            version: "1.0.0".to_string(),
            description: None,
            author: None,
            dependencies: vec![],
            plugin_type: PluginType::Protocol,
            file_path: PathBuf::from("plugin_b.so"),
        };
        
        manager.register_plugin(plugin_a).await.unwrap();
        manager.register_plugin(plugin_b).await.unwrap();
        
        // Should validate successfully
        let result = manager.validate_dependencies(&["plugin_a".to_string()]).await;
        assert!(result.is_ok());
        
        // Should fail for missing dependency
        let result = manager.validate_dependencies(&["nonexistent".to_string()]).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_collect_dependencies() {
        let manager = PluginManager::new();
        
        // Create dependency chain: A -> B -> C, A -> D
        let plugin_c = PluginMetadata {
            name: "plugin_c".to_string(),
            version: "1.0.0".to_string(),
            description: None,
            author: None,
            dependencies: vec![],
            plugin_type: PluginType::Protocol,
            file_path: PathBuf::from("plugin_c.so"),
        };
        
        let plugin_d = PluginMetadata {
            name: "plugin_d".to_string(),
            version: "1.0.0".to_string(),
            description: None,
            author: None,
            dependencies: vec![],
            plugin_type: PluginType::Protocol,
            file_path: PathBuf::from("plugin_d.so"),
        };
        
        let plugin_b = PluginMetadata {
            name: "plugin_b".to_string(),
            version: "1.0.0".to_string(),
            description: None,
            author: None,
            dependencies: vec!["plugin_c".to_string()],
            plugin_type: PluginType::Protocol,
            file_path: PathBuf::from("plugin_b.so"),
        };
        
        let plugin_a = PluginMetadata {
            name: "plugin_a".to_string(),
            version: "1.0.0".to_string(),
            description: None,
            author: None,
            dependencies: vec!["plugin_b".to_string(), "plugin_d".to_string()],
            plugin_type: PluginType::Protocol,
            file_path: PathBuf::from("plugin_a.so"),
        };
        
        manager.register_plugin(plugin_c).await.unwrap();
        manager.register_plugin(plugin_d).await.unwrap();
        manager.register_plugin(plugin_b).await.unwrap();
        manager.register_plugin(plugin_a).await.unwrap();
        
        let mut collected = HashSet::new();
        manager.collect_dependencies("plugin_a", &mut collected).await.unwrap();
        
        // Should collect A, B, C, D
        assert_eq!(collected.len(), 4);
        assert!(collected.contains("plugin_a"));
        assert!(collected.contains("plugin_b"));
        assert!(collected.contains("plugin_c"));
        assert!(collected.contains("plugin_d"));
    }

    #[tokio::test]
    async fn test_check_dependencies() {
        let manager = PluginManager::new();
        
        let plugin_a = PluginMetadata {
            name: "plugin_a".to_string(),
            version: "1.0.0".to_string(),
            description: None,
            author: None,
            dependencies: vec!["plugin_b".to_string()],
            plugin_type: PluginType::Protocol,
            file_path: PathBuf::from("plugin_a.so"),
        };
        
        let plugin_b = PluginMetadata {
            name: "plugin_b".to_string(),
            version: "1.0.0".to_string(),
            description: None,
            author: None,
            dependencies: vec![],
            plugin_type: PluginType::Protocol,
            file_path: PathBuf::from("plugin_b.so"),
        };
        
        manager.register_plugin(plugin_a).await.unwrap();
        manager.register_plugin(plugin_b).await.unwrap();
        
        // Dependencies not satisfied initially (plugins are just discovered)
        let satisfied = manager.check_dependencies("plugin_a").await.unwrap();
        assert!(!satisfied);
        
        // After marking plugin_b as loaded, dependencies should be satisfied
        {
            let mut plugins = manager.plugins.write().await;
            if let Some(entry) = plugins.get_mut("plugin_b") {
                entry.status = PluginStatus::Loaded;
            }
        }
        
        let satisfied = manager.check_dependencies("plugin_a").await.unwrap();
        assert!(satisfied);
    }

    #[tokio::test]
    async fn test_hot_reload_configuration() {
        let config = PluginManagerConfig {
            hot_reload_enabled: true,
            reload_debounce_ms: 200,
            ..Default::default()
        };
        
        let manager = PluginManager::with_config(config);
        assert!(manager.config.hot_reload_enabled);
        assert_eq!(manager.config.reload_debounce_ms, 200);
    }

    #[tokio::test]
    async fn test_plugin_reload_event_creation() {
        let event = PluginReloadEvent {
            plugin_name: "test_plugin".to_string(),
            file_path: PathBuf::from("test_plugin.so"),
            event_type: ReloadEventType::Modified,
            timestamp: SystemTime::now(),
        };
        
        assert_eq!(event.plugin_name, "test_plugin");
        assert_eq!(event.event_type, ReloadEventType::Modified);
    }

    #[tokio::test]
    async fn test_extract_plugin_name_from_path() {
        let path = PathBuf::from("plugins/my_plugin.so");
        let name = PluginManager::extract_plugin_name_from_path(&path).unwrap();
        assert_eq!(name, "my_plugin");
        
        let path = PathBuf::from("test.dylib");
        let name = PluginManager::extract_plugin_name_from_path(&path).unwrap();
        assert_eq!(name, "test");
    }

    #[test]
    fn test_is_plugin_file_static() {
        assert!(PluginManager::is_plugin_file_static(Path::new("plugin.so")));
        assert!(PluginManager::is_plugin_file_static(Path::new("plugin.dylib")));
        assert!(PluginManager::is_plugin_file_static(Path::new("plugin.dll")));
        assert!(!PluginManager::is_plugin_file_static(Path::new("plugin.txt")));
    }

    #[tokio::test]
    async fn test_reload_nonexistent_plugin() {
        let manager = PluginManager::new();
        let result = manager.reload_plugin("nonexistent").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_hot_reload_disabled() {
        let config = PluginManagerConfig {
            hot_reload_enabled: false,
            ..Default::default()
        };
        
        let mut manager = PluginManager::with_config(config);
        let result = manager.start_hot_reload().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_reload_event_types() {
        assert_eq!(ReloadEventType::Modified, ReloadEventType::Modified);
        assert_ne!(ReloadEventType::Modified, ReloadEventType::Created);
        assert_ne!(ReloadEventType::Created, ReloadEventType::Deleted);
    }

    #[tokio::test]
    async fn test_plugin_manager_with_hot_reload_config() {
        let config = PluginManagerConfig {
            auto_discover: true,
            auto_load: false,
            isolation_enabled: true,
            crash_recovery: true,
            hot_reload_enabled: true,
            reload_debounce_ms: 1000,
        };
        
        let manager = PluginManager::with_config(config.clone());
        assert_eq!(manager.config.hot_reload_enabled, config.hot_reload_enabled);
        assert_eq!(manager.config.reload_debounce_ms, config.reload_debounce_ms);
        assert!(manager.file_watcher.is_none());
        assert!(manager.reload_sender.is_none());
    }
}