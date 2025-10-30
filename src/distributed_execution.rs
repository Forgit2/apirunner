//! Distributed execution framework for API test runner
//! 
//! This module provides distributed execution capabilities with:
//! - Node discovery and registration
//! - Task distribution and load balancing
//! - Fault tolerance with automatic task redistribution
//! - Result aggregation from multiple execution nodes

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, RwLock};
use tokio::time::{interval, sleep};
use uuid::Uuid;

use crate::error::{ExecutionError, Result};
use crate::event::{Event, EventBus, TestLifecycleEvent, TestLifecycleType};
use crate::execution::{ExecutionContext, ExecutionResult, ExecutionStrategy, TestResult};
use crate::test_case_manager::TestCase;

/// Node information for distributed execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionNode {
    pub node_id: String,
    pub address: SocketAddr,
    pub capabilities: NodeCapabilities,
    pub status: NodeStatus,
    pub last_heartbeat: DateTime<Utc>,
    pub current_load: u32,
    pub max_concurrent_tasks: u32,
}

/// Node capabilities and configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeCapabilities {
    pub supported_protocols: Vec<String>,
    pub max_concurrent_tests: u32,
    pub cpu_cores: u32,
    pub memory_mb: u64,
    pub network_bandwidth_mbps: u32,
}

/// Current status of an execution node
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum NodeStatus {
    Available,
    Busy,
    Overloaded,
    Unreachable,
    Maintenance,
}

/// Task assignment for distributed execution
#[derive(Debug, Clone)]
pub struct TaskAssignment {
    pub task_id: String,
    pub node_id: String,
    pub test_cases: Vec<TestCase>,
    pub context: ExecutionContext,
    pub assigned_at: DateTime<Utc>,
    pub timeout: Duration,
    pub retry_count: u32,
}

/// Result from a distributed task execution
#[derive(Debug, Clone)]
pub struct DistributedTaskResult {
    pub task_id: String,
    pub node_id: String,
    pub test_results: Vec<TestResult>,
    pub execution_duration: Duration,
    pub completed_at: DateTime<Utc>,
    pub success: bool,
    pub error_message: Option<String>,
}

/// Messages for node communication
#[derive(Debug, Clone)]
pub enum NodeMessage {
    Heartbeat {
        node_id: String,
        status: NodeStatus,
        current_load: u32,
        timestamp: DateTime<Utc>,
    },
    TaskAssignment {
        assignment: TaskAssignment,
    },
    TaskResult {
        result: DistributedTaskResult,
    },
    TaskCancellation {
        task_id: String,
        reason: String,
    },
    NodeRegistration {
        node: ExecutionNode,
    },
    NodeDeregistration {
        node_id: String,
        reason: String,
    },
}

/// Node discovery service for finding and managing execution nodes
pub struct NodeDiscovery {
    nodes: Arc<RwLock<HashMap<String, ExecutionNode>>>,
    heartbeat_timeout: Duration,
    cleanup_interval: Duration,
}

impl NodeDiscovery {
    pub fn new(heartbeat_timeout: Duration) -> Self {
        Self {
            nodes: Arc::new(RwLock::new(HashMap::new())),
            heartbeat_timeout,
            cleanup_interval: Duration::from_secs(30),
        }
    }

    /// Register a new execution node
    pub async fn register_node(&self, node: ExecutionNode) -> Result<()> {
        let mut nodes = self.nodes.write().await;
        nodes.insert(node.node_id.clone(), node);
        Ok(())
    }

    /// Deregister an execution node
    pub async fn deregister_node(&self, node_id: &str) -> Result<()> {
        let mut nodes = self.nodes.write().await;
        nodes.remove(node_id);
        Ok(())
    }

    /// Update node heartbeat
    pub async fn update_heartbeat(&self, node_id: &str, status: NodeStatus, current_load: u32) -> Result<()> {
        let mut nodes = self.nodes.write().await;
        if let Some(node) = nodes.get_mut(node_id) {
            node.last_heartbeat = Utc::now();
            node.status = status;
            node.current_load = current_load;
        }
        Ok(())
    }

