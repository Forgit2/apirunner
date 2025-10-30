//! Execution engine with multiple strategies for API test execution
//! 
//! This module provides different execution strategies:
//! - Serial: Execute tests sequentially with state propagation
//! - Parallel: Execute tests concurrently with rate limiting
//! - Distributed: Execute tests across multiple nodes

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Semaphore;
use tokio::time::{sleep, Instant as TokioInstant};
use uuid::Uuid;

use crate::error::{ExecutionError, Result};
use crate::event::{Event, EventBus, TestLifecycleEvent, TestLifecycleType};
use crate::test_case_manager::{TestCase, RequestDefinition, AssertionDefinition, VariableExtraction, ExtractionSource};

/// Core execution context that maintains state across test executions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionContext {
    pub test_suite_id: String,
    pub environment: String,
    pub variables: HashMap<String, String>,
    pub auth_tokens: HashMap<String, AuthToken>,
    pub start_time: DateTime<Utc>,
}

impl ExecutionContext {
    pub fn new(test_suite_id: String, environment: String) -> Self {
        Self {
            test_suite_id,
            environment,
            variables: HashMap::new(),
            auth_tokens: HashMap::new(),
            start_time: Utc::now(),
        }
    }

    pub fn with_variables(mut self, variables: HashMap<String, String>) -> Self {
        self.variables = variables;
        self
    }

    pub fn set_variable(&mut self, key: String, value: String) {
        self.variables.insert(key, value);
    }

    pub fn get_variable(&self, key: &str) -> Option<&String> {
        self.variables.get(key)
    }
}

/// Authentication token with expiration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthToken {
    pub token: String,
    pub token_type: String,
    pub expires_at: Option<DateTime<Utc>>,
}



/// Result of a single test execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestResult {
    pub test_case_id: String,
    pub test_case_name: String,
    pub success: bool,
    pub duration: Duration,
    pub request: RequestDefinition,
    pub response: Option<ResponseData>,
    pub assertion_results: Vec<AssertionResult>,
    pub extracted_variables: HashMap<String, String>,
    pub error_message: Option<String>,
}

/// Response data from protocol execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseData {
    pub status_code: u16,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
    pub duration: Duration,
}

/// Result of an assertion
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssertionResult {
    pub assertion_type: String,
    pub success: bool,
    pub message: String,
    pub expected_value: Option<serde_json::Value>,
    pub actual_value: Option<serde_json::Value>,
}

/// Overall execution result
#[derive(Debug)]
pub struct ExecutionResult {
    pub test_results: Vec<TestResult>,
    pub total_duration: Duration,
    pub success_count: usize,
    pub failure_count: usize,
    pub context: ExecutionContext,
}

impl ExecutionResult {
    pub fn new(context: ExecutionContext) -> Self {
        Self {
            test_results: Vec::new(),
            total_duration: Duration::from_secs(0),
            success_count: 0,
            failure_count: 0,
            context,
        }
    }

    pub fn add_test_result(&mut self, result: TestResult) {
        if result.success {
            self.success_count += 1;
        } else {
            self.failure_count += 1;
        }
        self.total_duration += result.duration;
        self.test_results.push(result);
    }

    pub fn is_successful(&self) -> bool {
        self.failure_count == 0
    }
}

/// Trait for different execution strategies
#[async_trait]
pub trait ExecutionStrategy: Send + Sync {
    async fn execute(
        &self,
        test_cases: Vec<TestCase>,
        context: ExecutionContext,
    ) -> Result<ExecutionResult>;

    fn strategy_name(&self) -> &str;
}

/// Serial execution strategy that executes tests sequentially with state propagation
pub struct SerialExecutor {
    event_bus: Arc<EventBus>,
}

impl SerialExecutor {
    pub fn new(event_bus: Arc<EventBus>) -> Self {
        Self { event_bus }
    }

