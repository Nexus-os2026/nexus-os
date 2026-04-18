//! SwarmOracleBridge — the swarm's exclusive interface to the
//! GovernanceOracle.
//!
//! # Hybrid governance rationale
//!
//! This is more formally verifiable than per-invoke because the plan is one
//! reviewable artifact. Per-invoke governance reduces to rate-limited
//! middleware; class-based governance is what "architectural primitive"
//! means.
//!
//! The oracle approves a DAG once via `SealedToken`. The bridge unwraps the
//! token into a `SwarmTicket`. The `SwarmCoordinator` holds the ticket for
//! the lifetime of the run and does cheap local checks against the ticket's
//! privacy envelope and budget on each node. The oracle is re-invoked only
//! when one of the exhaustively enumerated [`HighRiskEvent`] variants
//! triggers, per [`HighRiskPolicy::should_recheck`]. The ticket's
//! `nonce: Uuid` threads through every provider-touching `SwarmEvent`,
//! giving us a cryptographic correlation between the event stream and the
//! oracle's hash-chained audit log.
//!
//! # CapabilityRequest parameter schema
//!
//! Non-standard fields are packed into `parameters: Value` under this exact
//! schema. Callers must not deviate; the oracle's audit log and any
//! downstream policy hot-swap relies on a stable shape:
//!
//! ```json
//! {
//!   "swarm": {
//!     "dag_content_hash": "<sha256 hex>",
//!     "privacy_envelope": "Public" | "Sensitive" | "StrictLocal",
//!     "caller_identity": { "public_key_hex": "...", "algorithm": "Ed25519" }
//!   }
//! }
//! ```
//!
//! `request_nonce` is the ticket's nonce field. `budget_hash` is a SHA-256
//! hex of the `Budget` fields.
//!
//! # Denial hints caveat
//!
//! `GovernanceDecision::Denied` carries no oracle-authored reason by
//! design — an oracle that leaks its internal reasoning is a side-channel
//! for callers to probe policy. When this bridge raises
//! `SwarmError::OraclePolicyDenied { hints }`, the hints are
//! **locally synthesized** from the denial class that was tripped (which
//! policy candidate was eliminated, which budget threshold was hit). They
//! are never claimed to be oracle-authored. UI surfaces that display hints
//! MUST attribute them to local analysis, not to the oracle.

use crate::budget::Budget;
use crate::dag::ExecutionDag;
use crate::error::SwarmError;
use crate::oracle_policy::{HighRiskEvent, OracleDecisionSummary, OracleDenial};
use crate::profile::PrivacyClass;
use nexus_crypto::{CryptoIdentity, SignatureAlgorithm};
use nexus_governance_oracle::{
    CapabilityRequest, GovernanceDecision, GovernanceOracle, OracleError, SealedToken,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fmt::Write as _;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use uuid::Uuid;

/// Default for how long the bridge waits on a single oracle submission.
/// Deliberately larger than the oracle's 100ms response ceiling — leaves
/// room for channel queuing without false timeouts under load.
pub const BRIDGE_RESPONSE_TIMEOUT: Duration = Duration::from_secs(5);

/// Opaque server-side correlation primitive for an approved plan. The
/// coordinator holds this across the run lifetime; only `ticket_id`,
/// `budget_hash`, and `privacy_envelope` ever cross the Tauri boundary.
#[derive(Debug, Clone)]
pub struct SwarmTicket {
    pub ticket_id: Uuid,
    pub nonce: Uuid,
    pub budget_hash: String,
    pub privacy_envelope: PrivacyClass,
    pub dag_content_hash: String,
    pub issued_at: SystemTime,
    pub token: SealedToken,
}

/// Minimal summary the coordinator hands to `finalize()`. Everything here
/// is already in the event stream; this is just a convenience bundle.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwarmSummary {
    pub run_id: Uuid,
    pub completed_nodes: usize,
    pub failed_nodes: usize,
    pub cancelled: bool,
}

/// The contract every swarm-facing oracle adapter must implement.
/// `SwarmOracleBridge` is the production impl; `NullSwarmOracleBridge`
/// (in `testing`) is the fixture. `Director::plan` and `SwarmCoordinator`
/// take `&dyn OracleBridge` / `Arc<dyn OracleBridge>` so either plugs in
/// without changing the orchestration path.
#[async_trait::async_trait]
pub trait OracleBridge: Send + Sync {
    async fn request_plan_approval(
        &self,
        dag: &ExecutionDag,
        budget: &Budget,
        caller: &CryptoIdentity,
    ) -> Result<SwarmTicket, SwarmError>;

