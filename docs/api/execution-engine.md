# Execution Engine

The Execution Engine is responsible for orchestrating test execution across different strategies (serial, parallel, distributed, and interactive). It provides a unified interface for running tests while supporting various execution modes and resource management.

## Core Execution Engine

The main execution engine coordinates different execution strategies and manages resources.

```rust
pub struct ExecutionEngine {
    strategies: HashMap<ExecutionMode, Box<dyn ExecutionStrategy>>,
    resource_manager: Arc<ResourceManager>,
    event_bus: Arc<EventBus>,
    metrics_collector: Arc<MetricsCollector>,
    plugin_manager: Arc<PluginManager>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ExecutionMode {
    Serial,
    Parallel,
    Distributed,
    Interactive,
}

impl ExecutionEngine {
    /// Create a new execution engine
    pub fn new(
        event_bus: Arc<EventBus>,
        metrics_collector: Arc<MetricsCollector>,
        plugin_manager: Arc<PluginManager>,
    ) -> Self {
        let resource_manager = Arc::new(ResourceManager::new());
        
        let mut strategies: HashMap<ExecutionMode, Box<dyn ExecutionStrategy>> = HashMap::new();
        
        // Register built-in execution strategies
        strategies.insert(
            ExecutionMode::Serial,
            Box::new(SerialExecutor::new(event_bus.clone(), metrics_collector.clone()))
        );
        strategies.insert(
            ExecutionMode::Parallel,
            Box::new(ParallelExecutor::new(event_bus.clone(), metrics_collector.clone()))
        );
        strategies.insert(
            ExecutionMode::Distributed,
            Box::new(DistributedExecutor::new(event_bus.clone(), metrics_collector.clone()))
        );
        strategies.insert(
            ExecutionMode::Interactive,
            Box::new(InteractiveExecutor::new(event_bus.clone(), metrics_collector.clone()))
        );
        
        Self {
            strategies,
            resource_manager,
            event_bus,
            metrics_collector,
            plugin_manager,
        }
    }
    
    /// Execute tests using the specified strategy
    pub async fn execute_tests(
        &self,
        test_cases: Vec<TestCase>,
        execution_config: ExecutionConfig,
    ) -> Result<ExecutionResult, ExecutionError> {
        let strategy = self.strategies.get(&execution_config.mode)
            .ok_or_else(|| ExecutionError::UnsupportedMode(execution_config.mode.clone()))?;
        
        // Create execution context
        let context = ExecutionContext {
            execution_id: Uuid::new_v4().to_string(),
            config: execution_config.clone(),
            variables: execution_config.variables.clone(),
            auth_tokens: HashMap::new(),
            metrics: self.metrics_collector.clone(),
            event_bus: self.event_bus.clone(),
            plugin_manager: self.plugin_manager.clone(),
            resource_manager: self.resource_manager.clone(),
        };
        
        // Publish execution started event
        self.publish_execution_event(ExecutionEvent::ExecutionStarted {
            execution_id: context.execution_id.clone(),
            test_count: test_cases.len(),
            mode: execution_config.mode.clone(),
            config: execution_config.clone(),
        }).await?;
        
        let start_time = Instant::now();
        
        // Execute tests
        let result = strategy.execute(test_cases, context).await;
        
        let duration = start_time.elapsed();
        
        // Publish execution completed event
        match &result {
            Ok(execution_result) => {
                self.publish_execution_event(ExecutionEvent::ExecutionCompleted {
                    execution_id: execution_result.execution_id.clone(),
                    summary: execution_result.summary.clone(),
                    duration,
                }).await?;
            }
            Err(error) => {
                self.publish_execution_event(ExecutionEvent::ExecutionFailed {
                    execution_id: "unknown".to_string(),
                    error: error.to_string(),
                    duration,
                }).await?;
            }
        }
        
        result
    }
    
    /// Register a custom execution strategy
    pub fn register_strategy(&mut self, mode: ExecutionMode, strategy: Box<dyn ExecutionStrategy>) {
        self.strategies.insert(mode, strategy);
    }
    
    /// Get available execution modes
    pub fn available_modes(&self) -> Vec<ExecutionMode> {
        self.strategies.keys().cloned().collect()
    }
    
    async fn publish_execution_event(&self, event: ExecutionEvent) -> Result<(), ExecutionError> {
        let event = Event {
            id: Uuid::new_v4().to_string(),
            event_type: "execution".to_string(),
            timestamp: Utc::now(),
            source: "execution_engine".to_string(),
            payload: EventPayload::Execution(event),
            metadata: HashMap::new(),
            correlation_id: None,
        };
        
        self.event_bus.publish(event).await
            .map_err(|e| ExecutionError::EventError(e.to_string()))?;
        
        Ok(())
    }
}
```

