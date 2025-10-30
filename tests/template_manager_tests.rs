use apirunner::template_manager::*;
use apirunner::template_storage::FileTemplateStorage;
use apirunner::test_case_manager::{RequestDefinition, AssertionDefinition};
use apirunner::error::TestCaseError;
use std::collections::HashMap;
use std::time::Duration;
use tempfile::TempDir;
use chrono::Utc;

async fn create_test_template_manager() -> (TestCaseTemplateManager, TempDir) {
    let temp_dir = TempDir::new().unwrap();
    let storage = FileTemplateStorage::new(temp_dir.path());
    storage.initialize().await.unwrap();
    let manager = TestCaseTemplateManager::new(Box::new(storage));
    (manager, temp_dir)
}

fn create_test_template() -> TestCaseTemplate {
    TestCaseTemplate {
        id: "test-template".to_string(),
        name: "Test Template".to_string(),
        description: "A test template for unit testing".to_string(),
        category: TemplateCategory::RestApi,
        template: TestCaseDefinition {
            request: RequestDefinition {
                protocol: "http".to_string(),
                method: "GET".to_string(),
                url: "{{base_url}}/{{endpoint}}".to_string(),
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
                    message: Some("Should be successful".to_string()),
                }
            ],
            variable_extractions: None,
            dependencies: vec![],
            timeout: Some(30),
        },
        variables: vec![
            TemplateVariable {
                name: "base_url".to_string(),
                description: "Base URL of the API".to_string(),
                variable_type: VariableType::Url,
                default_value: Some("https://api.example.com".to_string()),
                required: true,
                validation_pattern: Some(r"^https?://.*".to_string()),
                example_values: vec!["https://api.example.com".to_string()],
            },
            TemplateVariable {
                name: "endpoint".to_string(),
                description: "API endpoint".to_string(),
                variable_type: VariableType::Path,
                default_value: Some("users".to_string()),
                required: true,
                validation_pattern: None,
                example_values: vec!["users".to_string()],
            },
        ],
        instructions: Some("Test template instructions".to_string()),
        created_at: Utc::now(),
        updated_at: Utc::now(),
        version: "1.0.0".to_string(),
        author: Some("Test Author".to_string()),
        tags: vec!["test".to_string(), "rest".to_string()],
    }
}

#[tokio::test]
async fn test_create_template() {
    let (manager, _temp_dir) = create_test_template_manager().await;
    let template = create_test_template();
    
    let options = TemplateCreationOptions::default();
    let template_id = manager.create_template(template.clone(), options).await.unwrap();
    
    assert_eq!(template_id, template.id);
    
    // Verify template was saved
    let loaded_template = manager.get_template(&template_id).await.unwrap();
    assert_eq!(loaded_template.name, template.name);
    assert_eq!(loaded_template.description, template.description);
}

#[tokio::test]
async fn test_get_builtin_template() {
    let (manager, _temp_dir) = create_test_template_manager().await;
    
    // Test getting a builtin template
    let template = manager.get_template("rest-api-get").await.unwrap();
    assert_eq!(template.id, "rest-api-get");
    assert_eq!(template.name, "REST API GET Request");
    assert_eq!(template.category, TemplateCategory::RestApi);
}

#[tokio::test]
async fn test_list_templates() {
    let (manager, _temp_dir) = create_test_template_manager().await;
    
    // Create a custom template
    let template = create_test_template();
    let options = TemplateCreationOptions::default();
    manager.create_template(template, options).await.unwrap();
    
    // List all templates (should include builtin + custom)
    let filter = TemplateFilter::default();
    let templates = manager.list_templates(&filter).await.unwrap();
    
    // Should have builtin templates plus our custom one
    assert!(templates.len() > 1);
    
    // Check that our custom template is included
    let custom_template = templates.iter().find(|t| t.id == "test-template");
    assert!(custom_template.is_some());
}

