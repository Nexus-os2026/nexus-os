//! M2a-S Codex re-review #3 REJECT (oracles first, in their own file; they
//! do not replace or edit the earlier ones). Run from `real_broker.rs` (one
//! broker per process), with real threads.
//!
//! - MAJOR 1, the fence never waits: a broker call stalls where no
//!   cancellation reaches it (a test hook standing in for a DNS lookup) while
//!   another mission's Stop breaks the journal. The fence and the failing
//!   Stop return within 1 s while the call is still stalled; once it goes on,
//!   it does nothing at the broker or in the world. A run the broker opens
//!   after the fence is cancelled at once, never given to the mission.
//! - MAJOR 2: withdraw asks the broker for every handed commitment's own run
//!   and state, never trusting the record's strings: a record naming the
//!   mission's run R1 and commitment C2, where C2 is Prepared on run R2, is
//!   not withdrawn by cancelling R1.
//! - MINOR 4: the fence sets `fenced` before its first cancellation.

use super::m2as_support::{executed, rig, Disk, Rig, WorldServer};
use super::{GovernedPort, Home, MOTHER_AGENT};
use crate::coordinator::{Coordinator, Lanes};
use crate::effect::{EffectPort, PendingAction};
use crate::journal::{MemoryJournal, MissionId};
use crate::oracles::{contract_with, keyed_lanes, Yes};
use crate::state::MissionState;
use nexus_governed_control::authority::clock::SystemClock;
use nexus_governed_control::authority::commitment::CommitmentState;
use nexus_governed_control::authority::ids::{CommitmentId, RunId};
use nexus_governed_control::governed::GovernedControl;
use std::collections::BTreeSet;
use std::net::TcpListener;
use std::sync::mpsc;
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

/// The fence and the failing Stop return within this.
const PROMPT: Duration = Duration::from_secs(1);
/// The longest a thread waits for another at a meeting point (a safety
/// bound: a meeting that must happen is asserted).
const MEET: Duration = Duration::from_secs(10);
/// The longest a stalled call stays stalled if the test never releases it.
const STALL: Duration = Duration::from_secs(30);

#[derive(Clone, Default)]
struct Flag(Arc<(Mutex<bool>, Condvar)>);

impl Flag {
    fn set(&self) {
        *self.0 .0.lock().unwrap() = true;
        self.0 .1.notify_all();
    }

