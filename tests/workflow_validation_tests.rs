use apirunner::workflow_validation::{
    WorkflowValidation, WorkflowStep, WorkflowAssertion, StateExtraction, ExtractionType,
};
use apirunner::{
    Plugin, PluginConfig, AssertionPlugin, AssertionType, ExpectedValue, ResponseData,
    ProtocolRequest, HttpMethod,
};
use serde_json::json;
use std::collections::HashMap;
use std::time::Duration;

#[tokio::test]
async fn test_workflow_validation_initialization() {
    let steps = vec![
        WorkflowStep {
            id: "step1".to_string(),
            name: "Test Step".to_string(),
            request: ProtocolRequest {
                method: HttpMethod::GET,
                url: "/test".to_string(),
                headers: HashMap::new(),
                body: None,
                timeout: Duration::from_secs(30),
                protocol_version: None,
                tls_config: None,
            },
            assertions: vec![],
            state_extractions: vec![],
            rollback_request: None,
            depends_on: vec![],
        }
    ];

    let mut validation = WorkflowValidation::new(steps.clone());

    let config = PluginConfig {
        settings: HashMap::from([
            ("workflow_steps".to_string(), json!(steps)),
            ("rollback_on_failure".to_string(), json!(true)),
        ]),
        environment: "test".to_string(),
    };

    let result = validation.initialize(&config).await;
    assert!(result.is_ok());
    assert_eq!(validation.assertion_type(), AssertionType::Custom("workflow-validation".to_string()));
    assert_eq!(validation.priority(), 30);
}

