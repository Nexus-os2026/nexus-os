pub mod federation;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

/// Audit subsystem error — returned when an audit event cannot be recorded.
/// Fail-closed: callers MUST propagate this to abort the operation.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AuditError {
    #[error("audit batcher mutex poisoned — audit integrity compromised")]
    BatcherPoisoned,
}

const GENESIS_HASH: &str = "0000000000000000000000000000000000000000000000000000000000000000";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EventType {
    StateChange,
    ToolCall,
    LlmCall,
    Error,
    UserAction,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditEvent {
    pub event_id: Uuid,
    pub timestamp: u64,
    pub agent_id: Uuid,
    pub event_type: EventType,
    pub payload: Value,
    pub previous_hash: String,
    pub hash: String,
}

// ---------------------------------------------------------------------------
// Block batcher — optional bridge to distributed immutable audit
// ---------------------------------------------------------------------------

/// Callback trait for sealing a batch of events into a distributed audit block.
///
/// Implemented by the distributed crate's `AuditChain` bridge. The kernel
/// defines the interface; the distributed crate provides the implementation.
pub trait BlockBatchSink: Send + Sync {
    /// Seal a batch of events into an audit block.
    fn seal_batch(&mut self, events: Vec<AuditEvent>);
}

/// Configuration for the block batcher.
#[derive(Debug, Clone)]
pub struct BatcherConfig {
    /// Maximum number of events before auto-sealing a batch.
    pub max_events: usize,
    /// Maximum seconds before auto-sealing a batch (checked on each append).
    pub max_age_secs: u64,
}

impl Default for BatcherConfig {
    fn default() -> Self {
        Self {
            max_events: 50,
            max_age_secs: 10,
        }
    }
}

/// Internal state for the block batcher, shared via `Arc<Mutex<>>`.
struct BlockBatcherState {
    config: BatcherConfig,
    pending: Vec<AuditEvent>,
    batch_start: u64,
    sink: Box<dyn BlockBatchSink>,
    sealed_count: u64,
}

impl std::fmt::Debug for BlockBatcherState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BlockBatcherState")
            .field("config", &self.config)
            .field("pending_count", &self.pending.len())
            .field("sealed_count", &self.sealed_count)
            .finish()
    }
}

impl BlockBatcherState {
    fn push_event(&mut self, event: AuditEvent) {
        if self.pending.is_empty() {
            self.batch_start = current_unix_timestamp();
        }
        self.pending.push(event);
        self.maybe_seal();
    }

    fn maybe_seal(&mut self) {
        let should_seal = self.pending.len() >= self.config.max_events
            || (!self.pending.is_empty()
                && current_unix_timestamp().saturating_sub(self.batch_start)
                    >= self.config.max_age_secs);

        if should_seal {
            let batch = std::mem::take(&mut self.pending);
            self.sink.seal_batch(batch);
            self.sealed_count += 1;
        }
    }
}

/// Shared handle to an optional block batcher. Cloneable (Arc-backed).
#[derive(Debug, Clone, Default)]
struct BatcherHandle {
    inner: Option<Arc<Mutex<BlockBatcherState>>>,
}

impl BatcherHandle {
    fn none() -> Self {
        Self { inner: None }
    }

    fn new(config: BatcherConfig, sink: Box<dyn BlockBatchSink>) -> Self {
        Self {
            inner: Some(Arc::new(Mutex::new(BlockBatcherState {
                config,
                pending: Vec::new(),
                batch_start: 0,
                sink,
                sealed_count: 0,
            }))),
        }
    }

    fn push_event(&self, event: &AuditEvent) -> Result<(), AuditError> {
        if let Some(inner) = &self.inner {
            let mut state = inner.lock().map_err(|_| AuditError::BatcherPoisoned)?;
            state.push_event(event.clone());
        }
        Ok(())
    }

    fn flush(&self) {
        if let Some(inner) = &self.inner {
            if let Ok(mut state) = inner.lock() {
                if !state.pending.is_empty() {
                    let batch = std::mem::take(&mut state.pending);
                    state.sink.seal_batch(batch);
                    state.sealed_count += 1;
                }
            }
        }
    }

    fn sealed_count(&self) -> u64 {
        self.inner
            .as_ref()
            .and_then(|inner| inner.lock().ok())
            .map(|state| state.sealed_count)
            .unwrap_or(0)
    }

    fn pending_count(&self) -> usize {
        self.inner
            .as_ref()
            .and_then(|inner| inner.lock().ok())
            .map(|state| state.pending.len())
            .unwrap_or(0)
    }
}

