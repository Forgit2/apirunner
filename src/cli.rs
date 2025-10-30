use clap::{Parser, Subcommand, Args};
use std::path::PathBuf;
use serde::{Deserialize, Serialize};

/// API Test Runner - A comprehensive testing framework for APIs
#[derive(Parser)]
#[command(name = "apirunner")]
#[command(about = "A comprehensive testing framework for APIs")]
#[command(version = crate::VERSION)]
pub struct Cli {
    /// Enable verbose output
    #[arg(short, long, global = true)]
    pub verbose: bool,

    /// Configuration file path
    #[arg(short, long, global = true)]
    pub config: Option<PathBuf>,

    /// Environment to use
    #[arg(short, long, global = true)]
    pub environment: Option<String>,

    /// Output format
    #[arg(long, global = true, value_enum)]
    pub output_format: Option<OutputFormat>,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Execute test cases
    Run(RunArgs),
    
    /// Manage test cases
    TestCase(TestCaseArgs),
    
    /// Manage plugins
    Plugin(PluginArgs),
    
    /// Interactive execution and debugging
    Interactive(InteractiveArgs),
    
    /// Generate reports
    Report(ReportArgs),
    
    /// Configuration management
    Config(ConfigArgs),
    
    /// Template management
    Template(TemplateArgs),
    
    /// Import/Export operations
    ImportExport(ImportExportArgs),
    
    /// Quality metrics and analysis
    Quality(QualityArgs),
    
    /// Browse and analyze results
    Browse(BrowseArgs),
}

#[derive(Args)]
pub struct RunArgs {
    /// Test case files or directories to execute
    #[arg(required = true)]
    pub targets: Vec<PathBuf>,

    /// Execution strategy
    #[arg(short, long, value_enum, default_value = "serial")]
    pub strategy: ExecutionStrategy,

    /// Number of parallel workers (for parallel execution)
    #[arg(short = 'j', long)]
    pub workers: Option<usize>,

    /// Rate limit (requests per second)
    #[arg(short, long)]
    pub rate_limit: Option<f64>,

    /// Timeout for individual test cases (in seconds)
    #[arg(short, long)]
    pub timeout: Option<u64>,

    /// Filter tests by tag
    #[arg(long)]
    pub tags: Vec<String>,

    /// Exclude tests by tag
    #[arg(long)]
    pub exclude_tags: Vec<String>,

    /// Continue execution on failure
    #[arg(long)]
    pub continue_on_failure: bool,

    /// Dry run (validate without executing)
    #[arg(long)]
    pub dry_run: bool,

    /// Output directory for reports
    #[arg(short, long)]
    pub output: Option<PathBuf>,

    /// Report formats to generate
    #[arg(long, value_enum)]
    pub report_formats: Vec<ReportFormat>,

    /// Enable real-time monitoring
    #[arg(long)]
    pub monitor: bool,

    /// Watch mode (re-run on file changes)
    #[arg(short, long)]
    pub watch: bool,
}

#[derive(Args)]
pub struct TestCaseArgs {
    #[command(subcommand)]
    pub command: TestCaseCommands,
}

#[derive(Subcommand)]
pub enum TestCaseCommands {
    /// Create a new test case
    Create(CreateTestCaseArgs),
    
    /// List test cases
    List(ListTestCaseArgs),
    
    /// Show test case details
    Show(ShowTestCaseArgs),
    
    /// Update a test case
    Update(UpdateTestCaseArgs),
    
    /// Delete a test case
    Delete(DeleteTestCaseArgs),
    
    /// Search test cases
    Search(SearchTestCaseArgs),
    
    /// Validate test cases
    Validate(ValidateTestCaseArgs),
    
    /// Generate documentation
    Document(DocumentTestCaseArgs),
}

#[derive(Args)]
pub struct CreateTestCaseArgs {
    /// Test case name
    #[arg(short, long)]
    pub name: String,

    /// Test case description
    #[arg(short, long)]
    pub description: Option<String>,

    /// Output file path
    #[arg(short, long)]
    pub output: Option<PathBuf>,

    /// Template to use
    #[arg(short, long)]
    pub template: Option<String>,

