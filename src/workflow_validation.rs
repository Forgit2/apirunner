use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{Value as JsonValue};
use std::collections::HashMap;
use std::time::Duration;

use crate::error::PluginError;
use crate::plugin::{
    AssertionPlugin, AssertionResult, AssertionType, ExpectedValue, Plugin, PluginConfig,
    ResponseData, ProtocolRequest, HttpMethod,
};

/// Workflow step definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowStep {
    pub id: String,
    pub name: String,
    pub request: ProtocolRequest,
    pub assertions: Vec<WorkflowAssertion>,
    pub state_extractions: Vec<StateExtraction>,
    pub rollback_request: Option<ProtocolRequest>,
    pub depends_on: Vec<String>,
}

/// Workflow assertion definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowAssertion {
    pub assertion_type: String,
    pub expected_value: ExpectedValue,
    pub description: String,
}

/// State extraction configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateExtraction {
    pub variable_name: String,
    pub extraction_type: ExtractionType,
    pub source_path: String,
}

/// Types of state extraction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExtractionType {
    JsonPath,
    Header,
    StatusCode,
    ResponseTime,
}

/// Workflow execution state
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WorkflowState {
    pub variables: HashMap<String, JsonValue>,
    pub step_results: HashMap<String, StepResult>,
    pub execution_order: Vec<String>,
}

/// Result of a workflow step execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepResult {
    pub step_id: String,
    pub success: bool,
    pub response: Option<ResponseData>,
    pub assertions: Vec<AssertionResult>,
    pub extracted_state: HashMap<String, JsonValue>,
    pub execution_time: Duration,
    pub error: Option<String>,
}

/// Workflow Validation Plugin for multi-step API call sequences
#[derive(Debug, Default)]
pub struct WorkflowValidation {
    workflow_steps: Vec<WorkflowStep>,
    rollback_on_failure: bool,
    timeout: Duration,
    state: WorkflowState,
}

impl WorkflowValidation {
    pub fn new(steps: Vec<WorkflowStep>) -> Self {
        Self {
            workflow_steps: steps,
            rollback_on_failure: true,
            timeout: Duration::from_secs(300), // 5 minutes default
            state: WorkflowState {
                variables: HashMap::new(),
                step_results: HashMap::new(),
                execution_order: Vec::new(),
            },
        }
    }

