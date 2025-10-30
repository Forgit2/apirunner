use super::*;
use async_trait::async_trait;
use serde_json::{json, Value};
use std::io::Write;


pub struct JsonReporter {
    streaming_enabled: bool,
}

impl JsonReporter {
    pub fn new() -> Self {
        Self {
            streaming_enabled: false,
        }
    }

    pub fn with_streaming() -> Self {
        Self {
            streaming_enabled: true,
        }
    }

    fn create_comprehensive_json(&self, results: &TestExecutionResults, config: &ReportConfig) -> Value {
        let mut report = json!({
            "report_metadata": {
                "format": "json",
                "version": "1.0",
                "generated_at": chrono::Utc::now().to_rfc3339(),
                "generator": "apirunner",
                "generator_version": env!("CARGO_PKG_VERSION")
            },
            "execution": {
                "id": results.execution_id,
                "suite_name": results.test_suite_name,
                "start_time": results.start_time.to_rfc3339(),
                "end_time": results.end_time.to_rfc3339(),
                "duration_ms": results.total_duration.as_millis(),
                "duration_seconds": results.total_duration.as_secs_f64()
            },
            "summary": {
                "total_tests": results.summary.total_tests,
                "passed_tests": results.summary.passed_tests,
                "failed_tests": results.summary.failed_tests,
                "skipped_tests": results.summary.skipped_tests,
                "error_tests": results.summary.error_tests,
                "success_rate": results.summary.success_rate,
                "total_assertions": results.summary.total_assertions,
                "passed_assertions": results.summary.passed_assertions,
                "failed_assertions": results.summary.failed_assertions
            }
        });

        // Add performance metrics if enabled
        if config.include_performance_metrics {
            report["performance_metrics"] = json!({
                "total_requests": results.performance_metrics.total_requests,
                "requests_per_second": results.performance_metrics.requests_per_second,
                "response_times": {
                    "average_ms": results.performance_metrics.average_response_time.as_millis(),
                    "min_ms": results.performance_metrics.min_response_time.as_millis(),
                    "max_ms": results.performance_metrics.max_response_time.as_millis(),
                    "percentiles": {
                        "p50_ms": results.performance_metrics.percentile_50.as_millis(),
                        "p90_ms": results.performance_metrics.percentile_90.as_millis(),
                        "p95_ms": results.performance_metrics.percentile_95.as_millis(),
                        "p99_ms": results.performance_metrics.percentile_99.as_millis()
                    }
                },
                "error_rate": results.performance_metrics.error_rate,
                "throughput_bytes_per_second": results.performance_metrics.throughput_bytes_per_second
            });

            // Add detailed performance analysis
            report["performance_analysis"] = self.create_performance_analysis(results);
        }

        // Add environment information if enabled
        if config.include_environment_info {
            report["environment"] = json!({
                "hostname": results.environment.hostname,
                "os": results.environment.os,
                "arch": results.environment.arch,
                "rust_version": results.environment.rust_version,
                "runner_version": results.environment.runner_version,
                "environment_variables": results.environment.environment_variables
            });
        }

        // Add test results with configurable detail level
        report["test_results"] = self.create_test_results_json(&results.test_results, config);

        // Add failure analysis
        report["failure_analysis"] = self.create_failure_analysis(&results.test_results);

        // Add custom metadata if provided
        if !config.custom_metadata.is_empty() {
            report["custom_metadata"] = json!(config.custom_metadata);
        }

        report
    }

