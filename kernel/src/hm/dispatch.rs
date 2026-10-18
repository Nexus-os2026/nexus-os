//! The authorisation point and reply acceptance (P3-HM-CONTRACT-02 §12.4–
//! §12.7). The cloud-egress gate.
//!
//! Admission: observation point O2 (§4.6; a tightening commits in its own
//! transaction), then one `BEGIN IMMEDIATE` that re-reads the policy,
//! checks the generation (one rebuild, then `generation_unstable`), checks
//! the route class against the `RouteAttestation`, checks eligibility,
//! inserts the `hm_dispatch_admission` row and its `hm_event`, and commits.
//! That commit is the linearization point (§12.5); every failure refuses.
//! Under AD-4 there are no grant dependencies.
//!
//! Reply acceptance: O3, then a new read transaction (§12.6). The prompt
//! bytes are released only by an `Admission`.

use std::fmt;

use nexus_persistence::hm::{AdmissionRow, HmTx};
use nexus_persistence::Result as StoreResult;
use uuid::Uuid;

use super::context::{
    unit_taint_step, DerivedItem, DispatchContext, Principal, PromptEnvelope, RouteClass,
};
use super::events;
use super::issuer::{decode_policy, random_id, DispatchIssuer, ProbePoint, Unresolved};
use super::write::{evaluate_ref, is_migrated};
use super::OriginLabel;

// ── Transport attestation (§2.4, §11.0) ─────────────────────────────────────

/// Evidence about the transport actually selected, produced by the
/// connectors from the provider object itself. It carries no route class
/// and no label.
#[doc(hidden)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportEvidence {
    /// (L1) the verified loopback Ollama runtime over `DirectLoopback`.
    VerifiedLoopbackOllama,
    /// (L2) a Flash provider with a complete in-process attestation.
    VerifiedInProcessFlash,
    /// No complete attestation.
    Unattested,
}

/// The attested route class of the transport actually selected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RouteAttestation {
    class: RouteClass,
}

impl RouteAttestation {
    /// Only (L1) and (L2) evidence is `local`; everything else is
    /// `non_local`. Called by the connectors' `attest_route`.
    #[doc(hidden)]
    pub fn from_transport_evidence(evidence: TransportEvidence) -> Self {
        let class = match evidence {
            TransportEvidence::VerifiedLoopbackOllama
            | TransportEvidence::VerifiedInProcessFlash => RouteClass::Local,
            TransportEvidence::Unattested => RouteClass::NonLocal,
        };
        Self { class }
    }

    pub fn class(&self) -> RouteClass {
        self.class
    }
}

// ── Admission ───────────────────────────────────────────────────────────────

/// A committed admission (§12.5). It is the only holder of the prompt
/// bytes; it is consumed by reply acceptance.
pub struct Admission {
    admission_id: String,
    issuer_id: [u8; 16],
    ctx_nonce: [u8; 16],
    agent_id: Uuid,
    route_class: RouteClass,
    policy_generation: i64,
    revocation_seq: i64,
    raw_label: OriginLabel,
    text: String,
}

/// Content-free: the prompt is never formatted.
impl fmt::Debug for Admission {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Admission")
            .field("admission_id", &self.admission_id)
            .field("route_class", &self.route_class)
            .field("policy_generation", &self.policy_generation)
            .field("raw_label", &self.raw_label)
            .finish_non_exhaustive()
    }
}

impl Admission {
    pub fn admission_id(&self) -> &str {
        &self.admission_id
    }

    /// The attested class of the transport this admission is for.
    pub fn route_class(&self) -> RouteClass {
        self.route_class
    }

    /// The label of the reply (§11.2), evaluated in the admission
    /// transaction.
    pub fn raw_label(&self) -> OriginLabel {
        self.raw_label
    }

    pub fn policy_generation(&self) -> i64 {
        self.policy_generation
    }

    pub fn revocation_seq(&self) -> i64 {
        self.revocation_seq
    }

    /// The admitted prompt, to hand to the transport with no lock held.
    pub fn prompt(&self) -> &str {
        &self.text
    }
}

