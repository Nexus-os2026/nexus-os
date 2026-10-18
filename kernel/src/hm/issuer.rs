//! The `DispatchIssuer` (P3-HM-CONTRACT-02 §11.0): §4.2 route-policy
//! resolution, run at the observation points of §4.6; unit start (§6.3,
//! §7.5); and the child-context derivations of §11.0 and §11.3.
//!
//! Inputs are traits so tests inject them and the desktop supplies the
//! production adapters: `AgentRegistry` (supervisor identity) and
//! `ConfigInput` (an untrusted config reading that can only tighten).

use std::collections::HashSet;
use std::fmt;
use std::sync::{Arc, Mutex};

use nexus_persistence::hm::{PolicyChangeRow, PolicyRow, PolicyRowRaw};
use nexus_persistence::NexusDatabase;
use uuid::Uuid;

use super::context::{
    unit_start_state, unit_taint_step, DispatchContext, PolicySnapshot, Principal, Restrictions,
    Unit, WorkingSet,
};
use super::{events, migrated};

/// `restrictions.max_records` of every context (§11.1).
pub const MAX_RECORDS: usize = 64;

/// The basis values a policy row may carry (§5.1).
const BASES: [&str; 4] = [
    "t0_baseline",
    "default_restrictive",
    "owner_confirmed",
    "tightened",
];

// ── Inputs ──────────────────────────────────────────────────────────────────

/// Supervisor identity (§2.5, §4.2 step 1).
pub trait AgentRegistry: Send + Sync {
    /// Whether this is a supervisor- or meta-registered agent UUID.
    fn is_registered(&self, agent_id: &Uuid) -> bool;
    /// The agent's registered names (config keys may name it).
    fn names(&self, agent_id: &Uuid) -> Vec<String>;
}

/// An untrusted reading of the config file for one agent (§3, §4.2 step 3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigReading {
    /// True when any assignment key matching the agent's id, normalized id
    /// or registered names says `local_only = true`.
    pub local_only: bool,
    /// Digest of the config content the reading came from.
    pub content_digest: String,
}

/// The config could not be read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConfigReadError;

/// The config file as an input (§4.1, §4.2 step 3). It can only tighten.
pub trait ConfigInput: Send + Sync {
    fn read_local_only(
        &self,
        agent_id: &Uuid,
        names: &[String],
    ) -> Result<ConfigReading, ConfigReadError>;
}

// ── Errors and probes ───────────────────────────────────────────────────────

/// No dispatch on any route (§4.2, §4.6, §16).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unresolved {
    /// §4.2 step 1: not a registered agent UUID.
    UnregisteredAgent,
    /// §4.2 step 2: `hm_migration` is not `COMPLETE`.
    StoreNotMigrated,
    /// §4.2 step 2: the store could not be read or written.
    StoreUnreadable,
    /// §4.2 step 3: the config could not be read.
    ConfigUnreadable,
    /// §4.2 step 4: a malformed policy row.
    MalformedPolicy,
    /// Only `Agent` principals are issued contexts by this module.
    UnsupportedPrincipal,
    /// A unit that can only be derived (`PostGoal`), or a derivation from
    /// the wrong kind of unit.
    UnsupportedUnit,
    /// A context minted by another issuer.
    WrongIssuer,
    /// No entropy for a context identity.
    NoEntropy,
}

impl Unresolved {
    /// Class name for event fields.
    pub fn reason(self) -> &'static str {
        match self {
            Unresolved::UnregisteredAgent => "unregistered_agent",
            Unresolved::StoreNotMigrated => "store_not_migrated",
            Unresolved::StoreUnreadable => "store_unreadable",
            Unresolved::ConfigUnreadable => "config_unreadable",
            Unresolved::MalformedPolicy => "malformed_policy",
            Unresolved::UnsupportedPrincipal => "unsupported_principal",
            Unresolved::UnsupportedUnit => "unsupported_unit",
            Unresolved::WrongIssuer => "wrong_issuer",
            Unresolved::NoEntropy => "no_entropy",
        }
    }
}

impl fmt::Display for Unresolved {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "route policy unresolved: {}", self.reason())
    }
}

impl std::error::Error for Unresolved {}

/// Points at which an installed probe is called. Used by the crash and
/// ordering tests; a probe observes only.
#[doc(hidden)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProbePoint {
    /// §4.2 step 5: a restrictive row committed; its event not yet written.
    PolicyRowCreated,
    /// §4.2 step 5: a config tightening committed; its event not yet written.
    PolicyTightened,
    /// Inside a labelled-write transaction: the record row is written.
    WriteRecordWritten,
    /// Inside a labelled-write transaction: its label row is written.
    WriteLabelWritten,
    /// Inside a labelled-write transaction: its derivation edges are
    /// written; next is COMMIT.
    WriteEdgesWritten,
}