    fn create_test_results_json(&self, test_results: &[TestResult], config: &ReportConfig) -> Value {
        let mut results = Vec::new();

        for test in test_results {
            let mut test_json = json!({
                "test_id": test.test_id,
                "test_name": test.test_name,
                "test_description": test.test_description,
                "status": format!("{:?}", test.status),
                "start_time": test.start_time.to_rfc3339(),
                "end_time": test.end_time.to_rfc3339(),
                "duration_ms": test.duration.as_millis(),
                "duration_seconds": test.duration.as_secs_f64(),
                "tags": test.tags
            });

            // Add request details
            test_json["request"] = json!({
                "method": test.request_details.method,
                "url": test.request_details.url,
                "protocol_version": test.request_details.protocol_version
            });

            if config.include_request_response_details {
                test_json["request"]["headers"] = json!(test.request_details.headers);
                test_json["request"]["body"] = json!(test.request_details.body);
            }

            // Add response details if available
            if let Some(response) = &test.response_details {
                test_json["response"] = json!({
                    "status_code": response.status_code,
                    "size_bytes": response.size_bytes
                });

                if config.include_request_response_details {
                    test_json["response"]["headers"] = json!(response.headers);
                    test_json["response"]["body"] = json!(response.body);
                }
            }

            // Add assertions
            test_json["assertions"] = json!(test.assertions.iter().map(|assertion| {
                json!({
                    "type": assertion.assertion_type,
                    "success": assertion.success,
                    "message": assertion.message,
                    "expected_value": assertion.expected_value,
                    "actual_value": assertion.actual_value,
                    "duration_ms": assertion.duration.as_millis()
                })
            }).collect::<Vec<_>>());

            // Add error information if present
            if test.error_message.is_some() || test.error_details.is_some() {
                test_json["error"] = json!({
                    "message": test.error_message,
                    "details": test.error_details
                });
            }

            results.push(test_json);
        }

        json!(results)
    }

    fn create_performance_analysis(&self, results: &TestExecutionResults) -> Value {
        let response_times: Vec<u128> = results.test_results.iter()
            .map(|r| r.duration.as_millis())
            .collect();

        let mut sorted_times = response_times.clone();
        sorted_times.sort();

        // Calculate additional statistics
        let median = if sorted_times.is_empty() {
            0
        } else {
            sorted_times[sorted_times.len() / 2]
        };

        let variance = if response_times.len() > 1 {
            let mean = response_times.iter().sum::<u128>() as f64 / response_times.len() as f64;
            let sum_squared_diff: f64 = response_times.iter()
                .map(|&x| (x as f64 - mean).powi(2))
                .sum();
            sum_squared_diff / (response_times.len() - 1) as f64
        } else {
            0.0
        };

        let std_deviation = variance.sqrt();

        // Group tests by status for analysis
        let mut status_performance = std::collections::HashMap::new();
        for test in &results.test_results {
            let status_key = format!("{:?}", test.status);
            let entry = status_performance.entry(status_key).or_insert_with(|| Vec::new());
            entry.push(test.duration.as_millis());
        }

        let status_stats: std::collections::HashMap<String, Value> = status_performance.into_iter()
            .map(|(status, times)| {
                let avg = if times.is_empty() { 0.0 } else { times.iter().sum::<u128>() as f64 / times.len() as f64 };
                let min = times.iter().min().copied().unwrap_or(0);
                let max = times.iter().max().copied().unwrap_or(0);
                
                (status, json!({
                    "count": times.len(),
                    "average_ms": avg,
                    "min_ms": min,
                    "max_ms": max
                }))
            })
            .collect();

        json!({
            "response_time_distribution": {
                "median_ms": median,
                "variance": variance,
                "standard_deviation": std_deviation,
                "coefficient_of_variation": if response_times.is_empty() { 0.0 } else { std_deviation / (response_times.iter().sum::<u128>() as f64 / response_times.len() as f64) }
            },
            "performance_by_status": status_stats,
            "outliers": self.detect_outliers(&response_times),
            "trend_analysis": self.analyze_performance_trend(&results.test_results)
        })
    }

