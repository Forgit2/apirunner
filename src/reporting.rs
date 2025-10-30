use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;
use thiserror::Error;

pub mod junit_xml;
// pub mod html;  // Temporarily disabled due to template syntax issues
pub mod json;
pub mod template;



#[derive(Debug, Error)]
pub enum ReportingError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Serialization error: {0}")]
    Serialization(String),
    #[error("Template error: {0}")]
    Template(String),
    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),
}

#[async_trait]
pub trait Reporter: Send + Sync {
    fn name(&self) -> &str;
    fn supported_formats(&self) -> Vec<ReportFormat>;
    async fn generate_report(&self, results: &TestExecutionResults, config: &ReportConfig) -> Result<ReportOutput, ReportingError>;
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ReportFormat {
    JunitXml,
    Html,
    Json,
    Markdown,
    Custom(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportConfig {
    pub output_path: PathBuf,
    pub format: ReportFormat,
    pub include_environment_info: bool,
    pub include_performance_metrics: bool,
    pub include_request_response_details: bool,
    pub template_path: Option<PathBuf>,
    pub custom_metadata: HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct ReportOutput {
    pub content: Vec<u8>,
    pub content_type: String,
    pub file_extension: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestExecutionResults {
    pub execution_id: String,
    pub test_suite_name: String,
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub total_duration: Duration,
    pub environment: EnvironmentInfo,
    pub test_results: Vec<TestResult>,
    pub summary: ExecutionSummary,
    pub performance_metrics: PerformanceMetrics,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvironmentInfo {
    pub hostname: String,
    pub os: String,
    pub arch: String,
    pub rust_version: String,
    pub runner_version: String,
    pub environment_variables: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestResult {
    pub test_id: String,
    pub test_name: String,
    pub test_description: Option<String>,
    pub status: TestStatus,
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub duration: Duration,
    pub request_details: RequestDetails,
    pub response_details: Option<ResponseDetails>,
    pub assertions: Vec<AssertionResult>,
    pub error_message: Option<String>,
    pub error_details: Option<String>,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TestStatus {
    Passed,
    Failed,
    Skipped,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestDetails {
    pub method: String,
    pub url: String,
    pub headers: HashMap<String, String>,
    pub body: Option<String>,
    pub protocol_version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseDetails {
    pub status_code: u16,
    pub headers: HashMap<String, String>,
    pub body: Option<String>,
    pub size_bytes: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssertionResult {
    pub assertion_type: String,
    pub success: bool,
    pub message: String,
    pub expected_value: Option<serde_json::Value>,
    pub actual_value: Option<serde_json::Value>,
    pub duration: Duration,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionSummary {
    pub total_tests: usize,
    pub passed_tests: usize,
    pub failed_tests: usize,
    pub skipped_tests: usize,
    pub error_tests: usize,
    pub success_rate: f64,
    pub total_assertions: usize,
    pub passed_assertions: usize,
    pub failed_assertions: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    pub total_requests: usize,
    pub requests_per_second: f64,
    pub average_response_time: Duration,
    pub min_response_time: Duration,
    pub max_response_time: Duration,
    pub percentile_50: Duration,
    pub percentile_90: Duration,
    pub percentile_95: Duration,
    pub percentile_99: Duration,
    pub error_rate: f64,
    pub throughput_bytes_per_second: f64,
}

impl Default for ReportConfig {
    fn default() -> Self {
        Self {
            output_path: PathBuf::from("test-results"),
            format: ReportFormat::JunitXml,
            include_environment_info: true,
            include_performance_metrics: true,
            include_request_response_details: false,
            template_path: None,
            custom_metadata: HashMap::new(),
        }
    }
}

impl TestExecutionResults {
    pub fn new(execution_id: String, test_suite_name: String) -> Self {
        let now = Utc::now();
        Self {
            execution_id,
            test_suite_name,
            start_time: now,
            end_time: now,
            total_duration: Duration::from_secs(0),
            environment: EnvironmentInfo::current(),
            test_results: Vec::new(),
            summary: ExecutionSummary::default(),
            performance_metrics: PerformanceMetrics::default(),
        }
    }

    pub fn calculate_summary(&mut self) {
        let total_tests = self.test_results.len();
        let passed_tests = self.test_results.iter().filter(|r| matches!(r.status, TestStatus::Passed)).count();
        let failed_tests = self.test_results.iter().filter(|r| matches!(r.status, TestStatus::Failed)).count();
        let skipped_tests = self.test_results.iter().filter(|r| matches!(r.status, TestStatus::Skipped)).count();
        let error_tests = self.test_results.iter().filter(|r| matches!(r.status, TestStatus::Error)).count();

        let total_assertions: usize = self.test_results.iter().map(|r| r.assertions.len()).sum();
        let passed_assertions: usize = self.test_results.iter()
            .flat_map(|r| &r.assertions)
            .filter(|a| a.success)
            .count();
        let failed_assertions = total_assertions - passed_assertions;

        let success_rate = if total_tests > 0 {
            (passed_tests as f64 / total_tests as f64) * 100.0
        } else {
            0.0
        };

        self.summary = ExecutionSummary {
            total_tests,
            passed_tests,
            failed_tests,
            skipped_tests,
            error_tests,
            success_rate,
            total_assertions,
            passed_assertions,
            failed_assertions,
        };
    }

    pub fn calculate_performance_metrics(&mut self) {
        if self.test_results.is_empty() {
            return;
        }

        let mut response_times: Vec<Duration> = self.test_results.iter()
            .map(|r| r.duration)
            .collect();
        response_times.sort();

        let total_requests = self.test_results.len();
        let failed_requests = self.test_results.iter()
            .filter(|r| matches!(r.status, TestStatus::Failed | TestStatus::Error))
            .count();

        let total_duration_secs = self.total_duration.as_secs_f64();
        let requests_per_second = if total_duration_secs > 0.0 {
            total_requests as f64 / total_duration_secs
        } else {
            0.0
        };

        let average_response_time = Duration::from_nanos(
            (response_times.iter().map(|d| d.as_nanos()).sum::<u128>() / total_requests as u128) as u64
        );

        let min_response_time = response_times.first().copied().unwrap_or_default();
        let max_response_time = response_times.last().copied().unwrap_or_default();

        let percentile_50 = response_times.get(total_requests * 50 / 100).copied().unwrap_or_default();
        let percentile_90 = response_times.get(total_requests * 90 / 100).copied().unwrap_or_default();
        let percentile_95 = response_times.get(total_requests * 95 / 100).copied().unwrap_or_default();
        let percentile_99 = response_times.get(total_requests * 99 / 100).copied().unwrap_or_default();

        let error_rate = (failed_requests as f64 / total_requests as f64) * 100.0;

        // Calculate throughput based on response sizes
        let total_bytes: usize = self.test_results.iter()
            .filter_map(|r| r.response_details.as_ref())
            .map(|rd| rd.size_bytes)
            .sum();
        let throughput_bytes_per_second = if total_duration_secs > 0.0 {
            total_bytes as f64 / total_duration_secs
        } else {
            0.0
        };

        self.performance_metrics = PerformanceMetrics {
            total_requests,
            requests_per_second,
            average_response_time,
            min_response_time,
            max_response_time,
            percentile_50,
            percentile_90,
            percentile_95,
            percentile_99,
            error_rate,
            throughput_bytes_per_second,
        };
    }
}

impl EnvironmentInfo {
    pub fn current() -> Self {
        Self {
            hostname: std::env::var("HOSTNAME").unwrap_or_else(|_| "unknown".to_string()),
            os: std::env::consts::OS.to_string(),
            arch: std::env::consts::ARCH.to_string(),
            rust_version: env!("CARGO_PKG_RUST_VERSION").to_string(),
            runner_version: env!("CARGO_PKG_VERSION").to_string(),
            environment_variables: std::env::vars()
                .filter(|(key, _)| key.starts_with("TEST_") || key.starts_with("API_"))
                .collect(),
        }
    }
}

impl Default for ExecutionSummary {
    fn default() -> Self {
        Self {
            total_tests: 0,
            passed_tests: 0,
            failed_tests: 0,
            skipped_tests: 0,
            error_tests: 0,
            success_rate: 0.0,
            total_assertions: 0,
            passed_assertions: 0,
            failed_assertions: 0,
        }
    }
}

impl Default for PerformanceMetrics {
    fn default() -> Self {
        Self {
            total_requests: 0,
            requests_per_second: 0.0,
            average_response_time: Duration::from_secs(0),
            min_response_time: Duration::from_secs(0),
            max_response_time: Duration::from_secs(0),
            percentile_50: Duration::from_secs(0),
            percentile_90: Duration::from_secs(0),
            percentile_95: Duration::from_secs(0),
            percentile_99: Duration::from_secs(0),
            error_rate: 0.0,
            throughput_bytes_per_second: 0.0,
        }
    }
}

pub struct ReportManager {
    reporters: HashMap<String, Box<dyn Reporter>>,
}

impl ReportManager {
    pub fn new() -> Self {
        let mut manager = Self {
            reporters: HashMap::new(),
        };
        
        // Register default reporters
        manager.register_default_reporters();
        manager
    }

    fn register_default_reporters(&mut self) {
        // Register JUnit XML reporter
        self.reporters.insert("junit-xml".to_string(), Box::new(self::junit_xml::JunitXmlReporter::new()));
        
        // Register HTML reporter
        // TODO: Fix HTML reporter compilation issue
        // if let Ok(html_reporter) = self::html::HtmlReporter::new() {
        //     self.reporters.insert("html".to_string(), Box::new(html_reporter));
        // }
        
        // Register JSON reporters
        self.reporters.insert("json".to_string(), Box::new(self::json::JsonReporter::new()));
        self.reporters.insert("json-stream".to_string(), Box::new(self::json::JsonReporter::with_streaming()));
        
        // Register Markdown reporter
        if let Ok(markdown_reporter) = self::template::MarkdownReporter::new() {
            self.reporters.insert("markdown".to_string(), Box::new(markdown_reporter));
        }
    }

    pub fn register_reporter(&mut self, name: String, reporter: Box<dyn Reporter>) {
        self.reporters.insert(name, reporter);
    }

    pub fn register_custom_template_reporter(&mut self, name: String, template_content: &str, format: ReportFormat) -> Result<(), ReportingError> {
        let reporter = self::template::TemplateReporter::new(template_content, &name, format)?;
        self.reporters.insert(name.clone(), Box::new(reporter));
        Ok(())
    }

    pub fn register_template_file_reporter<P: AsRef<std::path::Path>>(&mut self, name: String, template_path: P, format: ReportFormat) -> Result<(), ReportingError> {
        let reporter = self::template::TemplateReporter::from_file(template_path, &name, format)?;
        self.reporters.insert(name.clone(), Box::new(reporter));
        Ok(())
    }

    pub fn get_reporter(&self, name: &str) -> Option<&dyn Reporter> {
        self.reporters.get(name).map(|r| r.as_ref())
    }

    pub fn list_reporters(&self) -> Vec<&str> {
        self.reporters.keys().map(|s| s.as_str()).collect()
    }

    pub fn get_supported_formats(&self, reporter_name: &str) -> Option<Vec<ReportFormat>> {
        self.get_reporter(reporter_name).map(|r| r.supported_formats())
    }

    pub async fn generate_report(&self, reporter_name: &str, results: &TestExecutionResults, config: &ReportConfig) -> Result<ReportOutput, ReportingError> {
        let reporter = self.get_reporter(reporter_name)
            .ok_or_else(|| ReportingError::InvalidConfig(format!("Reporter '{}' not found", reporter_name)))?;
        
        reporter.generate_report(results, config).await
    }

    pub async fn generate_multiple_reports(&self, reporter_names: &[&str], results: &TestExecutionResults, config: &ReportConfig) -> Result<Vec<(String, ReportOutput)>, ReportingError> {
        let mut reports = Vec::new();
        
        for &name in reporter_names {
            let report = self.generate_report(name, results, config).await?;
            reports.push((name.to_string(), report));
        }
        
        Ok(reports)
    }

    pub async fn save_report<P: AsRef<std::path::Path>>(&self, reporter_name: &str, results: &TestExecutionResults, config: &ReportConfig, output_dir: P) -> Result<std::path::PathBuf, ReportingError> {
        let report = self.generate_report(reporter_name, results, config).await?;
        
        let output_dir = output_dir.as_ref();
        std::fs::create_dir_all(output_dir).map_err(ReportingError::Io)?;
        
        let filename = format!("{}-{}.{}", 
            results.test_suite_name.replace(' ', "-").to_lowercase(),
            results.execution_id,
            report.file_extension
        );
        
        let file_path = output_dir.join(filename);
        std::fs::write(&file_path, report.content).map_err(ReportingError::Io)?;
        
        Ok(file_path)
    }

    pub async fn save_multiple_reports<P: AsRef<std::path::Path>>(&self, reporter_names: &[&str], results: &TestExecutionResults, config: &ReportConfig, output_dir: P) -> Result<Vec<std::path::PathBuf>, ReportingError> {
        let mut file_paths = Vec::new();
        
        for &name in reporter_names {
            let file_path = self.save_report(name, results, config, &output_dir).await?;
            file_paths.push(file_path);
        }
        
        Ok(file_paths)
    }
}

impl Default for ReportManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tempfile::TempDir;

    fn create_test_results() -> TestExecutionResults {
        let mut results = TestExecutionResults::new(
            "manager-test-execution".to_string(),
            "Report Manager Test Suite".to_string(),
        );

        results.test_results.push(TestResult {
            test_id: "manager-test-1".to_string(),
            test_name: "GET /api/test".to_string(),
            test_description: Some("Test endpoint for report manager".to_string()),
            status: TestStatus::Passed,
            start_time: Utc::now(),
            end_time: Utc::now(),
            duration: Duration::from_millis(150),
            request_details: RequestDetails {
                method: "GET".to_string(),
                url: "https://api.example.com/test".to_string(),
                headers: [("Accept".to_string(), "application/json".to_string())].into(),
                body: None,
                protocol_version: Some("HTTP/1.1".to_string()),
            },
            response_details: Some(ResponseDetails {
                status_code: 200,
                headers: [("Content-Type".to_string(), "application/json".to_string())].into(),
                body: Some(r#"{"result": "success"}"#.to_string()),
                size_bytes: 20,
            }),
            assertions: vec![
                AssertionResult {
                    assertion_type: "status_code".to_string(),
                    success: true,
                    message: "Status code is 200".to_string(),
                    expected_value: Some(serde_json::json!(200)),
                    actual_value: Some(serde_json::json!(200)),
                    duration: Duration::from_millis(1),
                }
            ],
            error_message: None,
            error_details: None,
            tags: vec!["test".to_string()],
        });

        results.total_duration = Duration::from_millis(150);
        results.end_time = results.start_time + chrono::Duration::milliseconds(150);
        results.calculate_summary();
        results.calculate_performance_metrics();

        results
    }

    #[tokio::test]
    async fn test_report_manager_default_reporters() {
        let manager = ReportManager::new();
        let reporters = manager.list_reporters();
        
        assert!(reporters.contains(&"junit-xml"));
        // TODO: Re-enable when HTML reporter compilation is fixed
        // assert!(reporters.contains(&"html"));
        assert!(reporters.contains(&"json"));
        assert!(reporters.contains(&"json-stream"));
        assert!(reporters.contains(&"markdown"));
    }

    #[tokio::test]
    async fn test_generate_junit_xml_report() {
        let manager = ReportManager::new();
        let results = create_test_results();
        let config = ReportConfig::default();

        let report = manager.generate_report("junit-xml", &results, &config).await.unwrap();
        let xml_content = String::from_utf8(report.content).unwrap();

        assert!(xml_content.contains("<?xml version=\"1.0\" encoding=\"UTF-8\"?>"));
        assert!(xml_content.contains("<testsuites"));
        assert_eq!(report.content_type, "application/xml");
        assert_eq!(report.file_extension, "xml");
    }

    // TODO: Re-enable when HTML reporter compilation is fixed
    // #[tokio::test]
    // async fn test_generate_html_report() {
    //     let manager = ReportManager::new();
    //     let results = create_test_results();
    //     let config = ReportConfig::default();

    //     let report = manager.generate_report("html", &results, &config).await.unwrap();
    //     let html_content = String::from_utf8(report.content).unwrap();

    //     assert!(html_content.contains("<!DOCTYPE html>"));
    //     assert!(html_content.contains("API Test Report"));
    //     assert_eq!(report.content_type, "text/html");
    //     assert_eq!(report.file_extension, "html");
    // }

    #[tokio::test]
    async fn test_generate_json_report() {
        let manager = ReportManager::new();
        let results = create_test_results();
        let config = ReportConfig::default();

        let report = manager.generate_report("json", &results, &config).await.unwrap();
        let json_content = String::from_utf8(report.content).unwrap();
        
        let parsed: serde_json::Value = serde_json::from_str(&json_content).unwrap();
        assert!(parsed["report_metadata"].is_object());
        assert_eq!(report.content_type, "application/json");
        assert_eq!(report.file_extension, "json");
    }

    #[tokio::test]
    async fn test_generate_markdown_report() {
        let manager = ReportManager::new();
        let results = create_test_results();
        let config = ReportConfig::default();

        let report = manager.generate_report("markdown", &results, &config).await.unwrap();
        let markdown_content = String::from_utf8(report.content).unwrap();

        assert!(markdown_content.contains("# API Test Report"));
        assert!(markdown_content.contains("Report Manager Test Suite"));
        assert_eq!(report.content_type, "text/markdown");
        assert_eq!(report.file_extension, "md");
    }

    #[tokio::test]
    async fn test_generate_multiple_reports() {
        let manager = ReportManager::new();
        let results = create_test_results();
        let config = ReportConfig::default();

        let reports = manager.generate_multiple_reports(&["junit-xml", "json", "markdown"], &results, &config).await.unwrap();
        
        assert_eq!(reports.len(), 3);
        assert_eq!(reports[0].0, "junit-xml");
        assert_eq!(reports[1].0, "json");
        assert_eq!(reports[2].0, "markdown");
    }

    #[tokio::test]
    async fn test_custom_template_reporter() {
        let mut manager = ReportManager::new();
        let custom_template = "Test Suite: {{execution.suite_name}}\nTotal Tests: {{summary.total_tests}}";
        
        manager.register_custom_template_reporter(
            "custom".to_string(),
            custom_template,
            ReportFormat::Custom("txt".to_string())
        ).unwrap();

        let results = create_test_results();
        let config = ReportConfig::default();

        let report = manager.generate_report("custom", &results, &config).await.unwrap();
        let content = String::from_utf8(report.content).unwrap();

        assert!(content.contains("Test Suite: Report Manager Test Suite"));
        assert!(content.contains("Total Tests: 1"));
    }

    #[tokio::test]
    async fn test_save_report() {
        let manager = ReportManager::new();
        let results = create_test_results();
        let config = ReportConfig::default();
        let temp_dir = TempDir::new().unwrap();

        let file_path = manager.save_report("json", &results, &config, temp_dir.path()).await.unwrap();
        
        assert!(file_path.exists());
        assert!(file_path.extension().unwrap() == "json");
        
        let content = std::fs::read_to_string(&file_path).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert!(parsed["report_metadata"].is_object());
    }

    #[tokio::test]
    async fn test_save_multiple_reports() {
        let manager = ReportManager::new();
        let results = create_test_results();
        let config = ReportConfig::default();
        let temp_dir = TempDir::new().unwrap();

        // TODO: Re-enable HTML when compilation is fixed
        let file_paths = manager.save_multiple_reports(&["junit-xml", "json", "markdown"], &results, &config, temp_dir.path()).await.unwrap();
        
        assert_eq!(file_paths.len(), 3);
        
        for path in &file_paths {
            assert!(path.exists());
        }
        
        // Check file extensions
        let extensions: Vec<_> = file_paths.iter()
            .map(|p| p.extension().unwrap().to_str().unwrap())
            .collect();
        assert!(extensions.contains(&"xml"));
        assert!(extensions.contains(&"json"));
        assert!(extensions.contains(&"md"));
    }

    #[tokio::test]
    async fn test_get_supported_formats() {
        let manager = ReportManager::new();
        
        let junit_formats = manager.get_supported_formats("junit-xml").unwrap();
        assert!(junit_formats.contains(&ReportFormat::JunitXml));
        
        // TODO: Re-enable when HTML reporter compilation is fixed
        // let html_formats = manager.get_supported_formats("html").unwrap();
        // assert!(html_formats.contains(&ReportFormat::Html));
        
        let json_formats = manager.get_supported_formats("json").unwrap();
        assert!(json_formats.contains(&ReportFormat::Json));
        
        let markdown_formats = manager.get_supported_formats("markdown").unwrap();
        assert!(markdown_formats.contains(&ReportFormat::Markdown));
    }

    #[tokio::test]
    async fn test_invalid_reporter() {
        let manager = ReportManager::new();
        let results = create_test_results();
        let config = ReportConfig::default();

        let result = manager.generate_report("nonexistent", &results, &config).await;
        assert!(result.is_err());
        
        if let Err(ReportingError::InvalidConfig(msg)) = result {
            assert!(msg.contains("Reporter 'nonexistent' not found"));
        } else {
            panic!("Expected InvalidConfig error");
        }
    }
}