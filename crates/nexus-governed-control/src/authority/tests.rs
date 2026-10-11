//! Authority core tests: the commitment state machine, approvals, grants,
//! the policy generation, cancellation and evidence, attacked directly.

use super::approval::{
    ActionConfirmation, ControlConfirmer, GrantConfirmation, ResumeConfirmation,
};
use super::clock::ManualClock;
use super::commitment::{CommitmentState, FailureClass, Outcome, PreparedAction, TargetIdentity};
use super::effect::{CapabilityKind, EffectClass};
use super::evidence::{EvidencePhase, MemoryEvidence};
use super::ids::{AgentId, CommitmentId, Digest, GrantId, RunId};
use super::policy::GrantScope;
use super::run::RunOrigin;
use super::{Authority, AuthorityError};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// A native confirmer stand-in: answers as told and records what it saw.
struct Confirmer {
    approve: AtomicBool,
    actions: AtomicU32,
    grants: AtomicU32,
    last: Mutex<Option<ActionConfirmation>>,
}

impl Confirmer {
    fn new(approve: bool) -> Self {
        Self {
            approve: AtomicBool::new(approve),
            actions: AtomicU32::new(0),
            grants: AtomicU32::new(0),
            last: Mutex::new(None),
        }
    }
}

impl ControlConfirmer for Confirmer {
    fn confirm_action(&self, request: &ActionConfirmation) -> bool {
        self.actions.fetch_add(1, Ordering::SeqCst);
        *self.last.lock().unwrap() = Some(request.clone());
        self.approve.load(Ordering::SeqCst)
    }

    fn confirm_grant(&self, _request: &GrantConfirmation) -> bool {
        self.grants.fetch_add(1, Ordering::SeqCst);
        self.approve.load(Ordering::SeqCst)
    }

    fn confirm_resume(&self, _request: &ResumeConfirmation) -> bool {
        self.approve.load(Ordering::SeqCst)
    }
}

struct Fixture {
    auth: Arc<Authority>,
    evidence: Arc<MemoryEvidence>,
    clock: Arc<ManualClock>,
    yes: Confirmer,
    agent: AgentId,
    run: RunId,
    grant: GrantId,
}

fn fixture() -> Fixture {
    let evidence = Arc::new(MemoryEvidence::new(10_000));
    let clock = Arc::new(ManualClock::default());
    let auth = Arc::new(Authority::new(evidence.clone(), clock.clone()));
    let yes = Confirmer::new(true);
    let agent = AgentId::new("agent-a").unwrap();
    let run = auth
        .open_run(
            agent.clone(),
            RunOrigin::Command {
                modalities: vec!["text".into()],
            },
        )
        .unwrap();
    let grant = auth
        .grants()
        .request(egress_scope(), Duration::from_secs(600), &yes)
        .unwrap();
    Fixture {
        auth,
        evidence,
        clock,
        yes,
        agent,
        run,
        grant,
    }
}

fn egress_scope() -> GrantScope {
    GrantScope::Egress {
        scheme: "https".into(),
        host: "example.invalid".into(),
        port: 443,
        methods: vec!["GET".into()],
        allow_private: false,
    }
}

fn target(text: &str) -> TargetIdentity {
    TargetIdentity {
        display: text.to_string(),
        digest: Digest::of("test.target", &[text.as_bytes()]),
    }
}

fn params(text: &str) -> Digest {
    Digest::of("test.params", &[text.as_bytes()])
}

fn prepared(class: EffectClass, grant: GrantId) -> PreparedAction {
    PreparedAction {
        kind: CapabilityKind::Egress,
        class,
        operation: "egress.fetch",
        target: target("https://example.invalid/"),
        parameters: params("GET"),
        grants: vec![grant],
        leases: vec![],
        summary: vec!["GET https://example.invalid/".into()],
    }
}

const TTL: Duration = Duration::from_secs(60);

impl Fixture {
    fn prepare(&self, class: EffectClass) -> CommitmentId {
        self.auth
            .commitments()
            .prepare(&self.agent, self.run, prepared(class, self.grant), TTL)
            .unwrap()
            .id
    }

    fn begin(&self, id: CommitmentId) -> Result<super::commitment::ExecutionGuard, AuthorityError> {
        self.auth.commitments().begin(
            id,
            &self.agent,
            self.run,
            &target("https://example.invalid/").digest,
            &params("GET"),
        )
    }

    fn state(&self, id: CommitmentId) -> CommitmentState {
        self.auth.commitments().view(id).unwrap().state
    }

    fn phases(&self) -> Vec<EvidencePhase> {
        self.evidence.records().iter().map(|r| r.phase).collect()
    }
}

#[test]
fn an_r1_commitment_runs_once_and_is_finalized_with_evidence() {
    let f = fixture();
    let id = f.prepare(EffectClass::R1);
    assert_eq!(f.state(id), CommitmentState::Prepared);
    f.auth
        .commitments()
        .authorize(id, &f.agent, f.run, None)
        .unwrap();
    let guard = f.begin(id).unwrap();
    assert_eq!(f.state(id), CommitmentState::Executing);
    guard.finish(Outcome::Succeeded {
        meta: vec![("status".into(), "200".into())],
    });
    assert_eq!(f.state(id), CommitmentState::Succeeded);
    let phases = f.phases();
    for phase in [
        EvidencePhase::RunOpened,
        EvidencePhase::GrantIssued,
        EvidencePhase::Prepared,
        EvidencePhase::Authorized,
        EvidencePhase::Started,
        EvidencePhase::Finished,
    ] {
        assert!(phases.contains(&phase), "{phase:?} in {phases:?}");
    }
    let finished = f.evidence.records().pop().unwrap();
    assert_eq!(finished.outcome, Some("succeeded"));
    assert_eq!(finished.commitment, Some(id.to_string()));
    assert!(finished.started_wall_ms.is_some() && finished.finished_wall_ms.is_some());
}

#[test]
fn a_forged_or_unknown_identity_names_nothing() {
    let f = fixture();
    let real = f.prepare(EffectClass::R1);
    // A well-formed but forged identity, and a garbage one.
    let forged = CommitmentId::parse("cmt-00000000000000000000000000000000").unwrap();
    assert!(CommitmentId::parse("cmt-zz").is_none());
    assert!(CommitmentId::parse("run-00000000000000000000000000000000").is_none());
    let c = f.auth.commitments();
    assert_eq!(
        c.authorize(forged, &f.agent, f.run, None),
        Err(AuthorityError::UnknownCommitment)
    );
    assert_eq!(
        f.begin(forged).unwrap_err(),
        AuthorityError::UnknownCommitment
    );
    let fresh_unknown = CommitmentId::fresh();
    assert_eq!(
        c.authorize(fresh_unknown, &f.agent, f.run, None),
        Err(AuthorityError::UnknownCommitment)
    );
    // The real one is untouched.
    assert_eq!(f.state(real), CommitmentState::Prepared);
}

#[test]
fn another_agent_or_another_run_cannot_use_a_commitment() {
    let f = fixture();
    let id = f.prepare(EffectClass::R1);
    let c = f.auth.commitments();
    let other_agent = AgentId::new("agent-b").unwrap();
    assert_eq!(
        c.authorize(id, &other_agent, f.run, None),
        Err(AuthorityError::WrongAgent)
    );
    let other_run = f
        .auth
        .open_run(f.agent.clone(), RunOrigin::AgentGoal)
        .unwrap();
    assert_eq!(
        c.authorize(id, &f.agent, other_run, None),
        Err(AuthorityError::WrongRun)
    );
    c.authorize(id, &f.agent, f.run, None).unwrap();
    assert_eq!(
        c.begin(
            id,
            &other_agent,
            f.run,
            &target("https://example.invalid/").digest,
            &params("GET")
        )
        .unwrap_err(),
        AuthorityError::WrongAgent
    );
    assert_eq!(
        c.begin(
            id,
            &f.agent,
            other_run,
            &target("https://example.invalid/").digest,
            &params("GET")
        )
        .unwrap_err(),
        AuthorityError::WrongRun
    );
    // A run of another agent cannot be used to prepare for this agent.
    let b_run = f
        .auth
        .open_run(other_agent.clone(), RunOrigin::AgentGoal)
        .unwrap();
    assert_eq!(
        c.prepare(&f.agent, b_run, prepared(EffectClass::R1, f.grant), TTL)
            .unwrap_err(),
        AuthorityError::WrongAgent
    );
}

#[test]
fn an_expired_commitment_cannot_be_authorized_or_started() {
    let f = fixture();
    let id = f.prepare(EffectClass::R1);
    f.clock.advance(TTL);
    assert_eq!(
        f.auth.commitments().authorize(id, &f.agent, f.run, None),
        Err(AuthorityError::Expired)
    );
    assert_eq!(f.state(id), CommitmentState::Expired);
    // Expiry after authorization also stops the start.
    let id = f.prepare(EffectClass::R1);
    f.auth
        .commitments()
        .authorize(id, &f.agent, f.run, None)
        .unwrap();
    f.clock.advance(TTL + Duration::from_millis(1));
    assert_eq!(f.begin(id).unwrap_err(), AuthorityError::Expired);
    assert!(f.phases().contains(&EvidencePhase::Expired));
}

/// T60b: suspension expires the commitment independently of its grant.
#[test]
fn wall_clock_expiry_ends_a_commitment_while_its_grant_stays_live() {
    let f = fixture();
    let id = f.prepare(EffectClass::R1); // 60 s commitment, 600 s grant.
    f.auth
        .commitments()
        .authorize(id, &f.agent, f.run, None)
        .unwrap();
    f.clock.suspend(TTL + Duration::from_secs(1));
    assert!(
        f.auth.grants().live(f.grant).is_some(),
        "the grant must still be live"
    );
    assert_eq!(
        f.begin(id).unwrap_err(),
        AuthorityError::Expired,
        "wall time alone must expire the commitment"
    );
    assert_eq!(f.state(id), CommitmentState::Expired);
    assert!(!f.phases().contains(&EvidencePhase::Started));
}

#[test]
fn a_revoked_grant_or_a_moved_policy_generation_ends_unconsumed_commitments() {
    let f = fixture();
    let prepared_only = f.prepare(EffectClass::R1);
    let authorized = f.prepare(EffectClass::R1);
    f.auth
        .commitments()
        .authorize(authorized, &f.agent, f.run, None)
        .unwrap();
    let before = f.auth.policy_generation();
    f.auth.grants().revoke(f.grant).unwrap();
    assert!(
        f.auth.policy_generation() > before,
        "revocation moves the generation"
    );
    assert_eq!(
        f.auth
            .commitments()
            .authorize(prepared_only, &f.agent, f.run, None),
        Err(AuthorityError::Stale)
    );
    assert_eq!(f.begin(authorized).unwrap_err(), AuthorityError::Stale);
    assert_eq!(f.state(prepared_only), CommitmentState::Revoked);
    assert_eq!(f.state(authorized), CommitmentState::Revoked);
    // A revoked grant cannot back a new commitment either.
    assert_eq!(
        f.auth
            .commitments()
            .prepare(&f.agent, f.run, prepared(EffectClass::R1, f.grant), TTL)
            .unwrap_err(),
        AuthorityError::GrantNotLive
    );
}

#[test]
fn an_expired_grant_is_not_live() {
    let f = fixture();
    let id = f.prepare(EffectClass::R1);
    f.clock.advance(Duration::from_secs(601));
    assert!(f.auth.grants().live(f.grant).is_none());
    // The commitment's own deadline passed first here; either way it ends.
    assert!(f
        .auth
        .commitments()
        .authorize(id, &f.agent, f.run, None)
        .is_err());
}

#[test]
fn a_commitment_is_consumed_exactly_once() {
    let f = fixture();
    let id = f.prepare(EffectClass::R1);
    let c = f.auth.commitments();
    c.authorize(id, &f.agent, f.run, None).unwrap();
    assert_eq!(
        c.authorize(id, &f.agent, f.run, None),
        Err(AuthorityError::NotPending)
    );
    let guard = f.begin(id).unwrap();
    assert_eq!(f.begin(id).unwrap_err(), AuthorityError::NotAuthorized);
    guard.finish(Outcome::Succeeded { meta: vec![] });
    assert_eq!(f.begin(id).unwrap_err(), AuthorityError::NotAuthorized);
    assert_eq!(
        c.authorize(id, &f.agent, f.run, None),
        Err(AuthorityError::NotPending)
    );
}

/// An executing commitment stays covered only while its grants are live
/// and the policy has not moved: effects that run in steps ask before each
/// one, and what outlives one call asks `liveness`, which also ends with
/// the run's cancellation.
#[test]
fn an_executing_commitment_stays_covered_only_while_its_grants_are_live() {
    let executing = |f: &Fixture| {
        let id = f.prepare(EffectClass::R1);
        f.auth
            .commitments()
            .authorize(id, &f.agent, f.run, None)
            .unwrap();
        let guard = f.begin(id).unwrap();
        assert!(guard.still_authorized() && (guard.liveness())());
        guard
    };
    // The grant expires.
    let f = fixture();
    let guard = executing(&f);
    f.clock.advance(Duration::from_secs(601));
    assert!(!guard.still_authorized() && !(guard.liveness())());
    assert_eq!(guard.lapse(), Some("its grant was revoked or expired"));
    // The grant is revoked.
    let f = fixture();
    let guard = executing(&f);
    let live = guard.liveness();
    f.auth.grants().revoke(f.grant).unwrap();
    assert!(!guard.still_authorized() && !live());
    // Another grant is revoked: the policy moved.
    let f = fixture();
    let other = f
        .auth
        .grants()
        .request(egress_scope(), Duration::from_secs(600), &f.yes)
        .unwrap();
    let guard = executing(&f);
    f.auth.grants().revoke(other).unwrap();
    assert!(!guard.still_authorized());
    // It says what happened: not its own grant.
    assert_eq!(
        guard.lapse(),
        Some("the policy changed (a grant was revoked or every run was stopped)")
    );
    // The run is cancelled: still covered, but no longer live.
    let f = fixture();
    let guard = executing(&f);
    let live = guard.liveness();
    f.auth.cancel_run(f.run).unwrap();
    assert!(guard.still_authorized() && !live());
    // A finished commitment is no longer executing.
    let f = fixture();
    let guard = executing(&f);
    let live = guard.liveness();
    let id = guard.commitment();
    guard.finish(Outcome::Succeeded { meta: vec![] });
    assert!(!f.auth.commitments().still_authorized(id) && !live());
    assert_eq!(
        f.auth.commitments().lapse(id),
        Some("the action is no longer executing")
    );
}

#[test]
fn a_changed_target_or_parameters_fail_the_start() {
    let f = fixture();
    let c = f.auth.commitments();
    let id = f.prepare(EffectClass::R1);
    c.authorize(id, &f.agent, f.run, None).unwrap();
    assert_eq!(
        c.begin(
            id,
            &f.agent,
            f.run,
            &target("https://elsewhere.invalid/").digest,
            &params("GET")
        )
        .unwrap_err(),
        AuthorityError::TargetChanged
    );
    assert_eq!(f.state(id), CommitmentState::Failed);
    assert_eq!(f.begin(id).unwrap_err(), AuthorityError::NotAuthorized);
    let id = f.prepare(EffectClass::R1);
    c.authorize(id, &f.agent, f.run, None).unwrap();
    assert_eq!(
        c.begin(
            id,
            &f.agent,
            f.run,
            &target("https://example.invalid/").digest,
            &params("POST")
        )
        .unwrap_err(),
        AuthorityError::ParametersChanged
    );
    assert_eq!(f.state(id), CommitmentState::Failed);
}

#[test]
fn r2_needs_the_native_approval_of_exactly_that_commitment() {
    let f = fixture();
    let c = f.auth.commitments();
    let a = f.prepare(EffectClass::R2);
    let b = f.prepare(EffectClass::R2);
    // No approval.
    assert_eq!(
        c.authorize(a, &f.agent, f.run, None),
        Err(AuthorityError::ApprovalRequired)
    );
    // An approval of B presented for A.
    let approval_b = c.request_approval(b, &f.agent, f.run, &f.yes).unwrap();
    assert_eq!(
        c.authorize(a, &f.agent, f.run, Some(approval_b)),
        Err(AuthorityError::ApprovalMismatch)
    );
    // The confirmation the owner saw named A, its class and its target.
    let approval_a = c.request_approval(a, &f.agent, f.run, &f.yes).unwrap();
    let seen = f.yes.last.lock().unwrap().clone().unwrap();
    assert_eq!(seen.commitment, a.to_string());
    assert_eq!(seen.class, EffectClass::R2);
    assert_eq!(seen.target, "https://example.invalid/");
    assert!(seen
        .message()
        .contains("R2 (a sensitive or irreversible effect)"));
    c.authorize(a, &f.agent, f.run, Some(approval_a)).unwrap();
    // An R1 commitment refuses an approval it does not need.
    let r1 = f.prepare(EffectClass::R1);
    let approval_c = {
        let c2 = f.prepare(EffectClass::R2);
        c.request_approval(c2, &f.agent, f.run, &f.yes).unwrap()
    };
    assert_eq!(
        c.authorize(r1, &f.agent, f.run, Some(approval_c)),
        Err(AuthorityError::ApprovalNotApplicable)
    );
    assert_eq!(
        c.request_approval(r1, &f.agent, f.run, &f.yes).unwrap_err(),
        AuthorityError::ApprovalNotApplicable
    );
}

#[test]
fn an_approval_cannot_be_replayed_or_survive_a_change() {
    let f = fixture();
    let c = f.auth.commitments();
    let a = f.prepare(EffectClass::R2);
    let approval = c.request_approval(a, &f.agent, f.run, &f.yes).unwrap();
    c.authorize(a, &f.agent, f.run, Some(approval)).unwrap();
    // A second approval request for the same commitment is refused: it is
    // no longer pending, so no second approval exists to replay.
    assert_eq!(
        c.request_approval(a, &f.agent, f.run, &f.yes).unwrap_err(),
        AuthorityError::NotPending
    );
    // An approval made under an old generation is useless after a change.
    let b = f.prepare(EffectClass::R2);
    let approval_b = c.request_approval(b, &f.agent, f.run, &f.yes).unwrap();
    let other = f
        .auth
        .grants()
        .request(egress_scope(), TTL, &f.yes)
        .unwrap();
    f.auth.grants().revoke(other).unwrap();
    assert_eq!(
        c.authorize(b, &f.agent, f.run, Some(approval_b)),
        Err(AuthorityError::Stale)
    );
}

#[test]
fn a_declined_approval_denies_the_commitment() {
    let f = fixture();
    let no = Confirmer::new(false);
    let a = f.prepare(EffectClass::R2);
    assert_eq!(
        f.auth
            .commitments()
            .request_approval(a, &f.agent, f.run, &no)
            .unwrap_err(),
        AuthorityError::Declined
    );
    assert_eq!(f.state(a), CommitmentState::Denied);
    assert!(f.phases().contains(&EvidencePhase::ApprovalDeclined));
    // A declined grant creates nothing.
    assert_eq!(
        f.auth
            .grants()
            .request(egress_scope(), TTL, &no)
            .unwrap_err(),
        AuthorityError::Declined
    );
    assert!(f.phases().contains(&EvidencePhase::GrantDeclined));
}

#[test]
fn an_approval_requested_then_invalidated_during_the_dialog_is_refused() {
    // The dialog is outside every lock; if the run is cancelled while it is
    // shown, the answer approves nothing.
    struct CancelWhileAsking<'a> {
        auth: &'a Authority,
        run: RunId,
    }
    impl ControlConfirmer for CancelWhileAsking<'_> {
        fn confirm_action(&self, _request: &ActionConfirmation) -> bool {
            self.auth.cancel_run(self.run).unwrap();
            true
        }
        fn confirm_grant(&self, _request: &GrantConfirmation) -> bool {
            true
        }
        fn confirm_resume(&self, _request: &ResumeConfirmation) -> bool {
            true
        }
    }
    let f = fixture();
    let a = f.prepare(EffectClass::R2);
    let confirmer = CancelWhileAsking {
        auth: &f.auth,
        run: f.run,
    };
    assert!(f
        .auth
        .commitments()
        .request_approval(a, &f.agent, f.run, &confirmer)
        .is_err());
    assert_ne!(f.state(a), CommitmentState::Authorized);
}

#[test]
fn a_parsed_display_identity_grants_nothing_more_than_the_original() {
    // The serialized form of a commitment is its display identity. Parsing it
    // back gives the same name, and the name alone authorizes nothing: the
    // registry state and the bound agent and run still decide.
    let f = fixture();
    let id = f.prepare(EffectClass::R2);
    let wire = serde_json::to_string(&id.to_string()).unwrap();
    let parsed = CommitmentId::parse(&serde_json::from_str::<String>(&wire).unwrap()).unwrap();
    assert_eq!(parsed, id);
    let c = f.auth.commitments();
    assert_eq!(
        c.authorize(parsed, &f.agent, f.run, None),
        Err(AuthorityError::ApprovalRequired)
    );
    assert_eq!(
        c.authorize(parsed, &AgentId::new("intruder").unwrap(), f.run, None),
        Err(AuthorityError::WrongAgent)
    );
    assert_eq!(f.begin(parsed).unwrap_err(), AuthorityError::NotAuthorized);
}

#[test]
fn concurrent_starts_consume_a_commitment_once() {
    let f = fixture();
    let id = f.prepare(EffectClass::R1);
    f.auth
        .commitments()
        .authorize(id, &f.agent, f.run, None)
        .unwrap();
    let wins = Arc::new(AtomicU32::new(0));
    let guards = Arc::new(Mutex::new(Vec::new()));
    std::thread::scope(|scope| {
        for _ in 0..16 {
            let (c, agent, run, wins, guards) = (
                f.auth.commitments().clone(),
                f.agent.clone(),
                f.run,
                wins.clone(),
                guards.clone(),
            );
            scope.spawn(move || {
                if let Ok(guard) = c.begin(
                    id,
                    &agent,
                    run,
                    &target("https://example.invalid/").digest,
                    &params("GET"),
                ) {
                    wins.fetch_add(1, Ordering::SeqCst);
                    guards.lock().unwrap().push(guard);
                }
            });
        }
    });
    assert_eq!(wins.load(Ordering::SeqCst), 1, "exactly one start");
    let started = f
        .evidence
        .records()
        .iter()
        .filter(|r| r.phase == EvidencePhase::Started)
        .count();
    assert_eq!(started, 1);
    for guard in guards.lock().unwrap().drain(..) {
        guard.finish(Outcome::Succeeded { meta: vec![] });
    }
}

