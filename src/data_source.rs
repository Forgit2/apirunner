//! Data source implementations for the API Test Runner
//! 
//! This module provides various data source plugins for loading test data from different sources
//! including CSV files, JSON files, YAML files, and databases.

use crate::plugin::{Plugin, PluginConfig, DataSourcePlugin, DataSourceType, DataSourceConfig, DataRecord, DataTransformation};
use crate::error::DataSourceError;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use tokio::fs::File;
use tokio::io::{AsyncBufReadExt, BufReader};

use jsonpath_rust::JsonPathFinder;
use sqlx::{Pool, Postgres, MySql, Sqlite, Row, Column};
use regex::Regex;

/// Configuration for file-based data sources
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileDataSourceConfig {
    pub file_path: PathBuf,
    pub streaming: bool,
    pub batch_size: Option<usize>,
    pub encoding: Option<String>,
}

/// Configuration for CSV data sources
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CsvDataSourceConfig {
    pub file_config: FileDataSourceConfig,
    pub delimiter: Option<u8>,
    pub has_headers: bool,
    pub quote_char: Option<u8>,
    pub escape_char: Option<u8>,
}

/// Configuration for JSON data sources
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonDataSourceConfig {
    pub file_config: FileDataSourceConfig,
    pub json_path: Option<String>,
    pub array_path: Option<String>,
}

/// Configuration for YAML data sources
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct YamlDataSourceConfig {
    pub file_config: FileDataSourceConfig,
    pub document_separator: bool,
    pub array_path: Option<String>,
}

/// Configuration for database data sources
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseDataSourceConfig {
    pub connection_string: String,
    pub database_type: DatabaseType,
    pub query: String,
    pub parameters: HashMap<String, serde_json::Value>,
    pub connection_pool_size: Option<u32>,
    pub batch_size: Option<usize>,
    pub transaction_support: bool,
}

/// Supported database types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DatabaseType {
    PostgreSQL,
    MySQL,
    SQLite,
}

/// CSV data source implementation with streaming support
pub struct CsvDataSource {
    config: CsvDataSourceConfig,
}

impl CsvDataSource {
    pub fn new(config: CsvDataSourceConfig) -> Self {
        Self { config }
    }

    fn parse_csv_line(&self, line: &str) -> Result<Vec<String>, DataSourceError> {
        let delimiter = self.config.delimiter.unwrap_or(b',') as char;
        let quote_char = self.config.quote_char.map(|c| c as char).unwrap_or('"');
        
        let mut fields = Vec::new();
        let mut current_field = String::new();
        let mut in_quotes = false;
        let mut chars = line.chars().peekable();
        
        while let Some(ch) = chars.next() {
            match ch {
                c if c == quote_char => {
                    if in_quotes && chars.peek() == Some(&quote_char) {
                        // Escaped quote
                        current_field.push(quote_char);
                        chars.next(); // consume the second quote
                    } else {
                        in_quotes = !in_quotes;
                    }
                }
                c if c == delimiter && !in_quotes => {
                    fields.push(current_field.trim().to_string());
                    current_field.clear();
                }
                c => current_field.push(c),
            }
        }
        
        fields.push(current_field.trim().to_string());
        Ok(fields)
    }

    fn parse_csv_record(line: &str, headers: &[String], delimiter: u8) -> Result<DataRecord, DataSourceError> {
        let delimiter_char = delimiter as char;
        let fields: Vec<&str> = line.split(delimiter_char).collect();
        
        let mut fields_map = HashMap::new();
        
        if headers.is_empty() {
            // No headers, use numeric indices
            for (i, field) in fields.iter().enumerate() {
                fields_map.insert(i.to_string(), serde_json::Value::String(field.trim().to_string()));
            }
        } else {
            // Use headers as keys
            for (i, header) in headers.iter().enumerate() {
                let value = fields.get(i).unwrap_or(&"").trim();
                fields_map.insert(header.clone(), serde_json::Value::String(value.to_string()));
            }
        }
        
        Ok(DataRecord {
            id: uuid::Uuid::new_v4().to_string(),
            fields: fields_map,
            metadata: HashMap::new(),
        })
    }
}

#[async_trait]
impl Plugin for CsvDataSource {
    fn name(&self) -> &str {
        "csv_data_source"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn dependencies(&self) -> Vec<String> {
        vec![]
    }

    async fn initialize(&mut self, _config: &PluginConfig) -> Result<(), crate::error::PluginError> {
        Ok(())
    }

    async fn shutdown(&mut self) -> Result<(), crate::error::PluginError> {
        Ok(())
    }
}

#[async_trait]
impl DataSourcePlugin for CsvDataSource {
    fn supported_types(&self) -> Vec<DataSourceType> {
        vec![DataSourceType::Csv]
    }

    async fn connect(&mut self, _config: &DataSourceConfig) -> Result<(), DataSourceError> {
        // CSV files don't require connection
        Ok(())
    }

    async fn disconnect(&mut self) -> Result<(), DataSourceError> {
        // CSV files don't require disconnection
        Ok(())
    }

