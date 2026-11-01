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
use crate::journal::{verify, Entry, Event, JournalStore, MemoryJournal};
use crate::oracles::{contract_with, keyed_lanes, No, Yes};
use crate::state::MissionState;
use crate::state::Trigger;
use nexus_governed_control::authority::approval::{
    AcknowledgementConfirmation, ActionConfirmation, ControlConfirmer, GrantConfirmation,
    ResumeConfirmation,
};
use nexus_governed_control::authority::clock::SystemClock;
use nexus_governed_control::authority::commitment::{CommitmentState, CommitmentView};
use nexus_governed_control::authority::ids::{AgentId, RunId};
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
    // M2a-S (Owner decision): after a crash the mission is held until the
    // Owner resumes it.
    assert_eq!(mother.resume(kept), Ok(MissionState::AwaitingApproval));
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

/// Second fresh-context review (oracle first): a handed-back action is taken
/// only from a live Mother run no other mission of this port holds, and the
/// deadline it carries is never trusted beyond the broker's own.
fn m2a_a_port_adopts_only_live_unclaimed_actions(w: &World) {
    let control = &w.home.control;
    let agent = mother_agent();
    let post = |path: &str| {
        Intent::Request(EgressIntent {
            method: "POST".into(),
            url: format!("{}/{path}", w.origin),
            headers: vec![],
            body: None,
        })
    };
    let park = |path: &str| {
        let run = control
            .open_run(agent.clone(), RunOrigin::AgentGoal)
            .unwrap();
        let AgentOutcome::AwaitingApproval(view) =
            control.agent_action(&agent, run, &post(path)).unwrap()
        else {
            panic!("a POST waits for the Owner");
        };
        (run, view)
    };
    let action = |run: RunId, view: &CommitmentView, deadline| PendingAction {
        run: run.to_string(),
        commitment: view.id.to_string(),
        deadline_mono_ms: deadline,
    };
    let port = GovernedPort::new(control.clone());

    // A cancelled run (its commitment revoked) is not taken back.
    let (cancelled, view) = park("cancelled");
    control.cancel_run(cancelled).unwrap();
    port.adopt(MissionId(9101), &action(cancelled, &view, 0));
    assert_eq!(port.run_of(MissionId(9101)), None);

    // A live run is taken by one mission only.
    let (live, view) = park("live");
    port.adopt(MissionId(9102), &action(live, &view, u64::MAX));
    assert_eq!(port.run_of(MissionId(9102)), Some(live));
    port.adopt(MissionId(9103), &action(live, &view, u64::MAX));
    assert_eq!(port.run_of(MissionId(9103)), None, "a run already held");
    port.cancel(MissionId(9103));
    assert!(!control.authority().runs().view(live).unwrap().cancelled);

    // The carried deadline is clamped to the broker's lifetime from now.
    let lifetime = view.expires_wall_ms - view.created_wall_ms;
    let now = control.authority().clock().monotonic_ms();
    let taken = port.pending(MissionId(9102)).unwrap();
    assert!(taken.deadline_mono_ms <= now + lifetime, "{taken:?}");
    port.cancel(MissionId(9102));
    assert!(control.authority().runs().view(live).unwrap().cancelled);
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
    // Second review MINOR and NIT: only a live, unclaimed run; a deadline
    // never later than the broker's.
    m2a_a_port_adopts_only_live_unclaimed_actions(&w);
    // Codex re-review MINOR (a), as its own oracle (Codex re-review #2).
    m2a_an_adopted_deadline_is_within_the_remaining_lifetime(&w);
    // Codex re-review MAJOR: every journaled crash point, recovered by a
    // brand-new port and coordinator over the same live broker.
    m2a_crash_points_on_the_real_broker(&w);
    // Added after mutation testing: the port's own settled check.
    m2a_the_port_settles_only_mother_s_final_actions(&w);
    // Codex re-review #2 MAJOR: recovery is fail-closed for every failing
    // append.
    m2a_recovery_fails_closed_at_every_append_on_the_real_broker(&w);
    // Codex re-review #3 MINOR: the same with an abandoned lane call.
    m2a_recovery_fails_closed_with_an_abandoned_lane_on_the_real_broker(&w);
    // M2a-S Codex REJECT (oracles in their own files; one broker per
    // process, so they run here, before the emergency stop): the fence at
    // the broker, adoption before the session record, and the class test.
    super::m2as_fence::on_the_real_broker(&w.home, &w.origin, &w.listener);
    super::m2as_adopt::on_the_real_broker(&w.home, &w.origin, &w.listener);
    super::m2as_model::on_the_real_broker(&w.home, &w.origin, &w.listener);
    super::m2as_mutants::on_the_real_broker(&w.home, &w.origin, &w.listener);
    super::m2as_review2::on_the_real_broker(&w.home, &w.origin, &w.listener);
    super::m2as_gate::on_the_real_broker(&w.home, &w.origin, &w.listener);
    super::m2as_inflight::on_the_real_broker(&w.home, &w.origin, &w.listener);
    super::m2as_gate_review::on_the_real_broker(&w.home, &w.origin);
    super::m2as_nowait::on_the_real_broker(&w.home, &w.origin, &w.listener);
    super::m2as_poison::on_the_real_broker(&w.home, &w.origin, &w.listener);
    super::m2as_sweep::on_the_real_broker(&w.home, &w.origin, &w.listener);

    // The Owner's emergency stop stops Mother (last: it is not lifted).
    let emergency = w.approved(&fetch);
    w.advance_to(emergency, 2);
    let performed = w.port.performed();
    // Codex re-review step 4: a request of Mother's agent that her journal
    // never named (Mother died before journaling it). The Owner's
    // emergency stop at the front door withdraws it.
    let agent = mother_agent();
    let orphan_run = w
        .home
        .control
        .open_run(agent.clone(), RunOrigin::AgentGoal)
        .unwrap();
    let AgentOutcome::AwaitingApproval(orphan) = w
        .home
        .control
        .agent_action(&agent, orphan_run, &post_to(&w.origin, "orphan"))
        .unwrap()
    else {
        panic!("a POST waits for the Owner");
    };
    w.home.control.emergency_stop();
    assert_eq!(w.mother.advance(emergency), Err(CoordinatorError::Stopped));
    assert_eq!(w.mother.state(emergency), Some(MissionState::Stopped));
    assert_eq!(w.port.performed(), performed);
    assert!(w
        .home
        .control
        .authorize(orphan.id, &agent, orphan_run, &OwnerApproves)
        .is_err());
    assert!(w
        .home
        .control
        .execute(orphan.id, &agent, orphan_run)
        .is_err());
    assert_ne!(
        w.home
            .control
            .authority()
            .commitments()
            .view(orphan.id)
            .unwrap()
            .state,
        CommitmentState::Prepared
    );
    assert_eq!(w.unserved(), 0);
}

