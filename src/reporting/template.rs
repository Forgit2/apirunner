use super::*;
use async_trait::async_trait;
use handlebars::{Handlebars, Helper, Context, RenderContext, Output, HelperResult};
use serde_json::json;

use std::path::Path;

pub struct TemplateReporter {
    handlebars: Handlebars<'static>,
    template_name: String,
    output_format: ReportFormat,
}

pub struct MarkdownReporter {
    template_reporter: TemplateReporter,
}

impl TemplateReporter {
    pub fn new(template_content: &str, template_name: &str, format: ReportFormat) -> Result<Self, ReportingError> {
        let mut handlebars = Handlebars::new();
        
        // Register all the helper functions
        Self::register_helpers(&mut handlebars);
        
        // Register the template
        handlebars.register_template_string(template_name, template_content)
            .map_err(|e| ReportingError::Template(e.to_string()))?;
        
        Ok(Self {
            handlebars,
            template_name: template_name.to_string(),
            output_format: format,
        })
    }

    pub fn from_file<P: AsRef<Path>>(template_path: P, template_name: &str, format: ReportFormat) -> Result<Self, ReportingError> {
        let template_content = std::fs::read_to_string(template_path)
            .map_err(ReportingError::Io)?;
        
        Self::new(&template_content, template_name, format)
    }

    fn register_helpers(handlebars: &mut Handlebars<'static>) {
        handlebars.register_helper("format_duration", Box::new(format_duration_helper));
        handlebars.register_helper("format_percentage", Box::new(format_percentage_helper));
        handlebars.register_helper("format_bytes", Box::new(format_bytes_helper));
        handlebars.register_helper("format_timestamp", Box::new(format_timestamp_helper));
        handlebars.register_helper("status_icon", Box::new(status_icon_helper));
        handlebars.register_helper("truncate", Box::new(truncate_helper));
        handlebars.register_helper("json_pretty", Box::new(json_pretty_helper));
        handlebars.register_helper("escape_markdown", Box::new(escape_markdown_helper));
        handlebars.register_helper("table_row", Box::new(table_row_helper));
        handlebars.register_helper("progress_bar", Box::new(progress_bar_helper));
    }

    fn prepare_template_data(&self, results: &TestExecutionResults, config: &ReportConfig) -> serde_json::Value {
        // Convert test results to include duration as seconds for template helpers
        let tests_with_duration: Vec<serde_json::Value> = results.test_results.iter().map(|test| {
            json!({
                "test_id": test.test_id,
                "test_name": test.test_name,
                "test_description": test.test_description,
                "status": format!("{:?}", test.status),
                "start_time": test.start_time,
                "end_time": test.end_time,
                "duration": test.duration.as_secs_f64(),
                "duration_ms": test.duration.as_millis(),
                "request_details": test.request_details,
                "response_details": test.response_details,
                "assertions": test.assertions,
                "error_message": test.error_message,
                "error_details": test.error_details,
                "tags": test.tags
            })
        }).collect();

        json!({
            "execution": {
                "id": results.execution_id,
                "suite_name": results.test_suite_name,
                "start_time": results.start_time,
                "end_time": results.end_time,
                "duration": results.total_duration.as_secs_f64(),
                "duration_ms": results.total_duration.as_millis(),
                "formatted_start": results.start_time.format("%Y-%m-%d %H:%M:%S UTC").to_string(),
                "formatted_end": results.end_time.format("%Y-%m-%d %H:%M:%S UTC").to_string()
            },
            "summary": results.summary,
            "performance": {
                "total_requests": results.performance_metrics.total_requests,
                "requests_per_second": results.performance_metrics.requests_per_second,
                "average_response_time": results.performance_metrics.average_response_time.as_secs_f64(),
                "min_response_time": results.performance_metrics.min_response_time.as_secs_f64(),
                "max_response_time": results.performance_metrics.max_response_time.as_secs_f64(),
                "percentile_50": results.performance_metrics.percentile_50.as_secs_f64(),
                "percentile_90": results.performance_metrics.percentile_90.as_secs_f64(),
                "percentile_95": results.performance_metrics.percentile_95.as_secs_f64(),
                "percentile_99": results.performance_metrics.percentile_99.as_secs_f64(),
                "error_rate": results.performance_metrics.error_rate,
                "throughput_bytes_per_second": results.performance_metrics.throughput_bytes_per_second
            },
            "environment": results.environment,
            "tests": tests_with_duration,
            "config": {
                "include_environment": config.include_environment_info,
                "include_performance": config.include_performance_metrics,
                "include_details": config.include_request_response_details,
                "custom_metadata": config.custom_metadata
            },
            "statistics": {
                "success_rate": results.summary.success_rate,
                "failure_rate": 100.0 - results.summary.success_rate,
                "avg_response_time_ms": results.performance_metrics.average_response_time.as_millis(),
                "total_requests": results.performance_metrics.total_requests,
                "error_rate": results.performance_metrics.error_rate
            }
        })
    }