    async fn read_data(&self, _query: Option<&str>) -> Result<Vec<DataRecord>, DataSourceError> {
        let file = File::open(&self.config.file_config.file_path).await
            .map_err(|e| DataSourceError::FileError(format!("Failed to open CSV file: {}", e)))?;
        
        let reader = BufReader::new(file);
        let mut lines = reader.lines();
        let mut records = Vec::new();
        
        // Skip header if present
        let mut headers = Vec::new();
        if self.config.has_headers {
            if let Some(header_line) = lines.next_line().await
                .map_err(|e| DataSourceError::ParseError(format!("Failed to read header: {}", e)))? {
                headers = self.parse_csv_line(&header_line)?;
            }
        }

        let delimiter = self.config.delimiter.unwrap_or(b',');
        
        while let Some(line) = lines.next_line().await
            .map_err(|e| DataSourceError::FileError(format!("Failed to read line: {}", e)))? {
            let record = Self::parse_csv_record(&line, &headers, delimiter)?;
            records.push(record);
        }
        
        Ok(records)
    }

    async fn transform_data(&self, data: Vec<DataRecord>, transformations: &[DataTransformation]) -> Result<Vec<DataRecord>, DataSourceError> {
        if transformations.is_empty() {
            return Ok(data);
        }
        
        let pipeline = DataTransformationPipeline::from_transformations(transformations)?;
        pipeline.transform(data).await
    }

    fn validate_config(&self, _config: &DataSourceConfig) -> Result<(), DataSourceError> {
        if !self.config.file_config.file_path.exists() {
            return Err(DataSourceError::ConfigError(
                format!("CSV file does not exist: {:?}", self.config.file_config.file_path)
            ));
        }
        Ok(())
    }
}

/// JSON data source implementation with JSONPath support
pub struct JsonDataSource {
    config: JsonDataSourceConfig,
}

impl JsonDataSource {
    pub fn new(config: JsonDataSourceConfig) -> Self {
        Self { config }
    }

    async fn load_json_data(&self) -> Result<serde_json::Value, DataSourceError> {
        let content = tokio::fs::read_to_string(&self.config.file_config.file_path).await
            .map_err(|e| DataSourceError::FileError(format!("Failed to read JSON file: {}", e)))?;
        
        serde_json::from_str(&content)
            .map_err(|e| DataSourceError::ParseError(format!("Invalid JSON: {}", e)))
    }

    fn extract_records_from_json(&self, json_data: &serde_json::Value) -> Result<Vec<DataRecord>, DataSourceError> {
        let mut records = Vec::new();
        
        let data_items: Vec<&serde_json::Value> = if let Some(json_path) = &self.config.json_path {
            // Use JSONPath to extract data
            let json_str = serde_json::to_string(json_data)
                .map_err(|e| DataSourceError::ParseError(format!("Failed to serialize JSON: {}", e)))?;
            let finder = JsonPathFinder::from_str(&json_str, json_path)
                .map_err(|e| DataSourceError::ParseError(format!("JSONPath error: {}", e)))?;
            
            // For now, use simple path extraction as JSONPath library is complex
            // This is a simplified implementation that supports basic path queries
            if json_path.starts_with('$') && json_path.contains('.') {
                let path_parts: Vec<&str> = json_path[1..].split('.').collect(); // Remove '$' prefix
                let mut current = json_data;
                
                for part in path_parts {
                    if part.is_empty() { continue; }
                    current = current.get(part)
                        .ok_or_else(|| DataSourceError::ParseError(format!("JSONPath not found: {}", json_path)))?;
                }
                
                match current {
                    serde_json::Value::Array(arr) => arr.iter().collect(),
                    other => vec![other],
                }
            } else {
                vec![json_data]
            }
        } else if let Some(array_path) = &self.config.array_path {
            // Extract array from specific path
            let path_parts: Vec<&str> = array_path.split('.').collect();
            let mut current = json_data;
            
            for part in path_parts {
                current = current.get(part)
                    .ok_or_else(|| DataSourceError::ParseError(format!("Path not found: {}", array_path)))?;
            }
            
            vec![current]
        } else {
            // Use root data
            vec![json_data]
        };
        
        for (index, item) in data_items.iter().enumerate() {
            match item {
                serde_json::Value::Object(obj) => {
                    let mut fields = HashMap::new();
                    for (key, value) in obj {
                        fields.insert(key.clone(), value.clone());
                    }
                    
                    records.push(DataRecord {
                        id: uuid::Uuid::new_v4().to_string(),
                        fields,
                        metadata: HashMap::from([
                            ("source_index".to_string(), index.to_string()),
                        ]),
                    });
                }
                serde_json::Value::Array(arr) => {
                    for (arr_index, arr_item) in arr.iter().enumerate() {
                        if let serde_json::Value::Object(obj) = arr_item {
                            let mut fields = HashMap::new();
                            for (key, value) in obj {
                                fields.insert(key.clone(), value.clone());
                            }
                            
                            records.push(DataRecord {
                                id: uuid::Uuid::new_v4().to_string(),
                                fields,
                                metadata: HashMap::from([
                                    ("source_index".to_string(), index.to_string()),
                                    ("array_index".to_string(), arr_index.to_string()),
                                ]),
                            });
                        }
                    }
                }
                _ => {
                    // Treat primitive values as single-field records
                    let mut fields = HashMap::new();
                    fields.insert("value".to_string(), (*item).clone());
                    
                    records.push(DataRecord {
                        id: uuid::Uuid::new_v4().to_string(),
                        fields,
                        metadata: HashMap::from([
                            ("source_index".to_string(), index.to_string()),
                        ]),
                    });
                }
            }
        }
        
        Ok(records)
    }
}

#[async_trait]
impl Plugin for JsonDataSource {
    fn name(&self) -> &str {
        "json_data_source"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn dependencies(&self) -> Vec<String> {
        vec![]
    }

    async fn initialize(&mut self, _config: &PluginConfig) -> Result<(), crate::error::PluginError> {
        Ok(())
    }

    async fn shutdown(&mut self) -> Result<(), crate::error::PluginError> {
        Ok(())
    }
}

#[async_trait]
impl DataSourcePlugin for JsonDataSource {
    fn supported_types(&self) -> Vec<DataSourceType> {
        vec![DataSourceType::Json]
    }