#[test]
fn a_failure_or_an_abandoned_guard_still_finalizes() {
    let f = fixture();
    let c = f.auth.commitments();
    let failed = f.prepare(EffectClass::R1);
    c.authorize(failed, &f.agent, f.run, None).unwrap();
    f.begin(failed).unwrap().finish(Outcome::Failed {
        class: FailureClass::Transport,
        detail: "connection refused".into(),
    });
    assert_eq!(f.state(failed), CommitmentState::Failed);
    // Dropped without a result.
    let dropped = f.prepare(EffectClass::R1);
    c.authorize(dropped, &f.agent, f.run, None).unwrap();
    drop(f.begin(dropped).unwrap());
    assert_eq!(f.state(dropped), CommitmentState::Failed);
    // A panic while executing.
    let panicked = f.prepare(EffectClass::R1);
    c.authorize(panicked, &f.agent, f.run, None).unwrap();
    let guard = f.begin(panicked).unwrap();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
        let _held = guard;
        panic!("actuator bug");
    }));
    assert!(result.is_err());
    assert_eq!(f.state(panicked), CommitmentState::Failed);
    let abandoned = f
        .evidence
        .records()
        .iter()
        .filter(|r| r.failure == Some("abandoned"))
        .count();
    assert_eq!(abandoned, 2);
}

#[test]
fn cancellation_before_the_effect_stops_everything_pending() {
    let f = fixture();
    let c = f.auth.commitments();
    let prepared_only = f.prepare(EffectClass::R1);
    let authorized = f.prepare(EffectClass::R1);
    c.authorize(authorized, &f.agent, f.run, None).unwrap();
    let token = f.auth.runs().check(f.run, &f.agent).unwrap();
    f.auth.cancel_run(f.run).unwrap();
    assert!(
        token.is_cancelled(),
        "whatever of it runs sees the cancellation"
    );
    assert_eq!(f.state(prepared_only), CommitmentState::Revoked);
    assert_eq!(f.state(authorized), CommitmentState::Revoked);
    assert!(f.begin(authorized).is_err());
    assert_eq!(
        c.prepare(&f.agent, f.run, prepared(EffectClass::R1, f.grant), TTL)
            .unwrap_err(),
        AuthorityError::RunCancelled
    );
}

#[test]
fn cancellation_during_execution_is_seen_and_recorded_truthfully() {
    let f = fixture();
    // The actuator observes the token and stops: recorded cancelled.
    let stopped = f.prepare(EffectClass::R1);
    let completed = f.prepare(EffectClass::R1);
    let c = f.auth.commitments();
    c.authorize(stopped, &f.agent, f.run, None).unwrap();
    c.authorize(completed, &f.agent, f.run, None).unwrap();
    let stopping = f.begin(stopped).unwrap();
    let completing = f.begin(completed).unwrap();
    assert!(!stopping.is_cancelled());
    f.auth.cancel_run(f.run).unwrap();
    assert!(
        stopping.is_cancelled(),
        "the bounded execution observes the token"
    );
    assert!(completing.is_cancelled());
    stopping.finish(Outcome::Cancelled { detail: None });
    assert_eq!(f.state(stopped), CommitmentState::Cancelled);
    let last = f.evidence.records().pop().unwrap();
    assert_eq!(last.outcome, Some("cancelled"));
    assert!(last.cancelled);
    // An effect that completed anyway is recorded as completed, with the
    // cancellation beside it: the evidence never hides that it happened.
    completing.finish(Outcome::Succeeded { meta: vec![] });
    assert_eq!(f.state(completed), CommitmentState::Succeeded);
    let last = f.evidence.records().pop().unwrap();
    assert_eq!(last.outcome, Some("succeeded"));
    assert!(last.cancelled);
}

#[test]
fn finishing_a_run_ends_what_it_left_and_releases_what_it_owns() {
    let f = fixture();
    let left = f.prepare(EffectClass::R1);
    let token = f.auth.runs().check(f.run, &f.agent).unwrap();
    f.auth.finish_run(f.run);
    assert!(token.is_cancelled(), "whatever of it runs sees the end");
    assert_eq!(f.state(left), CommitmentState::Revoked);
    assert_eq!(
        f.auth
            .commitments()
            .prepare(&f.agent, f.run, prepared(EffectClass::R1, f.grant), TTL)
            .unwrap_err(),
        AuthorityError::RunNotActive
    );
}

#[test]
fn a_sweep_ends_every_commitment_that_can_no_longer_start() {
    let f = fixture();
    let expiring = f.prepare(EffectClass::R1);
    let authorized = f.prepare(EffectClass::R1);
    f.auth
        .commitments()
        .authorize(authorized, &f.agent, f.run, None)
        .unwrap();
    assert_eq!(f.auth.commitments().sweep(), 0, "both are live");
    f.clock.advance(TTL + Duration::from_secs(1));
    assert_eq!(f.auth.commitments().sweep(), 2);
    assert_eq!(f.state(expiring), CommitmentState::Expired);
    assert_eq!(f.state(authorized), CommitmentState::Expired);
    assert!(!f.auth.commitments().is_unconsumed(expiring));
}

#[test]
fn an_emergency_stop_cancels_every_run_and_refuses_new_ones() {
    let f = fixture();
    let id = f.prepare(EffectClass::R1);
    let other = f
        .auth
        .open_run(f.agent.clone(), RunOrigin::AgentGoal)
        .unwrap();
    let cancelled = f.auth.emergency_stop();
    assert_eq!(cancelled, 2);
    assert_eq!(f.state(id), CommitmentState::Revoked);
    assert_eq!(
        f.auth
            .open_run(f.agent.clone(), RunOrigin::AgentGoal)
            .unwrap_err(),
        AuthorityError::EmergencyStopped
    );
    assert!(f.auth.runs().check(other, &f.agent).is_err());
    assert!(f.phases().contains(&EvidencePhase::EmergencyStop));
    // Lifting the stop raises authority again: only the owner, natively.
    assert_eq!(
        f.auth.resume(&Confirmer::new(false)).unwrap_err(),
        AuthorityError::Declined
    );
    assert!(f
        .auth
        .open_run(f.agent.clone(), RunOrigin::AgentGoal)
        .is_err());
    f.auth.resume(&f.yes).unwrap();
    assert!(f.phases().contains(&EvidencePhase::Resumed));
    assert!(f
        .auth
        .open_run(f.agent.clone(), RunOrigin::AgentGoal)
        .is_ok());
    // The stop moved the policy generation: the old commitment stays ended.
    assert_eq!(f.state(id), CommitmentState::Revoked);
}

#[test]
fn evidence_comes_first_nothing_unrecorded_is_created_or_started() {
    let f = fixture();
    let id = f.prepare(EffectClass::R1);
    f.auth
        .commitments()
        .authorize(id, &f.agent, f.run, None)
        .unwrap();
    f.evidence.fail_from_now();
    assert_eq!(
        f.begin(id).unwrap_err(),
        AuthorityError::EvidenceUnavailable
    );
    assert_eq!(f.state(id), CommitmentState::Authorized, "not started");
    assert_eq!(
        f.auth
            .commitments()
            .prepare(&f.agent, f.run, prepared(EffectClass::R1, f.grant), TTL)
            .unwrap_err(),
        AuthorityError::EvidenceUnavailable
    );
    assert_eq!(
        f.auth
            .grants()
            .request(egress_scope(), TTL, &f.yes)
            .unwrap_err(),
        AuthorityError::EvidenceUnavailable
    );
    assert_eq!(
        f.auth
            .open_run(f.agent.clone(), RunOrigin::AgentGoal)
            .unwrap_err(),
        AuthorityError::EvidenceUnavailable
    );
}

#[test]
fn bounded_summaries_and_bounded_evidence() {
    let f = fixture();
    let mut action = prepared(EffectClass::R1, f.grant);
    action.summary = vec!["x".repeat(super::evidence::MAX_FIELD + 1)];
    assert!(f
        .auth
        .commitments()
        .prepare(&f.agent, f.run, action, TTL)
        .is_err());
    // An action that cannot be shown whole is refused, never shortened.
    let lines = super::commitment::MAX_SUMMARY_LINES;
    let mut action = prepared(EffectClass::R1, f.grant);
    action.summary = vec!["ok".into(); lines + 1];
    assert_eq!(
        f.auth
            .commitments()
            .prepare(&f.agent, f.run, action, TTL)
            .unwrap_err(),
        AuthorityError::InvalidAction("too much to show the owner in full")
    );
    let mut action = prepared(EffectClass::R1, f.grant);
    action.summary = vec!["ok".into(); lines];
    assert!(f
        .auth
        .commitments()
        .prepare(&f.agent, f.run, action, TTL)
        .is_ok());
    // Evidence text is bounded and shows no hidden characters.
    let bounded = super::evidence::bounded(&format!("a\u{202E}b\u{0}\n{}", "c".repeat(400)));
    assert!(bounded.chars().count() <= super::evidence::MAX_FIELD);
    assert!(bounded.starts_with("a\u{FFFD}b  c"));
    assert!(bounded.ends_with('…'));
}

#[test]
fn what_the_owner_is_shown_reads_as_exactly_what_it_is() {
    let f = fixture();
    let hostile = [
        "pay\u{202E}lin.exe",       // right-to-left override
        "example.com\u{200B}.evil", // zero-width space
        "line one\nline two",       // a line break inside one line
        "tab\there",
        "isolate\u{2066}d\u{2069}",
        "tag\u{E0041}",
    ];
    for text in hostile {
        let mut action = prepared(EffectClass::R2, f.grant);
        action.target.display = text.into();
        assert_eq!(
            f.auth
                .commitments()
                .prepare(&f.agent, f.run, action, TTL)
                .unwrap_err(),
            AuthorityError::InvalidAction("display text is not plain"),
            "{text:?}"
        );
        let mut action = prepared(EffectClass::R2, f.grant);
        action.summary = vec![text.into()];
        assert!(f
            .auth
            .commitments()
            .prepare(&f.agent, f.run, action, TTL)
            .is_err());
        // Escaped, the same data is shown visibly and accepted.
        let shown = super::evidence::escaped(text);
        assert!(super::evidence::is_plain(&shown), "{shown:?}");
        let mut action = prepared(EffectClass::R2, f.grant);
        action.target.display = shown;
        assert!(f
            .auth
            .commitments()
            .prepare(&f.agent, f.run, action, TTL)
            .is_ok());
    }
    assert_eq!(super::evidence::escaped("a\u{202E}b\n"), "a\\u{202e}b\\n");
    // Escaping never cuts; a long value is shown whole across lines.
    let long = super::evidence::escaped(&"\u{202E}".repeat(200));
    assert_eq!(long, "\\u{202e}".repeat(200));
    assert!(
        !super::evidence::is_plain(&long),
        "one line would be too long"
    );
    let lines = super::evidence::wrapped(&"\u{202E}".repeat(200));
    assert!(lines.iter().all(|l| super::evidence::is_plain(l)));
    let rebuilt: String = lines
        .iter()
        .enumerate()
        .map(|(i, l)| {
            if i == 0 {
                l.as_str()
            } else {
                l.strip_prefix("↳ ").unwrap()
            }
        })
        .collect();
    assert_eq!(rebuilt, long);
    // A grant whose text is not plain is refused before the owner is asked.
    let asked = Confirmer::new(true);
    assert_eq!(
        f.auth
            .grants()
            .request(
                GrantScope::Perception {
                    display: "agent\u{202E}".into()
                },
                TTL,
                &asked
            )
            .unwrap_err(),
        AuthorityError::InvalidAction("grant text is not plain")
    );
    assert_eq!(asked.grants.load(Ordering::SeqCst), 0);
}

#[test]
fn identities_are_opaque_and_validated() {
    assert!(AgentId::new("").is_none());
    assert!(AgentId::new(&"a".repeat(65)).is_none());
    assert!(AgentId::new("agent/../x").is_none());
    assert!(AgentId::new("a b").is_none());
    let id = CommitmentId::fresh();
    assert_eq!(CommitmentId::parse(&id.to_string()), Some(id));
    assert_ne!(CommitmentId::fresh(), CommitmentId::fresh());
    // Domain-separated, length-prefixed digests.
    assert_ne!(Digest::of("a", &[b"bc"]), Digest::of("a", &[b"b", b"c"]));
    assert_ne!(Digest::of("a", &[b"x"]), Digest::of("b", &[b"x"]));
}

/// What the owner approves is shown whole: content keeps every line, each
/// line marked so that none can pass for the dialog's own text; a hint that
/// only selects is visibly shortened; every invisible format character is
/// escaped; a grant says who may use it.
#[test]
fn nothing_the_owner_approves_is_cut_or_disguised() {
    use super::evidence::{hint, is_plain, quoted, shown, MAX_FIELD};
    let text = format!("Target: https://safe.example\n{}\n\nend", "x".repeat(600));
    let lines = quoted("Body", &text);
    assert_eq!(lines[0], "Body:");
    assert!(lines.iter().all(|l| is_plain(l)), "{lines:?}");
    assert_eq!(lines[1], "│ Target: https://safe.example");
    assert!(lines[1..].iter().all(|l| l.starts_with('│')));
    // The lines rebuild the text exactly.
    let mut rebuilt: Vec<String> = Vec::new();
    for line in &lines[1..] {
        if let Some(more) = line.strip_prefix("│↳ ") {
            rebuilt.last_mut().unwrap().push_str(more);
        } else {
            rebuilt.push(line.strip_prefix("│ ").unwrap().to_string());
        }
    }
    assert_eq!(rebuilt.join("\n"), text);
    assert!(lines.iter().all(|l| l.chars().count() <= MAX_FIELD));
    assert_eq!(hint(&"t".repeat(64)), "t".repeat(64));
    assert_eq!(hint(&"t".repeat(65)), format!("{}…", "t".repeat(63)));
    for c in [
        '\u{0600}',
        '\u{06DD}',
        '\u{070F}',
        '\u{0890}',
        '\u{08E2}',
        '\u{110BD}',
        '\u{110CD}',
        '\u{13430}',
        '\u{1BCA0}',
        '\u{1D173}',
    ] {
        assert!(!shown(c), "{c:?}");
    }
    let grant = super::approval::GrantConfirmation {
        kind: CapabilityKind::Perception,
        lines: vec!["Observe the isolated agent display :200".into()],
        expires_in_secs: 60,
    };
    assert!(grant
        .message()
        .contains("Who may use it: any agent, until it expires or you revoke it"));
}

/// The Unicode spaces (`U+3000` draws blank and twice as wide) are never
/// shown as themselves: content padded with them could push its own lines
/// out of view with nothing visible. They are escaped, visibly; only the
/// ASCII space is shown as itself.
#[test]
fn no_wide_or_unusual_blank_is_shown_as_itself() {
    use super::evidence::{escaped, is_plain, quoted, shown};
    assert!(shown(' '));
    for c in [
        '\u{00A0}', '\u{1680}', '\u{2000}', '\u{2003}', '\u{2007}', '\u{200A}', '\u{2028}',
        '\u{2029}', '\u{202F}', '\u{205F}', '\u{3000}', '\u{0085}',
    ] {
        assert!(!shown(c), "{c:?}");
        assert!(!is_plain(&format!("a{c}b")), "{c:?}");
        let shown_as = escaped(&format!("a{c}b"));
        assert!(is_plain(&shown_as), "{c:?}: {shown_as}");
        assert!(shown_as.contains("\\u{"), "{c:?}: {shown_as}");
    }
    // The audit's padding: a line of ideographic spaces before a fake
    // header line is quoted as visible escapes, marked as content.
    let padded = format!(
        "{}Target: https://payments.trusted-bank.example:443",
        "\u{3000}".repeat(26)
    );
    let lines = quoted("Body", &padded);
    assert!(lines.iter().all(|l| is_plain(l)), "{lines:?}");
    assert!(lines[1].starts_with("│ \\u{3000}"), "{lines:?}");
    assert!(!lines.iter().any(|l| l.contains('\u{3000}')));
    // A line that would show one is refused, not shown.
    let f = fixture();
    let mut action = prepared(EffectClass::R2, f.grant);
    action.summary = vec![format!(
        "{}Target: https://evil.example",
        "\u{3000}".repeat(8)
    )];
    assert_eq!(
        f.auth
            .commitments()
            .prepare(&f.agent, f.run, action, TTL)
            .unwrap_err(),
        AuthorityError::InvalidAction("display text is not plain")
    );
}

/// The confirmation keeps the security identity in its header (what the
/// window shows fixed) and what the request carries in its details: no
/// detail line is a header row, and every header row is the backend's.
#[test]
fn an_approval_keeps_its_identity_apart_from_the_requests_content() {
    use super::evidence::quoted;
    let f = fixture();
    let c = f.auth.commitments();
    let mut action = prepared(EffectClass::R2, f.grant);
    action.summary = quoted(
        "Body",
        "Target: https://payments.trusted-bank.example:443\nOperation: nothing",
    );
    let id = c.prepare(&f.agent, f.run, action, TTL).unwrap().id;
    let _ = c.request_approval(id, &f.agent, f.run, &f.yes).unwrap();
    let seen = f.yes.last.lock().unwrap().clone().unwrap();
    let text = seen.text();
    let labels: Vec<&str> = text.header.iter().map(|(label, _)| *label).collect();
    assert_eq!(
        labels,
        [
            "Effect",
            "Operation",
            "Target",
            "Acting for",
            "Run",
            "Commitment",
            "Binding",
            "Expires in"
        ]
    );
    let value = |label: &str| {
        text.header
            .iter()
            .find(|(l, _)| *l == label)
            .map(|(_, v)| v.clone())
            .unwrap()
    };
    assert_eq!(value("Effect"), "R2 (a sensitive or irreversible effect)");
    assert_eq!(value("Operation"), "egress.fetch");
    assert_eq!(value("Target"), "https://example.invalid/");
    assert_eq!(value("Acting for"), f.agent.to_string());
    assert_eq!(value("Run"), f.run.to_string());
    assert_eq!(value("Commitment"), id.to_string());
    assert_eq!(value("Binding"), c.view(id).unwrap().binding_short);
    // The request's lines are all in the details, each marked.
    assert_eq!(text.details[0], "Body:");
    assert!(text.details[1..].iter().all(|l| l.starts_with('│')));
    assert!(text.header.iter().all(|(_, v)| !v.contains("trusted-bank")));
}

/// The evidence carries a commitment's parameters only salted with a nonce
/// kept in memory: two commitments with the same parameters cannot be
/// linked, and the parameters cannot be guessed from the record.
#[test]
fn evidence_carries_parameters_only_salted() {
    let f = fixture();
    let plain = prepared(EffectClass::R1, f.grant).parameters.to_hex();
    for _ in 0..2 {
        f.auth
            .commitments()
            .prepare(&f.agent, f.run, prepared(EffectClass::R1, f.grant), TTL)
            .unwrap();
    }
    let digests: Vec<String> = f
        .evidence
        .records()
        .iter()
        .filter(|r| r.phase == EvidencePhase::Prepared)
        .map(|r| r.parameters_digest.clone().unwrap())
        .collect();
    assert_eq!(digests.len(), 2);
    assert_ne!(digests[0], digests[1]);
    assert!(!digests.contains(&plain));
}

/// Every commitment rests on a live grant of its own kind: none, or one of
/// another kind, covers nothing.
#[test]
fn a_commitment_rests_on_a_live_grant_of_its_own_kind() {
    let f = fixture();
    let mut action = prepared(EffectClass::R1, f.grant);
    action.grants.clear();
    assert_eq!(
        f.auth
            .commitments()
            .prepare(&f.agent, f.run, action, TTL)
            .unwrap_err(),
        AuthorityError::NoCoveringGrant
    );
    let mut action = prepared(EffectClass::R1, f.grant);
    action.kind = CapabilityKind::Connector;
    assert_eq!(
        f.auth
            .commitments()
            .prepare(&f.agent, f.run, action, TTL)
            .unwrap_err(),
        AuthorityError::NoCoveringGrant,
        "an egress grant does not cover a connector action"
    );
}

/// Authorization is evidence-first: when its record cannot be written, the
/// commitment stays prepared and cannot start.
#[test]
fn an_unrecorded_authorization_authorizes_nothing() {
    let f = fixture();
    let id = f.prepare(EffectClass::R1);
    f.evidence.fail_from_now();
    assert_eq!(
        f.auth
            .commitments()
            .authorize(id, &f.agent, f.run, None)
            .unwrap_err(),
        AuthorityError::EvidenceUnavailable
    );
    assert_eq!(f.state(id), CommitmentState::Prepared);
}

/// Finishing a run ends it for whatever of it still runs (its token is
/// set), and nothing more starts in it.
#[test]
fn finishing_a_run_sets_its_token() {
    let f = fixture();
    let token = f.auth.runs().check(f.run, &f.agent).unwrap();
    f.auth.finish_run(f.run);
    assert!(token.is_cancelled());
    assert_eq!(
        f.auth.runs().check(f.run, &f.agent).unwrap_err(),
        AuthorityError::RunNotActive
    );
}

/// Grants are bounded: ended ones are pruned before a new one is refused.
#[test]
fn ended_grants_make_room_for_new_ones() {
    let f = fixture();
    for _ in 0..1023 {
        f.auth
            .grants()
            .request(egress_scope(), Duration::from_secs(1), &f.yes)
            .unwrap();
    }
    assert_eq!(
        f.auth
            .grants()
            .request(egress_scope(), TTL, &f.yes)
            .unwrap_err(),
        AuthorityError::Capacity
    );
    f.clock.advance(Duration::from_secs(2));
    assert!(f.auth.grants().request(egress_scope(), TTL, &f.yes).is_ok());
    assert!(f.auth.grants().live(f.grant).is_some(), "live grants stay");
}

/// Time the machine spends suspended counts: the monotonic clock does not
/// see it, the wall clock does, and either ends a grant or a commitment.
#[test]
fn a_suspended_machine_does_not_extend_a_grant_or_a_commitment() {
    let f = fixture();
    let id = f.prepare(EffectClass::R1);
    f.clock.suspend(Duration::from_secs(601));
    assert!(f.auth.grants().live(f.grant).is_none());
    assert_eq!(
        f.auth
            .commitments()
            .authorize(id, &f.agent, f.run, None)
            .unwrap_err(),
        AuthorityError::Expired
    );
    assert_eq!(f.state(id), CommitmentState::Expired);
}

