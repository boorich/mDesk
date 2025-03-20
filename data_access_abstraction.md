# Data Access Abstraction Layer for mDesk

This document outlines the design for a unified data access abstraction layer for mDesk. The goal is to create a flexible system that can handle various data sources (MCP resources, file systems, databases, and potentially distributed systems) through a consistent interface.

## Core Concepts

### 1. Resource-Oriented Interface

All data should be accessible through a resource-oriented interface, modeled after the MCP (Model Context Protocol) resource concept but extended to be more versatile. This approach provides:

- Consistent access patterns regardless of underlying storage
- Token-preserving data access for LLM interactions
- Compatibility with the MCP ecosystem
- Extensibility for future data sources

### 2. Architecture Overview

```
Application Layer
      ↓ ↑
Resource Access Layer (traits and interfaces)
      ↓ ↑
Resource Provider Registry (routing by URI scheme)
      ↓ ↑
╔════════╦════════╦═══════════╦═══════════╗
║  MCP   ║ Local  ║ Database  ║  Future   ║
║Provider║Provider║ Provider  ║ Providers ║
╚════════╩════════╩═══════════╩═══════════╝
```

## Core Components

### Resource Types

```rust
/// Represents a resource with metadata
pub struct Resource {
    /// URI representing the resource location (e.g., "file:///path/to/file")
    pub uri: String,
    /// Name of the resource
    pub name: String,
    /// Optional description of the resource
    pub description: Option<String>,
    /// MIME type of the resource content
    pub mime_type: String,
    /// Additional metadata
    pub metadata: ResourceMetadata,
}

/// Metadata for resources
pub struct ResourceMetadata {
    /// Priority of the resource (0.0 to 1.0)
    pub priority: Option<f32>,
    /// Creation/modification timestamp
    pub timestamp: Option<DateTime<Utc>>,
    /// Tags for organizing resources
    pub tags: Vec<String>,
    /// Custom metadata as key-value pairs
    pub custom: HashMap<String, Value>,
}

/// Content of a resource
pub enum ResourceContent {
    Text(String),
    Binary(Vec<u8>),
    // Other types as needed
}
```

### Core Traits

```rust
/// Error types for resource operations
pub enum ResourceError {
    NotFound(String),
    PermissionDenied(String),
    InvalidUri(String),
    ReadError(String),
    WriteError(String),
    // Other error types
}

/// Primary trait for resource access
pub trait ResourceProvider: Send + Sync {
    /// Get a list of available resources
    async fn list_resources(&self, query: Option<ResourceQuery>) -> Result<Vec<Resource>, ResourceError>;
    
    /// Read the content of a resource
    async fn read_resource(&self, uri: &str) -> Result<ResourceContent, ResourceError>;
    
    /// Write content to a resource (if supported)
    async fn write_resource(&self, uri: &str, content: ResourceContent) -> Result<Resource, ResourceError>;
    
    /// Update metadata for a resource (if supported)
    async fn update_metadata(&self, uri: &str, metadata: ResourceMetadata) -> Result<Resource, ResourceError>;
    
    /// Check if the provider can handle a specific URI scheme
    fn supports_scheme(&self, scheme: &str) -> bool;
    
    /// Get capabilities of this provider
    fn capabilities(&self) -> ResourceProviderCapabilities;
}

/// Capabilities of a resource provider
pub struct ResourceProviderCapabilities {
    pub readable: bool,
    pub writable: bool,
    pub searchable: bool,
    pub supports_metadata: bool,
    pub supports_streaming: bool,
    // Other capabilities
}

/// Query parameters for listing resources
pub struct ResourceQuery {
    pub tags: Option<Vec<String>>,
    pub mime_types: Option<Vec<String>>,
    pub text_search: Option<String>,
    pub min_priority: Option<f32>,
    pub limit: Option<usize>,
    pub cursor: Option<String>,
    // Other query parameters
}
```

### Provider Registry

```rust
/// Registry for resource providers
pub struct ResourceProviderRegistry {
    providers: HashMap<String, Box<dyn ResourceProvider>>,
    scheme_mappings: HashMap<String, String>,
}

impl ResourceProviderRegistry {
    /// Create a new empty registry
    pub fn new() -> Self { /* ... */ }
    
    /// Register a provider with a name
    pub fn register(&mut self, name: &str, provider: Box<dyn ResourceProvider>) { /* ... */ }
    
    /// Map a URI scheme to a specific provider
    pub fn map_scheme(&mut self, scheme: &str, provider_name: &str) { /* ... */ }
    
    /// Get a provider by name
    pub fn get_provider(&self, name: &str) -> Option<&dyn ResourceProvider> { /* ... */ }
    
    /// Get the appropriate provider for a URI
    pub fn provider_for_uri(&self, uri: &str) -> Option<&dyn ResourceProvider> { /* ... */ }
    
    /// List all registered providers
    pub fn list_providers(&self) -> Vec<(String, ResourceProviderCapabilities)> { /* ... */ }
}
```