## Execution Strategy Trait

All execution strategies implement this common interface.

```rust
#[async_trait]
pub trait ExecutionStrategy: Send + Sync {
    /// Execute a list of test cases
    async fn execute(
        &self,
        test_cases: Vec<TestCase>,
        context: ExecutionContext,
    ) -> Result<ExecutionResult, ExecutionError>;
    
    /// Get the strategy name
    fn name(&self) -> &str;
    
    /// Get strategy-specific configuration schema
    fn config_schema(&self) -> serde_json::Value;
    
    /// Validate strategy-specific configuration
    fn validate_config(&self, config: &ExecutionConfig) -> Result<(), ExecutionError>;
    
    /// Cancel ongoing execution (optional)
    async fn cancel(&self, execution_id: &str) -> Result<(), ExecutionError> {
        Err(ExecutionError::UnsupportedOperation("Cancel not supported".to_string()))
    }
    
    /// Get execution progress (optional)
    async fn get_progress(&self, execution_id: &str) -> Result<ExecutionProgress, ExecutionError> {
        Err(ExecutionError::UnsupportedOperation("Progress tracking not supported".to_string()))
    }
}
```

## Execution Context

The execution context provides shared state and resources for test execution.

```rust
#[derive(Debug, Clone)]
pub struct ExecutionContext {
    /// Unique execution identifier
    pub execution_id: String,
    /// Execution configuration
    pub config: ExecutionConfig,
    /// Global variables
    pub variables: HashMap<String, String>,
    /// Authentication tokens
    pub auth_tokens: HashMap<String, AuthToken>,
    /// Metrics collector
    pub metrics: Arc<MetricsCollector>,
    /// Event bus
    pub event_bus: Arc<EventBus>,
    /// Plugin manager
    pub plugin_manager: Arc<PluginManager>,
    /// Resource manager
    pub resource_manager: Arc<ResourceManager>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionConfig {
    /// Execution mode
    pub mode: ExecutionMode,
    /// Environment name
    pub environment: String,
    /// Global variables
    pub variables: HashMap<String, String>,
    /// Execution timeout
    pub timeout: Option<Duration>,
    /// Stop on first failure
    pub stop_on_failure: bool,
    /// Maximum retry attempts
    pub max_retries: u32,
    /// Retry delay
    pub retry_delay: Duration,
    /// Concurrency settings (for parallel execution)
    pub concurrency: Option<ConcurrencyConfig>,
    /// Distributed execution settings
    pub distributed: Option<DistributedConfig>,
    /// Interactive execution settings
    pub interactive: Option<InteractiveConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConcurrencyConfig {
    /// Maximum concurrent tests
    pub max_concurrent: usize,
    /// Rate limit (requests per second)
    pub rate_limit: Option<f64>,
    /// Connection pool size
    pub connection_pool_size: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DistributedConfig {
    /// Coordinator node address
    pub coordinator_address: Option<String>,
    /// Worker node capacity
    pub worker_capacity: Option<usize>,
    /// Node discovery method
    pub discovery: NodeDiscovery,
    /// Fault tolerance settings
    pub fault_tolerance: FaultToleranceConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InteractiveConfig {
    /// Enable step-by-step execution
    pub step_by_step: bool,
    /// Breakpoints (test indices)
    pub breakpoints: Vec<usize>,
    /// Enable variable inspection
    pub inspect_variables: bool,
    /// Enable dry run mode
    pub dry_run: bool,
}
```

## Serial Execution Strategy

Executes tests sequentially with state propagation between tests.

