//! The production sink over a real store (EL-1 §6, §8): owed records, the
//! gate, latching, the emergency stop and the projections, driven through
//! the pipeline with a fake effect.

use super::tests::Scratch;
use super::*;
use crate::authority::approval::{
    ActionConfirmation, ControlConfirmer, GrantConfirmation, ResumeConfirmation,
};
use crate::authority::clock::ManualClock;
use crate::authority::commitment::{
    CommitmentState, ExecutionGuard, FailureClass, Outcome, PreparedAction, TargetIdentity,
};
use crate::authority::effect::{CapabilityKind, EffectClass};
use crate::authority::evidence::{EvidencePhase, ExecutionEvidence, MemoryEvidence};
use crate::authority::ids::{AgentId, CommitmentId, Digest, GrantId, RunId};
use crate::authority::policy::GrantScope;
use crate::authority::run::RunOrigin;
use crate::authority::{Authority, AuthorityError};
use crate::control::{Control, EffectOutput, PendingEffect, Preparation};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

struct Yes;
impl ControlConfirmer for Yes {
    fn confirm_action(&self, _: &ActionConfirmation) -> bool {
        true
    }
    fn confirm_grant(&self, _: &GrantConfirmation) -> bool {
        true
    }
    fn confirm_resume(&self, _: &ResumeConfirmation) -> bool {
        true
    }
    fn confirm_acknowledgement(
        &self,
        _: &crate::authority::approval::AcknowledgementConfirmation,
    ) -> bool {
        false
    }
}

/// What the fake effect does to the store while it runs (between its start
/// record and its end record).
type During = Arc<dyn Fn(&PrimaryEvidence) + Send + Sync>;

struct Effect {
    evidence: Arc<PrimaryEvidence>,
    during: During,
}

fn target() -> Digest {
    Digest::of("fake.target", &[b"here"])
}

fn parameters() -> Digest {
    Digest::of("fake.params", &[b"p"])
}

impl PendingEffect for Effect {
    fn revalidate(&self) -> Result<Digest, AuthorityError> {
        Ok(target())
    }

    fn parameters(&self) -> Digest {
        parameters()
    }

    fn execute(
        self: Box<Self>,
        _guard: &ExecutionGuard,
    ) -> Result<EffectOutput, (FailureClass, String)> {
        (self.during)(&self.evidence);
        Ok(EffectOutput {
            text: Some("done".into()),
            ..Default::default()
        })
    }
}

struct Fixture {
    _scratch: Scratch,
    evidence: Arc<PrimaryEvidence>,
    recent: Arc<MemoryEvidence>,
    control: Arc<Control>,
    clock: Arc<ManualClock>,
    agent: AgentId,
    run: RunId,
    grant: GrantId,
}

fn fixture() -> Fixture {
    fixture_with(vec![])
}

/// The fixture, with `extra` projections after the recent history.
fn fixture_with(extra: Vec<Arc<dyn EvidenceProjection>>) -> Fixture {
    let scratch = Scratch::new();
    let recent = Arc::new(MemoryEvidence::new(10_000));
    let mut projections: Vec<Arc<dyn EvidenceProjection>> = vec![recent.clone()];
    projections.extend(extra);
    let evidence = PrimaryEvidence::open(&scratch.store(), projections).unwrap();
    let clock = Arc::new(ManualClock::default());
    let control = Arc::new(Control::new(Authority::new(
        evidence.clone(),
        clock.clone(),
    )));
    let agent = AgentId::new("agent-el1").unwrap();
    let run = control
        .authority()
        .open_run(agent.clone(), RunOrigin::AgentGoal)
        .unwrap();
    let grant = control
        .authority()
        .grants()
        .request(
            GrantScope::Perception {
                display: "agent".into(),
            },
            Duration::from_secs(600),
            &Yes,
        )
        .unwrap();
    Fixture {
        _scratch: scratch,
        evidence,
        recent,
        control,
        clock,
        agent,
        run,
        grant,
    }
}

