use apirunner::event::{
    Event, EventBus, EventFilter, TestLifecycleEvent, TestLifecycleType,
    RequestExecutionEvent, RequestExecutionType,
};
use apirunner::event_persistence::{EventPersistence, InMemoryEventPersistence};
use apirunner::event_transaction::{
    EventTransaction, EventTransactionManager, TransactionalEventHandler, TransactionState,
    TestExecutionTransactionHandler,
};
use async_trait::async_trait;
use chrono::Utc;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::timeout;
use uuid::Uuid;

#[tokio::test]
async fn test_event_bus_creation() {
    let event_bus = EventBus::new(100);
    assert_eq!(event_bus.subscriber_count(), 0);
}

#[tokio::test]
async fn test_event_publishing_and_subscription() {
    let event_bus = EventBus::new(100);
    
    // Create a filter for test lifecycle events
    let filter = EventFilter::new()
        .with_event_types(vec!["test_lifecycle".to_string()]);
    
    // Subscribe to events
    let mut subscription = event_bus.subscribe(filter).await.unwrap();
    
    // Verify subscriber count
    assert_eq!(event_bus.subscriber_count(), 1);
    
    // Create and publish a test lifecycle event
    let event = EventBus::create_test_lifecycle_event(
        "test_suite_1".to_string(),
        Some("test_case_1".to_string()),
        TestLifecycleType::TestCaseStarted,
    );
    
    event_bus.publish(event.clone()).await.unwrap();
    
    // Receive the event
    let received_event = timeout(Duration::from_secs(1), subscription.recv())
        .await
        .unwrap()
        .unwrap();
    
    // Verify the received event
    match received_event {
        Event::TestLifecycle(test_event) => {
            assert_eq!(test_event.test_suite_id, "test_suite_1");
            assert_eq!(test_event.test_case_id, Some("test_case_1".to_string()));
            assert!(matches!(test_event.lifecycle_type, TestLifecycleType::TestCaseStarted));
        }
        _ => panic!("Expected TestLifecycle event"),
    }
}

#[tokio::test]
async fn test_event_filtering() {
    let event_bus = EventBus::new(100);
    
    // Create a filter for only request execution events
    let filter = EventFilter::new()
        .with_event_types(vec!["request_execution".to_string()]);
    
    let mut subscription = event_bus.subscribe(filter).await.unwrap();
    
    // Publish a test lifecycle event (should be filtered out)
    let test_event = EventBus::create_test_lifecycle_event(
        "test_suite_1".to_string(),
        None,
        TestLifecycleType::SuiteStarted,
    );
    event_bus.publish(test_event).await.unwrap();
    
    // Publish a request execution event (should be received)
    let request_event = EventBus::create_request_execution_event(
        "test_case_1".to_string(),
        "request_1".to_string(),
        RequestExecutionType::Started,
        None,
        None,
        None,
    );
    event_bus.publish(request_event).await.unwrap();
    
    // Should only receive the request execution event
    let received_event = timeout(Duration::from_secs(1), subscription.recv())
        .await
        .unwrap()
        .unwrap();
    
    match received_event {
        Event::RequestExecution(req_event) => {
            assert_eq!(req_event.test_case_id, "test_case_1");
            assert_eq!(req_event.request_id, "request_1");
            assert!(matches!(req_event.execution_type, RequestExecutionType::Started));
        }
        _ => panic!("Expected RequestExecution event"),
    }
}

#[tokio::test]
async fn test_event_persistence() {
    let persistence = Arc::new(InMemoryEventPersistence::new());
    let event_bus = EventBus::new(100).with_persistence(persistence.clone());
    
    // Create and publish events
    let event1 = EventBus::create_test_lifecycle_event(
        "suite_1".to_string(),
        Some("case_1".to_string()),
        TestLifecycleType::TestCaseStarted,
    );
    
    let event2 = EventBus::create_request_execution_event(
        "case_1".to_string(),
        "req_1".to_string(),
        RequestExecutionType::Completed,
        Some(Duration::from_millis(500)),
        Some(200),
        None,
    );
    
    event_bus.publish(event1).await.unwrap();
    event_bus.publish(event2).await.unwrap();
    
    // Retrieve all events
    let filter = EventFilter::new();
    let events = persistence.get_events(&filter, None).await.unwrap();
    
    assert_eq!(events.len(), 2);
    
    // Verify events are in reverse chronological order (newest first)
    match &events[0] {
        Event::RequestExecution(_) => {},
        _ => panic!("Expected RequestExecution event first"),
    }
    
    match &events[1] {
        Event::TestLifecycle(_) => {},
        _ => panic!("Expected TestLifecycle event second"),
    }
}

