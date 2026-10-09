//! The broker, attacked directly: a lease releases its secret once, only to
//! the commitment that lists it, only for its origin, and ends with it.

use super::{CredentialBroker, CredentialSpec, Placement, SecretSource, SecretUnavailable};
use crate::authority::commitment::{CommitmentState, PreparedAction, TargetIdentity};
use crate::authority::effect::{CapabilityKind, EffectClass};
use crate::authority::ids::Digest;
use crate::authority::policy::GrantScope;
use crate::authority::AuthorityError;
use crate::control::{EffectOutput, PendingEffect, Preparation};
use crate::egress::destination::Destination;
use crate::egress::ReleaseCredential;
use crate::harness_tests::{harness, Harness, Yes};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use zeroize::Zeroizing;

pub(crate) const SECRET: &str = "tok-SECRET-0123456789";

/// A vault holding one secret, counting reads.
pub(crate) struct FakeVault(pub AtomicU32);

impl SecretSource for FakeVault {
    fn read(&self, scope: &str, name: &str) -> Result<Zeroizing<String>, SecretUnavailable> {
        self.0.fetch_add(1, Ordering::SeqCst);
        if scope == "http" && name == "fixture.token" {
            Ok(Zeroizing::new(SECRET.to_string()))
        } else {
            Err(SecretUnavailable::NotFound)
        }
    }
}

pub(crate) const SPEC: CredentialSpec = CredentialSpec {
    service: "Fixture",
    scope: "http",
    name: "fixture.token",
    placement: Placement::Authorization("Bearer"),
};

/// An effect that asks the broker for its lease and reports what it got.
struct Releasing {
    broker: Arc<CredentialBroker>,
    lease: crate::authority::ids::LeaseId,
    destination: Destination,
    got: Arc<Mutex<Option<Result<String, AuthorityError>>>>,
}

impl PendingEffect for Releasing {
    fn revalidate(&self) -> Result<Digest, AuthorityError> {
        Ok(self.destination.origin_digest())
    }
    fn parameters(&self) -> Digest {
        Digest::of("broker.test", &[])
    }
    fn execute(
        self: Box<Self>,
        guard: &crate::authority::commitment::ExecutionGuard,
    ) -> Result<EffectOutput, (crate::authority::commitment::FailureClass, String)> {
        let result = self
            .broker
            .release(self.lease, guard, &self.destination)
            .map(|header| format!("{}: {}", header.name, *header.value));
        *self.got.lock().unwrap() = Some(result);
        Ok(EffectOutput::default())
    }
}

fn setup() -> (Harness, Arc<CredentialBroker>, Arc<FakeVault>, Destination) {
    let h = harness();
    let vault = Arc::new(FakeVault(AtomicU32::new(0)));
    let broker = CredentialBroker::new(h.control.authority(), vault.clone());
    let destination = Destination::parse("https://api.fixture.example").unwrap();
    (h, broker, vault, destination)
}

/// Commit to an action listing `lease` for `destination`, then run it.
fn use_lease(
    h: &Harness,
    broker: &Arc<CredentialBroker>,
    lease: crate::authority::ids::LeaseId,
    destination: &Destination,
) -> Option<Result<String, AuthorityError>> {
    let (preparation, got) = lease_preparation(h, broker, lease, destination);
    let view = h.control.propose(&h.agent, h.run, preparation).ok()?;
    h.control
        .authorize(view.id, &h.agent, h.run, &Yes::new(true))
        .ok()?;
    let _ = h.control.execute(view.id, &h.agent, h.run);
    let result = got.lock().unwrap().take();
    result
}

type Got = Arc<Mutex<Option<Result<String, AuthorityError>>>>;

/// An action listing `lease` for `destination`, and where it puts what the
/// broker gave it.
fn lease_preparation(
    h: &Harness,
    broker: &Arc<CredentialBroker>,
    lease: crate::authority::ids::LeaseId,
    destination: &Destination,
) -> (Preparation, Got) {
    let grant = h
        .control
        .authority()
        .grants()
        .request(
            GrantScope::Connector {
                connector: "fixture".into(),
                account: "test".into(),
                operations: vec!["broker.test".into()],
            },
            Duration::from_secs(60),
            &Yes::new(true),
        )
        .unwrap();
    let got = Arc::new(Mutex::new(None));
    let preparation = Preparation {
        action: PreparedAction {
            kind: CapabilityKind::Connector,
            class: EffectClass::R1,
            operation: "broker.test",
            target: TargetIdentity {
                display: destination.origin_text(),
                digest: destination.origin_digest(),
            },
            parameters: Digest::of("broker.test", &[]),
            grants: vec![grant],
            leases: vec![lease],
            summary: vec![],
        },
        effect: Box::new(Releasing {
            broker: broker.clone(),
            lease,
            destination: destination.clone(),
            got: got.clone(),
        }),
        ttl: Duration::from_secs(60),
    };
    (preparation, got)
}