// ---------------------------------------------------------------------------
// AuditTrail
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct AuditTrail {
    events: Vec<AuditEvent>,
    batcher: BatcherHandle,
}

impl AuditTrail {
    pub fn new() -> Self {
        Self {
            events: Vec::new(),
            batcher: BatcherHandle::none(),
        }
    }

    /// Enable distributed audit block batching.
    ///
    /// Events appended after this call are additionally pushed to the batcher.
    /// When the batcher reaches `config.max_events` events or `config.max_age_secs`
    /// seconds since the batch started, it seals the batch via the provided sink.
    pub fn enable_distributed_audit(
        &mut self,
        config: BatcherConfig,
        sink: Box<dyn BlockBatchSink>,
    ) {
        self.batcher = BatcherHandle::new(config, sink);
    }

    /// Flush any pending events in the batcher, sealing a partial batch.
    pub fn flush_batcher(&self) {
        self.batcher.flush();
    }

    /// Number of batches sealed so far (0 if batcher not enabled).
    pub fn sealed_batch_count(&self) -> u64 {
        self.batcher.sealed_count()
    }

    /// Number of events pending in the current unsent batch (0 if batcher not enabled).
    pub fn pending_batch_count(&self) -> usize {
        self.batcher.pending_count()
    }

    pub fn append_event(
        &mut self,
        agent_id: Uuid,
        event_type: EventType,
        payload: Value,
    ) -> Result<Uuid, AuditError> {
        let event_id = Uuid::new_v4();
        let timestamp = current_unix_timestamp();
        let previous_hash = self
            .events
            .last()
            .map(|event| event.hash.clone())
            .unwrap_or_else(|| GENESIS_HASH.to_string());
        let hash = compute_hash(
            event_id,
            timestamp,
            agent_id,
            &event_type,
            &payload,
            &previous_hash,
        );

        let event = AuditEvent {
            event_id,
            timestamp,
            agent_id,
            event_type,
            payload,
            previous_hash,
            hash,
        };
        self.events.push(event.clone());
        self.batcher.push_event(&event)?;
        Ok(event_id)
    }

    pub fn events(&self) -> &[AuditEvent] {
        &self.events
    }

    pub fn events_mut(&mut self) -> &mut [AuditEvent] {
        &mut self.events
    }

    pub fn verify_integrity(&self) -> bool {
        let mut expected_previous = GENESIS_HASH.to_string();

        for event in &self.events {
            if event.previous_hash != expected_previous {
                return false;
            }

            let expected_hash = compute_hash(
                event.event_id,
                event.timestamp,
                event.agent_id,
                &event.event_type,
                &event.payload,
                &event.previous_hash,
            );
            if event.hash != expected_hash {
                return false;
            }

            expected_previous = event.hash.clone();
        }

        true
    }
}

fn current_unix_timestamp() -> u64 {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => duration.as_secs(),
        Err(_) => 0,
    }
}