fn post_to(origin: &str, path: &str) -> Intent {
    Intent::Request(EgressIntent {
        method: "POST".into(),
        url: format!("{origin}/{path}"),
        headers: vec![],
        body: None,
    })
}

/// Append one entry to the journal at `path`, chained after its last
/// (an interruption simulated: what the coordinator journaled before it
/// died).
fn journal_append(path: &std::path::Path, mission: MissionId, event: Event) {
    let mut store = FileJournal::open(path).unwrap();
    let entries = verify(&store.load().unwrap()).unwrap();
    let last = entries.last().unwrap();
    let entry = Entry::chained(entries.len() as u64, &last.hash, mission, event);
    store.append(&entry.to_line()).unwrap();
}

/// Drop the journal's last entry (the coordinator died before writing it).
fn journal_drop_last(path: &std::path::Path) {
    let store = FileJournal::open(path).unwrap();
    let mut lines = store.load().unwrap();
    lines.pop();
    let text: String = lines.iter().map(|l| format!("{l}\n")).collect();
    std::fs::write(path, text).unwrap();
}

fn closures(path: &std::path::Path, mission: MissionId) -> usize {
    let store = FileJournal::open(path).unwrap();
    verify(&store.load().unwrap())
        .unwrap()
        .iter()
        .filter(|e| e.mission == mission && matches!(e.event, Event::ApprovalClosed { .. }))
        .count()
}