/// Grant evidence keeps every line of the scope in its own bounded field,
/// so a long scope never loses its flags to one cut sentence.
#[test]
fn grant_evidence_keeps_every_line_of_the_scope() {
    let f = fixture();
    // Each line fits a field; joined, they would not.
    let origins: Vec<String> = (0..4)
        .map(|i| format!("https://a-rather-long-origin-name-{i}.example:443"))
        .collect();
    f.auth
        .grants()
        .request(
            GrantScope::Browser {
                origins,
                downloads: true,
                executable: "/opt/google/chrome/chrome".into(),
                identity: Digest::of("identity", &[]),
            },
            Duration::from_secs(60),
            &f.yes,
        )
        .unwrap();
    let record = f
        .evidence
        .records()
        .into_iter()
        .rev()
        .find(|r| r.phase == EvidencePhase::GrantIssued)
        .unwrap();
    let scope: String = record
        .detail
        .iter()
        .filter(|(key, _)| key.starts_with("scope."))
        .map(|(_, value)| value.as_str())
        .collect();
    assert!(
        scope.contains("a-rather-long-origin-name-3.example"),
        "{scope}"
    );
    assert!(
        scope.contains("Downloads: kept inside the session"),
        "{scope}"
    );
    assert!(
        scope.contains("Browser: /opt/google/chrome/chrome"),
        "{scope}"
    );
    let json = record.to_json().to_string();
    assert!(
        json.contains("Downloads: kept inside the session"),
        "{json}"
    );
}

/// A backslash in content is shown doubled, so text cannot pass for an
/// escaped hidden character; a blank-looking braille cell is escaped; and
/// a combining mark beyond two stacked on one character is escaped.
#[test]
fn shown_text_cannot_imitate_an_escape_or_draw_over_its_neighbours() {
    use crate::authority::evidence::escaped;
    assert_eq!(escaped("a\\u{202e}b"), "a\\\\u{202e}b");
    assert_eq!(escaped("a\u{202e}b"), "a\\u{202e}b");
    assert_eq!(escaped("x\u{2800}y"), "x\\u{2800}y");
    assert_eq!(
        escaped("e\u{301}\u{301}\u{301}\u{301}"),
        "e\u{301}\u{301}\\u{301}\\u{301}"
    );
    // Marks of any script count, and marks of different scripts interleaved
    // on one character still stack (re-audit Q9).
    assert_eq!(
        escaped("a\u{301}\u{e49}\u{301}\u{e49}"),
        "a\u{301}\u{e49}\\u{301}\\u{e49}"
    );
    for mark in ['\u{5b4}', '\u{e48}', '\u{489}', '\u{f90}', '\u{a8e0}'] {
        let stacked: String = std::iter::once('x')
            .chain(std::iter::repeat_n(mark, 4))
            .collect();
        let shown = escaped(&stacked);
        assert_eq!(
            shown.chars().filter(|c| *c == mark).count(),
            2,
            "{mark:?}: {shown}"
        );
    }
    // Ordinary text of other scripts is shown as written.
    assert_eq!(escaped("שלום עולם"), "שלום עולם");
    assert_eq!(escaped("café naïve"), "café naïve");
}

// ---- No authority lock is held while evidence is recorded ----
//
// Every record is written with every authority lock released: the
// evidence-first transitions (approval, authorization, start) reserve the
// commitment under the lock, record with it released and complete only if
// the same reservation still holds a live commitment.

use super::evidence::EvidenceRecord;
use super::scripted::{within, ScriptedSink};
use std::sync::Weak;

struct Scripted {
    auth: Arc<Authority>,
    sink: Arc<ScriptedSink>,
    clock: Arc<ManualClock>,
    yes: Confirmer,
    agent: AgentId,
    run: RunId,
    grant: GrantId,
}

fn scripted() -> Scripted {
    let sink = ScriptedSink::new();
    let clock = Arc::new(ManualClock::default());
    let auth = Arc::new(Authority::new(sink.clone(), clock.clone()));
    let yes = Confirmer::new(true);
    let agent = AgentId::new("agent-a").unwrap();
    let run = auth.open_run(agent.clone(), RunOrigin::AgentGoal).unwrap();
    let grant = auth
        .grants()
        .request(egress_scope(), Duration::from_secs(600), &yes)
        .unwrap();
    Scripted {
        auth,
        sink,
        clock,
        yes,
        agent,
        run,
        grant,
    }
}

impl Scripted {
    fn prepare(&self, class: EffectClass) -> CommitmentId {
        self.auth
            .commitments()
            .prepare(&self.agent, self.run, prepared(class, self.grant), TTL)
            .unwrap()
            .id
    }

    fn authorized(&self) -> CommitmentId {
        let id = self.prepare(EffectClass::R1);
        self.auth
            .commitments()
            .authorize(id, &self.agent, self.run, None)
            .unwrap();
        id
    }

    fn begin(&self, id: CommitmentId) -> Result<super::commitment::ExecutionGuard, AuthorityError> {
        begin_as(&self.auth, &self.agent, self.run, id)
    }

    fn state(&self, id: CommitmentId) -> CommitmentState {
        self.auth.commitments().view(id).unwrap().state
    }

    /// While every record is written, probe every authority lock from
    /// another thread.
    fn probe(&self) {
        let auth: Weak<Authority> = Arc::downgrade(&self.auth);
        self.sink.probe_with(move || {
            if let Some(auth) = auth.upgrade() {
                auth.probe_locks();
            }
        });
    }

    fn last(&self, phase: EvidencePhase, id: CommitmentId) -> EvidenceRecord {
        self.sink
            .memory
            .records()
            .into_iter()
            .rev()
            .find(|r| r.phase == phase && r.commitment.as_deref() == Some(&id.to_string()))
            .unwrap_or_else(|| panic!("no {phase:?} record for {id}"))
    }
}

fn begin_as(
    auth: &Authority,
    agent: &AgentId,
    run: RunId,
    id: CommitmentId,
) -> Result<super::commitment::ExecutionGuard, AuthorityError> {
    auth.commitments().begin(
        id,
        agent,
        run,
        &target("https://example.invalid/").digest,
        &params("GET"),
    )
}

/// Every record of every authority transition, with a probe of every
/// authority lock from another thread while it is written: none is held.
#[test]
fn no_authority_lock_is_held_while_any_record_is_written() {
    let f = scripted();
    let unprobed = f.sink.memory.records().len();
    f.probe();
    let c = f.auth.commitments();
    // Grant issue, run opening.
    let grant = f
        .auth
        .grants()
        .request(egress_scope(), Duration::from_secs(600), &f.yes)
        .unwrap();
    let declined = Confirmer::new(false);
    assert!(f
        .auth
        .grants()
        .request(egress_scope(), Duration::from_secs(600), &declined)
        .is_err());
    let run = f
        .auth
        .open_run(f.agent.clone(), RunOrigin::AgentGoal)
        .unwrap();
    // Prepare, authorize, start, finish.
    let id = f.prepare(EffectClass::R1);
    c.authorize(id, &f.agent, f.run, None).unwrap();
    f.begin(id)
        .unwrap()
        .finish(Outcome::Succeeded { meta: vec![] });
    // Approval, approval declined, denial.
    let r2 = f.prepare(EffectClass::R2);
    let approval = c.request_approval(r2, &f.agent, f.run, &f.yes).unwrap();
    c.authorize(r2, &f.agent, f.run, Some(approval)).unwrap();
    let declined_r2 = f.prepare(EffectClass::R2);
    assert!(c
        .request_approval(declined_r2, &f.agent, f.run, &declined)
        .is_err());
    let denied = f.prepare(EffectClass::R1);
    c.deny(denied, &f.agent, f.run).unwrap();
    // A changed target, a failure before the start, an abandoned guard.
    let changed = f.authorized();
    assert!(c
        .begin(
            changed,
            &f.agent,
            f.run,
            &target("elsewhere").digest,
            &params("GET")
        )
        .is_err());
    let unstarted = f.authorized();
    c.fail_unstarted(unstarted, &f.agent, f.run, &AuthorityError::TargetChanged)
        .unwrap();
    drop(f.begin(f.authorized()).unwrap());
    // Expiry by sweep, cancellation, run finish, grant revocation, an
    // emergency stop and the resume.
    let expiring = f.prepare(EffectClass::R1);
    f.clock.advance(TTL + Duration::from_secs(1));
    assert!(c.sweep() >= 1);
    assert_eq!(f.state(expiring), CommitmentState::Expired);
    let in_run = c
        .prepare(&f.agent, run, prepared(EffectClass::R1, f.grant), TTL)
        .unwrap()
        .id;
    f.auth.cancel_run(run).unwrap();
    assert_eq!(f.state(in_run), CommitmentState::Revoked);
    let finishing = f
        .auth
        .open_run(f.agent.clone(), RunOrigin::AgentGoal)
        .unwrap();
    c.prepare(&f.agent, finishing, prepared(EffectClass::R1, f.grant), TTL)
        .unwrap();
    f.auth.finish_run(finishing);
    f.auth.grants().revoke(grant).unwrap();
    f.prepare(EffectClass::R1);
    f.auth.emergency_stop();
    f.auth.resume(&f.yes).unwrap();
    // A preparation whose run is cancelled while it is recorded ends
    // revoked, recorded too.
    let run = f
        .auth
        .open_run(f.agent.clone(), RunOrigin::AgentGoal)
        .unwrap();
    f.sink.hold_next(EvidencePhase::Prepared);
    let (auth, agent, grant) = (f.auth.clone(), f.agent.clone(), f.grant);
    let preparing = std::thread::spawn(move || {
        auth.commitments()
            .prepare(&agent, run, prepared(EffectClass::R1, grant), TTL)
    });
    f.sink.wait_held();
    let auth = f.auth.clone();
    within(move || auth.cancel_run(run)).unwrap();
    f.sink.release();
    assert_eq!(
        preparing.join().unwrap().unwrap_err(),
        AuthorityError::RunCancelled
    );
    let phases = f.sink.phases();
    for phase in [
        EvidencePhase::GrantIssued,
        EvidencePhase::GrantDeclined,
        EvidencePhase::RunOpened,
        EvidencePhase::Prepared,
        EvidencePhase::Approved,
        EvidencePhase::ApprovalDeclined,
        EvidencePhase::Authorized,
        EvidencePhase::Started,
        EvidencePhase::Finished,
        EvidencePhase::Denied,
        EvidencePhase::Expired,
        EvidencePhase::Revoked,
        EvidencePhase::RunCancelled,
        EvidencePhase::GrantRevoked,
        EvidencePhase::EmergencyStop,
        EvidencePhase::Resumed,
    ] {
        assert!(phases.contains(&phase), "{phase:?} in {phases:?}");
    }
    assert!(
        f.sink.probes() >= phases.len() - unprobed,
        "{} probes, {} records",
        f.sink.probes(),
        phases.len()
    );
    assert_eq!(
        f.sink.violations(),
        0,
        "an authority lock was held while a record was written"
    );
}

/// A sink that reads the authority on its own recording thread (as an
/// audit sink that looks up what it records would): no transition holds a
/// lock it needs, so none deadlocks.
#[test]
fn a_reentrant_sink_never_deadlocks_the_authority() {
    let f = scripted();
    let auth: Weak<Authority> = Arc::downgrade(&f.auth);
    let run = f.run;
    f.sink.reenter_with(move |record| {
        let Some(auth) = auth.upgrade() else {
            return;
        };
        let c = auth.commitments();
        if let Some(id) = record.commitment.as_deref().and_then(CommitmentId::parse) {
            let _ = c.view(id);
            let _ = c.is_unconsumed(id);
            let _ = c.lapse(id);
        }
        let _ = c.views_of_run(run);
        let _ = auth.grants().all();
        let _ = auth.grants().live_of(CapabilityKind::Egress);
        let _ = auth.runs().views();
        let _ = auth.runs().view(run);
        let _ = auth.runs().class(run);
    });
    let (auth, agent, grant, yes) = (f.auth.clone(), f.agent.clone(), f.grant, f.yes);
    within(move || {
        let c = auth.commitments();
        let id = c
            .prepare(&agent, run, prepared(EffectClass::R2, grant), TTL)
            .unwrap()
            .id;
        let approval = c.request_approval(id, &agent, run, &yes).unwrap();
        c.authorize(id, &agent, run, Some(approval)).unwrap();
        begin_as(&auth, &agent, run, id)
            .unwrap()
            .finish(Outcome::Succeeded { meta: vec![] });
        let denied = c
            .prepare(&agent, run, prepared(EffectClass::R1, grant), TTL)
            .unwrap()
            .id;
        c.deny(denied, &agent, run).unwrap();
        c.prepare(&agent, run, prepared(EffectClass::R1, grant), TTL)
            .unwrap();
        auth.grants()
            .request(egress_scope(), Duration::from_secs(600), &yes)
            .unwrap();
        c.sweep();
        auth.cancel_run(run).unwrap();
        auth.emergency_stop();
    });
}

/// A slow sink: while the start of one commitment is being recorded, no
/// lock is held (the rest of the authority goes on), and the commitment is
/// not yet executing.
#[test]
fn a_slow_start_record_holds_no_lock_and_makes_nothing_executable() {
    let f = Arc::new(scripted());
    let id = f.authorized();
    f.sink.hold_next(EvidencePhase::Started);
    let starting = {
        let f = f.clone();
        std::thread::spawn(move || f.begin(id))
    };
    f.sink.wait_held();
    // Recorded, not yet in effect.
    assert_eq!(f.state(id), CommitmentState::Authorized);
    assert_eq!(
        f.auth.commitments().lapse(id),
        Some("the action is no longer executing")
    );
    // The authority goes on meanwhile.
    let other = {
        let f = f.clone();
        within(move || {
            let other = f.authorized();
            f.begin(other)
                .unwrap()
                .finish(Outcome::Succeeded { meta: vec![] });
            other
        })
    };
    assert_eq!(f.state(other), CommitmentState::Succeeded);
    f.sink.release();
    let guard = starting.join().unwrap().unwrap();
    assert_eq!(f.state(id), CommitmentState::Executing);
    guard.finish(Outcome::Succeeded { meta: vec![] });
    assert_eq!(f.state(id), CommitmentState::Succeeded);
}

/// A record that fails: the transition does not take effect, and the
/// commitment is as it was (an approval is never given, an authorization
/// never made, a start never made).
#[test]
fn a_failed_record_leaves_nothing_approved_authorized_or_started() {
    let f = scripted();
    let c = f.auth.commitments();
    // Authorization.
    let id = f.prepare(EffectClass::R1);
    f.sink.fail_on(Some(EvidencePhase::Authorized));
    assert_eq!(
        c.authorize(id, &f.agent, f.run, None),
        Err(AuthorityError::EvidenceUnavailable)
    );
    assert_eq!(f.state(id), CommitmentState::Prepared);
    assert_eq!(f.begin(id).unwrap_err(), AuthorityError::NotAuthorized);
    f.sink.fail_on(None);
    // Withdrawn, not stuck: it can be authorized once evidence works.
    c.authorize(id, &f.agent, f.run, None).unwrap();
    // Approval.
    let r2 = f.prepare(EffectClass::R2);
    f.sink.fail_on(Some(EvidencePhase::Approved));
    assert_eq!(
        c.request_approval(r2, &f.agent, f.run, &f.yes).unwrap_err(),
        AuthorityError::EvidenceUnavailable
    );
    assert_eq!(
        c.authorize(r2, &f.agent, f.run, None),
        Err(AuthorityError::ApprovalRequired)
    );
    f.sink.fail_on(None);
    // An approval whose authorization is not recorded is spent.
    let approval = c.request_approval(r2, &f.agent, f.run, &f.yes).unwrap();
    f.sink.fail_on(Some(EvidencePhase::Authorized));
    assert_eq!(
        c.authorize(r2, &f.agent, f.run, Some(approval)),
        Err(AuthorityError::EvidenceUnavailable)
    );
    assert_eq!(f.state(r2), CommitmentState::Prepared);
    assert_eq!(
        c.authorize(r2, &f.agent, f.run, None),
        Err(AuthorityError::ApprovalRequired)
    );
    f.sink.fail_on(None);
    // Start.
    let started = f.authorized();
    f.sink.fail_on(Some(EvidencePhase::Started));
    assert_eq!(
        f.begin(started).unwrap_err(),
        AuthorityError::EvidenceUnavailable
    );
    assert_eq!(f.state(started), CommitmentState::Authorized);
    assert!(c.lapse(started).is_some());
    f.sink.fail_on(None);
    let phases = f.sink.phases();
    assert!(!phases.contains(&EvidencePhase::Started), "{phases:?}");
}

/// A cancellation that comes while a start is being recorded wins: the
/// commitment ends revoked at once (its record says the start was
/// interrupted), and the start, once recorded, finds it gone.
#[test]
fn a_cancellation_while_the_start_is_recorded_wins() {
    let f = Arc::new(scripted());
    let id = f.authorized();
    f.sink.hold_next(EvidencePhase::Started);
    let starting = {
        let f = f.clone();
        std::thread::spawn(move || f.begin(id))
    };
    f.sink.wait_held();
    let (auth, run) = (f.auth.clone(), f.run);
    within(move || auth.cancel_run(run)).unwrap();
    assert_eq!(f.state(id), CommitmentState::Revoked);
    let revoked = f.last(EvidencePhase::Revoked, id);
    assert!(
        revoked
            .detail
            .contains(&("interrupted".to_string(), "start".to_string())),
        "{:?}",
        revoked.detail
    );
    f.sink.release();
    assert_eq!(
        starting.join().unwrap().unwrap_err(),
        AuthorityError::RunCancelled
    );
    // Nothing revives it.
    assert_eq!(f.state(id), CommitmentState::Revoked);
    assert_eq!(f.begin(id).unwrap_err(), AuthorityError::RunCancelled);
    assert!(f.auth.commitments().lapse(id).is_some());
}

/// A revocation, a denial, an emergency stop or an expiry that comes while
/// an evidence-first transition is being recorded wins; the stale
/// reservation never completes.
#[test]
fn whatever_ends_a_commitment_while_its_transition_is_recorded_wins() {
    // A grant revoked while the authorization is recorded.
    let f = Arc::new(scripted());
    let id = f.prepare(EffectClass::R1);
    f.sink.hold_next(EvidencePhase::Authorized);
    let authorizing = {
        let f = f.clone();
        std::thread::spawn(move || f.auth.commitments().authorize(id, &f.agent, f.run, None))
    };
    f.sink.wait_held();
    let (auth, grant) = (f.auth.clone(), f.grant);
    within(move || auth.grants().revoke(grant)).unwrap();
    f.sink.release();
    assert_eq!(authorizing.join().unwrap(), Err(AuthorityError::Stale));
    assert_eq!(f.state(id), CommitmentState::Revoked);
    assert_eq!(f.begin(id).unwrap_err(), AuthorityError::Stale);

    // A denial while the authorization is recorded.
    let f = Arc::new(scripted());
    let id = f.prepare(EffectClass::R1);
    f.sink.hold_next(EvidencePhase::Authorized);
    let authorizing = {
        let f = f.clone();
        std::thread::spawn(move || f.auth.commitments().authorize(id, &f.agent, f.run, None))
    };
    f.sink.wait_held();
    let (auth, agent, run) = (f.auth.clone(), f.agent.clone(), f.run);
    within(move || auth.commitments().deny(id, &agent, run)).unwrap();
    f.sink.release();
    assert_eq!(authorizing.join().unwrap(), Err(AuthorityError::NotPending));
    assert_eq!(f.state(id), CommitmentState::Denied);
    let denied = f.last(EvidencePhase::Denied, id);
    assert!(denied
        .detail
        .contains(&("interrupted".to_string(), "authorization".to_string())));

    // An emergency stop while the approval is recorded: no approval.
    let f = Arc::new(scripted());
    let id = f.prepare(EffectClass::R2);
    f.sink.hold_next(EvidencePhase::Approved);
    let approving = {
        let f = f.clone();
        std::thread::spawn(move || {
            f.auth
                .commitments()
                .request_approval(id, &f.agent, f.run, &f.yes)
                .map(|_| ())
        })
    };
    f.sink.wait_held();
    let auth = f.auth.clone();
    within(move || auth.emergency_stop());
    f.sink.release();
    assert_eq!(
        approving.join().unwrap(),
        Err(AuthorityError::EmergencyStopped)
    );
    assert_eq!(f.state(id), CommitmentState::Revoked);

    // An expiry while the start is recorded.
    let f = Arc::new(scripted());
    let id = f.authorized();
    f.sink.hold_next(EvidencePhase::Started);
    let starting = {
        let f = f.clone();
        std::thread::spawn(move || f.begin(id).map(|_| ()))
    };
    f.sink.wait_held();
    f.clock.advance(TTL + Duration::from_secs(1));
    f.sink.release();
    assert_eq!(starting.join().unwrap(), Err(AuthorityError::Expired));
    assert_eq!(f.state(id), CommitmentState::Expired);
}

/// A sink that panics, caught outside the authority: no authority lock is
/// poisoned, the interrupted transition is withdrawn, and the authority
/// goes on.
#[test]
fn a_panicking_sink_poisons_no_authority_lock() {
    use std::panic::{catch_unwind, AssertUnwindSafe};
    let f = scripted();
    let c = f.auth.commitments();
    let id = f.authorized();
    for phase in [
        EvidencePhase::Started,
        EvidencePhase::Authorized,
        EvidencePhase::Prepared,
        EvidencePhase::Revoked,
        EvidencePhase::GrantIssued,
    ] {
        f.sink.panic_on(Some(phase));
        let panicked = catch_unwind(AssertUnwindSafe(|| match phase {
            EvidencePhase::Started => drop(f.begin(id)),
            EvidencePhase::Authorized => {
                let other = f.prepare(EffectClass::R1);
                let _ = c.authorize(other, &f.agent, f.run, None);
            }
            EvidencePhase::Prepared => drop(f.prepare(EffectClass::R1)),
            EvidencePhase::Revoked => {
                f.prepare(EffectClass::R1);
                let _ = f.auth.cancel_run(f.run);
            }
            _ => drop(
                f.auth
                    .grants()
                    .request(egress_scope(), Duration::from_secs(600), &f.yes),
            ),
        }));
        assert!(panicked.is_err(), "{phase:?} did not panic");
        f.sink.panic_on(None);
        // Every lock is free and none is poisoned.
        let auth = f.auth.clone();
        within(move || auth.probe_locks());
        assert!(c.view(id).is_some());
        assert!(!f.auth.grants().all().is_empty());
        assert_eq!(f.auth.grants().rooms_held(), 0, "{phase:?}");
        assert!(!f.auth.runs().views().is_empty());
        c.sweep();
    }
    // The start the panic interrupted was withdrawn, so it can start now
    // (its run was cancelled since: it ended revoked).
    assert_eq!(f.state(id), CommitmentState::Revoked);
    let run = f
        .auth
        .open_run(f.agent.clone(), RunOrigin::AgentGoal)
        .unwrap();
    let fresh = c
        .prepare(&f.agent, run, prepared(EffectClass::R1, f.grant), TTL)
        .unwrap()
        .id;
    c.authorize(fresh, &f.agent, run, None).unwrap();
    f.sink.panic_on(Some(EvidencePhase::Started));
    assert!(catch_unwind(AssertUnwindSafe(|| drop(begin_as(
        &f.auth, &f.agent, run, fresh
    ))))
    .is_err());
    f.sink.panic_on(None);
    assert_eq!(f.state(fresh), CommitmentState::Authorized);
    begin_as(&f.auth, &f.agent, run, fresh)
        .unwrap()
        .finish(Outcome::Succeeded { meta: vec![] });
}

