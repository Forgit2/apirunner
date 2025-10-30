//! Interactive result analysis with filtering, sorting, and comparison capabilities
//! 
//! This module provides:
//! - Result exploration with advanced filtering and sorting
//! - Detailed failure analysis with request/response inspection
//! - Result comparison and diff visualization
//! - Interactive result browsing and navigation

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::fmt;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::error::{ExecutionError, Result};
use crate::execution::TestResult;
use crate::interactive_execution::InteractiveExecutionResult;

/// Interactive result analyzer for exploring and comparing test results
pub struct ResultAnalyzer {
    result_store: Arc<RwLock<ResultStore>>,
    comparison_engine: ComparisonEngine,
    filter_engine: FilterEngine,
}

/// Storage for test results with indexing and search capabilities
#[derive(Debug)]
pub struct ResultStore {
    results: HashMap<String, AnalyzableResult>,
    execution_history: Vec<ExecutionSession>,
    indices: ResultIndices,
}

/// Indices for fast result lookup and filtering
#[derive(Debug)]
pub struct ResultIndices {
    by_test_name: HashMap<String, Vec<String>>,
    by_status: HashMap<ResultStatus, Vec<String>>,
    by_execution_time: BTreeMap<DateTime<Utc>, Vec<String>>,
    by_duration: BTreeMap<u64, Vec<String>>, // Duration in milliseconds
    by_error_type: HashMap<String, Vec<String>>,
}

/// Analyzable result with enhanced metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalyzableResult {
    pub result_id: String,
    pub execution_session_id: String,
    pub test_case_id: String,
    pub test_case_name: String,
    pub status: ResultStatus,
    pub execution_time: DateTime<Utc>,
    pub duration: std::time::Duration,
    pub interactive_result: Option<InteractiveExecutionResult>,
    pub standard_result: Option<TestResult>,
    pub failure_analysis: Option<FailureAnalysis>,
    pub performance_metrics: PerformanceMetrics,
    pub tags: Vec<String>,
}

/// Status of test result
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ResultStatus {
    Passed,
    Failed,
    Skipped,
    Cancelled,
    Error,
}

/// Execution session containing multiple test results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionSession {
    pub session_id: String,
    pub session_name: String,
    pub start_time: DateTime<Utc>,
    pub end_time: Option<DateTime<Utc>>,
    pub total_tests: usize,
    pub passed_tests: usize,
    pub failed_tests: usize,
    pub environment: String,
    pub configuration: HashMap<String, String>,
    pub result_ids: Vec<String>,
}

/// Detailed failure analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailureAnalysis {
    pub failure_type: FailureType,
    pub root_cause: String,
    pub failed_assertions: Vec<FailedAssertion>,
    pub error_context: ErrorContext,
    pub suggested_fixes: Vec<String>,
    pub related_failures: Vec<String>, // IDs of related failed results
}

/// Type of failure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FailureType {
    AssertionFailure,
    NetworkError,
    TimeoutError,
    AuthenticationError,
    ValidationError,
    SystemError,
    UnknownError,
}

/// Failed assertion details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailedAssertion {
    pub assertion_type: String,
    pub expected_value: serde_json::Value,
    pub actual_value: serde_json::Value,
    pub difference: ValueDifference,
    pub assertion_path: String,
}

/// Error context for debugging
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorContext {
    pub request_details: RequestInspection,
    pub response_details: Option<ResponseInspection>,
    pub environment_state: HashMap<String, String>,
    pub timing_breakdown: TimingBreakdown,
}

/// Request inspection details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestInspection {
    pub method: String,
    pub url: String,
    pub headers: HashMap<String, String>,
    pub body: Option<String>,
    pub resolved_variables: HashMap<String, String>,
    pub authentication: Option<String>,
}

/// Response inspection details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseInspection {
    pub status_code: u16,
    pub headers: HashMap<String, String>,
    pub body: String,
    pub body_size: usize,
    pub content_type: Option<String>,
    pub encoding: Option<String>,
}

/// Timing breakdown for performance analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimingBreakdown {
    pub dns_lookup: std::time::Duration,
    pub tcp_connect: std::time::Duration,
    pub tls_handshake: Option<std::time::Duration>,
    pub request_send: std::time::Duration,
    pub response_receive: std::time::Duration,
    pub total_time: std::time::Duration,
}

