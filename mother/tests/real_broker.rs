//! M1 oracles against the real broker: a `GovernedControl` built as the
//! desktop builds it (its own runtime root and evidence store), a local
//! listener as the world. Pre-registered with the unit oracles in
//! `src/oracles/` (`MOTHER.md` §3.1).

use nexus_governed_control::authority::approval::{
    AcknowledgementConfirmation, ActionConfirmation, ControlConfirmer, GrantConfirmation,
    ResumeConfirmation,
};
use nexus_governed_control::authority::clock::SystemClock;
use nexus_governed_control::authority::evidence::MemoryEvidence;
use nexus_governed_control::authority::run::{RunClass, RunOrigin};
use nexus_governed_control::broker::Vault;
use nexus_governed_control::egress::EgressIntent;
use nexus_governed_control::evidence_store::PrimaryEvidence;
use nexus_governed_control::governed::{GovernedControl, GrantRequest, Intent};
use nexus_mother::broker::{Refusal, MOTHER_AGENT};
use nexus_mother::contract::{MissionContract, Step};
use nexus_mother::coordinator::{
    ConsentRequest, Coordinator, CoordinatorError, MissionState, OwnerConsent, StepOutcome,
};
use std::io::{ErrorKind, Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

/// The Owner's broker dialogs, answered yes (grants only are asked here).
struct OwnerAtTheBroker;
impl ControlConfirmer for OwnerAtTheBroker {
    fn confirm_action(&self, _: &ActionConfirmation) -> bool {
        true
    }
    fn confirm_grant(&self, _: &GrantConfirmation) -> bool {
        true
    }
    fn confirm_resume(&self, _: &ResumeConfirmation) -> bool {
        false
    }
    fn confirm_acknowledgement(&self, _: &AcknowledgementConfirmation) -> bool {
        false
    }
}

/// The Owner's consent to Mother's mission.
struct Yes;
impl OwnerConsent for Yes {
    fn confirm_mission(&self, _: &ConsentRequest) -> bool {
        true
    }
}

struct World {
    root: PathBuf,
    control: Arc<GovernedControl>,
    mother: Coordinator,
    listener: TcpListener,
    origin: String,
}

impl Drop for World {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn world(name: &str) -> World {
    let root = std::env::temp_dir().join(format!(
        "nexus-mother-m1-{name}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    let evidence = PrimaryEvidence::open(
        &root.join("p3-evidence").join("evidence.sqlite3"),
        vec![Arc::new(MemoryEvidence::new(256))],
    )
    .unwrap();
    let clock = Arc::new(SystemClock::default());
    let control = Arc::new(
        GovernedControl::new(&root, Vault::Disabled, evidence, clock.clone()).unwrap(),
    );
    let mother = Coordinator::new(control.clone(), clock);
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    World {
        root,
        control,
        mother,
        listener,
        origin,
    }
}

impl World {
    fn grant(&self) {
        self.control
            .request_grant(
                &GrantRequest::Egress {
                    origin: self.origin.clone(),
                    methods: vec!["GET".into()],
                    allow_private: true,
                },
                Duration::from_secs(60),
                &OwnerAtTheBroker,
            )
            .unwrap();
    }

    fn get(&self, path: &str) -> Step {
        Step {
            intent: Intent::Request(EgressIntent {
                method: "GET".into(),
                url: format!("{}{path}", self.origin),
                headers: vec![],
                body: None,
            }),
            declared_cost_minor: 0,
        }
    }

    fn contract(&self, paths: &[&str]) -> MissionContract {
        MissionContract {
            goal: "Read the local page".into(),
            steps: paths.iter().map(|path| self.get(path)).collect(),
            consent_ttl_ms: 60_000,
        }
    }

    /// Connections that reached the world and were not served.
    fn unserved(&self) -> usize {
        let mut count = 0;
        loop {
            match self.listener.accept() {
                Ok(_) => count += 1,
                Err(e) if e.kind() == ErrorKind::WouldBlock => return count,
                Err(e) => panic!("{e}"),
            }
        }
    }

    /// Serve exactly one request on a thread; returns its request line.
    fn serve_one(&self) -> std::thread::JoinHandle<String> {
        let listener = self.listener.try_clone().unwrap();
        std::thread::spawn(move || {
            listener.set_nonblocking(false).unwrap();
            let (mut stream, _) = listener.accept().unwrap();
            listener.set_nonblocking(true).unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut request = [0u8; 4096];
            let n = stream.read(&mut request).unwrap();
            stream
                .write_all(
                    b"HTTP/1.1 200 OK\r\nContent-Length: 6\r\nConnection: close\r\n\r\nmother",
                )
                .unwrap();
            String::from_utf8_lossy(&request[..n])
                .lines()
                .next()
                .unwrap_or_default()
                .to_string()
        })
    }

    fn mothers_runs(&self) -> usize {
        self.control
            .authority()
            .runs()
            .views()
            .iter()
            .filter(|run| run.agent.to_string() == MOTHER_AGENT)
            .count()
    }
}

#[test]
fn m1_real_i1_a_live_grant_does_not_let_mother_act_before_consent() {
    let w = world("before-consent");
    w.grant();
    let id = w.mother.draft(w.contract(&["/m1"])).unwrap();
    let _ = w.mother.consent_request(id).unwrap();
    assert_eq!(
        w.mother.step(id),
        Err(CoordinatorError::NotApproved(MissionState::Drafted))
    );
    assert_eq!(w.mothers_runs(), 0, "no run opened before consent");
    assert!(w.control.authority().runs().views().is_empty());
    assert_eq!(w.unserved(), 0, "nothing reached the world");
}

#[test]
fn m1_real_i3_consent_without_a_broker_grant_is_refused_by_the_broker() {
    let w = world("no-grant");
    let id = w.mother.draft(w.contract(&["/m1"])).unwrap();
    w.mother.request_consent(id, &Yes).unwrap();
    assert_eq!(
        w.mother.step(id),
        Err(CoordinatorError::Refused(Refusal {
            class: "no_covering_grant"
        }))
    );
    assert_eq!(
        w.mother.state(id),
        Some(MissionState::Failed {
            step: 0,
            class: "no_covering_grant"
        })
    );
    let run = w.mother.run_of(id).unwrap();
    assert!(w.control.authority().runs().view(run).unwrap().cancelled);
    assert_eq!(w.unserved(), 0);
}

#[test]
fn m1_real_i2_a_consented_granted_step_reaches_the_world_once_as_an_agent() {
    let w = world("consented");
    w.grant();
    let id = w.mother.draft(w.contract(&["/m1"])).unwrap();
    w.mother.request_consent(id, &Yes).unwrap();
    let served = w.serve_one();
    let report = w.mother.step(id).unwrap();
    assert_eq!(
        report.outcome,
        StepOutcome::Done {
            text: Some("mother".into())
        }
    );
    assert_eq!(served.join().unwrap(), "GET /m1 HTTP/1.1");
    assert_eq!(w.mother.state(id), Some(MissionState::Completed));
    let run = w.mother.run_of(id).unwrap();
    let view = w.control.authority().runs().view(run).unwrap();
    assert_eq!(view.agent.to_string(), MOTHER_AGENT);
    assert_eq!(view.origin, RunOrigin::AgentGoal);
    assert_eq!(w.control.authority().runs().class(run), Some(RunClass::Agent));
    assert!(view.finished, "the completed mission's run is finished");
    assert_eq!(w.unserved(), 0, "exactly once");
}

#[test]
fn m1_real_i4_stop_cancels_the_broker_run_and_nothing_more_reaches_the_world() {
    let w = world("stop");
    w.grant();
    let id = w.mother.draft(w.contract(&["/one", "/two"])).unwrap();
    w.mother.request_consent(id, &Yes).unwrap();
    let served = w.serve_one();
    w.mother.step(id).unwrap();
    assert_eq!(served.join().unwrap(), "GET /one HTTP/1.1");
    assert_eq!(w.mother.stop(id), Ok(MissionState::Stopped));
    let run = w.mother.run_of(id).unwrap();
    assert!(w.control.authority().runs().view(run).unwrap().cancelled);
    assert!(w.mother.step(id).is_err());
    assert_eq!(w.unserved(), 0);
}

#[test]
fn m1_real_i4_stop_before_the_first_step_opens_no_run() {
    let w = world("stop-early");
    w.grant();
    let id = w.mother.draft(w.contract(&["/m1"])).unwrap();
    w.mother.request_consent(id, &Yes).unwrap();
    assert_eq!(w.mother.stop(id), Ok(MissionState::Stopped));
    assert!(w.mother.step(id).is_err());
    assert_eq!(w.mothers_runs(), 0);
    assert_eq!(w.unserved(), 0);
}

#[test]
fn m1_real_i5_the_owner_s_emergency_stop_stops_mother() {
    let w = world("emergency");
    w.grant();
    let id = w.mother.draft(w.contract(&["/m1"])).unwrap();
    w.mother.request_consent(id, &Yes).unwrap();
    w.control.emergency_stop();
    assert_eq!(w.mother.step(id), Err(CoordinatorError::Stopped));
    assert_eq!(w.mother.state(id), Some(MissionState::Stopped));
    assert_eq!(w.mothers_runs(), 0);
    assert_eq!(w.unserved(), 0);
}
