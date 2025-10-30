use apirunner::xml_assertions::{XPathAssertion, XmlSchemaAssertion, NamespaceAwareXmlAssertion};
use apirunner::{
    Plugin, PluginConfig, AssertionPlugin, AssertionType, ExpectedValue, ResponseData,
};
use serde_json::json;
use std::collections::HashMap;
use std::time::Duration;

#[tokio::test]
async fn test_xpath_assertion_initialization() {
    let mut assertion = XPathAssertion::new("//name/text()".to_string());
    
    let config = PluginConfig {
        settings: HashMap::from([
            ("xpath_expression".to_string(), json!("//name/text()"))
        ]),
        environment: "test".to_string(),
    };
    
    let result = assertion.initialize(&config).await;
    assert!(result.is_ok());
    assert_eq!(assertion.assertion_type(), AssertionType::XPath);
    assert_eq!(assertion.priority(), 80);
}

#[tokio::test]
async fn test_xpath_assertion_text_content_success() {
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
        duration: Duration::from_millis(100),
        metadata: HashMap::new(),
    };

    let expected = ExpectedValue::Exact(json!("John Doe"));
    let result = assertion.execute(&response_data, &expected).await;

    assert!(result.success);
    assert!(result.message.contains("matches expected value"));
    assert_eq!(result.actual_value, Some(json!("John Doe")));
}

#[tokio::test]
async fn test_xpath_assertion_text_content_failure() {
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
        body: r#"<person><name>Jane Doe</name><age>25</age></person>"#.as_bytes().to_vec(),
        duration: Duration::from_millis(100),
        metadata: HashMap::new(),
    };

    let expected = ExpectedValue::Exact(json!("John Doe"));
    let result = assertion.execute(&response_data, &expected).await;

    assert!(!result.success);
    assert!(result.message.contains("mismatch"));
    assert_eq!(result.actual_value, Some(json!("Jane Doe")));
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
        duration: Duration::from_millis(100),
        metadata: HashMap::new(),
    };

    let expected = ExpectedValue::Exact(json!(3.0));
    let result = assertion.execute(&response_data, &expected).await;

    assert!(result.success);
}

#[tokio::test]
async fn test_xpath_assertion_contains() {
    let mut assertion = XPathAssertion::new("//description/text()".to_string());
    
    let config = PluginConfig {
        settings: HashMap::from([
            ("xpath_expression".to_string(), json!("//description/text()"))
        ]),
        environment: "test".to_string(),
    };
    
    assertion.initialize(&config).await.unwrap();

    let response_data = ResponseData {
        status_code: 200,
        headers: HashMap::new(),
        body: r#"<product><description>High quality product with warranty</description></product>"#.as_bytes().to_vec(),
        duration: Duration::from_millis(100),
        metadata: HashMap::new(),
    };

    let expected = ExpectedValue::Contains("quality".to_string());
    let result = assertion.execute(&response_data, &expected).await;

    assert!(result.success);
    assert!(result.message.contains("contains"));
}

#[tokio::test]
async fn test_xpath_assertion_pattern_match() {
    let mut assertion = XPathAssertion::new("//email/text()".to_string());
    
    let config = PluginConfig {
        settings: HashMap::from([
            ("xpath_expression".to_string(), json!("//email/text()"))
        ]),
        environment: "test".to_string(),
    };
    
    assertion.initialize(&config).await.unwrap();

    let response_data = ResponseData {
        status_code: 200,
        headers: HashMap::new(),
        body: r#"<contact><email>user@example.com</email></contact>"#.as_bytes().to_vec(),
        duration: Duration::from_millis(100),
        metadata: HashMap::new(),
    };

    let expected = ExpectedValue::Pattern(r"^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$".to_string());
    let result = assertion.execute(&response_data, &expected).await;

    assert!(result.success);
    assert!(result.message.contains("matches pattern"));
}

#[tokio::test]
async fn test_xpath_assertion_not_empty() {
    let mut assertion = XPathAssertion::new("//items/item".to_string());
    
    let config = PluginConfig {
        settings: HashMap::from([
            ("xpath_expression".to_string(), json!("//items/item"))
        ]),
        environment: "test".to_string(),
    };
    
    assertion.initialize(&config).await.unwrap();

    let response_data = ResponseData {
        status_code: 200,
        headers: HashMap::new(),
        body: r#"<items><item>1</item><item>2</item></items>"#.as_bytes().to_vec(),
        duration: Duration::from_millis(100),
        metadata: HashMap::new(),
    };

    let expected = ExpectedValue::NotEmpty;
    let result = assertion.execute(&response_data, &expected).await;

    assert!(result.success);
    assert!(result.message.contains("not empty"));
}

