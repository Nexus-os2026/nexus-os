//! M1 oracles against the real broker: a `GovernedControl` built as the
//! desktop builds it (its own runtime root and evidence store), the
//! journal in a file, and a local listener as the world.
//!
//! One test: a non-test build of the broker claims the process's
//! emergency-stop controller, so one broker per process. The scenarios run
//! in order on it, the emergency stop last.

use super::{mother_agent, FileJournal, GovernedPort, Home, MOTHER_AGENT};
use crate::coordinator::{Coordinator, CoordinatorError, Lanes};
use crate::effect::{ApprovalStatus, EffectPort, PendingAction};
use crate::journal::MissionId;
use crate::oracles::{contract_with, keyed_lanes, No, Yes};
use crate::state::MissionState;
use nexus_governed_control::authority::approval::{
    AcknowledgementConfirmation, ActionConfirmation, ControlConfirmer, GrantConfirmation,
    ResumeConfirmation,
};
use nexus_governed_control::authority::clock::SystemClock;
use nexus_governed_control::authority::commitment::{CommitmentState, CommitmentView};
use nexus_governed_control::authority::ids::AgentId;
use nexus_governed_control::authority::run::{RunClass, RunOrigin};
use nexus_governed_control::egress::EgressIntent;
use nexus_governed_control::governed::GrantRequest;
use nexus_governed_control::governed::{AgentOutcome, Intent};
use std::io::{ErrorKind, Read, Write};
use std::net::TcpListener;
use std::sync::Arc;
use std::time::Duration;

/// The Owner at the broker's own dialogs: grants only (test code).
struct OwnerAtTheBroker;
impl ControlConfirmer for OwnerAtTheBroker {
    fn confirm_action(&self, _: &ActionConfirmation) -> bool {
        false
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

/// The Owner approving an action at the broker's native dialog (test code).
struct OwnerApproves;
impl ControlConfirmer for OwnerApproves {
    fn confirm_action(&self, _: &ActionConfirmation) -> bool {
        true
    }
    fn confirm_grant(&self, _: &GrantConfirmation) -> bool {
        false
    }
    fn confirm_resume(&self, _: &ResumeConfirmation) -> bool {
        false
    }
    fn confirm_acknowledgement(&self, _: &AcknowledgementConfirmation) -> bool {
        false
    }
}

struct World {
    home: Home,
    port: Arc<GovernedPort>,
    mother: Coordinator<GovernedPort>,
    listener: TcpListener,
    origin: String,
}

impl Drop for World {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.home.dir);
    }
}

fn open_mother(home: &Home, port: &Arc<GovernedPort>) -> Coordinator<GovernedPort> {
    Coordinator::open(
        home.journal().unwrap(),
        port.clone(),
        Lanes::new(keyed_lanes()).unwrap(),
        Arc::new(SystemClock::default()),
    )
    .unwrap()
}

impl World {
    fn new() -> Self {
        let home = Home::fresh_temp("real-broker").unwrap();
        let port = Arc::new(GovernedPort::new(home.control.clone()));
        let mother = open_mother(&home, &port);
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        World {
            home,
            port,
            mother,
            listener,
            origin,
        }
    }

    fn grant_egress(&self) {
        self.home
            .control
            .request_grant(
                &GrantRequest::Egress {
                    origin: self.origin.clone(),
                    methods: vec!["GET".into()],
                    allow_private: true,
                },
                Duration::from_secs(600),
                &OwnerAtTheBroker,
            )
            .unwrap();
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
        self.home
            .control
            .authority()
            .runs()
            .views()
            .iter()
            .filter(|run| run.agent.to_string() == MOTHER_AGENT)
            .count()
    }

    fn approved(&self, markers: &str) -> MissionId {
        let id = self.mother.draft(contract_with(markers)).unwrap();
        self.mother.submit(id).unwrap();
        assert_eq!(
            self.mother.request_consent(id, &Yes),
            Ok(MissionState::Approved)
        );
        id
    }

    fn advance_to(&self, id: MissionId, n: usize) {
        for _ in 0..n {
            self.mother.advance(id).unwrap();
        }
    }

