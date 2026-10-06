//! The pipeline, driven with a fake domain whose effect counts how often it
//! really ran.

use super::{Control, EffectOutput, PendingEffect, Preparation};
use crate::authority::approval::{
    ActionConfirmation, ControlConfirmer, GrantConfirmation, ResumeConfirmation,
};
use crate::authority::clock::ManualClock;
use crate::authority::commitment::{
    CommitmentState, ExecutionGuard, FailureClass, PreparedAction, TargetIdentity,
};
use crate::authority::effect::{CapabilityKind, EffectClass};
use crate::authority::evidence::MemoryEvidence;
use crate::authority::ids::{AgentId, Digest, GrantId, RunId};
use crate::authority::policy::GrantScope;
use crate::authority::run::RunOrigin;
use crate::authority::{Authority, AuthorityError};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;

struct Yes(bool);
impl ControlConfirmer for Yes {
    fn confirm_action(&self, _: &ActionConfirmation) -> bool {
        self.0
    }
    fn confirm_grant(&self, _: &GrantConfirmation) -> bool {
        self.0
    }
    fn confirm_resume(&self, _: &ResumeConfirmation) -> bool {
        self.0
    }
}

/// The fake world the effect acts on.
#[derive(Default)]
struct World {
    ran: AtomicU32,
    /// The target identity revalidation reports.
    moved: AtomicBool,
    /// Make the effect loop until cancelled.
    long: AtomicBool,
}

struct Effect {
    world: Arc<World>,
    parameters: String,
}

fn target_digest(moved: bool) -> Digest {
    Digest::of("fake.target", &[if moved { b"moved" } else { b"here" }])
}

impl PendingEffect for Effect {
    fn revalidate(&self) -> Result<Digest, AuthorityError> {
        Ok(target_digest(self.world.moved.load(Ordering::SeqCst)))
    }

    fn parameters(&self) -> Digest {
        Digest::of("fake.params", &[self.parameters.as_bytes()])
    }

    fn execute(
        self: Box<Self>,
        guard: &ExecutionGuard,
    ) -> Result<EffectOutput, (FailureClass, String)> {
        if self.world.long.load(Ordering::SeqCst) {
            for _ in 0..1000 {
                if guard.is_cancelled() {
                    return Err((FailureClass::Actuator, "cancelled".into()));
                }
                std::thread::sleep(Duration::from_millis(2));
            }
            return Err((FailureClass::Timeout, "never cancelled".into()));
        }
        self.world.ran.fetch_add(1, Ordering::SeqCst);
        Ok(EffectOutput {
            text: Some(format!("did {}", self.parameters)),
            ..Default::default()
        })
    }
}

struct Fixture {
    control: Arc<Control>,
    clock: Arc<ManualClock>,
    world: Arc<World>,
    agent: AgentId,
    run: RunId,
    grant: GrantId,
}

fn fixture() -> Fixture {
    let evidence = Arc::new(MemoryEvidence::new(10_000));
    let clock = Arc::new(ManualClock::default());
    let control = Arc::new(Control::new(Authority::new(evidence, clock.clone())));
    let agent = AgentId::new("agent-a").unwrap();
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
            &Yes(true),
        )
        .unwrap();
    Fixture {
        control,
        clock,
        world: Arc::new(World::default()),
        agent,
        run,
        grant,
    }
}

impl Fixture {
    fn preparation(&self, class: EffectClass, parameters: &str, committed: &str) -> Preparation {
        Preparation {
            action: PreparedAction {
                kind: CapabilityKind::Perception,
                class,
                operation: "fake.observe",
                target: TargetIdentity {
                    display: "fake target".into(),
                    digest: target_digest(false),
                },
                parameters: Digest::of("fake.params", &[committed.as_bytes()]),
                grants: vec![self.grant],
                leases: vec![],
                summary: vec![format!("fake {parameters}")],
            },
            effect: Box::new(Effect {
                world: self.world.clone(),
                parameters: parameters.into(),
            }),
            ttl: Duration::from_secs(60),
        }
    }

    fn propose(&self, class: EffectClass) -> crate::authority::ids::CommitmentId {
        self.control
            .propose(&self.agent, self.run, self.preparation(class, "x", "x"))
            .unwrap()
            .id
    }

    fn propose_effect(
        &self,
        effect: Box<dyn PendingEffect>,
    ) -> crate::authority::ids::CommitmentId {
        let mut preparation = self.preparation(EffectClass::R1, "x", "x");
        preparation.effect = effect;
        self.control
            .propose(&self.agent, self.run, preparation)
            .unwrap()
            .id
    }

