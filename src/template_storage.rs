use async_trait::async_trait;
use std::path::{Path, PathBuf};
use tokio::fs;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::error::TestCaseError;
use crate::template_manager::{TestCaseTemplate, TestCaseFragment, TemplateFilter, FragmentType, TemplateStorage};

pub struct FileTemplateStorage {
    templates_dir: PathBuf,
    fragments_dir: PathBuf,
}

impl FileTemplateStorage {
    pub fn new<P: AsRef<Path>>(base_dir: P) -> Self {
        let base_path = base_dir.as_ref().to_path_buf();
        Self {
            templates_dir: base_path.join("templates"),
            fragments_dir: base_path.join("fragments"),
        }
    }

    pub async fn initialize(&self) -> Result<(), TestCaseError> {
        // Create directories if they don't exist
        fs::create_dir_all(&self.templates_dir).await
            .map_err(|e| TestCaseError::StorageError(format!("Failed to create templates directory: {}", e)))?;
        
        fs::create_dir_all(&self.fragments_dir).await
            .map_err(|e| TestCaseError::StorageError(format!("Failed to create fragments directory: {}", e)))?;
        
        Ok(())
    }

    fn template_file_path(&self, id: &str) -> PathBuf {
        self.templates_dir.join(format!("{}.yaml", id))
    }

    fn fragment_file_path(&self, id: &str) -> PathBuf {
        self.fragments_dir.join(format!("{}.yaml", id))
    }

    async fn write_yaml_file<T: serde::Serialize>(&self, path: &Path, data: &T) -> Result<(), TestCaseError> {
        let yaml_content = serde_yaml::to_string(data)
            .map_err(|e| TestCaseError::SerializationError(format!("Failed to serialize to YAML: {}", e)))?;
        
        let mut file = fs::File::create(path).await
            .map_err(|e| TestCaseError::StorageError(format!("Failed to create file {}: {}", path.display(), e)))?;
        
        file.write_all(yaml_content.as_bytes()).await
            .map_err(|e| TestCaseError::StorageError(format!("Failed to write file {}: {}", path.display(), e)))?;
        
        Ok(())
    }

    async fn read_yaml_file<T: serde::de::DeserializeOwned>(&self, path: &Path) -> Result<T, TestCaseError> {
        let mut file = fs::File::open(path).await
            .map_err(|e| TestCaseError::StorageError(format!("Failed to open file {}: {}", path.display(), e)))?;
        
        let mut contents = String::new();
        file.read_to_string(&mut contents).await
            .map_err(|e| TestCaseError::StorageError(format!("Failed to read file {}: {}", path.display(), e)))?;
        
        serde_yaml::from_str(&contents)
            .map_err(|e| TestCaseError::SerializationError(format!("Failed to deserialize YAML from {}: {}", path.display(), e)))
    }

    async fn list_yaml_files(&self, dir: &Path) -> Result<Vec<String>, TestCaseError> {
        let mut entries = fs::read_dir(dir).await
            .map_err(|e| TestCaseError::StorageError(format!("Failed to read directory {}: {}", dir.display(), e)))?;
        
        let mut file_names = Vec::new();
        while let Some(entry) = entries.next_entry().await
            .map_err(|e| TestCaseError::StorageError(format!("Failed to read directory entry: {}", e)))? {
            
            let path = entry.path();
            if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("yaml") {
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    file_names.push(stem.to_string());
                }
            }
        }
        
        Ok(file_names)
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
}

#[async_trait]
impl TemplateStorage for FileTemplateStorage {
    async fn save_template(&self, template: &TestCaseTemplate) -> Result<(), TestCaseError> {
        let file_path = self.template_file_path(&template.id);
        self.write_yaml_file(&file_path, template).await
    }

    async fn load_template(&self, id: &str) -> Result<TestCaseTemplate, TestCaseError> {
        let file_path = self.template_file_path(id);
        if !file_path.exists() {
            return Err(TestCaseError::NotFound(format!("Template with id '{}' not found", id)));
        }
        self.read_yaml_file(&file_path).await
    }

    async fn delete_template(&self, id: &str) -> Result<(), TestCaseError> {
        let file_path = self.template_file_path(id);
        if !file_path.exists() {
            return Err(TestCaseError::NotFound(format!("Template with id '{}' not found", id)));
        }
        
        fs::remove_file(&file_path).await
            .map_err(|e| TestCaseError::StorageError(format!("Failed to delete template file {}: {}", file_path.display(), e)))
    }

    async fn list_templates(&self, filter: &TemplateFilter) -> Result<Vec<TestCaseTemplate>, TestCaseError> {
        let template_ids = self.list_yaml_files(&self.templates_dir).await?;
        let mut templates = Vec::new();
        
        for id in template_ids {
            match self.load_template(&id).await {
                Ok(template) => {
                    if self.template_matches_filter(&template, filter) {
                        templates.push(template);
                    }
                }
                Err(e) => {
                    // Log error but continue with other templates
                    eprintln!("Warning: Failed to load template {}: {}", id, e);
                }
            }
        }
        
        Ok(templates)
    }

    async fn template_exists(&self, id: &str) -> Result<bool, TestCaseError> {
        let file_path = self.template_file_path(id);
        Ok(file_path.exists())
    }

    async fn save_fragment(&self, fragment: &TestCaseFragment) -> Result<(), TestCaseError> {
        let file_path = self.fragment_file_path(&fragment.id);
        self.write_yaml_file(&file_path, fragment).await
    }