    /// The one action of the mission's run that waits for the Owner.
    fn pending(&self, id: MissionId) -> CommitmentView {
        let run = self.port.run_of(id).expect("the mission's run");
        let views = self
            .home
            .control
            .authority()
            .commitments()
            .views_of_run(run);
        let waiting: Vec<CommitmentView> = views
            .into_iter()
            .filter(|v| v.state == CommitmentState::Prepared)
            .collect();
        assert_eq!(waiting.len(), 1, "{waiting:?}");
        waiting.into_iter().next().unwrap()
    }
}

/// Codex review MAJOR (oracle first): a request parked awaiting approval
/// is withdrawn by a Stop pressed after both the coordinator and the port
/// are rebuilt over the same live broker; the rebuilt port also still knows
/// what the mission waits on. Its own journal, so the World's coordinator
/// and journal are untouched. Runs after `m2a_waiting_on_the_real_broker`,
/// which granted POST to the origin.
fn m2a_a_rebuilt_port_withdraws_on_stop(w: &World) {
    let post = format!("POST-BUILDER:{}/m2a-rebuilt", w.origin);
    let agent = mother_agent();
    let path = w.home.dir.join("m2a-rebuilt-journal.jsonl");
    let open = |port: &Arc<GovernedPort>| {
        Coordinator::open(
            FileJournal::open(&path).unwrap(),
            port.clone(),
            Lanes::new(keyed_lanes()).unwrap(),
            Arc::new(SystemClock::default()),
        )
        .unwrap()
    };
    let clock = w.home.control.authority().clock().clone();
    let park = |mother: &Coordinator<GovernedPort>| {
        let id = mother.draft(contract_with(&post)).unwrap();
        mother.submit(id).unwrap();
        assert_eq!(mother.request_consent(id, &Yes), Ok(MissionState::Approved));
        for _ in 0..3 {
            mother.advance(id).unwrap();
        }
        assert_eq!(mother.state(id), Some(MissionState::AwaitingApproval));
        id
    };
    let pending_of = |run| {
        let views = w.home.control.authority().commitments().views_of_run(run);
        let waiting: Vec<CommitmentView> = views
            .into_iter()
            .filter(|v| v.state == CommitmentState::Prepared)
            .collect();
        assert_eq!(waiting.len(), 1, "{waiting:?}");
        waiting.into_iter().next().unwrap()
    };

    // Park two requests through a first port.
    let first = Arc::new(GovernedPort::new(w.home.control.clone()));
    let mother = open(&first);
    let before = clock.monotonic_ms();
    let stopped = park(&mother);
    let after = clock.monotonic_ms();
    let kept = park(&mother);
    let stopped_run = first.run_of(stopped).unwrap();
    let pending = pending_of(stopped_run);
    let kept_run = first.run_of(kept).unwrap();
    let kept_pending = pending_of(kept_run);
    // The port's own deadline for the action is never later than the
    // broker's (monotonic, taken before the broker was asked).
    let estimate = first.pending(stopped).expect("the pending action");
    assert_eq!(estimate.run, stopped_run.to_string());
    assert_eq!(estimate.commitment, pending.id.to_string());
    let ttl = pending.expires_wall_ms - pending.created_wall_ms;
    assert!(estimate.deadline_mono_ms >= before + ttl, "{estimate:?}");
    assert!(estimate.deadline_mono_ms <= after + ttl, "{estimate:?}");
    drop(mother);
    drop(first);

    // Rebuilt: a new coordinator and a brand-new port over the same broker.
    let port = Arc::new(GovernedPort::new(w.home.control.clone()));
    let mother = open(&port);
    assert_eq!(mother.state(stopped), Some(MissionState::AwaitingApproval));
    assert_eq!(mother.stop(stopped), Ok(MissionState::Stopped));
    let view = w
        .home
        .control
        .authority()
        .commitments()
        .view(pending.id)
        .unwrap();
    assert!(view.state != CommitmentState::Prepared, "{:?}", view.state);
    assert!(w
        .home
        .control
        .authorize(pending.id, &agent, stopped_run, &OwnerApproves)
        .is_err());
    assert!(w
        .home
        .control
        .execute(pending.id, &agent, stopped_run)
        .is_err());
    assert!(
        w.home
            .control
            .authority()
            .runs()
            .view(stopped_run)
            .unwrap()
            .cancelled
    );

    // The rebuilt port still knows what the other mission waits on.
    assert_eq!(
        mother.advance(kept).unwrap().to,
        MissionState::AwaitingApproval
    );
    assert_eq!(mother.stop(kept), Ok(MissionState::Stopped));
    assert!(w
        .home
        .control
        .authorize(kept_pending.id, &agent, kept_run, &OwnerApproves)
        .is_err());
    drop(mother);

    // Durable: rebuilt once more, both stay stopped.
    let again = open(&Arc::new(GovernedPort::new(w.home.control.clone())));
    assert_eq!(again.state(stopped), Some(MissionState::Stopped));
    assert_eq!(again.state(kept), Some(MissionState::Stopped));
    assert_eq!(w.unserved(), 0, "nothing reached the world");
}