    /// Get all available nodes
    pub async fn get_available_nodes(&self) -> Vec<ExecutionNode> {
        let nodes = self.nodes.read().await;
        nodes.values()
            .filter(|node| node.status == NodeStatus::Available)
            .cloned()
            .collect()
    }

    /// Get node by ID
    pub async fn get_node(&self, node_id: &str) -> Option<ExecutionNode> {
        let nodes = self.nodes.read().await;
        nodes.get(node_id).cloned()
    }

    /// Start cleanup task to remove stale nodes
    pub async fn start_cleanup_task(&self) {
        let nodes = Arc::clone(&self.nodes);
        let heartbeat_timeout = self.heartbeat_timeout;
        let cleanup_interval = self.cleanup_interval;

        tokio::spawn(async move {
            let mut interval = interval(cleanup_interval);
            loop {
                interval.tick().await;
                
                let now = Utc::now();
                let mut nodes_guard = nodes.write().await;
                let mut to_remove = Vec::new();

                for (node_id, node) in nodes_guard.iter_mut() {
                    let elapsed = now.signed_duration_since(node.last_heartbeat);
                    if elapsed.to_std().unwrap_or(Duration::MAX) > heartbeat_timeout {
                        node.status = NodeStatus::Unreachable;
                        to_remove.push(node_id.clone());
                    }
                }

                for node_id in to_remove {
                    nodes_guard.remove(&node_id);
                }
            }
        });
    }
}

/// Task distribution system for load balancing across nodes
pub struct TaskDistributor {
    node_discovery: Arc<NodeDiscovery>,
    pending_tasks: Arc<Mutex<VecDeque<TaskAssignment>>>,
    active_tasks: Arc<RwLock<HashMap<String, TaskAssignment>>>,
    completed_tasks: Arc<RwLock<HashMap<String, DistributedTaskResult>>>,
    task_timeout: Duration,
}

impl TaskDistributor {
    pub fn new(node_discovery: Arc<NodeDiscovery>, task_timeout: Duration) -> Self {
        Self {
            node_discovery,
            pending_tasks: Arc::new(Mutex::new(VecDeque::new())),
            active_tasks: Arc::new(RwLock::new(HashMap::new())),
            completed_tasks: Arc::new(RwLock::new(HashMap::new())),
            task_timeout,
        }
    }

    /// Add a task for distribution
    pub async fn add_task(&self, test_cases: Vec<TestCase>, context: ExecutionContext) -> String {
        let task_id = Uuid::new_v4().to_string();
        let assignment = TaskAssignment {
            task_id: task_id.clone(),
            node_id: String::new(), // Will be assigned during distribution
            test_cases,
            context,
            assigned_at: Utc::now(),
            timeout: self.task_timeout,
            retry_count: 0,
        };

        let mut pending = self.pending_tasks.lock().await;
        pending.push_back(assignment);
        
        task_id
    }

    /// Distribute pending tasks to available nodes
    pub async fn distribute_tasks(&self) -> Result<()> {
        let available_nodes = self.node_discovery.get_available_nodes().await;
        if available_nodes.is_empty() {
            return Ok(()); // No nodes available
        }

        let mut pending = self.pending_tasks.lock().await;
        let mut active = self.active_tasks.write().await;

        while let Some(mut task) = pending.pop_front() {
            // Find the best node for this task using load balancing
            if let Some(best_node) = self.select_best_node(&available_nodes).await {
                task.node_id = best_node.node_id.clone();
                task.assigned_at = Utc::now();
                
                // Send task to node (placeholder - would use actual network communication)
                self.send_task_to_node(&task, &best_node).await?;
                
                active.insert(task.task_id.clone(), task);
            } else {
                // No suitable node found, put task back
                pending.push_front(task);
                break;
            }
        }

        Ok(())
    }

    /// Select the best node for task assignment based on load and capabilities
    async fn select_best_node(&self, nodes: &[ExecutionNode]) -> Option<ExecutionNode> {
        nodes.iter()
            .filter(|node| node.status == NodeStatus::Available)
            .filter(|node| node.current_load < node.max_concurrent_tasks)
            .min_by_key(|node| node.current_load)
            .cloned()
    }