    /// Template variables (key=value format)
    #[arg(long)]
    pub variables: Vec<String>,

    /// Interactive mode
    #[arg(short, long)]
    pub interactive: bool,
}

#[derive(Args)]
pub struct ListTestCaseArgs {
    /// Directory to search
    #[arg(default_value = ".")]
    pub directory: PathBuf,

    /// Filter by tags
    #[arg(long)]
    pub tags: Vec<String>,

    /// Filter by protocol
    #[arg(long)]
    pub protocol: Option<String>,

    /// Show detailed information
    #[arg(short, long)]
    pub detailed: bool,

    /// Output format
    #[arg(long, value_enum)]
    pub format: Option<OutputFormat>,
}

#[derive(Args)]
pub struct ShowTestCaseArgs {
    /// Test case file path
    pub path: PathBuf,

    /// Show execution history
    #[arg(long)]
    pub history: bool,

    /// Show validation results
    #[arg(long)]
    pub validate: bool,
}

#[derive(Args)]
pub struct UpdateTestCaseArgs {
    /// Test case file path
    pub path: PathBuf,

    /// New name
    #[arg(long)]
    pub name: Option<String>,

    /// New description
    #[arg(long)]
    pub description: Option<String>,

    /// Add tags
    #[arg(long)]
    pub add_tags: Vec<String>,

    /// Remove tags
    #[arg(long)]
    pub remove_tags: Vec<String>,

    /// Interactive mode
    #[arg(short, long)]
    pub interactive: bool,
}

#[derive(Args)]
pub struct DeleteTestCaseArgs {
    /// Test case file paths
    pub paths: Vec<PathBuf>,

    /// Force deletion without confirmation
    #[arg(short, long)]
    pub force: bool,

    /// Delete recursively
    #[arg(short, long)]
    pub recursive: bool,
}

#[derive(Args)]
pub struct SearchTestCaseArgs {
    /// Search query
    pub query: String,

    /// Search in directory
    #[arg(short, long, default_value = ".")]
    pub directory: PathBuf,

    /// Search fields
    #[arg(long, value_enum)]
    pub fields: Vec<SearchField>,

    /// Case sensitive search
    #[arg(long)]
    pub case_sensitive: bool,

    /// Use regex patterns
    #[arg(long)]
    pub regex: bool,

    /// Maximum results
    #[arg(short, long)]
    pub limit: Option<usize>,
}

#[derive(Args)]
pub struct ValidateTestCaseArgs {
    /// Test case files or directories
    pub targets: Vec<PathBuf>,

    /// Validation rules file
    #[arg(long)]
    pub rules: Option<PathBuf>,

    /// Severity level to report
    #[arg(long, value_enum, default_value = "warning")]
    pub severity: ValidationSeverity,

    /// Fix issues automatically
    #[arg(long)]
    pub fix: bool,
}

#[derive(Args)]
pub struct DocumentTestCaseArgs {
    /// Test case files or directories
    pub targets: Vec<PathBuf>,

    /// Output file path
    #[arg(short, long)]
    pub output: Option<PathBuf>,

    /// Documentation format
    #[arg(short, long, value_enum, default_value = "html")]
    pub format: DocumentationFormat,

    /// Include execution history
    #[arg(long)]
    pub include_history: bool,

    /// Group by category
    #[arg(long, value_enum)]
    pub group_by: Option<GroupingStrategy>,
}

#[derive(Args)]
pub struct PluginArgs {
    #[command(subcommand)]
    pub command: PluginCommands,
}

#[derive(Subcommand)]
pub enum PluginCommands {
    /// List installed plugins
    List(ListPluginArgs),
    
    /// Install a plugin
    Install(InstallPluginArgs),
    
    /// Uninstall a plugin
    Uninstall(UninstallPluginArgs),
    
    /// Update plugins
    Update(UpdatePluginArgs),
    
    /// Show plugin information
    Info(InfoPluginArgs),
    
    /// Enable/disable plugins
    Toggle(TogglePluginArgs),
    
    /// Reload plugins
    Reload(ReloadPluginArgs),
}

#[derive(Args)]
pub struct ListPluginArgs {
    /// Show detailed information
    #[arg(short, long)]
    pub detailed: bool,

