//! Command-line interface for interactive result browsing
//! 
//! This module provides a CLI interface for exploring test results with:
//! - Interactive result browsing with filtering and sorting
//! - Detailed result inspection and comparison
//! - Result export and reporting capabilities

use chrono::{DateTime, Utc};
use clap::{Args, Parser, Subcommand, ValueEnum};
use serde_json;
use std::collections::HashMap;
use std::io::{self, Write};
use std::sync::Arc;
use std::time::Duration;

use crate::error::Result;
use crate::result_analysis::{
    ResultAnalyzer, ResultBrowser, ResultFilter, ResultSort, SortField, SortDirection,
    ResultStatus, DisplayConfig, AnalyzableResult,
};

/// CLI for interactive result browsing
#[derive(Parser)]
#[command(name = "result-browser")]
#[command(about = "Interactive API test result browser")]
pub struct ResultBrowserCli {
    #[command(subcommand)]
    pub command: BrowserCommand,
}

/// Available browser commands
#[derive(Subcommand)]
pub enum BrowserCommand {
    /// List and browse test results
    List(ListArgs),
    /// Show detailed information about a specific result
    Show(ShowArgs),
    /// Compare two test results
    Compare(CompareArgs),
    /// Generate summary statistics
    Summary(SummaryArgs),
    /// Export results to various formats
    Export(ExportArgs),
    /// Interactive browsing mode
    Interactive(InteractiveArgs),
}

/// Arguments for listing results
#[derive(Args)]
pub struct ListArgs {
    /// Filter by result status
    #[arg(long, value_enum)]
    pub status: Option<CliResultStatus>,
    
    /// Filter by test name pattern
    #[arg(long)]
    pub name: Option<String>,
    
    /// Filter by execution time (start)
    #[arg(long)]
    pub from: Option<String>,
    
    /// Filter by execution time (end)
    #[arg(long)]
    pub to: Option<String>,
    
    /// Filter by minimum duration (ms)
    #[arg(long)]
    pub min_duration: Option<u64>,
    
    /// Filter by maximum duration (ms)
    #[arg(long)]
    pub max_duration: Option<u64>,
    
    /// Sort field
    #[arg(long, value_enum, default_value = "execution-time")]
    pub sort: CliSortField,
    
    /// Sort direction
    #[arg(long, value_enum, default_value = "desc")]
    pub order: CliSortDirection,
    
    /// Maximum number of results to show
    #[arg(long, default_value = "50")]
    pub limit: usize,
    
    /// Show only results with failure analysis
    #[arg(long)]
    pub failures_only: bool,
}

/// Arguments for showing result details
#[derive(Args)]
pub struct ShowArgs {
    /// Result ID to show
    pub result_id: String,
    
    /// Show request details
    #[arg(long, default_value = "true")]
    pub show_request: bool,
    
    /// Show response details
    #[arg(long, default_value = "true")]
    pub show_response: bool,
    
    /// Show timing breakdown
    #[arg(long, default_value = "true")]
    pub show_timing: bool,
    
    /// Show assertion details
    #[arg(long, default_value = "true")]
    pub show_assertions: bool,
    
    /// Maximum body length to display
    #[arg(long, default_value = "1000")]
    pub max_body_length: usize,
}

/// Arguments for comparing results
#[derive(Args)]
pub struct CompareArgs {
    /// First result ID
    pub result1_id: String,
    
    /// Second result ID
    pub result2_id: String,
    
    /// Compare requests
    #[arg(long, default_value = "true")]
    pub compare_requests: bool,
    
    /// Compare responses
    #[arg(long, default_value = "true")]
    pub compare_responses: bool,
    
    /// Compare assertions
    #[arg(long, default_value = "true")]
    pub compare_assertions: bool,
    
    /// Compare performance metrics
    #[arg(long, default_value = "true")]
    pub compare_performance: bool,
    
    /// Ignore timing differences
    #[arg(long)]
    pub ignore_timing: bool,
}

