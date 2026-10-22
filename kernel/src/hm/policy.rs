//! Governed route-policy changes (P3-HM-CONTRACT-02 §4.3–§4.6, §7.1, §15.3,
//! §16; mission P3-C11-HM-M4 N2).
//!
//! - A governed tighten: the store first (`local_only = 1`, generation + 1,
//!   its `hm_policy_change` row with cause `governed_tighten`, one
//!   `BEGIN IMMEDIATE`); its `hm_event` after the commit. A missing event is
//!   reported and back-filled; the tightening stands. If the transaction
//!   cannot commit, the failure is reported (never success) and the issuer
//!   holds the agent local-only for the life of the process (§15.3 latch).
//! - An Owner loosening only through a native confirmation the kernel
//!   requests itself: the request is built from registry data, an
//!   [`RoutePolicyConfirmer`] is asked, and only its true answer mints the
//!   sealed [`OwnerConfirmation`] the store commit consumes. Then (§4.5) the
//!   config mirror, then the store transaction with its `hm_event` inside.
//!   No confirmer, a dialog failure, a refusal or any failure leaves the
//!   agent local-only.

use nexus_persistence::hm::{PolicyChangeRow, PolicyRow};
use uuid::Uuid;

use super::context::PolicySnapshot;
use super::events;
use super::issuer::{decode_policy, DispatchIssuer, ProbePoint, Unresolved};
use super::write::is_migrated;

/// The consequence of a loosening, in the words of §4.4.
pub const LOOSENING_CONSEQUENCE: &str = "New work by this agent may be sent to non-local \
providers. Records made while it was local-only are not released.";

/// What the Owner is asked to confirm before a loosening (§4.4): built by
/// the kernel from registry data, never from a caller's text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LooseningRequest {
    agent_name: String,
    uuid_prefix: String,
}

impl LooseningRequest {
    /// The dialog title.
    pub fn title(&self) -> &'static str {
        "Allow non-local providers for this agent?"
    }

    /// The dialog text: the agent's registered name and UUID prefix, the
    /// exact scope, and the consequence of §4.4 verbatim.
    pub fn message(&self) -> String {
        format!(
            "Agent: {} ({})\nScope: this agent's route policy, from local-only to unrestricted, \
for its new work only.\n{LOOSENING_CONSEQUENCE}",
            self.agent_name, self.uuid_prefix
        )
    }

    /// The agent's registered name.
    pub fn agent_name(&self) -> &str {
        &self.agent_name
    }

    /// The first eight characters of the agent's UUID.
    pub fn uuid_prefix(&self) -> &str {
        &self.uuid_prefix
    }
}

/// The Owner's native confirmation of a loosening (§2.10, §4.4): rendered
/// and invoked by backend code, never by the webview.
pub trait RoutePolicyConfirmer: Send + Sync {
    /// True only on the Owner's explicit confirmation. A dialog that cannot
    /// be shown, or no answer, is false.
    fn confirm_loosening(&self, request: &LooseningRequest) -> bool;
}

/// The proof that the Owner confirmed one loosening (§4.4): minted only
/// inside [`DispatchIssuer::request_loosening`] from a true answer of the
/// confirmer, and consumed by its store commit. No caller can construct
/// one: no IPC argument, config value, record content or model output
/// produces it.
///
/// ```compile_fail
/// use nexus_kernel::hm::OwnerConfirmation;
/// let _ = OwnerConfirmation { issuer_id: [0; 16], agent_id: uuid::Uuid::nil(), from_generation: 1 };
/// ```
/// ```compile_fail
/// use nexus_kernel::hm::OwnerConfirmation;
/// let _: OwnerConfirmation = Default::default();
/// ```
/// ```compile_fail
/// use nexus_kernel::hm::OwnerConfirmation;
/// let _ = "x".parse::<OwnerConfirmation>();
/// ```
/// ```compile_fail
/// use nexus_kernel::hm::OwnerConfirmation;
/// let _ = OwnerConfirmation::from(true);
/// ```
/// ```compile_fail
/// use nexus_kernel::hm::OwnerConfirmation;
/// fn dup(p: &OwnerConfirmation) -> OwnerConfirmation { OwnerConfirmation::clone(p) }
/// ```
/// ```compile_fail
/// use nexus_kernel::hm::OwnerConfirmation;
/// let _ = serde_json::from_str::<OwnerConfirmation>("{}");
/// ```
#[derive(Debug)]
pub struct OwnerConfirmation {
    issuer_id: [u8; 16],
    agent_id: Uuid,
    from_generation: i64,
}