impl Fixture {
    fn propose(&self, during: During) -> Result<CommitmentId, AuthorityError> {
        let preparation = Preparation {
            action: PreparedAction {
                kind: CapabilityKind::Perception,
                class: EffectClass::R1,
                operation: "fake.observe",
                target: TargetIdentity {
                    display: "fake target".into(),
                    digest: target(),
                },
                parameters: parameters(),
                grants: vec![self.grant],
                leases: vec![],
                summary: vec!["fake".into()],
            },
            effect: Box::new(Effect {
                evidence: self.evidence.clone(),
                during,
            }),
            ttl: Duration::from_secs(60),
        };
        self.control
            .propose(&self.agent, self.run, preparation)
            .map(|view| view.id)
    }

    /// Propose, authorize and execute one effect.
    fn run(&self, during: During) -> Result<EffectOutput, AuthorityError> {
        let id = self.propose(during)?;
        self.control.authorize(id, &self.agent, self.run, &Yes)?;
        self.control.execute(id, &self.agent, self.run)
    }

    fn rows(&self) -> Vec<Row> {
        self.evidence.store().rows()
    }

    fn phases(&self) -> Vec<String> {
        self.rows().into_iter().map(|row| row.phase).collect()
    }
}

fn nothing() -> During {
    Arc::new(|_| {})
}

/// The store refuses every insert while the effect runs: its start record is
/// written, its end record is not.
fn full_while_running() -> During {
    Arc::new(|evidence| evidence.store().refuse_inserts(true))
}

/// EL-1 §6, §8 (mutant 16): an end record that cannot be written is owed,
/// and the completed effect is reported as completed with its evidence
/// status, never as plain success.
#[test]
fn el1_n3_a_completed_effect_whose_end_record_is_owed_is_reported_owed() {
    let f = fixture();
    let output = f.run(full_while_running()).expect("the effect completed");
    assert_eq!(output.text.as_deref(), Some("done"));
    let start = f
        .rows()
        .iter()
        .find(|row| row.phase == "started")
        .expect("the start record")
        .seq;
    assert_eq!(
        output.evidence,
        Some(ExecutionEvidence {
            start,
            end: EndStatus::Owed
        })
    );
    assert_eq!(f.evidence.health().owed, 1);
    assert!(!f.evidence.health().latched);
    assert!(!f.phases().contains(&"finished".to_string()));
}

/// EL-1 §6 (XA-R1-01; mutant 2): while a record is owed, the next gating
/// record, and so the next admission, is refused; when the store accepts
/// again the owed record is written first, with its original body, and
/// admission proceeds.
#[test]
fn el1_n3_while_a_record_is_owed_nothing_new_is_admitted() {
    let f = fixture();
    f.run(full_while_running()).unwrap();
    let owed = f.evidence.store().owed_bodies();
    assert_eq!(owed.len(), 1);
    // The store refuses every insert: the record stays owed and nothing new is admitted.
    f.evidence.store().refuse_inserts(true);
    assert_eq!(
        f.propose(nothing()).unwrap_err(),
        AuthorityError::EvidenceUnavailable
    );
    assert_eq!(
        f.control
            .authority()
            .open_run(f.agent.clone(), RunOrigin::AgentGoal)
            .unwrap_err(),
        AuthorityError::EvidenceUnavailable
    );
    assert_eq!(f.evidence.health().owed, 1);
    // The store would take a new record; only the owed record's retry fails:
    // still nothing new is admitted or written.
    let before = f.rows();
    f.evidence.store().refuse_inserts(false);
    f.evidence.store().refuse_next_insert();
    assert_eq!(
        f.propose(nothing()).unwrap_err(),
        AuthorityError::EvidenceUnavailable
    );
    f.evidence.store().refuse_next_insert();
    assert_eq!(
        f.control
            .authority()
            .open_run(f.agent.clone(), RunOrigin::AgentGoal)
            .unwrap_err(),
        AuthorityError::EvidenceUnavailable
    );
    assert_eq!(f.rows().len(), before.len(), "nothing new is written");
    assert_eq!(f.evidence.health().owed, 1);
    assert!(!f.evidence.health().latched);
    // Accepting again: the owed record is written first, as it was.
    f.evidence.store().refuse_inserts(false);
    let id = f.propose(nothing()).expect("admitted once nothing is owed");
    assert_eq!(f.evidence.health().owed, 0);
    let rows = f.rows();
    let finished = rows.iter().position(|row| row.phase == "finished").unwrap();
    let prepared = rows
        .iter()
        .rposition(|row| row.phase == "prepared" && row.commitment == Some(id.to_string()))
        .unwrap();
    assert!(finished < prepared, "the owed record is written first");
    assert_eq!(rows[finished].body, owed[0], "with its original body");
    // The projection got it too, after the store.
    assert!(f
        .recent
        .records()
        .iter()
        .any(|r| r.phase == EvidencePhase::Finished));
}

