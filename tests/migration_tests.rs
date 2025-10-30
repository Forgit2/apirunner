use std::fs;
use std::time::Duration;
use chrono::Utc;
use serde_json;

use apirunner::{
    test_case_manager::{TestCase, ExtractionSource, VariableExtraction},
    error::TestCaseError,
};

#[cfg(test)]
mod migration_tests {
    use super::*;

    #[test]
    fn test_deserialize_old_timeout_format() {
        // Test deserialization of old u64 timeout format
        let json_content = fs::read_to_string("tests/data/migration/old_timeout_format.json")
            .expect("Failed to read old timeout format test file");
        
        let test_case: Result<TestCase, serde_json::Error> = serde_json::from_str(&json_content);
        
        assert!(test_case.is_ok(), "Failed to deserialize test case with old timeout format");
        
        let test_case = test_case.unwrap();
        assert_eq!(test_case.id, "test-old-timeout-format");
        assert_eq!(test_case.name, "Test Case with Old Timeout Format");
        
        // Verify timeout was correctly converted from u64 to Duration
        assert!(test_case.timeout.is_some());
        assert_eq!(test_case.timeout.unwrap(), Duration::from_secs(30));
    }

    #[test]
    fn test_deserialize_old_source_format() {
        // Test deserialization of old string source format
        let json_content = fs::read_to_string("tests/data/migration/old_source_format.json")
            .expect("Failed to read old source format test file");
        
        let test_case: Result<TestCase, serde_json::Error> = serde_json::from_str(&json_content);
        
        assert!(test_case.is_ok(), "Failed to deserialize test case with old source format");
        
        let test_case = test_case.unwrap();
        assert_eq!(test_case.id, "test-old-source-format");
        assert_eq!(test_case.name, "Test Case with Old Source Format");
        
        // Verify variable extractions were correctly converted
        assert!(test_case.variable_extractions.is_some());
        let extractions = test_case.variable_extractions.unwrap();
        assert_eq!(extractions.len(), 3);
        
        // Check first extraction (response -> ResponseBody)
        let user_id_extraction = &extractions[0];
        assert_eq!(user_id_extraction.name, "user_id");
        assert_eq!(user_id_extraction.source, ExtractionSource::ResponseBody);
        assert_eq!(user_id_extraction.path, "$.id");
        
        // Check second extraction (header -> ResponseHeader)
        let auth_token_extraction = &extractions[1];
        assert_eq!(auth_token_extraction.name, "auth_token");
        assert_eq!(auth_token_extraction.source, ExtractionSource::ResponseHeader);
        assert_eq!(auth_token_extraction.path, "Authorization");
        
        // Check third extraction (status -> StatusCode)
        let status_extraction = &extractions[2];
        assert_eq!(status_extraction.name, "status");
        assert_eq!(status_extraction.source, ExtractionSource::StatusCode);
        assert_eq!(status_extraction.path, "");
        assert_eq!(status_extraction.default_value, Some("200".to_string()));
    }

    #[test]
    fn test_deserialize_mixed_old_format() {
        // Test deserialization of mixed old formats (both timeout and source)
        let json_content = fs::read_to_string("tests/data/migration/mixed_old_format.json")
            .expect("Failed to read mixed old format test file");
        
        let test_case: Result<TestCase, serde_json::Error> = serde_json::from_str(&json_content);
        
        assert!(test_case.is_ok(), "Failed to deserialize test case with mixed old formats");
        
        let test_case = test_case.unwrap();
        assert_eq!(test_case.id, "test-mixed-old-format");
        assert_eq!(test_case.name, "Test Case with Mixed Old Formats");
        
        // Verify timeout conversion
        assert!(test_case.timeout.is_some());
        assert_eq!(test_case.timeout.unwrap(), Duration::from_secs(45));
        
        // Verify variable extractions conversion
        assert!(test_case.variable_extractions.is_some());
        let extractions = test_case.variable_extractions.unwrap();
        assert_eq!(extractions.len(), 2);
        
        // Check first extraction (body -> ResponseBody)
        let updated_user_id_extraction = &extractions[0];
        assert_eq!(updated_user_id_extraction.name, "updated_user_id");
        assert_eq!(updated_user_id_extraction.source, ExtractionSource::ResponseBody);
        assert_eq!(updated_user_id_extraction.path, "$.id");
        
        // Check second extraction (header -> ResponseHeader)
        let response_time_extraction = &extractions[1];
        assert_eq!(response_time_extraction.name, "response_time");
        assert_eq!(response_time_extraction.source, ExtractionSource::ResponseHeader);
        assert_eq!(response_time_extraction.path, "X-Response-Time");
        assert_eq!(response_time_extraction.default_value, Some("0ms".to_string()));
        
        // Verify dependencies are preserved
        assert_eq!(test_case.dependencies, vec!["test-old-source-format"]);
    }