/// No lock (the lease table's, the pipeline's or the authority's) is held
/// while a lease's issue or release is recorded, or while a lease ends with
/// its commitment; a sink that panics on an issue leaves no lock poisoned
/// and no room held.
#[test]
fn leases_are_issued_released_and_ended_with_no_lock_held_across_a_record() {
    use crate::authority::clock::SystemClock;
    use crate::authority::evidence::{EvidencePhase, MemoryEvidence};
    use crate::authority::ids::AgentId;
    use crate::authority::run::RunOrigin;
    use crate::authority::scripted::{within, ScriptedSink};
    use crate::authority::Authority;
    use crate::control::Control;
    use std::panic::{catch_unwind, AssertUnwindSafe};
    let sink = ScriptedSink::new();
    let control = Arc::new(Control::new(Authority::new(
        sink.clone(),
        Arc::new(SystemClock::default()),
    )));
    let agent = AgentId::new("agent-test").unwrap();
    let run = control
        .authority()
        .open_run(agent.clone(), RunOrigin::AgentGoal)
        .unwrap();
    let h = Harness {
        control,
        evidence: Arc::new(MemoryEvidence::new(1)),
        agent,
        run,
    };
    let vault = Arc::new(FakeVault(AtomicU32::new(0)));
    let broker = CredentialBroker::new(h.control.authority(), vault);
    let destination = Destination::parse("https://api.fixture.example").unwrap();
    let probe = {
        let (control, broker) = (Arc::downgrade(&h.control), Arc::downgrade(&broker));
        move || {
            if let Some(control) = control.upgrade() {
                control.probe_locks();
            }
            if let Some(broker) = broker.upgrade() {
                broker.probe_locks();
            }
        }
    };
    sink.probe_with(probe.clone());
    let ttl = Duration::from_secs(60);
    // Issued and released.
    let lease = broker
        .lease(&h.agent, h.run, SPEC, &destination, ttl)
        .unwrap();
    assert_eq!(
        use_lease(&h, &broker, lease, &destination)
            .unwrap()
            .unwrap(),
        format!("authorization: Bearer {SECRET}")
    );
    // Ended with its commitment.
    let ended = broker
        .lease(&h.agent, h.run, SPEC, &destination, ttl)
        .unwrap();
    let (preparation, _) = lease_preparation(&h, &broker, ended, &destination);
    let view = h.control.propose(&h.agent, h.run, preparation).unwrap();
    h.control.deny(view.id, &h.agent, h.run).unwrap();
    assert!(!broker.is_live(ended));
    // A sink that panics on an issue.
    sink.panic_on(Some(EvidencePhase::LeaseIssued));
    assert!(catch_unwind(AssertUnwindSafe(|| {
        broker.lease(&h.agent, h.run, SPEC, &destination, ttl)
    }))
    .is_err());
    sink.panic_on(None);
    within(probe);
    assert_eq!(broker.rooms_held(), 0);
    assert!(broker
        .lease(&h.agent, h.run, SPEC, &destination, ttl)
        .is_ok());
    let phases = sink.phases();
    assert!(phases.contains(&EvidencePhase::LeaseIssued));
    assert!(phases.contains(&EvidencePhase::CredentialReleased));
    assert!(phases.contains(&EvidencePhase::Denied));
    assert!(sink.probes() >= phases.len());
    assert_eq!(
        sink.violations(),
        0,
        "an authority lock was held while a record was written"
    );
}

#[test]
fn a_lease_releases_its_secret_once_to_its_commitment_only() {
    let (h, broker, vault, destination) = setup();
    let lease = broker
        .lease(&h.agent, h.run, SPEC, &destination, Duration::from_secs(60))
        .unwrap();
    assert!(broker.is_live(lease));
    assert_eq!(
        vault.0.load(Ordering::SeqCst),
        0,
        "nothing is read at lease time"
    );
    let first = use_lease(&h, &broker, lease, &destination)
        .unwrap()
        .unwrap();
    assert_eq!(first, format!("authorization: Bearer {SECRET}"));
    assert_eq!(vault.0.load(Ordering::SeqCst), 1);
    // A second commitment listing the same lease gets nothing.
    assert_eq!(
        use_lease(&h, &broker, lease, &destination)
            .unwrap()
            .unwrap_err(),
        AuthorityError::NotPending
    );
    assert_eq!(vault.0.load(Ordering::SeqCst), 1);
    assert!(!broker.is_live(lease));
    // The secret is in no evidence record.
    let records = h.evidence.records();
    assert!(!format!("{records:?}").contains(SECRET));
    assert!(records
        .iter()
        .any(|r| r.phase == crate::authority::evidence::EvidencePhase::CredentialReleased));
}

