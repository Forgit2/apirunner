use crate::event::{Event, EventBus, EventError};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

/// Transaction state for event processing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TransactionState {
    Active,
    Committed,
    RolledBack,
    Failed,
}

/// Event transaction for atomic event processing
#[derive(Debug, Clone)]
pub struct EventTransaction {
    pub transaction_id: String,
    pub started_at: DateTime<Utc>,
    pub state: TransactionState,
    pub events: Vec<Event>,
    pub rollback_events: Vec<Event>,
    pub metadata: HashMap<String, String>,
}

impl EventTransaction {
    pub fn new() -> Self {
        Self {
            transaction_id: Uuid::new_v4().to_string(),
            started_at: Utc::now(),
            state: TransactionState::Active,
            events: Vec::new(),
            rollback_events: Vec::new(),
            metadata: HashMap::new(),
        }
    }

    pub fn with_metadata(mut self, key: String, value: String) -> Self {
        self.metadata.insert(key, value);
        self
    }

    pub fn add_event(&mut self, event: Event) {
        if matches!(self.state, TransactionState::Active) {
            self.events.push(event);
        }
    }

    pub fn add_rollback_event(&mut self, event: Event) {
        if matches!(self.state, TransactionState::Active) {
            self.rollback_events.push(event);
        }
    }

    pub fn is_active(&self) -> bool {
        matches!(self.state, TransactionState::Active)
    }
}

/// Event handler trait for transactional processing
#[async_trait]
pub trait TransactionalEventHandler: Send + Sync {
    async fn handle_event(&self, event: &Event, transaction: &mut EventTransaction) -> Result<(), EventError>;
    async fn rollback(&self, transaction: &EventTransaction) -> Result<(), EventError>;
    fn handler_name(&self) -> &str;
}

/// Transaction manager for coordinating event transactions
pub struct EventTransactionManager {
    event_bus: Arc<EventBus>,
    active_transactions: Arc<RwLock<HashMap<String, EventTransaction>>>,
    handlers: Arc<RwLock<Vec<Arc<dyn TransactionalEventHandler>>>>,
    recovery_handlers: Arc<RwLock<Vec<Arc<dyn TransactionRecoveryHandler>>>>,
}