/// Added after mutation testing (the check survived every earlier oracle):
/// a journaled action is only a name. A port handed back one that is not
/// Mother's (another agent's run, or Mother's run with another run's
/// commitment) neither cancels it nor reports on it.
fn m2a_a_port_adopts_only_mother_s_own_actions(w: &World) {
    let control = &w.home.control;
    let other = AgentId::new("another-agent").unwrap();
    let run = control
        .open_run(other.clone(), RunOrigin::AgentGoal)
        .unwrap();
    let intent = Intent::Request(EgressIntent {
        method: "POST".into(),
        url: format!("{}/not-mother", w.origin),
        headers: vec![],
        body: None,
    });
    let AgentOutcome::AwaitingApproval(view) = control.agent_action(&other, run, &intent).unwrap()
    else {
        panic!("a POST waits for the Owner");
    };
    let mothers = control
        .open_run(mother_agent(), RunOrigin::AgentGoal)
        .unwrap();
    let port = GovernedPort::new(control.clone());
    let forged = [(MissionId(9001), run), (MissionId(9002), mothers)];
    for (mission, run) in forged {
        port.adopt(
            mission,
            &PendingAction {
                run: run.to_string(),
                commitment: view.id.to_string(),
                deadline_mono_ms: u64::MAX,
            },
        );
        assert_eq!(port.run_of(mission), None, "{mission}");
        assert_eq!(port.pending(mission), None, "{mission}");
        assert!(matches!(
            port.approval(mission),
            ApprovalStatus::Lapsed { .. }
        ));
        port.cancel(mission);
    }
    let runs = control.authority().runs();
    assert!(!runs.view(run).unwrap().cancelled, "another agent's run");
    assert!(!runs.view(mothers).unwrap().cancelled);
    assert_eq!(
        control
            .authority()
            .commitments()
            .view(view.id)
            .unwrap()
            .state,
        CommitmentState::Prepared
    );
    control.cancel_run(run).unwrap();
    control.cancel_run(mothers).unwrap();
    assert_eq!(w.unserved(), 0);
}

