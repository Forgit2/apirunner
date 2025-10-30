use apirunner::test_case_manager::*;
use apirunner::storage::FileSystemStorage;
use std::collections::HashMap;
use std::time::Duration;
use uuid::Uuid;
use chrono::Utc;
use tempfile::TempDir;

async fn create_test_manager() -> (TestCaseManager, TempDir) {
    let temp_dir = TempDir::new().unwrap();
    let storage = FileSystemStorage::new(temp_dir.path().to_path_buf(), TestCaseFormat::Json).await.unwrap();
    let manager = TestCaseManager::new(Box::new(storage));
    (manager, temp_dir)
}

fn create_sample_test_case(name: &str, protocol: &str, method: &str, tags: Vec<String>) -> TestCase {
    TestCase {
        id: Uuid::new_v4().to_string(),
        name: name.to_string(),
        description: Some(format!("Description for {}", name)),
        tags,
        protocol: protocol.to_string(),
        request: RequestDefinition {
            protocol: protocol.to_string(),
            method: method.to_string(),
            url: format!("https://api.example.com/{}", name.to_lowercase().replace(' ', "-")),
            headers: HashMap::new(),
            body: None,
            auth: None,
        },
        assertions: vec![
            AssertionDefinition {
                assertion_type: "status_code".to_string(),
                expected: serde_json::Value::Number(serde_json::Number::from(200)),
                message: Some("Expected successful response".to_string()),
            }
        ],
        variable_extractions: Some(Vec::new()),
        dependencies: Vec::new(),
        timeout: Some(Duration::from_secs(5)),
        created_at: Utc::now(),
        updated_at: Utc::now(),
        version: 1,
        change_log: vec![],
    }
}

#[tokio::test]
async fn test_search_by_name_pattern() {
    let (manager, _temp_dir) = create_test_manager().await;

    // Create test cases
    let test_cases = vec![
        create_sample_test_case("User Login", "http", "POST", vec!["auth".to_string(), "user".to_string()]),
        create_sample_test_case("User Logout", "http", "POST", vec!["auth".to_string(), "user".to_string()]),
        create_sample_test_case("Get Products", "http", "GET", vec!["product".to_string()]),
        create_sample_test_case("Create Order", "http", "POST", vec!["order".to_string()]),
    ];

    for test_case in test_cases {
        manager.create_test_case(test_case).await.unwrap();
    }

    // Search for test cases with "User" in the name
    let filter = TestCaseFilter {
        name_pattern: Some("User".to_string()),
        ..Default::default()
    };
    let options = SearchOptions::default();
    let results = manager.search_test_cases(&filter, &options).await.unwrap();

    assert_eq!(results.len(), 2);
    assert!(results.iter().any(|tc| tc.name == "User Login"));
    assert!(results.iter().any(|tc| tc.name == "User Logout"));
}

#[tokio::test]
async fn test_search_by_tags() {
    let (manager, _temp_dir) = create_test_manager().await;

    // Create test cases
    let test_cases = vec![
        create_sample_test_case("User Login", "http", "POST", vec!["auth".to_string(), "user".to_string()]),
        create_sample_test_case("Admin Login", "http", "POST", vec!["auth".to_string(), "admin".to_string()]),
        create_sample_test_case("Get Products", "http", "GET", vec!["product".to_string()]),
    ];

    for test_case in test_cases {
        manager.create_test_case(test_case).await.unwrap();
    }

    // Search for test cases with "auth" tag
    let results = manager.get_test_cases_by_tags(vec!["auth".to_string()], false).await.unwrap();
    assert_eq!(results.len(), 2);

    // Search for test cases with both "auth" and "user" tags (match all)
    let results = manager.get_test_cases_by_tags(vec!["auth".to_string(), "user".to_string()], true).await.unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].name, "User Login");
}