```rust
pub struct SerialExecutor {
    event_bus: Arc<EventBus>,
    metrics: Arc<MetricsCollector>,
}

impl SerialExecutor {
    pub fn new(event_bus: Arc<EventBus>, metrics: Arc<MetricsCollector>) -> Self {
        Self { event_bus, metrics }
    }
    
    async fn execute_single_test(
        &self,
        test_case: &TestCase,
        context: &ExecutionContext,
    ) -> Result<TestResult, ExecutionError> {
        let test_start = Instant::now();
        
        // Publish test started event
        self.publish_test_event(TestEvent::TestStarted {
            test_id: test_case.id.clone(),
            test_name: test_case.name.clone(),
            execution_id: context.execution_id.clone(),
        }).await?;
        
        // Get protocol plugin
        let protocol_plugin = context.plugin_manager
            .get_protocol_plugin(&test_case.request.protocol)
            .ok_or_else(|| ExecutionError::PluginNotFound(test_case.request.protocol.clone()))?;
        
        // Execute request
        let request = self.build_protocol_request(test_case, context)?;
        let response = protocol_plugin.execute_request(request).await
            .map_err(|e| ExecutionError::RequestFailed(e.to_string()))?;
        
        // Execute assertions
        let assertion_results = self.execute_assertions(test_case, &response, context).await?;
        
        // Extract variables
        let extracted_variables = self.extract_variables(test_case, &response)?;
        
        let duration = test_start.elapsed();
        
        // Determine test status
        let status = if assertion_results.iter().all(|r| r.success) {
            TestStatus::Passed
        } else {
            TestStatus::Failed
        };
        
        let test_result = TestResult {
            test_id: test_case.id.clone(),
            test_name: test_case.name.clone(),
            status: status.clone(),
            duration,
            assertion_results,
            extracted_variables,
            request_response: Some(RequestResponseData {
                request: request.clone(),
                response: response.clone(),
            }),
            error: None,
        };
        
        // Publish test completed event
        self.publish_test_event(TestEvent::TestCompleted {
            test_id: test_case.id.clone(),
            test_name: test_case.name.clone(),
            execution_id: context.execution_id.clone(),
            result: test_result.clone(),
        }).await?;
        
        // Record metrics
        self.metrics.increment_counter("tests_executed", 1);
        self.metrics.record_timer("test_duration", duration);
        match status {
            TestStatus::Passed => self.metrics.increment_counter("tests_passed", 1),
            TestStatus::Failed => self.metrics.increment_counter("tests_failed", 1),
            _ => {}
        }
        
        Ok(test_result)
    }
    
    async fn execute_assertions(
        &self,
        test_case: &TestCase,
        response: &ProtocolResponse,
        context: &ExecutionContext,
    ) -> Result<Vec<AssertionResult>, ExecutionError> {
        let mut results = Vec::new();
        
        // Convert response to ResponseData
        let response_data = ResponseData {
            status_code: response.status_code,
            headers: response.headers.clone(),
            body: response.body.clone(),
            body_text: String::from_utf8(response.body.clone()).ok(),
            json: serde_json::from_slice(&response.body).ok(),
            duration: response.duration,
            metadata: response.metadata.clone(),
        };
        
        for assertion in &test_case.assertions {
            let assertion_plugin = context.plugin_manager
                .get_assertion_plugin(&assertion.assertion_type)
                .ok_or_else(|| ExecutionError::PluginNotFound(assertion.assertion_type.to_string()))?;
            
            let result = assertion_plugin.execute(&response_data, &assertion.expected).await;
            results.push(result);
        }
        
        Ok(results)
    }
    
    fn extract_variables(
        &self,
        test_case: &TestCase,
        response: &ProtocolResponse,
    ) -> Result<HashMap<String, String>, ExecutionError> {
        let mut variables = HashMap::new();
        
        if let Some(extractions) = &test_case.variable_extractions {
            for extraction in extractions {
                if let Some(value) = self.extract_single_variable(extraction, response)? {
                    variables.insert(extraction.name.clone(), value);
                }
            }
        }
        
        Ok(variables)
    }
    
    fn extract_single_variable(
        &self,
        extraction: &VariableExtraction,
        response: &ProtocolResponse,
    ) -> Result<Option<String>, ExecutionError> {
        match &extraction.source {
            ExtractionSource::JsonPath(path) => {
                if let Ok(json) = serde_json::from_slice::<serde_json::Value>(&response.body) {
                    // Use JSONPath to extract value
                    // This is a simplified implementation
                    Ok(Some("extracted_value".to_string()))
                } else {
                    Ok(None)
                }
            }
            ExtractionSource::Header(header_name) => {
                Ok(response.headers.get(header_name).cloned())
            }
            ExtractionSource::StatusCode => {
                Ok(Some(response.status_code.to_string()))
            }
        }
    }
}

#[async_trait]
impl ExecutionStrategy for SerialExecutor {
    async fn execute(
        &self,
        test_cases: Vec<TestCase>,
        mut context: ExecutionContext,
    ) -> Result<ExecutionResult, ExecutionError> {
        let mut results = Vec::new();
        let execution_start = Instant::now();
        
        for (index, test_case) in test_cases.iter().enumerate() {
            // Check for cancellation
            if context.config.stop_on_failure && results.iter().any(|r: &TestResult| r.status == TestStatus::Failed) {
                break;
            }
            
            // Execute test with retries
            let mut attempts = 0;
            let mut test_result = None;
            
            while attempts <= context.config.max_retries {
                match self.execute_single_test(test_case, &context).await {
                    Ok(result) => {
                        test_result = Some(result);
                        break;
                    }
                    Err(e) if attempts < context.config.max_retries => {
                        attempts += 1;
                        tokio::time::sleep(context.config.retry_delay).await;
                        continue;
                    }
                    Err(e) => {
                        test_result = Some(TestResult {
                            test_id: test_case.id.clone(),
                            test_name: test_case.name.clone(),
                            status: TestStatus::Error,
                            duration: Duration::ZERO,
                            assertion_results: Vec::new(),
                            extracted_variables: HashMap::new(),
                            request_response: None,
                            error: Some(TestError {
                                message: e.to_string(),
                                details: None,
                            }),
                        });
                        break;
                    }
                }
            }
            
            if let Some(result) = test_result {
                // Update context with extracted variables for next test
                for (key, value) in &result.extracted_variables {
                    context.variables.insert(key.clone(), value.clone());
                }
                
                results.push(result);
            }
        }
        
        let total_duration = execution_start.elapsed();
        
        // Calculate summary
        let summary = ExecutionSummary {
            total_tests: results.len(),
            passed_tests: results.iter().filter(|r| r.status == TestStatus::Passed).count(),
            failed_tests: results.iter().filter(|r| r.status == TestStatus::Failed).count(),
            skipped_tests: 0,
            error_tests: results.iter().filter(|r| r.status == TestStatus::Error).count(),
            total_duration,
            success_rate: if results.is_empty() {
                0.0
            } else {
                results.iter().filter(|r| r.status == TestStatus::Passed).count() as f64 / results.len() as f64
            },
            start_time: Utc::now() - chrono::Duration::from_std(total_duration).unwrap(),
            end_time: Utc::now(),
        };
        
        Ok(ExecutionResult {
            execution_id: context.execution_id,
            summary,
            test_results: results,
            metrics: self.metrics.get_performance_summary(),
            metadata: HashMap::new(),
        })
    }
    
    fn name(&self) -> &str {
        "serial"
    }
    
    fn config_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "stop_on_failure": {
                    "type": "boolean",
                    "default": false,
                    "description": "Stop execution on first test failure"
                },
                "max_retries": {
                    "type": "integer",
                    "minimum": 0,
                    "default": 0,
                    "description": "Maximum number of retry attempts for failed tests"
                },
                "retry_delay": {
                    "type": "string",
                    "pattern": "^\\d+[smh]$",
                    "default": "1s",
                    "description": "Delay between retry attempts"
                }
            }
        })
    }
    
    fn validate_config(&self, config: &ExecutionConfig) -> Result<(), ExecutionError> {
        if config.mode != ExecutionMode::Serial {
            return Err(ExecutionError::InvalidConfiguration(
                "Serial executor requires Serial execution mode".to_string()
            ));
        }
        Ok(())
    }
}
```

