use std::collections::HashMap;
use std::time::Duration;
use std::sync::Arc;

use apirunner::error::Result;
use apirunner::event::EventBus;
use apirunner::execution::{ExecutionContext, ExecutionStrategy};
use apirunner::test_case_manager::{TestCase, RequestDefinition, AssertionDefinition, VariableExtraction, AuthDefinition, ExtractionSource, ChangeLogEntry, ChangeType};
use apirunner::interactive_execution::{
    InteractiveExecutor, DebugOptions, ValidationErrorType, StepType
};

#[tokio::test]
async fn test_interactive_executor_creation() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(100));
    let executor = InteractiveExecutor::new(event_bus);
    
    assert_eq!(executor.strategy_name(), "interactive");
    Ok(())
}

#[tokio::test]
async fn test_single_test_execution_with_debugging() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(100));
    let executor = InteractiveExecutor::new(event_bus);
    
    let test_case = create_sample_test_case();
    let context = create_sample_context();
    let debug_options = DebugOptions {
        dry_run: false,
        verbose: true,
        inspect_variables: true,
        breakpoints: std::collections::HashSet::new(),
        step_through: false,
        pause_on_failure: false,
        capture_request_response: true,
    };
    
    let result = executor.execute_single_with_debugging(&test_case, &context, debug_options).await?;
    
    assert_eq!(result.test_case_id, test_case.id);
    assert_eq!(result.test_case_name, test_case.name);
    assert!(!result.dry_run);
    assert!(!result.step_results.is_empty());
    
    Ok(())
}

#[tokio::test]
async fn test_dry_run_validation() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(100));
    let executor = InteractiveExecutor::new(event_bus);
    
    let test_case = create_invalid_test_case();
    let context = create_sample_context();
    let debug_options = DebugOptions {
        dry_run: true,
        ..Default::default()
    };
    
    let result = executor.execute_single_with_debugging(&test_case, &context, debug_options).await?;
    
    assert!(result.dry_run);
    assert!(!result.validation_errors.is_empty());
    
    // Check for expected validation errors
    let has_invalid_url_error = result.validation_errors.iter()
        .any(|e| matches!(e.error_type, ValidationErrorType::InvalidUrl));
    assert!(has_invalid_url_error);
    
    Ok(())
}

#[tokio::test]
async fn test_variable_inspection() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(100));
    let executor = InteractiveExecutor::new(event_bus);
    
    let test_case = create_sample_test_case();
    let context = create_sample_context();
    let debug_options = DebugOptions {
        inspect_variables: true,
        verbose: true,
        ..Default::default()
    };
    
    let result = executor.execute_single_with_debugging(&test_case, &context, debug_options).await?;
    
    // Should have variable snapshots when inspection is enabled
    assert!(!result.variable_snapshots.is_empty());
    
    // Check that context variables are captured
    if let Some(snapshot) = result.variable_snapshots.get(&0) {
        assert!(snapshot.contains_key("base_url"));
        assert!(snapshot.contains_key("api_key"));
    }
    
    Ok(())
}

#[tokio::test]
async fn test_step_execution_breakdown() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(100));
    let executor = InteractiveExecutor::new(event_bus);
    
    let test_case = create_sample_test_case();
    let context = create_sample_context();
    let debug_options = DebugOptions {
        verbose: true,
        capture_request_response: true,
        ..Default::default()
    };
    
    let result = executor.execute_single_with_debugging(&test_case, &context, debug_options).await?;
    
    // Should have multiple steps
    assert!(result.step_results.len() >= 2); // At least request execution and assertion validation
    
    // Check step types
    let has_request_step = result.step_results.iter()
        .any(|s| matches!(s.step_type, StepType::RequestExecution));
    let has_assertion_step = result.step_results.iter()
        .any(|s| matches!(s.step_type, StepType::AssertionValidation));
    
    assert!(has_request_step);
    assert!(has_assertion_step);
    
    // Check that request and response are captured
    let request_step = result.step_results.iter()
        .find(|s| matches!(s.step_type, StepType::RequestExecution))
        .unwrap();
    
    assert!(request_step.request.is_some());
    assert!(request_step.response.is_some());
    
    Ok(())
}

#[tokio::test]
async fn test_breakpoint_functionality() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(100));
    let executor = InteractiveExecutor::new(event_bus);
    
    let test_case = create_sample_test_case();
    let context = create_sample_context();
    
    let mut breakpoints = std::collections::HashSet::new();
    breakpoints.insert(0); // Breakpoint at first step
    
    let debug_options = DebugOptions {
        breakpoints,
        verbose: true,
        ..Default::default()
    };
    
    let result = executor.execute_single_with_debugging(&test_case, &context, debug_options).await?;
    
    // Execution should complete despite breakpoint (in test mode, breakpoints just add a small delay)
    assert!(!result.step_results.is_empty());
    
    Ok(())
}