/// Codex re-review MINOR (a), moved out of the oracle above (which is
/// restored as it was at d71cb3df): a handed-back action's deadline is no
/// later than its remaining lifetime by the broker's wall-clock expiry,
/// read before the port took it.
fn m2a_an_adopted_deadline_is_within_the_remaining_lifetime(w: &World) {
    let control = &w.home.control;
    let agent = mother_agent();
    let run = control
        .open_run(agent.clone(), RunOrigin::AgentGoal)
        .unwrap();
    let AgentOutcome::AwaitingApproval(view) = control
        .agent_action(&agent, run, &post_to(&w.origin, "deadline"))
        .unwrap()
    else {
        panic!("a POST waits for the Owner");
    };
    let port = GovernedPort::new(control.clone());
    let wall_before = control.authority().clock().wall_ms();
    port.adopt(
        MissionId(9201),
        &PendingAction {
            run: run.to_string(),
            commitment: view.id.to_string(),
            deadline_mono_ms: u64::MAX,
        },
    );
    let mono_after = control.authority().clock().monotonic_ms();
    let taken = port.pending(MissionId(9201)).unwrap();
    assert!(
        taken.deadline_mono_ms <= mono_after + view.expires_wall_ms.saturating_sub(wall_before),
        "{taken:?}"
    );
    port.cancel(MissionId(9201));
    assert!(control.authority().runs().view(run).unwrap().cancelled);
    assert_eq!(w.unserved(), 0);
}

