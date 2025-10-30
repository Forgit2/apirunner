use apirunner::data_source::{DataTransformationPipeline, TransformationStage};
use apirunner::plugin::DataRecord;
use apirunner::error::DataSourceError;
use async_trait::async_trait;
use serde_json::{json, Value};
use std::collections::HashMap;

// Mock transformation stage for testing
struct FieldMappingTransformer {
    field_mappings: HashMap<String, String>,
}

impl FieldMappingTransformer {
    fn new(mappings: HashMap<String, String>) -> Self {
        Self {
            field_mappings: mappings,
        }
    }
}

#[async_trait]
impl TransformationStage for FieldMappingTransformer {
    async fn transform(&self, records: Vec<DataRecord>) -> Result<Vec<DataRecord>, DataSourceError> {
        let mut transformed_records = Vec::new();
        
        for record in records {
            let mut transformed_fields = HashMap::new();
            
            for (old_field, new_field) in &self.field_mappings {
                if let Some(value) = record.fields.get(old_field) {
                    transformed_fields.insert(new_field.clone(), value.clone());
                }
            }
            
            // Copy unmapped fields
            for (key, value) in &record.fields {
                if !self.field_mappings.contains_key(key) {
                    transformed_fields.insert(key.clone(), value.clone());
                }
            }
            
            transformed_records.push(DataRecord {
                id: record.id,
                fields: transformed_fields,
                metadata: record.metadata,
            });
        }
        
        Ok(transformed_records)
    }
    
    fn name(&self) -> &str {
        "field_mapping"
    }
}

// Value substitution transformer
struct ValueSubstitutionTransformer {
    substitutions: HashMap<String, Value>,
}

impl ValueSubstitutionTransformer {
    fn new(substitutions: HashMap<String, Value>) -> Self {
        Self { substitutions }
    }
}

#[async_trait]
impl TransformationStage for ValueSubstitutionTransformer {
    async fn transform(&self, records: Vec<DataRecord>) -> Result<Vec<DataRecord>, DataSourceError> {
        let mut transformed_records = Vec::new();
        
        for record in records {
            let mut transformed_fields = HashMap::new();
            
            for (key, value) in &record.fields {
                let new_value = if let Some(substitution) = self.substitutions.get(key) {
                    substitution.clone()
                } else {
                    value.clone()
                };
                transformed_fields.insert(key.clone(), new_value);
            }
            
            transformed_records.push(DataRecord {
                id: record.id,
                fields: transformed_fields,
                metadata: record.metadata,
            });
        }
        
        Ok(transformed_records)
    }
    
    fn name(&self) -> &str {
        "value_substitution"
    }
}

// Type conversion transformer
struct TypeConversionTransformer {
    type_conversions: HashMap<String, String>,
}

impl TypeConversionTransformer {
    fn new(conversions: HashMap<String, String>) -> Self {
        Self {
            type_conversions: conversions,
        }
    }
}

#[async_trait]
impl TransformationStage for TypeConversionTransformer {
    async fn transform(&self, records: Vec<DataRecord>) -> Result<Vec<DataRecord>, DataSourceError> {
        let mut transformed_records = Vec::new();
        
        for record in records {
            let mut transformed_fields = HashMap::new();
            
            for (key, value) in &record.fields {
                let new_value = if let Some(target_type) = self.type_conversions.get(key) {
                    self.convert_value(value.clone(), target_type)?
                } else {
                    value.clone()
                };
                transformed_fields.insert(key.clone(), new_value);
            }
            
            transformed_records.push(DataRecord {
                id: record.id,
                fields: transformed_fields,
                metadata: record.metadata,
            });
        }
        
        Ok(transformed_records)
    }
    
    fn name(&self) -> &str {
        "type_conversion"
    }
}

impl TypeConversionTransformer {
    fn convert_value(&self, value: Value, target_type: &str) -> Result<Value, DataSourceError> {
        match target_type {
            "string" => Ok(Value::String(value.to_string())),
            "number" => {
                if let Value::String(s) = value {
                    s.parse::<f64>()
                        .map(|n| json!(n))
                        .map_err(|_| DataSourceError::InvalidFormat(
                            format!("Cannot convert '{}' to number", s)
                        ))
                } else if value.is_number() {
                    Ok(value)
                } else {
                    Err(DataSourceError::InvalidFormat(
                        format!("Cannot convert {:?} to number", value)
                    ))
                }
            }
            "boolean" => {
                match value {
                    Value::Bool(b) => Ok(Value::Bool(b)),
                    Value::String(s) => {
                        match s.to_lowercase().as_str() {
                            "true" | "1" | "yes" => Ok(Value::Bool(true)),
                            "false" | "0" | "no" => Ok(Value::Bool(false)),
                            _ => Err(DataSourceError::InvalidFormat(
                                format!("Cannot convert '{}' to boolean", s)
                            ))
                        }
                    }
                    _ => Err(DataSourceError::InvalidFormat(
                        format!("Cannot convert {:?} to boolean", value)
                    ))
                }
            }
            _ => Err(DataSourceError::InvalidFormat(format!("Unsupported type: {}", target_type)))
        }
    }
}

#[tokio::test]
async fn test_data_transformation_pipeline_creation() {
    let pipeline = DataTransformationPipeline::new();
    
    // Test that we can create an empty pipeline
    let test_records = vec![
        DataRecord {
            id: "record_1".to_string(),
            fields: HashMap::from([
                ("name".to_string(), json!("John")),
                ("age".to_string(), json!(30)),
            ]),
            metadata: HashMap::new(),
        }
    ];
    
    let result = pipeline.transform(test_records.clone()).await;
    assert!(result.is_ok());
    
    let transformed = result.unwrap();
    assert_eq!(transformed.len(), 1);
    assert_eq!(transformed[0].fields.get("name"), Some(&json!("John")));
}

