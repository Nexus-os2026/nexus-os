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
    unit_taint_step, DerivedItem, DispatchClass, DispatchContext, Principal, PromptEnvelope,
    RouteClass,
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

/// The envelope's inclusions and carried items evaluated in the
/// transaction: `(raw label, eligible for a non-local route)`. The label of
/// an inclusion is the label evaluated at recall ⊔ the label evaluated now
/// (mission R1 (i)); the raw label is the envelope's own raw label ⊔ this
/// evaluation, so it is never lower than the envelope says.
fn evaluate_envelope(
    tx: &HmTx<'_>,
    ctx: &DispatchContext,
    env: &PromptEnvelope,
) -> StoreResult<(OriginLabel, bool)> {
    let unrestricted = OriginLabel::UnrestrictedOrigin;
    let mut raw = env.raw_label.join(ctx.taint);
    let mut eligible = ctx.taint == unrestricted;
    for l in &env.carried {
        raw = raw.join(*l);
        eligible &= *l == unrestricted;
    }
    for (r, at_recall) in &env.inclusions {
        let l = at_recall.effective().join(evaluate_ref(tx, r)?);
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

/// Why a dispatch produced no used reply.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DispatchError {
    /// Refused before transmission: nothing was sent (§12.4, §16).
    Refused(Refused),
    /// Sent, but the reply is not used (§12.6); the disclosure stands.
    Reply(ReplyRefused),
    /// The transport failed after admission, with its message; recorded as
    /// possibly disclosed.
    Transport(String),
}

impl fmt::Display for DispatchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DispatchError::Refused(r) => write!(f, "{r}"),
            DispatchError::Reply(r) => write!(f, "{r}"),
            DispatchError::Transport(e) => write!(f, "transport failed: {e}"),
        }
    }
}

impl std::error::Error for DispatchError {}

/// What a non-agent dispatch decided before transmission.
struct NonAgentAdmitted<T> {
    raw_label: OriginLabel,
    route_class: RouteClass,
    admission_id: Option<String>,
    text: String,
    transport: T,
}

/// The dispatch builder: from the (possibly rebuilt) context, a fresh
/// envelope and the attestation of the transport selected for it, or
/// `None` when none can be built.
pub type BuildDispatch<'b> =
    dyn FnMut(&mut DispatchContext) -> Option<(PromptEnvelope, RouteAttestation)> + 'b;

