use apirunner::test_case_documentation::{
    TestCaseDocumentationGenerator, DocumentationConfig, DocumentationFormat, GroupingStrategy,
};
use apirunner::test_case_manager::{
    TestCase, RequestDefinition, AssertionDefinition, VariableExtraction, ExtractionSource,
};
use std::collections::HashMap;
use std::path::PathBuf;
use chrono::Utc;
use std::time::Duration;

fn create_sample_test_case() -> TestCase {
    TestCase {
        id: "test-001".to_string(),
        name: "Sample API Test".to_string(),
        description: Some("A sample test case for documentation".to_string()),
        tags: vec!["api".to_string(), "smoke".to_string()],
        protocol: "http".to_string(),
        request: RequestDefinition {
            protocol: "http".to_string(),
            method: "GET".to_string(),
            url: "https://api.example.com/users".to_string(),
            headers: {
                let mut headers = HashMap::new();
                headers.insert("Accept".to_string(), "application/json".to_string());
                headers.insert("Authorization".to_string(), "Bearer token".to_string());
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
                assertion_type: "json_path".to_string(),
                expected: serde_json::Value::String("users".to_string()),
                message: Some("Should contain users data".to_string()),
            }
        ],
        variable_extractions: Some(vec![
            VariableExtraction {
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

#[tokio::test]
async fn test_generate_markdown_documentation() {
    let generator = TestCaseDocumentationGenerator::new();
    let config = DocumentationConfig {
        output_format: DocumentationFormat::Markdown,
        output_path: PathBuf::from("test-docs.md"),
        include_examples: true,
        include_schemas: false,
        include_curl_commands: true,
        group_by: GroupingStrategy::ByTag,
        template_path: None,
    };
    
    let test_cases = vec![create_sample_test_case()];
    
    let result = generator.generate_documentation(&test_cases, &config).await;
    assert!(result.is_ok());
    
    let documentation = result.unwrap();
    assert!(documentation.contains("# API Test Cases Documentation"));
    assert!(documentation.contains("Sample API Test"));
}

#[tokio::test]
async fn test_documentation_generator_creation() {
    let generator = TestCaseDocumentationGenerator::new();
    
    // Should create successfully
    assert!(true); // Basic creation test
}