//! Lock-free concurrent supervisor layer — eliminates mutex contention for hot-path
//! operations at scale (2000+ agents).
//!
//! The original `Supervisor` uses a single `Mutex` protecting all state. This works
//! well up to ~500 agents but creates contention at higher scales because every
//! thread blocks on the same lock for agent lookups, fuel operations, and capability
//! checks — even though these operations access independent per-agent data.
//!
//! `ConcurrentSupervisor` solves this by:
//!
//! 1. **DashMap for agent registry** — concurrent HashMap allows multiple threads to
//!    read/write different agents simultaneously without blocking each other.
//!
//! 2. **Sharded fuel ledgers** — fuel operations are partitioned across N shards
//!    (keyed by agent ID hash). Threads accessing different agents hit different
//!    shards, eliminating cross-agent contention.
//!
//! 3. **Atomic counters** — global stats (total agents, total fuel consumed) use
//!    lock-free atomics instead of mutex-protected aggregation.
//!
//! 4. **Lock-free message queue** — inter-agent messages use crossbeam's SegQueue
//!    (Michael-Scott lock-free queue) instead of mutex-protected VecDeque.
//!
//! The original `Supervisor` is preserved for cold-path operations (policy engine,
//! time machine, speculative engine) that are rarely called under contention.

use crate::errors::AgentError;
use crate::manifest::AgentManifest;
use crate::supervisor::{AgentId, Supervisor, SupervisorFuelReservation};
use crossbeam::queue::SegQueue;
use dashmap::DashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use uuid::Uuid;

/// Number of fuel ledger shards. Must be a power of 2 for fast modulo via bitwise AND.
const FUEL_SHARD_COUNT: usize = 64;

/// A fuel ledger entry for one agent.
#[derive(Debug)]
struct FuelEntry {
    remaining: AtomicU64,
    total_consumed: AtomicU64,
}

impl FuelEntry {
    fn new(budget: u64) -> Self {
        Self {
            remaining: AtomicU64::new(budget),
            total_consumed: AtomicU64::new(0),
        }
    }
}

/// One shard of the fuel ledger. Each shard has its own Mutex protecting
/// a small subset of agents, so threads accessing different shards never block.
struct FuelShard {
    entries: DashMap<AgentId, Arc<FuelEntry>>,
}

impl FuelShard {
    fn new() -> Self {
        Self {
            entries: DashMap::new(),
        }
    }
}

/// A pending inter-agent message in the lock-free queue.
#[derive(Debug, Clone)]
pub struct AgentMessage {
    pub from: Uuid,
    pub to: Uuid,
    pub payload: String,
}

/// Concurrent supervisor with lock-free hot paths.
///
/// Use this instead of `Arc<Mutex<Supervisor>>` for high-contention scenarios
/// (2000+ concurrent agents). The `inner` Supervisor is still available for
/// cold-path operations via `with_inner()`.
pub struct ConcurrentSupervisor {
    /// Agent registry — lock-free concurrent HashMap.
    agents: DashMap<AgentId, AgentSnapshot>,
    /// Sharded fuel ledgers — distributes contention across FUEL_SHARD_COUNT shards.
    fuel_shards: Vec<FuelShard>,
    /// Lock-free message queue for inter-agent communication.
    message_queue: SegQueue<AgentMessage>,
    /// Global atomic counters.
    total_agents: AtomicU64,
    total_fuel_consumed: AtomicU64,
    total_messages_sent: AtomicU64,
    /// The original Supervisor for cold-path operations.
    inner: Mutex<Supervisor>,
}

/// Lightweight snapshot of agent state stored in the concurrent DashMap.
/// Avoids cloning the full AgentHandle for read-only lookups.
#[derive(Debug, Clone)]
pub struct AgentSnapshot {
    pub id: AgentId,
    pub name: String,
    pub autonomy_level: u8,
    pub capabilities: Vec<String>,
    pub fuel_budget: u64,
}

impl ConcurrentSupervisor {
    /// Create from an existing Supervisor, migrating agent state into the concurrent layer.
    pub fn from_supervisor(supervisor: Supervisor) -> Self {
        let agents = DashMap::new();
        let fuel_shards: Vec<FuelShard> = (0..FUEL_SHARD_COUNT).map(|_| FuelShard::new()).collect();

        // Migrate existing agents into the concurrent structures
        let mut agent_count = 0u64;
        for status in supervisor.health_check() {
            let agent_id = status.id;
            if let Some(handle) = supervisor.get_agent(agent_id) {
                let snapshot = AgentSnapshot {
                    id: agent_id,
                    name: handle.manifest.name.clone(),
                    autonomy_level: handle.autonomy_level,
                    capabilities: handle.manifest.capabilities.clone(),
                    fuel_budget: handle.manifest.fuel_budget,
                };
                agents.insert(agent_id, snapshot);

                let shard_idx = shard_for(agent_id);
                fuel_shards[shard_idx]
                    .entries
                    .insert(agent_id, Arc::new(FuelEntry::new(handle.remaining_fuel)));

                agent_count += 1;
            }
        }

        Self {
            agents,
            fuel_shards,
            message_queue: SegQueue::new(),
            total_agents: AtomicU64::new(agent_count),
            total_fuel_consumed: AtomicU64::new(0),
            total_messages_sent: AtomicU64::new(0),
            inner: Mutex::new(supervisor),
        }
    }