#[tokio::test]
async fn test_xpath_assertion_range() {
    let mut assertion = XPathAssertion::new("//score/text()".to_string());
    
    let config = PluginConfig {
        settings: HashMap::from([
            ("xpath_expression".to_string(), json!("//score/text()"))
        ]),
        environment: "test".to_string(),
    };
    
    assertion.initialize(&config).await.unwrap();

    let response_data = ResponseData {
        status_code: 200,
        headers: HashMap::new(),
        body: r#"<result><score>85.5</score></result>"#.as_bytes().to_vec(),
        duration: Duration::from_millis(100),
        metadata: HashMap::new(),
    };

    let expected = ExpectedValue::Range { min: 80.0, max: 100.0 };
    let result = assertion.execute(&response_data, &expected).await;

    assert!(result.success);
    assert!(result.message.contains("within range"));
}

#[tokio::test]
async fn test_xpath_assertion_invalid_xml() {
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
        body: b"<invalid><xml>".to_vec(),
        duration: Duration::from_millis(100),
        metadata: HashMap::new(),
    };

    let expected = ExpectedValue::Exact(json!("John"));
    let result = assertion.execute(&response_data, &expected).await;

    assert!(!result.success);
    assert!(result.message.contains("Failed to parse XML"));
}

#[tokio::test]
async fn test_xml_schema_assertion_valid_schema() {
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
        duration: Duration::from_millis(100),
        metadata: HashMap::new(),
    };

    let expected = ExpectedValue::NotEmpty;
    let result = assertion.execute(&response_data, &expected).await;

    assert!(result.success);
    assert!(result.message.contains("validates against schema"));
}

#[tokio::test]
async fn test_xml_schema_assertion_invalid_xml() {
    let schema = r#"<?xml version="1.0"?><xs:schema xmlns:xs="http://www.w3.org/2001/XMLSchema"></xs:schema>"#;

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
        body: b"<invalid><xml>".to_vec(),
        duration: Duration::from_millis(100),
        metadata: HashMap::new(),
    };

    let expected = ExpectedValue::NotEmpty;
    let result = assertion.execute(&response_data, &expected).await;

    assert!(!result.success);
    assert!(result.message.contains("Failed to parse XML"));
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
        duration: Duration::from_millis(100),
        metadata: HashMap::new(),
    };

    let expected = ExpectedValue::Exact(json!("John"));
    let result = assertion.execute(&response_data, &expected).await;

    assert!(result.success);
    assert!(result.message.contains("matches expected value"));
}

#[tokio::test]
async fn test_namespace_aware_xml_assertion_multiple_namespaces() {
    let mut assertion = NamespaceAwareXmlAssertion::new("//p:name/text() | //c:email/text()".to_string())
        .with_namespace("p".to_string(), "http://example.com/person".to_string())
        .with_namespace("c".to_string(), "http://example.com/contact".to_string());
    
    let config = PluginConfig {
        settings: HashMap::from([
            ("xpath_expression".to_string(), json!("//p:name/text() | //c:email/text()")),
            ("namespaces".to_string(), json!({
                "p": "http://example.com/person",
                "c": "http://example.com/contact"
            }))
        ]),
        environment: "test".to_string(),
    };
    
    assertion.initialize(&config).await.unwrap();

    let response_data = ResponseData {
        status_code: 200,
        headers: HashMap::new(),
        body: r#"<root xmlns:p="http://example.com/person" xmlns:c="http://example.com/contact">
            <p:name>John</p:name>
            <c:email>john@example.com</c:email>
        </root>"#.as_bytes().to_vec(),
        duration: Duration::from_millis(100),
        metadata: HashMap::new(),
    };

    let expected = ExpectedValue::NotEmpty;
    let result = assertion.execute(&response_data, &expected).await;

    assert!(result.success);
}

#[tokio::test]
async fn test_namespace_aware_xml_assertion_contains() {
    let mut assertion = NamespaceAwareXmlAssertion::new("//ns:description/text()".to_string())
        .with_namespace("ns".to_string(), "http://example.com/product".to_string());
    
    let config = PluginConfig {
        settings: HashMap::from([
            ("xpath_expression".to_string(), json!("//ns:description/text()")),
            ("namespaces".to_string(), json!({
                "ns": "http://example.com/product"
            }))
        ]),
        environment: "test".to_string(),
    };
    
    assertion.initialize(&config).await.unwrap();

    let response_data = ResponseData {
        status_code: 200,
        headers: HashMap::new(),
        body: r#"<product xmlns="http://example.com/product">
            <description>Premium quality product</description>
        </product>"#.as_bytes().to_vec(),
        duration: Duration::from_millis(100),
        metadata: HashMap::new(),
    };

    let expected = ExpectedValue::Contains("Premium".to_string());
    let result = assertion.execute(&response_data, &expected).await;

    assert!(result.success);
    assert!(result.message.contains("contains"));
}