fn compute_hash(
    event_id: Uuid,
    timestamp: u64,
    agent_id: Uuid,
    event_type: &EventType,
    payload: &Value,
    previous_hash: &str,
) -> String {
    #[derive(Serialize)]
    struct CanonicalEventData<'a> {
        event_id: &'a str,
        timestamp: u64,
        agent_id: &'a str,
        event_type: &'a EventType,
        payload: &'a Value,
    }

    let event_id_string = event_id.to_string();
    let agent_id_string = agent_id.to_string();
    let canonical = CanonicalEventData {
        event_id: &event_id_string,
        timestamp,
        agent_id: &agent_id_string,
        event_type,
        payload,
    };

    // Fallback to hashing the debug representation if serialization fails,
    // preserving hash-chain continuity without panicking.
    let serialized = match serde_json::to_vec(&canonical) {
        Ok(bytes) => bytes,
        Err(_) => format!("{event_id}:{timestamp}:{agent_id}:{event_type:?}").into_bytes(),
    };

    let mut hasher = Sha256::new();
    hasher.update(previous_hash.as_bytes());
    hasher.update(serialized);
    let digest = hasher.finalize();
    format!("{digest:x}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_audit_chain_integrity() {
        let mut trail = AuditTrail::new();
        let agent_id = Uuid::new_v4();

        for idx in 0..5 {
            let payload = json!({ "seq": idx, "status": "ok" });
            trail
                .append_event(agent_id, EventType::StateChange, payload)
                .expect("audit append");
        }

        assert!(trail.verify_integrity());

        let events = trail.events_mut();
        events[2].payload = json!({ "seq": 999, "status": "tampered" });

        assert!(!trail.verify_integrity());
    }

    // -------------------------------------------------------------------
    // Block batcher tests
    // -------------------------------------------------------------------

    /// Test sink that collects sealed batches for inspection.
    #[derive(Debug)]
    struct TestBatchSink {
        batches: Arc<Mutex<Vec<Vec<AuditEvent>>>>,
    }

    impl TestBatchSink {
        fn new() -> (Self, Arc<Mutex<Vec<Vec<AuditEvent>>>>) {
            let batches = Arc::new(Mutex::new(Vec::new()));
            (
                Self {
                    batches: batches.clone(),
                },
                batches,
            )
        }
    }

    impl BlockBatchSink for TestBatchSink {
        fn seal_batch(&mut self, events: Vec<AuditEvent>) {
            self.batches.lock().unwrap().push(events);
        }
    }

    #[test]
    fn batcher_disabled_means_no_blocks() {
        let mut trail = AuditTrail::new();
        let agent_id = Uuid::new_v4();

        for i in 0..100 {
            trail
                .append_event(agent_id, EventType::StateChange, json!({"i": i}))
                .expect("audit append");
        }

        assert_eq!(trail.events().len(), 100);
        assert_eq!(trail.sealed_batch_count(), 0);
        assert_eq!(trail.pending_batch_count(), 0);
        assert!(trail.verify_integrity());
    }

    #[test]
    fn batcher_enabled_batches_events() {
        let (sink, batches) = TestBatchSink::new();
        let mut trail = AuditTrail::new();
        let agent_id = Uuid::new_v4();

        trail.enable_distributed_audit(
            BatcherConfig {
                max_events: 10,
                max_age_secs: 3600,
            },
            Box::new(sink),
        );

        // Append 25 events: should seal 2 batches of 10, with 5 pending
        for i in 0..25 {
            trail
                .append_event(agent_id, EventType::ToolCall, json!({"i": i}))
                .expect("audit append");
        }

        assert_eq!(trail.sealed_batch_count(), 2);
        assert_eq!(trail.pending_batch_count(), 5);

        // Verify batches contain correct event counts
        let sealed = batches.lock().unwrap();
        assert_eq!(sealed.len(), 2);
        assert_eq!(sealed[0].len(), 10);
        assert_eq!(sealed[1].len(), 10);

        // In-memory trail still has all 25
        assert_eq!(trail.events().len(), 25);
        assert!(trail.verify_integrity());
    }

    #[test]
    fn batcher_flush_seals_partial_batch() {
        let (sink, batches) = TestBatchSink::new();
        let mut trail = AuditTrail::new();
        let agent_id = Uuid::new_v4();

        trail.enable_distributed_audit(
            BatcherConfig {
                max_events: 50,
                max_age_secs: 3600,
            },
            Box::new(sink),
        );

        // Append 7 events (under threshold)
        for i in 0..7 {
            trail
                .append_event(agent_id, EventType::LlmCall, json!({"i": i}))
                .expect("audit append");
        }

        assert_eq!(trail.sealed_batch_count(), 0);
        assert_eq!(trail.pending_batch_count(), 7);

        // Manual flush
        trail.flush_batcher();

        assert_eq!(trail.sealed_batch_count(), 1);
        assert_eq!(trail.pending_batch_count(), 0);

        let sealed = batches.lock().unwrap();
        assert_eq!(sealed[0].len(), 7);
    }

    #[test]
    fn batcher_preserves_original_event_uuids() {
        let (sink, batches) = TestBatchSink::new();
        let mut trail = AuditTrail::new();
        let agent_id = Uuid::new_v4();

        trail.enable_distributed_audit(
            BatcherConfig {
                max_events: 5,
                max_age_secs: 3600,
            },
            Box::new(sink),
        );

        let mut event_ids = Vec::new();
        for i in 0..5 {
            let eid = trail
                .append_event(agent_id, EventType::StateChange, json!({"i": i}))
                .expect("audit append");
            event_ids.push(eid);
        }

        // Batch should be sealed with all 5 events
        let sealed = batches.lock().unwrap();
        assert_eq!(sealed.len(), 1);

        let batch_event_ids: Vec<Uuid> = sealed[0].iter().map(|e| e.event_id).collect();
        assert_eq!(batch_event_ids, event_ids);

        // Verify hashes are preserved too
        for (batch_event, trail_event) in sealed[0].iter().zip(trail.events().iter()) {
            assert_eq!(batch_event.hash, trail_event.hash);
            assert_eq!(batch_event.previous_hash, trail_event.previous_hash);
        }
    }
}