    /// Send task to execution node (placeholder implementation)
    async fn send_task_to_node(&self, task: &TaskAssignment, _node: &ExecutionNode) -> Result<()> {
        // In a real implementation, this would send the task over network
        // For now, we'll simulate task execution
        tokio::spawn({
            let task = task.clone();
            async move {
                // Simulate task execution time
                sleep(Duration::from_millis(100)).await;
                
                // Create mock result
                let result = DistributedTaskResult {
                    task_id: task.task_id.clone(),
                    node_id: task.node_id.clone(),
                    test_results: Vec::new(), // Would contain actual test results
                    execution_duration: Duration::from_millis(100),
                    completed_at: Utc::now(),
                    success: true,
                    error_message: None,
                };
                
                // In real implementation, this would be received from the node
                // For now, we'll just simulate completion
            }
        });

        Ok(())
    }

    /// Handle task completion from a node
    pub async fn handle_task_completion(&self, result: DistributedTaskResult) -> Result<()> {
        let mut active = self.active_tasks.write().await;
        let mut completed = self.completed_tasks.write().await;

        if active.remove(&result.task_id).is_some() {
            completed.insert(result.task_id.clone(), result);
        }

        Ok(())
    }

    /// Handle task failure and retry logic
    pub async fn handle_task_failure(&self, task_id: &str, error: String) -> Result<()> {
        let mut active = self.active_tasks.write().await;
        let mut pending = self.pending_tasks.lock().await;

        if let Some(mut task) = active.remove(task_id) {
            task.retry_count += 1;
            
            if task.retry_count < 3 { // Max 3 retries
                task.node_id = String::new(); // Reset node assignment
                pending.push_back(task);
            } else {
                // Max retries exceeded, mark as failed
                let failed_result = DistributedTaskResult {
                    task_id: task_id.to_string(),
                    node_id: task.node_id.clone(),
                    test_results: Vec::new(),
                    execution_duration: Duration::from_secs(0),
                    completed_at: Utc::now(),
                    success: false,
                    error_message: Some(error),
                };
                
                let mut completed = self.completed_tasks.write().await;
                completed.insert(task_id.to_string(), failed_result);
            }
        }

        Ok(())
    }

    /// Get all completed tasks
    pub async fn get_completed_tasks(&self) -> HashMap<String, DistributedTaskResult> {
        let completed = self.completed_tasks.read().await;
        completed.clone()
    }

    /// Check if all tasks are completed
    pub async fn all_tasks_completed(&self) -> bool {
        let pending = self.pending_tasks.lock().await;
        let active = self.active_tasks.read().await;
        pending.is_empty() && active.is_empty()
    }

    /// Get active tasks count (for testing)
    pub async fn active_tasks_count(&self) -> usize {
        let active = self.active_tasks.read().await;
        active.len()
    }

    /// Get pending tasks count (for testing)
    pub async fn pending_tasks_count(&self) -> usize {
        let pending = self.pending_tasks.lock().await;
        pending.len()
    }
}

/// Fault tolerance manager for handling node failures and task redistribution
pub struct FaultToleranceManager {
    task_distributor: Arc<TaskDistributor>,
    node_discovery: Arc<NodeDiscovery>,
    failed_nodes: Arc<RwLock<HashSet<String>>>,
    monitoring_interval: Duration,
}

impl FaultToleranceManager {
    pub fn new(
        task_distributor: Arc<TaskDistributor>,
        node_discovery: Arc<NodeDiscovery>,
    ) -> Self {
        Self {
            task_distributor,
            node_discovery,
            failed_nodes: Arc::new(RwLock::new(HashSet::new())),
            monitoring_interval: Duration::from_secs(10),
        }
    }