#[test]
fn a_lease_is_bound_to_its_origin_agent_and_run() {
    let (h, broker, vault, destination) = setup();
    // Another origin.
    let lease = broker
        .lease(&h.agent, h.run, SPEC, &destination, Duration::from_secs(60))
        .unwrap();
    let elsewhere = Destination::parse("https://elsewhere.example").unwrap();
    assert_eq!(
        use_lease(&h, &broker, lease, &elsewhere)
            .unwrap()
            .unwrap_err(),
        AuthorityError::TargetChanged
    );
    // Another run of the same agent.
    let other_run = h
        .control
        .authority()
        .open_run(h.agent.clone(), crate::authority::run::RunOrigin::AgentGoal)
        .unwrap();
    let foreign = broker
        .lease(
            &h.agent,
            other_run,
            SPEC,
            &destination,
            Duration::from_secs(60),
        )
        .unwrap();
    assert_eq!(
        use_lease(&h, &broker, foreign, &destination)
            .unwrap()
            .unwrap_err(),
        AuthorityError::WrongRun
    );
    // An action that does not list the lease cannot use it (an unknown id
    // names nothing either).
    let unknown =
        crate::authority::ids::LeaseId::parse("lease-00000000000000000000000000000000").unwrap();
    assert!(use_lease(&h, &broker, unknown, &destination)
        .unwrap()
        .is_err());
    assert_eq!(
        vault.0.load(Ordering::SeqCst),
        0,
        "no refused release read the vault"
    );
}

#[test]
fn a_lease_ends_with_its_commitment_its_run_or_its_expiry() {
    let (h, broker, _vault, destination) = setup();
    // Denied commitment.
    let lease = broker
        .lease(&h.agent, h.run, SPEC, &destination, Duration::from_secs(60))
        .unwrap();
    let grant = h
        .control
        .authority()
        .grants()
        .request(
            GrantScope::Connector {
                connector: "fixture".into(),
                account: "test".into(),
                operations: vec!["broker.test".into()],
            },
            Duration::from_secs(60),
            &Yes::new(true),
        )
        .unwrap();
    let got = Arc::new(Mutex::new(None));
    let view = h
        .control
        .propose(
            &h.agent,
            h.run,
            Preparation {
                action: PreparedAction {
                    kind: CapabilityKind::Connector,
                    class: EffectClass::R1,
                    operation: "broker.test",
                    target: TargetIdentity {
                        display: destination.origin_text(),
                        digest: destination.origin_digest(),
                    },
                    parameters: Digest::of("broker.test", &[]),
                    grants: vec![grant],
                    leases: vec![lease],
                    summary: vec![],
                },
                effect: Box::new(Releasing {
                    broker: broker.clone(),
                    lease,
                    destination: destination.clone(),
                    got,
                }),
                ttl: Duration::from_secs(60),
            },
        )
        .unwrap();
    h.control.deny(view.id, &h.agent, h.run).unwrap();
    assert_eq!(
        h.control
            .authority()
            .commitments()
            .view(view.id)
            .unwrap()
            .state,
        CommitmentState::Denied
    );
    assert!(!broker.is_live(lease), "a denied commitment ends its lease");
    // Cancelled run.
    let lease = broker
        .lease(&h.agent, h.run, SPEC, &destination, Duration::from_secs(60))
        .unwrap();
    h.control.cancel_run(h.run).unwrap();
    assert!(
        broker
            .lease(&h.agent, h.run, SPEC, &destination, Duration::from_secs(60))
            .is_err(),
        "a cancelled run gets no lease"
    );
    let _ = lease;
}

#[test]
fn without_a_vault_every_credential_is_unavailable() {
    let h = harness();
    let broker = CredentialBroker::new(h.control.authority(), Arc::new(super::NoVault));
    let destination = Destination::parse("https://api.fixture.example").unwrap();
    let lease = broker
        .lease(&h.agent, h.run, SPEC, &destination, Duration::from_secs(60))
        .unwrap();
    assert_eq!(
        use_lease(&h, &broker, lease, &destination)
            .unwrap()
            .unwrap_err(),
        AuthorityError::Unavailable("the credential is not available")
    );
}

