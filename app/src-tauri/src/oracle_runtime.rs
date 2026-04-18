//! Production GovernanceOracle runtime wiring.
//!
//! Phase 1.5a: the desktop backend must ship an actually-running
//! `GovernanceOracle` + `DecisionEngine` pair. Prior to this module the app
//! held a `GovernanceRuleset` and a `DecisionAuditLog` on `AppState` but
//! never spawned a `DecisionEngine` task, which meant `GovernanceOracle`
//! was only ever constructed inside unit tests — no subsystem could
//! actually obtain a governance decision at runtime.
//!
//! Architecture
//! ------------
//! The `DecisionEngine::run` loop owns its `mpsc::Receiver<OracleRequest>`.
//! Callers (swarm, future cost_ceiling migrations, etc.) need the paired
//! `Sender` to submit capability requests. This module owns both sides
//! via a thin relay:
//!
//!   caller ──► external_tx ══► relay task ──► engine_tx ══► DecisionEngine
//!              (in AppState)                    (internal)
//!
//! The relay repacks each `OracleRequest`, forwards it to the engine,
//! awaits the engine's decision on an internal oneshot, increments the
//! processed counter, and forwards the decision to the caller's original
//! oneshot. This gives us an externally-visible counter without modifying
//! the `nexus-governance-engine` crate.
//!
//! Shutdown is cascading: drop `OracleRuntime` → external_tx drops →
//! relay's `external_rx.recv()` returns `None` → relay exits → engine_tx
//! drops → engine's `request_rx.recv()` returns `None` → engine task
//! exits. For tests that want to drive shutdown explicitly, use
//! [`OracleRuntime::shutdown`].

use nexus_governance_engine::{DecisionEngine, GovernanceRuleset};
use nexus_governance_oracle::{GovernanceDecision, OracleRequest};
use serde::Serialize;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

/// Channel buffer for in-flight oracle requests. Bounded to expose
/// backpressure — a flooded channel will make `Sender::send` await.
pub const ORACLE_CHANNEL_BUFFER: usize = 256;

/// Snapshot returned by the `oracle_runtime_status` Tauri command.
#[derive(Debug, Clone, Serialize)]
pub struct OracleRuntimeStatus {
    pub is_running: bool,
    pub pending_requests: usize,
    pub total_processed: u64,
    pub uptime_seconds: u64,
}

/// Long-lived governance oracle runtime. Stored on `AppState` as
/// `Arc<OracleRuntime>`; the sender is cloned out via
/// [`OracleRuntime::sender`] for downstream subsystems.
///
/// If `start` is called outside a tokio runtime (e.g. from a synchronous
/// unit test that only wants `AppState::new_in_memory()` for its DB
/// fixture), the runtime falls back to a *stub* mode: the tasks are not
/// spawned, `is_running()` returns `false`, and the external channel
/// receiver is parked on the struct so `sender().send()` up to the buffer
/// capacity still succeeds. Actual decisions never arrive in stub mode —
/// callers must not await them. Production always has a tokio runtime.
pub struct OracleRuntime {
    external_tx: mpsc::Sender<OracleRequest>,
    processed: Arc<AtomicU64>,
    started_at: Instant,
    relay_handle: std::sync::Mutex<Option<JoinHandle<()>>>,
    engine_handle: std::sync::Mutex<Option<JoinHandle<()>>>,
    /// Receiver parked here in stub mode so the channel stays alive.
    /// `None` in normal mode (the receiver was moved into the relay task).
    _stub_rx: std::sync::Mutex<Option<mpsc::Receiver<OracleRequest>>>,
}