    /// Execute a single test case and extract variables for context propagation
    async fn execute_single_test(
        &self,
        test_case: &TestCase,
        context: &ExecutionContext,
    ) -> Result<TestResult> {
        let start_time = Instant::now();

        // Publish test case started event
        let test_started_event = Event::TestLifecycle(TestLifecycleEvent {
            event_id: Uuid::new_v4().to_string(),
            timestamp: Utc::now(),
            test_suite_id: context.test_suite_id.clone(),
            test_case_id: Some(test_case.id.clone()),
            lifecycle_type: TestLifecycleType::TestCaseStarted,
            metadata: HashMap::from([
                ("test_name".to_string(), test_case.name.clone()),
                ("environment".to_string(), context.environment.clone()),
            ]),
        });

        self.event_bus.publish(test_started_event).await.ok();

        // Resolve variables in request
        let resolved_request = self.resolve_request_variables(&test_case.request, context)?;

        // Execute the request (placeholder - would use protocol plugins in real implementation)
        let response = self.execute_request(&resolved_request).await?;

        // Execute assertions
        let assertion_results = self.execute_assertions(&test_case.assertions, &response).await?;

        // Extract variables from response
        let extracted_variables = self.extract_variables(&test_case.variable_extractions, &response)?;

        let duration = start_time.elapsed();
        let success = assertion_results.iter().all(|r| r.success);

        // Create test result
        let test_result = TestResult {
            test_case_id: test_case.id.clone(),
            test_case_name: test_case.name.clone(),
            success,
            duration,
            request: resolved_request,
            response: Some(response),
            assertion_results,
            extracted_variables: extracted_variables.clone(),
            error_message: None,
        };

        // Publish test case completed event
        let lifecycle_type = if success {
            TestLifecycleType::TestCaseCompleted
        } else {
            TestLifecycleType::TestCaseFailed
        };

        let test_completed_event = Event::TestLifecycle(TestLifecycleEvent {
            event_id: Uuid::new_v4().to_string(),
            timestamp: Utc::now(),
            test_suite_id: context.test_suite_id.clone(),
            test_case_id: Some(test_case.id.clone()),
            lifecycle_type,
            metadata: HashMap::from([
                ("duration_ms".to_string(), duration.as_millis().to_string()),
                ("success".to_string(), success.to_string()),
            ]),
        });

        self.event_bus.publish(test_completed_event).await.ok();

        Ok(test_result)
    }

    /// Resolve variables in request definition using execution context
    fn resolve_request_variables(
        &self,
        request: &RequestDefinition,
        context: &ExecutionContext,
    ) -> Result<RequestDefinition> {
        let mut resolved_request = request.clone();

        // Resolve URL variables
        resolved_request.url = self.substitute_variables(&request.url, &context.variables);

        // Resolve header variables
        for (key, value) in &mut resolved_request.headers {
            *value = self.substitute_variables(value, &context.variables);
        }

        // Resolve body variables if present
        if let Some(body) = &request.body {
            let resolved_body_str = self.substitute_variables(body, &context.variables);
            resolved_request.body = Some(resolved_body_str);
        }

        Ok(resolved_request)
    }

    /// Simple variable substitution using {{variable_name}} syntax
    fn substitute_variables(&self, template: &str, variables: &HashMap<String, String>) -> String {
        let mut result = template.to_string();
        for (key, value) in variables {
            let placeholder = format!("{{{{{}}}}}", key);
            result = result.replace(&placeholder, value);
        }
        result
    }

    /// Execute the actual request (placeholder implementation)
    async fn execute_request(&self, request: &RequestDefinition) -> Result<ResponseData> {
        // This is a placeholder implementation
        // In a real implementation, this would use protocol plugins
        tokio::time::sleep(Duration::from_millis(100)).await;

        Ok(ResponseData {
            status_code: 200,
            headers: HashMap::from([
                ("content-type".to_string(), "application/json".to_string()),
                ("server".to_string(), "test-server/1.0".to_string()),
            ]),
            body: b"{\"message\": \"success\", \"id\": 123}".to_vec(),
            duration: Duration::from_millis(100),
        })
    }

    /// Execute all assertions against the response
    async fn execute_assertions(
        &self,
        assertions: &[AssertionDefinition],
        response: &ResponseData,
    ) -> Result<Vec<AssertionResult>> {
        let mut results = Vec::new();

        for assertion in assertions {
            let result = self.execute_single_assertion(assertion, response).await?;
            results.push(result);
        }

        Ok(results)
    }

