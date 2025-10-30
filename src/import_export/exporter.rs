use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;


use crate::error::TestCaseError;
use crate::test_case_manager::TestCase;
use super::{TestCaseExporter, ExportFormat, ExportOptions};

// JSON Exporter
pub struct JsonExporter;

impl JsonExporter {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl TestCaseExporter for JsonExporter {
    fn supported_formats(&self) -> Vec<ExportFormat> {
        vec![ExportFormat::Json]
    }

    async fn export(&self, test_cases: &[TestCase], format: ExportFormat, options: &ExportOptions) -> Result<String, TestCaseError> {
        match format {
            ExportFormat::Json => {
                let export_data = if options.include_metadata {
                    ExportData::with_metadata(test_cases)
                } else {
                    ExportData::simple(test_cases)
                };

                if options.pretty_format {
                    serde_json::to_string_pretty(&export_data)
                        .map_err(|e| TestCaseError::ExportError(format!("JSON serialization failed: {}", e)))
                } else {
                    serde_json::to_string(&export_data)
                        .map_err(|e| TestCaseError::ExportError(format!("JSON serialization failed: {}", e)))
                }
            }
            _ => Err(TestCaseError::ExportError("Unsupported format for JSON exporter".to_string())),
        }
    }
}

// YAML Exporter
pub struct YamlExporter;

impl YamlExporter {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl TestCaseExporter for YamlExporter {
    fn supported_formats(&self) -> Vec<ExportFormat> {
        vec![ExportFormat::Yaml]
    }

    async fn export(&self, test_cases: &[TestCase], format: ExportFormat, options: &ExportOptions) -> Result<String, TestCaseError> {
        match format {
            ExportFormat::Yaml => {
                let export_data = if options.include_metadata {
                    ExportData::with_metadata(test_cases)
                } else {
                    ExportData::simple(test_cases)
                };

                serde_yaml::to_string(&export_data)
                    .map_err(|e| TestCaseError::ExportError(format!("YAML serialization failed: {}", e)))
            }
            _ => Err(TestCaseError::ExportError("Unsupported format for YAML exporter".to_string())),
        }
    }
}

// Postman Collection Exporter
pub struct PostmanExporter;

impl PostmanExporter {
    pub fn new() -> Self {
        Self
    }

    fn convert_test_cases_to_postman(&self, test_cases: &[TestCase], options: &ExportOptions) -> PostmanCollection {
        let mut collection = PostmanCollection {
            info: PostmanInfo {
                name: "Exported Test Cases".to_string(),
                description: Some("Test cases exported from API Test Runner".to_string()),
                schema: Some("https://schema.getpostman.com/json/collection/v2.1.0/collection.json".to_string()),
            },
            item: Vec::new(),
            variable: Some(Vec::new()),
            auth: None,
        };

        // Group test cases by folder
        let mut folders: HashMap<String, Vec<&TestCase>> = HashMap::new();
        let mut root_items = Vec::new();

        for test_case in test_cases {
            // Extract folder path from tags
            let folder_path = test_case.tags.iter()
                .find(|tag| tag.starts_with("folder:"))
                .map(|tag| tag.strip_prefix("folder:").unwrap_or(tag).to_string());
                
            if let Some(folder_path) = folder_path {
                folders.entry(folder_path).or_insert_with(Vec::new).push(test_case);
            } else {
                root_items.push(test_case);
            }
        }

        // Add root items
        for test_case in root_items {
            collection.item.push(self.convert_test_case_to_postman_item(test_case, options));
        }

        // Add folders
        for (folder_name, folder_test_cases) in folders {
            let folder_item = PostmanItem {
                name: folder_name,
                description: None,
                request: None,
                item: Some(
                    folder_test_cases
                        .into_iter()
                        .map(|tc| self.convert_test_case_to_postman_item(tc, options))
                        .collect()
                ),
                event: None,
            };
            collection.item.push(folder_item);
        }

        collection
    }

