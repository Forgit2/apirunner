//! Interactive execution and debugging system for API tests
//! 
//! This module provides interactive test execution capabilities including:
//! - Single test execution with step-by-step debugging
//! - Variable inspection and manual override
//! - Dry-run mode for test validation
//! - Real-time progress monitoring with cancellation
//! - Interactive result analysis and comparison

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, RwLock};
use tokio::time::sleep;
use uuid::Uuid;

use crate::error::{ExecutionError, Result};
use crate::event::{Event, EventBus, TestLifecycleEvent, TestLifecycleType};
use crate::execution::{
    ExecutionContext, ExecutionStrategy, ExecutionResult, TestResult,
    ResponseData, AssertionResult,
};
use crate::test_case_manager::{TestCase, RequestDefinition, AssertionDefinition, VariableExtraction};

/// Interactive executor for single test execution with debugging capabilities
pub struct InteractiveExecutor {
    event_bus: Arc<EventBus>,
    variable_inspector: VariableInspector,
    progress_reporter: ProgressReporter,
    execution_state: Arc<RwLock<ExecutionState>>,
}

/// Current state of interactive execution
#[derive(Debug, Clone)]
pub struct ExecutionState {
    pub execution_id: String,
    pub current_step: usize,
    pub total_steps: usize,
    pub is_paused: bool,
    pub is_cancelled: bool,
    pub breakpoints: HashSet<usize>,
    pub step_results: Vec<StepExecutionResult>,
}

/// Configuration options for interactive debugging
#[derive(Debug, Clone)]
pub struct DebugOptions {
    pub dry_run: bool,
    pub verbose: bool,
    pub inspect_variables: bool,
    pub breakpoints: HashSet<usize>,
    pub step_through: bool,
    pub pause_on_failure: bool,
    pub capture_request_response: bool,
}

impl Default for DebugOptions {
    fn default() -> Self {
        Self {
            dry_run: false,
            verbose: false,
            inspect_variables: false,
            breakpoints: HashSet::new(),
            step_through: false,
            pause_on_failure: true,
            capture_request_response: true,
        }
    }
}

/// Result of interactive test execution with debugging information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InteractiveExecutionResult {
    pub execution_id: String,
    pub test_case_id: String,
    pub test_case_name: String,
    pub step_results: Vec<StepExecutionResult>,
    pub variable_snapshots: HashMap<usize, HashMap<String, String>>,
    pub total_duration: Duration,
    pub success: bool,
    pub dry_run: bool,
    pub validation_errors: Vec<ValidationError>,
}

/// Result of a single execution step
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepExecutionResult {
    pub step_index: usize,
    pub step_name: String,
    pub step_type: StepType,
    pub request: Option<RequestDefinition>,
    pub response: Option<ResponseData>,
    pub duration: Duration,
    pub success: bool,
    pub error_message: Option<String>,
    pub variables_extracted: HashMap<String, String>,
    pub assertion_results: Vec<AssertionResult>,
}

/// Type of execution step
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StepType {
    RequestExecution,
    VariableExtraction,
    AssertionValidation,
    PreConditionCheck,
    PostConditionCheck,
}

/// Validation error found during dry-run
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationError {
    pub step_index: usize,
    pub error_type: ValidationErrorType,
    pub message: String,
    pub suggestion: Option<String>,
}

/// Type of validation error
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ValidationErrorType {
    InvalidUrl,
    MissingVariable,
    InvalidJsonPath,
    InvalidAssertion,
    CircularDependency,
    MissingAuthentication,
}

impl InteractiveExecutor {
    pub fn new(event_bus: Arc<EventBus>) -> Self {
        Self {
            event_bus: event_bus.clone(),
            variable_inspector: VariableInspector::new(),
            progress_reporter: ProgressReporter::new(event_bus),
            execution_state: Arc::new(RwLock::new(ExecutionState {
                execution_id: Uuid::new_v4().to_string(),
                current_step: 0,
                total_steps: 0,
                is_paused: false,
                is_cancelled: false,
                breakpoints: HashSet::new(),
                step_results: Vec::new(),
            })),
        }
    }

