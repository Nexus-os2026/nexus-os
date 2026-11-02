//! M2c-B (PR-B) Mother oracles on the real broker, pre-registered before the
//! halt exists (design v3 §4.5, §4.8, §6, §7.3, §11 C3): M1–M8, A6-g
//! extended to a door halt. M9 (resume across the port and the real store:
//! A6-a, A6-c and A6-d) is `m2c_door.rs`'s, which already runs on this
//! broker. Run from `real_broker_m1_oracles` (one broker per process),
//! before its emergency stop.
//!
//! A broker write is stalled by the test home's gated projection (it holds
//! the writer of a matching record, after its commit, with no store lock
//! held); a Mother call is held by the port's own hooks. Threads are
//! joined through channels with a bounded wait; a wait that runs out is a
//! finding, never a pass. M1, M2 and M3 are red on PR-A, behaviourally.

use super::m2as_support::{executed, owner_runs, rig, Disk, Rig, WorldServer};
use super::m2c_support::{
    after_held_answer, arm_call_point, consent, door_open_run, fence_port, halt_lost, halt_pending,
    resume_with, try_port_for, CallPoint, HeldOwner, Home,
};
use super::GovernedPort;
use crate::coordinator::Coordinator;
use crate::journal::{MemoryJournal, MissionId};
use crate::oracles::{contract_with, Yes};
use crate::state::MissionState;
use nexus_governed_control::authority::commitment::{CommitmentState, CommitmentView};
use nexus_governed_control::authority::ids::RunId;
use nexus_governed_control::governed::GovernedControl;
use std::collections::BTreeSet;
use std::net::TcpListener;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Weak};
use std::thread::ThreadId;
use std::time::Duration;

/// A bounded join: it detects a hang and never makes a test pass.
const JOIN: Duration = Duration::from_secs(10);

/// Run `f` on a thread.
fn spawn<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> Receiver<T> {
    let (done, result) = channel();
    std::thread::spawn(move || {
        let _ = done.send(f());
    });
    result
}

/// Its result, or none if it did not come within the bound (or panicked).
fn within<T>(result: &Receiver<T>) -> Option<T> {
    result.recv_timeout(JOIN).ok()
}

/// A Mother call held at a port hook until released (or until this guard
/// drops).
struct Held {
    entered: Receiver<()>,
    release: Option<Sender<()>>,
}

impl Held {
    fn reached(&self) {
        self.entered
            .recv_timeout(JOIN)
            .expect("the held call never reached its hook");
    }

    fn release(mut self) {
        self.release.take();
    }
}

impl Drop for Held {
    fn drop(&mut self) {
        self.release.take();
    }
}

fn held() -> (Held, impl FnOnce() + Send + 'static) {
    let (entered, reached) = channel();
    let (release, released) = channel::<()>();
    let hook = move || {
        let _ = entered.send(());
        // Longer than a bounded join, so that a wait on a held call shows.
        let _ = released.recv_timeout(3 * JOIN);
    };
    (
        Held {
            entered: reached,
            release: Some(release),
        },
        hook,
    )
}

fn approved(r: &Rig, markers: &str) -> MissionId {
    let id = r.mother.draft(contract_with(markers)).unwrap();
    r.mother.submit(id).unwrap();
    assert_eq!(consent(&r.mother, id, &Yes), Ok(MissionState::Approved));
    id
}

/// A Stop of `victim` whose append fails (and whose poisoning fails too
/// unless `marker`), so the coordinator breaks, on the calling thread.
/// Returns that thread's id.
fn breaker(
    disk: &Disk,
    mother: &Arc<Coordinator<GovernedPort>>,
    victim: MissionId,
    marker: bool,
) -> impl FnOnce() -> ThreadId + Send + 'static {
    let (disk, mother): (Disk, Weak<Coordinator<GovernedPort>>) =
        (disk.clone(), Arc::downgrade(mother));
    move || {
        disk.fail(true, !marker);
        let mother = mother.upgrade().expect("the coordinator");
        assert!(mother.stop(victim).is_err(), "the Stop is not durable");
        assert!(mother.broken());
        disk.fail(false, false);
        std::thread::current().id()
    }
}

