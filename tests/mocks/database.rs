use super::*;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Mock database that simulates various database conditions
pub struct MockDatabaseWithConditions {
    data: Arc<Mutex<HashMap<String, Vec<HashMap<String, Value>>>>>,
    connection_info: ConnectionInfo,
    query_latency: Duration,
    failure_rate: f64,
    max_connections: usize,
    current_connections: Arc<Mutex<usize>>,
}

impl MockDatabaseWithConditions {
    pub fn new(connection_info: ConnectionInfo) -> Self {
        Self {
            data: Arc::new(Mutex::new(HashMap::new())),
            connection_info,
            query_latency: Duration::from_millis(10),
            failure_rate: 0.0,
            max_connections: 10,
            current_connections: Arc::new(Mutex::new(0)),
        }
    }
    
    pub fn with_query_latency(mut self, latency: Duration) -> Self {
        self.query_latency = latency;
        self
    }
    
    pub fn with_failure_rate(mut self, rate: f64) -> Self {
        self.failure_rate = rate.clamp(0.0, 1.0);
        self
    }
    
    pub fn with_max_connections(mut self, max: usize) -> Self {
        self.max_connections = max;
        self
    }
    
    pub async fn insert_test_data(&self, table: &str, rows: Vec<HashMap<String, Value>>) {
        self.data.lock().await.insert(table.to_string(), rows);
    }
    
    pub async fn get_table_data(&self, table: &str) -> Option<Vec<HashMap<String, Value>>> {
        self.data.lock().await.get(table).cloned()
    }
    
    pub async fn simulate_connection_exhaustion(&self) {
        let mut connections = self.current_connections.lock().await;
        *connections = self.max_connections;
    }
    
    pub async fn reset_connections(&self) {
        let mut connections = self.current_connections.lock().await;
        *connections = 0;
    }
    
    fn parse_simple_query(&self, query: &str) -> Result<(String, String), DatabaseError> {
        let query_lower = query.to_lowercase();
        
        if query_lower.starts_with("select") {
            if let Some(from_pos) = query_lower.find("from") {
                let table_part = &query[from_pos + 4..].trim();
                let table_name = table_part.split_whitespace().next()
                    .ok_or_else(|| DatabaseError::InvalidQuery("No table name found".to_string()))?;
                return Ok(("SELECT".to_string(), table_name.to_string()));
            }
        } else if query_lower.starts_with("insert") {
            if let Some(into_pos) = query_lower.find("into") {
                let table_part = &query[into_pos + 4..].trim();
                let table_name = table_part.split_whitespace().next()
                    .ok_or_else(|| DatabaseError::InvalidQuery("No table name found".to_string()))?;
                return Ok(("INSERT".to_string(), table_name.to_string()));
            }
        } else if query_lower.starts_with("update") {
            let table_name = query.split_whitespace().nth(1)
                .ok_or_else(|| DatabaseError::InvalidQuery("No table name found".to_string()))?;
            return Ok(("UPDATE".to_string(), table_name.to_string()));
        } else if query_lower.starts_with("delete") {
            if let Some(from_pos) = query_lower.find("from") {
                let table_part = &query[from_pos + 4..].trim();
                let table_name = table_part.split_whitespace().next()
                    .ok_or_else(|| DatabaseError::InvalidQuery("No table name found".to_string()))?;
                return Ok(("DELETE".to_string(), table_name.to_string()));
            }
        }
        
        Err(DatabaseError::InvalidQuery(format!("Unsupported query: {}", query)))
    }
}