    fn get_content_type(&self) -> &str {
        match self.output_format {
            ReportFormat::Html => "text/html",
            ReportFormat::Markdown => "text/markdown",
            ReportFormat::JunitXml => "application/xml",
            ReportFormat::Json => "application/json",
            ReportFormat::Custom(_) => "text/plain",
        }
    }

    fn get_file_extension(&self) -> &str {
        match self.output_format {
            ReportFormat::Html => "html",
            ReportFormat::Markdown => "md",
            ReportFormat::JunitXml => "xml",
            ReportFormat::Json => "json",
            ReportFormat::Custom(_) => "txt",
        }
    }
}

#[async_trait]
impl Reporter for TemplateReporter {
    fn name(&self) -> &str {
        "template"
    }

    fn supported_formats(&self) -> Vec<ReportFormat> {
        vec![self.output_format.clone()]
    }

    async fn generate_report(&self, results: &TestExecutionResults, config: &ReportConfig) -> Result<ReportOutput, ReportingError> {
        let template_data = self.prepare_template_data(results, config);
        
        let content = self.handlebars.render(&self.template_name, &template_data)
            .map_err(|e| ReportingError::Template(e.to_string()))?;
        
        Ok(ReportOutput {
            content: content.into_bytes(),
            content_type: self.get_content_type().to_string(),
            file_extension: self.get_file_extension().to_string(),
        })
    }
}

impl MarkdownReporter {
    pub fn new() -> Result<Self, ReportingError> {
        let template_reporter = TemplateReporter::new(
            DEFAULT_MARKDOWN_TEMPLATE,
            "markdown",
            ReportFormat::Markdown,
        )?;
        
        Ok(Self { template_reporter })
    }

    pub fn with_custom_template(template_content: &str) -> Result<Self, ReportingError> {
        let template_reporter = TemplateReporter::new(
            template_content,
            "custom_markdown",
            ReportFormat::Markdown,
        )?;
        
        Ok(Self { template_reporter })
    }
}

#[async_trait]
impl Reporter for MarkdownReporter {
    fn name(&self) -> &str {
        "markdown"
    }

    fn supported_formats(&self) -> Vec<ReportFormat> {
        vec![ReportFormat::Markdown]
    }

    async fn generate_report(&self, results: &TestExecutionResults, config: &ReportConfig) -> Result<ReportOutput, ReportingError> {
        self.template_reporter.generate_report(results, config).await
    }
}