    /// Execute a single test case with interactive debugging capabilities
    pub async fn execute_single_with_debugging(
        &self,
        test_case: &TestCase,
        context: &ExecutionContext,
        debug_options: DebugOptions,
    ) -> Result<InteractiveExecutionResult> {
        let execution_id = Uuid::new_v4().to_string();
        let start_time = Instant::now();

        // Initialize execution state
        {
            let mut state = self.execution_state.write().await;
            state.execution_id = execution_id.clone();
            state.current_step = 0;
            state.total_steps = self.calculate_total_steps(test_case);
            state.is_paused = false;
            state.is_cancelled = false;
            state.breakpoints = debug_options.breakpoints.clone();
            state.step_results.clear();
        }

        let mut result = InteractiveExecutionResult {
            execution_id: execution_id.clone(),
            test_case_id: test_case.id.clone(),
            test_case_name: test_case.name.clone(),
            step_results: Vec::new(),
            variable_snapshots: HashMap::new(),
            total_duration: Duration::from_secs(0),
            success: true,
            dry_run: debug_options.dry_run,
            validation_errors: Vec::new(),
        };

        // Perform dry-run validation if requested
        if debug_options.dry_run {
            result.validation_errors = self.validate_test_case(test_case, context).await?;
            result.total_duration = start_time.elapsed();
            return Ok(result);
        }

        // Publish execution started event
        self.publish_execution_event(&execution_id, test_case, ExecutionEventType::Started).await;

        // Execute test case steps with debugging
        let mut current_context = context.clone();
        let execution_steps = self.create_execution_steps(test_case);

        for (step_index, step) in execution_steps.iter().enumerate() {
            // Check for cancellation
            {
                let state = self.execution_state.read().await;
                if state.is_cancelled {
                    result.success = false;
                    break;
                }
            }

            // Update current step
            {
                let mut state = self.execution_state.write().await;
                state.current_step = step_index;
            }

            // Check for breakpoint
            if debug_options.breakpoints.contains(&step_index) || debug_options.step_through {
                self.pause_execution(&format!("Breakpoint at step {}: {}", step_index, step.name)).await?;
            }

            // Capture variable snapshot if requested
            if debug_options.inspect_variables {
                let variables = self.variable_inspector.inspect_context(&current_context).await?;
                result.variable_snapshots.insert(step_index, variables);
            }

            // Execute the step
            let step_result = self.execute_step_with_inspection(
                step,
                &current_context,
                &debug_options,
                step_index,
            ).await?;

            // Update context with extracted variables
            for (key, value) in &step_result.variables_extracted {
                current_context.variables.insert(key.clone(), value.clone());
            }

            // Check for failure and pause if configured
            if !step_result.success && debug_options.pause_on_failure {
                self.pause_execution(&format!("Step failed: {}", step_result.error_message.as_deref().unwrap_or("Unknown error"))).await?;
            }

            result.step_results.push(step_result.clone());
            result.success &= step_result.success;

            // Update execution state
            {
                let mut state = self.execution_state.write().await;
                state.step_results.push(step_result.clone());
            }

            // Report progress
            self.progress_reporter.report_step_progress(step_index, execution_steps.len(), &step_result).await?;
        }

        result.total_duration = start_time.elapsed();

        // Publish execution completed event
        let event_type = if result.success {
            ExecutionEventType::Completed
        } else {
            ExecutionEventType::Failed
        };
        self.publish_execution_event(&execution_id, test_case, event_type).await;

        Ok(result)
    }

