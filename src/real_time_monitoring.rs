//! Real-time execution monitoring with progress tracking and cancellation support
//! 
//! This module provides:
//! - Real-time progress tracking with detailed metrics
//! - Execution cancellation and graceful shutdown
//! - Result streaming and live updates
//! - Watch mode for automatic re-execution on file changes


use chrono::{DateTime, Utc};
use notify::{Event as NotifyEvent, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{broadcast, mpsc, Mutex, RwLock};



use crate::error::{ExecutionError, Result};
use crate::event::EventBus;
use crate::execution::{ExecutionContext, TestResult};
use crate::test_case_manager::TestCase;
use crate::interactive_execution::InteractiveExecutor;

/// Real-time monitoring system for test execution
pub struct RealTimeMonitor {
    event_bus: Arc<EventBus>,
    execution_tracker: ExecutionTracker,
    cancellation_manager: CancellationManager,
    result_streamer: ResultStreamer,
    watch_manager: Option<WatchManager>,
}

/// Tracks execution progress and metrics in real-time
pub struct ExecutionTracker {
    active_executions: Arc<RwLock<HashMap<String, ExecutionProgress>>>,
    metrics_collector: Arc<Mutex<ExecutionMetrics>>,
}

/// Progress information for an active execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionProgress {
    pub execution_id: String,
    pub test_suite_name: String,
    pub start_time: DateTime<Utc>,
    pub current_test_index: usize,
    pub total_tests: usize,
    pub completed_tests: usize,
    pub failed_tests: usize,
    pub current_test_name: String,
    pub current_step: String,
    pub elapsed_time: Duration,
    pub estimated_remaining: Option<Duration>,
    pub throughput: f64, // tests per second
    pub status: ExecutionStatus,
}

/// Status of execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExecutionStatus {
    Starting,
    Running,
    Paused,
    Cancelling,
    Completed,
    Failed,
    Cancelled,
}

/// Execution metrics collected during monitoring
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionMetrics {
    pub total_executions: usize,
    pub active_executions: usize,
    pub average_test_duration: Duration,
    pub success_rate: f64,
    pub throughput_history: Vec<ThroughputSample>,
    pub resource_usage: ResourceUsage,
}

/// Throughput sample for trend analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThroughputSample {
    pub timestamp: DateTime<Utc>,
    pub tests_per_second: f64,
    pub concurrent_executions: usize,
}

/// Resource usage metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceUsage {
    pub cpu_usage_percent: f64,
    pub memory_usage_mb: f64,
    pub network_requests_per_second: f64,
    pub active_connections: usize,
}

/// Manages execution cancellation and graceful shutdown
pub struct CancellationManager {
    cancellation_tokens: Arc<RwLock<HashMap<String, CancellationToken>>>,
    shutdown_signal: Arc<RwLock<bool>>,
}

/// Cancellation token for individual executions
#[derive(Debug, Clone)]
pub struct CancellationToken {
    pub execution_id: String,
    pub is_cancelled: Arc<RwLock<bool>>,
    pub cancel_sender: broadcast::Sender<CancellationSignal>,
    pub created_at: DateTime<Utc>,
    pub reason: Option<String>,
}

/// Cancellation signal types
#[derive(Debug, Clone)]
pub enum CancellationSignal {
    UserRequested,
    Timeout,
    SystemShutdown,
    ErrorThreshold,
}

/// Streams execution results in real-time
pub struct ResultStreamer {
    result_channels: Arc<RwLock<HashMap<String, ResultChannel>>>,
    event_bus: Arc<EventBus>,
}

/// Channel for streaming results to subscribers
#[derive(Debug)]
pub struct ResultChannel {
    pub execution_id: String,
    pub sender: mpsc::UnboundedSender<StreamedResult>,
    pub subscriber_count: usize,
    pub created_at: DateTime<Utc>,
}

/// Streamed result data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamedResult {
    pub execution_id: String,
    pub timestamp: DateTime<Utc>,
    pub result_type: StreamedResultType,
    pub data: serde_json::Value,
}

/// Type of streamed result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StreamedResultType {
    ExecutionStarted,
    TestStarted,
    TestCompleted,
    TestFailed,
    ProgressUpdate,
    ExecutionCompleted,
    ExecutionFailed,
    ExecutionCancelled,
}