#[tokio::test]
async fn test_field_mapping_transformation() {
    let mappings = HashMap::from([
        ("old_name".to_string(), "new_name".to_string()),
        ("old_age".to_string(), "new_age".to_string()),
    ]);
    
    let transformer = FieldMappingTransformer::new(mappings);
    
    let test_records = vec![
        DataRecord {
            id: "record_1".to_string(),
            fields: HashMap::from([
                ("old_name".to_string(), json!("Alice")),
                ("old_age".to_string(), json!(25)),
                ("unchanged".to_string(), json!("value")),
            ]),
            metadata: HashMap::new(),
        }
    ];
    
    let result = transformer.transform(test_records).await;
    assert!(result.is_ok());
    
    let transformed = result.unwrap();
    assert_eq!(transformed.len(), 1);
    assert_eq!(transformed[0].fields.get("new_name"), Some(&json!("Alice")));
    assert_eq!(transformed[0].fields.get("new_age"), Some(&json!(25)));
    assert_eq!(transformed[0].fields.get("unchanged"), Some(&json!("value")));
    assert!(transformed[0].fields.get("old_name").is_none());
}

#[tokio::test]
async fn test_value_substitution_transformation() {
    let substitutions = HashMap::from([
        ("status".to_string(), json!("active")),
        ("priority".to_string(), json!(1)),
    ]);
    
    let transformer = ValueSubstitutionTransformer::new(substitutions);
    
    let test_records = vec![
        DataRecord {
            id: "record_1".to_string(),
            fields: HashMap::from([
                ("name".to_string(), json!("Bob")),
                ("status".to_string(), json!("inactive")),
                ("priority".to_string(), json!(3)),
            ]),
            metadata: HashMap::new(),
        }
    ];
    
    let result = transformer.transform(test_records).await;
    assert!(result.is_ok());
    
    let transformed = result.unwrap();
    assert_eq!(transformed.len(), 1);
    assert_eq!(transformed[0].fields.get("name"), Some(&json!("Bob")));
    assert_eq!(transformed[0].fields.get("status"), Some(&json!("active")));
    assert_eq!(transformed[0].fields.get("priority"), Some(&json!(1)));
}

#[tokio::test]
async fn test_type_conversion_transformation() {
    let conversions = HashMap::from([
        ("age".to_string(), "string".to_string()),
        ("score".to_string(), "number".to_string()),
        ("active".to_string(), "boolean".to_string()),
    ]);
    
    let transformer = TypeConversionTransformer::new(conversions);
    
    let test_records = vec![
        DataRecord {
            id: "record_1".to_string(),
            fields: HashMap::from([
                ("age".to_string(), json!(30)),
                ("score".to_string(), json!("95.5")),
                ("active".to_string(), json!("true")),
                ("name".to_string(), json!("Charlie")),
            ]),
            metadata: HashMap::new(),
        }
    ];
    
    let result = transformer.transform(test_records).await;
    assert!(result.is_ok());
    
    let transformed = result.unwrap();
    assert_eq!(transformed.len(), 1);
    assert_eq!(transformed[0].fields.get("age"), Some(&json!("30")));
    assert_eq!(transformed[0].fields.get("score"), Some(&json!(95.5)));
    assert_eq!(transformed[0].fields.get("active"), Some(&json!(true)));
    assert_eq!(transformed[0].fields.get("name"), Some(&json!("Charlie")));
}

#[tokio::test]
async fn test_pipeline_with_multiple_transformations() {
    // Add field mapping
    let mappings = HashMap::from([
        ("old_name".to_string(), "name".to_string()),
    ]);
    
    // Add value substitution
    let substitutions = HashMap::from([
        ("status".to_string(), json!("processed")),
    ]);
    
    let pipeline = DataTransformationPipeline::new()
        .add_stage(Box::new(FieldMappingTransformer::new(mappings)))
        .add_stage(Box::new(ValueSubstitutionTransformer::new(substitutions)));
    
    let test_records = vec![
        DataRecord {
            id: "record_1".to_string(),
            fields: HashMap::from([
                ("old_name".to_string(), json!("David")),
                ("status".to_string(), json!("pending")),
            ]),
            metadata: HashMap::new(),
        }
    ];
    
    let result = pipeline.transform(test_records).await;
    assert!(result.is_ok());
    
    let transformed = result.unwrap();
    assert_eq!(transformed.len(), 1);
    assert_eq!(transformed[0].fields.get("name"), Some(&json!("David")));
    assert_eq!(transformed[0].fields.get("status"), Some(&json!("processed")));
    assert!(transformed[0].fields.get("old_name").is_none());
}

#[tokio::test]
async fn test_transformation_error_handling() {
    let conversions = HashMap::from([
        ("invalid_number".to_string(), "number".to_string()),
    ]);
    
    let transformer = TypeConversionTransformer::new(conversions);
    
    let test_records = vec![
        DataRecord {
            id: "record_1".to_string(),
            fields: HashMap::from([
                ("invalid_number".to_string(), json!("not_a_number")),
            ]),
            metadata: HashMap::new(),
        }
    ];
    
    let result = transformer.transform(test_records).await;
    assert!(result.is_err());
    
    if let Err(DataSourceError::InvalidFormat(msg)) = result {
        assert!(msg.contains("Cannot convert"));
    } else {
        panic!("Expected InvalidFormat error");
    }
}