    async fn connect(&mut self, _config: &DataSourceConfig) -> Result<(), DataSourceError> {
        // JSON files don't require connection
        Ok(())
    }

    async fn disconnect(&mut self) -> Result<(), DataSourceError> {
        // JSON files don't require disconnection
        Ok(())
    }

    async fn read_data(&self, _query: Option<&str>) -> Result<Vec<DataRecord>, DataSourceError> {
        let json_data = self.load_json_data().await?;
        self.extract_records_from_json(&json_data)
    }

    async fn transform_data(&self, data: Vec<DataRecord>, transformations: &[DataTransformation]) -> Result<Vec<DataRecord>, DataSourceError> {
        if transformations.is_empty() {
            return Ok(data);
        }
        
        let pipeline = DataTransformationPipeline::from_transformations(transformations)?;
        pipeline.transform(data).await
    }

    fn validate_config(&self, _config: &DataSourceConfig) -> Result<(), DataSourceError> {
        if !self.config.file_config.file_path.exists() {
            return Err(DataSourceError::ConfigError(
                format!("JSON file does not exist: {:?}", self.config.file_config.file_path)
            ));
        }
        
        // Validate JSONPath if provided
        if let Some(json_path) = &self.config.json_path {
            // Basic JSONPath validation - more comprehensive validation would require parsing
            if json_path.is_empty() {
                return Err(DataSourceError::ConfigError("JSONPath cannot be empty".to_string()));
            }
        }
        
        Ok(())
    }
}

/// YAML data source implementation with complex structure support
pub struct YamlDataSource {
    config: YamlDataSourceConfig,
}

impl YamlDataSource {
    pub fn new(config: YamlDataSourceConfig) -> Self {
        Self { config }
    }

    async fn load_yaml_data(&self) -> Result<Vec<serde_yaml::Value>, DataSourceError> {
        let content = tokio::fs::read_to_string(&self.config.file_config.file_path).await
            .map_err(|e| DataSourceError::FileError(format!("Failed to read YAML file: {}", e)))?;
        
        if self.config.document_separator {
            // Handle multiple YAML documents
            let documents: Result<Vec<serde_yaml::Value>, _> = serde_yaml::Deserializer::from_str(&content)
                .map(|doc| serde_yaml::Value::deserialize(doc))
                .collect();
            
            documents.map_err(|e| DataSourceError::ParseError(format!("Invalid YAML: {}", e)))
        } else {
            // Single YAML document
            let doc: serde_yaml::Value = serde_yaml::from_str(&content)
                .map_err(|e| DataSourceError::ParseError(format!("Invalid YAML: {}", e)))?;
            Ok(vec![doc])
        }
    }

    fn extract_records_from_yaml(&self, yaml_docs: &[serde_yaml::Value]) -> Result<Vec<DataRecord>, DataSourceError> {
        let mut records = Vec::new();
        
        for (doc_index, doc) in yaml_docs.iter().enumerate() {
            let data_items = if let Some(array_path) = &self.config.array_path {
                // Extract array from specific path
                let path_parts: Vec<&str> = array_path.split('.').collect();
                let mut current = doc;
                
                for part in path_parts {
                    current = current.get(part)
                        .ok_or_else(|| DataSourceError::ParseError(format!("Path not found: {}", array_path)))?;
                }
                
                match current {
                    serde_yaml::Value::Sequence(seq) => seq.clone(),
                    other => vec![other.clone()],
                }
            } else {
                match doc {
                    serde_yaml::Value::Sequence(seq) => seq.clone(),
                    other => vec![other.clone()],
                }
            };
            
            for (item_index, item) in data_items.iter().enumerate() {
                match item {
                    serde_yaml::Value::Mapping(mapping) => {
                        let mut fields = HashMap::new();
                        
                        for (key, value) in mapping {
                            if let serde_yaml::Value::String(key_str) = key {
                                // Convert YAML value to JSON value for consistency
                                let json_value = self.yaml_to_json_value(value)?;
                                fields.insert(key_str.clone(), json_value);
                            }
                        }
                        
                        records.push(DataRecord {
                            id: uuid::Uuid::new_v4().to_string(),
                            fields,
                            metadata: HashMap::from([
                                ("document_index".to_string(), doc_index.to_string()),
                                ("item_index".to_string(), item_index.to_string()),
                            ]),
                        });
                    }
                    _ => {
                        // Treat non-mapping values as single-field records
                        let mut fields = HashMap::new();
                        let json_value = self.yaml_to_json_value(item)?;
                        fields.insert("value".to_string(), json_value);
                        
                        records.push(DataRecord {
                            id: uuid::Uuid::new_v4().to_string(),
                            fields,
                            metadata: HashMap::from([
                                ("document_index".to_string(), doc_index.to_string()),
                                ("item_index".to_string(), item_index.to_string()),
                            ]),
                        });
                    }
                }
            }
        }
        
        Ok(records)
    }

