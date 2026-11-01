//! M2a-S Codex re-review REJECT (oracle first), on the real broker with
//! real threads. Run from `real_broker.rs` (one broker per process).
//!
//! BLOCKER, the fence raced broker entry:
//! - (a) B holds a run and enters the broker's action while the fence is
//!   mid-cancel of another run (A's);
//! - (b) B passes the port's fence check, the fence completes, then B asks
//!   the broker to open a run.
//!
//! In both, nothing reaches the world and nothing executes at the broker.
//! In (a) no run is opened after the fence; in (b), after re-review #3 (the
//! fence never waits; Owner-approved amendment of its run-count assertion),
//! a run the broker opens after the fence is cancelled at once and never
//! given to B. Threads meet at test hooks in the port
//! (bounded waits, never a fixed sleep as the proof): the assertions are on
//! the broker's and the world's own records.
//!
//! MAJOR, a reused port: mission M holds run R1 and is handed back request
//! C2 of run R2; withdraw(M) is not Ok while C2 is open at the broker.

use super::m2as_support::{executed, rig, Disk, Rig, WorldServer};
use super::{GovernedPort, Home, MOTHER_AGENT};
use crate::coordinator::{Coordinator, Lanes};
use crate::effect::EffectPort;
use crate::journal::{MemoryJournal, MissionId};
use crate::oracles::{contract_with, keyed_lanes, Yes};
use crate::state::MissionState;
use nexus_governed_control::authority::clock::SystemClock;
use nexus_governed_control::authority::ids::RunId;
use nexus_governed_control::governed::GovernedControl;
use std::collections::BTreeSet;
use std::net::TcpListener;
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

/// The longest a thread waits for another at a meeting point (a safety
/// bound: a meeting that must happen is asserted).
const MEET: Duration = Duration::from_secs(10);
/// How long the paused fence leaves B to reach the world (only to make a
/// race visible where there is one; with the gate B cannot get in).
const GRACE: Duration = Duration::from_millis(500);

#[derive(Clone, Default)]
struct Flag(Arc<(Mutex<bool>, Condvar)>);

impl Flag {
    fn set(&self) {
        *self.0 .0.lock().unwrap() = true;
        self.0 .1.notify_all();
    }

    fn wait(&self, within: Duration) -> bool {
        let deadline = Instant::now() + within;
        let mut set = self.0 .0.lock().unwrap();
        while !*set {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return false;
            }
            set = self.0 .1.wait_timeout(set, left).unwrap().0;
        }
        true
    }
}

fn approved(r: &Rig, markers: &str) -> MissionId {
    let id = r.mother.draft(contract_with(markers)).unwrap();
    r.mother.submit(id).unwrap();
    assert_eq!(
        r.mother.request_consent(id, &Yes),
        Ok(MissionState::Approved)
    );
    id
}

fn mothers_run_ids(control: &GovernedControl) -> BTreeSet<RunId> {
    control
        .authority()
        .runs()
        .views()
        .iter()
        .filter(|run| run.agent.to_string() == MOTHER_AGENT)
        .map(|run| run.id)
        .collect()
}

fn mothers_runs(control: &GovernedControl) -> usize {
    control
        .authority()
        .runs()
        .views()
        .iter()
        .filter(|run| run.agent.to_string() == MOTHER_AGENT)
        .count()
}

/// The break: a Stop of `victim` whose append and poisoning both fail.
fn break_by_stopping(r: &Rig, disk: &Disk, victim: MissionId) {
    disk.fail(true, true);
    assert!(r.mother.stop(victim).is_err());
    disk.fail(false, false);
    assert!(r.mother.broken());
}