/// A commitment starts exactly once, however its starts race, also while
/// its first start is being recorded.
#[test]
fn a_commitment_starts_exactly_once_while_its_start_is_recorded() {
    let f = Arc::new(scripted());
    let id = f.authorized();
    f.sink.hold_next(EvidencePhase::Started);
    let first = {
        let f = f.clone();
        std::thread::spawn(move || f.begin(id))
    };
    f.sink.wait_held();
    // A second start while the first is recorded: refused at once.
    let second = {
        let f = f.clone();
        within(move || f.begin(id).map(|_| ()))
    };
    assert_eq!(second, Err(AuthorityError::NotAuthorized));
    f.sink.release();
    let guard = first.join().unwrap().unwrap();
    assert_eq!(f.begin(id).unwrap_err(), AuthorityError::NotAuthorized);
    guard.finish(Outcome::Succeeded { meta: vec![] });
    // Many racing starts, one success.
    let id = f.authorized();
    let barrier = Arc::new(std::sync::Barrier::new(16));
    let starts: Vec<_> = (0..16)
        .map(|_| {
            let (f, barrier) = (f.clone(), barrier.clone());
            std::thread::spawn(move || {
                barrier.wait();
                f.begin(id).map(|guard| {
                    guard.finish(Outcome::Succeeded { meta: vec![] });
                })
            })
        })
        .collect();
    let started = starts
        .into_iter()
        .map(|start| start.join().unwrap())
        .filter(Result::is_ok)
        .count();
    assert_eq!(started, 1);
    let starts = f
        .sink
        .phases()
        .iter()
        .filter(|p| **p == EvidencePhase::Started)
        .count();
    assert_eq!(starts, 2);
}

/// Evidence first, observed from inside the sink: when an approval, an
/// authorization or a start is recorded, it has not taken effect yet.
#[test]
fn no_transition_is_in_effect_before_its_record() {
    let f = scripted();
    let seen: Arc<Mutex<Vec<(EvidencePhase, CommitmentState)>>> = Arc::default();
    let (auth, saw): (Weak<Authority>, _) = (Arc::downgrade(&f.auth), seen.clone());
    f.sink.reenter_with(move |record| {
        if !matches!(
            record.phase,
            EvidencePhase::Approved | EvidencePhase::Authorized | EvidencePhase::Started
        ) {
            return;
        }
        let (Some(auth), Some(id)) = (
            auth.upgrade(),
            record.commitment.as_deref().and_then(CommitmentId::parse),
        ) else {
            return;
        };
        let state = auth.commitments().view(id).unwrap().state;
        saw.lock().unwrap().push((record.phase, state));
    });
    let c = f.auth.commitments();
    let id = f.prepare(EffectClass::R2);
    let approval = c.request_approval(id, &f.agent, f.run, &f.yes).unwrap();
    c.authorize(id, &f.agent, f.run, Some(approval)).unwrap();
    assert_eq!(f.state(id), CommitmentState::Authorized);
    let guard = f.begin(id).unwrap();
    assert_eq!(f.state(id), CommitmentState::Executing);
    guard.finish(Outcome::Succeeded { meta: vec![] });
    assert_eq!(
        *seen.lock().unwrap(),
        [
            (EvidencePhase::Approved, CommitmentState::Prepared),
            (EvidencePhase::Authorized, CommitmentState::Prepared),
            (EvidencePhase::Started, CommitmentState::Authorized),
        ]
    );
}

// ---------------------------------------------------------------------------
// RS-01: a resume lifts only the stop the owner confirmed
// (P3-C11-RS01-CONTRACT-05 §8). A stop or a quit that comes while a resume
// waits for the owner, or while its record is written, is not undone by it.
// ---------------------------------------------------------------------------

use super::run::{CancelToken, Lift, PreviousStop, RunRegistry};

type During = Box<dyn FnOnce() + Send>;

/// The owner's resume dialog, scripted: it counts how often it is asked,
/// keeps what it showed, runs `during` once while it is open (as the native
/// window waits, with no authority lock held), then answers as told.
struct ResumeDialog {
    answer: bool,
    asked: AtomicU32,
    shown: Mutex<Vec<ResumeConfirmation>>,
    during: Mutex<Option<During>>,
}

impl ResumeDialog {
    fn answering(answer: bool) -> Self {
        Self {
            answer,
            asked: AtomicU32::new(0),
            shown: Mutex::new(Vec::new()),
            during: Mutex::new(None),
        }
    }

    fn during(answer: bool, during: impl FnOnce() + Send + 'static) -> Self {
        let dialog = Self::answering(answer);
        *dialog.during.lock().unwrap() = Some(Box::new(during));
        dialog
    }

    fn asked(&self) -> u32 {
        self.asked.load(Ordering::SeqCst)
    }

    fn shown(&self) -> Vec<ResumeConfirmation> {
        self.shown.lock().unwrap().clone()
    }
}

impl ControlConfirmer for ResumeDialog {
    fn confirm_action(&self, _request: &ActionConfirmation) -> bool {
        false
    }

    fn confirm_grant(&self, _request: &GrantConfirmation) -> bool {
        false
    }

    fn confirm_resume(&self, request: &ResumeConfirmation) -> bool {
        self.asked.fetch_add(1, Ordering::SeqCst);
        self.shown.lock().unwrap().push(request.clone());
        let during = self.during.lock().unwrap().take();
        if let Some(during) = during {
            during();
        }
        self.answer
    }
}

/// The records of `phase`, oldest first.
fn records_of(records: Vec<EvidenceRecord>, phase: EvidencePhase) -> Vec<EvidenceRecord> {
    records.into_iter().filter(|r| r.phase == phase).collect()
}

/// The value of `key` in a record's detail.
fn detail<'a>(record: &'a EvidenceRecord, key: &str) -> Option<&'a str> {
    record
        .detail
        .iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.as_str())
}

/// Each stop record as "its number/whether the stop before it was lifted",
/// with "/exhausted" when its record says so.
fn stops_of(records: Vec<EvidenceRecord>) -> Vec<String> {
    records_of(records, EvidencePhase::EmergencyStop)
        .iter()
        .map(|r| {
            let mut stop = format!(
                "{}/{}",
                detail(r, "stop_number").unwrap_or("-"),
                detail(r, "previous_stop_lifted").unwrap_or("-")
            );
            if let Some(exhausted) = detail(r, "exhausted") {
                stop.push_str(&format!("/exhausted {exhausted}"));
            }
            stop
        })
        .collect()
}

const QUITTING: AuthorityError = AuthorityError::Closed("the desktop is quitting");

/// RS-01 T1: a stop that comes while the owner's resume dialog is open is
/// not lifted by that resume, which writes no record.
#[test]
fn rs01_t1_a_stop_during_the_resume_dialog_is_not_lifted() {
    let f = fixture();
    assert_eq!(f.auth.emergency_stop(), 1);
    let auth = f.auth.clone();
    let dialog = ResumeDialog::during(true, move || {
        auth.emergency_stop();
    });
    let resumed = f.auth.resume(&dialog);
    assert!(
        f.auth.runs().is_stopped(),
        "the newer stop was lifted: {resumed:?}"
    );
    assert!(f
        .auth
        .open_run(f.agent.clone(), RunOrigin::AgentGoal)
        .is_err());
    assert_eq!(f.auth.runs().cancelled_by_stop(), 1);
    assert!(resumed.is_err(), "{resumed:?}");
    assert_eq!(dialog.asked(), 1);
    assert!(records_of(f.evidence.records(), EvidencePhase::Resumed).is_empty());
    assert_eq!(resumed, Err(AuthorityError::ResumeSuperseded));
    assert_eq!(dialog.shown()[0].stop_number, 1);
    assert!(
        dialog.shown()[0]
            .text()
            .header
            .contains(&("Lifts", "emergency stop 1".to_string())),
        "the dialog does not name stop 1:\n{}",
        dialog.shown()[0].message()
    );
}

/// RS-01 T2: a stop that comes while the Resumed record is written is not
/// lifted; the resume is superseded and the count stays.
#[test]
fn rs01_t2_a_stop_while_the_resumed_record_is_written_is_not_lifted() {
    let f = scripted();
    assert_eq!(f.auth.emergency_stop(), 1);
    f.sink.hold_next(EvidencePhase::Resumed);
    let resuming = {
        let auth = f.auth.clone();
        std::thread::spawn(move || auth.resume(&Confirmer::new(true)))
    };
    f.sink.wait_held();
    let auth = f.auth.clone();
    within(move || auth.emergency_stop());
    f.sink.release();
    let resumed = resuming.join().unwrap();
    assert!(
        f.auth.runs().is_stopped(),
        "the newer stop was lifted: {resumed:?}"
    );
    assert!(f
        .auth
        .open_run(f.agent.clone(), RunOrigin::AgentGoal)
        .is_err());
    assert_eq!(f.auth.runs().cancelled_by_stop(), 1);
    assert!(resumed.is_err(), "{resumed:?}");
    assert_eq!(
        records_of(f.sink.memory.records(), EvidencePhase::Resumed).len(),
        1
    );
    assert_eq!(resumed, Err(AuthorityError::ResumeSuperseded));
    let resumed_records = records_of(f.sink.memory.records(), EvidencePhase::Resumed);
    assert_eq!(detail(&resumed_records[0], "stop_number"), Some("1"));
    assert_eq!(stops_of(f.sink.memory.records()), ["1/none", "2/no"]);
}

/// RS-01 T3: the desktop quits while the Resumed record is written: the
/// resume comes too late and lifts nothing.
#[test]
fn rs01_t3_quitting_while_the_resumed_record_is_written_lifts_nothing() {
    let f = scripted();
    assert_eq!(f.auth.emergency_stop(), 1);
    f.sink.hold_next(EvidencePhase::Resumed);
    let resuming = {
        let auth = f.auth.clone();
        std::thread::spawn(move || auth.resume(&Confirmer::new(true)))
    };
    f.sink.wait_held();
    let auth = f.auth.clone();
    within(move || auth.runs().close());
    f.sink.release();
    let resumed = resuming.join().unwrap();
    assert!(
        f.auth.runs().is_stopped(),
        "a resume lifted the stop after quitting began: {resumed:?}"
    );
    assert_eq!(f.auth.runs().cancelled_by_stop(), 1);
    assert!(resumed.is_err(), "{resumed:?}");
    assert_eq!(resumed, Err(QUITTING));
}

/// RS-01 T3b: the desktop quits while the resume dialog is open: the resume
/// is refused and writes no record.
#[test]
fn rs01_t3b_quitting_during_the_resume_dialog_writes_no_record() {
    let f = fixture();
    assert_eq!(f.auth.emergency_stop(), 1);
    let auth = f.auth.clone();
    let dialog = ResumeDialog::during(true, move || auth.runs().close());
    let resumed = f.auth.resume(&dialog);
    assert!(
        resumed.is_err(),
        "a resume went on after quitting began: {resumed:?}"
    );
    assert!(records_of(f.evidence.records(), EvidencePhase::Resumed).is_empty());
    assert!(f.auth.runs().is_stopped());
    assert_eq!(f.auth.runs().cancelled_by_stop(), 1);
    assert_eq!(resumed, Err(QUITTING));
}

/// RS-01 T4: once the desktop is quitting, a resume asks nobody.
#[test]
fn rs01_t4_a_resume_after_quitting_asks_nobody() {
    let f = fixture();
    assert_eq!(f.auth.emergency_stop(), 1);
    f.auth.runs().close();
    let dialog = ResumeDialog::answering(true);
    let resumed = f.auth.resume(&dialog);
    assert_eq!(
        dialog.asked(),
        0,
        "the owner was asked after quitting began: {resumed:?}"
    );
    assert!(resumed.is_err(), "{resumed:?}");
    assert!(f.auth.runs().is_stopped());
    assert_eq!(resumed, Err(QUITTING));
}

/// RS-01 T4b: once the desktop is quitting, a resume is refused also when
/// nothing is stopped.
#[test]
fn rs01_t4b_a_resume_after_quitting_is_refused_also_when_not_stopped() {
    let f = fixture();
    f.auth.runs().close();
    let dialog = ResumeDialog::answering(true);
    let resumed = f.auth.resume(&dialog);
    assert!(
        resumed.is_err(),
        "a resume succeeded after quitting began: {resumed:?}"
    );
    assert_eq!(dialog.asked(), 0);
    assert_eq!(resumed, Err(QUITTING));
}

/// RS-01 T5: at its maximum the stop number does not wrap: the stop sets
/// `exhausted` and keeps the number, the registry stays stopped, and no
/// resume asks the owner.
#[test]
fn rs01_t5_an_exhausted_stop_number_never_wraps_and_stays_stopped() {
    let f = fixture();
    let max = u64::MAX;
    f.auth.runs().set_lifted_stop(max);
    assert_eq!(f.auth.emergency_stop(), 1);
    assert!(f.auth.runs().is_stopped());
    assert_eq!(
        stops_of(f.evidence.records()),
        [format!("{max}/yes/exhausted yes")]
    );
    let dialog = ResumeDialog::answering(true);
    assert_eq!(
        f.auth.resume(&dialog),
        Err(AuthorityError::EmergencyStopped)
    );
    assert_eq!(dialog.asked(), 0);
    assert!(f.auth.runs().is_stopped());
    assert_eq!(f.auth.runs().cancelled_by_stop(), 1);
    // A further stop: the same number, still exhausted, and the stop before
    // it not lifted.
    f.auth.emergency_stop();
    assert_eq!(
        stops_of(f.evidence.records()),
        [
            format!("{max}/yes/exhausted yes"),
            format!("{max}/no/exhausted yes")
        ]
    );
    assert!(f.auth.runs().is_stopped());
}

/// RS-01 T5b: a stop that exhausts the stop number while the resume dialog
/// is open: the resume is refused before any record.
#[test]
fn rs01_t5b_exhaustion_during_the_dialog_refuses_the_resume() {
    let f = fixture();
    f.auth.runs().set_lifted_stop(u64::MAX - 1);
    assert_eq!(f.auth.emergency_stop(), 1);
    let auth = f.auth.clone();
    let dialog = ResumeDialog::during(true, move || {
        auth.emergency_stop();
    });
    assert_eq!(
        f.auth.resume(&dialog),
        Err(AuthorityError::EmergencyStopped)
    );
    assert_eq!(dialog.shown()[0].stop_number, u64::MAX);
    assert!(records_of(f.evidence.records(), EvidencePhase::Resumed).is_empty());
    assert!(f.auth.runs().is_stopped());
    assert_eq!(f.auth.runs().cancelled_by_stop(), 1);
}

/// RS-01 T5c: the same, the stop that exhausts the number coming while the
/// Resumed record is written: refused, still stopped, the count kept.
#[test]
fn rs01_t5c_exhaustion_while_the_record_is_written_refuses_the_resume() {
    let f = scripted();
    f.auth.runs().set_lifted_stop(u64::MAX - 1);
    assert_eq!(f.auth.emergency_stop(), 1);
    f.sink.hold_next(EvidencePhase::Resumed);
    let resuming = {
        let auth = f.auth.clone();
        std::thread::spawn(move || auth.resume(&Confirmer::new(true)))
    };
    f.sink.wait_held();
    let auth = f.auth.clone();
    within(move || auth.emergency_stop());
    f.sink.release();
    assert_eq!(
        resuming.join().unwrap(),
        Err(AuthorityError::EmergencyStopped)
    );
    assert!(f.auth.runs().is_stopped());
    assert_eq!(f.auth.runs().cancelled_by_stop(), 1);
}

/// RS-01 T6: a resume confirmed for an older stop lifts nothing; one
/// confirmed for the newest lifts it and zeroes the count; each stop's
/// record says whether the stop before it was lifted.
#[test]
fn rs01_t6_each_stop_says_whether_the_one_before_it_was_lifted() {
    let f = fixture();
    assert_eq!(f.auth.emergency_stop(), 1);
    // Resume A reads stop 1 and waits in its dialog; stop 2 comes meanwhile.
    let auth = f.auth.clone();
    let a = ResumeDialog::during(true, move || {
        auth.emergency_stop();
    });
    let resumed_a = f.auth.resume(&a);
    assert!(
        f.auth.runs().is_stopped(),
        "resume A lifted stop 2: {resumed_a:?}"
    );
    assert!(resumed_a.is_err(), "{resumed_a:?}");
    assert!(records_of(f.evidence.records(), EvidencePhase::Resumed).is_empty());
    assert_eq!(f.auth.runs().cancelled_by_stop(), 1);
    assert_eq!(resumed_a, Err(AuthorityError::ResumeSuperseded));
    assert_eq!(a.shown()[0].stop_number, 1);
    // Resume B reads stop 2 and confirms: lifted, and the count is zero.
    let b = ResumeDialog::answering(true);
    f.auth.resume(&b).unwrap();
    assert!(!f.auth.runs().is_stopped());
    assert_eq!(f.auth.runs().cancelled_by_stop(), 0);
    assert_eq!(
        records_of(f.evidence.records(), EvidencePhase::Resumed).len(),
        1
    );
    assert_eq!(b.shown()[0].stop_number, 2);
    let resumed_records = records_of(f.evidence.records(), EvidencePhase::Resumed);
    assert_eq!(detail(&resumed_records[0], "stop_number"), Some("2"));
    f.auth
        .open_run(f.agent.clone(), RunOrigin::AgentGoal)
        .unwrap();
    // Stop 3.
    assert_eq!(f.auth.emergency_stop(), 1);
    assert_eq!(stops_of(f.evidence.records()), ["1/none", "2/no", "3/yes"]);
}

/// RS-01 T7: two resumes confirm the same stop. The one that comes second
/// is told the truth, that the stop it confirmed is lifted, and changes
/// nothing.
#[test]
fn rs01_t7_a_second_resume_of_the_same_stop_is_told_the_truth() {
    // (a) B's Resumed record is held; A runs through and lifts stop 1.
    let f = scripted();
    assert_eq!(f.auth.emergency_stop(), 1);
    f.sink.hold_next(EvidencePhase::Resumed);
    let b = {
        let auth = f.auth.clone();
        std::thread::spawn(move || auth.resume(&Confirmer::new(true)))
    };
    f.sink.wait_held();
    let auth = f.auth.clone();
    within(move || auth.resume(&Confirmer::new(true))).unwrap();
    f.sink.release();
    assert_eq!(b.join().unwrap(), Ok(()));
    assert!(!f.auth.runs().is_stopped());
    f.auth
        .open_run(f.agent.clone(), RunOrigin::AgentGoal)
        .unwrap();
    // Two records of one stop: neither is proof of the lift by itself.
    assert_eq!(
        records_of(f.sink.memory.records(), EvidencePhase::Resumed).len(),
        2
    );

    // (b) On a registry directly: the same compare-and-clear twice.
    let registry = RunRegistry::default();
    assert_eq!(registry.stop_all().number, 1);
    assert_eq!(registry.lift(1), Ok(Lift::Lifted));
    assert!(!registry.is_stopped());
    assert_eq!(registry.lift(1), Ok(Lift::AlreadyLifted));
    assert!(!registry.is_stopped());
    let next = registry.stop_all();
    assert_eq!((next.number, next.previous), (2, PreviousStop::Lifted));

    // (c) From inside B's dialog, A runs through and lifts stop 1; then B
    // confirms: success, and B writes no record of its own.
    let f = fixture();
    assert_eq!(f.auth.emergency_stop(), 1);
    let auth = f.auth.clone();
    let b = ResumeDialog::during(true, move || {
        auth.resume(&Confirmer::new(true)).unwrap();
    });
    assert_eq!(f.auth.resume(&b), Ok(()));
    assert_eq!(b.shown()[0].stop_number, 1);
    assert_eq!(
        records_of(f.evidence.records(), EvidencePhase::Resumed).len(),
        1
    );
    assert!(!f.auth.runs().is_stopped());
}

/// The acquisition T8 looks for.
const ACQUIRE: &str = "self.runs.lock()";

/// `source` without its comments; string and char literals are kept whole.
fn without_comments(source: &str) -> String {
    let b = source.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i..].starts_with(b"//") {
            while i < b.len() && b[i] != b'\n' {
                i += 1;
            }
        } else if b[i..].starts_with(b"/*") {
            let mut depth = 0usize;
            while i < b.len() {
                if b[i..].starts_with(b"/*") {
                    depth += 1;
                    i += 2;
                } else if b[i..].starts_with(b"*/") {
                    depth -= 1;
                    i += 2;
                    if depth == 0 {
                        break;
                    }
                } else {
                    i += 1;
                }
            }
            out.push(b' ');
        } else {
            let end = literal_end(b, i).unwrap_or(i + 1);
            out.extend_from_slice(&b[i..end]);
            i = end;
        }
    }
    String::from_utf8(out).expect("cuts fall on character boundaries")
}