    fn yaml_to_json_value(&self, yaml_value: &serde_yaml::Value) -> Result<serde_json::Value, DataSourceError> {
        match yaml_value {
            serde_yaml::Value::Null => Ok(serde_json::Value::Null),
            serde_yaml::Value::Bool(b) => Ok(serde_json::Value::Bool(*b)),
            serde_yaml::Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    Ok(serde_json::Value::Number(i.into()))
                } else if let Some(f) = n.as_f64() {
                    Ok(serde_json::Number::from_f64(f)
                        .map(serde_json::Value::Number)
                        .ok_or_else(|| DataSourceError::ParseError("Invalid number in YAML".to_string()))?)
                } else {
                    Err(DataSourceError::ParseError("Unsupported number format in YAML".to_string()))
                }
            }
            serde_yaml::Value::String(s) => Ok(serde_json::Value::String(s.clone())),
            serde_yaml::Value::Sequence(seq) => {
                let mut json_array = Vec::new();
                for item in seq {
                    json_array.push(self.yaml_to_json_value(item)?);
                }
                Ok(serde_json::Value::Array(json_array))
            }
            serde_yaml::Value::Mapping(mapping) => {
                let mut json_object = serde_json::Map::new();
                for (key, value) in mapping {
                    if let serde_yaml::Value::String(key_str) = key {
                        json_object.insert(key_str.clone(), self.yaml_to_json_value(value)?);
                    }
                }
                Ok(serde_json::Value::Object(json_object))
            }
            serde_yaml::Value::Tagged(tagged) => {
                // Handle tagged values by extracting the inner value
                self.yaml_to_json_value(&tagged.value)
            }
        }
    }
}

#[async_trait]
impl Plugin for YamlDataSource {
    fn name(&self) -> &str {
        "yaml_data_source"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn dependencies(&self) -> Vec<String> {
        vec![]
    }

    async fn initialize(&mut self, _config: &PluginConfig) -> Result<(), crate::error::PluginError> {
        Ok(())
    }

    async fn shutdown(&mut self) -> Result<(), crate::error::PluginError> {
        Ok(())
    }
}

#[async_trait]
impl DataSourcePlugin for YamlDataSource {
    fn supported_types(&self) -> Vec<DataSourceType> {
        vec![DataSourceType::Yaml]
    }

    async fn connect(&mut self, _config: &DataSourceConfig) -> Result<(), DataSourceError> {
        // YAML files don't require connection
        Ok(())
    }

    async fn disconnect(&mut self) -> Result<(), DataSourceError> {
        // YAML files don't require disconnection
        Ok(())
    }

    async fn read_data(&self, _query: Option<&str>) -> Result<Vec<DataRecord>, DataSourceError> {
        let yaml_docs = self.load_yaml_data().await?;
        self.extract_records_from_yaml(&yaml_docs)
    }

    async fn transform_data(&self, data: Vec<DataRecord>, transformations: &[DataTransformation]) -> Result<Vec<DataRecord>, DataSourceError> {
        if transformations.is_empty() {
            return Ok(data);
        }
        
        let pipeline = DataTransformationPipeline::from_transformations(transformations)?;
        pipeline.transform(data).await
    }

    fn validate_config(&self, _config: &DataSourceConfig) -> Result<(), DataSourceError> {
        if !self.config.file_config.file_path.exists() {
            return Err(DataSourceError::ConfigError(
                format!("YAML file does not exist: {:?}", self.config.file_config.file_path)
            ));
        }
        Ok(())
    }
}

/// Database data source implementation with connection pooling
pub struct DatabaseDataSource {
    config: DatabaseDataSourceConfig,
    pool: Option<DatabasePool>,
}

/// Enum to handle different database connection pools
pub enum DatabasePool {
    PostgreSQL(Pool<Postgres>),
    MySQL(Pool<MySql>),
    SQLite(Pool<Sqlite>),
}

impl DatabaseDataSource {
    pub fn new(config: DatabaseDataSourceConfig) -> Self {
        Self { 
            config,
            pool: None,
        }
    }

    pub async fn initialize(&mut self) -> Result<(), DataSourceError> {
        let pool_size = self.config.connection_pool_size.unwrap_or(10);
        
        let pool = match self.config.database_type {
            DatabaseType::PostgreSQL => {
                let pool = sqlx::postgres::PgPoolOptions::new()
                    .max_connections(pool_size)
                    .connect(&self.config.connection_string)
                    .await
                    .map_err(|e| DataSourceError::ConnectionError(format!("PostgreSQL connection failed: {}", e)))?;
                DatabasePool::PostgreSQL(pool)
            }
            DatabaseType::MySQL => {
                let pool = sqlx::mysql::MySqlPoolOptions::new()
                    .max_connections(pool_size)
                    .connect(&self.config.connection_string)
                    .await
                    .map_err(|e| DataSourceError::ConnectionError(format!("MySQL connection failed: {}", e)))?;
                DatabasePool::MySQL(pool)
            }
            DatabaseType::SQLite => {
                let pool = sqlx::sqlite::SqlitePoolOptions::new()
                    .max_connections(pool_size)
                    .connect(&self.config.connection_string)
                    .await
                    .map_err(|e| DataSourceError::ConnectionError(format!("SQLite connection failed: {}", e)))?;
                DatabasePool::SQLite(pool)
            }
        };
        
        self.pool = Some(pool);
        Ok(())
    }
}

#[async_trait]
impl Plugin for DatabaseDataSource {
    fn name(&self) -> &str {
        "database_data_source"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn dependencies(&self) -> Vec<String> {
        vec![]
    }

    async fn initialize(&mut self, _config: &PluginConfig) -> Result<(), crate::error::PluginError> {
        Ok(())
    }

    async fn shutdown(&mut self) -> Result<(), crate::error::PluginError> {
        Ok(())
    }
}

#[async_trait]
impl DataSourcePlugin for DatabaseDataSource {
    fn supported_types(&self) -> Vec<DataSourceType> {
        vec![DataSourceType::Database]
    }