/// EL-1 §5, §6 (mutant 5): an end record whose COMMIT fails is uncertain:
/// the store latches, the result reports uncertain, and every later
/// admission is refused.
#[test]
fn el1_n3_an_uncertain_end_record_latches_and_refuses_every_later_admission() {
    let f = fixture();
    let output = f
        .run(Arc::new(|evidence: &PrimaryEvidence| {
            evidence.store().plant_fault(Fault::CommitThenFail)
        }))
        .expect("the effect completed");
    assert_eq!(output.evidence.map(|e| e.end), Some(EndStatus::Uncertain));
    assert!(f.evidence.health().latched);
    assert_eq!(
        f.propose(nothing()).unwrap_err(),
        AuthorityError::EvidenceUnavailable
    );
    assert_eq!(
        f.control
            .authority()
            .open_run(f.agent.clone(), RunOrigin::AgentGoal)
            .unwrap_err(),
        AuthorityError::EvidenceUnavailable
    );
    assert_eq!(
        f.control
            .authority()
            .grants()
            .request(
                GrantScope::Perception {
                    display: "agent".into()
                },
                Duration::from_secs(60),
                &Yes
            )
            .unwrap_err(),
        AuthorityError::EvidenceUnavailable
    );
}

/// EL-1 §5 (mutant 6): a conflicting once-only record latches the store; a
/// gating record that conflicts is refused, and so is everything after it.
#[test]
fn el1_n3_a_conflict_latches_and_refuses_every_later_admission() {
    let f = fixture();
    let id = f.propose(nothing()).unwrap();
    // Another `prepared` record of the same commitment, another body.
    assert_eq!(
        f.evidence
            .store()
            .append("prepared", Some(&id.to_string()), "{\"other\":true}"),
        Append::Conflict
    );
    assert!(f.evidence.health().latched);
    assert_eq!(
        f.control.authorize(id, &f.agent, f.run, &Yes).unwrap_err(),
        AuthorityError::EvidenceUnavailable
    );
    assert_eq!(
        f.control
            .authority()
            .open_run(f.agent.clone(), RunOrigin::AgentGoal)
            .unwrap_err(),
        AuthorityError::EvidenceUnavailable
    );
}

/// EL-1 §6 (mutant 15): the emergency stop takes effect with a store that
/// refuses every write: every run is stopped, every unconsumed commitment
/// ended, the generation moved; only its records are owed.
#[test]
fn el1_n3_an_emergency_stop_with_a_failing_store_still_stops() {
    let f = fixture();
    let waiting = f.propose(nothing()).unwrap();
    let generation = f.control.authority().policy_generation();
    f.evidence.store().refuse_inserts(true);
    assert_eq!(f.control.emergency_stop(), 1);
    let authority = f.control.authority();
    assert!(authority.runs().is_stopped());
    assert!(authority.policy_generation() > generation);
    assert_eq!(
        authority.commitments().view(waiting).unwrap().state,
        CommitmentState::Revoked
    );
    assert_eq!(
        authority
            .open_run(f.agent.clone(), RunOrigin::AgentGoal)
            .unwrap_err(),
        AuthorityError::EmergencyStopped
    );
    // The revocation and the stop, owed.
    assert_eq!(f.evidence.health().owed, 2);
    assert!(!f.phases().contains(&"emergency_stop".to_string()));
    // Written, in order, once the store accepts again (the resume's record
    // is gating: it writes them first).
    f.evidence.store().refuse_inserts(false);
    authority.resume(&Yes).unwrap();
    let phases = f.phases();
    let tail: Vec<&str> = phases
        .iter()
        .rev()
        .take(3)
        .rev()
        .map(String::as_str)
        .collect();
    assert_eq!(tail, ["revoked", "emergency_stop", "resumed"]);
}

