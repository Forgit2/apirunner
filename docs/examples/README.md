# API Test Runner - Examples

This directory contains practical examples demonstrating various features and use cases of the API Test Runner.

## Example Categories

### Basic Examples
- [Simple REST API Test](basic/simple-rest-test.yaml) - Basic GET request with assertions
- [POST Request with JSON](basic/post-json-request.yaml) - Creating resources with JSON payload
- [Authentication Examples](basic/authentication-examples.yaml) - Various authentication methods

### Data-Driven Testing
- [CSV Data Source](data-driven/csv-data-source.yaml) - Using CSV files for test data
- [Database Data Source](data-driven/database-data-source.yaml) - Fetching test data from databases
- [API Data Source](data-driven/api-data-source.yaml) - Using API responses as test data

### Advanced Assertions
- [JSON Path Assertions](assertions/json-path-examples.yaml) - Complex JSON validation
- [XML Assertions](assertions/xml-assertions.yaml) - XML response validation
- [Cross-System Validation](assertions/cross-system-validation.yaml) - Database and queue validation

### Workflow Testing
- [User Registration Flow](workflows/user-registration-flow.yaml) - Multi-step user registration
- [E-commerce Checkout](workflows/ecommerce-checkout.yaml) - Complete checkout process
- [API Versioning Tests](workflows/api-versioning.yaml) - Testing multiple API versions

### Performance Testing
- [Load Testing](performance/load-testing.yaml) - Basic load testing configuration
- [Stress Testing](performance/stress-testing.yaml) - Stress testing with rate limiting
- [Performance Benchmarks](performance/benchmarks.yaml) - Performance baseline testing

### Plugin Examples
- [Custom Protocol Plugin](plugins/custom-protocol/) - Example custom protocol implementation
- [Business Logic Assertion](plugins/business-assertion/) - Custom assertion plugin
- [Custom Data Source](plugins/custom-datasource/) - Custom data source plugin

### Configuration Examples
- [Environment Configurations](config/environments/) - Multi-environment setup
- [Plugin Configurations](config/plugins/) - Plugin configuration examples
- [Security Configurations](config/security/) - Secure configuration patterns

## Getting Started

1. **Choose an example** that matches your use case
2. **Copy the example files** to your test project
3. **Modify the configuration** to match your environment
4. **Run the tests** using the API Test Runner CLI

## Running Examples

```bash
# Run a single example
api-test-runner run docs/examples/basic/simple-rest-test.yaml

# Run all examples in a category
api-test-runner run docs/examples/basic/

# Run with specific configuration
api-test-runner run --config docs/examples/config/development.yaml docs/examples/basic/
```

## Contributing Examples

We welcome contributions of new examples! Please:

1. **Follow the existing structure** and naming conventions
2. **Include clear documentation** and comments
3. **Test your examples** before submitting
4. **Add appropriate tags** and metadata
5. **Update this README** with your new example