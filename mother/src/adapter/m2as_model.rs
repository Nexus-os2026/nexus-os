//! M2a-S Codex REJECT, class test (oracle first): seeded, deterministic,
//! model-based sequences on the real broker.
//!
//! Each sequence (a fixed seed) runs random steps over two or three
//! missions: open, draft and consent (yes or no), advance, Stop, resume,
//! the Owner approving or denying a waiting request at the broker, a clean
//! close, a crash (dropped without close), append and poison failures
//! (now, or after a few appends), and a Stop that fails inside another
//! mission's call (before a perform, inside a lane, inside the Owner's
//! consent dialog, before an approval query). After every step:
//!
//! - (a) a Stop or denial that returned Ok holds forever;
//! - (b) once the coordinator breaks, nothing more reaches the world;
//! - (c) after an unclean session every unfinished mission is held, and a
//!   held mission dispatches nothing until a journaled resume;
//! - (d) after any open() error, no request of the sequence's runs is
//!   approvable or executable;
//! - (e) a journal Mother wrote reopens, or is refused fail-closed (poisoned,
//!   or a failure injected during that open) with no request left open.
//!
//! Every failing seed is collected with its steps, then the test fails.
//! `MOTHER_M2AS_SEQUENCES` raises the number of sequences (default 500).

use super::m2as_support::{
    open_requests, owner_runs, rig, Disk, HookConfirmer, HookSlot, Rig, WorldServer,
};
use super::{mother_agent, GovernedPort, Home};
use crate::coordinator::{Coordinator, CoordinatorError};
use crate::journal::{verify, Event, JournalError, MemoryJournal, MissionId};
use crate::oracles::{contract_with, No, Yes};
use crate::state::{MissionState, Trigger};
use nexus_governed_control::authority::ids::RunId;
use nexus_governed_control::governed::GovernedControl;
use std::collections::{BTreeMap, BTreeSet};
use std::net::TcpListener;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::{Arc, Mutex, Weak};

const STEPS: usize = 30;
const MAX_MISSIONS: usize = 3;

/// SplitMix64: a fixed seed gives the same sequence everywhere.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }

    fn percent(&mut self, p: usize) -> bool {
        self.below(100) < p
    }
}

type Check = Result<(), String>;
/// What a Stop run inside another call returned.
type Kept = Arc<Mutex<Option<Result<MissionState, CoordinatorError>>>>;

fn ensure(cond: bool, msg: impl FnOnce() -> String) -> Check {
    if cond {
        Ok(())
    } else {
        Err(msg())
    }
}

struct Seq<'a> {
    seed: u64,
    rng: Rng,
    log: Vec<String>,
    control: &'a Arc<GovernedControl>,
    origin: &'a str,
    world: &'a WorldServer,
    disk: Disk,
    rig: Option<Rig>,
    missions: Vec<MissionId>,
    /// A Stop or denial that returned Ok: the state it must keep.
    acked: BTreeMap<MissionId, MissionState>,
    /// The model's held missions.
    held: BTreeSet<MissionId>,
    runs: BTreeSet<RunId>,
    /// What had reached the world when the current coordinator broke.
    broke_at: Arc<Mutex<Option<usize>>>,
    ended: bool,
}

impl<'a> Seq<'a> {
    fn new(
        seed: u64,
        control: &'a Arc<GovernedControl>,
        origin: &'a str,
        world: &'a WorldServer,
    ) -> Self {
        let disk = Disk::over(MemoryJournal::new());
        let broke_at = Arc::new(Mutex::new(None));
        let (counter, at) = (world.counter(), broke_at.clone());
        disk.on_failure(Some(Arc::new(move || {
            let n = counter.lock().unwrap().len();
            at.lock().unwrap().get_or_insert(n);
        })));
        Self {
            seed,
            rng: Rng(seed.wrapping_mul(0xA24B_AED4_963E_E407) ^ 0x5EED),
            log: Vec::new(),
            control,
            origin,
            world,
            disk,
            rig: None,
            missions: Vec::new(),
            acked: BTreeMap::new(),
            held: BTreeSet::new(),
            runs: BTreeSet::new(),
            broke_at,
            ended: false,
        }
    }