    /// Filter by plugin type
    #[arg(long, value_enum)]
    pub plugin_type: Option<PluginType>,

    /// Show only enabled plugins
    #[arg(long)]
    pub enabled_only: bool,
}

#[derive(Args)]
pub struct InstallPluginArgs {
    /// Plugin path or URL
    pub source: String,

    /// Force installation
    #[arg(short, long)]
    pub force: bool,

    /// Skip dependency check
    #[arg(long)]
    pub skip_deps: bool,
}

#[derive(Args)]
pub struct UninstallPluginArgs {
    /// Plugin names
    pub plugins: Vec<String>,

    /// Force uninstallation
    #[arg(short, long)]
    pub force: bool,

    /// Remove dependencies
    #[arg(long)]
    pub remove_deps: bool,
}

#[derive(Args)]
pub struct UpdatePluginArgs {
    /// Plugin names (empty for all)
    pub plugins: Vec<String>,

    /// Check for updates only
    #[arg(long)]
    pub check_only: bool,
}

#[derive(Args)]
pub struct InfoPluginArgs {
    /// Plugin name
    pub plugin: String,

    /// Show configuration
    #[arg(long)]
    pub config: bool,

    /// Show dependencies
    #[arg(long)]
    pub deps: bool,
}

#[derive(Args)]
pub struct TogglePluginArgs {
    /// Plugin name
    pub plugin: String,

    /// Enable the plugin
    #[arg(long)]
    pub enable: bool,

    /// Disable the plugin
    #[arg(long)]
    pub disable: bool,
}

#[derive(Args)]
pub struct ReloadPluginArgs {
    /// Plugin names (empty for all)
    pub plugins: Vec<String>,

    /// Force reload
    #[arg(short, long)]
    pub force: bool,
}

#[derive(Args)]
pub struct InteractiveArgs {
    /// Test case file
    pub test_case: Option<PathBuf>,

    /// Enable step-by-step execution
    #[arg(long)]
    pub step_through: bool,

    /// Enable variable inspection
    #[arg(long)]
    pub inspect_variables: bool,

    /// Set breakpoints (step numbers)
    #[arg(long)]
    pub breakpoints: Vec<usize>,

    /// Enable verbose output
    #[arg(long)]
    pub verbose: bool,
}

#[derive(Args)]
pub struct ReportArgs {
    #[command(subcommand)]
    pub command: ReportCommands,
}

#[derive(Subcommand)]
pub enum ReportCommands {
    /// Generate reports from execution results
    Generate(GenerateReportArgs),
    
    /// List available reports
    List(ListReportArgs),
    
    /// Show report details
    Show(ShowReportArgs),
    
    /// Compare reports
    Compare(CompareReportArgs),
}

#[derive(Args)]
pub struct GenerateReportArgs {
    /// Input results file or directory
    pub input: PathBuf,

    /// Output directory
    #[arg(short, long)]
    pub output: Option<PathBuf>,

    /// Report formats
    #[arg(short, long, value_enum)]
    pub formats: Vec<ReportFormat>,

    /// Report template
    #[arg(long)]
    pub template: Option<String>,

    /// Include performance metrics
    #[arg(long)]
    pub include_metrics: bool,

    /// Include failure analysis
    #[arg(long)]
    pub include_analysis: bool,
}

#[derive(Args)]
pub struct ListReportArgs {
    /// Reports directory
    #[arg(default_value = "reports")]
    pub directory: PathBuf,

    /// Show detailed information
    #[arg(short, long)]
    pub detailed: bool,
}

#[derive(Args)]
pub struct ShowReportArgs {
    /// Report file path
    pub path: PathBuf,

    /// Open in browser (for HTML reports)
    #[arg(long)]
    pub browser: bool,
}

#[derive(Args)]
pub struct CompareReportArgs {
    /// First report file
    pub report1: PathBuf,

    /// Second report file
    pub report2: PathBuf,

    /// Output file for comparison
    #[arg(short, long)]
    pub output: Option<PathBuf>,

    /// Comparison format
    #[arg(short, long, value_enum, default_value = "html")]
    pub format: ReportFormat,
}

#[derive(Args)]
pub struct ConfigArgs {
    #[command(subcommand)]
    pub command: ConfigCommands,
}