/// A governed tighten as committed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tightened {
    /// The policy after the tighten.
    pub policy: PolicySnapshot,
    /// False when the agent was already local-only (nothing was written).
    pub changed: bool,
    /// False when the change's `hm_event` could not be written after the
    /// commit (the change stands; it is back-filled).
    pub event_recorded: bool,
}

/// A governed tighten that did not commit: nothing is claimed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TightenFailed {
    /// The agent is not a registered agent: there is nothing to tighten.
    UnregisteredAgent,
    /// The store could not commit the tightening (or could not be read). The
    /// issuer holds the agent local-only for the life of the process.
    NotCommitted,
}

impl std::fmt::Display for TightenFailed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TightenFailed::UnregisteredAgent => write!(f, "not a registered agent"),
            TightenFailed::NotCommitted => write!(
                f,
                "the tightening was not committed; the agent is held local-only for this process"
            ),
        }
    }
}

impl std::error::Error for TightenFailed {}

/// An Owner loosening as committed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Loosened {
    /// The policy after the loosening.
    pub policy: PolicySnapshot,
    /// False when the agent was already unrestricted (nothing was asked,
    /// mirrored or written).
    pub changed: bool,
    /// The config mirror write (§4.5 step 1): a failure is reported here and
    /// never changes the store decision.
    pub mirror: Result<(), String>,
}

/// A refused loosening: the agent stays local-only.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LooseningRefused {
    /// §4.2 could not establish the agent's policy.
    Unresolved(Unresolved),
    /// The agent is held local-only for this process (§15.3).
    Latched,
    /// No native confirmer exists (headless, tests without the test
    /// confirmer, a platform with no dialog): §4.4.
    NoConfirmer,
    /// The Owner did not confirm, or the dialog failed.
    Declined,
    /// The policy changed while the Owner was asked; nothing is applied
    /// (§8 item 5: the commit re-reads current state).
    PolicyChanged,
    /// The loosening transaction, or its `hm_event`, could not commit
    /// (§15.3): the agent stays local-only.
    NotCommitted,
}

impl std::fmt::Display for LooseningRefused {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LooseningRefused::Unresolved(u) => write!(f, "{u}"),
            LooseningRefused::Latched => {
                write!(f, "the agent is held local-only for this process")
            }
            LooseningRefused::NoConfirmer => {
                write!(f, "no native Owner confirmation is available here")
            }
            LooseningRefused::Declined => write!(f, "the Owner did not confirm"),
            LooseningRefused::PolicyChanged => {
                write!(f, "the agent's policy changed while the Owner was asked")
            }
            LooseningRefused::NotCommitted => write!(f, "the loosening could not be committed"),
        }
    }
}

impl std::error::Error for LooseningRefused {}

/// The outcome of a governed tighten's transaction.
enum TightenStep {
    Created(PolicySnapshot),
    Tightened(PolicySnapshot),
    Unchanged(PolicySnapshot),
}

