# Reporter Plugins

Reporter plugins generate test reports in various formats for different audiences and integration needs. They provide flexible reporting capabilities with support for custom templates and real-time reporting.

## ReporterPlugin Trait

```rust
#[async_trait]
pub trait ReporterPlugin: Plugin {
    /// Return the report format this plugin generates
    fn report_format(&self) -> ReportFormat;
    
    /// Generate a report from test results
    async fn generate_report(&self, results: &TestResults, config: &ReportConfig) -> Result<Report, ReportError>;
    
    /// Stream real-time updates during test execution (optional)
    async fn stream_update(&self, update: &ExecutionUpdate) -> Result<(), ReportError> {
        Ok(()) // Default implementation does nothing
    }
    
    /// Finalize the report after all tests complete
    async fn finalize_report(&self, report: &mut Report) -> Result<(), ReportError> {
        Ok(()) // Default implementation does nothing
    }
}
```

## Report Formats

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum ReportFormat {
    /// JUnit XML format for CI/CD integration
    JunitXml,
    /// HTML format with interactive features
    Html,
    /// JSON format for programmatic processing
    Json,
    /// Plain text format
    Text,
    /// Markdown format
    Markdown,
    /// CSV format for data analysis
    Csv,
    /// Custom format
    Custom(String),
}
```

## Test Results

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestResults {
    /// Execution summary
    pub summary: ExecutionSummary,
    /// Individual test case results
    pub test_results: Vec<TestCaseResult>,
    /// Execution metadata
    pub metadata: ExecutionMetadata,
    /// Performance metrics
    pub metrics: PerformanceMetrics,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionSummary {
    /// Total number of tests executed
    pub total_tests: usize,
    /// Number of passed tests
    pub passed_tests: usize,
    /// Number of failed tests
    pub failed_tests: usize,
    /// Number of skipped tests
    pub skipped_tests: usize,
    /// Total execution time
    pub total_duration: Duration,
    /// Success rate (0.0 to 1.0)
    pub success_rate: f64,
    /// Execution start time
    pub start_time: DateTime<Utc>,
    /// Execution end time
    pub end_time: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestCaseResult {
    /// Test case information
    pub test_case: TestCaseInfo,
    /// Test execution status
    pub status: TestStatus,
    /// Test duration
    pub duration: Duration,
    /// Assertion results
    pub assertion_results: Vec<AssertionResult>,
    /// Error information (if failed)
    pub error: Option<TestError>,
    /// Request/response data
    pub request_response: Option<RequestResponseData>,
    /// Variables extracted during execution
    pub extracted_variables: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TestStatus {
    Passed,
    Failed,
    Skipped,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    /// Average response time
    pub avg_response_time: Duration,
    /// Minimum response time
    pub min_response_time: Duration,
    /// Maximum response time
    pub max_response_time: Duration,
    /// Response time percentiles
    pub percentiles: HashMap<String, Duration>, // "p50", "p95", "p99"
    /// Throughput (requests per second)
    pub throughput: f64,
    /// Error rate (0.0 to 1.0)
    pub error_rate: f64,
}
```

## Report Configuration

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportConfig {
    /// Output file path or directory
    pub output_path: PathBuf,
    /// Report title
    pub title: Option<String>,
    /// Include request/response data
    pub include_request_response: bool,
    /// Include performance metrics
    pub include_metrics: bool,
    /// Include variable data
    pub include_variables: bool,
    /// Custom template path
    pub template_path: Option<PathBuf>,
    /// Additional configuration options
    pub options: HashMap<String, serde_json::Value>,
}
```

## Report Output

```rust
#[derive(Debug, Clone)]
pub struct Report {
    /// Report format
    pub format: ReportFormat,
    /// Report content
    pub content: ReportContent,
    /// Output file paths
    pub output_files: Vec<PathBuf>,
    /// Report metadata
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub enum ReportContent {
    /// Text-based content
    Text(String),
    /// Binary content
    Binary(Vec<u8>),
    /// Multiple files
    Files(HashMap<String, Vec<u8>>),
}
```

## Built-in Reporter Plugins

### JUnit XML Reporter

```rust
pub struct JunitXmlReporter {
    template_engine: TemplateEngine,
}

#[async_trait]
impl ReporterPlugin for JunitXmlReporter {
    fn report_format(&self) -> ReportFormat {
        ReportFormat::JunitXml
    }
    
