use std::sync::Arc;
use std::time::{Duration, Instant};
use parking_lot::RwLock;
use prometheus::{
    Counter, CounterVec, Gauge, GaugeVec, Histogram, HistogramVec, 
    Registry, Encoder, TextEncoder, Opts, HistogramOpts
};
use serde::{Deserialize, Serialize};
use sysinfo::{System, SystemExt, CpuExt, ProcessExt, Pid};
use tokio::time::interval;
use crate::error::ApiTestError;

/// Metrics collector that provides Prometheus-compatible metrics
#[derive(Clone)]
pub struct MetricsCollector {
    registry: Arc<Registry>,
    
    // Performance metrics
    request_duration: HistogramVec,
    request_count: CounterVec,
    error_count: CounterVec,
    throughput: GaugeVec,
    
    // Resource usage metrics
    cpu_usage: Gauge,
    memory_usage: Gauge,
    network_bytes_sent: Counter,
    network_bytes_received: Counter,
    
    // Test execution metrics
    test_cases_total: Counter,
    test_cases_passed: Counter,
    test_cases_failed: Counter,
    test_execution_duration: Histogram,
    
    // System monitoring
    system_info: Arc<RwLock<System>>,
    process_id: u32,
    
    // Configuration
    config: MetricsConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsConfig {
    pub enabled: bool,
    pub collection_interval: Duration,
    pub prometheus_endpoint: Option<String>,
    pub resource_monitoring: bool,
    pub detailed_request_metrics: bool,
    pub retention_period: Duration,
}

impl Default for MetricsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            collection_interval: Duration::from_secs(10),
            prometheus_endpoint: Some("/metrics".to_string()),
            resource_monitoring: true,
            detailed_request_metrics: true,
            retention_period: Duration::from_secs(24 * 60 * 60), // 24 hours
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct MetricsSnapshot {
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub performance: PerformanceMetrics,
    pub resource_usage: ResourceUsageMetrics,
    pub test_execution: TestExecutionMetrics,
}

#[derive(Debug, Clone, Serialize)]
pub struct PerformanceMetrics {
    pub total_requests: u64,
    pub successful_requests: u64,
    pub failed_requests: u64,
    pub average_response_time: f64,
    pub p95_response_time: f64,
    pub p99_response_time: f64,
    pub requests_per_second: f64,
    pub error_rate: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ResourceUsageMetrics {
    pub cpu_usage_percent: f64,
    pub memory_usage_bytes: u64,
    pub memory_usage_percent: f64,
    pub network_bytes_sent: u64,
    pub network_bytes_received: u64,
    pub open_file_descriptors: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TestExecutionMetrics {
    pub total_test_cases: u64,
    pub passed_test_cases: u64,
    pub failed_test_cases: u64,
    pub success_rate: f64,
    pub average_test_duration: f64,
    pub total_execution_time: f64,
}

impl MetricsCollector {
    pub fn new(config: MetricsConfig) -> Result<Self, ApiTestError> {
        let registry = Arc::new(Registry::new());
        
        // Create performance metrics
        let request_duration = HistogramVec::new(
            HistogramOpts::new(
                "api_request_duration_seconds",
                "Duration of API requests in seconds"
            ).buckets(vec![0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0]),
            &["method", "status_code", "endpoint"]
        )?;
        
        let request_count = CounterVec::new(
            Opts::new("api_requests_total", "Total number of API requests"),
            &["method", "status_code", "endpoint"]
        )?;
        
        let error_count = CounterVec::new(
            Opts::new("api_errors_total", "Total number of API errors"),
            &["error_type", "endpoint"]
        )?;
        
        let throughput = GaugeVec::new(
            Opts::new("api_throughput_rps", "Current throughput in requests per second"),
            &["endpoint"]
        )?;
        
        // Create resource usage metrics
        let cpu_usage = Gauge::new("system_cpu_usage_percent", "CPU usage percentage")?;
        let memory_usage = Gauge::new("system_memory_usage_bytes", "Memory usage in bytes")?;
        let network_bytes_sent = Counter::new("network_bytes_sent_total", "Total bytes sent over network")?;
        let network_bytes_received = Counter::new("network_bytes_received_total", "Total bytes received over network")?;
        
        // Create test execution metrics
        let test_cases_total = Counter::new("test_cases_total", "Total number of test cases executed")?;
        let test_cases_passed = Counter::new("test_cases_passed_total", "Total number of test cases passed")?;
        let test_cases_failed = Counter::new("test_cases_failed_total", "Total number of test cases failed")?;
        let test_execution_duration = Histogram::with_opts(
            HistogramOpts::new(
                "test_execution_duration_seconds",
                "Duration of test case execution in seconds"
            ).buckets(vec![0.1, 0.5, 1.0, 5.0, 10.0, 30.0, 60.0, 300.0])
        )?;
        
        // Register all metrics
        registry.register(Box::new(request_duration.clone()))?;
        registry.register(Box::new(request_count.clone()))?;
        registry.register(Box::new(error_count.clone()))?;
        registry.register(Box::new(throughput.clone()))?;
        registry.register(Box::new(cpu_usage.clone()))?;
        registry.register(Box::new(memory_usage.clone()))?;
        registry.register(Box::new(network_bytes_sent.clone()))?;
        registry.register(Box::new(network_bytes_received.clone()))?;
        registry.register(Box::new(test_cases_total.clone()))?;
        registry.register(Box::new(test_cases_passed.clone()))?;
        registry.register(Box::new(test_cases_failed.clone()))?;
        registry.register(Box::new(test_execution_duration.clone()))?;
        
        let system_info = Arc::new(RwLock::new(System::new_all()));
        let process_id = std::process::id();
        
        Ok(Self {
            registry,
            request_duration,
            request_count,
            error_count,
            throughput,
            cpu_usage,
            memory_usage,
            network_bytes_sent,
            network_bytes_received,
            test_cases_total,
            test_cases_passed,
            test_cases_failed,
            test_execution_duration,
            system_info,
            process_id,
            config,
        })
    }
    
    /// Record a request execution
    pub fn record_request(&self, method: &str, endpoint: &str, status_code: u16, duration: Duration) {
        if !self.config.enabled {
            return;
        }
        
        let labels = &[method, &status_code.to_string(), endpoint];
        
        self.request_duration
            .with_label_values(labels)
            .observe(duration.as_secs_f64());
            
        self.request_count
            .with_label_values(labels)
            .inc();
    }
    
    /// Record an error
    pub fn record_error(&self, error_type: &str, endpoint: &str) {
        if !self.config.enabled {
            return;
        }
        
        self.error_count
            .with_label_values(&[error_type, endpoint])
            .inc();
    }
    
    /// Update throughput metrics
    pub fn update_throughput(&self, endpoint: &str, rps: f64) {
        if !self.config.enabled {
            return;
        }
        
        self.throughput
            .with_label_values(&[endpoint])
            .set(rps);
    }
    
    /// Record test case execution
    pub fn record_test_case(&self, passed: bool, duration: Duration) {
        if !self.config.enabled {
            return;
        }
        
        self.test_cases_total.inc();
        
        if passed {
            self.test_cases_passed.inc();
        } else {
            self.test_cases_failed.inc();
        }
        
        self.test_execution_duration.observe(duration.as_secs_f64());
    }
    
    /// Record network usage
    pub fn record_network_usage(&self, bytes_sent: u64, bytes_received: u64) {
        if !self.config.enabled {
            return;
        }
        
        self.network_bytes_sent.inc_by(bytes_sent as f64);
        self.network_bytes_received.inc_by(bytes_received as f64);
    }
    
    /// Update system resource metrics
    pub fn update_system_metrics(&self) -> Result<(), ApiTestError> {
        if !self.config.enabled || !self.config.resource_monitoring {
            return Ok(());
        }
        
        let mut system = self.system_info.write();
        system.refresh_all();
        
        // Update CPU usage
        let cpu_usage = system.global_cpu_info().cpu_usage() as f64;
        self.cpu_usage.set(cpu_usage);
        
        // Update memory usage
        if let Some(process) = system.process(Pid::from(self.process_id as usize)) {
            let memory_bytes = process.memory() * 1024; // Convert KB to bytes
            self.memory_usage.set(memory_bytes as f64);
        }
        
        Ok(())
    }
    
    /// Get current metrics snapshot
    pub fn get_snapshot(&self) -> Result<MetricsSnapshot, ApiTestError> {
        let performance = self.get_performance_metrics()?;
        let resource_usage = self.get_resource_usage_metrics()?;
        let test_execution = self.get_test_execution_metrics()?;
        
        Ok(MetricsSnapshot {
            timestamp: chrono::Utc::now(),
            performance,
            resource_usage,
            test_execution,
        })
    }
    
    fn get_performance_metrics(&self) -> Result<PerformanceMetrics, ApiTestError> {
        let metric_families = self.registry.gather();
        
        let mut total_requests = 0u64;
        let mut successful_requests = 0u64;
        let mut failed_requests = 0u64;
        let mut total_duration = 0f64;
        let mut request_count = 0u64;
        
        for family in &metric_families {
            match family.get_name() {
                "api_requests_total" => {
                    for metric in family.get_metric() {
                        let count = metric.get_counter().get_value() as u64;
                        total_requests += count;
                        
                        // Check status code to determine success/failure
                        if let Some(label) = metric.get_label().iter().find(|l| l.get_name() == "status_code") {
                            let status_code: u16 = label.get_value().parse().unwrap_or(0);
                            if status_code >= 200 && status_code < 400 {
                                successful_requests += count;
                            } else {
                                failed_requests += count;
                            }
                        }
                    }
                }
                "api_request_duration_seconds" => {
                    for metric in family.get_metric() {
                        let histogram = metric.get_histogram();
                        total_duration += histogram.get_sample_sum();
                        request_count += histogram.get_sample_count();
                    }
                }
                _ => {}
            }
        }
        
        let average_response_time = if request_count > 0 {
            total_duration / request_count as f64
        } else {
            0.0
        };
        
        let error_rate = if total_requests > 0 {
            failed_requests as f64 / total_requests as f64 * 100.0
        } else {
            0.0
        };
        
        Ok(PerformanceMetrics {
            total_requests,
            successful_requests,
            failed_requests,
            average_response_time,
            p95_response_time: 0.0, // TODO: Calculate from histogram buckets
            p99_response_time: 0.0, // TODO: Calculate from histogram buckets
            requests_per_second: 0.0, // TODO: Calculate based on time window
            error_rate,
        })
    }
    
    fn get_resource_usage_metrics(&self) -> Result<ResourceUsageMetrics, ApiTestError> {
        let system = self.system_info.read();
        
        let cpu_usage_percent = system.global_cpu_info().cpu_usage() as f64;
        let total_memory = system.total_memory();
        let used_memory = system.used_memory();
        let memory_usage_percent = if total_memory > 0 {
            (used_memory as f64 / total_memory as f64) * 100.0
        } else {
            0.0
        };
        
        let process_memory = if let Some(process) = system.process(Pid::from(self.process_id as usize)) {
            process.memory() * 1024 // Convert KB to bytes
        } else {
            0
        };
        
        Ok(ResourceUsageMetrics {
            cpu_usage_percent,
            memory_usage_bytes: process_memory,
            memory_usage_percent,
            network_bytes_sent: self.network_bytes_sent.get() as u64,
            network_bytes_received: self.network_bytes_received.get() as u64,
            open_file_descriptors: None, // TODO: Implement if needed
        })
    }
    
    fn get_test_execution_metrics(&self) -> Result<TestExecutionMetrics, ApiTestError> {
        let total = self.test_cases_total.get() as u64;
        let passed = self.test_cases_passed.get() as u64;
        let failed = self.test_cases_failed.get() as u64;
        
        let success_rate = if total > 0 {
            (passed as f64 / total as f64) * 100.0
        } else {
            0.0
        };
        
        // Get average test duration from histogram
        let metric_families = self.registry.gather();
        let mut average_test_duration = 0.0;
        let mut total_execution_time = 0.0;
        
        for family in &metric_families {
            if family.get_name() == "test_execution_duration_seconds" {
                for metric in family.get_metric() {
                    let histogram = metric.get_histogram();
                    let sample_count = histogram.get_sample_count();
                    if sample_count > 0 {
                        average_test_duration = histogram.get_sample_sum() / sample_count as f64;
                        total_execution_time = histogram.get_sample_sum();
                    }
                }
            }
        }
        
        Ok(TestExecutionMetrics {
            total_test_cases: total,
            passed_test_cases: passed,
            failed_test_cases: failed,
            success_rate,
            average_test_duration,
            total_execution_time,
        })
    }
    
    /// Export metrics in Prometheus format
    pub fn export_prometheus(&self) -> Result<String, ApiTestError> {
        let encoder = TextEncoder::new();
        let metric_families = self.registry.gather();
        let mut buffer = Vec::new();
        encoder.encode(&metric_families, &mut buffer)?;
        Ok(String::from_utf8(buffer)?)
    }
    
    /// Start background metrics collection
    pub async fn start_collection(&self) -> Result<(), ApiTestError> {
        if !self.config.enabled {
            return Ok(());
        }
        
        let collector = self.clone();
        let interval_duration = self.config.collection_interval;
        
        tokio::spawn(async move {
            let mut interval = interval(interval_duration);
            
            loop {
                interval.tick().await;
                
                if let Err(e) = collector.update_system_metrics() {
                    log::error!("Failed to update system metrics: {}", e);
                }
            }
        });
        
        Ok(())
    }
}

/// Timer helper for measuring execution duration
pub struct MetricsTimer {
    start_time: Instant,
    collector: Arc<MetricsCollector>,
    labels: Vec<String>,
}

impl MetricsTimer {
    pub fn new(collector: Arc<MetricsCollector>, method: String, endpoint: String) -> Self {
        Self {
            start_time: Instant::now(),
            collector,
            labels: vec![method, endpoint],
        }
    }
    
    pub fn finish(self, status_code: u16) {
        let duration = self.start_time.elapsed();
        self.collector.record_request(
            &self.labels[0],
            &self.labels[1],
            status_code,
            duration,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    
    #[tokio::test]
    async fn test_metrics_collector_creation() {
        let config = MetricsConfig::default();
        let collector = MetricsCollector::new(config).unwrap();
        
        // Test recording a request
        collector.record_request("GET", "/api/test", 200, Duration::from_millis(100));
        
        // Test recording an error
        collector.record_error("timeout", "/api/test");
        
        // Test recording test case
        collector.record_test_case(true, Duration::from_secs(1));
        
        // Test getting snapshot
        let snapshot = collector.get_snapshot().unwrap();
        assert_eq!(snapshot.performance.total_requests, 1);
        assert_eq!(snapshot.test_execution.total_test_cases, 1);
    }
    
    #[test]
    fn test_prometheus_export() {
        let config = MetricsConfig::default();
        let collector = MetricsCollector::new(config).unwrap();
        
        collector.record_request("GET", "/api/test", 200, Duration::from_millis(100));
        
        let prometheus_output = collector.export_prometheus().unwrap();
        assert!(prometheus_output.contains("api_requests_total"));
        assert!(prometheus_output.contains("api_request_duration_seconds"));
    }
    
    #[test]
    fn test_metrics_timer() {
        let config = MetricsConfig::default();
        let collector = Arc::new(MetricsCollector::new(config).unwrap());
        
        let timer = MetricsTimer::new(
            collector.clone(),
            "POST".to_string(),
            "/api/create".to_string(),
        );
        
        // Simulate some work
        std::thread::sleep(Duration::from_millis(10));
        
        timer.finish(201);
        
        let snapshot = collector.get_snapshot().unwrap();
        assert_eq!(snapshot.performance.total_requests, 1);
    }
}