/// A commitment releases only leases it lists: a lease issued for the same
/// agent, run and origin but bound to no commitment stays sealed.
#[test]
fn a_commitment_cannot_release_a_lease_it_does_not_list() {
    let (h, broker, vault, destination) = setup();
    let listed = broker
        .lease(&h.agent, h.run, SPEC, &destination, Duration::from_secs(60))
        .unwrap();
    let other = broker
        .lease(&h.agent, h.run, SPEC, &destination, Duration::from_secs(60))
        .unwrap();
    let grant = h
        .control
        .authority()
        .grants()
        .request(
            GrantScope::Connector {
                connector: "fixture".into(),
                account: "test".into(),
                operations: vec!["broker.test".into()],
            },
            Duration::from_secs(60),
            &Yes::new(true),
        )
        .unwrap();
    let got = Arc::new(Mutex::new(None));
    let view = h
        .control
        .propose(
            &h.agent,
            h.run,
            Preparation {
                action: PreparedAction {
                    kind: CapabilityKind::Connector,
                    class: EffectClass::R1,
                    operation: "broker.test",
                    target: TargetIdentity {
                        display: destination.origin_text(),
                        digest: destination.origin_digest(),
                    },
                    parameters: Digest::of("broker.test", &[]),
                    grants: vec![grant],
                    // The commitment lists one lease ...
                    leases: vec![listed],
                    summary: vec![],
                },
                // ... and its effect asks for the other.
                effect: Box::new(Releasing {
                    broker: broker.clone(),
                    lease: other,
                    destination: destination.clone(),
                    got: got.clone(),
                }),
                ttl: Duration::from_secs(60),
            },
        )
        .unwrap();
    h.control
        .authorize(view.id, &h.agent, h.run, &Yes::new(true))
        .unwrap();
    let _ = h.control.execute(view.id, &h.agent, h.run);
    assert_eq!(
        got.lock().unwrap().take().unwrap().unwrap_err(),
        AuthorityError::Closed("the lease is not bound to this commitment")
    );
    assert!(broker.is_live(other), "the unlisted lease was not consumed");
    assert_eq!(vault.0.load(Ordering::SeqCst), 0, "the vault was not read");
}

/// Governed credentials come from the vault: one the facade would resolve
/// from the process environment is refused.
#[test]
fn a_credential_from_the_environment_is_refused() {
    use nexus_kernel::secrets::{ResolvedFrom, ResolvedSecret};
    let resolved = |source| ResolvedSecret {
        value: Zeroizing::new("tok-vault-1234".to_string()),
        source,
    };
    assert!(matches!(
        super::from_vault(resolved(ResolvedFrom::Env)),
        Err(SecretUnavailable::Ambient)
    ));
    for source in [
        ResolvedFrom::Keyring,
        ResolvedFrom::Sqlite,
        ResolvedFrom::Memory,
    ] {
        assert_eq!(
            super::from_vault(resolved(source)).unwrap().as_str(),
            "tok-vault-1234"
        );
    }
}

/// A preparation the authority refuses leaves no lease behind.
#[test]
fn a_refused_preparation_ends_its_lease() {
    let (h, broker, _vault, destination) = setup();
    let lease = broker
        .lease(&h.agent, h.run, SPEC, &destination, Duration::from_secs(60))
        .unwrap();
    let grant = h
        .control
        .authority()
        .grants()
        .request(
            GrantScope::Connector {
                connector: "fixture".into(),
                account: "test".into(),
                operations: vec!["broker.test".into()],
            },
            Duration::from_secs(60),
            &Yes::new(true),
        )
        .unwrap();
    let preparation = Preparation {
        action: PreparedAction {
            kind: CapabilityKind::Connector,
            class: EffectClass::R1,
            operation: "broker.test",
            target: TargetIdentity {
                display: destination.origin_text(),
                digest: destination.origin_digest(),
            },
            parameters: Digest::of("broker.test", &[]),
            grants: vec![grant],
            leases: vec![lease],
            // More than an approval may show: refused.
            summary: vec!["x".into(); crate::authority::commitment::MAX_SUMMARY_LINES + 1],
        },
        effect: Box::new(Releasing {
            broker: broker.clone(),
            lease,
            destination: destination.clone(),
            got: Arc::new(Mutex::new(None)),
        }),
        ttl: Duration::from_secs(60),
    };
    assert!(h.control.propose(&h.agent, h.run, preparation).is_err());
    assert!(!broker.is_live(lease));
}

/// A proposal refused before any commitment exists (its effect does not
/// match the parameters it declares) leaves no lease behind either.
#[test]
fn a_proposal_refused_before_its_commitment_ends_its_lease() {
    let (h, broker, _vault, destination) = setup();
    let lease = broker
        .lease(&h.agent, h.run, SPEC, &destination, Duration::from_secs(60))
        .unwrap();
    let grant = h
        .control
        .authority()
        .grants()
        .request(
            GrantScope::Connector {
                connector: "fixture".into(),
                account: "test".into(),
                operations: vec!["broker.test".into()],
            },
            Duration::from_secs(60),
            &Yes::new(true),
        )
        .unwrap();
    let preparation = Preparation {
        action: PreparedAction {
            kind: CapabilityKind::Connector,
            class: EffectClass::R1,
            operation: "broker.test",
            target: TargetIdentity {
                display: destination.origin_text(),
                digest: destination.origin_digest(),
            },
            parameters: Digest::of("something else", &[]),
            grants: vec![grant],
            leases: vec![lease],
            summary: vec!["test".into()],
        },
        effect: Box::new(Releasing {
            broker: broker.clone(),
            lease,
            destination: destination.clone(),
            got: Arc::new(Mutex::new(None)),
        }),
        ttl: Duration::from_secs(60),
    };
    assert!(matches!(
        h.control.propose(&h.agent, h.run, preparation),
        Err(AuthorityError::InvalidAction(_))
    ));
    assert!(!broker.is_live(lease));
}

