use super::*;
use async_trait::async_trait;
use handlebars::{Handlebars, Helper, Context, RenderContext, Output, HelperResult};
use serde_json::json;
use std::collections::HashMap;

pub struct HtmlReporter {
    handlebars: Handlebars<'static>,
}

impl HtmlReporter {
    pub fn new() -> Result<Self, ReportingError> {
        let mut handlebars = Handlebars::new();
        
        // Register custom helpers
        handlebars.register_helper("format_duration", Box::new(format_duration_helper));
        handlebars.register_helper("format_percentage", Box::new(format_percentage_helper));
        handlebars.register_helper("status_class", Box::new(status_class_helper));
        handlebars.register_helper("format_bytes", Box::new(format_bytes_helper));
        
        // Register default template
        handlebars.register_template_string("default", DEFAULT_HTML_TEMPLATE)
            .map_err(|e| ReportingError::Template(e.to_string()))?;
        
        Ok(Self { handlebars })
    }

    pub fn with_custom_template(template_content: &str) -> Result<Self, ReportingError> {
        let mut reporter = Self::new()?;
        reporter.handlebars.register_template_string("custom", template_content)
            .map_err(|e| ReportingError::Template(e.to_string()))?;
        Ok(reporter)
    }

    fn prepare_template_data(&self, results: &TestExecutionResults, config: &ReportConfig) -> serde_json::Value {
        let mut chart_data = HashMap::new();
        
        // Prepare status distribution data for pie chart
        chart_data.insert("status_distribution", json!({
            "passed": results.summary.passed_tests,
            "failed": results.summary.failed_tests,
            "skipped": results.summary.skipped_tests,
            "error": results.summary.error_tests
        }));

        // Prepare response time data for line chart
        let response_times: Vec<f64> = results.test_results.iter()
            .map(|r| r.duration.as_secs_f64() * 1000.0) // Convert to milliseconds
            .collect();
        
        chart_data.insert("response_times", json!({
            "labels": results.test_results.iter().map(|r| &r.test_name).collect::<Vec<_>>(),
            "data": response_times
        }));

        // Prepare performance trend data
        let mut performance_data = Vec::new();
        for (i, test) in results.test_results.iter().enumerate() {
            performance_data.push(json!({
                "test_index": i + 1,
                "response_time": test.duration.as_secs_f64() * 1000.0,
                "success": matches!(test.status, TestStatus::Passed)
            }));
        }

        // Prepare failure analysis data
        let failed_tests: Vec<_> = results.test_results.iter()
            .filter(|r| matches!(r.status, TestStatus::Failed | TestStatus::Error))
            .collect();

        let mut failure_categories = HashMap::new();
        for test in &failed_tests {
            for assertion in &test.assertions {
                if !assertion.success {
                    *failure_categories.entry(&assertion.assertion_type).or_insert(0) += 1;
                }
            }
        }

        json!({
            "execution": {
                "id": results.execution_id,
                "suite_name": results.test_suite_name,
                "start_time": results.start_time.format("%Y-%m-%d %H:%M:%S UTC").to_string(),
                "end_time": results.end_time.format("%Y-%m-%d %H:%M:%S UTC").to_string(),
                "duration": results.total_duration.as_secs_f64(),
                "duration_ms": results.total_duration.as_millis()
            },
            "summary": results.summary,
            "performance": results.performance_metrics,
            "environment": results.environment,
            "tests": results.test_results,
            "charts": chart_data,
            "performance_trend": performance_data,
            "failure_analysis": {
                "failed_tests": failed_tests,
                "failure_categories": failure_categories
            },
            "config": {
                "include_environment": config.include_environment_info,
                "include_performance": config.include_performance_metrics,
                "include_details": config.include_request_response_details
            }
        })
    }
}

#[async_trait]
impl Reporter for HtmlReporter {
    fn name(&self) -> &str {
        "html"
    }

    fn supported_formats(&self) -> Vec<ReportFormat> {
        vec![ReportFormat::Html]
    }

