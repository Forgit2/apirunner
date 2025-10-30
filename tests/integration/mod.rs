pub mod end_to_end;
pub mod performance_benchmarks;
pub mod distributed_execution;
pub mod real_http_services;

use std::sync::Once;
use tempfile::TempDir;
use tokio::sync::OnceCell;

static INIT: Once = Once::new();
static TEST_SERVER: OnceCell<TestHttpServer> = OnceCell::const_new();

/// Initialize test environment once for all integration tests
pub fn init_test_environment() {
    INIT.call_once(|| {
        env_logger::init();
    });
}

/// Test HTTP server for integration testing
pub struct TestHttpServer {
    pub base_url: String,
    pub server_handle: tokio::task::JoinHandle<()>,
}

impl TestHttpServer {
    pub async fn start() -> Self {
        use warp::Filter;
        
        let routes = warp::path("api")
            .and(warp::path("v1"))
            .and(
                warp::path("users")
                    .and(warp::get())
                    .map(|| warp::reply::json(&serde_json::json!({
                        "users": [
                            {"id": 1, "name": "John Doe", "email": "john@example.com"},
                            {"id": 2, "name": "Jane Smith", "email": "jane@example.com"}
                        ]
                    })))
                    .or(
                        warp::path("users")
                            .and(warp::post())
                            .and(warp::body::json())
                            .map(|user: serde_json::Value| {
                                warp::reply::with_status(
                                    warp::reply::json(&serde_json::json!({
                                        "id": 3,
                                        "name": user["name"],
                                        "email": user["email"],
                                        "created": true
                                    })),
                                    warp::http::StatusCode::CREATED
                                )
                            })
                    )
                    .or(
                        warp::path!("users" / u32)
                            .and(warp::get())
                            .map(|id| {
                                if id <= 2 {
                                    warp::reply::with_status(
                                        warp::reply::json(&serde_json::json!({
                                            "id": id,
                                            "name": format!("User {}", id),
                                            "email": format!("user{}@example.com", id)
                                        })),
                                        warp::http::StatusCode::OK
                                    )
                                } else {
                                    warp::reply::with_status(
                                        warp::reply::json(&serde_json::json!({
                                            "error": "User not found"
                                        })),
                                        warp::http::StatusCode::NOT_FOUND
                                    )
                                }
                            })
                    )
                    .or(
                        warp::path("slow")
                            .and(warp::get())
                            .and(warp::query::<std::collections::HashMap<String, String>>())
                            .and_then(|params: std::collections::HashMap<String, String>| async move {
                                let delay = params.get("delay")
                                    .and_then(|d| d.parse::<u64>().ok())
                                    .unwrap_or(1000);
                                
                                tokio::time::sleep(std::time::Duration::from_millis(delay)).await;
                                
                                Ok::<_, warp::Rejection>(warp::reply::json(&serde_json::json!({
                                    "message": "Slow response",
                                    "delay_ms": delay
                                })))
                            })
                    )
                    .or(
                        warp::path("error")
                            .and(warp::get())
                            .and(warp::query::<std::collections::HashMap<String, String>>())
                            .map(|params: std::collections::HashMap<String, String>| {
                                let status = params.get("status")
                                    .and_then(|s| s.parse::<u16>().ok())
                                    .unwrap_or(500);
                                
                                let status_code = warp::http::StatusCode::from_u16(status)
                                    .unwrap_or(warp::http::StatusCode::INTERNAL_SERVER_ERROR);
                                
                                warp::reply::with_status(
                                    warp::reply::json(&serde_json::json!({
                                        "error": format!("Simulated error with status {}", status)
                                    })),
                                    status_code
                                )
                            })
                    )
                    .or(
                        warp::path("auth")
                            .and(warp::get())
                            .and(warp::header::<String>("authorization"))
                            .map(|auth_header: String| {
                                if auth_header.starts_with("Bearer ") {
                                    let token = &auth_header[7..];
                                    if token == "valid_token" {
                                        warp::reply::with_status(
                                            warp::reply::json(&serde_json::json!({
                                                "authenticated": true,
                                                "user": "test_user"
                                            })),
                                            warp::http::StatusCode::OK
                                        )
                                    } else {
                                        warp::reply::with_status(
                                            warp::reply::json(&serde_json::json!({
                                                "error": "Invalid token"
                                            })),
                                            warp::http::StatusCode::UNAUTHORIZED
                                        )
                                    }
                                } else {
                                    warp::reply::with_status(
                                        warp::reply::json(&serde_json::json!({
                                            "error": "Missing or invalid authorization header"
                                        })),
                                        warp::http::StatusCode::UNAUTHORIZED
                                    )
                                }
                            })
                    )
            );
        
        let (addr, server) = warp::serve(routes)
            .bind_with_graceful_shutdown(([127, 0, 0, 1], 0), async {
                tokio::signal::ctrl_c().await.ok();
            });
        
        let base_url = format!("http://127.0.0.1:{}", addr.port());
        let server_handle = tokio::spawn(server);
        
        // Wait a bit for server to start
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        
        Self {
            base_url,
            server_handle,
        }
    }
    