/// A refused dispatch: nothing is transmitted (§12.4, §16).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refused {
    WrongIssuer,
    NotAnAgent,
    /// The envelope was not built from this context.
    EnvelopeMismatch,
    /// O2 or the rebuild could not resolve the policy (§4.6).
    Unresolved(Unresolved),
    NotMigrated,
    /// The policy row is missing or malformed inside the transaction.
    PolicyUnavailable,
    /// The attested transport class does not fit the unit (§12.4 step 2).
    RouteMismatch,
    /// A non-local dispatch with restricted taint, carried items or
    /// inclusions (§2.13).
    Ineligible,
    /// A retained record in a prompt of a unit without retained data.
    RetainedDataNotPermitted,
    /// The generation moved again after the one rebuild (§12.7).
    GenerationUnstable,
    /// No envelope or transport could be built for the unit.
    NoTransport,
    /// The admission row or its event could not be written (§15.3).
    AdmissionNotRecorded,
}

impl Refused {
    /// Class name for `memory.dispatch_refused`.
    pub fn reason(self) -> &'static str {
        match self {
            Refused::WrongIssuer => "wrong_issuer",
            Refused::NotAnAgent => "not_an_agent",
            Refused::EnvelopeMismatch => "envelope_mismatch",
            Refused::Unresolved(u) => u.reason(),
            Refused::NotMigrated => "store_not_migrated",
            Refused::PolicyUnavailable => "policy_unavailable",
            Refused::RouteMismatch => "route_mismatch",
            Refused::Ineligible => "ineligible",
            Refused::RetainedDataNotPermitted => "retained_data_not_permitted",
            Refused::GenerationUnstable => "generation_unstable",
            Refused::NoTransport => "no_transport",
            Refused::AdmissionNotRecorded => "admission_not_recorded",
        }
    }
}

impl fmt::Display for Refused {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "dispatch refused: {}", self.reason())
    }
}

impl std::error::Error for Refused {}

enum Attempt {
    GenerationMoved,
    Refused(Refused),
}

/// The envelope's inclusions and carried items re-evaluated in the
/// transaction: `(raw label, eligible for a non-local route)`.
fn evaluate_envelope(
    tx: &HmTx<'_>,
    ctx: &DispatchContext,
    env: &PromptEnvelope,
) -> StoreResult<(OriginLabel, bool)> {
    let unrestricted = OriginLabel::UnrestrictedOrigin;
    let mut raw = ctx.taint;
    let mut eligible = ctx.taint == unrestricted;
    for l in &env.carried {
        raw = raw.join(*l);
        eligible &= *l == unrestricted;
    }
    for (r, _) in &env.inclusions {
        let l = evaluate_ref(tx, r)?;
        raw = raw.join(l);
        eligible &= l == unrestricted;
    }
    Ok((raw, eligible))
}

/// What a reply's transport returned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TransportFailure;

/// A reply that is not used (§12.6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplyRefused {
    /// The admission belongs to another context or issuer.
    NotThisContext,
    /// The dispatch is no longer admissible; the disclosure stands.
    Discarded,
    /// The transport failed; recorded as possibly disclosed.
    TransportFailed,
}

impl fmt::Display for ReplyRefused {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "reply not used: {self:?}")
    }
}

impl std::error::Error for ReplyRefused {}

/// The dispatch builder: from the (possibly rebuilt) context, a fresh
/// envelope and the attestation of the transport selected for it, or
/// `None` when none can be built.
pub type BuildDispatch<'b> =
    dyn FnMut(&mut DispatchContext) -> Option<(PromptEnvelope, RouteAttestation)> + 'b;

impl DispatchIssuer {
    /// §12.4 with the §12.7 rebuild bound. `build` is called once, and once
    /// more after a rebuild when the generation moved. A refusal is audited
    /// (`memory.dispatch_refused`, observational).
    pub fn admit(
        &self,
        ctx: &mut DispatchContext,
        build: &mut BuildDispatch<'_>,
    ) -> Result<Admission, Refused> {
        let result = self.admit_bounded(ctx, build);
        if let Err(r) = &result {
            events::observational(
                self.db(),
                "memory.dispatch_refused",
                match ctx.principal {
                    Principal::Agent(a) => Some(a.to_string()),
                    _ => None,
                }
                .as_deref(),
                serde_json::json!({
                    "reason": r.reason(),
                    "route_class": ctx.route_class.class_name(),
                }),
            );
        }
        result
    }

