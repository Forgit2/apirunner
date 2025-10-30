use apirunner::execution::*;
use apirunner::event::EventBus;
use apirunner::test_case_manager::{TestCase, RequestDefinition, AssertionDefinition, VariableExtraction, ExtractionSource, ChangeLogEntry, ChangeType};
use chrono::Utc;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

#[tokio::test]
async fn test_serial_executor_basic_execution() {
    let event_bus = Arc::new(EventBus::new(100));
    let executor = SerialExecutor::new(event_bus);
    
    let context = ExecutionContext::new(
        "test-suite-1".to_string(),
        "test".to_string(),
    );
    
    let test_case = TestCase {
        id: "test-1".to_string(),
        name: "Basic GET Test".to_string(),
        description: Some("Test basic GET request".to_string()),
        tags: vec!["basic".to_string(), "get".to_string()],
        protocol: "http".to_string(),
        request: RequestDefinition {
            protocol: "http".to_string(),
            method: "GET".to_string(),
            url: "https://api.example.com/users".to_string(),
            headers: HashMap::from([
                ("Accept".to_string(), "application/json".to_string()),
            ]),
            body: None,
            auth: None,
        },
        assertions: vec![
            AssertionDefinition {
                assertion_type: "status_code".to_string(),
                expected: serde_json::Value::Number(200.into()),
                message: Some("Should return 200 OK".to_string()),
            },
            AssertionDefinition {
                assertion_type: "response_time".to_string(),
                expected: serde_json::Value::Number(1000.into()),
                message: Some("Should respond within 1 second".to_string()),
            },
        ],
        variable_extractions: None,
        dependencies: vec![],
        timeout: Some(Duration::from_secs(30)),
        created_at: Utc::now(),
        updated_at: Utc::now(),
        version: 1,
        change_log: vec![ChangeLogEntry {
            version: 1,
            timestamp: Utc::now(),
            change_type: ChangeType::Created,
            description: "Initial test case creation".to_string(),
            changed_fields: vec![],
        }],
    };
    
    let result = executor.execute(vec![test_case], context).await.unwrap();
    
    assert_eq!(result.test_results.len(), 1);
    assert_eq!(result.success_count, 1);
    assert_eq!(result.failure_count, 0);
    assert!(result.is_successful());
    
    let test_result = &result.test_results[0];
    assert_eq!(test_result.test_case_id, "test-1");
    assert!(test_result.success);
    assert_eq!(test_result.assertion_results.len(), 2);
}