/// Performance metrics for result analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    pub response_time: std::time::Duration,
    pub throughput: Option<f64>,
    pub memory_usage: Option<u64>,
    pub cpu_usage: Option<f64>,
    pub network_bytes_sent: Option<u64>,
    pub network_bytes_received: Option<u64>,
}

/// Value difference for assertion comparison
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValueDifference {
    pub difference_type: DifferenceType,
    pub path: String,
    pub expected: serde_json::Value,
    pub actual: serde_json::Value,
    pub message: String,
}

/// Type of value difference
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DifferenceType {
    TypeMismatch,
    ValueMismatch,
    MissingField,
    ExtraField,
    ArrayLengthMismatch,
    ArrayElementMismatch,
}

/// Filter criteria for result exploration
#[derive(Debug, Clone, Default)]
pub struct ResultFilter {
    pub status: Option<ResultStatus>,
    pub test_name_pattern: Option<String>,
    pub execution_time_range: Option<(DateTime<Utc>, DateTime<Utc>)>,
    pub duration_range: Option<(std::time::Duration, std::time::Duration)>,
    pub error_type: Option<String>,
    pub tags: Vec<String>,
    pub has_failure_analysis: Option<bool>,
}

/// Sort criteria for result ordering
#[derive(Debug, Clone)]
pub struct ResultSort {
    pub field: SortField,
    pub direction: SortDirection,
}

/// Fields available for sorting
#[derive(Debug, Clone)]
pub enum SortField {
    ExecutionTime,
    Duration,
    TestName,
    Status,
    FailureCount,
}

/// Sort direction
#[derive(Debug, Clone)]
pub enum SortDirection {
    Ascending,
    Descending,
}

/// Result comparison configuration
#[derive(Debug, Clone)]
pub struct ComparisonConfig {
    pub compare_requests: bool,
    pub compare_responses: bool,
    pub compare_assertions: bool,
    pub compare_performance: bool,
    pub ignore_timing_differences: bool,
    pub ignore_dynamic_fields: Vec<String>,
}

/// Comparison result between two test results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComparisonResult {
    pub result1_id: String,
    pub result2_id: String,
    pub overall_similarity: f64,
    pub request_differences: Vec<RequestDifference>,
    pub response_differences: Vec<ResponseDifference>,
    pub assertion_differences: Vec<AssertionDifference>,
    pub performance_differences: Vec<PerformanceDifference>,
    pub summary: ComparisonSummary,
}

/// Request difference details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestDifference {
    pub field: String,
    pub value1: serde_json::Value,
    pub value2: serde_json::Value,
    pub significance: DifferenceSignificance,
}

/// Response difference details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseDifference {
    pub field: String,
    pub value1: serde_json::Value,
    pub value2: serde_json::Value,
    pub significance: DifferenceSignificance,
}

/// Assertion difference details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssertionDifference {
    pub assertion_name: String,
    pub result1: bool,
    pub result2: bool,
    pub details: String,
}

/// Performance difference details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceDifference {
    pub metric: String,
    pub value1: f64,
    pub value2: f64,
    pub percentage_change: f64,
}

/// Significance of a difference
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DifferenceSignificance {
    Critical,
    Important,
    Minor,
    Negligible,
}

/// Summary of comparison results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComparisonSummary {
    pub total_differences: usize,
    pub critical_differences: usize,
    pub important_differences: usize,
    pub minor_differences: usize,
    pub categories_compared: Vec<String>,
    pub recommendation: String,
}

/// Engine for comparing test results
pub struct ComparisonEngine {
    config: ComparisonConfig,
}

/// Engine for filtering and searching results
pub struct FilterEngine {
    // Internal state for filtering operations
}

impl ResultAnalyzer {
    pub fn new() -> Self {
        Self {
            result_store: Arc::new(RwLock::new(ResultStore::new())),
            comparison_engine: ComparisonEngine::new(),
            filter_engine: FilterEngine::new(),
        }
    }

    /// Add a result to the analyzer
    pub async fn add_result(&self, result: AnalyzableResult) -> Result<()> {
        let mut store = self.result_store.write().await;
        store.add_result(result).await
    }