    async fn generate_report(&self, results: &TestResults, config: &ReportConfig) -> Result<Report, ReportError> {
        let mut xml = String::new();
        xml.push_str(r#"<?xml version="1.0" encoding="UTF-8"?>"#);
        xml.push('\n');
        
        // Root testsuite element
        xml.push_str(&format!(
            r#"<testsuite name="{}" tests="{}" failures="{}" errors="{}" skipped="{}" time="{:.3}">"#,
            config.title.as_deref().unwrap_or("API Tests"),
            results.summary.total_tests,
            results.summary.failed_tests,
            0, // JUnit doesn't distinguish between failures and errors
            results.summary.skipped_tests,
            results.summary.total_duration.as_secs_f64()
        ));
        xml.push('\n');
        
        // Properties section
        xml.push_str("  <properties>\n");
        xml.push_str(&format!(
            r#"    <property name="start_time" value="{}"/>"#,
            results.summary.start_time.to_rfc3339()
        ));
        xml.push('\n');
        xml.push_str(&format!(
            r#"    <property name="end_time" value="{}"/>"#,
            results.summary.end_time.to_rfc3339()
        ));
        xml.push('\n');
        xml.push_str(&format!(
            r#"    <property name="success_rate" value="{:.2}"/>"#,
            results.summary.success_rate * 100.0
        ));
        xml.push('\n');
        xml.push_str("  </properties>\n");
        
        // Test cases
        for test_result in &results.test_results {
            xml.push_str(&format!(
                r#"  <testcase name="{}" classname="{}" time="{:.3}""#,
                test_result.test_case.name,
                test_result.test_case.suite.as_deref().unwrap_or("default"),
                test_result.duration.as_secs_f64()
            ));
            
            match test_result.status {
                TestStatus::Passed => {
                    xml.push_str("/>\n");
                }
                TestStatus::Failed => {
                    xml.push_str(">\n");
                    if let Some(error) = &test_result.error {
                        xml.push_str(&format!(
                            r#"    <failure message="{}" type="AssertionError">{}</failure>"#,
                            escape_xml(&error.message),
                            escape_xml(&error.details.as_deref().unwrap_or(""))
                        ));
                        xml.push('\n');
                    }
                    xml.push_str("  </testcase>\n");
                }
                TestStatus::Skipped => {
                    xml.push_str(">\n");
                    xml.push_str("    <skipped/>\n");
                    xml.push_str("  </testcase>\n");
                }
                TestStatus::Error => {
                    xml.push_str(">\n");
                    if let Some(error) = &test_result.error {
                        xml.push_str(&format!(
                            r#"    <error message="{}" type="ExecutionError">{}</error>"#,
                            escape_xml(&error.message),
                            escape_xml(&error.details.as_deref().unwrap_or(""))
                        ));
                        xml.push('\n');
                    }
                    xml.push_str("  </testcase>\n");
                }
            }
        }
        
        xml.push_str("</testsuite>\n");
        
        let output_path = config.output_path.join("junit-report.xml");
        std::fs::write(&output_path, &xml)?;
        
        Ok(Report {
            format: ReportFormat::JunitXml,
            content: ReportContent::Text(xml),
            output_files: vec![output_path],
            metadata: HashMap::new(),
        })
    }
}

fn escape_xml(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
```

### HTML Reporter

```rust
pub struct HtmlReporter {
    template_engine: TemplateEngine,
}

#[async_trait]
impl ReporterPlugin for HtmlReporter {
    fn report_format(&self) -> ReportFormat {
        ReportFormat::Html
    }
    
    async fn generate_report(&self, results: &TestResults, config: &ReportConfig) -> Result<Report, ReportError> {
        let template_path = config.template_path.as_ref()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("templates/html_report.html"));
        
        let context = json!({
            "title": config.title.as_deref().unwrap_or("API Test Report"),
            "summary": results.summary,
            "test_results": results.test_results,
            "metrics": results.metrics,
            "metadata": results.metadata,
            "generated_at": Utc::now().to_rfc3339(),
            "include_request_response": config.include_request_response,
            "include_metrics": config.include_metrics,
            "include_variables": config.include_variables,
        });
        
        let html_content = self.template_engine.render(&template_path, &context)?;
        
        let output_path = config.output_path.join("report.html");
        std::fs::write(&output_path, &html_content)?;
        
        // Copy CSS and JS assets
        let assets_dir = config.output_path.join("assets");
        std::fs::create_dir_all(&assets_dir)?;
        
        let css_content = include_str!("../../templates/report.css");
        std::fs::write(assets_dir.join("report.css"), css_content)?;
        
        let js_content = include_str!("../../templates/report.js");
        std::fs::write(assets_dir.join("report.js"), js_content)?;
        
        Ok(Report {
            format: ReportFormat::Html,
            content: ReportContent::Text(html_content),
            output_files: vec![
                output_path,
                assets_dir.join("report.css"),
                assets_dir.join("report.js"),
            ],
            metadata: HashMap::new(),
        })
    }
}
```

### JSON Reporter

```rust
pub struct JsonReporter;

#[async_trait]
impl ReporterPlugin for JsonReporter {
    fn report_format(&self) -> ReportFormat {
        ReportFormat::Json
    }
    