/// Codex re-review #2 MAJOR (oracle first): recovery is fail-closed. Three
/// requests are left open by a dead coordinator: a stopped mission's (its
/// Stop journaled, never withdrawn), a waiting mission's, and an
/// interrupted mission's (identity journaled, wait not). For every N, a
/// journal that fails its Nth append during recovery makes `open` fail
/// and leaves no request approvable or executable; with no failure,
/// `open` succeeds and only the waiting request stays open.
fn m2a_recovery_fails_closed_at_every_append_on_the_real_broker(w: &World) {
    let control = &w.home.control;
    let agent = mother_agent();
    let lanes = || Lanes::new(keyed_lanes()).unwrap();
    let clock = || Arc::new(SystemClock::default());
    // Leave three requests open in a fresh journal; returns the journal's
    // lines and each request's run and commitment.
    let scene = |round: usize| {
        let journal = MemoryJournal::new();
        let port = Arc::new(GovernedPort::new(control.clone()));
        let mother = Coordinator::open(journal.clone(), port.clone(), lanes(), clock()).unwrap();
        let mut requests = Vec::new();
        for n in 1..=3u64 {
            let id = mother
                .draft(contract_with(&format!(
                    "POST-BUILDER:{}/fault-{round}-{n}",
                    w.origin
                )))
                .unwrap();
            assert_eq!(id, MissionId(n));
            mother.submit(id).unwrap();
            mother.request_consent(id, &Yes).unwrap();
            for _ in 0..3 {
                mother.advance(id).unwrap();
            }
            assert_eq!(mother.state(id), Some(MissionState::AwaitingApproval));
            let pending = port.pending(id).unwrap();
            requests.push((
                RunId::parse(&pending.run).unwrap(),
                nexus_governed_control::authority::ids::CommitmentId::parse(&pending.commitment)
                    .unwrap(),
            ));
        }
        drop(mother);
        drop(port);
        // Mission 3 died before its wait was journaled; mission 1's Stop
        // was journaled and never withdrawn.
        let mut lines = journal.lines();
        lines.pop();
        let entries = verify(&lines).unwrap();
        let last = entries.last().unwrap();
        let stop = Entry::chained(
            entries.len() as u64,
            &last.hash,
            MissionId(1),
            Event::Transition {
                trigger: Trigger::Stop {
                    reason: "owner".into(),
                },
            },
        );
        lines.push(stop.to_line());
        (lines, requests)
    };
    let unapprovable = |(run, commitment): (RunId, _), why: &str| {
        let state = control
            .authority()
            .commitments()
            .view(commitment)
            .unwrap()
            .state;
        assert!(
            !matches!(
                state,
                CommitmentState::Prepared | CommitmentState::Authorized
            ),
            "{why}: {state:?}"
        );
        assert!(
            control
                .authorize(commitment, &agent, run, &OwnerApproves)
                .is_err(),
            "{why}"
        );
        assert!(control.execute(commitment, &agent, run).is_err(), "{why}");
    };

    // How many appends a recovery that does not fail makes.
    let (lines, requests) = scene(0);
    let journal = MemoryJournal::new();
    journal.set_lines(lines.clone());
    let port = Arc::new(GovernedPort::new(control.clone()));
    let mother = Coordinator::open(journal.clone(), port, lanes(), clock()).unwrap();
    let appends = journal.lines().len() - lines.len();
    assert!(appends >= 3, "{appends}");
    assert_eq!(
        mother.state(MissionId(2)),
        Some(MissionState::AwaitingApproval)
    );
    unapprovable(requests[0], "no failure: the stopped mission's");
    unapprovable(requests[2], "no failure: the interrupted mission's");
    assert_eq!(
        control
            .authority()
            .commitments()
            .view(requests[1].1)
            .unwrap()
            .state,
        CommitmentState::Prepared,
        "no failure: the waiting request stays open"
    );
    mother.stop(MissionId(2)).unwrap();
    unapprovable(requests[1], "no failure, then Stop");
    drop(mother);

    for n in 0..appends {
        let (lines, requests) = scene(n + 1);
        let journal = MemoryJournal::new();
        journal.set_lines(lines);
        journal.fail_after(n);
        let port = Arc::new(GovernedPort::new(control.clone()));
        assert!(
            Coordinator::open(journal, port, lanes(), clock()).is_err(),
            "append {n} failed, yet open succeeded"
        );
        for (i, request) in requests.into_iter().enumerate() {
            unapprovable(request, &format!("append {n} failed: request {}", i + 1));
        }
    }
    assert_eq!(w.unserved(), 0);
}

