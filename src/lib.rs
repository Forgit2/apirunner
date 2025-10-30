//! API Test Runner - A comprehensive testing framework for APIs
//! 
//! This crate provides a plugin-based architecture for testing various API protocols
//! with support for multiple execution modes, data-driven testing, and comprehensive reporting.

pub mod error;
pub mod plugin;
pub mod plugin_manager;
pub mod event;
pub mod event_persistence;
pub mod event_transaction;
pub mod execution;
pub mod distributed_execution;
pub mod rest_api_plugin;
pub mod data_source;
pub mod json_assertions;
pub mod xml_assertions;
pub mod cross_system_validations;
pub mod workflow_validation;
pub mod auth_plugin;
pub mod oauth2_auth_plugin;
pub mod jwt_auth_plugin;
pub mod basic_auth_plugin;
pub mod auth_chain_executor;
pub mod reporting;
pub mod configuration;
pub mod metrics;
pub mod alerting;
pub mod metrics_history;
pub mod test_case_manager;
pub mod storage;
pub mod import_export;
pub mod interactive_execution;
pub mod real_time_monitoring;
pub mod result_analysis;
pub mod result_browser_cli;
pub mod template_manager;
pub mod template_storage;
pub mod test_case_linter;
pub mod test_case_documentation;
pub mod test_quality_metrics;
pub mod cli;
pub mod cli_handler;
pub mod app;