fn fresh(control: &Arc<GovernedControl>) -> (Rig, Disk, MissionId) {
    let disk = Disk::over(MemoryJournal::new());
    let r = rig(control, disk.clone()).unwrap();
    let victim = approved(&r, "");
    r.mother.advance(victim).unwrap();
    (r, disk, victim)
}

/// A mission whose Builder's POST waits for the Owner: its run.
fn waiting(r: &Rig, origin: &str, path: &str) -> (MissionId, RunId) {
    let id = approved(r, &format!("POST-BUILDER:{origin}/{path}"));
    for _ in 0..3 {
        r.mother.advance(id).unwrap();
    }
    assert_eq!(r.mother.state(id), Some(MissionState::AwaitingApproval));
    (id, r.port.run_of(id).expect("its run"))
}

fn waiting_view(control: &GovernedControl, run: RunId) -> CommitmentView {
    let views: Vec<CommitmentView> = control
        .authority()
        .commitments()
        .views_of_run(run)
        .into_iter()
        .filter(|v| v.state == CommitmentState::Prepared)
        .collect();
    assert_eq!(views.len(), 1, "{views:?}");
    views.into_iter().next().unwrap()
}

fn cancelled(control: &GovernedControl, run: RunId) -> bool {
    control.authority().runs().view(run).unwrap().cancelled
}

/// M1 (PR #49 MAJOR 1; red on PR-A): missions A and B have runs, A first.
/// A's cancellation record is held; B, already past the port's fenced
/// check, then goes to the broker. B is refused: the world sees nothing of
/// it and its run executes nothing. The world is read before the gate
/// opens.
fn m2c_m1_an_effect_past_enter_is_refused_after_the_break(
    home: &Home,
    origin: &str,
    world: &WorldServer,
) {
    let control = &home.control;
    let (r, disk, victim) = fresh(control);
    let (_a, run_a) = waiting(&r, origin, "m2c-m1-a");
    let b = approved(&r, &format!("FETCH-BUILDER:{origin}/m2c-m1-b"));
    r.mother.advance(b).unwrap();
    r.mother.advance(b).unwrap();
    let (b_at, b_hook) = held();
    r.port.in_broker_action.arm(0, b_hook);
    let mother = r.mother.clone();
    let b_call = spawn(move || mother.advance(b).map(|a| a.to));
    b_at.reached();
    let run_b = r.port.run_of(b).expect("B's run");
    home.gate.arm("run_cancelled", Some(run_a));
    let breaking = spawn(breaker(&disk, &r.mother, victim, false));
    assert!(home.gate.wait_held(), "A's cancellation record is held");
    b_at.release();
    let b_done = within(&b_call);
    let reached = world.saw("/m2c-m1-b");
    home.gate.open();
    let _ = within(&breaking);
    assert!(
        !reached,
        "M1: B's request reached the world while A's cancellation was recorded"
    );
    assert!(b_done.is_some(), "M1: B waited for the broker's records");
    assert_eq!(executed(control, &BTreeSet::from([run_b])), 0);
    assert!(cancelled(control, run_b));
}