    /// Add an interactive execution result
    pub async fn add_interactive_result(
        &self,
        interactive_result: InteractiveExecutionResult,
        session_id: String,
    ) -> Result<String> {
        let result_id = uuid::Uuid::new_v4().to_string();
        
        let failure_analysis = if !interactive_result.success {
            Some(self.analyze_failure(&interactive_result).await?)
        } else {
            None
        };

        let performance_metrics = self.calculate_performance_metrics(&interactive_result);

        let analyzable_result = AnalyzableResult {
            result_id: result_id.clone(),
            execution_session_id: session_id,
            test_case_id: interactive_result.test_case_id.clone(),
            test_case_name: interactive_result.test_case_name.clone(),
            status: if interactive_result.success {
                ResultStatus::Passed
            } else {
                ResultStatus::Failed
            },
            execution_time: Utc::now(),
            duration: interactive_result.total_duration,
            interactive_result: Some(interactive_result),
            standard_result: None,
            failure_analysis,
            performance_metrics,
            tags: Vec::new(),
        };

        self.add_result(analyzable_result).await?;
        Ok(result_id)
    }

    /// Filter results based on criteria
    pub async fn filter_results(&self, filter: ResultFilter) -> Result<Vec<AnalyzableResult>> {
        let store = self.result_store.read().await;
        self.filter_engine.apply_filter(&store, filter).await
    }

    /// Sort results based on criteria
    pub async fn sort_results(
        &self,
        results: Vec<AnalyzableResult>,
        sort: ResultSort,
    ) -> Vec<AnalyzableResult> {
        self.filter_engine.sort_results(results, sort).await
    }

    /// Compare two results
    pub async fn compare_results(
        &self,
        result1_id: &str,
        result2_id: &str,
        config: ComparisonConfig,
    ) -> Result<ComparisonResult> {
        let store = self.result_store.read().await;
        let result1 = store.get_result(result1_id)
            .ok_or_else(|| ExecutionError::TestCaseFailed(format!("Result not found: {}", result1_id)))?;
        let result2 = store.get_result(result2_id)
            .ok_or_else(|| ExecutionError::TestCaseFailed(format!("Result not found: {}", result2_id)))?;

        self.comparison_engine.compare(result1, result2, config).await
    }

    /// Get detailed failure analysis for a result
    pub async fn get_failure_analysis(&self, result_id: &str) -> Result<Option<FailureAnalysis>> {
        let store = self.result_store.read().await;
        if let Some(result) = store.get_result(result_id) {
            Ok(result.failure_analysis.clone())
        } else {
            Err(ExecutionError::TestCaseFailed(format!("Result not found: {}", result_id)).into())
        }
    }

    /// Get execution session summary
    pub async fn get_session_summary(&self, session_id: &str) -> Result<ExecutionSession> {
        let store = self.result_store.read().await;
        store.get_session(session_id)
            .ok_or_else(|| ExecutionError::TestCaseFailed(format!("Session not found: {}", session_id)).into())
    }

    /// Analyze failure and generate detailed analysis
    async fn analyze_failure(&self, result: &InteractiveExecutionResult) -> Result<FailureAnalysis> {
        let mut failed_assertions = Vec::new();
        let mut failure_type = FailureType::UnknownError;
        let mut root_cause = "Unknown failure".to_string();

        // Analyze step results for failures
        for step in &result.step_results {
            if !step.success {
                if let Some(error) = &step.error_message {
                    // Determine failure type based on error message
                    failure_type = self.classify_error(error);
                    root_cause = error.clone();
                }

                // Analyze failed assertions
                for assertion_result in &step.assertion_results {
                    if !assertion_result.success {
                        let failed_assertion = FailedAssertion {
                            assertion_type: assertion_result.assertion_type.clone(),
                            expected_value: assertion_result.expected_value.clone().unwrap_or_default(),
                            actual_value: assertion_result.actual_value.clone().unwrap_or_default(),
                            difference: self.calculate_value_difference(
                                &assertion_result.expected_value.clone().unwrap_or_default(),
                                &assertion_result.actual_value.clone().unwrap_or_default(),
                            ),
                            assertion_path: "".to_string(), // AssertionResult doesn't have path field
                        };
                        failed_assertions.push(failed_assertion);
                    }
                }
            }
        }

        // Create error context
        let error_context = self.create_error_context(result).await?;

        // Generate suggested fixes
        let suggested_fixes = self.generate_suggested_fixes(&failure_type, &failed_assertions);

        Ok(FailureAnalysis {
            failure_type,
            root_cause,
            failed_assertions,
            error_context,
            suggested_fixes,
            related_failures: Vec::new(), // TODO: Implement related failure detection
        })
    }