    fn admit_bounded(
        &self,
        ctx: &mut DispatchContext,
        build: &mut BuildDispatch<'_>,
    ) -> Result<Admission, Refused> {
        if !self.issued(ctx) {
            return Err(Refused::WrongIssuer);
        }
        let Principal::Agent(agent_id) = ctx.principal else {
            return Err(Refused::NotAnAgent);
        };
        for rebuilt in [false, true] {
            let (env, attestation) = build(ctx).ok_or(Refused::NoTransport)?;
            match self.admit_once(ctx, agent_id, env, attestation) {
                Ok(admission) => return Ok(admission),
                Err(Attempt::GenerationMoved) if !rebuilt => self.rebuild(ctx)?,
                Err(Attempt::GenerationMoved) => return Err(Refused::GenerationUnstable),
                Err(Attempt::Refused(r)) => return Err(r),
            }
        }
        Err(Refused::GenerationUnstable)
    }

    /// One rebuild (§12.7): a fresh §4.2 resolve. The snapshot moves to the
    /// committed policy; a local-only policy is an observed tightening. The
    /// taint never falls and the route class never loosens.
    fn rebuild(&self, ctx: &mut DispatchContext) -> Result<(), Refused> {
        let p = self
            .resolve_agent_route_policy(ctx.policy.agent_id)
            .map_err(Refused::Unresolved)?;
        ctx.policy = p;
        if p.local_only {
            ctx.restrictions.local_only = true;
            let (taint, route) =
                unit_taint_step(ctx.taint, ctx.route_class, OriginLabel::LocalOnlyOrigin);
            ctx.taint = taint;
            ctx.route_class = route;
        }
        Ok(())
    }

    fn admit_once(
        &self,
        ctx: &DispatchContext,
        agent_id: Uuid,
        env: PromptEnvelope,
        attestation: RouteAttestation,
    ) -> Result<Admission, Attempt> {
        if !ctx.owns(&env) {
            return Err(Attempt::Refused(Refused::EnvelopeMismatch));
        }
        if !ctx.restrictions.retained_data_permitted && !env.inclusions.is_empty() {
            return Err(Attempt::Refused(Refused::RetainedDataNotPermitted));
        }
        // O2: the config is read and any tightening is committed in its own
        // transaction, before the admission transaction.
        self.resolve_agent_route_policy(agent_id)
            .map_err(|u| Attempt::Refused(Refused::Unresolved(u)))?;
        let admission_id =
            hex::encode(random_id().map_err(|u| Attempt::Refused(Refused::Unresolved(u)))?);
        let agent = agent_id.to_string();
        let non_local = attestation.class == RouteClass::NonLocal;

        let decided = self.db().hm().immediate(|tx| {
            if !is_migrated(tx)? {
                return Ok(Err(Attempt::Refused(Refused::NotMigrated)));
            }
            // Step 1: re-read the policy.
            let policy = match tx
                .policy_row_raw(&agent)?
                .map(|raw| decode_policy(agent_id, &raw))
            {
                Some(Ok(p)) => p,
                _ => return Ok(Err(Attempt::Refused(Refused::PolicyUnavailable))),
            };
            self.hit(ProbePoint::AdmissionPolicyRead);
            // Generation: roll back and rebuild once.
            if policy.policy_generation != ctx.policy.policy_generation {
                return Ok(Err(Attempt::GenerationMoved));
            }
            // Route class: a non-local transport needs a non-local unit of
            // an agent that is not local-only. A local one always fits.
            if non_local && (ctx.route_class != RouteClass::NonLocal || policy.local_only) {
                return Ok(Err(Attempt::Refused(Refused::RouteMismatch)));
            }
            // Eligibility, re-evaluated here; no grant dependencies (AD-4).
            let (raw_label, eligible) = evaluate_envelope(tx, ctx, &env)?;
            if non_local && !eligible {
                return Ok(Err(Attempt::Refused(Refused::Ineligible)));
            }
            // Step 5: the admission row and its event, in this transaction.
            tx.insert_admission(&AdmissionRow {
                admission_id: admission_id.clone(),
                agent_id: Some(agent.clone()),
                unit_kind: ctx.unit.kind_name().to_string(),
                route_class: attestation.class.class_name().to_string(),
                policy_generation: policy.policy_generation,
                revocation_seq: policy.revocation_seq,
                record_count: env.inclusions.len() as i64,
                grant_dep_count: 0,
                raw_label: raw_label.class_name().to_string(),
                outcome: "admitted".to_string(),
            })?;
            self.hit(ProbePoint::AdmissionRowWritten);
            tx.insert_event(
                &format!("admission:{admission_id}"),
                "memory.dispatch_admitted",
                Some(&agent),
                &serde_json::json!({
                    "route_class": attestation.class.class_name(),
                    "record_count": env.inclusions.len(),
                    "grant_dep_count": 0,
                    "raw_label": raw_label.class_name(),
                    "policy_generation": policy.policy_generation,
                    "revocation_seq": policy.revocation_seq,
                })
                .to_string(),
            )?;
            Ok(Ok(Admission {
                admission_id: admission_id.clone(),
                issuer_id: ctx.issuer_id,
                ctx_nonce: ctx.nonce,
                agent_id,
                route_class: attestation.class,
                policy_generation: policy.policy_generation,
                revocation_seq: policy.revocation_seq,
                raw_label,
                text: env.text.clone(),
            }))
            // COMMIT: the linearization point (§12.5).
        });
        match decided {
            Ok(outcome) => outcome,
            // A failed insert, event or commit: not admitted (U16).
            Err(_) => Err(Attempt::Refused(Refused::AdmissionNotRecorded)),
        }
    }

