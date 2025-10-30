# Data Source Plugins

Data source plugins provide test data from various sources including files, databases, APIs, and custom data providers. They enable data-driven testing with flexible data transformation capabilities.

## DataSourcePlugin Trait

```rust
#[async_trait]
pub trait DataSourcePlugin: Plugin {
    /// Return the type of data source this plugin handles
    fn source_type(&self) -> DataSourceType;
    
    /// Connect to the data source and validate configuration
    async fn connect(&mut self, config: &DataSourceConfig) -> Result<(), DataSourceError>;
    
    /// Fetch data from the source with optional filtering
    async fn fetch_data(&self, query: &DataQuery) -> Result<DataSet, DataSourceError>;
    
    /// Check if the data source is available and responsive
    async fn health_check(&self) -> Result<HealthStatus, DataSourceError>;
    
    /// Close the connection and clean up resources
    async fn disconnect(&mut self) -> Result<(), DataSourceError>;
}
```

## Data Source Types

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum DataSourceType {
    /// File-based data sources (CSV, JSON, YAML)
    File(FileFormat),
    /// Database data sources
    Database(DatabaseType),
    /// HTTP API data sources
    Api,
    /// Custom data source
    Custom(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum FileFormat {
    Csv,
    Json,
    Yaml,
    Xml,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DatabaseType {
    PostgreSQL,
    MySQL,
    SQLite,
    MongoDB,
    Redis,
}
```

## Data Source Configuration

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataSourceConfig {
    /// Connection string or file path
    pub connection: String,
    /// Authentication credentials
    pub credentials: Option<Credentials>,
    /// Connection pool settings
    pub pool_config: Option<PoolConfig>,
    /// Data source specific settings
    pub settings: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Credentials {
    pub username: Option<String>,
    pub password: Option<String>,
    pub token: Option<String>,
    pub certificate: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolConfig {
    pub min_connections: Option<u32>,
    pub max_connections: Option<u32>,
    pub connection_timeout: Option<Duration>,
    pub idle_timeout: Option<Duration>,
}
```

## Data Query

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataQuery {
    /// Query expression (SQL, JSONPath, etc.)
    pub query: String,
    /// Query parameters
    pub parameters: HashMap<String, serde_json::Value>,
    /// Maximum number of records to return
    pub limit: Option<usize>,
    /// Number of records to skip
    pub offset: Option<usize>,
    /// Sorting criteria
    pub sort: Option<Vec<SortCriteria>>,
    /// Filtering criteria
    pub filter: Option<FilterCriteria>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SortCriteria {
    pub field: String,
    pub direction: SortDirection,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SortDirection {
    Ascending,
    Descending,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilterCriteria {
    pub conditions: Vec<FilterCondition>,
    pub operator: LogicalOperator,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilterCondition {
    pub field: String,
    pub operator: ComparisonOperator,
    pub value: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LogicalOperator {
    And,
    Or,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ComparisonOperator {
    Equal,
    NotEqual,
    GreaterThan,
    LessThan,
    GreaterThanOrEqual,
    LessThanOrEqual,
    Contains,
    StartsWith,
    EndsWith,
    In,
    NotIn,
}
```

## Data Set

```rust
#[derive(Debug, Clone)]
pub struct DataSet {
    /// Column names
    pub columns: Vec<String>,
    /// Data rows
    pub rows: Vec<DataRow>,
    /// Total number of records (may be larger than rows.len() if limited)
    pub total_count: Option<usize>,
    /// Metadata about the data set
    pub metadata: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone)]
pub struct DataRow {
    /// Column values
    pub values: HashMap<String, serde_json::Value>,
}

impl DataSet {
    /// Get a specific row by index
    pub fn get_row(&self, index: usize) -> Option<&DataRow> {
        self.rows.get(index)
    }
    
    /// Get all values for a specific column
    pub fn get_column_values(&self, column: &str) -> Vec<&serde_json::Value> {
        self.rows.iter()
            .filter_map(|row| row.values.get(column))
            .collect()
    }
    
    /// Convert to JSON array
    pub fn to_json(&self) -> serde_json::Value {
        json!(self.rows.iter().map(|row| &row.values).collect::<Vec<_>>())
    }
}
```

## Health Status

```rust
#[derive(Debug, Clone)]
pub struct HealthStatus {
    /// Whether the data source is healthy
    pub healthy: bool,
    /// Response time for health check
    pub response_time: Duration,
    /// Additional status information
    pub details: HashMap<String, serde_json::Value>,
}
```

## Built-in Data Source Plugins

### CSV Data Source

```rust
pub struct CsvDataSource {
    file_path: PathBuf,
    reader: Option<csv::Reader<File>>,
    config: DataSourceConfig,
}

#[async_trait]
impl DataSourcePlugin for CsvDataSource {
    fn source_type(&self) -> DataSourceType {
        DataSourceType::File(FileFormat::Csv)
    }
    
    async fn connect(&mut self, config: &DataSourceConfig) -> Result<(), DataSourceError> {
        self.file_path = PathBuf::from(&config.connection);
        self.config = config.clone();
        
        // Validate file exists and is readable
        if !self.file_path.exists() {
            return Err(DataSourceError::ConnectionFailed(
                format!("File not found: {}", self.file_path.display())
            ));
        }
        
        Ok(())
    }
    
    async fn fetch_data(&self, query: &DataQuery) -> Result<DataSet, DataSourceError> {
        let file = File::open(&self.file_path)?;
        let mut reader = csv::Reader::from_reader(file);
        
        // Get headers
        let headers = reader.headers()?.clone();
        let columns: Vec<String> = headers.iter().map(|h| h.to_string()).collect();
        
        let mut rows = Vec::new();
        let mut count = 0;
        
        for result in reader.records() {
            let record = result?;
            
            // Apply offset
            if let Some(offset) = query.offset {
                if count < offset {
                    count += 1;
                    continue;
                }
            }
            
            // Apply limit
            if let Some(limit) = query.limit {
                if rows.len() >= limit {
                    break;
                }
            }
            
            let mut values = HashMap::new();
            for (i, field) in record.iter().enumerate() {
                if let Some(column_name) = columns.get(i) {
                    values.insert(column_name.clone(), json!(field));
                }
            }
            
            rows.push(DataRow { values });
            count += 1;
        }
        
        Ok(DataSet {
            columns,
            rows,
            total_count: Some(count),
            metadata: HashMap::new(),
        })
    }
    
    async fn health_check(&self) -> Result<HealthStatus, DataSourceError> {
        let start = Instant::now();
        let exists = self.file_path.exists();
        let response_time = start.elapsed();
        
        Ok(HealthStatus {
            healthy: exists,
            response_time,
            details: HashMap::from([
                ("file_path".to_string(), json!(self.file_path.display().to_string())),
                ("exists".to_string(), json!(exists)),
            ]),
        })
    }
    
    async fn disconnect(&mut self) -> Result<(), DataSourceError> {
        self.reader = None;
        Ok(())
    }
}
```

### Database Data Source

```rust
pub struct DatabaseDataSource {
    connection_pool: Option<Arc<Pool<PostgresConnectionManager<NoTls>>>>,
    config: DataSourceConfig,
}

#[async_trait]
impl DataSourcePlugin for DatabaseDataSource {
    fn source_type(&self) -> DataSourceType {
        DataSourceType::Database(DatabaseType::PostgreSQL)
    }
    
    async fn connect(&mut self, config: &DataSourceConfig) -> Result<(), DataSourceError> {
        let manager = PostgresConnectionManager::new_from_stringlike(
            &config.connection,
            NoTls,
        )?;
        
        let pool_config = config.pool_config.as_ref();
        let pool = Pool::builder()
            .min_idle(pool_config.and_then(|p| p.min_connections).map(|n| n as u32))
            .max_size(pool_config.and_then(|p| p.max_connections).unwrap_or(10) as u32)
            .connection_timeout(pool_config.and_then(|p| p.connection_timeout).unwrap_or(Duration::from_secs(30)))
            .build(manager)?;
            
        self.connection_pool = Some(Arc::new(pool));
        self.config = config.clone();
        Ok(())
    }
    
    async fn fetch_data(&self, query: &DataQuery) -> Result<DataSet, DataSourceError> {
        let pool = self.connection_pool.as_ref()
            .ok_or_else(|| DataSourceError::NotConnected)?;
            
        let conn = pool.get().await?;
        
        // Build SQL query with parameters
        let mut sql = query.query.clone();
        
        // Apply limit and offset
        if let Some(limit) = query.limit {
            sql.push_str(&format!(" LIMIT {}", limit));
        }
        if let Some(offset) = query.offset {
            sql.push_str(&format!(" OFFSET {}", offset));
        }
        
        // Execute query
        let rows = conn.query(&sql, &[]).await?;
        
        if rows.is_empty() {
            return Ok(DataSet {
                columns: vec![],
                rows: vec![],
                total_count: Some(0),
                metadata: HashMap::new(),
            });
        }
        
        // Extract column names
        let columns: Vec<String> = rows[0].columns()
            .iter()
            .map(|col| col.name().to_string())
            .collect();
            
        // Convert rows to DataSet
        let mut data_rows = Vec::new();
        for row in rows {
            let mut values = HashMap::new();
            for (i, column) in columns.iter().enumerate() {
                let value: Option<String> = row.try_get(i).ok();
                values.insert(column.clone(), json!(value));
            }
            data_rows.push(DataRow { values });
        }
        
        Ok(DataSet {
            columns,
            rows: data_rows,
            total_count: None, // Would need separate COUNT query
            metadata: HashMap::new(),
        })
    }
    
    async fn health_check(&self) -> Result<HealthStatus, DataSourceError> {
        let start = Instant::now();
        
        let healthy = match &self.connection_pool {
            Some(pool) => {
                match pool.get().await {
                    Ok(conn) => {
                        conn.query("SELECT 1", &[]).await.is_ok()
                    }
                    Err(_) => false,
                }
            }
            None => false,
        };
        
        let response_time = start.elapsed();
        
        Ok(HealthStatus {
            healthy,
            response_time,
            details: HashMap::from([
                ("connection_string".to_string(), json!(self.config.connection)),
            ]),
        })
    }
    
    async fn disconnect(&mut self) -> Result<(), DataSourceError> {
        self.connection_pool = None;
        Ok(())
    }
}
```

## Data Transformation Pipeline

```rust
pub struct DataTransformationPipeline {
    transformers: Vec<Box<dyn DataTransformer>>,
}

#[async_trait]
pub trait DataTransformer: Send + Sync {
    /// Transform a data set
    async fn transform(&self, data: DataSet) -> Result<DataSet, TransformationError>;
    
    /// Get the name of this transformer
    fn name(&self) -> &str;
}

// Built-in transformers
pub struct FieldMappingTransformer {
    mappings: HashMap<String, String>,
}

pub struct ValueSubstitutionTransformer {
    substitutions: HashMap<String, HashMap<String, serde_json::Value>>,
}

pub struct TypeConversionTransformer {
    conversions: HashMap<String, DataType>,
}

#[derive(Debug, Clone)]
pub enum DataType {
    String,
    Integer,
    Float,
    Boolean,
    Date,
    DateTime,
}
```

## Error Types

```rust
#[derive(Debug, thiserror::Error)]
pub enum DataSourceError {
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),
    
    #[error("Not connected to data source")]
    NotConnected,
    
    #[error("Query failed: {0}")]
    QueryFailed(String),
    
    #[error("Invalid query: {0}")]
    InvalidQuery(String),
    
    #[error("Authentication failed: {0}")]
    AuthenticationFailed(String),
    
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    
    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),
}
```

## Best Practices

### Performance
- Use connection pooling for database sources
- Implement streaming for large data sets
- Cache frequently accessed data when appropriate
- Use pagination for large result sets

### Error Handling
- Provide detailed error messages with context
- Implement retry logic for transient failures
- Handle connection timeouts gracefully
- Validate queries before execution

### Security
- Use parameterized queries to prevent injection attacks
- Store credentials securely
- Implement proper authentication and authorization
- Validate data source configurations

### Resource Management
- Clean up connections and resources properly
- Implement connection health checks
- Use appropriate timeouts for operations
- Monitor resource usage and performance