/// M2a against the real broker: Mother waits for the Owner's native
/// approval of an R2 action (a POST) and continues exactly once; a denial
/// and a Stop while waiting reach nothing.
fn m2a_waiting_on_the_real_broker(w: &World) {
    w.home
        .control
        .request_grant(
            &GrantRequest::Egress {
                origin: w.origin.clone(),
                methods: vec!["POST".into()],
                allow_private: true,
            },
            Duration::from_secs(600),
            &OwnerAtTheBroker,
        )
        .unwrap();
    let post = format!("POST-BUILDER:{}/m2a", w.origin);
    let agent = mother_agent();

    // Approve: nothing reaches the world while Mother waits, not even
    // across a restart; the Owner's approval runs it once and Mother goes
    // on.
    let id = w.approved(&post);
    w.advance_to(id, 2);
    let performed = w.port.performed();
    assert_eq!(
        w.mother.advance(id).unwrap().to,
        MissionState::AwaitingApproval
    );
    assert_eq!(w.port.performed(), performed + 1);
    let pending = w.pending(id);
    assert!(pending.requires_approval);
    assert_eq!(pending.agent.to_string(), MOTHER_AGENT);
    assert_eq!(
        w.mother.advance(id).unwrap().to,
        MissionState::AwaitingApproval
    );
    let restarted = open_mother(&w.home, &w.port);
    assert_eq!(restarted.state(id), Some(MissionState::AwaitingApproval));
    drop(restarted);
    assert_eq!(w.unserved(), 0, "nothing reached the world while waiting");
    assert_eq!(w.port.performed(), performed + 1);
    let served = w.serve_one();
    let run = w.port.run_of(id).unwrap();
    w.home
        .control
        .authorize(pending.id, &agent, run, &OwnerApproves)
        .unwrap();
    w.home.control.execute(pending.id, &agent, run).unwrap();
    assert_eq!(served.join().unwrap(), "POST /m2a HTTP/1.1");
    assert_eq!(w.mother.advance(id).unwrap().to, MissionState::Verifying);
    w.advance_to(id, 2);
    assert_eq!(w.mother.state(id), Some(MissionState::Done));
    assert_eq!(w.unserved(), 0, "exactly once");
    assert_eq!(w.port.performed(), performed + 1, "Mother never re-sent it");

    // Deny: zero effect, the mission is Denied, its run cancelled.
    let denied = w.approved(&post);
    w.advance_to(denied, 3);
    assert_eq!(w.mother.state(denied), Some(MissionState::AwaitingApproval));
    let pending = w.pending(denied);
    let run = w.port.run_of(denied).unwrap();
    w.home.control.deny(pending.id, &agent, run).unwrap();
    assert_eq!(w.mother.advance(denied).unwrap().to, MissionState::Denied);
    assert!(
        w.home
            .control
            .authority()
            .runs()
            .view(run)
            .unwrap()
            .cancelled
    );
    assert_eq!(w.unserved(), 0);

    // Stop while waiting: the pending action is withdrawn, so the Owner
    // can no longer approve it, and nothing reaches the world.
    let stopped = w.approved(&post);
    w.advance_to(stopped, 3);
    assert_eq!(
        w.mother.state(stopped),
        Some(MissionState::AwaitingApproval)
    );
    let pending = w.pending(stopped);
    let run = w.port.run_of(stopped).unwrap();
    assert_eq!(w.mother.stop(stopped), Ok(MissionState::Stopped));
    let view = w
        .home
        .control
        .authority()
        .commitments()
        .view(pending.id)
        .unwrap();
    assert!(view.state != CommitmentState::Prepared, "{:?}", view.state);
    assert!(w
        .home
        .control
        .authorize(pending.id, &agent, run, &OwnerApproves)
        .is_err());
    assert!(w.home.control.execute(pending.id, &agent, run).is_err());
    assert_eq!(
        open_mother(&w.home, &w.port).state(stopped),
        Some(MissionState::Stopped)
    );
    assert_eq!(w.unserved(), 0);
}

