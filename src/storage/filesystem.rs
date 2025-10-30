use async_trait::async_trait;


use std::path::PathBuf;
use tokio::fs;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::error::TestCaseError;
use crate::test_case_manager::{StorageBackend, TestCase, TestCaseFilter, TestCaseFormat, TestCaseMetadata};

pub struct FileSystemStorage {
    base_path: PathBuf,
    format: TestCaseFormat,
}

impl FileSystemStorage {
    pub async fn new(base_path: PathBuf, format: TestCaseFormat) -> Result<Self, TestCaseError> {
        // Create base directory if it doesn't exist
        if !base_path.exists() {
            fs::create_dir_all(&base_path).await
                .map_err(|e| TestCaseError::StorageError(format!("Failed to create storage directory: {}", e)))?;
        }

        // Create subdirectories for organization
        let test_cases_dir = base_path.join("test_cases");
        let metadata_dir = base_path.join("metadata");
        
        fs::create_dir_all(&test_cases_dir).await
            .map_err(|e| TestCaseError::StorageError(format!("Failed to create test_cases directory: {}", e)))?;
        fs::create_dir_all(&metadata_dir).await
            .map_err(|e| TestCaseError::StorageError(format!("Failed to create metadata directory: {}", e)))?;

        Ok(Self { base_path, format })
    }

    fn get_test_case_path(&self, id: &str) -> PathBuf {
        let extension = match self.format {
            TestCaseFormat::Yaml => "yaml",
            TestCaseFormat::Json => "json",
        };
        self.base_path.join("test_cases").join(format!("{}.{}", id, extension))
    }

    fn get_metadata_path(&self, id: &str) -> PathBuf {
        self.base_path.join("metadata").join(format!("{}.json", id))
    }

    async fn serialize_test_case(&self, test_case: &TestCase) -> Result<String, TestCaseError> {
        match self.format {
            TestCaseFormat::Yaml => {
                serde_yaml::to_string(test_case)
                    .map_err(|e| TestCaseError::SerializationError(format!("YAML serialization failed: {}", e)))
            }
            TestCaseFormat::Json => {
                serde_json::to_string_pretty(test_case)
                    .map_err(|e| TestCaseError::SerializationError(format!("JSON serialization failed: {}", e)))
            }
        }
    }

    async fn deserialize_test_case(&self, content: &str) -> Result<TestCase, TestCaseError> {
        match self.format {
            TestCaseFormat::Yaml => {
                serde_yaml::from_str(content)
                    .map_err(|e| TestCaseError::SerializationError(format!("YAML deserialization failed: {}", e)))
            }
            TestCaseFormat::Json => {
                serde_json::from_str(content)
                    .map_err(|e| TestCaseError::SerializationError(format!("JSON deserialization failed: {}", e)))
            }
        }
    }

    async fn save_metadata(&self, test_case: &TestCase) -> Result<(), TestCaseError> {
        let metadata = TestCaseMetadata {
            id: test_case.id.clone(),
            name: test_case.name.clone(),
            description: test_case.description.clone(),
            tags: test_case.tags.clone(),
            protocol: test_case.protocol.clone(),
            created_at: test_case.created_at,
            updated_at: test_case.updated_at,
            version: test_case.version,
        };

        let metadata_json = serde_json::to_string_pretty(&metadata)
            .map_err(|e| TestCaseError::SerializationError(format!("Metadata serialization failed: {}", e)))?;

        let metadata_path = self.get_metadata_path(&test_case.id);
        let mut file = fs::File::create(&metadata_path).await
            .map_err(|e| TestCaseError::StorageError(format!("Failed to create metadata file: {}", e)))?;

        file.write_all(metadata_json.as_bytes()).await
            .map_err(|e| TestCaseError::StorageError(format!("Failed to write metadata: {}", e)))?;

        Ok(())
    }

    async fn load_metadata(&self, id: &str) -> Result<TestCaseMetadata, TestCaseError> {
        let metadata_path = self.get_metadata_path(id);
        
        if !metadata_path.exists() {
            return Err(TestCaseError::NotFound(id.to_string()));
        }

        let mut file = fs::File::open(&metadata_path).await
            .map_err(|e| TestCaseError::StorageError(format!("Failed to open metadata file: {}", e)))?;

        let mut content = String::new();
        file.read_to_string(&mut content).await
            .map_err(|e| TestCaseError::StorageError(format!("Failed to read metadata: {}", e)))?;

        serde_json::from_str(&content)
            .map_err(|e| TestCaseError::SerializationError(format!("Metadata deserialization failed: {}", e)))
    }