    fn convert_test_case_to_postman_item(&self, test_case: &TestCase, _options: &ExportOptions) -> PostmanItem {
        let url = PostmanUrl::String(test_case.request.url.clone());
        
        let headers = test_case.request.headers
            .iter()
            .map(|(key, value)| PostmanHeader {
                key: key.clone(),
                value: value.clone(),
                disabled: Some(false),
                description: None,
            })
            .collect();

        let body = test_case.request.body.as_ref().map(|body_content| {
            PostmanBody {
                mode: Some("raw".to_string()),
                raw: Some(body_content.clone()),
                urlencoded: None,
                formdata: None,
            }
        });

        let auth = test_case.request.auth.as_ref().map(|auth_def| {
            match auth_def.auth_type.as_str() {
                "bearer" => PostmanAuth {
                    auth_type: "bearer".to_string(),
                    bearer: Some(vec![PostmanKeyValue {
                        key: "token".to_string(),
                        value: auth_def.config.get("token").cloned().unwrap_or_default(),
                        disabled: Some(false),
                    }]),
                    basic: None,
                    oauth2: None,
                    apikey: None,
                },
                "basic" => PostmanAuth {
                    auth_type: "basic".to_string(),
                    bearer: None,
                    basic: Some(vec![
                        PostmanKeyValue {
                            key: "username".to_string(),
                            value: auth_def.config.get("username").cloned().unwrap_or_default(),
                            disabled: Some(false),
                        },
                        PostmanKeyValue {
                            key: "password".to_string(),
                            value: auth_def.config.get("password").cloned().unwrap_or_default(),
                            disabled: Some(false),
                        },
                    ]),
                    oauth2: None,
                    apikey: None,
                },
                "apikey" => PostmanAuth {
                    auth_type: "apikey".to_string(),
                    bearer: None,
                    basic: None,
                    oauth2: None,
                    apikey: Some(vec![
                        PostmanKeyValue {
                            key: "key".to_string(),
                            value: auth_def.config.get("key").cloned().unwrap_or_default(),
                            disabled: Some(false),
                        },
                        PostmanKeyValue {
                            key: "value".to_string(),
                            value: auth_def.config.get("value").cloned().unwrap_or_default(),
                            disabled: Some(false),
                        },
                    ]),
                },
                _ => PostmanAuth {
                    auth_type: "noauth".to_string(),
                    bearer: None,
                    basic: None,
                    oauth2: None,
                    apikey: None,
                },
            }
        });

        let request = PostmanRequest {
            method: test_case.request.method.clone(),
            header: Some(headers),
            body,
            url,
            auth,
            description: test_case.description.clone(),
        };

        PostmanItem {
            name: test_case.name.clone(),
            description: test_case.description.clone(),
            request: Some(request),
            item: None,
            event: None,
        }
    }
}

#[async_trait]
impl TestCaseExporter for PostmanExporter {
    fn supported_formats(&self) -> Vec<ExportFormat> {
        vec![ExportFormat::PostmanCollection]
    }

    async fn export(&self, test_cases: &[TestCase], format: ExportFormat, options: &ExportOptions) -> Result<String, TestCaseError> {
        match format {
            ExportFormat::PostmanCollection => {
                let collection = self.convert_test_cases_to_postman(test_cases, options);
                
                if options.pretty_format {
                    serde_json::to_string_pretty(&collection)
                        .map_err(|e| TestCaseError::ExportError(format!("Postman collection serialization failed: {}", e)))
                } else {
                    serde_json::to_string(&collection)
                        .map_err(|e| TestCaseError::ExportError(format!("Postman collection serialization failed: {}", e)))
                }
            }
            _ => Err(TestCaseError::ExportError("Unsupported format for Postman exporter".to_string())),
        }
    }
}

// Export data structures
#[derive(Debug, Serialize, Deserialize)]
struct ExportData {
    version: String,
    exported_at: chrono::DateTime<chrono::Utc>,
    test_cases: Vec<TestCase>,
    #[serde(skip_serializing_if = "Option::is_none")]
    metadata: Option<ExportMetadata>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ExportMetadata {
    total_count: usize,
    folders: Vec<String>,
    tags: Vec<String>,
    export_options: ExportOptions,
}

impl ExportData {
    fn simple(test_cases: &[TestCase]) -> Self {
        Self {
            version: "1.0".to_string(),
            exported_at: chrono::Utc::now(),
            test_cases: test_cases.to_vec(),
            metadata: None,
        }
    }