// XA-R1-02: a release is asked again once its record is written and once
// the vault has answered. A run cancelled, finished or stopped, or a lease
// expired, meanwhile gets no secret, and the lease ends.

/// The state of `lease` in the broker's table.
fn state_of(
    broker: &CredentialBroker,
    lease: crate::authority::ids::LeaseId,
) -> Option<super::LeaseState> {
    broker
        .table
        .leases
        .lock()
        .unwrap()
        .map
        .get(&lease)
        .map(|l| l.state)
}

/// A vault holding the fixture secret that runs `during` once, on the
/// reading thread, while it is read; it counts reads.
struct ActingVault {
    reads: AtomicU32,
    during: Mutex<Option<Box<dyn FnOnce() + Send>>>,
}

impl ActingVault {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            reads: AtomicU32::new(0),
            during: Mutex::new(None),
        })
    }

    fn during(&self, act: impl FnOnce() + Send + 'static) {
        *self.during.lock().unwrap() = Some(Box::new(act));
    }
}

impl SecretSource for ActingVault {
    fn read(&self, scope: &str, name: &str) -> Result<Zeroizing<String>, SecretUnavailable> {
        self.reads.fetch_add(1, Ordering::SeqCst);
        let during = self.during.lock().unwrap().take();
        if let Some(during) = during {
            during();
        }
        FakeVault(AtomicU32::new(0)).read(scope, name)
    }
}

/// A pipeline whose evidence is a scripted sink, on `clock`, with one agent
/// and one open run, a broker over `vault` and the fixture destination.
fn scripted(
    clock: Arc<dyn crate::authority::clock::Clock>,
    vault: Arc<dyn SecretSource>,
) -> (
    Harness,
    Arc<crate::authority::scripted::ScriptedSink>,
    Arc<CredentialBroker>,
    Destination,
) {
    use crate::authority::evidence::MemoryEvidence;
    use crate::authority::ids::AgentId;
    use crate::authority::run::RunOrigin;
    use crate::authority::scripted::ScriptedSink;
    use crate::authority::Authority;
    use crate::control::Control;
    let sink = ScriptedSink::new();
    let control = Arc::new(Control::new(Authority::new(sink.clone(), clock)));
    let agent = AgentId::new("agent-test").unwrap();
    let run = control
        .authority()
        .open_run(agent.clone(), RunOrigin::AgentGoal)
        .unwrap();
    let h = Harness {
        control,
        evidence: Arc::new(MemoryEvidence::new(1)),
        agent,
        run,
    };
    let broker = CredentialBroker::new(h.control.authority(), vault);
    let destination = Destination::parse("https://api.fixture.example").unwrap();
    (h, sink, broker, destination)
}

/// Run `act` once, on the recording thread, while the CredentialReleased
/// record is written: the lease is consumed, the vault not yet read.
fn while_release_recorded(
    sink: &crate::authority::scripted::ScriptedSink,
    act: impl Fn() + Send + Sync + 'static,
) {
    use crate::authority::evidence::EvidencePhase;
    let done = std::sync::atomic::AtomicBool::new(false);
    sink.reenter_with(move |record| {
        if record.phase == EvidencePhase::CredentialReleased && !done.swap(true, Ordering::SeqCst) {
            act();
        }
    });
}

/// What a `ReleasingTwice` effect saw.
#[derive(Default)]
struct Seen {
    /// What each release returned: the header's name (never its value), or
    /// the refusal.
    releases: Vec<Result<String, AuthorityError>>,
    /// The lease's state between the two releases.
    between: Option<super::LeaseState>,
}

/// An effect that asks the broker for its lease, then again under the same
/// guard; it fails, as egress does, when the first is refused (the
/// refusal's class is its detail).
struct ReleasingTwice {
    broker: Arc<CredentialBroker>,
    lease: crate::authority::ids::LeaseId,
    destination: Destination,
    seen: Arc<Mutex<Seen>>,
}

impl PendingEffect for ReleasingTwice {
    fn revalidate(&self) -> Result<Digest, AuthorityError> {
        Ok(self.destination.origin_digest())
    }
    fn parameters(&self) -> Digest {
        Digest::of("broker.test", &[])
    }
    fn execute(
        self: Box<Self>,
        guard: &crate::authority::commitment::ExecutionGuard,
    ) -> Result<EffectOutput, (crate::authority::commitment::FailureClass, String)> {
        let release = || {
            self.broker
                .release(self.lease, guard, &self.destination)
                .map(|header| header.name.to_string())
        };
        let first = release();
        let between = state_of(&self.broker, self.lease);
        let second = release();
        {
            let mut seen = self.seen.lock().unwrap();
            seen.releases = vec![first.clone(), second];
            seen.between = between;
        }
        match first {
            Ok(_) => Ok(EffectOutput::default()),
            Err(error) => Err((
                crate::authority::commitment::FailureClass::Unavailable,
                error.class().to_string(),
            )),
        }
    }
}