    fn state(&self, id: crate::authority::ids::CommitmentId) -> CommitmentState {
        self.control
            .authority()
            .commitments()
            .view(id)
            .unwrap()
            .state
    }
}

#[test]
fn the_pipeline_runs_an_r0_effect_once() {
    let f = fixture();
    let early = f.propose(EffectClass::R0);
    // Not before authorization; an execute attempt is one-shot, so the
    // refused commitment ends instead of waiting without its effect.
    assert_eq!(
        f.control.execute(early, &f.agent, f.run).unwrap_err(),
        AuthorityError::NotAuthorized
    );
    assert_eq!(f.world.ran.load(Ordering::SeqCst), 0);
    assert_eq!(f.state(early), CommitmentState::Failed);
    let id = f.propose(EffectClass::R0);
    f.control
        .authorize(id, &f.agent, f.run, &Yes(false))
        .unwrap();
    let out = f.control.execute(id, &f.agent, f.run).unwrap();
    assert_eq!(out.text.as_deref(), Some("did x"));
    assert_eq!(f.world.ran.load(Ordering::SeqCst), 1);
    assert_eq!(f.state(id), CommitmentState::Succeeded);
    // Never twice.
    assert_eq!(
        f.control.execute(id, &f.agent, f.run).unwrap_err(),
        AuthorityError::UnknownCommitment
    );
    assert_eq!(f.world.ran.load(Ordering::SeqCst), 1);
}

#[test]
fn r2_runs_only_after_the_native_approval() {
    let f = fixture();
    let declined = f.propose(EffectClass::R2);
    assert_eq!(
        f.control
            .authorize(declined, &f.agent, f.run, &Yes(false))
            .unwrap_err(),
        AuthorityError::Declined
    );
    assert!(f.control.execute(declined, &f.agent, f.run).is_err());
    let approved = f.propose(EffectClass::R2);
    f.control
        .authorize(approved, &f.agent, f.run, &Yes(true))
        .unwrap();
    f.control.execute(approved, &f.agent, f.run).unwrap();
    assert_eq!(f.world.ran.load(Ordering::SeqCst), 1);
}

#[test]
fn a_moved_target_fails_the_effect_before_it_happens() {
    let f = fixture();
    let id = f.propose(EffectClass::R1);
    f.control
        .authorize(id, &f.agent, f.run, &Yes(true))
        .unwrap();
    f.world.moved.store(true, Ordering::SeqCst);
    assert_eq!(
        f.control.execute(id, &f.agent, f.run).unwrap_err(),
        AuthorityError::TargetChanged
    );
    assert_eq!(f.world.ran.load(Ordering::SeqCst), 0);
    assert_eq!(f.state(id), CommitmentState::Failed);
}

#[test]
fn an_effect_whose_parameters_differ_from_the_commitment_is_refused() {
    let f = fixture();
    // The domain claims "x" but would do "y".
    assert!(matches!(
        f.control
            .propose(&f.agent, f.run, f.preparation(EffectClass::R1, "y", "x"))
            .unwrap_err(),
        AuthorityError::InvalidAction(_)
    ));
    assert_eq!(f.world.ran.load(Ordering::SeqCst), 0);
}

#[test]
fn another_agent_or_run_cannot_execute_a_pending_effect() {
    let f = fixture();
    let id = f.propose(EffectClass::R1);
    f.control
        .authorize(id, &f.agent, f.run, &Yes(true))
        .unwrap();
    let intruder = AgentId::new("intruder").unwrap();
    assert_eq!(
        f.control.execute(id, &intruder, f.run).unwrap_err(),
        AuthorityError::WrongAgent
    );
    let other_run = f
        .control
        .authority()
        .open_run(f.agent.clone(), RunOrigin::AgentGoal)
        .unwrap();
    assert_eq!(
        f.control.execute(id, &f.agent, other_run).unwrap_err(),
        AuthorityError::WrongRun
    );
    // The rightful owner still can.
    f.control.execute(id, &f.agent, f.run).unwrap();
    assert_eq!(f.world.ran.load(Ordering::SeqCst), 1);
}