    /// Validate test case without executing it (dry-run mode)
    async fn validate_test_case(
        &self,
        test_case: &TestCase,
        context: &ExecutionContext,
    ) -> Result<Vec<ValidationError>> {
        let mut errors = Vec::new();

        // Validate URL format and variable substitution
        if let Err(e) = self.validate_url(&test_case.request.url, &context.variables) {
            errors.push(ValidationError {
                step_index: 0,
                error_type: ValidationErrorType::InvalidUrl,
                message: format!("Invalid URL: {}", e),
                suggestion: Some("Check URL format and ensure all variables are defined".to_string()),
            });
        }

        // Validate variable references
        let missing_vars = self.find_missing_variables(&test_case.request, &context.variables);
        for var in missing_vars {
            errors.push(ValidationError {
                step_index: 0,
                error_type: ValidationErrorType::MissingVariable,
                message: format!("Missing variable: {}", var),
                suggestion: Some(format!("Define variable '{}' in execution context", var)),
            });
        }

        // Validate assertions
        for (index, assertion) in test_case.assertions.iter().enumerate() {
            if let Err(e) = self.validate_assertion(assertion) {
                errors.push(ValidationError {
                    step_index: index + 1,
                    error_type: ValidationErrorType::InvalidAssertion,
                    message: format!("Invalid assertion: {}", e),
                    suggestion: Some("Check assertion syntax and expected values".to_string()),
                });
            }
        }

        // Validate variable extractions
        if let Some(extractions) = &test_case.variable_extractions {
            for extraction in extractions {
                if let Err(e) = self.validate_variable_extraction(extraction) {
                    errors.push(ValidationError {
                        step_index: 0,
                        error_type: ValidationErrorType::InvalidJsonPath,
                        message: format!("Invalid variable extraction: {}", e),
                        suggestion: Some("Check JSONPath or XPath syntax".to_string()),
                    });
                }
            }
        }

        Ok(errors)
    }

    /// Execute a single step with detailed inspection
    async fn execute_step_with_inspection(
        &self,
        step: &ExecutionStep,
        context: &ExecutionContext,
        debug_options: &DebugOptions,
        step_index: usize,
    ) -> Result<StepExecutionResult> {
        let start_time = Instant::now();

        // Log request details if verbose
        if debug_options.verbose {
            self.log_step_details(step, context).await;
        }

        let mut step_result = StepExecutionResult {
            step_index,
            step_name: step.name.clone(),
            step_type: step.step_type.clone(),
            request: None,
            response: None,
            duration: Duration::from_secs(0),
            success: true,
            error_message: None,
            variables_extracted: HashMap::new(),
            assertion_results: Vec::new(),
        };

        match &step.step_type {
            StepType::RequestExecution => {
                if let Some(request) = &step.request {
                    // Resolve variables in request
                    let resolved_request = self.resolve_request_variables(request, context)?;
                    step_result.request = Some(resolved_request.clone());

                    // Execute the request
                    match self.execute_request(&resolved_request).await {
                        Ok(response) => {
                            step_result.response = Some(response);
                        }
                        Err(e) => {
                            step_result.success = false;
                            step_result.error_message = Some(e.to_string());
                        }
                    }
                }
            }
            StepType::AssertionValidation => {
                if let (Some(response), Some(assertions)) = (&step_result.response, &step.assertions) {
                    step_result.assertion_results = self.execute_assertions(assertions, response).await?;
                    step_result.success = step_result.assertion_results.iter().all(|r| r.success);
                }
            }
            StepType::VariableExtraction => {
                if let (Some(response), Some(extractions)) = (&step_result.response, &step.variable_extractions) {
                    step_result.variables_extracted = self.extract_variables(extractions, response)?;
                }
            }
            _ => {
                // Handle other step types as needed
            }
        }

        step_result.duration = start_time.elapsed();

        // Log response details if verbose
        if debug_options.verbose {
            self.log_step_result(&step_result).await;
        }

        Ok(step_result)
    }

    /// Pause execution and wait for user input
    async fn pause_execution(&self, message: &str) -> Result<()> {
        {
            let mut state = self.execution_state.write().await;
            state.is_paused = true;
        }

        println!("🔍 Debug: {}", message);
        println!("Press Enter to continue, 'c' to cancel, or 's' to skip to next breakpoint...");

        // In a real implementation, this would wait for actual user input
        // For now, we'll simulate a brief pause
        sleep(Duration::from_millis(100)).await;

        {
            let mut state = self.execution_state.write().await;
            state.is_paused = false;
        }

        Ok(())
    }