    /// Classify error type based on error message
    fn classify_error(&self, error_message: &str) -> FailureType {
        let error_lower = error_message.to_lowercase();
        
        if error_lower.contains("assertion") || error_lower.contains("expected") {
            FailureType::AssertionFailure
        } else if error_lower.contains("timeout") || error_lower.contains("timed out") {
            FailureType::TimeoutError
        } else if error_lower.contains("network") || error_lower.contains("connection") {
            FailureType::NetworkError
        } else if error_lower.contains("auth") || error_lower.contains("unauthorized") {
            FailureType::AuthenticationError
        } else if error_lower.contains("validation") || error_lower.contains("invalid") {
            FailureType::ValidationError
        } else {
            FailureType::SystemError
        }
    }

    /// Calculate difference between two values
    fn calculate_value_difference(
        &self,
        expected: &serde_json::Value,
        actual: &serde_json::Value,
    ) -> ValueDifference {
        let difference_type = match (expected, actual) {
            (serde_json::Value::String(_), serde_json::Value::String(_)) => DifferenceType::ValueMismatch,
            (serde_json::Value::Number(_), serde_json::Value::Number(_)) => DifferenceType::ValueMismatch,
            (serde_json::Value::Bool(_), serde_json::Value::Bool(_)) => DifferenceType::ValueMismatch,
            (serde_json::Value::Array(_), serde_json::Value::Array(_)) => DifferenceType::ArrayElementMismatch,
            (serde_json::Value::Object(_), serde_json::Value::Object(_)) => DifferenceType::ValueMismatch,
            (serde_json::Value::Null, serde_json::Value::Null) => DifferenceType::ValueMismatch,
            _ => DifferenceType::TypeMismatch,
        };

        ValueDifference {
            difference_type,
            path: "".to_string(),
            expected: expected.clone(),
            actual: actual.clone(),
            message: format!("Expected {:?}, but got {:?}", expected, actual),
        }
    }

    /// Create error context for debugging
    async fn create_error_context(&self, result: &InteractiveExecutionResult) -> Result<ErrorContext> {
        let mut request_details = RequestInspection {
            method: "GET".to_string(),
            url: "".to_string(),
            headers: HashMap::new(),
            body: None,
            resolved_variables: HashMap::new(),
            authentication: None,
        };

        let mut response_details = None;

        // Extract request and response details from step results
        for step in &result.step_results {
            if let Some(request) = &step.request {
                request_details = RequestInspection {
                    method: request.method.clone(),
                    url: request.url.clone(),
                    headers: request.headers.clone(),
                    body: request.body.as_ref().map(|v| v.to_string()),
                    resolved_variables: step.variables_extracted.clone(),
                    authentication: None, // TODO: Extract auth info
                };
            }

            if let Some(response) = &step.response {
                response_details = Some(ResponseInspection {
                    status_code: response.status_code,
                    headers: response.headers.clone(),
                    body: String::from_utf8_lossy(&response.body).to_string(),
                    body_size: response.body.len(),
                    content_type: response.headers.get("content-type").cloned(),
                    encoding: None,
                });
            }
        }

        let timing_breakdown = TimingBreakdown {
            dns_lookup: std::time::Duration::from_millis(10),
            tcp_connect: std::time::Duration::from_millis(20),
            tls_handshake: Some(std::time::Duration::from_millis(50)),
            request_send: std::time::Duration::from_millis(5),
            response_receive: std::time::Duration::from_millis(100),
            total_time: result.total_duration,
        };

        Ok(ErrorContext {
            request_details,
            response_details,
            environment_state: HashMap::new(),
            timing_breakdown,
        })
    }