/// Commit to an action listing `lease` whose effect is `ReleasingTwice`, and
/// authorize it: its id, and what its effect will have seen.
fn commit_twice(
    h: &Harness,
    broker: &Arc<CredentialBroker>,
    lease: crate::authority::ids::LeaseId,
    destination: &Destination,
) -> (crate::authority::ids::CommitmentId, Arc<Mutex<Seen>>) {
    let (mut preparation, _) = lease_preparation(h, broker, lease, destination);
    let seen = Arc::new(Mutex::new(Seen::default()));
    preparation.effect = Box::new(ReleasingTwice {
        broker: broker.clone(),
        lease,
        destination: destination.clone(),
        seen: seen.clone(),
    });
    let view = h.control.propose(&h.agent, h.run, preparation).unwrap();
    h.control
        .authorize(view.id, &h.agent, h.run, &Yes::new(true))
        .unwrap();
    (view.id, seen)
}

/// Execute `id` on another thread, within a bound: a lock held across the
/// record or the vault read would leave it blocked.
fn execute_within(
    h: &Harness,
    id: crate::authority::ids::CommitmentId,
) -> Result<EffectOutput, AuthorityError> {
    let (control, agent, run) = (h.control.clone(), h.agent.clone(), h.run);
    crate::authority::scripted::within(move || control.execute(id, &agent, run))
}

/// What `refused_while_recorded` saw.
struct Refused {
    /// What the pipeline returned.
    result: Result<EffectOutput, AuthorityError>,
    /// What the two releases returned.
    releases: Vec<Result<String, AuthorityError>>,
    /// The lease's state between them.
    between: Option<super::LeaseState>,
    /// How many times the vault was read.
    reads: u32,
}

/// A run, and what to do to it while the release is recorded: the refusal
/// the release returns, the vault reads, and the lease's state then.
fn refused_while_recorded(
    end: impl Fn(&crate::control::Control, crate::authority::ids::RunId) + Send + Sync + 'static,
) -> Refused {
    use crate::authority::clock::SystemClock;
    let vault = Arc::new(FakeVault(AtomicU32::new(0)));
    let (h, sink, broker, destination) = scripted(Arc::new(SystemClock::default()), vault.clone());
    let (control, run) = (Arc::downgrade(&h.control), h.run);
    while_release_recorded(&sink, move || {
        if let Some(control) = control.upgrade() {
            end(&control, run);
        }
    });
    let lease = broker
        .lease(&h.agent, h.run, SPEC, &destination, Duration::from_secs(60))
        .unwrap();
    let (id, seen) = commit_twice(&h, &broker, lease, &destination);
    let result = execute_within(&h, id);
    let seen = std::mem::take(&mut *seen.lock().unwrap());
    Refused {
        result,
        releases: seen.releases,
        between: seen.between,
        reads: vault.0.load(Ordering::SeqCst),
    }
}

/// T1: a run cancelled while its credential's release is recorded gets
/// nothing: the vault is not read, the refusal is the run's, the lease has
/// ended, and a second release of it under the same guard is refused and
/// reads nothing.
#[test]
fn a_run_cancelled_while_the_release_is_recorded_reads_no_secret() {
    let Refused {
        result,
        releases,
        between,
        reads,
    } = refused_while_recorded(|control, run| {
        let _ = control.cancel_run(run);
    });
    assert_eq!(
        releases,
        vec![
            Err(AuthorityError::RunCancelled),
            Err(AuthorityError::NotPending)
        ]
    );
    assert_eq!(reads, 0, "the vault was not read");
    assert_eq!(between, Some(super::LeaseState::Ended));
    assert_eq!(result.unwrap_err(), AuthorityError::RunCancelled);
}

/// T1b: a run that finished while the release is recorded: the same.
#[test]
fn a_run_finished_while_the_release_is_recorded_reads_no_secret() {
    let Refused {
        result,
        releases,
        between,
        reads,
    } = refused_while_recorded(|control, run| {
        control.finish_run(run);
    });
    assert_eq!(
        releases,
        vec![
            Err(AuthorityError::RunNotActive),
            Err(AuthorityError::NotPending)
        ]
    );
    assert_eq!(reads, 0, "the vault was not read");
    assert_eq!(between, Some(super::LeaseState::Ended));
    assert_eq!(result.unwrap_err(), AuthorityError::RunCancelled);
}

