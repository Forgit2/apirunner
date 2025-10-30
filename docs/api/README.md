# API Test Runner - API Documentation

This directory contains comprehensive API documentation for all plugin interfaces and core components of the API Test Runner.

## Documentation Structure

- [Plugin Interfaces](plugin-interfaces.md) - Core plugin trait definitions and interfaces
- [Protocol Plugins](protocol-plugins.md) - Protocol-specific plugin interfaces (HTTP, etc.)
- [Assertion Plugins](assertion-plugins.md) - Assertion plugin interfaces and implementations
- [Data Source Plugins](data-source-plugins.md) - Data source plugin interfaces
- [Authentication Plugins](auth-plugins.md) - Authentication plugin interfaces
- [Reporter Plugins](reporter-plugins.md) - Report generation plugin interfaces
- [Core Components](core-components.md) - Core system components and APIs
- [Event System](event-system.md) - Event-driven architecture APIs
- [Execution Engine](execution-engine.md) - Test execution engine APIs

## Quick Reference

### Core Plugin Trait
All plugins must implement the base `Plugin` trait:

```rust
#[async_trait]
pub trait Plugin: Send + Sync {
    fn name(&self) -> &str;
    fn version(&self) -> &str;
    fn dependencies(&self) -> Vec<String>;
    async fn initialize(&mut self, config: &PluginConfig) -> Result<(), PluginError>;
    async fn shutdown(&mut self) -> Result<(), PluginError>;
}
```

### Plugin Types
- **Protocol Plugins**: Handle specific communication protocols (HTTP, GraphQL, etc.)
- **Assertion Plugins**: Validate response data and system state
- **Data Source Plugins**: Provide test data from various sources
- **Authentication Plugins**: Handle authentication mechanisms
- **Reporter Plugins**: Generate test reports in various formats

## Getting Started

1. Read the [Plugin Interfaces](plugin-interfaces.md) documentation
2. Choose the appropriate plugin type for your needs
3. Refer to the [Plugin Development Guide](../guides/plugin-development.md)
4. See example implementations in the [examples](../examples/) directory