#[tokio::test]
async fn test_event_replay() {
    let persistence = Arc::new(InMemoryEventPersistence::new());
    
    // Store multiple events
    let events = vec![
        EventBus::create_test_lifecycle_event(
            "suite_1".to_string(),
            None,
            TestLifecycleType::SuiteStarted,
        ),
        EventBus::create_test_lifecycle_event(
            "suite_1".to_string(),
            Some("case_1".to_string()),
            TestLifecycleType::TestCaseStarted,
        ),
        EventBus::create_test_lifecycle_event(
            "suite_1".to_string(),
            Some("case_1".to_string()),
            TestLifecycleType::TestCaseCompleted,
        ),
    ];
    
    for event in &events {
        persistence.store_event(event).await.unwrap();
    }
    
    // Replay all events
    let replayed_events = persistence.replay_events(None).await.unwrap();
    assert_eq!(replayed_events.len(), 3);
    
    // Replay from second event
    let second_event_id = events[1].metadata().event_id;
    let partial_replay = persistence.replay_events(Some(second_event_id)).await.unwrap();
    assert_eq!(partial_replay.len(), 2);
}

#[tokio::test]
async fn test_event_transactions() {
    let event_bus = Arc::new(EventBus::new(100));
    let transaction_manager = EventTransactionManager::new(event_bus.clone());
    
    // Register a test handler
    let handler = Arc::new(TestExecutionTransactionHandler::new("test_handler".to_string()));
    transaction_manager.register_handler(handler).await;
    
    // Begin a transaction
    let transaction_id = transaction_manager.begin_transaction().await.unwrap();
    
    // Process events within the transaction
    let event1 = EventBus::create_test_lifecycle_event(
        "suite_1".to_string(),
        Some("case_1".to_string()),
        TestLifecycleType::TestCaseStarted,
    );
    
    let event2 = EventBus::create_request_execution_event(
        "case_1".to_string(),
        "req_1".to_string(),
        RequestExecutionType::Completed,
        Some(Duration::from_millis(200)),
        Some(200),
        None,
    );
    
    transaction_manager
        .process_event_transactionally(&transaction_id, event1)
        .await
        .unwrap();
    
    transaction_manager
        .process_event_transactionally(&transaction_id, event2)
        .await
        .unwrap();
    
    // Commit the transaction
    transaction_manager.commit_transaction(&transaction_id).await.unwrap();
    
    // Verify no active transactions remain
    assert_eq!(transaction_manager.active_transaction_count().await, 0);
}

#[tokio::test]
async fn test_transaction_rollback() {
    let event_bus = Arc::new(EventBus::new(100));
    let transaction_manager = EventTransactionManager::new(event_bus.clone());
    
    // Register a failing handler
    let handler = Arc::new(FailingTransactionHandler);
    transaction_manager.register_handler(handler).await;
    
    // Begin a transaction
    let transaction_id = transaction_manager.begin_transaction().await.unwrap();
    
    // Process an event that will cause the handler to fail
    let event = EventBus::create_test_lifecycle_event(
        "suite_1".to_string(),
        Some("case_1".to_string()),
        TestLifecycleType::TestCaseStarted,
    );
    
    // This should fail and trigger rollback
    let result = transaction_manager
        .process_event_transactionally(&transaction_id, event)
        .await;
    
    assert!(result.is_err());
    
    // Verify no active transactions remain (should be rolled back)
    assert_eq!(transaction_manager.active_transaction_count().await, 0);
}

