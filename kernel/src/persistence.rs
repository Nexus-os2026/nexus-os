//! Persistence bridge — connects kernel governance primitives to durable storage.
//!
//! Provides trait-based abstractions so tests can use in-memory stores while
//! production uses SQLite via `nexus-persistence`.

use std::sync::Arc;

use nexus_persistence::{
    AuditEventRow, CapabilityHistoryRow, ChainVerifyResult, FuelBalanceRow, FuelTransactionRow,
    HitlDecisionRow, NexusDatabase, PersistenceError, StateStore,
};
use serde_json::Value;
use uuid::Uuid;

use crate::audit::{AuditEvent, AuditTrail, EventType};

// ── Persistent Audit Store ──────────────────────────────────────────────────

/// Wraps a `NexusDatabase` and keeps the kernel's in-memory `AuditTrail` in sync.
///
/// Every `append` writes to both the in-memory trail (for fast verification) and
/// the SQLite store (for durability). On startup, the chain can be recovered from
/// disk and verified.
#[derive(Debug)]
pub struct PersistentAuditStore {
    db: Arc<NexusDatabase>,
    trail: std::sync::Mutex<AuditTrail>,
    sequence: std::sync::Mutex<i64>,
}

impl PersistentAuditStore {
    /// Create a new persistent audit store, recovering chain state from the database.
    pub fn new(db: Arc<NexusDatabase>) -> Result<Self, PersistenceError> {
        let latest_seq = db.get_audit_count()?;
        Ok(Self {
            db,
            trail: std::sync::Mutex::new(AuditTrail::new()),
            sequence: std::sync::Mutex::new(latest_seq),
        })
    }

    /// Append an event to both in-memory trail and persistent store.
    pub fn append_event(
        &self,
        agent_id: Uuid,
        event_type: EventType,
        payload: Value,
    ) -> Result<Uuid, PersistenceError> {
        let mut trail = self.trail.lock().unwrap_or_else(|p| p.into_inner());
        let mut seq = self.sequence.lock().unwrap_or_else(|p| p.into_inner());

        // Append to in-memory trail (computes hash chain)
        let event_id = trail
            .append_event(agent_id, event_type.clone(), payload.clone())
            .map_err(|e| PersistenceError::Serialization(e.to_string()))?;

        // Get the event we just appended (last one)
        let event = match trail.events().last() {
            Some(e) => e,
            None => {
                return Err(PersistenceError::Serialization(
                    "audit trail empty after append".to_string(),
                ));
            }
        };

        *seq += 1;
        let current_seq = *seq;

        // Persist to SQLite
        let event_type_str = match event_type {
            EventType::StateChange => "StateChange",
            EventType::ToolCall => "ToolCall",
            EventType::LlmCall => "LlmCall",
            EventType::Error => "Error",
            EventType::UserAction => "UserAction",
        };

        let detail_json = serde_json::to_string(&payload).unwrap_or_else(|_| "{}".to_string());

        self.db.append_audit_event(
            &agent_id.to_string(),
            event_type_str,
            &detail_json,
            &event.previous_hash,
            &event.hash,
            current_seq,
        )?;

        Ok(event_id)
    }

    /// Verify the in-memory hash chain integrity.
    pub fn verify_integrity(&self) -> bool {
        let trail = self.trail.lock().unwrap_or_else(|p| p.into_inner());
        trail.verify_integrity()
    }

    /// Verify the persistent hash chain integrity (full DB scan).
    pub fn verify_persistent_chain(&self) -> Result<ChainVerifyResult, PersistenceError> {
        self.db.verify_audit_chain()
    }

    /// Get the in-memory event count.
    pub fn in_memory_count(&self) -> usize {
        let trail = self.trail.lock().unwrap_or_else(|p| p.into_inner());
        trail.events().len()
    }

    /// Get the persistent event count.
    pub fn persistent_count(&self) -> Result<i64, PersistenceError> {
        self.db.get_audit_count()
    }

