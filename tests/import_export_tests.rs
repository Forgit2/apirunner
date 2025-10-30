use apirunner::import_export::*;
use apirunner::test_case_manager::{TestCase, RequestDefinition, AuthDefinition, ChangeLogEntry, ChangeType};
use std::collections::HashMap;
use std::time::Duration;
use uuid::Uuid;
use chrono::Utc;
use tempfile::NamedTempFile;
use std::io::Write;

fn create_test_case(name: &str, url: &str) -> TestCase {
    TestCase {
        id: Uuid::new_v4().to_string(),
        name: name.to_string(),
        description: Some("A test case".to_string()),
        tags: vec!["api".to_string(), "test".to_string()],
        protocol: "http".to_string(),
        request: RequestDefinition {
            protocol: "http".to_string(),
            method: "GET".to_string(),
            url: url.to_string(),
            headers: HashMap::new(),
            body: None,
            auth: None,
        },
        assertions: Vec::new(),
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
async fn test_postman_import() {
    let postman_collection = r#"
    {
        "info": {
            "name": "Test Collection",
            "description": "A test collection for import testing"
        },
        "item": [
            {
                "name": "Get Users",
                "request": {
                    "method": "GET",
                    "header": [
                        {
                            "key": "Authorization",
                            "value": "Bearer {{token}}"
                        }
                    ],
                    "url": {
                        "raw": "https://api.example.com/users",
                        "protocol": "https",
                        "host": ["api", "example", "com"],
                        "path": ["users"]
                    }
                }
            },
            {
                "name": "Create User",
                "request": {
                    "method": "POST",
                    "header": [
                        {
                            "key": "Content-Type",
                            "value": "application/json"
                        }
                    ],
                    "body": {
                        "mode": "raw",
                        "raw": "{\"name\": \"John Doe\", \"email\": \"john@example.com\"}"
                    },
                    "url": "https://api.example.com/users"
                }
            }
        ],
        "variable": [
            {
                "key": "token",
                "value": "abc123"
            }
        ]
    }
    "#;

    let manager = ImportExportManager::new();
    let options = ImportOptions::default();
    let source = ImportSource::Content(postman_collection.to_string());

    let result = manager.import_test_cases(&source, ImportFormat::PostmanCollection, &options).await;
    assert!(result.is_ok());

    let test_cases = result.unwrap();
    assert_eq!(test_cases.len(), 2);

    // Check first test case
    let get_users = &test_cases[0];
    assert_eq!(get_users.name, "Get Users");
    assert_eq!(get_users.request.method, "GET");
    assert_eq!(get_users.request.url, "https://api.example.com/users");
    assert!(get_users.request.headers.contains_key("Authorization"));

    // Check second test case
    let create_user = &test_cases[1];
    assert_eq!(create_user.name, "Create User");
    assert_eq!(create_user.request.method, "POST");
    assert!(create_user.request.body.is_some());
}

#[tokio::test]
async fn test_openapi_import() {
    let openapi_spec = r#"
    {
        "openapi": "3.0.0",
        "info": {
            "title": "Test API",
            "version": "1.0.0"
        },
        "servers": [
            {
                "url": "https://api.example.com"
            }
        ],
        "paths": {
            "/users": {
                "get": {
                    "summary": "Get all users",
                    "operationId": "getUsers",
                    "responses": {
                        "200": {
                            "description": "Success"
                        }
                    }
                },
                "post": {
                    "summary": "Create a user",
                    "operationId": "createUser",
                    "requestBody": {
                        "content": {
                            "application/json": {
                                "schema": {
                                    "type": "object",
                                    "properties": {
                                        "name": {"type": "string"},
                                        "email": {"type": "string"}
                                    }
                                }
                            }
                        }
                    },
                    "responses": {
                        "201": {
                            "description": "Created"
                        }
                    }
                }
            }
        }
    }
    "#;

    let manager = ImportExportManager::new();
    let options = ImportOptions::default();
    let source = ImportSource::Content(openapi_spec.to_string());

    let result = manager.import_test_cases(&source, ImportFormat::OpenApiSpec, &options).await;
    assert!(result.is_ok());

    let test_cases = result.unwrap();
    assert_eq!(test_cases.len(), 2);

    // Check GET operation
    let get_users = test_cases.iter().find(|tc| tc.request.method == "GET").unwrap();
    assert_eq!(get_users.name, "getUsers");
    assert_eq!(get_users.request.url, "https://api.example.com/users");

    // Check POST operation
    let create_user = test_cases.iter().find(|tc| tc.request.method == "POST").unwrap();
    assert_eq!(create_user.name, "createUser");
    assert!(create_user.request.body.is_some());
}

#[tokio::test]
async fn test_json_export() {
    let test_cases = vec![create_test_case("Test Case 1", "https://api.example.com/test")];

    let manager = ImportExportManager::new();
    let options = ExportOptions::default();

    let result = manager.export_test_cases(&test_cases, ExportFormat::Json, &options).await;
    assert!(result.is_ok());

    let json_output = result.unwrap();
    assert!(json_output.contains("Test Case 1"));
    assert!(json_output.contains("https://api.example.com/test"));

    // Verify it's valid JSON
    let parsed: serde_json::Value = serde_json::from_str(&json_output).unwrap();
    assert!(parsed.is_object());
}

#[tokio::test]
async fn test_yaml_export() {
    let test_cases = vec![create_test_case("Test Case 1", "https://api.example.com/test")];

    let manager = ImportExportManager::new();
    let options = ExportOptions::default();

    let result = manager.export_test_cases(&test_cases, ExportFormat::Yaml, &options).await;
    assert!(result.is_ok());

    let yaml_output = result.unwrap();
    assert!(yaml_output.contains("Test Case 1"));
    assert!(yaml_output.contains("https://api.example.com/test"));

    // Verify it's valid YAML
    let parsed: serde_yaml::Value = serde_yaml::from_str(&yaml_output).unwrap();
    assert!(parsed.is_mapping());
}

#[tokio::test]
async fn test_postman_export() {
    let mut test_case = create_test_case("Test Case 1", "https://api.example.com/test");
    test_case.tags.push("folder:API Tests".to_string());
    test_case.request.headers.insert("Authorization".to_string(), "Bearer token123".to_string());
    test_case.request.auth = Some(AuthDefinition {
        auth_type: "bearer".to_string(),
        config: {
            let mut config = HashMap::new();
            config.insert("token".to_string(), "token123".to_string());
            config
        },
    });

    let test_cases = vec![test_case];

    let manager = ImportExportManager::new();
    let options = ExportOptions::default();

    let result = manager.export_test_cases(&test_cases, ExportFormat::PostmanCollection, &options).await;
    assert!(result.is_ok());

    let postman_output = result.unwrap();
    assert!(postman_output.contains("Test Case 1"));
    assert!(postman_output.contains("https://api.example.com/test"));
    assert!(postman_output.contains("API Tests"));

    // Verify it's valid JSON
    let parsed: serde_json::Value = serde_json::from_str(&postman_output).unwrap();
    assert!(parsed.is_object());
    assert!(parsed["info"].is_object());
    assert!(parsed["item"].is_array());
}

#[tokio::test]
async fn test_import_from_file() {
    let postman_collection = r#"
    {
        "info": {
            "name": "File Test Collection"
        },
        "item": [
            {
                "name": "Test Request",
                "request": {
                    "method": "GET",
                    "url": "https://api.example.com/test"
                }
            }
        ]
    }
    "#;

    // Create a temporary file with .json extension
    let mut temp_file = NamedTempFile::with_suffix(".json").unwrap();
    temp_file.write_all(postman_collection.as_bytes()).unwrap();
    temp_file.flush().unwrap();

    let manager = ImportExportManager::new();
    let options = ImportOptions::default();
    let source = ImportSource::File(temp_file.path().to_path_buf());

    let result = manager.import_test_cases(&source, ImportFormat::PostmanCollection, &options).await;
    if let Err(e) = &result {
        println!("Import error: {:?}", e);
    }
    assert!(result.is_ok());

    let test_cases = result.unwrap();
    assert_eq!(test_cases.len(), 1);
    assert_eq!(test_cases[0].name, "Test Request");
}

#[tokio::test]
async fn test_import_with_options() {
    let postman_collection = r#"
    {
        "info": {
            "name": "Options Test Collection"
        },
        "item": [
            {
                "name": "Test Request",
                "request": {
                    "method": "GET",
                    "url": "https://api.example.com/test"
                }
            }
        ]
    }
    "#;

    let manager = ImportExportManager::new();
    let options = ImportOptions {
        generate_assertions: false,
        include_examples: false,
        base_url_override: Some("https://staging.example.com".to_string()),
        tag_prefix: Some("imported".to_string()),
    };
    let source = ImportSource::Content(postman_collection.to_string());

    let result = manager.import_test_cases(&source, ImportFormat::PostmanCollection, &options).await;
    assert!(result.is_ok());

    let test_cases = result.unwrap();
    assert_eq!(test_cases.len(), 1);
    
    let test_case = &test_cases[0];
    assert!(test_case.assertions.is_empty()); // No assertions generated
    assert!(test_case.tags.contains(&"imported:postman".to_string())); // Tag prefix applied
}

#[tokio::test]
async fn test_supported_formats() {
    let manager = ImportExportManager::new();
    
    let import_formats = manager.get_supported_import_formats();
    assert!(import_formats.contains(&ImportFormat::PostmanCollection));
    assert!(import_formats.contains(&ImportFormat::OpenApiSpec));
    
    let export_formats = manager.get_supported_export_formats();
    assert!(export_formats.contains(&ExportFormat::Json));
    assert!(export_formats.contains(&ExportFormat::Yaml));
    assert!(export_formats.contains(&ExportFormat::PostmanCollection));
}