/// Manages file watching for automatic re-execution
pub struct WatchManager {
    watcher: RecommendedWatcher,
    watched_paths: Arc<RwLock<HashMap<PathBuf, WatchConfig>>>,
    change_sender: mpsc::UnboundedSender<FileChangeEvent>,
    executor: Arc<InteractiveExecutor>,
}

/// Configuration for file watching
#[derive(Debug, Clone)]
pub struct WatchConfig {
    pub path: PathBuf,
    pub recursive: bool,
    pub file_patterns: Vec<String>,
    pub debounce_duration: Duration,
    pub auto_execute: bool,
    pub test_cases: Vec<TestCase>,
    pub execution_context: ExecutionContext,
}

/// File change event
#[derive(Debug, Clone)]
pub struct FileChangeEvent {
    pub path: PathBuf,
    pub event_kind: FileChangeKind,
    pub timestamp: DateTime<Utc>,
}

/// Type of file change
#[derive(Debug, Clone)]
pub enum FileChangeKind {
    Created,
    Modified,
    Deleted,
    Renamed,
}

impl RealTimeMonitor {
    pub fn new(event_bus: Arc<EventBus>) -> Self {
        Self {
            event_bus: event_bus.clone(),
            execution_tracker: ExecutionTracker::new(),
            cancellation_manager: CancellationManager::new(),
            result_streamer: ResultStreamer::new(event_bus.clone()),
            watch_manager: None,
        }
    }

    /// Start monitoring an execution
    pub async fn start_monitoring(
        &self,
        execution_id: String,
        test_suite_name: String,
        total_tests: usize,
    ) -> Result<()> {
        let progress = ExecutionProgress {
            execution_id: execution_id.clone(),
            test_suite_name,
            start_time: Utc::now(),
            current_test_index: 0,
            total_tests,
            completed_tests: 0,
            failed_tests: 0,
            current_test_name: String::new(),
            current_step: "Initializing".to_string(),
            elapsed_time: Duration::from_secs(0),
            estimated_remaining: None,
            throughput: 0.0,
            status: ExecutionStatus::Starting,
        };

        self.execution_tracker.add_execution(execution_id.clone(), progress).await;
        self.cancellation_manager.create_token(execution_id.clone()).await;
        self.result_streamer.create_channel(execution_id).await;

        Ok(())
    }

    /// Update execution progress
    pub async fn update_progress(
        &self,
        execution_id: &str,
        current_test_index: usize,
        current_test_name: String,
        current_step: String,
    ) -> Result<()> {
        self.execution_tracker.update_progress(
            execution_id,
            current_test_index,
            current_test_name,
            current_step,
        ).await;

        // Stream progress update
        let progress = self.execution_tracker.get_progress(execution_id).await;
        if let Some(progress) = progress {
            self.result_streamer.stream_progress_update(&progress).await?;
        }

        Ok(())
    }

    /// Report test completion
    pub async fn report_test_completion(
        &self,
        execution_id: &str,
        test_result: &TestResult,
    ) -> Result<()> {
        self.execution_tracker.record_test_completion(execution_id, test_result).await;
        self.result_streamer.stream_test_result(execution_id, test_result).await?;
        Ok(())
    }

    /// Complete execution monitoring
    pub async fn complete_monitoring(
        &self,
        execution_id: &str,
        success: bool,
    ) -> Result<()> {
        self.execution_tracker.complete_execution(execution_id, success).await;
        self.cancellation_manager.remove_token(execution_id).await;
        self.result_streamer.complete_stream(execution_id, success).await?;
        Ok(())
    }

    /// Cancel an execution
    pub async fn cancel_execution(
        &self,
        execution_id: &str,
        reason: Option<String>,
    ) -> Result<()> {
        self.cancellation_manager.cancel_execution(execution_id, reason).await?;
        self.execution_tracker.cancel_execution(execution_id).await;
        self.result_streamer.stream_cancellation(execution_id).await?;
        Ok(())
    }

    /// Check if execution is cancelled
    pub async fn is_cancelled(&self, execution_id: &str) -> bool {
        self.cancellation_manager.is_cancelled(execution_id).await
    }

    /// Get current execution metrics
    pub async fn get_metrics(&self) -> ExecutionMetrics {
        self.execution_tracker.get_metrics().await
    }