    /// Load audit events from the persistent store.
    pub fn load_events(
        &self,
        agent_id: Option<&str>,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<AuditEventRow>, PersistenceError> {
        self.db.load_audit_events(agent_id, limit, offset)
    }

    /// Get read-only access to the in-memory trail.
    pub fn events(&self) -> Vec<AuditEvent> {
        let trail = self.trail.lock().unwrap_or_else(|p| p.into_inner());
        trail.events().to_vec()
    }
}

// ── Persistent Fuel Store ───────────────────────────────────────────────────

/// Persistent fuel accounting with transaction-level ledger.
///
/// Every fuel operation (allocate, reserve, commit, cancel) is recorded as
/// an individual transaction row, and the balance summary is kept in sync.
#[derive(Debug)]
pub struct PersistentFuelStore {
    db: Arc<NexusDatabase>,
}

impl PersistentFuelStore {
    pub fn new(db: Arc<NexusDatabase>) -> Self {
        Self { db }
    }

    /// Allocate fuel to an agent.
    pub fn allocate(&self, agent_id: &str, amount: u64) -> Result<u64, PersistenceError> {
        let current = self.db.load_fuel_balance(agent_id)?;
        let (new_balance, new_allocated, consumed) = match current {
            Some(b) => (
                b.balance + amount as i64,
                b.total_allocated + amount as i64,
                b.total_consumed,
            ),
            None => (amount as i64, amount as i64, 0),
        };

        self.db
            .upsert_fuel_balance(agent_id, new_balance, new_allocated, consumed)?;

        let now = chrono::Utc::now().to_rfc3339();
        self.db.append_fuel_transaction(&FuelTransactionRow {
            id: Uuid::new_v4().to_string(),
            agent_id: agent_id.to_string(),
            operation: "allocate".to_string(),
            amount: amount as i64,
            balance_after: new_balance,
            reservation_id: None,
            metadata_json: None,
            created_at: now,
        })?;

        Ok(new_balance as u64)
    }

    /// Reserve fuel before execution (deducts from balance).
    pub fn reserve(&self, agent_id: &str, amount: u64) -> Result<String, PersistenceError> {
        let balance = self
            .db
            .load_fuel_balance(agent_id)?
            .ok_or_else(|| PersistenceError::NotFound(format!("fuel balance for {agent_id}")))?;

        if (balance.balance as u64) < amount {
            return Err(PersistenceError::NotFound(format!(
                "insufficient fuel: available={}, required={amount}",
                balance.balance
            )));
        }

        let new_balance = balance.balance - amount as i64;
        self.db.upsert_fuel_balance(
            agent_id,
            new_balance,
            balance.total_allocated,
            balance.total_consumed,
        )?;

        let reservation_id = Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        self.db.append_fuel_transaction(&FuelTransactionRow {
            id: Uuid::new_v4().to_string(),
            agent_id: agent_id.to_string(),
            operation: "reserve".to_string(),
            amount: -(amount as i64),
            balance_after: new_balance,
            reservation_id: Some(reservation_id.clone()),
            metadata_json: None,
            created_at: now,
        })?;

        Ok(reservation_id)
    }

    /// Commit actual fuel cost, refund unused portion.
    pub fn commit(
        &self,
        agent_id: &str,
        reservation_id: &str,
        reserved: u64,
        actual: u64,
    ) -> Result<(), PersistenceError> {
        let balance = self
            .db
            .load_fuel_balance(agent_id)?
            .ok_or_else(|| PersistenceError::NotFound(format!("fuel balance for {agent_id}")))?;

        let refund = reserved.saturating_sub(actual);
        let new_balance = balance.balance + refund as i64;
        let new_consumed = balance.total_consumed + actual as i64;

        self.db.upsert_fuel_balance(
            agent_id,
            new_balance,
            balance.total_allocated,
            new_consumed,
        )?;

        let now = chrono::Utc::now().to_rfc3339();
        self.db.append_fuel_transaction(&FuelTransactionRow {
            id: Uuid::new_v4().to_string(),
            agent_id: agent_id.to_string(),
            operation: "commit".to_string(),
            amount: -(actual as i64),
            balance_after: new_balance,
            reservation_id: Some(reservation_id.to_string()),
            metadata_json: Some(format!(
                "{{\"reserved\":{reserved},\"actual\":{actual},\"refunded\":{refund}}}"
            )),
            created_at: now,
        })?;

        Ok(())
    }