/// M2 (red on PR-A): every cancellation record is held, and the journal
/// breaks. While the gate is closed another thread's coordinator call
/// completes; B, held before its run opens, gets no usable run; the door
/// refuses.
fn m2c_m2_a_stalled_store_holds_no_lock(home: &Home, origin: &str, world: &WorldServer) {
    let control = &home.control;
    let (r, disk, victim) = fresh(control);
    let (a, _run_a) = waiting(&r, origin, "m2c-m2-a");
    let b = approved(&r, &format!("FETCH-BUILDER:{origin}/m2c-m2-b"));
    r.mother.advance(b).unwrap();
    r.mother.advance(b).unwrap();
    let (b_at, b_hook) = held();
    r.port.before_open_run.arm(0, b_hook);
    let mother = r.mother.clone();
    let b_call = spawn(move || mother.advance(b).map(|a| a.to));
    b_at.reached();
    home.gate.arm("run_cancelled", None);
    let breaking = spawn(breaker(&disk, &r.mother, victim, false));
    assert!(home.gate.wait_held(), "a cancellation record is held");
    let mother = r.mother.clone();
    let other = within(&spawn(move || mother.state(a)));
    b_at.release();
    let b_done = within(&b_call);
    let refused = door_open_run(&r.port);
    home.gate.open();
    let _ = within(&breaking);
    assert!(
        other.is_some(),
        "M2: another coordinator call waited for the stalled store"
    );
    assert!(
        b_done.as_ref().is_some_and(|b| b.is_err()),
        "M2: B got a usable run: {b_done:?}"
    );
    assert_eq!(refused, Err("halted".to_string()));
    assert!(!world.saw("/m2c-m2-b"));
}

/// M3 (red on PR-A): the fence panics before its first cancellation, with B
/// past the port's fenced check. B resumes before anything takes the port's
/// lock; the broker, read on the Owner's side, refuses B with zero effects
/// and has cancelled B's run, and the lost batch is counted.
fn m2c_m3_a_panic_in_the_fence_before_any_cancellation(
    home: &Home,
    origin: &str,
    world: &WorldServer,
) {
    let control = &home.control;
    let (r, disk, victim) = fresh(control);
    let b = approved(&r, &format!("FETCH-BUILDER:{origin}/m2c-m3-b"));
    r.mother.advance(b).unwrap();
    r.mother.advance(b).unwrap();
    let (b_at, b_hook) = held();
    r.port.in_broker_action.arm(0, b_hook);
    let mother = r.mother.clone();
    let b_call = spawn(move || mother.advance(b).map(|a| a.to));
    b_at.reached();
    let run_b = r.port.run_of(b).expect("B's run");
    r.port
        .first_fence_cancel
        .arm(|_| panic!("the fence panics before its first cancellation (test)"));
    let fail = breaker(&disk, &r.mother, victim, false);
    let breaking =
        spawn(move || std::panic::catch_unwind(std::panic::AssertUnwindSafe(fail)).is_err());
    let panicked = within(&breaking);
    b_at.release();
    let _ = within(&b_call);
    assert!(
        !world.saw("/m2c-m3-b"),
        "M3: B's effect ran after the fence panicked"
    );
    assert_eq!(executed(control, &BTreeSet::from([run_b])), 0);
    assert!(cancelled(control, run_b), "M3: B's run is cancelled");
    assert!(halt_lost(control) > 0, "M3: the dropped batch is counted");
    assert_eq!(panicked, Some(true));
    disk.fail(false, false);
}

/// M4 (regression; green on PR-A too): a Stop holds no lock while it writes.
/// A's cancellation record (matched by A's run only) is held; meanwhile
/// B's Stop completes, and A's waiting request cannot be approved or run.
fn m2c_m4_stop_holds_no_lock_while_it_writes(home: &Home, origin: &str) {
    let control = &home.control;
    let r = rig(control, Disk::over(MemoryJournal::new())).unwrap();
    let (a, run_a) = waiting(&r, origin, "m2c-m4-a");
    let (b, _run_b) = waiting(&r, origin, "m2c-m4-b");
    let view_a = waiting_view(control, run_a);
    home.gate.arm("run_cancelled", Some(run_a));
    let mother = r.mother.clone();
    let stop_a = spawn(move || mother.stop(a));
    assert!(home.gate.wait_held(), "A's cancellation record is held");
    let mother = r.mother.clone();
    let b_done = within(&spawn(move || mother.stop(b)));
    let approvable = owner_runs(control, &view_a);
    home.gate.open();
    let a_done = within(&stop_a);
    assert_eq!(b_done, Some(Ok(MissionState::Stopped)));
    assert!(
        !approvable,
        "M4: A's request ran while its Stop was recorded"
    );
    assert_eq!(a_done, Some(Ok(MissionState::Stopped)));
}