    pub async fn stop(self) {
        self.server_handle.abort();
    }
}

/// Get or start the test HTTP server
pub async fn get_test_server() -> &'static TestHttpServer {
    TEST_SERVER.get_or_init(|| async {
        TestHttpServer::start().await
    }).await
}

/// Create a temporary directory for test data
pub fn create_test_temp_dir() -> TempDir {
    TempDir::new().expect("Failed to create temporary directory")
}

/// Test utilities for integration tests
pub struct IntegrationTestUtils;

impl IntegrationTestUtils {
    /// Create a test configuration for integration tests
    pub fn create_test_config(temp_dir: &std::path::Path) -> apirunner::configuration::Configuration {
        use std::collections::HashMap;
        use apirunner::configuration::*;
        
        apirunner::configuration::Configuration {
            execution: ExecutionConfig {
                default_strategy: "serial".to_string(),
                max_concurrency: 4,
                request_timeout: 30,
                retry: RetryConfig {
                    max_attempts: 3,
                    base_delay_ms: 1000,
                    max_delay_ms: 10000,
                    backoff_multiplier: 2.0,
                },
                rate_limit: RateLimitConfig {
                    requests_per_second: 100.0,
                    burst_capacity: 10,
                },
            },
            plugins: ConfigPluginConfig {
                plugin_dir: "plugins".into(),
                auto_load: true,
                hot_reload: false,
                plugins: HashMap::new(),
            },
            reporting: ReportingConfig {
                formats: vec!["json".to_string(), "html".to_string()],
                output_dir: temp_dir.join("reports"),
                templates: HashMap::new(),
            },
            auth: ConfigAuthConfig {
                default_method: Some("none".to_string()),
                methods: HashMap::new(),
            },
            data_sources: ConfigDataSourceConfig {
                default_type: "file".to_string(),
                sources: HashMap::new(),
            },
            environments: HashMap::from([
                ("integration_test".to_string(), EnvironmentConfig {
                    name: "integration_test".to_string(),
                    base_urls: HashMap::from([
                        ("api".to_string(), "http://localhost:8080".to_string()),
                    ]),
                    variables: HashMap::from([
                        ("test_data_dir".to_string(), temp_dir.to_string_lossy().to_string()),
                    ]),
                    auth_overrides: HashMap::new(),
                }),
            ]),
            custom: HashMap::from([
                ("test_data_dir".to_string(), serde_json::Value::String(temp_dir.to_string_lossy().to_string())),
            ]),
        }
    }
    
