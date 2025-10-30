use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize, Deserializer, Serializer};
use std::collections::HashMap;

use std::time::Duration;
use uuid::Uuid;

use crate::error::TestCaseError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestCase {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub tags: Vec<String>,
    pub protocol: String,
    pub request: RequestDefinition,
    pub assertions: Vec<AssertionDefinition>,
    pub variable_extractions: Option<Vec<VariableExtraction>>,
    pub dependencies: Vec<String>,
    #[serde(
        serialize_with = "serialize_duration_as_secs",
        deserialize_with = "deserialize_duration_from_secs"
    )]
    pub timeout: Option<Duration>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub version: u32,
    pub change_log: Vec<ChangeLogEntry>,
}

// Custom serde functions for backward compatibility
fn serialize_duration_as_secs<S>(duration: &Option<Duration>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    match duration {
        Some(d) => serializer.serialize_some(&d.as_secs()),
        None => serializer.serialize_none(),
    }
}

fn deserialize_duration_from_secs<'de, D>(deserializer: D) -> Result<Option<Duration>, D::Error>
where
    D: Deserializer<'de>,
{
    let secs: Option<u64> = Option::deserialize(deserializer)?;
    Ok(secs.map(Duration::from_secs))
}

fn serialize_extraction_source<S>(source: &ExtractionSource, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let source_str = match source {
        ExtractionSource::ResponseBody => "response",
        ExtractionSource::ResponseHeader => "header",
        ExtractionSource::StatusCode => "status",
    };
    serializer.serialize_str(source_str)
}

fn deserialize_extraction_source<'de, D>(deserializer: D) -> Result<ExtractionSource, D::Error>
where
    D: Deserializer<'de>,
{
    let source_str: String = String::deserialize(deserializer)?;
    match source_str.as_str() {
        "response" | "body" => Ok(ExtractionSource::ResponseBody),
        "header" => Ok(ExtractionSource::ResponseHeader),
        "status" => Ok(ExtractionSource::StatusCode),
        _ => Ok(ExtractionSource::ResponseBody), // Default fallback for unknown values
    }
}

