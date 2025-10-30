use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use tokio::time::interval;
use crate::error::ApiTestError;
use crate::metrics::{MetricsCollector, MetricsSnapshot};

/// Historical metrics storage and analysis system
#[derive(Clone)]
pub struct MetricsHistory {
    config: MetricsHistoryConfig,
    storage: Arc<RwLock<MetricsStorage>>,
    metrics_collector: Arc<MetricsCollector>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsHistoryConfig {
    pub enabled: bool,
    pub collection_interval: Duration,
    pub retention_period: Duration,
    pub max_data_points: usize,
    pub baseline_calculation_period: Duration,
    pub anomaly_detection_enabled: bool,
    pub anomaly_threshold_multiplier: f64,
}

impl Default for MetricsHistoryConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            collection_interval: Duration::from_secs(60), // Collect every minute
            retention_period: Duration::from_secs(7 * 24 * 60 * 60), // 7 days
            max_data_points: 10080, // 7 days * 24 hours * 60 minutes
            baseline_calculation_period: Duration::from_secs(24 * 60 * 60), // 24 hours
            anomaly_detection_enabled: true,
            anomaly_threshold_multiplier: 2.0, // 2x standard deviation
        }
    }
}

#[derive(Debug, Clone)]
struct MetricsStorage {
    snapshots: VecDeque<TimestampedSnapshot>,
    baselines: HashMap<String, MetricBaseline>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TimestampedSnapshot {
    pub timestamp: SystemTime,
    pub snapshot: MetricsSnapshot,
}

#[derive(Debug, Clone, Serialize)]
pub struct MetricBaseline {
    pub metric_name: String,
    pub mean: f64,
    pub std_deviation: f64,
    pub min: f64,
    pub max: f64,
    pub sample_count: usize,
    pub calculated_at: SystemTime,
    pub calculation_period: Duration,
}

#[derive(Debug, Clone, Serialize)]
pub struct TrendAnalysis {
    pub metric_name: String,
    pub time_range: TimeRange,
    pub trend_direction: TrendDirection,
    pub trend_strength: f64, // -1.0 to 1.0
    pub slope: f64,
    pub r_squared: f64, // Coefficient of determination
    pub data_points: usize,
    pub anomalies: Vec<AnomalyPoint>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TimeRange {
    pub start: SystemTime,
    pub end: SystemTime,
}

#[derive(Debug, Clone, Serialize)]
pub enum TrendDirection {
    Increasing,
    Decreasing,
    Stable,
    Volatile,
}

#[derive(Debug, Clone, Serialize)]
pub struct AnomalyPoint {
    pub timestamp: SystemTime,
    pub value: f64,
    pub expected_value: f64,
    pub deviation_score: f64, // How many standard deviations from baseline
    pub anomaly_type: AnomalyType,
}

#[derive(Debug, Clone, Serialize)]
pub enum AnomalyType {
    Spike,
    Drop,
    Outlier,
}

#[derive(Debug, Clone, Serialize)]
pub struct PerformanceComparison {
    pub metric_name: String,
    pub current_period: MetricSummary,
    pub baseline_period: MetricSummary,
    pub improvement_percentage: f64,
    pub comparison_type: ComparisonType,
}

#[derive(Debug, Clone, Serialize)]
pub struct MetricSummary {
    pub mean: f64,
    pub median: f64,
    pub p95: f64,
    pub p99: f64,
    pub min: f64,
    pub max: f64,
    pub std_deviation: f64,
    pub sample_count: usize,
    pub time_range: TimeRange,
}

#[derive(Debug, Clone, Serialize)]
pub enum ComparisonType {
    Improvement,
    Degradation,
    NoSignificantChange,
}

impl MetricsHistory {
    pub fn new(
        config: MetricsHistoryConfig,
        metrics_collector: Arc<MetricsCollector>,
    ) -> Self {
        Self {
            config,
            storage: Arc::new(RwLock::new(MetricsStorage {
                snapshots: VecDeque::new(),
                baselines: HashMap::new(),
            })),
            metrics_collector,
        }
    }

