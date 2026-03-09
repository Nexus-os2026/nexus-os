use crate::AdaptationError;
use nexus_kernel::audit::{AuditTrail, EventType};
use nexus_kernel::privacy::{EncryptedField, PrivacyManager, UserKey};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Weekday {
    Monday,
    Tuesday,
    Wednesday,
    Thursday,
    Friday,
    Saturday,
    Sunday,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApprovalRecord {
    pub approved_at: u64,
    pub weekday: Weekday,
    pub content_style: String,
    pub posting_time: String,
    pub platform: String,
    pub tone: String,
    pub topic: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct UserPreferences {
    pub content_style: Option<String>,
    pub posting_times: Vec<String>,
    pub platforms: Vec<String>,
    pub tone: Option<String>,
    pub topics: Vec<String>,
    pub updated_at: u64,
}

impl UserPreferences {
    pub fn normalize(&mut self) {
        normalize_vec(&mut self.posting_times);
        normalize_vec(&mut self.platforms);
        normalize_vec(&mut self.topics);
        if let Some(tone) = self.tone.as_mut() {
            *tone = tone.trim().to_string();
            if tone.is_empty() {
                self.tone = None;
            }
        }
        if let Some(style) = self.content_style.as_mut() {
            *style = style.trim().to_string();
            if style.is_empty() {
                self.content_style = None;
            }
        }
    }
}

#[derive(Debug)]
pub struct PreferenceStore {
    privacy: PrivacyManager,
    encrypted_by_user: HashMap<String, EncryptedField>,
    storage_dir: Option<PathBuf>,
    audit_trail: AuditTrail,
}

impl PreferenceStore {
    pub fn new() -> Self {
        Self {
            privacy: PrivacyManager::new(),
            encrypted_by_user: HashMap::new(),
            storage_dir: None,
            audit_trail: AuditTrail::new(),
        }
    }

    pub fn with_storage_dir(storage_dir: impl Into<PathBuf>) -> Result<Self, AdaptationError> {
        let storage_dir = storage_dir.into();
        fs::create_dir_all(&storage_dir).map_err(|error| {
            AdaptationError::PreferencesError(format!(
                "failed to create storage directory: {error}"
            ))
        })?;

        Ok(Self {
            privacy: PrivacyManager::new(),
            encrypted_by_user: HashMap::new(),
            storage_dir: Some(storage_dir),
            audit_trail: AuditTrail::new(),
        })
    }

    pub fn get_preferences(
        &mut self,
        user_id: &str,
        user_key: &UserKey,
    ) -> Result<Option<UserPreferences>, AdaptationError> {
        if !self.encrypted_by_user.contains_key(user_id) {
            let _ = self.try_load_from_disk(user_id)?;
        }

        let Some(encrypted) = self.encrypted_by_user.get(user_id) else {
            return Ok(None);
        };

        let decrypted = self
            .privacy
            .decrypt_field(encrypted, user_key)
            .map_err(AdaptationError::from)?;

        let preferences: UserPreferences =
            serde_json::from_slice(decrypted.as_slice()).map_err(|error| {
                AdaptationError::PreferencesError(format!(
                    "failed to deserialize preferences: {error}"
                ))
            })?;

        Ok(Some(preferences))
    }

    pub fn set_preferences(
        &mut self,
        user_id: &str,
        mut preferences: UserPreferences,
        user_key: &UserKey,
    ) -> Result<(), AdaptationError> {
        preferences.updated_at = current_unix_timestamp();
        preferences.normalize();

        let serialized = serde_json::to_vec(&preferences).map_err(|error| {
            AdaptationError::PreferencesError(format!("failed to serialize preferences: {error}"))
        })?;

        let encrypted = self
            .privacy
            .encrypt_field(serialized.as_slice(), user_key)
            .map_err(AdaptationError::from)?;

        self.encrypted_by_user
            .insert(user_id.to_string(), encrypted.clone());
        self.persist_to_disk(user_id, &encrypted)?;

        self.audit_trail.append_event(
            uuid::Uuid::nil(),
            EventType::UserAction,
            json!({
                "event": "preferences_updated",
                "user_id": user_id,
                "updated_at": preferences.updated_at
            }),
        )?;

        Ok(())
    }

    pub fn learn_from_history(
        &mut self,
        user_id: &str,
        past_approvals: &[ApprovalRecord],
        user_key: &UserKey,
    ) -> Result<UserPreferences, AdaptationError> {
        if past_approvals.is_empty() {
            return Err(AdaptationError::PreferencesError(
                "cannot learn preferences from empty approval history".to_string(),
            ));
        }

        let mut style_counts = HashMap::<String, u64>::new();
        let mut tone_counts = HashMap::<String, u64>::new();
        let mut time_counts = HashMap::<String, u64>::new();
        let mut platform_counts = HashMap::<String, u64>::new();
        let mut topic_counts = HashMap::<String, u64>::new();

        for approval in past_approvals {
            increment_count(&mut style_counts, approval.content_style.as_str());
            increment_count(&mut tone_counts, approval.tone.as_str());
            increment_count(&mut time_counts, approval.posting_time.as_str());
            increment_count(&mut platform_counts, approval.platform.as_str());
            increment_count(&mut topic_counts, approval.topic.as_str());
        }

        let preferences = UserPreferences {
            content_style: select_top(&style_counts),
            posting_times: select_top_n(&time_counts, 3),
            platforms: select_top_n(&platform_counts, 3),
            tone: select_top(&tone_counts),
            topics: select_top_n(&topic_counts, 5),
            updated_at: current_unix_timestamp(),
        };

        self.set_preferences(user_id, preferences.clone(), user_key)?;

        self.audit_trail.append_event(
            uuid::Uuid::nil(),
            EventType::ToolCall,
            json!({
                "event": "preferences_learned",
                "user_id": user_id,
                "sample_size": past_approvals.len(),
                "tone": preferences.tone,
                "content_style": preferences.content_style
            }),
        )?;

        Ok(preferences)
    }

    pub fn delete_preferences(&mut self, user_id: &str) -> Result<(), AdaptationError> {
        self.encrypted_by_user.remove(user_id);
        self.delete_from_disk(user_id)?;

        self.audit_trail.append_event(
            uuid::Uuid::nil(),
            EventType::UserAction,
            json!({
                "event": "preferences_deleted",
                "user_id": user_id
            }),
        )?;

        Ok(())
    }

    pub fn list_users(&self) -> Vec<String> {
        let mut users = self
            .encrypted_by_user
            .keys()
            .cloned()
            .collect::<Vec<String>>();
        users.sort();
        users
    }

    pub fn audit_trail(&self) -> &AuditTrail {
        &self.audit_trail
    }

    fn persist_to_disk(
        &self,
        user_id: &str,
        encrypted: &EncryptedField,
    ) -> Result<(), AdaptationError> {
        let Some(dir) = self.storage_dir.as_ref() else {
            return Ok(());
        };

        let path = preference_file_path(dir, user_id);
        let data = serde_json::to_vec_pretty(encrypted).map_err(|error| {
            AdaptationError::PreferencesError(format!(
                "failed to serialize encrypted blob: {error}"
            ))
        })?;

        fs::write(path, data).map_err(|error| {
            AdaptationError::PreferencesError(format!("failed to write preference file: {error}"))
        })
    }

    fn try_load_from_disk(&mut self, user_id: &str) -> Result<bool, AdaptationError> {
        let Some(dir) = self.storage_dir.as_ref() else {
            return Ok(false);
        };

        let path = preference_file_path(dir, user_id);
        if !path.exists() {
            return Ok(false);
        }

        let raw = fs::read(path).map_err(|error| {
            AdaptationError::PreferencesError(format!("failed to read preference file: {error}"))
        })?;

        let encrypted: EncryptedField =
            serde_json::from_slice(raw.as_slice()).map_err(|error| {
                AdaptationError::PreferencesError(format!(
                    "failed to parse preference file: {error}"
                ))
            })?;

        self.encrypted_by_user
            .insert(user_id.to_string(), encrypted);
        Ok(true)
    }

    fn delete_from_disk(&self, user_id: &str) -> Result<(), AdaptationError> {
        let Some(dir) = self.storage_dir.as_ref() else {
            return Ok(());
        };

        let path = preference_file_path(dir, user_id);
        if !path.exists() {
            return Ok(());
        }

        fs::remove_file(path).map_err(|error| {
            AdaptationError::PreferencesError(format!("failed to delete preference file: {error}"))
        })
    }
}

impl Default for PreferenceStore {
    fn default() -> Self {
        Self::new()
    }
}

fn normalize_vec(values: &mut Vec<String>) {
    for value in values.iter_mut() {
        *value = value.trim().to_lowercase();
    }
    values.retain(|value| !value.is_empty());
    values.sort();
    values.dedup();
}

fn increment_count(counts: &mut HashMap<String, u64>, value: &str) {
    let normalized = value.trim().to_lowercase();
    if normalized.is_empty() {
        return;
    }

    let entry = counts.entry(normalized).or_insert(0);
    *entry = entry.saturating_add(1);
}

fn select_top(counts: &HashMap<String, u64>) -> Option<String> {
    let mut rows = counts
        .iter()
        .map(|(value, count)| (value.clone(), *count))
        .collect::<Vec<_>>();
    rows.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
    rows.first().map(|(value, _)| value.clone())
}

fn select_top_n(counts: &HashMap<String, u64>, n: usize) -> Vec<String> {
    let mut rows = counts
        .iter()
        .map(|(value, count)| (value.clone(), *count))
        .collect::<Vec<_>>();
    rows.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
    rows.into_iter().take(n).map(|(value, _)| value).collect()
}

fn preference_file_path(dir: &Path, user_id: &str) -> PathBuf {
    let file_safe = user_id
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>();
    dir.join(format!("{file_safe}.prefs.enc.json"))
}

fn current_unix_timestamp() -> u64 {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => duration.as_secs(),
        Err(_) => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::{ApprovalRecord, PreferenceStore, UserPreferences, Weekday};
    use nexus_kernel::privacy::UserKey;

    fn test_key() -> UserKey {
        UserKey {
            id: "user-key-1".to_string(),
            bytes: [9_u8; 32],
        }
    }

    #[test]
    fn test_preference_learning() {
        let key = test_key();
        let mut store = PreferenceStore::new();

        let approvals = (0..5)
            .map(|idx| ApprovalRecord {
                approved_at: 1_000 + idx,
                weekday: Weekday::Monday,
                content_style: "educational".to_string(),
                posting_time: "9am".to_string(),
                platform: "x".to_string(),
                tone: "professional".to_string(),
                topic: "rust".to_string(),
            })
            .collect::<Vec<_>>();

        let learned = store.learn_from_history("user-1", approvals.as_slice(), &key);
        assert!(learned.is_ok());

        if let Ok(preferences) = learned {
            assert_eq!(preferences.tone, Some("professional".to_string()));
            let loaded = store.get_preferences("user-1", &key);
            assert!(loaded.is_ok());
            if let Ok(Some(loaded)) = loaded {
                assert_eq!(loaded.tone, Some("professional".to_string()));
            } else {
                panic!("preferences should be present after learning");
            }
        }
    }

    #[test]
    fn test_preference_deletion() {
        let key = test_key();
        let mut store = PreferenceStore::new();
        let set = store.set_preferences(
            "user-2",
            UserPreferences {
                content_style: Some("tutorial".to_string()),
                posting_times: vec!["9am".to_string()],
                platforms: vec!["x".to_string()],
                tone: Some("friendly".to_string()),
                topics: vec!["rust".to_string()],
                updated_at: 0,
            },
            &key,
        );
        assert!(set.is_ok());

        let deleted = store.delete_preferences("user-2");
        assert!(deleted.is_ok());
        assert!(store.list_users().is_empty());

        let has_delete_event = store.audit_trail().events().iter().any(|event| {
            event.payload.get("event").and_then(|value| value.as_str())
                == Some("preferences_deleted")
        });
        assert!(has_delete_event);
    }
}
