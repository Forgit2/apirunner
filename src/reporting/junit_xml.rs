use super::*;
use async_trait::async_trait;
use quick_xml::events::{Event, BytesEnd, BytesStart, BytesText};
use quick_xml::Writer;
use std::io::Cursor;

pub struct JunitXmlReporter;

impl JunitXmlReporter {
    pub fn new() -> Self {
        Self
    }

    fn generate_xml(&self, results: &TestExecutionResults, config: &ReportConfig) -> Result<String, ReportingError> {
        let mut writer = Writer::new(Cursor::new(Vec::new()));
        
        // Write XML declaration
        writer.write_event(Event::Decl(quick_xml::events::BytesDecl::new("1.0", Some("UTF-8"), None)))
            .map_err(|e| ReportingError::Serialization(e.to_string()))?;

        // Start testsuites element
        let mut testsuites = BytesStart::new("testsuites");
        testsuites.push_attribute(("name", results.test_suite_name.as_str()));
        testsuites.push_attribute(("tests", results.summary.total_tests.to_string().as_str()));
        testsuites.push_attribute(("failures", results.summary.failed_tests.to_string().as_str()));
        testsuites.push_attribute(("errors", results.summary.error_tests.to_string().as_str()));
        testsuites.push_attribute(("skipped", results.summary.skipped_tests.to_string().as_str()));
        testsuites.push_attribute(("time", format!("{:.3}", results.total_duration.as_secs_f64()).as_str()));
        testsuites.push_attribute(("timestamp", results.start_time.to_rfc3339().as_str()));
        writer.write_event(Event::Start(testsuites))
            .map_err(|e| ReportingError::Serialization(e.to_string()))?;

        // Write properties if environment info is included
        if config.include_environment_info {
            self.write_properties(&mut writer, results)?;
        }

        // Start testsuite element
        let mut testsuite = BytesStart::new("testsuite");
        testsuite.push_attribute(("name", results.test_suite_name.as_str()));
        testsuite.push_attribute(("tests", results.summary.total_tests.to_string().as_str()));
        testsuite.push_attribute(("failures", results.summary.failed_tests.to_string().as_str()));
        testsuite.push_attribute(("errors", results.summary.error_tests.to_string().as_str()));
        testsuite.push_attribute(("skipped", results.summary.skipped_tests.to_string().as_str()));
        testsuite.push_attribute(("time", format!("{:.3}", results.total_duration.as_secs_f64()).as_str()));
        testsuite.push_attribute(("timestamp", results.start_time.to_rfc3339().as_str()));
        testsuite.push_attribute(("hostname", results.environment.hostname.as_str()));
        writer.write_event(Event::Start(testsuite))
            .map_err(|e| ReportingError::Serialization(e.to_string()))?;

        // Write test cases
        for test_result in &results.test_results {
            self.write_test_case(&mut writer, test_result, config)?;
        }

        // Write system-out with performance metrics if enabled
        if config.include_performance_metrics {
            self.write_system_out(&mut writer, results)?;
        }

        // Close testsuite
        writer.write_event(Event::End(BytesEnd::new("testsuite")))
            .map_err(|e| ReportingError::Serialization(e.to_string()))?;

        // Close testsuites
        writer.write_event(Event::End(BytesEnd::new("testsuites")))
            .map_err(|e| ReportingError::Serialization(e.to_string()))?;

        let result = writer.into_inner().into_inner();
        String::from_utf8(result)
            .map_err(|e| ReportingError::Serialization(e.to_string()))
    }

    fn write_properties<W: std::io::Write>(&self, writer: &mut Writer<W>, results: &TestExecutionResults) -> Result<(), ReportingError> {
        writer.write_event(Event::Start(BytesStart::new("properties")))
            .map_err(|e| ReportingError::Serialization(e.to_string()))?;

        // Environment properties
        self.write_property(writer, "os.name", &results.environment.os)?;
        self.write_property(writer, "os.arch", &results.environment.arch)?;
        self.write_property(writer, "rust.version", &results.environment.rust_version)?;
        self.write_property(writer, "runner.version", &results.environment.runner_version)?;
        self.write_property(writer, "execution.id", &results.execution_id)?;

        // Custom environment variables
        for (key, value) in &results.environment.environment_variables {
            self.write_property(writer, &format!("env.{}", key), value)?;
        }

        writer.write_event(Event::End(BytesEnd::new("properties")))
            .map_err(|e| ReportingError::Serialization(e.to_string()))?;

        Ok(())
    }