/// EL-1 §6 (mutant 15): the emergency stop does not wait for the store:
/// with the store's lock held (a slow write), every effect of the stop is
/// already in place; its records follow once the store answers.
#[test]
fn el1_n3_the_emergency_stop_does_not_wait_for_the_store() {
    let f = fixture();
    let waiting = f.propose(nothing()).unwrap();
    let generation = f.control.authority().policy_generation();
    let held = f.evidence.store().hold();
    let control = f.control.clone();
    let stopping = std::thread::spawn(move || control.emergency_stop());
    let authority = f.control.authority();
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    while !(authority.runs().is_stopped()
        && authority.policy_generation() > generation
        && authority.commitments().view(waiting).unwrap().state == CommitmentState::Revoked)
    {
        assert!(
            std::time::Instant::now() < deadline,
            "the stop waited for the store"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(!stopping.is_finished(), "its records wait for the store");
    drop(held);
    assert_eq!(stopping.join().unwrap(), 1);
    let phases = f.phases();
    assert!(phases.ends_with(&["revoked".to_string(), "emergency_stop".to_string()]));
}

/// EL-1 §6 (mutant 3): a projection is written only after the store
/// committed: a refused record leaves none; a projection that fails
/// changes nothing.
#[test]
fn el1_n3_projections_are_written_only_after_the_store_commits() {
    let scratch = Scratch::new();
    let copy = Arc::new(MemoryEvidence::new(100));
    let failing = Arc::new(MemoryEvidence::new(100));
    failing.fail_from_now();
    let evidence =
        PrimaryEvidence::open(&scratch.store(), vec![copy.clone(), failing.clone()]).unwrap();
    let record = |phase| EvidenceRecord::new(phase, 1, 1);
    evidence.store().refuse_inserts(true);
    assert!(evidence.record(&record(EvidencePhase::RunOpened)).is_err());
    assert_eq!(
        evidence.record_end(&record(EvidencePhase::RunCancelled)),
        EndStatus::Owed
    );
    assert!(copy.records().is_empty(), "no copy of what was not written");
    evidence.store().refuse_inserts(false);
    let seq = evidence
        .record(&record(EvidencePhase::RunOpened))
        .expect("a failing projection changes nothing");
    assert_eq!(
        seq, 3,
        "the session's opening, the owed record, then this one"
    );
    let copied: Vec<EvidencePhase> = copy.records().iter().map(|r| r.phase).collect();
    assert_eq!(
        copied,
        [EvidencePhase::RunCancelled, EvidencePhase::RunOpened]
    );
    assert!(failing.records().is_empty());
}

/// EL-1 §6: the owed list holds at least as many records as commitments can
/// be live at once; one more latches the store, and that record is
/// uncertain.
#[test]
fn el1_n3_the_owed_list_is_bounded_and_its_overflow_latches() {
    const { assert!(OWED_CAPACITY >= crate::authority::commitment::CAPACITY) };
    let scratch = Scratch::new();
    let evidence = PrimaryEvidence::open(&scratch.store(), vec![]).unwrap();
    evidence.store().refuse_inserts(true);
    let record = EvidenceRecord::new(EvidencePhase::RunCancelled, 1, 1);
    for _ in 0..OWED_CAPACITY {
        assert_eq!(evidence.record_end(&record), EndStatus::Owed);
    }
    assert!(!evidence.health().latched);
    assert_eq!(evidence.record_end(&record), EndStatus::Uncertain);
    assert!(evidence.health().latched);
    assert_eq!(evidence.health().owed, OWED_CAPACITY);
}

// ── P3-C11-M6: corrections to EL-1 (C-1, C-2) ──────────────────────────────

/// The fake effect's preparation over `grant` (as `Fixture::propose` builds
/// it), for an effect of the test's own.
fn preparation(grant: GrantId, effect: Box<dyn PendingEffect>) -> Preparation {
    Preparation {
        action: PreparedAction {
            kind: CapabilityKind::Perception,
            class: EffectClass::R1,
            operation: "fake.observe",
            target: TargetIdentity {
                display: "fake target".into(),
                digest: target(),
            },
            parameters: parameters(),
            grants: vec![grant],
            leases: vec![],
            summary: vec!["fake".into()],
        },
        effect,
        ttl: Duration::from_secs(60),
    }
}

/// Propose, authorize and execute one fake effect through `control` (an
/// effect run while another one executes).
fn run_effect(
    control: &Control,
    evidence: &Arc<PrimaryEvidence>,
    (agent, run, grant): (&AgentId, RunId, GrantId),
    during: During,
) -> Result<EffectOutput, AuthorityError> {
    let effect = Box::new(Effect {
        evidence: evidence.clone(),
        during,
    });
    let id = control.propose(agent, run, preparation(grant, effect))?.id;
    control.authorize(id, agent, run, &Yes)?;
    control.execute(id, agent, run)
}

/// A fake capability that records whether it was ever invoked.
struct Invoked(Arc<AtomicBool>);

impl PendingEffect for Invoked {
    fn revalidate(&self) -> Result<Digest, AuthorityError> {
        Ok(target())
    }

    fn parameters(&self) -> Digest {
        parameters()
    }

    fn execute(
        self: Box<Self>,
        _guard: &ExecutionGuard,
    ) -> Result<EffectOutput, (FailureClass, String)> {
        self.0.store(true, Ordering::SeqCst);
        Ok(EffectOutput::default())
    }
}

/// A projection that runs its task once, on the first start record it is
/// given: that record has committed (projections follow the store), and
/// `begin` has not yet completed the start. No sleep: the interleaving is
/// the call order.
struct OnStart(Mutex<Option<Box<dyn FnOnce() + Send>>>);

impl EvidenceProjection for OnStart {
    fn project(&self, record: &EvidenceRecord) {
        if record.phase == EvidencePhase::Started {
            // Taken first: the task's own records come back here.
            let task = self.0.lock().unwrap().take();
            if let Some(task) = task {
                task();
            }
        }
    }
}

/// The rows of `commitment` in `phase`.
fn rows_of<'a>(rows: &'a [Row], phase: &str, commitment: &str) -> Vec<&'a Row> {
    rows.iter()
        .filter(|row| row.phase == phase && row.commitment.as_deref() == Some(commitment))
        .collect()
}