/// Where the string or char literal that starts at `i` ends, if one does (a
/// lifetime is not a literal).
fn literal_end(b: &[u8], i: usize) -> Option<usize> {
    match b[i] {
        b'"' => {
            let mut j = i + 1;
            while j < b.len() {
                match b[j] {
                    b'\\' => j += 2,
                    b'"' => return Some(j + 1),
                    _ => j += 1,
                }
            }
            Some(b.len())
        }
        b'\'' if b.get(i + 1) == Some(&b'\\') => b
            .get(i + 3..)?
            .iter()
            .position(|c| *c == b'\'')
            .map(|at| i + 3 + at + 1),
        b'\'' => {
            let width = match *b.get(i + 1)? {
                c if c >= 0xF0 => 4,
                c if c >= 0xE0 => 3,
                c if c >= 0xC0 => 2,
                _ => 1,
            };
            (b.get(i + 1 + width) == Some(&b'\'')).then_some(i + 2 + width)
        }
        _ => None,
    }
}

/// The text between the braces of the body of `fn name`.
fn function_body(source: &str, name: &str) -> Option<String> {
    let b = source.as_bytes();
    let mut i = source.find(&format!("fn {name}("))?;
    let mut parens = 0usize;
    loop {
        match *b.get(i)? {
            b'(' => parens += 1,
            b')' => parens = parens.checked_sub(1)?,
            b'{' if parens == 0 => break,
            _ => {}
        }
        i += 1;
    }
    let open = i;
    let mut depth = 0usize;
    while i < b.len() {
        if let Some(end) = literal_end(b, i) {
            i = end;
            continue;
        }
        match b[i] {
            b'{' => depth += 1,
            b'}' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(source[open + 1..i].to_string());
                }
            }
            _ => {}
        }
        i += 1;
    }
    None
}

/// A character of a name or a keyword.
fn word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// `body` without its whitespace, except one space where whitespace
/// separates two word characters (`if self`, `let mut runs`): a keyword
/// stays a word of its own and is never read as the start of a name.
fn squeezed(body: &str) -> String {
    let mut out = String::with_capacity(body.len());
    let mut space = false;
    for c in body.chars() {
        if c.is_whitespace() {
            space = true;
            continue;
        }
        if space && word(c) && out.chars().next_back().is_some_and(word) {
            out.push(' ');
        }
        space = false;
        out.push(c);
    }
    out
}

/// Where `path`, which starts with `self.`, occurs in `text` with `self` a
/// whole word: whatever comes before it, but not the end of a longer name
/// (`myself.`).
fn on_self<'a>(text: &'a str, path: &'a str) -> impl Iterator<Item = usize> + 'a {
    text.match_indices(path)
        .map(|(at, _)| at)
        .filter(|at| !text[..*at].chars().next_back().is_some_and(word))
}

/// T8 (2): the acquisition is in a `let` statement that is a direct
/// statement of the body, and that statement binds the guard itself to a
/// name (not a temporary, not inside an inner block or expression, not by
/// `ref`). `text` is squeezed.
fn bound_by_a_direct_let(text: &str) -> Result<(), String> {
    let at = text.find(ACQUIRE).ok_or("no acquisition")?;
    let b = text.as_bytes();
    let (mut depth, mut statement, mut i) = (0usize, 0usize, 0usize);
    while i < at {
        if let Some(end) = literal_end(b, i) {
            i = end;
            continue;
        }
        match b[i] {
            b'{' | b'(' | b'[' => depth += 1,
            b'}' | b')' | b']' => {
                depth = depth.saturating_sub(1);
                if depth == 0 && b[i] == b'}' {
                    statement = i + 1;
                }
            }
            b';' if depth == 0 => statement = i + 1,
            _ => {}
        }
        i += 1;
    }
    if depth != 0 {
        return Err("the acquisition is inside an inner block or expression".into());
    }
    let mut end = at;
    while end < b.len() {
        if let Some(next) = literal_end(b, end) {
            end = next;
            continue;
        }
        match b[end] {
            b'{' | b'(' | b'[' => depth += 1,
            b'}' | b')' | b']' => depth = depth.saturating_sub(1),
            b';' if depth == 0 => break,
            _ => {}
        }
        end += 1;
    }
    let statement = &text[statement..(end + 1).min(text.len())];
    let named = |binding: &str| {
        let name = binding.strip_prefix("mut ").unwrap_or(binding);
        name != "_" && !name.is_empty() && name.chars().all(word)
    };
    for suffix in [
        "=self.runs.lock().unwrap_or_else(PoisonError::into_inner);",
        "=self.runs.lock().expect(\"runs\");",
    ] {
        if let Some(name) = statement
            .strip_prefix("let ")
            .and_then(|rest| rest.strip_suffix(suffix))
        {
            if named(name) {
                return Ok(());
            }
        }
    }
    if let Some((name, _)) = statement
        .strip_prefix("let Ok(")
        .and_then(|rest| rest.split_once(")=self.runs.lock()else{"))
    {
        if named(name) && statement.ends_with("};") {
            return Ok(());
        }
    }
    Err(format!(
        "the acquisition is not bound to a name by a direct let: {statement}"
    ))
}

/// Every `self.name(` in `text` (squeezed): a method of `self` called,
/// whatever comes before `self` (a keyword, an operator, a bracket), also
/// as `self.name::<..>(` and with a raw name `self.r#name(`. A field
/// followed by a method (`self.runs.lock()`) is not a call on `self`.
fn calls_on_self(text: &str) -> Vec<String> {
    let mut calls = Vec::new();
    for at in on_self(text, "self.") {
        let rest = &text[at + "self.".len()..];
        let raw = if rest.starts_with("r#") { "r#" } else { "" };
        let name: String = rest[raw.len()..].chars().take_while(|c| word(*c)).collect();
        let after = &rest[raw.len() + name.len()..];
        if !name.is_empty() && (after.starts_with('(') || after.starts_with("::<")) {
            calls.push(format!("self.{raw}{name}("));
        }
    }
    calls
}

/// Which of T8's criteria (1) to (5) a function body fails.
fn one_hold(body: &str, ordered: bool) -> Vec<String> {
    let text = squeezed(body);
    let mut failures = Vec::new();
    let acquisitions = text.matches(ACQUIRE).count();
    if acquisitions != 1 {
        failures.push(format!("(1) {ACQUIRE} occurs {acquisitions} times"));
    }
    if let Err(why) = bound_by_a_direct_let(&text) {
        failures.push(format!("(2) {why}"));
    }
    if text.contains("drop(") {
        failures.push("(3) the body contains drop(".to_string());
    }
    let calls = calls_on_self(&text);
    if !calls.is_empty() {
        failures.push(format!("(4) the body calls a method on self: {calls:?}"));
    }
    if ordered {
        let found = (
            text.find(ACQUIRE),
            on_self(&text, "self.closed.load(").next(),
            on_self(&text, "self.stopped.store(false").next(),
        );
        if !matches!(found, (Some(lock), Some(closed), Some(clear)) if lock < closed && closed < clear)
        {
            failures.push(format!(
                "(5) not in the order acquisition, read of closed, clear: {found:?}"
            ));
        }
    }
    failures
}

/// The stop before RS-01, `stop_all` as it is at commit 862a11ce: the flag
/// set before the lock, the ids read under a temporary guard, and each run
/// cancelled through `self.cancel`, which takes the lock again. T8's checker
/// must fail it on (2) and (4), and on nothing else.
const STOP_BEFORE_RS01: &str = r#"
    pub(crate) fn stop_all(&self) -> usize {
        self.stopped.store(true, Ordering::SeqCst);
        let ids: Vec<RunId> = self.runs.lock().expect("runs").keys().copied().collect();
        let cancelled = ids.into_iter().filter(|id| self.cancel(*id)).count();
        self.cancelled_by_stop
            .fetch_add(cancelled, Ordering::SeqCst);
        cancelled
    }
"#;

/// RS-01 T8, a tripwire on the source text of `run.rs` (comments removed):
/// the stop and the compare-and-clear each take the runs lock once, bind
/// its guard by a direct `let`, drop nothing and call no method on `self`;
/// the compare-and-clear reads `closed` under that lock before it clears
/// the stop; and nothing clears the lock's poison. A review of the code is
/// the proof (§8); this only catches a change of its shape.
#[test]
fn rs01_t8_the_stop_and_the_compare_and_clear_each_hold_the_runs_lock_once() {
    let before = function_body(&without_comments(STOP_BEFORE_RS01), "stop_all")
        .expect("the stop before RS-01 has a body");
    let reported = one_hold(&before, false);
    let criteria: Vec<&str> = reported.iter().map(|failure| &failure[..3]).collect();
    assert_eq!(
        criteria,
        ["(2)", "(4)"],
        "the stop before RS-01: {reported:#?}"
    );
    let source = without_comments(include_str!("run.rs"));
    let mut failures = Vec::new();
    for (function, ordered) in [("stop_all", false), ("lift", true)] {
        let Some(body) = function_body(&source, function) else {
            failures.push(format!("{function}: not found"));
            continue;
        };
        for failure in one_hold(&body, ordered) {
            failures.push(format!("{function}: {failure}"));
        }
    }
    if source.contains("clear_poison") {
        failures.push("run.rs: (6) the text clear_poison occurs".to_string());
    }
    assert!(failures.is_empty(), "{failures:#?}");
}

const POISONED: AuthorityError = AuthorityError::Unavailable("the run registry is poisoned");

/// Poison `auth`'s runs lock on purpose: the registry's test-only fixture
/// takes it on another thread and panics there; the panic is taken here.
fn poison(auth: &Authority) {
    let runs = auth.runs().clone();
    let poisoner = std::thread::spawn(move || runs.panic_holding_the_runs_lock());
    assert!(poisoner.join().is_err(), "the fixture did not panic");
}

/// Two runs beside the fixture's, and the cancel tokens of all three.
fn three_runs(f: &Scripted) -> Vec<CancelToken> {
    let mut runs = vec![f.run];
    for _ in 0..2 {
        runs.push(
            f.auth
                .open_run(f.agent.clone(), RunOrigin::AgentGoal)
                .unwrap(),
        );
    }
    runs.into_iter()
        .map(|run| f.auth.runs().check(run, &f.agent).unwrap())
        .collect()
}

/// How T11's registry stands just before its lock is poisoned (part (d):
/// also closed, or also exhausted).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BeforePoison {
    Plain,
    Closed,
    Exhausted,
}

/// T11, after a resume refused on the poisoned lock: the flag and the
/// count, read without the lock, as they were; the lock still poisoned, so
/// a second resume asks nobody; and, unless the number is exhausted, a
/// further stop records `next` (its number, and the stop before it not
/// lifted).
fn untouched(f: &Scripted, stopped: bool, count: usize, next: Option<&str>) {
    assert_eq!(f.auth.runs().is_stopped(), stopped);
    assert_eq!(f.auth.runs().cancelled_by_stop(), count);
    let again = ResumeDialog::answering(true);
    assert_eq!(f.auth.resume(&again), Err(POISONED));
    assert_eq!(again.asked(), 0, "the lock's poison was cleared");
    if let Some(next) = next {
        f.auth.emergency_stop();
        let stops = stops_of(f.sink.memory.records());
        assert_eq!(stops.last().map(String::as_str), Some(next), "{stops:?}");
    }
}

/// RS-01 T11: a poisoned runs lock. The stop still stops and cancels every
/// run; a resume lifts nothing through it at any of its three lock sites,
/// and poison is the first thing each tests. After the poison, the test
/// calls nothing but the stop, the resume and the lock-free reads.
#[test]
fn rs01_t11_a_poisoned_runs_lock_lifts_nothing() {
    use BeforePoison::{Closed, Exhausted, Plain};
    for before in [Plain, Closed, Exhausted] {
        let next = (before != Exhausted).then_some("2/no");

        // (a) The lock poisoned, then a stop: it stops and cancels every
        // run. A resume is refused at its read and asks nobody.
        let f = scripted();
        let tokens = three_runs(&f);
        match before {
            Plain => {}
            Closed => f.auth.runs().close(),
            Exhausted => {
                f.auth.runs().set_lifted_stop(u64::MAX);
                f.auth.emergency_stop();
            }
        }
        poison(&f.auth);
        let cancelled = f.auth.emergency_stop();
        assert_eq!(cancelled, if before == Exhausted { 0 } else { 3 });
        assert!(f.auth.runs().is_stopped());
        assert!(tokens.iter().all(CancelToken::is_cancelled));
        let dialog = ResumeDialog::answering(true);
        assert_eq!(f.auth.resume(&dialog), Err(POISONED), "(a) {before:?}");
        assert_eq!(dialog.asked(), 0);
        untouched(&f, true, 3, next);

        // (b) A stop; the dialog poisons the lock, then the owner confirms:
        // refused at the early check, so no record.
        let f = scripted();
        let _tokens = three_runs(&f);
        if before == Exhausted {
            f.auth.runs().set_lifted_stop(u64::MAX - 1);
        }
        assert_eq!(f.auth.emergency_stop(), 3);
        let auth = f.auth.clone();
        let dialog = ResumeDialog::during(true, move || {
            match before {
                Plain => {}
                Closed => auth.runs().close(),
                Exhausted => {
                    auth.emergency_stop();
                }
            }
            poison(&auth);
        });
        assert_eq!(f.auth.resume(&dialog), Err(POISONED), "(b) {before:?}");
        assert_eq!(dialog.asked(), 1);
        assert!(records_of(f.sink.memory.records(), EvidencePhase::Resumed).is_empty());
        untouched(&f, true, 3, next);

        // (c) A stop; the Resumed record held; the lock poisoned: refused at
        // the compare-and-clear.
        let f = scripted();
        let _tokens = three_runs(&f);
        if before == Exhausted {
            f.auth.runs().set_lifted_stop(u64::MAX - 1);
        }
        assert_eq!(f.auth.emergency_stop(), 3);
        f.sink.hold_next(EvidencePhase::Resumed);
        let resuming = {
            let auth = f.auth.clone();
            std::thread::spawn(move || auth.resume(&Confirmer::new(true)))
        };
        f.sink.wait_held();
        match before {
            Plain => {}
            Closed => f.auth.runs().close(),
            Exhausted => {
                f.auth.emergency_stop();
            }
        }
        poison(&f.auth);
        f.sink.release();
        assert_eq!(resuming.join().unwrap(), Err(POISONED), "(c) {before:?}");
        assert_eq!(
            records_of(f.sink.memory.records(), EvidencePhase::Resumed).len(),
            1
        );
        untouched(&f, true, 3, next);
    }

    // (e) No stop; the lock poisoned: a resume is refused, not a success.
    let f = scripted();
    let _tokens = three_runs(&f);
    poison(&f.auth);
    let dialog = ResumeDialog::answering(true);
    assert_eq!(f.auth.resume(&dialog), Err(POISONED));
    assert_eq!(dialog.asked(), 0);
    untouched(&f, false, 0, Some("1/none"));
}