#[tokio::test]
async fn test_execution_context_variable_resolution() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(100));
    let executor = InteractiveExecutor::new(event_bus);
    
    // Create test case with variable placeholders
    let mut test_case = create_sample_test_case();
    test_case.request.url = "{{base_url}}/users/{{user_id}}".to_string();
    
    let mut context = create_sample_context();
    context.variables.insert("user_id".to_string(), "123".to_string());
    
    let debug_options = DebugOptions {
        verbose: true,
        capture_request_response: true,
        ..Default::default()
    };
    
    let result = executor.execute_single_with_debugging(&test_case, &context, debug_options).await?;
    
    // Check that variables were resolved in the request
    let request_step = result.step_results.iter()
        .find(|s| matches!(s.step_type, StepType::RequestExecution))
        .unwrap();
    
    if let Some(request) = &request_step.request {
        assert!(request.url.contains("https://api.example.com"));
        assert!(request.url.contains("123"));
        assert!(!request.url.contains("{{"));
    }
    
    Ok(())
}

#[tokio::test]
async fn test_failure_handling_and_analysis() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(100));
    let executor = InteractiveExecutor::new(event_bus);
    
    let test_case = create_failing_test_case();
    let context = create_sample_context();
    let debug_options = DebugOptions {
        pause_on_failure: false, // Don't pause in tests
        verbose: true,
        ..Default::default()
    };
    
    let result = executor.execute_single_with_debugging(&test_case, &context, debug_options).await?;
    
    // Should complete but with failure
    assert!(!result.success);
    
    // Should have step results showing the failure
    let failed_steps: Vec<_> = result.step_results.iter()
        .filter(|s| !s.success)
        .collect();
    assert!(!failed_steps.is_empty());
    
    // Failed steps should have error messages
    for step in failed_steps {
        assert!(step.error_message.is_some());
    }
    
    Ok(())
}

// Helper functions for creating test data

fn create_sample_test_case() -> TestCase {
    use chrono::Utc;
    TestCase {
        id: "test-001".to_string(),
        name: "Sample API Test".to_string(),
        description: Some("A sample test case for testing".to_string()),
        tags: vec!["sample".to_string()],
        protocol: "http".to_string(),
        request: RequestDefinition {
            protocol: "http".to_string(),
            method: "GET".to_string(),
            url: "{{base_url}}/users".to_string(),
            headers: HashMap::from([
                ("Authorization".to_string(), "Bearer {{api_key}}".to_string()),
                ("Content-Type".to_string(), "application/json".to_string()),
            ]),
            body: None,
            auth: None,
        },
        assertions: vec![
            AssertionDefinition {
                assertion_type: "status_code".to_string(),
                expected: serde_json::json!(200),
                message: Some("Status should be 200".to_string()),
            }
        ],
        variable_extractions: None,
        dependencies: Vec::new(),
        timeout: Some(Duration::from_secs(60)),
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
    }
}

fn create_invalid_test_case() -> TestCase {
    use chrono::Utc;
    TestCase {
        id: "test-invalid".to_string(),
        name: "Invalid Test Case".to_string(),
        description: Some("A test case with validation errors".to_string()),
        tags: vec!["invalid".to_string()],
        protocol: "http".to_string(),
        request: RequestDefinition {
            protocol: "http".to_string(),
            method: "GET".to_string(),
            url: "".to_string(), // Invalid empty URL
            headers: HashMap::new(),
            body: None,
            auth: None,
        },
        assertions: vec![],
        variable_extractions: None,
        dependencies: Vec::new(),
        timeout: Some(Duration::from_secs(60)),
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
    }
}

fn create_failing_test_case() -> TestCase {
    use chrono::Utc;
    TestCase {
        id: "test-fail".to_string(),
        name: "Failing Test Case".to_string(),
        description: Some("A test case designed to fail".to_string()),
        tags: vec!["failing".to_string()],
        protocol: "http".to_string(),
        request: RequestDefinition {
            protocol: "http".to_string(),
            method: "GET".to_string(),
            url: "{{base_url}}/nonexistent".to_string(),
            headers: HashMap::new(),
            body: None,
            auth: None,
        },
        assertions: vec![
            AssertionDefinition {
                assertion_type: "status_code".to_string(),
                expected: serde_json::json!(200),
                message: Some("Status should be 200".to_string()),
            }
        ],
        variable_extractions: None,
        dependencies: Vec::new(),
        timeout: Some(Duration::from_millis(1)), // Very short timeout to cause failure
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
    }
}

fn create_sample_context() -> ExecutionContext {
    let mut context = ExecutionContext::new(
        "test-suite-001".to_string(),
        "test".to_string(),
    );
    
    context.variables.insert("base_url".to_string(), "https://api.example.com".to_string());
    context.variables.insert("api_key".to_string(), "test-api-key-123".to_string());
    
    context
}