    fn create_failure_analysis(&self, test_results: &[TestResult]) -> Value {
        let failed_tests: Vec<&TestResult> = test_results.iter()
            .filter(|r| matches!(r.status, TestStatus::Failed | TestStatus::Error))
            .collect();

        let mut failure_categories = std::collections::HashMap::new();
        let mut error_patterns = std::collections::HashMap::new();

        for test in &failed_tests {
            // Categorize by assertion failures
            for assertion in &test.assertions {
                if !assertion.success {
                    *failure_categories.entry(&assertion.assertion_type).or_insert(0) += 1;
                }
            }

            // Categorize by error messages
            if let Some(error_msg) = &test.error_message {
                *error_patterns.entry(error_msg.clone()).or_insert(0) += 1;
            }
        }

        json!({
            "total_failures": failed_tests.len(),
            "failure_rate": if test_results.is_empty() { 0.0 } else { failed_tests.len() as f64 / test_results.len() as f64 * 100.0 },
            "failure_categories": failure_categories,
            "error_patterns": error_patterns,
            "failed_tests": failed_tests.iter().map(|test| {
                json!({
                    "test_id": test.test_id,
                    "test_name": test.test_name,
                    "status": format!("{:?}", test.status),
                    "error_message": test.error_message,
                    "failed_assertions": test.assertions.iter()
                        .filter(|a| !a.success)
                        .map(|a| json!({
                            "type": a.assertion_type,
                            "message": a.message
                        }))
                        .collect::<Vec<_>>()
                })
            }).collect::<Vec<_>>()
        })
    }

    fn detect_outliers(&self, response_times: &[u128]) -> Value {
        if response_times.len() < 4 {
            return json!([]);
        }

        let mut sorted = response_times.to_vec();
        sorted.sort();

        let q1_idx = sorted.len() / 4;
        let q3_idx = 3 * sorted.len() / 4;
        let q1 = sorted[q1_idx] as f64;
        let q3 = sorted[q3_idx] as f64;
        let iqr = q3 - q1;

        let lower_bound = q1 - 1.5 * iqr;
        let upper_bound = q3 + 1.5 * iqr;

        let outliers: Vec<u128> = response_times.iter()
            .filter(|&&time| (time as f64) < lower_bound || (time as f64) > upper_bound)
            .copied()
            .collect();

        json!({
            "count": outliers.len(),
            "values_ms": outliers,
            "detection_method": "IQR",
            "bounds": {
                "lower_ms": lower_bound,
                "upper_ms": upper_bound
            }
        })
    }

    fn analyze_performance_trend(&self, test_results: &[TestResult]) -> Value {
        if test_results.len() < 2 {
            return json!({
                "trend": "insufficient_data",
                "slope": 0.0,
                "correlation": 0.0
            });
        }

        let response_times: Vec<f64> = test_results.iter()
            .map(|r| r.duration.as_secs_f64() * 1000.0)
            .collect();

        // Calculate linear regression
        let n = response_times.len() as f64;
        let x_values: Vec<f64> = (0..response_times.len()).map(|i| i as f64).collect();
        
        let sum_x: f64 = x_values.iter().sum();
        let sum_y: f64 = response_times.iter().sum();
        let sum_xy: f64 = x_values.iter().zip(&response_times).map(|(x, y)| x * y).sum();
        let sum_x2: f64 = x_values.iter().map(|x| x * x).sum();

        let slope = (n * sum_xy - sum_x * sum_y) / (n * sum_x2 - sum_x * sum_x);
        
        // Calculate correlation coefficient
        let mean_x = sum_x / n;
        let mean_y = sum_y / n;
        
        let numerator: f64 = x_values.iter().zip(&response_times)
            .map(|(x, y)| (x - mean_x) * (y - mean_y))
            .sum();
        
        let denom_x: f64 = x_values.iter().map(|x| (x - mean_x).powi(2)).sum();
        let denom_y: f64 = response_times.iter().map(|y| (y - mean_y).powi(2)).sum();
        
        let correlation = if denom_x * denom_y > 0.0 {
            numerator / (denom_x * denom_y).sqrt()
        } else {
            0.0
        };

        let trend = if slope.abs() < 0.1 {
            "stable"
        } else if slope > 0.0 {
            "degrading"
        } else {
            "improving"
        };

        json!({
            "trend": trend,
            "slope": slope,
            "correlation": correlation,
            "interpretation": {
                "slope_meaning": format!("Response time changes by {:.2}ms per test", slope),
                "correlation_strength": if correlation.abs() > 0.7 { "strong" } else if correlation.abs() > 0.3 { "moderate" } else { "weak" }
            }
        })
    }