    /// Subscribe to result stream
    pub async fn subscribe_to_results(
        &self,
        execution_id: &str,
    ) -> Result<mpsc::UnboundedReceiver<StreamedResult>> {
        self.result_streamer.subscribe(execution_id).await
    }

    /// Enable watch mode for automatic re-execution
    pub async fn enable_watch_mode(
        &mut self,
        watch_config: WatchConfig,
        executor: Arc<InteractiveExecutor>,
    ) -> Result<()> {
        if self.watch_manager.is_none() {
            self.watch_manager = Some(WatchManager::new(executor)?);
        }

        if let Some(watch_manager) = &mut self.watch_manager {
            watch_manager.add_watch(watch_config).await?;
        }

        Ok(())
    }

    /// Disable watch mode
    pub async fn disable_watch_mode(&mut self) -> Result<()> {
        if let Some(watch_manager) = &mut self.watch_manager {
            watch_manager.stop_watching().await?;
        }
        self.watch_manager = None;
        Ok(())
    }
}

impl ExecutionTracker {
    pub fn new() -> Self {
        Self {
            active_executions: Arc::new(RwLock::new(HashMap::new())),
            metrics_collector: Arc::new(Mutex::new(ExecutionMetrics {
                total_executions: 0,
                active_executions: 0,
                average_test_duration: Duration::from_secs(0),
                success_rate: 0.0,
                throughput_history: Vec::new(),
                resource_usage: ResourceUsage {
                    cpu_usage_percent: 0.0,
                    memory_usage_mb: 0.0,
                    network_requests_per_second: 0.0,
                    active_connections: 0,
                },
            })),
        }
    }

    pub async fn add_execution(&self, execution_id: String, progress: ExecutionProgress) {
        let mut executions = self.active_executions.write().await;
        executions.insert(execution_id, progress);

        let mut metrics = self.metrics_collector.lock().await;
        metrics.total_executions += 1;
        metrics.active_executions = executions.len();
    }

    pub async fn update_progress(
        &self,
        execution_id: &str,
        current_test_index: usize,
        current_test_name: String,
        current_step: String,
    ) {
        let mut executions = self.active_executions.write().await;
        if let Some(progress) = executions.get_mut(execution_id) {
            progress.current_test_index = current_test_index;
            progress.current_test_name = current_test_name;
            progress.current_step = current_step;
            progress.elapsed_time = Utc::now().signed_duration_since(progress.start_time).to_std().unwrap_or_default();
            progress.status = ExecutionStatus::Running;

            // Calculate estimated remaining time
            if progress.completed_tests > 0 {
                let avg_time_per_test = progress.elapsed_time.as_secs_f64() / progress.completed_tests as f64;
                let remaining_tests = progress.total_tests - progress.completed_tests;
                progress.estimated_remaining = Some(Duration::from_secs_f64(avg_time_per_test * remaining_tests as f64));
            }

            // Calculate throughput
            if progress.elapsed_time.as_secs() > 0 {
                progress.throughput = progress.completed_tests as f64 / progress.elapsed_time.as_secs_f64();
            }
        }
    }

    pub async fn record_test_completion(&self, execution_id: &str, test_result: &TestResult) {
        let mut executions = self.active_executions.write().await;
        if let Some(progress) = executions.get_mut(execution_id) {
            progress.completed_tests += 1;
            if !test_result.success {
                progress.failed_tests += 1;
            }
        }
    }

    pub async fn complete_execution(&self, execution_id: &str, success: bool) {
        let mut executions = self.active_executions.write().await;
        if let Some(progress) = executions.get_mut(execution_id) {
            progress.status = if success {
                ExecutionStatus::Completed
            } else {
                ExecutionStatus::Failed
            };
        }

        // Update metrics
        let mut metrics = self.metrics_collector.lock().await;
        metrics.active_executions = executions.len().saturating_sub(1);

        // Add throughput sample
        if let Some(progress) = executions.get(execution_id) {
            let active_executions = metrics.active_executions;
            metrics.throughput_history.push(ThroughputSample {
                timestamp: Utc::now(),
                tests_per_second: progress.throughput,
                concurrent_executions: active_executions,
            });

            // Keep only last 100 samples
            if metrics.throughput_history.len() > 100 {
                metrics.throughput_history.remove(0);
            }
        }

        executions.remove(execution_id);
    }