type Probe = Box<dyn FnMut(ProbePoint) + Send>;

pub(in crate::hm) fn random_id() -> Result<[u8; 16], Unresolved> {
    let mut id = [0u8; 16];
    getrandom::getrandom(&mut id).map_err(|_| Unresolved::NoEntropy)?;
    Ok(id)
}

/// A stored row decoded, or `MalformedPolicy` (§4.2 step 4).
pub(in crate::hm) fn decode_policy(
    agent_id: Uuid,
    raw: &PolicyRowRaw,
) -> Result<PolicySnapshot, Unresolved> {
    let local_only = match raw.local_only {
        Some(0) => false,
        Some(1) => true,
        _ => return Err(Unresolved::MalformedPolicy),
    };
    match (
        raw.policy_generation,
        raw.revocation_seq,
        raw.basis.as_deref(),
    ) {
        (Some(g), Some(r), Some(b))
            if g >= 1 && r >= 0 && BASES.contains(&b) && raw.agent_id == agent_id.to_string() =>
        {
            Ok(PolicySnapshot {
                agent_id,
                local_only,
                policy_generation: g,
                revocation_seq: r,
            })
        }
        _ => Err(Unresolved::MalformedPolicy),
    }
}

enum Step5 {
    Created(PolicySnapshot),
    Tightened(PolicySnapshot),
    Unchanged(PolicySnapshot),
}

// ── DispatchIssuer ──────────────────────────────────────────────────────────

/// The single minting authority of dispatch contexts (§11.0). Constructed
/// once in production, when `AppState` is built.
pub struct DispatchIssuer {
    id: [u8; 16],
    db: Arc<NexusDatabase>,
    registry: Arc<dyn AgentRegistry>,
    config: Arc<dyn ConfigInput>,
    /// `(agent, config digest)` pairs already audited as a refused
    /// loosening. The production issuer is a process singleton, so this is
    /// "at most once per process" (§4.2 step 5).
    loosening_audited: Mutex<HashSet<(Uuid, String)>>,
    probe: Mutex<Option<Probe>>,
}

impl DispatchIssuer {
    pub fn new(
        db: Arc<NexusDatabase>,
        registry: Arc<dyn AgentRegistry>,
        config: Arc<dyn ConfigInput>,
    ) -> Result<Self, Unresolved> {
        Ok(Self {
            id: random_id()?,
            db,
            registry,
            config,
            loosening_audited: Mutex::new(HashSet::new()),
            probe: Mutex::new(None),
        })
    }

    /// Installs a probe called at each `ProbePoint`. Tests only.
    #[doc(hidden)]
    pub fn install_probe(&self, probe: Box<dyn FnMut(ProbePoint) + Send>) {
        *self.probe.lock().unwrap_or_else(|p| p.into_inner()) = Some(probe);
    }

    pub(in crate::hm) fn db(&self) -> &NexusDatabase {
        &self.db
    }

    /// Whether `ctx` was minted by this issuer.
    pub(in crate::hm) fn issued(&self, ctx: &DispatchContext) -> bool {
        ctx.issuer_id == self.id
    }

    pub(in crate::hm) fn hit(&self, point: ProbePoint) {
        if let Some(probe) = self
            .probe
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .as_mut()
        {
            probe(point);
        }
    }

    /// `resolve_agent_route_policy` (§4.2), run at every observation point
    /// (§4.6). A config reading can tighten, never loosen. Every write is
    /// its own `BEGIN IMMEDIATE`; its event follows the commit.
    pub fn resolve_agent_route_policy(&self, agent_id: Uuid) -> Result<PolicySnapshot, Unresolved> {
        // Step 1: identity.
        if !self.registry.is_registered(&agent_id) {
            return Err(Unresolved::UnregisteredAgent);
        }
        let agent = agent_id.to_string();

        // Step 2: store readiness.
        match self.db.hm().read(|tx| Ok(migrated(tx))) {
            Ok(Ok(true)) => {}
            Ok(Ok(false)) => return Err(Unresolved::StoreNotMigrated),
            _ => return Err(Unresolved::StoreUnreadable),
        }

        // Step 3: the untrusted config input, read with no transaction open.
        let names = self.registry.names(&agent_id);
        let cfg = self
            .config
            .read_local_only(&agent_id, &names)
            .map_err(|_| Unresolved::ConfigUnreadable)?;

        // Step 4: read the row.
        let stored = self
            .db
            .hm()
            .read(|tx| tx.policy_row_raw(&agent))
            .map_err(|_| Unresolved::StoreUnreadable)?;
        let current = stored
            .as_ref()
            .map(|raw| decode_policy(agent_id, raw))
            .transpose()?;

        // Step 5: write only if needed.
        let committed = match current {
            Some(p) if p.local_only || !cfg.local_only => p,
            _ => self.apply_step5(agent_id, cfg.local_only)?,
        };
        if !cfg.local_only && committed.local_only {
            self.audit_loosening_refused(committed, &cfg.content_digest);
        }

        // Step 6.
        Ok(committed)
    }