// Helper functions for templates
fn format_duration_helper(
    h: &Helper,
    _: &Handlebars,
    _: &Context,
    _: &mut RenderContext,
    out: &mut dyn Output,
) -> HelperResult {
    let duration = h.param(0)
        .and_then(|v| v.value().as_f64())
        .ok_or_else(|| handlebars::RenderError::new("Duration parameter required"))?;
    
    let formatted = if duration < 1.0 {
        format!("{:.0}ms", duration * 1000.0)
    } else if duration < 60.0 {
        format!("{:.2}s", duration)
    } else {
        let minutes = (duration / 60.0) as u32;
        let seconds = duration % 60.0;
        format!("{}m {:.1}s", minutes, seconds)
    };
    
    out.write(&formatted)?;
    Ok(())
}

fn format_percentage_helper(
    h: &Helper,
    _: &Handlebars,
    _: &Context,
    _: &mut RenderContext,
    out: &mut dyn Output,
) -> HelperResult {
    let percentage = h.param(0)
        .and_then(|v| v.value().as_f64())
        .ok_or_else(|| handlebars::RenderError::new("Percentage parameter required"))?;
    
    out.write(&format!("{:.1}%", percentage))?;
    Ok(())
}

fn format_bytes_helper(
    h: &Helper,
    _: &Handlebars,
    _: &Context,
    _: &mut RenderContext,
    out: &mut dyn Output,
) -> HelperResult {
    let bytes = h.param(0)
        .and_then(|v| v.value().as_f64())
        .ok_or_else(|| handlebars::RenderError::new("Bytes parameter required"))?;
    
    let formatted = if bytes < 1024.0 {
        format!("{:.0} B", bytes)
    } else if bytes < 1024.0 * 1024.0 {
        format!("{:.1} KB", bytes / 1024.0)
    } else if bytes < 1024.0 * 1024.0 * 1024.0 {
        format!("{:.1} MB", bytes / (1024.0 * 1024.0))
    } else {
        format!("{:.1} GB", bytes / (1024.0 * 1024.0 * 1024.0))
    };
    
    out.write(&formatted)?;
    Ok(())
}

fn format_timestamp_helper(
    h: &Helper,
    _: &Handlebars,
    _: &Context,
    _: &mut RenderContext,
    out: &mut dyn Output,
) -> HelperResult {
    let timestamp_str = h.param(0)
        .and_then(|v| v.value().as_str())
        .ok_or_else(|| handlebars::RenderError::new("Timestamp parameter required"))?;
    
    let format = h.param(1)
        .and_then(|v| v.value().as_str())
        .unwrap_or("%Y-%m-%d %H:%M:%S");
    
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(timestamp_str) {
        out.write(&dt.format(format).to_string())?;
    } else {
        out.write(timestamp_str)?;
    }
    
    Ok(())
}

fn status_icon_helper(
    h: &Helper,
    _: &Handlebars,
    _: &Context,
    _: &mut RenderContext,
    out: &mut dyn Output,
) -> HelperResult {
    let status = h.param(0)
        .and_then(|v| v.value().as_str())
        .ok_or_else(|| handlebars::RenderError::new("Status parameter required"))?;
    
    let icon = match status {
        "Passed" => "✅",
        "Failed" => "❌",
        "Error" => "⚠️",
        "Skipped" => "⏭️",
        _ => "❓"
    };
    
    out.write(icon)?;
    Ok(())
}

fn truncate_helper(
    h: &Helper,
    _: &Handlebars,
    _: &Context,
    _: &mut RenderContext,
    out: &mut dyn Output,
) -> HelperResult {
    let text = h.param(0)
        .and_then(|v| v.value().as_str())
        .ok_or_else(|| handlebars::RenderError::new("Text parameter required"))?;
    
    let max_length = h.param(1)
        .and_then(|v| v.value().as_u64())
        .unwrap_or(100) as usize;
    
    let truncated = if text.chars().count() > max_length {
        let truncated_text: String = text.chars().take(max_length).collect();
        format!("{}...", truncated_text)
    } else {
        text.to_string()
    };
    
    out.write(&truncated)?;
    Ok(())
}