    fn mother(&self) -> &Arc<Coordinator<GovernedPort>> {
        &self.rig.as_ref().expect("a coordinator").mother
    }

    fn weak(&self) -> Weak<Coordinator<GovernedPort>> {
        Arc::downgrade(self.mother())
    }

    fn pick(&mut self) -> Option<MissionId> {
        if self.missions.is_empty() {
            return None;
        }
        Some(self.missions[self.rng.below(self.missions.len())])
    }

    fn other_than(&mut self, id: MissionId) -> Option<MissionId> {
        let others: Vec<MissionId> = self.missions.iter().copied().filter(|m| *m != id).collect();
        (!others.is_empty()).then(|| others[self.rng.below(others.len())])
    }

    fn note_runs(&mut self) {
        if let Some(rig) = &self.rig {
            for m in &self.missions {
                if let Some(run) = rig.port.run_of(*m) {
                    self.runs.insert(run);
                }
            }
        }
    }

    fn ack(&mut self, id: MissionId, state: MissionState) -> Check {
        ensure(state.is_terminal(), || {
            format!("{id} acknowledged as {state:?}")
        })?;
        self.acked.insert(id, state);
        self.held.remove(&id);
        Ok(())
    }

    /// A Stop of `victim` whose append and poisoning both fail, run inside
    /// another call; its result is kept for the model.
    fn breaker(&self, victim: MissionId) -> (impl FnOnce() + Send + 'static, Kept) {
        let (disk, mother) = (self.disk.clone(), self.weak());
        let result = Arc::new(Mutex::new(None));
        let kept = result.clone();
        let hook = move || {
            disk.fail(true, true);
            if let Some(mother) = mother.upgrade() {
                *kept.lock().unwrap() = Some(mother.stop(victim));
            }
            disk.fail(false, false);
        };
        (hook, result)
    }

    fn take_breaker(&mut self, victim: MissionId, result: &Kept) -> Check {
        if let Some(r) = result.lock().unwrap().take() {
            self.log
                .push(format!("    (inside: stop {victim} -> {r:?})"));
            if let Ok(state) = r {
                self.ack(victim, state)?;
            }
        }
        Ok(())
    }

    // ------------------------------------------------------------ open

    fn open(&mut self) -> Check {
        let lines = self.disk.inner.lines();
        let clean = match verify(&lines) {
            Ok(entries) => entries
                .last()
                .is_none_or(|e| e.event == Event::SessionClosed),
            Err(e) => return Err(format!("the journal Mother wrote does not verify: {e:?}")),
        };
        *self.broke_at.lock().unwrap() = None;
        let failures = self.disk.failures();
        match rig(self.control, self.disk.clone()) {
            Ok(rig) => {
                self.rig = Some(rig);
                self.log.push(format!(
                    "    opened ({})",
                    if clean { "clean" } else { "unclean" }
                ));
                *self.broke_at.lock().unwrap() = None;
                for m in self.missions.clone() {
                    let state = self.mother().state(m);
                    if state.is_some_and(|s| !s.is_terminal()) {
                        if !clean {
                            self.held.insert(m);
                        }
                    } else {
                        self.held.remove(&m);
                    }
                }
                self.journal_holds_are_respected()
            }
            Err(e) => {
                self.log.push(format!("    open -> Err({e:?})"));
                let poisoned = self.disk.inner.is_poisoned();
                let injected = self.disk.failures() > failures;
                ensure(
                    matches!(e, CoordinatorError::Journal(JournalError::Poisoned)) && poisoned
                        || injected,
                    || format!("(e) a journal Mother wrote did not reopen: {e:?}"),
                )?;
                self.no_request_open("(d) after an open error")?;
                if poisoned {
                    self.ended = true;
                    return Ok(());
                }
                // The disk recovers: it reopens, or is refused fail-closed.
                self.disk.fail(false, false);
                let failures = self.disk.failures();
                match rig(self.control, self.disk.clone()) {
                    Ok(rig) => {
                        self.rig = Some(rig);
                        self.log
                            .push("    reopened after the disk recovered".into());
                        for m in self.missions.clone() {
                            let state = self.mother().state(m);
                            match state {
                                Some(s) if !s.is_terminal() => {
                                    // A failed open may have journaled its
                                    // session: holds are only ever added.
                                    if self.mother().held(m).is_some() {
                                        self.held.insert(m);
                                    }
                                }
                                _ => {
                                    self.held.remove(&m);
                                }
                            }
                        }
                        self.journal_holds_are_respected()
                    }
                    Err(e) => {
                        let injected = self.disk.failures() > failures;
                        ensure(
                            matches!(e, CoordinatorError::Journal(JournalError::Poisoned))
                                || injected,
                            || format!("(e) a journal Mother wrote did not reopen: {e:?}"),
                        )?;
                        self.no_request_open("(d) after a second open error")?;
                        self.ended = true;
                        Ok(())
                    }
                }
            }
        }
    }

