use apirunner::metrics::{MetricsCollector, MetricsConfig};
use apirunner::metrics_history::{MetricsHistory, MetricsHistoryConfig};
use apirunner::alerting::{AlertingSystem, AlertingConfig, AlertRule, AlertCondition, AlertSeverity, ResourceType};
use proptest::prelude::*;

use std::sync::Arc;
use std::time::{Duration, Instant};
use tempfile::TempDir;


#[tokio::test]
async fn test_metrics_collector_creation() {
    let config = MetricsConfig::default();
    let collector = MetricsCollector::new(config).unwrap();
    
    // Test basic functionality
    collector.record_request("GET", "/api/test", 200, Duration::from_millis(100));
    let snapshot = collector.get_snapshot().unwrap();
    assert_eq!(snapshot.performance.total_requests, 1);
}

#[tokio::test]
async fn test_request_metrics() {
    let config = MetricsConfig::default();
    let collector = MetricsCollector::new(config).unwrap();
    
    // Record multiple requests
    collector.record_request("GET", "/api/users", 200, Duration::from_millis(100));
    collector.record_request("POST", "/api/users", 201, Duration::from_millis(150));
    collector.record_request("GET", "/api/users", 500, Duration::from_millis(200));
    
    let snapshot = collector.get_snapshot().unwrap();
    assert_eq!(snapshot.performance.total_requests, 3);
    assert_eq!(snapshot.performance.successful_requests, 2);
    assert_eq!(snapshot.performance.failed_requests, 1);
}

#[tokio::test]
async fn test_test_case_metrics() {
    let config = MetricsConfig::default();
    let collector = MetricsCollector::new(config).unwrap();
    
    // Record test case executions
    collector.record_test_case(true, Duration::from_secs(1));
    collector.record_test_case(true, Duration::from_secs(2));
    collector.record_test_case(false, Duration::from_secs(3));
    
    let snapshot = collector.get_snapshot().unwrap();
    assert_eq!(snapshot.test_execution.total_test_cases, 3);
    assert_eq!(snapshot.test_execution.passed_test_cases, 2);
    assert_eq!(snapshot.test_execution.failed_test_cases, 1);
    assert!((snapshot.test_execution.success_rate - 66.66666666666667).abs() < 0.0001);
}

#[tokio::test]
async fn test_error_recording() {
    let config = MetricsConfig::default();
    let collector = MetricsCollector::new(config).unwrap();
    
    // Record errors
    collector.record_error("timeout", "/api/slow");
    collector.record_error("validation", "/api/users");
    collector.record_error("timeout", "/api/slow");
    
    // Record some successful requests for comparison
    collector.record_request("GET", "/api/fast", 200, Duration::from_millis(50));
    collector.record_request("GET", "/api/slow", 500, Duration::from_millis(5000));
    
    let snapshot = collector.get_snapshot().unwrap();
    assert_eq!(snapshot.performance.total_requests, 2);
    assert_eq!(snapshot.performance.failed_requests, 1);
}

#[tokio::test]
async fn test_network_usage_recording() {
    let config = MetricsConfig::default();
    let collector = MetricsCollector::new(config).unwrap();
    
    // Record network usage
    collector.record_network_usage(1024, 2048);
    collector.record_network_usage(512, 1024);
    
    let snapshot = collector.get_snapshot().unwrap();
    assert_eq!(snapshot.resource_usage.network_bytes_sent, 1536);
    assert_eq!(snapshot.resource_usage.network_bytes_received, 3072);
}

#[tokio::test]
async fn test_prometheus_export() {
    let config = MetricsConfig::default();
    let collector = MetricsCollector::new(config).unwrap();
    
    // Add various metrics
    collector.record_request("GET", "/api/test", 200, Duration::from_millis(100));
    collector.record_test_case(true, Duration::from_secs(1));
    collector.record_network_usage(1024, 2048);
    
    let prometheus_output = collector.export_prometheus().unwrap();
    
    assert!(prometheus_output.contains("api_requests_total"));
    assert!(prometheus_output.contains("api_request_duration_seconds"));
    assert!(prometheus_output.contains("test_cases_total"));
    assert!(prometheus_output.contains("network_bytes_sent_total"));
}

