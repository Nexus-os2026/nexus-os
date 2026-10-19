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
    CommitmentState, ExecutionGuard, FailureClass, PreparedAction, TargetIdentity,
};
use crate::authority::effect::{CapabilityKind, EffectClass};
use crate::authority::evidence::{EvidencePhase, ExecutionEvidence, MemoryEvidence};
use crate::authority::ids::{AgentId, CommitmentId, Digest, GrantId, RunId};
use crate::authority::policy::GrantScope;
use crate::authority::run::RunOrigin;
use crate::authority::{Authority, AuthorityError};
use crate::control::{Control, EffectOutput, PendingEffect, Preparation};
use std::sync::Arc;
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
    agent: AgentId,
    run: RunId,
    grant: GrantId,
}

fn fixture() -> Fixture {
    let scratch = Scratch::new();
    let recent = Arc::new(MemoryEvidence::new(10_000));
    let evidence = PrimaryEvidence::open(&scratch.store(), vec![recent.clone()]).unwrap();
    let control = Arc::new(Control::new(Authority::new(
        evidence.clone(),
        Arc::new(ManualClock::default()),
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
    // The store accepts again, but the record is still owed: nothing new.
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