    fn no_request_open(&self, what: &str) -> Check {
        let open = open_requests(self.control, &self.runs);
        ensure(open.is_empty(), || {
            format!(
                "{what}: a request is still approvable: {:?}",
                open.iter()
                    .map(|v| (v.id.to_string(), v.state))
                    .collect::<Vec<_>>()
            )
        })?;
        Ok(())
    }

    /// Independent of replay: while a mission is held in the journal (from
    /// its hold to its resume or end), nothing of its work is journaled.
    fn journal_holds_are_respected(&self) -> Check {
        let entries = verify(&self.disk.inner.lines()).map_err(|e| format!("{e:?}"))?;
        let mut held = BTreeSet::new();
        for e in entries {
            match &e.event {
                Event::Held { .. } => {
                    held.insert(e.mission);
                }
                Event::Resumed => {
                    held.remove(&e.mission);
                }
                Event::Transition {
                    trigger: Trigger::Stop { .. },
                } => {
                    held.remove(&e.mission);
                }
                Event::LaneInvoked { .. }
                | Event::EffectDispatched { .. }
                | Event::ApprovalPending { .. }
                | Event::Transition { .. }
                    if held.contains(&e.mission) =>
                {
                    return Err(format!(
                        "(c) {} worked while held: {:?}",
                        e.mission, e.event
                    ));
                }
                _ => {}
            }
        }
        Ok(())
    }

    // ------------------------------------------------------------ checks

    fn check(&mut self) -> Check {
        self.note_runs();
        let Some(rig) = &self.rig else {
            return Ok(());
        };
        let mother = rig.mother.clone();
        // (a)
        for (id, state) in &self.acked {
            ensure(mother.state(*id) == Some(*state), || {
                format!(
                    "(a) {id} was acknowledged {state:?}, is {:?}",
                    mother.state(*id)
                )
            })?;
        }
        // (b)
        if mother.broken() {
            let at = *self.broke_at.lock().unwrap();
            let Some(at) = at else {
                return Err("a broken coordinator with no failing append".into());
            };
            ensure(self.world.count() == at, || {
                format!(
                    "(b) {} request(s) reached the world after the break",
                    self.world.count() - at
                )
            })?;
        }
        // (c)
        for m in &self.missions {
            if let Some(state) = mother.state(*m) {
                if state.is_terminal() {
                    self.held.remove(m);
                    continue;
                }
                ensure(mother.held(*m).is_some() == self.held.contains(m), || {
                    format!(
                        "(c) {m} held is {:?}, the model says {}",
                        mother.held(*m),
                        self.held.contains(m)
                    )
                })?;
            }
        }
        Ok(())
    }

    // ------------------------------------------------------------ steps

    fn step(&mut self) -> Check {
        if self.rig.is_none() {
            return self.open();
        }
        match self.rng.below(100) {
            0..=13 => self.new_mission(),
            14..=33 => self.advance(),
            34..=41 => self.stop(),
            42..=47 => self.resume(),
            48..=53 => self.owner(true),
            54..=57 => self.owner(false),
            58..=67 => self.fault(),
            68..=71 => {
                self.disk.fail(false, false);
                self.log.push("faults cleared".into());
                Ok(())
            }
            72..=78 => {
                self.log.push("crash".into());
                self.note_runs();
                self.rig = None;
                self.open()
            }
            79..=84 => self.close(),
            _ => self.interleave(),
        }?;
        self.check()
    }

