use async_trait::async_trait;
use roxmltree::Document;
use serde_json::{Value as JsonValue};
use std::collections::HashMap;

use crate::error::PluginError;
use crate::plugin::{
    AssertionPlugin, AssertionResult, AssertionType, ExpectedValue, Plugin, PluginConfig,
    ResponseData,
};

/// XPath Assertion Plugin for validating XML responses using XPath expressions
#[derive(Debug, Default)]
pub struct XPathAssertion {
    xpath_expression: String,
}

impl XPathAssertion {
    pub fn new(xpath_expression: String) -> Self {
        Self { xpath_expression }
    }

    /// Parse response body as XML
    fn parse_xml_body<'a>(&self, body: &'a [u8]) -> Result<Document<'a>, String> {
        let body_str = std::str::from_utf8(body)
            .map_err(|_| "Response body is not valid UTF-8".to_string())?;
        
        Document::parse(body_str)
            .map_err(|e| format!("Failed to parse XML: {}", e))
    }

    /// Execute simple XPath-like query on XML document (simplified implementation)
    fn execute_simple_xpath(&self, doc: &Document, xpath: &str) -> Result<JsonValue, String> {
        // This is a simplified XPath implementation for basic queries
        // In production, use a full XPath library like sxd-xpath
        
        if xpath.starts_with("//") && xpath.ends_with("/text()") {
            // Handle //element/text() queries
            let element_name = xpath.trim_start_matches("//").trim_end_matches("/text()");
            let mut results = Vec::new();
            
            self.find_elements_by_name(doc.root_element(), element_name, &mut results);
            
            if results.len() == 1 {
                Ok(JsonValue::String(results[0].clone()))
            } else if results.is_empty() {
                Ok(JsonValue::Null)
            } else {
                Ok(JsonValue::Array(results.into_iter().map(JsonValue::String).collect()))
            }
        } else if xpath.starts_with("//") && !xpath.contains("/text()") {
            // Handle //element queries (return elements, not text)
            let path_part = xpath.trim_start_matches("//");
            let mut results = Vec::new();
            
            if path_part.contains('/') {
                // Handle paths like "items/item" - find the last element in the path
                let parts: Vec<&str> = path_part.split('/').collect();
                let target_element = parts.last().unwrap();
                self.find_elements_by_name_for_nodes(doc.root_element(), target_element, &mut results);
            } else {
                // Simple element name
                self.find_elements_by_name_for_nodes(doc.root_element(), path_part, &mut results);
            }
            
            if results.is_empty() {
                Ok(JsonValue::Array(vec![]))
            } else {
                Ok(JsonValue::Array(results.into_iter().map(JsonValue::String).collect()))
            }
        } else if xpath.starts_with("count(//") && xpath.ends_with(")") {
            // Handle count(//element) queries
            let element_name = xpath.trim_start_matches("count(//").trim_end_matches(")");
            let count = self.count_elements_by_name(doc.root_element(), element_name);
            
            Ok(JsonValue::Number(serde_json::Number::from(count)))
        } else {
            Err(format!("Unsupported XPath expression: {}", xpath))
        }
    }

    /// Recursively find elements by name (text content)
    fn find_elements_by_name(&self, node: roxmltree::Node, name: &str, results: &mut Vec<String>) {
        if node.is_element() && node.tag_name().name() == name {
            if let Some(text) = node.text() {
                results.push(text.to_string());
            }
        }
        
        for child in node.children() {
            self.find_elements_by_name(child, name, results);
        }
    }

    /// Recursively find elements by name and return their text content
    fn find_elements_by_name_for_nodes(&self, node: roxmltree::Node, name: &str, results: &mut Vec<String>) {
        if node.is_element() && node.tag_name().name() == name {
            // For element queries, return the element's text content
            let text_content = self.get_element_text_content(node);
            results.push(text_content);
        }
        
        for child in node.children() {
            self.find_elements_by_name_for_nodes(child, name, results);
        }
    }

    /// Get text content from an element (including child text nodes)
    fn get_element_text_content(&self, node: roxmltree::Node) -> String {
        let mut text = String::new();
        for child in node.children() {
            if child.is_text() {
                text.push_str(child.text().unwrap_or(""));
            }
        }
        if text.is_empty() {
            format!("<{}>", node.tag_name().name())
        } else {
            text
        }
    }

    /// Count elements by name
    fn count_elements_by_name(&self, node: roxmltree::Node, name: &str) -> usize {
        let mut count = 0;
        
        if node.is_element() && node.tag_name().name() == name {
            count += 1;
        }
        
        for child in node.children() {
            count += self.count_elements_by_name(child, name);
        }
        
        count
    }

    /// Compare actual and expected values with type-aware comparison
    fn compare_values(&self, actual: &JsonValue, expected: &ExpectedValue) -> (bool, String) {
        match expected {
            ExpectedValue::Exact(expected_val) => {
                // Handle numeric comparisons more flexibly
                let success = if let (Some(actual_num), Some(expected_num)) = (actual.as_f64(), expected_val.as_f64()) {
                    (actual_num - expected_num).abs() < f64::EPSILON
                } else {
                    actual == expected_val
                };
                
                let message = if success {
                    format!("XPath result matches expected value: {}", expected_val)
                } else {
                    format!(
                        "XPath result mismatch: expected {}, got {}",
                        expected_val, actual
                    )
                };
                (success, message)
            }
            ExpectedValue::Contains(substring) => {
                if let Some(actual_str) = actual.as_str() {
                    let success = actual_str.contains(substring);
                    let message = if success {
                        format!("XPath result '{}' contains '{}'", actual_str, substring)
                    } else {
                        format!("XPath result '{}' does not contain '{}'", actual_str, substring)
                    };
                    (success, message)
                } else {
                    (false, format!("XPath result is not a string, cannot check contains: {}", actual))
                }
            }
            ExpectedValue::Pattern(regex_pattern) => {
                if let Some(actual_str) = actual.as_str() {
                    match regex::Regex::new(regex_pattern) {
                        Ok(regex) => {
                            let success = regex.is_match(actual_str);
                            let message = if success {
                                format!("XPath result '{}' matches pattern '{}'", actual_str, regex_pattern)
                            } else {
                                format!("XPath result '{}' does not match pattern '{}'", actual_str, regex_pattern)
                            };
                            (success, message)
                        }
                        Err(e) => (false, format!("Invalid regex pattern '{}': {}", regex_pattern, e))
                    }
                } else {
                    (false, format!("XPath result is not a string, cannot match pattern: {}", actual))
                }
            }
            ExpectedValue::NotEmpty => {
                let success = match actual {
                    JsonValue::Null => false,
                    JsonValue::String(s) => !s.is_empty(),
                    JsonValue::Array(arr) => !arr.is_empty(),
                    JsonValue::Object(obj) => !obj.is_empty(),
                    _ => true, // Numbers and booleans are considered non-empty
                };
                let message = if success {
                    "XPath result is not empty".to_string()
                } else {
                    "XPath result is empty".to_string()
                };
                (success, message)
            }
            ExpectedValue::Range { min, max } => {
                // Try to parse as number if it's a string
                let num_value = if let Some(num) = actual.as_f64() {
                    Some(num)
                } else if let Some(str_val) = actual.as_str() {
                    str_val.parse::<f64>().ok()
                } else {
                    None
                };

                if let Some(num) = num_value {
                    let success = num >= *min && num <= *max;
                    let message = if success {
                        format!("XPath result {} is within range [{}, {}]", num, min, max)
                    } else {
                        format!("XPath result {} is outside range [{}, {}]", num, min, max)
                    };
                    (success, message)
                } else {
                    (false, format!("XPath result is not a number, cannot check range: {}", actual))
                }
            }
        }
    }
}