## Parallel Execution Strategy

Executes tests concurrently with configurable concurrency limits and rate limiting.

```rust
pub struct ParallelExecutor {
    event_bus: Arc<EventBus>,
    metrics: Arc<MetricsCollector>,
}

impl ParallelExecutor {
    pub fn new(event_bus: Arc<EventBus>, metrics: Arc<MetricsCollector>) -> Self {
        Self { event_bus, metrics }
    }
}

#[async_trait]
impl ExecutionStrategy for ParallelExecutor {
    async fn execute(
        &self,
        test_cases: Vec<TestCase>,
        context: ExecutionContext,
    ) -> Result<ExecutionResult, ExecutionError> {
        let concurrency_config = context.config.concurrency
            .as_ref()
            .ok_or_else(|| ExecutionError::InvalidConfiguration("Concurrency config required for parallel execution".to_string()))?;
        
        let semaphore = Arc::new(Semaphore::new(concurrency_config.max_concurrent));
        let rate_limiter = if let Some(rate) = concurrency_config.rate_limit {
            Some(Arc::new(RateLimiter::new(rate)))
        } else {
            None
        };
        
        let execution_start = Instant::now();
        let mut tasks = Vec::new();
        
        for test_case in test_cases {
            let permit = semaphore.clone().acquire_owned().await
                .map_err(|e| ExecutionError::ResourceError(e.to_string()))?;
            
            let context_clone = context.clone();
            let rate_limiter_clone = rate_limiter.clone();
            let event_bus = self.event_bus.clone();
            let metrics = self.metrics.clone();
            
            let task = tokio::spawn(async move {
                let _permit = permit; // Hold permit until task completes
                
                // Apply rate limiting
                if let Some(limiter) = rate_limiter_clone {
                    limiter.wait().await;
                }
                
                // Execute test (similar to serial executor but isolated)
                let serial_executor = SerialExecutor::new(event_bus, metrics);
                serial_executor.execute_single_test(&test_case, &context_clone).await
            });
            
            tasks.push(task);
        }
        
        // Wait for all tasks to complete
        let mut results = Vec::new();
        for task in tasks {
            match task.await {
                Ok(Ok(result)) => results.push(result),
                Ok(Err(e)) => {
                    // Create error result
                    results.push(TestResult {
                        test_id: "unknown".to_string(),
                        test_name: "unknown".to_string(),
                        status: TestStatus::Error,
                        duration: Duration::ZERO,
                        assertion_results: Vec::new(),
                        extracted_variables: HashMap::new(),
                        request_response: None,
                        error: Some(TestError {
                            message: e.to_string(),
                            details: None,
                        }),
                    });
                }
                Err(e) => {
                    // Task panicked
                    results.push(TestResult {
                        test_id: "unknown".to_string(),
                        test_name: "unknown".to_string(),
                        status: TestStatus::Error,
                        duration: Duration::ZERO,
                        assertion_results: Vec::new(),
                        extracted_variables: HashMap::new(),
                        request_response: None,
                        error: Some(TestError {
                            message: format!("Task panicked: {}", e),
                            details: None,
                        }),
                    });
                }
            }
        }
        
        let total_duration = execution_start.elapsed();
        
        // Calculate summary
        let summary = ExecutionSummary {
            total_tests: results.len(),
            passed_tests: results.iter().filter(|r| r.status == TestStatus::Passed).count(),
            failed_tests: results.iter().filter(|r| r.status == TestStatus::Failed).count(),
            skipped_tests: 0,
            error_tests: results.iter().filter(|r| r.status == TestStatus::Error).count(),
            total_duration,
            success_rate: if results.is_empty() {
                0.0
            } else {
                results.iter().filter(|r| r.status == TestStatus::Passed).count() as f64 / results.len() as f64
            },
            start_time: Utc::now() - chrono::Duration::from_std(total_duration).unwrap(),
            end_time: Utc::now(),
        };
        
        Ok(ExecutionResult {
            execution_id: context.execution_id,
            summary,
            test_results: results,
            metrics: self.metrics.get_performance_summary(),
            metadata: HashMap::new(),
        })
    }
    
    fn name(&self) -> &str {
        "parallel"
    }
    
    fn config_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "concurrency": {
                    "type": "object",
                    "properties": {
                        "max_concurrent": {
                            "type": "integer",
                            "minimum": 1,
                            "default": 10,
                            "description": "Maximum number of concurrent test executions"
                        },
                        "rate_limit": {
                            "type": "number",
                            "minimum": 0.1,
                            "description": "Rate limit in requests per second"
                        },
                        "connection_pool_size": {
                            "type": "integer",
                            "minimum": 1,
                            "description": "Connection pool size for HTTP clients"
                        }
                    },
                    "required": ["max_concurrent"]
                }
            },
            "required": ["concurrency"]
        })
    }
    
    fn validate_config(&self, config: &ExecutionConfig) -> Result<(), ExecutionError> {
        if config.mode != ExecutionMode::Parallel {
            return Err(ExecutionError::InvalidConfiguration(
                "Parallel executor requires Parallel execution mode".to_string()
            ));
        }
        
        if config.concurrency.is_none() {
            return Err(ExecutionError::InvalidConfiguration(
                "Concurrency configuration required for parallel execution".to_string()
            ));
        }
        
        Ok(())
    }
}
```