    /// Generate suggested fixes based on failure analysis
    fn generate_suggested_fixes(
        &self,
        failure_type: &FailureType,
        failed_assertions: &[FailedAssertion],
    ) -> Vec<String> {
        let mut suggestions = Vec::new();

        match failure_type {
            FailureType::AssertionFailure => {
                suggestions.push("Review assertion criteria and expected values".to_string());
                suggestions.push("Check if API response format has changed".to_string());
                for assertion in failed_assertions {
                    suggestions.push(format!(
                        "Update {} assertion: expected {}, got {}",
                        assertion.assertion_type,
                        assertion.expected_value,
                        assertion.actual_value
                    ));
                }
            }
            FailureType::NetworkError => {
                suggestions.push("Check network connectivity".to_string());
                suggestions.push("Verify API endpoint URL".to_string());
                suggestions.push("Check firewall and proxy settings".to_string());
            }
            FailureType::TimeoutError => {
                suggestions.push("Increase timeout values".to_string());
                suggestions.push("Check API server performance".to_string());
                suggestions.push("Consider using retry mechanisms".to_string());
            }
            FailureType::AuthenticationError => {
                suggestions.push("Verify authentication credentials".to_string());
                suggestions.push("Check if tokens have expired".to_string());
                suggestions.push("Review authentication configuration".to_string());
            }
            FailureType::ValidationError => {
                suggestions.push("Check request data format".to_string());
                suggestions.push("Validate required fields are present".to_string());
                suggestions.push("Review API documentation for changes".to_string());
            }
            _ => {
                suggestions.push("Review error logs for more details".to_string());
                suggestions.push("Check system resources and dependencies".to_string());
            }
        }

        suggestions
    }

    /// Calculate performance metrics from execution result
    fn calculate_performance_metrics(&self, result: &InteractiveExecutionResult) -> PerformanceMetrics {
        PerformanceMetrics {
            response_time: result.total_duration,
            throughput: Some(1.0 / result.total_duration.as_secs_f64()),
            memory_usage: None,
            cpu_usage: None,
            network_bytes_sent: None,
            network_bytes_received: None,
        }
    }
}

impl ResultStore {
    pub fn new() -> Self {
        Self {
            results: HashMap::new(),
            execution_history: Vec::new(),
            indices: ResultIndices::new(),
        }
    }

    pub async fn add_result(&mut self, result: AnalyzableResult) -> Result<()> {
        let result_id = result.result_id.clone();
        
        // Update indices
        self.indices.add_result(&result);
        
        // Store result
        self.results.insert(result_id, result);
        
        Ok(())
    }

    pub fn get_result(&self, result_id: &str) -> Option<&AnalyzableResult> {
        self.results.get(result_id)
    }

    pub fn get_session(&self, session_id: &str) -> Option<ExecutionSession> {
        self.execution_history.iter()
            .find(|session| session.session_id == session_id)
            .cloned()
    }
}

impl ResultIndices {
    pub fn new() -> Self {
        Self {
            by_test_name: HashMap::new(),
            by_status: HashMap::new(),
            by_execution_time: BTreeMap::new(),
            by_duration: BTreeMap::new(),
            by_error_type: HashMap::new(),
        }
    }

    pub fn add_result(&mut self, result: &AnalyzableResult) {
        // Index by test name
        self.by_test_name
            .entry(result.test_case_name.clone())
            .or_insert_with(Vec::new)
            .push(result.result_id.clone());

        // Index by status
        self.by_status
            .entry(result.status.clone())
            .or_insert_with(Vec::new)
            .push(result.result_id.clone());

        // Index by execution time
        self.by_execution_time
            .entry(result.execution_time)
            .or_insert_with(Vec::new)
            .push(result.result_id.clone());

        // Index by duration
        self.by_duration
            .entry(result.duration.as_millis() as u64)
            .or_insert_with(Vec::new)
            .push(result.result_id.clone());

        // Index by error type if failed
        if let Some(failure_analysis) = &result.failure_analysis {
            let error_type = format!("{:?}", failure_analysis.failure_type);
            self.by_error_type
                .entry(error_type)
                .or_insert_with(Vec::new)
                .push(result.result_id.clone());
        }
    }
}

impl ComparisonEngine {
    pub fn new() -> Self {
        Self {
            config: ComparisonConfig {
                compare_requests: true,
                compare_responses: true,
                compare_assertions: true,
                compare_performance: true,
                ignore_timing_differences: false,
                ignore_dynamic_fields: vec!["timestamp".to_string(), "id".to_string()],
            },
        }
    }