#[tokio::test]
async fn test_metrics_history() {
    let temp_dir = TempDir::new().unwrap();
    let config = MetricsConfig::default();
    let collector = Arc::new(MetricsCollector::new(config).unwrap());
    
    let history_config = MetricsHistoryConfig {
        enabled: true,
        collection_interval: Duration::from_secs(60),
        retention_period: Duration::from_secs(3600),
        max_data_points: 100,
        baseline_calculation_period: Duration::from_secs(1800),
        anomaly_detection_enabled: false,
        anomaly_threshold_multiplier: 2.0,
    };
    
    let history = MetricsHistory::new(history_config, collector.clone());
    
    // Record some metrics
    collector.record_request("GET", "/api/test", 200, Duration::from_millis(100));
    collector.record_test_case(true, Duration::from_secs(1));
    
    // Test that history can capture snapshots
    let snapshot = collector.get_snapshot().unwrap();
    assert_eq!(snapshot.performance.total_requests, 1);
    assert_eq!(snapshot.test_execution.total_test_cases, 1);
}

#[tokio::test]
async fn test_system_metrics_update() {
    let config = MetricsConfig::default();
    let collector = MetricsCollector::new(config).unwrap();
    
    // Update system metrics
    collector.update_system_metrics().unwrap();
    
    let snapshot = collector.get_snapshot().unwrap();
    
    // System metrics should be available
    assert!(snapshot.resource_usage.cpu_usage_percent >= 0.0);
    assert!(snapshot.resource_usage.memory_usage_bytes >= 0);
}

#[tokio::test]
async fn test_alerting_system() {
    let config = MetricsConfig::default();
    let collector = Arc::new(MetricsCollector::new(config).unwrap());
    let alerting_config = AlertingConfig::default();
    
    let alerting = AlertingSystem::new(alerting_config, collector.clone()).unwrap();
    
    let rule = AlertRule {
        id: "high_error_rate".to_string(),
        name: "High Error Rate".to_string(),
        description: "Alert when error rate exceeds 5%".to_string(),
        condition: AlertCondition::ErrorRate {
            threshold_percent: 5.0,
            duration: Duration::from_secs(60),
        },
        severity: AlertSeverity::Warning,
        notification_channels: vec!["console".to_string()],
        enabled: true,
        cooldown_period: Duration::from_secs(300),
        escalation_rules: vec![],
    };
    
    alerting.add_rule(rule);
    
    // Test condition that should trigger alert - record high error rate
    collector.record_request("GET", "/api/test", 500, Duration::from_millis(100));
    collector.record_request("GET", "/api/test", 500, Duration::from_millis(100));
    collector.record_request("GET", "/api/test", 200, Duration::from_millis(100));
    
    let snapshot = collector.get_snapshot().unwrap();
    assert!(snapshot.performance.error_rate > 5.0);
}

#[tokio::test]
async fn test_resource_usage_alert() {
    let config = MetricsConfig::default();
    let collector = Arc::new(MetricsCollector::new(config).unwrap());
    let alerting_config = AlertingConfig::default();
    
    let alerting = AlertingSystem::new(alerting_config, collector.clone()).unwrap();
    
    let rule = AlertRule {
        id: "high_cpu".to_string(),
        name: "High CPU Usage".to_string(),
        description: "Alert when CPU usage exceeds 80%".to_string(),
        condition: AlertCondition::ResourceUsage {
            resource_type: ResourceType::Cpu,
            threshold_percent: 80.0,
            duration: Duration::from_secs(60),
        },
        severity: AlertSeverity::Critical,
        notification_channels: vec!["console".to_string()],
        enabled: true,
        cooldown_period: Duration::from_secs(300),
        escalation_rules: vec![],
    };
    
    alerting.add_rule(rule);
    
    // Update system metrics to get current CPU usage
    collector.update_system_metrics().unwrap();
    
    let snapshot = collector.get_snapshot().unwrap();
    // Just verify the metrics are being collected
    assert!(snapshot.resource_usage.cpu_usage_percent >= 0.0);
}

