use apirunner::{
    TestCaseLinter, LinterConfig, ValidationSeverity,
    test_case_manager::{TestCase as TestCaseManagerTestCase, RequestDefinition as TestCaseRequestDefinition,
        AssertionDefinition as TestCaseAssertionDefinition, VariableExtraction as TestCaseVariableExtraction,
        ExtractionSource, AuthDefinition, ChangeLogEntry, ChangeType},
};
use std::collections::HashMap;
use std::time::Duration;
use chrono::Utc;

fn create_sample_test_case() -> TestCaseManagerTestCase {
    TestCaseManagerTestCase {
        id: "test-001".to_string(),
        name: "Sample API Test".to_string(),
        description: Some("A sample test case for validation".to_string()),
        tags: vec!["api".to_string(), "smoke".to_string()],
        protocol: "http".to_string(),
        request: TestCaseRequestDefinition {
            protocol: "http".to_string(),
            method: "GET".to_string(),
            url: "https://api.example.com/users".to_string(),
            headers: {
                let mut headers = HashMap::new();
                headers.insert("Accept".to_string(), "application/json".to_string());
                headers
            },
            body: None,
            auth: None,
        },
        assertions: vec![
            TestCaseAssertionDefinition {
                assertion_type: "status_code".to_string(),
                expected: serde_json::Value::Number(200.into()),
                message: Some("Should return 200 OK".to_string()),
            }
        ],
        variable_extractions: Some(vec![
            TestCaseVariableExtraction {
                name: "user_id".to_string(),
                source: ExtractionSource::ResponseBody,
                path: "$.data[0].id".to_string(),
                default_value: None,
            }
        ]),
        dependencies: vec![],
        timeout: Some(Duration::from_secs(30)),
        created_at: Utc::now(),
        updated_at: Utc::now(),
        version: 1,
        change_log: vec![],
    }
}

fn create_problematic_test_case() -> TestCaseManagerTestCase {
    TestCaseManagerTestCase {
        id: "test-002".to_string(),
        name: "bad".to_string(), // Too short name
        description: None, // Missing description
        tags: vec![], // No tags
        protocol: "http".to_string(),
        request: TestCaseRequestDefinition {
            protocol: "http".to_string(),
            method: "INVALID".to_string(), // Invalid HTTP method
            url: "localhost:8080/api".to_string(), // Missing protocol, localhost
            headers: {
                let mut headers = HashMap::new();
                headers.insert("Authorization".to_string(), "Bearer hardcoded-token".to_string()); // Hardcoded credentials
                headers
            },
            body: Some(r#"{"data": "test"}"#.to_string()),
            auth: None,
        },
        assertions: vec![], // No assertions
        variable_extractions: Some(vec![
            TestCaseVariableExtraction {
                name: "X".to_string(), // Too short variable name
                source: ExtractionSource::ResponseBody,
                path: "$.unused".to_string(), // This variable is never used
                default_value: None,
            }
        ]),
        dependencies: vec![],
        timeout: Some(Duration::from_secs(500)), // Very high timeout
        created_at: Utc::now(),
        updated_at: Utc::now(),
        version: 1,
        change_log: vec![],
    }
}

#[tokio::test]
async fn test_lint_valid_test_case() {
    let config = LinterConfig::default();
    let linter = TestCaseLinter::new(config);
    let test_case = create_sample_test_case();
    
    let result = linter.lint_test_case(&test_case).await.unwrap();
    
    assert_eq!(result.test_case_id, "test-001");
    assert_eq!(result.test_case_name, "Sample API Test");
    
    // Should have minimal issues for a well-formed test case
    let error_count = result.issues.iter()
        .filter(|i| i.severity == ValidationSeverity::Error)
        .count();
    assert_eq!(error_count, 0);
    
    // Quality score should be high
    assert!(result.score > 80.0);
}

#[tokio::test]
async fn test_lint_problematic_test_case() {
    let config = LinterConfig::default();
    let linter = TestCaseLinter::new(config);
    let test_case = create_problematic_test_case();
    
    let result = linter.lint_test_case(&test_case).await.unwrap();
    
    assert_eq!(result.test_case_id, "test-002");
    
    // Should have multiple issues
    assert!(!result.issues.is_empty());
    
    // Should have at least one error (no assertions, invalid method, missing protocol)
    let error_count = result.issues.iter()
        .filter(|i| i.severity == ValidationSeverity::Error)
        .count();
    assert!(error_count > 0);
    
    // Quality score should be low
    assert!(result.score < 50.0);
    
    // Check for specific issues
    let issue_rules: Vec<&str> = result.issues.iter()
        .map(|i| i.rule_id.as_str())
        .collect();
    
    // Should detect some issues
    assert!(issue_rules.len() > 0);
}

#[tokio::test]
async fn test_lint_multiple_test_cases() {
    let config = LinterConfig::default();
    let linter = TestCaseLinter::new(config);
    let test_cases = vec![
        create_sample_test_case(),
        create_problematic_test_case(),
    ];
    
    let report = linter.lint_test_cases(&test_cases).await.unwrap();
    
    assert_eq!(report.total_test_cases, 2);
    assert_eq!(report.test_case_results.len(), 2);
    
    // Should have issues from the problematic test case
    let total_errors = report.issues_by_severity.get(&ValidationSeverity::Error).unwrap_or(&0);
    assert!(*total_errors > 0);
    
    // Average score should be moderate
    assert!(report.average_score > 0.0);
    assert!(report.average_score < 100.0);
}

#[tokio::test]
async fn test_linter_config_severity_threshold() {
    let mut config = LinterConfig::default();
    config.severity_threshold = ValidationSeverity::Error;
    
    let linter = TestCaseLinter::new(config);
    let test_case = create_problematic_test_case();
    
    let result = linter.lint_test_case(&test_case).await.unwrap();
    
    // Should only include errors, not warnings or info
    for issue in &result.issues {
        assert_eq!(issue.severity, ValidationSeverity::Error);
    }
}

#[tokio::test]
async fn test_linter_config_disabled_rules() {
    let mut config = LinterConfig::default();
    config.disabled_rules = vec!["assertion-required".to_string()];
    
    let linter = TestCaseLinter::new(config);
    let test_case = create_problematic_test_case();
    
    let result = linter.lint_test_case(&test_case).await.unwrap();
    
    // Should not include the disabled rule
    let issue_rules: Vec<&str> = result.issues.iter()
        .map(|i| i.rule_id.as_str())
        .collect();
    
    assert!(!issue_rules.contains(&"assertion-required"));
}

#[tokio::test]
async fn test_linter_config_enabled_rules_only() {
    let mut config = LinterConfig::default();
    config.enabled_rules = vec!["naming-convention".to_string(), "description-required".to_string()];
    
    let linter = TestCaseLinter::new(config);
    let test_case = create_problematic_test_case();
    
    let result = linter.lint_test_case(&test_case).await.unwrap();
    
    // Should only include the enabled rules
    let issue_rules: Vec<&str> = result.issues.iter()
        .map(|i| i.rule_id.as_str())
        .collect();
    
    for rule in &issue_rules {
        assert!(["naming-convention", "description-required"].contains(rule));
    }
}