    async fn connect(&mut self, _config: &DataSourceConfig) -> Result<(), DataSourceError> {
        self.initialize().await
    }

    async fn disconnect(&mut self) -> Result<(), DataSourceError> {
        self.pool = None;
        Ok(())
    }

    async fn read_data(&self, _query: Option<&str>) -> Result<Vec<DataRecord>, DataSourceError> {
        let pool = self.pool.as_ref()
            .ok_or_else(|| DataSourceError::ConnectionError("Database pool not initialized".to_string()))?;

        let mut records = Vec::new();

        match pool {
            DatabasePool::PostgreSQL(pg_pool) => {
                let mut query = sqlx::query(&self.config.query);
                
                // Bind parameters
                for (_key, value) in &self.config.parameters {
                    query = match value {
                        serde_json::Value::String(s) => query.bind(s),
                        serde_json::Value::Number(n) => {
                            if let Some(i) = n.as_i64() {
                                query.bind(i)
                            } else if let Some(f) = n.as_f64() {
                                query.bind(f)
                            } else {
                                return Err(DataSourceError::ParseError("Invalid number parameter".to_string()));
                            }
                        }
                        serde_json::Value::Bool(b) => query.bind(*b),
                        serde_json::Value::Null => query.bind(Option::<String>::None),
                        _ => return Err(DataSourceError::ParseError("Unsupported parameter type".to_string())),
                    };
                }

                let rows = query.fetch_all(pg_pool).await
                    .map_err(|e| DataSourceError::QueryError(format!("PostgreSQL query failed: {}", e)))?;

                for (index, row) in rows.iter().enumerate() {
                    let mut fields = HashMap::new();
                    
                    // Extract all columns from the row
                    for (col_index, column) in row.columns().iter().enumerate() {
                        let column_name = column.name();
                        // For now, just extract as string - could be enhanced to handle types properly
                        let value: Option<String> = row.try_get(col_index).ok();
                        fields.insert(column_name.to_string(), 
                            value.map(serde_json::Value::String).unwrap_or(serde_json::Value::Null));
                    }

                    records.push(DataRecord {
                        id: uuid::Uuid::new_v4().to_string(),
                        fields,
                        metadata: HashMap::from([
                            ("row_index".to_string(), index.to_string()),
                            ("database_type".to_string(), "postgresql".to_string()),
                        ]),
                    });
                }
            }
            DatabasePool::MySQL(_mysql_pool) => {
                // Similar implementation for MySQL - simplified for now
                return Err(DataSourceError::ConfigError("MySQL support not fully implemented yet".to_string()));
            }
            DatabasePool::SQLite(_sqlite_pool) => {
                // Similar implementation for SQLite - simplified for now
                return Err(DataSourceError::ConfigError("SQLite support not fully implemented yet".to_string()));
            }
        }

        Ok(records)
    }

    async fn transform_data(&self, data: Vec<DataRecord>, transformations: &[DataTransformation]) -> Result<Vec<DataRecord>, DataSourceError> {
        if transformations.is_empty() {
            return Ok(data);
        }
        
        let pipeline = DataTransformationPipeline::from_transformations(transformations)?;
        pipeline.transform(data).await
    }

    fn validate_config(&self, _config: &DataSourceConfig) -> Result<(), DataSourceError> {
        if self.config.connection_string.is_empty() {
            return Err(DataSourceError::ConfigError("Connection string cannot be empty".to_string()));
        }
        
        if self.config.query.is_empty() {
            return Err(DataSourceError::ConfigError("Query cannot be empty".to_string()));
        }
        
        Ok(())
    }
}

/// Data transformation pipeline with configurable stages
pub struct DataTransformationPipeline {
    stages: Vec<Box<dyn TransformationStage>>,
}

/// Trait for transformation stages
#[async_trait]
pub trait TransformationStage: Send + Sync {
    async fn transform(&self, records: Vec<DataRecord>) -> Result<Vec<DataRecord>, DataSourceError>;
    fn name(&self) -> &str;
}

impl DataTransformationPipeline {
    pub fn new() -> Self {
        Self {
            stages: Vec::new(),
        }
    }

    pub fn add_stage(mut self, stage: Box<dyn TransformationStage>) -> Self {
        self.stages.push(stage);
        self
    }

    pub async fn transform(&self, mut records: Vec<DataRecord>) -> Result<Vec<DataRecord>, DataSourceError> {
        for stage in &self.stages {
            records = stage.transform(records).await?;
        }
        Ok(records)
    }

