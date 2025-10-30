use crate::event::{Event, EventError, EventFilter};
use async_trait::async_trait;
use chrono::{DateTime, Utc};

/// Event persistence trait for storing and retrieving events
#[async_trait]
pub trait EventPersistence: Send + Sync {
    async fn store_event(&self, event: &Event) -> Result<(), EventError>;
    async fn get_events(&self, filter: &EventFilter, limit: Option<usize>) -> Result<Vec<Event>, EventError>;
    async fn get_events_by_time_range(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        limit: Option<usize>,
    ) -> Result<Vec<Event>, EventError>;
    async fn replay_events(&self, from_event_id: Option<String>) -> Result<Vec<Event>, EventError>;
}
use serde::{Deserialize, Serialize};
use sled::{Db, Tree};
use std::sync::Arc;

/// Sled-based event persistence implementation
pub struct SledEventPersistence {
    db: Arc<Db>,
    events_tree: Tree,
    index_tree: Tree,
}

#[derive(Debug, Serialize, Deserialize)]
struct StoredEvent {
    event: Event,
    stored_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
struct EventIndex {
    event_id: String,
    timestamp: DateTime<Utc>,
    event_type: String,
    source: String,
    tags: Vec<String>,
}

impl SledEventPersistence {
    pub fn new(db_path: &str) -> Result<Self, EventError> {
        let db = sled::open(db_path)
            .map_err(|e| EventError::PersistenceError(format!("Failed to open database: {}", e)))?;
        
        let events_tree = db
            .open_tree("events")
            .map_err(|e| EventError::PersistenceError(format!("Failed to open events tree: {}", e)))?;
        
        let index_tree = db
            .open_tree("event_index")
            .map_err(|e| EventError::PersistenceError(format!("Failed to open index tree: {}", e)))?;

        Ok(Self {
            db: Arc::new(db),
            events_tree,
            index_tree,
        })
    }

    fn generate_event_key(&self, timestamp: DateTime<Utc>, event_id: &str) -> String {
        format!("{}_{}", timestamp.timestamp_nanos(), event_id)
    }

    fn parse_event_key(&self, key: &str) -> Option<(i64, String)> {
        let parts: Vec<&str> = key.splitn(2, '_').collect();
        if parts.len() == 2 {
            if let Ok(timestamp) = parts[0].parse::<i64>() {
                return Some((timestamp, parts[1].to_string()));
            }
        }
        None
    }
}

#[async_trait]
impl EventPersistence for SledEventPersistence {
    async fn store_event(&self, event: &Event) -> Result<(), EventError> {
        let metadata = event.metadata();
        let stored_event = StoredEvent {
            event: event.clone(),
            stored_at: Utc::now(),
        };

        // Serialize the event
        let event_data = serde_json::to_vec(&stored_event)?;
        let key = self.generate_event_key(metadata.timestamp, &metadata.event_id);

        // Store the event
        self.events_tree
            .insert(key.as_bytes(), event_data)
            .map_err(|e| EventError::PersistenceError(format!("Failed to store event: {}", e)))?;

        // Create index entry
        let index_entry = EventIndex {
            event_id: metadata.event_id.clone(),
            timestamp: metadata.timestamp,
            event_type: metadata.event_type,
            source: metadata.source,
            tags: metadata.tags,
        };

        let index_data = serde_json::to_vec(&index_entry)?;
        let index_key = format!("{}_{}", metadata.timestamp.timestamp_nanos(), metadata.event_id);
        
        self.index_tree
            .insert(index_key.as_bytes(), index_data)
            .map_err(|e| EventError::PersistenceError(format!("Failed to store index: {}", e)))?;

        // Flush to ensure durability
        self.db
            .flush()
            .map_err(|e| EventError::PersistenceError(format!("Failed to flush database: {}", e)))?;

        Ok(())
    }

    async fn get_events(&self, filter: &EventFilter, limit: Option<usize>) -> Result<Vec<Event>, EventError> {
        let mut events = Vec::new();
        let mut count = 0;
        let max_count = limit.unwrap_or(usize::MAX);

        // Iterate through events in reverse chronological order (newest first)
        for result in self.events_tree.iter().rev() {
            if count >= max_count {
                break;
            }

            let (key, value) = result
                .map_err(|e| EventError::PersistenceError(format!("Failed to read event: {}", e)))?;

            let stored_event: StoredEvent = serde_json::from_slice(&value)?;
            
            // Apply filter
            if filter.matches(&stored_event.event) {
                events.push(stored_event.event);
                count += 1;
            }
        }

        Ok(events)
    }