    /// Start collecting historical metrics
    pub async fn start_collection(&self) -> Result<(), ApiTestError> {
        if !self.config.enabled {
            return Ok(());
        }

        let history = self.clone();
        let collection_interval = self.config.collection_interval;

        tokio::spawn(async move {
            let mut interval = interval(collection_interval);

            loop {
                interval.tick().await;

                if let Err(e) = history.collect_snapshot().await {
                    log::error!("Failed to collect metrics snapshot: {}", e);
                }

                if let Err(e) = history.cleanup_old_data() {
                    log::error!("Failed to cleanup old metrics data: {}", e);
                }

                if let Err(e) = history.update_baselines() {
                    log::error!("Failed to update metrics baselines: {}", e);
                }
            }
        });

        Ok(())
    }

    async fn collect_snapshot(&self) -> Result<(), ApiTestError> {
        let snapshot = self.metrics_collector.get_snapshot()?;
        let timestamped_snapshot = TimestampedSnapshot {
            timestamp: SystemTime::now(),
            snapshot,
        };

        let mut storage = self.storage.write();
        storage.snapshots.push_back(timestamped_snapshot);

        // Limit the number of stored snapshots
        while storage.snapshots.len() > self.config.max_data_points {
            storage.snapshots.pop_front();
        }

        Ok(())
    }

    fn cleanup_old_data(&self) -> Result<(), ApiTestError> {
        let cutoff_time = SystemTime::now()
            .checked_sub(self.config.retention_period)
            .unwrap_or(UNIX_EPOCH);

        let mut storage = self.storage.write();
        
        // Remove old snapshots
        while let Some(front) = storage.snapshots.front() {
            if front.timestamp < cutoff_time {
                storage.snapshots.pop_front();
            } else {
                break;
            }
        }

        // Remove old baselines
        storage.baselines.retain(|_, baseline| {
            baseline.calculated_at
                .checked_add(self.config.baseline_calculation_period)
                .map(|expiry| expiry > SystemTime::now())
                .unwrap_or(false)
        });

        Ok(())
    }

    fn update_baselines(&self) -> Result<(), ApiTestError> {
        let snapshots: Vec<TimestampedSnapshot> = {
            let storage = self.storage.read();
            storage.snapshots.iter().cloned().collect()
        };

        if snapshots.is_empty() {
            return Ok(());
        }

        let baseline_cutoff = SystemTime::now()
            .checked_sub(self.config.baseline_calculation_period)
            .unwrap_or(UNIX_EPOCH);

        let baseline_snapshots: Vec<&TimestampedSnapshot> = snapshots
            .iter()
            .filter(|s| s.timestamp >= baseline_cutoff)
            .collect();

        if baseline_snapshots.len() < 10 {
            // Need at least 10 data points for meaningful baseline
            return Ok(());
        }

        let mut new_baselines = HashMap::new();

        // Calculate baselines for key metrics
        self.calculate_baseline(&baseline_snapshots, "error_rate", &mut new_baselines)?;
        self.calculate_baseline(&baseline_snapshots, "avg_response_time", &mut new_baselines)?;
        self.calculate_baseline(&baseline_snapshots, "p95_response_time", &mut new_baselines)?;
        self.calculate_baseline(&baseline_snapshots, "cpu_usage", &mut new_baselines)?;
        self.calculate_baseline(&baseline_snapshots, "memory_usage", &mut new_baselines)?;
        self.calculate_baseline(&baseline_snapshots, "test_success_rate", &mut new_baselines)?;

        // Update storage with new baselines
        let mut storage = self.storage.write();
        for (key, baseline) in new_baselines {
            storage.baselines.insert(key, baseline);
        }

        Ok(())
    }

    fn calculate_baseline(
        &self,
        snapshots: &[&TimestampedSnapshot],
        metric_name: &str,
        baselines: &mut HashMap<String, MetricBaseline>,
    ) -> Result<(), ApiTestError> {
        let values: Vec<f64> = snapshots
            .iter()
            .map(|s| self.extract_metric_value(&s.snapshot, metric_name))
            .collect();

        if values.is_empty() {
            return Ok(());
        }

        let mean = values.iter().sum::<f64>() / values.len() as f64;
        let variance = values
            .iter()
            .map(|v| (v - mean).powi(2))
            .sum::<f64>() / values.len() as f64;
        let std_deviation = variance.sqrt();
        let min = values.iter().fold(f64::INFINITY, |a, &b| a.min(b));
        let max = values.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));

        let baseline = MetricBaseline {
            metric_name: metric_name.to_string(),
            mean,
            std_deviation,
            min,
            max,
            sample_count: values.len(),
            calculated_at: SystemTime::now(),
            calculation_period: self.config.baseline_calculation_period,
        };