    fn markers(&mut self, n: usize) -> String {
        let (o, s) = (self.origin, self.seed);
        match self.rng.below(4) {
            0 => format!("FETCH-BUILDER:{o}/m{s}-{n}-b"),
            1 => format!("POST-BUILDER:{o}/m{s}-{n}-p"),
            2 => format!("FETCH-BUILDER:{o}/m{s}-{n}-b FETCH-VERIFIER:{o}/m{s}-{n}-v"),
            _ => format!("FETCH-BUILDER:{o}/m{s}-{n}-b POST-VERIFIER:{o}/m{s}-{n}-vp"),
        }
    }

    fn new_mission(&mut self) -> Check {
        if self.missions.len() >= MAX_MISSIONS {
            return self.advance();
        }
        let markers = self.markers(self.missions.len());
        let mother = self.mother().clone();
        let Ok(id) = mother.draft(contract_with(&markers)) else {
            self.log.push("draft -> Err".into());
            return Ok(());
        };
        self.missions.push(id);
        let submitted = mother.submit(id);
        let yes = self.rng.percent(75);
        let inside = if self.rng.percent(20) {
            self.other_than(id)
        } else {
            None
        };
        let result = match inside {
            Some(victim) if submitted.is_ok() => {
                let (hook, kept) = self.breaker(victim);
                let slot = HookSlot::default();
                slot.arm(0, hook);
                let owner = HookConfirmer { hook: slot, yes };
                let r = mother.request_consent(id, &owner);
                self.log.push(format!("new {id} [{markers}] consent({yes}) with a failing stop of {victim} inside -> {r:?}"));
                self.take_breaker(victim, &kept)?;
                r
            }
            _ => {
                let r = if yes {
                    mother.request_consent(id, &Yes)
                } else {
                    mother.request_consent(id, &No)
                };
                self.log.push(format!(
                    "new {id} [{markers}] submit {submitted:?} consent({yes}) -> {r:?}"
                ));
                r
            }
        };
        if result == Err(CoordinatorError::Denied) {
            self.ack(id, MissionState::Denied)?;
        }
        Ok(())
    }

    fn advance(&mut self) -> Check {
        let Some(id) = self.pick() else {
            return self.new_mission();
        };
        let before = self.world.count();
        let held = self.held.contains(&id);
        let r = self.mother().advance(id);
        self.log
            .push(format!("advance {id} -> {:?}", r.as_ref().map(|a| a.to)));
        if held {
            ensure(r.is_err() && self.world.count() == before, || {
                format!("(c) held {id} advanced: {:?}", r.as_ref().map(|a| a.to))
            })?;
        }
        if let Ok(a) = &r {
            if a.to == MissionState::Denied {
                self.ack(id, MissionState::Denied)?;
            }
        }
        Ok(())
    }

    fn stop(&mut self) -> Check {
        let Some(id) = self.pick() else {
            return Ok(());
        };
        let r = self.mother().stop(id);
        self.log.push(format!("stop {id} -> {r:?}"));
        if let Ok(state) = r {
            self.ack(id, state)?;
        }
        Ok(())
    }

    fn resume(&mut self) -> Check {
        let Some(id) = self.pick() else {
            return Ok(());
        };
        let r = self.mother().resume(id);
        self.log.push(format!("resume {id} -> {r:?}"));
        if r.is_ok() {
            self.held.remove(&id);
        }
        Ok(())
    }

    /// The Owner at the broker: approves (and runs) or denies a waiting
    /// request of the sequence.
    fn owner(&mut self, approve: bool) -> Check {
        self.note_runs();
        let open = open_requests(self.control, &self.runs);
        if open.is_empty() {
            return Ok(());
        }
        let view = open[self.rng.below(open.len())].clone();
        if approve {
            let ran = owner_runs(self.control, &view);
            self.log
                .push(format!("owner approves {} -> ran {ran}", view.id));
        } else {
            let r = self.control.deny(view.id, &mother_agent(), view.run);
            self.log
                .push(format!("owner denies {} -> {:?}", view.id, r.is_ok()));
        }
        Ok(())
    }

    fn fault(&mut self) -> Check {
        let poison = self.rng.percent(85);
        let after = if self.rng.percent(40) {
            0
        } else {
            1 + self.rng.below(3)
        };
        self.disk.fail_after(Some(after), poison);
        self.log.push(format!(
            "fault: appends fail after {after}, poison fails {poison}"
        ));
        Ok(())
    }