/// T2: an emergency stop while the release is recorded: nothing is read,
/// and the refusal is the stop's.
#[test]
fn an_emergency_stop_while_the_release_is_recorded_reads_no_secret() {
    let Refused {
        result,
        releases,
        between,
        reads,
    } = refused_while_recorded(|control, _| {
        control.emergency_stop();
    });
    assert_eq!(
        releases,
        vec![
            Err(AuthorityError::EmergencyStopped),
            Err(AuthorityError::NotPending)
        ]
    );
    assert_eq!(reads, 0, "the vault was not read");
    assert_eq!(between, Some(super::LeaseState::Ended));
    assert_eq!(result.unwrap_err(), AuthorityError::RunCancelled);
}

/// T3: a lease whose deadline passes while its release is recorded is
/// expired: nothing is read and the lease ends.
#[test]
fn a_lease_that_expires_while_its_release_is_recorded_reads_no_secret() {
    use crate::authority::clock::ManualClock;
    let clock = Arc::new(ManualClock::default());
    let vault = Arc::new(FakeVault(AtomicU32::new(0)));
    let (h, sink, broker, destination) = scripted(clock.clone(), vault.clone());
    let lease = broker
        .lease(&h.agent, h.run, SPEC, &destination, Duration::from_secs(5))
        .unwrap();
    {
        let clock = clock.clone();
        while_release_recorded(&sink, move || clock.advance(Duration::from_secs(10)));
    }
    let (id, seen) = commit_twice(&h, &broker, lease, &destination);
    let result = execute_within(&h, id);
    let seen = seen.lock().unwrap();
    assert_eq!(
        seen.releases,
        vec![
            Err(AuthorityError::Expired),
            Err(AuthorityError::NotPending)
        ]
    );
    assert_eq!(vault.0.load(Ordering::SeqCst), 0, "the vault was not read");
    assert_eq!(seen.between, Some(super::LeaseState::Ended));
    assert_eq!(
        result.unwrap_err(),
        AuthorityError::Unavailable("the effect failed")
    );
}

/// T4: a run cancelled while the vault is being read: the secret read is
/// dropped, no header is built, and the lease ends. The vault's reader
/// also takes the pipeline's and the broker's locks: none is held across
/// the read.
#[test]
fn a_run_cancelled_while_the_vault_is_read_gets_no_header() {
    use crate::authority::clock::SystemClock;
    let vault = ActingVault::new();
    let (h, _sink, broker, destination) = scripted(Arc::new(SystemClock::default()), vault.clone());
    let lease = broker
        .lease(&h.agent, h.run, SPEC, &destination, Duration::from_secs(60))
        .unwrap();
    {
        let (control, weak, run) = (Arc::downgrade(&h.control), Arc::downgrade(&broker), h.run);
        vault.during(move || {
            if let Some(control) = control.upgrade() {
                let _ = control.cancel_run(run);
                control.probe_locks();
            }
            if let Some(broker) = weak.upgrade() {
                broker.probe_locks();
            }
        });
    }
    let (id, seen) = commit_twice(&h, &broker, lease, &destination);
    let result = execute_within(&h, id);
    let seen = seen.lock().unwrap();
    assert_eq!(
        seen.releases,
        vec![
            Err(AuthorityError::RunCancelled),
            Err(AuthorityError::NotPending)
        ]
    );
    assert_eq!(vault.reads.load(Ordering::SeqCst), 1, "one read, no more");
    assert_eq!(seen.between, Some(super::LeaseState::Ended));
    assert_eq!(result.unwrap_err(), AuthorityError::RunCancelled);
}

/// T5: a sink that, while the release is recorded, re-enters the authority
/// (cancels the run) and the broker (asks whether the lease is live, takes
/// its lock) does not deadlock the release: it returns, refused, within a
/// bound; no lock is held while any record is written.
#[test]
fn a_sink_reentering_the_authority_and_the_broker_during_the_release_does_not_deadlock() {
    use crate::authority::clock::SystemClock;
    use std::sync::mpsc;
    let vault = Arc::new(FakeVault(AtomicU32::new(0)));
    let (h, sink, broker, destination) = scripted(Arc::new(SystemClock::default()), vault.clone());
    let lease = broker
        .lease(&h.agent, h.run, SPEC, &destination, Duration::from_secs(60))
        .unwrap();
    let live_then = Arc::new(Mutex::new(None));
    {
        let (control, weak, run) = (Arc::downgrade(&h.control), Arc::downgrade(&broker), h.run);
        let live_then = live_then.clone();
        while_release_recorded(&sink, move || {
            if let Some(control) = control.upgrade() {
                let _ = control.cancel_run(run);
            }
            if let Some(broker) = weak.upgrade() {
                *live_then.lock().unwrap() = Some(broker.is_live(lease));
                broker.probe_locks();
            }
        });
    }
    {
        let (control, weak) = (Arc::downgrade(&h.control), Arc::downgrade(&broker));
        sink.probe_with(move || {
            if let Some(control) = control.upgrade() {
                control.probe_locks();
            }
            if let Some(broker) = weak.upgrade() {
                broker.probe_locks();
            }
        });
    }
    let (id, seen) = commit_twice(&h, &broker, lease, &destination);
    let (done, returned) = mpsc::channel();
    let (control, agent, run) = (h.control.clone(), h.agent.clone(), h.run);
    std::thread::spawn(move || {
        let _ = done.send(control.execute(id, &agent, run));
    });
    let result = returned
        .recv_timeout(Duration::from_secs(10))
        .expect("the release returned: no lock was held across its record");
    assert_eq!(
        seen.lock().unwrap().releases.first(),
        Some(&Err(AuthorityError::RunCancelled))
    );
    assert_eq!(result.unwrap_err(), AuthorityError::RunCancelled);
    assert_eq!(
        *live_then.lock().unwrap(),
        Some(false),
        "consumed before it was recorded"
    );
    assert_eq!(vault.0.load(Ordering::SeqCst), 0, "the vault was not read");
    assert_eq!(
        sink.violations(),
        0,
        "a lock was held while a record was written"
    );
}