/// EL-1 §7 as amended by C-1 (P3-C11-M6): an effect whose start record has
/// committed, overtaken by an emergency stop before the start completes,
/// never runs; its commitment ends revoked (interrupted at its start), and
/// that end counts: the next session holds no `unsettled` statement for it,
/// requires no recovery and does not start stopped.
#[test]
fn el1_m6_c1_a_stop_after_a_recorded_start_ends_the_commitment_and_nothing_is_unsettled() {
    let on_start = Arc::new(OnStart(Mutex::new(None)));
    let f = fixture_with(vec![on_start.clone()]);
    let control = Arc::downgrade(&f.control);
    *on_start.0.lock().unwrap() = Some(Box::new(move || {
        let control = control.upgrade().expect("the control");
        assert_eq!(control.emergency_stop(), 1);
    }));
    let invoked = Arc::new(AtomicBool::new(false));
    let id = f
        .control
        .propose(
            &f.agent,
            f.run,
            preparation(f.grant, Box::new(Invoked(invoked.clone()))),
        )
        .unwrap()
        .id;
    f.control.authorize(id, &f.agent, f.run, &Yes).unwrap();
    assert_eq!(
        f.control.execute(id, &f.agent, f.run).unwrap_err(),
        AuthorityError::EmergencyStopped
    );
    assert!(on_start.0.lock().unwrap().is_none(), "the stop ran");
    assert!(
        !invoked.load(Ordering::SeqCst),
        "the capability was invoked"
    );
    let id = id.to_string();
    let rows = f.rows();
    assert_eq!(rows_of(&rows, "started", &id).len(), 1);
    let revoked = rows_of(&rows, "revoked", &id);
    assert_eq!(revoked.len(), 1);
    let body: serde_json::Value = serde_json::from_str(&revoked[0].body).unwrap();
    assert_eq!(body["detail"]["interrupted"], "start");
    assert!(rows_of(&rows, "finished", &id).is_empty());
    // A new session over the same file.
    let Fixture {
        _scratch: scratch,
        evidence,
        control,
        ..
    } = f;
    let path = scratch.store();
    drop(control);
    drop(evidence);
    let evidence = PrimaryEvidence::open(&path, vec![]).unwrap();
    let unsettled: Vec<Row> = evidence
        .store()
        .rows()
        .into_iter()
        .filter(|row| row.phase == "unsettled")
        .collect();
    assert!(
        unsettled.is_empty(),
        "a start that a stop overtook is not unsettled: {unsettled:?}"
    );
    assert!(!evidence.health().recovery_required);
    let governed = crate::governed::GovernedControl::with_secrets(
        &scratch.path("runtime"),
        crate::broker::Vault::Disabled.source(),
        evidence.clone(),
        Arc::new(ManualClock::default()),
    )
    .unwrap();
    assert!(!governed.status().emergency_stopped);
}