    fn write_property<W: std::io::Write>(&self, writer: &mut Writer<W>, name: &str, value: &str) -> Result<(), ReportingError> {
        let mut property = BytesStart::new("property");
        property.push_attribute(("name", name));
        property.push_attribute(("value", value));
        writer.write_event(Event::Empty(property))
            .map_err(|e| ReportingError::Serialization(e.to_string()))?;
        Ok(())
    }

    fn write_test_case<W: std::io::Write>(&self, writer: &mut Writer<W>, test_result: &TestResult, config: &ReportConfig) -> Result<(), ReportingError> {
        let mut testcase = BytesStart::new("testcase");
        testcase.push_attribute(("name", test_result.test_name.as_str()));
        testcase.push_attribute(("classname", test_result.test_id.as_str()));
        testcase.push_attribute(("time", format!("{:.3}", test_result.duration.as_secs_f64()).as_str()));

        match test_result.status {
            TestStatus::Passed => {
                writer.write_event(Event::Empty(testcase))
                    .map_err(|e| ReportingError::Serialization(e.to_string()))?;
            }
            TestStatus::Failed => {
                writer.write_event(Event::Start(testcase))
                    .map_err(|e| ReportingError::Serialization(e.to_string()))?;

                // Write failure element
                let mut failure = BytesStart::new("failure");
                if let Some(error_msg) = &test_result.error_message {
                    failure.push_attribute(("message", error_msg.as_str()));
                }
                failure.push_attribute(("type", "AssertionFailure"));

                writer.write_event(Event::Start(failure))
                    .map_err(|e| ReportingError::Serialization(e.to_string()))?;

                // Write failure details
                let failure_details = self.format_failure_details(test_result, config);
                writer.write_event(Event::Text(BytesText::new(&failure_details)))
                    .map_err(|e| ReportingError::Serialization(e.to_string()))?;

                writer.write_event(Event::End(BytesEnd::new("failure")))
                    .map_err(|e| ReportingError::Serialization(e.to_string()))?;

                writer.write_event(Event::End(BytesEnd::new("testcase")))
                    .map_err(|e| ReportingError::Serialization(e.to_string()))?;
            }
            TestStatus::Error => {
                writer.write_event(Event::Start(testcase))
                    .map_err(|e| ReportingError::Serialization(e.to_string()))?;

                // Write error element
                let mut error = BytesStart::new("error");
                if let Some(error_msg) = &test_result.error_message {
                    error.push_attribute(("message", error_msg.as_str()));
                }
                error.push_attribute(("type", "TestExecutionError"));

                writer.write_event(Event::Start(error))
                    .map_err(|e| ReportingError::Serialization(e.to_string()))?;

                // Write error details
                if let Some(error_details) = &test_result.error_details {
                    writer.write_event(Event::Text(BytesText::new(error_details)))
                        .map_err(|e| ReportingError::Serialization(e.to_string()))?;
                }

                writer.write_event(Event::End(BytesEnd::new("error")))
                    .map_err(|e| ReportingError::Serialization(e.to_string()))?;

                writer.write_event(Event::End(BytesEnd::new("testcase")))
                    .map_err(|e| ReportingError::Serialization(e.to_string()))?;
            }
            TestStatus::Skipped => {
                writer.write_event(Event::Start(testcase))
                    .map_err(|e| ReportingError::Serialization(e.to_string()))?;

                let skipped = BytesStart::new("skipped");
                writer.write_event(Event::Empty(skipped))
                    .map_err(|e| ReportingError::Serialization(e.to_string()))?;

                writer.write_event(Event::End(BytesEnd::new("testcase")))
                    .map_err(|e| ReportingError::Serialization(e.to_string()))?;
            }
        }

        Ok(())
    }