/// M5: a coordinator reopened on a new door takes back a waiting action of
/// an earlier, dropped door; a break cancels that run.
fn m2c_m5_adopted_runs_are_covered(home: &Home, origin: &str) {
    let control = &home.control;
    let disk = Disk::over(MemoryJournal::new());
    let first = rig(control, disk.clone()).unwrap();
    let (id, run) = waiting(&first, origin, "m2c-m5");
    let view = waiting_view(control, run);
    drop(first);
    let r = rig(control, disk.clone()).unwrap();
    assert_eq!(r.port.run_of(id), Some(run), "taken back");
    let victim = r.mother.draft(contract_with("")).unwrap();
    breaker(&disk, &r.mother, victim, false)();
    assert!(cancelled(control, run), "M5: the adopted run is cancelled");
    assert!(!owner_runs(control, &view));
}

/// M6: only the Owner lets Mother act. Every new port's door is asked for:
/// after a break, after a clean close, and in the same process with
/// neither. No gives no port; yes gives a working door; the old door is
/// Halted on its acting calls.
fn m2c_m6_only_the_owner_lets_mother_act(home: &Home) {
    let control = &home.control;
    let asked = || home.gate.count("agent_door_opened") + home.gate.count("agent_door_declined");
    let (r, disk, victim) = fresh(control);
    breaker(&disk, &r.mother, victim, false)();
    let before = asked();
    assert!(try_port_for(control, false).is_none(), "no: no port");
    assert_eq!(asked(), before + 1);
    let port = try_port_for(control, true).expect("yes: a port");
    assert_eq!(asked(), before + 2);
    assert!(door_open_run(&port).is_ok(), "the new door works");
    assert_eq!(
        door_open_run(&r.port),
        Err("halted".to_string()),
        "M6: the old door is halted"
    );

    let disk = Disk::over(MemoryJournal::new());
    let closing = rig(control, disk.clone()).unwrap();
    closing.mother.close_ref().unwrap();
    drop(closing);
    let before = asked();
    let reopened = rig(control, disk).unwrap();
    assert_eq!(asked(), before + 1, "after a clean close");
    drop(reopened);

    let before = asked();
    let again = try_port_for(control, true).expect("a port");
    assert_eq!(asked(), before + 1, "in the same process, with no break");
    drop(again);
}

/// M7 (S2): a coordinator call whose body breaks the journal and returns
/// through `?` still writes its batch on its way out: every record of the
/// halt is written by the failing call's own thread.
fn m2c_m7_the_failing_call_writes_its_own_batch(home: &Home, origin: &str) {
    let control = &home.control;
    let (r, disk, victim) = fresh(control);
    let (_a, run_a) = waiting(&r, origin, "m2c-m7");
    let start = home.gate.seen().len();
    let me = breaker(&disk, &r.mother, victim, true)();
    let seen = home.gate.seen()[start..].to_vec();
    let halted: Vec<_> = seen.iter().filter(|s| s.phase == "door_halted").collect();
    assert_eq!(halted.len(), 1, "M7: the halt is recorded once");
    let run_a = run_a.to_string();
    let cancel_a: Vec<_> = seen
        .iter()
        .filter(|s| s.phase == "run_cancelled" && s.run.as_deref() == Some(run_a.as_str()))
        .collect();
    assert_eq!(cancel_a.len(), 1);
    assert!(halted.iter().chain(cancel_a.iter()).all(|s| s.thread == me));
    assert_eq!(halt_pending(control), 0, "written on its way out");
}