/// (a) B enters the broker's action while the fence is mid-cancel of A's
/// run.
fn an_action_entering_during_the_fence_is_refused(
    control: &Arc<GovernedControl>,
    origin: &str,
    world: &WorldServer,
) {
    let disk = Disk::over(MemoryJournal::new());
    let r = rig(control, disk.clone()).unwrap();
    let victim = approved(&r, "");
    r.mother.advance(victim).unwrap();
    // A's run is cancelled first (runs are held in mission order).
    let a = approved(&r, &format!("FETCH-BUILDER:{origin}/gate-a"));
    let b = approved(
        &r,
        &format!("FETCH-BUILDER:{origin}/gate-b-1 FETCH-VERIFIER:{origin}/gate-b-2"),
    );
    assert!(a < b);
    for _ in 0..3 {
        r.mother.advance(a).unwrap();
        r.mother.advance(b).unwrap();
    }
    let (a_run, b_run) = (r.port.run_of(a).unwrap(), r.port.run_of(b).unwrap());
    let b_runs = BTreeSet::from([b_run]);
    let executed_before = executed(control, &b_runs);
    let runs_before = mothers_runs(control);

    let (b_waiting, fence_mid, b_entering) = (Flag::default(), Flag::default(), Flag::default());
    {
        let (b_waiting, fence_mid, b_entering) =
            (b_waiting.clone(), fence_mid.clone(), b_entering.clone());
        r.port.before_broker_action.arm(0, move || {
            b_waiting.set();
            assert!(fence_mid.wait(MEET), "the fence never paused");
            b_entering.set();
        });
    }
    {
        let (fence_mid, b_entering, seen) =
            (fence_mid.clone(), b_entering.clone(), world.counter());
        r.port.fence_between_cancels.arm(0, move || {
            fence_mid.set();
            assert!(b_entering.wait(MEET), "B never came to the broker");
            let deadline = Instant::now() + GRACE;
            while Instant::now() < deadline
                && !seen.lock().unwrap().iter().any(|l| l.contains("/gate-b-2"))
            {
                std::thread::yield_now();
            }
        });
    }
    let mother = r.mother.clone();
    let b_thread = std::thread::spawn(move || mother.advance(b).map(|x| x.to));
    assert!(b_waiting.wait(MEET), "B never reached the broker");
    break_by_stopping(&r, &disk, victim);
    let answer = b_thread.join().unwrap();

    assert!(answer.is_err(), "B answered {answer:?} after the break");
    assert!(
        !world.saw("/gate-b-2"),
        "an effect entered the broker while the fence was cancelling"
    );
    assert_eq!(
        executed(control, &b_runs),
        executed_before,
        "the broker ran it"
    );
    assert_eq!(
        mothers_runs(control),
        runs_before,
        "a run was opened after the fence"
    );
    let runs = control.authority().runs();
    assert!(runs.view(a_run).unwrap().cancelled);
    assert!(runs.view(b_run).unwrap().cancelled);
}

/// (b) B passes the port's fence check, the fence completes, then B asks the
/// broker to open a run.
fn no_run_opens_after_the_fence(control: &Arc<GovernedControl>, origin: &str, world: &WorldServer) {
    let disk = Disk::over(MemoryJournal::new());
    let r = rig(control, disk.clone()).unwrap();
    let victim = approved(&r, "");
    r.mother.advance(victim).unwrap();
    let b = approved(&r, &format!("FETCH-BUILDER:{origin}/gate-open"));
    for _ in 0..2 {
        r.mother.advance(b).unwrap();
    }
    assert_eq!(r.port.run_of(b), None);

    let (b_checked, fence_done) = (Flag::default(), Flag::default());
    let after_fence: Arc<Mutex<Option<BTreeSet<RunId>>>> = Arc::new(Mutex::new(None));
    {
        let (b_checked, fence_done) = (b_checked.clone(), fence_done.clone());
        r.port.before_open_run.arm(0, move || {
            b_checked.set();
            // Wait for the fence to complete (it never waits for B).
            let _ = fence_done.wait(GRACE);
        });
    }
    {
        let (fence_done, after_fence, control) =
            (fence_done.clone(), after_fence.clone(), control.clone());
        r.port.after_fence.arm(0, move || {
            *after_fence.lock().unwrap() = Some(mothers_run_ids(&control));
            fence_done.set();
        });
    }
    let mother = r.mother.clone();
    let b_thread = std::thread::spawn(move || mother.advance(b).map(|x| x.to));
    assert!(b_checked.wait(MEET), "B never checked the fence");
    break_by_stopping(&r, &disk, victim);
    let answer = b_thread.join().unwrap();

    assert!(answer.is_err(), "B answered {answer:?} after the break");
    assert!(!world.saw("/gate-open"), "an effect reached the world");
    let at_fence = after_fence
        .lock()
        .unwrap()
        .clone()
        .expect("the fence completed");
    // Owner-approved amendment (re-review #3, the fence never waits): every
    // run opened after the fence is cancelled at once, never given to B, and
    // executes nothing (it replaces "the run count is unchanged").
    let late: BTreeSet<RunId> = mothers_run_ids(control)
        .difference(&at_fence)
        .copied()
        .collect();
    for run in &late {
        assert!(
            control.authority().runs().view(*run).unwrap().cancelled,
            "a run opened after the fence was left open"
        );
        assert_ne!(r.port.run_of(b), Some(*run), "B was given a late run");
    }
    assert_eq!(executed(control, &late), 0, "a late run executed");
    if let Some(run) = r.port.run_of(b) {
        assert!(control.authority().runs().view(run).unwrap().cancelled);
        assert_eq!(executed(control, &BTreeSet::from([run])), 0);
    }
}