    /// Cancel a reservation, return all fuel.
    pub fn cancel_reservation(
        &self,
        agent_id: &str,
        reservation_id: &str,
        amount: u64,
    ) -> Result<(), PersistenceError> {
        let balance = self
            .db
            .load_fuel_balance(agent_id)?
            .ok_or_else(|| PersistenceError::NotFound(format!("fuel balance for {agent_id}")))?;

        let new_balance = balance.balance + amount as i64;
        self.db.upsert_fuel_balance(
            agent_id,
            new_balance,
            balance.total_allocated,
            balance.total_consumed,
        )?;

        let now = chrono::Utc::now().to_rfc3339();
        self.db.append_fuel_transaction(&FuelTransactionRow {
            id: Uuid::new_v4().to_string(),
            agent_id: agent_id.to_string(),
            operation: "cancel".to_string(),
            amount: amount as i64,
            balance_after: new_balance,
            reservation_id: Some(reservation_id.to_string()),
            metadata_json: None,
            created_at: now,
        })?;

        Ok(())
    }

    /// Get current balance for an agent.
    pub fn get_balance(&self, agent_id: &str) -> Result<Option<FuelBalanceRow>, PersistenceError> {
        self.db.load_fuel_balance(agent_id)
    }

    /// Get the transaction ledger for an agent.
    pub fn get_transactions(
        &self,
        agent_id: &str,
        limit: usize,
    ) -> Result<Vec<FuelTransactionRow>, PersistenceError> {
        self.db.load_fuel_transactions(agent_id, limit)
    }
}

// ── Persistent HITL Store ───────────────────────────────────────────────────

/// Records every human-in-the-loop decision permanently.
#[derive(Debug)]
pub struct PersistentHitlStore {
    db: Arc<NexusDatabase>,
}

impl PersistentHitlStore {
    pub fn new(db: Arc<NexusDatabase>) -> Self {
        Self { db }
    }

    /// Record a HITL decision.
    #[allow(clippy::too_many_arguments)]
    pub fn record_decision(
        &self,
        agent_id: &str,
        action: &str,
        decision: &str,
        decided_by: Option<&str>,
        response_time_ms: i64,
        context_json: Option<&str>,
        metadata_json: Option<&str>,
    ) -> Result<String, PersistenceError> {
        let id = Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        self.db.record_hitl_decision(&HitlDecisionRow {
            id: id.clone(),
            agent_id: agent_id.to_string(),
            action: action.to_string(),
            context_json: context_json.map(|s| s.to_string()),
            decision: decision.to_string(),
            decided_by: decided_by.map(|s| s.to_string()),
            decided_at: now,
            response_time_ms,
            metadata_json: metadata_json.map(|s| s.to_string()),
        })?;
        Ok(id)
    }

    /// Load decision history.
    pub fn get_history(
        &self,
        agent_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<HitlDecisionRow>, PersistenceError> {
        self.db.load_hitl_decisions(agent_id, limit)
    }

    /// Get approval rate (1.0 if no decisions recorded).
    pub fn approval_rate(&self, agent_id: Option<&str>) -> Result<f64, PersistenceError> {
        self.db.hitl_approval_rate(agent_id)
    }
}

// ── Persistent Capability History Store ─────────────────────────────────────

/// Records every capability grant/revocation permanently.
#[derive(Debug)]
pub struct PersistentCapabilityStore {
    db: Arc<NexusDatabase>,
}

impl PersistentCapabilityStore {
    pub fn new(db: Arc<NexusDatabase>) -> Self {
        Self { db }
    }

    /// Record a capability grant.
    pub fn record_grant(
        &self,
        agent_id: &str,
        capability: &str,
        resource: Option<&str>,
        granted_by: Option<&str>,
    ) -> Result<String, PersistenceError> {
        let id = Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        self.db.append_capability_history(&CapabilityHistoryRow {
            id: id.clone(),
            agent_id: agent_id.to_string(),
            capability: capability.to_string(),
            action: "grant".to_string(),
            resource: resource.map(|s| s.to_string()),
            performed_by: granted_by.map(|s| s.to_string()),
            created_at: now,
            metadata_json: None,
        })?;
        Ok(id)
    }