        baselines.insert(metric_name.to_string(), baseline);
        Ok(())
    }

    fn extract_metric_value(&self, snapshot: &MetricsSnapshot, metric_name: &str) -> f64 {
        match metric_name {
            "error_rate" => snapshot.performance.error_rate,
            "avg_response_time" => snapshot.performance.average_response_time,
            "p95_response_time" => snapshot.performance.p95_response_time,
            "cpu_usage" => snapshot.resource_usage.cpu_usage_percent,
            "memory_usage" => snapshot.resource_usage.memory_usage_percent,
            "test_success_rate" => snapshot.test_execution.success_rate,
            _ => 0.0,
        }
    }

    /// Get trend analysis for a specific metric over a time range
    pub fn get_trend_analysis(
        &self,
        metric_name: &str,
        time_range: TimeRange,
    ) -> Result<TrendAnalysis, ApiTestError> {
        let storage = self.storage.read();
        
        let filtered_snapshots: Vec<_> = storage
            .snapshots
            .iter()
            .filter(|s| s.timestamp >= time_range.start && s.timestamp <= time_range.end)
            .collect();

        if filtered_snapshots.len() < 2 {
            return Err(ApiTestError::Configuration(
                "Insufficient data points for trend analysis".to_string(),
            ));
        }

        let data_points: Vec<(f64, f64)> = filtered_snapshots
            .iter()
            .enumerate()
            .map(|(i, s)| {
                let x = i as f64;
                let y = self.extract_metric_value(&s.snapshot, metric_name);
                (x, y)
            })
            .collect();

        let (slope, r_squared) = self.calculate_linear_regression(&data_points);
        let trend_direction = self.determine_trend_direction(slope, r_squared);
        let trend_strength = self.calculate_trend_strength(slope, &data_points);

        let anomalies = if self.config.anomaly_detection_enabled {
            self.detect_anomalies(metric_name, &filtered_snapshots)?
        } else {
            Vec::new()
        };

        Ok(TrendAnalysis {
            metric_name: metric_name.to_string(),
            time_range,
            trend_direction,
            trend_strength,
            slope,
            r_squared,
            data_points: data_points.len(),
            anomalies,
        })
    }

    fn calculate_linear_regression(&self, data_points: &[(f64, f64)]) -> (f64, f64) {
        let n = data_points.len() as f64;
        let sum_x: f64 = data_points.iter().map(|(x, _)| x).sum();
        let sum_y: f64 = data_points.iter().map(|(_, y)| y).sum();
        let sum_xy: f64 = data_points.iter().map(|(x, y)| x * y).sum();
        let sum_x_squared: f64 = data_points.iter().map(|(x, _)| x * x).sum();
        let _sum_y_squared: f64 = data_points.iter().map(|(_, y)| y * y).sum();

        let slope = (n * sum_xy - sum_x * sum_y) / (n * sum_x_squared - sum_x * sum_x);
        
        let mean_y = sum_y / n;
        let ss_tot: f64 = data_points.iter().map(|(_, y)| (y - mean_y).powi(2)).sum();
        let ss_res: f64 = data_points
            .iter()
            .enumerate()
            .map(|(i, (_, y))| {
                let predicted = slope * (i as f64) + (sum_y - slope * sum_x) / n;
                (y - predicted).powi(2)
            })
            .sum();

        let r_squared = if ss_tot > 0.0 { 1.0 - (ss_res / ss_tot) } else { 0.0 };

        (slope, r_squared.max(0.0).min(1.0))
    }

    fn determine_trend_direction(&self, slope: f64, r_squared: f64) -> TrendDirection {
        if r_squared < 0.1 {
            return TrendDirection::Volatile;
        }

        if slope.abs() < 0.001 {
            TrendDirection::Stable
        } else if slope > 0.0 {
            TrendDirection::Increasing
        } else {
            TrendDirection::Decreasing
        }
    }