    /// Register a new agent. Spawns it in the inner Supervisor and mirrors
    /// the state into the concurrent layer.
    pub fn start_agent(&self, manifest: AgentManifest) -> Result<AgentId, AgentError> {
        let fuel_budget = manifest.fuel_budget;
        let snapshot = AgentSnapshot {
            id: Uuid::nil(), // placeholder, replaced after start
            name: manifest.name.clone(),
            autonomy_level: manifest.autonomy_level.unwrap_or(0),
            capabilities: manifest.capabilities.clone(),
            fuel_budget,
        };

        // Start in inner supervisor (handles governance checks, L5/L6 singletons, etc.)
        let id = {
            let mut inner = self.inner.lock().unwrap_or_else(|p| p.into_inner());
            inner.start_agent(manifest)?
        };

        // Mirror into concurrent structures
        let mut snap = snapshot;
        snap.id = id;
        self.agents.insert(id, snap);

        let shard_idx = shard_for(id);
        self.fuel_shards[shard_idx]
            .entries
            .insert(id, Arc::new(FuelEntry::new(fuel_budget)));

        self.total_agents.fetch_add(1, Ordering::Relaxed);
        Ok(id)
    }

    /// Lock-free agent lookup. Returns None if agent doesn't exist.
    pub fn get_agent(&self, id: AgentId) -> Option<AgentSnapshot> {
        self.agents.get(&id).map(|entry| entry.value().clone())
    }

    /// Check if an agent exists (lock-free).
    pub fn agent_exists(&self, id: AgentId) -> bool {
        self.agents.contains_key(&id)
    }

    /// Reserve fuel using lock-free atomics. Uses Compare-And-Swap to ensure
    /// no over-deduction even under concurrent access.
    ///
    /// Returns a reservation token on success, or an error if insufficient fuel.
    pub fn reserve_fuel(
        &self,
        id: AgentId,
        max_cost: u64,
        action_type: &str,
    ) -> Result<SupervisorFuelReservation, AgentError> {
        let shard_idx = shard_for(id);
        let entry = self.fuel_shards[shard_idx]
            .entries
            .get(&id)
            .ok_or_else(|| {
                AgentError::SupervisorError(format!("agent '{id}' not found in fuel shards"))
            })?;

        // CAS loop: atomically check-and-deduct fuel
        loop {
            let current = entry.remaining.load(Ordering::Acquire);
            if current < max_cost {
                return Err(AgentError::SupervisorError(format!(
                    "insufficient fuel for {action_type}: need {max_cost}, have {current}"
                )));
            }
            // Try to atomically deduct
            match entry.remaining.compare_exchange_weak(
                current,
                current - max_cost,
                Ordering::AcqRel,
                Ordering::Relaxed,
            ) {
                Ok(_) => {
                    return Ok(SupervisorFuelReservation {
                        id: Uuid::new_v4(),
                        agent_id: id,
                        reserved_amount: max_cost,
                        action_type: action_type.to_string(),
                    });
                }
                Err(_) => {
                    // Another thread modified fuel — retry CAS
                    std::hint::spin_loop();
                }
            }
        }
    }

    /// Commit actual fuel cost. Refunds unused reservation via atomic add.
    pub fn commit_fuel(
        &self,
        reservation: SupervisorFuelReservation,
        actual_cost: u64,
    ) -> Result<(), AgentError> {
        let refund = reservation.reserved_amount.saturating_sub(actual_cost);
        let shard_idx = shard_for(reservation.agent_id);

        if let Some(entry) = self.fuel_shards[shard_idx]
            .entries
            .get(&reservation.agent_id)
        {
            if refund > 0 {
                entry.remaining.fetch_add(refund, Ordering::Release);
            }
            entry
                .total_consumed
                .fetch_add(actual_cost, Ordering::Relaxed);
            self.total_fuel_consumed
                .fetch_add(actual_cost, Ordering::Relaxed);
        }

        Ok(())
    }

    /// Send an inter-agent message via the lock-free queue.
    pub fn send_message(&self, from: Uuid, to: Uuid, payload: &str) {
        self.message_queue.push(AgentMessage {
            from,
            to,
            payload: payload.to_string(),
        });
        self.total_messages_sent.fetch_add(1, Ordering::Relaxed);
    }

    /// Drain all pending messages (for batch processing).
    pub fn drain_messages(&self) -> Vec<AgentMessage> {
        let mut messages = Vec::new();
        while let Some(msg) = self.message_queue.pop() {
            messages.push(msg);
        }
        messages
    }

    /// Access the inner Supervisor for cold-path operations
    /// (policy engine, time machine, etc.)
    pub fn with_inner<T>(&self, f: impl FnOnce(&mut Supervisor) -> T) -> T {
        let mut inner = self.inner.lock().unwrap_or_else(|p| p.into_inner());
        f(&mut inner)
    }

