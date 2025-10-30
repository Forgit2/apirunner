use apirunner::test_case_manager::*;
use apirunner::storage::FileSystemStorage;
use apirunner::error::TestCaseError;
use chrono::Utc;
use std::collections::HashMap;
use std::time::Duration;
use tempfile::TempDir;

fn create_sample_test_case() -> TestCase {
    TestCase {
        id: "test-case-1".to_string(),
        name: "Sample API Test".to_string(),
        description: Some("A sample test case for API testing".to_string()),
        tags: vec!["api".to_string(), "integration".to_string()],
        protocol: "http".to_string(),
        request: RequestDefinition {
            protocol: "http".to_string(),
            method: "GET".to_string(),
            url: "https://api.example.com/users".to_string(),
            headers: {
                let mut headers = HashMap::new();
                headers.insert("Accept".to_string(), "application/json".to_string());
                headers.insert("User-Agent".to_string(), "API-Test-Runner/1.0".to_string());
                headers
            },
            body: None,
            auth: Some(AuthDefinition {
                auth_type: "bearer".to_string(),
                config: {
                    let mut config = HashMap::new();
                    config.insert("token".to_string(), "{{auth_token}}".to_string());
                    config
                },
            }),
        },
        assertions: vec![
            AssertionDefinition {
                assertion_type: "status_code".to_string(),
                expected: serde_json::json!(200),
                message: Some("Response should be successful".to_string()),
            },
            AssertionDefinition {
                assertion_type: "response_time".to_string(),
                expected: serde_json::json!(1000),
                message: Some("Response should be under 1 second".to_string()),
            },
        ],
        variable_extractions: Some(vec![
            VariableExtraction {
                name: "user_id".to_string(),
                source: ExtractionSource::ResponseBody,
                path: "$.data[0].id".to_string(),
                default_value: None,
            },
        ]),
        dependencies: vec![],
        timeout: Some(Duration::from_secs(30)),
        created_at: Utc::now(),
        updated_at: Utc::now(),
        version: 1,
        change_log: vec![],
    }
}

async fn create_test_manager() -> (TestCaseManager, TempDir) {
    let temp_dir = TempDir::new().unwrap();
    let storage = FileSystemStorage::new(temp_dir.path().to_path_buf(), TestCaseFormat::Json)
        .await
        .unwrap();
    let manager = TestCaseManager::new(Box::new(storage));
    (manager, temp_dir)
}

#[tokio::test]
async fn test_create_test_case() {
    let (manager, _temp_dir) = create_test_manager().await;
    let test_case = create_sample_test_case();

    let id = manager.create_test_case(test_case.clone()).await.unwrap();
    assert_eq!(id, test_case.id);

    // Verify the test case was created
    let loaded_test_case = manager.read_test_case(&id).await.unwrap();
    assert_eq!(loaded_test_case.name, test_case.name);
    assert_eq!(loaded_test_case.protocol, test_case.protocol);
    assert_eq!(loaded_test_case.tags, test_case.tags);
}

#[tokio::test]
async fn test_create_duplicate_test_case() {
    let (manager, _temp_dir) = create_test_manager().await;
    let test_case = create_sample_test_case();

    // Create the test case first time
    manager.create_test_case(test_case.clone()).await.unwrap();

    // Try to create the same test case again
    let result = manager.create_test_case(test_case).await;
    assert!(matches!(result, Err(TestCaseError::AlreadyExists(_))));
}

#[tokio::test]
async fn test_read_nonexistent_test_case() {
    let (manager, _temp_dir) = create_test_manager().await;

    let result = manager.read_test_case("nonexistent-id").await;
    assert!(matches!(result, Err(TestCaseError::NotFound(_))));
}