#[derive(Subcommand)]
pub enum ConfigCommands {
    /// Show current configuration
    Show(ShowConfigArgs),
    
    /// Set configuration value
    Set(SetConfigArgs),
    
    /// Get configuration value
    Get(GetConfigArgs),
    
    /// Validate configuration
    Validate(ValidateConfigArgs),
    
    /// Reset configuration
    Reset(ResetConfigArgs),
    
    /// Export configuration
    Export(ExportConfigArgs),
    
    /// Import configuration
    Import(ImportConfigArgs),
}

#[derive(Args)]
pub struct ShowConfigArgs {
    /// Configuration section
    pub section: Option<String>,

    /// Show sensitive values
    #[arg(long)]
    pub show_sensitive: bool,

    /// Output format
    #[arg(short, long, value_enum)]
    pub format: Option<OutputFormat>,
}

#[derive(Args)]
pub struct SetConfigArgs {
    /// Configuration key
    pub key: String,

    /// Configuration value
    pub value: String,

    /// Configuration section
    #[arg(short, long)]
    pub section: Option<String>,

    /// Mark as sensitive
    #[arg(long)]
    pub sensitive: bool,
}

#[derive(Args)]
pub struct GetConfigArgs {
    /// Configuration key
    pub key: String,

    /// Configuration section
    #[arg(short, long)]
    pub section: Option<String>,

    /// Show default value if not set
    #[arg(long)]
    pub show_default: bool,
}

#[derive(Args)]
pub struct ValidateConfigArgs {
    /// Configuration file
    pub file: Option<PathBuf>,

    /// Strict validation
    #[arg(long)]
    pub strict: bool,
}

#[derive(Args)]
pub struct ResetConfigArgs {
    /// Configuration section to reset
    pub section: Option<String>,

    /// Force reset without confirmation
    #[arg(short, long)]
    pub force: bool,
}

#[derive(Args)]
pub struct ExportConfigArgs {
    /// Output file
    #[arg(short, long)]
    pub output: Option<PathBuf>,

    /// Export format
    #[arg(short, long, value_enum, default_value = "yaml")]
    pub format: ConfigFormat,

    /// Include sensitive values
    #[arg(long)]
    pub include_sensitive: bool,
}

#[derive(Args)]
pub struct ImportConfigArgs {
    /// Input file
    pub input: PathBuf,

    /// Merge with existing configuration
    #[arg(short, long)]
    pub merge: bool,

    /// Force import without confirmation
    #[arg(short, long)]
    pub force: bool,
}

#[derive(Args)]
pub struct TemplateArgs {
    #[command(subcommand)]
    pub command: TemplateCommands,
}

#[derive(Subcommand)]
pub enum TemplateCommands {
    /// List available templates
    List(ListTemplateArgs),
    
    /// Show template details
    Show(ShowTemplateArgs),
    
    /// Create a new template
    Create(CreateTemplateArgs),
    
    /// Update a template
    Update(UpdateTemplateArgs),
    
    /// Delete a template
    Delete(DeleteTemplateArgs),
    
    /// Validate templates
    Validate(ValidateTemplateArgs),
}

#[derive(Args)]
pub struct ListTemplateArgs {
    /// Filter by category
    #[arg(long, value_enum)]
    pub category: Option<TemplateCategory>,

    /// Show detailed information
    #[arg(short, long)]
    pub detailed: bool,

    /// Include custom templates
    #[arg(long)]
    pub include_custom: bool,
}

#[derive(Args)]
pub struct ShowTemplateArgs {
    /// Template ID
    pub template: String,

    /// Show variables
    #[arg(long)]
    pub variables: bool,

    /// Show instructions
    #[arg(long)]
    pub instructions: bool,
}

#[derive(Args)]
pub struct CreateTemplateArgs {
    /// Template name
    #[arg(short, long)]
    pub name: String,

    /// Template description
    #[arg(short, long)]
    pub description: String,

    /// Template category
    #[arg(short, long, value_enum)]
    pub category: TemplateCategory,

    /// Source test case file
    #[arg(short, long)]
    pub source: Option<PathBuf>,