    /// Calculate total number of execution steps for a test case
    fn calculate_total_steps(&self, test_case: &TestCase) -> usize {
        let mut steps = 1; // Request execution
        steps += test_case.assertions.len(); // Assertions
        if test_case.variable_extractions.is_some() {
            steps += 1; // Variable extraction
        }
        steps
    }

    /// Create execution steps from test case
    fn create_execution_steps(&self, test_case: &TestCase) -> Vec<ExecutionStep> {
        let mut steps = Vec::new();

        // Request execution step
        steps.push(ExecutionStep {
            name: "Execute Request".to_string(),
            step_type: StepType::RequestExecution,
            request: Some(test_case.request.clone()),
            assertions: None,
            variable_extractions: None,
        });

        // Assertion steps
        if !test_case.assertions.is_empty() {
            steps.push(ExecutionStep {
                name: "Validate Assertions".to_string(),
                step_type: StepType::AssertionValidation,
                request: None,
                assertions: Some(test_case.assertions.clone()),
                variable_extractions: None,
            });
        }

        // Variable extraction step
        if let Some(extractions) = &test_case.variable_extractions {
            steps.push(ExecutionStep {
                name: "Extract Variables".to_string(),
                step_type: StepType::VariableExtraction,
                request: None,
                assertions: None,
                variable_extractions: Some(extractions.clone()),
            });
        }

        steps
    }

    // Helper methods (placeholder implementations)
    fn validate_url(&self, url: &str, _variables: &HashMap<String, String>) -> Result<()> {
        if url.is_empty() {
            return Err(ExecutionError::TestCaseFailed("Empty URL".to_string()).into());
        }
        Ok(())
    }

    fn find_missing_variables(&self, _request: &RequestDefinition, _variables: &HashMap<String, String>) -> Vec<String> {
        // Placeholder implementation
        Vec::new()
    }

    fn validate_assertion(&self, _assertion: &AssertionDefinition) -> Result<()> {
        // Placeholder implementation
        Ok(())
    }

    fn validate_variable_extraction(&self, _extraction: &VariableExtraction) -> Result<()> {
        // Placeholder implementation
        Ok(())
    }

    fn resolve_request_variables(&self, request: &RequestDefinition, context: &ExecutionContext) -> Result<RequestDefinition> {
        // Placeholder implementation - same as in execution.rs
        let mut resolved = request.clone();
        for (key, value) in &context.variables {
            let placeholder = format!("{{{{{}}}}}", key);
            resolved.url = resolved.url.replace(&placeholder, value);
        }
        Ok(resolved)
    }

    async fn execute_request(&self, _request: &RequestDefinition) -> Result<ResponseData> {
        // Placeholder implementation
        sleep(Duration::from_millis(50)).await;
        Ok(ResponseData {
            status_code: 200,
            headers: HashMap::new(),
            body: b"{}".to_vec(),
            duration: Duration::from_millis(50),
        })
    }

    async fn execute_assertions(&self, _assertions: &[AssertionDefinition], _response: &ResponseData) -> Result<Vec<AssertionResult>> {
        // Placeholder implementation
        Ok(Vec::new())
    }

    fn extract_variables(&self, _extractions: &[VariableExtraction], _response: &ResponseData) -> Result<HashMap<String, String>> {
        // Placeholder implementation
        Ok(HashMap::new())
    }

    async fn log_step_details(&self, step: &ExecutionStep, _context: &ExecutionContext) {
        println!("🔧 Executing step: {}", step.name);
    }

    async fn log_step_result(&self, result: &StepExecutionResult) {
        println!("✅ Step completed: {} ({}ms)", result.step_name, result.duration.as_millis());
    }