impl DispatchIssuer {
    /// A governed tighten of a registered agent (§4.4, §4.5 row 1, §7.1):
    /// the store first, in one `BEGIN IMMEDIATE`; its `hm_event` after the
    /// commit, never a condition of it. A missing row is created restrictive
    /// (§4.2 step 5). If the transaction cannot commit the agent is latched
    /// local-only for the life of the process and no success is reported.
    pub fn governed_tighten(&self, agent_id: Uuid) -> Result<Tightened, TightenFailed> {
        if !self.registry_has(&agent_id) {
            return Err(TightenFailed::UnregisteredAgent);
        }
        let agent = agent_id.to_string();
        let step = self.db().hm().immediate(|tx| {
            if !is_migrated(tx)? {
                return Ok(None);
            }
            let Some(raw) = tx.policy_row_raw(&agent)? else {
                tx.insert_policy_change(&PolicyChangeRow {
                    agent_id: agent.clone(),
                    policy_generation: 1,
                    local_only: 1,
                    basis: "default_restrictive".into(),
                    cause: "row_created".into(),
                })?;
                tx.insert_policy(&PolicyRow {
                    agent_id: agent.clone(),
                    local_only: 1,
                    policy_generation: 1,
                    revocation_seq: 0,
                    basis: "default_restrictive".into(),
                })?;
                return Ok(Some(TightenStep::Created(PolicySnapshot {
                    agent_id,
                    local_only: true,
                    policy_generation: 1,
                    revocation_seq: 0,
                })));
            };
            let Ok(p) = decode_policy(agent_id, &raw) else {
                return Ok(None);
            };
            if p.local_only {
                return Ok(Some(TightenStep::Unchanged(p)));
            }
            let generation = p.policy_generation + 1;
            tx.insert_policy_change(&PolicyChangeRow {
                agent_id: agent.clone(),
                policy_generation: generation,
                local_only: 1,
                basis: "tightened".into(),
                cause: "governed_tighten".into(),
            })?;
            tx.update_policy(&PolicyRow {
                agent_id: agent.clone(),
                local_only: 1,
                policy_generation: generation,
                revocation_seq: p.revocation_seq,
                basis: "tightened".into(),
            })?;
            Ok(Some(TightenStep::Tightened(PolicySnapshot {
                local_only: true,
                policy_generation: generation,
                ..p
            })))
        });
        let step = match step {
            Ok(Some(step)) => step,
            // Not migrated, malformed, unreadable, or the commit failed: the
            // tightening has not happened durably (§15.3).
            Ok(None) | Err(_) => {
                self.latch(agent_id);
                return Err(TightenFailed::NotCommitted);
            }
        };
        // After the commit: the change stands whatever happens to its event.
        let (policy, kind) = match step {
            TightenStep::Unchanged(p) => {
                return Ok(Tightened {
                    policy: p,
                    changed: false,
                    event_recorded: true,
                })
            }
            TightenStep::Created(p) => (p, "memory.policy_row_created"),
            TightenStep::Tightened(p) => (p, "memory.policy_tightened"),
        };
        self.hit(ProbePoint::GovernedTightenCommitted);
        let event_recorded = events::after_commit(
            self.db(),
            &events::policy_subject(&agent, policy.policy_generation),
            kind,
            Some(&agent),
            serde_json::json!({
                "policy_generation": policy.policy_generation,
                "local_only": 1,
            }),
        );
        if !event_recorded {
            let _ = events::backfill_authority_events(self.db());
        }
        Ok(Tightened {
            policy,
            changed: true,
            event_recorded,
        })
    }