/// Arguments for summary generation
#[derive(Args)]
pub struct SummaryArgs {
    /// Include detailed failure analysis
    #[arg(long)]
    pub detailed: bool,
    
    /// Group by test name
    #[arg(long)]
    pub group_by_test: bool,
    
    /// Group by execution session
    #[arg(long)]
    pub group_by_session: bool,
}

/// Arguments for result export
#[derive(Args)]
pub struct ExportArgs {
    /// Export format
    #[arg(long, value_enum, default_value = "json")]
    pub format: ExportFormat,
    
    /// Output file path
    #[arg(long)]
    pub output: Option<String>,
    
    /// Include failure analysis in export
    #[arg(long)]
    pub include_failures: bool,
    
    /// Include performance metrics
    #[arg(long)]
    pub include_performance: bool,
}

/// Arguments for interactive mode
#[derive(Args)]
pub struct InteractiveArgs {
    /// Start with specific filter
    #[arg(long)]
    pub initial_filter: Option<String>,
}

/// CLI-compatible result status enum
#[derive(Clone, ValueEnum)]
pub enum CliResultStatus {
    Passed,
    Failed,
    Skipped,
    Cancelled,
    Error,
}

impl From<CliResultStatus> for ResultStatus {
    fn from(status: CliResultStatus) -> Self {
        match status {
            CliResultStatus::Passed => ResultStatus::Passed,
            CliResultStatus::Failed => ResultStatus::Failed,
            CliResultStatus::Skipped => ResultStatus::Skipped,
            CliResultStatus::Cancelled => ResultStatus::Cancelled,
            CliResultStatus::Error => ResultStatus::Error,
        }
    }
}

/// CLI-compatible sort field enum
#[derive(Clone, ValueEnum)]
pub enum CliSortField {
    ExecutionTime,
    Duration,
    TestName,
    Status,
    FailureCount,
}

impl From<CliSortField> for SortField {
    fn from(field: CliSortField) -> Self {
        match field {
            CliSortField::ExecutionTime => SortField::ExecutionTime,
            CliSortField::Duration => SortField::Duration,
            CliSortField::TestName => SortField::TestName,
            CliSortField::Status => SortField::Status,
            CliSortField::FailureCount => SortField::FailureCount,
        }
    }
}

/// CLI-compatible sort direction enum
#[derive(Clone, ValueEnum)]
pub enum CliSortDirection {
    Asc,
    Desc,
}

impl From<CliSortDirection> for SortDirection {
    fn from(direction: CliSortDirection) -> Self {
        match direction {
            CliSortDirection::Asc => SortDirection::Ascending,
            CliSortDirection::Desc => SortDirection::Descending,
        }
    }
}

/// Export format options
#[derive(Clone, ValueEnum)]
pub enum ExportFormat {
    Json,
    Csv,
    Html,
    Markdown,
}

/// CLI handler for result browsing
pub struct ResultBrowserCliHandler {
    browser: ResultBrowser,
}

impl ResultBrowserCliHandler {
    pub fn new(analyzer: Arc<ResultAnalyzer>) -> Self {
        Self {
            browser: ResultBrowser::new(analyzer),
        }
    }

    /// Handle CLI commands
    pub async fn handle_command(&mut self, command: BrowserCommand) -> Result<()> {
        match command {
            BrowserCommand::List(args) => self.handle_list(args).await,
            BrowserCommand::Show(args) => self.handle_show(args).await,
            BrowserCommand::Compare(args) => self.handle_compare(args).await,
            BrowserCommand::Summary(args) => self.handle_summary(args).await,
            BrowserCommand::Export(args) => self.handle_export(args).await,
            BrowserCommand::Interactive(args) => self.handle_interactive(args).await,
        }
    }