/// Codex re-review #3 MINOR (new oracle; the one above is unchanged): a
/// dead coordinator leaves a waiting request (mission 1), a stopped
/// mission's request never withdrawn (mission 2) and an unanswered lane
/// call (mission 3, which recovery journals abandoned). For every N, a
/// journal failing its Nth recovery append makes `open` fail, and neither
/// request stays approvable or executable.
fn m2a_recovery_fails_closed_with_an_abandoned_lane_on_the_real_broker(w: &World) {
    let control = &w.home.control;
    let agent = mother_agent();
    let lanes = || Lanes::new(keyed_lanes()).unwrap();
    let clock = || Arc::new(SystemClock::default());
    let scene = |round: usize| {
        let journal = MemoryJournal::new();
        let port = Arc::new(GovernedPort::new(control.clone()));
        let mother = Coordinator::open(journal.clone(), port.clone(), lanes(), clock()).unwrap();
        let mut requests = Vec::new();
        for n in 1..=2u64 {
            let id = mother
                .draft(contract_with(&format!(
                    "POST-BUILDER:{}/lane-{round}-{n}",
                    w.origin
                )))
                .unwrap();
            mother.submit(id).unwrap();
            mother.request_consent(id, &Yes).unwrap();
            for _ in 0..3 {
                mother.advance(id).unwrap();
            }
            assert_eq!(mother.state(id), Some(MissionState::AwaitingApproval));
            let pending = port.pending(id).unwrap();
            requests.push((
                RunId::parse(&pending.run).unwrap(),
                nexus_governed_control::authority::ids::CommitmentId::parse(&pending.commitment)
                    .unwrap(),
            ));
        }
        let lane = mother.draft(contract_with("")).unwrap();
        assert_eq!(lane, MissionId(3));
        mother.submit(lane).unwrap();
        mother.request_consent(lane, &Yes).unwrap();
        mother.advance(lane).unwrap();
        assert_eq!(mother.state(lane), Some(MissionState::Investigating));
        drop(mother);
        drop(port);
        let mut lines = journal.lines();
        let mut push = |mission: MissionId, event: Event| {
            let entries = verify(&lines).unwrap();
            let entry = Entry::chained(
                entries.len() as u64,
                &entries.last().unwrap().hash,
                mission,
                event,
            );
            lines.push(entry.to_line());
        };
        push(
            MissionId(2),
            Event::Transition {
                trigger: Trigger::Stop {
                    reason: "owner".into(),
                },
            },
        );
        let seq = verify(&journal.lines()).unwrap().len() as u64 + 1;
        push(
            lane,
            Event::LaneInvoked {
                invocation: crate::lane::InvocationId { mission: lane, seq },
                identity: crate::lane::LaneIdentity {
                    role: crate::lane::LaneRole::Sentinel,
                    name: "keyed-sentinel".into(),
                    version: "m1-test".into(),
                },
                input_hash: "0".repeat(64),
            },
        );
        (lines, requests)
    };
    let unapprovable = |(run, commitment): (RunId, _), why: &str| {
        let state = control
            .authority()
            .commitments()
            .view(commitment)
            .unwrap()
            .state;
        assert!(
            !matches!(
                state,
                CommitmentState::Prepared | CommitmentState::Authorized
            ),
            "{why}: {state:?}"
        );
        assert!(
            control
                .authorize(commitment, &agent, run, &OwnerApproves)
                .is_err(),
            "{why}"
        );
        assert!(control.execute(commitment, &agent, run).is_err(), "{why}");
    };

    // How many appends a recovery that does not fail makes.
    let (lines, requests) = scene(0);
    let journal = MemoryJournal::new();
    journal.set_lines(lines.clone());
    let mother = Coordinator::open(
        journal.clone(),
        Arc::new(GovernedPort::new(control.clone())),
        lanes(),
        clock(),
    )
    .unwrap();
    let appends = journal.lines().len() - lines.len();
    assert!(appends >= 2, "{appends}");
    assert!(verify(&journal.lines())
        .unwrap()
        .iter()
        .any(|e| e.mission == MissionId(3) && matches!(e.event, Event::LaneOvertaken { .. })));
    unapprovable(requests[1], "no failure: the stopped mission's");
    mother.stop(MissionId(1)).unwrap();
    unapprovable(requests[0], "no failure, then Stop");
    drop(mother);

    for n in 0..appends {
        let (lines, requests) = scene(n + 1);
        let journal = MemoryJournal::new();
        journal.set_lines(lines);
        journal.fail_after(n);
        let port = Arc::new(GovernedPort::new(control.clone()));
        assert!(
            Coordinator::open(journal, port, lanes(), clock()).is_err(),
            "append {n} failed, yet open succeeded"
        );
        for (i, request) in requests.into_iter().enumerate() {
            unapprovable(request, &format!("append {n} failed: request {}", i + 1));
        }
    }
    assert_eq!(w.unserved(), 0);
}