/// C-1's premise (P3-C11-M6): an executing commitment is never ended by
/// what ends an unconsumed one (a denial, a refused start, expiry, a revoked
/// grant, a moved generation, a cancelled or finished run, an emergency
/// stop); only its finish ends it, with `finished`.
#[test]
fn el1_m6_c1_an_executing_commitment_is_ended_only_by_its_finish() {
    let f = fixture();
    let id = f.propose(nothing()).unwrap();
    f.control.authorize(id, &f.agent, f.run, &Yes).unwrap();
    let authority = f.control.authority();
    let commitments = authority.commitments();
    let guard = commitments
        .begin(id, &f.agent, f.run, &target(), &parameters())
        .unwrap();
    assert_eq!(
        commitments.deny(id, &f.agent, f.run).unwrap_err(),
        AuthorityError::NotPending
    );
    assert_eq!(
        commitments
            .fail_unstarted(id, &f.agent, f.run, &AuthorityError::Stale)
            .unwrap_err(),
        AuthorityError::NotPending
    );
    f.clock.advance(Duration::from_secs(3600));
    commitments.sweep();
    authority.grants().revoke(f.grant).unwrap();
    commitments.sweep();
    authority.cancel_run(f.run).unwrap();
    authority.finish_run(f.run);
    f.control.emergency_stop();
    assert_eq!(
        commitments.view(id).unwrap().state,
        CommitmentState::Executing
    );
    let id_text = id.to_string();
    let ends = |rows: &[Row]| -> Vec<String> {
        ["finished", "revoked", "expired", "denied"]
            .iter()
            .flat_map(|phase| rows_of(rows, phase, &id_text))
            .map(|row| row.phase.clone())
            .collect()
    };
    assert!(ends(&f.rows()).is_empty(), "{:?}", ends(&f.rows()));
    assert!(matches!(
        guard.finish(Outcome::Succeeded { meta: vec![] }),
        EndStatus::Recorded { .. }
    ));
    assert_eq!(ends(&f.rows()), ["finished"]);
    assert_eq!(
        commitments.view(id).unwrap().state,
        CommitmentState::Succeeded
    );
}

/// EL-1 §6 as amended by C-2 (P3-C11-M6): a latched store keeps nothing
/// owed. An effect that completes after the store latched (another effect's
/// end record was uncertain: its COMMIT failed) reports its end uncertain,
/// and the owed list is unchanged; a clean shutdown's `session_closed` on the
/// latched store is uncertain too.
#[test]
fn el1_m6_c2_a_latched_store_keeps_nothing_owed() {
    let f = fixture();
    let control = Arc::downgrade(&f.control);
    let evidence = f.evidence.clone();
    let (agent, run, grant) = (f.agent.clone(), f.run, f.grant);
    let inner = Arc::new(Mutex::new(None));
    let seen = inner.clone();
    // While this effect runs, another one runs whole and its end record's
    // COMMIT fails: the store latches.
    let latch_while_running: During = Arc::new(move |_| {
        let control = control.upgrade().expect("the control");
        let fail_commit: During =
            Arc::new(|evidence: &PrimaryEvidence| evidence.store().plant_fault(Fault::FailCommit));
        let result = run_effect(&control, &evidence, (&agent, run, grant), fail_commit);
        *seen.lock().unwrap() = Some(result);
    });
    let output = f.run(latch_while_running).expect("the effect completed");
    let inner = inner
        .lock()
        .unwrap()
        .take()
        .expect("the inner effect ran")
        .expect("the inner effect completed");
    assert_eq!(inner.evidence.map(|e| e.end), Some(EndStatus::Uncertain));
    assert!(f.evidence.health().latched);
    assert_eq!(
        output.evidence.map(|e| e.end),
        Some(EndStatus::Uncertain),
        "a latched store keeps nothing owed"
    );
    assert_eq!(f.evidence.health().owed, 0);
    assert!(f.evidence.store().owed_bodies().is_empty());
    assert_eq!(f.evidence.close_session(), EndStatus::Uncertain);
    assert_eq!(f.evidence.health().owed, 0);
}

