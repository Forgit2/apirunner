use apirunner::{Result, VERSION, NAME, Configuration};
use apirunner::{ConfigPluginConfig, ReportingConfig, ConfigAuthConfig, ConfigDataSourceConfig, ExecutionConfig, ConfigRetryConfig, RateLimitConfig};
use apirunner::cli::{Cli, Commands};
use apirunner::cli_handler::CliHandler;
use apirunner::app::{Application, AppState};
use clap::Parser;
use std::process;
use std::collections::HashMap;
use std::path::PathBuf;
use tokio::signal;

#[tokio::main]
async fn main() -> Result<()> {
    // Parse command line arguments
    let cli = Cli::parse();
    
    // Initialize logging based on verbosity
    init_logging(cli.verbose);
    
    // Print banner for non-quiet operations
    if should_print_banner(&cli) {
        println!("🚀 {} v{}", NAME, VERSION);
    }
    
    // Create default configuration
    let config = create_default_configuration();
    
    // Initialize application
    let mut app = match Application::new(config).await {
        Ok(app) => app,
        Err(e) => {
            eprintln!("❌ Failed to initialize application: {}", e);
            process::exit(1);
        }
    };
    
    // Set up signal handling for graceful shutdown
    let shutdown_handler = setup_signal_handling(app.state.clone());
    
    // Start the application
    if let Err(e) = app.start().await {
        eprintln!("❌ Failed to start application: {}", e);
        let _ = app.shutdown().await;
        process::exit(1);
    }
    
    // Create CLI handler with configuration
    let mut cli_handler = match CliHandler::new(cli.config.clone()).await {
        Ok(handler) => handler,
        Err(e) => {
            eprintln!("❌ Failed to initialize CLI handler: {}", e);
            let _ = app.shutdown().await;
            process::exit(1);
        }
    };
    
    // Handle the command with graceful shutdown support
    let command_result = tokio::select! {
        result = cli_handler.handle_command(cli) => result,
        _ = shutdown_handler => {
            println!("\n🛑 Shutdown signal received during command execution");
            Ok(())
        }
    };
    
    // Shutdown the application
    if let Err(e) = app.shutdown().await {
        eprintln!("⚠️  Warning: Error during application shutdown: {}", e);
    }
    
    // Handle the result
    match command_result {
        Ok(()) => {
            if std::env::var("APIRUNNER_VERBOSE").is_ok() {
                println!("✅ Command completed successfully");
            }
            process::exit(0);
        }
        Err(e) => {
            eprintln!("❌ Command failed: {}", e);
            
            // Print additional context for debugging if verbose
            if std::env::var("APIRUNNER_DEBUG").is_ok() {
                eprintln!("Debug info: {:?}", e);
            }
            
            process::exit(1);
        }
    }
}

/// Initialize logging based on verbosity level
fn init_logging(verbose: bool) {
    let log_level = if verbose {
        "debug".to_string()
    } else {
        std::env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string())
    };
    
    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or(&log_level)
    ).init();
}

/// Determine if we should print the application banner
fn should_print_banner(cli: &Cli) -> bool {
    !matches!(cli.command, Commands::Config(_)) || cli.verbose
}

/// Set up signal handling for graceful shutdown
async fn setup_signal_handling(app_state: AppState) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        #[cfg(unix)]
        {
            let mut sigterm = signal::unix::signal(signal::unix::SignalKind::terminate())
                .expect("Failed to install SIGTERM handler");
            let mut sigint = signal::unix::signal(signal::unix::SignalKind::interrupt())
                .expect("Failed to install SIGINT handler");
            
            tokio::select! {
                _ = sigterm.recv() => {
                    println!("\n🛑 Received SIGTERM, initiating graceful shutdown...");
                }
                _ = sigint.recv() => {
                    println!("\n🛑 Received SIGINT (Ctrl+C), initiating graceful shutdown...");
                }
            }
        }
        
        #[cfg(windows)]
        {
            match signal::ctrl_c().await {
                Ok(()) => {
                    println!("\n🛑 Received Ctrl+C, initiating graceful shutdown...");
                }
                Err(err) => {
                    eprintln!("❌ Failed to listen for Ctrl+C: {}", err);
                    return;
                }
            }
        }
        
        app_state.shutdown();
    })
}

/// Create default configuration
fn create_default_configuration() -> Configuration {
    Configuration {
        environments: HashMap::new(),
        plugins: ConfigPluginConfig {
            plugin_dir: PathBuf::from("plugins"),
            hot_reload: true,
            auto_load: true,
            plugins: HashMap::new(),
        },
        reporting: ReportingConfig {
            output_dir: PathBuf::from("reports"),
            formats: vec![],
            templates: HashMap::new(),
        },
        auth: ConfigAuthConfig {
            default_method: Some("none".to_string()),
            methods: HashMap::new(),
        },
        data_sources: ConfigDataSourceConfig {
            default_type: "json".to_string(),
            sources: HashMap::new(),
        },
        execution: ExecutionConfig {
            default_strategy: "serial".to_string(),
            max_concurrency: 4,
            request_timeout: 30,
            retry: ConfigRetryConfig {
                max_attempts: 3,
                base_delay_ms: 1000,
                max_delay_ms: 10000,
                backoff_multiplier: 2.0,
            },
            rate_limit: RateLimitConfig {
                requests_per_second: 10.0,
                burst_capacity: 20,
            },
        },
        custom: HashMap::new(),
    }
}