    pub fn from_transformations(transformations: &[DataTransformation]) -> Result<Self, DataSourceError> {
        let mut pipeline = Self::new();
        
        for transformation in transformations {
            let stage: Box<dyn TransformationStage> = match transformation.transformation_type.as_str() {
                "field_mapping" => Box::new(FieldMappingTransform::from_config(&transformation.parameters)?),
                "value_substitution" => Box::new(ValueSubstitutionTransform::from_config(&transformation.parameters)?),
                "type_conversion" => Box::new(TypeConversionTransform::from_config(&transformation.parameters)?),
                "field_filter" => Box::new(FieldFilterTransform::from_config(&transformation.parameters)?),
                "value_validation" => Box::new(ValueValidationTransform::from_config(&transformation.parameters)?),
                _ => return Err(DataSourceError::TransformationFailed(
                    format!("Unknown transformation type: {}", transformation.transformation_type)
                )),
            };
            pipeline = pipeline.add_stage(stage);
        }
        
        Ok(pipeline)
    }
}

/// Field mapping transformation - renames fields
pub struct FieldMappingTransform {
    mappings: HashMap<String, String>,
}

impl FieldMappingTransform {
    pub fn new(mappings: HashMap<String, String>) -> Self {
        Self { mappings }
    }

    pub fn from_config(config: &HashMap<String, serde_json::Value>) -> Result<Self, DataSourceError> {
        let mappings_value = config.get("mappings")
            .ok_or_else(|| DataSourceError::ConfigError("Missing 'mappings' in field mapping config".to_string()))?;
        
        let mappings: HashMap<String, String> = serde_json::from_value(mappings_value.clone())
            .map_err(|e| DataSourceError::ConfigError(format!("Invalid mappings format: {}", e)))?;
        
        Ok(Self::new(mappings))
    }
}

#[async_trait]
impl TransformationStage for FieldMappingTransform {
    async fn transform(&self, records: Vec<DataRecord>) -> Result<Vec<DataRecord>, DataSourceError> {
        let mut transformed_records = Vec::new();
        
        for mut record in records {
            let mut new_fields = HashMap::new();
            
            for (old_key, value) in record.fields {
                let new_key = self.mappings.get(&old_key).unwrap_or(&old_key);
                new_fields.insert(new_key.clone(), value);
            }
            
            record.fields = new_fields;
            transformed_records.push(record);
        }
        
        Ok(transformed_records)
    }

    fn name(&self) -> &str {
        "field_mapping"
    }
}

/// Value substitution transformation - replaces values based on patterns
pub struct ValueSubstitutionTransform {
    substitutions: HashMap<String, HashMap<String, String>>, // field -> (old_value -> new_value)
    regex_substitutions: HashMap<String, Vec<(Regex, String)>>, // field -> [(pattern, replacement)]
}

impl ValueSubstitutionTransform {
    pub fn new(
        substitutions: HashMap<String, HashMap<String, String>>,
        regex_substitutions: HashMap<String, Vec<(Regex, String)>>,
    ) -> Self {
        Self { 
            substitutions,
            regex_substitutions,
        }
    }

    pub fn from_config(config: &HashMap<String, serde_json::Value>) -> Result<Self, DataSourceError> {
        let substitutions = if let Some(subs_value) = config.get("substitutions") {
            serde_json::from_value(subs_value.clone())
                .map_err(|e| DataSourceError::ConfigError(format!("Invalid substitutions format: {}", e)))?
        } else {
            HashMap::new()
        };

        let regex_substitutions = if let Some(regex_subs_value) = config.get("regex_substitutions") {
            let regex_config: HashMap<String, Vec<(String, String)>> = serde_json::from_value(regex_subs_value.clone())
                .map_err(|e| DataSourceError::ConfigError(format!("Invalid regex substitutions format: {}", e)))?;
            
            let mut compiled_regex = HashMap::new();
            for (field, patterns) in regex_config {
                let mut compiled_patterns = Vec::new();
                for (pattern, replacement) in patterns {
                    let regex = Regex::new(&pattern)
                        .map_err(|e| DataSourceError::ConfigError(format!("Invalid regex pattern '{}': {}", pattern, e)))?;
                    compiled_patterns.push((regex, replacement));
                }
                compiled_regex.insert(field, compiled_patterns);
            }
            compiled_regex
        } else {
            HashMap::new()
        };

        Ok(Self::new(substitutions, regex_substitutions))
    }
}

#[async_trait]
impl TransformationStage for ValueSubstitutionTransform {
    async fn transform(&self, records: Vec<DataRecord>) -> Result<Vec<DataRecord>, DataSourceError> {
        let mut transformed_records = Vec::new();
        
        for mut record in records {
            for (field_name, value) in &mut record.fields {
                if let serde_json::Value::String(string_value) = value {
                    // Apply exact substitutions
                    if let Some(field_substitutions) = self.substitutions.get(field_name) {
                        if let Some(new_value) = field_substitutions.get(string_value) {
                            *value = serde_json::Value::String(new_value.clone());
                            continue;
                        }
                    }
                    
                    // Apply regex substitutions
                    if let Some(regex_patterns) = self.regex_substitutions.get(field_name) {
                        let mut result = string_value.clone();
                        for (regex, replacement) in regex_patterns {
                            result = regex.replace_all(&result, replacement).to_string();
                        }
                        if result != *string_value {
                            *value = serde_json::Value::String(result);
                        }
                    }
                }
            }
            transformed_records.push(record);
        }
        
        Ok(transformed_records)
    }