// ── P3-C11-M6: corrections to EL-1 (C-5, C-6) ──────────────────────────────

/// The owner answers every dialog `.1`, and each one asked is counted.
struct Counting(std::sync::atomic::AtomicU32, bool);

impl Counting {
    fn answering(answer: bool) -> Self {
        Self(std::sync::atomic::AtomicU32::new(0), answer)
    }

    fn asked(&self) -> u32 {
        self.0.load(Ordering::SeqCst)
    }

    fn ask(&self) -> bool {
        self.0.fetch_add(1, Ordering::SeqCst);
        self.1
    }
}

impl ControlConfirmer for Counting {
    fn confirm_action(&self, _: &ActionConfirmation) -> bool {
        self.ask()
    }
    fn confirm_grant(&self, _: &GrantConfirmation) -> bool {
        self.ask()
    }
    fn confirm_resume(&self, _: &ResumeConfirmation) -> bool {
        self.ask()
    }
    fn confirm_acknowledgement(
        &self,
        _: &crate::authority::approval::AcknowledgementConfirmation,
    ) -> bool {
        self.ask()
    }
}

/// EL-1 §6 as amended by C-5 (P3-C11-M6): the owner's decline of a grant
/// has taken effect and permits nothing: its record is an end record. With
/// the store refusing every insert the request is still declined, and its
/// `grant_declined` record is owed.
#[test]
fn el1_m6_c5_a_declined_grant_is_an_end_record() {
    let f = fixture();
    f.evidence.store().refuse_inserts(true);
    let owner = Counting::answering(false);
    assert_eq!(
        f.control
            .authority()
            .grants()
            .request(
                GrantScope::Perception {
                    display: "agent".into()
                },
                Duration::from_secs(60),
                &owner
            )
            .unwrap_err(),
        AuthorityError::Declined
    );
    assert_eq!(owner.asked(), 1);
    assert_eq!(f.evidence.health().owed, 1);
    let owed: serde_json::Value =
        serde_json::from_str(&f.evidence.store().owed_bodies()[0]).unwrap();
    assert_eq!(owed["phase"], "grant_declined");
}

/// EL-1 §7 as amended by C-6 (P3-C11-M6): on a latched store the resume
/// refuses before any dialog (it could record nothing): the owner is asked
/// nothing, the stop stays, nothing is written.
#[test]
fn el1_m6_c6_resume_on_a_latched_store_asks_nothing() {
    let f = fixture();
    let id = f.propose(nothing()).unwrap();
    assert_eq!(f.control.emergency_stop(), 1);
    assert_eq!(
        f.evidence
            .store()
            .append("prepared", Some(&id.to_string()), "{\"other\":true}"),
        Append::Conflict
    );
    assert!(f.evidence.health().latched);
    let rows = f.rows();
    let owner = Counting::answering(true);
    assert_eq!(
        f.control.authority().resume(&owner).unwrap_err(),
        AuthorityError::EvidenceUnavailable
    );
    assert_eq!(owner.asked(), 0, "a dialog was shown on a latched store");
    assert!(f.control.authority().runs().is_stopped());
    assert_eq!(f.rows(), rows);
}

// ── P3-C11-M6 C-9: results, and transitions that have taken effect ─────────

/// A fake effect that fails, the store refusing every insert from the
/// moment it runs: its start record is written, its end record is owed.
struct FailsWhileFull(Arc<PrimaryEvidence>);

impl PendingEffect for FailsWhileFull {
    fn revalidate(&self) -> Result<Digest, AuthorityError> {
        Ok(target())
    }