    fn is_set(&self) -> bool {
        *self.0 .0.lock().unwrap()
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

/// A broker call stalled where no cancellation reaches it: `stalled` once
/// it is, `gone_on` once `release` let it go on.
#[derive(Clone, Default)]
struct Stall {
    stalled: Flag,
    release: Flag,
    gone_on: Flag,
}

impl Stall {
    fn hook(&self) -> impl FnOnce() + Send + 'static {
        let stall = self.clone();
        move || {
            stall.stalled.set();
            let _ = stall.release.wait(STALL);
            stall.gone_on.set();
        }
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

fn mothers_runs(control: &GovernedControl) -> BTreeSet<RunId> {
    control
        .authority()
        .runs()
        .views()
        .iter()
        .filter(|run| run.agent.to_string() == MOTHER_AGENT)
        .map(|run| run.id)
        .collect()
}

/// Break the journal by stopping `victim` (its append and poisoning both
/// fail) on another thread, while `stall` holds a broker call. Returns
/// whether the Stop (and the fence inside it) returned within PROMPT with
/// the call still stalled; the call is released either way.
fn break_while_stalled(r: &Rig, disk: &Disk, victim: MissionId, stall: &Stall) -> bool {
    let fenced = Flag::default();
    {
        let fenced = fenced.clone();
        r.port.after_fence.arm(0, move || fenced.set());
    }
    disk.fail(true, true);
    let (sent, answer) = mpsc::channel();
    let mother = r.mother.clone();
    let started = Instant::now();
    let stop = std::thread::spawn(move || {
        let _ = sent.send(mother.stop(victim).is_err());
    });
    let in_time = answer.recv_timeout(PROMPT);
    let took = started.elapsed();
    let still_stalled = !stall.gone_on.is_set();
    stall.release.set();
    stop.join().unwrap();
    disk.fail(false, false);
    assert!(
        in_time.is_ok(),
        "the fence waited for a stalled broker call: the Stop had not returned after {took:?}"
    );
    assert!(in_time.unwrap(), "the failing Stop said Ok");
    assert!(fenced.is_set(), "the fence did not complete");
    assert!(r.mother.broken());
    still_stalled
}

/// A stalled action: B's broker call stalls just after the fence check
/// (the call has started); the break must not wait for it.
fn the_fence_never_waits_for_a_stalled_action(
    control: &Arc<GovernedControl>,
    origin: &str,
    world: &WorldServer,
) {
    let disk = Disk::over(MemoryJournal::new());
    let r = rig(control, disk.clone()).unwrap();
    let victim = approved(&r, "");
    r.mother.advance(victim).unwrap();
    let b = approved(
        &r,
        &format!("FETCH-BUILDER:{origin}/nowait-b-1 FETCH-VERIFIER:{origin}/nowait-b-2"),
    );
    for _ in 0..3 {
        r.mother.advance(b).unwrap();
    }
    let b_run = r.port.run_of(b).expect("B's run");
    let b_runs = BTreeSet::from([b_run]);
    let executed_before = executed(control, &b_runs);

    let stall = Stall::default();
    r.port.in_broker_action.arm(0, stall.hook());
    let mother = r.mother.clone();
    let b_thread = std::thread::spawn(move || mother.advance(b).map(|x| x.to));
    assert!(stall.stalled.wait(MEET), "B's broker call never started");
    assert!(
        break_while_stalled(&r, &disk, victim, &stall),
        "the Stop returned only once the stalled call went on"
    );
    let answer = b_thread.join().unwrap();

    assert!(answer.is_err(), "B answered {answer:?} after the break");
    assert!(
        !world.saw("/nowait-b-2"),
        "a stalled call reached the world after the break"
    );
    assert_eq!(
        executed(control, &b_runs),
        executed_before,
        "the broker ran it"
    );
    assert!(control.authority().runs().view(b_run).unwrap().cancelled);
}

/// A stalled open: B's call to open its run stalls once the fence was
/// checked. The break must not wait; the run the broker opens once the call
/// goes on is cancelled at once and never given to B.
fn the_fence_never_waits_for_a_stalled_open(
    control: &Arc<GovernedControl>,
    origin: &str,
    world: &WorldServer,
) {
    let disk = Disk::over(MemoryJournal::new());
    let r = rig(control, disk.clone()).unwrap();
    let victim = approved(&r, "");
    r.mother.advance(victim).unwrap();
    let b = approved(&r, &format!("FETCH-BUILDER:{origin}/nowait-open"));
    for _ in 0..2 {
        r.mother.advance(b).unwrap();
    }
    assert_eq!(r.port.run_of(b), None);
    let runs_before = mothers_runs(control);

    let stall = Stall::default();
    r.port.before_open_run.arm(0, stall.hook());
    let mother = r.mother.clone();
    let b_thread = std::thread::spawn(move || mother.advance(b).map(|x| x.to));
    assert!(stall.stalled.wait(MEET), "B never came to open its run");
    assert!(
        break_while_stalled(&r, &disk, victim, &stall),
        "the Stop returned only once the stalled open went on"
    );
    let answer = b_thread.join().unwrap();

    assert!(answer.is_err(), "B answered {answer:?} after the break");
    assert!(!world.saw("/nowait-open"), "an effect reached the world");
    assert_eq!(r.port.run_of(b), None, "B was given a run after the fence");
    let late: BTreeSet<RunId> = mothers_runs(control)
        .difference(&runs_before)
        .copied()
        .collect();
    let runs = control.authority().runs();
    for run in &late {
        assert!(
            runs.view(*run).unwrap().cancelled,
            "a run opened after the fence was left open"
        );
    }
    assert_eq!(executed(control, &late), 0, "a late run executed");
}

/// M holds R1 with nothing waiting; a record {run R1, commitment C2} is
/// handed back for M, where C2 is Prepared on R2. Cancelling R1 does not
/// withdraw C2: withdraw(M) is not Ok while C2 can be approved.
fn withdraw_asks_the_broker_for_a_handed_commitment(control: &Arc<GovernedControl>, origin: &str) {
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
    let m = approve(&format!("FETCH-BUILDER:{origin}/nowait-m"));
    let n = approve(&format!("POST-BUILDER:{origin}/nowait-n"));
    assert_eq!(mother.state(n), Some(MissionState::AwaitingApproval));
    let r1 = port.run_of(m).expect("R1");
    assert_eq!(port.pending(m), None, "M waits on nothing");
    let c2 = port.pending(n).expect("C2, waiting at the broker");
    assert_ne!(c2.run, r1.to_string());
    let commitment = CommitmentId::parse(&c2.commitment).unwrap();
    let prepared = || {
        control
            .authority()
            .commitments()
            .view(commitment)
            .is_some_and(|view| view.state == CommitmentState::Prepared)
    };

    let forged = PendingAction {
        run: r1.to_string(),
        ..c2.clone()
    };
    port.hand(m, &forged);
    assert!(prepared());
    assert!(
        port.withdraw(m).is_err(),
        "withdraw(M) said Ok while C2 (Prepared on R2, recorded under R1) can be approved"
    );
    assert!(prepared(), "C2 is still open: the error was due");
    assert!(control.authority().runs().view(r1).unwrap().cancelled);

    // C2 final at the broker: nothing more to withdraw.
    port.withdraw(n).unwrap();
    assert!(!prepared());
    assert!(port.withdraw(m).is_ok());
}

/// The fence sets `fenced` before its first cancellation (deterministic:
/// the hook is given the port's flag at that moment).
fn the_fence_is_fenced_before_its_first_cancellation(control: &Arc<GovernedControl>, origin: &str) {
    let disk = Disk::over(MemoryJournal::new());
    let r = rig(control, disk.clone()).unwrap();
    let victim = approved(&r, "");
    r.mother.advance(victim).unwrap();
    let b = approved(&r, &format!("FETCH-BUILDER:{origin}/nowait-first"));
    for _ in 0..3 {
        r.mother.advance(b).unwrap();
    }
    let b_run = r.port.run_of(b).expect("B's run");

    let seen: Arc<Mutex<Option<bool>>> = Arc::new(Mutex::new(None));
    {
        let seen = seen.clone();
        r.port
            .first_fence_cancel
            .arm(move |fenced| *seen.lock().unwrap() = Some(fenced));
    }
    disk.fail(true, true);
    assert!(r.mother.stop(victim).is_err());
    disk.fail(false, false);
    assert!(r.mother.broken());
    assert!(
        !r.port.first_fence_cancel.armed(),
        "the fence cancelled nothing"
    );
    assert_eq!(
        *seen.lock().unwrap(),
        Some(true),
        "the fence cancelled a run before it set fenced"
    );
    assert!(control.authority().runs().view(b_run).unwrap().cancelled);
}

pub(super) fn on_the_real_broker(home: &Home, origin: &str, listener: &TcpListener) {
    let world = WorldServer::start(listener);
    the_fence_never_waits_for_a_stalled_action(&home.control, origin, &world);
    the_fence_never_waits_for_a_stalled_open(&home.control, origin, &world);
    withdraw_asks_the_broker_for_a_handed_commitment(&home.control, origin);
    the_fence_is_fenced_before_its_first_cancellation(&home.control, origin);
    world.stop();
}