fn json_pretty_helper(
    h: &Helper,
    _: &Handlebars,
    _: &Context,
    _: &mut RenderContext,
    out: &mut dyn Output,
) -> HelperResult {
    let json_str = h.param(0)
        .and_then(|v| v.value().as_str())
        .ok_or_else(|| handlebars::RenderError::new("JSON string parameter required"))?;
    
    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(json_str) {
        if let Ok(pretty) = serde_json::to_string_pretty(&parsed) {
            out.write(&pretty)?;
        } else {
            out.write(json_str)?;
        }
    } else {
        out.write(json_str)?;
    }
    
    Ok(())
}

fn escape_markdown_helper(
    h: &Helper,
    _: &Handlebars,
    _: &Context,
    _: &mut RenderContext,
    out: &mut dyn Output,
) -> HelperResult {
    let text = h.param(0)
        .and_then(|v| v.value().as_str())
        .ok_or_else(|| handlebars::RenderError::new("Text parameter required"))?;
    
    let escaped = text
        .replace('\\', "\\\\")
        .replace('*', "\\*")
        .replace('_', "\\_")
        .replace('[', "\\[")
        .replace(']', "\\]")
        .replace('(', "\\(")
        .replace(')', "\\)")
        .replace('#', "\\#")
        .replace('+', "\\+")
        .replace('-', "\\-")
        .replace('.', "\\.")
        .replace('!', "\\!")
        .replace('`', "\\`");
    
    out.write(&escaped)?;
    Ok(())
}

fn table_row_helper(
    h: &Helper,
    _: &Handlebars,
    _: &Context,
    _: &mut RenderContext,
    out: &mut dyn Output,
) -> HelperResult {
    let columns: Vec<String> = h.params().iter()
        .map(|p| p.value().as_str().unwrap_or("").to_string())
        .collect();
    
    let row = format!("| {} |", columns.join(" | "));
    out.write(&row)?;
    Ok(())
}

fn progress_bar_helper(
    h: &Helper,
    _: &Handlebars,
    _: &Context,
    _: &mut RenderContext,
    out: &mut dyn Output,
) -> HelperResult {
    let percentage = h.param(0)
        .and_then(|v| v.value().as_f64())
        .ok_or_else(|| handlebars::RenderError::new("Percentage parameter required"))?;
    
    let width = h.param(1)
        .and_then(|v| v.value().as_u64())
        .unwrap_or(20) as usize;
    
    let filled = ((percentage / 100.0) * width as f64) as usize;
    let empty = width - filled;
    
    let bar = format!("[{}{}] {:.1}%", 
        "█".repeat(filled),
        "░".repeat(empty),
        percentage
    );
    
    out.write(&bar)?;
    Ok(())
}

const DEFAULT_MARKDOWN_TEMPLATE: &str = r#"# API Test Report

**Test Suite:** {{execution.suite_name}}  
**Execution ID:** {{execution.id}}  
**Start Time:** {{execution.formatted_start}}  
**End Time:** {{execution.formatted_end}}  
**Duration:** {{format_duration execution.duration}}

## Summary

{{progress_bar statistics.success_rate 30}}

| Metric | Value |
|--------|-------|
| Total Tests | {{summary.total_tests}} |
| Passed | {{status_icon "Passed"}} {{summary.passed_tests}} |
| Failed | {{status_icon "Failed"}} {{summary.failed_tests}} |
| Skipped | {{status_icon "Skipped"}} {{summary.skipped_tests}} |
| Errors | {{status_icon "Error"}} {{summary.error_tests}} |
| Success Rate | {{format_percentage summary.success_rate}} |