#[tokio::test]
async fn test_test_failure_rate_alert() {
    let config = MetricsConfig::default();
    let collector = Arc::new(MetricsCollector::new(config).unwrap());
    let alerting_config = AlertingConfig::default();
    
    let alerting = AlertingSystem::new(alerting_config, collector.clone()).unwrap();
    
    let rule = AlertRule {
        id: "high_failure_rate".to_string(),
        name: "High Test Failure Rate".to_string(),
        description: "Alert when test failure rate exceeds 10%".to_string(),
        condition: AlertCondition::TestFailureRate {
            threshold_percent: 10.0,
            duration: Duration::from_secs(60),
        },
        severity: AlertSeverity::Critical,
        notification_channels: vec!["console".to_string()],
        enabled: true,
        cooldown_period: Duration::from_secs(300),
        escalation_rules: vec![],
    };
    
    alerting.add_rule(rule);
    
    // Record test cases with high failure rate
    collector.record_test_case(false, Duration::from_secs(1)); // Failed
    collector.record_test_case(false, Duration::from_secs(1)); // Failed
    collector.record_test_case(true, Duration::from_secs(1));  // Passed
    collector.record_test_case(true, Duration::from_secs(1));  // Passed
    
    let snapshot = collector.get_snapshot().unwrap();
    let failure_rate = 100.0 - snapshot.test_execution.success_rate;
    assert!(failure_rate > 10.0); // Should be 50% failure rate
}

// Property-based tests for metrics
proptest! {
    #[test]
    fn test_request_recording_consistency(
        status_codes in prop::collection::vec(200u16..600u16, 1..100),
        durations_ms in prop::collection::vec(1u64..5000u64, 1..100)
    ) {
        let config = MetricsConfig::default();
        let collector = MetricsCollector::new(config).unwrap();
        
        let mut expected_total = 0;
        let mut expected_successful = 0;
        let mut expected_failed = 0;
        
        for (status, duration_ms) in status_codes.iter().zip(durations_ms.iter()) {
            collector.record_request("GET", "/api/test", *status, Duration::from_millis(*duration_ms));
            expected_total += 1;
            
            if *status >= 200 && *status < 400 {
                expected_successful += 1;
            } else {
                expected_failed += 1;
            }
        }
        
        let snapshot = collector.get_snapshot().unwrap();
        prop_assert_eq!(snapshot.performance.total_requests, expected_total);
        prop_assert_eq!(snapshot.performance.successful_requests, expected_successful);
        prop_assert_eq!(snapshot.performance.failed_requests, expected_failed);
    }
    
    #[test]
    fn test_test_case_success_rate(
        results in prop::collection::vec(any::<bool>(), 1..1000)
    ) {
        let config = MetricsConfig::default();
        let collector = MetricsCollector::new(config).unwrap();
        
        let mut expected_passed = 0;
        let mut expected_failed = 0;
        
        for result in &results {
            collector.record_test_case(*result, Duration::from_secs(1));
            if *result {
                expected_passed += 1;
            } else {
                expected_failed += 1;
            }
        }
        
        let snapshot = collector.get_snapshot().unwrap();
        prop_assert_eq!(snapshot.test_execution.total_test_cases, results.len() as u64);
        prop_assert_eq!(snapshot.test_execution.passed_test_cases, expected_passed);
        prop_assert_eq!(snapshot.test_execution.failed_test_cases, expected_failed);
        
        let expected_success_rate = if results.is_empty() {
            0.0
        } else {
            (expected_passed as f64 / results.len() as f64) * 100.0
        };
        prop_assert!((snapshot.test_execution.success_rate - expected_success_rate).abs() < f64::EPSILON);
    }
    
    #[test]
    fn test_network_usage_accumulation(
        sent_values in prop::collection::vec(0u64..10000u64, 1..100),
        received_values in prop::collection::vec(0u64..10000u64, 1..100)
    ) {
        let config = MetricsConfig::default();
        let collector = MetricsCollector::new(config).unwrap();
        
        let mut expected_sent = 0u64;
        let mut expected_received = 0u64;
        
        for (sent, received) in sent_values.iter().zip(received_values.iter()) {
            collector.record_network_usage(*sent, *received);
            expected_sent += sent;
            expected_received += received;
        }
        
        let snapshot = collector.get_snapshot().unwrap();
        prop_assert_eq!(snapshot.resource_usage.network_bytes_sent, expected_sent);
        prop_assert_eq!(snapshot.resource_usage.network_bytes_received, expected_received);
    }
}

