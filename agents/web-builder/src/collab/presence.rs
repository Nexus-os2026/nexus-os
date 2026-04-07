//! Presence — tracks what each collaborator is viewing/editing.
//!
//! Presence is at the SECTION level (which section each user is editing),
//! not pixel-level cursors. Updates are broadcast via the awareness protocol.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ─── Types ────────────────────────────────────────────────────────────────

/// A single user's presence state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PresenceState {
    pub public_key: String,
    pub display_name: String,
    pub color: String,
    pub selected_section: Option<String>,
    pub active_panel: Option<String>,
    pub last_seen: String,
}

/// Aggregated presence for all users in a session.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PresenceMap {
    pub users: HashMap<String, PresenceState>,
}

// ─── Operations ───────────────────────────────────────────────────────────

impl PresenceMap {
    /// Update a user's presence.
    pub fn update(
        &mut self,
        public_key: &str,
        display_name: &str,
        color: &str,
        section: Option<&str>,
        panel: Option<&str>,
    ) {
        self.users.insert(
            public_key.to_string(),
            PresenceState {
                public_key: public_key.to_string(),
                display_name: display_name.to_string(),
                color: color.to_string(),
                selected_section: section.map(String::from),
                active_panel: panel.map(String::from),
                last_seen: crate::deploy::now_iso8601(),
            },
        );
    }

    /// Remove a user's presence (on disconnect).
    pub fn remove(&mut self, public_key: &str) {
        self.users.remove(public_key);
    }

    /// Get all users currently viewing/editing a specific section.
    pub fn users_in_section(&self, section_id: &str) -> Vec<&PresenceState> {
        self.users
            .values()
            .filter(|p| p.selected_section.as_deref() == Some(section_id))
            .collect()
    }

    /// Get count of active users.
    pub fn active_count(&self) -> usize {
        self.users.len()
    }

    /// Get all presence states as a vec for serialization.
    pub fn all_users(&self) -> Vec<&PresenceState> {
        self.users.values().collect()
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_update_presence() {
        let mut map = PresenceMap::default();
        map.update("key1", "Alice", "#3b82f6", Some("hero"), None);
        assert_eq!(map.active_count(), 1);
        assert_eq!(map.users["key1"].display_name, "Alice");
        assert_eq!(map.users["key1"].selected_section, Some("hero".into()));
    }

    #[test]
    fn test_remove_presence() {
        let mut map = PresenceMap::default();
        map.update("key1", "Alice", "#3b82f6", Some("hero"), None);
        map.update("key2", "Bob", "#ef4444", Some("pricing"), None);
        assert_eq!(map.active_count(), 2);

        map.remove("key1");
        assert_eq!(map.active_count(), 1);
        assert!(!map.users.contains_key("key1"));
    }

    #[test]
    fn test_users_in_section() {
        let mut map = PresenceMap::default();
        map.update("key1", "Alice", "#3b82f6", Some("hero"), None);
        map.update("key2", "Bob", "#ef4444", Some("hero"), None);
        map.update("key3", "Carol", "#22c55e", Some("pricing"), None);

        let hero_users = map.users_in_section("hero");
        assert_eq!(hero_users.len(), 2);

        let pricing_users = map.users_in_section("pricing");
        assert_eq!(pricing_users.len(), 1);
    }

    #[test]
    fn test_update_replaces_previous() {
        let mut map = PresenceMap::default();
        map.update("key1", "Alice", "#3b82f6", Some("hero"), None);
        map.update("key1", "Alice", "#3b82f6", Some("pricing"), None);
        assert_eq!(map.active_count(), 1);
        assert_eq!(map.users["key1"].selected_section, Some("pricing".into()));
    }

    #[test]
    fn test_all_users() {
        let mut map = PresenceMap::default();
        map.update("key1", "Alice", "#3b82f6", None, None);
        map.update("key2", "Bob", "#ef4444", None, None);
        assert_eq!(map.all_users().len(), 2);
    }
}