/// M8 (Codex R2-5's schedule, forced): G0 releases the coordinator lock and
/// is held before its (empty) write; G1 breaks the journal. G1's marker
/// exists before its first record is projected; G0 writes nothing of G1's;
/// G1 writes its own batch.
fn m2c_m8_no_call_writes_another_call_s_batch(home: &Home, origin: &str) {
    let control = &home.control;
    let (r, disk, victim) = fresh(control);
    waiting(&r, origin, "m2c-m8");
    let (g0_at, g0_hook) = held();
    arm_call_point(&r.mother, CallPoint::BeforeOwedWrite, g0_hook);
    let mother = r.mother.clone();
    let g0 = spawn(move || {
        let _ = mother.draft(contract_with(""));
        std::thread::current().id()
    });
    g0_at.reached();
    let marker = Arc::new(AtomicBool::new(false));
    let (journal, seen_marker) = (disk.inner.clone(), marker.clone());
    home.gate.arm_with("door_halted", None, move || {
        seen_marker.store(journal.is_poisoned(), Ordering::SeqCst);
    });
    let g1 = spawn(breaker(&disk, &r.mother, victim, true));
    assert!(home.gate.wait_held(), "G1's first record is held");
    home.gate.open();
    let g1 = within(&g1).expect("G1 returned");
    g0_at.release();
    let g0 = within(&g0).expect("G0 returned");
    assert!(
        marker.load(Ordering::SeqCst),
        "M8: G1's marker existed before its first record"
    );
    let seen = home.gate.seen();
    let halt_record = |s: &&crate::test_home::Seen| {
        s.phase == "door_halted" || s.phase == "run_cancelled" || s.phase == "revoked"
    };
    assert!(
        !seen.iter().filter(halt_record).any(|s| s.thread == g0),
        "M8: G0 wrote G1's records"
    );
    assert!(seen
        .iter()
        .any(|s| s.phase == "door_halted" && s.thread == g1));
}

/// A6-g extended to a door halt (§11 C3, PR-B): resume passes the broker's
/// post-record check; another thread fences the port (which halts its
/// door), with no journal failure; the take journals Resumed; afterwards
/// the door is halted and no effect runs through it.
fn m2c_a6_g_a_door_halt_outside_a_break(home: &Home, origin: &str, world: &WorldServer) {
    let control = &home.control;
    let disk = Disk::over(MemoryJournal::new());
    let first = rig(control, disk.clone()).unwrap();
    let id = approved(&first, &format!("FETCH-BUILDER:{origin}/m2c-a6g-halt"));
    first.mother.advance(id).unwrap();
    drop(first);
    let r = rig(control, disk.clone()).unwrap();
    assert!(r.mother.held(id).is_some());
    let port = r.port.clone();
    after_held_answer(&r.port, move || {
        let fencing = std::thread::spawn(move || fence_port(&port));
        fencing.join().unwrap();
    });
    let before = world.count();
    assert_eq!(
        resume_with(&r.mother, id, &HeldOwner::new(true)),
        Ok(MissionState::Investigating)
    );
    assert!(!r.mother.broken(), "no journal failure");
    assert_eq!(door_open_run(&r.port), Err("halted".to_string()));
    for _ in 0..4 {
        let _ = r.mother.advance(id);
    }
    assert_eq!(world.count(), before);
    assert!(!world.saw("/m2c-a6g-halt"));
}

pub(super) fn on_the_real_broker(home: &Home, origin: &str, listener: &TcpListener) {
    let world = WorldServer::start(listener);
    m2c_m1_an_effect_past_enter_is_refused_after_the_break(home, origin, &world);
    m2c_m2_a_stalled_store_holds_no_lock(home, origin, &world);
    m2c_m3_a_panic_in_the_fence_before_any_cancellation(home, origin, &world);
    m2c_m4_stop_holds_no_lock_while_it_writes(home, origin);
    m2c_m5_adopted_runs_are_covered(home, origin);
    m2c_m6_only_the_owner_lets_mother_act(home);
    m2c_m7_the_failing_call_writes_its_own_batch(home, origin);
    m2c_m8_no_call_writes_another_call_s_batch(home, origin);
    m2c_a6_g_a_door_halt_outside_a_break(home, origin, &world);
    super::m2c_crash::in_process_on_the_real_broker(home, origin);
    world.stop();
}