#[tokio::test]
async fn test_search_by_protocol() {
    let (manager, _temp_dir) = create_test_manager().await;

    // Create test cases with different protocols
    let test_cases = vec![
        create_sample_test_case("HTTP Test", "http", "GET", vec!["web".to_string()]),
        create_sample_test_case("HTTPS Test", "https", "GET", vec!["web".to_string()]),
        create_sample_test_case("GraphQL Test", "graphql", "POST", vec!["api".to_string()]),
    ];

    for test_case in test_cases {
        manager.create_test_case(test_case).await.unwrap();
    }

    // Search for HTTP test cases
    let results = manager.get_test_cases_by_protocol("http").await.unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].name, "HTTP Test");

    // Search for HTTPS test cases
    let results = manager.get_test_cases_by_protocol("https").await.unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].name, "HTTPS Test");
}

#[tokio::test]
async fn test_search_with_method_filter() {
    let (manager, _temp_dir) = create_test_manager().await;

    // Create test cases with different methods
    let test_cases = vec![
        create_sample_test_case("Get Users", "http", "GET", vec!["user".to_string()]),
        create_sample_test_case("Create User", "http", "POST", vec!["user".to_string()]),
        create_sample_test_case("Update User", "http", "PUT", vec!["user".to_string()]),
        create_sample_test_case("Delete User", "http", "DELETE", vec!["user".to_string()]),
    ];

    for test_case in test_cases {
        manager.create_test_case(test_case).await.unwrap();
    }

    // Search for POST methods
    let filter = TestCaseFilter {
        method: Some("POST".to_string()),
        ..Default::default()
    };
    let options = SearchOptions::default();
    let results = manager.search_test_cases(&filter, &options).await.unwrap();

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].name, "Create User");
}

#[tokio::test]
async fn test_search_with_url_pattern() {
    let (manager, _temp_dir) = create_test_manager().await;

    // Create test cases with different URLs
    let mut test_case1 = create_sample_test_case("API Test", "http", "GET", vec!["api".to_string()]);
    test_case1.request.url = "https://api.example.com/users".to_string();

    let mut test_case2 = create_sample_test_case("Web Test", "http", "GET", vec!["web".to_string()]);
    test_case2.request.url = "https://web.example.com/login".to_string();

    manager.create_test_case(test_case1).await.unwrap();
    manager.create_test_case(test_case2).await.unwrap();

    // Search for URLs containing "api"
    let filter = TestCaseFilter {
        url_pattern: Some("api".to_string()),
        ..Default::default()
    };
    let options = SearchOptions::default();
    let results = manager.search_test_cases(&filter, &options).await.unwrap();

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].name, "API Test");
}

#[tokio::test]
async fn test_search_with_regex() {
    let (manager, _temp_dir) = create_test_manager().await;

    // Create test cases
    let test_cases = vec![
        create_sample_test_case("Test123", "http", "GET", vec!["test".to_string()]),
        create_sample_test_case("TestABC", "http", "GET", vec!["test".to_string()]),
        create_sample_test_case("Sample", "http", "GET", vec!["sample".to_string()]),
    ];

    for test_case in test_cases {
        manager.create_test_case(test_case).await.unwrap();
    }

    // Search using regex pattern for names ending with digits
    let filter = TestCaseFilter {
        name_pattern: Some(r"Test\d+$".to_string()),
        ..Default::default()
    };
    let options = SearchOptions {
        use_regex: true,
        ..Default::default()
    };
    let results = manager.search_test_cases(&filter, &options).await.unwrap();

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].name, "Test123");
}