impl OracleRuntime {
    /// Start the runtime: spawn the relay task and the DecisionEngine task.
    /// Logs a startup line and returns an `Arc<Self>` ready to be placed on
    /// `AppState`.
    ///
    /// When called outside a tokio runtime context, falls back to the stub
    /// mode described on [`OracleRuntime`].
    pub fn start(ruleset: GovernanceRuleset) -> Arc<Self> {
        let (external_tx, external_rx) = mpsc::channel::<OracleRequest>(ORACLE_CHANNEL_BUFFER);
        let processed = Arc::new(AtomicU64::new(0));
        let started_at = Instant::now();

        if tokio::runtime::Handle::try_current().is_err() {
            eprintln!(
                "[startup] GovernanceOracle runtime in stub mode — no tokio runtime in context"
            );
            return Arc::new(Self {
                external_tx,
                processed,
                started_at,
                relay_handle: std::sync::Mutex::new(None),
                engine_handle: std::sync::Mutex::new(None),
                _stub_rx: std::sync::Mutex::new(Some(external_rx)),
            });
        }

        let (engine_tx, engine_rx) = mpsc::channel::<OracleRequest>(ORACLE_CHANNEL_BUFFER);
        let processed_relay = Arc::clone(&processed);
        let mut external_rx = external_rx;

        let relay_handle = tokio::spawn(async move {
            while let Some(req) = external_rx.recv().await {
                let OracleRequest {
                    request,
                    response_tx: original_response,
                } = req;
                let (inner_tx, inner_rx) = tokio::sync::oneshot::channel();
                let repacked = OracleRequest {
                    request,
                    response_tx: inner_tx,
                };
                if engine_tx.send(repacked).await.is_err() {
                    let _ = original_response.send(GovernanceDecision::Denied);
                    break;
                }
                match inner_rx.await {
                    Ok(decision) => {
                        processed_relay.fetch_add(1, Ordering::Relaxed);
                        let _ = original_response.send(decision);
                    }
                    Err(_) => {
                        let _ = original_response.send(GovernanceDecision::Denied);
                    }
                }
            }
        });

        let engine_handle = tokio::spawn(async move {
            let mut engine = DecisionEngine::new(engine_rx, ruleset);
            engine.run().await;
        });

        eprintln!(
            "[startup] GovernanceOracle runtime started on mpsc channel (buffer={})",
            ORACLE_CHANNEL_BUFFER
        );

        Arc::new(Self {
            external_tx,
            processed,
            started_at,
            relay_handle: std::sync::Mutex::new(Some(relay_handle)),
            engine_handle: std::sync::Mutex::new(Some(engine_handle)),
            _stub_rx: std::sync::Mutex::new(None),
        })
    }

    /// Obtain a cloned `Sender<OracleRequest>` for submitting capability
    /// requests. This is the one public handle downstream subsystems use;
    /// they never see the relay or the engine directly.
    pub fn sender(&self) -> mpsc::Sender<OracleRequest> {
        self.external_tx.clone()
    }

    /// Current runtime snapshot for the `oracle_runtime_status` command.
    ///
    /// `pending_requests` is measured from the external channel only
    /// (caller-visible queue). The engine's internal channel drains fast
    /// enough that including it adds noise without signal.
    pub fn status(&self) -> OracleRuntimeStatus {
        let max = self.external_tx.max_capacity();
        let free = self.external_tx.capacity();
        let pending = max.saturating_sub(free);
        OracleRuntimeStatus {
            is_running: self.is_running(),
            pending_requests: pending,
            total_processed: self.processed.load(Ordering::Relaxed),
            uptime_seconds: self.started_at.elapsed().as_secs(),
        }
    }

    /// Running if the relay and engine tasks are still alive.
    pub fn is_running(&self) -> bool {
        let relay_alive = match self.relay_handle.lock() {
            Ok(guard) => guard.as_ref().map(|h| !h.is_finished()).unwrap_or(false),
            Err(_) => false,
        };
        let engine_alive = match self.engine_handle.lock() {
            Ok(guard) => guard.as_ref().map(|h| !h.is_finished()).unwrap_or(false),
            Err(_) => false,
        };
        relay_alive && engine_alive
    }

    pub fn total_processed(&self) -> u64 {
        self.processed.load(Ordering::Relaxed)
    }

    /// Abort the relay and engine tasks. Used on app shutdown and by tests
    /// that want deterministic teardown. After this call, the runtime
    /// cannot accept new requests.
    pub fn shutdown(&self) {
        if let Ok(mut guard) = self.relay_handle.lock() {
            if let Some(h) = guard.take() {
                h.abort();
            }
        }
        if let Ok(mut guard) = self.engine_handle.lock() {
            if let Some(h) = guard.take() {
                h.abort();
            }
        }
        eprintln!(
            "[shutdown] GovernanceOracle runtime stopped (total_processed={})",
            self.processed.load(Ordering::Relaxed)
        );
    }
}