    /// Interactive mode
    #[arg(short, long)]
    pub interactive: bool,
}

#[derive(Args)]
pub struct UpdateTemplateArgs {
    /// Template ID
    pub template: String,

    /// New name
    #[arg(long)]
    pub name: Option<String>,

    /// New description
    #[arg(long)]
    pub description: Option<String>,

    /// New category
    #[arg(long, value_enum)]
    pub category: Option<TemplateCategory>,

    /// Interactive mode
    #[arg(short, long)]
    pub interactive: bool,
}

#[derive(Args)]
pub struct DeleteTemplateArgs {
    /// Template IDs
    pub templates: Vec<String>,

    /// Force deletion without confirmation
    #[arg(short, long)]
    pub force: bool,
}

#[derive(Args)]
pub struct ValidateTemplateArgs {
    /// Template IDs (empty for all)
    pub templates: Vec<String>,

    /// Fix issues automatically
    #[arg(long)]
    pub fix: bool,
}

#[derive(Args)]
pub struct ImportExportArgs {
    #[command(subcommand)]
    pub command: ImportExportCommands,
}

#[derive(Subcommand)]
pub enum ImportExportCommands {
    /// Import test cases
    Import(ImportArgs),
    
    /// Export test cases
    Export(ExportArgs),
}

#[derive(Args)]
pub struct ImportArgs {
    /// Source file or URL
    pub source: String,

    /// Import format
    #[arg(short, long, value_enum)]
    pub format: ImportFormat,

    /// Output directory
    #[arg(short, long)]
    pub output: Option<PathBuf>,

    /// Conversion options
    #[arg(long)]
    pub options: Vec<String>,

    /// Dry run (validate without importing)
    #[arg(long)]
    pub dry_run: bool,

    /// Force import (overwrite existing)
    #[arg(short, long)]
    pub force: bool,
}

#[derive(Args)]
pub struct ExportArgs {
    /// Source directory or files
    pub sources: Vec<PathBuf>,

    /// Export format
    #[arg(short, long, value_enum)]
    pub format: ExportFormat,

    /// Output file or directory
    #[arg(short, long)]
    pub output: Option<PathBuf>,

    /// Export options
    #[arg(long)]
    pub options: Vec<String>,

    /// Include metadata
    #[arg(long)]
    pub include_metadata: bool,
}

#[derive(Args)]
pub struct QualityArgs {
    #[command(subcommand)]
    pub command: QualityCommands,
}

#[derive(Subcommand)]
pub enum QualityCommands {
    /// Analyze test quality
    Analyze(AnalyzeQualityArgs),
    
    /// Generate quality report
    Report(QualityReportArgs),
    
    /// Show quality metrics
    Metrics(QualityMetricsArgs),
    
    /// Get maintenance recommendations
    Recommendations(RecommendationsArgs),
}

#[derive(Args)]
pub struct AnalyzeQualityArgs {
    /// Test case files or directories
    pub targets: Vec<PathBuf>,

    /// Output file
    #[arg(short, long)]
    pub output: Option<PathBuf>,

    /// Analysis depth
    #[arg(long, value_enum, default_value = "standard")]
    pub depth: AnalysisDepth,

    /// Include coverage analysis
    #[arg(long)]
    pub coverage: bool,

    /// Include maintenance metrics
    #[arg(long)]
    pub maintenance: bool,
}

#[derive(Args)]
pub struct QualityReportArgs {
    /// Analysis results file
    pub input: PathBuf,

    /// Output file
    #[arg(short, long)]
    pub output: Option<PathBuf>,

    /// Report format
    #[arg(short, long, value_enum, default_value = "html")]
    pub format: ReportFormat,

    /// Include recommendations
    #[arg(long)]
    pub recommendations: bool,
}

#[derive(Args)]
pub struct QualityMetricsArgs {
    /// Test case files or directories
    pub targets: Vec<PathBuf>,

    /// Metric types to show
    #[arg(long, value_enum)]
    pub metrics: Vec<MetricType>,

    /// Output format
    #[arg(short, long, value_enum)]
    pub format: Option<OutputFormat>,
}

#[derive(Args)]
pub struct RecommendationsArgs {
    /// Test case files or directories
    pub targets: Vec<PathBuf>,