impl TestCase {

}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestDefinition {
    pub protocol: String,
    pub method: String,
    pub url: String,
    pub headers: HashMap<String, String>,
    pub body: Option<String>,
    pub auth: Option<AuthDefinition>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AssertionDefinition {
    pub assertion_type: String,
    pub expected: serde_json::Value,
    pub message: Option<String>,
}

/// Source for variable extraction
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ExtractionSource {
    ResponseBody,
    ResponseHeader,
    StatusCode,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VariableExtraction {
    pub name: String,
    #[serde(
        serialize_with = "serialize_extraction_source",
        deserialize_with = "deserialize_extraction_source"
    )]
    pub source: ExtractionSource,
    pub path: String,   // JSONPath, XPath, or header name
    pub default_value: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AuthDefinition {
    pub auth_type: String,
    pub config: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeLogEntry {
    pub version: u32,
    pub timestamp: DateTime<Utc>,
    pub change_type: ChangeType,
    pub description: String,
    pub changed_fields: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ChangeType {
    Created,
    Updated,
    TagsModified,
    RequestModified,
    AssertionsModified,
    VariablesModified,
    DependenciesModified,
    MetadataModified,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestCaseVersionHistory {
    pub id: String,
    pub name: String,
    pub current_version: u32,
    pub total_changes: usize,
    pub change_log: Vec<ChangeLogEntry>,
    pub created_at: DateTime<Utc>,
    pub last_updated: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestCaseMetadata {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub tags: Vec<String>,
    pub protocol: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub version: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestCaseStatistics {
    pub total_count: usize,
    pub protocols: HashMap<String, usize>,
    pub tags: HashMap<String, usize>,
    pub created_this_week: usize,
    pub created_this_month: usize,
    pub updated_this_week: usize,
    pub updated_this_month: usize,
}

#[derive(Debug, Clone)]
pub struct TestCaseFilter {
    pub name_pattern: Option<String>,
    pub description_pattern: Option<String>,
    pub tags: Option<Vec<String>>,
    pub tags_match_all: bool, // true = AND, false = OR
    pub protocol: Option<String>,
    pub method: Option<String>,
    pub url_pattern: Option<String>,
    pub created_after: Option<DateTime<Utc>>,
    pub created_before: Option<DateTime<Utc>>,
    pub updated_after: Option<DateTime<Utc>>,
    pub updated_before: Option<DateTime<Utc>>,
    pub has_auth: Option<bool>,
    pub has_assertions: Option<bool>,
    pub has_variables: Option<bool>,
    pub version: Option<u32>,
}

impl Default for TestCaseFilter {
    fn default() -> Self {
        Self {
            name_pattern: None,
            description_pattern: None,
            tags: None,
            tags_match_all: false,
            protocol: None,
            method: None,
            url_pattern: None,
            created_after: None,
            created_before: None,
            updated_after: None,
            updated_before: None,
            has_auth: None,
            has_assertions: None,
            has_variables: None,
            version: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SearchOptions {
    pub case_sensitive: bool,
    pub use_regex: bool,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
    pub sort_by: Option<SortField>,
    pub sort_order: SortOrder,
}

#[derive(Debug, Clone)]
pub enum SortField {
    Name,
    CreatedAt,
    UpdatedAt,
    Protocol,
    Method,
}

#[derive(Debug, Clone)]
pub enum SortOrder {
    Ascending,
    Descending,
}

impl Default for SearchOptions {
    fn default() -> Self {
        Self {
            case_sensitive: false,
            use_regex: false,
            limit: None,
            offset: None,
            sort_by: Some(SortField::UpdatedAt),
            sort_order: SortOrder::Descending,
        }
    }
}

#[derive(Debug, Clone)]
pub enum TestCaseFormat {
    Yaml,
    Json,
}

#[async_trait]
pub trait StorageBackend: Send + Sync {
    async fn create_test_case(&self, test_case: &TestCase) -> Result<String, TestCaseError>;
    async fn read_test_case(&self, id: &str) -> Result<TestCase, TestCaseError>;
    async fn update_test_case(&self, id: &str, test_case: &TestCase) -> Result<(), TestCaseError>;
    async fn delete_test_case(&self, id: &str) -> Result<(), TestCaseError>;
    async fn list_test_cases(&self, filter: &TestCaseFilter) -> Result<Vec<TestCaseMetadata>, TestCaseError>;
    async fn test_case_exists(&self, id: &str) -> Result<bool, TestCaseError>;
}

pub struct TestCaseManager {
    storage_backend: Box<dyn StorageBackend>,
    validator: TestCaseValidator,
}

impl TestCaseManager {
    pub fn new(storage_backend: Box<dyn StorageBackend>) -> Self {
        Self {
            storage_backend,
            validator: TestCaseValidator::new(),
        }
    }

    pub async fn create_test_case(&self, mut test_case: TestCase) -> Result<String, TestCaseError> {
        // Generate ID if not provided
        if test_case.id.is_empty() {
            test_case.id = Uuid::new_v4().to_string();
        }

        // Set timestamps
        let now = Utc::now();
        test_case.created_at = now;
        test_case.updated_at = now;
        test_case.version = 1;

        // Initialize change log
        test_case.change_log = vec![ChangeLogEntry {
            version: 1,
            timestamp: now,
            change_type: ChangeType::Created,
            description: "Test case created".to_string(),
            changed_fields: vec!["*".to_string()],
        }];

        // Validate test case
        self.validator.validate(&test_case)?;

        // Check if test case with same ID already exists
        if self.storage_backend.test_case_exists(&test_case.id).await? {
            return Err(TestCaseError::AlreadyExists(test_case.id));
        }

        self.storage_backend.create_test_case(&test_case).await
    }

    pub async fn read_test_case(&self, id: &str) -> Result<TestCase, TestCaseError> {
        self.storage_backend.read_test_case(id).await
    }

    pub async fn update_test_case(&self, id: &str, mut test_case: TestCase) -> Result<(), TestCaseError> {
        // Ensure the ID matches
        test_case.id = id.to_string();
        
        // Get existing test case to preserve creation time and increment version
        let existing = self.storage_backend.read_test_case(id).await?;
        test_case.created_at = existing.created_at;
        test_case.updated_at = Utc::now();
        test_case.version = existing.version + 1;

        // Detect changes and update change log
        let changed_fields = self.detect_changes(&existing, &test_case);
        let change_type = self.determine_change_type(&changed_fields);
        
        // Preserve existing change log and add new entry
        test_case.change_log = existing.change_log;
        test_case.change_log.push(ChangeLogEntry {
            version: test_case.version,
            timestamp: test_case.updated_at,
            change_type,
            description: self.generate_change_description(&changed_fields),
            changed_fields,
        });

        // Validate updated test case
        self.validator.validate(&test_case)?;

        self.storage_backend.update_test_case(id, &test_case).await
    }

    pub async fn delete_test_case(&self, id: &str) -> Result<(), TestCaseError> {
        // Check if test case exists before deletion
        if !self.storage_backend.test_case_exists(id).await? {
            return Err(TestCaseError::NotFound(id.to_string()));
        }

        self.storage_backend.delete_test_case(id).await
    }

    pub async fn list_test_cases(&self, filter: &TestCaseFilter) -> Result<Vec<TestCaseMetadata>, TestCaseError> {
        self.storage_backend.list_test_cases(filter).await
    }

    pub async fn search_test_cases(
        &self, 
        filter: &TestCaseFilter, 
        options: &SearchOptions
    ) -> Result<Vec<TestCaseMetadata>, TestCaseError> {
        // Create a filter for storage backend that excludes complex patterns
        let mut storage_filter = filter.clone();
        
        // If using regex or advanced options, skip name pattern filtering at storage level
        if options.use_regex || filter.method.is_some() || filter.url_pattern.is_some() || 
           filter.description_pattern.is_some() || filter.has_auth.is_some() || 
           filter.has_assertions.is_some() || filter.has_variables.is_some() {
            storage_filter.name_pattern = None;
        }
        
        // Get all matching test cases from storage
        let mut results = self.storage_backend.list_test_cases(&storage_filter).await?;
        
        // Apply additional filtering that might not be supported by storage backend
        results = self.apply_advanced_filtering(results, filter, options).await?;
        
        // Apply sorting
        self.sort_results(&mut results, options);
        
        // Apply pagination
        if let Some(offset) = options.offset {
            if offset < results.len() {
                results = results.into_iter().skip(offset).collect();
            } else {
                results.clear();
            }
        }
        
        if let Some(limit) = options.limit {
            results.truncate(limit);
        }
        
        Ok(results)
    }

    pub async fn search_by_text(&self, query: &str, options: &SearchOptions) -> Result<Vec<TestCaseMetadata>, TestCaseError> {
        let filter = TestCaseFilter {
            name_pattern: Some(query.to_string()),
            description_pattern: Some(query.to_string()),
            ..Default::default()
        };
        self.search_test_cases(&filter, options).await
    }

    pub async fn get_test_cases_by_tags(&self, tags: Vec<String>, match_all: bool) -> Result<Vec<TestCaseMetadata>, TestCaseError> {
        let filter = TestCaseFilter {
            tags: Some(tags),
            tags_match_all: match_all,
            ..Default::default()
        };
        let options = SearchOptions::default();
        self.search_test_cases(&filter, &options).await
    }

    pub async fn get_test_cases_by_protocol(&self, protocol: &str) -> Result<Vec<TestCaseMetadata>, TestCaseError> {
        let filter = TestCaseFilter {
            protocol: Some(protocol.to_string()),
            ..Default::default()
        };
        let options = SearchOptions::default();
        self.search_test_cases(&filter, &options).await
    }

    pub async fn get_recent_test_cases(&self, limit: usize) -> Result<Vec<TestCaseMetadata>, TestCaseError> {
        let filter = TestCaseFilter::default();
        let options = SearchOptions {
            limit: Some(limit),
            sort_by: Some(SortField::UpdatedAt),
            sort_order: SortOrder::Descending,
            ..Default::default()
        };
        self.search_test_cases(&filter, &options).await
    }

    pub async fn get_test_case_statistics(&self) -> Result<TestCaseStatistics, TestCaseError> {
        let all_cases = self.list_test_cases(&TestCaseFilter::default()).await?;
        
        let mut stats = TestCaseStatistics {
            total_count: all_cases.len(),
            protocols: HashMap::new(),
            tags: HashMap::new(),
            created_this_week: 0,
            created_this_month: 0,
            updated_this_week: 0,
            updated_this_month: 0,
        };

        let now = Utc::now();
        let week_ago = now - chrono::Duration::days(7);
        let month_ago = now - chrono::Duration::days(30);

        for case in &all_cases {
            // Count by protocol
            *stats.protocols.entry(case.protocol.clone()).or_insert(0) += 1;
            
            // Count by tags
            for tag in &case.tags {
                *stats.tags.entry(tag.clone()).or_insert(0) += 1;
            }
            
            // Count recent activity
            if case.created_at > week_ago {
                stats.created_this_week += 1;
            }
            if case.created_at > month_ago {
                stats.created_this_month += 1;
            }
            if case.updated_at > week_ago {
                stats.updated_this_week += 1;
            }
            if case.updated_at > month_ago {
                stats.updated_this_month += 1;
            }
        }

        Ok(stats)
    }

    async fn apply_advanced_filtering(
        &self,
        mut results: Vec<TestCaseMetadata>,
        filter: &TestCaseFilter,
        options: &SearchOptions,
    ) -> Result<Vec<TestCaseMetadata>, TestCaseError> {
        // For advanced filtering, we need to load full test cases
        // This is less efficient but provides more flexibility
        let mut filtered_results = Vec::new();
        
        for metadata in results {
            let test_case = self.read_test_case(&metadata.id).await?;
            
            if self.matches_advanced_filter(&test_case, filter, options)? {
                filtered_results.push(metadata);
            }
        }
        
        Ok(filtered_results)
    }

    fn matches_advanced_filter(
        &self,
        test_case: &TestCase,
        filter: &TestCaseFilter,
        options: &SearchOptions,
    ) -> Result<bool, TestCaseError> {
        // Check name pattern (for regex support)
        let mut name_matches = true;
        let mut desc_matches = true;
        
        if let Some(name_pattern) = &filter.name_pattern {
            name_matches = self.matches_pattern(&test_case.name, name_pattern, options)?;
        }
        
        // Check description pattern
        if let Some(desc_pattern) = &filter.description_pattern {
            if let Some(description) = &test_case.description {
                desc_matches = self.matches_pattern(description, desc_pattern, options)?;
            } else {
                desc_matches = false;
            }
        }
        
        // If both name and description patterns are specified, use OR logic for text search
        if filter.name_pattern.is_some() && filter.description_pattern.is_some() {
            if !name_matches && !desc_matches {
                return Ok(false);
            }
        } else {
            // If only one pattern is specified, it must match
            if filter.name_pattern.is_some() && !name_matches {
                return Ok(false);
            }
            if filter.description_pattern.is_some() && !desc_matches {
                return Ok(false);
            }
        }
        
        // Check method filter
        if let Some(method) = &filter.method {
            if !self.matches_pattern(&test_case.request.method, method, options)? {
                return Ok(false);
            }
        }
        
        // Check URL pattern
        if let Some(url_pattern) = &filter.url_pattern {
            if !self.matches_pattern(&test_case.request.url, url_pattern, options)? {
                return Ok(false);
            }
        }
        

        
        // Check auth presence
        if let Some(has_auth) = filter.has_auth {
            let test_case_has_auth = test_case.request.auth.is_some();
            if has_auth != test_case_has_auth {
                return Ok(false);
            }
        }
        
        // Check assertions presence
        if let Some(has_assertions) = filter.has_assertions {
            let test_case_has_assertions = !test_case.assertions.is_empty();
            if has_assertions != test_case_has_assertions {
                return Ok(false);
            }
        }
        
        // Check variables presence
        if let Some(has_variables) = filter.has_variables {
            let test_case_has_variables = test_case.variable_extractions
                .as_ref()
                .map(|v| !v.is_empty())
                .unwrap_or(false);
            if has_variables != test_case_has_variables {
                return Ok(false);
            }
        }
        
        // Check version
        if let Some(version) = filter.version {
            if test_case.version != version {
                return Ok(false);
            }
        }
        
        // Check updated date ranges
        if let Some(updated_after) = filter.updated_after {
            if test_case.updated_at <= updated_after {
                return Ok(false);
            }
        }
        
        if let Some(updated_before) = filter.updated_before {
            if test_case.updated_at >= updated_before {
                return Ok(false);
            }
        }
        
        Ok(true)
    }

    fn matches_pattern(&self, text: &str, pattern: &str, options: &SearchOptions) -> Result<bool, TestCaseError> {
        if options.use_regex {
            let regex = if options.case_sensitive {
                regex::Regex::new(pattern)
            } else {
                regex::RegexBuilder::new(pattern)
                    .case_insensitive(true)
                    .build()
            };
            
            match regex {
                Ok(re) => Ok(re.is_match(text)),
                Err(e) => Err(TestCaseError::SearchError(format!("Invalid regex pattern: {}", e))),
            }
        } else {
            let text_to_search = if options.case_sensitive { text } else { &text.to_lowercase() };
            let pattern_to_match = if options.case_sensitive { pattern } else { &pattern.to_lowercase() };
            Ok(text_to_search.contains(pattern_to_match))
        }
    }

    fn sort_results(&self, results: &mut Vec<TestCaseMetadata>, options: &SearchOptions) {
        if let Some(sort_field) = &options.sort_by {
            results.sort_by(|a, b| {
                let comparison = match sort_field {
                    SortField::Name => a.name.cmp(&b.name),
                    SortField::CreatedAt => a.created_at.cmp(&b.created_at),
                    SortField::UpdatedAt => a.updated_at.cmp(&b.updated_at),
                    SortField::Protocol => a.protocol.cmp(&b.protocol),
                    SortField::Method => {
                        // For method sorting, we'd need to load the full test case
                        // For now, just sort by name as fallback
                        a.name.cmp(&b.name)
                    }
                };
                
                match options.sort_order {
                    SortOrder::Ascending => comparison,
                    SortOrder::Descending => comparison.reverse(),
                }
            });
        }
    }

    fn detect_changes(&self, old: &TestCase, new: &TestCase) -> Vec<String> {
        let mut changed_fields = Vec::new();

        if old.name != new.name {
            changed_fields.push("name".to_string());
        }
        if old.description != new.description {
            changed_fields.push("description".to_string());
        }
        if old.tags != new.tags {
            changed_fields.push("tags".to_string());
        }
        if old.protocol != new.protocol {
            changed_fields.push("protocol".to_string());
        }
        if old.request.method != new.request.method {
            changed_fields.push("request.method".to_string());
        }
        if old.request.url != new.request.url {
            changed_fields.push("request.url".to_string());
        }
        if old.request.headers != new.request.headers {
            changed_fields.push("request.headers".to_string());
        }
        if old.request.body != new.request.body {
            changed_fields.push("request.body".to_string());
        }
        if old.request.auth != new.request.auth {
            changed_fields.push("request.auth".to_string());
        }
        if old.assertions != new.assertions {
            changed_fields.push("assertions".to_string());
        }
        if old.variable_extractions != new.variable_extractions {
            changed_fields.push("variable_extractions".to_string());
        }
        if old.dependencies != new.dependencies {
            changed_fields.push("dependencies".to_string());
        }
        if old.timeout != new.timeout {
            changed_fields.push("timeout".to_string());
        }

        changed_fields
    }

    fn determine_change_type(&self, changed_fields: &[String]) -> ChangeType {
        if changed_fields.contains(&"tags".to_string()) {
            ChangeType::TagsModified
        } else if changed_fields.iter().any(|f| f.starts_with("request.")) {
            ChangeType::RequestModified
        } else if changed_fields.contains(&"assertions".to_string()) {
            ChangeType::AssertionsModified
        } else if changed_fields.contains(&"variable_extractions".to_string()) {
            ChangeType::VariablesModified
        } else if changed_fields.contains(&"dependencies".to_string()) {
            ChangeType::DependenciesModified
        } else {
            ChangeType::MetadataModified
        }
    }

    fn generate_change_description(&self, changed_fields: &[String]) -> String {
        if changed_fields.is_empty() {
            return "No changes detected".to_string();
        }

        let field_descriptions: Vec<String> = changed_fields
            .iter()
            .map(|field| match field.as_str() {
                "name" => "name".to_string(),
                "description" => "description".to_string(),
                "tags" => "tags".to_string(),
                "protocol" => "protocol".to_string(),
                "request.method" => "HTTP method".to_string(),
                "request.url" => "URL".to_string(),
                "request.headers" => "headers".to_string(),
                "request.body" => "request body".to_string(),
                "request.auth" => "authentication".to_string(),
                "assertions" => "assertions".to_string(),
                "variable_extractions" => "variable extractions".to_string(),
                "dependencies" => "dependencies".to_string(),
                "timeout" => "timeout".to_string(),
                _ => field.clone(),
            })
            .collect();

        if field_descriptions.len() == 1 {
            format!("Updated {}", field_descriptions[0])
        } else if field_descriptions.len() == 2 {
            format!("Updated {} and {}", field_descriptions[0], field_descriptions[1])
        } else {
            let last = field_descriptions.last().unwrap();
            let others = &field_descriptions[..field_descriptions.len() - 1];
            format!("Updated {}, and {}", others.join(", "), last)
        }
    }

    pub async fn get_test_case_versions(&self, id: &str) -> Result<Vec<ChangeLogEntry>, TestCaseError> {
        let test_case = self.read_test_case(id).await?;
        Ok(test_case.change_log)
    }

    pub async fn get_test_case_version_history(&self, id: &str) -> Result<TestCaseVersionHistory, TestCaseError> {
        let test_case = self.read_test_case(id).await?;
        
        Ok(TestCaseVersionHistory {
            id: test_case.id,
            name: test_case.name,
            current_version: test_case.version,
            total_changes: test_case.change_log.len(),
            change_log: test_case.change_log,
            created_at: test_case.created_at,
            last_updated: test_case.updated_at,
        })
    }

    pub async fn search_by_change_type(&self, change_type: ChangeType) -> Result<Vec<TestCaseMetadata>, TestCaseError> {
        let all_cases = self.list_test_cases(&TestCaseFilter::default()).await?;
        let mut matching_cases = Vec::new();

        for metadata in all_cases {
            let test_case = self.read_test_case(&metadata.id).await?;
            if test_case.change_log.iter().any(|entry| entry.change_type == change_type) {
                matching_cases.push(metadata);
            }
        }

        Ok(matching_cases)
    }

    pub async fn get_recently_modified_test_cases(&self, days: i64) -> Result<Vec<TestCaseMetadata>, TestCaseError> {
        let cutoff_date = Utc::now() - chrono::Duration::days(days);
        let filter = TestCaseFilter {
            updated_after: Some(cutoff_date),
            ..Default::default()
        };
        let options = SearchOptions {
            sort_by: Some(SortField::UpdatedAt),
            sort_order: SortOrder::Descending,
            ..Default::default()
        };
        self.search_test_cases(&filter, &options).await
    }
}

pub struct TestCaseValidator;

impl TestCaseValidator {
    pub fn new() -> Self {
        Self
    }

    pub fn validate(&self, test_case: &TestCase) -> Result<(), TestCaseError> {
        // Validate required fields
        if test_case.name.trim().is_empty() {
            return Err(TestCaseError::ValidationError("Test case name cannot be empty".to_string()));
        }

        if test_case.protocol.trim().is_empty() {
            return Err(TestCaseError::ValidationError("Protocol cannot be empty".to_string()));
        }

        // Validate request
        self.validate_request(&test_case.request)?;

        // Validate assertions
        for assertion in &test_case.assertions {
            self.validate_assertion(assertion)?;
        }

        // Validate variable extractions if present
        if let Some(extractions) = &test_case.variable_extractions {
            for extraction in extractions {
                self.validate_variable_extraction(extraction)?;
            }
        }

        Ok(())
    }

    fn validate_request(&self, request: &RequestDefinition) -> Result<(), TestCaseError> {
        if request.method.trim().is_empty() {
            return Err(TestCaseError::ValidationError("HTTP method cannot be empty".to_string()));
        }

        if request.url.trim().is_empty() {
            return Err(TestCaseError::ValidationError("URL cannot be empty".to_string()));
        }

        // Basic URL validation
        if !request.url.starts_with("http://") && !request.url.starts_with("https://") && !request.url.starts_with("{{") {
            return Err(TestCaseError::ValidationError("URL must start with http://, https://, or be a template variable".to_string()));
        }

        Ok(())
    }

    fn validate_assertion(&self, assertion: &AssertionDefinition) -> Result<(), TestCaseError> {
        if assertion.assertion_type.trim().is_empty() {
            return Err(TestCaseError::ValidationError("Assertion type cannot be empty".to_string()));
        }

        Ok(())
    }

    fn validate_variable_extraction(&self, extraction: &VariableExtraction) -> Result<(), TestCaseError> {
        if extraction.name.trim().is_empty() {
            return Err(TestCaseError::ValidationError("Variable extraction name cannot be empty".to_string()));
        }

        if extraction.path.trim().is_empty() {
            return Err(TestCaseError::ValidationError("Variable extraction path cannot be empty".to_string()));
        }

        Ok(())
    }
}