#[tokio::test]
async fn test_search_with_sorting() {
    let (manager, _temp_dir) = create_test_manager().await;

    // Create test cases with different names
    let test_cases = vec![
        create_sample_test_case("Zebra Test", "http", "GET", vec!["animal".to_string()]),
        create_sample_test_case("Alpha Test", "http", "GET", vec!["animal".to_string()]),
        create_sample_test_case("Beta Test", "http", "GET", vec!["animal".to_string()]),
    ];

    for test_case in test_cases {
        manager.create_test_case(test_case).await.unwrap();
    }

    // Search with ascending name sort
    let filter = TestCaseFilter::default();
    let options = SearchOptions {
        sort_by: Some(SortField::Name),
        sort_order: SortOrder::Ascending,
        ..Default::default()
    };
    let results = manager.search_test_cases(&filter, &options).await.unwrap();

    assert_eq!(results.len(), 3);
    assert_eq!(results[0].name, "Alpha Test");
    assert_eq!(results[1].name, "Beta Test");
    assert_eq!(results[2].name, "Zebra Test");

    // Search with descending name sort
    let options = SearchOptions {
        sort_by: Some(SortField::Name),
        sort_order: SortOrder::Descending,
        ..Default::default()
    };
    let results = manager.search_test_cases(&filter, &options).await.unwrap();

    assert_eq!(results[0].name, "Zebra Test");
    assert_eq!(results[1].name, "Beta Test");
    assert_eq!(results[2].name, "Alpha Test");
}

#[tokio::test]
async fn test_search_with_pagination() {
    let (manager, _temp_dir) = create_test_manager().await;

    // Create multiple test cases
    for i in 1..=10 {
        let test_case = create_sample_test_case(&format!("Test {}", i), "http", "GET", vec!["test".to_string()]);
        manager.create_test_case(test_case).await.unwrap();
    }

    // Test pagination with limit
    let filter = TestCaseFilter::default();
    let options = SearchOptions {
        limit: Some(5),
        sort_by: Some(SortField::Name),
        sort_order: SortOrder::Ascending,
        ..Default::default()
    };
    let results = manager.search_test_cases(&filter, &options).await.unwrap();

    assert_eq!(results.len(), 5);

    // Test pagination with offset
    let options = SearchOptions {
        limit: Some(3),
        offset: Some(2),
        sort_by: Some(SortField::Name),
        sort_order: SortOrder::Ascending,
        ..Default::default()
    };
    let results = manager.search_test_cases(&filter, &options).await.unwrap();

    assert_eq!(results.len(), 3);
}

#[tokio::test]
async fn test_get_recent_test_cases() {
    let (manager, _temp_dir) = create_test_manager().await;

    // Create test cases
    for i in 1..=5 {
        let test_case = create_sample_test_case(&format!("Test {}", i), "http", "GET", vec!["test".to_string()]);
        manager.create_test_case(test_case).await.unwrap();
        
        // Small delay to ensure different timestamps
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
    }

    // Get recent test cases
    let results = manager.get_recent_test_cases(3).await.unwrap();

    assert_eq!(results.len(), 3);
    // Should be sorted by updated_at descending, so most recent first
    assert_eq!(results[0].name, "Test 5");
    assert_eq!(results[1].name, "Test 4");
    assert_eq!(results[2].name, "Test 3");
}

#[tokio::test]
async fn test_get_statistics() {
    let (manager, _temp_dir) = create_test_manager().await;

    // Create test cases with different protocols and tags
    let test_cases = vec![
        create_sample_test_case("HTTP Test 1", "http", "GET", vec!["web".to_string(), "api".to_string()]),
        create_sample_test_case("HTTP Test 2", "http", "POST", vec!["web".to_string()]),
        create_sample_test_case("GraphQL Test", "graphql", "POST", vec!["api".to_string(), "graphql".to_string()]),
        create_sample_test_case("gRPC Test", "grpc", "CALL", vec!["api".to_string(), "grpc".to_string()]),
    ];

    for test_case in test_cases {
        manager.create_test_case(test_case).await.unwrap();
    }

    // Get statistics
    let stats = manager.get_test_case_statistics().await.unwrap();

    assert_eq!(stats.total_count, 4);
    assert_eq!(stats.protocols.get("http"), Some(&2));
    assert_eq!(stats.protocols.get("graphql"), Some(&1));
    assert_eq!(stats.protocols.get("grpc"), Some(&1));
    
    assert_eq!(stats.tags.get("web"), Some(&2));
    assert_eq!(stats.tags.get("api"), Some(&3));
    assert_eq!(stats.tags.get("graphql"), Some(&1));
    assert_eq!(stats.tags.get("grpc"), Some(&1));
    
    // All test cases were created recently
    assert_eq!(stats.created_this_week, 4);
    assert_eq!(stats.created_this_month, 4);
}