    async fn check_highrisk(
        &self,
        ticket: &SwarmTicket,
        event: HighRiskEvent,
    ) -> Result<OracleDecisionSummary, OracleDenial>;

    async fn finalize(&self, ticket: SwarmTicket, summary: SwarmSummary);
}

/// The swarm's exclusive entry point to the oracle. Cheap to construct —
/// it just holds an `Arc<GovernanceOracle>` clone — so Tauri commands
/// build a fresh one per plan.
pub struct SwarmOracleBridge {
    oracle: Arc<GovernanceOracle>,
    response_timeout: Duration,
}

impl SwarmOracleBridge {
    /// Construct with the default 5s response timeout.
    pub fn new(oracle: Arc<GovernanceOracle>) -> Self {
        Self {
            oracle,
            response_timeout: BRIDGE_RESPONSE_TIMEOUT,
        }
    }

    /// Construct with a caller-specified timeout. Primarily for tests that
    /// want deterministic fast failure on ephemeral oracles.
    pub fn with_timeout(oracle: Arc<GovernanceOracle>, response_timeout: Duration) -> Self {
        Self {
            oracle,
            response_timeout,
        }
    }

    /// Internal helper: build + submit the plan-approval CapabilityRequest.
    /// See the `OracleBridge` impl below for the public entry point.
    async fn do_request_plan_approval(
        &self,
        dag: &ExecutionDag,
        budget: &Budget,
        caller: &CryptoIdentity,
    ) -> Result<SwarmTicket, SwarmError> {
        let dag_content_hash = dag_content_hash(dag);
        let privacy_envelope = most_restrictive_privacy(dag);
        let budget_hash = budget_hash(budget);
        let request_nonce = Uuid::new_v4();

        let parameters = serde_json::json!({
            "swarm": {
                "dag_content_hash": dag_content_hash,
                "privacy_envelope": privacy_class_label(privacy_envelope),
                "caller_identity": caller_identity_blob(caller),
            }
        });

        let request = CapabilityRequest {
            agent_id: "swarm.director".into(),
            capability: "swarm.plan_approval".into(),
            parameters,
            budget_hash: budget_hash.clone(),
            request_nonce: request_nonce.to_string(),
        };

        let token = self.submit_with_timeout(request).await?;
        let payload = self.verify(&token)?;

        match payload.decision {
            GovernanceDecision::Approved { .. } => Ok(SwarmTicket {
                ticket_id: Uuid::new_v4(),
                nonce: request_nonce,
                budget_hash,
                privacy_envelope,
                dag_content_hash,
                issued_at: SystemTime::now(),
                token,
            }),
            GovernanceDecision::Denied => Err(SwarmError::OraclePolicyDenied {
                hints: synthesize_plan_denial_hints(privacy_envelope, budget),
            }),
        }
    }

    async fn do_check_highrisk(
        &self,
        ticket: &SwarmTicket,
        event: HighRiskEvent,
    ) -> Result<OracleDecisionSummary, OracleDenial> {
        let request_nonce = Uuid::new_v4();

        let parameters = serde_json::json!({
            "swarm": {
                "ticket_id": ticket.ticket_id.to_string(),
                "ticket_nonce": ticket.nonce.to_string(),
                "dag_content_hash": ticket.dag_content_hash,
                "privacy_envelope": privacy_class_label(ticket.privacy_envelope),
                "event": event,
            }
        });

        let request = CapabilityRequest {
            agent_id: "swarm.coordinator".into(),
            capability: highrisk_capability_id(&event),
            parameters,
            budget_hash: ticket.budget_hash.clone(),
            request_nonce: request_nonce.to_string(),
        };

        let token = match self.submit_with_timeout(request).await {
            Ok(t) => t,
            Err(SwarmError::OracleUnreachable { detail }) => {
                return Err(OracleDenial {
                    hints: vec![format!("oracle unreachable: {detail}")],
                });
            }
            Err(other) => {
                return Err(OracleDenial {
                    hints: vec![format!("oracle transport error: {other}")],
                });
            }
        };

        let payload = match self.verify(&token) {
            Ok(p) => p,
            Err(e) => {
                return Err(OracleDenial {
                    hints: vec![format!("oracle unreachable: {e}")],
                });
            }
        };

        match payload.decision {
            GovernanceDecision::Approved { .. } => Ok(OracleDecisionSummary {
                approved: true,
                token_id: Uuid::parse_str(&token.token_id).ok(),
            }),
            GovernanceDecision::Denied => Err(OracleDenial {
                hints: synthesize_highrisk_denial_hints(&event),
            }),
        }
    }