#[tokio::test]
async fn test_multiple_subscribers() {
    let event_bus = EventBus::new(100);
    
    // Create multiple subscribers with different filters
    let filter1 = EventFilter::new()
        .with_event_types(vec!["test_lifecycle".to_string()]);
    let filter2 = EventFilter::new()
        .with_event_types(vec!["request_execution".to_string()]);
    let filter3 = EventFilter::new(); // No filter - receives all events
    
    let mut sub1 = event_bus.subscribe(filter1).await.unwrap();
    let mut sub2 = event_bus.subscribe(filter2).await.unwrap();
    let mut sub3 = event_bus.subscribe(filter3).await.unwrap();
    
    assert_eq!(event_bus.subscriber_count(), 3);
    
    // Publish different types of events
    let test_event = EventBus::create_test_lifecycle_event(
        "suite_1".to_string(),
        None,
        TestLifecycleType::SuiteStarted,
    );
    
    let request_event = EventBus::create_request_execution_event(
        "case_1".to_string(),
        "req_1".to_string(),
        RequestExecutionType::Started,
        None,
        None,
        None,
    );
    
    event_bus.publish(test_event).await.unwrap();
    event_bus.publish(request_event).await.unwrap();
    
    // Subscriber 1 should only receive test lifecycle event
    let received1 = timeout(Duration::from_secs(1), sub1.recv()).await.unwrap().unwrap();
    assert!(matches!(received1, Event::TestLifecycle(_)));
    
    // Subscriber 2 should only receive request execution event
    let received2 = timeout(Duration::from_secs(1), sub2.recv()).await.unwrap().unwrap();
    assert!(matches!(received2, Event::RequestExecution(_)));
    
    // Subscriber 3 should receive both events
    let received3a = timeout(Duration::from_secs(1), sub3.recv()).await.unwrap().unwrap();
    let received3b = timeout(Duration::from_secs(1), sub3.recv()).await.unwrap().unwrap();
    
    // Verify both events were received (order may vary)
    let mut received_test = false;
    let mut received_request = false;
    
    for event in [received3a, received3b] {
        match event {
            Event::TestLifecycle(_) => received_test = true,
            Event::RequestExecution(_) => received_request = true,
            _ => {}
        }
    }
    
    assert!(received_test && received_request);
}

// Helper struct for testing transaction failures
struct FailingTransactionHandler;

#[async_trait]
impl TransactionalEventHandler for FailingTransactionHandler {
    async fn handle_event(&self, _event: &Event, _transaction: &mut EventTransaction) -> Result<(), apirunner::EventError> {
        Err(apirunner::EventError::FilterError("Simulated failure".to_string()))
    }
    
    async fn rollback(&self, _transaction: &EventTransaction) -> Result<(), apirunner::EventError> {
        Ok(())
    }
    
    fn handler_name(&self) -> &str {
        "failing_handler"
    }
}#[tokio
::test]
async fn test_advanced_event_filtering() {
    let event_bus = EventBus::new(100);
    
    // Create events with different test suite IDs
    let suite1_event = EventBus::create_test_lifecycle_event(
        "suite_1".to_string(),
        Some("suite_1_case_1".to_string()),
        TestLifecycleType::TestCaseStarted,
    );
    
    let suite2_event = EventBus::create_test_lifecycle_event(
        "suite_2".to_string(),
        Some("suite_2_case_1".to_string()),
        TestLifecycleType::TestCaseStarted,
    );
    
    // Filter by test suite ID
    let suite_filter = EventFilter::new()
        .with_test_suite_ids(vec!["suite_1".to_string()]);
    
    let mut subscription = event_bus.subscribe(suite_filter).await.unwrap();
    
    // Publish both events
    event_bus.publish(suite1_event).await.unwrap();
    event_bus.publish(suite2_event).await.unwrap();
    
    // Should only receive the suite_1 event
    let received_event = timeout(Duration::from_secs(1), subscription.recv())
        .await
        .unwrap()
        .unwrap();
    
    match received_event {
        Event::TestLifecycle(test_event) => {
            assert_eq!(test_event.test_suite_id, "suite_1");
        }
        _ => panic!("Expected TestLifecycle event"),
    }
}