    /// Handle list command
    async fn handle_list(&mut self, args: ListArgs) -> Result<()> {
        // Build filter from arguments
        let mut filter = ResultFilter::default();
        
        if let Some(status) = args.status {
            filter.status = Some(status.into());
        }
        
        if let Some(name) = args.name {
            filter.test_name_pattern = Some(name);
        }
        
        if let Some(from_str) = args.from {
            if let Ok(from_time) = DateTime::parse_from_rfc3339(&from_str) {
                let to_time = if let Some(to_str) = args.to {
                    DateTime::parse_from_rfc3339(&to_str)
                        .map(|dt| dt.with_timezone(&Utc))
                        .unwrap_or_else(|_| Utc::now())
                } else {
                    Utc::now()
                };
                filter.execution_time_range = Some((from_time.with_timezone(&Utc), to_time));
            }
        }
        
        if let (Some(min), Some(max)) = (args.min_duration, args.max_duration) {
            filter.duration_range = Some((
                Duration::from_millis(min),
                Duration::from_millis(max),
            ));
        }
        
        if args.failures_only {
            filter.has_failure_analysis = Some(true);
        }
        
        // Set filter and sort
        self.browser.set_filter(filter);
        self.browser.set_sort(ResultSort {
            field: args.sort.into(),
            direction: args.order.into(),
        });
        
        // Get results
        let results = self.browser.browse_results().await?;
        let limited_results: Vec<_> = results.into_iter().take(args.limit).collect();
        
        // Display results
        println!("📊 Test Results ({} shown)", limited_results.len());
        println!("{}", "=".repeat(80));
        
        for result in limited_results {
            println!("{}", result);
        }
        
        Ok(())
    }

    /// Handle show command
    async fn handle_show(&mut self, args: ShowArgs) -> Result<()> {
        // Configure display options
        let display_config = DisplayConfig {
            show_request_details: args.show_request,
            show_response_details: args.show_response,
            show_timing_breakdown: args.show_timing,
            show_assertion_details: args.show_assertions,
            max_body_length: args.max_body_length,
            highlight_differences: true,
        };
        
        self.browser.set_display_config(display_config);
        
        // Display result details
        let details = self.browser.display_result_details(&args.result_id).await?;
        println!("{}", details);
        
        Ok(())
    }

    /// Handle compare command
    async fn handle_compare(&mut self, args: CompareArgs) -> Result<()> {
        let comparison_result = self.browser.compare_and_display(
            &args.result1_id,
            &args.result2_id,
        ).await?;
        
        println!("{}", comparison_result);
        Ok(())
    }

    /// Handle summary command
    async fn handle_summary(&mut self, args: SummaryArgs) -> Result<()> {
        let summary = self.browser.generate_summary().await?;
        println!("{}", summary);
        
        if args.detailed {
            // Add more detailed analysis
            println!("\n📈 Detailed Analysis");
            println!("{}", "=".repeat(50));
            
            // Get all results for detailed analysis
            let results = self.browser.browse_results().await?;
            
            if args.group_by_test {
                self.display_grouped_by_test(&results).await?;
            }
            
            if args.group_by_session {
                self.display_grouped_by_session(&results).await?;
            }
        }
        
        Ok(())
    }

    /// Handle export command
    async fn handle_export(&mut self, args: ExportArgs) -> Result<()> {
        let results = self.browser.browse_results().await?;
        
        let output = match args.format {
            ExportFormat::Json => self.export_json(&results, &args).await?,
            ExportFormat::Csv => self.export_csv(&results, &args).await?,
            ExportFormat::Html => self.export_html(&results, &args).await?,
            ExportFormat::Markdown => self.export_markdown(&results, &args).await?,
        };
        
        if let Some(output_path) = args.output {
            std::fs::write(output_path, output)?;
            println!("✅ Results exported successfully");
        } else {
            println!("{}", output);
        }
        
        Ok(())
    }