    fn format_failure_details(&self, test_result: &TestResult, config: &ReportConfig) -> String {
        let mut details = Vec::new();

        // Add assertion failures
        for assertion in &test_result.assertions {
            if !assertion.success {
                details.push(format!("Assertion '{}' failed: {}", assertion.assertion_type, assertion.message));
                
                if let (Some(expected), Some(actual)) = (&assertion.expected_value, &assertion.actual_value) {
                    details.push(format!("  Expected: {}", expected));
                    details.push(format!("  Actual: {}", actual));
                }
            }
        }

        // Add request/response details if enabled
        if config.include_request_response_details {
            details.push("\nRequest Details:".to_string());
            details.push(format!("  Method: {}", test_result.request_details.method));
            details.push(format!("  URL: {}", test_result.request_details.url));
            
            if !test_result.request_details.headers.is_empty() {
                details.push("  Headers:".to_string());
                for (key, value) in &test_result.request_details.headers {
                    details.push(format!("    {}: {}", key, value));
                }
            }

            if let Some(body) = &test_result.request_details.body {
                details.push(format!("  Body: {}", body));
            }

            if let Some(response) = &test_result.response_details {
                details.push("\nResponse Details:".to_string());
                details.push(format!("  Status Code: {}", response.status_code));
                
                if !response.headers.is_empty() {
                    details.push("  Headers:".to_string());
                    for (key, value) in &response.headers {
                        details.push(format!("    {}: {}", key, value));
                    }
                }

                if let Some(body) = &response.body {
                    let truncated_body = if body.len() > 1000 {
                        format!("{}... (truncated)", &body[..1000])
                    } else {
                        body.clone()
                    };
                    details.push(format!("  Body: {}", truncated_body));
                }
            }
        }

        details.join("\n")
    }

    fn write_system_out<W: std::io::Write>(&self, writer: &mut Writer<W>, results: &TestExecutionResults) -> Result<(), ReportingError> {
        writer.write_event(Event::Start(BytesStart::new("system-out")))
            .map_err(|e| ReportingError::Serialization(e.to_string()))?;

        let metrics_output = format!(
            "Performance Metrics:\n\
            Total Requests: {}\n\
            Requests/Second: {:.2}\n\
            Average Response Time: {:.3}s\n\
            Min Response Time: {:.3}s\n\
            Max Response Time: {:.3}s\n\
            50th Percentile: {:.3}s\n\
            90th Percentile: {:.3}s\n\
            95th Percentile: {:.3}s\n\
            99th Percentile: {:.3}s\n\
            Error Rate: {:.2}%\n\
            Throughput: {:.2} bytes/sec\n",
            results.performance_metrics.total_requests,
            results.performance_metrics.requests_per_second,
            results.performance_metrics.average_response_time.as_secs_f64(),
            results.performance_metrics.min_response_time.as_secs_f64(),
            results.performance_metrics.max_response_time.as_secs_f64(),
            results.performance_metrics.percentile_50.as_secs_f64(),
            results.performance_metrics.percentile_90.as_secs_f64(),
            results.performance_metrics.percentile_95.as_secs_f64(),
            results.performance_metrics.percentile_99.as_secs_f64(),
            results.performance_metrics.error_rate,
            results.performance_metrics.throughput_bytes_per_second,
        );

        writer.write_event(Event::Text(BytesText::new(&metrics_output)))
            .map_err(|e| ReportingError::Serialization(e.to_string()))?;

        writer.write_event(Event::End(BytesEnd::new("system-out")))
            .map_err(|e| ReportingError::Serialization(e.to_string()))?;

        Ok(())
    }
}

#[async_trait]
impl Reporter for JunitXmlReporter {
    fn name(&self) -> &str {
        "junit-xml"
    }

    fn supported_formats(&self) -> Vec<ReportFormat> {
        vec![ReportFormat::JunitXml]
    }