#[tokio::test]
async fn test_search_by_auth_presence() {
    let (manager, _temp_dir) = create_test_manager().await;

    // Create test cases with and without auth
    let mut test_case_with_auth = create_sample_test_case("Auth Test", "http", "GET", vec!["auth".to_string()]);
    test_case_with_auth.request.auth = Some(AuthDefinition {
        auth_type: "bearer".to_string(),
        config: {
            let mut config = HashMap::new();
            config.insert("token".to_string(), "test-token".to_string());
            config
        },
    });

    let test_case_without_auth = create_sample_test_case("No Auth Test", "http", "GET", vec!["public".to_string()]);

    manager.create_test_case(test_case_with_auth).await.unwrap();
    manager.create_test_case(test_case_without_auth).await.unwrap();

    // Search for test cases with auth
    let filter = TestCaseFilter {
        has_auth: Some(true),
        ..Default::default()
    };
    let options = SearchOptions::default();
    let results = manager.search_test_cases(&filter, &options).await.unwrap();

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].name, "Auth Test");

    // Search for test cases without auth
    let filter = TestCaseFilter {
        has_auth: Some(false),
        ..Default::default()
    };
    let results = manager.search_test_cases(&filter, &options).await.unwrap();

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].name, "No Auth Test");
}

#[tokio::test]
async fn test_version_tracking() {
    let (manager, _temp_dir) = create_test_manager().await;

    // Create a test case
    let mut test_case = create_sample_test_case("Version Test", "http", "GET", vec!["test".to_string()]);
    let id = manager.create_test_case(test_case.clone()).await.unwrap();

    // Verify initial version and change log
    let loaded_case = manager.read_test_case(&id).await.unwrap();
    assert_eq!(loaded_case.version, 1);
    assert_eq!(loaded_case.change_log.len(), 1);
    assert_eq!(loaded_case.change_log[0].change_type, ChangeType::Created);

    // Update the test case
    test_case.name = "Updated Version Test".to_string();
    test_case.tags.push("updated".to_string());
    manager.update_test_case(&id, test_case).await.unwrap();

    // Verify version increment and change log
    let updated_case = manager.read_test_case(&id).await.unwrap();
    assert_eq!(updated_case.version, 2);
    assert_eq!(updated_case.change_log.len(), 2);
    assert_eq!(updated_case.change_log[1].change_type, ChangeType::TagsModified);
    assert!(updated_case.change_log[1].changed_fields.contains(&"name".to_string()));
    assert!(updated_case.change_log[1].changed_fields.contains(&"tags".to_string()));
}

#[tokio::test]
async fn test_get_version_history() {
    let (manager, _temp_dir) = create_test_manager().await;

    // Create and update a test case multiple times
    let mut test_case = create_sample_test_case("History Test", "http", "GET", vec!["test".to_string()]);
    let id = manager.create_test_case(test_case.clone()).await.unwrap();

    // First update - change name
    test_case.name = "Updated History Test".to_string();
    manager.update_test_case(&id, test_case.clone()).await.unwrap();

    // Second update - change method
    test_case.request.method = "POST".to_string();
    manager.update_test_case(&id, test_case.clone()).await.unwrap();

    // Third update - change assertions
    test_case.assertions.push(AssertionDefinition {
        assertion_type: "response_time".to_string(),
        expected: serde_json::Value::Number(serde_json::Number::from(500)),
        message: Some("Should be fast".to_string()),
    });
    manager.update_test_case(&id, test_case).await.unwrap();

    // Get version history
    let history = manager.get_test_case_version_history(&id).await.unwrap();
    assert_eq!(history.current_version, 4);
    assert_eq!(history.total_changes, 4);
    assert_eq!(history.change_log.len(), 4);

    // Verify change types
    assert_eq!(history.change_log[0].change_type, ChangeType::Created);
    assert_eq!(history.change_log[1].change_type, ChangeType::MetadataModified);
    assert_eq!(history.change_log[2].change_type, ChangeType::RequestModified);
    assert_eq!(history.change_log[3].change_type, ChangeType::AssertionsModified);
}