#[tokio::test]
async fn test_serial_executor_variable_extraction_and_propagation() {
    let event_bus = Arc::new(EventBus::new(100));
    let executor = SerialExecutor::new(event_bus);
    
    let context = ExecutionContext::new(
        "test-suite-2".to_string(),
        "test".to_string(),
    );
    
    // First test case that extracts a variable
    let test_case_1 = TestCase {
        id: "test-1".to_string(),
        name: "Extract User ID".to_string(),
        description: Some("Extract user ID from response".to_string()),
        tags: vec!["extraction".to_string()],
        protocol: "http".to_string(),
        request: RequestDefinition {
            protocol: "http".to_string(),
            method: "GET".to_string(),
            url: "https://api.example.com/users".to_string(),
            headers: HashMap::new(),
            body: None,
            auth: None,
        },
        assertions: vec![
            AssertionDefinition {
                assertion_type: "status_code".to_string(),
                expected: serde_json::Value::Number(200.into()),
                message: None,
            },
        ],
        variable_extractions: Some(vec![
            VariableExtraction {
                name: "user_id".to_string(),
                source: ExtractionSource::ResponseBody,
                path: "id".to_string(),
                default_value: Some("default_id".to_string()),
            },
        ]),
        dependencies: vec![],
        timeout: None,
        created_at: Utc::now(),
        updated_at: Utc::now(),
        version: 1,
        change_log: vec![ChangeLogEntry {
            version: 1,
            timestamp: Utc::now(),
            change_type: ChangeType::Created,
            description: "Initial test case creation".to_string(),
            changed_fields: vec![],
        }],
    };
    
    // Second test case that uses the extracted variable
    let test_case_2 = TestCase {
        id: "test-2".to_string(),
        name: "Use Extracted User ID".to_string(),
        description: Some("Use the extracted user ID in URL".to_string()),
        tags: vec!["dependent".to_string()],
        protocol: "http".to_string(),
        request: RequestDefinition {
            protocol: "http".to_string(),
            method: "GET".to_string(),
            url: "https://api.example.com/users/{{user_id}}".to_string(),
            headers: HashMap::new(),
            body: None,
            auth: None,
        },
        assertions: vec![
            AssertionDefinition {
                assertion_type: "status_code".to_string(),
                expected: serde_json::Value::Number(200.into()),
                message: None,
            },
        ],
        variable_extractions: None,
        dependencies: vec!["test-1".to_string()],
        timeout: None,
        created_at: Utc::now(),
        updated_at: Utc::now(),
        version: 1,
        change_log: vec![ChangeLogEntry {
            version: 1,
            timestamp: Utc::now(),
            change_type: ChangeType::Created,
            description: "Initial test case creation".to_string(),
            changed_fields: vec![],
        }],
    };
    
    let result = executor.execute(vec![test_case_1, test_case_2], context).await.unwrap();
    
    assert_eq!(result.test_results.len(), 2);
    assert_eq!(result.success_count, 2);
    assert_eq!(result.failure_count, 0);
    
    // Check that variable was extracted
    let first_result = &result.test_results[0];
    assert!(first_result.extracted_variables.contains_key("user_id"));
    
    // Check that variable was propagated to context
    assert!(result.context.variables.contains_key("user_id"));
}

#[tokio::test]
async fn test_serial_executor_dependency_ordering() {
    let event_bus = Arc::new(EventBus::new(100));
    let executor = SerialExecutor::new(event_bus);
    
    let context = ExecutionContext::new(
        "test-suite-3".to_string(),
        "test".to_string(),
    );
    
    // Create test cases with dependencies in wrong order
    let test_case_c = TestCase {
        id: "test-c".to_string(),
        name: "Test C".to_string(),
        description: None,
        tags: vec!["dependency".to_string()],
        protocol: "http".to_string(),
        request: RequestDefinition {
            protocol: "http".to_string(),
            method: "GET".to_string(),
            url: "https://api.example.com/c".to_string(),
            headers: HashMap::new(),
            body: None,
            auth: None,
        },
        assertions: vec![],
        variable_extractions: None,
        dependencies: vec!["test-a".to_string(), "test-b".to_string()],
        timeout: None,
        created_at: Utc::now(),
        updated_at: Utc::now(),
        version: 1,
        change_log: vec![ChangeLogEntry {
            version: 1,
            timestamp: Utc::now(),
            change_type: ChangeType::Created,
            description: "Initial test case creation".to_string(),
            changed_fields: vec![],
        }],
    };
    
    let test_case_b = TestCase {
        id: "test-b".to_string(),
        name: "Test B".to_string(),
        description: None,
        tags: vec!["dependency".to_string()],
        protocol: "http".to_string(),
        request: RequestDefinition {
            protocol: "http".to_string(),
            method: "GET".to_string(),
            url: "https://api.example.com/b".to_string(),
            headers: HashMap::new(),
            body: None,
            auth: None,
        },
        assertions: vec![],
        variable_extractions: None,
        dependencies: vec!["test-a".to_string()],
        timeout: None,
        created_at: Utc::now(),
        updated_at: Utc::now(),
        version: 1,
        change_log: vec![ChangeLogEntry {
            version: 1,
            timestamp: Utc::now(),
            change_type: ChangeType::Created,
            description: "Initial test case creation".to_string(),
            changed_fields: vec![],
        }],
    };
    
    let test_case_a = TestCase {
        id: "test-a".to_string(),
        name: "Test A".to_string(),
        description: None,
        tags: vec!["dependency".to_string()],
        protocol: "http".to_string(),
        request: RequestDefinition {
            protocol: "http".to_string(),
            method: "GET".to_string(),
            url: "https://api.example.com/a".to_string(),
            headers: HashMap::new(),
            body: None,
            auth: None,
        },
        assertions: vec![],
        variable_extractions: None,
        dependencies: vec![],
        timeout: None,
        created_at: Utc::now(),
        updated_at: Utc::now(),
        version: 1,
        change_log: vec![ChangeLogEntry {
            version: 1,
            timestamp: Utc::now(),
            change_type: ChangeType::Created,
            description: "Initial test case creation".to_string(),
            changed_fields: vec![],
        }],
    };
    
    // Pass in wrong order: C, B, A
    let result = executor.execute(vec![test_case_c, test_case_b, test_case_a], context).await.unwrap();
    
    assert_eq!(result.test_results.len(), 3);
    
    // Check that execution order was corrected: A, B, C
    assert_eq!(result.test_results[0].test_case_id, "test-a");
    assert_eq!(result.test_results[1].test_case_id, "test-b");
    assert_eq!(result.test_results[2].test_case_id, "test-c");
}