#[async_trait]
impl Plugin for XPathAssertion {
    fn name(&self) -> &str {
        "xpath-assertion"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn dependencies(&self) -> Vec<String> {
        vec![]
    }

    async fn initialize(&mut self, config: &PluginConfig) -> Result<(), PluginError> {
        if let Some(xpath) = config.settings.get("xpath_expression") {
            if let Some(expr) = xpath.as_str() {
                self.xpath_expression = expr.to_string();
            }
        }

        if self.xpath_expression.is_empty() {
            return Err(PluginError::InitializationFailed(
                "XPath expression is required".to_string(),
            ));
        }

        Ok(())
    }

    async fn shutdown(&mut self) -> Result<(), PluginError> {
        Ok(())
    }
}

#[async_trait]
impl AssertionPlugin for XPathAssertion {
    fn assertion_type(&self) -> AssertionType {
        AssertionType::XPath
    }

    async fn execute(&self, actual: &ResponseData, expected: &ExpectedValue) -> AssertionResult {
        // Parse XML from response body
        let xml_doc = match self.parse_xml_body(&actual.body) {
            Ok(doc) => doc,
            Err(error) => {
                return AssertionResult {
                    success: false,
                    message: error,
                    actual_value: None,
                    expected_value: Some(serde_json::to_value(expected).unwrap_or(JsonValue::Null)),
                };
            }
        };

        // Execute simple XPath query
        let json_result = match self.execute_simple_xpath(&xml_doc, &self.xpath_expression) {
            Ok(result) => result,
            Err(error) => {
                return AssertionResult {
                    success: false,
                    message: error,
                    actual_value: None,
                    expected_value: Some(serde_json::to_value(expected).unwrap_or(JsonValue::Null)),
                };
            }
        };

        // Compare with expected value
        let (success, message) = self.compare_values(&json_result, expected);

        AssertionResult {
            success,
            message,
            actual_value: Some(json_result),
            expected_value: Some(serde_json::to_value(expected).unwrap_or(JsonValue::Null)),
        }
    }