    async fn generate_report(&self, results: &TestResults, config: &ReportConfig) -> Result<Report, ReportError> {
        let report_data = json!({
            "title": config.title.as_deref().unwrap_or("API Test Report"),
            "generated_at": Utc::now().to_rfc3339(),
            "summary": results.summary,
            "test_results": results.test_results,
            "metrics": results.metrics,
            "metadata": results.metadata,
        });
        
        let json_content = serde_json::to_string_pretty(&report_data)?;
        
        let output_path = config.output_path.join("report.json");
        std::fs::write(&output_path, &json_content)?;
        
        Ok(Report {
            format: ReportFormat::Json,
            content: ReportContent::Text(json_content),
            output_files: vec![output_path],
            metadata: HashMap::new(),
        })
    }
}
```

### Markdown Reporter

```rust
pub struct MarkdownReporter;

#[async_trait]
impl ReporterPlugin for MarkdownReporter {
    fn report_format(&self) -> ReportFormat {
        ReportFormat::Markdown
    }
    
    async fn generate_report(&self, results: &TestResults, config: &ReportConfig) -> Result<Report, ReportError> {
        let mut md = String::new();
        
        // Title and summary
        md.push_str(&format!("# {}\n\n", config.title.as_deref().unwrap_or("API Test Report")));
        md.push_str(&format!("**Generated:** {}\n\n", Utc::now().format("%Y-%m-%d %H:%M:%S UTC")));
        
        // Summary table
        md.push_str("## Summary\n\n");
        md.push_str("| Metric | Value |\n");
        md.push_str("|--------|-------|\n");
        md.push_str(&format!("| Total Tests | {} |\n", results.summary.total_tests));
        md.push_str(&format!("| Passed | {} |\n", results.summary.passed_tests));
        md.push_str(&format!("| Failed | {} |\n", results.summary.failed_tests));
        md.push_str(&format!("| Skipped | {} |\n", results.summary.skipped_tests));
        md.push_str(&format!("| Success Rate | {:.1}% |\n", results.summary.success_rate * 100.0));
        md.push_str(&format!("| Total Duration | {:.2}s |\n", results.summary.total_duration.as_secs_f64()));
        md.push('\n');
        
        // Performance metrics
        if config.include_metrics {
            md.push_str("## Performance Metrics\n\n");
            md.push_str("| Metric | Value |\n");
            md.push_str("|--------|-------|\n");
            md.push_str(&format!("| Average Response Time | {:.0}ms |\n", results.metrics.avg_response_time.as_millis()));
            md.push_str(&format!("| Min Response Time | {:.0}ms |\n", results.metrics.min_response_time.as_millis()));
            md.push_str(&format!("| Max Response Time | {:.0}ms |\n", results.metrics.max_response_time.as_millis()));
            md.push_str(&format!("| Throughput | {:.2} req/s |\n", results.metrics.throughput));
            md.push_str(&format!("| Error Rate | {:.1}% |\n", results.metrics.error_rate * 100.0));
            md.push('\n');
        }
        
        // Test results
        md.push_str("## Test Results\n\n");
        for test_result in &results.test_results {
            let status_emoji = match test_result.status {
                TestStatus::Passed => "✅",
                TestStatus::Failed => "❌",
                TestStatus::Skipped => "⏭️",
                TestStatus::Error => "💥",
            };
            
            md.push_str(&format!("### {} {} {}\n\n", status_emoji, test_result.test_case.name, 
                match test_result.status {
                    TestStatus::Passed => "(PASSED)",
                    TestStatus::Failed => "(FAILED)",
                    TestStatus::Skipped => "(SKIPPED)",
                    TestStatus::Error => "(ERROR)",
                }));
            
            md.push_str(&format!("**Duration:** {:.0}ms\n\n", test_result.duration.as_millis()));
            
            if let Some(description) = &test_result.test_case.description {
                md.push_str(&format!("**Description:** {}\n\n", description));
            }
            
            if let Some(error) = &test_result.error {
                md.push_str(&format!("**Error:** {}\n\n", error.message));
                if let Some(details) = &error.details {
                    md.push_str(&format!("**Details:**\n```\n{}\n```\n\n", details));
                }
            }
            
            // Assertion results
            if !test_result.assertion_results.is_empty() {
                md.push_str("**Assertions:**\n");
                for assertion in &test_result.assertion_results {
                    let assertion_status = if assertion.passed { "✅" } else { "❌" };
                    md.push_str(&format!("- {} {}\n", assertion_status, assertion.description));
                    if !assertion.passed {
                        if let Some(expected) = &assertion.expected {
                            md.push_str(&format!("  - Expected: `{}`\n", expected));
                        }
                        if let Some(actual) = &assertion.actual {
                            md.push_str(&format!("  - Actual: `{}`\n", actual));
                        }
                    }
                }
                md.push('\n');
            }
            
            // Request/Response data
            if config.include_request_response {
                if let Some(req_resp) = &test_result.request_response {
                    md.push_str("**Request:**\n");
                    md.push_str(&format!("```http\n{} {}\n", req_resp.request.method, req_resp.request.url));
                    for (key, value) in &req_resp.request.headers {
                        md.push_str(&format!("{}: {}\n", key, value));
                    }
                    if let Some(body) = &req_resp.request.body {
                        md.push_str(&format!("\n{}\n", body));
                    }
                    md.push_str("```\n\n");
                    
                    md.push_str("**Response:**\n");
                    md.push_str(&format!("```http\nHTTP/1.1 {}\n", req_resp.response.status));
                    for (key, value) in &req_resp.response.headers {
                        md.push_str(&format!("{}: {}\n", key, value));
                    }
                    if let Some(body) = &req_resp.response.body {
                        md.push_str(&format!("\n{}\n", body));
                    }
                    md.push_str("```\n\n");
                }
            }
        }
        
        let output_path = config.output_path.join("report.md");
        std::fs::write(&output_path, &md)?;
        
        Ok(Report {
            format: ReportFormat::Markdown,
            content: ReportContent::Text(md),
            output_files: vec![output_path],
            metadata: HashMap::new(),
        })
    }
}
```

## Real-time Reporting

Reporter plugins can implement real-time reporting by handling execution updates:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExecutionUpdate {
    /// Test execution started
    TestStarted {
        test_case: TestCaseInfo,
        timestamp: DateTime<Utc>,
    },
    /// Test execution completed
    TestCompleted {
        test_case: TestCaseInfo,
        result: TestCaseResult,
        timestamp: DateTime<Utc>,
    },
    /// Suite execution started
    SuiteStarted {
        suite_name: String,
        test_count: usize,
        timestamp: DateTime<Utc>,
    },
    /// Suite execution completed
    SuiteCompleted {
        suite_name: String,
        summary: SuiteSummary,
        timestamp: DateTime<Utc>,
    },
    /// Progress update
    Progress {
        completed: usize,
        total: usize,
        current_test: Option<String>,
        timestamp: DateTime<Utc>,
    },
}

// Example real-time console reporter
pub struct ConsoleReporter {
    progress_bar: Option<ProgressBar>,
}

#[async_trait]
impl ReporterPlugin for ConsoleReporter {
    fn report_format(&self) -> ReportFormat {
        ReportFormat::Text
    }
    
    async fn stream_update(&self, update: &ExecutionUpdate) -> Result<(), ReportError> {
        match update {
            ExecutionUpdate::TestStarted { test_case, .. } => {
                println!("🚀 Starting: {}", test_case.name);
            }
            ExecutionUpdate::TestCompleted { result, .. } => {
                let status_icon = match result.status {
                    TestStatus::Passed => "✅",
                    TestStatus::Failed => "❌",
                    TestStatus::Skipped => "⏭️",
                    TestStatus::Error => "💥",
                };
                println!("{} {} ({:.0}ms)", 
                    status_icon, 
                    result.test_case.name, 
                    result.duration.as_millis()
                );
            }
            ExecutionUpdate::Progress { completed, total, current_test, .. } => {
                if let Some(pb) = &self.progress_bar {
                    pb.set_position(*completed as u64);
                    if let Some(test) = current_test {
                        pb.set_message(format!("Running: {}", test));
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }
    
    async fn generate_report(&self, results: &TestResults, _config: &ReportConfig) -> Result<Report, ReportError> {
        // Generate final summary
        let summary = format!(
            "\n📊 Test Summary:\n  Total: {}\n  Passed: {}\n  Failed: {}\n  Skipped: {}\n  Success Rate: {:.1}%\n  Duration: {:.2}s\n",
            results.summary.total_tests,
            results.summary.passed_tests,
            results.summary.failed_tests,
            results.summary.skipped_tests,
            results.summary.success_rate * 100.0,
            results.summary.total_duration.as_secs_f64()
        );
        
        println!("{}", summary);
        
        Ok(Report {
            format: ReportFormat::Text,
            content: ReportContent::Text(summary),
            output_files: vec![],
            metadata: HashMap::new(),
        })
    }
}
```

## Custom Reporter Plugin Example

```rust
use crate::plugin::{Plugin, PluginInfo, PluginType};
use crate::reporting::{ReporterPlugin, ReportFormat, Report, ReportContent, ReportError};
use async_trait::async_trait;

pub struct SlackReporter {
    webhook_url: String,
    channel: String,
}

impl SlackReporter {
    pub fn new(webhook_url: String, channel: String) -> Self {
        Self { webhook_url, channel }
    }
}

#[async_trait]
impl Plugin for SlackReporter {
    fn info(&self) -> PluginInfo {
        PluginInfo {
            name: "slack_reporter".to_string(),
            version: "1.0.0".to_string(),
            description: "Sends test results to Slack".to_string(),
            plugin_type: PluginType::Reporter,
        }
    }
    
    async fn initialize(&mut self, _config: &serde_json::Value) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Validate webhook URL
        if self.webhook_url.is_empty() {
            return Err("Slack webhook URL is required".into());
        }
        Ok(())
    }
}

#[async_trait]
impl ReporterPlugin for SlackReporter {
    fn report_format(&self) -> ReportFormat {
        ReportFormat::Custom("slack".to_string())
    }
    
    async fn generate_report(&self, results: &TestResults, config: &ReportConfig) -> Result<Report, ReportError> {
        let color = if results.summary.failed_tests == 0 { "good" } else { "danger" };
        
        let payload = json!({
            "channel": self.channel,
            "username": "API Test Runner",
            "icon_emoji": ":robot_face:",
            "attachments": [{
                "color": color,
                "title": config.title.as_deref().unwrap_or("API Test Results"),
                "fields": [
                    {
                        "title": "Total Tests",
                        "value": results.summary.total_tests,
                        "short": true
                    },
                    {
                        "title": "Passed",
                        "value": results.summary.passed_tests,
                        "short": true
                    },
                    {
                        "title": "Failed",
                        "value": results.summary.failed_tests,
                        "short": true
                    },
                    {
                        "title": "Success Rate",
                        "value": format!("{:.1}%", results.summary.success_rate * 100.0),
                        "short": true
                    },
                    {
                        "title": "Duration",
                        "value": format!("{:.2}s", results.summary.total_duration.as_secs_f64()),
                        "short": true
                    }
                ],
                "footer": "API Test Runner",
                "ts": results.summary.end_time.timestamp()
            }]
        });
        
        let client = reqwest::Client::new();
        let response = client
            .post(&self.webhook_url)
            .json(&payload)
            .send()
            .await?;
        
        if !response.status().is_success() {
            return Err(ReportError::GenerationError(
                format!("Failed to send Slack notification: {}", response.status())
            ));
        }
        
        Ok(Report {
            format: ReportFormat::Custom("slack".to_string()),
            content: ReportContent::Text(serde_json::to_string_pretty(&payload)?),
            output_files: vec![],
            metadata: [("webhook_url".to_string(), self.webhook_url.clone())].into(),
        })
    }
}
```

## Error Handling

```rust
#[derive(Debug, thiserror::Error)]
pub enum ReportError {
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    
    #[error("Template error: {0}")]
    TemplateError(String),
    
    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),
    
    #[error("Report generation error: {0}")]
    GenerationError(String),
    
    #[error("Configuration error: {0}")]
    ConfigError(String),
    
    #[error("Network error: {0}")]
    NetworkError(#[from] reqwest::Error),
}
```

## Usage Examples

### Basic Usage

```rust
use api_test_runner::reporting::{JunitXmlReporter, ReportConfig};

let reporter = JunitXmlReporter::new();
let config = ReportConfig {
    output_path: PathBuf::from("./reports"),
    title: Some("API Integration Tests".to_string()),
    include_request_response: true,
    include_metrics: true,
    include_variables: false,
    template_path: None,
    options: HashMap::new(),
};

let report = reporter.generate_report(&test_results, &config).await?;
println!("Report generated: {:?}", report.output_files);
```

### Multiple Reporters

```rust
let reporters: Vec<Box<dyn ReporterPlugin>> = vec![
    Box::new(JunitXmlReporter::new()),
    Box::new(HtmlReporter::new()),
    Box::new(JsonReporter::new()),
    Box::new(SlackReporter::new(webhook_url, "#testing".to_string())),
];

for reporter in reporters {
    let report = reporter.generate_report(&test_results, &config).await?;
    println!("Generated {} report", reporter.report_format());
}
```

### Real-time Reporting

```rust
let console_reporter = ConsoleReporter::new();

// During test execution
for update in execution_updates {
    console_reporter.stream_update(&update).await?;
}

// Final report
let final_report = console_reporter.generate_report(&test_results, &config).await?;
```