#[tokio::test]
async fn test_serial_executor_variable_substitution() {
    let event_bus = Arc::new(EventBus::new(100));
    let executor = SerialExecutor::new(event_bus);
    
    let mut context = ExecutionContext::new(
        "test-suite-4".to_string(),
        "test".to_string(),
    );
    
    // Add some variables to context
    context.set_variable("base_url".to_string(), "https://api.example.com".to_string());
    context.set_variable("api_key".to_string(), "secret123".to_string());
    
    let test_case = TestCase {
        id: "test-1".to_string(),
        name: "Variable Substitution Test".to_string(),
        description: None,
        tags: vec!["substitution".to_string()],
        protocol: "http".to_string(),
        request: RequestDefinition {
            protocol: "http".to_string(),
            method: "GET".to_string(),
            url: "{{base_url}}/users".to_string(),
            headers: HashMap::from([
                ("Authorization".to_string(), "Bearer {{api_key}}".to_string()),
            ]),
            body: Some("{{api_key}}".to_string()),
            auth: None,
        },
        assertions: vec![],
        variable_extractions: None,
        dependencies: vec![],
        timeout: None,
        created_at: Utc::now(),
        updated_at: Utc::now(),
        version: 1,
        change_log: vec![ChangeLogEntry {
            version: 1,
            timestamp: Utc::now(),
            change_type: ChangeType::Created,
            description: "Initial test case creation".to_string(),
            changed_fields: vec![],
        }],
    };
    
    let result = executor.execute(vec![test_case], context).await.unwrap();
    
    assert_eq!(result.test_results.len(), 1);
    
    let test_result = &result.test_results[0];
    assert_eq!(test_result.request.url, "https://api.example.com/users");
    assert_eq!(test_result.request.headers.get("Authorization").unwrap(), "Bearer secret123");
    
    // Check body variable substitution
    if let Some(body) = &test_result.request.body {
        assert_eq!(body, "secret123");
    }
}