#[tokio::test]
async fn test_filter_templates_by_category() {
    let (manager, _temp_dir) = create_test_template_manager().await;
    
    let filter = TemplateFilter {
        category: Some(TemplateCategory::RestApi),
        ..Default::default()
    };
    let templates = manager.list_templates(&filter).await.unwrap();
    
    // All returned templates should be REST API templates
    for template in &templates {
        assert_eq!(template.category, TemplateCategory::RestApi);
    }
    
    // Should have at least the builtin REST API templates
    assert!(templates.len() >= 2); // GET and POST templates
}

#[tokio::test]
async fn test_create_from_template() {
    let (manager, _temp_dir) = create_test_template_manager().await;
    
    let variables = HashMap::from([
        ("base_url".to_string(), "https://jsonplaceholder.typicode.com".to_string()),
        ("endpoint".to_string(), "posts".to_string()),
    ]);
    
    let test_case = manager.create_from_template("rest-api-get", variables).await.unwrap();
    
    assert_eq!(test_case.request.method, "GET");
    assert_eq!(test_case.request.url, "https://jsonplaceholder.typicode.com/posts");
    assert_eq!(test_case.protocol, "http");
    assert!(!test_case.assertions.is_empty());
}

#[tokio::test]
async fn test_create_from_template_missing_required_variable() {
    let (manager, _temp_dir) = create_test_template_manager().await;
    
    // Missing required 'base_url' variable
    let variables = HashMap::from([
        ("endpoint".to_string(), "posts".to_string()),
    ]);
    
    let result = manager.create_from_template("rest-api-get", variables).await;
    assert!(result.is_err());
    
    if let Err(TestCaseError::ValidationError(msg)) = result {
        assert!(msg.contains("Missing required variable: base_url"));
    } else {
        panic!("Expected ValidationError for missing required variable");
    }
}