    async fn load_all_metadata(&self) -> Result<Vec<TestCaseMetadata>, TestCaseError> {
        let metadata_dir = self.base_path.join("metadata");
        let mut metadata_list = Vec::new();

        let mut entries = fs::read_dir(&metadata_dir).await
            .map_err(|e| TestCaseError::StorageError(format!("Failed to read metadata directory: {}", e)))?;

        while let Some(entry) = entries.next_entry().await
            .map_err(|e| TestCaseError::StorageError(format!("Failed to read directory entry: {}", e)))? {
            
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                if let Some(file_stem) = path.file_stem().and_then(|s| s.to_str()) {
                    match self.load_metadata(file_stem).await {
                        Ok(metadata) => metadata_list.push(metadata),
                        Err(_) => continue, // Skip corrupted metadata files
                    }
                }
            }
        }

        Ok(metadata_list)
    }

    fn matches_filter(&self, metadata: &TestCaseMetadata, filter: &TestCaseFilter) -> bool {
        // Check name pattern
        if let Some(pattern) = &filter.name_pattern {
            if !metadata.name.to_lowercase().contains(&pattern.to_lowercase()) {
                return false;
            }
        }

        // Check tags
        if let Some(filter_tags) = &filter.tags {
            if filter.tags_match_all {
                // All tags must be present
                if !filter_tags.iter().all(|tag| metadata.tags.contains(tag)) {
                    return false;
                }
            } else {
                // At least one tag must be present
                if !filter_tags.iter().any(|tag| metadata.tags.contains(tag)) {
                    return false;
                }
            }
        }

        // Check protocol
        if let Some(protocol) = &filter.protocol {
            if metadata.protocol != *protocol {
                return false;
            }
        }

        // Check date range
        if let Some(after) = filter.created_after {
            if metadata.created_at < after {
                return false;
            }
        }

        if let Some(before) = filter.created_before {
            if metadata.created_at > before {
                return false;
            }
        }

        true
    }
}

#[async_trait]
impl StorageBackend for FileSystemStorage {
    async fn create_test_case(&self, test_case: &TestCase) -> Result<String, TestCaseError> {
        let test_case_path = self.get_test_case_path(&test_case.id);
        
        // Check if file already exists
        if test_case_path.exists() {
            return Err(TestCaseError::AlreadyExists(test_case.id.clone()));
        }

        // Serialize and save test case
        let content = self.serialize_test_case(test_case).await?;
        let mut file = fs::File::create(&test_case_path).await
            .map_err(|e| TestCaseError::StorageError(format!("Failed to create test case file: {}", e)))?;

        file.write_all(content.as_bytes()).await
            .map_err(|e| TestCaseError::StorageError(format!("Failed to write test case: {}", e)))?;

        // Save metadata
        self.save_metadata(test_case).await?;

        Ok(test_case.id.clone())
    }

    async fn read_test_case(&self, id: &str) -> Result<TestCase, TestCaseError> {
        let test_case_path = self.get_test_case_path(id);
        
        if !test_case_path.exists() {
            return Err(TestCaseError::NotFound(id.to_string()));
        }

        let mut file = fs::File::open(&test_case_path).await
            .map_err(|e| TestCaseError::StorageError(format!("Failed to open test case file: {}", e)))?;

        let mut content = String::new();
        file.read_to_string(&mut content).await
            .map_err(|e| TestCaseError::StorageError(format!("Failed to read test case: {}", e)))?;

        self.deserialize_test_case(&content).await
    }

    async fn update_test_case(&self, id: &str, test_case: &TestCase) -> Result<(), TestCaseError> {
        let test_case_path = self.get_test_case_path(id);
        
        if !test_case_path.exists() {
            return Err(TestCaseError::NotFound(id.to_string()));
        }

        // Serialize and save updated test case
        let content = self.serialize_test_case(test_case).await?;
        let mut file = fs::File::create(&test_case_path).await
            .map_err(|e| TestCaseError::StorageError(format!("Failed to open test case file for update: {}", e)))?;

        file.write_all(content.as_bytes()).await
            .map_err(|e| TestCaseError::StorageError(format!("Failed to update test case: {}", e)))?;

        // Update metadata
        self.save_metadata(test_case).await?;

        Ok(())
    }