#[tokio::test]
async fn test_parallel_executor_basic_execution() {
    let event_bus = Arc::new(EventBus::new(100));
    let executor = ParallelExecutor::new(event_bus, 3, 10.0); // 3 concurrent, 10 req/sec
    
    let context = ExecutionContext::new(
        "test-suite-parallel-1".to_string(),
        "test".to_string(),
    );
    
    // Create multiple independent test cases
    let mut test_cases = Vec::new();
    for i in 1..=5 {
        let test_case = TestCase {
            id: format!("test-{}", i),
            name: format!("Parallel Test {}", i),
            description: Some(format!("Test parallel execution {}", i)),
            tags: vec!["parallel".to_string()],
            protocol: "http".to_string(),
            request: RequestDefinition {
                protocol: "http".to_string(),
                method: "GET".to_string(),
                url: format!("https://api.example.com/test/{}", i),
                headers: HashMap::from([
                    ("Accept".to_string(), "application/json".to_string()),
                ]),
                body: None,
                auth: None,
            },
            assertions: vec![
                AssertionDefinition {
                    assertion_type: "status_code".to_string(),
                    expected: serde_json::Value::Number(200.into()),
                    message: Some("Should return 200 OK".to_string()),
                },
            ],
            variable_extractions: None,
            dependencies: vec![], // No dependencies for parallel execution
            timeout: Some(Duration::from_secs(30)),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            version: 1,
            change_log: vec![ChangeLogEntry {
                version: 1,
                timestamp: Utc::now(),
                change_type: ChangeType::Created,
                description: "Initial test case creation".to_string(),
                changed_fields: vec![],
            }],
        };
        test_cases.push(test_case);
    }
    
    let start_time = std::time::Instant::now();
    let result = executor.execute(test_cases, context).await.unwrap();
    let execution_time = start_time.elapsed();
    
    assert_eq!(result.test_results.len(), 5);
    assert_eq!(result.success_count, 5);
    assert_eq!(result.failure_count, 0);
    assert!(result.is_successful());
    
    // Parallel execution should be faster than serial (each test takes ~50ms)
    // With 3 concurrent workers, 5 tests should complete in roughly 100-150ms
    // Allow more tolerance for CI environments
    assert!(execution_time < Duration::from_millis(500));
}

#[tokio::test]
async fn test_parallel_executor_concurrency_control() {
    let event_bus = Arc::new(EventBus::new(100));
    let executor = ParallelExecutor::new(event_bus, 2, 0.0); // 2 concurrent, no rate limit
    
    let context = ExecutionContext::new(
        "test-suite-parallel-2".to_string(),
        "test".to_string(),
    );
    
    // Create test cases that would take longer if run serially
    let mut test_cases = Vec::new();
    for i in 1..=4 {
        let test_case = TestCase {
            id: format!("test-{}", i),
            name: format!("Concurrent Test {}", i),
            description: None,
            tags: vec!["concurrent".to_string()],
            protocol: "http".to_string(),
            request: RequestDefinition {
                protocol: "http".to_string(),
                method: "GET".to_string(),
                url: format!("https://api.example.com/concurrent/{}", i),
                headers: HashMap::new(),
                body: None,
                auth: None,
            },
            assertions: vec![],
            variable_extractions: None,
            dependencies: vec![],
            timeout: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            version: 1,
            change_log: vec![ChangeLogEntry {
                version: 1,
                timestamp: Utc::now(),
                change_type: ChangeType::Created,
                description: "Initial test case creation".to_string(),
                changed_fields: vec![],
            }],
        };
        test_cases.push(test_case);
    }
    
    let start_time = std::time::Instant::now();
    let result = executor.execute(test_cases, context).await.unwrap();
    let execution_time = start_time.elapsed();
    
    assert_eq!(result.test_results.len(), 4);
    assert_eq!(result.success_count, 4);
    
    // With 2 concurrent workers and 4 tests (each ~50ms), should complete in ~100ms
    assert!(execution_time < Duration::from_millis(150));
}