#[async_trait]
impl DatabaseConnectionTrait for MockDatabaseWithConditions {
    async fn execute_query(&self, query: &str, params: Vec<Value>) -> Result<Vec<HashMap<String, Value>>, DatabaseError> {
        // Check connection limit
        {
            let mut connections = self.current_connections.lock().await;
            if *connections >= self.max_connections {
                return Err(DatabaseError::ConnectionFailed("Connection pool exhausted".to_string()));
            }
            *connections += 1;
        }
        
        // Simulate query latency
        tokio::time::sleep(self.query_latency).await;
        
        // Simulate random failures
        if fastrand::f64() < self.failure_rate {
            return Err(DatabaseError::QueryFailed("Random query failure".to_string()));
        }
        
        // Parse and execute query
        let (operation, table_name) = self.parse_simple_query(query)?;
        
        let result = match operation.as_str() {
            "SELECT" => {
                let data = self.data.lock().await;
                data.get(&table_name).cloned().unwrap_or_default()
            }
            "INSERT" => {
                // For INSERT, return empty result but could modify data
                let mut data = self.data.lock().await;
                let table_data = data.entry(table_name).or_insert_with(Vec::new);
                
                // Create a new row from parameters (simplified)
                let mut new_row = HashMap::new();
                for (i, param) in params.iter().enumerate() {
                    new_row.insert(format!("col_{}", i), param.clone());
                }
                table_data.push(new_row);
                
                vec![]
            }
            "UPDATE" => {
                // For UPDATE, return affected row count (simplified)
                vec![HashMap::from([
                    ("affected_rows".to_string(), Value::Number(1.into()))
                ])]
            }
            "DELETE" => {
                // For DELETE, return affected row count (simplified)
                let mut data = self.data.lock().await;
                let affected = data.get(&table_name).map(|rows| rows.len()).unwrap_or(0);
                data.remove(&table_name);
                
                vec![HashMap::from([
                    ("affected_rows".to_string(), Value::Number(affected.into()))
                ])]
            }
            _ => return Err(DatabaseError::InvalidQuery(format!("Unsupported operation: {}", operation)))
        };
        
        // Release connection
        {
            let mut connections = self.current_connections.lock().await;
            *connections = connections.saturating_sub(1);
        }
        
        Ok(result)
    }
    
    async fn execute_transaction(&self, queries: Vec<(String, Vec<Value>)>) -> Result<(), DatabaseError> {
        // Simulate transaction latency
        tokio::time::sleep(self.query_latency * queries.len() as u32).await;
        
        // Simulate transaction failures
        if fastrand::f64() < self.failure_rate {
            return Err(DatabaseError::TransactionFailed("Random transaction failure".to_string()));
        }
        
        // Execute all queries in sequence (simplified transaction)
        for (query, params) in queries {
            self.execute_query(&query, params).await?;
        }
        
        Ok(())
    }
    
    async fn test_connection(&self) -> Result<(), DatabaseError> {
        // Simulate connection test latency
        tokio::time::sleep(Duration::from_millis(50)).await;
        
        // Check if we can establish a connection
        let connections = self.current_connections.lock().await;
        if *connections >= self.max_connections {
            return Err(DatabaseError::ConnectionFailed("Too many connections".to_string()));
        }
        
        Ok(())
    }
    
    fn get_connection_info(&self) -> ConnectionInfo {
        self.connection_info.clone()
    }
}

/// Mock database that records all queries for verification
pub struct RecordingDatabase {
    queries: Arc<Mutex<Vec<(String, Vec<Value>)>>>,
    transactions: Arc<Mutex<Vec<Vec<(String, Vec<Value>)>>>>,
    connection_info: ConnectionInfo,
}

impl RecordingDatabase {
    pub fn new(connection_info: ConnectionInfo) -> Self {
        Self {
            queries: Arc::new(Mutex::new(Vec::new())),
            transactions: Arc::new(Mutex::new(Vec::new())),
            connection_info,
        }
    }
    
    pub async fn get_recorded_queries(&self) -> Vec<(String, Vec<Value>)> {
        self.queries.lock().await.clone()
    }
    
    pub async fn get_recorded_transactions(&self) -> Vec<Vec<(String, Vec<Value>)>> {
        self.transactions.lock().await.clone()
    }
    
    pub async fn clear_recorded_queries(&self) {
        self.queries.lock().await.clear();
        self.transactions.lock().await.clear();
    }
    
    pub async fn query_count(&self) -> usize {
        self.queries.lock().await.len()
    }
    
    pub async fn transaction_count(&self) -> usize {
        self.transactions.lock().await.len()
    }
}

