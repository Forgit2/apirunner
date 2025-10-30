pub mod postman;
pub mod openapi;
pub mod exporter;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::error::TestCaseError;
use crate::test_case_manager::TestCase;

#[derive(Debug, Clone, PartialEq)]
pub enum ImportFormat {
    PostmanCollection,
    OpenApiSpec,
    InsomniaCollection,
    SwaggerSpec,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ExportFormat {
    Json,
    Yaml,
    PostmanCollection,
    OpenApiSpec,
}

#[derive(Debug, Clone)]
pub enum ImportSource {
    File(PathBuf),
    Url(String),
    Content(String),
}

#[derive(Debug, Clone)]
pub struct ImportOptions {
    pub generate_assertions: bool,
    pub include_examples: bool,
    pub base_url_override: Option<String>,
    pub tag_prefix: Option<String>,
}

impl Default for ImportOptions {
    fn default() -> Self {
        Self {
            generate_assertions: true,
            include_examples: true,
            base_url_override: None,
            tag_prefix: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportOptions {
    pub include_metadata: bool,
    pub pretty_format: bool,
    pub include_dependencies: bool,
}

impl Default for ExportOptions {
    fn default() -> Self {
        Self {
            include_metadata: true,
            pretty_format: true,
            include_dependencies: false,
        }
    }
}

#[async_trait]
pub trait TestCaseImporter: Send + Sync {
    fn supported_formats(&self) -> Vec<ImportFormat>;
    async fn import(&self, source: &ImportSource, options: &ImportOptions) -> Result<Vec<TestCase>, TestCaseError>;
    fn validate_source(&self, source: &ImportSource) -> Result<(), TestCaseError>;
}

#[async_trait]
pub trait TestCaseExporter: Send + Sync {
    fn supported_formats(&self) -> Vec<ExportFormat>;
    async fn export(&self, test_cases: &[TestCase], format: ExportFormat, options: &ExportOptions) -> Result<String, TestCaseError>;
}

pub struct ImportExportManager {
    importers: Vec<Box<dyn TestCaseImporter>>,
    exporters: Vec<Box<dyn TestCaseExporter>>,
}

impl ImportExportManager {
    pub fn new() -> Self {
        let mut manager = Self {
            importers: Vec::new(),
            exporters: Vec::new(),
        };
        
        // Register built-in importers and exporters
        manager.register_builtin_handlers();
        manager
    }

    fn register_builtin_handlers(&mut self) {
        // Register importers
        self.importers.push(Box::new(postman::PostmanImporter::new()));
        self.importers.push(Box::new(openapi::OpenApiImporter::new()));
        
        // Register exporters
        self.exporters.push(Box::new(exporter::JsonExporter::new()));
        self.exporters.push(Box::new(exporter::YamlExporter::new()));
        self.exporters.push(Box::new(exporter::PostmanExporter::new()));
    }

    pub async fn import_test_cases(
        &self,
        source: &ImportSource,
        format: ImportFormat,
        options: &ImportOptions,
    ) -> Result<Vec<TestCase>, TestCaseError> {
        let importer = self.find_importer(&format)?;
        importer.validate_source(source)?;
        importer.import(source, options).await
    }

    pub async fn export_test_cases(
        &self,
        test_cases: &[TestCase],
        format: ExportFormat,
        options: &ExportOptions,
    ) -> Result<String, TestCaseError> {
        let exporter = self.find_exporter(&format)?;
        exporter.export(test_cases, format, options).await
    }

    pub fn get_supported_import_formats(&self) -> Vec<ImportFormat> {
        let mut formats = Vec::new();
        for importer in &self.importers {
            formats.extend(importer.supported_formats());
        }
        formats
    }

    pub fn get_supported_export_formats(&self) -> Vec<ExportFormat> {
        let mut formats = Vec::new();
        for exporter in &self.exporters {
            formats.extend(exporter.supported_formats());
        }
        formats
    }

    fn find_importer(&self, format: &ImportFormat) -> Result<&dyn TestCaseImporter, TestCaseError> {
        for importer in &self.importers {
            if importer.supported_formats().contains(format) {
                return Ok(importer.as_ref());
            }
        }
        Err(TestCaseError::ImportError(format!("No importer found for format: {:?}", format)))
    }

    fn find_exporter(&self, format: &ExportFormat) -> Result<&dyn TestCaseExporter, TestCaseError> {
        for exporter in &self.exporters {
            if exporter.supported_formats().contains(format) {
                return Ok(exporter.as_ref());
            }
        }
        Err(TestCaseError::ExportError(format!("No exporter found for format: {:?}", format)))
    }
}