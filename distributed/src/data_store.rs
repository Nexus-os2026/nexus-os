//! Trait-based data store abstraction for desktop (SQLite) vs server (PostgreSQL) mode.
//!
//! All data access flows through the `DataStore` trait, allowing the runtime to
//! swap backends without changing business logic.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Unified data store error.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DataStoreError {
    ConnectionFailed(String),
    QueryFailed(String),
    NotFound(String),
    Conflict(String),
    SerializationError(String),
}

impl std::fmt::Display for DataStoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ConnectionFailed(e) => write!(f, "connection failed: {e}"),
            Self::QueryFailed(e) => write!(f, "query failed: {e}"),
            Self::NotFound(e) => write!(f, "not found: {e}"),
            Self::Conflict(e) => write!(f, "conflict: {e}"),
            Self::SerializationError(e) => write!(f, "serialization error: {e}"),
        }
    }
}

// ---------------------------------------------------------------------------
// Audit types (subset for the trait boundary)
// ---------------------------------------------------------------------------

/// A portable audit entry for cross-backend storage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    pub id: String,
    pub timestamp: u64,
    pub agent_id: String,
    pub event_type: String,
    pub payload: serde_json::Value,
    pub hash: String,
    pub instance_id: String,
}

/// Filter for audit queries.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AuditFilter {
    pub agent_id: Option<String>,
    pub event_type: Option<String>,
    pub since: Option<u64>,
    pub until: Option<u64>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

// ---------------------------------------------------------------------------
// DataStore trait
// ---------------------------------------------------------------------------

/// Abstraction over the persistence backend (SQLite for desktop, PostgreSQL for
/// server/HA deployments).
///
/// Implementations must be `Send + Sync` so they can be shared across async tasks.
pub trait DataStore: Send + Sync {
    /// Write an audit event.
    fn write_audit(&self, entry: &AuditEntry) -> Result<(), DataStoreError>;

    /// Query audit events with filtering.
    fn query_audit(&self, filter: &AuditFilter) -> Result<Vec<AuditEntry>, DataStoreError>;

    /// Store a key-value pair (for config, state, leases, etc.).
    fn put(&self, namespace: &str, key: &str, value: &str) -> Result<(), DataStoreError>;

    /// Get a value by namespace and key.
    fn get(&self, namespace: &str, key: &str) -> Result<Option<String>, DataStoreError>;

    /// Delete a key.
    fn delete(&self, namespace: &str, key: &str) -> Result<bool, DataStoreError>;

    /// List all keys in a namespace.
    fn list_keys(&self, namespace: &str) -> Result<Vec<String>, DataStoreError>;

    /// Health check — returns true if the backend is reachable.
    fn is_healthy(&self) -> bool;

    /// Backend name for diagnostics.
    fn backend_name(&self) -> &str;
}

// ---------------------------------------------------------------------------
// In-memory implementation (for testing and desktop fallback)
// ---------------------------------------------------------------------------

/// In-memory data store for testing and single-node desktop deployments.
#[derive(Debug)]
pub struct InMemoryStore {
    audit: std::sync::Mutex<Vec<AuditEntry>>,
    kv: std::sync::Mutex<HashMap<String, HashMap<String, String>>>,
}

impl InMemoryStore {
    pub fn new() -> Self {
        Self {
            audit: std::sync::Mutex::new(Vec::new()),
            kv: std::sync::Mutex::new(HashMap::new()),
        }
    }
}

impl Default for InMemoryStore {
    fn default() -> Self {
        Self::new()
    }
}

impl DataStore for InMemoryStore {
    fn write_audit(&self, entry: &AuditEntry) -> Result<(), DataStoreError> {
        self.audit
            .lock()
            .map_err(|e| DataStoreError::QueryFailed(e.to_string()))?
            .push(entry.clone());
        Ok(())
    }

    fn query_audit(&self, filter: &AuditFilter) -> Result<Vec<AuditEntry>, DataStoreError> {
        let entries = self
            .audit
            .lock()
            .map_err(|e| DataStoreError::QueryFailed(e.to_string()))?;

        let mut results: Vec<AuditEntry> = entries
            .iter()
            .filter(|e| {
                if let Some(ref aid) = filter.agent_id {
                    if &e.agent_id != aid {
                        return false;
                    }
                }
                if let Some(ref et) = filter.event_type {
                    if &e.event_type != et {
                        return false;
                    }
                }
                if let Some(since) = filter.since {
                    if e.timestamp < since {
                        return false;
                    }
                }
                if let Some(until) = filter.until {
                    if e.timestamp > until {
                        return false;
                    }
                }
                true
            })
            .cloned()
            .collect();

        let offset = filter.offset.unwrap_or(0);
        let limit = filter.limit.unwrap_or(usize::MAX);
        results = results.into_iter().skip(offset).take(limit).collect();

        Ok(results)
    }