## Interactive Execution Strategy

Provides step-by-step execution with debugging capabilities.

```rust
pub struct InteractiveExecutor {
    event_bus: Arc<EventBus>,
    metrics: Arc<MetricsCollector>,
    debugger: TestDebugger,
}

impl InteractiveExecutor {
    pub fn new(event_bus: Arc<EventBus>, metrics: Arc<MetricsCollector>) -> Self {
        Self {
            event_bus,
            metrics,
            debugger: TestDebugger::new(),
        }
    }
    
    async fn execute_with_debugging(
        &self,
        test_case: &TestCase,
        context: &ExecutionContext,
    ) -> Result<TestResult, ExecutionError> {
        let interactive_config = context.config.interactive
            .as_ref()
            .ok_or_else(|| ExecutionError::InvalidConfiguration("Interactive config required".to_string()))?;
        
        if interactive_config.dry_run {
            return self.dry_run_test(test_case, context).await;
        }
        
        if interactive_config.step_by_step {
            self.debugger.wait_for_user_input(&format!("About to execute test: {}", test_case.name)).await?;
        }
        
        // Execute test with debugging information
        let result = self.execute_single_test_with_debug(test_case, context, interactive_config).await?;
        
        if interactive_config.inspect_variables {
            self.debugger.display_variables(&result.extracted_variables).await?;
        }
        
        Ok(result)
    }
    
    async fn dry_run_test(
        &self,
        test_case: &TestCase,
        context: &ExecutionContext,
    ) -> Result<TestResult, ExecutionError> {
        // Validate test case without executing
        println!("DRY RUN: {}", test_case.name);
        println!("  Request: {} {}", test_case.request.method, test_case.request.url);
        println!("  Assertions: {}", test_case.assertions.len());
        
        // Simulate successful execution
        Ok(TestResult {
            test_id: test_case.id.clone(),
            test_name: test_case.name.clone(),
            status: TestStatus::Passed,
            duration: Duration::ZERO,
            assertion_results: Vec::new(),
            extracted_variables: HashMap::new(),
            request_response: None,
            error: None,
        })
    }
    
    async fn execute_single_test_with_debug(
        &self,
        test_case: &TestCase,
        context: &ExecutionContext,
        interactive_config: &InteractiveConfig,
    ) -> Result<TestResult, ExecutionError> {
        // Similar to serial execution but with debugging hooks
        let serial_executor = SerialExecutor::new(self.event_bus.clone(), self.metrics.clone());
        
        if interactive_config.step_by_step {
            println!("Executing request...");
        }
        
        let result = serial_executor.execute_single_test(test_case, context).await?;
        
        if interactive_config.step_by_step {
            println!("Test completed with status: {:?}", result.status);
            if let Some(req_resp) = &result.request_response {
                println!("Response status: {}", req_resp.response.status_code);
                println!("Response time: {:?}", result.duration);
            }
        }
        
        Ok(result)
    }
}

#[async_trait]
impl ExecutionStrategy for InteractiveExecutor {
    async fn execute(
        &self,
        test_cases: Vec<TestCase>,
        context: ExecutionContext,
    ) -> Result<ExecutionResult, ExecutionError> {
        let interactive_config = context.config.interactive
            .as_ref()
            .ok_or_else(|| ExecutionError::InvalidConfiguration("Interactive config required".to_string()))?;
        
        let mut results = Vec::new();
        let execution_start = Instant::now();
        
        for (index, test_case) in test_cases.iter().enumerate() {
            // Check for breakpoints
            if interactive_config.breakpoints.contains(&index) {
                self.debugger.wait_for_user_input(&format!("Breakpoint at test {}: {}", index, test_case.name)).await?;
            }
            
            let result = self.execute_with_debugging(test_case, &context).await?;
            results.push(result);
            
            // Stop on failure if configured
            if context.config.stop_on_failure && results.last().unwrap().status == TestStatus::Failed {
                println!("Stopping execution due to test failure");
                break;
            }
        }
        
        let total_duration = execution_start.elapsed();
        
        // Calculate summary
        let summary = ExecutionSummary {
            total_tests: results.len(),
            passed_tests: results.iter().filter(|r| r.status == TestStatus::Passed).count(),
            failed_tests: results.iter().filter(|r| r.status == TestStatus::Failed).count(),
            skipped_tests: 0,
            error_tests: results.iter().filter(|r| r.status == TestStatus::Error).count(),
            total_duration,
            success_rate: if results.is_empty() {
                0.0
            } else {
                results.iter().filter(|r| r.status == TestStatus::Passed).count() as f64 / results.len() as f64
            },
            start_time: Utc::now() - chrono::Duration::from_std(total_duration).unwrap(),
            end_time: Utc::now(),
        };
        
        Ok(ExecutionResult {
            execution_id: context.execution_id,
            summary,
            test_results: results,
            metrics: self.metrics.get_performance_summary(),
            metadata: HashMap::new(),
        })
    }
    
    fn name(&self) -> &str {
        "interactive"
    }
    
    fn config_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "interactive": {
                    "type": "object",
                    "properties": {
                        "step_by_step": {
                            "type": "boolean",
                            "default": false,
                            "description": "Enable step-by-step execution with user prompts"
                        },
                        "breakpoints": {
                            "type": "array",
                            "items": {"type": "integer"},
                            "description": "Test indices where execution should pause"
                        },
                        "inspect_variables": {
                            "type": "boolean",
                            "default": false,
                            "description": "Display extracted variables after each test"
                        },
                        "dry_run": {
                            "type": "boolean",
                            "default": false,
                            "description": "Validate tests without executing requests"
                        }
                    }
                }
            },
            "required": ["interactive"]
        })
    }
    
    fn validate_config(&self, config: &ExecutionConfig) -> Result<(), ExecutionError> {
        if config.mode != ExecutionMode::Interactive {
            return Err(ExecutionError::InvalidConfiguration(
                "Interactive executor requires Interactive execution mode".to_string()
            ));
        }
        
        if config.interactive.is_none() {
            return Err(ExecutionError::InvalidConfiguration(
                "Interactive configuration required for interactive execution".to_string()
            ));
        }
        
        Ok(())
    }
}
```