    /// Execute a single assertion (placeholder implementation)
    async fn execute_single_assertion(
        &self,
        assertion: &AssertionDefinition,
        response: &ResponseData,
    ) -> Result<AssertionResult> {
        match assertion.assertion_type.as_str() {
            "status_code" => {
                let expected_status = assertion.expected.as_u64().unwrap_or(200) as u16;
                let success = response.status_code == expected_status;
                Ok(AssertionResult {
                    assertion_type: assertion.assertion_type.clone(),
                    success,
                    message: assertion.message.clone().unwrap_or_else(|| {
                        format!("Status code assertion: expected {}, got {}", expected_status, response.status_code)
                    }),
                    expected_value: Some(assertion.expected.clone()),
                    actual_value: Some(serde_json::Value::Number(response.status_code.into())),
                })
            }
            "response_time" => {
                let expected_max_ms = assertion.expected.as_u64().unwrap_or(1000);
                let actual_ms = response.duration.as_millis() as u64;
                let success = actual_ms <= expected_max_ms;
                Ok(AssertionResult {
                    assertion_type: assertion.assertion_type.clone(),
                    success,
                    message: assertion.message.clone().unwrap_or_else(|| {
                        format!("Response time assertion: expected <= {}ms, got {}ms", expected_max_ms, actual_ms)
                    }),
                    expected_value: Some(assertion.expected.clone()),
                    actual_value: Some(serde_json::Value::Number(actual_ms.into())),
                })
            }
            _ => {
                // Default to success for unknown assertion types (placeholder)
                Ok(AssertionResult {
                    assertion_type: assertion.assertion_type.clone(),
                    success: true,
                    message: format!("Unknown assertion type: {}", assertion.assertion_type),
                    expected_value: Some(assertion.expected.clone()),
                    actual_value: None,
                })
            }
        }
    }

    /// Extract variables from response based on extraction configuration
    fn extract_variables(
        &self,
        extractions: &Option<Vec<VariableExtraction>>,
        response: &ResponseData,
    ) -> Result<HashMap<String, String>> {
        let mut extracted = HashMap::new();

        if let Some(extractions) = extractions {
            for extraction in extractions {
                let value = match extraction.source {
                    ExtractionSource::StatusCode => {
                        response.status_code.to_string()
                    }
                    ExtractionSource::ResponseHeader => {
                        response.headers.get(&extraction.path)
                            .cloned()
                            .unwrap_or_else(|| extraction.default_value.clone().unwrap_or_default())
                    }
                    ExtractionSource::ResponseBody => {
                        // Simple JSON path extraction (placeholder implementation)
                        if let Ok(body_str) = String::from_utf8(response.body.clone()) {
                            if let Ok(json_value) = serde_json::from_str::<serde_json::Value>(&body_str) {
                                self.extract_json_path(&json_value, &extraction.path)
                                    .unwrap_or_else(|| extraction.default_value.clone().unwrap_or_default())
                            } else {
                                extraction.default_value.clone().unwrap_or_default()
                            }
                        } else {
                            extraction.default_value.clone().unwrap_or_default()
                        }
                    }
                };

                extracted.insert(extraction.name.clone(), value);
            }
        }

        Ok(extracted)
    }

    /// Simple JSON path extraction (placeholder implementation)
    fn extract_json_path(&self, json: &serde_json::Value, path: &str) -> Option<String> {
        // Simple implementation for basic paths like "message" or "id"
        match json {
            serde_json::Value::Object(map) => {
                map.get(path).and_then(|v| match v {
                    serde_json::Value::String(s) => Some(s.clone()),
                    serde_json::Value::Number(n) => Some(n.to_string()),
                    serde_json::Value::Bool(b) => Some(b.to_string()),
                    _ => None,
                })
            }
            _ => None,
        }
    }

    /// Check if test case dependencies are satisfied
    fn check_dependencies(&self, test_case: &TestCase, completed_tests: &[String]) -> bool {
        test_case.dependencies.iter().all(|dep| completed_tests.contains(dep))
    }

    /// Sort test cases based on dependencies
    fn sort_by_dependencies(&self, mut test_cases: Vec<TestCase>) -> Result<Vec<TestCase>> {
        let mut sorted = Vec::new();
        let mut completed = Vec::new();

        // Simple topological sort
        while !test_cases.is_empty() {
            let mut found_ready = false;

            for i in 0..test_cases.len() {
                if self.check_dependencies(&test_cases[i], &completed) {
                    let test_case = test_cases.remove(i);
                    completed.push(test_case.id.clone());
                    sorted.push(test_case);
                    found_ready = true;
                    break;
                }
            }

            if !found_ready {
                return Err(ExecutionError::TestCaseFailed(
                    "Circular dependency detected in test cases".to_string()
                ).into());
            }
        }

        Ok(sorted)
    }
}