#[async_trait]
impl DatabaseConnectionTrait for RecordingDatabase {
    async fn execute_query(&self, query: &str, params: Vec<Value>) -> Result<Vec<HashMap<String, Value>>, DatabaseError> {
        // Record the query
        self.queries.lock().await.push((query.to_string(), params));
        
        // Return mock result based on query type
        let query_lower = query.to_lowercase();
        if query_lower.starts_with("select") {
            Ok(vec![
                HashMap::from([
                    ("id".to_string(), Value::Number(1.into())),
                    ("name".to_string(), Value::String("Test Record".to_string())),
                ])
            ])
        } else {
            Ok(vec![])
        }
    }
    
    async fn execute_transaction(&self, queries: Vec<(String, Vec<Value>)>) -> Result<(), DatabaseError> {
        // Record the transaction
        self.transactions.lock().await.push(queries.clone());
        
        // Also record individual queries
        for (query, params) in queries {
            self.queries.lock().await.push((query, params));
        }
        
        Ok(())
    }
    
    async fn test_connection(&self) -> Result<(), DatabaseError> {
        Ok(())
    }
    
    fn get_connection_info(&self) -> ConnectionInfo {
        self.connection_info.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_mock_database_with_conditions() {
        let connection_info = ConnectionInfo {
            host: "localhost".to_string(),
            port: 5432,
            database: "testdb".to_string(),
            username: "testuser".to_string(),
        };
        
        let db = MockDatabaseWithConditions::new(connection_info)
            .with_query_latency(Duration::from_millis(10))
            .with_failure_rate(0.0);
        
        // Insert test data
        let test_data = vec![
            HashMap::from([
                ("id".to_string(), Value::Number(1.into())),
                ("name".to_string(), Value::String("User 1".to_string())),
            ]),
            HashMap::from([
                ("id".to_string(), Value::Number(2.into())),
                ("name".to_string(), Value::String("User 2".to_string())),
            ]),
        ];
        
        db.insert_test_data("users", test_data.clone()).await;
        
        // Test SELECT query
        let result = db.execute_query("SELECT * FROM users", vec![]).await.unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0]["name"], Value::String("User 1".to_string()));
    }
    
    #[tokio::test]
    async fn test_recording_database() {
        let connection_info = ConnectionInfo {
            host: "localhost".to_string(),
            port: 5432,
            database: "testdb".to_string(),
            username: "testuser".to_string(),
        };
        
        let db = RecordingDatabase::new(connection_info);
        
        // Execute some queries
        db.execute_query("SELECT * FROM users", vec![]).await.unwrap();
        db.execute_query("INSERT INTO users (name) VALUES (?)", vec![Value::String("New User".to_string())]).await.unwrap();
        
        // Execute a transaction
        let transaction_queries = vec![
            ("UPDATE users SET active = ? WHERE id = ?".to_string(), vec![Value::Bool(true), Value::Number(1.into())]),
            ("INSERT INTO audit_log (action) VALUES (?)".to_string(), vec![Value::String("user_updated".to_string())]),
        ];
        db.execute_transaction(transaction_queries.clone()).await.unwrap();
        
        // Verify recordings
        let recorded_queries = db.get_recorded_queries().await;
        assert_eq!(recorded_queries.len(), 4); // 2 individual + 2 from transaction
        
        let recorded_transactions = db.get_recorded_transactions().await;
        assert_eq!(recorded_transactions.len(), 1);
        assert_eq!(recorded_transactions[0].len(), 2);
    }
    
    #[tokio::test]
    async fn test_connection_exhaustion() {
        let connection_info = ConnectionInfo {
            host: "localhost".to_string(),
            port: 5432,
            database: "testdb".to_string(),
            username: "testuser".to_string(),
        };
        
        let db = MockDatabaseWithConditions::new(connection_info)
            .with_max_connections(2);
        
        // Simulate connection exhaustion
        db.simulate_connection_exhaustion().await;
        
        // Should fail due to connection limit
        let result = db.execute_query("SELECT 1", vec![]).await;
        assert!(result.is_err());
        
        // Reset connections
        db.reset_connections().await;
        
        // Should succeed now
        let result = db.execute_query("SELECT 1", vec![]).await;
        assert!(result.is_ok());
    }
}