# Event System

The API Test Runner uses an event-driven architecture to enable real-time monitoring, plugin communication, and system integration. The event system provides asynchronous, decoupled communication between components.

## Event Bus

The Event Bus is the central component that manages event publishing, subscription, and routing.

```rust
pub struct EventBus {
    channels: HashMap<String, broadcast::Sender<Event>>,
    subscribers: Arc<Mutex<HashMap<String, Vec<EventSubscriber>>>>,
    event_store: Option<Arc<dyn EventStore>>,
    filters: Vec<Box<dyn EventFilter>>,
}

impl EventBus {
    /// Create a new event bus
    pub fn new() -> Self {
        Self {
            channels: HashMap::new(),
            subscribers: Arc::new(Mutex::new(HashMap::new())),
            event_store: None,
            filters: Vec::new(),
        }
    }
    
    /// Create event bus with persistent storage
    pub fn with_storage(event_store: Arc<dyn EventStore>) -> Self {
        Self {
            channels: HashMap::new(),
            subscribers: Arc::new(Mutex::new(HashMap::new())),
            event_store: Some(event_store),
            filters: Vec::new(),
        }
    }
    
    /// Publish an event to all subscribers
    pub async fn publish(&self, event: Event) -> Result<(), EventError> {
        // Apply filters
        let filtered_event = self.apply_filters(event)?;
        
        // Store event if persistence is enabled
        if let Some(store) = &self.event_store {
            store.store_event(&filtered_event).await?;
        }
        
        // Get or create channel for event type
        let event_type = filtered_event.event_type.clone();
        let sender = self.get_or_create_channel(&event_type);
        
        // Publish to channel
        if let Err(broadcast::error::SendError(_)) = sender.send(filtered_event.clone()) {
            // No active receivers, which is fine
        }
        
        // Notify direct subscribers
        if let Ok(subscribers) = self.subscribers.lock() {
            if let Some(event_subscribers) = subscribers.get(&event_type) {
                for subscriber in event_subscribers {
                    if let Err(e) = subscriber.handle_event(&filtered_event).await {
                        eprintln!("Error handling event in subscriber: {}", e);
                    }
                }
            }
        }
        
        Ok(())
    }
    
    /// Subscribe to events of a specific type
    pub async fn subscribe(&self, event_type: &str) -> broadcast::Receiver<Event> {
        let sender = self.get_or_create_channel(event_type);
        sender.subscribe()
    }
    
    /// Subscribe with a callback handler
    pub async fn subscribe_with_handler<F>(&self, event_type: &str, handler: F) -> Result<(), EventError>
    where
        F: Fn(&Event) -> Result<(), EventError> + Send + Sync + 'static,
    {
        let subscriber = CallbackSubscriber::new(handler);
        
        if let Ok(mut subscribers) = self.subscribers.lock() {
            subscribers
                .entry(event_type.to_string())
                .or_insert_with(Vec::new)
                .push(Box::new(subscriber));
        }
        
        Ok(())
    }
    
    /// Add an event filter
    pub fn add_filter(&mut self, filter: Box<dyn EventFilter>) {
        self.filters.push(filter);
    }
    
    /// Get or create a broadcast channel for an event type
    fn get_or_create_channel(&self, event_type: &str) -> &broadcast::Sender<Event> {
        self.channels
            .entry(event_type.to_string())
            .or_insert_with(|| broadcast::channel(1000).0)
    }
    
    /// Apply all filters to an event
    fn apply_filters(&self, mut event: Event) -> Result<Event, EventError> {
        for filter in &self.filters {
            event = filter.filter(event)?;
        }
        Ok(event)
    }
    
    /// Get event history from storage
    pub async fn get_event_history(
        &self,
        query: &EventQuery,
    ) -> Result<Vec<Event>, EventError> {
        match &self.event_store {
            Some(store) => store.query_events(query).await,
            None => Ok(Vec::new()),
        }
    }
    
    /// Replay events from storage
    pub async fn replay_events(
        &self,
        query: &EventQuery,
        subscriber: Box<dyn EventSubscriber>,
    ) -> Result<(), EventError> {
        let events = self.get_event_history(query).await?;
        
        for event in events {
            subscriber.handle_event(&event).await?;
        }
        
        Ok(())
    }
}
```

