use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

use crate::error::TestCaseError;
use crate::test_case_manager::{TestCase, RequestDefinition, AssertionDefinition};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentationConfig {
    pub output_format: DocumentationFormat,
    pub output_path: PathBuf,
    pub include_examples: bool,
    pub include_schemas: bool,
    pub include_curl_commands: bool,
    pub group_by: GroupingStrategy,
    pub template_path: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum DocumentationFormat {
    Markdown,
    Html,
    Json,
    OpenApi,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum GroupingStrategy {
    None,
    ByTag,
    ByProtocol,
    ByEndpoint,
    ByMethod,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestCaseDocumentation {
    pub test_case_id: String,
    pub name: String,
    pub description: Option<String>,
    pub tags: Vec<String>,
    pub protocol: String,
    pub method: String,
    pub url: String,
    pub headers: HashMap<String, String>,
    pub body: Option<String>,
    pub assertions: Vec<AssertionDocumentation>,
    pub variable_extractions: Vec<VariableExtractionDocumentation>,
    pub curl_command: Option<String>,
    pub examples: Vec<TestCaseExample>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssertionDocumentation {
    pub assertion_type: String,
    pub expected_value: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VariableExtractionDocumentation {
    pub name: String,
    pub source: String,
    pub path: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestCaseExample {
    pub name: String,
    pub description: String,
    pub request_example: String,
    pub response_example: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentationReport {
    pub title: String,
    pub description: String,
    pub generated_at: DateTime<Utc>,
    pub total_test_cases: usize,
    pub test_cases_by_group: HashMap<String, Vec<TestCaseDocumentation>>,
    pub statistics: DocumentationStatistics,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentationStatistics {
    pub protocols: HashMap<String, usize>,
    pub methods: HashMap<String, usize>,
    pub tags: HashMap<String, usize>,
    pub assertions_per_test: f64,
    pub variables_per_test: f64,
}

#[async_trait]
pub trait DocumentationGenerator: Send + Sync {
    async fn generate(&self, report: &DocumentationReport, config: &DocumentationConfig) -> Result<String, TestCaseError>;
    fn format(&self) -> DocumentationFormat;
}

pub struct TestCaseDocumentationGenerator {
    generators: HashMap<DocumentationFormat, Box<dyn DocumentationGenerator>>,
}

impl TestCaseDocumentationGenerator {
    pub fn new() -> Self {
        let mut generators: HashMap<DocumentationFormat, Box<dyn DocumentationGenerator>> = HashMap::new();
        generators.insert(DocumentationFormat::Markdown, Box::new(MarkdownGenerator::new()));
        generators.insert(DocumentationFormat::Html, Box::new(HtmlGenerator::new()));
        generators.insert(DocumentationFormat::Json, Box::new(JsonGenerator::new()));
        generators.insert(DocumentationFormat::OpenApi, Box::new(OpenApiGenerator::new()));
        
        Self { generators }
    }

    pub async fn generate_documentation(
        &self,
        test_cases: &[TestCase],
        config: &DocumentationConfig,
    ) -> Result<String, TestCaseError> {
        let report = self.create_documentation_report(test_cases, config).await?;
        
        let generator = self.generators.get(&config.output_format)
            .ok_or_else(|| TestCaseError::ValidationError(
                format!("Unsupported documentation format: {:?}", config.output_format)
            ))?;
        
        generator.generate(&report, config).await
    }

    async fn create_documentation_report(
        &self,
        test_cases: &[TestCase],
        config: &DocumentationConfig,
    ) -> Result<DocumentationReport, TestCaseError> {
        let mut test_case_docs = Vec::new();
        let mut statistics = DocumentationStatistics {
            protocols: HashMap::new(),
            methods: HashMap::new(),
            tags: HashMap::new(),
            assertions_per_test: 0.0,
            variables_per_test: 0.0,
        };

        let mut total_assertions = 0;
        let mut total_variables = 0;

        for test_case in test_cases {
            let doc = self.create_test_case_documentation(test_case, config).await?;
            
            // Update statistics
            *statistics.protocols.entry(test_case.protocol.clone()).or_insert(0) += 1;
            *statistics.methods.entry(test_case.request.method.clone()).or_insert(0) += 1;
            
            for tag in &test_case.tags {
                *statistics.tags.entry(tag.clone()).or_insert(0) += 1;
            }
            
            total_assertions += test_case.assertions.len();
            total_variables += test_case.variable_extractions.as_ref().map_or(0, |v| v.len());
            
            test_case_docs.push(doc);
        }

        statistics.assertions_per_test = if test_cases.is_empty() {
            0.0
        } else {
            total_assertions as f64 / test_cases.len() as f64
        };

        statistics.variables_per_test = if test_cases.is_empty() {
            0.0
        } else {
            total_variables as f64 / test_cases.len() as f64
        };

        let test_cases_by_group = self.group_test_cases(test_case_docs, config);

        Ok(DocumentationReport {
            title: "API Test Cases Documentation".to_string(),
            description: "Automatically generated documentation for API test cases".to_string(),
            generated_at: Utc::now(),
            total_test_cases: test_cases.len(),
            test_cases_by_group,
            statistics,
        })
    }

    async fn create_test_case_documentation(
        &self,
        test_case: &TestCase,
        config: &DocumentationConfig,
    ) -> Result<TestCaseDocumentation, TestCaseError> {
        let assertions = test_case.assertions.iter()
            .map(|a| AssertionDocumentation {
                assertion_type: a.assertion_type.clone(),
                expected_value: a.expected.to_string(),
                description: a.message.clone(),
            })
            .collect();

        let variable_extractions = test_case.variable_extractions.as_ref()
            .map(|vars| vars.iter()
                .map(|v| VariableExtractionDocumentation {
                    name: v.name.clone(),
                    source: match v.source {
                        crate::test_case_manager::ExtractionSource::ResponseBody => "response".to_string(),
                        crate::test_case_manager::ExtractionSource::ResponseHeader => "header".to_string(),
                        crate::test_case_manager::ExtractionSource::StatusCode => "status".to_string(),
                    },
                    path: v.path.clone(),
                    description: None, // Could be enhanced with descriptions
                })
                .collect())
            .unwrap_or_default();

        let curl_command = if config.include_curl_commands {
            Some(self.generate_curl_command(&test_case.request))
        } else {
            None
        };

        let examples = if config.include_examples {
            self.generate_examples(test_case)
        } else {
            Vec::new()
        };

        Ok(TestCaseDocumentation {
            test_case_id: test_case.id.clone(),
            name: test_case.name.clone(),
            description: test_case.description.clone(),
            tags: test_case.tags.clone(),
            protocol: test_case.protocol.clone(),
            method: test_case.request.method.clone(),
            url: test_case.request.url.clone(),
            headers: test_case.request.headers.clone(),
            body: test_case.request.body.clone(),
            assertions,
            variable_extractions,
            curl_command,
            examples,
        })
    }

    fn group_test_cases(
        &self,
        test_cases: Vec<TestCaseDocumentation>,
        config: &DocumentationConfig,
    ) -> HashMap<String, Vec<TestCaseDocumentation>> {
        let mut grouped = HashMap::new();

        match config.group_by {
            GroupingStrategy::None => {
                grouped.insert("All Test Cases".to_string(), test_cases);
            }
            GroupingStrategy::ByTag => {
                for test_case in test_cases {
                    if test_case.tags.is_empty() {
                        grouped.entry("Untagged".to_string()).or_insert_with(Vec::new).push(test_case);
                    } else {
                        for tag in &test_case.tags {
                            grouped.entry(tag.clone()).or_insert_with(Vec::new).push(test_case.clone());
                        }
                    }
                }
            }
            GroupingStrategy::ByProtocol => {
                for test_case in test_cases {
                    grouped.entry(test_case.protocol.clone()).or_insert_with(Vec::new).push(test_case);
                }
            }
            GroupingStrategy::ByMethod => {
                for test_case in test_cases {
                    grouped.entry(test_case.method.clone()).or_insert_with(Vec::new).push(test_case);
                }
            }
            GroupingStrategy::ByEndpoint => {
                for test_case in test_cases {
                    let endpoint = self.extract_endpoint(&test_case.url);
                    grouped.entry(endpoint).or_insert_with(Vec::new).push(test_case);
                }
            }
        }

        grouped
    }

    fn extract_endpoint(&self, url: &str) -> String {
        // Extract endpoint from URL (remove protocol, host, query params)
        if let Some(path_start) = url.find("://") {
            if let Some(path) = url[path_start + 3..].find('/') {
                let full_path = &url[path_start + 3 + path..];
                if let Some(query_start) = full_path.find('?') {
                    full_path[..query_start].to_string()
                } else {
                    full_path.to_string()
                }
            } else {
                "/".to_string()
            }
        } else {
            url.to_string()
        }
    }

    fn generate_curl_command(&self, request: &RequestDefinition) -> String {
        let mut curl = format!("curl -X {} '{}'", request.method, request.url);
        
        for (key, value) in &request.headers {
            curl.push_str(&format!(" -H '{}: {}'", key, value));
        }
        
        if let Some(body) = &request.body {
            curl.push_str(&format!(" -d '{}'", body));
        }
        
        curl
    }

    fn generate_examples(&self, test_case: &TestCase) -> Vec<TestCaseExample> {
        // Generate basic examples - could be enhanced with real data
        vec![
            TestCaseExample {
                name: "Basic Example".to_string(),
                description: format!("Example request for {}", test_case.name),
                request_example: self.generate_request_example(&test_case.request),
                response_example: self.generate_response_example(&test_case.assertions),
            }
        ]
    }

    fn generate_request_example(&self, request: &RequestDefinition) -> String {
        let mut example = format!("{} {}\n", request.method, request.url);
        
        for (key, value) in &request.headers {
            example.push_str(&format!("{}: {}\n", key, value));
        }
        
        if let Some(body) = &request.body {
            example.push_str("\n");
            example.push_str(body);
        }
        
        example
    }

    fn generate_response_example(&self, assertions: &[AssertionDefinition]) -> String {
        let mut example = "HTTP/1.1 200 OK\n".to_string();
        example.push_str("Content-Type: application/json\n\n");
        
        // Generate a basic JSON response based on assertions
        let mut response_body = serde_json::Map::new();
        
        for assertion in assertions {
            if assertion.assertion_type == "json_path" {
                response_body.insert("example".to_string(), assertion.expected.clone());
            }
        }
        
        if !response_body.is_empty() {
            example.push_str(&serde_json::to_string_pretty(&response_body).unwrap_or_default());
        } else {
            example.push_str("{\n  \"status\": \"success\"\n}");
        }
        
        example
    }
}

// Documentation generators

pub struct MarkdownGenerator;

impl MarkdownGenerator {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl DocumentationGenerator for MarkdownGenerator {
    async fn generate(&self, report: &DocumentationReport, _config: &DocumentationConfig) -> Result<String, TestCaseError> {
        let mut markdown = String::new();
        
        // Title and metadata
        markdown.push_str(&format!("# {}\n\n", report.title));
        markdown.push_str(&format!("{}\n\n", report.description));
        markdown.push_str(&format!("**Generated:** {}\n", report.generated_at.format("%Y-%m-%d %H:%M:%S UTC")));
        markdown.push_str(&format!("**Total Test Cases:** {}\n\n", report.total_test_cases));
        
        // Statistics
        markdown.push_str("## Statistics\n\n");
        markdown.push_str(&format!("- **Average Assertions per Test:** {:.1}\n", report.statistics.assertions_per_test));
        markdown.push_str(&format!("- **Average Variables per Test:** {:.1}\n\n", report.statistics.variables_per_test));
        
        // Protocols
        if !report.statistics.protocols.is_empty() {
            markdown.push_str("### Protocols\n\n");
            for (protocol, count) in &report.statistics.protocols {
                markdown.push_str(&format!("- **{}:** {} test cases\n", protocol, count));
            }
            markdown.push_str("\n");
        }
        
        // Methods
        if !report.statistics.methods.is_empty() {
            markdown.push_str("### HTTP Methods\n\n");
            for (method, count) in &report.statistics.methods {
                markdown.push_str(&format!("- **{}:** {} test cases\n", method, count));
            }
            markdown.push_str("\n");
        }
        
        // Test cases by group
        for (group_name, test_cases) in &report.test_cases_by_group {
            markdown.push_str(&format!("## {}\n\n", group_name));
            
            for test_case in test_cases {
                markdown.push_str(&format!("### {}\n\n", test_case.name));
                
                if let Some(description) = &test_case.description {
                    markdown.push_str(&format!("{}\n\n", description));
                }
                
                // Tags
                if !test_case.tags.is_empty() {
                    markdown.push_str("**Tags:** ");
                    markdown.push_str(&test_case.tags.join(", "));
                    markdown.push_str("\n\n");
                }
                
                // Request details
                markdown.push_str("#### Request\n\n");
                markdown.push_str(&format!("- **Method:** {}\n", test_case.method));
                markdown.push_str(&format!("- **URL:** `{}`\n", test_case.url));
                
                if !test_case.headers.is_empty() {
                    markdown.push_str("- **Headers:**\n");
                    for (key, value) in &test_case.headers {
                        markdown.push_str(&format!("  - `{}`: `{}`\n", key, value));
                    }
                }
                
                if let Some(body) = &test_case.body {
                    markdown.push_str("- **Body:**\n");
                    markdown.push_str("```json\n");
                    markdown.push_str(body);
                    markdown.push_str("\n```\n");
                }
                
                // Assertions
                if !test_case.assertions.is_empty() {
                    markdown.push_str("\n#### Assertions\n\n");
                    for assertion in &test_case.assertions {
                        markdown.push_str(&format!("- **{}:** `{}`", assertion.assertion_type, assertion.expected_value));
                        if let Some(desc) = &assertion.description {
                            markdown.push_str(&format!(" - {}", desc));
                        }
                        markdown.push_str("\n");
                    }
                }
                
                // Variable extractions
                if !test_case.variable_extractions.is_empty() {
                    markdown.push_str("\n#### Variable Extractions\n\n");
                    for var in &test_case.variable_extractions {
                        markdown.push_str(&format!("- **{}:** {} from `{}`\n", var.name, var.source, var.path));
                    }
                }
                
                // cURL command
                if let Some(curl) = &test_case.curl_command {
                    markdown.push_str("\n#### cURL Command\n\n");
                    markdown.push_str("```bash\n");
                    markdown.push_str(curl);
                    markdown.push_str("\n```\n");
                }
                
                // Examples
                if !test_case.examples.is_empty() {
                    markdown.push_str("\n#### Examples\n\n");
                    for example in &test_case.examples {
                        markdown.push_str(&format!("##### {}\n\n", example.name));
                        markdown.push_str(&format!("{}\n\n", example.description));
                        
                        markdown.push_str("**Request:**\n```\n");
                        markdown.push_str(&example.request_example);
                        markdown.push_str("\n```\n\n");
                        
                        markdown.push_str("**Response:**\n```\n");
                        markdown.push_str(&example.response_example);
                        markdown.push_str("\n```\n\n");
                    }
                }
                
                markdown.push_str("---\n\n");
            }
        }
        
        Ok(markdown)
    }

    fn format(&self) -> DocumentationFormat {
        DocumentationFormat::Markdown
    }
}

pub struct HtmlGenerator;

impl HtmlGenerator {
    pub fn new() -> Self {
        Self
    }
    
    fn get_default_css(&self) -> &'static str {
        r#"
/* API Test Case Documentation Styles */
body {
    font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
    line-height: 1.6;
    color: #333;
    max-width: 1200px;
    margin: 0 auto;
    padding: 20px;
    background-color: #f8f9fa;
}

h1, h2, h3, h4, h5 {
    color: #2c3e50;
    margin-top: 2em;
    margin-bottom: 0.5em;
}

h1 {
    border-bottom: 3px solid #3498db;
    padding-bottom: 10px;
}

h2 {
    border-bottom: 2px solid #e74c3c;
    padding-bottom: 8px;
}

h3 {
    border-bottom: 1px solid #95a5a6;
    padding-bottom: 5px;
}

.statistics {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(250px, 1fr));
    gap: 15px;
    margin: 20px 0;
}

.stat {
    background: white;
    padding: 15px;
    border-radius: 8px;
    box-shadow: 0 2px 4px rgba(0,0,0,0.1);
    border-left: 4px solid #3498db;
}

.stat .label {
    font-weight: 600;
    color: #2c3e50;
}

.stat .value {
    font-size: 1.2em;
    font-weight: bold;
    color: #e74c3c;
}

.test-case {
    background: white;
    margin: 20px 0;
    padding: 20px;
    border-radius: 8px;
    box-shadow: 0 2px 8px rgba(0,0,0,0.1);
    border-left: 4px solid #27ae60;
}

.test-case h3 {
    margin-top: 0;
    color: #27ae60;
}

.description {
    font-style: italic;
    color: #7f8c8d;
    margin-bottom: 15px;
}

.tags {
    margin: 10px 0;
}

.tag {
    display: inline-block;
    background: #3498db;
    color: white;
    padding: 4px 8px;
    border-radius: 4px;
    font-size: 0.85em;
    margin-right: 5px;
    margin-bottom: 5px;
}

.request, .assertions {
    background: #f8f9fa;
    padding: 15px;
    border-radius: 6px;
    margin: 15px 0;
    border: 1px solid #e9ecef;
}

.request h4, .assertions h4 {
    margin-top: 0;
    color: #495057;
}

code {
    background: #f1f3f4;
    padding: 2px 6px;
    border-radius: 3px;
    font-family: 'Monaco', 'Menlo', 'Ubuntu Mono', monospace;
    font-size: 0.9em;
}

pre {
    background: #2d3748;
    color: #e2e8f0;
    padding: 15px;
    border-radius: 6px;
    overflow-x: auto;
    margin: 10px 0;
}

pre code {
    background: none;
    padding: 0;
    color: inherit;
}

ul {
    padding-left: 20px;
}

li {
    margin-bottom: 5px;
}

.request ul li {
    font-family: monospace;
    background: #fff;
    padding: 5px;
    border-radius: 3px;
    margin-bottom: 3px;
}

@media (max-width: 768px) {
    body {
        padding: 10px;
    }
    
    .statistics {
        grid-template-columns: 1fr;
    }
    
    .test-case {
        padding: 15px;
    }
}
        "#
    }
}

#[async_trait]
impl DocumentationGenerator for HtmlGenerator {
    async fn generate(&self, report: &DocumentationReport, _config: &DocumentationConfig) -> Result<String, TestCaseError> {
        let mut html = String::new();
        
        // HTML header
        html.push_str("<!DOCTYPE html>\n<html>\n<head>\n");
        html.push_str(&format!("<title>{}</title>\n", report.title));
        html.push_str("<style>\n");
        html.push_str(self.get_default_css());
        html.push_str("</style>\n");
        html.push_str("</head>\n<body>\n");
        
        // Title and metadata
        html.push_str(&format!("<h1>{}</h1>\n", report.title));
        html.push_str(&format!("<p>{}</p>\n", report.description));
        html.push_str(&format!("<p><strong>Generated:</strong> {}</p>\n", report.generated_at.format("%Y-%m-%d %H:%M:%S UTC")));
        html.push_str(&format!("<p><strong>Total Test Cases:</strong> {}</p>\n", report.total_test_cases));
        
        // Statistics
        html.push_str("<h2>Statistics</h2>\n");
        html.push_str("<div class='statistics'>\n");
        html.push_str(&format!("<div class='stat'><span class='label'>Average Assertions per Test:</span> <span class='value'>{:.1}</span></div>\n", report.statistics.assertions_per_test));
        html.push_str(&format!("<div class='stat'><span class='label'>Average Variables per Test:</span> <span class='value'>{:.1}</span></div>\n", report.statistics.variables_per_test));
        html.push_str("</div>\n");
        
        // Test cases by group
        for (group_name, test_cases) in &report.test_cases_by_group {
            html.push_str(&format!("<h2>{}</h2>\n", group_name));
            
            for test_case in test_cases {
                html.push_str("<div class='test-case'>\n");
                html.push_str(&format!("<h3>{}</h3>\n", test_case.name));
                
                if let Some(description) = &test_case.description {
                    html.push_str(&format!("<p class='description'>{}</p>\n", description));
                }
                
                // Tags
                if !test_case.tags.is_empty() {
                    html.push_str("<div class='tags'>\n");
                    for tag in &test_case.tags {
                        html.push_str(&format!("<span class='tag'>{}</span>\n", tag));
                    }
                    html.push_str("</div>\n");
                }
                
                // Request details
                html.push_str("<div class='request'>\n");
                html.push_str("<h4>Request</h4>\n");
                html.push_str(&format!("<p><strong>Method:</strong> <code>{}</code></p>\n", test_case.method));
                html.push_str(&format!("<p><strong>URL:</strong> <code>{}</code></p>\n", test_case.url));
                
                if !test_case.headers.is_empty() {
                    html.push_str("<h5>Headers</h5>\n<ul>\n");
                    for (key, value) in &test_case.headers {
                        html.push_str(&format!("<li><code>{}:</code> <code>{}</code></li>\n", key, value));
                    }
                    html.push_str("</ul>\n");
                }
                
                if let Some(body) = &test_case.body {
                    html.push_str("<h5>Body</h5>\n");
                    html.push_str("<pre><code>");
                    html.push_str(body);
                    html.push_str("</code></pre>\n");
                }
                html.push_str("</div>\n");
                
                // Assertions
                if !test_case.assertions.is_empty() {
                    html.push_str("<div class='assertions'>\n");
                    html.push_str("<h4>Assertions</h4>\n<ul>\n");
                    for assertion in &test_case.assertions {
                        html.push_str(&format!("<li><strong>{}:</strong> <code>{}</code>", assertion.assertion_type, assertion.expected_value));
                        if let Some(desc) = &assertion.description {
                            html.push_str(&format!(" - {}", desc));
                        }
                        html.push_str("</li>\n");
                    }
                    html.push_str("</ul>\n</div>\n");
                }
                
                html.push_str("</div>\n");
            }
        }
        
        html.push_str("</body>\n</html>");
        Ok(html)
    }

    fn format(&self) -> DocumentationFormat {
        DocumentationFormat::Html
    }
}

pub struct JsonGenerator;

impl JsonGenerator {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl DocumentationGenerator for JsonGenerator {
    async fn generate(&self, report: &DocumentationReport, _config: &DocumentationConfig) -> Result<String, TestCaseError> {
        serde_json::to_string_pretty(report)
            .map_err(|e| TestCaseError::SerializationError(e.to_string()))
    }

    fn format(&self) -> DocumentationFormat {
        DocumentationFormat::Json
    }
}

pub struct OpenApiGenerator;

impl OpenApiGenerator {
    pub fn new() -> Self {
        Self
    }
    
    fn extract_path(&self, url: &str) -> String {
        // Extract path from URL for OpenAPI spec
        if let Some(path_start) = url.find("://") {
            if let Some(path) = url[path_start + 3..].find('/') {
                let full_path = &url[path_start + 3 + path..];
                if let Some(query_start) = full_path.find('?') {
                    full_path[..query_start].to_string()
                } else {
                    full_path.to_string()
                }
            } else {
                "/".to_string()
            }
        } else {
            "/".to_string()
        }
    }
}

#[async_trait]
impl DocumentationGenerator for OpenApiGenerator {
    async fn generate(&self, report: &DocumentationReport, _config: &DocumentationConfig) -> Result<String, TestCaseError> {
        let mut openapi = serde_json::json!({
            "openapi": "3.0.0",
            "info": {
                "title": report.title,
                "description": report.description,
                "version": "1.0.0"
            },
            "paths": {}
        });
        
        let paths = openapi["paths"].as_object_mut().unwrap();
        
        for test_cases in report.test_cases_by_group.values() {
            for test_case in test_cases {
                let path = self.extract_path(&test_case.url);
                let method = test_case.method.to_lowercase();
                
                if !paths.contains_key(&path) {
                    paths.insert(path.clone(), serde_json::json!({}));
                }
                
                let path_obj = paths.get_mut(&path).unwrap().as_object_mut().unwrap();
                
                let mut operation = serde_json::json!({
                    "summary": test_case.name,
                    "tags": test_case.tags,
                    "responses": {
                        "200": {
                            "description": "Successful response"
                        }
                    }
                });
                
                if let Some(description) = &test_case.description {
                    operation["description"] = serde_json::Value::String(description.clone());
                }
                
                // Add request body if present
                if test_case.body.is_some() {
                    operation["requestBody"] = serde_json::json!({
                        "content": {
                            "application/json": {
                                "schema": {
                                    "type": "object"
                                }
                            }
                        }
                    });
                }
                
                path_obj.insert(method, operation);
            }
        }
        
        serde_json::to_string_pretty(&openapi)
            .map_err(|e| TestCaseError::SerializationError(e.to_string()))
    }

    fn format(&self) -> DocumentationFormat {
        DocumentationFormat::OpenApi
    }
}

impl Default for DocumentationConfig {
    fn default() -> Self {
        Self {
            output_format: DocumentationFormat::Markdown,
            output_path: PathBuf::from("docs/test-cases.md"),
            include_examples: true,
            include_schemas: false,
            include_curl_commands: true,
            group_by: GroupingStrategy::ByTag,
            template_path: None,
        }
    }
}