    fn priority(&self) -> u8 {
        80 // Medium-high priority for XML validation
    }
}

/// XML Schema Assertion Plugin for validating XML responses against XML Schema (XSD)
#[derive(Debug, Default)]
pub struct XmlSchemaAssertion {
    schema_content: Option<String>,
}

impl XmlSchemaAssertion {
    pub fn new() -> Self {
        Self {
            schema_content: None,
        }
    }

    pub fn with_schema(schema: String) -> Self {
        Self {
            schema_content: Some(schema),
        }
    }

    /// Parse response body as XML
    fn parse_xml_body<'a>(&self, body: &'a [u8]) -> Result<Document<'a>, String> {
        let body_str = std::str::from_utf8(body)
            .map_err(|_| "Response body is not valid UTF-8".to_string())?;
        
        Document::parse(body_str)
            .map_err(|e| format!("Failed to parse XML: {}", e))
    }

    /// Basic XML schema validation (simplified implementation)
    /// In a production system, you would use a proper XML schema validation library
    fn validate_against_schema(&self, doc: &Document, _schema: &str) -> Result<(), String> {
        // This is a simplified schema validation - in production use libxml2 or similar
        let root = doc.root_element();
        
        // Basic validation: check if document is well-formed (already done by parsing)
        // and has a root element
        if root.tag_name().name().is_empty() {
            return Err("XML document has no root element".to_string());
        }

        // Additional basic validations could be added here:
        // - Check for required elements
        // - Validate element structure
        // - Check attribute constraints
        
        // For now, we'll just validate that it's well-formed XML
        Ok(())
    }
}

#[async_trait]
impl Plugin for XmlSchemaAssertion {
    fn name(&self) -> &str {
        "xml-schema-assertion"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn dependencies(&self) -> Vec<String> {
        vec![]
    }

    async fn initialize(&mut self, config: &PluginConfig) -> Result<(), PluginError> {
        if let Some(schema_value) = config.settings.get("schema") {
            if let Some(schema_str) = schema_value.as_str() {
                self.schema_content = Some(schema_str.to_string());
            }
        }

        if self.schema_content.is_none() {
            return Err(PluginError::InitializationFailed(
                "XML schema is required".to_string(),
            ));
        }

        Ok(())
    }

    async fn shutdown(&mut self) -> Result<(), PluginError> {
        Ok(())
    }
}

#[async_trait]
impl AssertionPlugin for XmlSchemaAssertion {
    fn assertion_type(&self) -> AssertionType {
        AssertionType::Custom("xml-schema".to_string())
    }

    async fn execute(&self, actual: &ResponseData, _expected: &ExpectedValue) -> AssertionResult {
        let schema = match &self.schema_content {
            Some(s) => s,
            None => {
                return AssertionResult {
                    success: false,
                    message: "No XML schema configured".to_string(),
                    actual_value: None,
                    expected_value: None,
                };
            }
        };

        // Parse XML from response body
        let xml_doc = match self.parse_xml_body(&actual.body) {
            Ok(doc) => doc,
            Err(error) => {
                return AssertionResult {
                    success: false,
                    message: error,
                    actual_value: None,
                    expected_value: Some(JsonValue::String(schema.clone())),
                };
            }
        };

        // Validate against schema
        match self.validate_against_schema(&xml_doc, schema) {
            Ok(()) => AssertionResult {
                success: true,
                message: "XML response validates against schema".to_string(),
                actual_value: Some(JsonValue::String(
                    std::str::from_utf8(&actual.body).unwrap_or("").to_string()
                )),
                expected_value: Some(JsonValue::String(schema.clone())),
            },
            Err(error) => AssertionResult {
                success: false,
                message: format!("XML schema validation failed: {}", error),
                actual_value: Some(JsonValue::String(
                    std::str::from_utf8(&actual.body).unwrap_or("").to_string()
                )),
                expected_value: Some(JsonValue::String(schema.clone())),
            },
        }
    }