    fn with_metadata(test_cases: &[TestCase]) -> Self {
        let folders: std::collections::HashSet<String> = test_cases
            .iter()
            .filter_map(|tc| {
                tc.tags.iter()
                    .find(|tag| tag.starts_with("folder:"))
                    .map(|tag| tag.strip_prefix("folder:").unwrap_or(tag).to_string())
            })
            .collect();

        let tags: std::collections::HashSet<String> = test_cases
            .iter()
            .flat_map(|tc| tc.tags.clone())
            .collect();

        let metadata = ExportMetadata {
            total_count: test_cases.len(),
            folders: folders.into_iter().collect(),
            tags: tags.into_iter().collect(),
            export_options: ExportOptions::default(),
        };

        Self {
            version: "1.0".to_string(),
            exported_at: chrono::Utc::now(),
            test_cases: test_cases.to_vec(),
            metadata: Some(metadata),
        }
    }
}

// Postman collection structures for export
#[derive(Debug, Serialize)]
struct PostmanCollection {
    info: PostmanInfo,
    item: Vec<PostmanItem>,
    variable: Option<Vec<PostmanVariable>>,
    auth: Option<PostmanAuth>,
}

#[derive(Debug, Serialize)]
struct PostmanInfo {
    name: String,
    description: Option<String>,
    schema: Option<String>,
}

#[derive(Debug, Serialize)]
struct PostmanItem {
    name: String,
    description: Option<String>,
    request: Option<PostmanRequest>,
    item: Option<Vec<PostmanItem>>,
    event: Option<Vec<PostmanEvent>>,
}

#[derive(Debug, Serialize)]
struct PostmanRequest {
    method: String,
    header: Option<Vec<PostmanHeader>>,
    body: Option<PostmanBody>,
    url: PostmanUrl,
    auth: Option<PostmanAuth>,
    description: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
enum PostmanUrl {
    String(String),
}

#[derive(Debug, Serialize)]
struct PostmanHeader {
    key: String,
    value: String,
    disabled: Option<bool>,
    description: Option<String>,
}

#[derive(Debug, Serialize)]
struct PostmanBody {
    mode: Option<String>,
    raw: Option<String>,
    urlencoded: Option<Vec<PostmanKeyValue>>,
    formdata: Option<Vec<PostmanFormData>>,
}

#[derive(Debug, Serialize)]
struct PostmanKeyValue {
    key: String,
    value: String,
    disabled: Option<bool>,
}

#[derive(Debug, Serialize)]
struct PostmanFormData {
    key: String,
    value: Option<String>,
    src: Option<String>,
    #[serde(rename = "type")]
    data_type: Option<String>,
    disabled: Option<bool>,
}

#[derive(Debug, Serialize)]
struct PostmanAuth {
    #[serde(rename = "type")]
    auth_type: String,
    bearer: Option<Vec<PostmanKeyValue>>,
    basic: Option<Vec<PostmanKeyValue>>,
    oauth2: Option<Vec<PostmanKeyValue>>,
    apikey: Option<Vec<PostmanKeyValue>>,
}

#[derive(Debug, Serialize)]
struct PostmanVariable {
    key: String,
    value: String,
    description: Option<String>,
}

#[derive(Debug, Serialize)]
struct PostmanEvent {
    listen: String,
    script: PostmanScript,
}

#[derive(Debug, Serialize)]
struct PostmanScript {
    #[serde(rename = "type")]
    script_type: Option<String>,
    exec: Option<Vec<String>>,
}