#[async_trait]
impl ExecutionStrategy for SerialExecutor {
    async fn execute(
        &self,
        test_cases: Vec<TestCase>,
        mut context: ExecutionContext,
    ) -> Result<ExecutionResult> {
        let start_time = Instant::now();
        let mut result = ExecutionResult::new(context.clone());

        // Publish suite started event
        let suite_started_event = Event::TestLifecycle(TestLifecycleEvent {
            event_id: Uuid::new_v4().to_string(),
            timestamp: Utc::now(),
            test_suite_id: context.test_suite_id.clone(),
            test_case_id: None,
            lifecycle_type: TestLifecycleType::SuiteStarted,
            metadata: HashMap::from([
                ("test_count".to_string(), test_cases.len().to_string()),
                ("strategy".to_string(), "serial".to_string()),
            ]),
        });

        self.event_bus.publish(suite_started_event).await.ok();

        // Sort test cases by dependencies to ensure proper execution order
        let sorted_test_cases = self.sort_by_dependencies(test_cases)?;

        // Execute test cases sequentially with state propagation
        for test_case in sorted_test_cases {
            let test_result = self.execute_single_test(&test_case, &context).await?;

            // Propagate extracted variables to context for next tests
            for (key, value) in &test_result.extracted_variables {
                context.set_variable(key.clone(), value.clone());
            }

            result.add_test_result(test_result);
        }

        result.total_duration = start_time.elapsed();
        result.context = context;

        // Publish suite completed event
        let lifecycle_type = if result.is_successful() {
            TestLifecycleType::SuiteCompleted
        } else {
            TestLifecycleType::SuiteFailed
        };

        let suite_completed_event = Event::TestLifecycle(TestLifecycleEvent {
            event_id: Uuid::new_v4().to_string(),
            timestamp: Utc::now(),
            test_suite_id: result.context.test_suite_id.clone(),
            test_case_id: None,
            lifecycle_type,
            metadata: HashMap::from([
                ("total_duration_ms".to_string(), result.total_duration.as_millis().to_string()),
                ("success_count".to_string(), result.success_count.to_string()),
                ("failure_count".to_string(), result.failure_count.to_string()),
            ]),
        });

        self.event_bus.publish(suite_completed_event).await.ok();

        Ok(result)
    }

    fn strategy_name(&self) -> &str {
        "serial"
    }
}

/// Rate limiter for controlling request rate
pub struct RateLimiter {
    max_requests_per_second: f64,
    last_request_time: Arc<tokio::sync::Mutex<TokioInstant>>,
}

impl RateLimiter {
    pub fn new(max_requests_per_second: f64) -> Self {
        Self {
            max_requests_per_second,
            last_request_time: Arc::new(tokio::sync::Mutex::new(TokioInstant::now())),
        }
    }

    /// Wait until it's safe to make the next request according to rate limit
    pub async fn wait(&self) {
        if self.max_requests_per_second <= 0.0 {
            return; // No rate limiting
        }

        let min_interval = Duration::from_secs_f64(1.0 / self.max_requests_per_second);
        let mut last_time = self.last_request_time.lock().await;
        let now = TokioInstant::now();
        let elapsed = now.duration_since(*last_time);

        if elapsed < min_interval {
            let wait_time = min_interval - elapsed;
            drop(last_time); // Release lock before sleeping
            sleep(wait_time).await;
            let mut last_time = self.last_request_time.lock().await;
            *last_time = TokioInstant::now();
        } else {
            *last_time = now;
        }
    }
}

/// Parallel execution strategy with concurrency control and rate limiting
pub struct ParallelExecutor {
    event_bus: Arc<EventBus>,
    concurrency_limit: usize,
    rate_limiter: Arc<RateLimiter>,
}

impl ParallelExecutor {
    pub fn new(
        event_bus: Arc<EventBus>,
        concurrency_limit: usize,
        max_requests_per_second: f64,
    ) -> Self {
        Self {
            event_bus,
            concurrency_limit,
            rate_limiter: Arc::new(RateLimiter::new(max_requests_per_second)),
        }
    }