## Resource Manager

Manages system resources during test execution.

```rust
pub struct ResourceManager {
    cpu_monitor: CpuMonitor,
    memory_monitor: MemoryMonitor,
    network_monitor: NetworkMonitor,
    limits: ResourceLimits,
}

#[derive(Debug, Clone)]
pub struct ResourceLimits {
    pub max_cpu_usage: f64,      // Percentage (0.0 - 100.0)
    pub max_memory_usage: u64,   // Bytes
    pub max_network_bandwidth: u64, // Bytes per second
    pub max_concurrent_connections: usize,
}

impl ResourceManager {
    pub fn new() -> Self {
        Self {
            cpu_monitor: CpuMonitor::new(),
            memory_monitor: MemoryMonitor::new(),
            network_monitor: NetworkMonitor::new(),
            limits: ResourceLimits::default(),
        }
    }
    
    /// Check if resources are within limits
    pub async fn check_resource_limits(&self) -> Result<ResourceStatus, ResourceError> {
        let cpu_usage = self.cpu_monitor.get_usage().await?;
        let memory_usage = self.memory_monitor.get_usage().await?;
        let network_usage = self.network_monitor.get_usage().await?;
        
        let mut violations = Vec::new();
        
        if cpu_usage > self.limits.max_cpu_usage {
            violations.push(ResourceViolation::CpuExceeded { current: cpu_usage, limit: self.limits.max_cpu_usage });
        }
        
        if memory_usage > self.limits.max_memory_usage {
            violations.push(ResourceViolation::MemoryExceeded { current: memory_usage, limit: self.limits.max_memory_usage });
        }
        
        Ok(ResourceStatus {
            cpu_usage,
            memory_usage,
            network_usage,
            violations,
        })
    }
    
    /// Wait for resources to become available
    pub async fn wait_for_resources(&self) -> Result<(), ResourceError> {
        loop {
            let status = self.check_resource_limits().await?;
            if status.violations.is_empty() {
                break;
            }
            
            // Wait before checking again
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        Ok(())
    }
}
```