    fn put(&self, namespace: &str, key: &str, value: &str) -> Result<(), DataStoreError> {
        self.kv
            .lock()
            .map_err(|e| DataStoreError::QueryFailed(e.to_string()))?
            .entry(namespace.to_string())
            .or_default()
            .insert(key.to_string(), value.to_string());
        Ok(())
    }

    fn get(&self, namespace: &str, key: &str) -> Result<Option<String>, DataStoreError> {
        Ok(self
            .kv
            .lock()
            .map_err(|e| DataStoreError::QueryFailed(e.to_string()))?
            .get(namespace)
            .and_then(|ns| ns.get(key))
            .cloned())
    }

    fn delete(&self, namespace: &str, key: &str) -> Result<bool, DataStoreError> {
        Ok(self
            .kv
            .lock()
            .map_err(|e| DataStoreError::QueryFailed(e.to_string()))?
            .get_mut(namespace)
            .and_then(|ns| ns.remove(key))
            .is_some())
    }

    fn list_keys(&self, namespace: &str) -> Result<Vec<String>, DataStoreError> {
        Ok(self
            .kv
            .lock()
            .map_err(|e| DataStoreError::QueryFailed(e.to_string()))?
            .get(namespace)
            .map(|ns| ns.keys().cloned().collect())
            .unwrap_or_default())
    }

    fn is_healthy(&self) -> bool {
        true
    }

    fn backend_name(&self) -> &str {
        "in-memory"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_store() -> InMemoryStore {
        InMemoryStore::new()
    }

    #[test]
    fn kv_crud() {
        let store = test_store();
        store.put("config", "mode", "server").unwrap();
        assert_eq!(store.get("config", "mode").unwrap(), Some("server".into()));
        assert!(store.delete("config", "mode").unwrap());
        assert_eq!(store.get("config", "mode").unwrap(), None);
    }

    #[test]
    fn kv_list_keys() {
        let store = test_store();
        store.put("ns", "a", "1").unwrap();
        store.put("ns", "b", "2").unwrap();
        let mut keys = store.list_keys("ns").unwrap();
        keys.sort();
        assert_eq!(keys, vec!["a", "b"]);
    }

    #[test]
    fn audit_write_and_query() {
        let store = test_store();
        let entry = AuditEntry {
            id: "e1".into(),
            timestamp: 1000,
            agent_id: "agent-1".into(),
            event_type: "action".into(),
            payload: serde_json::json!({"key": "value"}),
            hash: "abc".into(),
            instance_id: "node-1".into(),
        };
        store.write_audit(&entry).unwrap();

        let results = store
            .query_audit(&AuditFilter {
                agent_id: Some("agent-1".into()),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "e1");
    }

    #[test]
    fn audit_filter_time_range() {
        let store = test_store();
        for i in 0..5 {
            store
                .write_audit(&AuditEntry {
                    id: format!("e{i}"),
                    timestamp: 100 + i * 10,
                    agent_id: "a1".into(),
                    event_type: "action".into(),
                    payload: serde_json::json!({}),
                    hash: String::new(),
                    instance_id: "node-1".into(),
                })
                .unwrap();
        }

        let results = store
            .query_audit(&AuditFilter {
                since: Some(120),
                until: Some(130),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(results.len(), 2); // timestamps 120 and 130
    }

    #[test]
    fn audit_pagination() {
        let store = test_store();
        for i in 0..10 {
            store
                .write_audit(&AuditEntry {
                    id: format!("e{i}"),
                    timestamp: i,
                    agent_id: "a1".into(),
                    event_type: "action".into(),
                    payload: serde_json::json!({}),
                    hash: String::new(),
                    instance_id: "node-1".into(),
                })
                .unwrap();
        }

        let page = store
            .query_audit(&AuditFilter {
                offset: Some(3),
                limit: Some(2),
                ..Default::default()
            })
            .unwrap();
        assert_eq!(page.len(), 2);
        assert_eq!(page[0].id, "e3");
        assert_eq!(page[1].id, "e4");
    }

    #[test]
    fn backend_is_healthy() {
        let store = test_store();
        assert!(store.is_healthy());
        assert_eq!(store.backend_name(), "in-memory");
    }
}