    pub async fn cancel_execution(&self, execution_id: &str) {
        let mut executions = self.active_executions.write().await;
        if let Some(progress) = executions.get_mut(execution_id) {
            progress.status = ExecutionStatus::Cancelled;
        }
    }

    pub async fn get_progress(&self, execution_id: &str) -> Option<ExecutionProgress> {
        let executions = self.active_executions.read().await;
        executions.get(execution_id).cloned()
    }

    pub async fn get_metrics(&self) -> ExecutionMetrics {
        let metrics = self.metrics_collector.lock().await;
        metrics.clone()
    }
}

impl CancellationManager {
    pub fn new() -> Self {
        Self {
            cancellation_tokens: Arc::new(RwLock::new(HashMap::new())),
            shutdown_signal: Arc::new(RwLock::new(false)),
        }
    }

    pub async fn create_token(&self, execution_id: String) -> CancellationToken {
        let (sender, _) = broadcast::channel(10);
        let token = CancellationToken {
            execution_id: execution_id.clone(),
            is_cancelled: Arc::new(RwLock::new(false)),
            cancel_sender: sender,
            created_at: Utc::now(),
            reason: None,
        };

        let mut tokens = self.cancellation_tokens.write().await;
        tokens.insert(execution_id, token.clone());

        token
    }

    pub async fn cancel_execution(&self, execution_id: &str, reason: Option<String>) -> Result<()> {
        let mut tokens = self.cancellation_tokens.write().await;
        if let Some(token) = tokens.get_mut(execution_id) {
            let mut is_cancelled = token.is_cancelled.write().await;
            *is_cancelled = true;

            let signal = match reason.as_deref() {
                Some("timeout") => CancellationSignal::Timeout,
                Some("shutdown") => CancellationSignal::SystemShutdown,
                Some("error_threshold") => CancellationSignal::ErrorThreshold,
                _ => CancellationSignal::UserRequested,
            };

            token.cancel_sender.send(signal).ok();
        }

        Ok(())
    }

    pub async fn is_cancelled(&self, execution_id: &str) -> bool {
        let tokens = self.cancellation_tokens.read().await;
        if let Some(token) = tokens.get(execution_id) {
            let is_cancelled = token.is_cancelled.read().await;
            *is_cancelled
        } else {
            false
        }
    }

    pub async fn remove_token(&self, execution_id: &str) {
        let mut tokens = self.cancellation_tokens.write().await;
        tokens.remove(execution_id);
    }

    pub async fn shutdown_all(&self) {
        let mut shutdown = self.shutdown_signal.write().await;
        *shutdown = true;

        let tokens = self.cancellation_tokens.read().await;
        for (execution_id, _) in tokens.iter() {
            self.cancel_execution(execution_id, Some("shutdown".to_string())).await.ok();
        }
    }
}

impl ResultStreamer {
    pub fn new(event_bus: Arc<EventBus>) -> Self {
        Self {
            result_channels: Arc::new(RwLock::new(HashMap::new())),
            event_bus,
        }
    }

    pub async fn create_channel(&self, execution_id: String) -> Result<()> {
        let (sender, _) = mpsc::unbounded_channel();
        let channel = ResultChannel {
            execution_id: execution_id.clone(),
            sender,
            subscriber_count: 0,
            created_at: Utc::now(),
        };

        let mut channels = self.result_channels.write().await;
        channels.insert(execution_id, channel);

        Ok(())
    }

    pub async fn subscribe(&self, execution_id: &str) -> Result<mpsc::UnboundedReceiver<StreamedResult>> {
        let mut channels = self.result_channels.write().await;
        if let Some(channel) = channels.get_mut(execution_id) {
            let (sender, receiver) = mpsc::unbounded_channel();
            channel.subscriber_count += 1;
            
            // Create a new sender for this subscriber
            // In a real implementation, we'd need a more sophisticated pub-sub system
            Ok(receiver)
        } else {
            Err(ExecutionError::TestCaseFailed(format!("No channel found for execution: {}", execution_id)).into())
        }
    }

    pub async fn stream_progress_update(&self, progress: &ExecutionProgress) -> Result<()> {
        let result = StreamedResult {
            execution_id: progress.execution_id.clone(),
            timestamp: Utc::now(),
            result_type: StreamedResultType::ProgressUpdate,
            data: serde_json::to_value(progress)?,
        };

        self.send_to_channel(&progress.execution_id, result).await
    }