    async fn do_finalize(&self, ticket: SwarmTicket, summary: SwarmSummary) {
        let request_nonce = Uuid::new_v4();
        let parameters = serde_json::json!({
            "swarm": {
                "ticket_id": ticket.ticket_id.to_string(),
                "ticket_nonce": ticket.nonce.to_string(),
                "summary": summary,
            }
        });
        let request = CapabilityRequest {
            agent_id: "swarm.coordinator".into(),
            capability: "swarm.finalize".into(),
            parameters,
            budget_hash: ticket.budget_hash,
            request_nonce: request_nonce.to_string(),
        };
        if let Err(e) = self.submit_with_timeout(request).await {
            tracing::warn!(
                target: "nexus_swarm::oracle_bridge",
                "finalize submission failed (non-fatal): {e}"
            );
        }
    }

    async fn submit_with_timeout(
        &self,
        request: CapabilityRequest,
    ) -> Result<SealedToken, SwarmError> {
        let fut = self.oracle.submit_request(request);
        match tokio::time::timeout(self.response_timeout, fut).await {
            Ok(Ok(token)) => Ok(token),
            Ok(Err(e)) => Err(oracle_error_to_swarm(&e)),
            Err(_) => Err(SwarmError::OracleUnreachable {
                detail: format!(
                    "bridge timeout after {}ms",
                    self.response_timeout.as_millis()
                ),
            }),
        }
    }

    fn verify(
        &self,
        token: &SealedToken,
    ) -> Result<nexus_governance_oracle::TokenPayload, SwarmError> {
        self.oracle
            .verify_token(token)
            .map_err(|e| SwarmError::OracleUnreachable {
                detail: format!("token verification failed: {e}"),
            })
    }

    pub fn oracle(&self) -> &Arc<GovernanceOracle> {
        &self.oracle
    }
}

#[async_trait::async_trait]
impl OracleBridge for SwarmOracleBridge {
    async fn request_plan_approval(
        &self,
        dag: &ExecutionDag,
        budget: &Budget,
        caller: &CryptoIdentity,
    ) -> Result<SwarmTicket, SwarmError> {
        self.do_request_plan_approval(dag, budget, caller).await
    }

    async fn check_highrisk(
        &self,
        ticket: &SwarmTicket,
        event: HighRiskEvent,
    ) -> Result<OracleDecisionSummary, OracleDenial> {
        self.do_check_highrisk(ticket, event).await
    }

    async fn finalize(&self, ticket: SwarmTicket, summary: SwarmSummary) {
        self.do_finalize(ticket, summary).await
    }
}

fn oracle_error_to_swarm(e: &OracleError) -> SwarmError {
    match e {
        OracleError::EngineUnavailable => SwarmError::OracleUnreachable {
            detail: "engine unavailable".into(),
        },
        OracleError::DecisionTimeout => SwarmError::OracleUnreachable {
            detail: "decision timeout".into(),
        },
        OracleError::SealingError(s) => SwarmError::OracleUnreachable {
            detail: format!("sealing error: {s}"),
        },
        OracleError::InvalidSignature => SwarmError::OracleUnreachable {
            detail: "invalid signature".into(),
        },
        OracleError::InvalidPayload(s) => SwarmError::OracleUnreachable {
            detail: format!("invalid payload: {s}"),
        },
    }
}

fn highrisk_capability_id(event: &HighRiskEvent) -> String {
    match event {
        HighRiskEvent::CloudCallAboveThreshold { .. } => "swarm.highrisk.cloud_call".into(),
        HighRiskEvent::SubagentSpawnAttempt { .. } => "swarm.highrisk.subagent_spawn".into(),
        HighRiskEvent::PrivacyClassEscalation { .. } => "swarm.highrisk.privacy_escalation".into(),
        HighRiskEvent::BudgetSoftLimitApproach { .. } => "swarm.highrisk.budget_soft_limit".into(),
        HighRiskEvent::PlanDrift { .. } => "swarm.highrisk.plan_drift".into(),
    }
}