    fn name(&self) -> &str {
        "value_substitution"
    }
}

/// Data type for type conversion
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DataType {
    String,
    Integer,
    Float,
    Boolean,
    Array,
    Object,
}

/// Type conversion transformation - converts field types
pub struct TypeConversionTransform {
    conversions: HashMap<String, DataType>,
}

impl TypeConversionTransform {
    pub fn new(conversions: HashMap<String, DataType>) -> Self {
        Self { conversions }
    }

    pub fn from_config(config: &HashMap<String, serde_json::Value>) -> Result<Self, DataSourceError> {
        let conversions_value = config.get("conversions")
            .ok_or_else(|| DataSourceError::ConfigError("Missing 'conversions' in type conversion config".to_string()))?;
        
        let conversions: HashMap<String, DataType> = serde_json::from_value(conversions_value.clone())
            .map_err(|e| DataSourceError::ConfigError(format!("Invalid conversions format: {}", e)))?;
        
        Ok(Self::new(conversions))
    }

    fn convert_value(&self, value: &serde_json::Value, target_type: &DataType) -> Result<serde_json::Value, DataSourceError> {
        match (value, target_type) {
            (_, DataType::String) => Ok(serde_json::Value::String(
                match value {
                    serde_json::Value::String(s) => s.clone(),
                    other => other.to_string(),
                }
            )),
            (serde_json::Value::String(s), DataType::Integer) => {
                let int_val: i64 = s.parse()
                    .map_err(|_| DataSourceError::TransformationFailed(format!("Cannot convert '{}' to integer", s)))?;
                Ok(serde_json::Value::Number(int_val.into()))
            }
            (serde_json::Value::Number(n), DataType::Integer) => {
                if let Some(int_val) = n.as_i64() {
                    Ok(serde_json::Value::Number(int_val.into()))
                } else {
                    Err(DataSourceError::TransformationFailed("Number is not an integer".to_string()))
                }
            }
            (serde_json::Value::String(s), DataType::Float) => {
                let float_val: f64 = s.parse()
                    .map_err(|_| DataSourceError::TransformationFailed(format!("Cannot convert '{}' to float", s)))?;
                Ok(serde_json::Number::from_f64(float_val)
                    .map(serde_json::Value::Number)
                    .ok_or_else(|| DataSourceError::TransformationFailed("Invalid float value".to_string()))?)
            }
            (serde_json::Value::Number(n), DataType::Float) => {
                if let Some(float_val) = n.as_f64() {
                    Ok(serde_json::Number::from_f64(float_val)
                        .map(serde_json::Value::Number)
                        .ok_or_else(|| DataSourceError::TransformationFailed("Invalid float value".to_string()))?)
                } else {
                    Err(DataSourceError::TransformationFailed("Number cannot be converted to float".to_string()))
                }
            }
            (serde_json::Value::String(s), DataType::Boolean) => {
                let bool_val = match s.to_lowercase().as_str() {
                    "true" | "1" | "yes" | "on" => true,
                    "false" | "0" | "no" | "off" => false,
                    _ => return Err(DataSourceError::TransformationFailed(format!("Cannot convert '{}' to boolean", s))),
                };
                Ok(serde_json::Value::Bool(bool_val))
            }
            (serde_json::Value::Bool(b), DataType::Boolean) => Ok(serde_json::Value::Bool(*b)),
            (serde_json::Value::String(s), DataType::Array) => {
                let array: Vec<serde_json::Value> = serde_json::from_str(s)
                    .map_err(|_| DataSourceError::TransformationFailed(format!("Cannot parse '{}' as JSON array", s)))?;
                Ok(serde_json::Value::Array(array))
            }
            (serde_json::Value::Array(arr), DataType::Array) => Ok(serde_json::Value::Array(arr.clone())),
            (serde_json::Value::String(s), DataType::Object) => {
                let object: serde_json::Value = serde_json::from_str(s)
                    .map_err(|_| DataSourceError::TransformationFailed(format!("Cannot parse '{}' as JSON object", s)))?;
                Ok(object)
            }
            (serde_json::Value::Object(obj), DataType::Object) => Ok(serde_json::Value::Object(obj.clone())),
            _ => Err(DataSourceError::TransformationFailed(
                format!("Cannot convert {:?} to {:?}", value, target_type)
            )),
        }
    }
}

#[async_trait]
impl TransformationStage for TypeConversionTransform {
    async fn transform(&self, records: Vec<DataRecord>) -> Result<Vec<DataRecord>, DataSourceError> {
        let mut transformed_records = Vec::new();
        
        for mut record in records {
            for (field_name, target_type) in &self.conversions {
                if let Some(value) = record.fields.get(field_name) {
                    let converted_value = self.convert_value(value, target_type)?;
                    record.fields.insert(field_name.clone(), converted_value);
                }
            }
            transformed_records.push(record);
        }
        
        Ok(transformed_records)
    }

    fn name(&self) -> &str {
        "type_conversion"
    }
}

/// Field filter transformation - removes or keeps specific fields
pub struct FieldFilterTransform {
    include_fields: Option<Vec<String>>,
    exclude_fields: Option<Vec<String>>,
}

impl FieldFilterTransform {
    pub fn new(include_fields: Option<Vec<String>>, exclude_fields: Option<Vec<String>>) -> Self {
        Self { include_fields, exclude_fields }
    }