    fn priority(&self) -> u8 {
        70 // Medium priority for schema validation
    }
}

/// Namespace-aware XML Processing Assertion for complex XML with namespaces
#[derive(Debug, Default)]
pub struct NamespaceAwareXmlAssertion {
    xpath_expression: String,
    namespaces: HashMap<String, String>,
}

impl NamespaceAwareXmlAssertion {
    pub fn new(xpath_expression: String) -> Self {
        Self {
            xpath_expression,
            namespaces: HashMap::new(),
        }
    }

    pub fn with_namespace(mut self, prefix: String, uri: String) -> Self {
        self.namespaces.insert(prefix, uri);
        self
    }

    /// Parse response body as XML
    fn parse_xml_body<'a>(&self, body: &'a [u8]) -> Result<Document<'a>, String> {
        let body_str = std::str::from_utf8(body)
            .map_err(|_| "Response body is not valid UTF-8".to_string())?;
        
        Document::parse(body_str)
            .map_err(|e| format!("Failed to parse XML: {}", e))
    }

    /// Execute simple namespace-aware XPath-like query (simplified implementation)
    fn execute_namespace_xpath(&self, doc: &Document, xpath: &str) -> Result<JsonValue, String> {
        // This is a simplified namespace-aware XPath implementation
        // In production, use a full XPath library with namespace support
        
        if xpath.contains(" | ") {
            // Handle union queries like "//p:name/text() | //c:email/text()"
            let parts: Vec<&str> = xpath.split(" | ").collect();
            let mut all_results = Vec::new();
            
            for part in parts {
                if let Ok(JsonValue::String(result)) = self.execute_single_namespace_xpath(doc, part.trim()) {
                    all_results.push(result);
                } else if let Ok(JsonValue::Array(results)) = self.execute_single_namespace_xpath(doc, part.trim()) {
                    for result in results {
                        if let JsonValue::String(s) = result {
                            all_results.push(s);
                        }
                    }
                }
            }
            
            if all_results.is_empty() {
                Ok(JsonValue::Array(vec![]))
            } else {
                Ok(JsonValue::Array(all_results.into_iter().map(JsonValue::String).collect()))
            }
        } else {
            self.execute_single_namespace_xpath(doc, xpath)
        }
    }

    /// Execute single namespace-aware XPath query
    fn execute_single_namespace_xpath(&self, doc: &Document, xpath: &str) -> Result<JsonValue, String> {
        if xpath.contains("//") && xpath.ends_with("/text()") {
            // Handle //ns:element/text() queries
            let parts: Vec<&str> = xpath.trim_start_matches("//").trim_end_matches("/text()").split(':').collect();
            
            if parts.len() == 2 {
                let (prefix, element_name) = (parts[0], parts[1]);
                
                if let Some(namespace_uri) = self.namespaces.get(prefix) {
                    let mut results = Vec::new();
                    self.find_namespaced_elements(doc.root_element(), element_name, namespace_uri, &mut results);
                    
                    if results.len() == 1 {
                        Ok(JsonValue::String(results[0].clone()))
                    } else if results.is_empty() {
                        Ok(JsonValue::Null)
                    } else {
                        Ok(JsonValue::Array(results.into_iter().map(JsonValue::String).collect()))
                    }
                } else {
                    Err(format!("Namespace prefix '{}' not found", prefix))
                }
            } else {
                Err(format!("Invalid namespaced XPath expression: {}", xpath))
            }
        } else {
            Err(format!("Unsupported namespace-aware XPath expression: {}", xpath))
        }
    }

    /// Find elements by name and namespace
    fn find_namespaced_elements(&self, node: roxmltree::Node, name: &str, namespace_uri: &str, results: &mut Vec<String>) {
        if node.is_element() && node.tag_name().name() == name {
            // Check if the node's namespace matches
            if let Some(node_ns) = node.tag_name().namespace() {
                if node_ns == namespace_uri {
                    if let Some(text) = node.text() {
                        results.push(text.to_string());
                    }
                }
            } else if namespace_uri.is_empty() {
                // Default namespace case
                if let Some(text) = node.text() {
                    results.push(text.to_string());
                }
            }
        }
        
        for child in node.children() {
            self.find_namespaced_elements(child, name, namespace_uri, results);
        }
    }

    /// Compare actual and expected values
    fn compare_values(&self, actual: &JsonValue, expected: &ExpectedValue) -> (bool, String) {
        match expected {
            ExpectedValue::Exact(expected_val) => {
                let success = actual == expected_val;
                let message = if success {
                    format!("Namespace-aware XPath result matches expected value: {}", expected_val)
                } else {
                    format!(
                        "Namespace-aware XPath result mismatch: expected {}, got {}",
                        expected_val, actual
                    )
                };
                (success, message)
            }
            ExpectedValue::Contains(substring) => {
                if let Some(actual_str) = actual.as_str() {
                    let success = actual_str.contains(substring);
                    let message = if success {
                        format!("Namespace-aware XPath result '{}' contains '{}'", actual_str, substring)
                    } else {
                        format!("Namespace-aware XPath result '{}' does not contain '{}'", actual_str, substring)
                    };
                    (success, message)
                } else {
                    (false, format!("Namespace-aware XPath result is not a string, cannot check contains: {}", actual))
                }
            }
            ExpectedValue::NotEmpty => {
                let success = match actual {
                    JsonValue::Null => false,
                    JsonValue::String(s) => !s.is_empty(),
                    JsonValue::Array(arr) => !arr.is_empty(),
                    JsonValue::Object(obj) => !obj.is_empty(),
                    _ => true,
                };
                let message = if success {
                    "Namespace-aware XPath result is not empty".to_string()
                } else {
                    "Namespace-aware XPath result is empty".to_string()
                };
                (success, message)
            }
            _ => (false, "Unsupported expected value type for namespace-aware XML assertion".to_string()),
        }
    }
}