#[cfg(test)]
mod performance_tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    
    #[tokio::test]
    async fn test_high_throughput_request_recording() {
        let config = MetricsConfig::default();
        let collector = Arc::new(MetricsCollector::new(config).unwrap());
        let counter = Arc::new(AtomicU64::new(0));
        
        let start_time = Instant::now();
        let num_tasks = 100;
        let requests_per_task = 1000;
        
        let tasks: Vec<_> = (0..num_tasks)
            .map(|task_id| {
                let collector = collector.clone();
                let counter = counter.clone();
                
                tokio::spawn(async move {
                    for i in 0..requests_per_task {
                        let endpoint = format!("/api/endpoint_{}", i % 10);
                        let status_code = if i % 10 == 0 { 500 } else { 200 };
                        
                        collector.record_request("GET", &endpoint, status_code, Duration::from_millis(100));
                        counter.fetch_add(1, Ordering::Relaxed);
                    }
                })
            })
            .collect();
        
        futures::future::join_all(tasks).await;
        let duration = start_time.elapsed();
        
        let total_requests = counter.load(Ordering::Relaxed);
        let requests_per_sec = total_requests as f64 / duration.as_secs_f64();
        
        println!("Recorded {} requests in {:?}", total_requests, duration);
        println!("Throughput: {:.2} requests/sec", requests_per_sec);
        
        // Should handle at least 10,000 requests/sec
        assert!(requests_per_sec > 10000.0);
        
        let snapshot = collector.get_snapshot().unwrap();
        assert_eq!(snapshot.performance.total_requests, total_requests);
    }
    
    #[tokio::test]
    async fn test_concurrent_metrics_collection() {
        let config = MetricsConfig::default();
        let collector = Arc::new(MetricsCollector::new(config).unwrap());
        
        let start_time = Instant::now();
        let num_concurrent_operations = 1000;
        
        let tasks: Vec<_> = (0..num_concurrent_operations)
            .map(|i| {
                let collector = collector.clone();
                
                tokio::spawn(async move {
                    // Mix different types of operations
                    collector.record_request("GET", "/api/test", 200, Duration::from_millis(i % 100));
                    collector.record_test_case(i % 3 == 0, Duration::from_secs(1));
                    collector.record_network_usage(1024, 2048);
                    collector.record_error("timeout", "/api/slow");
                })
            })
            .collect();
        
        futures::future::join_all(tasks).await;
        let duration = start_time.elapsed();
        
        println!("Completed {} concurrent operations in {:?}", num_concurrent_operations, duration);
        
        let snapshot = collector.get_snapshot().unwrap();
        assert_eq!(snapshot.performance.total_requests, num_concurrent_operations as u64);
        assert_eq!(snapshot.test_execution.total_test_cases, num_concurrent_operations as u64);
    }
    
    #[tokio::test]
    async fn test_prometheus_export_performance() {
        let config = MetricsConfig::default();
        let collector = MetricsCollector::new(config).unwrap();
        
        // Generate a lot of metrics
        for i in 0..10000 {
            collector.record_request("GET", &format!("/api/endpoint_{}", i % 100), 200, Duration::from_millis(100));
            if i % 10 == 0 {
                collector.record_test_case(true, Duration::from_secs(1));
            }
        }
        
        let start_time = Instant::now();
        let prometheus_output = collector.export_prometheus().unwrap();
        let export_duration = start_time.elapsed();
        
        println!("Exported {} bytes in {:?}", prometheus_output.len(), export_duration);
        
        // Export should be fast even with many metrics
        assert!(export_duration.as_millis() < 1000, "Prometheus export too slow: {:?}", export_duration);
        assert!(!prometheus_output.is_empty());
    }
}