### Unified Resource Service

```rust
/// Main service that applications will interact with
pub struct ResourceService {
    registry: ResourceProviderRegistry,
}

impl ResourceService {
    /// Create a new service with the given registry
    pub fn new(registry: ResourceProviderRegistry) -> Self { /* ... */ }
    
    /// List resources across all providers or from a specific provider
    pub async fn list_resources(
        &self, 
        query: Option<ResourceQuery>,
        provider: Option<&str>
    ) -> Result<Vec<Resource>, ResourceError> { /* ... */ }
    
    /// Read a resource from the appropriate provider
    pub async fn read_resource(&self, uri: &str) -> Result<ResourceContent, ResourceError> { /* ... */ }
    
    /// Write to a resource using the appropriate provider
    pub async fn write_resource(
        &self, 
        uri: &str, 
        content: ResourceContent
    ) -> Result<Resource, ResourceError> { /* ... */ }
    
    /// Search across all providers
    pub async fn search(
        &self, 
        query: ResourceQuery
    ) -> Result<Vec<Resource>, ResourceError> { /* ... */ }
    
    // Other methods that operate across providers
}
```

## Provider Implementations

### 1. MCP Resource Provider

Wraps the MCP client to expose resources through our abstraction:

```rust
pub struct McpResourceProvider {
    client: Box<dyn McpClientTrait>,
}

impl ResourceProvider for McpResourceProvider {
    // Implementation that delegates to MCP client
}
```

### 2. Local Resource Provider

Provides access to the local filesystem:

```rust
pub struct LocalResourceProvider {
    root_path: PathBuf,
    // Configuration options
}

impl ResourceProvider for LocalResourceProvider {
    // Implementation using std::fs or async-fs
}
```

### 3. Database Resource Provider

Provides access to database-stored resources:

```rust
pub struct DatabaseResourceProvider<DB> {
    connection: DB,
    // Configuration options
}

impl<DB: DatabaseConnection> ResourceProvider for DatabaseResourceProvider<DB> {
    // Implementation for database access
}
```

## Extension Points

### Resource Transformation

```rust
/// Trait for transforming resources
pub trait ResourceTransformer: Send + Sync {
    /// Transform a resource's content
    async fn transform(
        &self, 
        content: ResourceContent,
        metadata: &ResourceMetadata
    ) -> Result<ResourceContent, ResourceError>;
    
    /// Get MIME types this transformer can handle
    fn supported_mime_types(&self) -> Vec<String>;
}
```

### Caching Layer

```rust
pub struct CachedResourceProvider<P: ResourceProvider> {
    inner: P,
    cache: ResourceCache,
}

impl<P: ResourceProvider> ResourceProvider for CachedResourceProvider<P> {
    // Implementation that adds caching
}
```

## Configuration

Example configuration for setting up the resource service:

```rust
let mut registry = ResourceProviderRegistry::new();

// Add MCP provider
let mcp_client = McpClient::new(service);
registry.register("mcp", Box::new(McpResourceProvider::new(mcp_client)));
registry.map_scheme("mcp", "mcp");

// Add local filesystem provider
let local_provider = LocalResourceProvider::new("/Users/martinmaurer/Documents");
registry.register("local", Box::new(local_provider));
registry.map_scheme("file", "local");

// Add database provider
let db_connection = establish_database_connection().await?;
let db_provider = DatabaseResourceProvider::new(db_connection);
registry.register("database", Box::new(db_provider));
registry.map_scheme("db", "database");

// Create the service
let resource_service = ResourceService::new(registry);
```

## Implementation Roadmap

1. **Phase 1: Core Abstractions**
   - Define the core traits and data structures
   - Implement the registry and basic routing

2. **Phase 2: Basic Providers**
   - Implement MCP resource provider
   - Implement local filesystem provider
   - Basic testing infrastructure

3. **Phase 3: Enhanced Features**
   - Add caching capabilities
   - Implement search functionality
   - Add metadata management

4. **Phase 4: Advanced Providers**
   - Database provider implementation
   - Remote API provider
   - Consider distributed storage options

5. **Phase 5: Performance & Optimization**
   - Implement efficient batch operations
   - Add resource streaming capabilities
   - Performance benchmarking

## Integration with Tauri/Dioxus

For a Tauri or Dioxus application, this abstraction can be integrated by:

1. Initializing the resource service at application startup
2. Exposing key functionality through Tauri commands or Dioxus state
3. Creating UI components for resource browsing, searching, and management
4. Using the resource service for all data access needs throughout the application

## Considerations for the Future

1. **Security**: Consider access control for resources
2. **Synchronization**: Mechanisms for keeping distributed resources in sync
3. **Version Control**: Track changes to resources over time
4. **Resource Relationships**: Model connections between related resources
5. **Schema Validation**: Add validation for structured resources

This abstraction provides a solid foundation for building a flexible, extensible data access layer that can grow with your application's needs while maintaining compatibility with the MCP ecosystem.