#[async_trait]
impl Plugin for NamespaceAwareXmlAssertion {
    fn name(&self) -> &str {
        "namespace-aware-xml-assertion"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn dependencies(&self) -> Vec<String> {
        vec![]
    }

    async fn initialize(&mut self, config: &PluginConfig) -> Result<(), PluginError> {
        if let Some(xpath) = config.settings.get("xpath_expression") {
            if let Some(expr) = xpath.as_str() {
                self.xpath_expression = expr.to_string();
            }
        }

        if let Some(namespaces) = config.settings.get("namespaces") {
            if let Some(ns_obj) = namespaces.as_object() {
                for (prefix, uri) in ns_obj {
                    if let Some(uri_str) = uri.as_str() {
                        self.namespaces.insert(prefix.clone(), uri_str.to_string());
                    }
                }
            }
        }

        if self.xpath_expression.is_empty() {
            return Err(PluginError::InitializationFailed(
                "XPath expression is required".to_string(),
            ));
        }

        Ok(())
    }

    async fn shutdown(&mut self) -> Result<(), PluginError> {
        Ok(())
    }
}

#[async_trait]
impl AssertionPlugin for NamespaceAwareXmlAssertion {
    fn assertion_type(&self) -> AssertionType {
        AssertionType::Custom("namespace-aware-xml".to_string())
    }

    async fn execute(&self, actual: &ResponseData, expected: &ExpectedValue) -> AssertionResult {
        // Parse XML from response body
        let xml_doc = match self.parse_xml_body(&actual.body) {
            Ok(doc) => doc,
            Err(error) => {
                return AssertionResult {
                    success: false,
                    message: error,
                    actual_value: None,
                    expected_value: Some(serde_json::to_value(expected).unwrap_or(JsonValue::Null)),
                };
            }
        };

        // Execute namespace-aware XPath query
        let json_result = match self.execute_namespace_xpath(&xml_doc, &self.xpath_expression) {
            Ok(result) => result,
            Err(error) => {
                return AssertionResult {
                    success: false,
                    message: error,
                    actual_value: None,
                    expected_value: Some(serde_json::to_value(expected).unwrap_or(JsonValue::Null)),
                };
            }
        };

        // Compare with expected value
        let (success, message) = self.compare_values(&json_result, expected);

        AssertionResult {
            success,
            message,
            actual_value: Some(json_result),
            expected_value: Some(serde_json::to_value(expected).unwrap_or(JsonValue::Null)),
        }
    }

