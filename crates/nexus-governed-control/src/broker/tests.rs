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
    let grant = h
        .control
        .authority()
        .grants()
        .request(
            GrantScope::Perception {
                display: "test".into(),
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
    let view = h.control.propose(&h.agent, h.run, preparation).ok()?;
    h.control
        .authorize(view.id, &h.agent, h.run, &Yes::new(true))
        .ok()?;
    let _ = h.control.execute(view.id, &h.agent, h.run);
    let result = got.lock().unwrap().take();
    result
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
            GrantScope::Perception {
                display: "test".into(),
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
            GrantScope::Perception {
                display: "test".into(),
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