    pub async fn stream_results<W: Write + Send>(&self, mut writer: W, results: &TestExecutionResults, config: &ReportConfig) -> Result<(), ReportingError> {
        if !self.streaming_enabled {
            return Err(ReportingError::InvalidConfig("Streaming not enabled".to_string()));
        }

        // Write opening brace
        writer.write_all(b"{\n").map_err(ReportingError::Io)?;

        // Stream metadata first
        let metadata = json!({
            "report_metadata": {
                "format": "json_stream",
                "version": "1.0",
                "generated_at": chrono::Utc::now().to_rfc3339(),
                "streaming": true
            }
        });
        
        writer.write_all(format!("\"metadata\": {},\n", metadata).as_bytes()).map_err(ReportingError::Io)?;

        // Stream test results one by one
        writer.write_all(b"\"test_results\": [\n").map_err(ReportingError::Io)?;
        
        for (i, test) in results.test_results.iter().enumerate() {
            let test_json = self.create_test_results_json(&[test.clone()], config);
            let test_str = serde_json::to_string(&test_json["0"]).map_err(|e| ReportingError::Serialization(e.to_string()))?;
            
            writer.write_all(test_str.as_bytes()).map_err(ReportingError::Io)?;
            
            if i < results.test_results.len() - 1 {
                writer.write_all(b",\n").map_err(ReportingError::Io)?;
            } else {
                writer.write_all(b"\n").map_err(ReportingError::Io)?;
            }
        }
        
        writer.write_all(b"]\n").map_err(ReportingError::Io)?;
        writer.write_all(b"}\n").map_err(ReportingError::Io)?;

        Ok(())
    }
}

#[async_trait]
impl Reporter for JsonReporter {
    fn name(&self) -> &str {
        "json"
    }

    fn supported_formats(&self) -> Vec<ReportFormat> {
        vec![ReportFormat::Json]
    }