## Event Types

The system defines various event types for different aspects of test execution and system operation.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    /// Unique event identifier
    pub id: String,
    /// Event type
    pub event_type: String,
    /// Event timestamp
    pub timestamp: DateTime<Utc>,
    /// Event source (component that generated the event)
    pub source: String,
    /// Event payload
    pub payload: EventPayload,
    /// Event metadata
    pub metadata: HashMap<String, String>,
    /// Event correlation ID for tracing
    pub correlation_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EventPayload {
    /// Test lifecycle events
    TestLifecycle(TestLifecycleEvent),
    /// Request execution events
    RequestExecution(RequestExecutionEvent),
    /// Assertion events
    Assertion(AssertionEvent),
    /// System events
    System(SystemEvent),
    /// Plugin events
    Plugin(PluginEvent),
    /// Configuration events
    Configuration(ConfigurationEvent),
    /// Metrics events
    Metrics(MetricsEvent),
    /// Custom event payload
    Custom(serde_json::Value),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TestLifecycleEvent {
    /// Test suite started
    SuiteStarted {
        suite_name: String,
        test_count: usize,
        environment: String,
    },
    /// Test suite completed
    SuiteCompleted {
        suite_name: String,
        summary: ExecutionSummary,
        duration: Duration,
    },
    /// Individual test started
    TestStarted {
        test_id: String,
        test_name: String,
        test_case: TestCase,
    },
    /// Individual test completed
    TestCompleted {
        test_id: String,
        test_name: String,
        result: TestResult,
        duration: Duration,
    },
    /// Test skipped
    TestSkipped {
        test_id: String,
        test_name: String,
        reason: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RequestExecutionEvent {
    /// Request about to be sent
    RequestStarted {
        request_id: String,
        method: String,
        url: String,
        headers: HashMap<String, String>,
    },
    /// Request completed successfully
    RequestCompleted {
        request_id: String,
        response: ProtocolResponse,
        duration: Duration,
    },
    /// Request failed
    RequestFailed {
        request_id: String,
        error: String,
        duration: Duration,
    },
    /// Request retried
    RequestRetried {
        request_id: String,
        attempt: u32,
        reason: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AssertionEvent {
    /// Assertion started
    AssertionStarted {
        assertion_id: String,
        assertion_type: String,
        test_id: String,
    },
    /// Assertion passed
    AssertionPassed {
        assertion_id: String,
        assertion_type: String,
        test_id: String,
        message: String,
    },
    /// Assertion failed
    AssertionFailed {
        assertion_id: String,
        assertion_type: String,
        test_id: String,
        message: String,
        expected: Option<serde_json::Value>,
        actual: Option<serde_json::Value>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SystemEvent {
    /// System startup
    SystemStarted {
        version: String,
        configuration: String,
    },
    /// System shutdown
    SystemShutdown {
        reason: String,
    },
    /// Resource usage update
    ResourceUsage {
        cpu_usage: f64,
        memory_usage: u64,
        network_usage: NetworkUsage,
    },
    /// Error occurred
    Error {
        component: String,
        error: String,
        severity: ErrorSeverity,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PluginEvent {
    /// Plugin loaded
    PluginLoaded {
        plugin_name: String,
        plugin_version: String,
        plugin_type: String,
    },
    /// Plugin initialized
    PluginInitialized {
        plugin_name: String,
    },
    /// Plugin error
    PluginError {
        plugin_name: String,
        error: String,
    },
    /// Plugin unloaded
    PluginUnloaded {
        plugin_name: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConfigurationEvent {
    /// Configuration loaded
    ConfigurationLoaded {
        source: String,
        environment: String,
    },
    /// Configuration changed
    ConfigurationChanged {
        source: String,
        changes: Vec<ConfigChange>,
    },
    /// Configuration error
    ConfigurationError {
        source: String,
        error: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MetricsEvent {
    /// Metric recorded
    MetricRecorded {
        metric_name: String,
        metric_value: f64,
        metric_type: String,
        tags: HashMap<String, String>,
    },
    /// Metrics exported
    MetricsExported {
        exporter: String,
        metric_count: usize,
    },
    /// Alert triggered
    AlertTriggered {
        alert_name: String,
        condition: String,
        value: f64,
        threshold: f64,
    },
}
```

## Event Subscribers

Event subscribers handle events asynchronously and can perform various actions based on event data.

```rust
#[async_trait]
pub trait EventSubscriber: Send + Sync {
    /// Handle an event
    async fn handle_event(&self, event: &Event) -> Result<(), EventError>;
    
    /// Get the subscriber name
    fn name(&self) -> &str;
    
    /// Get the event types this subscriber is interested in
    fn interested_events(&self) -> Vec<String>;
}

/// Callback-based event subscriber
pub struct CallbackSubscriber<F>
where
    F: Fn(&Event) -> Result<(), EventError> + Send + Sync,
{
    name: String,
    callback: F,
    interested_events: Vec<String>,
}

impl<F> CallbackSubscriber<F>
where
    F: Fn(&Event) -> Result<(), EventError> + Send + Sync,
{
    pub fn new(callback: F) -> Self {
        Self {
            name: "callback_subscriber".to_string(),
            callback,
            interested_events: vec!["*".to_string()], // Subscribe to all events
        }
    }
    
    pub fn with_name(mut self, name: String) -> Self {
        self.name = name;
        self
    }
    
    pub fn with_events(mut self, events: Vec<String>) -> Self {
        self.interested_events = events;
        self
    }
}

#[async_trait]
impl<F> EventSubscriber for CallbackSubscriber<F>
where
    F: Fn(&Event) -> Result<(), EventError> + Send + Sync,
{
    async fn handle_event(&self, event: &Event) -> Result<(), EventError> {
        (self.callback)(event)
    }
    
    fn name(&self) -> &str {
        &self.name
    }
    
    fn interested_events(&self) -> Vec<String> {
        self.interested_events.clone()
    }
}

/// Metrics collection subscriber
pub struct MetricsSubscriber {
    metrics_collector: Arc<MetricsCollector>,
}

impl MetricsSubscriber {
    pub fn new(metrics_collector: Arc<MetricsCollector>) -> Self {
        Self { metrics_collector }
    }
}

#[async_trait]
impl EventSubscriber for MetricsSubscriber {
    async fn handle_event(&self, event: &Event) -> Result<(), EventError> {
        match &event.payload {
            EventPayload::TestLifecycle(TestLifecycleEvent::TestCompleted { result, duration, .. }) => {
                self.metrics_collector.increment_counter("tests_completed", 1);
                self.metrics_collector.record_timer("test_duration", *duration);
                
                match result.status {
                    TestStatus::Passed => self.metrics_collector.increment_counter("tests_passed", 1),
                    TestStatus::Failed => self.metrics_collector.increment_counter("tests_failed", 1),
                    TestStatus::Skipped => self.metrics_collector.increment_counter("tests_skipped", 1),
                    TestStatus::Error => self.metrics_collector.increment_counter("tests_error", 1),
                }
            }
            EventPayload::RequestExecution(RequestExecutionEvent::RequestCompleted { duration, .. }) => {
                self.metrics_collector.record_timer("request_duration", *duration);
                self.metrics_collector.increment_counter("requests_completed", 1);
            }
            EventPayload::RequestExecution(RequestExecutionEvent::RequestFailed { .. }) => {
                self.metrics_collector.increment_counter("requests_failed", 1);
            }
            _ => {}
        }
        
        Ok(())
    }
    
    fn name(&self) -> &str {
        "metrics_subscriber"
    }
    
    fn interested_events(&self) -> Vec<String> {
        vec![
            "test_lifecycle".to_string(),
            "request_execution".to_string(),
        ]
    }
}

/// Logging subscriber
pub struct LoggingSubscriber {
    logger: slog::Logger,
    log_level: LogLevel,
}

#[derive(Debug, Clone)]
pub enum LogLevel {
    Debug,
    Info,
    Warn,
    Error,
}

impl LoggingSubscriber {
    pub fn new(logger: slog::Logger, log_level: LogLevel) -> Self {
        Self { logger, log_level }
    }
}

#[async_trait]
impl EventSubscriber for LoggingSubscriber {
    async fn handle_event(&self, event: &Event) -> Result<(), EventError> {
        let log_message = format!(
            "[{}] {} - {} ({})",
            event.timestamp.format("%Y-%m-%d %H:%M:%S%.3f"),
            event.source,
            event.event_type,
            event.id
        );
        
        match &event.payload {
            EventPayload::System(SystemEvent::Error { severity, .. }) => {
                match severity {
                    ErrorSeverity::Critical | ErrorSeverity::High => {
                        slog::error!(self.logger, "{}", log_message; "event" => ?event);
                    }
                    ErrorSeverity::Medium => {
                        slog::warn!(self.logger, "{}", log_message; "event" => ?event);
                    }
                    ErrorSeverity::Low => {
                        slog::info!(self.logger, "{}", log_message; "event" => ?event);
                    }
                }
            }
            EventPayload::TestLifecycle(TestLifecycleEvent::TestCompleted { result, .. }) => {
                match result.status {
                    TestStatus::Failed | TestStatus::Error => {
                        slog::warn!(self.logger, "{}", log_message; "event" => ?event);
                    }
                    _ => {
                        slog::info!(self.logger, "{}", log_message; "event" => ?event);
                    }
                }
            }
            _ => {
                match self.log_level {
                    LogLevel::Debug => slog::debug!(self.logger, "{}", log_message; "event" => ?event),
                    LogLevel::Info => slog::info!(self.logger, "{}", log_message; "event" => ?event),
                    _ => {} // Don't log other events at higher levels
                }
            }
        }
        
        Ok(())
    }
    
    fn name(&self) -> &str {
        "logging_subscriber"
    }
    
    fn interested_events(&self) -> Vec<String> {
        vec!["*".to_string()]
    }
}
```

## Event Filters

Event filters allow modification or filtering of events before they are published to subscribers.

```rust
pub trait EventFilter: Send + Sync {
    /// Filter or modify an event
    fn filter(&self, event: Event) -> Result<Event, EventError>;
    
    /// Get the filter name
    fn name(&self) -> &str;
}

/// Filter events by type
pub struct EventTypeFilter {
    allowed_types: HashSet<String>,
}

impl EventTypeFilter {
    pub fn new(allowed_types: Vec<String>) -> Self {
        Self {
            allowed_types: allowed_types.into_iter().collect(),
        }
    }
}

impl EventFilter for EventTypeFilter {
    fn filter(&self, event: Event) -> Result<Event, EventError> {
        if self.allowed_types.contains(&event.event_type) || self.allowed_types.contains("*") {
            Ok(event)
        } else {
            Err(EventError::EventFiltered(format!("Event type '{}' not allowed", event.event_type)))
        }
    }
    
    fn name(&self) -> &str {
        "event_type_filter"
    }
}

/// Filter events by severity
pub struct SeverityFilter {
    min_severity: ErrorSeverity,
}

impl SeverityFilter {
    pub fn new(min_severity: ErrorSeverity) -> Self {
        Self { min_severity }
    }
}

impl EventFilter for SeverityFilter {
    fn filter(&self, event: Event) -> Result<Event, EventError> {
        match &event.payload {
            EventPayload::System(SystemEvent::Error { severity, .. }) => {
                if severity >= &self.min_severity {
                    Ok(event)
                } else {
                    Err(EventError::EventFiltered("Event severity too low".to_string()))
                }
            }
            _ => Ok(event), // Pass through non-error events
        }
    }
    
    fn name(&self) -> &str {
        "severity_filter"
    }
}

/// Add metadata to events
pub struct MetadataEnrichmentFilter {
    metadata: HashMap<String, String>,
}

impl MetadataEnrichmentFilter {
    pub fn new(metadata: HashMap<String, String>) -> Self {
        Self { metadata }
    }
}

impl EventFilter for MetadataEnrichmentFilter {
    fn filter(&self, mut event: Event) -> Result<Event, EventError> {
        // Add additional metadata
        for (key, value) in &self.metadata {
            event.metadata.insert(key.clone(), value.clone());
        }
        Ok(event)
    }
    
    fn name(&self) -> &str {
        "metadata_enrichment_filter"
    }
}
```

## Event Storage

Event storage provides persistence for events, enabling replay and historical analysis.

```rust
#[async_trait]
pub trait EventStore: Send + Sync {
    /// Store an event
    async fn store_event(&self, event: &Event) -> Result<(), EventError>;
    
    /// Query events based on criteria
    async fn query_events(&self, query: &EventQuery) -> Result<Vec<Event>, EventError>;
    
    /// Delete events older than the specified duration
    async fn cleanup_old_events(&self, older_than: Duration) -> Result<usize, EventError>;
    
    /// Get event statistics
    async fn get_statistics(&self) -> Result<EventStatistics, EventError>;
}

#[derive(Debug, Clone)]
pub struct EventQuery {
    /// Event types to include
    pub event_types: Option<Vec<String>>,
    /// Time range
    pub time_range: Option<TimeRange>,
    /// Event source filter
    pub source: Option<String>,
    /// Correlation ID filter
    pub correlation_id: Option<String>,
    /// Maximum number of events to return
    pub limit: Option<usize>,
    /// Offset for pagination
    pub offset: Option<usize>,
    /// Sort order
    pub sort_order: SortOrder,
}

#[derive(Debug, Clone)]
pub struct TimeRange {
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub enum SortOrder {
    Ascending,
    Descending,
}

#[derive(Debug, Clone)]
pub struct EventStatistics {
    pub total_events: u64,
    pub events_by_type: HashMap<String, u64>,
    pub events_by_source: HashMap<String, u64>,
    pub oldest_event: Option<DateTime<Utc>>,
    pub newest_event: Option<DateTime<Utc>>,
}

/// In-memory event store (for testing and development)
pub struct InMemoryEventStore {
    events: Arc<Mutex<Vec<Event>>>,
}

impl InMemoryEventStore {
    pub fn new() -> Self {
        Self {
            events: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

#[async_trait]
impl EventStore for InMemoryEventStore {
    async fn store_event(&self, event: &Event) -> Result<(), EventError> {
        if let Ok(mut events) = self.events.lock() {
            events.push(event.clone());
        }
        Ok(())
    }
    
    async fn query_events(&self, query: &EventQuery) -> Result<Vec<Event>, EventError> {
        let events = self.events.lock().map_err(|_| EventError::StorageError("Lock error".to_string()))?;
        
        let mut filtered_events: Vec<Event> = events
            .iter()
            .filter(|event| {
                // Filter by event type
                if let Some(ref types) = query.event_types {
                    if !types.contains(&event.event_type) {
                        return false;
                    }
                }
                
                // Filter by time range
                if let Some(ref time_range) = query.time_range {
                    if event.timestamp < time_range.start || event.timestamp > time_range.end {
                        return false;
                    }
                }
                
                // Filter by source
                if let Some(ref source) = query.source {
                    if &event.source != source {
                        return false;
                    }
                }
                
                // Filter by correlation ID
                if let Some(ref correlation_id) = query.correlation_id {
                    if event.correlation_id.as_ref() != Some(correlation_id) {
                        return false;
                    }
                }
                
                true
            })
            .cloned()
            .collect();
        
        // Sort events
        match query.sort_order {
            SortOrder::Ascending => filtered_events.sort_by(|a, b| a.timestamp.cmp(&b.timestamp)),
            SortOrder::Descending => filtered_events.sort_by(|a, b| b.timestamp.cmp(&a.timestamp)),
        }
        
        // Apply pagination
        if let Some(offset) = query.offset {
            if offset < filtered_events.len() {
                filtered_events = filtered_events[offset..].to_vec();
            } else {
                filtered_events.clear();
            }
        }
        
        if let Some(limit) = query.limit {
            filtered_events.truncate(limit);
        }
        
        Ok(filtered_events)
    }
    
    async fn cleanup_old_events(&self, older_than: Duration) -> Result<usize, EventError> {
        let cutoff_time = Utc::now() - chrono::Duration::from_std(older_than)
            .map_err(|e| EventError::StorageError(e.to_string()))?;
        
        if let Ok(mut events) = self.events.lock() {
            let original_count = events.len();
            events.retain(|event| event.timestamp > cutoff_time);
            Ok(original_count - events.len())
        } else {
            Err(EventError::StorageError("Lock error".to_string()))
        }
    }
    
    async fn get_statistics(&self) -> Result<EventStatistics, EventError> {
        let events = self.events.lock().map_err(|_| EventError::StorageError("Lock error".to_string()))?;
        
        let mut events_by_type = HashMap::new();
        let mut events_by_source = HashMap::new();
        let mut oldest_event = None;
        let mut newest_event = None;
        
        for event in events.iter() {
            *events_by_type.entry(event.event_type.clone()).or_insert(0) += 1;
            *events_by_source.entry(event.source.clone()).or_insert(0) += 1;
            
            if oldest_event.is_none() || Some(event.timestamp) < oldest_event {
                oldest_event = Some(event.timestamp);
            }
            
            if newest_event.is_none() || Some(event.timestamp) > newest_event {
                newest_event = Some(event.timestamp);
            }
        }
        
        Ok(EventStatistics {
            total_events: events.len() as u64,
            events_by_type,
            events_by_source,
            oldest_event,
            newest_event,
        })
    }
}
```

## Event Transaction Support

Event transactions provide atomicity and rollback capabilities for complex operations.

```rust
pub struct EventTransaction {
    id: String,
    events: Vec<Event>,
    rollback_handlers: Vec<Box<dyn RollbackHandler>>,
    committed: bool,
}

#[async_trait]
pub trait RollbackHandler: Send + Sync {
    /// Execute rollback logic
    async fn rollback(&self) -> Result<(), EventError>;
}

impl EventTransaction {
    /// Create a new transaction
    pub fn new() -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            events: Vec::new(),
            rollback_handlers: Vec::new(),
            committed: false,
        }
    }
    
    /// Add an event to the transaction
    pub fn add_event(&mut self, event: Event) {
        self.events.push(event);
    }
    
    /// Add a rollback handler
    pub fn add_rollback_handler(&mut self, handler: Box<dyn RollbackHandler>) {
        self.rollback_handlers.push(handler);
    }
    
    /// Commit the transaction
    pub async fn commit(&mut self, event_bus: &EventBus) -> Result<(), EventError> {
        if self.committed {
            return Err(EventError::TransactionError("Transaction already committed".to_string()));
        }
        
        // Publish all events
        for event in &self.events {
            event_bus.publish(event.clone()).await?;
        }
        
        self.committed = true;
        Ok(())
    }
    
    /// Rollback the transaction
    pub async fn rollback(&self) -> Result<(), EventError> {
        if self.committed {
            return Err(EventError::TransactionError("Cannot rollback committed transaction".to_string()));
        }
        
        // Execute rollback handlers in reverse order
        for handler in self.rollback_handlers.iter().rev() {
            if let Err(e) = handler.rollback().await {
                eprintln!("Rollback handler failed: {}", e);
            }
        }
        
        Ok(())
    }
}

/// Transaction manager for handling multiple concurrent transactions
pub struct TransactionManager {
    active_transactions: Arc<Mutex<HashMap<String, EventTransaction>>>,
    event_bus: Arc<EventBus>,
}

impl TransactionManager {
    pub fn new(event_bus: Arc<EventBus>) -> Self {
        Self {
            active_transactions: Arc::new(Mutex::new(HashMap::new())),
            event_bus,
        }
    }
    
    /// Begin a new transaction
    pub async fn begin_transaction(&self) -> Result<String, EventError> {
        let transaction = EventTransaction::new();
        let transaction_id = transaction.id.clone();
        
        if let Ok(mut transactions) = self.active_transactions.lock() {
            transactions.insert(transaction_id.clone(), transaction);
        }
        
        Ok(transaction_id)
    }
    
    /// Add event to transaction
    pub async fn add_event_to_transaction(&self, transaction_id: &str, event: Event) -> Result<(), EventError> {
        if let Ok(mut transactions) = self.active_transactions.lock() {
            if let Some(transaction) = transactions.get_mut(transaction_id) {
                transaction.add_event(event);
                Ok(())
            } else {
                Err(EventError::TransactionError(format!("Transaction {} not found", transaction_id)))
            }
        } else {
            Err(EventError::TransactionError("Lock error".to_string()))
        }
    }
    
    /// Commit transaction
    pub async fn commit_transaction(&self, transaction_id: &str) -> Result<(), EventError> {
        let mut transaction = if let Ok(mut transactions) = self.active_transactions.lock() {
            transactions.remove(transaction_id)
                .ok_or_else(|| EventError::TransactionError(format!("Transaction {} not found", transaction_id)))?
        } else {
            return Err(EventError::TransactionError("Lock error".to_string()));
        };
        
        transaction.commit(&self.event_bus).await
    }
    
    /// Rollback transaction
    pub async fn rollback_transaction(&self, transaction_id: &str) -> Result<(), EventError> {
        let transaction = if let Ok(mut transactions) = self.active_transactions.lock() {
            transactions.remove(transaction_id)
                .ok_or_else(|| EventError::TransactionError(format!("Transaction {} not found", transaction_id)))?
        } else {
            return Err(EventError::TransactionError("Lock error".to_string()));
        };
        
        transaction.rollback().await
    }
}
```

## Error Types

```rust
#[derive(Debug, thiserror::Error)]
pub enum EventError {
    #[error("Event publishing failed: {0}")]
    PublishError(String),
    
    #[error("Event subscription failed: {0}")]
    SubscriptionError(String),
    
    #[error("Event storage error: {0}")]
    StorageError(String),
    
    #[error("Event filtered: {0}")]
    EventFiltered(String),
    
    #[error("Transaction error: {0}")]
    TransactionError(String),
    
    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),
    
    #[error("Channel error: {0}")]
    ChannelError(String),
}

#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub enum ErrorSeverity {
    Low,
    Medium,
    High,
    Critical,
}
```

## Usage Examples

### Basic Event Publishing and Subscription

```rust
use api_test_runner::event::{EventBus, Event, EventPayload, TestLifecycleEvent};

// Create event bus
let event_bus = EventBus::new();

// Subscribe to events
let mut receiver = event_bus.subscribe("test_lifecycle").await;

// Publish an event
let event = Event {
    id: Uuid::new_v4().to_string(),
    event_type: "test_lifecycle".to_string(),
    timestamp: Utc::now(),
    source: "test_runner".to_string(),
    payload: EventPayload::TestLifecycle(TestLifecycleEvent::TestStarted {
        test_id: "test_1".to_string(),
        test_name: "Get Users Test".to_string(),
        test_case: test_case.clone(),
    }),
    metadata: HashMap::new(),
    correlation_id: Some("execution_123".to_string()),
};

event_bus.publish(event).await?;

// Receive events
tokio::spawn(async move {
    while let Ok(event) = receiver.recv().await {
        println!("Received event: {:?}", event);
    }
});
```

### Event Subscribers

```rust
// Metrics subscriber
let metrics_collector = Arc::new(MetricsCollector::new());
let metrics_subscriber = MetricsSubscriber::new(metrics_collector);

// Logging subscriber
let logger = slog::Logger::root(slog::Discard, slog::o!());
let logging_subscriber = LoggingSubscriber::new(logger, LogLevel::Info);

// Register subscribers
event_bus.subscribe_with_handler("test_lifecycle", move |event| {
    metrics_subscriber.handle_event(event).await
}).await?;

event_bus.subscribe_with_handler("*", move |event| {
    logging_subscriber.handle_event(event).await
}).await?;
```

### Event Transactions

```rust
let transaction_manager = TransactionManager::new(Arc::new(event_bus));

// Begin transaction
let transaction_id = transaction_manager.begin_transaction().await?;

// Add events to transaction
let event1 = Event { /* ... */ };
let event2 = Event { /* ... */ };

transaction_manager.add_event_to_transaction(&transaction_id, event1).await?;
transaction_manager.add_event_to_transaction(&transaction_id, event2).await?;

// Commit or rollback
if success {
    transaction_manager.commit_transaction(&transaction_id).await?;
} else {
    transaction_manager.rollback_transaction(&transaction_id).await?;
}
```

### Event Storage and Replay

```rust
// Create event bus with storage
let event_store = Arc::new(InMemoryEventStore::new());
let event_bus = EventBus::with_storage(event_store.clone());

// Query historical events
let query = EventQuery {
    event_types: Some(vec!["test_lifecycle".to_string()]),
    time_range: Some(TimeRange {
        start: Utc::now() - chrono::Duration::hours(1),
        end: Utc::now(),
    }),
    source: None,
    correlation_id: None,
    limit: Some(100),
    offset: None,
    sort_order: SortOrder::Descending,
};

let historical_events = event_bus.get_event_history(&query).await?;

// Replay events
let replay_subscriber = Box::new(LoggingSubscriber::new(logger, LogLevel::Info));
event_bus.replay_events(&query, replay_subscriber).await?;
```