    /// An Owner loosening (§4.4, §4.5 row 2, §7.1), only through the native
    /// confirmation the kernel requests here:
    ///
    /// 1. §4.2 for the agent (a latched agent is refused);
    /// 2. the request from registry data, shown by `confirmer` with no lock
    ///    held and no transaction open (§8 item 5); none, a failure or a
    ///    refusal → refused;
    /// 3. the config mirror (`mirror`, §4.5 step 1), whose failure is
    ///    reported and never changes the store decision;
    /// 4. the store transaction: it re-reads the policy, writes the
    ///    `owner_loosen` history row, the policy row and the
    ///    `memory.policy_loosened` event together; any failure → refused,
    ///    the agent stays local-only (§15.3).
    pub fn request_loosening(
        &self,
        agent_id: Uuid,
        confirmer: Option<&dyn RoutePolicyConfirmer>,
        mirror: &mut dyn FnMut() -> Result<(), String>,
    ) -> Result<Loosened, LooseningRefused> {
        if self.is_latched(agent_id) {
            return Err(LooseningRefused::Latched);
        }
        let current = self
            .resolve_agent_route_policy(agent_id)
            .map_err(LooseningRefused::Unresolved)?;
        if !current.local_only {
            return Ok(Loosened {
                policy: current,
                changed: false,
                mirror: Ok(()),
            });
        }
        let request = LooseningRequest {
            agent_name: self
                .registry_names(&agent_id)
                .into_iter()
                .next()
                .unwrap_or_else(|| "unnamed agent".to_string()),
            uuid_prefix: agent_id.to_string()[..8].to_string(),
        };
        let Some(confirmer) = confirmer else {
            return Err(LooseningRefused::NoConfirmer);
        };
        if !confirmer.confirm_loosening(&request) {
            return Err(LooseningRefused::Declined);
        }
        let proof = OwnerConfirmation {
            issuer_id: self.issuer_identity(),
            agent_id,
            from_generation: current.policy_generation,
        };
        let mirrored = mirror();
        let policy = self.commit_loosening(proof)?;
        Ok(Loosened {
            policy,
            changed: true,
            mirror: mirrored,
        })
    }

    /// The store transaction of a confirmed loosening (§4.5 step 2): the
    /// history row, the policy row and the event, all in one
    /// `BEGIN IMMEDIATE`. It needs the Owner's sealed confirmation.
    fn commit_loosening(
        &self,
        proof: OwnerConfirmation,
    ) -> Result<PolicySnapshot, LooseningRefused> {
        if proof.issuer_id != self.issuer_identity() {
            return Err(LooseningRefused::NotCommitted);
        }
        let agent_id = proof.agent_id;
        let agent = agent_id.to_string();
        let committed = self.db().hm().immediate(|tx| {
            if !is_migrated(tx)? {
                return Ok(Err(LooseningRefused::Unresolved(
                    Unresolved::StoreNotMigrated,
                )));
            }
            let p = match tx
                .policy_row_raw(&agent)?
                .map(|raw| decode_policy(agent_id, &raw))
            {
                Some(Ok(p)) => p,
                _ => return Ok(Err(LooseningRefused::PolicyChanged)),
            };
            if !p.local_only || p.policy_generation != proof.from_generation {
                return Ok(Err(LooseningRefused::PolicyChanged));
            }
            let generation = p.policy_generation + 1;
            tx.insert_policy_change(&PolicyChangeRow {
                agent_id: agent.clone(),
                policy_generation: generation,
                local_only: 0,
                basis: "owner_confirmed".into(),
                cause: "owner_loosen".into(),
            })?;
            tx.update_policy(&PolicyRow {
                agent_id: agent.clone(),
                local_only: 0,
                policy_generation: generation,
                revocation_seq: p.revocation_seq,
                basis: "owner_confirmed".into(),
            })?;
            tx.insert_event(
                &events::policy_subject(&agent, generation),
                "memory.policy_loosened",
                Some(&agent),
                &serde_json::json!({
                    "policy_generation": generation,
                    "local_only": 0,
                })
                .to_string(),
            )?;
            Ok(Ok(PolicySnapshot {
                local_only: false,
                policy_generation: generation,
                ..p
            }))
        });
        match committed {
            Ok(outcome) => outcome,
            // The history row, the policy row or the event failed: nothing
            // committed; the agent stays local-only (§15.3).
            Err(_) => Err(LooseningRefused::NotCommitted),
        }
    }
}