    /// Create sample test cases for integration testing
    pub fn create_sample_test_cases() -> Vec<apirunner::test_case_manager::TestCase> {
        use std::collections::HashMap;
        use apirunner::test_case_manager::*;
        use chrono::Utc;
        use std::time::Duration;
        
        vec![
            TestCase {
                id: "integration_test_1".to_string(),
                name: "Get Users List".to_string(),
                description: Some("Test getting list of users".to_string()),
                tags: vec!["integration".to_string(), "users".to_string()],
                protocol: "http".to_string(),
                request: RequestDefinition {
                    protocol: "http".to_string(),
                    method: "GET".to_string(),
                    url: "{{base_url}}/api/v1/users".to_string(),
                    headers: HashMap::from([
                        ("Accept".to_string(), "application/json".to_string()),
                    ]),
                    body: None,
                    auth: None,
                },
                assertions: vec![
                    AssertionDefinition {
                        assertion_type: "status_code".to_string(),
                        expected: serde_json::json!(200),
                        message: Some("Should return 200 OK".to_string()),
                    },
                    AssertionDefinition {
                        assertion_type: "json_path".to_string(),
                        expected: serde_json::json!("$.users[0].name"),
                        message: Some("Should have users with names".to_string()),
                    },
                ],
                variable_extractions: Some(vec![
                    VariableExtraction {
                        name: "first_user_id".to_string(),
                        source: ExtractionSource::ResponseBody,
                        path: "$.users[0].id".to_string(),
                        default_value: None,
                    },
                ]),
                dependencies: vec![],
                timeout: Some(Duration::from_secs(30)),
                created_at: Utc::now(),
                updated_at: Utc::now(),
                version: 1,
                change_log: vec![
                    ChangeLogEntry {
                        version: 1,
                        timestamp: Utc::now(),
                        change_type: ChangeType::Created,
                        description: "Initial test case creation".to_string(),
                        changed_fields: vec!["all".to_string()],
                    },
                ],
            },
            TestCase {
                id: "integration_test_2".to_string(),
                name: "Create New User".to_string(),
                description: Some("Test creating a new user".to_string()),
                tags: vec!["integration".to_string(), "users".to_string(), "create".to_string()],
                protocol: "http".to_string(),
                request: RequestDefinition {
                    protocol: "http".to_string(),
                    method: "POST".to_string(),
                    url: "{{base_url}}/api/v1/users".to_string(),
                    headers: HashMap::from([
                        ("Content-Type".to_string(), "application/json".to_string()),
                        ("Accept".to_string(), "application/json".to_string()),
                    ]),
                    body: Some(serde_json::json!({
                        "name": "New User",
                        "email": "newuser@example.com"
                    }).to_string()),
                    auth: None,
                },
                assertions: vec![
                    AssertionDefinition {
                        assertion_type: "status_code".to_string(),
                        expected: serde_json::json!(201),
                        message: Some("Should return 201 Created".to_string()),
                    },
                    AssertionDefinition {
                        assertion_type: "json_path".to_string(),
                        expected: serde_json::json!("New User"),
                        message: Some("Should return created user name".to_string()),
                    },
                ],
                variable_extractions: Some(vec![
                    VariableExtraction {
                        name: "created_user_id".to_string(),
                        source: ExtractionSource::ResponseBody,
                        path: "$.id".to_string(),
                        default_value: None,
                    },
                ]),
                dependencies: vec![],
                timeout: Some(Duration::from_secs(30)),
                created_at: Utc::now(),
                updated_at: Utc::now(),
                version: 1,
                change_log: vec![
                    ChangeLogEntry {
                        version: 1,
                        timestamp: Utc::now(),
                        change_type: ChangeType::Created,
                        description: "Initial test case creation".to_string(),
                        changed_fields: vec!["all".to_string()],
                    },
                ],
            },
            TestCase {
                id: "integration_test_3".to_string(),
                name: "Get User by ID".to_string(),
                description: Some("Test getting a specific user by ID".to_string()),
                tags: vec!["integration".to_string(), "users".to_string(), "get".to_string()],
                protocol: "http".to_string(),
                request: RequestDefinition {
                    protocol: "http".to_string(),
                    method: "GET".to_string(),
                    url: "{{base_url}}/api/v1/users/{{first_user_id}}".to_string(),
                    headers: HashMap::from([
                        ("Accept".to_string(), "application/json".to_string()),
                    ]),
                    body: None,
                    auth: None,
                },
                assertions: vec![
                    AssertionDefinition {
                        assertion_type: "status_code".to_string(),
                        expected: serde_json::json!(200),
                        message: Some("Should return 200 OK".to_string()),
                    },
                    AssertionDefinition {
                        assertion_type: "json_path".to_string(),
                        expected: serde_json::json!("{{first_user_id}}"),
                        message: Some("Should return correct user ID".to_string()),
                    },
                ],
                variable_extractions: None,
                dependencies: vec!["integration_test_1".to_string()],
                timeout: Some(Duration::from_secs(30)),
                created_at: Utc::now(),
                updated_at: Utc::now(),
                version: 1,
                change_log: vec![
                    ChangeLogEntry {
                        version: 1,
                        timestamp: Utc::now(),
                        change_type: ChangeType::Created,
                        description: "Initial test case creation".to_string(),
                        changed_fields: vec!["all".to_string()],
                    },
                ],
            },
        ]
    }
    
    /// Wait for a condition to be true with timeout
    pub async fn wait_for_condition<F, Fut>(
        condition: F,
        timeout: std::time::Duration,
        check_interval: std::time::Duration,
    ) -> bool
    where
        F: Fn() -> Fut,
        Fut: std::future::Future<Output = bool>,
    {
        let start = std::time::Instant::now();
        
        while start.elapsed() < timeout {
            if condition().await {
                return true;
            }
            tokio::time::sleep(check_interval).await;
        }
        
        false
    }
}