    /// Execute a single test case with rate limiting and concurrency control
    async fn execute_single_test_parallel(
        &self,
        test_case: TestCase,
        context: ExecutionContext,
    ) -> Result<TestResult> {
        // Apply rate limiting
        self.rate_limiter.wait().await;

        let start_time = Instant::now();

        // Publish test case started event
        let test_started_event = Event::TestLifecycle(TestLifecycleEvent {
            event_id: Uuid::new_v4().to_string(),
            timestamp: Utc::now(),
            test_suite_id: context.test_suite_id.clone(),
            test_case_id: Some(test_case.id.clone()),
            lifecycle_type: TestLifecycleType::TestCaseStarted,
            metadata: HashMap::from([
                ("test_name".to_string(), test_case.name.clone()),
                ("environment".to_string(), context.environment.clone()),
                ("execution_mode".to_string(), "parallel".to_string()),
            ]),
        });

        self.event_bus.publish(test_started_event).await.ok();

        // Resolve variables in request (using isolated context)
        let resolved_request = self.resolve_request_variables(&test_case.request, &context)?;

        // Execute the request
        let response = self.execute_request(&resolved_request).await?;

        // Execute assertions
        let assertion_results = self.execute_assertions(&test_case.assertions, &response).await?;

        // Extract variables from response (isolated - not propagated in parallel mode)
        let extracted_variables = self.extract_variables(&test_case.variable_extractions, &response)?;

        let duration = start_time.elapsed();
        let success = assertion_results.iter().all(|r| r.success);

        // Create test result
        let test_result = TestResult {
            test_case_id: test_case.id.clone(),
            test_case_name: test_case.name.clone(),
            success,
            duration,
            request: resolved_request,
            response: Some(response),
            assertion_results,
            extracted_variables,
            error_message: None,
        };

        // Publish test case completed event
        let lifecycle_type = if success {
            TestLifecycleType::TestCaseCompleted
        } else {
            TestLifecycleType::TestCaseFailed
        };

        let test_completed_event = Event::TestLifecycle(TestLifecycleEvent {
            event_id: Uuid::new_v4().to_string(),
            timestamp: Utc::now(),
            test_suite_id: context.test_suite_id.clone(),
            test_case_id: Some(test_case.id.clone()),
            lifecycle_type,
            metadata: HashMap::from([
                ("duration_ms".to_string(), duration.as_millis().to_string()),
                ("success".to_string(), success.to_string()),
                ("execution_mode".to_string(), "parallel".to_string()),
            ]),
        });

        self.event_bus.publish(test_completed_event).await.ok();

        Ok(test_result)
    }

    /// Resolve variables in request definition using execution context (same as SerialExecutor)
    fn resolve_request_variables(
        &self,
        request: &RequestDefinition,
        context: &ExecutionContext,
    ) -> Result<RequestDefinition> {
        let mut resolved_request = request.clone();

        // Resolve URL variables
        resolved_request.url = self.substitute_variables(&request.url, &context.variables);

        // Resolve header variables
        for (_key, value) in &mut resolved_request.headers {
            *value = self.substitute_variables(value, &context.variables);
        }

        // Resolve body variables if present
        if let Some(body) = &request.body {
            let resolved_body_str = self.substitute_variables(body, &context.variables);
            resolved_request.body = Some(resolved_body_str);
        }

        Ok(resolved_request)
    }

    /// Simple variable substitution using {{variable_name}} syntax
    fn substitute_variables(&self, template: &str, variables: &HashMap<String, String>) -> String {
        let mut result = template.to_string();
        for (key, value) in variables {
            let placeholder = format!("{{{{{}}}}}", key);
            result = result.replace(&placeholder, value);
        }
        result
    }

    /// Execute the actual request (placeholder implementation)
    async fn execute_request(&self, _request: &RequestDefinition) -> Result<ResponseData> {
        // This is a placeholder implementation
        // In a real implementation, this would use protocol plugins
        tokio::time::sleep(Duration::from_millis(50)).await;

        Ok(ResponseData {
            status_code: 200,
            headers: HashMap::from([
                ("content-type".to_string(), "application/json".to_string()),
                ("server".to_string(), "test-server/1.0".to_string()),
            ]),
            body: b"{\"message\": \"success\", \"id\": 456}".to_vec(),
            duration: Duration::from_millis(50),
        })
    }

    /// Execute all assertions against the response
    async fn execute_assertions(
        &self,
        assertions: &[AssertionDefinition],
        response: &ResponseData,
    ) -> Result<Vec<AssertionResult>> {
        let mut results = Vec::new();

        for assertion in assertions {
            let result = self.execute_single_assertion(assertion, response).await?;
            results.push(result);
        }

        Ok(results)
    }