    async fn generate_report(&self, results: &TestExecutionResults, config: &ReportConfig) -> Result<ReportOutput, ReportingError> {
        let template_data = self.prepare_template_data(results, config);
        
        let template_name = if config.template_path.is_some() { "custom" } else { "default" };
        
        let html_content = self.handlebars.render(template_name, &template_data)
            .map_err(|e| ReportingError::Template(e.to_string()))?;
        
        Ok(ReportOutput {
            content: html_content.into_bytes(),
            content_type: "text/html".to_string(),
            file_extension: "html".to_string(),
        })
    }
}

// Handlebars helpers
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

fn status_class_helper(
    h: &Helper,
    _: &Handlebars,
    _: &Context,
    _: &mut RenderContext,
    out: &mut dyn Output,
) -> HelperResult {
    let status = h.param(0)
        .and_then(|v| v.value().as_str())
        .ok_or_else(|| handlebars::RenderError::new("Status parameter required"))?;
    
    let class = match status {
        "Passed" => "success",
        "Failed" => "danger",
        "Error" => "warning",
        "Skipped" => "secondary",
        _ => "light"
    };
    
    out.write(class)?;
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

const DEFAULT_HTML_TEMPLATE: &str = r#"
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>API Test Report - {{execution.suite_name}}</title>
    <link href="https://cdn.jsdelivr.net/npm/bootstrap@5.1.3/dist/css/bootstrap.min.css" rel="stylesheet">
    <script src="https://cdn.jsdelivr.net/npm/chart.js"></script>
    <style>
        .test-card { margin-bottom: 1rem; }
        .status-badge { font-size: 0.8rem; }
        .metric-card { text-align: center; }
        .chart-container { position: relative; height: 400px; margin: 20px 0; }
        .failure-details { background-color: #f8f9fa; padding: 10px; border-radius: 5px; margin-top: 10px; }
        .request-response { font-family: monospace; font-size: 0.9rem; }
        .performance-summary { background: linear-gradient(135deg, #667eea 0%, #764ba2 100%); color: white; }
    </style>
</head>
<body>
    <div class="container-fluid py-4">
        <!-- Header -->
        <div class="row mb-4">
            <div class="col-12">
                <h1 class="display-4">API Test Report</h1>
                <p class="lead">{{execution.suite_name}} - {{execution.start_time}}</p>
            </div>
        </div>

        <!-- Summary Cards -->
        <div class="row mb-4">
            <div class="col-md-3">
                <div class="card metric-card">
                    <div class="card-body">
                        <h5 class="card-title text-primary">Total Tests</h5>
                        <h2 class="card-text">{{summary.total_tests}}</h2>
                    </div>
                </div>
            </div>
            <div class="col-md-3">
                <div class="card metric-card">
                    <div class="card-body">
                        <h5 class="card-title text-success">Passed</h5>
                        <h2 class="card-text">{{summary.passed_tests}}</h2>
                    </div>
                </div>
            </div>
            <div class="col-md-3">
                <div class="card metric-card">
                    <div class="card-body">
                        <h5 class="card-title text-danger">Failed</h5>
                        <h2 class="card-text">{{summary.failed_tests}}</h2>
                    </div>
                </div>
            </div>
            <div class="col-md-3">
                <div class="card metric-card">
                    <div class="card-body">
                        <h5 class="card-title text-info">Success Rate</h5>
                        <h2 class="card-text">{{format_percentage summary.success_rate}}</h2>
                    </div>
                </div>
            </div>
        </div>

        {{#if config.include_performance}}
        <!-- Performance Summary -->
        <div class="row mb-4">
            <div class="col-12">
                <div class="card performance-summary">
                    <div class="card-body">
                        <h5 class="card-title">Performance Metrics</h5>
                        <div class="row">
                            <div class="col-md-2">
                                <strong>Avg Response Time</strong><br>
                                {{format_duration performance.average_response_time}}
                            </div>
                            <div class="col-md-2">
                                <strong>Requests/sec</strong><br>
                                {{performance.requests_per_second}}
                            </div>
                            <div class="col-md-2">
                                <strong>95th Percentile</strong><br>
                                {{format_duration performance.percentile_95}}
                            </div>
                            <div class="col-md-2">
                                <strong>Error Rate</strong><br>
                                {{format_percentage performance.error_rate}}
                            </div>
                            <div class="col-md-2">
                                <strong>Throughput</strong><br>
                                {{format_bytes performance.throughput_bytes_per_second}}/s
                            </div>
                            <div class="col-md-2">
                                <strong>Total Duration</strong><br>
                                {{format_duration execution.duration}}
                            </div>
                        </div>
                    </div>
                </div>
            </div>
        </div>
        {{/if}}

        <!-- Charts -->
        <div class="row mb-4">
            <div class="col-md-6">
                <div class="card">
                    <div class="card-header">
                        <h5>Test Status Distribution</h5>
                    </div>
                    <div class="card-body">
                        <div class="chart-container">
                            <canvas id="statusChart"></canvas>
                        </div>
                    </div>
                </div>
            </div>
            <div class="col-md-6">
                <div class="card">
                    <div class="card-header">
                        <h5>Response Time Trend</h5>
                    </div>
                    <div class="card-body">
                        <div class="chart-container">
                            <canvas id="responseTimeChart"></canvas>
                        </div>
                    </div>
                </div>
            </div>
        </div>

        <!-- Performance Trend Chart -->
        {{#if config.include_performance}}
        <div class="row mb-4">
            <div class="col-12">
                <div class="card">
                    <div class="card-header">
                        <h5>Performance Trend Analysis</h5>
                    </div>
                    <div class="card-body">
                        <div class="chart-container">
                            <canvas id="performanceTrendChart"></canvas>
                        </div>
                    </div>
                </div>
            </div>
        </div>
        {{/if}}

        <!-- Test Results -->
        <div class="row">
            <div class="col-12">
                <div class="card">
                    <div class="card-header">
                        <h5>Test Results</h5>
                    </div>
                    <div class="card-body">
                        {{#each tests}}
                        <div class="test-card card">
                            <div class="card-header d-flex justify-content-between align-items-center">
                                <h6 class="mb-0">{{test_name}}</h6>
                                <span class="badge bg-{{status_class status}} status-badge">{{status}}</span>
                            </div>
                            <div class="card-body">
                                <div class="row">
                                    <div class="col-md-8">
                                        <p><strong>Test ID:</strong> {{test_id}}</p>
                                        {{#if test_description}}
                                        <p><strong>Description:</strong> {{test_description}}</p>
                                        {{/if}}
                                        <p><strong>Duration:</strong> {{format_duration duration}}</p>
                                        <p><strong>Request:</strong> {{request_details.method}} {{request_details.url}}</p>
                                        {{#if response_details}}
                                        <p><strong>Response:</strong> {{response_details.status_code}} ({{format_bytes response_details.size_bytes}})</p>
                                        {{/if}}
                                    </div>
                                    <div class="col-md-4">
                                        <h6>Assertions ({{assertions.length}})</h6>
                                        {{#each assertions}}
                                        <div class="d-flex justify-content-between">
                                            <span>{{assertion_type}}</span>
                                            <span class="badge bg-{{#if success}}success{{else}}danger{{/if}}">
                                                {{#if success}}PASS{{else}}FAIL{{/if}}
                                            </span>
                                        </div>
                                        {{/each}}
                                    </div>
                                </div>

                                {{#unless (eq status "Passed")}}
                                <div class="failure-details">
                                    <h6>Failure Details</h6>
                                    {{#if error_message}}
                                    <p><strong>Error:</strong> {{error_message}}</p>
                                    {{/if}}
                                    
                                    {{#each assertions}}
                                    {{#unless success}}
                                    <div class="mb-2">
                                        <strong>{{assertion_type}}:</strong> {{message}}
                                        {{#if expected_value}}
                                        <br><small>Expected: {{expected_value}}</small>
                                        {{/if}}
                                        {{#if actual_value}}
                                        <br><small>Actual: {{actual_value}}</small>
                                        {{/if}}
                                    </div>
                                    {{/unless}}
                                    {{/each}}

                                    {{#if ../config.include_details}}
                                    <div class="mt-3">
                                        <h6>Request/Response Details</h6>
                                        <div class="request-response">
                                            <strong>Request Headers:</strong>
                                            <pre>{{#each request_details.headers}}{{@key}}: {{this}}
{{/each}}</pre>
                                            {{#if request_details.body}}
                                            <strong>Request Body:</strong>
                                            <pre>{{request_details.body}}</pre>
                                            {{/if}}
                                            {{#if response_details}}
                                            <strong>Response Headers:</strong>
                                            <pre>{{#each response_details.headers}}{{@key}}: {{this}}
{{/each}}</pre>
                                            {{#if response_details.body}}
                                            <strong>Response Body:</strong>
                                            <pre>{{response_details.body}}</pre>
                                            {{/if}}
                                            {{/if}}
                                        </div>
                                    </div>
                                    {{/if}}
                                </div>
                                {{/unless}}
                            </div>
                        </div>
                        {{/each}}
                    </div>
                </div>
            </div>
        </div>

        {{#if config.include_environment}}
        <!-- Environment Information -->
        <div class="row mt-4">
            <div class="col-12">
                <div class="card">
                    <div class="card-header">
                        <h5>Environment Information</h5>
                    </div>
                    <div class="card-body">
                        <div class="row">
                            <div class="col-md-6">
                                <p><strong>Hostname:</strong> {{environment.hostname}}</p>
                                <p><strong>OS:</strong> {{environment.os}}</p>
                                <p><strong>Architecture:</strong> {{environment.arch}}</p>
                            </div>
                            <div class="col-md-6">
                                <p><strong>Rust Version:</strong> {{environment.rust_version}}</p>
                                <p><strong>Runner Version:</strong> {{environment.runner_version}}</p>
                                <p><strong>Execution ID:</strong> {{execution.id}}</p>
                            </div>
                        </div>
                        {{#if environment.environment_variables}}
                        <h6>Environment Variables</h6>
                        <div class="row">
                            {{#each environment.environment_variables}}
                            <div class="col-md-6">
                                <code>{{@key}}={{this}}</code>
                            </div>
                            {{/each}}
                        </div>
                        {{/if}}
                    </div>
                </div>
            </div>
        </div>
        {{/if}}
    </div>

    <script>
        // Status Distribution Pie Chart
        const statusCtx = document.getElementById("statusChart").getContext("2d");
        new Chart(statusCtx, {
            type: "doughnut",
            data: {
                labels: ["Passed", "Failed", "Skipped", "Error"],
                datasets: [{
                    data: [
                        {{charts.status_distribution.passed}},
                        {{charts.status_distribution.failed}},
                        {{charts.status_distribution.skipped}},
                        {{charts.status_distribution.error}}
                    ],
                    backgroundColor: ["#28a745", "#dc3545", "#6c757d", "#ffc107"]
                }]
            },
            options: {
                responsive: true,
                maintainAspectRatio: false,
                plugins: {
                    legend: {
                        position: "bottom"
                    }
                }
            }
        });

        // Response Time Line Chart
        const responseTimeCtx = document.getElementById("responseTimeChart").getContext("2d");
        new Chart(responseTimeCtx, {
            type: "line",
            data: {
                labels: {{{json charts.response_times.labels}}},
                datasets: [{
                    label: "Response Time (ms)",
                    data: {{{json charts.response_times.data}}},
                    borderColor: "#007bff",
                    backgroundColor: "rgba(0, 123, 255, 0.1)",
                    tension: 0.4
                }]
            },
            options: {
                responsive: true,
                maintainAspectRatio: false,
                scales: {
                    y: {
                        beginAtZero: true,
                        title: {
                            display: true,
                            text: "Response Time (ms)"
                        }
                    }
                }
            }
        });

        {{#if config.include_performance}}
        // Performance Trend Scatter Chart
        const performanceTrendCtx = document.getElementById("performanceTrendChart").getContext("2d");
        new Chart(performanceTrendCtx, {
            type: "scatter",
            data: {
                datasets: [{
                    label: "Successful Tests",
                    data: {{{json performance_trend}}}.filter(p => p.success).map(p => ({x: p.test_index, y: p.response_time})),
                    backgroundColor: "#28a745",
                    borderColor: "#28a745"
                }, {
                    label: "Failed Tests",
                    data: {{{json performance_trend}}}.filter(p => !p.success).map(p => ({x: p.test_index, y: p.response_time})),
                    backgroundColor: "#dc3545",
                    borderColor: "#dc3545"
                }]
            },
            options: {
                responsive: true,
                maintainAspectRatio: false,
                scales: {
                    x: {
                        title: {
                            display: true,
                            text: "Test Execution Order"
                        }
                    },
                    y: {
                        title: {
                            display: true,
                            text: "Response Time (ms)"
                        }
                    }
                }
            }
        });
        {{/if}}
    </script>
</body>
</html>"#;

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn create_test_results() -> TestExecutionResults {
        let mut results = TestExecutionResults::new(
            "test-execution-456".to_string(),
            "HTML Test Suite".to_string(),
        );

        results.test_results.push(TestResult {
            test_id: "html-test-1".to_string(),
            test_name: "GET /api/users".to_string(),
            test_description: Some("Test user listing with HTML report".to_string()),
            status: TestStatus::Passed,
            start_time: Utc::now(),
            end_time: Utc::now(),
            duration: Duration::from_millis(200),
            request_details: RequestDetails {
                method: "GET".to_string(),
                url: "https://api.example.com/users".to_string(),
                headers: [("Accept".to_string(), "application/json".to_string())].into(),
                body: None,
                protocol_version: Some("HTTP/1.1".to_string()),
            },
            response_details: Some(ResponseDetails {
                status_code: 200,
                headers: [("Content-Type".to_string(), "application/json".to_string())].into(),
                body: Some(r#"[{"id": 1, "name": "Alice"}, {"id": 2, "name": "Bob"}]"#.to_string()),
                size_bytes: 50,
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
            tags: vec!["api".to_string(), "users".to_string()],
        });

        results.total_duration = Duration::from_millis(200);
        results.end_time = results.start_time + chrono::Duration::milliseconds(200);
        results.calculate_summary();
        results.calculate_performance_metrics();

        results
    }

    #[tokio::test]
    async fn test_html_report_generation() {
        let reporter = HtmlReporter::new().unwrap();
        let results = create_test_results();
        let config = ReportConfig::default();

        let report = reporter.generate_report(&results, &config).await.unwrap();
        let html_content = String::from_utf8(report.content).unwrap();

        assert!(html_content.contains("<!DOCTYPE html>"));
        assert!(html_content.contains("API Test Report"));
        assert!(html_content.contains("HTML Test Suite"));
        assert!(html_content.contains("GET /api/users"));
        assert!(html_content.contains("chart.js"));
        assert_eq!(report.content_type, "text/html");
        assert_eq!(report.file_extension, "html");
    }

    #[tokio::test]
    async fn test_html_report_with_performance_metrics() {
        let reporter = HtmlReporter::new().unwrap();
        let results = create_test_results();
        let mut config = ReportConfig::default();
        config.include_performance_metrics = true;

        let report = reporter.generate_report(&results, &config).await.unwrap();
        let html_content = String::from_utf8(report.content).unwrap();

        assert!(html_content.contains("Performance Metrics"));
        assert!(html_content.contains("Avg Response Time"));
        assert!(html_content.contains("performanceTrendChart"));
    }

    #[tokio::test]
    async fn test_html_report_with_environment_info() {
        let reporter = HtmlReporter::new().unwrap();
        let results = create_test_results();
        let mut config = ReportConfig::default();
        config.include_environment_info = true;

        let report = reporter.generate_report(&results, &config).await.unwrap();
        let html_content = String::from_utf8(report.content).unwrap();

        assert!(html_content.contains("Environment Information"));
        assert!(html_content.contains("Hostname:"));
        assert!(html_content.contains("Rust Version:"));
    }
}