    /// §4.2 step 5 in one `BEGIN IMMEDIATE`: re-read, then create the row
    /// restrictive or apply the config tightening. History row first.
    fn apply_step5(&self, agent_id: Uuid, cfg_local: bool) -> Result<PolicySnapshot, Unresolved> {
        let agent = agent_id.to_string();
        let step = self
            .db
            .hm()
            .immediate(|tx| {
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
                    return Ok(Ok(Step5::Created(PolicySnapshot {
                        agent_id,
                        local_only: true,
                        policy_generation: 1,
                        revocation_seq: 0,
                    })));
                };
                let p = match decode_policy(agent_id, &raw) {
                    Ok(p) => p,
                    Err(e) => return Ok(Err(e)),
                };
                if p.local_only || !cfg_local {
                    return Ok(Ok(Step5::Unchanged(p)));
                }
                let generation = p.policy_generation + 1;
                tx.insert_policy_change(&PolicyChangeRow {
                    agent_id: agent.clone(),
                    policy_generation: generation,
                    local_only: 1,
                    basis: "tightened".into(),
                    cause: "config_tighten".into(),
                })?;
                tx.update_policy(&PolicyRow {
                    agent_id: agent.clone(),
                    local_only: 1,
                    policy_generation: generation,
                    revocation_seq: p.revocation_seq,
                    basis: "tightened".into(),
                })?;
                Ok(Ok(Step5::Tightened(PolicySnapshot {
                    local_only: true,
                    policy_generation: generation,
                    ..p
                })))
            })
            .map_err(|_| Unresolved::StoreUnreadable)??;

        // After the commit: the change stands whatever happens to its event.
        match step {
            Step5::Created(p) => {
                self.hit(ProbePoint::PolicyRowCreated);
                self.policy_event(p, "memory.policy_row_created");
                Ok(p)
            }
            Step5::Tightened(p) => {
                self.hit(ProbePoint::PolicyTightened);
                self.policy_event(p, "memory.policy_tightened_by_config");
                Ok(p)
            }
            Step5::Unchanged(p) => Ok(p),
        }
    }

    fn policy_event(&self, p: PolicySnapshot, kind: &str) {
        let agent = p.agent_id.to_string();
        events::after_commit(
            &self.db,
            &events::policy_subject(&agent, p.policy_generation),
            kind,
            Some(&agent),
            serde_json::json!({
                "policy_generation": p.policy_generation,
                "local_only": i64::from(p.local_only),
            }),
        );
    }

    /// §4.2 step 5, third case: no change; audited at most once per
    /// `(agent, config content digest)`.
    fn audit_loosening_refused(&self, p: PolicySnapshot, config_digest: &str) {
        let first = self
            .loosening_audited
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert((p.agent_id, config_digest.to_string()));
        if first {
            events::observational(
                &self.db,
                "memory.policy_loosening_not_authorised",
                Some(&p.agent_id.to_string()),
                serde_json::json!({ "policy_generation": p.policy_generation }),
            );
        }
    }

    /// O1 (§4.6): mints the context of a new unit. The starting taint and
    /// route class follow the policy as committed (§6.3, §7.5).
    pub fn begin_unit(
        &self,
        principal: Principal,
        unit: Unit,
    ) -> Result<DispatchContext, Unresolved> {
        let Principal::Agent(agent_id) = principal else {
            return Err(Unresolved::UnsupportedPrincipal);
        };
        if matches!(unit, Unit::PostGoal { .. }) {
            return Err(Unresolved::UnsupportedUnit);
        }
        let policy = self.resolve_agent_route_policy(agent_id)?;
        let (taint, route_class) = unit_start_state(policy.local_only, false);
        Ok(DispatchContext {
            nonce: random_id()?,
            issuer_id: self.id,
            principal,
            unit,
            policy,
            floor_local_only: false,
            route_class,
            taint,
            working_set: WorkingSet {
                verbatim: Vec::new(),
                derived: Vec::new(),
            },
            restrictions: Restrictions {
                local_only: policy.local_only,
                retained_data_permitted: !matches!(unit, Unit::ChatTurn { .. }),
                max_records: MAX_RECORDS,
            },
        })
    }
}