#[test]
fn real_broker_m1_oracles() {
    let w = World::new();
    w.grant_egress();
    let fetch = format!("FETCH-BUILDER:{}/m1", w.origin);

    // MN-01 with a live grant: nothing before consent.
    let early = w.mother.draft(contract_with(&fetch)).unwrap();
    w.mother.submit(early).unwrap();
    assert!(w.mother.advance(early).is_err());
    assert_eq!(w.mothers_runs(), 0, "no run opened before consent");
    assert_eq!(w.port.performed(), 0);
    assert_eq!(w.unserved(), 0, "nothing reached the world before consent");

    // Denial: zero effect.
    assert_eq!(
        w.mother.request_consent(early, &No),
        Err(CoordinatorError::Denied)
    );
    assert!(w.mother.advance(early).is_err());
    assert_eq!(w.mothers_runs(), 0);
    assert_eq!(w.port.performed(), 0);
    assert_eq!(w.unserved(), 0);

    // A consented, granted effect reaches the world once, as Mother's agent
    // run, and the done mission's run is finished.
    let done = w.approved(&fetch);
    w.advance_to(done, 2);
    let served = w.serve_one();
    let built = w.mother.advance(done).unwrap();
    assert_eq!(built.to, MissionState::Verifying);
    assert_eq!(built.effects.len(), 1);
    assert_eq!(built.effects[0].outcome, "done");
    assert_eq!(served.join().unwrap(), "GET /m1 HTTP/1.1");
    w.advance_to(done, 2);
    assert_eq!(w.mother.state(done), Some(MissionState::Done));
    let run = w.port.run_of(done).expect("the mission's run");
    let runs = w.home.control.authority().runs();
    let view = runs.view(run).unwrap();
    assert_eq!(view.agent.to_string(), MOTHER_AGENT);
    assert_eq!(view.origin, RunOrigin::AgentGoal);
    assert_eq!(runs.class(run), Some(RunClass::Agent));
    assert!(view.finished, "the done mission's run is finished");
    assert_eq!(w.unserved(), 0, "exactly once");
    assert_eq!(w.port.performed(), 1);

    // The broker's refusal: an origin the Owner did not grant blocks the
    // mission, cancels its run, and reaches nothing.
    let other = TcpListener::bind("127.0.0.1:0").unwrap();
    other.set_nonblocking(true).unwrap();
    let ungranted = format!("FETCH-BUILDER:http://{}/x", other.local_addr().unwrap());
    let refused = w.approved(&ungranted);
    w.advance_to(refused, 2);
    assert_eq!(w.mother.advance(refused).unwrap().to, MissionState::Blocked);
    assert_eq!(
        w.mother.reason(refused).as_deref(),
        Some("broker_refused:no_covering_grant")
    );
    let refused_run = w.port.run_of(refused).unwrap();
    assert!(runs.view(refused_run).unwrap().cancelled);
    assert!(matches!(other.accept(), Err(e) if e.kind() == ErrorKind::WouldBlock));

    // The broker has no file route: a file effect is refused at the port.
    let file = w.approved("ASK-BUILDER-FILE");
    w.advance_to(file, 2);
    assert_eq!(w.mother.advance(file).unwrap().to, MissionState::Blocked);
    assert_eq!(
        w.mother.reason(file).as_deref(),
        Some("broker_refused:no_broker_route")
    );

    // Stop after an effect: the run is cancelled, nothing more reaches the
    // world, and a restarted Mother keeps it stopped.
    let stopped = w.approved(&format!("{fetch} TEXT-SKEPTIC:x|"));
    w.advance_to(stopped, 2);
    let served = w.serve_one();
    w.mother.advance(stopped).unwrap();
    assert_eq!(served.join().unwrap(), "GET /m1 HTTP/1.1");
    assert_eq!(w.mother.stop(stopped), Ok(MissionState::Stopped));
    let stopped_run = w.port.run_of(stopped).unwrap();
    assert!(runs.view(stopped_run).unwrap().cancelled);
    let restarted = open_mother(&w.home, &w.port);
    assert_eq!(restarted.state(stopped), Some(MissionState::Stopped));
    assert_eq!(restarted.state(done), Some(MissionState::Done));
    assert!(restarted.advance(stopped).is_err());
    assert_eq!(w.unserved(), 0);

    // Stop before any effect opens no run.
    let before = w.approved(&fetch);
    w.advance_to(before, 1);
    assert_eq!(w.mother.stop(before), Ok(MissionState::Stopped));
    assert!(w.mother.advance(before).is_err());
    assert_eq!(w.port.run_of(before), None);

    // M2a: waiting for the Owner's approval.
    m2a_waiting_on_the_real_broker(&w);
    // M2a Codex review: Stop withdraws a pending request after Mother and
    // her port are rebuilt.
    m2a_a_rebuilt_port_withdraws_on_stop(&w);
    // Added after mutation testing: a port takes back only Mother's own.
    m2a_a_port_adopts_only_mother_s_own_actions(&w);

    // The Owner's emergency stop stops Mother (last: it is not lifted).
    let emergency = w.approved(&fetch);
    w.advance_to(emergency, 2);
    let performed = w.port.performed();
    w.home.control.emergency_stop();
    assert_eq!(w.mother.advance(emergency), Err(CoordinatorError::Stopped));
    assert_eq!(w.mother.state(emergency), Some(MissionState::Stopped));
    assert_eq!(w.port.performed(), performed);
    assert_eq!(w.unserved(), 0);
}