    pub fn with_rollback(mut self, rollback: bool) -> Self {
        self.rollback_on_failure = rollback;
        self
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Execute the entire workflow
    pub async fn execute_workflow(&mut self) -> Result<WorkflowState, String> {
        // Determine execution order based on dependencies
        let execution_order = self.resolve_execution_order()?;
        
        for step_id in execution_order {
            let step = self.find_step(&step_id)?;
            
            // Execute the step
            match self.execute_step(&step).await {
                Ok(result) => {
                    self.state.step_results.insert(step_id.clone(), result.clone());
                    self.state.execution_order.push(step_id.clone());
                    
                    // Extract state variables
                    for (var_name, var_value) in result.extracted_state {
                        self.state.variables.insert(var_name, var_value);
                    }
                    
                    // Check if step failed
                    if !result.success {
                        if self.rollback_on_failure {
                            self.rollback_workflow().await?;
                        }
                        return Err(format!("Workflow step '{}' failed: {}", step_id, 
                            result.error.unwrap_or("Unknown error".to_string())));
                    }
                }
                Err(error) => {
                    if self.rollback_on_failure {
                        self.rollback_workflow().await?;
                    }
                    return Err(format!("Failed to execute step '{}': {}", step_id, error));
                }
            }
        }
        
        Ok(self.state.clone())
    }

    /// Resolve execution order based on step dependencies
    fn resolve_execution_order(&self) -> Result<Vec<String>, String> {
        let mut order = Vec::new();
        let mut visited = HashMap::new();
        let mut visiting = HashMap::new();
        
        for step in &self.workflow_steps {
            if !visited.contains_key(&step.id) {
                self.visit_step(&step.id, &mut order, &mut visited, &mut visiting)?;
            }
        }
        
        Ok(order)
    }

    /// Depth-first search for topological sorting
    fn visit_step(
        &self,
        step_id: &str,
        order: &mut Vec<String>,
        visited: &mut HashMap<String, bool>,
        visiting: &mut HashMap<String, bool>,
    ) -> Result<(), String> {
        if visiting.contains_key(step_id) {
            return Err(format!("Circular dependency detected involving step '{}'", step_id));
        }
        
        if visited.contains_key(step_id) {
            return Ok(());
        }
        
        visiting.insert(step_id.to_string(), true);
        
        let step = self.find_step(step_id)?;
        for dep in &step.depends_on {
            self.visit_step(dep, order, visited, visiting)?;
        }
        
        visiting.remove(step_id);
        visited.insert(step_id.to_string(), true);
        order.push(step_id.to_string());
        
        Ok(())
    }

    /// Find a workflow step by ID
    fn find_step(&self, step_id: &str) -> Result<WorkflowStep, String> {
        self.workflow_steps
            .iter()
            .find(|step| step.id == step_id)
            .cloned()
            .ok_or_else(|| format!("Step '{}' not found", step_id))
    }

    /// Execute a single workflow step
    async fn execute_step(&self, step: &WorkflowStep) -> Result<StepResult, String> {
        let start_time = std::time::Instant::now();
        
        // Substitute variables in the request
        let mut request = step.request.clone();
        self.substitute_variables(&mut request)?;
        
        // Execute the request (simplified - in production use actual HTTP client)
        let response = self.simulate_request_execution(&request).await?;
        
        // Extract state variables
        let extracted_state = self.extract_state_variables(&response, &step.state_extractions)?;
        
        // Execute assertions
        let mut assertion_results = Vec::new();
        let mut all_assertions_passed = true;
        
        for assertion in &step.assertions {
            let result = self.execute_assertion(&response, assertion).await;
            if !result.success {
                all_assertions_passed = false;
            }
            assertion_results.push(result);
        }
        
        let execution_time = start_time.elapsed();
        
        Ok(StepResult {
            step_id: step.id.clone(),
            success: all_assertions_passed,
            response: Some(response),
            assertions: assertion_results,
            extracted_state,
            execution_time,
            error: if all_assertions_passed { None } else { Some("One or more assertions failed".to_string()) },
        })
    }

    /// Substitute workflow variables in the request
    fn substitute_variables(&self, request: &mut ProtocolRequest) -> Result<(), String> {
        // Substitute in URL
        request.url = self.substitute_string_variables(&request.url)?;
        
        // Substitute in headers
        for (_, value) in request.headers.iter_mut() {
            *value = self.substitute_string_variables(value)?;
        }
        
        // Substitute in body if it's JSON
        if let Some(body) = &request.body {
            if let Ok(body_str) = std::str::from_utf8(body) {
                let substituted = self.substitute_string_variables(body_str)?;
                request.body = Some(substituted.into_bytes());
            }
        }
        
        Ok(())
    }

    /// Substitute variables in a string
    fn substitute_string_variables(&self, input: &str) -> Result<String, String> {
        let mut result = input.to_string();
        
        // Simple variable substitution: ${variable_name}
        for (var_name, var_value) in &self.state.variables {
            let placeholder = format!("${{{}}}", var_name);
            let replacement = match var_value {
                JsonValue::String(s) => s.clone(),
                JsonValue::Number(n) => n.to_string(),
                JsonValue::Bool(b) => b.to_string(),
                _ => serde_json::to_string(var_value).unwrap_or_default(),
            };
            result = result.replace(&placeholder, &replacement);
        }
        
        Ok(result)
    }

    /// Simulate request execution (in production, use actual HTTP client)
    async fn simulate_request_execution(&self, request: &ProtocolRequest) -> Result<ResponseData, String> {
        // This is a simplified simulation
        // In production, use the actual protocol plugin to execute the request
        
        let mut headers = HashMap::new();
        headers.insert("Content-Type".to_string(), "application/json".to_string());
        
        // Simulate different responses based on URL patterns
        let (status_code, body) = if request.url.contains("/auth/login") {
            // Login endpoint - return auth token and user ID
            (200, serde_json::to_vec(&serde_json::json!({
                "token": "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9...",
                "user_id": 12345,
                "status": "success"
            })).unwrap_or_default())
        } else if request.url.contains("/users/") && request.url.contains("/profile") {
            // User profile endpoint
            (200, serde_json::to_vec(&serde_json::json!({
                "id": 12345,
                "name": "Test User",
                "email": "test@example.com",
                "profile": {
                    "bio": "Test user profile",
                    "avatar": "https://example.com/avatar.jpg"
                }
            })).unwrap_or_default())
        } else {
            // Default responses based on method
            match request.method {
                HttpMethod::POST | HttpMethod::PUT => {
                    (201, serde_json::to_vec(&serde_json::json!({
                        "id": 123,
                        "status": "success",
                        "created_at": "2024-01-01T12:00:00Z"
                    })).unwrap_or_default())
                }
                HttpMethod::GET => {
                    (200, serde_json::to_vec(&serde_json::json!({
                        "data": [
                            {"id": 1, "name": "Item 1"},
                            {"id": 2, "name": "Item 2"}
                        ],
                        "total": 2
                    })).unwrap_or_default())
                }
                HttpMethod::DELETE => (204, Vec::new()),
                _ => (200, serde_json::to_vec(&serde_json::json!({
                    "status": "success"
                })).unwrap_or_default()),
            }
        };
        
        Ok(ResponseData {
            status_code,
            headers,
            body,
            duration: Duration::from_millis(100),
            metadata: HashMap::new(),
        })
    }

    /// Extract state variables from response
    fn extract_state_variables(
        &self,
        response: &ResponseData,
        extractions: &[StateExtraction],
    ) -> Result<HashMap<String, JsonValue>, String> {
        let mut extracted = HashMap::new();
        
        for extraction in extractions {
            let value = match extraction.extraction_type {
                ExtractionType::JsonPath => {
                    self.extract_from_json_path(response, &extraction.source_path)?
                }
                ExtractionType::Header => {
                    response.headers.get(&extraction.source_path)
                        .map(|v| JsonValue::String(v.clone()))
                        .unwrap_or(JsonValue::Null)
                }
                ExtractionType::StatusCode => {
                    JsonValue::Number(serde_json::Number::from(response.status_code))
                }
                ExtractionType::ResponseTime => {
                    JsonValue::Number(serde_json::Number::from(response.duration.as_millis() as u64))
                }
            };
            
            extracted.insert(extraction.variable_name.clone(), value);
        }
        
        Ok(extracted)
    }

    /// Extract value using JSONPath
    fn extract_from_json_path(&self, response: &ResponseData, path: &str) -> Result<JsonValue, String> {
        let body_str = std::str::from_utf8(&response.body)
            .map_err(|_| "Response body is not valid UTF-8".to_string())?;
        
        let json_data: JsonValue = serde_json::from_str(body_str)
            .map_err(|e| format!("Failed to parse JSON: {}", e))?;
        
        // Simplified JSONPath extraction - support more common patterns
        if path.starts_with("$.") {
            let field_name = &path[2..]; // Remove "$."
            
            // Handle nested paths like "$.data.length"
            if field_name == "data.length" {
                if let Some(JsonValue::Array(arr)) = json_data.get("data") {
                    return Ok(JsonValue::Number(serde_json::Number::from(arr.len())));
                } else {
                    return Ok(JsonValue::Null);
                }
            }
            
            // Handle simple field access
            Ok(json_data.get(field_name).cloned().unwrap_or(JsonValue::Null))
        } else {
            Ok(JsonValue::Null)
        }
    }

    /// Execute a workflow assertion
    async fn execute_assertion(&self, response: &ResponseData, assertion: &WorkflowAssertion) -> AssertionResult {
        match assertion.assertion_type.as_str() {
            "status_code" => {
                let actual = JsonValue::Number(serde_json::Number::from(response.status_code));
                self.compare_values(&actual, &assertion.expected_value, &assertion.description)
            }
            "json_path" => {
                // For simplicity, assume the description contains the JSONPath
                match self.extract_from_json_path(response, &assertion.description) {
                    Ok(actual) => self.compare_values(&actual, &assertion.expected_value, &assertion.description),
                    Err(error) => AssertionResult {
                        success: false,
                        message: error,
                        actual_value: None,
                        expected_value: Some(serde_json::to_value(&assertion.expected_value).unwrap_or(JsonValue::Null)),
                    }
                }
            }
            _ => AssertionResult {
                success: false,
                message: format!("Unsupported assertion type: {}", assertion.assertion_type),
                actual_value: None,
                expected_value: Some(serde_json::to_value(&assertion.expected_value).unwrap_or(JsonValue::Null)),
            }
        }
    }

    /// Compare values for assertions
    fn compare_values(&self, actual: &JsonValue, expected: &ExpectedValue, description: &str) -> AssertionResult {
        match expected {
            ExpectedValue::Exact(expected_val) => {
                let success = actual == expected_val;
                AssertionResult {
                    success,
                    message: if success {
                        format!("Assertion '{}' passed: {} matches expected value", description, actual)
                    } else {
                        format!("Assertion '{}' failed: expected {}, got {}", description, expected_val, actual)
                    },
                    actual_value: Some(actual.clone()),
                    expected_value: Some(expected_val.clone()),
                }
            }
            ExpectedValue::NotEmpty => {
                let success = match actual {
                    JsonValue::Null => false,
                    JsonValue::String(s) => !s.is_empty(),
                    JsonValue::Array(arr) => !arr.is_empty(),
                    JsonValue::Object(obj) => !obj.is_empty(),
                    _ => true,
                };
                AssertionResult {
                    success,
                    message: if success {
                        format!("Assertion '{}' passed: value is not empty", description)
                    } else {
                        format!("Assertion '{}' failed: value is empty", description)
                    },
                    actual_value: Some(actual.clone()),
                    expected_value: Some(JsonValue::String("NotEmpty".to_string())),
                }
            }
            _ => AssertionResult {
                success: false,
                message: format!("Unsupported expected value type for assertion '{}'", description),
                actual_value: Some(actual.clone()),
                expected_value: Some(serde_json::to_value(expected).unwrap_or(JsonValue::Null)),
            }
        }
    }

    /// Rollback the workflow by executing rollback requests in reverse order
    async fn rollback_workflow(&mut self) -> Result<(), String> {
        let mut rollback_order = self.state.execution_order.clone();
        rollback_order.reverse();
        
        for step_id in rollback_order {
            let step = self.find_step(&step_id)?;
            
            if let Some(rollback_request) = &step.rollback_request {
                // Execute rollback request
                let mut request = rollback_request.clone();
                self.substitute_variables(&mut request)?;
                
                match self.simulate_request_execution(&request).await {
                    Ok(_) => {
                        // Rollback successful
                    }
                    Err(error) => {
                        // Log rollback failure but continue with other rollbacks
                        eprintln!("Failed to rollback step '{}': {}", step_id, error);
                    }
                }
            }
        }
        
        Ok(())
    }
}

#[async_trait]
impl Plugin for WorkflowValidation {
    fn name(&self) -> &str {
        "workflow-validation"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn dependencies(&self) -> Vec<String> {
        vec![]
    }

    async fn initialize(&mut self, config: &PluginConfig) -> Result<(), PluginError> {
        if let Some(steps_value) = config.settings.get("workflow_steps") {
            if let Ok(steps) = serde_json::from_value::<Vec<WorkflowStep>>(steps_value.clone()) {
                self.workflow_steps = steps;
            }
        }

        if let Some(rollback) = config.settings.get("rollback_on_failure") {
            self.rollback_on_failure = rollback.as_bool().unwrap_or(true);
        }

        if let Some(timeout) = config.settings.get("timeout_seconds") {
            if let Some(timeout_secs) = timeout.as_u64() {
                self.timeout = Duration::from_secs(timeout_secs);
            }
        }

        if self.workflow_steps.is_empty() {
            return Err(PluginError::InitializationFailed(
                "At least one workflow step is required".to_string(),
            ));
        }

        Ok(())
    }

    async fn shutdown(&mut self) -> Result<(), PluginError> {
        Ok(())
    }
}

#[async_trait]
impl AssertionPlugin for WorkflowValidation {
    fn assertion_type(&self) -> AssertionType {
        AssertionType::Custom("workflow-validation".to_string())
    }