    pub async fn compare(
        &self,
        result1: &AnalyzableResult,
        result2: &AnalyzableResult,
        config: ComparisonConfig,
    ) -> Result<ComparisonResult> {
        let mut request_differences = Vec::new();
        let mut response_differences = Vec::new();
        let mut assertion_differences = Vec::new();
        let mut performance_differences = Vec::new();

        // Compare requests if enabled
        if config.compare_requests {
            request_differences = self.compare_requests(result1, result2).await?;
        }

        // Compare responses if enabled
        if config.compare_responses {
            response_differences = self.compare_responses(result1, result2).await?;
        }

        // Compare assertions if enabled
        if config.compare_assertions {
            assertion_differences = self.compare_assertions(result1, result2).await?;
        }

        // Compare performance if enabled
        if config.compare_performance && !config.ignore_timing_differences {
            performance_differences = self.compare_performance(result1, result2).await?;
        }

        let total_differences = request_differences.len() + response_differences.len() + 
                              assertion_differences.len() + performance_differences.len();

        let critical_differences = request_differences.iter()
            .filter(|d| matches!(d.significance, DifferenceSignificance::Critical))
            .count() + 
            response_differences.iter()
            .filter(|d| matches!(d.significance, DifferenceSignificance::Critical))
            .count();

        let overall_similarity = if total_differences == 0 {
            1.0
        } else {
            1.0 - (critical_differences as f64 / total_differences as f64)
        };

        let summary = ComparisonSummary {
            total_differences,
            critical_differences,
            important_differences: 0, // TODO: Calculate
            minor_differences: 0,     // TODO: Calculate
            categories_compared: vec!["requests".to_string(), "responses".to_string()],
            recommendation: if overall_similarity > 0.9 {
                "Results are very similar".to_string()
            } else if overall_similarity > 0.7 {
                "Results have some differences".to_string()
            } else {
                "Results are significantly different".to_string()
            },
        };

        Ok(ComparisonResult {
            result1_id: result1.result_id.clone(),
            result2_id: result2.result_id.clone(),
            overall_similarity,
            request_differences,
            response_differences,
            assertion_differences,
            performance_differences,
            summary,
        })
    }

    async fn compare_requests(
        &self,
        _result1: &AnalyzableResult,
        _result2: &AnalyzableResult,
    ) -> Result<Vec<RequestDifference>> {
        // TODO: Implement request comparison
        Ok(Vec::new())
    }

    async fn compare_responses(
        &self,
        _result1: &AnalyzableResult,
        _result2: &AnalyzableResult,
    ) -> Result<Vec<ResponseDifference>> {
        // TODO: Implement response comparison
        Ok(Vec::new())
    }

    async fn compare_assertions(
        &self,
        _result1: &AnalyzableResult,
        _result2: &AnalyzableResult,
    ) -> Result<Vec<AssertionDifference>> {
        // TODO: Implement assertion comparison
        Ok(Vec::new())
    }

    async fn compare_performance(
        &self,
        result1: &AnalyzableResult,
        result2: &AnalyzableResult,
    ) -> Result<Vec<PerformanceDifference>> {
        let mut differences = Vec::new();

        let duration1 = result1.duration.as_millis() as f64;
        let duration2 = result2.duration.as_millis() as f64;
        
        if (duration1 - duration2).abs() > 100.0 { // More than 100ms difference
            let percentage_change = ((duration2 - duration1) / duration1) * 100.0;
            differences.push(PerformanceDifference {
                metric: "response_time".to_string(),
                value1: duration1,
                value2: duration2,
                percentage_change,
            });
        }

        Ok(differences)
    }
}

impl FilterEngine {
    pub fn new() -> Self {
        Self {}
    }

    pub async fn apply_filter(
        &self,
        store: &ResultStore,
        filter: ResultFilter,
    ) -> Result<Vec<AnalyzableResult>> {
        let mut results = Vec::new();

        for result in store.results.values() {
            if self.matches_filter(result, &filter) {
                results.push(result.clone());
            }
        }

        Ok(results)
    }

    pub async fn sort_results(
        &self,
        mut results: Vec<AnalyzableResult>,
        sort: ResultSort,
    ) -> Vec<AnalyzableResult> {
        results.sort_by(|a, b| {
            let comparison = match sort.field {
                SortField::ExecutionTime => a.execution_time.cmp(&b.execution_time),
                SortField::Duration => a.duration.cmp(&b.duration),
                SortField::TestName => a.test_case_name.cmp(&b.test_case_name),
                SortField::Status => format!("{:?}", a.status).cmp(&format!("{:?}", b.status)),
                SortField::FailureCount => {
                    let a_failures = if matches!(a.status, ResultStatus::Failed) { 1 } else { 0 };
                    let b_failures = if matches!(b.status, ResultStatus::Failed) { 1 } else { 0 };
                    a_failures.cmp(&b_failures)
                }
            };

            match sort.direction {
                SortDirection::Ascending => comparison,
                SortDirection::Descending => comparison.reverse(),
            }
        });

        results
    }