    /// Handle interactive mode
    async fn handle_interactive(&mut self, _args: InteractiveArgs) -> Result<()> {
        println!("🔍 Interactive Result Browser");
        println!("Type 'help' for available commands, 'quit' to exit");
        
        loop {
            print!("> ");
            io::stdout().flush()?;
            
            let mut input = String::new();
            io::stdin().read_line(&mut input)?;
            let input = input.trim();
            
            if input == "quit" || input == "exit" {
                break;
            }
            
            match self.handle_interactive_command(input).await {
                Ok(()) => {},
                Err(e) => println!("❌ Error: {}", e),
            }
        }
        
        println!("👋 Goodbye!");
        Ok(())
    }

    /// Handle interactive commands
    async fn handle_interactive_command(&mut self, input: &str) -> Result<()> {
        let parts: Vec<&str> = input.split_whitespace().collect();
        if parts.is_empty() {
            return Ok(());
        }
        
        match parts[0] {
            "help" => {
                println!("Available commands:");
                println!("  list [filters]     - List results with optional filters");
                println!("  show <id>          - Show detailed result information");
                println!("  compare <id1> <id2> - Compare two results");
                println!("  summary            - Show summary statistics");
                println!("  filter <criteria>  - Set result filter");
                println!("  sort <field> <dir> - Set sort criteria");
                println!("  clear              - Clear current filters");
                println!("  quit/exit          - Exit interactive mode");
            }
            "list" => {
                let results = self.browser.browse_results().await?;
                for result in results.iter().take(20) {
                    println!("{}", result);
                }
                if results.len() > 20 {
                    println!("... and {} more results", results.len() - 20);
                }
            }
            "show" => {
                if parts.len() < 2 {
                    println!("Usage: show <result_id>");
                    return Ok(());
                }
                let details = self.browser.display_result_details(parts[1]).await?;
                println!("{}", details);
            }
            "compare" => {
                if parts.len() < 3 {
                    println!("Usage: compare <result_id1> <result_id2>");
                    return Ok(());
                }
                let comparison = self.browser.compare_and_display(parts[1], parts[2]).await?;
                println!("{}", comparison);
            }
            "summary" => {
                let summary = self.browser.generate_summary().await?;
                println!("{}", summary);
            }
            "clear" => {
                self.browser.set_filter(ResultFilter::default());
                println!("✅ Filters cleared");
            }
            _ => {
                println!("Unknown command: {}. Type 'help' for available commands.", parts[0]);
            }
        }
        
        Ok(())
    }

    /// Display results grouped by test name
    async fn display_grouped_by_test(&self, results: &[AnalyzableResult]) -> Result<()> {
        let mut grouped: HashMap<String, Vec<&AnalyzableResult>> = HashMap::new();
        
        for result in results {
            grouped.entry(result.test_case_name.clone())
                .or_insert_with(Vec::new)
                .push(result);
        }
        
        println!("\n📋 Results by Test Name:");
        for (test_name, test_results) in grouped {
            let passed = test_results.iter().filter(|r| r.status == ResultStatus::Passed).count();
            let failed = test_results.iter().filter(|r| r.status == ResultStatus::Failed).count();
            let avg_duration = test_results.iter()
                .map(|r| r.duration.as_millis())
                .sum::<u128>() / test_results.len() as u128;
            
            println!("  {} ({} results): ✅{} ❌{} ⏱️{}ms avg", 
                test_name, test_results.len(), passed, failed, avg_duration);
        }
        
        Ok(())
    }

    /// Display results grouped by session
    async fn display_grouped_by_session(&self, results: &[AnalyzableResult]) -> Result<()> {
        let mut grouped: HashMap<String, Vec<&AnalyzableResult>> = HashMap::new();
        
        for result in results {
            grouped.entry(result.execution_session_id.clone())
                .or_insert_with(Vec::new)
                .push(result);
        }
        
        println!("\n📅 Results by Session:");
        for (session_id, session_results) in grouped {
            let passed = session_results.iter().filter(|r| r.status == ResultStatus::Passed).count();
            let failed = session_results.iter().filter(|r| r.status == ResultStatus::Failed).count();
            
            println!("  {} ({} results): ✅{} ❌{}", 
                session_id, session_results.len(), passed, failed);
        }
        
        Ok(())
    }

