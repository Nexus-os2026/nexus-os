//! Lease-based leader election for singleton tasks in HA deployments.
//!
//! In a multi-replica Nexus OS deployment certain operations (scheduled backups,
//! genome evolution, audit chain verification) must only run on a single instance.
//! This module provides a simple lease-based leader election mechanism that can be
//! backed by either a database or in-memory state.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// A lease record for a named leadership task.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Lease {
    /// The task being leased (e.g. "backup", "genome-evolution").
    pub task: String,
    /// The instance that currently holds the lease.
    pub holder: String,
    /// Lease acquisition time (unix seconds).
    pub acquired_at: u64,
    /// Lease expiry time (unix seconds).
    pub expires_at: u64,
    /// Monotonic version for compare-and-swap.
    pub version: u64,
}

impl Lease {
    /// Returns true if the lease has expired.
    pub fn is_expired(&self) -> bool {
        now_secs() >= self.expires_at
    }

    /// Returns the remaining TTL in seconds, or 0 if expired.
    pub fn remaining_secs(&self) -> u64 {
        self.expires_at.saturating_sub(now_secs())
    }
}

/// In-memory leader election store. In production this would be backed by
/// PostgreSQL or etcd, but the interface is identical.
#[derive(Debug)]
pub struct LeaderElection {
    instance_id: String,
    leases: HashMap<String, Lease>,
}

impl LeaderElection {
    /// Create a new leader election participant with the given instance ID.
    pub fn new(instance_id: impl Into<String>) -> Self {
        Self {
            instance_id: instance_id.into(),
            leases: HashMap::new(),
        }
    }

    /// Try to acquire the leader lease for `task` with the given TTL.
    ///
    /// Returns `true` if this instance now holds the lease (either freshly
    /// acquired or already held). Returns `false` if another instance holds a
    /// non-expired lease.
    pub fn try_acquire(&mut self, task: &str, ttl: Duration) -> bool {
        let now = now_secs();

        if let Some(existing) = self.leases.get(task) {
            if !existing.is_expired() && existing.holder != self.instance_id {
                // Someone else holds a valid lease.
                return false;
            }
        }

        let version = self.leases.get(task).map(|l| l.version + 1).unwrap_or(1);

        self.leases.insert(
            task.to_string(),
            Lease {
                task: task.to_string(),
                holder: self.instance_id.clone(),
                acquired_at: now,
                expires_at: now + ttl.as_secs(),
                version,
            },
        );
        true
    }

    /// Renew an existing lease held by this instance.
    ///
    /// Returns `true` if the lease was renewed, `false` if this instance does
    /// not currently hold the lease.
    pub fn renew(&mut self, task: &str, ttl: Duration) -> bool {
        let now = now_secs();

        if let Some(lease) = self.leases.get_mut(task) {
            if lease.holder == self.instance_id && !lease.is_expired() {
                lease.expires_at = now + ttl.as_secs();
                lease.version += 1;
                return true;
            }
        }
        false
    }

    /// Release the lease for `task` if held by this instance.
    ///
    /// Returns `true` if the lease was released.
    pub fn release(&mut self, task: &str) -> bool {
        if let Some(lease) = self.leases.get(task) {
            if lease.holder == self.instance_id {
                self.leases.remove(task);
                return true;
            }
        }
        false
    }

    /// Check if this instance is the leader for `task`.
    pub fn is_leader(&self, task: &str) -> bool {
        self.leases
            .get(task)
            .is_some_and(|lease| lease.holder == self.instance_id && !lease.is_expired())
    }

    /// Get the current lease for a task, if any.
    pub fn get_lease(&self, task: &str) -> Option<&Lease> {
        self.leases.get(task)
    }

    /// List all active (non-expired) leases.
    pub fn active_leases(&self) -> Vec<&Lease> {
        self.leases.values().filter(|l| !l.is_expired()).collect()
    }

    /// Return this participant's instance ID.
    pub fn instance_id(&self) -> &str {
        &self.instance_id
    }
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn acquire_and_release() {
        let mut le = LeaderElection::new("node-1");
        assert!(le.try_acquire("backup", Duration::from_secs(60)));
        assert!(le.is_leader("backup"));
        assert!(le.release("backup"));
        assert!(!le.is_leader("backup"));
    }

    #[test]
    fn contested_lease() {
        let mut node1 = LeaderElection::new("node-1");
        let mut node2 = LeaderElection::new("node-2");

        // node-1 acquires first.
        assert!(node1.try_acquire("backup", Duration::from_secs(3600)));

        // Simulate node-2 seeing the same lease store (copy the lease).
        node2.leases = node1.leases.clone();

        // node-2 cannot acquire while node-1 holds it.
        assert!(!node2.try_acquire("backup", Duration::from_secs(60)));
    }

    #[test]
    fn expired_lease_can_be_taken() {
        let mut node1 = LeaderElection::new("node-1");
        let mut node2 = LeaderElection::new("node-2");

        // node-1 acquires with 0-second TTL (immediately expired).
        assert!(node1.try_acquire("backup", Duration::from_secs(0)));

        // Copy state to node-2.
        node2.leases = node1.leases.clone();

        // node-2 can take the expired lease.
        assert!(node2.try_acquire("backup", Duration::from_secs(60)));
        assert!(node2.is_leader("backup"));
    }

    #[test]
    fn renew_extends_ttl() {
        let mut le = LeaderElection::new("node-1");
        assert!(le.try_acquire("task-a", Duration::from_secs(10)));

        let v1 = le.get_lease("task-a").unwrap().version;
        assert!(le.renew("task-a", Duration::from_secs(3600)));
        let v2 = le.get_lease("task-a").unwrap().version;
        assert!(v2 > v1);
    }

    #[test]
    fn active_leases_excludes_expired() {
        let mut le = LeaderElection::new("node-1");
        le.try_acquire("active", Duration::from_secs(3600));
        le.try_acquire("expired", Duration::from_secs(0));

        let active = le.active_leases();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].task, "active");
    }
}