    async fn publish_execution_event(&self, execution_id: &str, test_case: &TestCase, event_type: ExecutionEventType) {
        let event = Event::TestLifecycle(TestLifecycleEvent {
            event_id: Uuid::new_v4().to_string(),
            timestamp: Utc::now(),
            test_suite_id: execution_id.to_string(),
            test_case_id: Some(test_case.id.clone()),
            lifecycle_type: match event_type {
                ExecutionEventType::Started => TestLifecycleType::TestCaseStarted,
                ExecutionEventType::Completed => TestLifecycleType::TestCaseCompleted,
                ExecutionEventType::Failed => TestLifecycleType::TestCaseFailed,
            },
            metadata: HashMap::from([
                ("execution_mode".to_string(), "interactive".to_string()),
                ("test_name".to_string(), test_case.name.clone()),
            ]),
        });

        self.event_bus.publish(event).await.ok();
    }
}

/// Execution step definition
#[derive(Debug, Clone)]
pub struct ExecutionStep {
    pub name: String,
    pub step_type: StepType,
    pub request: Option<RequestDefinition>,
    pub assertions: Option<Vec<AssertionDefinition>>,
    pub variable_extractions: Option<Vec<VariableExtraction>>,
}

/// Type of execution event
#[derive(Debug, Clone)]
enum ExecutionEventType {
    Started,
    Completed,
    Failed,
}

/// Variable inspector for examining execution context
pub struct VariableInspector {
    inspection_history: Arc<Mutex<Vec<VariableSnapshot>>>,
}

/// Snapshot of variables at a point in time
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VariableSnapshot {
    pub timestamp: DateTime<Utc>,
    pub step_index: usize,
    pub variables: HashMap<String, String>,
    pub auth_tokens: HashMap<String, String>,
}

