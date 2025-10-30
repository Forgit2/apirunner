use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};

use tokio::time::interval;
use crate::error::ApiTestError;
use crate::metrics::{MetricsCollector, MetricsSnapshot};

/// Alerting system that monitors metrics and triggers notifications
#[derive(Clone)]
pub struct AlertingSystem {
    config: AlertingConfig,
    rules: Arc<RwLock<Vec<AlertRule>>>,
    notification_manager: NotificationManager,
    alert_state: Arc<RwLock<HashMap<String, AlertState>>>,
    metrics_collector: Arc<MetricsCollector>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertingConfig {
    pub enabled: bool,
    pub evaluation_interval: Duration,
    pub default_notification_channels: Vec<String>,
    pub alert_retention_period: Duration,
}

impl Default for AlertingConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            evaluation_interval: Duration::from_secs(30),
            default_notification_channels: vec!["console".to_string()],
            alert_retention_period: Duration::from_secs(24 * 60 * 60), // 24 hours
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertRule {
    pub id: String,
    pub name: String,
    pub description: String,
    pub condition: AlertCondition,
    pub severity: AlertSeverity,
    pub notification_channels: Vec<String>,
    pub enabled: bool,
    pub cooldown_period: Duration,
    pub escalation_rules: Vec<EscalationRule>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AlertCondition {
    MetricThreshold {
        metric_name: String,
        operator: ComparisonOperator,
        threshold: f64,
        duration: Duration,
    },
    ErrorRate {
        threshold_percent: f64,
        duration: Duration,
    },
    ResponseTime {
        percentile: u8, // 50, 95, 99
        threshold_ms: f64,
        duration: Duration,
    },
    ResourceUsage {
        resource_type: ResourceType,
        threshold_percent: f64,
        duration: Duration,
    },
    TestFailureRate {
        threshold_percent: f64,
        duration: Duration,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ComparisonOperator {
    GreaterThan,
    LessThan,
    Equal,
    GreaterThanOrEqual,
    LessThanOrEqual,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ResourceType {
    Cpu,
    Memory,
    Network,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AlertSeverity {
    Critical,
    Warning,
    Info,
}

impl std::fmt::Display for AlertSeverity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AlertSeverity::Critical => write!(f, "CRITICAL"),
            AlertSeverity::Warning => write!(f, "WARNING"),
            AlertSeverity::Info => write!(f, "INFO"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EscalationRule {
    pub delay: Duration,
    pub notification_channels: Vec<String>,
    pub repeat_interval: Option<Duration>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Alert {
    pub id: String,
    pub rule_id: String,
    pub name: String,
    pub description: String,
    pub severity: AlertSeverity,
    pub status: AlertStatus,
    pub triggered_at: chrono::DateTime<chrono::Utc>,
    pub resolved_at: Option<chrono::DateTime<chrono::Utc>>,
    pub acknowledged_at: Option<chrono::DateTime<chrono::Utc>>,
    pub acknowledged_by: Option<String>,
    pub current_value: f64,
    pub threshold: f64,
    pub labels: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AlertStatus {
    Firing,
    Resolved,
    Acknowledged,
}

#[derive(Debug, Clone)]
struct AlertState {
    rule_id: String,
    first_triggered: Instant,
    last_evaluated: Instant,
    current_alert: Option<Alert>,
    escalation_level: usize,
    last_notification: Option<Instant>,
}

/// Notification manager handles different notification channels
#[derive(Clone)]
pub struct NotificationManager {
    channels: Arc<RwLock<HashMap<String, Box<dyn NotificationChannel>>>>,
}

#[async_trait::async_trait]
pub trait NotificationChannel: Send + Sync {
    async fn send_notification(&self, notification: &Notification) -> Result<(), ApiTestError>;
    fn channel_type(&self) -> &str;
    fn is_available(&self) -> bool;
    fn clone_box(&self) -> Box<dyn NotificationChannel>;
}

impl Clone for Box<dyn NotificationChannel> {
    fn clone(&self) -> Self {
        self.clone_box()
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Notification {
    pub alert: Alert,
    pub message: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub notification_type: NotificationType,
}

#[derive(Debug, Clone, Serialize)]
pub enum NotificationType {
    AlertTriggered,
    AlertResolved,
    AlertEscalated,
    AlertAcknowledged,
}

/// Console notification channel for development/testing
#[derive(Clone)]
pub struct ConsoleNotificationChannel;

#[async_trait::async_trait]
impl NotificationChannel for ConsoleNotificationChannel {
    async fn send_notification(&self, notification: &Notification) -> Result<(), ApiTestError> {
        println!("[ALERT] {}: {}", notification.alert.severity, notification.message);
        Ok(())
    }

    fn channel_type(&self) -> &str {
        "console"
    }

    fn is_available(&self) -> bool {
        true
    }

    fn clone_box(&self) -> Box<dyn NotificationChannel> {
        Box::new(self.clone())
    }
}

/// Email notification channel
#[derive(Clone)]
pub struct EmailNotificationChannel {
    smtp_config: EmailConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmailConfig {
    pub smtp_server: String,
    pub smtp_port: u16,
    pub username: String,
    pub password: String,
    pub from_address: String,
    pub to_addresses: Vec<String>,
    pub use_tls: bool,
}

#[async_trait::async_trait]
impl NotificationChannel for EmailNotificationChannel {
    async fn send_notification(&self, notification: &Notification) -> Result<(), ApiTestError> {
        // TODO: Implement actual email sending
        log::info!("Email notification sent: {}", notification.message);
        Ok(())
    }

    fn channel_type(&self) -> &str {
        "email"
    }

    fn is_available(&self) -> bool {
        // TODO: Check SMTP connectivity
        true
    }

    fn clone_box(&self) -> Box<dyn NotificationChannel> {
        Box::new(self.clone())
    }
}

/// Slack notification channel
#[derive(Clone)]
pub struct SlackNotificationChannel {
    webhook_url: String,
    channel: String,
}

#[async_trait::async_trait]
impl NotificationChannel for SlackNotificationChannel {
    async fn send_notification(&self, notification: &Notification) -> Result<(), ApiTestError> {
        let payload = serde_json::json!({
            "channel": self.channel,
            "text": notification.message,
            "attachments": [{
                "color": match notification.alert.severity {
                    AlertSeverity::Critical => "danger",
                    AlertSeverity::Warning => "warning",
                    AlertSeverity::Info => "good",
                },
                "fields": [
                    {
                        "title": "Alert",
                        "value": notification.alert.name,
                        "short": true
                    },
                    {
                        "title": "Severity",
                        "value": format!("{:?}", notification.alert.severity),
                        "short": true
                    },
                    {
                        "title": "Current Value",
                        "value": notification.alert.current_value,
                        "short": true
                    },
                    {
                        "title": "Threshold",
                        "value": notification.alert.threshold,
                        "short": true
                    }
                ]
            }]
        });

        let client = reqwest::Client::new();
        let response = client
            .post(&self.webhook_url)
            .json(&payload)
            .send()
            .await
            .map_err(|e| ApiTestError::Configuration(format!("Slack notification failed: {}", e)))?;

        if !response.status().is_success() {
            return Err(ApiTestError::Configuration(format!(
                "Slack notification failed with status: {}",
                response.status()
            )));
        }

        Ok(())
    }

    fn channel_type(&self) -> &str {
        "slack"
    }

    fn is_available(&self) -> bool {
        !self.webhook_url.is_empty()
    }

    fn clone_box(&self) -> Box<dyn NotificationChannel> {
        Box::new(self.clone())
    }
}

/// Webhook notification channel
#[derive(Clone)]
pub struct WebhookNotificationChannel {
    url: String,
    headers: HashMap<String, String>,
}

#[async_trait::async_trait]
impl NotificationChannel for WebhookNotificationChannel {
    async fn send_notification(&self, notification: &Notification) -> Result<(), ApiTestError> {
        let client = reqwest::Client::new();
        let mut request = client.post(&self.url).json(notification);

        for (key, value) in &self.headers {
            request = request.header(key, value);
        }

        let response = request
            .send()
            .await
            .map_err(|e| ApiTestError::Configuration(format!("Webhook notification failed: {}", e)))?;

        if !response.status().is_success() {
            return Err(ApiTestError::Configuration(format!(
                "Webhook notification failed with status: {}",
                response.status()
            )));
        }

        Ok(())
    }

    fn channel_type(&self) -> &str {
        "webhook"
    }

    fn is_available(&self) -> bool {
        !self.url.is_empty()
    }

    fn clone_box(&self) -> Box<dyn NotificationChannel> {
        Box::new(self.clone())
    }
}

impl AlertingSystem {
    pub fn new(
        config: AlertingConfig,
        metrics_collector: Arc<MetricsCollector>,
    ) -> Result<Self, ApiTestError> {
        let notification_manager = NotificationManager::new();
        
        Ok(Self {
            config,
            rules: Arc::new(RwLock::new(Vec::new())),
            notification_manager,
            alert_state: Arc::new(RwLock::new(HashMap::new())),
            metrics_collector,
        })
    }

    /// Add an alert rule
    pub fn add_rule(&self, rule: AlertRule) {
        let mut rules = self.rules.write();
        rules.push(rule);
    }

    /// Remove an alert rule
    pub fn remove_rule(&self, rule_id: &str) -> bool {
        let mut rules = self.rules.write();
        if let Some(pos) = rules.iter().position(|r| r.id == rule_id) {
            rules.remove(pos);
            true
        } else {
            false
        }
    }

    /// Get all alert rules
    pub fn get_rules(&self) -> Vec<AlertRule> {
        self.rules.read().clone()
    }

    /// Get active alerts
    pub fn get_active_alerts(&self) -> Vec<Alert> {
        self.alert_state
            .read()
            .values()
            .filter_map(|state| state.current_alert.clone())
            .filter(|alert| matches!(alert.status, AlertStatus::Firing | AlertStatus::Acknowledged))
            .collect()
    }

    /// Acknowledge an alert
    pub fn acknowledge_alert(&self, alert_id: &str, acknowledged_by: String) -> Result<(), ApiTestError> {
        let mut alert_state = self.alert_state.write();
        
        for state in alert_state.values_mut() {
            if let Some(ref mut alert) = state.current_alert {
                if alert.id == alert_id {
                    alert.status = AlertStatus::Acknowledged;
                    alert.acknowledged_at = Some(chrono::Utc::now());
                    alert.acknowledged_by = Some(acknowledged_by);
                    return Ok(());
                }
            }
        }
        
        Err(ApiTestError::Configuration(format!("Alert not found: {}", alert_id)))
    }

    /// Start the alerting system
    pub async fn start(&self) -> Result<(), ApiTestError> {
        if !self.config.enabled {
            return Ok(());
        }

        let alerting_system = self.clone();
        let evaluation_interval = self.config.evaluation_interval;

        tokio::spawn(async move {
            let mut interval = interval(evaluation_interval);

            loop {
                interval.tick().await;

                if let Err(e) = alerting_system.evaluate_rules().await {
                    log::error!("Failed to evaluate alert rules: {}", e);
                }
            }
        });

        Ok(())
    }

    async fn evaluate_rules(&self) -> Result<(), ApiTestError> {
        let snapshot = self.metrics_collector.get_snapshot()?;
        let rules = self.rules.read().clone();
        let now = Instant::now();

        for rule in rules {
            if !rule.enabled {
                continue;
            }

            let should_trigger = self.evaluate_condition(&rule.condition, &snapshot)?;
            self.process_rule_evaluation(&rule, should_trigger, now, &snapshot).await?;
        }

        Ok(())
    }

    fn evaluate_condition(
        &self,
        condition: &AlertCondition,
        snapshot: &MetricsSnapshot,
    ) -> Result<bool, ApiTestError> {
        match condition {
            AlertCondition::ErrorRate { threshold_percent, .. } => {
                Ok(snapshot.performance.error_rate > *threshold_percent)
            }
            AlertCondition::ResponseTime { percentile, threshold_ms, .. } => {
                let response_time = match percentile {
                    95 => snapshot.performance.p95_response_time,
                    99 => snapshot.performance.p99_response_time,
                    _ => snapshot.performance.average_response_time,
                };
                Ok(response_time * 1000.0 > *threshold_ms)
            }
            AlertCondition::ResourceUsage { resource_type, threshold_percent, .. } => {
                let usage = match resource_type {
                    ResourceType::Cpu => snapshot.resource_usage.cpu_usage_percent,
                    ResourceType::Memory => snapshot.resource_usage.memory_usage_percent,
                    ResourceType::Network => 0.0, // TODO: Calculate network usage percentage
                };
                Ok(usage > *threshold_percent)
            }
            AlertCondition::TestFailureRate { threshold_percent, .. } => {
                let failure_rate = 100.0 - snapshot.test_execution.success_rate;
                Ok(failure_rate > *threshold_percent)
            }
            AlertCondition::MetricThreshold { threshold, operator, .. } => {
                // TODO: Implement generic metric threshold evaluation
                Ok(match operator {
                    ComparisonOperator::GreaterThan => snapshot.performance.total_requests as f64 > *threshold,
                    ComparisonOperator::LessThan => (snapshot.performance.total_requests as f64) < *threshold,
                    _ => false,
                })
            }
        }
    }

    async fn process_rule_evaluation(
        &self,
        rule: &AlertRule,
        should_trigger: bool,
        now: Instant,
        snapshot: &MetricsSnapshot,
    ) -> Result<(), ApiTestError> {
        // First, get or create the alert state
        let (mut needs_new_alert, mut needs_escalation_check, mut needs_resolution) = {
            let mut alert_state = self.alert_state.write();
            let state = alert_state
                .entry(rule.id.clone())
                .or_insert_with(|| AlertState {
                    rule_id: rule.id.clone(),
                    first_triggered: now,
                    last_evaluated: now,
                    current_alert: None,
                    escalation_level: 0,
                    last_notification: None,
                });

            state.last_evaluated = now;

            let needs_new_alert = should_trigger && state.current_alert.is_none();
            let needs_escalation_check = should_trigger && state.current_alert.is_some();
            let needs_resolution = !should_trigger && state.current_alert.is_some() 
                && matches!(state.current_alert.as_ref().unwrap().status, AlertStatus::Firing | AlertStatus::Acknowledged);

            (needs_new_alert, needs_escalation_check, needs_resolution)
        };

        if needs_new_alert {
            // Create new alert
            let alert = Alert {
                id: uuid::Uuid::new_v4().to_string(),
                rule_id: rule.id.clone(),
                name: rule.name.clone(),
                description: rule.description.clone(),
                severity: rule.severity.clone(),
                status: AlertStatus::Firing,
                triggered_at: chrono::Utc::now(),
                resolved_at: None,
                acknowledged_at: None,
                acknowledged_by: None,
                current_value: self.get_current_value(&rule.condition, snapshot),
                threshold: self.get_threshold_value(&rule.condition),
                labels: HashMap::new(),
            };

            // Update state
            {
                let mut alert_state = self.alert_state.write();
                if let Some(state) = alert_state.get_mut(&rule.id) {
                    state.current_alert = Some(alert.clone());
                    state.first_triggered = now;
                    state.escalation_level = 0;
                    state.last_notification = Some(now);
                }
            }

            self.send_notification(&alert, NotificationType::AlertTriggered, &rule.notification_channels).await?;
        } else if needs_escalation_check {
            self.check_escalation_async(rule, now).await?;
        } else if needs_resolution {
            // Resolve alert
            let alert_to_resolve = {
                let mut alert_state = self.alert_state.write();
                if let Some(state) = alert_state.get_mut(&rule.id) {
                    if let Some(ref mut alert) = state.current_alert {
                        alert.status = AlertStatus::Resolved;
                        alert.resolved_at = Some(chrono::Utc::now());
                        let alert_clone = alert.clone();
                        state.current_alert = None;
                        Some(alert_clone)
                    } else {
                        None
                    }
                } else {
                    None
                }
            };

            if let Some(alert) = alert_to_resolve {
                self.send_notification(&alert, NotificationType::AlertResolved, &rule.notification_channels).await?;
            }
        }

        Ok(())
    }

    async fn check_escalation_async(
        &self,
        rule: &AlertRule,
        now: Instant,
    ) -> Result<(), ApiTestError> {
        let escalation_info = {
            let mut alert_state = self.alert_state.write();
            if let Some(state) = alert_state.get_mut(&rule.id) {
                if let Some(alert) = &state.current_alert {
                    if matches!(alert.status, AlertStatus::Acknowledged) {
                        None
                    } else {
                        let mut result = None;
                        for (level, escalation_rule) in rule.escalation_rules.iter().enumerate() {
                            if level <= state.escalation_level {
                                continue;
                            }

                            let time_since_triggered = now.duration_since(state.first_triggered);
                            if time_since_triggered >= escalation_rule.delay {
                                state.escalation_level = level;
                                state.last_notification = Some(now);
                                result = Some((alert.clone(), escalation_rule.notification_channels.clone()));
                                break;
                            }
                        }
                        result
                    }
                } else {
                    None
                }
            } else {
                None
            }
        };

        if let Some((alert, channels)) = escalation_info {
            self.send_notification(
                &alert,
                NotificationType::AlertEscalated,
                &channels,
            ).await?;
        }

        Ok(())
    }

    fn get_current_value(&self, condition: &AlertCondition, snapshot: &MetricsSnapshot) -> f64 {
        match condition {
            AlertCondition::ErrorRate { .. } => snapshot.performance.error_rate,
            AlertCondition::ResponseTime { percentile, .. } => {
                (match percentile {
                    95 => snapshot.performance.p95_response_time,
                    99 => snapshot.performance.p99_response_time,
                    _ => snapshot.performance.average_response_time,
                }) * 1000.0
            }
            AlertCondition::ResourceUsage { resource_type, .. } => {
                match resource_type {
                    ResourceType::Cpu => snapshot.resource_usage.cpu_usage_percent,
                    ResourceType::Memory => snapshot.resource_usage.memory_usage_percent,
                    ResourceType::Network => 0.0,
                }
            }
            AlertCondition::TestFailureRate { .. } => 100.0 - snapshot.test_execution.success_rate,
            AlertCondition::MetricThreshold { .. } => snapshot.performance.total_requests as f64,
        }
    }

    fn get_threshold_value(&self, condition: &AlertCondition) -> f64 {
        match condition {
            AlertCondition::ErrorRate { threshold_percent, .. } => *threshold_percent,
            AlertCondition::ResponseTime { threshold_ms, .. } => *threshold_ms,
            AlertCondition::ResourceUsage { threshold_percent, .. } => *threshold_percent,
            AlertCondition::TestFailureRate { threshold_percent, .. } => *threshold_percent,
            AlertCondition::MetricThreshold { threshold, .. } => *threshold,
        }
    }

    async fn send_notification(
        &self,
        alert: &Alert,
        notification_type: NotificationType,
        channels: &[String],
    ) -> Result<(), ApiTestError> {
        let message = self.format_alert_message(alert, &notification_type);
        let notification = Notification {
            alert: alert.clone(),
            message,
            timestamp: chrono::Utc::now(),
            notification_type,
        };

        let channels_to_use = if channels.is_empty() {
            &self.config.default_notification_channels
        } else {
            channels
        };

        for channel_name in channels_to_use {
            if let Err(e) = self.notification_manager.send_notification(channel_name, &notification).await {
                log::error!("Failed to send notification to channel {}: {}", channel_name, e);
            }
        }

        Ok(())
    }

    fn format_alert_message(&self, alert: &Alert, notification_type: &NotificationType) -> String {
        match notification_type {
            NotificationType::AlertTriggered => {
                format!(
                    "🚨 ALERT TRIGGERED: {} ({})\n{}\nCurrent: {:.2}, Threshold: {:.2}",
                    alert.name,
                    format!("{:?}", alert.severity),
                    alert.description,
                    alert.current_value,
                    alert.threshold
                )
            }
            NotificationType::AlertResolved => {
                format!("✅ ALERT RESOLVED: {} ({})", alert.name, format!("{:?}", alert.severity))
            }
            NotificationType::AlertEscalated => {
                format!("⚠️ ALERT ESCALATED: {} ({})", alert.name, format!("{:?}", alert.severity))
            }
            NotificationType::AlertAcknowledged => {
                format!(
                    "👍 ALERT ACKNOWLEDGED: {} ({}) by {}",
                    alert.name,
                    format!("{:?}", alert.severity),
                    alert.acknowledged_by.as_deref().unwrap_or("Unknown")
                )
            }
        }
    }
}

impl NotificationManager {
    pub fn new() -> Self {
        let mut channels: HashMap<String, Box<dyn NotificationChannel>> = HashMap::new();
        channels.insert("console".to_string(), Box::new(ConsoleNotificationChannel));

        Self {
            channels: Arc::new(RwLock::new(channels)),
        }
    }

    pub fn add_email_channel(&self, name: String, config: EmailConfig) {
        let mut channels = self.channels.write();
        channels.insert(name, Box::new(EmailNotificationChannel { smtp_config: config }));
    }

    pub fn add_slack_channel(&self, name: String, webhook_url: String, channel: String) {
        let mut channels = self.channels.write();
        channels.insert(name, Box::new(SlackNotificationChannel { webhook_url, channel }));
    }

    pub fn add_webhook_channel(&self, name: String, url: String, headers: HashMap<String, String>) {
        let mut channels = self.channels.write();
        channels.insert(name, Box::new(WebhookNotificationChannel { url, headers }));
    }

    pub async fn send_notification(
        &self,
        channel_name: &str,
        notification: &Notification,
    ) -> Result<(), ApiTestError> {
        let channel_available = {
            let channels = self.channels.read();
            channels.get(channel_name).map(|c| c.is_available()).unwrap_or(false)
        };

        if !channel_available {
            return Err(ApiTestError::Configuration(format!(
                "Notification channel '{}' is not available or not found",
                channel_name
            )));
        }

        // Clone the channel to avoid holding the lock across await
        let channel = {
            let channels = self.channels.read();
            channels.get(channel_name).cloned()
        };

        if let Some(channel) = channel {
            channel.send_notification(notification).await
        } else {
            Err(ApiTestError::Configuration(format!(
                "Notification channel '{}' not found",
                channel_name
            )))
        }
    }

    pub fn list_channels(&self) -> Vec<String> {
        self.channels.read().keys().cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metrics::MetricsConfig;
    use std::time::Duration;

    #[tokio::test]
    async fn test_alerting_system_creation() {
        let metrics_config = MetricsConfig::default();
        let metrics_collector = Arc::new(MetricsCollector::new(metrics_config).unwrap());
        let alerting_config = AlertingConfig::default();
        
        let alerting_system = AlertingSystem::new(alerting_config, metrics_collector).unwrap();
        
        assert_eq!(alerting_system.get_rules().len(), 0);
        assert_eq!(alerting_system.get_active_alerts().len(), 0);
    }

    #[tokio::test]
    async fn test_alert_rule_management() {
        let metrics_config = MetricsConfig::default();
        let metrics_collector = Arc::new(MetricsCollector::new(metrics_config).unwrap());
        let alerting_config = AlertingConfig::default();
        
        let alerting_system = AlertingSystem::new(alerting_config, metrics_collector).unwrap();
        
        let rule = AlertRule {
            id: "test-rule".to_string(),
            name: "Test Rule".to_string(),
            description: "Test alert rule".to_string(),
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

        alerting_system.add_rule(rule);
        assert_eq!(alerting_system.get_rules().len(), 1);

        assert!(alerting_system.remove_rule("test-rule"));
        assert_eq!(alerting_system.get_rules().len(), 0);
    }

    #[test]
    fn test_notification_manager() {
        let manager = NotificationManager::new();
        let channels = manager.list_channels();
        
        assert!(channels.contains(&"console".to_string()));
        
        manager.add_slack_channel(
            "test-slack".to_string(),
            "https://hooks.slack.com/test".to_string(),
            "#alerts".to_string(),
        );
        
        let channels = manager.list_channels();
        assert!(channels.contains(&"test-slack".to_string()));
    }
}