    pub fn from_config(config: &HashMap<String, serde_json::Value>) -> Result<Self, DataSourceError> {
        let include_fields = if let Some(include_value) = config.get("include_fields") {
            Some(serde_json::from_value(include_value.clone())
                .map_err(|e| DataSourceError::ConfigError(format!("Invalid include_fields format: {}", e)))?)
        } else {
            None
        };

        let exclude_fields = if let Some(exclude_value) = config.get("exclude_fields") {
            Some(serde_json::from_value(exclude_value.clone())
                .map_err(|e| DataSourceError::ConfigError(format!("Invalid exclude_fields format: {}", e)))?)
        } else {
            None
        };

        Ok(Self::new(include_fields, exclude_fields))
    }
}

#[async_trait]
impl TransformationStage for FieldFilterTransform {
    async fn transform(&self, records: Vec<DataRecord>) -> Result<Vec<DataRecord>, DataSourceError> {
        let mut transformed_records = Vec::new();
        
        for mut record in records {
            let mut new_fields = HashMap::new();
            
            for (field_name, value) in record.fields {
                let should_include = match (&self.include_fields, &self.exclude_fields) {
                    (Some(include), _) => include.contains(&field_name),
                    (None, Some(exclude)) => !exclude.contains(&field_name),
                    (None, None) => true,
                };
                
                if should_include {
                    new_fields.insert(field_name, value);
                }
            }
            
            record.fields = new_fields;
            transformed_records.push(record);
        }
        
        Ok(transformed_records)
    }

    fn name(&self) -> &str {
        "field_filter"
    }
}

/// Value validation transformation - validates field values
pub struct ValueValidationTransform {
    validations: HashMap<String, ValidationRule>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ValidationRule {
    Required,
    MinLength(usize),
    MaxLength(usize),
    Pattern(String),
    Range { min: f64, max: f64 },
    OneOf(Vec<String>),
}

impl ValueValidationTransform {
    pub fn new(validations: HashMap<String, ValidationRule>) -> Self {
        Self { validations }
    }

    pub fn from_config(config: &HashMap<String, serde_json::Value>) -> Result<Self, DataSourceError> {
        let validations_value = config.get("validations")
            .ok_or_else(|| DataSourceError::ConfigError("Missing 'validations' in validation config".to_string()))?;
        
        let validations: HashMap<String, ValidationRule> = serde_json::from_value(validations_value.clone())
            .map_err(|e| DataSourceError::ConfigError(format!("Invalid validations format: {}", e)))?;
        
        Ok(Self::new(validations))
    }

    fn validate_value(&self, field_name: &str, value: &serde_json::Value, rule: &ValidationRule) -> Result<(), DataSourceError> {
        match rule {
            ValidationRule::Required => {
                if value.is_null() {
                    return Err(DataSourceError::TransformationFailed(
                        format!("Field '{}' is required but is null", field_name)
                    ));
                }
            }
            ValidationRule::MinLength(min_len) => {
                if let serde_json::Value::String(s) = value {
                    if s.len() < *min_len {
                        return Err(DataSourceError::TransformationFailed(
                            format!("Field '{}' length {} is less than minimum {}", field_name, s.len(), min_len)
                        ));
                    }
                }
            }
            ValidationRule::MaxLength(max_len) => {
                if let serde_json::Value::String(s) = value {
                    if s.len() > *max_len {
                        return Err(DataSourceError::TransformationFailed(
                            format!("Field '{}' length {} exceeds maximum {}", field_name, s.len(), max_len)
                        ));
                    }
                }
            }
            ValidationRule::Pattern(pattern) => {
                if let serde_json::Value::String(s) = value {
                    let regex = Regex::new(pattern)
                        .map_err(|e| DataSourceError::ConfigError(format!("Invalid regex pattern: {}", e)))?;
                    if !regex.is_match(s) {
                        return Err(DataSourceError::TransformationFailed(
                            format!("Field '{}' value '{}' does not match pattern '{}'", field_name, s, pattern)
                        ));
                    }
                }
            }
            ValidationRule::Range { min, max } => {
                if let serde_json::Value::Number(n) = value {
                    if let Some(num_val) = n.as_f64() {
                        if num_val < *min || num_val > *max {
                            return Err(DataSourceError::TransformationFailed(
                                format!("Field '{}' value {} is outside range [{}, {}]", field_name, num_val, min, max)
                            ));
                        }
                    }
                }
            }
            ValidationRule::OneOf(allowed_values) => {
                if let serde_json::Value::String(s) = value {
                    if !allowed_values.contains(s) {
                        return Err(DataSourceError::TransformationFailed(
                            format!("Field '{}' value '{}' is not one of allowed values: {:?}", field_name, s, allowed_values)
                        ));
                    }
                }
            }
        }
        Ok(())
    }
}

#[async_trait]
impl TransformationStage for ValueValidationTransform {
    async fn transform(&self, records: Vec<DataRecord>) -> Result<Vec<DataRecord>, DataSourceError> {
        for record in &records {
            for (field_name, rule) in &self.validations {
                if let Some(value) = record.fields.get(field_name) {
                    self.validate_value(field_name, value, rule)?;
                }
            }
        }
        
        Ok(records)
    }

    fn name(&self) -> &str {
        "value_validation"
    }
}