// ── Derivations (§11.0, §11.3) ──────────────────────────────────────────────

impl WorkingSet {
    fn duplicate(&self) -> WorkingSet {
        WorkingSet {
            verbatim: self.verbatim.clone(),
            derived: self.derived.clone(),
        }
    }
}

impl DispatchContext {
    /// The evolution context after a goal (§11.3): the goal's own policy
    /// snapshot, taint, route class and working set. No policy is
    /// re-resolved, so nothing can loosen.
    pub fn derive_post_goal(&self) -> Result<DispatchContext, Unresolved> {
        let Unit::Goal { goal_id } = self.unit else {
            return Err(Unresolved::UnsupportedUnit);
        };
        Ok(DispatchContext {
            nonce: random_id()?,
            issuer_id: self.issuer_id,
            principal: self.principal,
            unit: Unit::PostGoal { goal_id },
            policy: self.policy,
            floor_local_only: self.floor_local_only,
            route_class: self.route_class,
            taint: self.taint,
            working_set: self.working_set.duplicate(),
            restrictions: self.restrictions,
        })
    }

    /// A team member's task context (§11.3): the member's own §4.2 policy ⊔
    /// the director floor; the director's taint is inherited.
    pub fn derive_member(
        &self,
        issuer: &DispatchIssuer,
        member: Uuid,
        task_index: u32,
    ) -> Result<DispatchContext, Unresolved> {
        let Unit::TeamTask { workflow_id, .. } = self.unit else {
            return Err(Unresolved::UnsupportedUnit);
        };
        self.derive_child(
            issuer,
            member,
            Unit::TeamTask {
                workflow_id,
                task_index,
            },
        )
    }

    /// A HiveMind sub-task context (§11.3): the participant's own §4.2
    /// policy ⊔ the session floor; the session's taint is inherited.
    pub fn derive_subtask(
        &self,
        issuer: &DispatchIssuer,
        participant: Uuid,
        subtask_id: u32,
    ) -> Result<DispatchContext, Unresolved> {
        let Unit::HiveSubtask { session_id, .. } = self.unit else {
            return Err(Unresolved::UnsupportedUnit);
        };
        self.derive_child(
            issuer,
            participant,
            Unit::HiveSubtask {
                session_id,
                subtask_id,
            },
        )
    }

    fn derive_child(
        &self,
        issuer: &DispatchIssuer,
        agent_id: Uuid,
        unit: Unit,
    ) -> Result<DispatchContext, Unresolved> {
        if self.issuer_id != issuer.id {
            return Err(Unresolved::WrongIssuer);
        }
        // O1 for the child's own agent.
        let policy = issuer.resolve_agent_route_policy(agent_id)?;
        let floor = self.restrictions.local_only;
        let (start_taint, start_route) = unit_start_state(policy.local_only, floor);
        let (taint, route_class) = unit_taint_step(start_taint, start_route, self.taint);
        Ok(DispatchContext {
            nonce: random_id()?,
            issuer_id: self.issuer_id,
            principal: Principal::Agent(agent_id),
            unit,
            policy,
            floor_local_only: floor,
            route_class,
            taint,
            working_set: self.working_set.duplicate(),
            restrictions: Restrictions {
                local_only: policy.local_only || floor,
                retained_data_permitted: self.restrictions.retained_data_permitted,
                max_records: self.restrictions.max_records,
            },
        })
    }

    /// Joins a child's result into this context (§11.3, §14 rule 5): the
    /// taint joins the child's taint and the child's verbatim items enter
    /// the working set. Nothing is lowered.
    pub fn join_child(&mut self, child: &DispatchContext) -> Result<(), Unresolved> {
        if !self.same_issuer(child) {
            return Err(Unresolved::WrongIssuer);
        }
        let (taint, route_class) = unit_taint_step(self.taint, self.route_class, child.taint);
        self.taint = taint;
        self.route_class = route_class;
        self.working_set.derived.push(child.taint);
        self.working_set
            .verbatim
            .extend(child.working_set.verbatim.iter().cloned());
        Ok(())
    }
}