#[tokio::test]
async fn test_time_range_filtering() {
    let persistence = Arc::new(InMemoryEventPersistence::new());
    
    let now = Utc::now();
    let one_hour_ago = now - chrono::Duration::hours(1);
    let two_hours_ago = now - chrono::Duration::hours(2);
    
    // Create events with different timestamps (simulated by creating them with metadata)
    let old_event = Event::TestLifecycle(TestLifecycleEvent {
        event_id: uuid::Uuid::new_v4().to_string(),
        timestamp: two_hours_ago,
        test_suite_id: "suite_1".to_string(),
        test_case_id: Some("case_1".to_string()),
        lifecycle_type: TestLifecycleType::TestCaseStarted,
        metadata: std::collections::HashMap::new(),
    });
    
    let recent_event = Event::TestLifecycle(TestLifecycleEvent {
        event_id: uuid::Uuid::new_v4().to_string(),
        timestamp: one_hour_ago,
        test_suite_id: "suite_1".to_string(),
        test_case_id: Some("case_2".to_string()),
        lifecycle_type: TestLifecycleType::TestCaseStarted,
        metadata: std::collections::HashMap::new(),
    });
    
    // Store events
    persistence.store_event(&old_event).await.unwrap();
    persistence.store_event(&recent_event).await.unwrap();
    
    // Filter by time range (last hour)
    let time_filter = EventFilter::new()
        .with_time_range(Some(one_hour_ago), Some(now));
    
    let filtered_events = persistence.get_events(&time_filter, None).await.unwrap();
    
    // Should only get the recent event
    assert_eq!(filtered_events.len(), 1);
    match &filtered_events[0] {
        Event::TestLifecycle(test_event) => {
            assert_eq!(test_event.test_case_id, Some("case_2".to_string()));
        }
        _ => panic!("Expected TestLifecycle event"),
    }
}

#[tokio::test]
async fn test_test_case_id_filtering() {
    let event_bus = EventBus::new(100);
    
    // Create events for different test cases
    let case1_event = EventBus::create_request_execution_event(
        "test_case_1".to_string(),
        "req_1".to_string(),
        RequestExecutionType::Started,
        None,
        None,
        None,
    );
    
    let case2_event = EventBus::create_request_execution_event(
        "test_case_2".to_string(),
        "req_2".to_string(),
        RequestExecutionType::Started,
        None,
        None,
        None,
    );
    
    // Filter by specific test case ID
    let case_filter = EventFilter::new()
        .with_test_case_ids(vec!["test_case_1".to_string()]);
    
    let mut subscription = event_bus.subscribe(case_filter).await.unwrap();
    
    // Publish both events
    event_bus.publish(case1_event).await.unwrap();
    event_bus.publish(case2_event).await.unwrap();
    
    // Should only receive the test_case_1 event
    let received_event = timeout(Duration::from_secs(1), subscription.recv())
        .await
        .unwrap()
        .unwrap();
    
    match received_event {
        Event::RequestExecution(req_event) => {
            assert_eq!(req_event.test_case_id, "test_case_1");
        }
        _ => panic!("Expected RequestExecution event"),
    }
}