    /// Record a capability revocation.
    pub fn record_revoke(
        &self,
        agent_id: &str,
        capability: &str,
        revoked_by: Option<&str>,
    ) -> Result<String, PersistenceError> {
        let id = Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        self.db.append_capability_history(&CapabilityHistoryRow {
            id: id.clone(),
            agent_id: agent_id.to_string(),
            capability: capability.to_string(),
            action: "revoke".to_string(),
            resource: None,
            performed_by: revoked_by.map(|s| s.to_string()),
            created_at: now,
            metadata_json: None,
        })?;
        Ok(id)
    }

    /// Load capability change history for an agent.
    pub fn get_history(
        &self,
        agent_id: &str,
        limit: usize,
    ) -> Result<Vec<CapabilityHistoryRow>, PersistenceError> {
        self.db.load_capability_history(agent_id, limit)
    }
}

// ── Governance Database (unified access) ────────────────────────────────────

/// Unified access to all governance persistence stores.
///
/// Production code creates this once at startup and shares it via `Arc`.
#[derive(Debug)]
pub struct GovernanceDb {
    pub audit: PersistentAuditStore,
    pub fuel: PersistentFuelStore,
    pub hitl: PersistentHitlStore,
    pub capabilities: PersistentCapabilityStore,
    pub db: Arc<NexusDatabase>,
}

impl GovernanceDb {
    /// Open (or create) the governance database at the given path.
    pub fn open(path: &std::path::Path) -> Result<Self, PersistenceError> {
        let db = Arc::new(NexusDatabase::open(path)?);
        let audit = PersistentAuditStore::new(db.clone())?;
        let fuel = PersistentFuelStore::new(db.clone());
        let hitl = PersistentHitlStore::new(db.clone());
        let capabilities = PersistentCapabilityStore::new(db.clone());
        Ok(Self {
            audit,
            fuel,
            hitl,
            capabilities,
            db,
        })
    }

    /// Create an in-memory governance database (for tests).
    pub fn in_memory() -> Result<Self, PersistenceError> {
        let db = Arc::new(NexusDatabase::in_memory()?);
        let audit = PersistentAuditStore::new(db.clone())?;
        let fuel = PersistentFuelStore::new(db.clone());
        let hitl = PersistentHitlStore::new(db.clone());
        let capabilities = PersistentCapabilityStore::new(db.clone());
        Ok(Self {
            audit,
            fuel,
            hitl,
            capabilities,
            db,
        })
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn test_gov_db() -> GovernanceDb {
        GovernanceDb::in_memory().expect("failed to create in-memory governance db")
    }

    // ── Audit Tests ─────────────────────────────────────────────────────

    #[test]
    fn test_persistent_audit_append_and_query() {
        let gov = test_gov_db();
        let agent = Uuid::new_v4();

        for i in 0..5 {
            gov.audit
                .append_event(agent, EventType::ToolCall, json!({"seq": i}))
                .expect("append");
        }

        assert_eq!(gov.audit.in_memory_count(), 5);
        assert_eq!(gov.audit.persistent_count().unwrap(), 5);
    }

    #[test]
    fn test_persistent_audit_hash_chain_integrity() {
        let gov = test_gov_db();
        let agent = Uuid::new_v4();

        for i in 0..10 {
            gov.audit
                .append_event(agent, EventType::StateChange, json!({"i": i}))
                .expect("append");
        }

        assert!(gov.audit.verify_integrity());

        let chain = gov.audit.verify_persistent_chain().unwrap();
        assert!(chain.verified);
        assert_eq!(chain.chain_length, 10);
        assert!(chain.break_at_sequence.is_none());
    }

    #[test]
    fn test_persistent_audit_chain_detects_tamper() {
        let gov = test_gov_db();
        let agent = Uuid::new_v4();

        for i in 0..5 {
            gov.audit
                .append_event(agent, EventType::ToolCall, json!({"i": i}))
                .expect("append");
        }

        // Tamper with the persistent store directly
        gov.db
            .execute_raw("UPDATE audit_events SET current_hash = 'tampered' WHERE sequence = 3")
            .unwrap();

        let chain = gov.audit.verify_persistent_chain().unwrap();
        assert!(!chain.verified);
        assert_eq!(chain.break_at_sequence, Some(4));
    }

    #[test]
    fn test_persistent_audit_query_by_agent() {
        let gov = test_gov_db();
        let a1 = Uuid::new_v4();
        let a2 = Uuid::new_v4();

        gov.audit
            .append_event(a1, EventType::ToolCall, json!({"who": "a1"}))
            .unwrap();
        gov.audit
            .append_event(a2, EventType::LlmCall, json!({"who": "a2"}))
            .unwrap();
        gov.audit
            .append_event(a1, EventType::StateChange, json!({"who": "a1_again"}))
            .unwrap();

        let a1_events = gov
            .audit
            .load_events(Some(&a1.to_string()), 100, 0)
            .unwrap();
        assert_eq!(a1_events.len(), 2);

        let a2_events = gov
            .audit
            .load_events(Some(&a2.to_string()), 100, 0)
            .unwrap();
        assert_eq!(a2_events.len(), 1);
    }

    #[test]
    fn test_persistent_audit_count() {
        let gov = test_gov_db();
        assert_eq!(gov.audit.persistent_count().unwrap(), 0);

        let agent = Uuid::new_v4();
        for _ in 0..7 {
            gov.audit
                .append_event(agent, EventType::UserAction, json!({}))
                .unwrap();
        }
        assert_eq!(gov.audit.persistent_count().unwrap(), 7);
    }

    #[test]
    fn test_persistent_audit_events_vec() {
        let gov = test_gov_db();
        let agent = Uuid::new_v4();

        let eid = gov
            .audit
            .append_event(agent, EventType::Error, json!({"msg": "oops"}))
            .unwrap();

        let events = gov.audit.events();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_id, eid);
    }