    /// Start monitoring for node failures
    pub async fn start_monitoring(&self) {
        let task_distributor = Arc::clone(&self.task_distributor);
        let node_discovery = Arc::clone(&self.node_discovery);
        let failed_nodes = Arc::clone(&self.failed_nodes);
        let monitoring_interval = self.monitoring_interval;

        tokio::spawn(async move {
            let mut interval = interval(monitoring_interval);
            loop {
                interval.tick().await;
                
                // Check for failed nodes and redistribute their tasks
                let available_nodes = node_discovery.get_available_nodes().await;
                let current_node_ids: HashSet<String> = available_nodes
                    .iter()
                    .map(|node| node.node_id.clone())
                    .collect();

                let mut failed = failed_nodes.write().await;
                let active_tasks = task_distributor.active_tasks.read().await;

                // Find tasks on failed nodes
                for (task_id, task) in active_tasks.iter() {
                    if !current_node_ids.contains(&task.node_id) && !failed.contains(&task.node_id) {
                        // Node has failed, mark it and redistribute task
                        failed.insert(task.node_id.clone());
                        
                        // Handle task failure (will trigger redistribution)
                        let error_msg = format!("Node {} became unreachable", task.node_id);
                        if let Err(e) = task_distributor.handle_task_failure(task_id, error_msg).await {
                            eprintln!("Failed to handle task failure: {}", e);
                        }
                    }
                }
            }
        });
    }

    /// Handle node recovery
    pub async fn handle_node_recovery(&self, node_id: &str) -> Result<()> {
        let mut failed = self.failed_nodes.write().await;
        failed.remove(node_id);
        Ok(())
    }
}

/// Distributed execution strategy that coordinates test execution across multiple nodes
pub struct DistributedExecutor {
    event_bus: Arc<EventBus>,
    node_discovery: Arc<NodeDiscovery>,
    task_distributor: Arc<TaskDistributor>,
    fault_tolerance: Arc<FaultToleranceManager>,
    max_wait_time: Duration,
}

impl DistributedExecutor {
    pub fn new(event_bus: Arc<EventBus>) -> Self {
        let node_discovery = Arc::new(NodeDiscovery::new(Duration::from_secs(30)));
        let task_distributor = Arc::new(TaskDistributor::new(
            Arc::clone(&node_discovery),
            Duration::from_secs(300), // 5 minute task timeout
        ));
        let fault_tolerance = Arc::new(FaultToleranceManager::new(
            Arc::clone(&task_distributor),
            Arc::clone(&node_discovery),
        ));

        Self {
            event_bus,
            node_discovery,
            task_distributor,
            fault_tolerance,
            max_wait_time: Duration::from_secs(600), // 10 minute max wait
        }
    }

    /// Initialize the distributed execution environment
    pub async fn initialize(&self) -> Result<()> {
        // Start background services
        self.node_discovery.start_cleanup_task().await;
        self.fault_tolerance.start_monitoring().await;

        // Register some mock nodes for testing
        self.register_mock_nodes().await?;

        Ok(())
    }

    /// Register mock nodes for testing purposes
    async fn register_mock_nodes(&self) -> Result<()> {
        let mock_nodes = vec![
            ExecutionNode {
                node_id: "node-1".to_string(),
                address: "127.0.0.1:8001".parse().unwrap(),
                capabilities: NodeCapabilities {
                    supported_protocols: vec!["http".to_string(), "https".to_string()],
                    max_concurrent_tests: 10,
                    cpu_cores: 4,
                    memory_mb: 8192,
                    network_bandwidth_mbps: 1000,
                },
                status: NodeStatus::Available,
                last_heartbeat: Utc::now(),
                current_load: 0,
                max_concurrent_tasks: 10,
            },
            ExecutionNode {
                node_id: "node-2".to_string(),
                address: "127.0.0.1:8002".parse().unwrap(),
                capabilities: NodeCapabilities {
                    supported_protocols: vec!["http".to_string(), "https".to_string(), "grpc".to_string()],
                    max_concurrent_tests: 20,
                    cpu_cores: 8,
                    memory_mb: 16384,
                    network_bandwidth_mbps: 1000,
                },
                status: NodeStatus::Available,
                last_heartbeat: Utc::now(),
                current_load: 0,
                max_concurrent_tasks: 20,
            },
        ];

        for node in mock_nodes {
            self.node_discovery.register_node(node).await?;
        }

        Ok(())
    }

