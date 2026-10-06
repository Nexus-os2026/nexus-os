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