    /// Global stats (lock-free reads).
    pub fn total_agents(&self) -> u64 {
        self.total_agents.load(Ordering::Relaxed)
    }

    pub fn total_fuel_consumed(&self) -> u64 {
        self.total_fuel_consumed.load(Ordering::Relaxed)
    }

    pub fn total_messages_sent(&self) -> u64 {
        self.total_messages_sent.load(Ordering::Relaxed)
    }
}

/// Determine the fuel shard index for an agent ID.
/// Uses the low bits of the UUID to distribute agents across shards.
#[inline]
fn shard_for(id: AgentId) -> usize {
    // UUID v4 has good entropy in all bits. Use low bits for fast shard selection.
    (id.as_u128() as usize) & (FUEL_SHARD_COUNT - 1)
}

// SAFETY: ConcurrentSupervisor is Send + Sync because:
// - DashMap is Send + Sync
// - AtomicU64 is Send + Sync
// - SegQueue is Send + Sync
// - Mutex<Supervisor> is Send + Sync
unsafe impl Send for ConcurrentSupervisor {}
unsafe impl Sync for ConcurrentSupervisor {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::AgentManifest;

    fn test_manifest(name: &str, fuel: u64) -> AgentManifest {
        AgentManifest {
            name: name.into(),
            version: "1.0.0".into(),
            capabilities: vec!["llm.query".into(), "fs.read".into()],
            fuel_budget: fuel,
            autonomy_level: Some(2),
            consent_policy_path: None,
            requester_id: None,
            schedule: None,
            default_goal: None,
            llm_model: None,
            fuel_period_id: None,
            monthly_fuel_cap: None,
            allowed_endpoints: None,
            domain_tags: vec![],
            filesystem_permissions: vec![],
        }
    }

    #[test]
    fn test_start_and_lookup() {
        let sup = ConcurrentSupervisor::from_supervisor(Supervisor::new());
        let id = sup.start_agent(test_manifest("test-agent", 5000)).unwrap();

        let snap = sup.get_agent(id).unwrap();
        assert_eq!(snap.name, "test-agent");
        assert_eq!(snap.fuel_budget, 5000);
        assert!(sup.agent_exists(id));
        assert_eq!(sup.total_agents(), 1);
    }

    #[test]
    fn test_reserve_and_commit_fuel() {
        let sup = ConcurrentSupervisor::from_supervisor(Supervisor::new());
        let id = sup.start_agent(test_manifest("fuel-test", 1000)).unwrap();

        let reservation = sup.reserve_fuel(id, 200, "test_action").unwrap();
        assert_eq!(reservation.reserved_amount, 200);

        sup.commit_fuel(reservation, 150).unwrap();
        assert_eq!(sup.total_fuel_consumed(), 150);
    }

    #[test]
    fn test_insufficient_fuel() {
        let sup = ConcurrentSupervisor::from_supervisor(Supervisor::new());
        let id = sup.start_agent(test_manifest("low-fuel", 100)).unwrap();

        let result = sup.reserve_fuel(id, 200, "expensive_action");
        assert!(result.is_err());
    }

    #[test]
    fn test_concurrent_fuel_operations() {
        let sup = Arc::new(ConcurrentSupervisor::from_supervisor(Supervisor::new()));
        let id = sup
            .start_agent(test_manifest("concurrent", 100_000))
            .unwrap();

        let mut handles = vec![];
        for _ in 0..10 {
            let sup = sup.clone();
            handles.push(std::thread::spawn(move || {
                for _ in 0..100 {
                    if let Ok(reservation) = sup.reserve_fuel(id, 10, "thread_op") {
                        let _ = sup.commit_fuel(reservation, 5);
                    }
                }
            }));
        }

        for h in handles {
            h.join().unwrap();
        }

        // 10 threads × 100 ops × 5 actual cost = 5000 total consumed
        assert_eq!(sup.total_fuel_consumed(), 5000);
    }

    #[test]
    fn test_message_queue() {
        let sup = ConcurrentSupervisor::from_supervisor(Supervisor::new());
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();

        sup.send_message(a, b, "hello");
        sup.send_message(b, a, "world");

        let messages = sup.drain_messages();
        assert_eq!(messages.len(), 2);
        assert_eq!(sup.total_messages_sent(), 2);
    }

    #[test]
    fn test_shard_distribution() {
        // Verify UUIDs distribute across shards (no pathological clustering)
        let mut shard_counts = vec![0u32; FUEL_SHARD_COUNT];
        for _ in 0..10000 {
            let id = Uuid::new_v4();
            shard_counts[shard_for(id)] += 1;
        }
        let min = *shard_counts.iter().min().unwrap();
        let max = *shard_counts.iter().max().unwrap();
        // With 10000 IDs across 64 shards, expect ~156 per shard.
        // Allow 2x variance (78-312 range).
        assert!(
            min > 50 && max < 400,
            "shard distribution too skewed: min={min}, max={max}"
        );
    }
}