fn privacy_class_label(p: PrivacyClass) -> &'static str {
    match p {
        PrivacyClass::Public => "Public",
        PrivacyClass::Sensitive => "Sensitive",
        PrivacyClass::StrictLocal => "StrictLocal",
    }
}

fn caller_identity_blob(caller: &CryptoIdentity) -> serde_json::Value {
    let alg = match caller.algorithm() {
        SignatureAlgorithm::Ed25519 => "Ed25519",
    };
    let mut hex = String::with_capacity(caller.verifying_key().len() * 2);
    for b in caller.verifying_key() {
        let _ = write!(hex, "{b:02x}");
    }
    serde_json::json!({
        "public_key_hex": hex,
        "algorithm": alg,
    })
}

/// Compute a stable content hash over the DAG's *shape* — every node's
/// id, capability_id, profile, inputs, plus the edge set — excluding the
/// per-node `status` field. Status changes during execution (Pending →
/// Running → Done); the ticket's hash must survive those transitions so
/// the plan-drift check only fires on real structural tampering.
pub fn dag_content_hash(dag: &ExecutionDag) -> String {
    let mut snap = dag.to_snapshot();
    for node in snap.nodes.iter_mut() {
        node.status = crate::dag::DagNodeStatus::Pending;
    }
    let bytes = serde_json::to_vec(&snap).unwrap_or_default();
    let digest = Sha256::digest(&bytes);
    bytes_to_hex(&digest)
}

fn budget_hash(budget: &Budget) -> String {
    let bytes = serde_json::to_vec(&serde_json::json!({
        "tokens": budget.tokens,
        "cost_cents": budget.cost_cents,
        "wall_ms": budget.wall_ms,
    }))
    .unwrap_or_default();
    let digest = Sha256::digest(&bytes);
    bytes_to_hex(&digest)
}

fn bytes_to_hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        let _ = write!(s, "{b:02x}");
    }
    s
}

/// Scan every DAG node's profile; the tightest `PrivacyClass` becomes the
/// envelope for the plan as a whole. `StrictLocal` > `Sensitive` > `Public`
/// where `>` means "more restrictive; dominates".
pub fn most_restrictive_privacy(dag: &ExecutionDag) -> PrivacyClass {
    let snapshot = dag.to_snapshot();
    let mut envelope = PrivacyClass::Public;
    for node in snapshot.nodes {
        envelope = tighter(envelope, node.profile.privacy);
    }
    envelope
}

fn tighter(a: PrivacyClass, b: PrivacyClass) -> PrivacyClass {
    use PrivacyClass::*;
    match (a, b) {
        (StrictLocal, _) | (_, StrictLocal) => StrictLocal,
        (Sensitive, _) | (_, Sensitive) => Sensitive,
        _ => Public,
    }
}

fn synthesize_plan_denial_hints(envelope: PrivacyClass, _budget: &Budget) -> Vec<String> {
    vec![
        format!(
            "plan denied — privacy envelope `{}` may violate active policy",
            privacy_class_label(envelope)
        ),
        "see oracle audit log for the decision's hash-chain entry".into(),
    ]
}

fn synthesize_highrisk_denial_hints(event: &HighRiskEvent) -> Vec<String> {
    match event {
        HighRiskEvent::CloudCallAboveThreshold {
            provider_id,
            estimated_cents,
        } => vec![
            format!(
                "cloud call to `{provider_id}` denied at runtime (estimated {estimated_cents}¢)"
            ),
            "consider a local provider or reduce max_tokens".into(),
        ],
        HighRiskEvent::SubagentSpawnAttempt { parent_node, depth } => vec![format!(
            "subagent spawn denied (parent={parent_node}, depth={depth}); not permitted in Phase 1"
        )],
        HighRiskEvent::PrivacyClassEscalation { from, to } => vec![format!(
            "privacy class escalation denied: {} → {}",
            privacy_class_label(*from),
            privacy_class_label(*to)
        )],
        HighRiskEvent::BudgetSoftLimitApproach { consumed_pct } => vec![format!(
            "budget soft-limit denial: {consumed_pct}% of approved budget consumed"
        )],
        HighRiskEvent::PlanDrift {
            original_hash,
            current_hash,
        } => vec![format!(
            "plan drift detected: approved hash {original_hash} ≠ current {current_hash}"
        )],
    }
}

