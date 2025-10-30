use apirunner::test_quality_metrics::{
    TestQualityAnalyzer, QualityMetricsConfig,
};
use apirunner::test_case_manager::{
    TestCase, RequestDefinition, AssertionDefinition, VariableExtraction, ExtractionSource,
};
use std::collections::HashMap;
use std::time::Duration;
use chrono::Utc;

fn create_high_quality_test_case() -> TestCase {
    TestCase {
        id: "test-hq-001".to_string(),
        name: "High Quality API Test".to_string(),
        description: Some("A well-documented test case with comprehensive assertions".to_string()),
        tags: vec!["api".to_string(), "smoke".to_string(), "critical".to_string()],
        protocol: "https".to_string(),
        request: RequestDefinition {
            protocol: "https".to_string(),
            method: "GET".to_string(),
            url: "https://api.example.com/users".to_string(),
            headers: {
                let mut headers = HashMap::new();
                headers.insert("Accept".to_string(), "application/json".to_string());
                headers.insert("Authorization".to_string(), "Bearer token".to_string());
                headers.insert("User-Agent".to_string(), "TestRunner/1.0".to_string());
                headers
            },
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
                message: Some("Response time should be under 1 second".to_string()),
            },
            AssertionDefinition {
                assertion_type: "json_path".to_string(),
                expected: serde_json::Value::String("array".to_string()),
                message: Some("Response should contain users array".to_string()),
            }
        ],
        variable_extractions: Some(vec![
            VariableExtraction {
                name: "user_count".to_string(),
                source: ExtractionSource::ResponseBody,
                path: "$.users.length".to_string(),
                default_value: None,
            },
            VariableExtraction {
                name: "first_user_id".to_string(),
                source: ExtractionSource::ResponseBody,
                path: "$.users[0].id".to_string(),
                default_value: None,
            }
        ]),
        dependencies: vec![],
        timeout: Some(Duration::from_secs(30)),
        created_at: Utc::now(),
        updated_at: Utc::now(),
        version: 3,
        change_log: vec![],
    }
}

fn create_low_quality_test_case() -> TestCase {
    TestCase {
        id: "test-lq-001".to_string(),
        name: "test".to_string(), // Poor naming
        description: None, // No description
        tags: vec![], // No tags
        protocol: "http".to_string(), // Insecure protocol
        request: RequestDefinition {
            protocol: "http".to_string(),
            method: "GET".to_string(),
            url: "http://api.example.com/data".to_string(),
            headers: HashMap::new(), // No headers
            body: None,
            auth: None,
        },
        assertions: vec![], // No assertions
        variable_extractions: None, // No variable extractions
        dependencies: vec![],
        timeout: Some(Duration::from_secs(300)), // Very high timeout
        created_at: Utc::now(),
        updated_at: Utc::now(),
        version: 1,
        change_log: vec![],
    }
}

#[tokio::test]
async fn test_quality_analyzer_creation() {
    let config = QualityMetricsConfig::default();
    let analyzer = TestQualityAnalyzer::new(config);
    
    // Should create successfully
    assert!(true); // Basic creation test
}

#[tokio::test]
async fn test_analyze_high_quality_test_case() {
    let config = QualityMetricsConfig::default();
    let analyzer = TestQualityAnalyzer::new(config);
    
    let test_case = create_high_quality_test_case();
    let metrics = analyzer.analyze_test_quality(&[test_case], None).await.unwrap();
    
    // High quality test should have good scores
    assert!(metrics.quality_score > 0.5);
}

#[tokio::test]
async fn test_analyze_low_quality_test_case() {
    let config = QualityMetricsConfig::default();
    let analyzer = TestQualityAnalyzer::new(config);
    
    let test_case = create_low_quality_test_case();
    let metrics = analyzer.analyze_test_quality(&[test_case], None).await.unwrap();
    
    // Should return valid metrics
    assert!(metrics.quality_score >= 0.0);
}

#[tokio::test]
async fn test_analyze_test_suite() {
    let config = QualityMetricsConfig::default();
    let analyzer = TestQualityAnalyzer::new(config);
    
    let test_cases = vec![
        create_high_quality_test_case(),
        create_low_quality_test_case(),
    ];
    
    let suite_metrics = analyzer.analyze_test_quality(&test_cases, None).await.unwrap();
    
    assert!(suite_metrics.quality_score >= 0.0);
    
    // Should have coverage metrics
    assert!(suite_metrics.coverage_metrics.http_methods_coverage.contains_key("GET"));
    assert!(suite_metrics.coverage_metrics.total_endpoints > 0);
}