    fn parameters(&self) -> Digest {
        parameters()
    }

    fn execute(
        self: Box<Self>,
        _guard: &ExecutionGuard,
    ) -> Result<EffectOutput, (FailureClass, String)> {
        self.0.store().refuse_inserts(true);
        Err((FailureClass::Actuator, "it failed".into()))
    }
}

/// Run one failing effect whose end record is owed; its error, and the
/// sequence number of its start record.
fn failed_with_its_end_owed(f: &Fixture) -> (AuthorityError, u64) {
    let id = f
        .control
        .propose(
            &f.agent,
            f.run,
            preparation(f.grant, Box::new(FailsWhileFull(f.evidence.clone()))),
        )
        .unwrap()
        .id;
    f.control.authorize(id, &f.agent, f.run, &Yes).unwrap();
    let error = f.control.execute(id, &f.agent, f.run).unwrap_err();
    let start = rows_of(&f.rows(), "started", &id.to_string())[0].seq;
    (error, start)
}

/// EL-1 §8 (C-9 M-B1): an effect that started and failed is `Ended`, and its
/// evidence carries what became of its end record: owed here.
#[test]
fn el1_m6_mb1_a_failed_effect_whose_end_is_owed_reports_it_owed() {
    let f = fixture();
    let (error, start) = failed_with_its_end_owed(&f);
    let AuthorityError::Ended { cause, evidence } = error else {
        panic!("not ended: {error:?}");
    };
    assert_eq!(*cause, AuthorityError::Unavailable("the effect failed"));
    assert_eq!(
        evidence,
        ExecutionEvidence {
            start,
            end: EndStatus::Owed
        }
    );
    assert_eq!(f.evidence.health().owed, 1);
}

/// EL-1 §8 (C-9 M-B2): an `Ended` error reads as its cause, then its
/// evidence in parentheses.
#[test]
fn el1_m6_mb2_an_ended_error_reads_as_its_cause_then_its_evidence() {
    let f = fixture();
    let (error, start) = failed_with_its_end_owed(&f);
    let AuthorityError::Ended { cause, evidence } = &error else {
        panic!("not ended: {error:?}");
    };
    assert_eq!(error.to_string(), format!("{cause} ({evidence})"));
    assert_eq!(
        error.to_string(),
        format!(
            "governed control: unavailable: the effect failed \
             (evidence: start recorded {start}, end owed)"
        )
    );
}

/// EL-1 §6 (C-9 M-B7): a run's cancellation has taken effect: with the
/// store refusing every insert it returns Ok, the run is cancelled, and its
/// `run_cancelled` record is owed.
#[test]
fn el1_m6_mb7_a_cancellation_with_a_refusing_store_takes_effect_and_is_owed() {
    let f = fixture();
    f.evidence.store().refuse_inserts(true);
    assert_eq!(f.control.authority().cancel_run(f.run), Ok(()));
    assert!(f.control.authority().runs().view(f.run).unwrap().cancelled);
    let owed: Vec<serde_json::Value> = f
        .evidence
        .store()
        .owed_bodies()
        .iter()
        .map(|body| serde_json::from_str(body).unwrap())
        .collect();
    assert_eq!(owed.len(), 1);
    assert_eq!(owed[0]["phase"], "run_cancelled");
}

/// EL-1 §6 (C-9 M-B8): a grant's revocation has taken effect: with the
/// store refusing every insert it returns Ok, the grant is revoked (the
/// generation moved), and its `grant_revoked` record is owed.
#[test]
fn el1_m6_mb8_a_revocation_with_a_refusing_store_takes_effect_and_is_owed() {
    let f = fixture();
    let generation = f.control.authority().policy_generation();
    f.evidence.store().refuse_inserts(true);
    assert_eq!(f.control.authority().grants().revoke(f.grant), Ok(()));
    assert!(f.control.authority().grants().live(f.grant).is_none());
    assert!(f.control.authority().policy_generation() > generation);
    let owed: Vec<serde_json::Value> = f
        .evidence
        .store()
        .owed_bodies()
        .iter()
        .map(|body| serde_json::from_str(body).unwrap())
        .collect();
    assert_eq!(owed.len(), 1);
    assert_eq!(owed[0]["phase"], "grant_revoked");
}