    fn calculate_trend_strength(&self, slope: f64, data_points: &[(f64, f64)]) -> f64 {
        if data_points.is_empty() {
            return 0.0;
        }

        let y_values: Vec<f64> = data_points.iter().map(|(_, y)| *y).collect();
        let y_range = y_values.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b))
            - y_values.iter().fold(f64::INFINITY, |a, &b| a.min(b));

        if y_range == 0.0 {
            return 0.0;
        }

        let normalized_slope = slope / y_range * data_points.len() as f64;
        normalized_slope.max(-1.0).min(1.0)
    }

    fn detect_anomalies(
        &self,
        metric_name: &str,
        snapshots: &[&TimestampedSnapshot],
    ) -> Result<Vec<AnomalyPoint>, ApiTestError> {
        let storage = self.storage.read();
        let baseline = match storage.baselines.get(metric_name) {
            Some(b) => b,
            None => return Ok(Vec::new()), // No baseline available
        };

        let threshold = baseline.std_deviation * self.config.anomaly_threshold_multiplier;
        let mut anomalies = Vec::new();

        for snapshot in snapshots {
            let value = self.extract_metric_value(&snapshot.snapshot, metric_name);
            let deviation = (value - baseline.mean).abs();
            
            if deviation > threshold {
                let deviation_score = deviation / baseline.std_deviation;
                let anomaly_type = if value > baseline.mean + threshold {
                    AnomalyType::Spike
                } else if value < baseline.mean - threshold {
                    AnomalyType::Drop
                } else {
                    AnomalyType::Outlier
                };

                anomalies.push(AnomalyPoint {
                    timestamp: snapshot.timestamp,
                    value,
                    expected_value: baseline.mean,
                    deviation_score,
                    anomaly_type,
                });
            }
        }

        Ok(anomalies)
    }

    /// Compare current performance against baseline
    pub fn compare_with_baseline(
        &self,
        metric_name: &str,
        current_period: Duration,
    ) -> Result<PerformanceComparison, ApiTestError> {
        let now = SystemTime::now();
        let current_start = now.checked_sub(current_period).unwrap_or(UNIX_EPOCH);
        
        let storage = self.storage.read();
        let baseline = storage.baselines.get(metric_name)
            .ok_or_else(|| ApiTestError::Configuration(format!("No baseline found for metric: {}", metric_name)))?;

        let current_snapshots: Vec<_> = storage
            .snapshots
            .iter()
            .filter(|s| s.timestamp >= current_start)
            .collect();

        if current_snapshots.is_empty() {
            return Err(ApiTestError::Configuration(
                "No current data available for comparison".to_string(),
            ));
        }

        let current_values: Vec<f64> = current_snapshots
            .iter()
            .map(|s| self.extract_metric_value(&s.snapshot, metric_name))
            .collect();

        let current_summary = self.calculate_metric_summary(&current_values, TimeRange {
            start: current_start,
            end: now,
        });

        let baseline_summary = MetricSummary {
            mean: baseline.mean,
            median: baseline.mean, // Approximation
            p95: baseline.mean + 1.645 * baseline.std_deviation, // Approximation
            p99: baseline.mean + 2.326 * baseline.std_deviation, // Approximation
            min: baseline.min,
            max: baseline.max,
            std_deviation: baseline.std_deviation,
            sample_count: baseline.sample_count,
            time_range: TimeRange {
                start: baseline.calculated_at.checked_sub(baseline.calculation_period).unwrap_or(UNIX_EPOCH),
                end: baseline.calculated_at,
            },
        };

        let improvement_percentage = if baseline.mean != 0.0 {
            ((baseline.mean - current_summary.mean) / baseline.mean) * 100.0
        } else {
            0.0
        };

        let comparison_type = if improvement_percentage.abs() < 5.0 {
            ComparisonType::NoSignificantChange
        } else if improvement_percentage > 0.0 {
            ComparisonType::Improvement
        } else {
            ComparisonType::Degradation
        };

        Ok(PerformanceComparison {
            metric_name: metric_name.to_string(),
            current_period: current_summary,
            baseline_period: baseline_summary,
            improvement_percentage,
            comparison_type,
        })
    }

    fn calculate_metric_summary(&self, values: &[f64], time_range: TimeRange) -> MetricSummary {
        if values.is_empty() {
            return MetricSummary {
                mean: 0.0,
                median: 0.0,
                p95: 0.0,
                p99: 0.0,
                min: 0.0,
                max: 0.0,
                std_deviation: 0.0,
                sample_count: 0,
                time_range,
            };
        }

        let mut sorted_values = values.to_vec();
        sorted_values.sort_by(|a, b| a.partial_cmp(b).unwrap());

        let mean = values.iter().sum::<f64>() / values.len() as f64;
        let median = if sorted_values.len() % 2 == 0 {
            (sorted_values[sorted_values.len() / 2 - 1] + sorted_values[sorted_values.len() / 2]) / 2.0
        } else {
            sorted_values[sorted_values.len() / 2]
        };

        let p95_index = ((sorted_values.len() as f64) * 0.95) as usize;
        let p99_index = ((sorted_values.len() as f64) * 0.99) as usize;
        let p95 = sorted_values[p95_index.min(sorted_values.len() - 1)];
        let p99 = sorted_values[p99_index.min(sorted_values.len() - 1)];

        let variance = values
            .iter()
            .map(|v| (v - mean).powi(2))
            .sum::<f64>() / values.len() as f64;
        let std_deviation = variance.sqrt();

        MetricSummary {
            mean,
            median,
            p95,
            p99,
            min: sorted_values[0],
            max: sorted_values[sorted_values.len() - 1],
            std_deviation,
            sample_count: values.len(),
            time_range,
        }
    }

    /// Get all available baselines
    pub fn get_baselines(&self) -> HashMap<String, MetricBaseline> {
        self.storage.read().baselines.clone()
    }

    /// Get historical snapshots within a time range
    pub fn get_snapshots(&self, time_range: TimeRange) -> Vec<TimestampedSnapshot> {
        self.storage
            .read()
            .snapshots
            .iter()
            .filter(|s| s.timestamp >= time_range.start && s.timestamp <= time_range.end)
            .cloned()
            .collect()
    }

    /// Get metrics summary for the last N periods
    pub fn get_recent_summary(&self, periods: usize) -> Result<Vec<MetricSummary>, ApiTestError> {
        let storage = self.storage.read();
        let snapshots: Vec<_> = storage.snapshots.iter().rev().take(periods).collect();
        
        if snapshots.is_empty() {
            return Ok(Vec::new());
        }

        let period_duration = self.config.collection_interval;
        let mut summaries = Vec::new();

        for (_i, snapshot) in snapshots.iter().enumerate() {
            let start_time = snapshot.timestamp.checked_sub(period_duration).unwrap_or(UNIX_EPOCH);
            let time_range = TimeRange {
                start: start_time,
                end: snapshot.timestamp,
            };

            // For simplicity, create a summary with just this snapshot's values
            let values = vec![
                snapshot.snapshot.performance.error_rate,
                snapshot.snapshot.performance.average_response_time,
                snapshot.snapshot.resource_usage.cpu_usage_percent,
            ];

            summaries.push(self.calculate_metric_summary(&values, time_range));
        }

        Ok(summaries)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metrics::MetricsConfig;
    use std::time::Duration;

    #[tokio::test]
    async fn test_metrics_history_creation() {
        let metrics_config = MetricsConfig::default();
        let metrics_collector = Arc::new(MetricsCollector::new(metrics_config).unwrap());
        let history_config = MetricsHistoryConfig::default();
        
        let history = MetricsHistory::new(history_config, metrics_collector);
        
        assert_eq!(history.get_baselines().len(), 0);
    }

    #[test]
    fn test_linear_regression() {
        let metrics_config = MetricsConfig::default();
        let metrics_collector = Arc::new(MetricsCollector::new(metrics_config).unwrap());
        let history_config = MetricsHistoryConfig::default();
        let history = MetricsHistory::new(history_config, metrics_collector);

        // Test with perfect linear relationship
        let data_points = vec![(0.0, 0.0), (1.0, 1.0), (2.0, 2.0), (3.0, 3.0)];
        let (slope, r_squared) = history.calculate_linear_regression(&data_points);
        
        assert!((slope - 1.0).abs() < 0.001);
        assert!(r_squared > 0.99);
    }

    #[test]
    fn test_trend_direction_determination() {
        let metrics_config = MetricsConfig::default();
        let metrics_collector = Arc::new(MetricsCollector::new(metrics_config).unwrap());
        let history_config = MetricsHistoryConfig::default();
        let history = MetricsHistory::new(history_config, metrics_collector);

        assert!(matches!(
            history.determine_trend_direction(0.1, 0.8),
            TrendDirection::Increasing
        ));
        assert!(matches!(
            history.determine_trend_direction(-0.1, 0.8),
            TrendDirection::Decreasing
        ));
        assert!(matches!(
            history.determine_trend_direction(0.0001, 0.8),
            TrendDirection::Stable
        ));
        assert!(matches!(
            history.determine_trend_direction(0.1, 0.05),
            TrendDirection::Volatile
        ));
    }
}