    // ── Fuel Tests ──────────────────────────────────────────────────────

    #[test]
    fn test_fuel_allocate_and_balance() {
        let gov = test_gov_db();
        let balance = gov.fuel.allocate("agent-1", 1000).unwrap();
        assert_eq!(balance, 1000);

        let b = gov.fuel.get_balance("agent-1").unwrap().unwrap();
        assert_eq!(b.balance, 1000);
        assert_eq!(b.total_allocated, 1000);
        assert_eq!(b.total_consumed, 0);
    }

    #[test]
    fn test_fuel_reserve_deducts() {
        let gov = test_gov_db();
        gov.fuel.allocate("agent-1", 1000).unwrap();

        let _rid = gov.fuel.reserve("agent-1", 300).unwrap();
        let b = gov.fuel.get_balance("agent-1").unwrap().unwrap();
        assert_eq!(b.balance, 700);
    }

    #[test]
    fn test_fuel_commit_refunds_unused() {
        let gov = test_gov_db();
        gov.fuel.allocate("agent-1", 1000).unwrap();

        let rid = gov.fuel.reserve("agent-1", 500).unwrap();
        gov.fuel.commit("agent-1", &rid, 500, 200).unwrap();

        let b = gov.fuel.get_balance("agent-1").unwrap().unwrap();
        assert_eq!(b.balance, 800); // 1000 - 500 + 300 refund
        assert_eq!(b.total_consumed, 200);
    }

    #[test]
    fn test_fuel_cancel_returns_all() {
        let gov = test_gov_db();
        gov.fuel.allocate("agent-1", 1000).unwrap();

        let rid = gov.fuel.reserve("agent-1", 400).unwrap();
        gov.fuel.cancel_reservation("agent-1", &rid, 400).unwrap();

        let b = gov.fuel.get_balance("agent-1").unwrap().unwrap();
        assert_eq!(b.balance, 1000);
    }

    #[test]
    fn test_fuel_insufficient_blocked() {
        let gov = test_gov_db();
        gov.fuel.allocate("agent-1", 100).unwrap();

        let result = gov.fuel.reserve("agent-1", 200);
        assert!(result.is_err());
    }

    #[test]
    fn test_fuel_ledger_records_all_operations() {
        let gov = test_gov_db();
        gov.fuel.allocate("agent-1", 1000).unwrap();
        let rid = gov.fuel.reserve("agent-1", 300).unwrap();
        gov.fuel.commit("agent-1", &rid, 300, 150).unwrap();

        let txs = gov.fuel.get_transactions("agent-1", 100).unwrap();
        assert_eq!(txs.len(), 3); // allocate, reserve, commit

        // Most recent first
        assert_eq!(txs[0].operation, "commit");
        assert_eq!(txs[1].operation, "reserve");
        assert_eq!(txs[2].operation, "allocate");
    }