## Error Types

```rust
#[derive(Debug, thiserror::Error)]
pub enum ExecutionError {
    #[error("Unsupported execution mode: {0:?}")]
    UnsupportedMode(ExecutionMode),
    
    #[error("Plugin not found: {0}")]
    PluginNotFound(String),
    
    #[error("Request failed: {0}")]
    RequestFailed(String),
    
    #[error("Assertion failed: {0}")]
    AssertionFailed(String),
    
    #[error("Invalid configuration: {0}")]
    InvalidConfiguration(String),
    
    #[error("Resource error: {0}")]
    ResourceError(String),
    
    #[error("Event error: {0}")]
    EventError(String),
    
    #[error("Timeout error: {0}")]
    TimeoutError(String),
    
    #[error("Cancellation error: {0}")]
    CancellationError(String),
    
    #[error("Unsupported operation: {0}")]
    UnsupportedOperation(String),
}

#[derive(Debug, thiserror::Error)]
pub enum ResourceError {
    #[error("Resource monitoring error: {0}")]
    MonitoringError(String),
    
    #[error("Resource limit exceeded: {0}")]
    LimitExceeded(String),
    
    #[error("Resource unavailable: {0}")]
    Unavailable(String),
}
```

## Usage Examples

### Basic Execution

```rust
use api_test_runner::execution::{ExecutionEngine, ExecutionConfig, ExecutionMode};

let execution_engine = ExecutionEngine::new(event_bus, metrics_collector, plugin_manager);

let config = ExecutionConfig {
    mode: ExecutionMode::Serial,
    environment: "development".to_string(),
    variables: HashMap::new(),
    timeout: Some(Duration::from_secs(300)),
    stop_on_failure: false,
    max_retries: 3,
    retry_delay: Duration::from_secs(1),
    concurrency: None,
    distributed: None,
    interactive: None,
};

let result = execution_engine.execute_tests(test_cases, config).await?;
println!("Execution completed: {} passed, {} failed", 
         result.summary.passed_tests, result.summary.failed_tests);
```