    async fn generate_report(&self, results: &TestExecutionResults, config: &ReportConfig) -> Result<ReportOutput, ReportingError> {
        let json_data = self.create_comprehensive_json(results, config);
        
        let json_content = if self.streaming_enabled {
            // For streaming, we'll return a compact format
            serde_json::to_string(&json_data)
        } else {
            // For regular reports, use pretty printing
            serde_json::to_string_pretty(&json_data)
        }.map_err(|e| ReportingError::Serialization(e.to_string()))?;
        
        Ok(ReportOutput {
            content: json_content.into_bytes(),
            content_type: "application/json".to_string(),
            file_extension: "json".to_string(),
        })
    }
}#[cfg(test
)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn create_test_results() -> TestExecutionResults {
        let mut results = TestExecutionResults::new(
            "json-test-execution-789".to_string(),
            "JSON Test Suite".to_string(),
        );

        // Add a successful test
        results.test_results.push(TestResult {
            test_id: "json-test-1".to_string(),
            test_name: "GET /api/health".to_string(),
            test_description: Some("Health check endpoint test".to_string()),
            status: TestStatus::Passed,
            start_time: Utc::now(),
            end_time: Utc::now(),
            duration: Duration::from_millis(120),
            request_details: RequestDetails {
                method: "GET".to_string(),
                url: "https://api.example.com/health".to_string(),
                headers: [("Accept".to_string(), "application/json".to_string())].into(),
                body: None,
                protocol_version: Some("HTTP/1.1".to_string()),
            },
            response_details: Some(ResponseDetails {
                status_code: 200,
                headers: [("Content-Type".to_string(), "application/json".to_string())].into(),
                body: Some(r#"{"status": "healthy"}"#.to_string()),
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
            tags: vec!["health".to_string(), "monitoring".to_string()],
        });

        // Add a failed test
        results.test_results.push(TestResult {
            test_id: "json-test-2".to_string(),
            test_name: "POST /api/invalid".to_string(),
            test_description: Some("Test invalid endpoint".to_string()),
            status: TestStatus::Failed,
            start_time: Utc::now(),
            end_time: Utc::now(),
            duration: Duration::from_millis(250),
            request_details: RequestDetails {
                method: "POST".to_string(),
                url: "https://api.example.com/invalid".to_string(),
                headers: [
                    ("Accept".to_string(), "application/json".to_string()),
                    ("Content-Type".to_string(), "application/json".to_string()),
                ].into(),
                body: Some(r#"{"test": "data"}"#.to_string()),
                protocol_version: Some("HTTP/1.1".to_string()),
            },
            response_details: Some(ResponseDetails {
                status_code: 404,
                headers: [("Content-Type".to_string(), "application/json".to_string())].into(),
                body: Some(r#"{"error": "Not found"}"#.to_string()),
                size_bytes: 22,
            }),
            assertions: vec![
                AssertionResult {
                    assertion_type: "status_code".to_string(),
                    success: false,
                    message: "Expected status code 200, got 404".to_string(),
                    expected_value: Some(serde_json::json!(200)),
                    actual_value: Some(serde_json::json!(404)),
                    duration: Duration::from_millis(1),
                }
            ],
            error_message: Some("Status code assertion failed".to_string()),
            error_details: Some("Expected 200 OK but received 404 Not Found".to_string()),
            tags: vec!["negative".to_string(), "error".to_string()],
        });

        results.total_duration = Duration::from_millis(370);
        results.end_time = results.start_time + chrono::Duration::milliseconds(370);
        results.calculate_summary();
        results.calculate_performance_metrics();

        results
    }

    #[tokio::test]
    async fn test_json_report_generation() {
        let reporter = JsonReporter::new();
        let results = create_test_results();
        let config = ReportConfig::default();

        let report = reporter.generate_report(&results, &config).await.unwrap();
        let json_content = String::from_utf8(report.content).unwrap();
        
        // Parse the JSON to verify it's valid
        let parsed: serde_json::Value = serde_json::from_str(&json_content).unwrap();

        assert!(parsed["report_metadata"].is_object());
        assert_eq!(parsed["report_metadata"]["format"], "json");
        assert_eq!(parsed["execution"]["suite_name"], "JSON Test Suite");
        assert_eq!(parsed["summary"]["total_tests"], 2);
        assert_eq!(parsed["summary"]["passed_tests"], 1);
        assert_eq!(parsed["summary"]["failed_tests"], 1);
        assert_eq!(report.content_type, "application/json");
        assert_eq!(report.file_extension, "json");
    }

    #[tokio::test]
    async fn test_json_report_with_performance_metrics() {
        let reporter = JsonReporter::new();
        let results = create_test_results();
        let mut config = ReportConfig::default();
        config.include_performance_metrics = true;

        let report = reporter.generate_report(&results, &config).await.unwrap();
        let json_content = String::from_utf8(report.content).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json_content).unwrap();

        assert!(parsed["performance_metrics"].is_object());
        assert!(parsed["performance_analysis"].is_object());
        assert!(parsed["performance_metrics"]["response_times"]["percentiles"].is_object());
        assert!(parsed["performance_analysis"]["response_time_distribution"].is_object());
        assert!(parsed["performance_analysis"]["trend_analysis"].is_object());
    }

    #[tokio::test]
    async fn test_json_report_with_environment_info() {
        let reporter = JsonReporter::new();
        let results = create_test_results();
        let mut config = ReportConfig::default();
        config.include_environment_info = true;

        let report = reporter.generate_report(&results, &config).await.unwrap();
        let json_content = String::from_utf8(report.content).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json_content).unwrap();

        assert!(parsed["environment"].is_object());
        assert!(parsed["environment"]["hostname"].is_string());
        assert!(parsed["environment"]["os"].is_string());
        assert!(parsed["environment"]["rust_version"].is_string());
    }

    #[tokio::test]
    async fn test_json_report_with_request_response_details() {
        let reporter = JsonReporter::new();
        let results = create_test_results();
        let mut config = ReportConfig::default();
        config.include_request_response_details = true;

        let report = reporter.generate_report(&results, &config).await.unwrap();
        let json_content = String::from_utf8(report.content).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json_content).unwrap();

        let test_results = &parsed["test_results"];
        assert!(test_results[0]["request"]["headers"].is_object());
        assert!(test_results[0]["response"]["headers"].is_object());
        assert!(test_results[1]["request"]["body"].is_string());
        assert!(test_results[1]["response"]["body"].is_string());
    }

    #[tokio::test]
    async fn test_failure_analysis() {
        let reporter = JsonReporter::new();
        let results = create_test_results();
        let config = ReportConfig::default();

        let report = reporter.generate_report(&results, &config).await.unwrap();
        let json_content = String::from_utf8(report.content).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json_content).unwrap();

        let failure_analysis = &parsed["failure_analysis"];
        assert_eq!(failure_analysis["total_failures"], 1);
        assert!(failure_analysis["failure_rate"].as_f64().unwrap() > 0.0);
        assert!(failure_analysis["failure_categories"].is_object());
        assert!(failure_analysis["failed_tests"].is_array());
        assert_eq!(failure_analysis["failed_tests"].as_array().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn test_streaming_json_reporter() {
        let reporter = JsonReporter::with_streaming();
        let results = create_test_results();
        let config = ReportConfig::default();

        let report = reporter.generate_report(&results, &config).await.unwrap();
        let json_content = String::from_utf8(report.content).unwrap();
        
        // Should be compact JSON for streaming
        assert!(!json_content.contains("  ")); // No pretty printing spaces
        
        let parsed: serde_json::Value = serde_json::from_str(&json_content).unwrap();
        assert!(parsed.is_object());
    }

    #[tokio::test]
    async fn test_stream_results() {
        let reporter = JsonReporter::with_streaming();
        let results = create_test_results();
        let config = ReportConfig::default();

        let mut buffer = Vec::new();
        reporter.stream_results(&mut buffer, &results, &config).await.unwrap();
        
        let streamed_content = String::from_utf8(buffer).unwrap();
        assert!(streamed_content.contains("\"metadata\""));
        assert!(streamed_content.contains("\"test_results\""));
        assert!(streamed_content.starts_with("{"));
        assert!(streamed_content.ends_with("}\n"));
    }

    #[tokio::test]
    async fn test_performance_trend_analysis() {
        let reporter = JsonReporter::new();
        
        // Create results with a clear trend
        let mut results = TestExecutionResults::new(
            "trend-test".to_string(),
            "Trend Analysis Test".to_string(),
        );

        // Add tests with increasing response times (degrading performance)
        for i in 0..5 {
            results.test_results.push(TestResult {
                test_id: format!("trend-test-{}", i),
                test_name: format!("Test {}", i),
                test_description: None,
                status: TestStatus::Passed,
                start_time: Utc::now(),
                end_time: Utc::now(),
                duration: Duration::from_millis(100 + i * 50), // Increasing duration
                request_details: RequestDetails {
                    method: "GET".to_string(),
                    url: "https://api.example.com/test".to_string(),
                    headers: HashMap::new(),
                    body: None,
                    protocol_version: Some("HTTP/1.1".to_string()),
                },
                response_details: Some(ResponseDetails {
                    status_code: 200,
                    headers: HashMap::new(),
                    body: Some("{}".to_string()),
                    size_bytes: 2,
                }),
                assertions: vec![],
                error_message: None,
                error_details: None,
                tags: vec![],
            });
        }

        results.calculate_summary();
        results.calculate_performance_metrics();

        let mut config = ReportConfig::default();
        config.include_performance_metrics = true;

        let report = reporter.generate_report(&results, &config).await.unwrap();
        let json_content = String::from_utf8(report.content).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json_content).unwrap();

        let trend_analysis = &parsed["performance_analysis"]["trend_analysis"];
        assert_eq!(trend_analysis["trend"], "degrading");
        assert!(trend_analysis["slope"].as_f64().unwrap() > 0.0);
    }
}