{{#if config.include_performance}}
## Performance Metrics

| Metric | Value |
|--------|-------|
| Average Response Time | {{format_duration performance.average_response_time}} |
| Requests per Second | {{performance.requests_per_second}} |
| 95th Percentile | {{format_duration performance.percentile_95}} |
| Error Rate | {{format_percentage performance.error_rate}} |
| Throughput | {{format_bytes performance.throughput_bytes_per_second}}/s |

{{/if}}

## Test Results

{{#each tests}}
### {{status_icon status}} {{test_name}}

**Status:** {{status}}  
**Duration:** {{format_duration duration}}  
**Request:** `{{request_details.method}} {{request_details.url}}`  
{{#if response_details}}**Response:** {{response_details.status_code}} ({{format_bytes response_details.size_bytes}}){{/if}}

{{#if test_description}}
*{{test_description}}*
{{/if}}

#### Assertions
{{#each assertions}}
- {{status_icon (if success "Passed" "Failed")}} **{{assertion_type}}**: {{message}}
{{/each}}

{{#unless (eq status "Passed")}}
#### Failure Details
{{#if error_message}}
**Error:** {{error_message}}
{{/if}}

{{#each assertions}}
{{#unless success}}
- **{{assertion_type}}**: {{message}}
  {{#if expected_value}}
  - Expected: `{{expected_value}}`
  {{/if}}
  {{#if actual_value}}
  - Actual: `{{actual_value}}`
  {{/if}}
{{/unless}}
{{/each}}

{{#if ../config.include_details}}
#### Request Details
```
{{request_details.method}} {{request_details.url}}
{{#each request_details.headers}}
{{@key}}: {{this}}
{{/each}}
{{#if request_details.body}}

{{request_details.body}}
{{/if}}
```

{{#if response_details}}
#### Response Details
```
Status: {{response_details.status_code}}
{{#each response_details.headers}}
{{@key}}: {{this}}
{{/each}}
{{#if response_details.body}}

{{truncate response_details.body 500}}
{{/if}}
```
{{/if}}
{{/if}}
{{/unless}}

---

{{/each}}

{{#if config.include_environment}}
## Environment Information

| Property | Value |
|----------|-------|
| Hostname | {{environment.hostname}} |
| OS | {{environment.os}} |
| Architecture | {{environment.arch}} |
| Rust Version | {{environment.rust_version}} |
| Runner Version | {{environment.runner_version}} |

{{#if environment.environment_variables}}
### Environment Variables
{{#each environment.environment_variables}}
- `{{@key}}={{this}}`
{{/each}}
{{/if}}
{{/if}}

---
*Report generated on {{format_timestamp execution.end_time "%Y-%m-%d at %H:%M:%S UTC"}}*
"#;#[cfg
(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn create_test_results() -> TestExecutionResults {
        let mut results = TestExecutionResults::new(
            "template-test-execution".to_string(),
            "Template Test Suite".to_string(),
        );

        results.test_results.push(TestResult {
            test_id: "template-test-1".to_string(),
            test_name: "GET /api/status".to_string(),
            test_description: Some("Test API status endpoint".to_string()),
            status: TestStatus::Passed,
            start_time: Utc::now(),
            end_time: Utc::now(),
            duration: Duration::from_millis(180),
            request_details: RequestDetails {
                method: "GET".to_string(),
                url: "https://api.example.com/status".to_string(),
                headers: [("Accept".to_string(), "application/json".to_string())].into(),
                body: None,
                protocol_version: Some("HTTP/1.1".to_string()),
            },
            response_details: Some(ResponseDetails {
                status_code: 200,
                headers: [("Content-Type".to_string(), "application/json".to_string())].into(),
                body: Some(r#"{"status": "ok", "version": "1.0"}"#.to_string()),
                size_bytes: 30,
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
            tags: vec!["status".to_string()],
        });

        results.test_results.push(TestResult {
            test_id: "template-test-2".to_string(),
            test_name: "POST /api/data".to_string(),
            test_description: Some("Test data submission".to_string()),
            status: TestStatus::Failed,
            start_time: Utc::now(),
            end_time: Utc::now(),
            duration: Duration::from_millis(320),
            request_details: RequestDetails {
                method: "POST".to_string(),
                url: "https://api.example.com/data".to_string(),
                headers: [
                    ("Accept".to_string(), "application/json".to_string()),
                    ("Content-Type".to_string(), "application/json".to_string()),
                ].into(),
                body: Some(r#"{"data": "test"}"#.to_string()),
                protocol_version: Some("HTTP/1.1".to_string()),
            },
            response_details: Some(ResponseDetails {
                status_code: 422,
                headers: [("Content-Type".to_string(), "application/json".to_string())].into(),
                body: Some(r#"{"error": "Validation failed", "details": "Invalid data format"}"#.to_string()),
                size_bytes: 65,
            }),
            assertions: vec![
                AssertionResult {
                    assertion_type: "status_code".to_string(),
                    success: false,
                    message: "Expected status code 201, got 422".to_string(),
                    expected_value: Some(serde_json::json!(201)),
                    actual_value: Some(serde_json::json!(422)),
                    duration: Duration::from_millis(1),
                },
                AssertionResult {
                    assertion_type: "response_body".to_string(),
                    success: false,
                    message: "Response body validation failed".to_string(),
                    expected_value: Some(serde_json::json!({"success": true})),
                    actual_value: Some(serde_json::json!({"error": "Validation failed"})),
                    duration: Duration::from_millis(2),
                }
            ],
            error_message: Some("Multiple assertion failures".to_string()),
            error_details: Some("Status code and response body validation failed".to_string()),
            tags: vec!["data".to_string(), "validation".to_string()],
        });

        results.total_duration = Duration::from_millis(500);
        results.end_time = results.start_time + chrono::Duration::milliseconds(500);
        results.calculate_summary();
        results.calculate_performance_metrics();

        results
    }

    #[tokio::test]
    async fn test_markdown_reporter() {
        let reporter = MarkdownReporter::new().unwrap();
        let results = create_test_results();
        let config = ReportConfig::default();

        let report = reporter.generate_report(&results, &config).await.unwrap();
        let markdown_content = String::from_utf8(report.content).unwrap();

        assert!(markdown_content.contains("# API Test Report"));
        assert!(markdown_content.contains("Template Test Suite"));
        assert!(markdown_content.contains("## Summary"));
        assert!(markdown_content.contains("## Test Results"));
        assert!(markdown_content.contains("### ✅ GET /api/status"));
        assert!(markdown_content.contains("### ❌ POST /api/data"));
        assert!(markdown_content.contains("| Total Tests | 2 |"));
        assert_eq!(report.content_type, "text/markdown");
        assert_eq!(report.file_extension, "md");
    }

    #[tokio::test]
    async fn test_markdown_reporter_with_performance() {
        let reporter = MarkdownReporter::new().unwrap();
        let results = create_test_results();
        let mut config = ReportConfig::default();
        config.include_performance_metrics = true;

        let report = reporter.generate_report(&results, &config).await.unwrap();
        let markdown_content = String::from_utf8(report.content).unwrap();

        assert!(markdown_content.contains("## Performance Metrics"));
        assert!(markdown_content.contains("Average Response Time"));
        assert!(markdown_content.contains("Requests per Second"));
        assert!(markdown_content.contains("95th Percentile"));
    }

    #[tokio::test]
    async fn test_markdown_reporter_with_environment() {
        let reporter = MarkdownReporter::new().unwrap();
        let results = create_test_results();
        let mut config = ReportConfig::default();
        config.include_environment_info = true;

        let report = reporter.generate_report(&results, &config).await.unwrap();
        let markdown_content = String::from_utf8(report.content).unwrap();

        assert!(markdown_content.contains("## Environment Information"));
        assert!(markdown_content.contains("| Hostname |"));
        assert!(markdown_content.contains("| OS |"));
        assert!(markdown_content.contains("| Rust Version |"));
    }

    #[tokio::test]
    async fn test_custom_template_reporter() {
        let custom_template = r#"
# Custom Report for {{execution.suite_name}}

Total: {{summary.total_tests}} tests
Success Rate: {{format_percentage summary.success_rate}}

{{#each tests}}
- {{test_name}}: {{status}}
{{/each}}
"#;

        let reporter = TemplateReporter::new(custom_template, "custom", ReportFormat::Custom("txt".to_string())).unwrap();
        let results = create_test_results();
        let config = ReportConfig::default();

        let report = reporter.generate_report(&results, &config).await.unwrap();
        let content = String::from_utf8(report.content).unwrap();

        assert!(content.contains("# Custom Report for Template Test Suite"));
        assert!(content.contains("Total: 2 tests"));
        assert!(content.contains("- GET /api/status: Passed"));
        assert!(content.contains("- POST /api/data: Failed"));
        assert_eq!(report.content_type, "text/plain");
        assert_eq!(report.file_extension, "txt");
    }

    #[tokio::test]
    async fn test_template_helpers() {
        let template_with_helpers = r#"
Duration: {{format_duration 1.5}}
Percentage: {{format_percentage 85.7}}
Bytes: {{format_bytes 1048576}}
Status: {{status_icon "Passed"}}
Truncated: {{truncate "This is a very long text that should be truncated" 20}}
Progress: {{progress_bar 75 10}}
"#;

        let reporter = TemplateReporter::new(template_with_helpers, "helpers", ReportFormat::Custom("txt".to_string())).unwrap();
        let results = create_test_results();
        let config = ReportConfig::default();

        let report = reporter.generate_report(&results, &config).await.unwrap();
        let content = String::from_utf8(report.content).unwrap();

        assert!(content.contains("Duration: 1.50s"));
        assert!(content.contains("Percentage: 85.7%"));
        assert!(content.contains("Bytes: 1.0 MB"));
        assert!(content.contains("Status: ✅"));
        assert!(content.contains("Truncated: This is a very long ..."));
        assert!(content.contains("Progress: [███████░░░] 75.0%"));
    }

    #[tokio::test]
    async fn test_markdown_escape_helper() {
        let template_with_escape = r#"
Escaped: {{escape_markdown "Text with *special* [characters] and `code`"}}
"#;

        let reporter = TemplateReporter::new(template_with_escape, "escape", ReportFormat::Markdown).unwrap();
        let results = create_test_results();
        let config = ReportConfig::default();

        let report = reporter.generate_report(&results, &config).await.unwrap();
        let content = String::from_utf8(report.content).unwrap();

        assert!(content.contains("Text with \\*special\\* \\[characters\\] and \\`code\\`"));
    }

    #[tokio::test]
    async fn test_json_pretty_helper() {
        let template_with_json = r#"
{{json_pretty '{"key": "value", "number": 42}'}}
"#;

        let reporter = TemplateReporter::new(template_with_json, "json", ReportFormat::Custom("txt".to_string())).unwrap();
        let results = create_test_results();
        let config = ReportConfig::default();

        let report = reporter.generate_report(&results, &config).await.unwrap();
        let content = String::from_utf8(report.content).unwrap();

        assert!(content.contains("{\n  \"key\": \"value\",\n  \"number\": 42\n}"));
    }

    #[tokio::test]
    async fn test_custom_markdown_reporter() {
        let custom_markdown = r#"
# {{execution.suite_name}} Results

{{#each tests}}
## {{status_icon status}} {{test_name}}
Duration: {{format_duration duration}}
{{/each}}
"#;

        let reporter = MarkdownReporter::with_custom_template(custom_markdown).unwrap();
        let results = create_test_results();
        let config = ReportConfig::default();

        let report = reporter.generate_report(&results, &config).await.unwrap();
        let content = String::from_utf8(report.content).unwrap();

        assert!(content.contains("# Template Test Suite Results"));
        assert!(content.contains("## ✅ GET /api/status"));
        assert!(content.contains("## ❌ POST /api/data"));
        assert_eq!(report.content_type, "text/markdown");
    }
}