use async_trait::async_trait;
use serde::Deserialize;
use std::collections::HashMap;
use std::time::Duration;
use tokio::fs;
use uuid::Uuid;
use chrono::Utc;

use crate::error::TestCaseError;
use crate::test_case_manager::{TestCase, RequestDefinition, AssertionDefinition, AuthDefinition};
use super::{TestCaseImporter, ImportFormat, ImportSource, ImportOptions};

#[derive(Debug, Clone, Deserialize)]
pub struct OpenApiSpec {
    pub openapi: String,
    pub info: OpenApiInfo,
    pub servers: Option<Vec<OpenApiServer>>,
    pub paths: HashMap<String, OpenApiPathItem>,
    pub components: Option<OpenApiComponents>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OpenApiInfo {
    pub title: String,
    pub version: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OpenApiServer {
    pub url: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OpenApiPathItem {
    pub get: Option<OpenApiOperation>,
    pub post: Option<OpenApiOperation>,
    pub put: Option<OpenApiOperation>,
    pub delete: Option<OpenApiOperation>,
    pub patch: Option<OpenApiOperation>,
    pub head: Option<OpenApiOperation>,
    pub options: Option<OpenApiOperation>,
    pub parameters: Option<Vec<OpenApiParameter>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OpenApiOperation {
    pub summary: Option<String>,
    pub description: Option<String>,
    #[serde(rename = "operationId")]
    pub operation_id: Option<String>,
    pub tags: Option<Vec<String>>,
    pub parameters: Option<Vec<OpenApiParameter>>,
    #[serde(rename = "requestBody")]
    pub request_body: Option<OpenApiRequestBody>,
    pub responses: HashMap<String, OpenApiResponse>,
    pub security: Option<Vec<HashMap<String, Vec<String>>>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OpenApiParameter {
    pub name: String,
    #[serde(rename = "in")]
    pub location: String, // query, header, path, cookie
    pub required: Option<bool>,
    pub description: Option<String>,
    pub schema: Option<OpenApiSchema>,
    pub example: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OpenApiRequestBody {
    pub description: Option<String>,
    pub content: HashMap<String, OpenApiMediaType>,
    pub required: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OpenApiResponse {
    pub description: String,
    pub content: Option<HashMap<String, OpenApiMediaType>>,
    pub headers: Option<HashMap<String, OpenApiHeader>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OpenApiMediaType {
    pub schema: Option<OpenApiSchema>,
    pub example: Option<serde_json::Value>,
    pub examples: Option<HashMap<String, OpenApiExample>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OpenApiSchema {
    #[serde(rename = "type")]
    pub schema_type: Option<String>,
    pub format: Option<String>,
    pub properties: Option<HashMap<String, OpenApiSchema>>,
    pub items: Option<Box<OpenApiSchema>>,
    pub required: Option<Vec<String>>,
    pub example: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OpenApiHeader {
    pub description: Option<String>,
    pub schema: Option<OpenApiSchema>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OpenApiExample {
    pub summary: Option<String>,
    pub description: Option<String>,
    pub value: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OpenApiComponents {
    pub schemas: Option<HashMap<String, OpenApiSchema>>,
    #[serde(rename = "securitySchemes")]
    pub security_schemes: Option<HashMap<String, OpenApiSecurityScheme>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OpenApiSecurityScheme {
    #[serde(rename = "type")]
    pub scheme_type: String,
    pub scheme: Option<String>,
    #[serde(rename = "bearerFormat")]
    pub bearer_format: Option<String>,
    pub name: Option<String>,
    #[serde(rename = "in")]
    pub location: Option<String>,
}

pub struct OpenApiImporter {
    // Configuration for the importer
}

impl OpenApiImporter {
    pub fn new() -> Self {
        Self {}
    }

    fn convert_operation_to_test_case(
        &self,
        path: &str,
        method: &str,
        operation: &OpenApiOperation,
        spec: &OpenApiSpec,
        options: &ImportOptions,
    ) -> Result<TestCase, TestCaseError> {
        let name = operation.operation_id
            .clone()
            .or_else(|| operation.summary.clone())
            .unwrap_or_else(|| format!("{} {}", method.to_uppercase(), path));

        let base_url = self.get_base_url(spec, options);
        let full_url = format!("{}{}", base_url, path);

        let mut test_case = TestCase {
            id: Uuid::new_v4().to_string(),
            name,
            description: operation.description.clone(),
            tags: self.extract_tags(operation, options),
            protocol: "http".to_string(),
            request: self.convert_operation_to_request(path, method, operation, &full_url, options)?,
            assertions: if options.generate_assertions {
                self.generate_assertions_from_responses(&operation.responses)
            } else {
                Vec::new()
            },
            variable_extractions: Some(Vec::new()), // OpenAPI doesn't define variable extractions
            dependencies: Vec::new(),
            timeout: Some(Duration::from_millis(30000)),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            version: 1,
            change_log: vec![],
        };

        // Add folder path as tag if present
        if let Some(folder_path) = self.get_folder_path(&operation.tags, options) {
            test_case.tags.push(format!("folder:{}", folder_path));
        }
        
        // Add operation ID as tag if present
        if let Some(operation_id) = &operation.operation_id {
            test_case.tags.push(format!("operation:{}", operation_id));
        }

        Ok(test_case)
    }

    fn get_base_url(&self, spec: &OpenApiSpec, options: &ImportOptions) -> String {
        if let Some(base_url) = &options.base_url_override {
            base_url.clone()
        } else if let Some(servers) = &spec.servers {
            if let Some(server) = servers.first() {
                server.url.clone()
            } else {
                "https://api.example.com".to_string()
            }
        } else {
            "https://api.example.com".to_string()
        }
    }

    fn get_folder_path(&self, tags: &Option<Vec<String>>, options: &ImportOptions) -> Option<String> {
        if let Some(tags) = tags {
            if let Some(tag) = tags.first() {
                if let Some(prefix) = &options.tag_prefix {
                    Some(format!("{}/{}", prefix, tag))
                } else {
                    Some(tag.clone())
                }
            } else {
                None
            }
        } else {
            None
        }
    }

    fn extract_tags(&self, operation: &OpenApiOperation, options: &ImportOptions) -> Vec<String> {
        let mut tags = Vec::new();

        // Add prefix tag if specified
        if let Some(prefix) = &options.tag_prefix {
            tags.push(format!("{}:openapi", prefix));
        } else {
            tags.push("openapi".to_string());
        }

        // Add operation tags
        if let Some(op_tags) = &operation.tags {
            tags.extend(op_tags.clone());
        }

        tags
    }

    fn convert_operation_to_request(
        &self,
        path: &str,
        method: &str,
        operation: &OpenApiOperation,
        full_url: &str,
        options: &ImportOptions,
    ) -> Result<RequestDefinition, TestCaseError> {
        let mut headers = HashMap::new();
        let mut final_url = full_url.to_string();
        let mut body = None;

        // Process parameters
        if let Some(parameters) = &operation.parameters {
            for param in parameters {
                match param.location.as_str() {
                    "header" => {
                        if let Some(example) = &param.example {
                            headers.insert(param.name.clone(), example.to_string());
                        } else {
                            headers.insert(param.name.clone(), format!("${{{}}}", param.name));
                        }
                    }
                    "query" => {
                        let separator = if final_url.contains('?') { "&" } else { "?" };
                        let value = if let Some(example) = &param.example {
                            example.to_string()
                        } else {
                            format!("${{{}}}", param.name)
                        };
                        final_url = format!("{}{}{}={}", final_url, separator, param.name, value);
                    }
                    "path" => {
                        let placeholder = format!("{{{}}}", param.name);
                        let value = if let Some(example) = &param.example {
                            example.to_string()
                        } else {
                            format!("${{{}}}", param.name)
                        };
                        final_url = final_url.replace(&placeholder, &value);
                    }
                    _ => {} // cookie parameters not supported yet
                }
            }
        }

        // Process request body
        if let Some(request_body) = &operation.request_body {
            if let Some((content_type, media_type)) = request_body.content.iter().next() {
                headers.insert("Content-Type".to_string(), content_type.clone());
                
                if options.include_examples {
                    if let Some(example) = &media_type.example {
                        body = Some(serde_json::to_string_pretty(example)?);
                    } else if let Some(schema) = &media_type.schema {
                        body = Some(self.generate_example_from_schema(schema)?);
                    }
                }
            }
        }

        Ok(RequestDefinition {
            protocol: "http".to_string(),
            method: method.to_uppercase(),
            url: final_url,
            headers,
            body,
            auth: None, // Auth will be set at test case level
        })
    }

    fn generate_example_from_schema(&self, schema: &OpenApiSchema) -> Result<String, TestCaseError> {
        if let Some(example) = &schema.example {
            return Ok(serde_json::to_string_pretty(example)?);
        }

        let example_value = match schema.schema_type.as_deref() {
            Some("object") => {
                let mut obj = serde_json::Map::new();
                if let Some(properties) = &schema.properties {
                    for (key, prop_schema) in properties {
                        let prop_example = self.generate_value_from_schema(prop_schema);
                        obj.insert(key.clone(), prop_example);
                    }
                }
                serde_json::Value::Object(obj)
            }
            Some("array") => {
                if let Some(items) = &schema.items {
                    let item_example = self.generate_value_from_schema(items);
                    serde_json::Value::Array(vec![item_example])
                } else {
                    serde_json::Value::Array(vec![])
                }
            }
            _ => self.generate_value_from_schema(schema),
        };

        Ok(serde_json::to_string_pretty(&example_value)?)
    }

    fn generate_value_from_schema(&self, schema: &OpenApiSchema) -> serde_json::Value {
        if let Some(example) = &schema.example {
            return example.clone();
        }

        match schema.schema_type.as_deref() {
            Some("string") => match schema.format.as_deref() {
                Some("email") => serde_json::Value::String("user@example.com".to_string()),
                Some("date") => serde_json::Value::String("2023-01-01".to_string()),
                Some("date-time") => serde_json::Value::String("2023-01-01T00:00:00Z".to_string()),
                Some("uuid") => serde_json::Value::String("123e4567-e89b-12d3-a456-426614174000".to_string()),
                _ => serde_json::Value::String("string".to_string()),
            },
            Some("integer") => serde_json::Value::Number(serde_json::Number::from(42)),
            Some("number") => serde_json::Value::Number(serde_json::Number::from_f64(3.14).unwrap()),
            Some("boolean") => serde_json::Value::Bool(true),
            Some("array") => {
                if let Some(items) = &schema.items {
                    serde_json::Value::Array(vec![self.generate_value_from_schema(items)])
                } else {
                    serde_json::Value::Array(vec![])
                }
            }
            Some("object") => {
                let mut obj = serde_json::Map::new();
                if let Some(properties) = &schema.properties {
                    for (key, prop_schema) in properties {
                        obj.insert(key.clone(), self.generate_value_from_schema(prop_schema));
                    }
                }
                serde_json::Value::Object(obj)
            }
            _ => serde_json::Value::String("example".to_string()),
        }
    }

    fn generate_assertions_from_responses(&self, responses: &HashMap<String, OpenApiResponse>) -> Vec<AssertionDefinition> {
        let mut assertions = Vec::new();

        // Find the first successful response code
        for (status_code, _response) in responses {
            if let Ok(code) = status_code.parse::<u16>() {
                if (200..300).contains(&code) {
                    assertions.push(AssertionDefinition {
                        assertion_type: "status_code".to_string(),
                        expected: serde_json::Value::Number(serde_json::Number::from(code)),
                        message: Some(format!("Expected status code {}", code)),
                    });
                    break;
                }
            } else if status_code == "2XX" {
                assertions.push(AssertionDefinition {
                    assertion_type: "status_code".to_string(),
                    expected: serde_json::Value::Number(serde_json::Number::from(200)),
                    message: Some("Expected successful response".to_string()),
                });
                break;
            }
        }

        // Add default response time assertion
        assertions.push(AssertionDefinition {
            assertion_type: "response_time".to_string(),
            expected: serde_json::Value::Number(serde_json::Number::from(5000)),
            message: Some("Response should be within 5 seconds".to_string()),
        });

        assertions
    }

    fn convert_security_to_auth(
        &self,
        security: &Option<Vec<HashMap<String, Vec<String>>>>,
        spec: &OpenApiSpec,
    ) -> Option<AuthDefinition> {
        if let Some(security_requirements) = security {
            if let Some(requirement) = security_requirements.first() {
                if let Some((scheme_name, _scopes)) = requirement.iter().next() {
                    if let Some(components) = &spec.components {
                        if let Some(security_schemes) = &components.security_schemes {
                            if let Some(scheme) = security_schemes.get(scheme_name) {
                                return match scheme.scheme_type.as_str() {
                                    "http" => match scheme.scheme.as_deref() {
                                        Some("bearer") => Some(AuthDefinition {
                                            auth_type: "bearer".to_string(),
                                            config: {
                                                let mut config = HashMap::new();
                                                config.insert("token".to_string(), "${bearer_token}".to_string());
                                                config
                                            },
                                        }),
                                        Some("basic") => Some(AuthDefinition {
                                            auth_type: "basic".to_string(),
                                            config: {
                                                let mut config = HashMap::new();
                                                config.insert("username".to_string(), "${username}".to_string());
                                                config.insert("password".to_string(), "${password}".to_string());
                                                config
                                            },
                                        }),
                                        _ => None,
                                    },
                                    "apiKey" => {
                                        if let (Some(name), Some(location)) = (&scheme.name, &scheme.location) {
                                            Some(AuthDefinition {
                                                auth_type: "apikey".to_string(),
                                                config: {
                                                    let mut config = HashMap::new();
                                                    config.insert("key".to_string(), name.clone());
                                                    config.insert("value".to_string(), format!("${{{}}}", name));
                                                    config.insert("location".to_string(), location.clone());
                                                    config
                                                },
                                            })
                                        } else {
                                            None
                                        }
                                    }
                                    _ => None,
                                };
                            }
                        }
                    }
                }
            }
        }
        None
    }
}

#[async_trait]
impl TestCaseImporter for OpenApiImporter {
    fn supported_formats(&self) -> Vec<ImportFormat> {
        vec![ImportFormat::OpenApiSpec, ImportFormat::SwaggerSpec]
    }

    async fn import(&self, source: &ImportSource, options: &ImportOptions) -> Result<Vec<TestCase>, TestCaseError> {
        let content = match source {
            ImportSource::File(path) => {
                fs::read_to_string(path).await
                    .map_err(|e| TestCaseError::ImportError(format!("Failed to read file: {}", e)))?
            }
            ImportSource::Url(url) => {
                let response = reqwest::get(url).await
                    .map_err(|e| TestCaseError::ImportError(format!("Failed to fetch URL: {}", e)))?;
                response.text().await
                    .map_err(|e| TestCaseError::ImportError(format!("Failed to read response: {}", e)))?
            }
            ImportSource::Content(content) => content.clone(),
        };

        // Try to parse as JSON first, then YAML
        let spec: OpenApiSpec = if content.trim_start().starts_with('{') {
            serde_json::from_str(&content)
                .map_err(|e| TestCaseError::ImportError(format!("Failed to parse OpenAPI JSON: {}", e)))?
        } else {
            serde_yaml::from_str(&content)
                .map_err(|e| TestCaseError::ImportError(format!("Failed to parse OpenAPI YAML: {}", e)))?
        };

        let mut test_cases = Vec::new();

        for (path, path_item) in &spec.paths {
            let operations = [
                ("get", &path_item.get),
                ("post", &path_item.post),
                ("put", &path_item.put),
                ("delete", &path_item.delete),
                ("patch", &path_item.patch),
                ("head", &path_item.head),
                ("options", &path_item.options),
            ];

            for (method, operation) in operations {
                if let Some(op) = operation {
                    let test_case = self.convert_operation_to_test_case(path, method, op, &spec, options)?;
                    test_cases.push(test_case);
                }
            }
        }

        Ok(test_cases)
    }

    fn validate_source(&self, source: &ImportSource) -> Result<(), TestCaseError> {
        match source {
            ImportSource::File(path) => {
                if !path.exists() {
                    return Err(TestCaseError::ImportError("File does not exist".to_string()));
                }
                let ext = path.extension().and_then(|s| s.to_str());
                if !matches!(ext, Some("json") | Some("yaml") | Some("yml")) {
                    return Err(TestCaseError::ImportError("File must have .json, .yaml, or .yml extension".to_string()));
                }
            }
            ImportSource::Url(url) => {
                if !url.starts_with("http://") && !url.starts_with("https://") {
                    return Err(TestCaseError::ImportError("URL must use HTTP or HTTPS protocol".to_string()));
                }
            }
            ImportSource::Content(_) => {
                // Content validation will be done during parsing
            }
        }
        Ok(())
    }
}