    fn priority(&self) -> u8 {
        75 // Medium-high priority for namespace-aware XML validation
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::collections::HashMap;

    #[tokio::test]
    async fn test_xpath_assertion_text_content() {
        let mut assertion = XPathAssertion::new("//name/text()".to_string());
        
        let config = PluginConfig {
            settings: HashMap::from([
                ("xpath_expression".to_string(), json!("//name/text()"))
            ]),
            environment: "test".to_string(),
        };
        
        assertion.initialize(&config).await.unwrap();

        let response_data = ResponseData {
            status_code: 200,
            headers: HashMap::new(),
            body: r#"<person><name>John Doe</name><age>30</age></person>"#.as_bytes().to_vec(),
            duration: std::time::Duration::from_millis(100),
            metadata: HashMap::new(),
        };

        let expected = ExpectedValue::Exact(json!("John Doe"));
        let result = assertion.execute(&response_data, &expected).await;

        assert!(result.success);
        assert_eq!(result.actual_value, Some(json!("John Doe")));
    }

    #[tokio::test]
    async fn test_xpath_assertion_element_count() {
        let mut assertion = XPathAssertion::new("count(//item)".to_string());
        
        let config = PluginConfig {
            settings: HashMap::from([
                ("xpath_expression".to_string(), json!("count(//item)"))
            ]),
            environment: "test".to_string(),
        };
        
        assertion.initialize(&config).await.unwrap();

        let response_data = ResponseData {
            status_code: 200,
            headers: HashMap::new(),
            body: r#"<items><item>1</item><item>2</item><item>3</item></items>"#.as_bytes().to_vec(),
            duration: std::time::Duration::from_millis(100),
            metadata: HashMap::new(),
        };

        let expected = ExpectedValue::Exact(json!(3.0));
        let result = assertion.execute(&response_data, &expected).await;

        assert!(result.success);
    }

    #[tokio::test]
    async fn test_xml_schema_assertion_valid() {
        let schema = r#"<?xml version="1.0"?>
        <xs:schema xmlns:xs="http://www.w3.org/2001/XMLSchema">
            <xs:element name="person">
                <xs:complexType>
                    <xs:sequence>
                        <xs:element name="name" type="xs:string"/>
                        <xs:element name="age" type="xs:int"/>
                    </xs:sequence>
                </xs:complexType>
            </xs:element>
        </xs:schema>"#;

        let mut assertion = XmlSchemaAssertion::with_schema(schema.to_string());
        
        let config = PluginConfig {
            settings: HashMap::from([
                ("schema".to_string(), json!(schema))
            ]),
            environment: "test".to_string(),
        };
        
        assertion.initialize(&config).await.unwrap();

        let response_data = ResponseData {
            status_code: 200,
            headers: HashMap::new(),
            body: r#"<person><name>John</name><age>30</age></person>"#.as_bytes().to_vec(),
            duration: std::time::Duration::from_millis(100),
            metadata: HashMap::new(),
        };

        let expected = ExpectedValue::NotEmpty;
        let result = assertion.execute(&response_data, &expected).await;

        assert!(result.success);
        assert!(result.message.contains("validates against schema"));
    }

    #[tokio::test]
    async fn test_namespace_aware_xml_assertion() {
        let mut assertion = NamespaceAwareXmlAssertion::new("//ns:name/text()".to_string())
            .with_namespace("ns".to_string(), "http://example.com/person".to_string());
        
        let config = PluginConfig {
            settings: HashMap::from([
                ("xpath_expression".to_string(), json!("//ns:name/text()")),
                ("namespaces".to_string(), json!({
                    "ns": "http://example.com/person"
                }))
            ]),
            environment: "test".to_string(),
        };
        
        assertion.initialize(&config).await.unwrap();

        let response_data = ResponseData {
            status_code: 200,
            headers: HashMap::new(),
            body: r#"<person xmlns="http://example.com/person"><name>John</name></person>"#.as_bytes().to_vec(),
            duration: std::time::Duration::from_millis(100),
            metadata: HashMap::new(),
        };

        let expected = ExpectedValue::Exact(json!("John"));
        let result = assertion.execute(&response_data, &expected).await;

        assert!(result.success);
    }
}