#[tokio::test]
async fn test_search_by_change_type() {
    let (manager, _temp_dir) = create_test_manager().await;

    // Create test cases and update them with different change types
    let mut test_case1 = create_sample_test_case("Tags Test", "http", "GET", vec!["test".to_string()]);
    let id1 = manager.create_test_case(test_case1.clone()).await.unwrap();
    test_case1.tags.push("modified".to_string());
    manager.update_test_case(&id1, test_case1).await.unwrap();

    let mut test_case2 = create_sample_test_case("Request Test", "http", "GET", vec!["test".to_string()]);
    let id2 = manager.create_test_case(test_case2.clone()).await.unwrap();
    test_case2.request.method = "POST".to_string();
    manager.update_test_case(&id2, test_case2).await.unwrap();

    let test_case3 = create_sample_test_case("No Changes", "http", "GET", vec!["test".to_string()]);
    manager.create_test_case(test_case3).await.unwrap();

    // Search for test cases with tags modifications
    let results = manager.search_by_change_type(ChangeType::TagsModified).await.unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].name, "Tags Test");

    // Search for test cases with request modifications
    let results = manager.search_by_change_type(ChangeType::RequestModified).await.unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].name, "Request Test");
}

#[tokio::test]
async fn test_get_recently_modified() {
    let (manager, _temp_dir) = create_test_manager().await;

    // Create test cases
    let test_case1 = create_sample_test_case("Old Test", "http", "GET", vec!["old".to_string()]);
    let id1 = manager.create_test_case(test_case1).await.unwrap();

    let test_case2 = create_sample_test_case("Recent Test", "http", "GET", vec!["recent".to_string()]);
    let id2 = manager.create_test_case(test_case2).await.unwrap();

    // Update the recent test case
    let mut updated_case = manager.read_test_case(&id2).await.unwrap();
    updated_case.name = "Recently Modified Test".to_string();
    manager.update_test_case(&id2, updated_case).await.unwrap();

    // Get recently modified test cases (within 1 day)
    let results = manager.get_recently_modified_test_cases(1).await.unwrap();
    
    // Both should be included since they were created/modified recently
    assert_eq!(results.len(), 2);
    
    // The recently modified one should be first (sorted by updated_at desc)
    assert_eq!(results[0].name, "Recently Modified Test");
}

#[tokio::test]
async fn test_change_description_generation() {
    let (manager, _temp_dir) = create_test_manager().await;

    // Create a test case
    let mut test_case = create_sample_test_case("Description Test", "http", "GET", vec!["test".to_string()]);
    let id = manager.create_test_case(test_case.clone()).await.unwrap();

    // Update multiple fields
    test_case.name = "Updated Description Test".to_string();
    test_case.request.method = "POST".to_string();
    test_case.tags.push("updated".to_string());
    manager.update_test_case(&id, test_case).await.unwrap();

    // Check the generated description
    let updated_case = manager.read_test_case(&id).await.unwrap();
    let last_change = &updated_case.change_log[1];
    
    // Should contain information about the changed fields
    assert!(last_change.description.contains("name") || 
            last_change.description.contains("HTTP method") || 
            last_change.description.contains("tags"));
    assert!(last_change.changed_fields.contains(&"name".to_string()));
    assert!(last_change.changed_fields.contains(&"request.method".to_string()));
    assert!(last_change.changed_fields.contains(&"tags".to_string()));
}