    fn matches_filter(&self, result: &AnalyzableResult, filter: &ResultFilter) -> bool {
        // Check status filter
        if let Some(status) = &filter.status {
            if &result.status != status {
                return false;
            }
        }

        // Check test name pattern
        if let Some(pattern) = &filter.test_name_pattern {
            if !result.test_case_name.contains(pattern) {
                return false;
            }
        }

        // Check execution time range
        if let Some((start, end)) = &filter.execution_time_range {
            if result.execution_time < *start || result.execution_time > *end {
                return false;
            }
        }

        // Check duration range
        if let Some((min_duration, max_duration)) = &filter.duration_range {
            if result.duration < *min_duration || result.duration > *max_duration {
                return false;
            }
        }

        // Check failure analysis requirement
        if let Some(has_failure_analysis) = filter.has_failure_analysis {
            if has_failure_analysis && result.failure_analysis.is_none() {
                return false;
            }
            if !has_failure_analysis && result.failure_analysis.is_some() {
                return false;
            }
        }

        true
    }
}

impl fmt::Display for ResultStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ResultStatus::Passed => write!(f, "✅ Passed"),
            ResultStatus::Failed => write!(f, "❌ Failed"),
            ResultStatus::Skipped => write!(f, "⏭️ Skipped"),
            ResultStatus::Cancelled => write!(f, "🚫 Cancelled"),
            ResultStatus::Error => write!(f, "💥 Error"),
        }
    }
}

/// Interactive result browser for exploring test results
pub struct ResultBrowser {
    analyzer: Arc<ResultAnalyzer>,
    current_session: Option<String>,
    current_filter: ResultFilter,
    current_sort: ResultSort,
    display_config: DisplayConfig,
}

/// Configuration for result display
#[derive(Debug, Clone)]
pub struct DisplayConfig {
    pub show_request_details: bool,
    pub show_response_details: bool,
    pub show_timing_breakdown: bool,
    pub show_assertion_details: bool,
    pub max_body_length: usize,
    pub highlight_differences: bool,
}

impl Default for DisplayConfig {
    fn default() -> Self {
        Self {
            show_request_details: true,
            show_response_details: true,
            show_timing_breakdown: true,
            show_assertion_details: true,
            max_body_length: 1000,
            highlight_differences: true,
        }
    }
}

impl ResultBrowser {
    pub fn new(analyzer: Arc<ResultAnalyzer>) -> Self {
        Self {
            analyzer,
            current_session: None,
            current_filter: ResultFilter::default(),
            current_sort: ResultSort {
                field: SortField::ExecutionTime,
                direction: SortDirection::Descending,
            },
            display_config: DisplayConfig::default(),
        }
    }

    /// Browse results with current filter and sort settings
    pub async fn browse_results(&self) -> Result<Vec<AnalyzableResult>> {
        let filtered_results = self.analyzer.filter_results(self.current_filter.clone()).await?;
        let sorted_results = self.analyzer.sort_results(filtered_results, self.current_sort.clone()).await;
        Ok(sorted_results)
    }

    /// Set current filter
    pub fn set_filter(&mut self, filter: ResultFilter) {
        self.current_filter = filter;
    }

    /// Set current sort
    pub fn set_sort(&mut self, sort: ResultSort) {
        self.current_sort = sort;
    }

    /// Set display configuration
    pub fn set_display_config(&mut self, config: DisplayConfig) {
        self.display_config = config;
    }

    /// Display result details
    pub async fn display_result_details(&self, result_id: &str) -> Result<String> {
        let store = self.analyzer.result_store.read().await;
        let result = store.get_result(result_id)
            .ok_or_else(|| ExecutionError::TestCaseFailed(format!("Result not found: {}", result_id)))?;

        let mut output = String::new();
        
        // Basic information
        output.push_str(&format!("📊 Test Result: {}\n", result.test_case_name));
        output.push_str(&format!("🆔 ID: {}\n", result.result_id));
        output.push_str(&format!("📅 Executed: {}\n", result.execution_time.format("%Y-%m-%d %H:%M:%S UTC")));
        output.push_str(&format!("⏱️  Duration: {}ms\n", result.duration.as_millis()));
        output.push_str(&format!("📈 Status: {:?}\n", result.status));
        output.push_str("\n");

        // Add more details based on display config
        if self.display_config.show_request_details {
            output.push_str("📤 Request details would be shown here\n");
        }

        if self.display_config.show_response_details {
            output.push_str("📥 Response details would be shown here\n");
        }

        if self.display_config.show_assertion_details {
            output.push_str("✅ Assertion details would be shown here\n");
        }

        if let Some(failure_analysis) = &result.failure_analysis {
            output.push_str("🔍 Failure Analysis:\n");
            output.push_str(&format!("   Type: {:?}\n", failure_analysis.failure_type));
            output.push_str(&format!("   Root Cause: {}\n", failure_analysis.root_cause));
            
            if !failure_analysis.suggested_fixes.is_empty() {
                output.push_str("   Suggested Fixes:\n");
                for fix in &failure_analysis.suggested_fixes {
                    output.push_str(&format!("     • {}\n", fix));
                }
            }
            output.push_str("\n");
        }

        Ok(output)
    }

