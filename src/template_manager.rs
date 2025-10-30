use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use std::time::Duration;
use uuid::Uuid;

use crate::error::TestCaseError;
use crate::test_case_manager::{TestCase, RequestDefinition, AssertionDefinition, AuthDefinition, VariableExtraction};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestCaseTemplate {
    pub id: String,
    pub name: String,
    pub description: String,
    pub category: TemplateCategory,
    pub template: TestCaseDefinition,
    pub variables: Vec<TemplateVariable>,
    pub instructions: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub version: String,
    pub author: Option<String>,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestCaseDefinition {
    pub request: RequestDefinition,
    pub assertions: Vec<AssertionDefinition>,
    pub variable_extractions: Option<Vec<VariableExtraction>>,
    pub dependencies: Vec<String>,
    pub timeout: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TemplateCategory {
    RestApi,
    GraphQL,
    Authentication,
    DataValidation,
    Performance,
    Integration,
    Workflow,
    Custom(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateVariable {
    pub name: String,
    pub description: String,
    pub variable_type: VariableType,
    pub default_value: Option<String>,
    pub required: bool,
    pub validation_pattern: Option<String>,
    pub example_values: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum VariableType {
    String,
    Number,
    Boolean,
    Url,
    Email,
    Json,
    Header,
    Path,
    Query,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestCaseFragment {
    pub id: String,
    pub name: String,
    pub description: String,
    pub fragment_type: FragmentType,
    pub content: FragmentContent,
    pub variables: Vec<TemplateVariable>,
    pub tags: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum FragmentType {
    Request,
    Assertion,
    Authentication,
    Headers,
    Body,
    VariableExtraction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FragmentContent {
    Request(RequestDefinition),
    Assertions(Vec<AssertionDefinition>),
    Auth(AuthDefinition),
    Headers(HashMap<String, String>),
    Body(String),
    Variables(Vec<VariableExtraction>),
}

#[derive(Debug, Clone)]
pub struct TemplateFilter {
    pub category: Option<TemplateCategory>,
    pub name_pattern: Option<String>,
    pub tags: Option<Vec<String>>,
    pub author: Option<String>,
    pub created_after: Option<DateTime<Utc>>,
    pub created_before: Option<DateTime<Utc>>,
}

impl Default for TemplateFilter {
    fn default() -> Self {
        Self {
            category: None,
            name_pattern: None,
            tags: None,
            author: None,
            created_after: None,
            created_before: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct TemplateCreationOptions {
    pub generate_id: bool,
    pub set_timestamps: bool,
    pub validate_variables: bool,
    pub author: Option<String>,
}

impl Default for TemplateCreationOptions {
    fn default() -> Self {
        Self {
            generate_id: true,
            set_timestamps: true,
            validate_variables: true,
            author: None,
        }
    }
}

#[async_trait]
pub trait TemplateStorage: Send + Sync {
    async fn save_template(&self, template: &TestCaseTemplate) -> Result<(), TestCaseError>;
    async fn load_template(&self, id: &str) -> Result<TestCaseTemplate, TestCaseError>;
    async fn delete_template(&self, id: &str) -> Result<(), TestCaseError>;
    async fn list_templates(&self, filter: &TemplateFilter) -> Result<Vec<TestCaseTemplate>, TestCaseError>;
    async fn template_exists(&self, id: &str) -> Result<bool, TestCaseError>;
    
    async fn save_fragment(&self, fragment: &TestCaseFragment) -> Result<(), TestCaseError>;
    async fn load_fragment(&self, id: &str) -> Result<TestCaseFragment, TestCaseError>;
    async fn delete_fragment(&self, id: &str) -> Result<(), TestCaseError>;
    async fn list_fragments(&self, fragment_type: Option<FragmentType>) -> Result<Vec<TestCaseFragment>, TestCaseError>;
}

pub struct TestCaseTemplateManager {
    storage: Box<dyn TemplateStorage>,
    builtin_templates: HashMap<String, TestCaseTemplate>,
    builtin_fragments: HashMap<String, TestCaseFragment>,
}

impl TestCaseTemplateManager {
    pub fn new(storage: Box<dyn TemplateStorage>) -> Self {
        let mut manager = Self {
            storage,
            builtin_templates: HashMap::new(),
            builtin_fragments: HashMap::new(),
        };
        
        manager.register_builtin_templates();
        manager.register_builtin_fragments();
        manager
    }

    pub async fn create_template(&self, mut template: TestCaseTemplate, options: TemplateCreationOptions) -> Result<String, TestCaseError> {
        if options.generate_id && template.id.is_empty() {
            template.id = Uuid::new_v4().to_string();
        }

        if options.set_timestamps {
            let now = Utc::now();
            template.created_at = now;
            template.updated_at = now;
        }

        if let Some(author) = options.author {
            template.author = Some(author);
        }

        if options.validate_variables {
            self.validate_template(&template)?;
        }

        // Check if template already exists
        if self.storage.template_exists(&template.id).await? {
            return Err(TestCaseError::AlreadyExists(template.id));
        }

        self.storage.save_template(&template).await?;
        Ok(template.id)
    }

    pub async fn get_template(&self, id: &str) -> Result<TestCaseTemplate, TestCaseError> {
        // First check builtin templates
        if let Some(template) = self.builtin_templates.get(id) {
            return Ok(template.clone());
        }
        
        // Then check storage
        self.storage.load_template(id).await
    }

    pub async fn list_templates(&self, filter: &TemplateFilter) -> Result<Vec<TestCaseTemplate>, TestCaseError> {
        let mut templates = Vec::new();
        
        // Add builtin templates that match filter
        for template in self.builtin_templates.values() {
            if self.template_matches_filter(template, filter) {
                templates.push(template.clone());
            }
        }
        
        // Add custom templates from storage
        let custom_templates = self.storage.list_templates(filter).await?;
        templates.extend(custom_templates);
        
        Ok(templates)
    }

    pub async fn create_from_template(&self, template_id: &str, variables: HashMap<String, String>) -> Result<TestCase, TestCaseError> {
        let template = self.get_template(template_id).await?;
        
        // Validate required variables
        for template_var in &template.variables {
            if template_var.required && !variables.contains_key(&template_var.name) {
                return Err(TestCaseError::ValidationError(
                    format!("Missing required variable: {}", template_var.name)
                ));
            }
        }
        
        // Validate variable values
        for template_var in &template.variables {
            if let Some(value) = variables.get(&template_var.name) {
                self.validate_variable_value(template_var, value)?;
            }
        }
        
        // Substitute variables in template
        let substituted_template = self.substitute_variables(&template.template, &variables)?;
        
        // Create test case from template
        let now = Utc::now();
        Ok(TestCase {
            id: Uuid::new_v4().to_string(),
            name: format!("Generated from {}", template.name),
            description: Some(template.description.clone()),
            tags: template.tags.clone(),
            protocol: substituted_template.request.protocol.clone(),
            request: substituted_template.request,
            assertions: substituted_template.assertions,
            variable_extractions: substituted_template.variable_extractions,
            dependencies: substituted_template.dependencies,
            timeout: substituted_template.timeout.map(Duration::from_secs),
            created_at: now,
            updated_at: now,
            version: 1,
            change_log: vec![],
        })
    }

    pub async fn create_fragment(&self, mut fragment: TestCaseFragment) -> Result<String, TestCaseError> {
        if fragment.id.is_empty() {
            fragment.id = Uuid::new_v4().to_string();
        }

        let now = Utc::now();
        fragment.created_at = now;
        fragment.updated_at = now;

        self.validate_fragment(&fragment)?;

        self.storage.save_fragment(&fragment).await?;
        Ok(fragment.id)
    }

    pub async fn get_fragment(&self, id: &str) -> Result<TestCaseFragment, TestCaseError> {
        // First check builtin fragments
        if let Some(fragment) = self.builtin_fragments.get(id) {
            return Ok(fragment.clone());
        }
        
        // Then check storage
        self.storage.load_fragment(id).await
    }

    pub async fn list_fragments(&self, fragment_type: Option<FragmentType>) -> Result<Vec<TestCaseFragment>, TestCaseError> {
        let mut fragments = Vec::new();
        
        // Add builtin fragments
        for fragment in self.builtin_fragments.values() {
            if fragment_type.is_none() || fragment_type.as_ref() == Some(&fragment.fragment_type) {
                fragments.push(fragment.clone());
            }
        }
        
        // Add custom fragments from storage
        let custom_fragments = self.storage.list_fragments(fragment_type).await?;
        fragments.extend(custom_fragments);
        
        Ok(fragments)
    }

    pub async fn apply_fragment_to_template(&self, template: &mut TestCaseTemplate, fragment_id: &str) -> Result<(), TestCaseError> {
        let fragment = self.get_fragment(fragment_id).await?;
        
        match fragment.content {
            FragmentContent::Request(request) => {
                template.template.request = request;
            }
            FragmentContent::Assertions(assertions) => {
                template.template.assertions.extend(assertions);
            }
            FragmentContent::Auth(auth) => {
                template.template.request.auth = Some(auth);
            }
            FragmentContent::Headers(headers) => {
                template.template.request.headers.extend(headers);
            }
            FragmentContent::Body(body) => {
                template.template.request.body = Some(body);
            }
            FragmentContent::Variables(variables) => {
                if template.template.variable_extractions.is_none() {
                    template.template.variable_extractions = Some(Vec::new());
                }
                template.template.variable_extractions.as_mut().unwrap().extend(variables);
            }
        }
        
        // Merge fragment variables with template variables
        template.variables.extend(fragment.variables);
        
        Ok(())
    }

    fn register_builtin_templates(&mut self) {
        // REST API GET template
        let get_template = self.create_rest_get_template();
        self.builtin_templates.insert(get_template.id.clone(), get_template);
        
        // REST API POST template
        let post_template = self.create_rest_post_template();
        self.builtin_templates.insert(post_template.id.clone(), post_template);
        
        // Authentication templates
        let oauth_template = self.create_oauth_template();
        self.builtin_templates.insert(oauth_template.id.clone(), oauth_template);
        
        let jwt_template = self.create_jwt_template();
        self.builtin_templates.insert(jwt_template.id.clone(), jwt_template);
        
        // Data validation template
        let validation_template = self.create_validation_template();
        self.builtin_templates.insert(validation_template.id.clone(), validation_template);
        
        // Performance testing template
        let performance_template = self.create_performance_template();
        self.builtin_templates.insert(performance_template.id.clone(), performance_template);
    }

    fn register_builtin_fragments(&mut self) {
        // Common headers fragment
        let headers_fragment = self.create_common_headers_fragment();
        self.builtin_fragments.insert(headers_fragment.id.clone(), headers_fragment);
        
        // JSON assertions fragment
        let json_assertions_fragment = self.create_json_assertions_fragment();
        self.builtin_fragments.insert(json_assertions_fragment.id.clone(), json_assertions_fragment);
        
        // Performance assertions fragment
        let perf_assertions_fragment = self.create_performance_assertions_fragment();
        self.builtin_fragments.insert(perf_assertions_fragment.id.clone(), perf_assertions_fragment);
        
        // OAuth authentication fragment
        let oauth_auth_fragment = self.create_oauth_auth_fragment();
        self.builtin_fragments.insert(oauth_auth_fragment.id.clone(), oauth_auth_fragment);
    }

    fn substitute_variables(&self, template: &TestCaseDefinition, variables: &HashMap<String, String>) -> Result<TestCaseDefinition, TestCaseError> {
        let template_json = serde_json::to_string(template)
            .map_err(|e| TestCaseError::SerializationError(e.to_string()))?;
        
        let mut substituted_json = template_json;
        for (var_name, var_value) in variables {
            substituted_json = substituted_json.replace(&format!("{{{{{}}}}}", var_name), var_value);
        }
        
        let mut result: TestCaseDefinition = serde_json::from_str(&substituted_json)
            .map_err(|e| TestCaseError::SerializationError(e.to_string()))?;
        
        // Handle special numeric substitutions for timeout
        if let Some(max_response_time) = variables.get("max_response_time") {
            if let Ok(timeout_value) = max_response_time.parse::<u64>() {
                result.timeout = Some(timeout_value);
            }
        }
        
        Ok(result)
    }

    fn validate_template(&self, template: &TestCaseTemplate) -> Result<(), TestCaseError> {
        if template.name.trim().is_empty() {
            return Err(TestCaseError::ValidationError("Template name cannot be empty".to_string()));
        }

        if template.description.trim().is_empty() {
            return Err(TestCaseError::ValidationError("Template description cannot be empty".to_string()));
        }

        // Validate template variables
        for variable in &template.variables {
            self.validate_template_variable(variable)?;
        }

        Ok(())
    }

    fn validate_template_variable(&self, variable: &TemplateVariable) -> Result<(), TestCaseError> {
        if variable.name.trim().is_empty() {
            return Err(TestCaseError::ValidationError("Variable name cannot be empty".to_string()));
        }

        if variable.description.trim().is_empty() {
            return Err(TestCaseError::ValidationError("Variable description cannot be empty".to_string()));
        }

        // Validate default value if present
        if let Some(default_value) = &variable.default_value {
            self.validate_variable_value(variable, default_value)?;
        }

        Ok(())
    }

    fn validate_variable_value(&self, variable: &TemplateVariable, value: &str) -> Result<(), TestCaseError> {
        // Validate based on variable type
        match variable.variable_type {
            VariableType::Number => {
                value.parse::<f64>().map_err(|_| {
                    TestCaseError::ValidationError(format!("Variable '{}' must be a number", variable.name))
                })?;
            }
            VariableType::Boolean => {
                value.parse::<bool>().map_err(|_| {
                    TestCaseError::ValidationError(format!("Variable '{}' must be a boolean", variable.name))
                })?;
            }
            VariableType::Url => {
                if !value.starts_with("http://") && !value.starts_with("https://") {
                    return Err(TestCaseError::ValidationError(
                        format!("Variable '{}' must be a valid URL", variable.name)
                    ));
                }
            }
            VariableType::Email => {
                if !value.contains('@') {
                    return Err(TestCaseError::ValidationError(
                        format!("Variable '{}' must be a valid email", variable.name)
                    ));
                }
            }
            VariableType::Json => {
                serde_json::from_str::<serde_json::Value>(value).map_err(|_| {
                    TestCaseError::ValidationError(format!("Variable '{}' must be valid JSON", variable.name))
                })?;
            }
            _ => {} // String, Header, Path, Query don't need special validation
        }

        // Validate against pattern if present
        if let Some(pattern) = &variable.validation_pattern {
            let regex = regex::Regex::new(pattern).map_err(|e| {
                TestCaseError::ValidationError(format!("Invalid validation pattern for variable '{}': {}", variable.name, e))
            })?;
            
            if !regex.is_match(value) {
                return Err(TestCaseError::ValidationError(
                    format!("Variable '{}' value '{}' does not match pattern '{}'", variable.name, value, pattern)
                ));
            }
        }

        Ok(())
    }

    fn validate_fragment(&self, fragment: &TestCaseFragment) -> Result<(), TestCaseError> {
        if fragment.name.trim().is_empty() {
            return Err(TestCaseError::ValidationError("Fragment name cannot be empty".to_string()));
        }

        if fragment.description.trim().is_empty() {
            return Err(TestCaseError::ValidationError("Fragment description cannot be empty".to_string()));
        }

        // Validate fragment variables
        for variable in &fragment.variables {
            self.validate_template_variable(variable)?;
        }

        Ok(())
    }

    fn template_matches_filter(&self, template: &TestCaseTemplate, filter: &TemplateFilter) -> bool {
        if let Some(category) = &filter.category {
            if &template.category != category {
                return false;
            }
        }

        if let Some(name_pattern) = &filter.name_pattern {
            if !template.name.to_lowercase().contains(&name_pattern.to_lowercase()) {
                return false;
            }
        }

        if let Some(tags) = &filter.tags {
            if !tags.iter().any(|tag| template.tags.contains(tag)) {
                return false;
            }
        }

        if let Some(author) = &filter.author {
            if template.author.as_ref() != Some(author) {
                return false;
            }
        }

        if let Some(created_after) = filter.created_after {
            if template.created_at <= created_after {
                return false;
            }
        }

        if let Some(created_before) = filter.created_before {
            if template.created_at >= created_before {
                return false;
            }
        }

        true
    }

    // Built-in template creation methods
    fn create_rest_get_template(&self) -> TestCaseTemplate {
        TestCaseTemplate {
            id: "rest-api-get".to_string(),
            name: "REST API GET Request".to_string(),
            description: "Basic GET request template for REST APIs with common assertions".to_string(),
            category: TemplateCategory::RestApi,
            template: TestCaseDefinition {
                request: RequestDefinition {
                    protocol: "http".to_string(),
                    method: "GET".to_string(),
                    url: "{{base_url}}/{{endpoint}}".to_string(),
                    headers: HashMap::from([
                        ("Accept".to_string(), "application/json".to_string()),
                        ("User-Agent".to_string(), "API-Test-Runner/1.0".to_string()),
                    ]),
                    body: None,
                    auth: None,
                },
                assertions: vec![
                    AssertionDefinition {
                        assertion_type: "status_code".to_string(),
                        expected: serde_json::json!(200),
                        message: Some("Response should be successful".to_string()),
                    },
                    AssertionDefinition {
                        assertion_type: "response_time".to_string(),
                        expected: serde_json::json!(1000),
                        message: Some("Response should be under 1 second".to_string()),
                    },
                ],
                variable_extractions: None,
                dependencies: vec![],
                timeout: Some(30),
            },
            variables: vec![
                TemplateVariable {
                    name: "base_url".to_string(),
                    description: "Base URL of the API".to_string(),
                    variable_type: VariableType::Url,
                    default_value: Some("https://api.example.com".to_string()),
                    required: true,
                    validation_pattern: Some(r"^https?://.*".to_string()),
                    example_values: vec![
                        "https://api.example.com".to_string(),
                        "http://localhost:8080".to_string(),
                    ],
                },
                TemplateVariable {
                    name: "endpoint".to_string(),
                    description: "API endpoint path".to_string(),
                    variable_type: VariableType::Path,
                    default_value: Some("users".to_string()),
                    required: true,
                    validation_pattern: None,
                    example_values: vec![
                        "users".to_string(),
                        "products".to_string(),
                        "orders".to_string(),
                    ],
                },
            ],
            instructions: Some("This template creates a basic GET request. Fill in the base_url and endpoint variables. The template includes standard success and performance assertions.".to_string()),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            version: "1.0.0".to_string(),
            author: Some("System".to_string()),
            tags: vec!["rest".to_string(), "get".to_string(), "basic".to_string()],
        }
    }

    fn create_rest_post_template(&self) -> TestCaseTemplate {
        TestCaseTemplate {
            id: "rest-api-post".to_string(),
            name: "REST API POST Request".to_string(),
            description: "POST request template for creating resources with JSON payload".to_string(),
            category: TemplateCategory::RestApi,
            template: TestCaseDefinition {
                request: RequestDefinition {
                    protocol: "http".to_string(),
                    method: "POST".to_string(),
                    url: "{{base_url}}/{{endpoint}}".to_string(),
                    headers: HashMap::from([
                        ("Accept".to_string(), "application/json".to_string()),
                        ("Content-Type".to_string(), "application/json".to_string()),
                        ("User-Agent".to_string(), "API-Test-Runner/1.0".to_string()),
                    ]),
                    body: Some("{{request_body}}".to_string()),
                    auth: None,
                },
                assertions: vec![
                    AssertionDefinition {
                        assertion_type: "status_code".to_string(),
                        expected: serde_json::json!(201),
                        message: Some("Resource should be created successfully".to_string()),
                    },
                    AssertionDefinition {
                        assertion_type: "json_path".to_string(),
                        expected: serde_json::json!("{{expected_id}}"),
                        message: Some("Response should contain the created resource ID".to_string()),
                    },
                ],
                variable_extractions: Some(vec![
                    VariableExtraction {
                        name: "created_id".to_string(),
                        source: crate::test_case_manager::ExtractionSource::ResponseBody,
                        path: "$.id".to_string(),
                        default_value: None,
                    },
                ]),
                dependencies: vec![],
                timeout: Some(30),
            },
            variables: vec![
                TemplateVariable {
                    name: "base_url".to_string(),
                    description: "Base URL of the API".to_string(),
                    variable_type: VariableType::Url,
                    default_value: Some("https://api.example.com".to_string()),
                    required: true,
                    validation_pattern: Some(r"^https?://.*".to_string()),
                    example_values: vec!["https://api.example.com".to_string()],
                },
                TemplateVariable {
                    name: "endpoint".to_string(),
                    description: "API endpoint path for resource creation".to_string(),
                    variable_type: VariableType::Path,
                    default_value: Some("users".to_string()),
                    required: true,
                    validation_pattern: None,
                    example_values: vec!["users".to_string(), "products".to_string()],
                },
                TemplateVariable {
                    name: "request_body".to_string(),
                    description: "JSON payload for the POST request".to_string(),
                    variable_type: VariableType::Json,
                    default_value: Some(r#"{"name": "John Doe", "email": "john@example.com"}"#.to_string()),
                    required: true,
                    validation_pattern: None,
                    example_values: vec![r#"{"name": "John Doe", "email": "john@example.com"}"#.to_string()],
                },
                TemplateVariable {
                    name: "expected_id".to_string(),
                    description: "Expected ID pattern in response".to_string(),
                    variable_type: VariableType::String,
                    default_value: Some("*".to_string()),
                    required: false,
                    validation_pattern: None,
                    example_values: vec!["*".to_string(), "123".to_string()],
                },
            ],
            instructions: Some("This template creates a POST request for resource creation. Provide the request body as JSON and specify the expected response structure.".to_string()),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            version: "1.0.0".to_string(),
            author: Some("System".to_string()),
            tags: vec!["rest".to_string(), "post".to_string(), "create".to_string()],
        }
    }

    fn create_oauth_template(&self) -> TestCaseTemplate {
        TestCaseTemplate {
            id: "oauth2-auth".to_string(),
            name: "OAuth 2.0 Authentication".to_string(),
            description: "Template for testing OAuth 2.0 authentication flow".to_string(),
            category: TemplateCategory::Authentication,
            template: TestCaseDefinition {
                request: RequestDefinition {
                    protocol: "http".to_string(),
                    method: "POST".to_string(),
                    url: "{{auth_server}}/oauth/token".to_string(),
                    headers: HashMap::from([
                        ("Content-Type".to_string(), "application/x-www-form-urlencoded".to_string()),
                        ("Accept".to_string(), "application/json".to_string()),
                    ]),
                    body: Some("grant_type=client_credentials&client_id={{client_id}}&client_secret={{client_secret}}&scope={{scope}}".to_string()),
                    auth: None,
                },
                assertions: vec![
                    AssertionDefinition {
                        assertion_type: "status_code".to_string(),
                        expected: serde_json::json!(200),
                        message: Some("OAuth token request should be successful".to_string()),
                    },
                    AssertionDefinition {
                        assertion_type: "json_path".to_string(),
                        expected: serde_json::json!("Bearer"),
                        message: Some("Token type should be Bearer".to_string()),
                    },
                    AssertionDefinition {
                        assertion_type: "json_path".to_string(),
                        expected: serde_json::json!("*"),
                        message: Some("Access token should be present".to_string()),
                    },
                ],
                variable_extractions: Some(vec![
                    VariableExtraction {
                        name: "access_token".to_string(),
                        source: crate::test_case_manager::ExtractionSource::ResponseBody,
                        path: "$.access_token".to_string(),
                        default_value: None,
                    },
                    VariableExtraction {
                        name: "token_type".to_string(),
                        source: crate::test_case_manager::ExtractionSource::ResponseBody,
                        path: "$.token_type".to_string(),
                        default_value: None,
                    },
                ]),
                dependencies: vec![],
                timeout: Some(30),
            },
            variables: vec![
                TemplateVariable {
                    name: "auth_server".to_string(),
                    description: "OAuth 2.0 authorization server URL".to_string(),
                    variable_type: VariableType::Url,
                    default_value: Some("https://auth.example.com".to_string()),
                    required: true,
                    validation_pattern: Some(r"^https?://.*".to_string()),
                    example_values: vec!["https://auth.example.com".to_string()],
                },
                TemplateVariable {
                    name: "client_id".to_string(),
                    description: "OAuth 2.0 client identifier".to_string(),
                    variable_type: VariableType::String,
                    default_value: None,
                    required: true,
                    validation_pattern: None,
                    example_values: vec!["my-client-id".to_string()],
                },
                TemplateVariable {
                    name: "client_secret".to_string(),
                    description: "OAuth 2.0 client secret".to_string(),
                    variable_type: VariableType::String,
                    default_value: None,
                    required: true,
                    validation_pattern: None,
                    example_values: vec!["my-client-secret".to_string()],
                },
                TemplateVariable {
                    name: "scope".to_string(),
                    description: "OAuth 2.0 scope for the access token".to_string(),
                    variable_type: VariableType::String,
                    default_value: Some("read write".to_string()),
                    required: false,
                    validation_pattern: None,
                    example_values: vec!["read".to_string(), "read write".to_string()],
                },
            ],
            instructions: Some("This template tests OAuth 2.0 client credentials flow. Provide your client credentials and the authorization server URL.".to_string()),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            version: "1.0.0".to_string(),
            author: Some("System".to_string()),
            tags: vec!["oauth".to_string(), "authentication".to_string(), "security".to_string()],
        }
    }

    fn create_jwt_template(&self) -> TestCaseTemplate {
        TestCaseTemplate {
            id: "jwt-auth".to_string(),
            name: "JWT Authentication".to_string(),
            description: "Template for testing JWT token-based authentication".to_string(),
            category: TemplateCategory::Authentication,
            template: TestCaseDefinition {
                request: RequestDefinition {
                    protocol: "http".to_string(),
                    method: "GET".to_string(),
                    url: "{{base_url}}/{{protected_endpoint}}".to_string(),
                    headers: HashMap::from([
                        ("Accept".to_string(), "application/json".to_string()),
                        ("Authorization".to_string(), "Bearer {{jwt_token}}".to_string()),
                    ]),
                    body: None,
                    auth: Some(AuthDefinition {
                        auth_type: "jwt".to_string(),
                        config: HashMap::from([
                            ("token".to_string(), "{{jwt_token}}".to_string()),
                        ]),
                    }),
                },
                assertions: vec![
                    AssertionDefinition {
                        assertion_type: "status_code".to_string(),
                        expected: serde_json::json!(200),
                        message: Some("Request with valid JWT should be successful".to_string()),
                    },
                    AssertionDefinition {
                        assertion_type: "header".to_string(),
                        expected: serde_json::json!("application/json"),
                        message: Some("Response should be JSON".to_string()),
                    },
                ],
                variable_extractions: None,
                dependencies: vec![],
                timeout: Some(30),
            },
            variables: vec![
                TemplateVariable {
                    name: "base_url".to_string(),
                    description: "Base URL of the protected API".to_string(),
                    variable_type: VariableType::Url,
                    default_value: Some("https://api.example.com".to_string()),
                    required: true,
                    validation_pattern: Some(r"^https?://.*".to_string()),
                    example_values: vec!["https://api.example.com".to_string()],
                },
                TemplateVariable {
                    name: "protected_endpoint".to_string(),
                    description: "Protected endpoint that requires JWT authentication".to_string(),
                    variable_type: VariableType::Path,
                    default_value: Some("profile".to_string()),
                    required: true,
                    validation_pattern: None,
                    example_values: vec!["profile".to_string(), "dashboard".to_string()],
                },
                TemplateVariable {
                    name: "jwt_token".to_string(),
                    description: "Valid JWT token for authentication".to_string(),
                    variable_type: VariableType::String,
                    default_value: None,
                    required: true,
                    validation_pattern: Some(r"^[A-Za-z0-9-_]+\.[A-Za-z0-9-_]+\.[A-Za-z0-9-_]+$".to_string()),
                    example_values: vec!["eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9...".to_string()],
                },
            ],
            instructions: Some("This template tests JWT authentication. Provide a valid JWT token and the protected endpoint URL.".to_string()),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            version: "1.0.0".to_string(),
            author: Some("System".to_string()),
            tags: vec!["jwt".to_string(), "authentication".to_string(), "bearer".to_string()],
        }
    }

    fn create_validation_template(&self) -> TestCaseTemplate {
        TestCaseTemplate {
            id: "data-validation".to_string(),
            name: "Data Validation".to_string(),
            description: "Comprehensive template for validating API response data structure and content".to_string(),
            category: TemplateCategory::DataValidation,
            template: TestCaseDefinition {
                request: RequestDefinition {
                    protocol: "http".to_string(),
                    method: "GET".to_string(),
                    url: "{{base_url}}/{{endpoint}}".to_string(),
                    headers: HashMap::from([
                        ("Accept".to_string(), "application/json".to_string()),
                    ]),
                    body: None,
                    auth: None,
                },
                assertions: vec![
                    AssertionDefinition {
                        assertion_type: "status_code".to_string(),
                        expected: serde_json::json!(200),
                        message: Some("Response should be successful".to_string()),
                    },
                    AssertionDefinition {
                        assertion_type: "json_schema".to_string(),
                        expected: serde_json::json!({"type": "object", "required": ["{{required_fields}}"]}),
                        message: Some("Response should match expected schema".to_string()),
                    },
                    AssertionDefinition {
                        assertion_type: "json_path".to_string(),
                        expected: serde_json::json!("{{expected_value}}"),
                        message: Some("Specific field should have expected value".to_string()),
                    },
                ],
                variable_extractions: Some(vec![
                    VariableExtraction {
                        name: "extracted_value".to_string(),
                        source: crate::test_case_manager::ExtractionSource::ResponseBody,
                        path: "$.{{extraction_path}}".to_string(),
                        default_value: None,
                    },
                ]),
                dependencies: vec![],
                timeout: Some(30),
            },
            variables: vec![
                TemplateVariable {
                    name: "base_url".to_string(),
                    description: "Base URL of the API".to_string(),
                    variable_type: VariableType::Url,
                    default_value: Some("https://api.example.com".to_string()),
                    required: true,
                    validation_pattern: Some(r"^https?://.*".to_string()),
                    example_values: vec!["https://api.example.com".to_string()],
                },
                TemplateVariable {
                    name: "endpoint".to_string(),
                    description: "API endpoint to validate".to_string(),
                    variable_type: VariableType::Path,
                    default_value: Some("users/1".to_string()),
                    required: true,
                    validation_pattern: None,
                    example_values: vec!["users/1".to_string(), "products/123".to_string()],
                },
                TemplateVariable {
                    name: "required_fields".to_string(),
                    description: "Comma-separated list of required fields in response".to_string(),
                    variable_type: VariableType::String,
                    default_value: Some("id,name,email".to_string()),
                    required: true,
                    validation_pattern: None,
                    example_values: vec!["id,name,email".to_string(), "id,title,price".to_string()],
                },
                TemplateVariable {
                    name: "expected_value".to_string(),
                    description: "Expected value for specific field validation".to_string(),
                    variable_type: VariableType::String,
                    default_value: None,
                    required: false,
                    validation_pattern: None,
                    example_values: vec!["active".to_string(), "published".to_string()],
                },
                TemplateVariable {
                    name: "extraction_path".to_string(),
                    description: "JSONPath for extracting value from response".to_string(),
                    variable_type: VariableType::String,
                    default_value: Some("id".to_string()),
                    required: false,
                    validation_pattern: None,
                    example_values: vec!["id".to_string(), "user.profile.name".to_string()],
                },
            ],
            instructions: Some("This template provides comprehensive data validation including schema validation, field presence checks, and value extraction.".to_string()),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            version: "1.0.0".to_string(),
            author: Some("System".to_string()),
            tags: vec!["validation".to_string(), "schema".to_string(), "data".to_string()],
        }
    }

    fn create_performance_template(&self) -> TestCaseTemplate {
        TestCaseTemplate {
            id: "performance-test".to_string(),
            name: "Performance Test".to_string(),
            description: "Template for performance testing with response time and throughput validation".to_string(),
            category: TemplateCategory::Performance,
            template: TestCaseDefinition {
                request: RequestDefinition {
                    protocol: "http".to_string(),
                    method: "GET".to_string(),
                    url: "{{base_url}}/{{endpoint}}".to_string(),
                    headers: HashMap::from([
                        ("Accept".to_string(), "application/json".to_string()),
                        ("User-Agent".to_string(), "API-Test-Runner-Performance/1.0".to_string()),
                    ]),
                    body: None,
                    auth: None,
                },
                assertions: vec![
                    AssertionDefinition {
                        assertion_type: "status_code".to_string(),
                        expected: serde_json::json!(200),
                        message: Some("Response should be successful".to_string()),
                    },
                    AssertionDefinition {
                        assertion_type: "response_time".to_string(),
                        expected: serde_json::json!("{{max_response_time}}"),
                        message: Some("Response time should be within acceptable limits".to_string()),
                    },
                    AssertionDefinition {
                        assertion_type: "header".to_string(),
                        expected: serde_json::json!("*"),
                        message: Some("Response should include performance headers".to_string()),
                    },
                ],
                variable_extractions: Some(vec![
                    VariableExtraction {
                        name: "response_time".to_string(),
                        source: crate::test_case_manager::ExtractionSource::ResponseBody,
                        path: "duration_ms".to_string(),
                        default_value: None,
                    },
                ]),
                dependencies: vec![],
                timeout: None, // Will be set during substitution
            },
            variables: vec![
                TemplateVariable {
                    name: "base_url".to_string(),
                    description: "Base URL of the API to performance test".to_string(),
                    variable_type: VariableType::Url,
                    default_value: Some("https://api.example.com".to_string()),
                    required: true,
                    validation_pattern: Some(r"^https?://.*".to_string()),
                    example_values: vec!["https://api.example.com".to_string()],
                },
                TemplateVariable {
                    name: "endpoint".to_string(),
                    description: "API endpoint to performance test".to_string(),
                    variable_type: VariableType::Path,
                    default_value: Some("health".to_string()),
                    required: true,
                    validation_pattern: None,
                    example_values: vec!["health".to_string(), "users".to_string()],
                },
                TemplateVariable {
                    name: "max_response_time".to_string(),
                    description: "Maximum acceptable response time in milliseconds".to_string(),
                    variable_type: VariableType::Number,
                    default_value: Some("1000".to_string()),
                    required: true,
                    validation_pattern: Some(r"^\d+$".to_string()),
                    example_values: vec!["500".to_string(), "1000".to_string(), "2000".to_string()],
                },
            ],
            instructions: Some("This template tests API performance by validating response times. Set the maximum acceptable response time threshold.".to_string()),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            version: "1.0.0".to_string(),
            author: Some("System".to_string()),
            tags: vec!["performance".to_string(), "response-time".to_string(), "load".to_string()],
        }
    }

    // Built-in fragment creation methods
    fn create_common_headers_fragment(&self) -> TestCaseFragment {
        TestCaseFragment {
            id: "common-headers".to_string(),
            name: "Common HTTP Headers".to_string(),
            description: "Standard HTTP headers for API requests".to_string(),
            fragment_type: FragmentType::Headers,
            content: FragmentContent::Headers(HashMap::from([
                ("Accept".to_string(), "application/json".to_string()),
                ("Content-Type".to_string(), "application/json".to_string()),
                ("User-Agent".to_string(), "API-Test-Runner/1.0".to_string()),
                ("Cache-Control".to_string(), "no-cache".to_string()),
            ])),
            variables: vec![],
            tags: vec!["headers".to_string(), "common".to_string()],
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    fn create_json_assertions_fragment(&self) -> TestCaseFragment {
        TestCaseFragment {
            id: "json-assertions".to_string(),
            name: "JSON Response Assertions".to_string(),
            description: "Common assertions for JSON API responses".to_string(),
            fragment_type: FragmentType::Assertion,
            content: FragmentContent::Assertions(vec![
                AssertionDefinition {
                    assertion_type: "header".to_string(),
                    expected: serde_json::json!("application/json"),
                    message: Some("Response should be JSON".to_string()),
                },
                AssertionDefinition {
                    assertion_type: "json_path".to_string(),
                    expected: serde_json::json!("*"),
                    message: Some("Response should be valid JSON".to_string()),
                },
            ]),
            variables: vec![],
            tags: vec!["json".to_string(), "assertions".to_string()],
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    fn create_performance_assertions_fragment(&self) -> TestCaseFragment {
        TestCaseFragment {
            id: "performance-assertions".to_string(),
            name: "Performance Assertions".to_string(),
            description: "Standard performance-related assertions".to_string(),
            fragment_type: FragmentType::Assertion,
            content: FragmentContent::Assertions(vec![
                AssertionDefinition {
                    assertion_type: "response_time".to_string(),
                    expected: serde_json::json!(1000),
                    message: Some("Response should be fast".to_string()),
                },
                AssertionDefinition {
                    assertion_type: "header".to_string(),
                    expected: serde_json::json!("*"),
                    message: Some("Response should include server timing headers".to_string()),
                },
            ]),
            variables: vec![
                TemplateVariable {
                    name: "max_response_time".to_string(),
                    description: "Maximum acceptable response time in milliseconds".to_string(),
                    variable_type: VariableType::Number,
                    default_value: Some("1000".to_string()),
                    required: true,
                    validation_pattern: Some(r"^\d+$".to_string()),
                    example_values: vec!["500".to_string(), "1000".to_string()],
                },
            ],
            tags: vec!["performance".to_string(), "assertions".to_string()],
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    fn create_oauth_auth_fragment(&self) -> TestCaseFragment {
        TestCaseFragment {
            id: "oauth-auth".to_string(),
            name: "OAuth 2.0 Authentication".to_string(),
            description: "OAuth 2.0 Bearer token authentication fragment".to_string(),
            fragment_type: FragmentType::Authentication,
            content: FragmentContent::Auth(AuthDefinition {
                auth_type: "oauth2".to_string(),
                config: HashMap::from([
                    ("token".to_string(), "{{access_token}}".to_string()),
                    ("token_type".to_string(), "Bearer".to_string()),
                ]),
            }),
            variables: vec![
                TemplateVariable {
                    name: "access_token".to_string(),
                    description: "OAuth 2.0 access token".to_string(),
                    variable_type: VariableType::String,
                    default_value: None,
                    required: true,
                    validation_pattern: None,
                    example_values: vec!["eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9...".to_string()],
                },
            ],
            tags: vec!["oauth".to_string(), "authentication".to_string()],
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }
}