    /// Priority level
    #[arg(short, long, value_enum)]
    pub priority: Option<RecommendationPriority>,

    /// Category filter
    #[arg(long, value_enum)]
    pub category: Option<MaintenanceCategory>,

    /// Show implementation details
    #[arg(long)]
    pub detailed: bool,
}

#[derive(Args)]
pub struct BrowseArgs {
    /// Results file or directory
    pub input: Option<PathBuf>,

    /// Filter by status
    #[arg(long, value_enum)]
    pub status: Option<ResultStatus>,

    /// Filter by test name pattern
    #[arg(long)]
    pub name_pattern: Option<String>,

    /// Show only failures
    #[arg(long)]
    pub failures_only: bool,

    /// Interactive mode
    #[arg(short, long)]
    pub interactive: bool,

    /// Comparison mode
    #[arg(long)]
    pub compare: Option<PathBuf>,
}

// Enums for CLI arguments
#[derive(clap::ValueEnum, Clone, Debug, Serialize, Deserialize)]
pub enum OutputFormat {
    Json,
    Yaml,
    Table,
    Csv,
}

#[derive(clap::ValueEnum, Clone, Debug, Serialize, Deserialize)]
pub enum ExecutionStrategy {
    Serial,
    Parallel,
    Distributed,
    Interactive,
}

#[derive(clap::ValueEnum, Clone, Debug, Serialize, Deserialize)]
pub enum ReportFormat {
    Html,
    Json,
    Xml,
    Junit,
    Markdown,
}

#[derive(clap::ValueEnum, Clone, Debug, Serialize, Deserialize)]
pub enum SearchField {
    Name,
    Description,
    Tags,
    Protocol,
    Url,
    All,
}

#[derive(clap::ValueEnum, Clone, Debug, Serialize, Deserialize)]
pub enum ValidationSeverity {
    Error,
    Warning,
    Info,
}

#[derive(clap::ValueEnum, Clone, Debug, Serialize, Deserialize)]
pub enum DocumentationFormat {
    Html,
    Markdown,
    Pdf,
}

#[derive(clap::ValueEnum, Clone, Debug, Serialize, Deserialize)]
pub enum GroupingStrategy {
    Protocol,
    Tags,
    Directory,
    None,
}

#[derive(clap::ValueEnum, Clone, Debug, Serialize, Deserialize)]
pub enum PluginType {
    Protocol,
    Assertion,
    DataSource,
    Auth,
    Reporter,
    Monitor,
}

#[derive(clap::ValueEnum, Clone, Debug, Serialize, Deserialize)]
pub enum ConfigFormat {
    Yaml,
    Json,
    Toml,
}

#[derive(clap::ValueEnum, Clone, Debug, Serialize, Deserialize)]
pub enum TemplateCategory {
    RestApi,
    GraphQL,
    Authentication,
    DataValidation,
    Performance,
    Integration,
    Custom,
}

#[derive(clap::ValueEnum, Clone, Debug, Serialize, Deserialize)]
pub enum ImportFormat {
    Postman,
    OpenApi,
    Insomnia,
    Swagger,
}

#[derive(clap::ValueEnum, Clone, Debug, Serialize, Deserialize)]
pub enum ExportFormat {
    Postman,
    OpenApi,
    Yaml,
    Json,
}

#[derive(clap::ValueEnum, Clone, Debug, Serialize, Deserialize)]
pub enum AnalysisDepth {
    Basic,
    Standard,
    Comprehensive,
}

#[derive(clap::ValueEnum, Clone, Debug, Serialize, Deserialize)]
pub enum MetricType {
    Coverage,
    Maintenance,
    Collaboration,
    Quality,
}

#[derive(clap::ValueEnum, Clone, Debug, Serialize, Deserialize)]
pub enum RecommendationPriority {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(clap::ValueEnum, Clone, Debug, Serialize, Deserialize)]
pub enum MaintenanceCategory {
    Structure,
    Assertions,
    DataSources,
    Authentication,
    Performance,
}

#[derive(clap::ValueEnum, Clone, Debug, Serialize, Deserialize)]
pub enum ResultStatus {
    Passed,
    Failed,
    Skipped,
    Error,
}