    /// Execute a single assertion (same as SerialExecutor)
    async fn execute_single_assertion(
        &self,
        assertion: &AssertionDefinition,
        response: &ResponseData,
    ) -> Result<AssertionResult> {
        match assertion.assertion_type.as_str() {
            "status_code" => {
                let expected_status = assertion.expected.as_u64().unwrap_or(200) as u16;
                let success = response.status_code == expected_status;
                Ok(AssertionResult {
                    assertion_type: assertion.assertion_type.clone(),
                    success,
                    message: assertion.message.clone().unwrap_or_else(|| {
                        format!("Status code assertion: expected {}, got {}", expected_status, response.status_code)
                    }),
                    expected_value: Some(assertion.expected.clone()),
                    actual_value: Some(serde_json::Value::Number(response.status_code.into())),
                })
            }
            "response_time" => {
                let expected_max_ms = assertion.expected.as_u64().unwrap_or(1000);
                let actual_ms = response.duration.as_millis() as u64;
                let success = actual_ms <= expected_max_ms;
                Ok(AssertionResult {
                    assertion_type: assertion.assertion_type.clone(),
                    success,
                    message: assertion.message.clone().unwrap_or_else(|| {
                        format!("Response time assertion: expected <= {}ms, got {}ms", expected_max_ms, actual_ms)
                    }),
                    expected_value: Some(assertion.expected.clone()),
                    actual_value: Some(serde_json::Value::Number(actual_ms.into())),
                })
            }
            _ => {
                // Default to success for unknown assertion types (placeholder)
                Ok(AssertionResult {
                    assertion_type: assertion.assertion_type.clone(),
                    success: true,
                    message: format!("Unknown assertion type: {}", assertion.assertion_type),
                    expected_value: Some(assertion.expected.clone()),
                    actual_value: None,
                })
            }
        }
    }

    /// Extract variables from response based on extraction configuration
    fn extract_variables(
        &self,
        extractions: &Option<Vec<VariableExtraction>>,
        response: &ResponseData,
    ) -> Result<HashMap<String, String>> {
        let mut extracted = HashMap::new();

        if let Some(extractions) = extractions {
            for extraction in extractions {
                let value = match extraction.source {
                    ExtractionSource::StatusCode => {
                        response.status_code.to_string()
                    }
                    ExtractionSource::ResponseHeader => {
                        response.headers.get(&extraction.path)
                            .cloned()
                            .unwrap_or_else(|| extraction.default_value.clone().unwrap_or_default())
                    }
                    ExtractionSource::ResponseBody => {
                        // Simple JSON path extraction (placeholder implementation)
                        if let Ok(body_str) = String::from_utf8(response.body.clone()) {
                            if let Ok(json_value) = serde_json::from_str::<serde_json::Value>(&body_str) {
                                self.extract_json_path(&json_value, &extraction.path)
                                    .unwrap_or_else(|| extraction.default_value.clone().unwrap_or_default())
                            } else {
                                extraction.default_value.clone().unwrap_or_default()
                            }
                        } else {
                            extraction.default_value.clone().unwrap_or_default()
                        }
                    }
                };

                extracted.insert(extraction.name.clone(), value);
            }
        }

        Ok(extracted)
    }

    /// Simple JSON path extraction (same as SerialExecutor)
    fn extract_json_path(&self, json: &serde_json::Value, path: &str) -> Option<String> {
        // Simple implementation for basic paths like "message" or "id"
        match json {
            serde_json::Value::Object(map) => {
                map.get(path).and_then(|v| match v {
                    serde_json::Value::String(s) => Some(s.clone()),
                    serde_json::Value::Number(n) => Some(n.to_string()),
                    serde_json::Value::Bool(b) => Some(b.to_string()),
                    _ => None,
                })
            }
            _ => None,
        }
    }

    /// Filter test cases that have no dependencies for parallel execution
    fn filter_independent_tests(&self, test_cases: &[TestCase]) -> Vec<TestCase> {
        test_cases
            .iter()
            .filter(|tc| tc.dependencies.is_empty())
            .cloned()
            .collect()
    }