#[tokio::test]
async fn test_update_test_case() {
    let (manager, _temp_dir) = create_test_manager().await;
    let mut test_case = create_sample_test_case();

    // Create the test case
    let id = manager.create_test_case(test_case.clone()).await.unwrap();

    // Update the test case
    test_case.name = "Updated Test Case".to_string();
    test_case.description = Some("Updated description".to_string());
    test_case.tags.push("updated".to_string());

    manager.update_test_case(&id, test_case.clone()).await.unwrap();

    // Verify the update
    let loaded_test_case = manager.read_test_case(&id).await.unwrap();
    assert_eq!(loaded_test_case.name, "Updated Test Case");
    assert_eq!(loaded_test_case.description, Some("Updated description".to_string()));
    assert!(loaded_test_case.tags.contains(&"updated".to_string()));
    assert_eq!(loaded_test_case.version, 2); // Version should be incremented
}

#[tokio::test]
async fn test_update_nonexistent_test_case() {
    let (manager, _temp_dir) = create_test_manager().await;
    let test_case = create_sample_test_case();

    let result = manager.update_test_case("nonexistent-id", test_case).await;
    assert!(matches!(result, Err(TestCaseError::NotFound(_))));
}

#[tokio::test]
async fn test_delete_test_case() {
    let (manager, _temp_dir) = create_test_manager().await;
    let test_case = create_sample_test_case();

    // Create the test case
    let id = manager.create_test_case(test_case).await.unwrap();

    // Verify it exists
    assert!(manager.read_test_case(&id).await.is_ok());

    // Delete the test case
    manager.delete_test_case(&id).await.unwrap();

    // Verify it's deleted
    let result = manager.read_test_case(&id).await;
    assert!(matches!(result, Err(TestCaseError::NotFound(_))));
}

#[tokio::test]
async fn test_delete_nonexistent_test_case() {
    let (manager, _temp_dir) = create_test_manager().await;

    let result = manager.delete_test_case("nonexistent-id").await;
    assert!(matches!(result, Err(TestCaseError::NotFound(_))));
}

#[tokio::test]
async fn test_list_test_cases() {
    let (manager, _temp_dir) = create_test_manager().await;

    // Create multiple test cases
    let mut test_case1 = create_sample_test_case();
    test_case1.id = "test-1".to_string();
    test_case1.name = "API Test 1".to_string();
    test_case1.tags = vec!["api".to_string(), "smoke".to_string()];

    let mut test_case2 = create_sample_test_case();
    test_case2.id = "test-2".to_string();
    test_case2.name = "Database Test".to_string();
    test_case2.protocol = "database".to_string();
    test_case2.tags = vec!["database".to_string(), "integration".to_string()];

    let mut test_case3 = create_sample_test_case();
    test_case3.id = "test-3".to_string();
    test_case3.name = "API Test 2".to_string();
    test_case3.tags = vec!["api".to_string(), "regression".to_string()];

    manager.create_test_case(test_case1).await.unwrap();
    manager.create_test_case(test_case2).await.unwrap();
    manager.create_test_case(test_case3).await.unwrap();

    // Test listing all test cases
    let filter = TestCaseFilter::default();
    let all_test_cases = manager.list_test_cases(&filter).await.unwrap();
    assert_eq!(all_test_cases.len(), 3);

    // Test filtering by protocol
    let filter = TestCaseFilter {
        protocol: Some("http".to_string()),
        ..Default::default()
    };
    let http_test_cases = manager.list_test_cases(&filter).await.unwrap();
    assert_eq!(http_test_cases.len(), 2);

    // Test filtering by tags
    let filter = TestCaseFilter {
        tags: Some(vec!["smoke".to_string()]),
        ..Default::default()
    };
    let smoke_test_cases = manager.list_test_cases(&filter).await.unwrap();
    assert_eq!(smoke_test_cases.len(), 1);
    assert_eq!(smoke_test_cases[0].name, "API Test 1");

    // Test filtering by name pattern
    let filter = TestCaseFilter {
        name_pattern: Some("API".to_string()),
        ..Default::default()
    };
    let api_test_cases = manager.list_test_cases(&filter).await.unwrap();
    assert_eq!(api_test_cases.len(), 2);
}