    async fn generate_report(&self, results: &TestExecutionResults, config: &ReportConfig) -> Result<ReportOutput, ReportingError> {
        let xml_content = self.generate_xml(results, config)?;
        
        Ok(ReportOutput {
            content: xml_content.into_bytes(),
            content_type: "application/xml".to_string(),
            file_extension: "xml".to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn create_test_results() -> TestExecutionResults {
        let mut results = TestExecutionResults::new(
            "test-execution-123".to_string(),
            "API Test Suite".to_string(),
        );

        results.test_results.push(TestResult {
            test_id: "test-1".to_string(),
            test_name: "GET /users".to_string(),
            test_description: Some("Test user listing endpoint".to_string()),
            status: TestStatus::Passed,
            start_time: Utc::now(),
            end_time: Utc::now(),
            duration: Duration::from_millis(150),
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
                body: Some(r#"[{"id": 1, "name": "John"}]"#.to_string()),
                size_bytes: 25,
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

        results.test_results.push(TestResult {
            test_id: "test-2".to_string(),
            test_name: "POST /users".to_string(),
            test_description: Some("Test user creation endpoint".to_string()),
            status: TestStatus::Failed,
            start_time: Utc::now(),
            end_time: Utc::now(),
            duration: Duration::from_millis(300),
            request_details: RequestDetails {
                method: "POST".to_string(),
                url: "https://api.example.com/users".to_string(),
                headers: [
                    ("Accept".to_string(), "application/json".to_string()),
                    ("Content-Type".to_string(), "application/json".to_string()),
                ].into(),
                body: Some(r#"{"name": "Jane", "email": "jane@example.com"}"#.to_string()),
                protocol_version: Some("HTTP/1.1".to_string()),
            },
            response_details: Some(ResponseDetails {
                status_code: 400,
                headers: [("Content-Type".to_string(), "application/json".to_string())].into(),
                body: Some(r#"{"error": "Invalid email format"}"#.to_string()),
                size_bytes: 35,
            }),
            assertions: vec![
                AssertionResult {
                    assertion_type: "status_code".to_string(),
                    success: false,
                    message: "Expected status code 201, got 400".to_string(),
                    expected_value: Some(serde_json::json!(201)),
                    actual_value: Some(serde_json::json!(400)),
                    duration: Duration::from_millis(1),
                }
            ],
            error_message: Some("Status code assertion failed".to_string()),
            error_details: Some("Expected 201 Created but received 400 Bad Request".to_string()),
            tags: vec!["api".to_string(), "users".to_string(), "create".to_string()],
        });

        results.total_duration = Duration::from_millis(450);
        results.end_time = results.start_time + chrono::Duration::milliseconds(450);
        results.calculate_summary();
        results.calculate_performance_metrics();

        results
    }

    #[tokio::test]
    async fn test_junit_xml_generation() {
        let reporter = JunitXmlReporter::new();
        let results = create_test_results();
        let config = ReportConfig::default();

        let report = reporter.generate_report(&results, &config).await.unwrap();
        let xml_content = String::from_utf8(report.content).unwrap();

        assert!(xml_content.contains("<?xml version=\"1.0\" encoding=\"UTF-8\"?>"));
        assert!(xml_content.contains("<testsuites"));
        assert!(xml_content.contains("name=\"API Test Suite\""));
        assert!(xml_content.contains("tests=\"2\""));
        assert!(xml_content.contains("failures=\"1\""));
        assert!(xml_content.contains("<testsuite"));
        assert!(xml_content.contains("<testcase name=\"GET /users\""));
        assert!(xml_content.contains("<testcase name=\"POST /users\""));
        assert!(xml_content.contains("<failure"));
        assert!(xml_content.contains("Status code assertion failed"));
    }

    #[tokio::test]
    async fn test_junit_xml_with_environment_info() {
        let reporter = JunitXmlReporter::new();
        let results = create_test_results();
        let mut config = ReportConfig::default();
        config.include_environment_info = true;

        let report = reporter.generate_report(&results, &config).await.unwrap();
        let xml_content = String::from_utf8(report.content).unwrap();

        assert!(xml_content.contains("<properties>"));
        assert!(xml_content.contains("os.name"));
        assert!(xml_content.contains("rust.version"));
        assert!(xml_content.contains("execution.id"));
    }

    #[tokio::test]
    async fn test_junit_xml_with_performance_metrics() {
        let reporter = JunitXmlReporter::new();
        let results = create_test_results();
        let mut config = ReportConfig::default();
        config.include_performance_metrics = true;

        let report = reporter.generate_report(&results, &config).await.unwrap();
        let xml_content = String::from_utf8(report.content).unwrap();

        assert!(xml_content.contains("<system-out>"));
        assert!(xml_content.contains("Performance Metrics:"));
        assert!(xml_content.contains("Total Requests:"));
        assert!(xml_content.contains("Average Response Time:"));
    }
}