    async fn delete_test_case(&self, id: &str) -> Result<(), TestCaseError> {
        let test_case_path = self.get_test_case_path(id);
        let metadata_path = self.get_metadata_path(id);
        
        if !test_case_path.exists() {
            return Err(TestCaseError::NotFound(id.to_string()));
        }

        // Delete test case file
        fs::remove_file(&test_case_path).await
            .map_err(|e| TestCaseError::StorageError(format!("Failed to delete test case file: {}", e)))?;

        // Delete metadata file
        if metadata_path.exists() {
            fs::remove_file(&metadata_path).await
                .map_err(|e| TestCaseError::StorageError(format!("Failed to delete metadata file: {}", e)))?;
        }

        Ok(())
    }

    async fn list_test_cases(&self, filter: &TestCaseFilter) -> Result<Vec<TestCaseMetadata>, TestCaseError> {
        let all_metadata = self.load_all_metadata().await?;
        
        let filtered_metadata: Vec<TestCaseMetadata> = all_metadata
            .into_iter()
            .filter(|metadata| self.matches_filter(metadata, filter))
            .collect();

        Ok(filtered_metadata)
    }

    async fn test_case_exists(&self, id: &str) -> Result<bool, TestCaseError> {
        let test_case_path = self.get_test_case_path(id);
        Ok(test_case_path.exists())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use chrono::Utc;

    async fn create_test_storage() -> (FileSystemStorage, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let storage = FileSystemStorage::new(temp_dir.path().to_path_buf(), TestCaseFormat::Json)
            .await
            .unwrap();
        (storage, temp_dir)
    }

    fn create_test_case() -> TestCase {
        TestCase {
            id: "test-1".to_string(),
            name: "Test API Endpoint".to_string(),
            description: Some("A test case for API endpoint".to_string()),
            tags: vec!["api".to_string(), "integration".to_string()],
            protocol: "http".to_string(),
            request: crate::test_case_manager::RequestDefinition {
                protocol: "http".to_string(),
                method: "GET".to_string(),
                url: "https://api.example.com/users".to_string(),
                headers: std::collections::HashMap::new(),
                body: None,
                auth: None,
            },
            assertions: vec![],
            variable_extractions: None,
            dependencies: vec![],
            timeout: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            version: 1,
            change_log: vec![],
        }
    }

    #[tokio::test]
    async fn test_create_and_read_test_case() {
        let (storage, _temp_dir) = create_test_storage().await;
        let test_case = create_test_case();

        // Create test case
        let id = storage.create_test_case(&test_case).await.unwrap();
        assert_eq!(id, test_case.id);

        // Read test case
        let loaded_test_case = storage.read_test_case(&id).await.unwrap();
        assert_eq!(loaded_test_case.name, test_case.name);
        assert_eq!(loaded_test_case.protocol, test_case.protocol);
    }

    #[tokio::test]
    async fn test_update_test_case() {
        let (storage, _temp_dir) = create_test_storage().await;
        let mut test_case = create_test_case();

        // Create test case
        storage.create_test_case(&test_case).await.unwrap();

        // Update test case
        test_case.name = "Updated Test Case".to_string();
        test_case.version = 2;
        storage.update_test_case(&test_case.id, &test_case).await.unwrap();

        // Verify update
        let loaded_test_case = storage.read_test_case(&test_case.id).await.unwrap();
        assert_eq!(loaded_test_case.name, "Updated Test Case");
        assert_eq!(loaded_test_case.version, 2);
    }

    #[tokio::test]
    async fn test_delete_test_case() {
        let (storage, _temp_dir) = create_test_storage().await;
        let test_case = create_test_case();

        // Create test case
        storage.create_test_case(&test_case).await.unwrap();

        // Verify it exists
        assert!(storage.test_case_exists(&test_case.id).await.unwrap());

        // Delete test case
        storage.delete_test_case(&test_case.id).await.unwrap();

        // Verify it's deleted
        assert!(!storage.test_case_exists(&test_case.id).await.unwrap());
    }

    #[tokio::test]
    async fn test_list_test_cases_with_filter() {
        let (storage, _temp_dir) = create_test_storage().await;
        
        // Create multiple test cases
        let mut test_case1 = create_test_case();
        test_case1.id = "test-1".to_string();
        test_case1.tags = vec!["api".to_string()];
        
        let mut test_case2 = create_test_case();
        test_case2.id = "test-2".to_string();
        test_case2.name = "Database Test".to_string();
        test_case2.tags = vec!["database".to_string()];

        storage.create_test_case(&test_case1).await.unwrap();
        storage.create_test_case(&test_case2).await.unwrap();

        // Filter by tags
        let filter = TestCaseFilter {
            name_pattern: None,
            description_pattern: None,
            tags: Some(vec!["api".to_string()]),
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
        };

        let results = storage.list_test_cases(&filter).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "test-1");
    }
}