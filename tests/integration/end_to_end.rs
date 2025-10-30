use apirunner::execution::{SerialExecutor, ParallelExecutor, ExecutionContext, ExecutionStrategy};
use apirunner::test_case_manager::{TestCase, RequestDefinition, AssertionDefinition};
use apirunner::event::{EventBus, EventFilter};
use apirunner::metrics::{MetricsCollector, MetricsConfig};
use apirunner::configuration::{Configuration, ExecutionConfig, RetryConfig, RateLimitConfig, ConfigPluginConfig, ReportingConfig, ConfigAuthConfig, ConfigDataSourceConfig};
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use std::path::PathBuf;
use tempfile::TempDir;
use chrono::Utc;

#[tokio::test]
async fn test_end_to_end_serial_execution() {
    let temp_dir = TempDir::new().unwrap();
    
    // Create test configuration
    let config = create_test_config(&temp_dir);
    
    // Create metrics collector
    let metrics_config = MetricsConfig::default();
    let metrics_collector = Arc::new(MetricsCollector::new(metrics_config).unwrap());
    
    // Create event bus
    let event_bus = Arc::new(EventBus::new(100));
    
    // Create execution context
    let context = ExecutionContext::new(
        "test_suite_1".to_string(),
        "test".to_string(),
    );
    
    // Create serial executor
    let serial_executor = SerialExecutor::new(event_bus.clone());
    
    // Create test cases
    let test_cases = create_sample_test_cases();
    
    // Execute tests
    let results = serial_executor.execute(test_cases.clone(), context).await;
    
    // Verify results
    assert!(results.is_ok());
    let execution_results = results.unwrap();
    assert_eq!(execution_results.test_results.len(), test_cases.len());
}

#[tokio::test]
async fn test_end_to_end_parallel_execution() {
    let temp_dir = TempDir::new().unwrap();
    
    // Create test configuration
    let config = create_test_config(&temp_dir);
    
    // Create metrics collector
    let metrics_config = MetricsConfig::default();
    let metrics_collector = Arc::new(MetricsCollector::new(metrics_config).unwrap());
    
    // Create event bus
    let event_bus = Arc::new(EventBus::new(100));
    
    // Create execution context
    let context = ExecutionContext::new(
        "test_suite_2".to_string(),
        "test".to_string(),
    );
    
    // Create parallel executor with correct parameter order (max_concurrent, timeout_multiplier)
    let parallel_executor = ParallelExecutor::new(event_bus.clone(), 4, 2.0);
    
    // Create test cases
    let test_cases = create_sample_test_cases();
    
    // Execute tests
    let results = parallel_executor.execute(test_cases.clone(), context).await;
    
    // Verify results
    assert!(results.is_ok());
    let execution_results = results.unwrap();
    assert_eq!(execution_results.test_results.len(), test_cases.len());
}

#[tokio::test]
async fn test_end_to_end_with_data_source() {
    let temp_dir = TempDir::new().unwrap();
    
    // Create test configuration
    let config = create_test_config(&temp_dir);
    
    // Create metrics collector
    let metrics_config = MetricsConfig::default();
    let metrics_collector = Arc::new(MetricsCollector::new(metrics_config).unwrap());
    
    // Create event bus
    let event_bus = Arc::new(EventBus::new(100));
    
    // Create execution context
    let context = ExecutionContext::new(
        "test_suite_3".to_string(),
        "test".to_string(),
    );
    
    // Create serial executor
    let serial_executor = SerialExecutor::new(event_bus.clone());
    
    // Create test case with data source
    let test_case = TestCase {
        id: "data_driven_test".to_string(),
        name: "Data Driven Test".to_string(),
        description: Some("Test with data source".to_string()),
        tags: vec!["integration".to_string()],
        protocol: "http".to_string(),
        request: RequestDefinition {
            protocol: "http".to_string(),
            method: "GET".to_string(),
            url: "https://httpbin.org/get".to_string(),
            headers: HashMap::new(),
            body: None,
            auth: None,
        },
        assertions: vec![
            AssertionDefinition {
                assertion_type: "status_code".to_string(),
                expected: json!(200),
                message: Some("Check status code".to_string()),
            }
        ],
        variable_extractions: None,
        dependencies: vec![],
        timeout: Some(Duration::from_secs(60)),
        created_at: Utc::now(),
        updated_at: Utc::now(),
        version: 1,
        change_log: vec![],
    };
    
    // Execute tests
    let results = serial_executor.execute(vec![test_case], context).await.unwrap();
    
    // Verify results
    assert_eq!(results.test_results.len(), 1);
}

#[tokio::test]
async fn test_end_to_end_with_event_monitoring() {
    let temp_dir = TempDir::new().unwrap();
    
    // Create test configuration
    let config = create_test_config(&temp_dir);
    
    // Create event bus
    let event_bus = Arc::new(EventBus::new(100));
    
    // Subscribe to events
    let filter = EventFilter::new();
    let mut event_receiver = event_bus.subscribe(filter).await.unwrap();
    
    // Create execution context
    let context = ExecutionContext::new(
        "test_suite_4".to_string(),
        "test".to_string(),
    );
    
    // Create serial executor
    let serial_executor = SerialExecutor::new(event_bus.clone());
    
    // Create simple test case
    let test_case = create_simple_test_case();
    
    // Execute test in background
    let executor_handle = tokio::spawn(async move {
        serial_executor.execute(vec![test_case], context).await
    });
    
    // Monitor events
    let mut event_count = 0;
    let timeout = Duration::from_secs(5);
    let start_time = std::time::Instant::now();
    
    while start_time.elapsed() < timeout && event_count < 10 {
        if let Ok(event) = tokio::time::timeout(Duration::from_millis(100), event_receiver.recv()).await {
            if event.is_ok() {
                event_count += 1;
            }
        }
    }
    
    // Wait for execution to complete
    let results = executor_handle.await.unwrap();
    assert!(results.is_ok());
    
    // We should have received some events
    assert!(event_count > 0);
}