### Parallel Execution

```rust
let config = ExecutionConfig {
    mode: ExecutionMode::Parallel,
    environment: "staging".to_string(),
    variables: HashMap::new(),
    timeout: Some(Duration::from_secs(600)),
    stop_on_failure: false,
    max_retries: 2,
    retry_delay: Duration::from_secs(2),
    concurrency: Some(ConcurrencyConfig {
        max_concurrent: 20,
        rate_limit: Some(100.0), // 100 requests per second
        connection_pool_size: Some(50),
    }),
    distributed: None,
    interactive: None,
};

let result = execution_engine.execute_tests(test_cases, config).await?;
```

### Interactive Execution

```rust
let config = ExecutionConfig {
    mode: ExecutionMode::Interactive,
    environment: "development".to_string(),
    variables: HashMap::new(),
    timeout: None,
    stop_on_failure: true,
    max_retries: 0,
    retry_delay: Duration::from_secs(1),
    concurrency: None,
    distributed: None,
    interactive: Some(InteractiveConfig {
        step_by_step: true,
        breakpoints: vec![0, 5, 10], // Pause at tests 0, 5, and 10
        inspect_variables: true,
        dry_run: false,
    }),
};

let result = execution_engine.execute_tests(test_cases, config).await?;
```

### Custom Execution Strategy

```rust
struct CustomExecutor;

#[async_trait]
impl ExecutionStrategy for CustomExecutor {
    async fn execute(
        &self,
        test_cases: Vec<TestCase>,
        context: ExecutionContext,
    ) -> Result<ExecutionResult, ExecutionError> {
        // Custom execution logic
        todo!()
    }
    
    fn name(&self) -> &str {
        "custom"
    }
    
    fn config_schema(&self) -> serde_json::Value {
        json!({})
    }
    
    fn validate_config(&self, config: &ExecutionConfig) -> Result<(), ExecutionError> {
        Ok(())
    }
}

// Register custom strategy
let mut execution_engine = ExecutionEngine::new(event_bus, metrics_collector, plugin_manager);
execution_engine.register_strategy(ExecutionMode::Custom("custom".to_string()), Box::new(CustomExecutor));
```