// Re-export commonly used types
pub use error::{ApiTestError, PluginError, ProtocolError, AssertionError, DataSourceError, ExecutionError, TestCaseError, Result};
pub use plugin::{
    Plugin, PluginConfig,
    ProtocolPlugin, ProtocolRequest, ProtocolResponse, ProtocolVersion, HttpMethod, ConnectionConfig, TlsConfig,
    AssertionPlugin, AssertionType, AssertionResult, ExpectedValue, ResponseData,
    DataSourcePlugin, DataSourceType, DataSourceConfig, DataRecord, DataTransformation,
};
pub use plugin_manager::{
    PluginManager, PluginManagerConfig, PluginMetadata, PluginEntry, PluginType, PluginStatus,
};
pub use event::{
    Event, EventBus, EventSubscription, EventFilter, EventMetadata, EventError,
    TestLifecycleEvent, TestLifecycleType, RequestExecutionEvent, RequestExecutionType,
    AssertionResultEvent, PluginLifecycleEvent, PluginLifecycleType, SystemMetricsEvent, ErrorEvent,
};
pub use event_persistence::{EventPersistence, SledEventPersistence, InMemoryEventPersistence};
pub use event_transaction::{
    EventTransaction, EventTransactionManager, TransactionalEventHandler, TransactionRecoveryHandler,
    TransactionState, TestExecutionTransactionHandler, LoggingRecoveryHandler,
};
pub use execution::{
    ExecutionContext, ExecutionStrategy, ExecutionResult, TestResult,
    SerialExecutor, ParallelExecutor, RateLimiter, AuthToken,
};
pub use test_case_manager::{
    TestCase, RequestDefinition, AssertionDefinition, VariableExtraction, AuthDefinition, ExtractionSource,
    ChangeLogEntry, ChangeType, TestCaseVersionHistory, TestCaseMetadata, TestCaseStatistics,
    TestCaseFilter, SearchOptions, SortField, SortOrder, TestCaseFormat, TestCaseManager, 
    TestCaseValidator, StorageBackend,
};
pub use distributed_execution::{
    DistributedExecutor, ExecutionNode, NodeCapabilities, NodeStatus, TaskAssignment,
    DistributedTaskResult, NodeDiscovery, TaskDistributor, FaultToleranceManager,
};
pub use rest_api_plugin::{
    RestApiPlugin, RetryConfig, HttpStatusAssertion, HttpHeaderAssertion, ResponseTimeAssertion,
};
pub use json_assertions::{
    JsonPathAssertion, JsonSchemaAssertion, JsonStructureAssertion,
};
pub use xml_assertions::{
    XPathAssertion, XmlSchemaAssertion, NamespaceAwareXmlAssertion,
};
pub use cross_system_validations::{
    DatabaseValidation, MessageQueueValidation, CacheValidation,
    DatabaseType as CrossSystemDatabaseType, QueueType, CacheType,
};
pub use workflow_validation::{
    WorkflowValidation, WorkflowStep, WorkflowAssertion, StateExtraction, ExtractionType,
    WorkflowState, StepResult,
};
pub use data_source::{
    CsvDataSource, CsvDataSourceConfig, JsonDataSource, JsonDataSourceConfig,
    YamlDataSource, YamlDataSourceConfig, DatabaseDataSource, DatabaseDataSourceConfig,
    DatabaseType, FileDataSourceConfig, DataTransformationPipeline, TransformationStage,
    FieldMappingTransform, ValueSubstitutionTransform, TypeConversionTransform,
    FieldFilterTransform, ValueValidationTransform, DataType, ValidationRule,
};
pub use auth_plugin::{
    AuthPlugin, AuthConfig, AuthResult, AuthFailure, AuthToken as AuthPluginToken, AuthChainConfig, AuthStepConfig,
    FailureStrategy, RetryConfig as AuthRetryConfig, AuthChainResult, AuthManager,
};
pub use oauth2_auth_plugin::OAuth2AuthPlugin;
pub use jwt_auth_plugin::JwtAuthPlugin;
pub use basic_auth_plugin::{BasicAuthPlugin, ApiKeyAuthPlugin};
pub use auth_chain_executor::{AuthChainExecutor, ExecutionAttempt, StepExecution, ExecutionOutcome};
pub use reporting::{
    Reporter, ReportFormat, ReportConfig, ReportOutput, ReportManager, ReportingError,
    TestExecutionResults, TestResult as ReportingTestResult, TestStatus, EnvironmentInfo,
    ExecutionSummary, PerformanceMetrics, RequestDetails, ResponseDetails, AssertionResult as ReportingAssertionResult,
};
pub use configuration::{
    Configuration, ConfigurationManager, ConfigurationError, ConfigurationEvent,
    ExecutionConfig, ConfigPluginConfig, ReportingConfig, ConfigAuthConfig, ConfigDataSourceConfig, EnvironmentConfig,
    RetryConfig as ConfigRetryConfig, RateLimitConfig,
};
pub use metrics::{
    MetricsCollector, MetricsConfig, MetricsSnapshot, ResourceUsageMetrics,
    TestExecutionMetrics, MetricsTimer,
};
pub use alerting::{
    AlertingSystem, AlertingConfig, AlertRule, AlertCondition, AlertSeverity,
    NotificationManager, NotificationChannel, Notification, Alert, AlertStatus,
    ComparisonOperator, ResourceType, EscalationRule, NotificationType,
    ConsoleNotificationChannel, EmailNotificationChannel, SlackNotificationChannel,
    WebhookNotificationChannel, EmailConfig,
};
pub use metrics_history::{
    MetricsHistory, MetricsHistoryConfig, TimestampedSnapshot, MetricBaseline,
    TrendAnalysis, TimeRange, TrendDirection, AnomalyPoint, AnomalyType,
    PerformanceComparison, MetricSummary, ComparisonType,
};
pub use storage::{FileSystemStorage};
pub use import_export::{
    ImportExportManager, TestCaseImporter, TestCaseExporter, ImportFormat, ExportFormat,
    ImportSource, ImportOptions, ExportOptions,
};
pub use interactive_execution::{
    InteractiveExecutor, DebugOptions, InteractiveExecutionResult, StepExecutionResult,
    StepType, ValidationError, ValidationErrorType, VariableInspector, ProgressReporter,
};
pub use real_time_monitoring::{
    RealTimeMonitor, ExecutionTracker, ExecutionProgress, ExecutionStatus, CancellationManager,
    ResultStreamer, WatchManager,
};
pub use result_analysis::{
    ResultAnalyzer, AnalyzableResult, ResultStatus, FailureAnalysis, FailureType,
    ComparisonEngine, ComparisonResult, ResultBrowser, ResultFilter, ResultSort,
};
pub use result_browser_cli::{
    ResultBrowserCli, ResultBrowserCliHandler, BrowserCommand, ListArgs, ShowArgs, CompareArgs,
};
pub use template_manager::{
    TestCaseTemplateManager, TestCaseTemplate, TestCaseDefinition, TemplateCategory,
    TemplateVariable, VariableType, TestCaseFragment, FragmentType, FragmentContent,
    TemplateFilter, TemplateCreationOptions, TemplateStorage,
};
pub use template_storage::FileTemplateStorage;
pub use test_case_linter::{
    TestCaseLinter, LinterConfig, ValidationRule as LinterValidationRule, ValidationSeverity, ValidationCategory,
    ValidationIssue, ValidationLocation, ValidationResult, LintingReport, ValidationRuleEngine,
};
pub use test_quality_metrics::{
    TestQualityAnalyzer, TestQualityMetrics, CoverageMetrics, MaintenanceMetrics, CollaborationMetrics,
    QualityMetricsConfig, QualityMetricsReporter, MaintenanceRecommendation, MaintenanceCategory,
    RecommendationPriority, EstimatedEffort,
};
// pub use test_case_documentation::{
//     TestCaseDocumentationGenerator, DocumentationConfig, DocumentationFormat, GroupingStrategy,
//     TestCaseDocumentation, DocumentationReport, DocumentationStatistics, DocumentationGenerator,
// };

/// Version information for the API Test Runner
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const NAME: &str = env!("CARGO_PKG_NAME");

/// Initialize the API Test Runner library
pub fn init() -> Result<()> {
    // Future initialization logic will go here
    Ok(())
}