//! M2a-S, oracle added after the Codex final review (MAJOR 3; it does not
//! replace or edit the earlier ones). Run from `real_broker.rs` (one broker
//! per process), with real threads.
//!
//! The fence's cancellation sweep is panic-safe. Two registered runs: A's
//! (an action waiting for approval) and B's (B's broker call passed the
//! fence check and is paused). The coordinator breaks and:
//! - the fence panics while cancelling A: B's run is still cancelled;
//! - the fence panics before its first cancellation: the next port call,
//!   which takes the poisoned mutex back, completes the sweep.
//!
//! Either way B's broker call, once it goes on, is refused, with no effect.

use super::m2as_support::{executed, rig, Disk, Rig, WorldServer};
use super::GovernedPort;
use super::Home;
use crate::effect::{EffectOutcome, EffectPort, EffectRequest};
use crate::journal::{MemoryJournal, MissionId};
use crate::oracles::{contract_with, get, Yes};
use crate::state::MissionState;
use nexus_governed_control::authority::ids::RunId;
use nexus_governed_control::egress::EgressIntent;
use nexus_governed_control::governed::GovernedControl;
use std::collections::BTreeSet;
use std::net::TcpListener;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

/// The longest a thread waits for another at a meeting point (a safety
/// bound: a meeting that must happen is asserted).
const MEET: Duration = Duration::from_secs(10);

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

fn post(url: &str) -> EffectRequest {
    EffectRequest::Network(EgressIntent {
        method: "POST".into(),
        url: url.into(),
        headers: vec![],
        body: None,
    })
}

#[derive(Clone, Copy)]
enum Panic {
    /// While cancelling A, the first run of the sweep.
    CancellingA,
    /// Before the first cancellation.
    BeforeTheSweep,
}

fn the_sweep_cancels_every_run(
    control: &Arc<GovernedControl>,
    origin: &str,
    world: &WorldServer,
    panic: Panic,
    tag: &str,
) {
    let disk = Disk::over(MemoryJournal::new());
    let r = rig(control, disk.clone()).unwrap();
    let victim = approved(&r, "");
    r.mother.advance(victim).unwrap();
    assert_eq!(r.port.run_of(victim), None, "the victim holds no run");

    // A (the smaller mission: the sweep's first run) waits for approval.
    let (a, b) = (MissionId(9_501), MissionId(9_502));
    assert_eq!(
        r.port.perform(a, &post(&format!("{origin}/{tag}-a"))),
        Ok(EffectOutcome::AwaitingApproval)
    );
    let a_run = r.port.run_of(a).expect("A's run");

    // B's broker call passes the fence check and pauses.
    let (paused, release) = (Flag::default(), Flag::default());
    {
        let (paused, release) = (paused.clone(), release.clone());
        r.port.in_broker_action.arm(0, move || {
            paused.set();
            let _ = release.wait(MEET);
        });
    }
    let port: Arc<GovernedPort> = r.port.clone();
    let b_url = format!("{origin}/{tag}-b");
    let b_thread = std::thread::spawn(move || port.perform(b, &get(&b_url)));
    assert!(paused.wait(MEET), "B's broker call never started");
    let b_run = r.port.run_of(b).expect("B's run");
    let runs = BTreeSet::from([a_run, b_run]);
    let executed_before = executed(control, &runs);

    match panic {
        Panic::CancellingA => r
            .port
            .fence_between_cancels
            .arm(0, || panic!("the broker panicked cancelling A")),
        Panic::BeforeTheSweep => r
            .port
            .first_fence_cancel
            .arm(|_| panic!("the broker panicked before the sweep")),
    }
    disk.fail(true, true);
    let stopped = catch_unwind(AssertUnwindSafe(|| r.mother.stop(victim)));
    disk.fail(false, false);
    assert!(stopped.is_err(), "the fence's panic was not raised");
    if let Panic::BeforeTheSweep = panic {
        // The next port call takes the poisoned mutex back.
        assert!(
            r.port.runs.is_poisoned(),
            "the port's mutex is not poisoned"
        );
        let _ = r.port.run_of(a);
    }
    let cancelled = |run: RunId| control.authority().runs().view(run).unwrap().cancelled;
    assert!(cancelled(a_run), "A's run is not cancelled");
    assert!(cancelled(b_run), "the sweep skipped B's run");

    release.set();
    let answer = b_thread.join().expect("B's call panicked");
    assert!(answer.is_err(), "B answered {answer:?} after the break");
    assert_eq!(
        executed(control, &runs),
        executed_before,
        "the broker ran an action after the break"
    );
    assert!(
        !world.saw(&format!("/{tag}-b")),
        "B's effect reached the world"
    );
}

pub(super) fn on_the_real_broker(home: &Home, origin: &str, listener: &TcpListener) {
    let world = WorldServer::start(listener);
    the_sweep_cancels_every_run(&home.control, origin, &world, Panic::CancellingA, "sweep-1");
    the_sweep_cancels_every_run(
        &home.control,
        origin,
        &world,
        Panic::BeforeTheSweep,
        "sweep-2",
    );
    world.stop();
}