    async fn load_fragment(&self, id: &str) -> Result<TestCaseFragment, TestCaseError> {
        let file_path = self.fragment_file_path(id);
        if !file_path.exists() {
            return Err(TestCaseError::NotFound(format!("Fragment with id '{}' not found", id)));
        }
        self.read_yaml_file(&file_path).await
    }

    async fn delete_fragment(&self, id: &str) -> Result<(), TestCaseError> {
        let file_path = self.fragment_file_path(id);
        if !file_path.exists() {
            return Err(TestCaseError::NotFound(format!("Fragment with id '{}' not found", id)));
        }
        
        fs::remove_file(&file_path).await
            .map_err(|e| TestCaseError::StorageError(format!("Failed to delete fragment file {}: {}", file_path.display(), e)))
    }

    async fn list_fragments(&self, fragment_type: Option<FragmentType>) -> Result<Vec<TestCaseFragment>, TestCaseError> {
        let fragment_ids = self.list_yaml_files(&self.fragments_dir).await?;
        let mut fragments = Vec::new();
        
        for id in fragment_ids {
            match self.load_fragment(&id).await {
                Ok(fragment) => {
                    if fragment_type.is_none() || fragment_type.as_ref() == Some(&fragment.fragment_type) {
                        fragments.push(fragment);
                    }
                }
                Err(e) => {
                    // Log error but continue with other fragments
                    eprintln!("Warning: Failed to load fragment {}: {}", id, e);
                }
            }
        }
        
        Ok(fragments)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use crate::template_manager::{TemplateCategory, VariableType, TemplateVariable, TestCaseDefinition};
    use crate::test_case_manager::{RequestDefinition, AssertionDefinition};
    use chrono::Utc;
    use std::collections::HashMap;

    async fn create_test_storage() -> (FileTemplateStorage, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let storage = FileTemplateStorage::new(temp_dir.path());
        storage.initialize().await.unwrap();
        (storage, temp_dir)
    }

    fn create_test_template() -> TestCaseTemplate {
        TestCaseTemplate {
            id: "test-template".to_string(),
            name: "Test Template".to_string(),
            description: "A test template".to_string(),
            category: TemplateCategory::RestApi,
            template: TestCaseDefinition {
                request: RequestDefinition {
                    protocol: "http".to_string(),
                    method: "GET".to_string(),
                    url: "{{base_url}}/test".to_string(),
                    headers: HashMap::new(),
                    body: None,
                    auth: None,
                },
                assertions: vec![
                    AssertionDefinition {
                        assertion_type: "status_code".to_string(),
                        expected: serde_json::json!(200),
                        message: Some("Should be successful".to_string()),
                    }
                ],
                variable_extractions: None,
                dependencies: vec![],
                timeout: Some(30),
            },
            variables: vec![
                TemplateVariable {
                    name: "base_url".to_string(),
                    description: "Base URL".to_string(),
                    variable_type: VariableType::Url,
                    default_value: Some("https://api.example.com".to_string()),
                    required: true,
                    validation_pattern: None,
                    example_values: vec![],
                }
            ],
            instructions: Some("Test instructions".to_string()),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            version: "1.0.0".to_string(),
            author: Some("Test Author".to_string()),
            tags: vec!["test".to_string()],
        }
    }

    #[tokio::test]
    async fn test_save_and_load_template() {
        let (storage, _temp_dir) = create_test_storage().await;
        let template = create_test_template();

        // Save template
        storage.save_template(&template).await.unwrap();

        // Load template
        let loaded_template = storage.load_template(&template.id).await.unwrap();
        assert_eq!(loaded_template.id, template.id);
        assert_eq!(loaded_template.name, template.name);
        assert_eq!(loaded_template.description, template.description);
    }

    #[tokio::test]
    async fn test_template_exists() {
        let (storage, _temp_dir) = create_test_storage().await;
        let template = create_test_template();

        // Template should not exist initially
        assert!(!storage.template_exists(&template.id).await.unwrap());

        // Save template
        storage.save_template(&template).await.unwrap();

        // Template should now exist
        assert!(storage.template_exists(&template.id).await.unwrap());
    }

    #[tokio::test]
    async fn test_delete_template() {
        let (storage, _temp_dir) = create_test_storage().await;
        let template = create_test_template();

        // Save template
        storage.save_template(&template).await.unwrap();
        assert!(storage.template_exists(&template.id).await.unwrap());

        // Delete template
        storage.delete_template(&template.id).await.unwrap();
        assert!(!storage.template_exists(&template.id).await.unwrap());
    }

    #[tokio::test]
    async fn test_list_templates() {
        let (storage, _temp_dir) = create_test_storage().await;
        let template1 = create_test_template();
        let mut template2 = create_test_template();
        template2.id = "test-template-2".to_string();
        template2.name = "Test Template 2".to_string();

        // Save templates
        storage.save_template(&template1).await.unwrap();
        storage.save_template(&template2).await.unwrap();

        // List all templates
        let filter = TemplateFilter::default();
        let templates = storage.list_templates(&filter).await.unwrap();
        assert_eq!(templates.len(), 2);

        // Filter by name
        let filter = TemplateFilter {
            name_pattern: Some("Template 2".to_string()),
            ..Default::default()
        };
        let filtered_templates = storage.list_templates(&filter).await.unwrap();
        assert_eq!(filtered_templates.len(), 1);
        assert_eq!(filtered_templates[0].id, template2.id);
    }
}