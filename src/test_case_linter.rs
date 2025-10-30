use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

use crate::error::TestCaseError;
use crate::test_case_manager::TestCase;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationRule {
    pub id: String,
    pub name: String,
    pub description: String,
    pub severity: ValidationSeverity,
    pub category: ValidationCategory,
    pub enabled: bool,
    pub config: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ValidationSeverity {
    Error,
    Warning,
    Info,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ValidationCategory {
    Structure,
    Naming,
    Security,
    Performance,
    Maintainability,
    BestPractices,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationIssue {
    pub rule_id: String,
    pub severity: ValidationSeverity,
    pub message: String,
    pub location: ValidationLocation,
    pub suggestion: Option<String>,
    pub auto_fixable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ValidationLocation {
    TestCase,
    Request,
    Assertion(usize),
    VariableExtraction(usize),
    Field(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    pub test_case_id: String,
    pub test_case_name: String,
    pub issues: Vec<ValidationIssue>,
    pub score: f64, // Quality score from 0.0 to 100.0
    pub validated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LintingReport {
    pub total_test_cases: usize,
    pub issues_by_severity: HashMap<ValidationSeverity, usize>,
    pub issues_by_category: HashMap<ValidationCategory, usize>,
    pub average_score: f64,
    pub test_case_results: Vec<ValidationResult>,
    pub generated_at: DateTime<Utc>,
}

#[async_trait]
pub trait ValidationRuleEngine: Send + Sync {
    async fn validate(&self, test_case: &TestCase) -> Result<Vec<ValidationIssue>, TestCaseError>;
    fn rule_id(&self) -> &str;
    fn severity(&self) -> ValidationSeverity;
    fn category(&self) -> ValidationCategory;
}

pub struct TestCaseLinter {
    rules: Vec<Box<dyn ValidationRuleEngine>>,
    config: LinterConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinterConfig {
    pub enabled_rules: Vec<String>,
    pub disabled_rules: Vec<String>,
    pub severity_threshold: ValidationSeverity,
    pub fail_on_error: bool,
    pub auto_fix: bool,
    pub custom_rules_path: Option<PathBuf>,
}

impl Default for LinterConfig {
    fn default() -> Self {
        Self {
            enabled_rules: vec![],
            disabled_rules: vec![],
            severity_threshold: ValidationSeverity::Warning,
            fail_on_error: true,
            auto_fix: false,
            custom_rules_path: None,
        }
    }
}

impl TestCaseLinter {
    pub fn new(config: LinterConfig) -> Self {
        let mut linter = Self {
            rules: Vec::new(),
            config,
        };
        
        linter.register_builtin_rules();
        linter
    }

    pub async fn lint_test_case(&self, test_case: &TestCase) -> Result<ValidationResult, TestCaseError> {
        let mut all_issues = Vec::new();
        
        for rule in &self.rules {
            if self.is_rule_enabled(rule.rule_id()) {
                let issues = rule.validate(test_case).await?;
                all_issues.extend(issues);
            }
        }
        
        // Filter by severity threshold
        all_issues.retain(|issue| self.meets_severity_threshold(&issue.severity));
        
        let score = self.calculate_quality_score(&all_issues);
        
        Ok(ValidationResult {
            test_case_id: test_case.id.clone(),
            test_case_name: test_case.name.clone(),
            issues: all_issues,
            score,
            validated_at: Utc::now(),
        })
    }

    pub async fn lint_test_cases(&self, test_cases: &[TestCase]) -> Result<LintingReport, TestCaseError> {
        let mut results = Vec::new();
        let mut issues_by_severity = HashMap::new();
        let mut issues_by_category = HashMap::new();
        
        for test_case in test_cases {
            let result = self.lint_test_case(test_case).await?;
            
            // Count issues by severity and category
            for issue in &result.issues {
                *issues_by_severity.entry(issue.severity.clone()).or_insert(0) += 1;
                
                // Get category from rule
                if let Some(rule) = self.rules.iter().find(|r| r.rule_id() == issue.rule_id) {
                    *issues_by_category.entry(rule.category()).or_insert(0) += 1;
                }
            }
            
            results.push(result);
        }
        
        let average_score = if results.is_empty() {
            100.0
        } else {
            results.iter().map(|r| r.score).sum::<f64>() / results.len() as f64
        };
        
        Ok(LintingReport {
            total_test_cases: test_cases.len(),
            issues_by_severity,
            issues_by_category,
            average_score,
            test_case_results: results,
            generated_at: Utc::now(),
        })
    }

    pub fn add_rule(&mut self, rule: Box<dyn ValidationRuleEngine>) {
        self.rules.push(rule);
    }

    fn register_builtin_rules(&mut self) {
        self.add_rule(Box::new(NamingConventionRule::new()));
        self.add_rule(Box::new(DescriptionRequiredRule::new()));
        self.add_rule(Box::new(TagsRequiredRule::new()));
        self.add_rule(Box::new(AssertionRequiredRule::new()));
        self.add_rule(Box::new(TimeoutValidationRule::new()));
        self.add_rule(Box::new(UrlValidationRule::new()));
        self.add_rule(Box::new(HttpMethodValidationRule::new()));
        self.add_rule(Box::new(HeaderValidationRule::new()));
        self.add_rule(Box::new(SecurityHeadersRule::new()));
        self.add_rule(Box::new(PerformanceAssertionRule::new()));
        self.add_rule(Box::new(VariableNamingRule::new()));
        self.add_rule(Box::new(DuplicateAssertionRule::new()));
        self.add_rule(Box::new(UnusedVariableRule::new()));
        self.add_rule(Box::new(HardcodedCredentialsRule::new()));
    }

    fn is_rule_enabled(&self, rule_id: &str) -> bool {
        if !self.config.disabled_rules.is_empty() && self.config.disabled_rules.contains(&rule_id.to_string()) {
            return false;
        }
        
        if !self.config.enabled_rules.is_empty() {
            return self.config.enabled_rules.contains(&rule_id.to_string());
        }
        
        true // Default to enabled
    }

    fn meets_severity_threshold(&self, severity: &ValidationSeverity) -> bool {
        match (&self.config.severity_threshold, severity) {
            (ValidationSeverity::Error, ValidationSeverity::Error) => true,
            (ValidationSeverity::Warning, ValidationSeverity::Error | ValidationSeverity::Warning) => true,
            (ValidationSeverity::Info, _) => true,
            _ => false,
        }
    }

    fn calculate_quality_score(&self, issues: &[ValidationIssue]) -> f64 {
        if issues.is_empty() {
            return 100.0;
        }
        
        let mut penalty = 0.0;
        for issue in issues {
            penalty += match issue.severity {
                ValidationSeverity::Error => 10.0,
                ValidationSeverity::Warning => 5.0,
                ValidationSeverity::Info => 1.0,
            };
        }
        
        (100.0_f64 - penalty).max(0.0)
    }
}

// Built-in validation rules

pub struct NamingConventionRule;

impl NamingConventionRule {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl ValidationRuleEngine for NamingConventionRule {
    async fn validate(&self, test_case: &TestCase) -> Result<Vec<ValidationIssue>, TestCaseError> {
        let mut issues = Vec::new();
        
        // Check test case name follows convention
        if !test_case.name.chars().any(|c| c.is_ascii_uppercase()) {
            issues.push(ValidationIssue {
                rule_id: self.rule_id().to_string(),
                severity: self.severity(),
                message: "Test case name should use proper capitalization".to_string(),
                location: ValidationLocation::Field("name".to_string()),
                suggestion: Some("Use title case for test case names".to_string()),
                auto_fixable: false,
            });
        }
        
        // Check for descriptive names
        if test_case.name.len() < 10 {
            issues.push(ValidationIssue {
                rule_id: self.rule_id().to_string(),
                severity: ValidationSeverity::Warning,
                message: "Test case name is too short, consider making it more descriptive".to_string(),
                location: ValidationLocation::Field("name".to_string()),
                suggestion: Some("Use descriptive names that explain what the test validates".to_string()),
                auto_fixable: false,
            });
        }
        
        Ok(issues)
    }

    fn rule_id(&self) -> &str {
        "naming-convention"
    }

    fn severity(&self) -> ValidationSeverity {
        ValidationSeverity::Warning
    }

    fn category(&self) -> ValidationCategory {
        ValidationCategory::Naming
    }
}

pub struct DescriptionRequiredRule;

impl DescriptionRequiredRule {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl ValidationRuleEngine for DescriptionRequiredRule {
    async fn validate(&self, test_case: &TestCase) -> Result<Vec<ValidationIssue>, TestCaseError> {
        let mut issues = Vec::new();
        
        if test_case.description.is_none() || test_case.description.as_ref().unwrap().trim().is_empty() {
            issues.push(ValidationIssue {
                rule_id: self.rule_id().to_string(),
                severity: self.severity(),
                message: "Test case should have a description".to_string(),
                location: ValidationLocation::Field("description".to_string()),
                suggestion: Some("Add a description explaining the purpose of this test".to_string()),
                auto_fixable: false,
            });
        }
        
        Ok(issues)
    }

    fn rule_id(&self) -> &str {
        "description-required"
    }

    fn severity(&self) -> ValidationSeverity {
        ValidationSeverity::Warning
    }

    fn category(&self) -> ValidationCategory {
        ValidationCategory::Maintainability
    }
}

pub struct TagsRequiredRule;

impl TagsRequiredRule {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl ValidationRuleEngine for TagsRequiredRule {
    async fn validate(&self, test_case: &TestCase) -> Result<Vec<ValidationIssue>, TestCaseError> {
        let mut issues = Vec::new();
        
        if test_case.tags.is_empty() {
            issues.push(ValidationIssue {
                rule_id: self.rule_id().to_string(),
                severity: self.severity(),
                message: "Test case should have at least one tag for organization".to_string(),
                location: ValidationLocation::Field("tags".to_string()),
                suggestion: Some("Add tags like 'smoke', 'regression', 'api', etc.".to_string()),
                auto_fixable: false,
            });
        }
        
        Ok(issues)
    }

    fn rule_id(&self) -> &str {
        "tags-required"
    }

    fn severity(&self) -> ValidationSeverity {
        ValidationSeverity::Info
    }

    fn category(&self) -> ValidationCategory {
        ValidationCategory::Maintainability
    }
}

pub struct AssertionRequiredRule;

impl AssertionRequiredRule {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl ValidationRuleEngine for AssertionRequiredRule {
    async fn validate(&self, test_case: &TestCase) -> Result<Vec<ValidationIssue>, TestCaseError> {
        let mut issues = Vec::new();
        
        if test_case.assertions.is_empty() {
            issues.push(ValidationIssue {
                rule_id: self.rule_id().to_string(),
                severity: self.severity(),
                message: "Test case must have at least one assertion".to_string(),
                location: ValidationLocation::TestCase,
                suggestion: Some("Add assertions to validate the response".to_string()),
                auto_fixable: false,
            });
        }
        
        Ok(issues)
    }

    fn rule_id(&self) -> &str {
        "assertion-required"
    }

    fn severity(&self) -> ValidationSeverity {
        ValidationSeverity::Error
    }

    fn category(&self) -> ValidationCategory {
        ValidationCategory::Structure
    }
}

pub struct TimeoutValidationRule;

impl TimeoutValidationRule {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl ValidationRuleEngine for TimeoutValidationRule {
    async fn validate(&self, test_case: &TestCase) -> Result<Vec<ValidationIssue>, TestCaseError> {
        let mut issues = Vec::new();
        
        if let Some(timeout) = test_case.timeout {
            if timeout > Duration::from_secs(300) {
                issues.push(ValidationIssue {
                    rule_id: self.rule_id().to_string(),
                    severity: self.severity(),
                    message: "Timeout is very high (>5 minutes), consider reducing it".to_string(),
                    location: ValidationLocation::Field("timeout".to_string()),
                    suggestion: Some("Use shorter timeouts for better test performance".to_string()),
                    auto_fixable: false,
                });
            }
            
            if timeout < Duration::from_secs(1) {
                issues.push(ValidationIssue {
                    rule_id: self.rule_id().to_string(),
                    severity: ValidationSeverity::Error,
                    message: "Timeout must be at least 1 second".to_string(),
                    location: ValidationLocation::Field("timeout".to_string()),
                    suggestion: Some("Set a reasonable timeout value".to_string()),
                    auto_fixable: true,
                });
            }
        }
        
        Ok(issues)
    }

    fn rule_id(&self) -> &str {
        "timeout-validation"
    }

    fn severity(&self) -> ValidationSeverity {
        ValidationSeverity::Warning
    }

    fn category(&self) -> ValidationCategory {
        ValidationCategory::Performance
    }
}

pub struct UrlValidationRule;

impl UrlValidationRule {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl ValidationRuleEngine for UrlValidationRule {
    async fn validate(&self, test_case: &TestCase) -> Result<Vec<ValidationIssue>, TestCaseError> {
        let mut issues = Vec::new();
        
        let url = &test_case.request.url;
        
        // Check for hardcoded localhost URLs
        if url.contains("localhost") || url.contains("127.0.0.1") {
            issues.push(ValidationIssue {
                rule_id: self.rule_id().to_string(),
                severity: self.severity(),
                message: "Avoid hardcoded localhost URLs, use environment variables".to_string(),
                location: ValidationLocation::Request,
                suggestion: Some("Use {{base_url}} or environment-specific URLs".to_string()),
                auto_fixable: false,
            });
        }
        
        // Check for missing protocol
        if !url.starts_with("http://") && !url.starts_with("https://") && !url.starts_with("{{") {
            issues.push(ValidationIssue {
                rule_id: self.rule_id().to_string(),
                severity: ValidationSeverity::Error,
                message: "URL must include protocol (http:// or https://)".to_string(),
                location: ValidationLocation::Request,
                suggestion: Some("Add http:// or https:// prefix to the URL".to_string()),
                auto_fixable: true,
            });
        }
        
        Ok(issues)
    }

    fn rule_id(&self) -> &str {
        "url-validation"
    }

    fn severity(&self) -> ValidationSeverity {
        ValidationSeverity::Warning
    }

    fn category(&self) -> ValidationCategory {
        ValidationCategory::BestPractices
    }
}

pub struct HttpMethodValidationRule;

impl HttpMethodValidationRule {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl ValidationRuleEngine for HttpMethodValidationRule {
    async fn validate(&self, test_case: &TestCase) -> Result<Vec<ValidationIssue>, TestCaseError> {
        let mut issues = Vec::new();
        
        let method = &test_case.request.method.to_uppercase();
        let valid_methods = ["GET", "POST", "PUT", "DELETE", "PATCH", "HEAD", "OPTIONS"];
        
        if !valid_methods.contains(&method.as_str()) {
            issues.push(ValidationIssue {
                rule_id: self.rule_id().to_string(),
                severity: self.severity(),
                message: format!("Invalid HTTP method: {}", method),
                location: ValidationLocation::Request,
                suggestion: Some("Use standard HTTP methods: GET, POST, PUT, DELETE, PATCH, HEAD, OPTIONS".to_string()),
                auto_fixable: false,
            });
        }
        
        Ok(issues)
    }

    fn rule_id(&self) -> &str {
        "http-method-validation"
    }

    fn severity(&self) -> ValidationSeverity {
        ValidationSeverity::Error
    }

    fn category(&self) -> ValidationCategory {
        ValidationCategory::Structure
    }
}

pub struct HeaderValidationRule;

impl HeaderValidationRule {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl ValidationRuleEngine for HeaderValidationRule {
    async fn validate(&self, test_case: &TestCase) -> Result<Vec<ValidationIssue>, TestCaseError> {
        let mut issues = Vec::new();
        
        // Check for Content-Type on POST/PUT/PATCH requests
        let method = test_case.request.method.to_uppercase();
        if ["POST", "PUT", "PATCH"].contains(&method.as_str()) {
            if !test_case.request.headers.contains_key("Content-Type") && 
               !test_case.request.headers.contains_key("content-type") {
                issues.push(ValidationIssue {
                    rule_id: self.rule_id().to_string(),
                    severity: self.severity(),
                    message: format!("{} requests should specify Content-Type header", method),
                    location: ValidationLocation::Request,
                    suggestion: Some("Add Content-Type header (e.g., application/json)".to_string()),
                    auto_fixable: true,
                });
            }
        }
        
        // Check for Accept header
        if !test_case.request.headers.contains_key("Accept") && 
           !test_case.request.headers.contains_key("accept") {
            issues.push(ValidationIssue {
                rule_id: self.rule_id().to_string(),
                severity: ValidationSeverity::Info,
                message: "Consider adding Accept header to specify expected response format".to_string(),
                location: ValidationLocation::Request,
                suggestion: Some("Add Accept header (e.g., application/json)".to_string()),
                auto_fixable: true,
            });
        }
        
        Ok(issues)
    }

    fn rule_id(&self) -> &str {
        "header-validation"
    }

    fn severity(&self) -> ValidationSeverity {
        ValidationSeverity::Warning
    }

    fn category(&self) -> ValidationCategory {
        ValidationCategory::BestPractices
    }
}

pub struct SecurityHeadersRule;

impl SecurityHeadersRule {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl ValidationRuleEngine for SecurityHeadersRule {
    async fn validate(&self, test_case: &TestCase) -> Result<Vec<ValidationIssue>, TestCaseError> {
        let mut issues = Vec::new();
        
        // Check for hardcoded API keys in headers
        for (key, value) in &test_case.request.headers {
            if key.to_lowercase().contains("key") || key.to_lowercase().contains("token") {
                if !value.starts_with("{{") && !value.starts_with("$") {
                    issues.push(ValidationIssue {
                        rule_id: self.rule_id().to_string(),
                        severity: self.severity(),
                        message: format!("Avoid hardcoded credentials in header '{}'", key),
                        location: ValidationLocation::Request,
                        suggestion: Some("Use environment variables or template variables for credentials".to_string()),
                        auto_fixable: false,
                    });
                }
            }
        }
        
        Ok(issues)
    }

    fn rule_id(&self) -> &str {
        "security-headers"
    }

    fn severity(&self) -> ValidationSeverity {
        ValidationSeverity::Error
    }

    fn category(&self) -> ValidationCategory {
        ValidationCategory::Security
    }
}

pub struct PerformanceAssertionRule;

impl PerformanceAssertionRule {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl ValidationRuleEngine for PerformanceAssertionRule {
    async fn validate(&self, test_case: &TestCase) -> Result<Vec<ValidationIssue>, TestCaseError> {
        let mut issues = Vec::new();
        
        let has_performance_assertion = test_case.assertions.iter()
            .any(|a| a.assertion_type == "response_time");
        
        if !has_performance_assertion {
            issues.push(ValidationIssue {
                rule_id: self.rule_id().to_string(),
                severity: self.severity(),
                message: "Consider adding response time assertion for performance monitoring".to_string(),
                location: ValidationLocation::TestCase,
                suggestion: Some("Add response_time assertion to monitor API performance".to_string()),
                auto_fixable: false,
            });
        }
        
        Ok(issues)
    }

    fn rule_id(&self) -> &str {
        "performance-assertion"
    }

    fn severity(&self) -> ValidationSeverity {
        ValidationSeverity::Info
    }

    fn category(&self) -> ValidationCategory {
        ValidationCategory::Performance
    }
}

pub struct VariableNamingRule;

impl VariableNamingRule {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl ValidationRuleEngine for VariableNamingRule {
    async fn validate(&self, test_case: &TestCase) -> Result<Vec<ValidationIssue>, TestCaseError> {
        let mut issues = Vec::new();
        
        if let Some(extractions) = &test_case.variable_extractions {
            for (index, extraction) in extractions.iter().enumerate() {
                // Check variable naming convention (snake_case)
                if !extraction.name.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_') {
                    issues.push(ValidationIssue {
                        rule_id: self.rule_id().to_string(),
                        severity: self.severity(),
                        message: format!("Variable '{}' should use snake_case naming", extraction.name),
                        location: ValidationLocation::VariableExtraction(index),
                        suggestion: Some("Use lowercase letters, numbers, and underscores only".to_string()),
                        auto_fixable: false,
                    });
                }
                
                // Check for descriptive names
                if extraction.name.len() < 3 {
                    issues.push(ValidationIssue {
                        rule_id: self.rule_id().to_string(),
                        severity: ValidationSeverity::Warning,
                        message: format!("Variable name '{}' is too short", extraction.name),
                        location: ValidationLocation::VariableExtraction(index),
                        suggestion: Some("Use descriptive variable names".to_string()),
                        auto_fixable: false,
                    });
                }
            }
        }
        
        Ok(issues)
    }

    fn rule_id(&self) -> &str {
        "variable-naming"
    }

    fn severity(&self) -> ValidationSeverity {
        ValidationSeverity::Warning
    }

    fn category(&self) -> ValidationCategory {
        ValidationCategory::Naming
    }
}

pub struct DuplicateAssertionRule;

impl DuplicateAssertionRule {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl ValidationRuleEngine for DuplicateAssertionRule {
    async fn validate(&self, test_case: &TestCase) -> Result<Vec<ValidationIssue>, TestCaseError> {
        let mut issues = Vec::new();
        let mut seen_assertions = HashMap::new();
        
        for (index, assertion) in test_case.assertions.iter().enumerate() {
            let key = (&assertion.assertion_type, &assertion.expected);
            if let Some(first_index) = seen_assertions.get(&key) {
                issues.push(ValidationIssue {
                    rule_id: self.rule_id().to_string(),
                    severity: self.severity(),
                    message: format!("Duplicate assertion found (same as assertion {})", first_index),
                    location: ValidationLocation::Assertion(index),
                    suggestion: Some("Remove duplicate assertions".to_string()),
                    auto_fixable: true,
                });
            } else {
                seen_assertions.insert(key, index);
            }
        }
        
        Ok(issues)
    }

    fn rule_id(&self) -> &str {
        "duplicate-assertion"
    }

    fn severity(&self) -> ValidationSeverity {
        ValidationSeverity::Warning
    }

    fn category(&self) -> ValidationCategory {
        ValidationCategory::Maintainability
    }
}

pub struct UnusedVariableRule;

impl UnusedVariableRule {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl ValidationRuleEngine for UnusedVariableRule {
    async fn validate(&self, test_case: &TestCase) -> Result<Vec<ValidationIssue>, TestCaseError> {
        let mut issues = Vec::new();
        
        if let Some(extractions) = &test_case.variable_extractions {
            for (index, extraction) in extractions.iter().enumerate() {
                // Check if variable is used anywhere in the test case
                let variable_pattern = format!("{{{{{}}}}}", extraction.name);
                let is_used = test_case.request.url.contains(&variable_pattern) ||
                    test_case.request.headers.values().any(|v| v.contains(&variable_pattern)) ||
                    test_case.request.body.as_ref().map_or(false, |b| b.contains(&variable_pattern)) ||
                    test_case.assertions.iter().any(|a| {
                        a.expected.to_string().contains(&variable_pattern) ||
                        a.message.as_ref().map_or(false, |m| m.contains(&variable_pattern))
                    });
                
                if !is_used {
                    issues.push(ValidationIssue {
                        rule_id: self.rule_id().to_string(),
                        severity: self.severity(),
                        message: format!("Variable '{}' is extracted but never used", extraction.name),
                        location: ValidationLocation::VariableExtraction(index),
                        suggestion: Some("Remove unused variable extraction or use it in subsequent requests".to_string()),
                        auto_fixable: true,
                    });
                }
            }
        }
        
        Ok(issues)
    }

    fn rule_id(&self) -> &str {
        "unused-variable"
    }

    fn severity(&self) -> ValidationSeverity {
        ValidationSeverity::Info
    }

    fn category(&self) -> ValidationCategory {
        ValidationCategory::Maintainability
    }
}

pub struct HardcodedCredentialsRule;

impl HardcodedCredentialsRule {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl ValidationRuleEngine for HardcodedCredentialsRule {
    async fn validate(&self, test_case: &TestCase) -> Result<Vec<ValidationIssue>, TestCaseError> {
        let mut issues = Vec::new();
        
        // Check URL for credentials
        if test_case.request.url.contains("://") {
            if let Some(url_part) = test_case.request.url.split("://").nth(1) {
                if url_part.contains('@') && !url_part.starts_with("{{") {
                    issues.push(ValidationIssue {
                        rule_id: self.rule_id().to_string(),
                        severity: self.severity(),
                        message: "Avoid hardcoded credentials in URL".to_string(),
                        location: ValidationLocation::Request,
                        suggestion: Some("Use authentication configuration or environment variables".to_string()),
                        auto_fixable: false,
                    });
                }
            }
        }
        
        // Check request body for potential credentials
        if let Some(body) = &test_case.request.body {
            let suspicious_patterns = ["password", "secret", "key", "token"];
            for pattern in &suspicious_patterns {
                if body.to_lowercase().contains(pattern) && !body.contains("{{") {
                    issues.push(ValidationIssue {
                        rule_id: self.rule_id().to_string(),
                        severity: self.severity(),
                        message: format!("Potential hardcoded credential found in request body ({})", pattern),
                        location: ValidationLocation::Request,
                        suggestion: Some("Use template variables for sensitive data".to_string()),
                        auto_fixable: false,
                    });
                    break; // Only report once per test case
                }
            }
        }
        
        Ok(issues)
    }

    fn rule_id(&self) -> &str {
        "hardcoded-credentials"
    }

    fn severity(&self) -> ValidationSeverity {
        ValidationSeverity::Error
    }

    fn category(&self) -> ValidationCategory {
        ValidationCategory::Security
    }
}