#[tokio::test]
async fn test_workflow_validation_simple_sequence() {
    let steps = vec![
        WorkflowStep {
            id: "create_user".to_string(),
            name: "Create User".to_string(),
            request: ProtocolRequest {
                method: HttpMethod::POST,
                url: "/users".to_string(),
                headers: HashMap::from([
                    ("Content-Type".to_string(), "application/json".to_string())
                ]),
                body: Some(r#"{"name": "Test User", "email": "test@example.com"}"#.as_bytes().to_vec()),
                timeout: Duration::from_secs(30),
                protocol_version: None,
                tls_config: None,
            },
            assertions: vec![
                WorkflowAssertion {
                    assertion_type: "status_code".to_string(),
                    expected_value: ExpectedValue::Exact(json!(201)),
                    description: "User created successfully".to_string(),
                }
            ],
            state_extractions: vec![
                StateExtraction {
                    variable_name: "user_id".to_string(),
                    extraction_type: ExtractionType::JsonPath,
                    source_path: "$.id".to_string(),
                }
            ],
            rollback_request: Some(ProtocolRequest {
                method: HttpMethod::DELETE,
                url: "/users/${user_id}".to_string(),
                headers: HashMap::new(),
                body: None,
                timeout: Duration::from_secs(30),
                protocol_version: None,
                tls_config: None,
            }),
            depends_on: vec![],
        },
        WorkflowStep {
            id: "get_user".to_string(),
            name: "Get Created User".to_string(),
            request: ProtocolRequest {
                method: HttpMethod::GET,
                url: "/users/${user_id}".to_string(),
                headers: HashMap::new(),
                body: None,
                timeout: Duration::from_secs(30),
                protocol_version: None,
                tls_config: None,
            },
            assertions: vec![
                WorkflowAssertion {
                    assertion_type: "status_code".to_string(),
                    expected_value: ExpectedValue::Exact(json!(200)),
                    description: "User retrieved successfully".to_string(),
                }
            ],
            state_extractions: vec![],
            rollback_request: None,
            depends_on: vec!["create_user".to_string()],
        },
    ];

    let validation = WorkflowValidation::new(steps);

    let response_data = ResponseData {
        status_code: 200,
        headers: HashMap::new(),
        body: Vec::new(),
        duration: Duration::from_millis(100),
        metadata: HashMap::new(),
    };

    let expected = ExpectedValue::NotEmpty;
    let result = validation.execute(&response_data, &expected).await;

    assert!(result.success);
    assert!(result.message.contains("Workflow completed successfully"));
}

#[tokio::test]
async fn test_workflow_validation_with_state_propagation() {
    let steps = vec![
        WorkflowStep {
            id: "login".to_string(),
            name: "User Login".to_string(),
            request: ProtocolRequest {
                method: HttpMethod::POST,
                url: "/auth/login".to_string(),
                headers: HashMap::from([
                    ("Content-Type".to_string(), "application/json".to_string())
                ]),
                body: Some(r#"{"username": "testuser", "password": "password123"}"#.as_bytes().to_vec()),
                timeout: Duration::from_secs(30),
                protocol_version: None,
                tls_config: None,
            },
            assertions: vec![
                WorkflowAssertion {
                    assertion_type: "status_code".to_string(),
                    expected_value: ExpectedValue::Exact(json!(200)),
                    description: "Login successful".to_string(),
                }
            ],
            state_extractions: vec![
                StateExtraction {
                    variable_name: "auth_token".to_string(),
                    extraction_type: ExtractionType::JsonPath,
                    source_path: "$.token".to_string(),
                },
                StateExtraction {
                    variable_name: "user_id".to_string(),
                    extraction_type: ExtractionType::JsonPath,
                    source_path: "$.user_id".to_string(),
                }
            ],
            rollback_request: None,
            depends_on: vec![],
        },
        WorkflowStep {
            id: "get_profile".to_string(),
            name: "Get User Profile".to_string(),
            request: ProtocolRequest {
                method: HttpMethod::GET,
                url: "/users/${user_id}/profile".to_string(),
                headers: HashMap::from([
                    ("Authorization".to_string(), "Bearer ${auth_token}".to_string())
                ]),
                body: None,
                timeout: Duration::from_secs(30),
                protocol_version: None,
                tls_config: None,
            },
            assertions: vec![
                WorkflowAssertion {
                    assertion_type: "status_code".to_string(),
                    expected_value: ExpectedValue::Exact(json!(200)),
                    description: "Profile retrieved successfully".to_string(),
                }
            ],
            state_extractions: vec![],
            rollback_request: None,
            depends_on: vec!["login".to_string()],
        },
    ];

    let validation = WorkflowValidation::new(steps);

    let response_data = ResponseData {
        status_code: 200,
        headers: HashMap::new(),
        body: Vec::new(),
        duration: Duration::from_millis(150),
        metadata: HashMap::new(),
    };

    let expected = ExpectedValue::NotEmpty;
    let result = validation.execute(&response_data, &expected).await;

    assert!(result.success);
    assert!(result.message.contains("Workflow completed successfully"));
}

#[tokio::test]
async fn test_workflow_validation_complex_dependencies() {
    let steps = vec![
        WorkflowStep {
            id: "setup_data".to_string(),
            name: "Setup Test Data".to_string(),
            request: ProtocolRequest {
                method: HttpMethod::POST,
                url: "/setup".to_string(),
                headers: HashMap::new(),
                body: None,
                timeout: Duration::from_secs(30),
                protocol_version: None,
                tls_config: None,
            },
            assertions: vec![
                WorkflowAssertion {
                    assertion_type: "status_code".to_string(),
                    expected_value: ExpectedValue::Exact(json!(201)),
                    description: "Setup completed".to_string(),
                }
            ],
            state_extractions: vec![
                StateExtraction {
                    variable_name: "setup_id".to_string(),
                    extraction_type: ExtractionType::JsonPath,
                    source_path: "$.id".to_string(),
                }
            ],
            rollback_request: Some(ProtocolRequest {
                method: HttpMethod::DELETE,
                url: "/setup/${setup_id}".to_string(),
                headers: HashMap::new(),
                body: None,
                timeout: Duration::from_secs(30),
                protocol_version: None,
                tls_config: None,
            }),
            depends_on: vec![],
        },
        WorkflowStep {
            id: "create_resource_a".to_string(),
            name: "Create Resource A".to_string(),
            request: ProtocolRequest {
                method: HttpMethod::POST,
                url: "/resources/a".to_string(),
                headers: HashMap::new(),
                body: Some(r#"{"setup_id": "${setup_id}"}"#.as_bytes().to_vec()),
                timeout: Duration::from_secs(30),
                protocol_version: None,
                tls_config: None,
            },
            assertions: vec![
                WorkflowAssertion {
                    assertion_type: "status_code".to_string(),
                    expected_value: ExpectedValue::Exact(json!(201)),
                    description: "Resource A created".to_string(),
                }
            ],
            state_extractions: vec![
                StateExtraction {
                    variable_name: "resource_a_id".to_string(),
                    extraction_type: ExtractionType::JsonPath,
                    source_path: "$.id".to_string(),
                }
            ],
            rollback_request: Some(ProtocolRequest {
                method: HttpMethod::DELETE,
                url: "/resources/a/${resource_a_id}".to_string(),
                headers: HashMap::new(),
                body: None,
                timeout: Duration::from_secs(30),
                protocol_version: None,
                tls_config: None,
            }),
            depends_on: vec!["setup_data".to_string()],
        },
        WorkflowStep {
            id: "create_resource_b".to_string(),
            name: "Create Resource B".to_string(),
            request: ProtocolRequest {
                method: HttpMethod::POST,
                url: "/resources/b".to_string(),
                headers: HashMap::new(),
                body: Some(r#"{"setup_id": "${setup_id}"}"#.as_bytes().to_vec()),
                timeout: Duration::from_secs(30),
                protocol_version: None,
                tls_config: None,
            },
            assertions: vec![
                WorkflowAssertion {
                    assertion_type: "status_code".to_string(),
                    expected_value: ExpectedValue::Exact(json!(201)),
                    description: "Resource B created".to_string(),
                }
            ],
            state_extractions: vec![
                StateExtraction {
                    variable_name: "resource_b_id".to_string(),
                    extraction_type: ExtractionType::JsonPath,
                    source_path: "$.id".to_string(),
                }
            ],
            rollback_request: Some(ProtocolRequest {
                method: HttpMethod::DELETE,
                url: "/resources/b/${resource_b_id}".to_string(),
                headers: HashMap::new(),
                body: None,
                timeout: Duration::from_secs(30),
                protocol_version: None,
                tls_config: None,
            }),
            depends_on: vec!["setup_data".to_string()],
        },
        WorkflowStep {
            id: "link_resources".to_string(),
            name: "Link Resources".to_string(),
            request: ProtocolRequest {
                method: HttpMethod::POST,
                url: "/links".to_string(),
                headers: HashMap::new(),
                body: Some(r#"{"resource_a": "${resource_a_id}", "resource_b": "${resource_b_id}"}"#.as_bytes().to_vec()),
                timeout: Duration::from_secs(30),
                protocol_version: None,
                tls_config: None,
            },
            assertions: vec![
                WorkflowAssertion {
                    assertion_type: "status_code".to_string(),
                    expected_value: ExpectedValue::Exact(json!(201)),
                    description: "Resources linked successfully".to_string(),
                }
            ],
            state_extractions: vec![],
            rollback_request: None,
            depends_on: vec!["create_resource_a".to_string(), "create_resource_b".to_string()],
        },
    ];

    let validation = WorkflowValidation::new(steps);

    let response_data = ResponseData {
        status_code: 200,
        headers: HashMap::new(),
        body: Vec::new(),
        duration: Duration::from_millis(200),
        metadata: HashMap::new(),
    };

    let expected = ExpectedValue::NotEmpty;
    let result = validation.execute(&response_data, &expected).await;

    assert!(result.success);
    assert!(result.message.contains("Workflow completed successfully"));
}

#[tokio::test]
async fn test_workflow_validation_circular_dependency_detection() {
    let steps = vec![
        WorkflowStep {
            id: "step_a".to_string(),
            name: "Step A".to_string(),
            request: ProtocolRequest {
                method: HttpMethod::GET,
                url: "/a".to_string(),
                headers: HashMap::new(),
                body: None,
                timeout: Duration::from_secs(30),
                protocol_version: None,
                tls_config: None,
            },
            assertions: vec![],
            state_extractions: vec![],
            rollback_request: None,
            depends_on: vec!["step_b".to_string()],
        },
        WorkflowStep {
            id: "step_b".to_string(),
            name: "Step B".to_string(),
            request: ProtocolRequest {
                method: HttpMethod::GET,
                url: "/b".to_string(),
                headers: HashMap::new(),
                body: None,
                timeout: Duration::from_secs(30),
                protocol_version: None,
                tls_config: None,
            },
            assertions: vec![],
            state_extractions: vec![],
            rollback_request: None,
            depends_on: vec!["step_a".to_string()],
        },
    ];

    let validation = WorkflowValidation::new(steps);

    let response_data = ResponseData {
        status_code: 200,
        headers: HashMap::new(),
        body: Vec::new(),
        duration: Duration::from_millis(100),
        metadata: HashMap::new(),
    };

    let expected = ExpectedValue::NotEmpty;
    let result = validation.execute(&response_data, &expected).await;

    assert!(!result.success);
    assert!(result.message.contains("Circular dependency detected"));
}

#[tokio::test]
async fn test_workflow_validation_rollback_on_failure() {
    let steps = vec![
        WorkflowStep {
            id: "step1".to_string(),
            name: "Step 1".to_string(),
            request: ProtocolRequest {
                method: HttpMethod::POST,
                url: "/step1".to_string(),
                headers: HashMap::new(),
                body: None,
                timeout: Duration::from_secs(30),
                protocol_version: None,
                tls_config: None,
            },
            assertions: vec![
                WorkflowAssertion {
                    assertion_type: "status_code".to_string(),
                    expected_value: ExpectedValue::Exact(json!(201)),
                    description: "Step 1 success".to_string(),
                }
            ],
            state_extractions: vec![],
            rollback_request: Some(ProtocolRequest {
                method: HttpMethod::DELETE,
                url: "/step1/rollback".to_string(),
                headers: HashMap::new(),
                body: None,
                timeout: Duration::from_secs(30),
                protocol_version: None,
                tls_config: None,
            }),
            depends_on: vec![],
        },
        WorkflowStep {
            id: "step2".to_string(),
            name: "Step 2 (Failing)".to_string(),
            request: ProtocolRequest {
                method: HttpMethod::POST,
                url: "/step2".to_string(),
                headers: HashMap::new(),
                body: None,
                timeout: Duration::from_secs(30),
                protocol_version: None,
                tls_config: None,
            },
            assertions: vec![
                WorkflowAssertion {
                    assertion_type: "status_code".to_string(),
                    expected_value: ExpectedValue::Exact(json!(500)), // This will fail since we simulate 201
                    description: "Step 2 should fail".to_string(),
                }
            ],
            state_extractions: vec![],
            rollback_request: None,
            depends_on: vec!["step1".to_string()],
        },
    ];

    let validation = WorkflowValidation::new(steps).with_rollback(true);

    let response_data = ResponseData {
        status_code: 200,
        headers: HashMap::new(),
        body: Vec::new(),
        duration: Duration::from_millis(100),
        metadata: HashMap::new(),
    };

    let expected = ExpectedValue::NotEmpty;
    let result = validation.execute(&response_data, &expected).await;

    assert!(!result.success);
    assert!(result.message.contains("failed"));
}

#[tokio::test]
async fn test_workflow_validation_state_extraction_types() {
    let steps = vec![
        WorkflowStep {
            id: "test_extractions".to_string(),
            name: "Test All Extraction Types".to_string(),
            request: ProtocolRequest {
                method: HttpMethod::GET,
                url: "/test".to_string(),
                headers: HashMap::from([
                    ("X-Custom-Header".to_string(), "custom-value".to_string())
                ]),
                body: None,
                timeout: Duration::from_secs(30),
                protocol_version: None,
                tls_config: None,
            },
            assertions: vec![
                WorkflowAssertion {
                    assertion_type: "status_code".to_string(),
                    expected_value: ExpectedValue::Exact(json!(200)),
                    description: "Request successful".to_string(),
                }
            ],
            state_extractions: vec![
                StateExtraction {
                    variable_name: "response_status".to_string(),
                    extraction_type: ExtractionType::StatusCode,
                    source_path: "".to_string(),
                },
                StateExtraction {
                    variable_name: "response_time".to_string(),
                    extraction_type: ExtractionType::ResponseTime,
                    source_path: "".to_string(),
                },
                StateExtraction {
                    variable_name: "content_type".to_string(),
                    extraction_type: ExtractionType::Header,
                    source_path: "Content-Type".to_string(),
                },
                StateExtraction {
                    variable_name: "data_count".to_string(),
                    extraction_type: ExtractionType::JsonPath,
                    source_path: "$.data.length".to_string(),
                }
            ],
            rollback_request: None,
            depends_on: vec![],
        },
    ];

    let validation = WorkflowValidation::new(steps);

    let response_data = ResponseData {
        status_code: 200,
        headers: HashMap::new(),
        body: Vec::new(),
        duration: Duration::from_millis(100),
        metadata: HashMap::new(),
    };

    let expected = ExpectedValue::NotEmpty;
    let result = validation.execute(&response_data, &expected).await;

    assert!(result.success);
    assert!(result.message.contains("Workflow completed successfully"));
}

#[tokio::test]
async fn test_workflow_validation_empty_workflow() {
    let steps = vec![];

    let mut validation = WorkflowValidation::new(steps);

    let config = PluginConfig {
        settings: HashMap::new(),
        environment: "test".to_string(),
    };

    let result = validation.initialize(&config).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("At least one workflow step is required"));
}