    /// Partition test cases into chunks for distribution
    fn partition_test_cases(&self, test_cases: Vec<TestCase>, chunk_size: usize) -> Vec<Vec<TestCase>> {
        test_cases
            .chunks(chunk_size)
            .map(|chunk| chunk.to_vec())
            .collect()
    }

    /// Aggregate results from all distributed tasks
    async fn aggregate_results(
        &self,
        task_results: HashMap<String, DistributedTaskResult>,
        context: ExecutionContext,
    ) -> ExecutionResult {
        let mut result = ExecutionResult::new(context);
        let start_time = Instant::now();

        for (_, task_result) in task_results {
            for test_result in task_result.test_results {
                result.add_test_result(test_result);
            }
        }

        result.total_duration = start_time.elapsed();
        result
    }
}

#[async_trait]
impl ExecutionStrategy for DistributedExecutor {
    async fn execute(
        &self,
        test_cases: Vec<TestCase>,
        context: ExecutionContext,
    ) -> Result<ExecutionResult> {
        // Initialize if not already done
        self.initialize().await?;

        // Publish suite started event
        let suite_started_event = Event::TestLifecycle(TestLifecycleEvent {
            event_id: Uuid::new_v4().to_string(),
            timestamp: Utc::now(),
            test_suite_id: context.test_suite_id.clone(),
            test_case_id: None,
            lifecycle_type: TestLifecycleType::SuiteStarted,
            metadata: HashMap::from([
                ("test_count".to_string(), test_cases.len().to_string()),
                ("strategy".to_string(), "distributed".to_string()),
            ]),
        });

        self.event_bus.publish(suite_started_event).await.ok();

        // Partition test cases into chunks for distribution
        let available_nodes = self.node_discovery.get_available_nodes().await;
        if available_nodes.is_empty() {
            return Err(ExecutionError::UnsupportedStrategy(
                "No execution nodes available for distributed execution".to_string()
            ).into());
        }

        let chunk_size = (test_cases.len() / available_nodes.len()).max(1);
        let test_chunks = self.partition_test_cases(test_cases, chunk_size);

        // Create tasks for each chunk
        let mut task_ids = Vec::new();
        for chunk in test_chunks {
            let task_id = self.task_distributor.add_task(chunk, context.clone()).await;
            task_ids.push(task_id);
        }

        // Distribute tasks to nodes
        self.task_distributor.distribute_tasks().await?;

        // Wait for all tasks to complete
        let start_wait = Instant::now();
        while !self.task_distributor.all_tasks_completed().await {
            if start_wait.elapsed() > self.max_wait_time {
                return Err(ExecutionError::ResourceExhausted(
                    "Distributed execution timeout exceeded".to_string()
                ).into());
            }

            // Redistribute tasks periodically
            self.task_distributor.distribute_tasks().await?;
            sleep(Duration::from_secs(1)).await;
        }

        // Aggregate results from all tasks
        let completed_tasks = self.task_distributor.get_completed_tasks().await;
        let result = self.aggregate_results(completed_tasks, context.clone()).await;

        // Publish suite completed event
        let lifecycle_type = if result.is_successful() {
            TestLifecycleType::SuiteCompleted
        } else {
            TestLifecycleType::SuiteFailed
        };

        let suite_completed_event = Event::TestLifecycle(TestLifecycleEvent {
            event_id: Uuid::new_v4().to_string(),
            timestamp: Utc::now(),
            test_suite_id: result.context.test_suite_id.clone(),
            test_case_id: None,
            lifecycle_type,
            metadata: HashMap::from([
                ("total_duration_ms".to_string(), result.total_duration.as_millis().to_string()),
                ("success_count".to_string(), result.success_count.to_string()),
                ("failure_count".to_string(), result.failure_count.to_string()),
                ("nodes_used".to_string(), available_nodes.len().to_string()),
            ]),
        });

        self.event_bus.publish(suite_completed_event).await.ok();

        Ok(result)
    }

    fn strategy_name(&self) -> &str {
        "distributed"
    }
}

impl DistributedExecutor {
    /// Get available nodes (for testing)
    pub async fn get_available_nodes(&self) -> Vec<ExecutionNode> {
        self.node_discovery.get_available_nodes().await
    }
}