#[tokio::test]
async fn test_sled_persistence_integration() {
    use tempfile::TempDir;
    
    // Create a temporary directory for the test database
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test_events.db");
    
    let persistence = Arc::new(
        apirunner::event_persistence::SledEventPersistence::new(db_path.to_str().unwrap()).unwrap()
    );
    
    let event_bus = EventBus::new(100).with_persistence(persistence.clone());
    
    // Create and publish events
    let event1 = EventBus::create_test_lifecycle_event(
        "suite_1".to_string(),
        Some("case_1".to_string()),
        TestLifecycleType::TestCaseStarted,
    );
    
    let event2 = EventBus::create_assertion_result_event(
        "case_1".to_string(),
        "status_code".to_string(),
        true,
        Some(serde_json::json!(200)),
        Some(serde_json::json!(200)),
        None,
    );
    
    event_bus.publish(event1).await.unwrap();
    event_bus.publish(event2).await.unwrap();
    
    // Retrieve events with filtering
    let filter = EventFilter::new()
        .with_event_types(vec!["assertion_result".to_string()]);
    
    let events = persistence.get_events(&filter, None).await.unwrap();
    
    assert_eq!(events.len(), 1);
    match &events[0] {
        Event::AssertionResult(assertion_event) => {
            assert_eq!(assertion_event.test_case_id, "case_1");
            assert_eq!(assertion_event.assertion_type, "status_code");
            assert!(assertion_event.success);
        }
        _ => panic!("Expected AssertionResult event"),
    }
    
    // Test time range queries
    let now = Utc::now();
    let one_minute_ago = now - chrono::Duration::minutes(1);
    
    let time_events = persistence
        .get_events_by_time_range(one_minute_ago, now, None)
        .await
        .unwrap();
    
    assert_eq!(time_events.len(), 2);
}#
[tokio::test]
async fn test_transaction_failure_scenarios() {
    let event_bus = Arc::new(EventBus::new(100));
    let transaction_manager = EventTransactionManager::new(event_bus.clone());
    
    // Register multiple handlers, one that will fail
    let good_handler = Arc::new(TestExecutionTransactionHandler::new("good_handler".to_string()));
    let bad_handler = Arc::new(FailingTransactionHandler);
    
    transaction_manager.register_handler(good_handler).await;
    transaction_manager.register_handler(bad_handler).await;
    
    // Begin a transaction
    let transaction_id = transaction_manager.begin_transaction().await.unwrap();
    
    // Process an event that will cause one handler to fail
    let event = EventBus::create_test_lifecycle_event(
        "suite_1".to_string(),
        Some("case_1".to_string()),
        TestLifecycleType::TestCaseStarted,
    );
    
    // This should fail due to the failing handler
    let result = transaction_manager
        .process_event_transactionally(&transaction_id, event)
        .await;
    
    assert!(result.is_err());
    
    // Verify transaction was automatically rolled back
    assert_eq!(transaction_manager.active_transaction_count().await, 0);
}

#[tokio::test]
async fn test_transaction_cleanup() {
    let event_bus = Arc::new(EventBus::new(100));
    let transaction_manager = EventTransactionManager::new(event_bus.clone());
    
    // Begin multiple transactions
    let tx1 = transaction_manager.begin_transaction().await.unwrap();
    let tx2 = transaction_manager.begin_transaction().await.unwrap();
    let tx3 = transaction_manager.begin_transaction().await.unwrap();
    
    assert_eq!(transaction_manager.active_transaction_count().await, 3);
    
    // Commit one transaction
    transaction_manager.commit_transaction(&tx1).await.unwrap();
    assert_eq!(transaction_manager.active_transaction_count().await, 2);
    
    // Rollback another transaction
    transaction_manager.rollback_transaction(&tx2).await.unwrap();
    assert_eq!(transaction_manager.active_transaction_count().await, 1);
    
    // Clean up stale transactions (simulate old transactions)
    // In a real scenario, this would clean up transactions older than the timeout
    let cleaned_count = transaction_manager.cleanup_stale_transactions(0).await.unwrap();
    assert_eq!(cleaned_count, 1); // Should clean up tx3
    assert_eq!(transaction_manager.active_transaction_count().await, 0);
}

#[tokio::test]
async fn test_transaction_recovery_handler() {
    let event_bus = Arc::new(EventBus::new(100));
    let transaction_manager = EventTransactionManager::new(event_bus.clone());
    
    // Register a recovery handler
    let recovery_handler = Arc::new(apirunner::LoggingRecoveryHandler);
    transaction_manager.register_recovery_handler(recovery_handler).await;
    
    // Register a handler that will fail during rollback
    let failing_rollback_handler = Arc::new(FailingRollbackHandler);
    transaction_manager.register_handler(failing_rollback_handler).await;
    
    // Begin a transaction
    let transaction_id = transaction_manager.begin_transaction().await.unwrap();
    
    // Add an event to the transaction
    let event = EventBus::create_test_lifecycle_event(
        "suite_1".to_string(),
        Some("case_1".to_string()),
        TestLifecycleType::TestCaseStarted,
    );
    
    transaction_manager
        .process_event_transactionally(&transaction_id, event)
        .await
        .unwrap();
    
    // Force rollback (which will fail and trigger recovery)
    let rollback_result = transaction_manager.rollback_transaction(&transaction_id).await;
    
    // Rollback should complete but report errors
    assert!(rollback_result.is_err());
    
    // Transaction should still be cleaned up
    assert_eq!(transaction_manager.active_transaction_count().await, 0);
}

