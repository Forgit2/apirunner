# API Test Runner - User Guide

This comprehensive guide will help you get started with the API Test Runner, from basic usage to advanced testing scenarios.

## Table of Contents

1. [Getting Started](#getting-started)
2. [Basic Usage](#basic-usage)
3. [Test Case Management](#test-case-management)
4. [Data-Driven Testing](#data-driven-testing)
5. [Assertions and Validations](#assertions-and-validations)
6. [Authentication](#authentication)
7. [Execution Modes](#execution-modes)
8. [Reporting](#reporting)
9. [Configuration](#configuration)
10. [Best Practices](#best-practices)
11. [Troubleshooting](#troubleshooting)

## Getting Started

### Installation

1. **Download the latest release** from the GitHub releases page
2. **Extract the binary** to a directory in your PATH
3. **Verify installation**:
   ```bash
   api-test-runner --version
   ```

### Quick Start

1. **Initialize a new test project**:
   ```bash
   api-test-runner init my-api-tests
   cd my-api-tests
   ```

2. **Create your first test case**:
   ```bash
   api-test-runner create-test --name "Get Users" --template rest-api-get
   ```

3. **Run the test**:
   ```bash
   api-test-runner run tests/get-users.yaml
   ```

## Basic Usage

### Command Line Interface

The API Test Runner provides a comprehensive CLI with the following main commands:

```bash
# Run tests
api-test-runner run [OPTIONS] <TEST_FILES>

# Manage test cases
api-test-runner create-test [OPTIONS]
api-test-runner list-tests [OPTIONS]
api-test-runner validate-test <TEST_FILE>

# Plugin management
api-test-runner plugins list
api-test-runner plugins install <PLUGIN>

# Configuration
api-test-runner config show
api-test-runner config set <KEY> <VALUE>
```

### Your First Test Case

Create a simple REST API test:

```yaml
# tests/get-users.yaml
name: "Get Users API Test"
description: "Test the users endpoint"

request:
  protocol: "http"
  method: "GET"
  url: "https://jsonplaceholder.typicode.com/users"
  headers:
    Accept: "application/json"
    User-Agent: "API-Test-Runner/1.0"

assertions:
  - type: "status_code"
    expected: 200
    message: "Should return 200 OK"
  
  - type: "response_time"
    expected: 1000
    message: "Should respond within 1 second"
  
  - type: "json_path"
    path: "$[0].name"
    expected: "Leanne Graham"
    message: "First user should be Leanne Graham"

timeout: 30s
```

Run this test:
```bash
api-test-runner run tests/get-users.yaml
```

## Test Case Management

### Creating Test Cases

#### Using Templates

List available templates:
```bash
api-test-runner templates list
```

Create from template:
```bash
api-test-runner create-test \
  --name "User Registration" \
  --template rest-api-post \
  --var base_url=https://api.example.com \
  --var endpoint=users
```

#### Manual Creation

Create a test case file manually:

```yaml
# tests/user-registration.yaml
name: "User Registration Test"
description: "Test user registration endpoint"

variables:
  base_url: "https://api.example.com"
  test_email: "test@example.com"

request:
  protocol: "http"
  method: "POST"
  url: "{{base_url}}/users"
  headers:
    Content-Type: "application/json"
    Accept: "application/json"
  body: |
    {
      "name": "Test User",
      "email": "{{test_email}}",
      "password": "securepassword123"
    }

assertions:
  - type: "status_code"
    expected: 201
  
  - type: "json_path"
    path: "$.email"
    expected: "{{test_email}}"

variable_extractions:
  - name: "user_id"
    json_path: "$.id"
```

### Organizing Test Cases

#### Directory Structure
```
tests/
├── auth/
│   ├── login.yaml
│   └── logout.yaml
├── users/
│   ├── create-user.yaml
│   ├── get-user.yaml
│   └── update-user.yaml
└── products/
    ├── list-products.yaml
    └── create-product.yaml
```

#### Test Suites

Create test suites to group related tests:

```yaml
# test-suites/user-management.yaml
name: "User Management Test Suite"
description: "Complete user management workflow tests"

tests:
  - "tests/users/create-user.yaml"
  - "tests/users/get-user.yaml"
  - "tests/users/update-user.yaml"
  - "tests/users/delete-user.yaml"

configuration:
  environment: "staging"
  parallel: false
  stop_on_failure: true
```

Run a test suite:
```bash
api-test-runner run test-suites/user-management.yaml
```

### Importing Existing Tests

#### From Postman Collections

```bash
api-test-runner import postman \
  --collection my-collection.json \
  --output tests/imported/
```

#### From OpenAPI Specifications

```bash
api-test-runner import openapi \
  --spec api-spec.yaml \
  --output tests/generated/ \
  --generate-examples
```

## Data-Driven Testing

### Using CSV Data Sources

```yaml
# tests/data-driven-user-test.yaml
name: "Data-Driven User Creation"

data_source:
  type: "csv"
  connection: "data/users.csv"
  
request:
  protocol: "http"
  method: "POST"
  url: "https://api.example.com/users"
  headers:
    Content-Type: "application/json"
  body: |
    {
      "name": "{{name}}",
      "email": "{{email}}",
      "department": "{{department}}"
    }

assertions:
  - type: "status_code"
    expected: 201
```

CSV data file:
```csv
# data/users.csv
name,email,department
John Doe,john@example.com,Engineering
Jane Smith,jane@example.com,Marketing
Bob Johnson,bob@example.com,Sales
```

### Using Database Data Sources

```yaml
# tests/database-driven-test.yaml
name: "Database-Driven Product Test"

data_source:
  type: "database"
  connection: "postgresql://user:pass@localhost/testdb"
  query: "SELECT id, name, price FROM products WHERE active = true"
  
request:
  protocol: "http"
  method: "GET"
  url: "https://api.example.com/products/{{id}}"

assertions:
  - type: "status_code"
    expected: 200
  
  - type: "json_path"
    path: "$.name"
    expected: "{{name}}"
  
  - type: "json_path"
    path: "$.price"
    expected: "{{price}}"
```

### Data Transformations

Apply transformations to your data:

```yaml
data_source:
  type: "csv"
  connection: "data/raw-users.csv"
  transformations:
    - type: "field_mapping"
      mappings:
        full_name: "name"
        email_address: "email"
    
    - type: "value_substitution"
      field: "department"
      substitutions:
        "eng": "Engineering"
        "mkt": "Marketing"
        "sales": "Sales"
    
    - type: "type_conversion"
      conversions:
        age: "integer"
        salary: "float"
```

## Assertions and Validations

### HTTP Assertions

```yaml
assertions:
  # Status code
  - type: "status_code"
    expected: 200
  
  # Status code range
  - type: "status_code"
    expected: [200, 201, 202]
  
  # Headers
  - type: "header"
    name: "Content-Type"
    expected: "application/json"
  
  # Response time
  - type: "response_time"
    expected: 1000  # milliseconds
  
  # Response time range
  - type: "response_time"
    min: 100
    max: 2000
```

### JSON Assertions

```yaml
assertions:
  # JSONPath exact match
  - type: "json_path"
    path: "$.user.name"
    expected: "John Doe"
  
  # JSONPath exists
  - type: "json_path"
    path: "$.user.id"
    exists: true
  
  # JSONPath regex
  - type: "json_path"
    path: "$.user.email"
    regex: "^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\\.[a-zA-Z]{2,}$"
  
  # JSON Schema validation
  - type: "json_schema"
    schema: |
      {
        "type": "object",
        "properties": {
          "id": {"type": "integer"},
          "name": {"type": "string"},
          "email": {"type": "string", "format": "email"}
        },
        "required": ["id", "name", "email"]
      }
```

### XML Assertions

```yaml
assertions:
  # XPath exact match
  - type: "xpath"
    path: "//user/name/text()"
    expected: "John Doe"
  
  # XPath exists
  - type: "xpath"
    path: "//user[@id='123']"
    exists: true
  
  # XML Schema validation
  - type: "xml_schema"
    schema_file: "schemas/user.xsd"
```

### Cross-System Validations

```yaml
assertions:
  # Database validation
  - type: "database"
    connection: "postgresql://user:pass@localhost/db"
    query: "SELECT COUNT(*) FROM users WHERE email = '{{response.email}}'"
    expected: 1
  
  # Message queue validation
  - type: "message_queue"
    connection: "amqp://localhost"
    queue: "user.created"
    timeout: 5000
    expected_message_count: 1
  
  # Cache validation
  - type: "cache"
    connection: "redis://localhost:6379"
    key: "user:{{response.id}}"
    exists: true
```

## Authentication

### Basic Authentication

```yaml
request:
  protocol: "http"
  method: "GET"
  url: "https://api.example.com/protected"
  auth:
    type: "basic"
    username: "{{username}}"
    password: "{{password}}"
```

### Bearer Token Authentication

```yaml
request:
  protocol: "http"
  method: "GET"
  url: "https://api.example.com/protected"
  auth:
    type: "bearer"
    token: "{{access_token}}"
```

### OAuth 2.0 Authentication

```yaml
# Configuration
auth_config:
  oauth2:
    client_id: "{{client_id}}"
    client_secret: "{{client_secret}}"
    token_url: "https://auth.example.com/oauth/token"
    scope: "read write"

request:
  protocol: "http"
  method: "GET"
  url: "https://api.example.com/protected"
  auth:
    type: "oauth2"
    config_ref: "oauth2"
```

### JWT Authentication

```yaml
request:
  protocol: "http"
  method: "GET"
  url: "https://api.example.com/protected"
  auth:
    type: "jwt"
    token: "{{jwt_token}}"
    # Optional: validate token before use
    validate: true
    public_key_url: "https://auth.example.com/.well-known/jwks.json"
```

### Authentication Chaining

```yaml
# Multi-step authentication
auth_chain:
  - type: "basic"
    username: "{{username}}"
    password: "{{password}}"
    extract_token: "$.access_token"
  
  - type: "bearer"
    token: "{{extracted_token}}"

request:
  protocol: "http"
  method: "GET"
  url: "https://api.example.com/protected"
  auth:
    type: "chain"
    chain_ref: "auth_chain"
```

## Execution Modes

### Serial Execution

Execute tests one after another, maintaining state:

```bash
api-test-runner run --mode serial tests/
```

```yaml
# Configuration for serial execution
execution:
  mode: "serial"
  stop_on_failure: true
  state_propagation: true
```

### Parallel Execution

Execute tests concurrently:

```bash
api-test-runner run --mode parallel --concurrency 10 tests/
```

```yaml
# Configuration for parallel execution
execution:
  mode: "parallel"
  concurrency: 10
  rate_limit: 100  # requests per second
  isolation: true  # isolate test data
```

### Distributed Execution

Execute tests across multiple nodes:

```bash
# Start coordinator node
api-test-runner run --mode distributed --coordinator tests/

# Start worker nodes
api-test-runner worker --coordinator-url http://coordinator:8080
```

```yaml
# Configuration for distributed execution
execution:
  mode: "distributed"
  coordinator:
    port: 8080
    discovery: "consul://localhost:8500"
  worker:
    capacity: 50
    heartbeat_interval: 30s
```

### Interactive Execution

Run tests interactively with debugging:

```bash
# Interactive mode
api-test-runner run --interactive tests/debug-test.yaml

# Debug mode with breakpoints
api-test-runner run --debug --breakpoints 1,3,5 tests/debug-test.yaml

# Dry run (validate without executing)
api-test-runner run --dry-run tests/
```

## Reporting

### Built-in Report Formats

#### JUnit XML (for CI/CD)

```bash
api-test-runner run --report junit --output reports/junit.xml tests/
```

#### HTML Report

```bash
api-test-runner run --report html --output reports/ tests/
```

#### JSON Report

```bash
api-test-runner run --report json --output reports/results.json tests/
```

### Custom Report Templates

Create custom report templates:

```yaml
# templates/custom-report.yaml
name: "Custom Team Report"
format: "html"
template: |
  <!DOCTYPE html>
  <html>
  <head>
    <title>API Test Results - {{execution.timestamp}}</title>
    <style>
      /* Custom CSS */
      .passed { color: green; }
      .failed { color: red; }
    </style>
  </head>
  <body>
    <h1>Test Results Summary</h1>
    <p>Total Tests: {{summary.total}}</p>
    <p>Passed: <span class="passed">{{summary.passed}}</span></p>
    <p>Failed: <span class="failed">{{summary.failed}}</span></p>
    
    {{#each test_results}}
    <div class="test-case">
      <h3>{{name}}</h3>
      <p>Status: <span class="{{status}}">{{status}}</span></p>
      <p>Duration: {{duration}}ms</p>
    </div>
    {{/each}}
  </body>
  </html>
```

Use custom template:
```bash
api-test-runner run --report custom --template templates/custom-report.yaml tests/
```

### Real-time Monitoring

Enable real-time monitoring and alerts:

```yaml
# Configuration
monitoring:
  enabled: true
  metrics:
    - response_time
    - success_rate
    - error_rate
  
  alerts:
    - name: "High Error Rate"
      condition: "error_rate > 0.05"
      notification:
        type: "slack"
        webhook: "{{slack_webhook}}"
    
    - name: "Slow Response Time"
      condition: "avg_response_time > 2000"
      notification:
        type: "email"
        recipients: ["team@example.com"]
```

## Configuration

### Configuration Hierarchy

Configuration is loaded in the following order (later values override earlier ones):

1. Built-in defaults
2. Global configuration file (`~/.api-test-runner/config.yaml`)
3. Project configuration file (`./config.yaml`)
4. Environment-specific configuration (`./config.{environment}.yaml`)
5. Command-line arguments
6. Environment variables

### Global Configuration

```yaml
# ~/.api-test-runner/config.yaml
default_environment: "development"
plugin_directory: "~/.api-test-runner/plugins"

execution:
  default_timeout: 30s
  max_retries: 3
  retry_delay: 1s

reporting:
  default_format: "html"
  output_directory: "./reports"

logging:
  level: "info"
  format: "json"
```

### Project Configuration

```yaml
# ./config.yaml
project_name: "My API Tests"
base_url: "https://api.example.com"

environments:
  development:
    base_url: "https://dev-api.example.com"
    database_url: "postgresql://localhost/dev_db"
  
  staging:
    base_url: "https://staging-api.example.com"
    database_url: "postgresql://staging-db/staging_db"
  
  production:
    base_url: "https://api.example.com"
    database_url: "postgresql://prod-db/prod_db"

variables:
  api_key: "{{API_KEY}}"
  timeout: 30s

plugins:
  - name: "custom-auth-plugin"
    path: "./plugins/custom-auth.so"
    config:
      auth_server: "https://auth.example.com"
```

### Environment Variables

Set configuration via environment variables:

```bash
export API_TEST_RUNNER_ENVIRONMENT=staging
export API_TEST_RUNNER_BASE_URL=https://staging-api.example.com
export API_TEST_RUNNER_LOG_LEVEL=debug
export API_KEY=your-secret-api-key
```

### Secure Configuration

Store sensitive values securely:

```yaml
# Use environment variables for secrets
database_url: "{{DATABASE_URL}}"
api_key: "{{API_KEY}}"

# Use encrypted configuration files
secrets:
  encryption_key: "{{ENCRYPTION_KEY}}"
  encrypted_values:
    database_password: "encrypted:AES256:base64encodedvalue"
    oauth_client_secret: "encrypted:AES256:anotherencryptedvalue"
```

## Best Practices

### Test Organization

1. **Use descriptive names** for test cases and assertions
2. **Group related tests** into suites and directories
3. **Use tags** to categorize tests (smoke, regression, integration)
4. **Keep tests independent** - avoid dependencies between test cases
5. **Use data-driven testing** for similar test scenarios with different data

### Test Data Management

1. **Use external data sources** instead of hardcoding test data
2. **Clean up test data** after test execution
3. **Use realistic test data** that represents production scenarios
4. **Implement data isolation** for parallel test execution
5. **Version control test data** alongside test cases

### Error Handling

1. **Use meaningful assertion messages** that help with debugging
2. **Implement proper timeout handling** for all operations
3. **Add retry logic** for flaky network operations
4. **Log detailed information** for failed tests
5. **Use health checks** to validate system state before testing

### Performance

1. **Use connection pooling** for database and HTTP connections
2. **Implement rate limiting** to avoid overwhelming target systems
3. **Use parallel execution** for independent tests
4. **Monitor resource usage** during test execution
5. **Optimize data queries** and transformations

### Security

1. **Store credentials securely** using environment variables or encrypted config
2. **Use HTTPS** for all API communications
3. **Validate TLS certificates** in production environments
4. **Implement proper authentication** for test environments
5. **Avoid logging sensitive data** in test outputs

### Maintenance

1. **Keep tests up to date** with API changes
2. **Review and refactor** test cases regularly
3. **Monitor test execution times** and optimize slow tests
4. **Use version control** for all test artifacts
5. **Document test scenarios** and expected behaviors

## Troubleshooting

### Common Issues

#### Connection Timeouts

```yaml
# Increase timeout values
request:
  timeout: 60s

# Configure retry logic
execution:
  max_retries: 3
  retry_delay: 2s
  retry_on_timeout: true
```

#### Authentication Failures

```bash
# Debug authentication
api-test-runner run --debug --log-level debug tests/auth-test.yaml

# Validate tokens
api-test-runner validate-token --token "{{jwt_token}}"
```

#### Data Source Connection Issues

```bash
# Test data source connectivity
api-test-runner test-datasource --config config.yaml --source users_db

# Validate queries
api-test-runner validate-query --source users_db --query "SELECT * FROM users LIMIT 1"
```

#### Plugin Loading Errors

```bash
# List loaded plugins
api-test-runner plugins list

# Validate plugin
api-test-runner plugins validate ./plugins/custom-plugin.so

# Check plugin dependencies
api-test-runner plugins deps ./plugins/custom-plugin.so
```

### Debugging Tests

#### Enable Debug Logging

```bash
api-test-runner run --log-level debug tests/
```

#### Interactive Debugging

```bash
# Step through test execution
api-test-runner run --interactive --step-through tests/debug-test.yaml

# Inspect variables
api-test-runner run --interactive --inspect-variables tests/debug-test.yaml
```

#### Dry Run Mode

```bash
# Validate tests without execution
api-test-runner run --dry-run tests/

# Check variable substitution
api-test-runner run --dry-run --show-variables tests/
```

### Performance Issues

#### Monitor Resource Usage

```bash
# Enable resource monitoring
api-test-runner run --monitor-resources tests/

# Generate performance report
api-test-runner run --report performance tests/
```

#### Optimize Execution

```bash
# Use parallel execution
api-test-runner run --mode parallel --concurrency 20 tests/

# Enable connection pooling
api-test-runner run --connection-pool-size 50 tests/
```

### Getting Help

1. **Check the documentation** in the `docs/` directory
2. **Use the built-in help** system:
   ```bash
   api-test-runner help
   api-test-runner help run
   ```
3. **Enable verbose logging** for detailed error information
4. **Check the GitHub issues** for known problems and solutions
5. **Join the community** discussions for support and tips