impl EventTransactionManager {
    pub fn new(event_bus: Arc<EventBus>) -> Self {
        Self {
            event_bus,
            active_transactions: Arc::new(RwLock::new(HashMap::new())),
            handlers: Arc::new(RwLock::new(Vec::new())),
            recovery_handlers: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Register a transactional event handler
    pub async fn register_handler(&self, handler: Arc<dyn TransactionalEventHandler>) {
        let mut handlers = self.handlers.write().await;
        handlers.push(handler);
    }

    /// Register a transaction recovery handler
    pub async fn register_recovery_handler(&self, handler: Arc<dyn TransactionRecoveryHandler>) {
        let mut recovery_handlers = self.recovery_handlers.write().await;
        recovery_handlers.push(handler);
    }

    /// Begin a new transaction
    pub async fn begin_transaction(&self) -> Result<String, EventError> {
        let transaction = EventTransaction::new();
        let transaction_id = transaction.transaction_id.clone();

        let mut transactions = self.active_transactions.write().await;
        transactions.insert(transaction_id.clone(), transaction);

        Ok(transaction_id)
    }

    /// Process an event within a transaction
    pub async fn process_event_transactionally(
        &self,
        transaction_id: &str,
        event: Event,
    ) -> Result<(), EventError> {
        let mut transactions = self.active_transactions.write().await;
        let transaction = transactions
            .get_mut(transaction_id)
            .ok_or_else(|| EventError::FilterError(format!("Transaction {} not found", transaction_id)))?;

        if !transaction.is_active() {
            return Err(EventError::FilterError(format!(
                "Transaction {} is not active",
                transaction_id
            )));
        }

        // Add event to transaction
        transaction.add_event(event.clone());

        // Process event with all handlers
        let handlers = self.handlers.read().await;
        for handler in handlers.iter() {
            if let Err(e) = handler.handle_event(&event, transaction).await {
                // Mark transaction as failed and initiate rollback
                transaction.state = TransactionState::Failed;
                drop(handlers); // Release lock before rollback
                drop(transactions); // Release lock before rollback
                
                self.rollback_transaction(transaction_id).await?;
                return Err(e);
            }
        }

        Ok(())
    }

    /// Commit a transaction
    pub async fn commit_transaction(&self, transaction_id: &str) -> Result<(), EventError> {
        let mut transactions = self.active_transactions.write().await;
        let transaction = transactions
            .get_mut(transaction_id)
            .ok_or_else(|| EventError::FilterError(format!("Transaction {} not found", transaction_id)))?;

        if !transaction.is_active() {
            return Err(EventError::FilterError(format!(
                "Transaction {} is not active",
                transaction_id
            )));
        }

        // Publish all events in the transaction
        for event in &transaction.events {
            self.event_bus.publish(event.clone()).await?;
        }

        // Mark transaction as committed
        transaction.state = TransactionState::Committed;

        // Remove from active transactions
        transactions.remove(transaction_id);

        Ok(())
    }

    /// Rollback a transaction
    pub async fn rollback_transaction(&self, transaction_id: &str) -> Result<(), EventError> {
        let mut transactions = self.active_transactions.write().await;
        let transaction = transactions
            .get_mut(transaction_id)
            .ok_or_else(|| EventError::FilterError(format!("Transaction {} not found", transaction_id)))?;

        // Execute rollback with all handlers
        let handlers = self.handlers.read().await;
        let mut rollback_errors = Vec::new();

        for handler in handlers.iter() {
            if let Err(e) = handler.rollback(transaction).await {
                rollback_errors.push(format!("{}: {}", handler.handler_name(), e));
            }
        }

        // Publish rollback events
        for event in &transaction.rollback_events {
            if let Err(e) = self.event_bus.publish(event.clone()).await {
                rollback_errors.push(format!("Failed to publish rollback event: {}", e));
            }
        }

        // Mark transaction as rolled back
        transaction.state = TransactionState::RolledBack;

        // Execute recovery handlers if there were rollback errors
        if !rollback_errors.is_empty() {
            let recovery_handlers = self.recovery_handlers.read().await;
            for recovery_handler in recovery_handlers.iter() {
                if let Err(e) = recovery_handler.handle_rollback_failure(transaction, &rollback_errors).await {
                    eprintln!("Recovery handler failed: {}", e);
                }
            }
        }

        // Remove from active transactions
        transactions.remove(transaction_id);

        if !rollback_errors.is_empty() {
            return Err(EventError::PersistenceError(format!(
                "Rollback completed with errors: {}",
                rollback_errors.join(", ")
            )));
        }

        Ok(())
    }

    /// Get active transaction count
    pub async fn active_transaction_count(&self) -> usize {
        let transactions = self.active_transactions.read().await;
        transactions.len()
    }

    /// Clean up stale transactions (older than timeout)
    pub async fn cleanup_stale_transactions(&self, timeout_minutes: i64) -> Result<usize, EventError> {
        let cutoff_time = Utc::now() - chrono::Duration::minutes(timeout_minutes);
        let mut transactions = self.active_transactions.write().await;
        let mut stale_transaction_ids = Vec::new();

        // Find stale transactions
        for (id, transaction) in transactions.iter() {
            if transaction.started_at < cutoff_time && transaction.is_active() {
                stale_transaction_ids.push(id.clone());
            }
        }

        let stale_count = stale_transaction_ids.len();

        // Rollback stale transactions
        for transaction_id in stale_transaction_ids {
            if let Some(mut transaction) = transactions.remove(&transaction_id) {
                transaction.state = TransactionState::Failed;
                
                // Execute rollback for stale transaction
                let handlers = self.handlers.read().await;
                for handler in handlers.iter() {
                    if let Err(e) = handler.rollback(&transaction).await {
                        eprintln!("Failed to rollback stale transaction {}: {}", transaction_id, e);
                    }
                }
            }
        }

        Ok(stale_count)
    }
}

/// Recovery handler for transaction failures
#[async_trait]
pub trait TransactionRecoveryHandler: Send + Sync {
    async fn handle_rollback_failure(
        &self,
        transaction: &EventTransaction,
        errors: &[String],
    ) -> Result<(), EventError>;
    fn recovery_handler_name(&self) -> &str;
}

/// Default recovery handler that logs errors
pub struct LoggingRecoveryHandler;

#[async_trait]
impl TransactionRecoveryHandler for LoggingRecoveryHandler {
    async fn handle_rollback_failure(
        &self,
        transaction: &EventTransaction,
        errors: &[String],
    ) -> Result<(), EventError> {
        eprintln!(
            "Transaction {} rollback failed with errors: {}",
            transaction.transaction_id,
            errors.join(", ")
        );
        
        // In a real implementation, this might:
        // - Send alerts to monitoring systems
        // - Write to error logs
        // - Trigger manual intervention workflows
        
        Ok(())
    }

    fn recovery_handler_name(&self) -> &str {
        "logging_recovery_handler"
    }
}

/// Example transactional event handler for test execution
pub struct TestExecutionTransactionHandler {
    name: String,
}

impl TestExecutionTransactionHandler {
    pub fn new(name: String) -> Self {
        Self { name }
    }
}

#[async_trait]
impl TransactionalEventHandler for TestExecutionTransactionHandler {
    async fn handle_event(&self, event: &Event, transaction: &mut EventTransaction) -> Result<(), EventError> {
        match event {
            Event::TestLifecycle(test_event) => {
                // Add rollback event for test lifecycle changes
                let rollback_event = Event::TestLifecycle(crate::event::TestLifecycleEvent {
                    event_id: Uuid::new_v4().to_string(),
                    timestamp: Utc::now(),
                    test_suite_id: test_event.test_suite_id.clone(),
                    test_case_id: test_event.test_case_id.clone(),
                    lifecycle_type: match test_event.lifecycle_type {
                        crate::event::TestLifecycleType::SuiteStarted => crate::event::TestLifecycleType::SuiteFailed,
                        crate::event::TestLifecycleType::TestCaseStarted => crate::event::TestLifecycleType::TestCaseFailed,
                        _ => return Ok(()), // No rollback needed for completion events
                    },
                    metadata: HashMap::from([
                        ("rollback".to_string(), "true".to_string()),
                        ("original_event_id".to_string(), test_event.event_id.clone()),
                    ]),
                });
                
                transaction.add_rollback_event(rollback_event);
            }
            Event::RequestExecution(req_event) => {
                // Validate request execution events
                if req_event.test_case_id.is_empty() {
                    return Err(EventError::FilterError("Test case ID cannot be empty".to_string()));
                }
            }
            _ => {
                // Handle other event types as needed
            }
        }

        Ok(())
    }

    async fn rollback(&self, transaction: &EventTransaction) -> Result<(), EventError> {
        // Perform any cleanup specific to test execution
        println!(
            "Rolling back test execution transaction {} with {} events",
            transaction.transaction_id,
            transaction.events.len()
        );
        
        // In a real implementation, this might:
        // - Clean up test resources
        // - Reset test state
        // - Cancel running requests
        
        Ok(())
    }

    fn handler_name(&self) -> &str {
        &self.name
    }
}