    async fn execute(&self, _actual: &ResponseData, expected: &ExpectedValue) -> AssertionResult {
        // Clone self to make it mutable for workflow execution
        let mut workflow = self.clone();
        
        match workflow.execute_workflow().await {
            Ok(final_state) => {
                // Check if all steps succeeded
                let all_success = final_state.step_results.values().all(|result| result.success);
                
                AssertionResult {
                    success: all_success,
                    message: if all_success {
                        format!("Workflow completed successfully with {} steps", final_state.execution_order.len())
                    } else {
                        "Workflow failed - one or more steps did not complete successfully".to_string()
                    },
                    actual_value: Some(serde_json::to_value(&final_state).unwrap_or(JsonValue::Null)),
                    expected_value: Some(serde_json::to_value(expected).unwrap_or(JsonValue::Null)),
                }
            }
            Err(error) => {
                AssertionResult {
                    success: false,
                    message: format!("Workflow execution failed: {}", error),
                    actual_value: None,
                    expected_value: Some(serde_json::to_value(expected).unwrap_or(JsonValue::Null)),
                }
            }
        }
    }

    fn priority(&self) -> u8 {
        30 // Lower priority since workflows are complex and should run after basic assertions
    }
}

// Implement Clone for WorkflowValidation
impl Clone for WorkflowValidation {
    fn clone(&self) -> Self {
        Self {
            workflow_steps: self.workflow_steps.clone(),
            rollback_on_failure: self.rollback_on_failure,
            timeout: self.timeout,
            state: self.state.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::collections::HashMap;

    #[tokio::test]
    async fn test_workflow_validation_simple_sequence() {
        let steps = vec![
            WorkflowStep {
                id: "step1".to_string(),
                name: "Create User".to_string(),
                request: ProtocolRequest {
                    method: HttpMethod::POST,
                    url: "/users".to_string(),
                    headers: HashMap::new(),
                    body: Some(r#"{"name": "Test User"}"#.as_bytes().to_vec()),
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
                id: "step2".to_string(),
                name: "Get User".to_string(),
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
                depends_on: vec!["step1".to_string()],
            },
        ];

        let mut workflow = WorkflowValidation::new(steps);
        let result = workflow.execute_workflow().await;

        assert!(result.is_ok());
        let state = result.unwrap();
        assert_eq!(state.execution_order.len(), 2);
        assert_eq!(state.execution_order[0], "step1");
        assert_eq!(state.execution_order[1], "step2");
        assert!(state.variables.contains_key("user_id"));
    }

    #[tokio::test]
    async fn test_workflow_validation_dependency_resolution() {
        let steps = vec![
            WorkflowStep {
                id: "step_c".to_string(),
                name: "Step C".to_string(),
                request: ProtocolRequest {
                    method: HttpMethod::GET,
                    url: "/c".to_string(),
                    headers: HashMap::new(),
                    body: None,
                    timeout: Duration::from_secs(30),
                    protocol_version: None,
                    tls_config: None,
                },
                assertions: vec![],
                state_extractions: vec![],
                rollback_request: None,
                depends_on: vec!["step_a".to_string(), "step_b".to_string()],
            },
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
                depends_on: vec![],
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

        let mut workflow = WorkflowValidation::new(steps);
        let result = workflow.execute_workflow().await;

        assert!(result.is_ok());
        let state = result.unwrap();
        assert_eq!(state.execution_order.len(), 3);
        
        // Check that dependencies are respected
        let a_index = state.execution_order.iter().position(|x| x == "step_a").unwrap();
        let b_index = state.execution_order.iter().position(|x| x == "step_b").unwrap();
        let c_index = state.execution_order.iter().position(|x| x == "step_c").unwrap();
        
        assert!(a_index < b_index); // A must come before B
        assert!(a_index < c_index); // A must come before C
        assert!(b_index < c_index); // B must come before C
    }
}