    /// Export results as JSON
    async fn export_json(&self, results: &[AnalyzableResult], args: &ExportArgs) -> Result<String> {
        let export_data = if args.include_failures || args.include_performance {
            serde_json::to_string_pretty(results)?
        } else {
            // Create simplified export without detailed analysis
            let simplified: Vec<_> = results.iter().map(|r| {
                serde_json::json!({
                    "result_id": r.result_id,
                    "test_case_name": r.test_case_name,
                    "status": r.status,
                    "execution_time": r.execution_time,
                    "duration_ms": r.duration.as_millis(),
                })
            }).collect();
            serde_json::to_string_pretty(&simplified)?
        };
        
        Ok(export_data)
    }

    /// Export results as CSV
    async fn export_csv(&self, results: &[AnalyzableResult], _args: &ExportArgs) -> Result<String> {
        let mut csv = String::new();
        csv.push_str("result_id,test_case_name,status,execution_time,duration_ms\n");
        
        for result in results {
            csv.push_str(&format!(
                "{},{},{:?},{},{}\n",
                result.result_id,
                result.test_case_name,
                result.status,
                result.execution_time.to_rfc3339(),
                result.duration.as_millis()
            ));
        }
        
        Ok(csv)
    }

    /// Export results as HTML
    async fn export_html(&self, results: &[AnalyzableResult], _args: &ExportArgs) -> Result<String> {
        let mut html = String::new();
        html.push_str("<!DOCTYPE html>\n<html>\n<head>\n");
        html.push_str("<title>API Test Results</title>\n");
        html.push_str("<style>table { border-collapse: collapse; width: 100%; }\n");
        html.push_str("th, td { border: 1px solid #ddd; padding: 8px; text-align: left; }\n");
        html.push_str("th { background-color: #f2f2f2; }</style>\n");
        html.push_str("</head>\n<body>\n");
        html.push_str("<h1>API Test Results</h1>\n");
        html.push_str("<table>\n<tr><th>Test Name</th><th>Status</th><th>Duration</th><th>Execution Time</th></tr>\n");
        
        for result in results {
            let status_color = match result.status {
                ResultStatus::Passed => "green",
                ResultStatus::Failed => "red",
                ResultStatus::Error => "orange",
                _ => "gray",
            };
            
            html.push_str(&format!(
                "<tr><td>{}</td><td style=\"color: {}\">{:?}</td><td>{}ms</td><td>{}</td></tr>\n",
                result.test_case_name,
                status_color,
                result.status,
                result.duration.as_millis(),
                result.execution_time.format("%Y-%m-%d %H:%M:%S")
            ));
        }
        
        html.push_str("</table>\n</body>\n</html>");
        Ok(html)
    }

    /// Export results as Markdown
    async fn export_markdown(&self, results: &[AnalyzableResult], _args: &ExportArgs) -> Result<String> {
        let mut md = String::new();
        md.push_str("# API Test Results\n\n");
        md.push_str("| Test Name | Status | Duration | Execution Time |\n");
        md.push_str("|-----------|--------|----------|----------------|\n");
        
        for result in results {
            let status_emoji = match result.status {
                ResultStatus::Passed => "✅",
                ResultStatus::Failed => "❌",
                ResultStatus::Error => "🚨",
                ResultStatus::Skipped => "⏭️",
                ResultStatus::Cancelled => "🚫",
            };
            
            md.push_str(&format!(
                "| {} | {} {:?} | {}ms | {} |\n",
                result.test_case_name,
                status_emoji,
                result.status,
                result.duration.as_millis(),
                result.execution_time.format("%Y-%m-%d %H:%M:%S")
            ));
        }
        
        Ok(md)
    }
}