#[tokio::test]
async fn test_parallel_executor_dependency_levels() {
    let event_bus = Arc::new(EventBus::new(100));
    let executor = ParallelExecutor::new(event_bus, 3, 0.0);
    
    let context = ExecutionContext::new(
        "test-suite-parallel-3".to_string(),
        "test".to_string(),
    );
    
    // Create test cases with dependencies forming levels
    // Level 0: test-a, test-b (no dependencies)
    // Level 1: test-c (depends on test-a), test-d (depends on test-b)
    // Level 2: test-e (depends on test-c and test-d)
    
    let test_case_a = TestCase {
        id: "test-a".to_string(),
        name: "Test A".to_string(),
        description: None,
        tags: vec!["level".to_string()],
        protocol: "http".to_string(),
        request: RequestDefinition {
            protocol: "http".to_string(),
            method: "GET".to_string(),
            url: "https://api.example.com/a".to_string(),
            headers: HashMap::new(),
            body: None,
            auth: None,
        },
        assertions: vec![],
        variable_extractions: None,
        dependencies: vec![],
        timeout: None,
        created_at: Utc::now(),
        updated_at: Utc::now(),
        version: 1,
        change_log: vec![ChangeLogEntry {
            version: 1,
            timestamp: Utc::now(),
            change_type: ChangeType::Created,
            description: "Initial test case creation".to_string(),
            changed_fields: vec![],
        }],
    };
    
    let test_case_b = TestCase {
        id: "test-b".to_string(),
        name: "Test B".to_string(),
        description: None,
        tags: vec!["level".to_string()],
        protocol: "http".to_string(),
        request: RequestDefinition {
            protocol: "http".to_string(),
            method: "GET".to_string(),
            url: "https://api.example.com/b".to_string(),
            headers: HashMap::new(),
            body: None,
            auth: None,
        },
        assertions: vec![],
        variable_extractions: None,
        dependencies: vec![],
        timeout: None,
        created_at: Utc::now(),
        updated_at: Utc::now(),
        version: 1,
        change_log: vec![ChangeLogEntry {
            version: 1,
            timestamp: Utc::now(),
            change_type: ChangeType::Created,
            description: "Initial test case creation".to_string(),
            changed_fields: vec![],
        }],
    };
    
    let test_case_c = TestCase {
        id: "test-c".to_string(),
        name: "Test C".to_string(),
        description: None,
        tags: vec!["level".to_string()],
        protocol: "http".to_string(),
        request: RequestDefinition {
            protocol: "http".to_string(),
            method: "GET".to_string(),
            url: "https://api.example.com/c".to_string(),
            headers: HashMap::new(),
            body: None,
            auth: None,
        },
        assertions: vec![],
        variable_extractions: None,
        dependencies: vec!["test-a".to_string()],
        timeout: None,
        created_at: Utc::now(),
        updated_at: Utc::now(),
        version: 1,
        change_log: vec![ChangeLogEntry {
            version: 1,
            timestamp: Utc::now(),
            change_type: ChangeType::Created,
            description: "Initial test case creation".to_string(),
            changed_fields: vec![],
        }],
    };
    
    let test_case_d = TestCase {
        id: "test-d".to_string(),
        name: "Test D".to_string(),
        description: None,
        tags: vec!["level".to_string()],
        protocol: "http".to_string(),
        request: RequestDefinition {
            protocol: "http".to_string(),
            method: "GET".to_string(),
            url: "https://api.example.com/d".to_string(),
            headers: HashMap::new(),
            body: None,
            auth: None,
        },
        assertions: vec![],
        variable_extractions: None,
        dependencies: vec!["test-b".to_string()],
        timeout: None,
        created_at: Utc::now(),
        updated_at: Utc::now(),
        version: 1,
        change_log: vec![ChangeLogEntry {
            version: 1,
            timestamp: Utc::now(),
            change_type: ChangeType::Created,
            description: "Initial test case creation".to_string(),
            changed_fields: vec![],
        }],
    };
    
    let test_case_e = TestCase {
        id: "test-e".to_string(),
        name: "Test E".to_string(),
        description: None,
        tags: vec!["level".to_string()],
        protocol: "http".to_string(),
        request: RequestDefinition {
            protocol: "http".to_string(),
            method: "GET".to_string(),
            url: "https://api.example.com/e".to_string(),
            headers: HashMap::new(),
            body: None,
            auth: None,
        },
        assertions: vec![],
        variable_extractions: None,
        dependencies: vec!["test-c".to_string(), "test-d".to_string()],
        timeout: None,
        created_at: Utc::now(),
        updated_at: Utc::now(),
        version: 1,
        change_log: vec![ChangeLogEntry {
            version: 1,
            timestamp: Utc::now(),
            change_type: ChangeType::Created,
            description: "Initial test case creation".to_string(),
            changed_fields: vec![],
        }],
    };
    
    let result = executor.execute(
        vec![test_case_e, test_case_d, test_case_c, test_case_b, test_case_a], 
        context
    ).await.unwrap();
    
    assert_eq!(result.test_results.len(), 5);
    assert_eq!(result.success_count, 5);
    
    // Verify that dependencies were respected
    // We can't guarantee exact order within levels due to parallel execution,
    // but we can verify all tests completed successfully
    let test_ids: Vec<String> = result.test_results.iter()
        .map(|r| r.test_case_id.clone())
        .collect();
    
    assert!(test_ids.contains(&"test-a".to_string()));
    assert!(test_ids.contains(&"test-b".to_string()));
    assert!(test_ids.contains(&"test-c".to_string()));
    assert!(test_ids.contains(&"test-d".to_string()));
    assert!(test_ids.contains(&"test-e".to_string()));
}

