//! Synthetic oracle wiring for harness scenarios.
//!
//! `nexus_swarm::oracle_bridge::testing::NullSwarmOracleBridge` covers
//! the "always approve, no audit" path. For scenarios that need a
//! specific `GovernanceDecision` (Approved, Denied, …) returned through
//! the real `SwarmOracleBridge::do_request_plan_approval` path, we
//! reproduce the pattern from
//! `crates/nexus-swarm/tests/oracle_bridge_tests.rs` (the
//! `oracle_with_decision` helper there is test-private).
//!
//! The construction shape is intentionally identical so behavior
//! matches between the in-tree bridge tests and the harness.

use nexus_crypto::{CryptoIdentity, SignatureAlgorithm};
use nexus_governance_oracle::{GovernanceDecision, GovernanceOracle, OracleRequest};
use nexus_swarm::oracle_bridge::{OracleBridge, SwarmOracleBridge};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;

/// Build an `Arc<dyn OracleBridge>` whose backing
/// `GovernanceOracle` always returns `decision` for every request.
///
/// The returned bridge runs a dedicated responder task that listens on
/// the oracle's request channel and replies with the canned decision.
/// The responder lives until the channel closes; a scenario that drops
/// the bridge before `Director::plan` returns will dangle the responder,
/// which is harmless.
pub fn oracle_returning(decision: GovernanceDecision) -> Arc<dyn OracleBridge> {
    let (tx, mut rx) = mpsc::channel::<OracleRequest>(32);
    let identity = CryptoIdentity::generate(SignatureAlgorithm::Ed25519)
        .expect("Ed25519 keygen for harness oracle");

    let oracle = Arc::new(GovernanceOracle::with_identity(
        tx,
        Duration::from_millis(50),
        identity,
    ));

    tokio::spawn(async move {
        while let Some(req) = rx.recv().await {
            let _ = req.response_tx.send(decision.clone());
        }
    });

    Arc::new(SwarmOracleBridge::with_timeout(
        oracle,
        Duration::from_secs(2),
    ))
}