#[test]
fn cancelling_during_a_bounded_effect_stops_it_and_reports_cancellation() {
    let f = fixture();
    f.world.long.store(true, Ordering::SeqCst);
    let id = f.propose(EffectClass::R1);
    f.control
        .authorize(id, &f.agent, f.run, &Yes(true))
        .unwrap();
    let control = f.control.clone();
    let (agent, run) = (f.agent.clone(), f.run);
    let worker = std::thread::spawn(move || control.execute(id, &agent, run));
    std::thread::sleep(Duration::from_millis(30));
    f.control.cancel_run(f.run).unwrap();
    assert_eq!(
        worker.join().unwrap().unwrap_err(),
        AuthorityError::RunCancelled
    );
    assert_eq!(f.state(id), CommitmentState::Cancelled);
}

#[test]
fn an_effect_that_completes_while_its_run_is_cancelled_is_recorded_as_done() {
    let f = fixture();
    let id = f.propose(EffectClass::R1);
    f.control
        .authorize(id, &f.agent, f.run, &Yes(true))
        .unwrap();
    // The effect's last step is in flight when the owner cancels.
    struct Racing(Arc<Control>, RunId);
    impl PendingEffect for Racing {
        fn revalidate(&self) -> Result<Digest, AuthorityError> {
            Ok(target_digest(false))
        }
        fn parameters(&self) -> Digest {
            Digest::of("fake.params", &[b"x"])
        }
        fn execute(
            self: Box<Self>,
            _: &ExecutionGuard,
        ) -> Result<EffectOutput, (FailureClass, String)> {
            self.0.cancel_run(self.1).unwrap();
            Ok(EffectOutput::default())
        }
    }
    let racing = f.propose_effect(Box::new(Racing(f.control.clone(), f.run)));
    f.control
        .authorize(racing, &f.agent, f.run, &Yes(true))
        .unwrap();
    assert!(f.control.execute(racing, &f.agent, f.run).is_ok());
    assert_eq!(f.state(racing), CommitmentState::Succeeded);
    // The other pending commitment of the cancelled run ended unconsumed.
    assert_eq!(f.state(id), CommitmentState::Revoked);
    assert!(f.control.execute(id, &f.agent, f.run).is_err());
    assert_eq!(f.world.ran.load(Ordering::SeqCst), 0);
}

#[test]
fn ended_commitments_do_not_hold_pending_effects() {
    let f = fixture();
    for _ in 0..super::MAX_PENDING_PER_RUN {
        f.propose(EffectClass::R1);
    }
    // A run full of live commitments: refused.
    assert_eq!(
        f.control
            .propose(&f.agent, f.run, f.preparation(EffectClass::R1, "x", "x"))
            .unwrap_err(),
        AuthorityError::Capacity
    );
    // Once they expire, the pipeline reclaims their effects.
    f.clock.advance(Duration::from_secs(61));
    f.propose(EffectClass::R1);
}

/// Agents together cannot take the places kept for the owner's own
/// commands: with every other place held by agents, an agent is refused
/// and the owner is not.
#[test]
fn agents_cannot_fill_the_places_kept_for_the_owner() {
    let f = fixture();
    let agents_limit = super::MAX_PENDING - super::OWNER_RESERVE;
    let mut held = 0;
    while held < agents_limit {
        let run = f
            .control
            .authority()
            .open_run(f.agent.clone(), RunOrigin::AgentGoal)
            .unwrap();
        for _ in 0..super::MAX_PENDING_PER_RUN.min(agents_limit - held) {
            f.control
                .propose(&f.agent, run, f.preparation(EffectClass::R1, "x", "x"))
                .unwrap();
            held += 1;
        }
    }
    let run = f
        .control
        .authority()
        .open_run(f.agent.clone(), RunOrigin::AgentGoal)
        .unwrap();
    assert_eq!(
        f.control
            .propose(&f.agent, run, f.preparation(EffectClass::R1, "x", "x"))
            .unwrap_err(),
        AuthorityError::Capacity
    );
    let owner = AgentId::owner_session();
    let run = f
        .control
        .authority()
        .open_run(owner.clone(), RunOrigin::Command { modalities: vec![] })
        .unwrap();
    assert!(f
        .control
        .propose(&owner, run, f.preparation(EffectClass::R1, "x", "x"))
        .is_ok());
}

#[test]
fn an_emergency_stop_drops_every_pending_effect() {
    let f = fixture();
    let id = f.propose(EffectClass::R1);
    f.control
        .authorize(id, &f.agent, f.run, &Yes(true))
        .unwrap();
    f.control.emergency_stop();
    assert!(f.control.execute(id, &f.agent, f.run).is_err());
    assert_eq!(f.world.ran.load(Ordering::SeqCst), 0);
}