    #[test]
    fn test_serialize_new_format_compatibility() {
        // Test that new format can be serialized and deserialized correctly
        let test_case = TestCase {
            id: "test-new-format".to_string(),
            name: "Test Case with New Format".to_string(),
            description: Some("Test case using new Duration and enum formats".to_string()),
            tags: vec!["new".to_string(), "format".to_string()],
            protocol: "HTTP".to_string(),
            request: apirunner::test_case_manager::RequestDefinition {
                protocol: "HTTP".to_string(),
                method: "GET".to_string(),
                url: "https://api.example.com/test".to_string(),
                headers: std::collections::HashMap::new(),
                body: None,
                auth: None,
            },
            assertions: vec![],
            variable_extractions: Some(vec![
                VariableExtraction {
                    name: "test_var".to_string(),
                    source: ExtractionSource::ResponseBody,
                    path: "$.data".to_string(),
                    default_value: None,
                }
            ]),
            dependencies: vec![],
            timeout: Some(Duration::from_secs(60)),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            version: 1,
            change_log: vec![],
        };

        // Serialize to JSON
        let json_result = serde_json::to_string(&test_case);
        assert!(json_result.is_ok(), "Failed to serialize test case with new format");
        
        let json_string = json_result.unwrap();
        
        // Deserialize back
        let deserialized_result: Result<TestCase, serde_json::Error> = serde_json::from_str(&json_string);
        assert!(deserialized_result.is_ok(), "Failed to deserialize test case with new format");
        
        let deserialized_test_case = deserialized_result.unwrap();
        assert_eq!(deserialized_test_case.id, test_case.id);
        assert_eq!(deserialized_test_case.timeout, test_case.timeout);
        assert_eq!(deserialized_test_case.variable_extractions, test_case.variable_extractions);
    }

    #[test]
    fn test_backward_compatibility_source_string_variants() {
        // Test various string representations of source field
        let test_cases = vec![
            ("response", ExtractionSource::ResponseBody),
            ("body", ExtractionSource::ResponseBody),
            ("header", ExtractionSource::ResponseHeader),
            ("status", ExtractionSource::StatusCode),
            ("unknown", ExtractionSource::ResponseBody), // Default fallback
        ];

        for (source_str, expected_source) in test_cases {
            let json_content = format!(r#"{{
                "name": "test_var",
                "source": "{}",
                "path": "$.test",
                "default_value": null
            }}"#, source_str);

            let extraction_result: Result<VariableExtraction, serde_json::Error> = 
                serde_json::from_str(&json_content);
            
            assert!(extraction_result.is_ok(), 
                "Failed to deserialize variable extraction with source: {}", source_str);
            
            let extraction = extraction_result.unwrap();
            assert_eq!(extraction.source, expected_source, 
                "Source mismatch for string: {}", source_str);
        }
    }

    #[test]
    fn test_timeout_edge_cases() {
        // Test edge cases for timeout deserialization
        let test_cases = vec![
            ("null", None),
            ("0", Some(Duration::from_secs(0))),
            ("1", Some(Duration::from_secs(1))),
            ("3600", Some(Duration::from_secs(3600))), // 1 hour
        ];

        for (timeout_json, expected_timeout) in test_cases {
            let json_content = format!(r#"{{
                "id": "test-timeout-edge-case",
                "name": "Test Timeout Edge Case",
                "description": null,
                "tags": [],
                "protocol": "HTTP",
                "request": {{
                    "protocol": "HTTP",
                    "method": "GET",
                    "url": "https://example.com",
                    "headers": {{}},
                    "body": null,
                    "auth": null
                }},
                "assertions": [],
                "variable_extractions": null,
                "dependencies": [],
                "timeout": {},
                "created_at": "2024-01-01T00:00:00Z",
                "updated_at": "2024-01-01T00:00:00Z",
                "version": 1,
                "change_log": []
            }}"#, timeout_json);

            let test_case_result: Result<TestCase, serde_json::Error> = 
                serde_json::from_str(&json_content);
            
            assert!(test_case_result.is_ok(), 
                "Failed to deserialize test case with timeout: {}", timeout_json);
            
            let test_case = test_case_result.unwrap();
            assert_eq!(test_case.timeout, expected_timeout, 
                "Timeout mismatch for JSON: {}", timeout_json);
        }
    }

    #[test]
    fn test_round_trip_serialization() {
        // Test that old format can be loaded and re-saved without data loss
        let json_content = fs::read_to_string("tests/data/migration/old_source_format.json")
            .expect("Failed to read old source format test file");
        
        // Deserialize old format
        let original_test_case: TestCase = serde_json::from_str(&json_content)
            .expect("Failed to deserialize original test case");
        
        // Serialize to new format
        let serialized_json = serde_json::to_string(&original_test_case)
            .expect("Failed to serialize test case");
        
        // Deserialize again
        let round_trip_test_case: TestCase = serde_json::from_str(&serialized_json)
            .expect("Failed to deserialize round-trip test case");
        
        // Verify data integrity
        assert_eq!(original_test_case.id, round_trip_test_case.id);
        assert_eq!(original_test_case.name, round_trip_test_case.name);
        assert_eq!(original_test_case.timeout, round_trip_test_case.timeout);
        assert_eq!(original_test_case.variable_extractions, round_trip_test_case.variable_extractions);
        assert_eq!(original_test_case.dependencies, round_trip_test_case.dependencies);
    }
}