/// Added after mutation testing (two mutants of `settled` survived the
/// crash-point oracles, where a cancel always came first): an action is
/// settled only when it is Mother's and final at the broker, or the broker
/// holds no such action.
fn m2a_the_port_settles_only_mother_s_final_actions(w: &World) {
    let control = &w.home.control;
    let port = GovernedPort::new(control.clone());
    let park = |agent: &AgentId, path: &str| {
        let run = control
            .open_run(agent.clone(), RunOrigin::AgentGoal)
            .unwrap();
        let AgentOutcome::AwaitingApproval(view) = control
            .agent_action(agent, run, &post_to(&w.origin, path))
            .unwrap()
        else {
            panic!("a POST waits for the Owner");
        };
        (run, view.id)
    };
    let named = |run: RunId, commitment: &dyn std::fmt::Display| PendingAction {
        run: run.to_string(),
        commitment: commitment.to_string(),
        deadline_mono_ms: 0,
    };
    let mother = mother_agent();
    let other = AgentId::new("another-agent").unwrap();
    let (run, commitment) = park(&mother, "settle-mine");
    let (other_run, other_commitment) = park(&other, "settle-other");
    // Waiting at the broker: not settled.
    assert!(!port.settled(&named(run, &commitment)));
    // Someone else's action, or Mother's run with someone else's commitment.
    assert!(!port.settled(&named(other_run, &other_commitment)));
    assert!(!port.settled(&named(run, &other_commitment)));
    // A commitment the broker does not hold.
    assert!(port.settled(&named(run, &"cmt-00000000000000000000000000000000")));
    // Withdrawn: settled. Someone else's stays unsettled even when final.
    control.cancel_run(run).unwrap();
    control.cancel_run(other_run).unwrap();
    assert!(port.settled(&named(run, &commitment)));
    assert!(!port.settled(&named(other_run, &other_commitment)));
    assert_eq!(w.unserved(), 0);
}