// ---- RS-01 T10 BEGIN ----
/// RS-01 T10 and T10a (P3-C11-RS01-CONTRACT-05 §8). T10 is a diagnostic
/// measurement of the run registry's stop step alone, seen from outside its
/// lock: every number is a span between two clock reads made outside the
/// lock. It does not measure the stop's wait for the lock, nor its sweep,
/// and it is not a worst-case bound. It is ignored: it runs only by its own
/// command, optimised, after T10a, and writes its receipt to the file
/// `NEXUS_T10_RECEIPT` names, outside the repository. T10a tests the three
/// functions T10 uses to turn what its controller received into numbers,
/// statistics and receipt text. This block is hashed between its markers.
mod stop_measurement {
    use crate::authority::ids::{AgentId, RunId};
    use crate::authority::run::{CancelToken, RunClass, RunOrigin, RunRegistry};
    use crate::authority::AuthorityError;
    use sha2::{Digest as _, Sha256};
    use std::io::Write as _;
    use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant};

    /// Nanoseconds since the run's origin, read from one monotonic clock.
    type Nanos = u64;

    /// Active runs in each sample's new registry.
    const RUNS: usize = 4096;
    /// Samples per workload, all kept.
    const SAMPLES: usize = 1000;
    /// Records each competing thread's log holds.
    const LOG_ENTRIES: usize = 65_536;
    /// Calls a competing thread completes before it is ready.
    const READY_CALLS: u64 = 100;
    /// The controller's three bounded waits.
    const READINESS_WAIT: Nanos = 5_000_000_000;
    const COMPLETION_WAIT: Nanos = 5_000_000_000;
    const COLLECTION_WAIT: Duration = Duration::from_secs(1);
    /// How often the controller looks at the signals and the slots; it
    /// sleeps in between so that it competes for the processors as little
    /// as it can.
    const POLL: Duration = Duration::from_micros(200);
    /// Host conditions are recorded at the start, after every this many
    /// samples, and at the end.
    const HOST_EVERY: usize = 100;
    /// The provisional targets (§8). They are applied by the reviewer and
    /// never asserted here; the receipt only lists the samples above them.
    const IDLE_TOTAL_TARGET: Nanos = 1_000_000;
    const BUSY_TOTAL_P99_TARGET: Nanos = 10_000_000;
    /// A progress marker no call has set yet.
    const NO_PROGRESS: Nanos = Nanos::MAX;
    /// Local-model runtimes, matched against each process's name and the
    /// file name of its executable (a loaded Ollama model runs as its own
    /// `ollama runner` process).
    const MODEL_RUNTIMES: &[&str] = &[
        "ollama",
        "llama",
        "vllm",
        "lmstudio",
        "lm-studio",
        "koboldcpp",
        "localai",
        "local-ai",
        "text-generation",
        "mlc_llm",
        "gpt4all",
        "sglang",
        "exllama",
        "tabby",
        "whisper",
    ];

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Workload {
        Idle,
        Busy,
    }

    impl Workload {
        fn as_str(self) -> &'static str {
            match self {
                Workload::Idle => "idle",
                Workload::Busy => "busy",
            }
        }
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Method {
        Open,
        Check,
    }

    impl Method {
        fn as_str(self) -> &'static str {
            match self {
                Method::Open => "open",
                Method::Check => "check",
            }
        }
    }

    /// The competing threads of the busy workload.
    const CALLERS: [(&str, Method); 4] = [
        ("open-1", Method::Open),
        ("open-2", Method::Open),
        ("check-1", Method::Check),
        ("check-2", Method::Check),
    ];

    /// One logged competing call: its method, its thread's clock reads
    /// before and after it, and its result.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    struct Call {
        method: Method,
        start: Nanos,
        end: Nanos,
        result: &'static str,
    }

    impl Call {
        const UNUSED: Call = Call {
            method: Method::Open,
            start: 0,
            end: 0,
            result: "",
        };
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    struct StopReport {
        start: Nanos,
        ret: Nanos,
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    struct ObserverReport {
        seen: Option<Nanos>,
    }

    /// A competing thread's final report: its log comes back inside it.
    #[derive(Clone, Debug, PartialEq, Eq)]
    struct CallerReport {
        calls: u64,
        log: Vec<Call>,
        kept: usize,
        overflow: bool,
    }

    impl CallerReport {
        fn kept(&self) -> &[Call] {
            &self.log[..self.kept.min(self.log.len())]
        }
    }

    /// The controller's own clock readings for one sample.
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    struct Readings {
        threads_started: Nanos,
        released: Option<Nanos>,
        asked_to_end: Option<Nanos>,
        gave_up: Option<Nanos>,
    }

    /// One competing thread as the controller knows it: its final report if
    /// that arrived by the deadline, else the last start it published.
    #[derive(Clone, Debug, PartialEq, Eq)]
    struct Caller {
        name: &'static str,
        report: Option<CallerReport>,
        progress: Option<Nanos>,
    }

    /// A report found only in the controller's last look, after it gave up.
    #[derive(Clone, Debug, PartialEq, Eq)]
    enum Late {
        Stop(StopReport),
        Observer(ObserverReport),
        Caller(&'static str, CallerReport),
    }

    /// What the controller received for one sample by its deadlines.
    #[derive(Clone, Debug, PartialEq, Eq)]
    struct Received {
        index: usize,
        workload: Workload,
        readings: Readings,
        not_ready: Vec<&'static str>,
        stop: Option<StopReport>,
        observer: Option<ObserverReport>,
        callers: Vec<Caller>,
        late: Vec<Late>,
    }

    /// How a sample ended: what the controller received, never what a
    /// thread was doing.
    #[derive(Clone, Debug, PartialEq, Eq)]
    enum Outcome {
        Complete,
        ReadinessNotReported(Vec<&'static str>),
        StopReturnNotObserved,
        ParticipantReportNotRequested(Vec<&'static str>),
        ParticipantReportMissing {
            participant: &'static str,
            last_start: Option<Nanos>,
        },
        ObserverResultMissing,
        HarnessFault(&'static str),
        LogOverflow(&'static str),
    }

    /// A span between two clock reads that both reached the controller, the
    /// later not before the earlier; otherwise unavailable, never zero.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Value {
        Of(Nanos),
        Unavailable,
    }

    impl Value {
        fn span(earlier: Option<Nanos>, later: Option<Nanos>) -> Value {
            match (earlier, later) {
                (Some(earlier), Some(later)) => later
                    .checked_sub(earlier)
                    .map_or(Value::Unavailable, Value::Of),
                _ => Value::Unavailable,
            }
        }

        fn get(self) -> Option<Nanos> {
            match self {
                Value::Of(value) => Some(value),
                Value::Unavailable => None,
            }
        }
    }

    /// A competing call whose timestamps overlap the stop's.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    struct Overlap {
        caller: &'static str,
        record: usize,
        latency: Value,
    }

    /// One sample's numbers and results: T10a's first function's output.
    #[derive(Clone, Debug, PartialEq, Eq)]
    struct Sample {
        index: usize,
        workload: Workload,
        results: Vec<Outcome>,
        readings: Readings,
        waiting_time: Option<Nanos>,
        observed_flag_latency: Value,
        total: Value,
        flag_latency_bound: Value,
        observation_to_return: Value,
        late_observer: bool,
        overlaps: Vec<Overlap>,
        largest_overlap: Value,
    }

    impl Sample {
        fn complete(&self) -> bool {
            self.results == [Outcome::Complete]
        }
    }

    /// The controller's waiting time, from one of its own readings to its
    /// reading `gave up`.
    fn waited(gave_up: Option<Nanos>, from: Option<Nanos>) -> Option<Nanos> {
        gave_up?.checked_sub(from?)
    }

    /// T10a's first function: what the controller received for one sample,
    /// turned into that sample's numbers and results. A number exists only
    /// if every timestamp it is computed from reached the controller by the
    /// deadline; a late report changes nothing here.
    fn numbers(r: &Received) -> Sample {
        let mut sample = Sample {
            index: r.index,
            workload: r.workload,
            results: Vec::new(),
            readings: r.readings,
            waiting_time: None,
            observed_flag_latency: Value::Unavailable,
            total: Value::Unavailable,
            flag_latency_bound: Value::Unavailable,
            observation_to_return: Value::Unavailable,
            late_observer: false,
            overlaps: Vec::new(),
            largest_overlap: Value::Unavailable,
        };
        if !r.not_ready.is_empty() {
            // No stop was issued: this is not a sample of the stop.
            sample
                .results
                .push(Outcome::ReadinessNotReported(r.not_ready.clone()));
            sample.waiting_time = waited(r.readings.gave_up, Some(r.readings.threads_started));
            return sample;
        }
        let start = r.stop.map(|stop| stop.start);
        let ret = r.stop.map(|stop| stop.ret);
        let seen = r.observer.and_then(|observer| observer.seen);
        if r.stop.is_none() {
            sample.results.push(Outcome::StopReturnNotObserved);
            if !r.callers.is_empty() {
                sample.results.push(Outcome::ParticipantReportNotRequested(
                    r.callers.iter().map(|caller| caller.name).collect(),
                ));
            }
        } else {
            for caller in r.callers.iter().filter(|caller| caller.report.is_none()) {
                sample.results.push(Outcome::ParticipantReportMissing {
                    participant: caller.name,
                    last_start: caller.progress,
                });
            }
        }
        if seen.is_none() {
            sample.results.push(Outcome::ObserverResultMissing);
        }
        if matches!((start, ret), (Some(start), Some(ret)) if ret < start) {
            sample
                .results
                .push(Outcome::HarnessFault("return before start"));
        }
        if matches!((start, seen), (Some(start), Some(seen)) if seen < start) {
            sample
                .results
                .push(Outcome::HarnessFault("seen before start"));
        }
        for caller in &r.callers {
            if caller.report.as_ref().is_some_and(|report| report.overflow) {
                sample.results.push(Outcome::LogOverflow(caller.name));
            }
        }
        if let (Some(start), Some(ret)) = (start, ret) {
            let mut reversed = false;
            for caller in &r.callers {
                let Some(report) = &caller.report else {
                    continue;
                };
                for (record, call) in report.kept().iter().enumerate() {
                    // Closed intervals: touching at one instant is overlap.
                    if call.end >= start && call.start <= ret {
                        let latency = Value::span(Some(call.start), Some(call.end));
                        reversed |= latency == Value::Unavailable;
                        sample.overlaps.push(Overlap {
                            caller: caller.name,
                            record,
                            latency,
                        });
                    }
                }
            }
            if reversed {
                sample
                    .results
                    .push(Outcome::HarnessFault("a call's end before its start"));
            }
        }
        sample.largest_overlap = sample
            .overlaps
            .iter()
            .filter_map(|overlap| overlap.latency.get())
            .max()
            .map_or(Value::Unavailable, Value::Of);
        sample.observed_flag_latency = Value::span(start, seen);
        sample.total = Value::span(start, ret);
        sample.flag_latency_bound = match (sample.observed_flag_latency, sample.total) {
            (Value::Of(observed), Value::Of(total)) => Value::Of(observed.min(total)),
            _ => Value::Unavailable,
        };
        sample.late_observer = matches!((seen, ret), (Some(seen), Some(ret)) if seen > ret);
        if !sample.late_observer {
            sample.observation_to_return = Value::span(seen, ret);
        }
        let missing = sample.results.iter().any(|outcome| {
            matches!(
                outcome,
                Outcome::StopReturnNotObserved
                    | Outcome::ParticipantReportNotRequested(_)
                    | Outcome::ParticipantReportMissing { .. }
                    | Outcome::ObserverResultMissing
            )
        });
        if missing {
            sample.waiting_time = waited(r.readings.gave_up, r.readings.released);
        }
        if sample.results.is_empty() {
            sample.results.push(Outcome::Complete);
        }
        sample
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Group {
        Idle,
        Busy,
        BusyWithOverlap,
        BusyWithoutOverlap,
    }

    const GROUPS: [Group; 4] = [
        Group::Idle,
        Group::Busy,
        Group::BusyWithOverlap,
        Group::BusyWithoutOverlap,
    ];

    impl Group {
        fn as_str(self) -> &'static str {
            match self {
                Group::Idle => "idle",
                Group::Busy => "busy (all)",
                Group::BusyWithOverlap => "busy with overlap",
                Group::BusyWithoutOverlap => "busy without overlap",
            }
        }

        fn holds(self, sample: &Sample) -> bool {
            let busy = sample.workload == Workload::Busy;
            match self {
                Group::Idle => !busy,
                Group::Busy => busy,
                Group::BusyWithOverlap => busy && !sample.overlaps.is_empty(),
                Group::BusyWithoutOverlap => busy && sample.overlaps.is_empty(),
            }
        }
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Number {
        ObservedFlagLatency,
        Total,
        FlagLatencyBound,
        ObservationToReturn,
        OverlappingCallerLatency,
        LargestOverlappingCallerLatency,
    }

    const NUMBERS: [Number; 6] = [
        Number::ObservedFlagLatency,
        Number::Total,
        Number::FlagLatencyBound,
        Number::ObservationToReturn,
        Number::OverlappingCallerLatency,
        Number::LargestOverlappingCallerLatency,
    ];

    impl Number {
        fn as_str(self) -> &'static str {
            match self {
                Number::ObservedFlagLatency => "observed flag latency",
                Number::Total => "total",
                Number::FlagLatencyBound => "flag latency bound",
                Number::ObservationToReturn => "observation to return",
                Number::OverlappingCallerLatency => "overlapping caller latency (per call)",
                Number::LargestOverlappingCallerLatency => {
                    "largest overlapping caller latency (per sample)"
                }
            }
        }

        /// This number's values in one sample (one per overlapping call
        /// for the overlapping caller latency).
        fn values(self, sample: &Sample) -> Vec<Value> {
            match self {
                Number::ObservedFlagLatency => vec![sample.observed_flag_latency],
                Number::Total => vec![sample.total],
                Number::FlagLatencyBound => vec![sample.flag_latency_bound],
                Number::ObservationToReturn => vec![sample.observation_to_return],
                Number::OverlappingCallerLatency => sample
                    .overlaps
                    .iter()
                    .map(|overlap| overlap.latency)
                    .collect(),
                Number::LargestOverlappingCallerLatency => vec![sample.largest_overlap],
            }
        }
    }

    /// One number over one group: `n` available values; the unavailable
    /// ones only counted.
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    struct Summary {
        n: usize,
        unavailable: usize,
        min: Option<Nanos>,
        median: Option<Nanos>,
        p99: Option<Nanos>,
        max: Option<Nanos>,
    }

    /// The `p`-th percentile by nearest rank: the value at position
    /// ⌈p × n ÷ 100⌉ of `sorted`, counting from one (`sorted` not empty).
    fn percentile(sorted: &[Nanos], p: usize) -> Nanos {
        let rank = (p * sorted.len()).div_ceil(100);
        sorted[rank - 1]
    }

    fn summary(values: impl Iterator<Item = Value>) -> Summary {
        let mut summary = Summary::default();
        let mut available = Vec::new();
        for value in values {
            match value {
                Value::Of(value) => available.push(value),
                Value::Unavailable => summary.unavailable += 1,
            }
        }
        available.sort_unstable();
        summary.n = available.len();
        if !available.is_empty() {
            summary.min = available.first().copied();
            summary.median = Some(percentile(&available, 50));
            summary.max = available.last().copied();
            if available.len() >= 100 {
                summary.p99 = Some(percentile(&available, 99));
            }
        }
        summary
    }

    /// Every number over every group.
    struct Statistics(Vec<(Group, Number, Summary)>);

    impl Statistics {
        fn of(&self, group: Group, number: Number) -> Summary {
            self.0
                .iter()
                .find(|(g, n, _)| *g == group && *n == number)
                .map(|(_, _, summary)| *summary)
                .expect("every group has every number")
        }
    }

    /// T10a's second function: the samples combined into statistics, only
    /// if every sample is complete.
    fn statistics(samples: &[Sample]) -> Option<Statistics> {
        if !samples.iter().all(Sample::complete) {
            return None;
        }
        let mut all = Vec::new();
        for group in GROUPS {
            for number in NUMBERS {
                let values = samples
                    .iter()
                    .filter(|sample| group.holds(sample))
                    .flat_map(|sample| number.values(sample));
                all.push((group, number, summary(values)));
            }
        }
        Some(Statistics(all))
    }

    fn ns(at: Option<Nanos>) -> String {
        at.map_or_else(|| "none".to_string(), |at| format!("{at} ns"))
    }

    fn value(value: Value) -> String {
        value
            .get()
            .map_or_else(|| "unavailable".to_string(), |value| format!("{value} ns"))
    }

    fn outcome_name(outcome: &Outcome) -> String {
        match outcome {
            Outcome::Complete => "complete".to_string(),
            Outcome::ReadinessNotReported(names) => {
                format!("readiness not reported ({})", names.join(", "))
            }
            Outcome::StopReturnNotObserved => "stop return not observed".to_string(),
            Outcome::ParticipantReportNotRequested(names) => {
                format!("participant report not requested ({})", names.join(", "))
            }
            Outcome::ParticipantReportMissing { participant, .. } => {
                format!("participant report missing ({participant})")
            }
            Outcome::ObserverResultMissing => "observer result missing".to_string(),
            Outcome::HarnessFault(why) => format!("harness fault ({why})"),
            Outcome::LogOverflow(name) => format!("log overflow ({name})"),
        }
    }

    /// What one result says the controller knows, and what it does not.
    fn explanation(outcome: &Outcome, sample: &Sample) -> Option<String> {
        Some(match outcome {
            Outcome::Complete => return None,
            Outcome::ReadinessNotReported(names) => format!(
                "readiness not reported: {}; no stop was issued; this is not a sample of the stop",
                names.join(", ")
            ),
            Outcome::StopReturnNotObserved => format!(
                "stop return not observed: the stop's report had not arrived by the deadline; \
                 whether the stop returned is not known; start and total are unavailable; \
                 released {} is the controller's own reading, not the stop's start",
                ns(sample.readings.released)
            ),
            Outcome::ParticipantReportNotRequested(names) => format!(
                "participant report not requested: the stop's report did not arrive, so {} \
                 were not asked for their reports before the deadline; nothing is known or \
                 implied about those threads",
                names.join(", ")
            ),
            Outcome::ParticipantReportMissing {
                participant,
                last_start,
            } => format!(
                "participant report missing: {participant}: its final report had not arrived \
                 by the deadline; whether its last call finished is not known; last start it \
                 published (progress marker) {}; asked to end {}; no duration is given for \
                 that call",
                ns(*last_start),
                ns(sample.readings.asked_to_end)
            ),
            Outcome::ObserverResultMissing => "observer result missing: no observer report \
                 carrying seen had arrived by the deadline; whether the flag was set is not \
                 known"
                .to_string(),
            Outcome::HarnessFault(why) => format!("harness fault: {why}"),
            Outcome::LogOverflow(name) => format!("log overflow: the log of {name} filled"),
        })
    }

    fn late_text(late: &Late) -> String {
        match late {
            Late::Stop(stop) => format!("stop: start {} ns; return {} ns", stop.start, stop.ret),
            Late::Observer(observer) => format!("observer: seen {}", ns(observer.seen)),
            Late::Caller(name, report) => {
                let mut text = format!(
                    "{name}: calls made {}; records kept {}; log full: {}",
                    report.calls,
                    report.kept().len(),
                    if report.overflow { "yes" } else { "no" }
                );
                for call in report.kept() {
                    text.push_str(&format!(
                        "\n    logged {} start {} ns end {} ns result {}",
                        call.method.as_str(),
                        call.start,
                        call.end,
                        call.result
                    ));
                }
                text
            }
        }
    }

    /// T10a's third function: one sample's results as receipt text.
    fn sample_text(r: &Received, sample: &Sample) -> String {
        let results: Vec<String> = sample.results.iter().map(outcome_name).collect();
        let mut text = format!(
            "sample {} {}: {}\n",
            sample.index,
            sample.workload.as_str(),
            results.join("; ")
        );
        text.push_str(&format!(
            "  controller: threads started {}; released {}; asked to end {}; gave up {}\n",
            ns(Some(sample.readings.threads_started)),
            ns(sample.readings.released),
            ns(sample.readings.asked_to_end),
            ns(sample.readings.gave_up)
        ));
        text.push_str(&format!(
            "  received: start {}; return {}; seen {}\n",
            ns(r.stop.map(|stop| stop.start)),
            ns(r.stop.map(|stop| stop.ret)),
            ns(r.observer.and_then(|observer| observer.seen))
        ));
        text.push_str(&format!(
            "  numbers: observed flag latency {}; total {}; flag latency bound {}; observation to return {}",
            value(sample.observed_flag_latency),
            value(sample.total),
            value(sample.flag_latency_bound),
            value(sample.observation_to_return)
        ));
        if sample.workload == Workload::Busy {
            text.push_str(&format!(
                "; overlapping calls {}; largest overlapping caller latency {}",
                sample.overlaps.len(),
                value(sample.largest_overlap)
            ));
        }
        text.push('\n');
        if sample.late_observer {
            text.push_str("  note: late observer (seen after return)\n");
        }
        for outcome in &sample.results {
            if let Some(line) = explanation(outcome, sample) {
                text.push_str(&format!("  {line}\n"));
            }
        }
        if let Some(waited) = sample.waiting_time {
            let from = if r.not_ready.is_empty() {
                "gave up minus released"
            } else {
                "gave up minus threads started"
            };
            text.push_str(&format!(
                "  controller waiting time: {waited} ns ({from}). This is the controller's \
                 waiting time. It is not a lower bound on how long any call took.\n"
            ));
        }
        for late in &r.late {
            text.push_str(&format!(
                "  late report (changes no result; enters no statistic): {}\n",
                late_text(late)
            ));
        }
        text
    }

    /// Every logged call of every report that arrived by the deadline.
    fn calls_text(r: &Received) -> String {
        let mut text = String::new();
        for caller in &r.callers {
            let Some(report) = &caller.report else {
                continue;
            };
            text.push_str(&format!(
                "  {}: calls made {}; records kept {}; log full: {}\n",
                caller.name,
                report.calls,
                report.kept().len(),
                if report.overflow { "yes" } else { "no" }
            ));
            for call in report.kept() {
                text.push_str(&format!(
                    "    logged {} start {} ns end {} ns result {}\n",
                    call.method.as_str(),
                    call.start,
                    call.end,
                    call.result
                ));
            }
        }
        text
    }

    fn statistics_text(statistics: &Statistics) -> String {
        let mut text =
            String::from("statistics (nearest rank; the 99th percentile only for n >= 100):\n");
        for group in GROUPS {
            for number in NUMBERS {
                let s = statistics.of(group, number);
                text.push_str(&format!(
                    "  {} / {}: n {}; unavailable {}",
                    group.as_str(),
                    number.as_str(),
                    s.n,
                    s.unavailable
                ));
                if s.n == 0 {
                    text.push_str("; no statistics\n");
                    continue;
                }
                text.push_str(&format!(
                    "; min {}; median {}; p99 {}; max {}\n",
                    ns(s.min),
                    ns(s.median),
                    s.p99
                        .map_or_else(|| "not given (n < 100)".to_string(), |p| format!("{p} ns")),
                    ns(s.max)
                ));
            }
        }
        text
    }

    /// Every sample above a target, and every value above ten times its
    /// number's median in its group (idle, busy with overlap, busy without
    /// overlap), one by one.
    fn outliers_text(samples: &[Sample], statistics: &Statistics) -> String {
        let mut text = String::from(
            "samples above a provisional target (applied by the reviewer, not asserted):\n",
        );
        for sample in samples {
            let total = sample.total.get();
            if Group::Idle.holds(sample) && total > Some(IDLE_TOTAL_TARGET) {
                text.push_str(&format!(
                    "  sample {} idle: total {} above 1 ms (idle target: the largest total at most 1 ms)\n",
                    sample.index,
                    value(sample.total)
                ));
            }
            if Group::BusyWithOverlap.holds(sample) && total > Some(BUSY_TOTAL_P99_TARGET) {
                text.push_str(&format!(
                    "  sample {} busy with overlap: total {} above 10 ms (busy target: the 99th percentile of total at most 10 ms)\n",
                    sample.index,
                    value(sample.total)
                ));
            }
        }
        text.push_str("values above ten times their number's median in their group:\n");
        for group in [
            Group::Idle,
            Group::BusyWithOverlap,
            Group::BusyWithoutOverlap,
        ] {
            for number in NUMBERS {
                let Some(median) = statistics.of(group, number).median else {
                    continue;
                };
                let limit = median.saturating_mul(10);
                for sample in samples.iter().filter(|sample| group.holds(sample)) {
                    for found in number.values(sample) {
                        if found.get() > Some(limit) {
                            text.push_str(&format!(
                                "  {} / {}: sample {}: {} (median {median} ns)\n",
                                group.as_str(),
                                number.as_str(),
                                sample.index,
                                value(found)
                            ));
                        }
                    }
                }
            }
        }
        text
    }

    /// The bytes between this block's marker lines, as compiled, hashed.
    fn block_hash() -> String {
        const BEGIN: &str = concat!("\n// ---- RS-01 T10 ", "BEGIN ----\n");
        const END: &str = concat!("\n// ---- RS-01 T10 ", "END ----\n");
        let source = include_str!("tests.rs");
        let begin = source.find(BEGIN).expect("the begin marker") + BEGIN.len();
        let end = source.find(END).expect("the end marker") + 1;
        hex::encode(Sha256::digest(&source.as_bytes()[begin..end]))
    }

    // -- the harness ----------------------------------------------------

    /// The one monotonic clock every timestamp of a run is read from.
    #[derive(Clone, Copy)]
    struct Clock(Instant);

    impl Clock {
        fn now(self) -> Nanos {
            Nanos::try_from(self.0.elapsed().as_nanos()).unwrap_or(Nanos::MAX)
        }
    }

    /// A report slot. A thread moves its report in as its last act; the
    /// controller only tries it, and never waits on it.
    struct Slot<T>(Mutex<Option<T>>);

    impl<T> Slot<T> {
        fn empty() -> Self {
            Self(Mutex::new(None))
        }

        fn put(&self, report: T) {
            if let Ok(mut slot) = self.0.lock() {
                *slot = Some(report);
            }
        }

        fn try_take(&self) -> Option<T> {
            self.0.try_lock().ok()?.take()
        }
    }

    /// One sample's flags, signals, progress markers and report slots, all
    /// created before its threads start. Neither flag is in the registry.
    struct Signals {
        /// Set by the stopping thread before a clock read that returned
        /// less than *start*.
        flag_1: AtomicBool,
        /// Set by the stopping thread after a clock read that returned more
        /// than *return*.
        flag_2: AtomicBool,
        release: AtomicBool,
        end: AtomicBool,
        stopper_ready: AtomicBool,
        observer_ready: AtomicBool,
        caller_ready: [AtomicBool; 4],
        /// The start of the call each competing thread began most recently:
        /// a progress marker, not a sign that the call still runs.
        progress: [AtomicU64; 4],
        stop: Slot<StopReport>,
        observer: Slot<ObserverReport>,
        callers: [Slot<CallerReport>; 4],
    }

    impl Signals {
        fn new() -> Self {
            Self {
                flag_1: AtomicBool::new(false),
                flag_2: AtomicBool::new(false),
                release: AtomicBool::new(false),
                end: AtomicBool::new(false),
                stopper_ready: AtomicBool::new(false),
                observer_ready: AtomicBool::new(false),
                caller_ready: Default::default(),
                progress: [const { AtomicU64::new(NO_PROGRESS) }; 4],
                stop: Slot::empty(),
                observer: Slot::empty(),
                callers: std::array::from_fn(|_| Slot::empty()),
            }
        }

        /// The readiness signals that have not come yet, by name.
        fn not_ready(&self, callers: usize) -> Vec<&'static str> {
            let mut missing = Vec::new();
            if !self.stopper_ready.load(Ordering::SeqCst) {
                missing.push("stopping thread");
            }
            if !self.observer_ready.load(Ordering::SeqCst) {
                missing.push("observer");
            }
            for (ready, (name, _)) in self.caller_ready.iter().zip(CALLERS).take(callers) {
                if !ready.load(Ordering::SeqCst) {
                    missing.push(name);
                }
            }
            missing
        }
    }

    /// The stopping thread: ready, released, flag 1, a clock read below
    /// *start*, *start*, the stop, *return*, a clock read above *return*,
    /// flag 2; then its reference to the registry goes, and its report is
    /// its last act.
    fn stopper(registry: Arc<RunRegistry>, signals: Arc<Signals>, clock: Clock) {
        signals.stopper_ready.store(true, Ordering::SeqCst);
        while !signals.release.load(Ordering::SeqCst) {
            if signals.end.load(Ordering::SeqCst) {
                // Never released: no stop was issued, and nothing to report.
                return;
            }
            std::hint::spin_loop();
        }
        signals.flag_1.store(true, Ordering::SeqCst);
        let below = clock.now();
        while clock.now() <= below {
            std::hint::spin_loop();
        }
        let start = clock.now();
        registry.stop_all();
        let ret = clock.now();
        while clock.now() <= ret {
            std::hint::spin_loop();
        }
        signals.flag_2.store(true, Ordering::SeqCst);
        drop(registry);
        signals.stop.put(StopReport { start, ret });
    }

    /// The observer: it reads the flag in a tight loop, and in every turn
    /// tests the flag before it looks at the request to end.
    fn observer(registry: Arc<RunRegistry>, signals: Arc<Signals>, clock: Clock) {
        signals.observer_ready.store(true, Ordering::SeqCst);
        let seen = loop {
            if registry.is_stopped() {
                break Some(clock.now());
            }
            if signals.end.load(Ordering::SeqCst) {
                break None;
            }
        };
        drop(registry);
        signals.observer.put(ObserverReport { seen });
    }

    /// A competing thread's own: its place, its method, the run and agent it
    /// calls with, and its log.
    struct Competing {
        index: usize,
        method: Method,
        run: RunId,
        agent: AgentId,
        log: Vec<Call>,
    }

    /// What a measured call returned, kept whole until after its end is read.
    enum Answer {
        Open(Result<RunId, AuthorityError>),
        Check(Result<CancelToken, AuthorityError>),
    }

    impl Answer {
        fn class(&self) -> &'static str {
            match self {
                Answer::Open(Ok(_)) | Answer::Check(Ok(_)) => "ok",
                Answer::Open(Err(error)) | Answer::Check(Err(error)) => error.class(),
            }
        }
    }

    /// A competing thread. Per call: flag 2, *start*, the progress marker,
    /// the call, *end*, flag 1, the record (unless flag 2 was set or flag 1
    /// was not), the count, the request to end. Its report is its last act.
    fn competing(registry: Arc<RunRegistry>, signals: Arc<Signals>, clock: Clock, me: Competing) {
        let Competing {
            index,
            method,
            run,
            agent,
            mut log,
        } = me;
        let (mut kept, mut calls, mut overflow) = (0usize, 0u64, false);
        loop {
            // `open` takes its agent by value: made before the call's clock.
            let owned = (method == Method::Open).then(|| agent.clone());
            let after_flag_2 = signals.flag_2.load(Ordering::SeqCst);
            let start = clock.now();
            signals.progress[index].store(start, Ordering::SeqCst);
            let answer = match owned {
                Some(owned) => {
                    Answer::Open(registry.open(owned, RunOrigin::AgentGoal, RunClass::Agent, 0))
                }
                None => Answer::Check(registry.check(run, &agent)),
            };
            let end = clock.now();
            let flag_1 = signals.flag_1.load(Ordering::SeqCst);
            if flag_1 && !after_flag_2 {
                match log.get_mut(kept) {
                    Some(entry) => {
                        *entry = Call {
                            method,
                            start,
                            end,
                            result: answer.class(),
                        };
                        kept += 1;
                    }
                    None => overflow = true,
                }
            }
            drop(answer);
            calls += 1;
            if calls == READY_CALLS {
                signals.caller_ready[index].store(true, Ordering::SeqCst);
            }
            if signals.end.load(Ordering::SeqCst) {
                break;
            }
        }
        drop(registry);
        signals.callers[index].put(CallerReport {
            calls,
            log,
            kept,
            overflow,
        });
    }

    /// What the controller reads and does. In T10 that is the real clock,
    /// the real readiness signals and the real report slots (`Live`); in
    /// T10b it is a script, with no thread and no real time. The decisions
    /// are made by `control` alone, whichever drives it.
    trait World {
        /// One clock read.
        fn now(&mut self) -> Nanos;
        /// The readiness signals that have not come yet, by name.
        fn not_ready(&mut self) -> Vec<&'static str>;
        /// Releases the stopping thread.
        fn release(&mut self);
        /// Tells every thread to end.
        fn end(&mut self);
        /// Each tries one slot without waiting.
        fn take_stop(&mut self) -> Option<StopReport>;
        fn take_observer(&mut self) -> Option<ObserverReport>;
        fn take_caller(&mut self, index: usize) -> Option<CallerReport>;
        /// A competing thread's progress marker, if a call has set it.
        fn progress(&mut self, index: usize) -> Option<Nanos>;
        /// The wait between two looks.
        fn pause(&mut self);
        /// The fixed wait before the last look.
        fn collection_wait(&mut self);
    }

    /// The real world of one sample.
    struct Live<'a> {
        signals: &'a Signals,
        clock: Clock,
        callers: usize,
    }

    impl World for Live<'_> {
        fn now(&mut self) -> Nanos {
            self.clock.now()
        }

        fn not_ready(&mut self) -> Vec<&'static str> {
            self.signals.not_ready(self.callers)
        }

        fn release(&mut self) {
            self.signals.release.store(true, Ordering::SeqCst);
        }

        fn end(&mut self) {
            self.signals.end.store(true, Ordering::SeqCst);
        }

        fn take_stop(&mut self) -> Option<StopReport> {
            self.signals.stop.try_take()
        }

        fn take_observer(&mut self) -> Option<ObserverReport> {
            self.signals.observer.try_take()
        }

        fn take_caller(&mut self, index: usize) -> Option<CallerReport> {
            self.signals.callers[index].try_take()
        }

        fn progress(&mut self, index: usize) -> Option<Nanos> {
            let at = self.signals.progress[index].load(Ordering::SeqCst);
            (at != NO_PROGRESS).then_some(at)
        }

        fn pause(&mut self) {
            std::thread::sleep(POLL);
        }

        fn collection_wait(&mut self) {
            std::thread::sleep(COLLECTION_WAIT);
        }
    }

    /// What the controller took in one turn, before the clock read that
    /// follows showed whether it was on time.
    #[derive(Default)]
    struct Turn {
        stop: Option<StopReport>,
        observer: Option<ObserverReport>,
        callers: [Option<CallerReport>; CALLERS.len()],
    }

    /// Every readiness signal of a sample, by name.
    fn every_signal(callers: usize) -> Vec<&'static str> {
        let mut every = vec!["stopping thread", "observer"];
        every.extend(CALLERS[..callers].iter().map(|&(name, _)| name));
        every
    }

    /// The controller's last look, one fixed second after it gave up: a
    /// report found now, or first seen in a turn whose clock read was past
    /// the deadline, is a late report, kept raw, and changes nothing.
    fn collect_late(world: &mut impl World, received: &mut Received, seen: Turn) {
        world.collection_wait();
        if received.stop.is_none() {
            if let Some(stop) = seen.stop.or_else(|| world.take_stop()) {
                received.late.push(Late::Stop(stop));
            }
        }
        if received.observer.is_none() {
            if let Some(observer) = seen.observer.or_else(|| world.take_observer()) {
                received.late.push(Late::Observer(observer));
            }
        }
        let mut late = Vec::new();
        for (index, (caller, seen)) in received.callers.iter().zip(seen.callers).enumerate() {
            if caller.report.is_none() {
                if let Some(report) = seen.or_else(|| world.take_caller(index)) {
                    late.push(Late::Caller(caller.name, report));
                }
            }
        }
        received.late.extend(late);
    }

    /// The controller gives up at its reading `now`: it tells every thread
    /// to end, keeps the progress markers of the threads it asked and has
    /// not heard from, and takes its last look.
    fn give_up(world: &mut impl World, received: &mut Received, now: Nanos, seen: Turn) {
        received.readings.gave_up = Some(now);
        world.end();
        if received.stop.is_some() {
            for (index, caller) in received.callers.iter_mut().enumerate() {
                if caller.report.is_none() {
                    caller.progress = world.progress(index);
                }
            }
        }
        collect_late(world, received, seen);
    }

    /// The controller: what it received within its bounded waits. It
    /// joins nothing and never blocks on a slot.
    fn control(
        index: usize,
        workload: Workload,
        callers: usize,
        world: &mut impl World,
    ) -> Received {
        let threads_started = world.now();
        let mut received = Received {
            index,
            workload,
            readings: Readings {
                threads_started,
                ..Readings::default()
            },
            not_ready: Vec::new(),
            stop: None,
            observer: None,
            callers: CALLERS[..callers]
                .iter()
                .map(|&(name, _)| Caller {
                    name,
                    report: None,
                    progress: None,
                })
                .collect(),
            late: Vec::new(),
        };
        // An observation counts as on time only if a clock read made after
        // it is not after the deadline. So in each turn the controller first
        // looks, and only then reads the clock.
        //
        // Readiness, within its own deadline. A signal seen in a turn whose
        // reading is past the deadline is not on time: the stopping thread
        // is never released.
        let readiness_deadline = threads_started.saturating_add(READINESS_WAIT);
        let mut missing = every_signal(callers);
        loop {
            let not_ready = world.not_ready();
            let now = world.now();
            if now <= readiness_deadline {
                missing = not_ready;
                if missing.is_empty() {
                    break;
                }
            }
            if now >= readiness_deadline {
                received.not_ready = missing;
                give_up(world, &mut received, now, Turn::default());
                return received;
            }
            world.pause();
        }
        // Completion, within 5 seconds of the release.
        let released = world.now();
        received.readings.released = Some(released);
        world.release();
        let deadline = released.saturating_add(COMPLETION_WAIT);
        loop {
            let mut seen = Turn::default();
            if received.stop.is_none() {
                seen.stop = world.take_stop();
            }
            if received.observer.is_none() {
                seen.observer = world.take_observer();
            }
            if received.readings.asked_to_end.is_some() {
                for (index, caller) in received.callers.iter().enumerate() {
                    if caller.report.is_none() {
                        seen.callers[index] = world.take_caller(index);
                    }
                }
            }
            let now = world.now();
            if now > deadline {
                // Nothing first seen in this turn is on time: the sample
                // ends as it stood before it, and what was seen is late.
                give_up(world, &mut received, now, seen);
                return received;
            }
            if let Some(stop) = seen.stop {
                received.stop = Some(stop);
                received.readings.asked_to_end = Some(now);
                world.end();
            }
            if seen.observer.is_some() {
                received.observer = seen.observer;
            }
            for (caller, report) in received.callers.iter_mut().zip(seen.callers) {
                if report.is_some() {
                    caller.report = report;
                }
            }
            if received.stop.is_some()
                && received.observer.is_some()
                && received
                    .callers
                    .iter()
                    .all(|caller| caller.report.is_some())
            {
                return received;
            }
            if now >= deadline {
                give_up(world, &mut received, now, Turn::default());
                return received;
            }
            world.pause();
        }
    }

    /// One sample: its threads (detached, never joined), the stop, and what
    /// the controller received within its bounded waits.
    fn sample(
        index: usize,
        workload: Workload,
        registry: &Arc<RunRegistry>,
        run: RunId,
        clock: Clock,
    ) -> Received {
        let callers = match workload {
            Workload::Idle => 0,
            Workload::Busy => CALLERS.len(),
        };
        let signals = Arc::new(Signals::new());
        // The logs, written through once so that their memory is in place.
        let logs: Vec<Vec<Call>> = (0..callers)
            .map(|_| vec![Call::UNUSED; LOG_ENTRIES])
            .collect();
        let agent = AgentId::new("t10-agent").expect("an agent");
        {
            let (registry, signals) = (registry.clone(), signals.clone());
            std::thread::spawn(move || stopper(registry, signals, clock));
        }
        {
            let (registry, signals) = (registry.clone(), signals.clone());
            std::thread::spawn(move || observer(registry, signals, clock));
        }
        for (index, log) in logs.into_iter().enumerate() {
            let me = Competing {
                index,
                method: CALLERS[index].1,
                run,
                agent: agent.clone(),
                log,
            };
            let (registry, signals) = (registry.clone(), signals.clone());
            std::thread::spawn(move || competing(registry, signals, clock, me));
        }
        let mut world = Live {
            signals: &signals,
            clock,
            callers,
        };
        control(index, workload, callers, &mut world)
    }

    /// The local-model workloads loaded or running now: every process whose
    /// name, or whose executable's file name, names a local-model runtime.
    /// Read only: nothing is started or stopped.
    fn model_processes() -> Vec<String> {
        let Ok(entries) = std::fs::read_dir("/proc") else {
            return vec!["unavailable (no /proc)".to_string()];
        };
        let mut found = Vec::new();
        for entry in entries.flatten() {
            let name = entry.file_name();
            let Some(pid) = name
                .to_str()
                .filter(|n| n.bytes().all(|b| b.is_ascii_digit()))
            else {
                continue;
            };
            let comm = std::fs::read_to_string(entry.path().join("comm"))
                .unwrap_or_default()
                .trim()
                .to_lowercase();
            let cmdline = std::fs::read(entry.path().join("cmdline")).unwrap_or_default();
            let args: Vec<String> = cmdline
                .split(|b| *b == 0)
                .filter(|arg| !arg.is_empty())
                .map(|arg| String::from_utf8_lossy(arg).into_owned())
                .collect();
            let exe = args
                .first()
                .map(|arg| arg.rsplit('/').next().unwrap_or(arg).to_lowercase())
                .unwrap_or_default();
            if MODEL_RUNTIMES
                .iter()
                .any(|runtime| comm.contains(runtime) || exe.contains(runtime))
            {
                let shown: String = args.join(" ").chars().take(160).collect();
                found.push(format!("pid {pid} {comm}: {shown}"));
            }
        }
        found.sort();
        found
    }

    fn host(at: &str, clock: Clock) -> String {
        let load = std::fs::read_to_string("/proc/loadavg")
            .ok()
            .and_then(|line| line.split_whitespace().next().map(str::to_string))
            .unwrap_or_else(|| "unavailable".to_string());
        let workloads = model_processes();
        format!(
            "host at {at} ({} ns): load average (1 min) {load}; local-model workloads: {}\n",
            clock.now(),
            if workloads.is_empty() {
                "none".to_string()
            } else {
                workloads.join(" | ")
            }
        )
    }

    fn given(variable: &str) -> String {
        std::env::var(variable).unwrap_or_else(|_| "not given".to_string())
    }

    fn first_line(path: &str, prefix: &str) -> String {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|text| {
                text.lines()
                    .find(|line| line.starts_with(prefix))
                    .map(|line| line.to_string())
            })
            .unwrap_or_else(|| "unavailable".to_string())
    }

    fn header() -> String {
        let cores = std::thread::available_parallelism().map_or(0, usize::from);
        let mut text = String::from(
            "RS-01 T10 receipt (P3-C11-RS01-CONTRACT-05 section 8): a diagnostic measurement of \
             RunRegistry::stop_all, the registry step alone, seen from outside its lock.\n",
        );
        text.push_str(&format!(
            "T10 and T10a source sha256 (between the block markers, as compiled): {}\n",
            block_hash()
        ));
        text.push_str(&format!("commit: {}\n", given("NEXUS_T10_COMMIT")));
        text.push_str(&format!("tree: {}\n", given("NEXUS_T10_TREE")));
        text.push_str(&format!("command: {}\n", given("NEXUS_T10_COMMAND")));
        text.push_str(&format!(
            "test binary arguments: {:?}\n",
            std::env::args().collect::<Vec<_>>()
        ));
        text.push_str(&format!(
            "machine: {}; kernel {}; {}; logical cores {cores}\n",
            first_line("/proc/sys/kernel/hostname", ""),
            first_line("/proc/sys/kernel/osrelease", ""),
            first_line("/proc/cpuinfo", "model name")
        ));
        text.push_str(&format!(
            "parameters: {RUNS} active runs per new registry; {SAMPLES} samples per workload (idle first, then busy); \
             {LOG_ENTRIES} log entries per competing thread; readiness wait 5 s; completion wait 5 s after released; \
             collection wait 1 s; controller poll {} us; busy workload: open-1, open-2 call open, check-1, check-2 call check\n",
            POLL.as_micros()
        ));
        text.push_str(
            "reading: every number is a span between two clock reads, not between the events themselves; \
             observed flag latency is an upper bound on when the flag was set, not that moment; \
             T10 does not measure the stop's wait for the lock, nor the sweep; \
             overlapping caller latency is a whole call's duration, not its wait for the lock; \
             the busy results are a competing-call workload, not a contended lock\n",
        );
        text.push_str(
            "not controlled: a thread that has reported may still be exiting when the next sample starts; \
             freeing a sample's registry and logs between samples can disturb the next sample. \
             The logging disturbs what it measures (two clock reads, two flag reads, one progress store, \
             one count, one look at the request to end per call, and one write for a kept call); \
             it is the same on both trees, its effect need not be. Threads are detached and never joined.\n",
        );
        text
    }

    /// A receipt is kept outside the repository.
    fn outside_the_repository(path: &std::path::Path) {
        let repository = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .expect("the repository");
        let directory = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or(std::path::Path::new("."))
            .canonicalize()
            .expect("the receipt's directory exists");
        assert!(
            !directory.starts_with(&repository),
            "the receipt is kept outside the repository: {}",
            path.display()
        );
    }

    fn write(receipt: &mut impl std::io::Write, text: &str) {
        receipt
            .write_all(text.as_bytes())
            .expect("the receipt is written");
    }

    /// RS-01 T10. Each sample builds a new registry of 4,096 active runs on
    /// this thread and times one stop; 1,000 samples per workload, all
    /// kept. Any sample that is not complete ends the run INCOMPLETE: the
    /// receipt is written, there are no statistics, and the test fails.
    #[test]
    #[ignore = "RS-01 T10: a diagnostic measurement, run only by its own command after T10a"]
    fn rs01_t10_the_registry_stop_seen_from_outside_its_lock() {
        let path = std::path::PathBuf::from(
            std::env::var_os("NEXUS_T10_RECEIPT")
                .expect("NEXUS_T10_RECEIPT names the receipt file"),
        );
        outside_the_repository(&path);
        let mut receipt =
            std::io::BufWriter::new(std::fs::File::create(&path).expect("the receipt file"));
        let clock = Clock(Instant::now());
        write(&mut receipt, &header());
        write(&mut receipt, &host("start", clock));
        let agent = AgentId::new("t10-agent").expect("an agent");
        let mut samples: Vec<Sample> = Vec::new();
        let mut incomplete = None;
        'run: for workload in [Workload::Idle, Workload::Busy] {
            for _ in 0..SAMPLES {
                let index = samples.len() + 1;
                // Before the sample: its registry, on this thread.
                let registry = Arc::new(RunRegistry::default());
                let mut first = None;
                for _ in 0..RUNS {
                    let run = registry
                        .open(agent.clone(), RunOrigin::AgentGoal, RunClass::Agent, 0)
                        .expect("a run opens");
                    first.get_or_insert(run);
                }
                let received = sample(index, workload, &registry, first.expect("a run"), clock);
                let measured = numbers(&received);
                write(&mut receipt, &sample_text(&received, &measured));
                write(&mut receipt, &calls_text(&received));
                let complete = measured.complete();
                samples.push(measured);
                if !complete {
                    incomplete = Some(index);
                    break 'run;
                }
                // Between samples: the logs and the registry are freed here,
                // on this thread (every thread dropped its reference before
                // it reported), before the next sample's threads start.
                drop(received);
                drop(registry);
                if index.is_multiple_of(HOST_EVERY) {
                    write(&mut receipt, &host(&format!("after sample {index}"), clock));
                }
            }
        }
        write(&mut receipt, &host("end", clock));
        let late_observers = samples.iter().filter(|sample| sample.late_observer).count();
        write(
            &mut receipt,
            &format!("late observer samples: {late_observers}\n"),
        );
        if let Some(index) = incomplete {
            write(
                &mut receipt,
                &format!(
                    "RUN INCOMPLETE at sample {index}: no statistics; not a pass; it supports no comparison of cost\n"
                ),
            );
            receipt.flush().expect("the receipt is flushed");
            panic!(
                "T10 ended INCOMPLETE at sample {index}; see {}",
                path.display()
            );
        }
        let combined = statistics(&samples).expect("every sample is complete");
        write(&mut receipt, &statistics_text(&combined));
        write(&mut receipt, &outliers_text(&samples, &combined));
        write(&mut receipt, "RUN COMPLETE: every sample complete\n");
        receipt.flush().expect("the receipt is flushed");
    }

    // -- T10a -------------------------------------------------------------

    const STOP: StopReport = StopReport {
        start: 2_000,
        ret: 2_800,
    };

    fn readings() -> Readings {
        Readings {
            threads_started: 100,
            released: Some(1_000),
            asked_to_end: Some(3_000),
            gave_up: None,
        }
    }

    fn call(method: Method, start: Nanos, end: Nanos) -> Call {
        Call {
            method,
            start,
            end,
            result: "capacity",
        }
    }

    fn report(log: &[Call]) -> CallerReport {
        CallerReport {
            calls: 100 + log.len() as u64,
            log: log.to_vec(),
            kept: log.len(),
            overflow: false,
        }
    }

    /// A complete idle sample: start 2000, return 2800, seen as given.
    fn idle(seen: Option<Nanos>) -> Received {
        Received {
            index: 1,
            workload: Workload::Idle,
            readings: readings(),
            not_ready: Vec::new(),
            stop: Some(STOP),
            observer: Some(ObserverReport { seen }),
            callers: Vec::new(),
            late: Vec::new(),
        }
    }

    /// A complete idle sample whose total is `total`, seen 1 ns after start.
    fn idle_total(total: Nanos) -> Received {
        let mut r = idle(Some(1_001));
        r.stop = Some(StopReport {
            start: 1_000,
            ret: 1_000 + total,
        });
        r
    }

    /// A complete busy sample, each competing thread's log as given.
    fn busy(logs: [Vec<Call>; 4]) -> Received {
        let callers = CALLERS
            .iter()
            .zip(logs)
            .map(|(&(name, _), log)| Caller {
                name,
                report: Some(report(&log)),
                progress: None,
            })
            .collect();
        Received {
            index: 2,
            workload: Workload::Busy,
            readings: readings(),
            not_ready: Vec::new(),
            stop: Some(STOP),
            observer: Some(ObserverReport { seen: Some(2_400) }),
            callers,
            late: Vec::new(),
        }
    }

    const PARTICIPANT_MISSING: &str = "\
sample 2 busy: participant report missing (check-2)
  controller: threads started 100 ns; released 1000 ns; asked to end 3000 ns; gave up 5001000 ns
  received: start 2000 ns; return 2800 ns; seen 2400 ns
  numbers: observed flag latency 400 ns; total 800 ns; flag latency bound 400 ns; observation to return 400 ns; overlapping calls 1; largest overlapping caller latency 100 ns
  participant report missing: check-2: its final report had not arrived by the deadline; whether its last call finished is not known; last start it published (progress marker) 2950 ns; asked to end 3000 ns; no duration is given for that call
  controller waiting time: 5000000 ns (gave up minus released). This is the controller's waiting time. It is not a lower bound on how long any call took.