#[tokio::test]
async fn test_concurrent_transactions() {
    let event_bus = Arc::new(EventBus::new(100));
    let transaction_manager = Arc::new(EventTransactionManager::new(event_bus.clone()));
    
    // Register a handler
    let handler = Arc::new(TestExecutionTransactionHandler::new("concurrent_handler".to_string()));
    transaction_manager.register_handler(handler).await;
    
    // Create multiple concurrent transactions
    let mut handles = Vec::new();
    
    for i in 0..10 {
        let tm = transaction_manager.clone();
        let handle = tokio::spawn(async move {
            let tx_id = tm.begin_transaction().await.unwrap();
            
            let event = EventBus::create_test_lifecycle_event(
                format!("suite_{}", i),
                Some(format!("case_{}", i)),
                TestLifecycleType::TestCaseStarted,
            );
            
            tm.process_event_transactionally(&tx_id, event).await.unwrap();
            tm.commit_transaction(&tx_id).await.unwrap();
        });
        
        handles.push(handle);
    }
    
    // Wait for all transactions to complete
    for handle in handles {
        handle.await.unwrap();
    }
    
    // All transactions should be completed
    assert_eq!(transaction_manager.active_transaction_count().await, 0);
}

#[tokio::test]
async fn test_transaction_rollback_events() {
    let persistence = Arc::new(InMemoryEventPersistence::new());
    let event_bus = Arc::new(EventBus::new(100).with_persistence(persistence.clone()));
    let transaction_manager = EventTransactionManager::new(event_bus.clone());
    
    // Register a handler that creates rollback events
    let handler = Arc::new(TestExecutionTransactionHandler::new("rollback_test".to_string()));
    transaction_manager.register_handler(handler).await;
    
    // Begin a transaction
    let transaction_id = transaction_manager.begin_transaction().await.unwrap();
    
    // Process a test lifecycle event (which should create a rollback event)
    let event = EventBus::create_test_lifecycle_event(
        "suite_1".to_string(),
        Some("case_1".to_string()),
        TestLifecycleType::TestCaseStarted,
    );
    
    transaction_manager
        .process_event_transactionally(&transaction_id, event)
        .await
        .unwrap();
    
    // Rollback the transaction
    transaction_manager.rollback_transaction(&transaction_id).await.unwrap();
    
    // Check that rollback events were published
    let filter = EventFilter::new()
        .with_event_types(vec!["test_lifecycle".to_string()]);
    
    let events = persistence.get_events(&filter, None).await.unwrap();
    
    // Should have at least one rollback event
    let rollback_events: Vec<_> = events
        .iter()
        .filter(|event| {
            if let Event::TestLifecycle(test_event) = event {
                test_event.metadata.get("rollback") == Some(&"true".to_string())
            } else {
                false
            }
        })
        .collect();
    
    assert!(!rollback_events.is_empty(), "Should have rollback events");
}

// Helper struct for testing rollback failures
struct FailingRollbackHandler;

#[async_trait]
impl TransactionalEventHandler for FailingRollbackHandler {
    async fn handle_event(&self, _event: &Event, _transaction: &mut EventTransaction) -> Result<(), apirunner::EventError> {
        Ok(()) // Handle events successfully
    }
    
    async fn rollback(&self, _transaction: &EventTransaction) -> Result<(), apirunner::EventError> {
        Err(apirunner::EventError::PersistenceError("Rollback failed".to_string()))
    }
    
    fn handler_name(&self) -> &str {
        "failing_rollback_handler"
    }
}