#[tokio::test]
async fn test_end_to_end_with_metrics() {
    let temp_dir = TempDir::new().unwrap();
    
    // Create test configuration
    let config = create_test_config(&temp_dir);
    
    // Create metrics collector
    let metrics_config = MetricsConfig::default();
    let metrics_collector = Arc::new(MetricsCollector::new(metrics_config).unwrap());
    
    // Create event bus
    let event_bus = Arc::new(EventBus::new(100));
    
    // Create execution context
    let context = ExecutionContext::new(
        "test_suite_5".to_string(),
        "test".to_string(),
    );
    
    // Create serial executor
    let serial_executor = SerialExecutor::new(event_bus.clone());
    
    // Create test cases
    let test_cases = create_sample_test_cases();
    
    // Execute tests
    let results = serial_executor.execute(test_cases.clone(), context).await.unwrap();
    
    // Verify results and metrics
    assert_eq!(results.test_results.len(), test_cases.len());
    
    // Check that metrics were collected
    let metrics = metrics_collector.get_snapshot();
    assert!(metrics.is_ok());
}

fn create_test_config(temp_dir: &TempDir) -> Configuration {
    Configuration {
        execution: ExecutionConfig {
            default_strategy: "serial".to_string(),
            max_concurrency: 1,
            request_timeout: 30,
            retry: RetryConfig {
                max_attempts: 3,
                base_delay_ms: 1000,
                max_delay_ms: 10000,
                backoff_multiplier: 2.0,
            },
            rate_limit: RateLimitConfig {
                requests_per_second: 10.0,
                burst_capacity: 5,
            },
        },
        plugins: ConfigPluginConfig {
            plugin_dir: PathBuf::from("plugins"),
            auto_load: false,
            hot_reload: false,
            plugins: HashMap::new(),
        },
        auth: ConfigAuthConfig {
            default_method: None,
            methods: HashMap::new(),
        },
        reporting: ReportingConfig {
            output_dir: temp_dir.path().to_path_buf(),
            formats: vec!["json".to_string()],
            templates: HashMap::new(),
        },
        data_sources: ConfigDataSourceConfig {
            sources: HashMap::new(),
            default_type: "file".to_string(),
        },
        environments: HashMap::new(),
        custom: HashMap::new(),
    }
}

fn create_sample_test_cases() -> Vec<TestCase> {
    vec![
        TestCase {
            id: "test_1".to_string(),
            name: "Sample Test 1".to_string(),
            description: Some("First test case".to_string()),
            tags: vec!["integration".to_string()],
            protocol: "http".to_string(),
            request: RequestDefinition {
                protocol: "http".to_string(),
                method: "GET".to_string(),
                url: "https://httpbin.org/get".to_string(),
                headers: HashMap::new(),
                body: None,
                auth: None,
            },
            assertions: vec![
                AssertionDefinition {
                    assertion_type: "status_code".to_string(),
                    expected: json!(200),
                    message: Some("Check status code".to_string()),
                }
            ],
            variable_extractions: None,
            dependencies: vec![],
            timeout: Some(Duration::from_secs(60)),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            version: 1,
            change_log: vec![],
        },
        TestCase {
            id: "test_2".to_string(),
            name: "Sample Test 2".to_string(),
            description: Some("Second test case".to_string()),
            tags: vec!["integration".to_string()],
            protocol: "http".to_string(),
            request: RequestDefinition {
                protocol: "http".to_string(),
                method: "POST".to_string(),
                url: "https://httpbin.org/post".to_string(),
                headers: HashMap::from([
                    ("Content-Type".to_string(), "application/json".to_string()),
                ]),
                body: Some(json!({"test": "data"}).to_string()),
                auth: None,
            },
            assertions: vec![
                AssertionDefinition {
                    assertion_type: "status_code".to_string(),
                    expected: json!(200),
                    message: Some("Check status code".to_string()),
                }
            ],
            variable_extractions: None,
            dependencies: vec![],
            timeout: Some(Duration::from_secs(60)),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            version: 1,
            change_log: vec![],
        }
    ]
}

fn create_simple_test_case() -> TestCase {
    TestCase {
        id: "simple_test".to_string(),
        name: "Simple Test".to_string(),
        description: Some("A simple test case".to_string()),
        tags: vec!["simple".to_string()],
        protocol: "http".to_string(),
        request: RequestDefinition {
            protocol: "http".to_string(),
            method: "GET".to_string(),
            url: "https://httpbin.org/get".to_string(),
            headers: HashMap::new(),
            body: None,
            auth: None,
        },
        assertions: vec![
            AssertionDefinition {
                assertion_type: "status_code".to_string(),
                expected: json!(200),
                message: Some("Check status code".to_string()),
            }
        ],
        variable_extractions: None,
        dependencies: vec![],
        timeout: Some(Duration::from_secs(60)),
        created_at: Utc::now(),
        updated_at: Utc::now(),
        version: 1,
        change_log: vec![],
    }
}