    /// §12.6: O3, then a new read transaction. A used reply is a derived
    /// item labelled with the admission's `raw_label` and enters the
    /// working set. The outcome is written after the decision; a failure to
    /// write it changes nothing.
    pub fn accept_reply(
        &self,
        ctx: &mut DispatchContext,
        admission: Admission,
        reply: Result<String, TransportFailure>,
    ) -> Result<DerivedItem, ReplyRefused> {
        if admission.issuer_id != ctx.issuer_id
            || admission.ctx_nonce != ctx.nonce
            || !self.issued(ctx)
        {
            return Err(ReplyRefused::NotThisContext);
        }
        let reply = match reply {
            Ok(text) => text,
            Err(TransportFailure) => {
                self.record_outcome(
                    &admission,
                    "transport_failed",
                    Some("memory.transport_failed"),
                );
                return Err(ReplyRefused::TransportFailed);
            }
        };

        // O3: a tightening found here is committed before the snapshot.
        let o3 = self.resolve_agent_route_policy(admission.agent_id);
        if let Ok(p) = o3 {
            if p.local_only {
                let (taint, route) =
                    unit_taint_step(ctx.taint, ctx.route_class, OriginLabel::LocalOnlyOrigin);
                ctx.taint = taint;
                ctx.route_class = route;
                ctx.restrictions.local_only = true;
            }
        }
        let agent = admission.agent_id.to_string();
        let still_admissible = o3.is_ok()
            && self
                .db()
                .hm()
                .read(|tx| {
                    Ok(
                        match tx
                            .policy_row_raw(&agent)?
                            .map(|raw| decode_policy(admission.agent_id, &raw))
                        {
                            Some(Ok(p)) => {
                                p.policy_generation == admission.policy_generation
                                    && (admission.route_class == RouteClass::Local || !p.local_only)
                            }
                            _ => false,
                        },
                    )
                })
                .unwrap_or(false);

        if !still_admissible {
            self.record_outcome(
                &admission,
                "reply_discarded",
                Some("memory.reply_discarded"),
            );
            return Err(ReplyRefused::Discarded);
        }
        self.record_outcome(&admission, "reply_used", None);
        let (taint, route) = unit_taint_step(ctx.taint, ctx.route_class, admission.raw_label);
        ctx.taint = taint;
        ctx.route_class = route;
        ctx.working_set.derived.push(admission.raw_label);
        Ok(DerivedItem {
            label: admission.raw_label,
            text: reply,
        })
    }

    /// Writes an admission's outcome, and its authority event when there
    /// is one, after the decision. Best effort.
    fn record_outcome(&self, admission: &Admission, outcome: &str, event_kind: Option<&str>) {
        let agent = admission.agent_id.to_string();
        let _ = self.db().hm().immediate(|tx| {
            tx.update_admission_outcome(&admission.admission_id, outcome)?;
            if let Some(kind) = event_kind {
                tx.insert_event(
                    &format!("admission:{}:outcome", admission.admission_id),
                    kind,
                    Some(&agent),
                    &serde_json::json!({
                        "outcome": outcome,
                        "route_class": admission.route_class.class_name(),
                    })
                    .to_string(),
                )?;
            }
            Ok(())
        });
    }
}
