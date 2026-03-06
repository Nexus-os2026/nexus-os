//! Shared blackboard with per-agent access control for collaborative state.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use uuid::Uuid;

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AccessLevel {
    ReadOnly,
    ReadWrite,
    Owner,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlackboardEntry {
    pub key: String,
    pub value: Value,
    pub owner: Uuid,
    pub created_at: u64,
    pub updated_at: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BlackboardError {
    AccessDenied,
    KeyNotFound,
}

impl std::fmt::Display for BlackboardError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AccessDenied => write!(f, "access denied"),
            Self::KeyNotFound => write!(f, "key not found"),
        }
    }
}

#[derive(Debug)]
pub struct Blackboard {
    entries: HashMap<String, BlackboardEntry>,
    acl: HashMap<(Uuid, String), AccessLevel>,
}

impl Blackboard {
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
            acl: HashMap::new(),
        }
    }

    pub fn grant_access(&mut self, agent_id: Uuid, key: &str, level: AccessLevel) {
        self.acl
            .insert((agent_id, key.to_string()), level);
    }

    fn get_access(&self, agent_id: Uuid, key: &str) -> Option<AccessLevel> {
        self.acl.get(&(agent_id, key.to_string())).copied()
    }

    /// Write a value. Requires Owner or ReadWrite access.
    /// If the key doesn't exist, creates it (agent becomes implicit owner via ACL).
    pub fn write(
        &mut self,
        agent_id: Uuid,
        key: &str,
        value: Value,
    ) -> Result<(), BlackboardError> {
        let access = self.get_access(agent_id, key);
        match access {
            Some(AccessLevel::Owner) | Some(AccessLevel::ReadWrite) => {}
            _ => return Err(BlackboardError::AccessDenied),
        }

        let now = unix_now();
        if let Some(entry) = self.entries.get_mut(key) {
            entry.value = value;
            entry.updated_at = now;
        } else {
            self.entries.insert(
                key.to_string(),
                BlackboardEntry {
                    key: key.to_string(),
                    value,
                    owner: agent_id,
                    created_at: now,
                    updated_at: now,
                },
            );
        }
        Ok(())
    }

    /// Read a value. Requires any access level.
    pub fn read(&self, agent_id: Uuid, key: &str) -> Result<&Value, BlackboardError> {
        if self.get_access(agent_id, key).is_none() {
            return Err(BlackboardError::AccessDenied);
        }
        self.entries
            .get(key)
            .map(|e| &e.value)
            .ok_or(BlackboardError::KeyNotFound)
    }

    /// Delete a key. Requires Owner access.
    pub fn delete(&mut self, agent_id: Uuid, key: &str) -> Result<(), BlackboardError> {
        match self.get_access(agent_id, key) {
            Some(AccessLevel::Owner) => {}
            Some(_) => return Err(BlackboardError::AccessDenied),
            None => return Err(BlackboardError::AccessDenied),
        }
        if self.entries.remove(key).is_some() {
            Ok(())
        } else {
            Err(BlackboardError::KeyNotFound)
        }
    }

    /// List keys the agent has any access to.
    pub fn list_keys(&self, agent_id: Uuid) -> Vec<String> {
        self.acl
            .keys()
            .filter(|(id, _)| *id == agent_id)
            .map(|(_, key)| key.clone())
            .collect()
    }
}

impl Default for Blackboard {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn owner_can_write_read_delete() {
        let mut bb = Blackboard::new();
        let owner = Uuid::new_v4();

        bb.grant_access(owner, "config", AccessLevel::Owner);
        bb.write(owner, "config", json!({"debug": true})).unwrap();

        let val = bb.read(owner, "config").unwrap();
        assert_eq!(val, &json!({"debug": true}));

        bb.delete(owner, "config").unwrap();
        assert_eq!(bb.read(owner, "config"), Err(BlackboardError::KeyNotFound));
    }

    #[test]
    fn readwrite_can_write_and_read_but_not_delete() {
        let mut bb = Blackboard::new();
        let owner = Uuid::new_v4();
        let writer = Uuid::new_v4();

        bb.grant_access(owner, "data", AccessLevel::Owner);
        bb.grant_access(writer, "data", AccessLevel::ReadWrite);

        bb.write(owner, "data", json!("initial")).unwrap();

        // Writer can read
        assert_eq!(bb.read(writer, "data").unwrap(), &json!("initial"));

        // Writer can write (overwrite)
        bb.write(writer, "data", json!("updated")).unwrap();
        assert_eq!(bb.read(owner, "data").unwrap(), &json!("updated"));

        // Writer cannot delete
        assert_eq!(bb.delete(writer, "data"), Err(BlackboardError::AccessDenied));
    }

    #[test]
    fn readonly_can_only_read() {
        let mut bb = Blackboard::new();
        let owner = Uuid::new_v4();
        let reader = Uuid::new_v4();

        bb.grant_access(owner, "secret", AccessLevel::Owner);
        bb.grant_access(reader, "secret", AccessLevel::ReadOnly);

        bb.write(owner, "secret", json!(42)).unwrap();

        // Reader can read
        assert_eq!(bb.read(reader, "secret").unwrap(), &json!(42));

        // Reader cannot write
        assert_eq!(
            bb.write(reader, "secret", json!(0)),
            Err(BlackboardError::AccessDenied)
        );

        // Reader cannot delete
        assert_eq!(
            bb.delete(reader, "secret"),
            Err(BlackboardError::AccessDenied)
        );
    }

    #[test]
    fn no_access_returns_error() {
        let mut bb = Blackboard::new();
        let owner = Uuid::new_v4();
        let stranger = Uuid::new_v4();

        bb.grant_access(owner, "private", AccessLevel::Owner);
        bb.write(owner, "private", json!("hidden")).unwrap();

        assert_eq!(
            bb.read(stranger, "private"),
            Err(BlackboardError::AccessDenied)
        );
        assert_eq!(
            bb.write(stranger, "private", json!("hacked")),
            Err(BlackboardError::AccessDenied)
        );
        assert_eq!(
            bb.delete(stranger, "private"),
            Err(BlackboardError::AccessDenied)
        );
    }

    #[test]
    fn list_keys_returns_accessible_keys() {
        let mut bb = Blackboard::new();
        let agent = Uuid::new_v4();
        let other = Uuid::new_v4();

        bb.grant_access(agent, "key_a", AccessLevel::Owner);
        bb.grant_access(agent, "key_b", AccessLevel::ReadOnly);
        bb.grant_access(other, "key_c", AccessLevel::Owner);

        let mut keys = bb.list_keys(agent);
        keys.sort();
        assert_eq!(keys, vec!["key_a", "key_b"]);

        let other_keys = bb.list_keys(other);
        assert_eq!(other_keys, vec!["key_c"]);
    }

    #[test]
    fn write_updates_existing_entry() {
        let mut bb = Blackboard::new();
        let owner = Uuid::new_v4();

        bb.grant_access(owner, "counter", AccessLevel::Owner);
        bb.write(owner, "counter", json!(1)).unwrap();
        bb.write(owner, "counter", json!(2)).unwrap();

        assert_eq!(bb.read(owner, "counter").unwrap(), &json!(2));
    }

    #[test]
    fn delete_nonexistent_key_returns_not_found() {
        let mut bb = Blackboard::new();
        let owner = Uuid::new_v4();
        bb.grant_access(owner, "ghost", AccessLevel::Owner);
        assert_eq!(bb.delete(owner, "ghost"), Err(BlackboardError::KeyNotFound));
    }
}