/// T6: a vault read that fails leaves the lease consumed: a later release
/// of it is refused and reads nothing.
#[test]
fn a_failed_vault_read_leaves_the_lease_unusable() {
    let (h, broker, vault, destination) = setup();
    let missing = CredentialSpec {
        name: "missing.token",
        ..SPEC
    };
    let lease = broker
        .lease(
            &h.agent,
            h.run,
            missing,
            &destination,
            Duration::from_secs(60),
        )
        .unwrap();
    assert_eq!(
        use_lease(&h, &broker, lease, &destination)
            .unwrap()
            .unwrap_err(),
        AuthorityError::Unavailable("the credential is not available")
    );
    assert_eq!(vault.0.load(Ordering::SeqCst), 1);
    assert_eq!(
        use_lease(&h, &broker, lease, &destination)
            .unwrap()
            .unwrap_err(),
        AuthorityError::NotPending
    );
    assert_eq!(
        vault.0.load(Ordering::SeqCst),
        1,
        "the second release read nothing"
    );
    assert!(!broker.is_live(lease));
    assert_ne!(
        state_of(&broker, lease),
        Some(super::LeaseState::Issued),
        "a lease never returns to issued"
    );
}

/// T7: with nothing cancelled, a release is what it was: one record, one
/// vault read, one header.
#[test]
fn an_uninterrupted_release_records_once_reads_once_and_returns_its_header() {
    use crate::authority::evidence::EvidencePhase;
    let (h, broker, vault, destination) = setup();
    let lease = broker
        .lease(&h.agent, h.run, SPEC, &destination, Duration::from_secs(60))
        .unwrap();
    assert_eq!(
        use_lease(&h, &broker, lease, &destination)
            .unwrap()
            .unwrap(),
        format!("authorization: Bearer {SECRET}")
    );
    assert_eq!(vault.0.load(Ordering::SeqCst), 1);
    assert_eq!(
        h.evidence
            .records()
            .iter()
            .filter(|r| r.phase == EvidencePhase::CredentialReleased)
            .count(),
        1
    );
}

/// T9: what the evidence says of a release refused after its record: one
/// CredentialReleased record, then the commitment's terminal record, which
/// says the run was cancelled and the action did not succeed.
#[test]
fn a_release_refused_after_its_record_ends_its_commitment_cancelled() {
    use crate::authority::clock::SystemClock;
    use crate::authority::evidence::EvidencePhase;
    let vault = Arc::new(FakeVault(AtomicU32::new(0)));
    let (h, sink, broker, destination) = scripted(Arc::new(SystemClock::default()), vault.clone());
    {
        let (control, run) = (Arc::downgrade(&h.control), h.run);
        while_release_recorded(&sink, move || {
            if let Some(control) = control.upgrade() {
                let _ = control.cancel_run(run);
            }
        });
    }
    let lease = broker
        .lease(&h.agent, h.run, SPEC, &destination, Duration::from_secs(60))
        .unwrap();
    let (id, _) = commit_twice(&h, &broker, lease, &destination);
    let _ = execute_within(&h, id);
    let records = sink.memory.records();
    let id = id.to_string();
    let terminal = records
        .iter()
        .rev()
        .find(|r| r.commitment.as_deref() == Some(id.as_str()))
        .unwrap();
    println!(
        "T9 terminal record: phase={:?} outcome={:?} failure={:?} cancelled={} detail={:?}",
        terminal.phase, terminal.outcome, terminal.failure, terminal.cancelled, terminal.detail
    );
    assert_eq!(terminal.phase, EvidencePhase::Finished);
    assert_ne!(terminal.outcome, Some("succeeded"));
    assert_eq!(terminal.outcome, Some("cancelled"));
    assert!(terminal.cancelled);
    assert_eq!(
        records
            .iter()
            .filter(|r| r.phase == EvidencePhase::CredentialReleased)
            .count(),
        1
    );
    assert_eq!(vault.0.load(Ordering::SeqCst), 0);
    assert!(!format!("{records:?}").contains(SECRET));
}