";

    const PARTICIPANT_LATE: &str = "  late report (changes no result; enters no statistic): check-2: calls made 101; records kept 1; log full: no
    logged check start 2950 ns end 5000900 ns result emergency_stopped
";

    const STOP_MISSING: &str = "\
sample 2 busy: stop return not observed; participant report not requested (open-1, open-2, check-1, check-2)
  controller: threads started 100 ns; released 1000 ns; asked to end none; gave up 5001000 ns
  received: start none; return none; seen 2400 ns
  numbers: observed flag latency unavailable; total unavailable; flag latency bound unavailable; observation to return unavailable; overlapping calls 0; largest overlapping caller latency unavailable
  stop return not observed: the stop's report had not arrived by the deadline; whether the stop returned is not known; start and total are unavailable; released 1000 ns is the controller's own reading, not the stop's start
  participant report not requested: the stop's report did not arrive, so open-1, open-2, check-1, check-2 were not asked for their reports before the deadline; nothing is known or implied about those threads
  controller waiting time: 5000000 ns (gave up minus released). This is the controller's waiting time. It is not a lower bound on how long any call took.
";

    const STOP_LATE: &str = "  late report (changes no result; enters no statistic): stop: start 2000 ns; return 5000500 ns
";

    /// RS-01 T10a: T10's arithmetic, results and receipt wording, on
    /// fixed inputs. Nothing here runs a thread or reads a clock.
    #[test]
    fn rs01_t10a_the_receipt_claims_only_what_the_controller_received() {
        use Outcome::*;
        use Value::{Of, Unavailable};
        let four = |s: &Sample| {
            (
                s.observed_flag_latency,
                s.total,
                s.flag_latency_bound,
                s.observation_to_return,
            )
        };
        assert_eq!(block_hash().len(), 64);

        // The observation before the return, after it, and the two equal.
        let s = numbers(&idle(Some(2_400)));
        assert_eq!(s.results, [Complete]);
        assert_eq!(four(&s), (Of(400), Of(800), Of(400), Of(400)));
        assert!(!s.late_observer);
        assert_eq!(s.waiting_time, None);
        let s = numbers(&idle(Some(3_000)));
        assert_eq!(s.results, [Complete]);
        // The bound is the smaller of the two; a late observer's interval
        // is unavailable, never zero.
        assert_eq!(four(&s), (Of(1_000), Of(800), Of(800), Unavailable));
        assert!(s.late_observer);
        let s = numbers(&idle(Some(2_800)));
        assert_eq!(four(&s), (Of(800), Of(800), Of(800), Of(0)));
        assert!(!s.late_observer);

        // The observer's report missing, or without seen.
        for observer in [None, Some(ObserverReport { seen: None })] {
            let mut r = idle(None);
            r.observer = observer;
            r.readings.gave_up = Some(5_001_000);
            let s = numbers(&r);
            assert_eq!(s.results, [ObserverResultMissing]);
            assert_eq!(four(&s), (Unavailable, Of(800), Unavailable, Unavailable));
            assert_eq!(s.waiting_time, Some(5_000_000));
        }

        // seen, or return, before start: a harness fault, not a number.
        let s = numbers(&idle(Some(1_500)));
        assert_eq!(s.results, [HarnessFault("seen before start")]);
        assert_eq!(s.observed_flag_latency, Unavailable);
        assert_eq!(s.flag_latency_bound, Unavailable);
        let mut r = idle(Some(1_950));
        r.stop = Some(StopReport {
            start: 2_000,
            ret: 1_900,
        });
        let s = numbers(&r);
        assert_eq!(
            s.results,
            [
                HarnessFault("return before start"),
                HarnessFault("seen before start")
            ]
        );
        assert_eq!(
            four(&s),
            (Unavailable, Unavailable, Unavailable, Unavailable)
        );

        // A readiness signal missing: no stop, no number, the waiting time.
        let mut r = idle(None);
        r.not_ready = vec!["observer"];
        r.stop = None;
        r.observer = None;
        r.readings = Readings {
            threads_started: 100,
            released: None,
            asked_to_end: None,
            gave_up: Some(5_000_100),
        };
        let s = numbers(&r);
        assert_eq!(s.results, [ReadinessNotReported(vec!["observer"])]);
        assert_eq!(
            four(&s),
            (Unavailable, Unavailable, Unavailable, Unavailable)
        );
        assert_eq!(s.waiting_time, Some(5_000_000));
        assert!(sample_text(&r, &s).contains(
            "controller waiting time: 5000000 ns (gave up minus threads started). This is the \
             controller's waiting time. It is not a lower bound on how long any call took."
        ));

        // A full log, and two results at once.
        let mut r = busy(Default::default());
        r.callers[0].report.as_mut().expect("a report").overflow = true;
        assert_eq!(numbers(&r).results, [LogOverflow("open-1")]);
        r.observer = None;
        r.readings.gave_up = Some(5_001_000);
        assert_eq!(
            numbers(&r).results,
            [ObserverResultMissing, LogOverflow("open-1")]
        );

        // Overlap follows the timestamps, not the filter: every call here
        // was logged; the stop ran from 2000 to 2800.
        let s = numbers(&busy([
            // wholly inside; starting before
            vec![
                call(Method::Open, 2_100, 2_200),
                call(Method::Open, 1_900, 2_100),
            ],
            // ending exactly at start; wholly outside
            vec![
                call(Method::Open, 1_950, 2_000),
                call(Method::Open, 3_000, 3_100),
            ],
            // ending after; spanning
            vec![
                call(Method::Check, 2_700, 2_900),
                call(Method::Check, 1_900, 2_900),
            ],
            // starting exactly at return; between flag 1 and start; between
            // return and flag 2
            vec![
                call(Method::Check, 2_800, 2_850),
                call(Method::Check, 1_800, 1_900),
                call(Method::Check, 2_810, 2_850),
            ],
        ]));
        assert_eq!(s.results, [Complete]);
        let overlaps: Vec<(&str, usize, Value)> = s
            .overlaps
            .iter()
            .map(|overlap| (overlap.caller, overlap.record, overlap.latency))
            .collect();
        assert_eq!(
            overlaps,
            [
                ("open-1", 0, Of(100)),
                ("open-1", 1, Of(200)),
                ("open-2", 0, Of(50)),
                ("check-1", 0, Of(200)),
                ("check-1", 1, Of(1_000)),
                ("check-2", 0, Of(50)),
            ]
        );
        assert_eq!(s.largest_overlap, Of(1_000));
        // A logged call whose end is before its start: a harness fault.
        let s = numbers(&busy([
            vec![call(Method::Open, 2_200, 2_100)],
            vec![],
            vec![],
            vec![],
        ]));
        assert_eq!(s.results, [HarnessFault("a call's end before its start")]);
        assert_eq!(s.largest_overlap, Unavailable);

        // RS4-01: a competing call ended before the deadline, but its
        // thread's report had not reached the controller by then.
        let mut missing = busy([
            vec![call(Method::Open, 2_100, 2_200)],
            vec![],
            vec![],
            vec![],
        ]);
        missing.callers[3].report = None;
        missing.callers[3].progress = Some(2_950);
        missing.readings.gave_up = Some(5_001_000);
        let s = numbers(&missing);
        assert_eq!(
            s.results,
            [ParticipantReportMissing {
                participant: "check-2",
                last_start: Some(2_950)
            }]
        );
        assert_eq!(s.waiting_time, Some(5_000_000));
        assert_eq!(sample_text(&missing, &s), PARTICIPANT_MISSING);
        // The same report, found afterwards: listed raw, changing nothing.
        let mut late = missing.clone();
        late.late.push(Late::Caller(
            "check-2",
            report(&[Call {
                method: Method::Check,
                start: 2_950,
                end: 5_000_900,
                result: "emergency_stopped",
            }]),
        ));
        assert_eq!(numbers(&late), s);
        assert_eq!(
            sample_text(&late, &numbers(&late)),
            format!("{PARTICIPANT_MISSING}{PARTICIPANT_LATE}")
        );

        // The stop's own report missing: the competing threads were not
        // asked, so they are not missing.
        let mut stop_missing = busy(Default::default());
        stop_missing.stop = None;
        for caller in &mut stop_missing.callers {
            caller.report = None;
        }
        stop_missing.readings.asked_to_end = None;
        stop_missing.readings.gave_up = Some(5_001_000);
        let s = numbers(&stop_missing);
        assert_eq!(
            s.results,
            [
                StopReturnNotObserved,
                ParticipantReportNotRequested(vec!["open-1", "open-2", "check-1", "check-2"])
            ]
        );
        assert_eq!(
            four(&s),
            (Unavailable, Unavailable, Unavailable, Unavailable)
        );
        assert!(s.overlaps.is_empty());
        assert_eq!(s.largest_overlap, Unavailable);
        assert_eq!(s.waiting_time, Some(5_000_000));
        assert_eq!(sample_text(&stop_missing, &s), STOP_MISSING);
        let mut late = stop_missing.clone();
        late.late.push(Late::Stop(StopReport {
            start: 2_000,
            ret: 5_000_500,
        }));
        assert_eq!(numbers(&late), s);
        assert_eq!(
            sample_text(&late, &numbers(&late)),
            format!("{STOP_MISSING}{STOP_LATE}")
        );
        for text in [PARTICIPANT_MISSING, STOP_MISSING] {
            assert!(text.contains("not a lower bound on how long any call took"));
            assert!(!text.contains("at least"));
        }

        // Statistics: none at all for a run with a sample that is not
        // complete, and a late report enters none.
        let complete = numbers(&idle(Some(2_400)));
        assert!(statistics(&[complete.clone(), numbers(&stop_missing)]).is_none());
        assert!(statistics(&[complete, numbers(&late)]).is_none());
        let totals = |n: Nanos| {
            (1..=n)
                .map(|total| numbers(&idle_total(total)))
                .collect::<Vec<_>>()
        };
        // 99 values: minimum, median and maximum, no 99th percentile.
        assert_eq!(
            statistics(&totals(99))
                .expect("complete")
                .of(Group::Idle, Number::Total),
            Summary {
                n: 99,
                unavailable: 0,
                min: Some(1),
                median: Some(50),
                p99: None,
                max: Some(99)
            }
        );
        // 100 values: all four.
        assert_eq!(
            statistics(&totals(100))
                .expect("complete")
                .of(Group::Idle, Number::Total),
            Summary {
                n: 100,
                unavailable: 0,
                min: Some(1),
                median: Some(50),
                p99: Some(99),
                max: Some(100)
            }
        );
        // Nearest rank on 1..=101: rounding the position down would give 50
        // and 99; counting from zero, 52 and 101.
        let rule = statistics(&totals(101))
            .expect("complete")
            .of(Group::Idle, Number::Total);
        assert_eq!((rule.median, rule.p99), (Some(51), Some(100)));
        // A group of one value, and empty groups (n = 0, no statistic).
        let one = statistics(&[numbers(&idle_total(7))]).expect("complete");
        assert_eq!(
            one.of(Group::Idle, Number::Total),
            Summary {
                n: 1,
                unavailable: 0,
                min: Some(7),
                median: Some(7),
                p99: None,
                max: Some(7)
            }
        );
        for group in [
            Group::Busy,
            Group::BusyWithOverlap,
            Group::BusyWithoutOverlap,
        ] {
            assert_eq!(one.of(group, Number::Total), Summary::default());
        }
        // An unavailable value among others: in neither n nor a statistic.
        let mut mixed = totals(3);
        let mut late_observer = idle_total(3);
        late_observer.observer = Some(ObserverReport { seen: Some(9_999) });
        mixed.push(numbers(&late_observer));
        assert_eq!(
            statistics(&mixed)
                .expect("complete")
                .of(Group::Idle, Number::ObservationToReturn),
            Summary {
                n: 3,
                unavailable: 1,
                min: Some(0),
                median: Some(1),
                p99: None,
                max: Some(2)
            }
        );
        // The busy groups: one sample with overlap, one without.
        let with = numbers(&busy([
            vec![call(Method::Open, 2_100, 2_400)],
            vec![],
            vec![],
            vec![],
        ]));
        let without = numbers(&busy([
            vec![call(Method::Open, 3_000, 3_100)],
            vec![],
            vec![],
            vec![],
        ]));
        let split = statistics(&[with, without]).expect("complete");
        assert_eq!(split.of(Group::Busy, Number::Total).n, 2);
        assert_eq!(split.of(Group::BusyWithOverlap, Number::Total).n, 1);
        assert_eq!(split.of(Group::BusyWithoutOverlap, Number::Total).n, 1);
        assert_eq!(
            split.of(Group::Busy, Number::LargestOverlappingCallerLatency),
            Summary {
                n: 1,
                unavailable: 1,
                min: Some(300),
                median: Some(300),
                p99: None,
                max: Some(300)
            }
        );
        assert_eq!(
            split.of(Group::BusyWithoutOverlap, Number::OverlappingCallerLatency),
            Summary::default()
        );
    }

    // -- T10b -------------------------------------------------------------

    const SECOND: Nanos = 1_000_000_000;
    /// Both of T10b's deadlines: threads started, and released, at 100 ns.
    const DEADLINE: Nanos = 100 + 5 * SECOND;
    const STOP_IN_TIME: StopReport = StopReport {
        start: 1_000,
        ret: 2_000,
    };
    const OBSERVER_IN_TIME: ObserverReport = ObserverReport { seen: Some(1_500) };

    /// T10b's world: a script, with no thread and no real time. The clock
    /// starts at 100 ns and stands still except where the script moves it:
    /// a pause moves it one second on, or, for a pause begun at a time
    /// `pauses` names, to the time given there; the taking of the report
    /// `stall` names moves it to the time given there, as if the
    /// controller lost its processor right after that taking. A signal or
    /// a report is there from its time on.
    struct Script {
        t: Nanos,
        ready: Vec<(&'static str, Nanos)>,
        stop: Option<(Nanos, StopReport)>,
        observer: Option<(Nanos, ObserverReport)>,
        callers: Vec<Option<(Nanos, CallerReport)>>,
        progress: Vec<Option<Nanos>>,
        pauses: Vec<(Nanos, Nanos)>,
        stall: Option<(&'static str, Nanos)>,
        released: bool,
        ended: bool,
    }

    impl Script {
        /// Every readiness signal there at 100 ns; no report anywhere.
        fn new(callers: usize) -> Self {
            let mut ready = vec![("stopping thread", 100), ("observer", 100)];
            ready.extend(CALLERS[..callers].iter().map(|&(name, _)| (name, 100)));
            Self {
                t: 100,
                ready,
                stop: None,
                observer: None,
                callers: vec![None; callers],
                progress: vec![None; callers],
                pauses: Vec::new(),
                stall: None,
                released: false,
                ended: false,
            }
        }

        /// A report taken: then the stall, if the script puts one after
        /// this taking.
        fn taken<T>(&mut self, name: &'static str, report: Option<T>) -> Option<T> {
            if report.is_some() {
                if let Some((_, to)) = self.stall.take_if(|(after, _)| *after == name) {
                    self.t = to;
                }
            }
            report
        }
    }

    impl World for Script {
        fn now(&mut self) -> Nanos {
            self.t
        }

        fn not_ready(&mut self) -> Vec<&'static str> {
            let t = self.t;
            self.ready
                .iter()
                .filter(|&&(_, at)| at > t)
                .map(|&(name, _)| name)
                .collect()
        }

        fn release(&mut self) {
            self.released = true;
        }

        fn end(&mut self) {
            self.ended = true;
        }

        fn take_stop(&mut self) -> Option<StopReport> {
            let t = self.t;
            let report = self.stop.take_if(|(at, _)| *at <= t).map(|(_, r)| r);
            self.taken("stop", report)
        }

        fn take_observer(&mut self) -> Option<ObserverReport> {
            let t = self.t;
            let report = self.observer.take_if(|(at, _)| *at <= t).map(|(_, r)| r);
            self.taken("observer", report)
        }

        fn take_caller(&mut self, index: usize) -> Option<CallerReport> {
            let t = self.t;
            let report = self.callers[index]
                .take_if(|(at, _)| *at <= t)
                .map(|(_, r)| r);
            self.taken(CALLERS[index].0, report)
        }

        fn progress(&mut self, index: usize) -> Option<Nanos> {
            self.progress[index]
        }

        fn pause(&mut self) {
            let from = self.t;
            self.t = self
                .pauses
                .iter()
                .find(|&&(begun, _)| begun == from)
                .map_or(from + SECOND, |&(_, to)| to);
        }

        fn collection_wait(&mut self) {
            self.t += SECOND;
        }
    }

    /// RS-01 T10b (finding MSR-01): the controller's own decisions, driven
    /// by scripted schedules. An observation is on time only if a clock read
    /// made after it is not after the deadline. Each schedule runs on its
    /// own; every one that fails is named at the end. Nothing here runs a
    /// thread or reads a clock.
    #[test]
    fn rs01_t10b_the_controller_accepts_nothing_after_its_deadline() {
        let schedules: [(&str, fn()); 5] = [
            ("S1", || {
                use Outcome::*;
                // Completion: the stop's report, the observer's and three of
                // four caller reports in time. The fourth is not there
                // before the deadline; the clock passes the deadline while
                // the controller pauses; then the fourth is there.
                let fourth = report(&[call(Method::Check, 1_500, 1_700)]);
                let mut w = Script::new(4);
                w.stop = Some((SECOND, STOP_IN_TIME));
                w.observer = Some((SECOND, OBSERVER_IN_TIME));
                for slot in &mut w.callers[..3] {
                    *slot = Some((2 * SECOND, report(&[])));
                }
                w.callers[3] = Some((DEADLINE + 1_000, fourth.clone()));
                w.progress[3] = Some(1_900);
                w.pauses.push((4 * SECOND + 100, DEADLINE + 2_000));
                let r = control(2, Workload::Busy, 4, &mut w);
                let s = numbers(&r);
                assert!(
                    !s.complete(),
                    "S1 (completion): a report first there after the deadline completed the sample: {r:?}"
                );
                assert_eq!(
                    s.results,
                    [ParticipantReportMissing {
                        participant: "check-2",
                        last_start: Some(1_900)
                    }],
                    "S1 (completion): the result names the missing participant"
                );
                assert_eq!(
                    r.readings,
                    Readings {
                        threads_started: 100,
                        released: Some(100),
                        asked_to_end: Some(SECOND + 100),
                        gave_up: Some(DEADLINE + 2_000),
                    },
                    "S1 (completion): gave up has a reading"
                );
                assert_eq!(s.waiting_time, Some(5 * SECOND + 2_000));
                assert_eq!(r.callers[3].report, None);
                assert_eq!(
                    r.late,
                    [Late::Caller("check-2", fourth)],
                    "S1 (completion): the fourth report is a late report"
                );
                assert!(
                    s.overlaps.iter().all(|overlap| overlap.caller != "check-2"),
                    "S1 (completion): the late report entered a number: {:?}",
                    s.overlaps
                );
                assert!(w.released && w.ended);
            }),
            ("S2", || {
                use Outcome::*;
                use Value::Unavailable;
                // Readiness: the observer's signal is first there after the
                // readiness deadline, which the clock passes while the
                // controller pauses.
                let mut w = Script::new(0);
                w.ready[1].1 = DEADLINE + 1_000;
                w.pauses.push((4 * SECOND + 100, DEADLINE + 2_000));
                let r = control(1, Workload::Idle, 0, &mut w);
                let s = numbers(&r);
                assert_eq!(
                    s.results,
                    [ReadinessNotReported(vec!["observer"])],
                    "S2 (readiness): a signal first there after the readiness deadline was accepted: {r:?}"
                );
                assert!(
                    !w.released,
                    "S2 (readiness): the stopping thread was released"
                );
                assert_eq!(
                    r.readings,
                    Readings {
                        threads_started: 100,
                        released: None,
                        asked_to_end: None,
                        gave_up: Some(DEADLINE + 2_000),
                    }
                );
                assert_eq!(s.waiting_time, Some(5 * SECOND + 2_000));
                assert_eq!(
                    (
                        s.observed_flag_latency,
                        s.total,
                        s.flag_latency_bound,
                        s.observation_to_return
                    ),
                    (Unavailable, Unavailable, Unavailable, Unavailable)
                );
                assert!(r.stop.is_none() && r.observer.is_none() && r.late.is_empty());
                assert!(w.ended);
            }),
            ("S3", || {
                use Outcome::*;
                use Value::Unavailable;
                // The processor lost inside one turn: the observer's report
                // is there before the deadline, but the clock read that
                // follows its taking returns a time after the deadline.
                let mut w = Script::new(0);
                w.stop = Some((SECOND, STOP_IN_TIME));
                w.observer = Some((4 * SECOND, OBSERVER_IN_TIME));
                w.stall = Some(("observer", DEADLINE + 1));
                let r = control(1, Workload::Idle, 0, &mut w);
                let s = numbers(&r);
                assert!(
                    !s.complete(),
                    "S3 (a turn across the deadline): a report not shown to be on time completed the sample: {r:?}"
                );
                assert_eq!(s.results, [ObserverResultMissing]);
                assert_eq!(r.observer, None);
                assert_eq!(
                    r.late,
                    [Late::Observer(OBSERVER_IN_TIME)],
                    "S3 (a turn across the deadline): the report is kept as a late report"
                );
                assert_eq!(r.readings.gave_up, Some(DEADLINE + 1));
                assert_eq!(s.observed_flag_latency, Unavailable);
            }),
            ("S4", || {
                use Outcome::*;
                use Value::Of;
                // Everything in time.
                let mut w = Script::new(4);
                w.stop = Some((SECOND, STOP_IN_TIME));
                w.observer = Some((SECOND, OBSERVER_IN_TIME));
                for slot in &mut w.callers {
                    *slot = Some((2 * SECOND, report(&[call(Method::Open, 1_500, 1_700)])));
                }
                let r = control(2, Workload::Busy, 4, &mut w);
                let s = numbers(&r);
                assert_eq!(s.results, [Complete], "S4 (everything in time): {r:?}");
                assert_eq!(
                    r.readings,
                    Readings {
                        threads_started: 100,
                        released: Some(100),
                        asked_to_end: Some(SECOND + 100),
                        gave_up: None,
                    }
                );
                assert!(r.late.is_empty());
                assert_eq!(s.waiting_time, None);
                assert_eq!((s.total, s.overlaps.len()), (Of(1_000), 4));
                assert!(w.released);
            }),
            ("S5", || {
                use Outcome::*;
                // The edge: the clock read that follows the last taking
                // equals the deadline exactly.
                let mut w = Script::new(0);
                w.stop = Some((SECOND, STOP_IN_TIME));
                w.observer = Some((4 * SECOND + 500, OBSERVER_IN_TIME));
                let r = control(1, Workload::Idle, 0, &mut w);
                let s = numbers(&r);
                assert_eq!(
                    s.results,
                    [Complete],
                    "S5 (the edge): a clock read equal to the deadline is on time: {r:?}"
                );
                assert_eq!(r.observer, Some(OBSERVER_IN_TIME));
                assert_eq!(r.readings.gave_up, None);
                assert!(r.late.is_empty());
            }),
        ];
        let mut failed = Vec::new();
        for (name, schedule) in schedules {
            if std::panic::catch_unwind(schedule).is_err() {
                failed.push(name);
            }
        }
        assert!(
            failed.is_empty(),
            "T10b: these schedules failed: {}",
            failed.join(", ")
        );
    }
}
// ---- RS-01 T10 END ----
