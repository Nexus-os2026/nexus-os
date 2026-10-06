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
    stopping.finish(Outcome::Cancelled);
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
        .contains("Any agent may use it until it expires or you revoke it"));
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
