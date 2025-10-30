use async_trait::async_trait;
use serde::Deserialize;
use std::collections::HashMap;

use std::time::Duration;
use tokio::fs;
use uuid::Uuid;
use chrono::Utc;

use crate::error::TestCaseError;
use crate::test_case_manager::{TestCase, RequestDefinition, AssertionDefinition, VariableExtraction, AuthDefinition};
use super::{TestCaseImporter, ImportFormat, ImportSource, ImportOptions};

#[derive(Debug, Clone, Deserialize)]
pub struct PostmanCollection {
    pub info: PostmanInfo,
    pub item: Vec<PostmanItem>,
    pub variable: Option<Vec<PostmanVariable>>,
    pub auth: Option<PostmanAuth>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PostmanInfo {
    pub name: String,
    pub description: Option<String>,
    pub schema: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PostmanItem {
    pub name: String,
    pub description: Option<String>,
    pub request: Option<PostmanRequest>,
    pub item: Option<Vec<PostmanItem>>, // For folders
    pub event: Option<Vec<PostmanEvent>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PostmanRequest {
    pub method: String,
    pub header: Option<Vec<PostmanHeader>>,
    pub body: Option<PostmanBody>,
    pub url: PostmanUrl,
    pub auth: Option<PostmanAuth>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum PostmanUrl {
    String(String),
    Object {
        raw: Option<String>,
        protocol: Option<String>,
        host: Option<Vec<String>>,
        port: Option<String>,
        path: Option<Vec<String>>,
        query: Option<Vec<PostmanQueryParam>>,
    },
}

#[derive(Debug, Clone, Deserialize)]
pub struct PostmanHeader {
    pub key: String,
    pub value: String,
    pub disabled: Option<bool>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PostmanBody {
    pub mode: Option<String>,
    pub raw: Option<String>,
    pub urlencoded: Option<Vec<PostmanKeyValue>>,
    pub formdata: Option<Vec<PostmanFormData>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PostmanKeyValue {
    pub key: String,
    pub value: String,
    pub disabled: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PostmanFormData {
    pub key: String,
    pub value: Option<String>,
    pub src: Option<String>,
    #[serde(rename = "type")]
    pub data_type: Option<String>,
    pub disabled: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PostmanQueryParam {
    pub key: String,
    pub value: String,
    pub disabled: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PostmanAuth {
    #[serde(rename = "type")]
    pub auth_type: String,
    pub bearer: Option<Vec<PostmanKeyValue>>,
    pub basic: Option<Vec<PostmanKeyValue>>,
    pub oauth2: Option<Vec<PostmanKeyValue>>,
    pub apikey: Option<Vec<PostmanKeyValue>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PostmanVariable {
    pub key: String,
    pub value: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PostmanEvent {
    pub listen: String,
    pub script: PostmanScript,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PostmanScript {
    #[serde(rename = "type")]
    pub script_type: Option<String>,
    pub exec: Option<Vec<String>>,
}

pub struct PostmanImporter {
    // Configuration for the importer
}

impl PostmanImporter {
    pub fn new() -> Self {
        Self {}
    }

    fn convert_postman_item_to_test_case(
        &self,
        item: &PostmanItem,
        collection_variables: &HashMap<String, String>,
        options: &ImportOptions,
        folder_path: &str,
    ) -> Result<Option<TestCase>, TestCaseError> {
        if let Some(request) = &item.request {
            let mut test_case = TestCase {
                id: Uuid::new_v4().to_string(),
                name: item.name.clone(),
                description: item.description.clone(),
                tags: self.extract_tags(&item.name, options),
                protocol: "http".to_string(),
                request: self.convert_postman_request(request, collection_variables, options)?,
                assertions: if options.generate_assertions {
                    self.generate_default_assertions(request)
                } else {
                    Vec::new()
                },
                variable_extractions: Some(self.extract_variables_from_tests(&item.event)),
                dependencies: Vec::new(),
                timeout: Some(Duration::from_millis(30000)),
                created_at: Utc::now(),
                updated_at: Utc::now(),
                version: 1,
                change_log: vec![],
            };

            // Add folder path as tag if present
            if !folder_path.is_empty() {
                test_case.tags.push(format!("folder:{}", folder_path));
            }

            Ok(Some(test_case))
        } else {
            // This is a folder, return None
            Ok(None)
        }
    }

    fn convert_postman_request(
        &self,
        request: &PostmanRequest,
        variables: &HashMap<String, String>,
        options: &ImportOptions,
    ) -> Result<RequestDefinition, TestCaseError> {
        let url = self.convert_postman_url(&request.url, variables, options)?;
        let headers = self.convert_postman_headers(&request.header);
        let body = self.convert_postman_body(&request.body)?;

        Ok(RequestDefinition {
            protocol: "http".to_string(),
            method: request.method.to_uppercase(),
            url,
            headers,
            body,
            auth: self.convert_postman_auth(&request.auth),
        })
    }

    fn convert_postman_url(
        &self,
        url: &PostmanUrl,
        variables: &HashMap<String, String>,
        options: &ImportOptions,
    ) -> Result<String, TestCaseError> {
        let url_string = match url {
            PostmanUrl::String(s) => s.clone(),
            PostmanUrl::Object { raw, protocol, host, port, path, query } => {
                if let Some(raw_url) = raw {
                    raw_url.clone()
                } else {
                    // Construct URL from parts
                    let protocol = protocol.as_deref().unwrap_or("https");
                    let host_str = host.as_ref()
                        .map(|h| h.join("."))
                        .unwrap_or_else(|| "localhost".to_string());
                    let port_str = port.as_ref()
                        .map(|p| format!(":{}", p))
                        .unwrap_or_default();
                    let path_str = path.as_ref()
                        .map(|p| format!("/{}", p.join("/")))
                        .unwrap_or_default();
                    let query_str = query.as_ref()
                        .map(|q| {
                            let params: Vec<String> = q.iter()
                                .filter(|param| !param.disabled.unwrap_or(false))
                                .map(|param| format!("{}={}", param.key, param.value))
                                .collect();
                            if params.is_empty() {
                                String::new()
                            } else {
                                format!("?{}", params.join("&"))
                            }
                        })
                        .unwrap_or_default();

                    format!("{}://{}{}{}{}", protocol, host_str, port_str, path_str, query_str)
                }
            }
        };

        // Apply base URL override if provided
        let final_url = if let Some(base_url) = &options.base_url_override {
            // Simple base URL replacement - just replace the protocol and host part
            if url_string.starts_with("http://") || url_string.starts_with("https://") {
                // Find the third slash (after protocol://)
                let mut slash_count = 0;
                let mut path_start = None;
                for (i, c) in url_string.char_indices() {
                    if c == '/' {
                        slash_count += 1;
                        if slash_count == 3 {
                            path_start = Some(i);
                            break;
                        }
                    }
                }
                
                if let Some(start) = path_start {
                    format!("{}{}", base_url.trim_end_matches('/'), &url_string[start..])
                } else {
                    base_url.clone()
                }
            } else {
                format!("{}/{}", base_url.trim_end_matches('/'), url_string.trim_start_matches('/'))
            }
        } else {
            url_string
        };

        // Replace Postman variables with our variable syntax
        let mut result = final_url;
        for (key, value) in variables {
            result = result.replace(&format!("{{{{{}}}}}", key), &format!("${{{}}}", key));
        }

        Ok(result)
    }

    fn convert_postman_headers(&self, headers: &Option<Vec<PostmanHeader>>) -> HashMap<String, String> {
        let mut result = HashMap::new();
        if let Some(headers) = headers {
            for header in headers {
                if !header.disabled.unwrap_or(false) {
                    result.insert(header.key.clone(), header.value.clone());
                }
            }
        }
        result
    }

    fn convert_postman_body(&self, body: &Option<PostmanBody>) -> Result<Option<String>, TestCaseError> {
        if let Some(body) = body {
            match body.mode.as_deref() {
                Some("raw") => Ok(body.raw.clone()),
                Some("urlencoded") => {
                    if let Some(data) = &body.urlencoded {
                        let params: Vec<String> = data.iter()
                            .filter(|param| !param.disabled.unwrap_or(false))
                            .map(|param| format!("{}={}", param.key, param.value))
                            .collect();
                        Ok(Some(params.join("&")))
                    } else {
                        Ok(None)
                    }
                }
                Some("formdata") => {
                    // For form data, we'll convert to JSON representation
                    if let Some(data) = &body.formdata {
                        let mut form_obj = serde_json::Map::new();
                        for field in data {
                            if !field.disabled.unwrap_or(false) {
                                if let Some(value) = &field.value {
                                    form_obj.insert(field.key.clone(), serde_json::Value::String(value.clone()));
                                }
                            }
                        }
                        Ok(Some(serde_json::to_string_pretty(&form_obj)?))
                    } else {
                        Ok(None)
                    }
                }
                _ => Ok(body.raw.clone()),
            }
        } else {
            Ok(None)
        }
    }

    fn convert_postman_auth(&self, auth: &Option<PostmanAuth>) -> Option<AuthDefinition> {
        if let Some(auth) = auth {
            match auth.auth_type.as_str() {
                "bearer" => {
                    if let Some(bearer_data) = &auth.bearer {
                        for item in bearer_data {
                            if item.key == "token" {
                                return Some(AuthDefinition {
                                    auth_type: "bearer".to_string(),
                                    config: {
                                        let mut config = HashMap::new();
                                        config.insert("token".to_string(), item.value.clone());
                                        config
                                    },
                                });
                            }
                        }
                    }
                }
                "basic" => {
                    if let Some(basic_data) = &auth.basic {
                        let mut username = None;
                        let mut password = None;
                        for item in basic_data {
                            match item.key.as_str() {
                                "username" => username = Some(item.value.clone()),
                                "password" => password = Some(item.value.clone()),
                                _ => {}
                            }
                        }
                        if let (Some(u), Some(p)) = (username, password) {
                            return Some(AuthDefinition {
                                auth_type: "basic".to_string(),
                                config: {
                                    let mut config = HashMap::new();
                                    config.insert("username".to_string(), u);
                                    config.insert("password".to_string(), p);
                                    config
                                },
                            });
                        }
                    }
                }
                "apikey" => {
                    if let Some(apikey_data) = &auth.apikey {
                        let mut key = None;
                        let mut value = None;
                        let mut location = "header".to_string();
                        for item in apikey_data {
                            match item.key.as_str() {
                                "key" => key = Some(item.value.clone()),
                                "value" => value = Some(item.value.clone()),
                                "in" => location = item.value.clone(),
                                _ => {}
                            }
                        }
                        if let (Some(k), Some(v)) = (key, value) {
                            return Some(AuthDefinition {
                                auth_type: "apikey".to_string(),
                                config: {
                                    let mut config = HashMap::new();
                                    config.insert("key".to_string(), k);
                                    config.insert("value".to_string(), v);
                                    config.insert("location".to_string(), location);
                                    config
                                },
                            });
                        }
                    }
                }
                _ => {}
            }
        }
        None
    }

    fn extract_tags(&self, name: &str, options: &ImportOptions) -> Vec<String> {
        let mut tags = Vec::new();
        
        // Add prefix tag if specified
        if let Some(prefix) = &options.tag_prefix {
            tags.push(format!("{}:postman", prefix));
        } else {
            tags.push("postman".to_string());
        }

        // Extract tags from name (look for patterns like [tag] or #tag)
        let tag_patterns = [
            regex::Regex::new(r"\[([^\]]+)\]").unwrap(),
            regex::Regex::new(r"#(\w+)").unwrap(),
        ];

        for pattern in &tag_patterns {
            for cap in pattern.captures_iter(name) {
                if let Some(tag) = cap.get(1) {
                    tags.push(tag.as_str().to_lowercase());
                }
            }
        }

        tags
    }

    fn generate_default_assertions(&self, request: &PostmanRequest) -> Vec<AssertionDefinition> {
        vec![
            AssertionDefinition {
                assertion_type: "status_code".to_string(),
                expected: serde_json::Value::Number(serde_json::Number::from(200)),
                message: Some("Expected successful response".to_string()),
            },
            AssertionDefinition {
                assertion_type: "response_time".to_string(),
                expected: serde_json::Value::Number(serde_json::Number::from(5000)),
                message: Some("Response should be within 5 seconds".to_string()),
            },
        ]
    }

    fn extract_variables_from_tests(&self, events: &Option<Vec<PostmanEvent>>) -> Vec<VariableExtraction> {
        let mut extractions = Vec::new();
        
        if let Some(events) = events {
            for event in events {
                if event.listen == "test" {
                    if let Some(exec) = &event.script.exec {
                        for line in exec {
                            // Look for pm.environment.set() or pm.globals.set() calls
                            if let Some(captures) = regex::Regex::new(r#"pm\.(environment|globals)\.set\("([^"]+)",\s*([^)]+)\)"#)
                                .unwrap()
                                .captures(line) {
                                if let (Some(var_name), Some(source)) = (captures.get(2), captures.get(3)) {
                                    // Try to convert Postman extraction to JSONPath
                                    let jsonpath = if source.as_str().contains("responseJson") {
                                        // Convert pm.response.json().field to $.field
                                        source.as_str()
                                            .replace("pm.response.json().", "$.")
                                            .replace("pm.response.json()", "$")
                                    } else {
                                        format!("$.{}", source.as_str())
                                    };

                                    extractions.push(VariableExtraction {
                                        name: var_name.as_str().to_string(),
                                        source: crate::test_case_manager::ExtractionSource::ResponseBody,
                                        path: jsonpath,
                                        default_value: None,
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }

        extractions
    }

    fn collect_variables(&self, collection: &PostmanCollection) -> HashMap<String, String> {
        let mut variables = HashMap::new();
        
        if let Some(vars) = &collection.variable {
            for var in vars {
                variables.insert(var.key.clone(), var.value.clone());
            }
        }

        variables
    }

    fn process_items_recursively(
        &self,
        items: &[PostmanItem],
        collection_variables: &HashMap<String, String>,
        options: &ImportOptions,
        folder_path: &str,
    ) -> Result<Vec<TestCase>, TestCaseError> {
        let mut test_cases = Vec::new();

        for item in items {
            if let Some(sub_items) = &item.item {
                // This is a folder, process recursively
                let new_folder_path = if folder_path.is_empty() {
                    item.name.clone()
                } else {
                    format!("{}/{}", folder_path, item.name)
                };
                let mut sub_cases = self.process_items_recursively(
                    sub_items,
                    collection_variables,
                    options,
                    &new_folder_path,
                )?;
                test_cases.append(&mut sub_cases);
            } else if let Some(test_case) = self.convert_postman_item_to_test_case(
                item,
                collection_variables,
                options,
                folder_path,
            )? {
                test_cases.push(test_case);
            }
        }

        Ok(test_cases)
    }
}

#[async_trait]
impl TestCaseImporter for PostmanImporter {
    fn supported_formats(&self) -> Vec<ImportFormat> {
        vec![ImportFormat::PostmanCollection]
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

        let collection: PostmanCollection = serde_json::from_str(&content)
            .map_err(|e| TestCaseError::ImportError(format!("Failed to parse Postman collection: {}", e)))?;

        let collection_variables = self.collect_variables(&collection);
        self.process_items_recursively(&collection.item, &collection_variables, options, "")
    }

    fn validate_source(&self, source: &ImportSource) -> Result<(), TestCaseError> {
        match source {
            ImportSource::File(path) => {
                if !path.exists() {
                    return Err(TestCaseError::ImportError("File does not exist".to_string()));
                }
                if path.extension().and_then(|s| s.to_str()) != Some("json") {
                    return Err(TestCaseError::ImportError("File must have .json extension".to_string()));
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