    async fn get_events_by_time_range(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        limit: Option<usize>,
    ) -> Result<Vec<Event>, EventError> {
        let mut events = Vec::new();
        let mut count = 0;
        let max_count = limit.unwrap_or(usize::MAX);

        let start_key = format!("{}_", start.timestamp_nanos());
        let end_key = format!("{}_", end.timestamp_nanos());

        // Range scan between start and end timestamps
        for result in self.events_tree.range(start_key.as_bytes()..end_key.as_bytes()) {
            if count >= max_count {
                break;
            }

            let (_, value) = result
                .map_err(|e| EventError::PersistenceError(format!("Failed to read event: {}", e)))?;

            let stored_event: StoredEvent = serde_json::from_slice(&value)?;
            events.push(stored_event.event);
            count += 1;
        }

        Ok(events)
    }

    async fn replay_events(&self, from_event_id: Option<String>) -> Result<Vec<Event>, EventError> {
        let mut events = Vec::new();
        let mut start_found = from_event_id.is_none();

        // Iterate through all events in chronological order
        for result in self.events_tree.iter() {
            let (key, value) = result
                .map_err(|e| EventError::PersistenceError(format!("Failed to read event: {}", e)))?;

            let stored_event: StoredEvent = serde_json::from_slice(&value)?;
            
            // If we have a starting event ID, find it first
            if let Some(ref start_id) = from_event_id {
                if !start_found {
                    if stored_event.event.metadata().event_id == *start_id {
                        start_found = true;
                        // Include the starting event in the replay
                        events.push(stored_event.event.clone());
                    }
                    continue;
                }
            }

            if start_found || from_event_id.is_none() {
                events.push(stored_event.event);
            }
        }

        if from_event_id.is_some() && !start_found {
            return Err(EventError::PersistenceError(
                "Starting event ID not found".to_string(),
            ));
        }

        Ok(events)
    }
}

/// In-memory event persistence for testing
pub struct InMemoryEventPersistence {
    events: Arc<tokio::sync::RwLock<Vec<StoredEvent>>>,
}

impl InMemoryEventPersistence {
    pub fn new() -> Self {
        Self {
            events: Arc::new(tokio::sync::RwLock::new(Vec::new())),
        }
    }
}

#[async_trait]
impl EventPersistence for InMemoryEventPersistence {
    async fn store_event(&self, event: &Event) -> Result<(), EventError> {
        let stored_event = StoredEvent {
            event: event.clone(),
            stored_at: Utc::now(),
        };

        let mut events = self.events.write().await;
        events.push(stored_event);
        Ok(())
    }

    async fn get_events(&self, filter: &EventFilter, limit: Option<usize>) -> Result<Vec<Event>, EventError> {
        let events = self.events.read().await;
        let mut filtered_events = Vec::new();
        let mut count = 0;
        let max_count = limit.unwrap_or(usize::MAX);

        // Iterate in reverse order (newest first)
        for stored_event in events.iter().rev() {
            if count >= max_count {
                break;
            }

            if filter.matches(&stored_event.event) {
                filtered_events.push(stored_event.event.clone());
                count += 1;
            }
        }

        Ok(filtered_events)
    }

    async fn get_events_by_time_range(
        &self,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
        limit: Option<usize>,
    ) -> Result<Vec<Event>, EventError> {
        let events = self.events.read().await;
        let mut filtered_events = Vec::new();
        let mut count = 0;
        let max_count = limit.unwrap_or(usize::MAX);

        for stored_event in events.iter() {
            if count >= max_count {
                break;
            }

            let event_time = stored_event.event.metadata().timestamp;
            if event_time >= start && event_time <= end {
                filtered_events.push(stored_event.event.clone());
                count += 1;
            }
        }

        Ok(filtered_events)
    }

    async fn replay_events(&self, from_event_id: Option<String>) -> Result<Vec<Event>, EventError> {
        let events = self.events.read().await;
        let mut replay_events = Vec::new();
        let mut start_found = from_event_id.is_none();

        for stored_event in events.iter() {
            if let Some(ref start_id) = from_event_id {
                if !start_found {
                    if stored_event.event.metadata().event_id == *start_id {
                        start_found = true;
                        // Include the starting event in the replay
                        replay_events.push(stored_event.event.clone());
                    }
                    continue;
                }
            }

            if start_found || from_event_id.is_none() {
                replay_events.push(stored_event.event.clone());
            }
        }

        if from_event_id.is_some() && !start_found {
            return Err(EventError::PersistenceError(
                "Starting event ID not found".to_string(),
            ));
        }

        Ok(replay_events)
    }
}