impl DispatchIssuer {
    /// §12.4 with the §12.7 rebuild bound. `build` is called once, and once
    /// more after a rebuild when the generation moved. A refusal is audited
    /// (`memory.dispatch_refused`, observational).
    ///
    /// Called only by [`DispatchIssuer::dispatch`] and the hm tests (guard
    /// A-S3).
    #[doc(hidden)]
    pub fn admit(
        &self,
        ctx: &mut DispatchContext,
        build: &mut BuildDispatch<'_>,
    ) -> Result<Admission, Refused> {
        // The unit's route class when the refusal was decided, before the
        // envelope was absorbed.
        let mut decision_route = ctx.route_class;
        let result = self.admit_bounded(ctx, build, &mut decision_route);
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
                    "route_class": decision_route.class_name(),
                }),
            );
        }
        result
    }

    /// The single dispatch entry (mission R2): admission, transmission and
    /// reply acceptance of one model request, by the context's class
    /// (§12.1):
    /// - an agent's context (AR, AC): §12.4 and §12.6 in full;
    /// - an Owner's retained entry (HR) or an Owner's request shaped by
    ///   backend state with no registered agent (HB): §12.4 steps 2, 4 and 5
    ///   with no agent: an admission row whose `agent_id` is NULL;
    /// - a plain non-agent request (HD, SY, XS): no admission row; any
    ///   inclusion is refused; on a non-local route the taint and every
    ///   carried item must be `UNRESTRICTED_ORIGIN`.
    ///
    /// `build` returns, for the (possibly rebuilt) context, the envelope,
    /// the handle of the transport chosen for it and that transport's
    /// attestation; it is called once, and once more after a rebuild.
    /// `send` receives the handle and the admitted prompt bytes, with no
    /// transaction or lock held. The reply leaves only as a `DerivedItem`.
    #[allow(clippy::type_complexity)]
    pub fn dispatch<T>(
        &self,
        ctx: &mut DispatchContext,
        build: &mut dyn FnMut(
            &mut DispatchContext,
        ) -> Option<(PromptEnvelope, T, RouteAttestation)>,
        send: &mut dyn FnMut(T, &str) -> Result<String, String>,
    ) -> Result<DerivedItem, DispatchError> {
        if !self.issued(ctx) {
            return Err(DispatchError::Refused(Refused::WrongIssuer));
        }
        if ctx.class != DispatchClass::Agent {
            return self.dispatch_non_agent(ctx, build, send);
        }
        let mut handle: Option<T> = None;
        let admission = {
            let mut admit_build = |c: &mut DispatchContext| {
                let (env, transport, attestation) = build(c)?;
                handle = Some(transport);
                Some((env, attestation))
            };
            self.admit(ctx, &mut admit_build)
                .map_err(DispatchError::Refused)?
        };
        let Some(transport) = handle else {
            return Err(DispatchError::Refused(Refused::NoTransport));
        };
        if admission.route_class == RouteClass::Local {
            ctx.local_transport_used = true;
        }
        // Transmission: the admission transaction has committed; no lock or
        // transaction of this module is held.
        let reply = send(transport, &admission.text);
        match reply {
            Ok(text) => self
                .accept_reply(ctx, admission, Ok(text))
                .map_err(DispatchError::Reply),
            Err(e) => {
                let _ = self.accept_reply(ctx, admission, Err(TransportFailure));
                Err(DispatchError::Transport(e))
            }
        }
    }

    /// HR, HB and plain dispatch (mission R2; P3-C11-HM-M4 C-3).
    #[allow(clippy::type_complexity)]
    fn dispatch_non_agent<T>(
        &self,
        ctx: &mut DispatchContext,
        build: &mut dyn FnMut(
            &mut DispatchContext,
        ) -> Option<(PromptEnvelope, T, RouteAttestation)>,
        send: &mut dyn FnMut(T, &str) -> Result<String, String>,
    ) -> Result<DerivedItem, DispatchError> {
        let admitted = match self.admit_non_agent(ctx, build) {
            Ok(a) => a,
            Err(r) => {
                events::observational(
                    self.db(),
                    "memory.dispatch_refused",
                    None,
                    serde_json::json!({
                        "reason": r.reason(),
                        "route_class": ctx.route_class.class_name(),
                    }),
                );
                return Err(DispatchError::Refused(r));
            }
        };
        if admitted.route_class == RouteClass::Local {
            ctx.local_transport_used = true;
        }
        let reply = send(admitted.transport, &admitted.text);
        let outcome = |s: &Self, outcome: &str, kind: Option<&str>| {
            if let Some(id) = &admitted.admission_id {
                s.record_owner_outcome(id, admitted.route_class, outcome, kind);
            }
        };
        match reply {
            Ok(text) => {
                outcome(self, "reply_used", None);
                let (taint, route) =
                    unit_taint_step(ctx.taint, ctx.route_class, admitted.raw_label);
                ctx.taint = taint;
                ctx.route_class = route;
                if !ctx.working_set.derived.contains(&admitted.raw_label) {
                    ctx.working_set.derived.push(admitted.raw_label);
                }
                Ok(DerivedItem {
                    label: admitted.raw_label,
                    text,
                })
            }
            Err(e) => {
                outcome(self, "transport_failed", Some("memory.transport_failed"));
                Err(DispatchError::Transport(e))
            }
        }
    }

    #[allow(clippy::type_complexity)]
    fn admit_non_agent<T>(
        &self,
        ctx: &mut DispatchContext,
        build: &mut dyn FnMut(
            &mut DispatchContext,
        ) -> Option<(PromptEnvelope, T, RouteAttestation)>,
    ) -> Result<NonAgentAdmitted<T>, Refused> {
        let (env, transport, attestation) = build(ctx).ok_or(Refused::NoTransport)?;
        if !ctx.owns(&env) {
            return Err(Refused::EnvelopeMismatch);
        }
        let (inclusions, carried) = (env.inclusions.clone(), env.carried.clone());
        let decided = self.decide_non_agent(ctx, &env, attestation);
        ctx.absorb(&inclusions, &carried);
        let (raw_label, admission_id) = decided?;
        Ok(NonAgentAdmitted {
            raw_label,
            route_class: attestation.class,
            admission_id,
            text: env.text,
            transport,
        })
    }

    /// The non-agent decision: no inclusion; a non-local transport needs a
    /// non-local unit with unrestricted taint and carried items (§2.13 rules
    /// 1, 2). An HR or HB admission is then recorded with no agent (§12.4
    /// step 5).
    fn decide_non_agent(
        &self,
        ctx: &DispatchContext,
        env: &PromptEnvelope,
        attestation: RouteAttestation,
    ) -> Result<(OriginLabel, Option<String>), Refused> {
        if !env.inclusions.is_empty() {
            return Err(Refused::RetainedDataNotPermitted);
        }
        let unrestricted = OriginLabel::UnrestrictedOrigin;
        let non_local = attestation.class == RouteClass::NonLocal;
        // Step 2: the attested class fits the unit.
        if non_local && ctx.route_class != RouteClass::NonLocal {
            return Err(Refused::RouteMismatch);
        }
        // Step 4: eligibility of a non-local dispatch.
        let raw_label = env
            .carried
            .iter()
            .fold(env.raw_label.join(ctx.taint), |l, c| l.join(*c));
        let eligible = ctx.taint == unrestricted && env.carried.iter().all(|c| *c == unrestricted);
        if non_local && !eligible {
            return Err(Refused::Ineligible);
        }
        if !matches!(
            ctx.class,
            DispatchClass::OwnerRetained | DispatchClass::OwnerBackendShaped
        ) {
            return Ok((raw_label, None));
        }
        // Step 5: the HR or HB admission row and its event, agent NULL.
        let admission_id = hex::encode(random_id().map_err(Refused::Unresolved)?);
        let decided = self.db().hm().immediate(|tx| {
            if !is_migrated(tx)? {
                return Ok(Err(Refused::NotMigrated));
            }
            tx.insert_admission(&AdmissionRow {
                admission_id: admission_id.clone(),
                agent_id: None,
                unit_kind: ctx.unit.kind_name().to_string(),
                route_class: attestation.class.class_name().to_string(),
                policy_generation: 0,
                revocation_seq: 0,
                record_count: 0,
                grant_dep_count: 0,
                raw_label: raw_label.class_name().to_string(),
                outcome: "admitted".to_string(),
            })?;
            self.hit(ProbePoint::AdmissionRowWritten);
            tx.insert_event(
                &format!("admission:{admission_id}"),
                "memory.dispatch_admitted",
                None,
                &serde_json::json!({
                    "route_class": attestation.class.class_name(),
                    "record_count": 0,
                    "grant_dep_count": 0,
                    "raw_label": raw_label.class_name(),
                    "policy_generation": 0,
                    "revocation_seq": 0,
                })
                .to_string(),
            )?;
            Ok(Ok(()))
        });
        match decided {
            Ok(Ok(())) => Ok((raw_label, Some(admission_id))),
            Ok(Err(r)) => Err(r),
            Err(_) => Err(Refused::AdmissionNotRecorded),
        }
    }

    /// The outcome of an HR or HB admission, after the decision. Best
    /// effort.
    fn record_owner_outcome(
        &self,
        admission_id: &str,
        route_class: RouteClass,
        outcome: &str,
        event_kind: Option<&str>,
    ) {
        let _ = self.db().hm().immediate(|tx| {
            tx.update_admission_outcome(admission_id, outcome)?;
            if let Some(kind) = event_kind {
                tx.insert_event(
                    &format!("admission:{admission_id}:outcome"),
                    kind,
                    None,
                    &serde_json::json!({
                        "outcome": outcome,
                        "route_class": route_class.class_name(),
                    })
                    .to_string(),
                )?;
            }
            Ok(())
        });
    }

    fn admit_bounded(
        &self,
        ctx: &mut DispatchContext,
        build: &mut BuildDispatch<'_>,
        decision_route: &mut RouteClass,
    ) -> Result<Admission, Refused> {
        if !self.issued(ctx) {
            return Err(Refused::WrongIssuer);
        }
        let Principal::Agent(agent_id) = ctx.principal else {
            return Err(Refused::NotAnAgent);
        };
        for rebuilt in [false, true] {
            let built = build(ctx);
            *decision_route = ctx.route_class;
            let (env, attestation) = built.ok_or(Refused::NoTransport)?;
            // Mission R1 (ii): an envelope this context owns enters its
            // working set whatever the outcome, after the decision.
            let owned = ctx
                .owns(&env)
                .then(|| (env.inclusions.clone(), env.carried.clone()));
            let attempt = self.admit_once(ctx, agent_id, env, attestation);
            if let Some((inclusions, carried)) = owned {
                ctx.absorb(&inclusions, &carried);
            }
            match attempt {
                Ok(admission) => return Ok(admission),
                Err(Attempt::GenerationMoved) if !rebuilt => {
                    *decision_route = ctx.route_class;
                    self.rebuild(ctx)?
                }
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
    ///
    /// Called only by [`DispatchIssuer::dispatch`] and the hm tests (guard
    /// A-S3).
    #[doc(hidden)]
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