#[tokio::test]
async fn test_rate_limiter() {
    let rate_limiter = RateLimiter::new(2.0); // 2 requests per second
    
    let start_time = std::time::Instant::now();
    
    // Make 3 requests
    rate_limiter.wait().await; // Should be immediate
    rate_limiter.wait().await; // Should wait ~500ms
    rate_limiter.wait().await; // Should wait another ~500ms
    
    let elapsed = start_time.elapsed();
    
    // Should take at least 1 second for 3 requests at 2 req/sec
    assert!(elapsed >= Duration::from_millis(800)); // Allow some tolerance
    assert!(elapsed < Duration::from_millis(2000)); // Allow more tolerance for CI environments
}

#[tokio::test]
async fn test_parallel_executor_data_isolation() {
    let event_bus = Arc::new(EventBus::new(100));
    let executor = ParallelExecutor::new(event_bus, 3, 0.0);
    
    let mut context = ExecutionContext::new(
        "test-suite-parallel-4".to_string(),
        "test".to_string(),
    );
    
    // Set initial variable
    context.set_variable("shared_var".to_string(), "initial_value".to_string());
    
    // Create test cases that would extract different values
    let mut test_cases = Vec::new();
    for i in 1..=3 {
        let test_case = TestCase {
            id: format!("test-{}", i),
            name: format!("Isolation Test {}", i),
            description: None,
            tags: vec!["isolation".to_string()],
            protocol: "http".to_string(),
            request: RequestDefinition {
                protocol: "http".to_string(),
                method: "GET".to_string(),
                url: format!("https://api.example.com/isolation/{}", i),
                headers: HashMap::new(),
                body: None,
                auth: None,
            },
            assertions: vec![],
            variable_extractions: Some(vec![
                VariableExtraction {
                    name: "extracted_id".to_string(),
                    source: ExtractionSource::ResponseBody,
                    path: "id".to_string(),
                    default_value: Some(format!("default_{}", i)),
                },
            ]),
            dependencies: vec![],
            timeout: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            version: 1,
            change_log: vec![ChangeLogEntry {
                version: 1,
                timestamp: Utc::now(),
                change_type: ChangeType::Created,
                description: "Initial test case creation".to_string(),
                changed_fields: vec![],
            }],
        };
        test_cases.push(test_case);
    }
    
    let result = executor.execute(test_cases, context).await.unwrap();
    
    assert_eq!(result.test_results.len(), 3);
    assert_eq!(result.success_count, 3);
    
    // In parallel execution, variable extractions should be isolated
    // Each test should extract its own variables without affecting others
    for test_result in &result.test_results {
        assert!(test_result.extracted_variables.contains_key("extracted_id"));
    }
    
    // The original context should not be modified by parallel extractions
    assert_eq!(result.context.get_variable("shared_var").unwrap(), "initial_value");
}