/// Used for the assumed-current-time epoch in TokenPayload; exposed for
/// tests that inject a mock oracle and need to stamp payloads.
pub fn epoch_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

// ───────────────────────────────────────────────────────────────────────────
// Testing fixture
// ───────────────────────────────────────────────────────────────────────────

/// A null bridge that approves every plan and every high-risk check. Used
/// by existing director tests and by swarm-coordinator tests that are not
/// exercising oracle behavior. Exposed through `nexus_swarm::testing` so
/// downstream tests can reach it without the `test-fixtures` feature gate.
pub mod testing {
    use super::*;

    /// Minimal stand-in for a `SwarmOracleBridge`. Implements the same
    /// public surface but never touches an oracle.
    pub struct NullSwarmOracleBridge;

    impl NullSwarmOracleBridge {
        pub fn new() -> Self {
            Self
        }
    }

    #[async_trait::async_trait]
    impl super::OracleBridge for NullSwarmOracleBridge {
        /// Build a `SwarmTicket` with dummy values but a real SHA-256 over
        /// the DAG. Callers that check dag_content_hash round-trip still
        /// see a consistent value.
        async fn request_plan_approval(
            &self,
            dag: &ExecutionDag,
            budget: &Budget,
            _caller: &CryptoIdentity,
        ) -> Result<SwarmTicket, SwarmError> {
            Ok(null_ticket(dag, budget))
        }

        async fn check_highrisk(
            &self,
            _ticket: &SwarmTicket,
            _event: HighRiskEvent,
        ) -> Result<OracleDecisionSummary, OracleDenial> {
            Ok(OracleDecisionSummary {
                approved: true,
                token_id: Some(Uuid::nil()),
            })
        }

        async fn finalize(&self, _ticket: SwarmTicket, _summary: SwarmSummary) {}
    }

    impl Default for NullSwarmOracleBridge {
        fn default() -> Self {
            Self::new()
        }
    }

    fn null_ticket(dag: &ExecutionDag, budget: &Budget) -> SwarmTicket {
        SwarmTicket {
            ticket_id: Uuid::new_v4(),
            nonce: Uuid::new_v4(),
            budget_hash: super::budget_hash(budget),
            privacy_envelope: super::most_restrictive_privacy(dag),
            dag_content_hash: super::dag_content_hash(dag),
            issued_at: SystemTime::now(),
            token: SealedToken {
                payload: vec![],
                signature: vec![],
                token_id: Uuid::nil().to_string(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dag::{DagNode, DagNodeStatus};
    use crate::profile::TaskProfile;

    fn node_with_privacy(id: &str, privacy: PrivacyClass) -> DagNode {
        let mut profile = TaskProfile::public_heavy();
        profile.privacy = privacy;
        DagNode {
            id: id.into(),
            capability_id: "c".into(),
            profile,
            inputs: serde_json::Value::Null,
            status: DagNodeStatus::Pending,
        }
    }

    #[test]
    fn privacy_envelope_picks_tightest() {
        let mut dag = ExecutionDag::new();
        dag.add_node(node_with_privacy("a", PrivacyClass::Public))
            .unwrap();
        dag.add_node(node_with_privacy("b", PrivacyClass::Sensitive))
            .unwrap();
        assert_eq!(most_restrictive_privacy(&dag), PrivacyClass::Sensitive);

        dag.add_node(node_with_privacy("c", PrivacyClass::StrictLocal))
            .unwrap();
        assert_eq!(most_restrictive_privacy(&dag), PrivacyClass::StrictLocal);
    }

    #[test]
    fn dag_content_hash_is_stable_for_same_dag() {
        let mut dag = ExecutionDag::new();
        dag.add_node(node_with_privacy("a", PrivacyClass::Public))
            .unwrap();
        let h1 = dag_content_hash(&dag);
        let h2 = dag_content_hash(&dag);
        assert_eq!(h1, h2);
        assert_eq!(h1.len(), 64, "sha256 hex = 64 chars");
    }

    #[test]
    fn dag_content_hash_changes_with_new_node() {
        let mut dag = ExecutionDag::new();
        dag.add_node(node_with_privacy("a", PrivacyClass::Public))
            .unwrap();
        let h1 = dag_content_hash(&dag);
        dag.add_node(node_with_privacy("b", PrivacyClass::Public))
            .unwrap();
        let h2 = dag_content_hash(&dag);
        assert_ne!(h1, h2);
    }
}