impl VariableInspector {
    pub fn new() -> Self {
        Self {
            inspection_history: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Inspect current execution context and return variable snapshot
    pub async fn inspect_context(&self, context: &ExecutionContext) -> Result<HashMap<String, String>> {
        let mut variables = context.variables.clone();
        
        // Add auth token information
        for (key, token) in &context.auth_tokens {
            variables.insert(format!("auth.{}.type", key), token.token_type.clone());
            if let Some(expires_at) = &token.expires_at {
                variables.insert(format!("auth.{}.expires_at", key), expires_at.to_rfc3339());
            }
        }

        // Store snapshot in history
        let snapshot = VariableSnapshot {
            timestamp: Utc::now(),
            step_index: 0, // Will be set by caller
            variables: variables.clone(),
            auth_tokens: context.auth_tokens.iter()
                .map(|(k, v)| (k.clone(), v.token.clone()))
                .collect(),
        };

        {
            let mut history = self.inspection_history.lock().await;
            history.push(snapshot);
        }

        Ok(variables)
    }

    /// Get variable inspection history
    pub async fn get_inspection_history(&self) -> Vec<VariableSnapshot> {
        let history = self.inspection_history.lock().await;
        history.clone()
    }

    /// Clear inspection history
    pub async fn clear_history(&self) {
        let mut history = self.inspection_history.lock().await;
        history.clear();
    }
}

/// Progress reporter for real-time execution monitoring
pub struct ProgressReporter {
    event_bus: Arc<EventBus>,
    progress_subscribers: Arc<Mutex<Vec<Box<dyn ProgressSubscriber>>>>,
}

/// Trait for progress subscription
#[async_trait]
pub trait ProgressSubscriber: Send + Sync {
    async fn on_progress_update(&self, update: ProgressUpdate) -> Result<()>;
    async fn on_execution_complete(&self, summary: ExecutionSummary) -> Result<()>;
}

/// Progress update information
#[derive(Debug, Clone, Serialize)]
pub struct ProgressUpdate {
    pub execution_id: String,
    pub current_step: usize,
    pub total_steps: usize,
    pub current_test_case: String,
    pub elapsed_time: Duration,
    pub estimated_remaining: Option<Duration>,
    pub success_count: usize,
    pub failure_count: usize,
}

/// Execution summary
#[derive(Debug, Clone, Serialize)]
pub struct ExecutionSummary {
    pub execution_id: String,
    pub total_tests: usize,
    pub passed_tests: usize,
    pub failed_tests: usize,
    pub total_duration: Duration,
    pub success_rate: f64,
}

impl ProgressReporter {
    pub fn new(event_bus: Arc<EventBus>) -> Self {
        Self {
            event_bus,
            progress_subscribers: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Add a progress subscriber
    pub async fn add_subscriber(&self, subscriber: Box<dyn ProgressSubscriber>) {
        let mut subscribers = self.progress_subscribers.lock().await;
        subscribers.push(subscriber);
    }

    /// Report progress for a single step
    pub async fn report_step_progress(
        &self,
        step_index: usize,
        total_steps: usize,
        step_result: &StepExecutionResult,
    ) -> Result<()> {
        let update = ProgressUpdate {
            execution_id: "interactive".to_string(),
            current_step: step_index,
            total_steps,
            current_test_case: step_result.step_name.clone(),
            elapsed_time: step_result.duration,
            estimated_remaining: None,
            success_count: if step_result.success { 1 } else { 0 },
            failure_count: if step_result.success { 0 } else { 1 },
        };

        let subscribers = self.progress_subscribers.lock().await;
        for subscriber in subscribers.iter() {
            subscriber.on_progress_update(update.clone()).await?;
        }

        Ok(())
    }

    /// Report execution completion
    pub async fn report_completion(&self, summary: ExecutionSummary) -> Result<()> {
        let subscribers = self.progress_subscribers.lock().await;
        for subscriber in subscribers.iter() {
            subscriber.on_execution_complete(summary.clone()).await?;
        }

        Ok(())
    }
}

/// Console progress subscriber for CLI output
pub struct ConsoleProgressSubscriber {
    show_details: bool,
}

impl ConsoleProgressSubscriber {
    pub fn new(show_details: bool) -> Self {
        Self { show_details }
    }
}

#[async_trait]
impl ProgressSubscriber for ConsoleProgressSubscriber {
    async fn on_progress_update(&self, update: ProgressUpdate) -> Result<()> {
        if self.show_details {
            println!(
                "📊 Progress: {}/{} - {} ({}ms)",
                update.current_step + 1,
                update.total_steps,
                update.current_test_case,
                update.elapsed_time.as_millis()
            );
        }
        Ok(())
    }

    async fn on_execution_complete(&self, summary: ExecutionSummary) -> Result<()> {
        println!(
            "🎯 Execution complete: {}/{} passed ({:.1}%) in {}ms",
            summary.passed_tests,
            summary.total_tests,
            summary.success_rate * 100.0,
            summary.total_duration.as_millis()
        );
        Ok(())
    }
}

#[async_trait]
impl ExecutionStrategy for InteractiveExecutor {
    async fn execute(
        &self,
        test_cases: Vec<TestCase>,
        context: ExecutionContext,
    ) -> Result<ExecutionResult> {
        // For interactive execution, we typically handle one test at a time
        // This implementation provides a basic batch execution capability
        let mut result = ExecutionResult::new(context.clone());
        
        for test_case in test_cases {
            let debug_options = DebugOptions::default();
            let interactive_result = self.execute_single_with_debugging(&test_case, &context, debug_options).await?;
            
            // Convert interactive result to standard test result
            let test_result = TestResult {
                test_case_id: test_case.id.clone(),
                test_case_name: test_case.name.clone(),
                success: interactive_result.success,
                duration: interactive_result.total_duration,
                request: test_case.request.clone(),
                response: interactive_result.step_results.first()
                    .and_then(|s| s.response.clone()),
                assertion_results: interactive_result.step_results.iter()
                    .flat_map(|s| s.assertion_results.clone())
                    .collect(),
                extracted_variables: interactive_result.step_results.iter()
                    .flat_map(|s| s.variables_extracted.clone())
                    .collect(),
                error_message: interactive_result.step_results.iter()
                    .find_map(|s| s.error_message.clone()),
            };
            
            result.add_test_result(test_result);
        }
        
        Ok(result)
    }

    fn strategy_name(&self) -> &str {
        "interactive"
    }
}