    /// Compare two results and display differences
    pub async fn compare_and_display(
        &self,
        result1_id: &str,
        result2_id: &str,
    ) -> Result<String> {
        let comparison = self.analyzer.compare_results(
            result1_id,
            result2_id,
            ComparisonConfig {
                compare_requests: true,
                compare_responses: true,
                compare_assertions: true,
                compare_performance: true,
                ignore_timing_differences: false,
                ignore_dynamic_fields: vec!["timestamp".to_string()],
            },
        ).await?;

        let mut output = String::new();
        
        output.push_str(&format!("🔍 Comparison Results\n"));
        output.push_str(&format!("📊 Overall Similarity: {:.1}%\n", comparison.overall_similarity * 100.0));
        output.push_str(&format!("📈 Total Differences: {}\n", comparison.summary.total_differences));
        output.push_str(&format!("🚨 Critical Differences: {}\n", comparison.summary.critical_differences));
        output.push_str(&format!("💡 Recommendation: {}\n", comparison.summary.recommendation));
        output.push_str("\n");

        Ok(output)
    }

    /// Generate result summary statistics
    pub async fn generate_summary(&self) -> Result<String> {
        let results = self.browse_results().await?;
        
        let total_results = results.len();
        let passed_results = results.iter().filter(|r| r.status == ResultStatus::Passed).count();
        let failed_results = results.iter().filter(|r| r.status == ResultStatus::Failed).count();
        let error_results = results.iter().filter(|r| r.status == ResultStatus::Error).count();
        
        let avg_duration = if !results.is_empty() {
            results.iter().map(|r| r.duration.as_millis()).sum::<u128>() / results.len() as u128
        } else {
            0
        };

        let success_rate = if total_results > 0 {
            (passed_results as f64 / total_results as f64) * 100.0
        } else {
            0.0
        };

        let mut output = String::new();
        output.push_str("📊 Result Summary\n");
        output.push_str(&format!("📈 Total Results: {}\n", total_results));
        output.push_str(&format!("✅ Passed: {} ({:.1}%)\n", passed_results, success_rate));
        output.push_str(&format!("❌ Failed: {}\n", failed_results));
        output.push_str(&format!("🚨 Errors: {}\n", error_results));
        output.push_str(&format!("⏱️  Average Duration: {}ms\n", avg_duration));
        output.push_str(&format!("🎯 Success Rate: {:.1}%\n", success_rate));

        Ok(output)
    }
}

impl fmt::Display for AnalyzableResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let status_icon = match self.status {
            ResultStatus::Passed => "✅",
            ResultStatus::Failed => "❌",
            ResultStatus::Skipped => "⏭️",
            ResultStatus::Cancelled => "🚫",
            ResultStatus::Error => "🚨",
        };

        write!(
            f,
            "{} {} ({}) - {}ms [{}]",
            status_icon,
            self.test_case_name,
            self.result_id[..8].to_string(),
            self.duration.as_millis(),
            self.execution_time.format("%H:%M:%S")
        )
    }
}

impl fmt::Display for FailureType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FailureType::AssertionFailure => write!(f, "Assertion Failure"),
            FailureType::NetworkError => write!(f, "Network Error"),
            FailureType::TimeoutError => write!(f, "Timeout Error"),
            FailureType::AuthenticationError => write!(f, "Authentication Error"),
            FailureType::ValidationError => write!(f, "Validation Error"),
            FailureType::SystemError => write!(f, "System Error"),
            FailureType::UnknownError => write!(f, "Unknown Error"),
        }
    }
} 