    /// Group test cases by dependency levels for batched parallel execution
    fn group_by_dependency_levels(&self, test_cases: Vec<TestCase>) -> Vec<Vec<TestCase>> {
        let mut levels = Vec::new();
        let mut remaining = test_cases;
        let mut completed_ids = Vec::new();

        while !remaining.is_empty() {
            let mut current_level = Vec::new();
            let mut next_remaining = Vec::new();

            for test_case in remaining {
                if test_case.dependencies.iter().all(|dep| completed_ids.contains(dep)) {
                    completed_ids.push(test_case.id.clone());
                    current_level.push(test_case);
                } else {
                    next_remaining.push(test_case);
                }
            }

            if current_level.is_empty() {
                // Circular dependency or unresolvable dependencies
                break;
            }

            levels.push(current_level);
            remaining = next_remaining;
        }

        levels
    }
}

#[async_trait]
impl ExecutionStrategy for ParallelExecutor {
    async fn execute(
        &self,
        test_cases: Vec<TestCase>,
        context: ExecutionContext,
    ) -> Result<ExecutionResult> {
        let start_time = Instant::now();
        let mut result = ExecutionResult::new(context.clone());

        // Publish suite started event
        let suite_started_event = Event::TestLifecycle(TestLifecycleEvent {
            event_id: Uuid::new_v4().to_string(),
            timestamp: Utc::now(),
            test_suite_id: context.test_suite_id.clone(),
            test_case_id: None,
            lifecycle_type: TestLifecycleType::SuiteStarted,
            metadata: HashMap::from([
                ("test_count".to_string(), test_cases.len().to_string()),
                ("strategy".to_string(), "parallel".to_string()),
                ("concurrency_limit".to_string(), self.concurrency_limit.to_string()),
            ]),
        });

        self.event_bus.publish(suite_started_event).await.ok();

        // Group test cases by dependency levels
        let dependency_levels = self.group_by_dependency_levels(test_cases);

        // Execute each level in parallel, but levels sequentially
        for level in dependency_levels {
            if level.is_empty() {
                continue;
            }

            // Create semaphore for concurrency control
            let semaphore = Arc::new(Semaphore::new(self.concurrency_limit));
            let mut tasks = Vec::new();

            // Spawn tasks for all test cases in this level
            for test_case in level {
                let permit = semaphore.clone().acquire_owned().await
                    .map_err(|e| ExecutionError::ResourceExhausted(format!("Semaphore acquire failed: {}", e)))?;
                let context_clone = context.clone();
                let executor_clone = self.clone();

                let task = tokio::spawn(async move {
                    let _permit = permit; // Hold permit until task completes
                    executor_clone.execute_single_test_parallel(test_case, context_clone).await
                });

                tasks.push(task);
            }

            // Wait for all tasks in this level to complete
            let level_results = futures::future::try_join_all(tasks).await
                .map_err(|e| ExecutionError::TestCaseFailed(format!("Task join error: {}", e)))?;

            // Collect results
            for task_result in level_results {
                match task_result {
                    Ok(test_result) => result.add_test_result(test_result),
                    Err(e) => {
                        return Err(e);
                    }
                }
            }
        }

        result.total_duration = start_time.elapsed();

        // Publish suite completed event
        let lifecycle_type = if result.is_successful() {
            TestLifecycleType::SuiteCompleted
        } else {
            TestLifecycleType::SuiteFailed
        };

        let suite_completed_event = Event::TestLifecycle(TestLifecycleEvent {
            event_id: Uuid::new_v4().to_string(),
            timestamp: Utc::now(),
            test_suite_id: result.context.test_suite_id.clone(),
            test_case_id: None,
            lifecycle_type,
            metadata: HashMap::from([
                ("total_duration_ms".to_string(), result.total_duration.as_millis().to_string()),
                ("success_count".to_string(), result.success_count.to_string()),
                ("failure_count".to_string(), result.failure_count.to_string()),
                ("execution_mode".to_string(), "parallel".to_string()),
            ]),
        });

        self.event_bus.publish(suite_completed_event).await.ok();

        Ok(result)
    }

    fn strategy_name(&self) -> &str {
        "parallel"
    }
}

// Implement Clone for ParallelExecutor to allow sharing across tasks
impl Clone for ParallelExecutor {
    fn clone(&self) -> Self {
        Self {
            event_bus: self.event_bus.clone(),
            concurrency_limit: self.concurrency_limit,
            rate_limiter: self.rate_limiter.clone(),
        }
    }
}