    fn close(&mut self) -> Check {
        self.note_runs();
        let rig = self.rig.take().expect("a coordinator");
        let was_broken = rig.mother.broken();
        let mother = Arc::try_unwrap(rig.mother).ok().expect("one handle");
        let r = mother.close();
        self.log.push(format!("close -> {r:?}"));
        ensure(!(was_broken && r.is_ok()), || {
            "a broken coordinator closed cleanly".into()
        })?;
        self.open()
    }

    /// Advance B while a Stop of A whose append and poisoning both fail
    /// runs inside B's call: before a perform, inside a lane, or before an
    /// approval query.
    fn interleave(&mut self) -> Check {
        let Some(b) = self.pick() else {
            return self.new_mission();
        };
        let Some(a) = self.other_than(b) else {
            return self.advance();
        };
        let point = self.rng.below(3);
        let skip = self.rng.below(2);
        let (hook, kept) = self.breaker(a);
        let rig = self.rig.as_ref().expect("a coordinator");
        let slot = match point {
            0 => rig.port.before_perform.clone(),
            1 => rig.lane_hook.clone(),
            _ => rig.port.before_approval.clone(),
        };
        slot.arm(skip, hook);
        let mother = rig.mother.clone();
        let r = mother.advance(b);
        let rig = self.rig.as_ref().expect("a coordinator");
        let fired = !slot.armed();
        rig.port.before_perform.disarm();
        rig.port.before_approval.disarm();
        rig.lane_hook.disarm();
        let at = ["perform", "lane", "approval"][point];
        self.log.push(format!(
            "advance {b} with a failing stop of {a} before {at} #{skip} (fired {fired}) -> {:?}",
            r.as_ref().map(|x| x.to)
        ));
        self.take_breaker(a, &kept)?;
        if let Ok(x) = &r {
            if x.to == MissionState::Denied {
                self.ack(b, MissionState::Denied)?;
            }
        }
        Ok(())
    }

    fn run(&mut self) -> Check {
        self.open()?;
        self.check()?;
        for _ in 0..STEPS {
            if self.ended {
                break;
            }
            self.step()?;
        }
        Ok(())
    }
}

pub(super) fn on_the_real_broker(home: &Home, origin: &str, listener: &TcpListener) {
    let sequences: u64 = std::env::var("MOTHER_M2AS_SEQUENCES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(500);
    let world = WorldServer::start(listener);
    let mut failures: Vec<(u64, String, Vec<String>)> = Vec::new();
    for seed in 0..sequences {
        let mut seq = Seq::new(seed, &home.control, origin, &world);
        let outcome = catch_unwind(AssertUnwindSafe(|| seq.run()));
        let msg = match outcome {
            Ok(Ok(())) => continue,
            Ok(Err(msg)) => msg,
            Err(panic) => format!(
                "panicked: {}",
                panic
                    .downcast_ref::<String>()
                    .cloned()
                    .or_else(|| panic.downcast_ref::<&str>().map(|s| s.to_string()))
                    .unwrap_or_default()
            ),
        };
        failures.push((seed, msg, std::mem::take(&mut seq.log)));
    }
    world.stop();
    if !failures.is_empty() {
        let mut by_kind: BTreeMap<String, Vec<u64>> = BTreeMap::new();
        for (seed, msg, _) in &failures {
            let kind = msg
                .split(':')
                .next()
                .unwrap_or(msg)
                .chars()
                .take(60)
                .collect::<String>();
            by_kind.entry(kind).or_default().push(*seed);
        }
        let mut report = format!(
            "M2a-S class test: {} of {sequences} sequences failed\n",
            failures.len()
        );
        for (kind, seeds) in &by_kind {
            report.push_str(&format!(
                "  {} x {kind} (seeds {:?})\n",
                seeds.len(),
                &seeds[..seeds.len().min(10)]
            ));
        }
        for (seed, msg, log) in failures.iter().take(3) {
            report.push_str(&format!(
                "\nseed {seed}: {msg}\nsteps:\n  {}\n",
                log.join("\n  ")
            ));
        }
        panic!("{report}");
    }
}