#[tokio::test]
async fn test_variable_validation() {
    let (manager, _temp_dir) = create_test_template_manager().await;
    
    // Test with invalid URL
    let variables = HashMap::from([
        ("base_url".to_string(), "not-a-url".to_string()),
        ("endpoint".to_string(), "posts".to_string()),
    ]);
    
    let result = manager.create_from_template("rest-api-get", variables).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_create_fragment() {
    let (manager, _temp_dir) = create_test_template_manager().await;
    
    let fragment = TestCaseFragment {
        id: "test-fragment".to_string(),
        name: "Test Fragment".to_string(),
        description: "A test fragment".to_string(),
        fragment_type: FragmentType::Headers,
        content: FragmentContent::Headers(HashMap::from([
            ("X-Test-Header".to_string(), "test-value".to_string()),
        ])),
        variables: vec![],
        tags: vec!["test".to_string()],
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };
    
    let fragment_id = manager.create_fragment(fragment.clone()).await.unwrap();
    assert_eq!(fragment_id, fragment.id);
    
    // Verify fragment was saved
    let loaded_fragment = manager.get_fragment(&fragment_id).await.unwrap();
    assert_eq!(loaded_fragment.name, fragment.name);
    assert_eq!(loaded_fragment.fragment_type, fragment.fragment_type);
}

#[tokio::test]
async fn test_get_builtin_fragment() {
    let (manager, _temp_dir) = create_test_template_manager().await;
    
    // Test getting a builtin fragment
    let fragment = manager.get_fragment("common-headers").await.unwrap();
    assert_eq!(fragment.id, "common-headers");
    assert_eq!(fragment.name, "Common HTTP Headers");
    assert_eq!(fragment.fragment_type, FragmentType::Headers);
}

#[tokio::test]
async fn test_list_fragments() {
    let (manager, _temp_dir) = create_test_template_manager().await;
    
    // List all fragments
    let fragments = manager.list_fragments(None).await.unwrap();
    assert!(fragments.len() > 0); // Should have builtin fragments
    
    // List only header fragments
    let header_fragments = manager.list_fragments(Some(FragmentType::Headers)).await.unwrap();
    for fragment in &header_fragments {
        assert_eq!(fragment.fragment_type, FragmentType::Headers);
    }
    
    // Should have at least the builtin common-headers fragment
    assert!(header_fragments.len() >= 1);
}

#[tokio::test]
async fn test_apply_fragment_to_template() {
    let (manager, _temp_dir) = create_test_template_manager().await;
    
    let mut template = create_test_template();
    
    // Apply common headers fragment
    manager.apply_fragment_to_template(&mut template, "common-headers").await.unwrap();
    
    // Template should now have additional headers from the fragment
    assert!(template.template.request.headers.len() > 1);
    assert!(template.template.request.headers.contains_key("User-Agent"));
    assert!(template.template.request.headers.contains_key("Cache-Control"));
}

#[tokio::test]
async fn test_template_with_performance_variables() {
    let (manager, _temp_dir) = create_test_template_manager().await;
    
    let variables = HashMap::from([
        ("base_url".to_string(), "https://httpbin.org".to_string()),
        ("endpoint".to_string(), "delay/1".to_string()),
        ("max_response_time".to_string(), "2000".to_string()),
    ]);
    
    let test_case = manager.create_from_template("performance-test", variables).await.unwrap();
    
    assert_eq!(test_case.request.url, "https://httpbin.org/delay/1");
    assert_eq!(test_case.timeout, Some(Duration::from_secs(2)));
    
    // Should have performance-related assertions
    let has_response_time_assertion = test_case.assertions.iter()
        .any(|a| a.assertion_type == "response_time");
    assert!(has_response_time_assertion);
}

#[tokio::test]
async fn test_oauth_template() {
    let (manager, _temp_dir) = create_test_template_manager().await;
    
    let variables = HashMap::from([
        ("auth_server".to_string(), "https://auth.example.com".to_string()),
        ("client_id".to_string(), "test-client".to_string()),
        ("client_secret".to_string(), "test-secret".to_string()),
        ("scope".to_string(), "read write".to_string()),
    ]);
    
    let test_case = manager.create_from_template("oauth2-auth", variables).await.unwrap();
    
    assert_eq!(test_case.request.method, "POST");
    assert_eq!(test_case.request.url, "https://auth.example.com/oauth/token");
    assert!(test_case.request.body.is_some());
    
    // Should have variable extractions for tokens
    assert!(test_case.variable_extractions.is_some());
    let extractions = test_case.variable_extractions.unwrap();
    let has_token_extraction = extractions.iter()
        .any(|e| e.name == "access_token");
    assert!(has_token_extraction);
}

#[tokio::test]
async fn test_data_validation_template() {
    let (manager, _temp_dir) = create_test_template_manager().await;
    
    let variables = HashMap::from([
        ("base_url".to_string(), "https://jsonplaceholder.typicode.com".to_string()),
        ("endpoint".to_string(), "users/1".to_string()),
        ("required_fields".to_string(), "id,name,email".to_string()),
        ("expected_value".to_string(), "1".to_string()),
        ("extraction_path".to_string(), "id".to_string()),
    ]);
    
    let test_case = manager.create_from_template("data-validation", variables).await.unwrap();
    
    assert_eq!(test_case.request.url, "https://jsonplaceholder.typicode.com/users/1");
    
    // Should have schema validation assertion
    let has_schema_assertion = test_case.assertions.iter()
        .any(|a| a.assertion_type == "json_schema");
    assert!(has_schema_assertion);
    
    // Should have variable extraction
    assert!(test_case.variable_extractions.is_some());
}

#[tokio::test]
async fn test_template_variable_types() {
    let (manager, _temp_dir) = create_test_template_manager().await;
    
    // Test number validation
    let variables = HashMap::from([
        ("base_url".to_string(), "https://httpbin.org".to_string()),
        ("endpoint".to_string(), "delay/1".to_string()),
        ("max_response_time".to_string(), "not-a-number".to_string()),
    ]);
    
    let result = manager.create_from_template("performance-test", variables).await;
    assert!(result.is_err());
    
    // Test valid number
    let variables = HashMap::from([
        ("base_url".to_string(), "https://httpbin.org".to_string()),
        ("endpoint".to_string(), "delay/1".to_string()),
        ("max_response_time".to_string(), "1500".to_string()),
    ]);
    
    let result = manager.create_from_template("performance-test", variables).await;
    assert!(result.is_ok());
}