#[tokio::test]
async fn test_search_test_cases() {
    let (manager, _temp_dir) = create_test_manager().await;

    let mut test_case = create_sample_test_case();
    test_case.name = "User Authentication Test".to_string();
    manager.create_test_case(test_case).await.unwrap();

    let results = manager.search_by_text("Authentication", &SearchOptions::default()).await.unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].name, "User Authentication Test");
}

#[tokio::test]
async fn test_get_test_cases_by_tags() {
    let (manager, _temp_dir) = create_test_manager().await;

    let mut test_case1 = create_sample_test_case();
    test_case1.id = "test-1".to_string();
    test_case1.tags = vec!["api".to_string(), "critical".to_string()];

    let mut test_case2 = create_sample_test_case();
    test_case2.id = "test-2".to_string();
    test_case2.tags = vec!["database".to_string(), "critical".to_string()];

    manager.create_test_case(test_case1).await.unwrap();
    manager.create_test_case(test_case2).await.unwrap();

    let results = manager.get_test_cases_by_tags(vec!["critical".to_string()], false).await.unwrap();
    assert_eq!(results.len(), 2);
}

#[tokio::test]
async fn test_get_test_cases_by_protocol() {
    let (manager, _temp_dir) = create_test_manager().await;

    let mut test_case1 = create_sample_test_case();
    test_case1.id = "test-1".to_string();
    test_case1.protocol = "http".to_string();

    let mut test_case2 = create_sample_test_case();
    test_case2.id = "test-2".to_string();
    test_case2.protocol = "grpc".to_string();

    manager.create_test_case(test_case1).await.unwrap();
    manager.create_test_case(test_case2).await.unwrap();

    let results = manager.get_test_cases_by_protocol("http").await.unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].protocol, "http");
}

#[tokio::test]
async fn test_validation_empty_name() {
    let (manager, _temp_dir) = create_test_manager().await;
    let mut test_case = create_sample_test_case();
    test_case.name = "".to_string();

    let result = manager.create_test_case(test_case).await;
    assert!(matches!(result, Err(TestCaseError::ValidationError(_))));
}

#[tokio::test]
async fn test_validation_empty_protocol() {
    let (manager, _temp_dir) = create_test_manager().await;
    let mut test_case = create_sample_test_case();
    test_case.protocol = "".to_string();

    let result = manager.create_test_case(test_case).await;
    assert!(matches!(result, Err(TestCaseError::ValidationError(_))));
}

#[tokio::test]
async fn test_validation_empty_method() {
    let (manager, _temp_dir) = create_test_manager().await;
    let mut test_case = create_sample_test_case();
    test_case.request.method = "".to_string();

    let result = manager.create_test_case(test_case).await;
    assert!(matches!(result, Err(TestCaseError::ValidationError(_))));
}

#[tokio::test]
async fn test_validation_empty_url() {
    let (manager, _temp_dir) = create_test_manager().await;
    let mut test_case = create_sample_test_case();
    test_case.request.url = "".to_string();

    let result = manager.create_test_case(test_case).await;
    assert!(matches!(result, Err(TestCaseError::ValidationError(_))));
}

#[tokio::test]
async fn test_validation_invalid_url() {
    let (manager, _temp_dir) = create_test_manager().await;
    let mut test_case = create_sample_test_case();
    test_case.request.url = "invalid-url".to_string();

    let result = manager.create_test_case(test_case).await;
    assert!(matches!(result, Err(TestCaseError::ValidationError(_))));
}

#[tokio::test]
async fn test_yaml_format() {
    let temp_dir = TempDir::new().unwrap();
    let storage = FileSystemStorage::new(temp_dir.path().to_path_buf(), TestCaseFormat::Yaml)
        .await
        .unwrap();
    let manager = TestCaseManager::new(Box::new(storage));
    
    let test_case = create_sample_test_case();
    let id = manager.create_test_case(test_case.clone()).await.unwrap();
    
    let loaded_test_case = manager.read_test_case(&id).await.unwrap();
    assert_eq!(loaded_test_case.name, test_case.name);
    assert_eq!(loaded_test_case.protocol, test_case.protocol);
}