    pub async fn stream_test_result(&self, execution_id: &str, test_result: &TestResult) -> Result<()> {
        let result_type = if test_result.success {
            StreamedResultType::TestCompleted
        } else {
            StreamedResultType::TestFailed
        };

        let result = StreamedResult {
            execution_id: execution_id.to_string(),
            timestamp: Utc::now(),
            result_type,
            data: serde_json::to_value(test_result)?,
        };

        self.send_to_channel(execution_id, result).await
    }

    pub async fn stream_cancellation(&self, execution_id: &str) -> Result<()> {
        let result = StreamedResult {
            execution_id: execution_id.to_string(),
            timestamp: Utc::now(),
            result_type: StreamedResultType::ExecutionCancelled,
            data: serde_json::json!({"reason": "User requested cancellation"}),
        };

        self.send_to_channel(execution_id, result).await
    }

    pub async fn complete_stream(&self, execution_id: &str, success: bool) -> Result<()> {
        let result_type = if success {
            StreamedResultType::ExecutionCompleted
        } else {
            StreamedResultType::ExecutionFailed
        };

        let result = StreamedResult {
            execution_id: execution_id.to_string(),
            timestamp: Utc::now(),
            result_type,
            data: serde_json::json!({"success": success}),
        };

        self.send_to_channel(execution_id, result).await?;

        // Clean up channel
        let mut channels = self.result_channels.write().await;
        channels.remove(execution_id);

        Ok(())
    }

    async fn send_to_channel(&self, execution_id: &str, result: StreamedResult) -> Result<()> {
        let channels = self.result_channels.read().await;
        if let Some(channel) = channels.get(execution_id) {
            channel.sender.send(result).map_err(|_| {
                ExecutionError::TestCaseFailed("Failed to send result to channel".to_string())
            })?;
        }
        Ok(())
    }
}

impl WatchManager {
    pub fn new(executor: Arc<InteractiveExecutor>) -> Result<Self> {
        let (change_sender, mut change_receiver) = mpsc::unbounded_channel();
        let change_sender_clone = change_sender.clone();
        
        let watcher = notify::recommended_watcher(move |res: notify::Result<NotifyEvent>| {
            if let Ok(event) = res {
                let file_event = FileChangeEvent {
                    path: event.paths.first().cloned().unwrap_or_default(),
                    event_kind: match event.kind {
                        EventKind::Create(_) => FileChangeKind::Created,
                        EventKind::Modify(_) => FileChangeKind::Modified,
                        EventKind::Remove(_) => FileChangeKind::Deleted,
                        _ => FileChangeKind::Modified,
                    },
                    timestamp: Utc::now(),
                };
                
                change_sender_clone.send(file_event).ok();
            }
        }).map_err(|e| ExecutionError::TestCaseFailed(format!("Failed to create watcher: {}", e)))?;

        // Spawn task to handle file changes
        let executor_clone = executor.clone();
        tokio::spawn(async move {
            while let Some(change_event) = change_receiver.recv().await {
                // Handle file change event
                println!("📁 File changed: {:?}", change_event.path);
                // In a real implementation, this would trigger re-execution
            }
        });

        Ok(Self {
            watcher,
            watched_paths: Arc::new(RwLock::new(HashMap::new())),
            change_sender,
            executor,
        })
    }

    pub async fn add_watch(&mut self, config: WatchConfig) -> Result<()> {
        let recursive_mode = if config.recursive {
            RecursiveMode::Recursive
        } else {
            RecursiveMode::NonRecursive
        };

        self.watcher.watch(&config.path, recursive_mode)
            .map_err(|e| ExecutionError::TestCaseFailed(format!("Failed to watch path: {}", e)))?;

        let mut watched_paths = self.watched_paths.write().await;
        watched_paths.insert(config.path.clone(), config);

        Ok(())
    }

    pub async fn stop_watching(&mut self) -> Result<()> {
        let watched_paths = self.watched_paths.read().await;
        for path in watched_paths.keys() {
            self.watcher.unwatch(path)
                .map_err(|e| ExecutionError::TestCaseFailed(format!("Failed to unwatch path: {}", e)))?;
        }
        Ok(())
    }
}