/// A reused port: M holds R1; C2 of R2 is handed back for M. While C2 is
/// open at the broker, withdraw(M) is an error (the request it names is not
/// withdrawn by cancelling R1); once C2 is final, it is Ok.
fn withdraw_is_an_error_while_a_handed_request_of_another_run_is_open(
    control: &Arc<GovernedControl>,
    origin: &str,
) {
    let port = Arc::new(GovernedPort::new(control.clone()));
    let mother = Coordinator::open(
        MemoryJournal::new(),
        port.clone(),
        Lanes::new(keyed_lanes()).unwrap(),
        Arc::new(SystemClock::default()),
    )
    .unwrap();
    let approve = |markers: &str| {
        let id = mother.draft(contract_with(markers)).unwrap();
        mother.submit(id).unwrap();
        assert_eq!(mother.request_consent(id, &Yes), Ok(MissionState::Approved));
        for _ in 0..3 {
            mother.advance(id).unwrap();
        }
        id
    };
    let m = approve(&format!("FETCH-BUILDER:{origin}/gate-m"));
    let k = approve(&format!("FETCH-BUILDER:{origin}/gate-k"));
    let n = approve(&format!("POST-BUILDER:{origin}/gate-n"));
    assert_eq!(mother.state(n), Some(MissionState::AwaitingApproval));
    let (r1, r3) = (port.run_of(m).unwrap(), port.run_of(k).unwrap());
    let c2 = port.pending(n).expect("C2, waiting at the broker");
    assert_ne!(c2.run, r1.to_string());

    // Handed back without being taken (hand), and through adopt (refused:
    // K already holds a run).
    port.hand(m, &c2);
    port.adopt(k, &c2);
    assert!(
        port.withdraw(m).is_err(),
        "withdraw(M) said Ok while the handed request C2 is open"
    );
    assert!(
        port.withdraw(k).is_err(),
        "withdraw(K) said Ok while the handed request C2 is open"
    );
    let runs = control.authority().runs();
    assert!(runs.view(r1).unwrap().cancelled, "R1 is still withdrawn");
    assert!(runs.view(r3).unwrap().cancelled, "R3 is still withdrawn");

    // C2 final at the broker: nothing more to withdraw.
    port.withdraw(n).unwrap();
    assert!(port.withdraw(m).is_ok());
    assert!(port.withdraw(k).is_ok());
}

pub(super) fn on_the_real_broker(home: &Home, origin: &str, listener: &TcpListener) {
    let world = WorldServer::start(listener);
    an_action_entering_during_the_fence_is_refused(&home.control, origin, &world);
    no_run_opens_after_the_fence(&home.control, origin, &world);
    withdraw_is_an_error_while_a_handed_request_of_another_run_is_open(&home.control, origin);
    world.stop();
}
