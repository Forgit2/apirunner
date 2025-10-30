//! Example demonstrating dynamic configuration reload functionality
//! 
//! This example shows how to:
//! 1. Set up configuration management with hot reload
//! 2. Subscribe to configuration change events
//! 3. Handle configuration reload failures with automatic rollback
//! 4. Monitor configuration changes in real-time

use apirunner::configuration::{ConfigurationManager, ConfigurationEvent, ChangeType};
use std::path::PathBuf;
use std::time::Duration;
use tokio::fs;
use tokio::time::sleep;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔧 Dynamic Configuration Reload Example");
    println!("========================================");

    // Create a temporary configuration file
    let config_path = PathBuf::from("temp_config.yaml");
    let initial_config = r#"
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

    fs::write(&config_path, initial_config).await?;
    println!("📝 Created initial configuration file");

    // Create configuration manager
    let mut manager = ConfigurationManager::new()?;
    manager.add_config_path(&config_path, 1)?;
    manager.load_configuration().await?;

    println!("⚙️  Initial configuration loaded:");
    let config = manager.get_configuration();
    println!("   Strategy: {}", config.execution.default_strategy);
    println!("   Concurrency: {}", config.execution.max_concurrency);
    println!("   Timeout: {}s", config.execution.request_timeout);

    // Subscribe to configuration change events
    let mut event_receiver = manager.subscribe_to_changes();

    // Enable hot reload
    manager.enable_hot_reload()?;
    println!("🔥 Hot reload enabled");

    // Spawn a task to monitor configuration events
    let event_monitor = tokio::spawn(async move {
        while let Ok(event) = event_receiver.recv().await {
            match event {
                ConfigurationEvent::FileModified { path, timestamp } => {
                    println!("📁 File modified: {:?} at {:?}", path, timestamp);
                }
                ConfigurationEvent::ValidationStarted { path, timestamp } => {
                    println!("🔍 Validation started for: {:?} at {:?}", path, timestamp);
                }
                ConfigurationEvent::ValidationCompleted { path, timestamp } => {
                    println!("✅ Validation completed for: {:?} at {:?}", path, timestamp);
                }
                ConfigurationEvent::Reloaded { path, timestamp, changes } => {
                    println!("🔄 Configuration reloaded: {:?} at {:?}", path, timestamp);
                    if !changes.is_empty() {
                        println!("   Changes detected:");
                        for change in &changes {
                            let symbol = match change.change_type {
                                ChangeType::Added => "+",
                                ChangeType::Modified => "~",
                                ChangeType::Removed => "-",
                            };
                            println!("   {} {}: {}", symbol, change.path, change.new_value);
                        }
                    }
                }
                ConfigurationEvent::RolledBack { path, error, timestamp } => {
                    println!("⚠️  Configuration rolled back: {:?} at {:?}", path, timestamp);
                    println!("   Error: {}", error);
                }
                ConfigurationEvent::ReloadFailed { path, error, timestamp } => {
                    println!("❌ Configuration reload failed: {:?} at {:?}", path, timestamp);
                    println!("   Error: {}", error);
                }
            }
        }
    });

    // Simulate configuration changes
    println!("\n🔄 Simulating configuration changes...");
    
    // Wait a bit for the file watcher to be ready
    sleep(Duration::from_millis(500)).await;

    // Change 1: Update execution strategy and concurrency
    println!("\n1️⃣  Updating execution strategy to parallel and increasing concurrency...");
    let updated_config = r#"
execution:
  default_strategy: "parallel"
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
  auto_load: true
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

    fs::write(&config_path, updated_config).await?;
    sleep(Duration::from_secs(1)).await;

    // Check the updated configuration
    let config = manager.get_configuration();
    println!("   New Strategy: {}", config.execution.default_strategy);
    println!("   New Concurrency: {}", config.execution.max_concurrency);

    // Change 2: Add a new custom setting
    println!("\n2️⃣  Adding custom settings...");
    let config_with_custom = r#"
execution:
  default_strategy: "parallel"
  max_concurrency: 20
  request_timeout: 60
  retry:
    max_attempts: 5
    base_delay_ms: 500
    max_delay_ms: 30000
    backoff_multiplier: 2.0
  rate_limit:
    requests_per_second: 15.0
    burst_capacity: 30
plugins:
  plugin_dir: "plugins"
  auto_load: true
  hot_reload: true
  plugins: {}
reporting:
  output_dir: "reports"
  formats: ["junit", "html", "json"]
  templates: {}
auth:
  default_method: null
  methods: {}
data_sources:
  default_type: "file"
  sources: {}
environments: {}
custom:
  api_version: "v2"
  debug_mode: true
"#;

    fs::write(&config_path, config_with_custom).await?;
    sleep(Duration::from_secs(1)).await;

    // Change 3: Invalid configuration (should trigger rollback)
    println!("\n3️⃣  Testing invalid configuration (should rollback)...");
    let invalid_config = r#"
execution:
  default_strategy: "invalid_strategy"
  max_concurrency: 0
  request_timeout: 0
  retry:
    max_attempts: 0
    base_delay_ms: 0
    max_delay_ms: 30000
    backoff_multiplier: 2.0
  rate_limit:
    requests_per_second: -1.0
    burst_capacity: 30
plugins:
  plugin_dir: "plugins"
  auto_load: true
  hot_reload: true
  plugins: {}
reporting:
  output_dir: "reports"
  formats: ["junit", "html", "json"]
  templates: {}
auth:
  default_method: null
  methods: {}
data_sources:
  default_type: "file"
  sources: {}
environments: {}
custom:
  api_version: "v2"
  debug_mode: true
"#;

    fs::write(&config_path, invalid_config).await?;
    sleep(Duration::from_secs(1)).await;

    // Verify configuration was rolled back
    let config = manager.get_configuration();
    println!("   After rollback - Strategy: {}", config.execution.default_strategy);
    println!("   After rollback - Concurrency: {}", config.execution.max_concurrency);

    // Manual reload test
    println!("\n4️⃣  Testing manual configuration reload...");
    
    // First, write a valid configuration
    fs::write(&config_path, updated_config).await?;
    
    // Force a manual reload
    manager.force_reload().await?;
    println!("   Manual reload completed successfully");

    // Show configuration history
    println!("\n📚 Configuration History:");
    let history = manager.get_configuration_history();
    for (i, snapshot) in history.iter().enumerate() {
        println!("   {}. {:?} - {} (trigger: {:?})", 
            i + 1, 
            snapshot.timestamp, 
            snapshot.reason, 
            snapshot.trigger_path
        );
    }

    // Cleanup
    sleep(Duration::from_secs(1)).await;
    event_monitor.abort();
    
    if config_path.exists() {
        fs::remove_file(&config_path).await?;
        println!("\n🧹 Cleaned up temporary configuration file");
    }

    println!("\n✅ Dynamic configuration reload example completed!");
    println!("   - Hot reload functionality demonstrated");
    println!("   - Configuration change detection working");
    println!("   - Automatic rollback on invalid configuration");
    println!("   - Event notifications functioning properly");

    Ok(())
}