/// Codex re-review MAJOR (oracle first): each point at which Mother can be
/// interrupted after a request's identity is journaled. Each runs on a
/// journal of its own: a first port and coordinator park a request and
/// die (dropped, nothing cancelled) after journaling what they had
/// reached; a brand-new port and coordinator over the same live broker
/// recover. Wherever the mission no longer waits, the request must end up
/// unapprovable, with its closure journaled once, whatever the mission's
/// state.
fn m2a_crash_points_on_the_real_broker(w: &World) {
    let control = &w.home.control;
    let agent = mother_agent();
    let open = |path: &std::path::Path, port: &Arc<GovernedPort>| {
        Coordinator::open(
            FileJournal::open(path).unwrap(),
            port.clone(),
            Lanes::new(keyed_lanes()).unwrap(),
            Arc::new(SystemClock::default()),
        )
        .unwrap()
    };
    let id = MissionId(1);
    // Park one request on a fresh journal; returns its run and commitment.
    let park = |name: &str| {
        let path = w.home.dir.join(format!("crash-{name}.jsonl"));
        let port = Arc::new(GovernedPort::new(control.clone()));
        let mother = open(&path, &port);
        assert_eq!(
            mother.draft(contract_with(&format!(
                "POST-BUILDER:{}/crash-{name}",
                w.origin
            ))),
            Ok(id)
        );
        mother.submit(id).unwrap();
        assert_eq!(mother.request_consent(id, &Yes), Ok(MissionState::Approved));
        for _ in 0..3 {
            mother.advance(id).unwrap();
        }
        assert_eq!(mother.state(id), Some(MissionState::AwaitingApproval));
        let pending = port.pending(id).unwrap();
        let run = RunId::parse(&pending.run).unwrap();
        let commitment = control
            .authority()
            .commitments()
            .views_of_run(run)
            .into_iter()
            .find(|v| v.id.to_string() == pending.commitment)
            .unwrap();
        assert_eq!(commitment.state, CommitmentState::Prepared);
        (path, run, commitment.id)
    };
    let unapprovable = |run, commitment, point: &str| {
        let state = control
            .authority()
            .commitments()
            .view(commitment)
            .unwrap()
            .state;
        assert!(
            !matches!(
                state,
                CommitmentState::Prepared | CommitmentState::Authorized
            ),
            "{point}: {state:?}"
        );
        assert!(
            control
                .authorize(commitment, &agent, run, &OwnerApproves)
                .is_err(),
            "{point}"
        );
        assert!(control.execute(commitment, &agent, run).is_err(), "{point}");
    };
    // Recover with a brand-new port and coordinator; then once more, to
    // see the closure is not repeated.
    let recover = |path: &std::path::Path, expected: MissionState, run, commitment, point: &str| {
        let mother = open(path, &Arc::new(GovernedPort::new(control.clone())));
        assert_eq!(mother.state(id), Some(expected), "{point}");
        drop(mother);
        unapprovable(run, commitment, point);
        assert_eq!(closures(path, id), 1, "{point}: closure journaled once");
        let again = open(path, &Arc::new(GovernedPort::new(control.clone())));
        assert_eq!(again.state(id), Some(expected), "{point}");
        drop(again);
        assert_eq!(closures(path, id), 1, "{point}: closure not repeated");
    };

    // A. The identity was journaled, the wait was not.
    let (path, run, commitment) = park("a");
    journal_drop_last(&path);
    recover(
        &path,
        MissionState::Blocked,
        run,
        commitment,
        "A: identity, no wait",
    );

    // B. The wait was journaled: the mission still waits and the Owner may
    // still approve; a Stop through the rebuilt port withdraws it.
    let (path, run, commitment) = park("b");
    let port = Arc::new(GovernedPort::new(control.clone()));
    let mother = open(&path, &port);
    assert_eq!(mother.state(id), Some(MissionState::AwaitingApproval));
    assert_eq!(
        control
            .authority()
            .commitments()
            .view(commitment)
            .unwrap()
            .state,
        CommitmentState::Prepared
    );
    assert_eq!(closures(&path, id), 0, "B: a waiting request is not closed");
    assert_eq!(mother.stop(id), Ok(MissionState::Stopped));
    drop(mother);
    unapprovable(run, commitment, "B: Stop after rebuild");
    assert_eq!(closures(&path, id), 1, "B");

    // C. Stop was journaled; the port was never cancelled.
    let (path, run, commitment) = park("c");
    journal_append(
        &path,
        id,
        Event::Transition {
            trigger: Trigger::Stop {
                reason: "owner".into(),
            },
        },
    );
    recover(
        &path,
        MissionState::Stopped,
        run,
        commitment,
        "C: Stop, no cancel",
    );

    // D. The wait lapsed (Mother's budget) and was journaled; no cancel.
    let (path, run, commitment) = park("d");
    journal_append(
        &path,
        id,
        Event::Transition {
            trigger: Trigger::ApprovalLapsed {
                effect: 0,
                reason: "budget_time".into(),
            },
        },
    );
    recover(
        &path,
        MissionState::Blocked,
        run,
        commitment,
        "D: lapse, no cancel",
    );

    // E. The Owner denied it and the denial was journaled; no cancel.
    let (path, run, commitment) = park("e");
    control.deny(commitment, &agent, run).unwrap();
    journal_append(
        &path,
        id,
        Event::Transition {
            trigger: Trigger::ActionDenied { effect: 0 },
        },
    );
    recover(
        &path,
        MissionState::Denied,
        run,
        commitment,
        "E: deny, no cancel",
    );

    // F. Stop was journaled and the port cancelled, but the closure was
    // not journaled.
    let (path, run, commitment) = park("f");
    journal_append(
        &path,
        id,
        Event::Transition {
            trigger: Trigger::Stop {
                reason: "owner".into(),
            },
        },
    );
    control.cancel_run(run).unwrap();
    recover(
        &path,
        MissionState::Stopped,
        run,
        commitment,
        "F: cancelled, not closed",
    );

    // G. The Owner approved and ran it and the approval was journaled; the
    // closure was not: the restart blocks the mission mid-phase, and the
    // action ran once.
    let (path, run, commitment) = park("g");
    let served = w.serve_one();
    control
        .authorize(commitment, &agent, run, &OwnerApproves)
        .unwrap();
    control.execute(commitment, &agent, run).unwrap();
    assert_eq!(served.join().unwrap(), "POST /crash-g HTTP/1.1");
    journal_append(
        &path,
        id,
        Event::Transition {
            trigger: Trigger::ActionApproved {
                effect: 0,
                phase: MissionState::Building,
            },
        },
    );
    recover(
        &path,
        MissionState::Blocked,
        run,
        commitment,
        "G: approved, not closed",
    );
    assert_eq!(w.unserved(), 0, "G: exactly once");
}