    #[test]
    fn test_fuel_multiple_allocations() {
        let gov = test_gov_db();
        gov.fuel.allocate("agent-1", 500).unwrap();
        let balance = gov.fuel.allocate("agent-1", 300).unwrap();
        assert_eq!(balance, 800);

        let b = gov.fuel.get_balance("agent-1").unwrap().unwrap();
        assert_eq!(b.total_allocated, 800);
    }

    // ── HITL Tests ──────────────────────────────────────────────────────

    #[test]
    fn test_hitl_record_and_query() {
        let gov = test_gov_db();

        gov.hitl
            .record_decision(
                "agent-1",
                "file.delete",
                "approved",
                Some("admin"),
                150,
                None,
                None,
            )
            .unwrap();
        gov.hitl
            .record_decision(
                "agent-1",
                "net.send",
                "denied",
                Some("admin"),
                200,
                None,
                None,
            )
            .unwrap();

        let decisions = gov.hitl.get_history(Some("agent-1"), 100).unwrap();
        assert_eq!(decisions.len(), 2);
    }

    #[test]
    fn test_hitl_approval_rate() {
        let gov = test_gov_db();

        gov.hitl
            .record_decision("a1", "action1", "approved", None, 100, None, None)
            .unwrap();
        gov.hitl
            .record_decision("a1", "action2", "approved", None, 100, None, None)
            .unwrap();
        gov.hitl
            .record_decision("a1", "action3", "denied", None, 100, None, None)
            .unwrap();

        let rate = gov.hitl.approval_rate(Some("a1")).unwrap();
        assert!((rate - 2.0 / 3.0).abs() < 0.01);

        // No decisions = 100% approval
        let empty_rate = gov.hitl.approval_rate(Some("nonexistent")).unwrap();
        assert!((empty_rate - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_hitl_global_query() {
        let gov = test_gov_db();

        gov.hitl
            .record_decision("a1", "op1", "approved", None, 50, None, None)
            .unwrap();
        gov.hitl
            .record_decision("a2", "op2", "denied", None, 80, None, None)
            .unwrap();

        let all = gov.hitl.get_history(None, 100).unwrap();
        assert_eq!(all.len(), 2);

        let rate = gov.hitl.approval_rate(None).unwrap();
        assert!((rate - 0.5).abs() < f64::EPSILON);
    }

    // ── Capability History Tests ────────────────────────────────────────

    #[test]
    fn test_capability_grant_and_revoke_history() {
        let gov = test_gov_db();

        gov.capabilities
            .record_grant("agent-1", "file.read", None, Some("admin"))
            .unwrap();
        gov.capabilities
            .record_grant("agent-1", "net.http", Some("*.api.com"), Some("admin"))
            .unwrap();
        gov.capabilities
            .record_revoke("agent-1", "file.read", Some("admin"))
            .unwrap();

        let history = gov.capabilities.get_history("agent-1", 100).unwrap();
        assert_eq!(history.len(), 3);

        // Most recent first
        assert_eq!(history[0].action, "revoke");
        assert_eq!(history[1].action, "grant");
        assert_eq!(history[2].action, "grant");
    }

    // ── Persistence Across Reopen (power failure simulation) ────────────

    #[test]
    fn test_audit_persistence_across_reopen() {
        let dir = tempfile::tempdir().expect("tmpdir");
        let db_path = dir.path().join("test_governance.db");

        // Phase 1: Write data
        {
            let gov = GovernanceDb::open(&db_path).expect("open");
            let agent = Uuid::new_v4();
            for i in 0..10 {
                gov.audit
                    .append_event(agent, EventType::ToolCall, json!({"seq": i}))
                    .expect("append");
            }
            assert_eq!(gov.audit.persistent_count().unwrap(), 10);
        }
        // DB dropped here — simulates power off

        // Phase 2: Reopen and verify
        {
            let gov = GovernanceDb::open(&db_path).expect("reopen");
            assert_eq!(gov.audit.persistent_count().unwrap(), 10);

            let chain = gov.audit.verify_persistent_chain().unwrap();
            assert!(chain.verified);
            assert_eq!(chain.chain_length, 10);
        }
    }

    #[test]
    fn test_fuel_persistence_across_reopen() {
        let dir = tempfile::tempdir().expect("tmpdir");
        let db_path = dir.path().join("test_governance.db");

        {
            let gov = GovernanceDb::open(&db_path).expect("open");
            gov.fuel.allocate("agent-1", 5000).unwrap();
            let rid = gov.fuel.reserve("agent-1", 2000).unwrap();
            gov.fuel.commit("agent-1", &rid, 2000, 1500).unwrap();
        }

        {
            let gov = GovernanceDb::open(&db_path).expect("reopen");
            let b = gov.fuel.get_balance("agent-1").unwrap().unwrap();
            assert_eq!(b.balance, 3500); // 5000 - 2000 + 500 refund
            assert_eq!(b.total_consumed, 1500);

            let txs = gov.fuel.get_transactions("agent-1", 100).unwrap();
            assert_eq!(txs.len(), 3);
        }
    }

    #[test]
    fn test_hitl_persistence_across_reopen() {
        let dir = tempfile::tempdir().expect("tmpdir");
        let db_path = dir.path().join("test_governance.db");

        {
            let gov = GovernanceDb::open(&db_path).expect("open");
            gov.hitl
                .record_decision("a1", "shell.exec", "denied", Some("root"), 42, None, None)
                .unwrap();
        }

        {
            let gov = GovernanceDb::open(&db_path).expect("reopen");
            let decisions = gov.hitl.get_history(Some("a1"), 10).unwrap();
            assert_eq!(decisions.len(), 1);
            assert_eq!(decisions[0].decision, "denied");
            assert_eq!(decisions[0].response_time_ms, 42);
        }
    }

    #[test]
    fn test_capability_persistence_across_reopen() {
        let dir = tempfile::tempdir().expect("tmpdir");
        let db_path = dir.path().join("test_governance.db");

        {
            let gov = GovernanceDb::open(&db_path).expect("open");
            gov.capabilities
                .record_grant("a1", "file.write", None, Some("admin"))
                .unwrap();
            gov.capabilities
                .record_revoke("a1", "file.write", Some("security"))
                .unwrap();
        }

        {
            let gov = GovernanceDb::open(&db_path).expect("reopen");
            let history = gov.capabilities.get_history("a1", 100).unwrap();
            assert_eq!(history.len(), 2);
            assert_eq!(history[0].action, "revoke");
            assert_eq!(history[0].performed_by.as_deref(), Some("security"));
        }
    }

    // ── Edge Cases ──────────────────────────────────────────────────────

    #[test]
    fn test_fuel_no_balance_returns_none() {
        let gov = test_gov_db();
        assert!(gov.fuel.get_balance("ghost").unwrap().is_none());
    }

    #[test]
    fn test_fuel_reserve_nonexistent_agent() {
        let gov = test_gov_db();
        assert!(gov.fuel.reserve("ghost", 100).is_err());
    }

    #[test]
    fn test_empty_chain_verification() {
        let gov = test_gov_db();
        let chain = gov.audit.verify_persistent_chain().unwrap();
        assert!(chain.verified);
        assert_eq!(chain.chain_length, 0);
    }

    #[test]
    fn test_hitl_with_context_and_metadata() {
        let gov = test_gov_db();
        gov.hitl
            .record_decision(
                "a1",
                "deploy.production",
                "approved",
                Some("cto"),
                3200,
                Some(r#"{"env":"prod","region":"us-east-1"}"#),
                Some(r#"{"reason":"scheduled release"}"#),
            )
            .unwrap();

        let decisions = gov.hitl.get_history(Some("a1"), 10).unwrap();
        assert_eq!(decisions.len(), 1);
        assert!(decisions[0].context_json.is_some());
        assert!(decisions[0].metadata_json.is_some());
    }

    #[test]
    fn test_fuel_commit_zero_actual() {
        let gov = test_gov_db();
        gov.fuel.allocate("a1", 1000).unwrap();
        let rid = gov.fuel.reserve("a1", 500).unwrap();
        gov.fuel.commit("a1", &rid, 500, 0).unwrap();

        let b = gov.fuel.get_balance("a1").unwrap().unwrap();
        assert_eq!(b.balance, 1000); // full refund
        assert_eq!(b.total_consumed, 0);
    }
}
