use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// State snapshot for rollback support.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateSnapshot {
    pub snapshot_id: String,
    pub scenario_id: String,
    pub virtual_fs: HashMap<String, String>,
    pub timestamp: u64,
}

/// Manages snapshots for rollback.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RollbackManager {
    snapshots: Vec<StateSnapshot>,
    max_snapshots: usize,
}

impl RollbackManager {
    pub fn new(max_snapshots: usize) -> Self {
        Self {
            snapshots: Vec::new(),
            max_snapshots,
        }
    }

    /// Take a snapshot of current sandbox state.
    pub fn snapshot(&mut self, scenario_id: &str, virtual_fs: &HashMap<String, String>) -> String {
        let id = uuid::Uuid::new_v4().to_string();
        let snapshot = StateSnapshot {
            snapshot_id: id.clone(),
            scenario_id: scenario_id.into(),
            virtual_fs: virtual_fs.clone(),
            timestamp: epoch_secs(),
        };

        self.snapshots.push(snapshot);

        while self.snapshots.len() > self.max_snapshots {
            self.snapshots.remove(0);
        }

        id
    }

    /// Restore a snapshot.
    pub fn restore(&self, snapshot_id: &str) -> Option<&HashMap<String, String>> {
        self.snapshots
            .iter()
            .find(|s| s.snapshot_id == snapshot_id)
            .map(|s| &s.virtual_fs)
    }

    pub fn snapshots(&self) -> &[StateSnapshot] {
        &self.snapshots
    }
}

impl Default for RollbackManager {
    fn default() -> Self {
        Self::new(20)
    }
}

fn epoch_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rollback_snapshot_restore() {
        let mut mgr = RollbackManager::new(10);
        let mut fs = HashMap::new();
        fs.insert("/tmp/a.txt".into(), "hello".into());
        fs.insert("/tmp/b.txt".into(), "world".into());

        let snap_id = mgr.snapshot("scenario-1", &fs);

        // Modify fs
        let mut fs2 = fs.clone();
        fs2.insert("/tmp/c.txt".into(), "new file".into());
        fs2.remove("/tmp/a.txt");

        // Restore original
        let restored = mgr.restore(&snap_id).unwrap();
        assert_eq!(restored.len(), 2);
        assert_eq!(restored.get("/tmp/a.txt").unwrap(), "hello");
        assert!(!restored.contains_key("/tmp/c.txt"));
    }
}
