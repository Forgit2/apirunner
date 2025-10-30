use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::broadcast;
use uuid::Uuid;

/// Core event types for the API test runner
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Event {
    TestLifecycle(TestLifecycleEvent),
    RequestExecution(RequestExecutionEvent),
    AssertionResult(AssertionResultEvent),
    PluginLifecycle(PluginLifecycleEvent),
    SystemMetrics(SystemMetricsEvent),
    Error(ErrorEvent),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestLifecycleEvent {
    pub event_id: String,
    pub timestamp: DateTime<Utc>,
    pub test_suite_id: String,
    pub test_case_id: Option<String>,
    pub lifecycle_type: TestLifecycleType,
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TestLifecycleType {
    SuiteStarted,
    SuiteCompleted,
    SuiteFailed,
    TestCaseStarted,
    TestCaseCompleted,
    TestCaseFailed,
    TestCaseSkipped,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestExecutionEvent {
    pub event_id: String,
    pub timestamp: DateTime<Utc>,
    pub test_case_id: String,
    pub request_id: String,
    pub execution_type: RequestExecutionType,
    pub duration: Option<Duration>,
    pub status_code: Option<u16>,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RequestExecutionType {
    Started,
    Completed,
    Failed,
    Timeout,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssertionResultEvent {
    pub event_id: String,
    pub timestamp: DateTime<Utc>,
    pub test_case_id: String,
    pub assertion_type: String,
    pub success: bool,
    pub expected_value: Option<serde_json::Value>,
    pub actual_value: Option<serde_json::Value>,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginLifecycleEvent {
    pub event_id: String,
    pub timestamp: DateTime<Utc>,
    pub plugin_name: String,
    pub plugin_version: String,
    pub lifecycle_type: PluginLifecycleType,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PluginLifecycleType {
    Loading,
    Loaded,
    Initializing,
    Initialized,
    Unloading,
    Unloaded,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemMetricsEvent {
    pub event_id: String,
    pub timestamp: DateTime<Utc>,
    pub cpu_usage: f64,
    pub memory_usage: u64,
    pub active_connections: u32,
    pub request_rate: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorEvent {
    pub event_id: String,
    pub timestamp: DateTime<Utc>,
    pub error_type: String,
    pub error_message: String,
    pub context: HashMap<String, String>,
    pub stack_trace: Option<String>,
}

/// Event metadata for filtering and routing
#[derive(Debug, Clone)]
pub struct EventMetadata {
    pub event_id: String,
    pub timestamp: DateTime<Utc>,
    pub event_type: String,
    pub source: String,
    pub tags: Vec<String>,
}

impl Event {
    pub fn metadata(&self) -> EventMetadata {
        match self {
            Event::TestLifecycle(e) => EventMetadata {
                event_id: e.event_id.clone(),
                timestamp: e.timestamp,
                event_type: "test_lifecycle".to_string(),
                source: "test_engine".to_string(),
                tags: vec!["test".to_string(), "lifecycle".to_string()],
            },
            Event::RequestExecution(e) => EventMetadata {
                event_id: e.event_id.clone(),
                timestamp: e.timestamp,
                event_type: "request_execution".to_string(),
                source: "protocol_plugin".to_string(),
                tags: vec!["request".to_string(), "execution".to_string()],
            },
            Event::AssertionResult(e) => EventMetadata {
                event_id: e.event_id.clone(),
                timestamp: e.timestamp,
                event_type: "assertion_result".to_string(),
                source: "assertion_engine".to_string(),
                tags: vec!["assertion".to_string(), "validation".to_string()],
            },
            Event::PluginLifecycle(e) => EventMetadata {
                event_id: e.event_id.clone(),
                timestamp: e.timestamp,
                event_type: "plugin_lifecycle".to_string(),
                source: "plugin_manager".to_string(),
                tags: vec!["plugin".to_string(), "lifecycle".to_string()],
            },
            Event::SystemMetrics(e) => EventMetadata {
                event_id: e.event_id.clone(),
                timestamp: e.timestamp,
                event_type: "system_metrics".to_string(),
                source: "metrics_collector".to_string(),
                tags: vec!["metrics".to_string(), "system".to_string()],
            },
            Event::Error(e) => EventMetadata {
                event_id: e.event_id.clone(),
                timestamp: e.timestamp,
                event_type: "error".to_string(),
                source: "error_handler".to_string(),
                tags: vec!["error".to_string(), "system".to_string()],
            },
        }
    }
}
/// Event subscription handle for managing subscriptions
pub struct EventSubscription {
    pub subscription_id: String,
    pub event_filter: EventFilter,
    receiver: broadcast::Receiver<Event>,
}

impl EventSubscription {
    pub async fn recv(&mut self) -> Result<Event, EventError> {
        loop {
            match self.receiver.recv().await {
                Ok(event) => {
                    if self.event_filter.matches(&event) {
                        return Ok(event);
                    }
                    // Continue loop if event doesn't match filter
                }
                Err(broadcast::error::RecvError::Closed) => {
                    return Err(EventError::ChannelClosed);
                }
                Err(broadcast::error::RecvError::Lagged(skipped)) => {
                    // Log warning about skipped events but continue
                    eprintln!("Warning: Skipped {} events due to slow consumer", skipped);
                }
            }
        }
    }
}

/// Event filter for subscription filtering
#[derive(Debug, Clone)]
pub struct EventFilter {
    pub event_types: Option<Vec<String>>,
    pub sources: Option<Vec<String>>,
    pub tags: Option<Vec<String>>,
    pub custom_predicate: Option<String>, // JSON path expression for custom filtering
    pub time_range: Option<TimeRange>,
    pub test_suite_ids: Option<Vec<String>>,
    pub test_case_ids: Option<Vec<String>>,
}

#[derive(Debug, Clone)]
pub struct TimeRange {
    pub start: Option<DateTime<Utc>>,
    pub end: Option<DateTime<Utc>>,
}

impl EventFilter {
    pub fn new() -> Self {
        Self {
            event_types: None,
            sources: None,
            tags: None,
            custom_predicate: None,
            time_range: None,
            test_suite_ids: None,
            test_case_ids: None,
        }
    }

    pub fn with_event_types(mut self, types: Vec<String>) -> Self {
        self.event_types = Some(types);
        self
    }

    pub fn with_sources(mut self, sources: Vec<String>) -> Self {
        self.sources = Some(sources);
        self
    }

    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.tags = Some(tags);
        self
    }

    pub fn with_custom_predicate(mut self, predicate: String) -> Self {
        self.custom_predicate = Some(predicate);
        self
    }

    pub fn with_time_range(mut self, start: Option<DateTime<Utc>>, end: Option<DateTime<Utc>>) -> Self {
        self.time_range = Some(TimeRange { start, end });
        self
    }

    pub fn with_test_suite_ids(mut self, suite_ids: Vec<String>) -> Self {
        self.test_suite_ids = Some(suite_ids);
        self
    }

    pub fn with_test_case_ids(mut self, case_ids: Vec<String>) -> Self {
        self.test_case_ids = Some(case_ids);
        self
    }

    pub fn matches(&self, event: &Event) -> bool {
        let metadata = event.metadata();

        // Check event type filter
        if let Some(ref types) = self.event_types {
            if !types.contains(&metadata.event_type) {
                return false;
            }
        }

        // Check source filter
        if let Some(ref sources) = self.sources {
            if !sources.contains(&metadata.source) {
                return false;
            }
        }

        // Check tags filter (any tag must match)
        if let Some(ref filter_tags) = self.tags {
            if !filter_tags.iter().any(|tag| metadata.tags.contains(tag)) {
                return false;
            }
        }

        // Check time range filter
        if let Some(ref time_range) = self.time_range {
            if let Some(start) = time_range.start {
                if metadata.timestamp < start {
                    return false;
                }
            }
            if let Some(end) = time_range.end {
                if metadata.timestamp > end {
                    return false;
                }
            }
        }

        // Check test suite ID filter
        if let Some(ref suite_ids) = self.test_suite_ids {
            match event {
                Event::TestLifecycle(test_event) => {
                    if !suite_ids.contains(&test_event.test_suite_id) {
                        return false;
                    }
                }
                Event::RequestExecution(req_event) => {
                    // For request events, we need to check if the test case belongs to any of the filtered suites
                    // This is a simplified check - in a real implementation, you might need to maintain
                    // a mapping of test cases to test suites
                    if !suite_ids.iter().any(|suite_id| req_event.test_case_id.starts_with(suite_id)) {
                        return false;
                    }
                }
                Event::AssertionResult(assertion_event) => {
                    // Similar logic for assertion events
                    if !suite_ids.iter().any(|suite_id| assertion_event.test_case_id.starts_with(suite_id)) {
                        return false;
                    }
                }
                _ => {
                    // For other event types, we don't filter by test suite
                }
            }
        }

        // Check test case ID filter
        if let Some(ref case_ids) = self.test_case_ids {
            match event {
                Event::TestLifecycle(test_event) => {
                    if let Some(ref case_id) = test_event.test_case_id {
                        if !case_ids.contains(case_id) {
                            return false;
                        }
                    } else {
                        // Suite-level events don't have case IDs, so they don't match case ID filters
                        return false;
                    }
                }
                Event::RequestExecution(req_event) => {
                    if !case_ids.contains(&req_event.test_case_id) {
                        return false;
                    }
                }
                Event::AssertionResult(assertion_event) => {
                    if !case_ids.contains(&assertion_event.test_case_id) {
                        return false;
                    }
                }
                _ => {
                    // For other event types, we don't filter by test case
                }
            }
        }

        true
    }
}

/// Core EventBus implementation
pub struct EventBus {
    sender: broadcast::Sender<Event>,
    subscriptions: Arc<tokio::sync::RwLock<HashMap<String, EventFilter>>>,
    persistence: Option<Arc<dyn crate::event_persistence::EventPersistence>>,
}

impl EventBus {
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        
        Self {
            sender,
            subscriptions: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
            persistence: None,
        }
    }

    pub fn with_persistence(mut self, persistence: Arc<dyn crate::event_persistence::EventPersistence>) -> Self {
        self.persistence = Some(persistence);
        self
    }

    /// Publish an event to all subscribers
    pub async fn publish(&self, event: Event) -> Result<(), EventError> {
        // Persist event if persistence is configured
        if let Some(ref persistence) = self.persistence {
            persistence.store_event(&event).await?;
        }

        // Publish to broadcast channel
        match self.sender.send(event) {
            Ok(_) => Ok(()),
            Err(broadcast::error::SendError(_)) => {
                // No active receivers, which is fine
                Ok(())
            }
        }
    }

    /// Subscribe to events with optional filtering
    pub async fn subscribe(&self, filter: EventFilter) -> Result<EventSubscription, EventError> {
        let subscription_id = Uuid::new_v4().to_string();
        let receiver = self.sender.subscribe();

        // Store subscription for management
        {
            let mut subscriptions = self.subscriptions.write().await;
            subscriptions.insert(subscription_id.clone(), filter.clone());
        }

        Ok(EventSubscription {
            subscription_id,
            event_filter: filter,
            receiver,
        })
    }

    /// Unsubscribe from events
    pub async fn unsubscribe(&self, subscription_id: &str) -> Result<(), EventError> {
        let mut subscriptions = self.subscriptions.write().await;
        subscriptions.remove(subscription_id);
        Ok(())
    }

    /// Get current subscriber count
    pub fn subscriber_count(&self) -> usize {
        self.sender.receiver_count()
    }

    /// Create a test lifecycle event
    pub fn create_test_lifecycle_event(
        test_suite_id: String,
        test_case_id: Option<String>,
        lifecycle_type: TestLifecycleType,
    ) -> Event {
        Event::TestLifecycle(TestLifecycleEvent {
            event_id: Uuid::new_v4().to_string(),
            timestamp: Utc::now(),
            test_suite_id,
            test_case_id,
            lifecycle_type,
            metadata: HashMap::new(),
        })
    }

    /// Create a request execution event
    pub fn create_request_execution_event(
        test_case_id: String,
        request_id: String,
        execution_type: RequestExecutionType,
        duration: Option<Duration>,
        status_code: Option<u16>,
        error_message: Option<String>,
    ) -> Event {
        Event::RequestExecution(RequestExecutionEvent {
            event_id: Uuid::new_v4().to_string(),
            timestamp: Utc::now(),
            test_case_id,
            request_id,
            execution_type,
            duration,
            status_code,
            error_message,
        })
    }

    /// Create an assertion result event
    pub fn create_assertion_result_event(
        test_case_id: String,
        assertion_type: String,
        success: bool,
        expected_value: Option<serde_json::Value>,
        actual_value: Option<serde_json::Value>,
        error_message: Option<String>,
    ) -> Event {
        Event::AssertionResult(AssertionResultEvent {
            event_id: Uuid::new_v4().to_string(),
            timestamp: Utc::now(),
            test_case_id,
            assertion_type,
            success,
            expected_value,
            actual_value,
            error_message,
        })
    }
}



/// Event-related errors
#[derive(Debug, thiserror::Error)]
pub enum EventError {
    #[error("Event channel closed")]
    ChannelClosed,
    #[error("Persistence error: {0}")]
    PersistenceError(String),
    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),
    #[error("Filter error: {0}")]
    FilterError(String),
}