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
/// Minimum coverage per 500 sequences (the fixed seeds reach, at the time
/// of writing: perform 13, lane 35, approval query 3, consent dialog 68,
/// with a request open 4, failed open after a clean close 3).
const MIN_PERFORM: usize = 8;
const MIN_LANE: usize = 20;
const MIN_APPROVAL: usize = 2;
const MIN_CONSENT: usize = 40;
const MIN_WITH_REQUEST: usize = 3;
const MIN_FAILED_OPEN: usize = 2;

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

/// How often a sequence reached the states the class test exists for.
#[derive(Clone, Copy, Default)]
struct Coverage {
    /// Breaks caused inside B's call: before a perform, inside a lane,
    /// before an approval query, inside the Owner's consent dialog.
    breaks: [usize; 4],
    /// Of those, breaks while a request of the sequence was open at the
    /// broker.
    breaks_with_open_request: usize,
    /// Opens that failed at their first write right after a clean close
    /// with a request open at the broker.
    failed_opens_after_clean_close: usize,
}

impl Coverage {
    fn add(&mut self, other: Coverage) {
        for (a, b) in self.breaks.iter_mut().zip(other.breaks) {
            *a += b;
        }
        self.breaks_with_open_request += other.breaks_with_open_request;
        self.failed_opens_after_clean_close += other.failed_opens_after_clean_close;
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
    cov: Coverage,
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
            cov: Coverage::default(),
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

    fn live_other_than(&mut self, id: MissionId) -> Option<MissionId> {
        let mother = self.mother().clone();
        let others: Vec<MissionId> = self
            .missions
            .iter()
            .copied()
            .filter(|m| *m != id && mother.state(*m).is_some_and(|s| !s.is_terminal()))
            .collect();
        (!others.is_empty()).then(|| others[self.rng.below(others.len())])
    }

    /// Count a break caused inside a call at `point`.
    fn covered(&mut self, point: usize, was_broken: bool, fired: bool, open_before: bool) {
        if fired && !was_broken && self.mother().broken() {
            self.cov.breaks[point] += 1;
            if open_before {
                self.cov.breaks_with_open_request += 1;
            }
        }
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

    /// Clean only on positive evidence: the journal is empty or ends in a
    /// clean close (the model's own reading of the journal, not Mother's).
    fn journal_clean(&self) -> Result<bool, String> {
        match verify(&self.disk.inner.lines()) {
            Ok(entries) => Ok(entries
                .last()
                .is_none_or(|e| e.event == Event::SessionClosed)),
            Err(e) => Err(format!("the journal Mother wrote does not verify: {e:?}")),
        }
    }

    /// A coordinator opened: every unfinished mission is held if the journal
    /// was unclean; holds the model already knows stay.
    fn expect_holds(&mut self, clean: bool) -> Check {
        for m in self.missions.clone() {
            match self.mother().state(m) {
                Some(s) if !s.is_terminal() => {
                    if !clean {
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

    fn open(&mut self) -> Check {
        let clean = self.journal_clean()?;
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
                self.expect_holds(clean)
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
                // A new coordinator: the failed open's measure is not its,
                // and the journal (perhaps with the failed open's records)
                // says again whether it is clean.
                self.disk.fail(false, false);
                *self.broke_at.lock().unwrap() = None;
                let clean = self.journal_clean()?;
                let failures = self.disk.failures();
                match rig(self.control, self.disk.clone()) {
                    Ok(rig) => {
                        self.rig = Some(rig);
                        self.log.push(format!(
                            "    reopened after the disk recovered ({})",
                            if clean { "clean" } else { "unclean" }
                        ));
                        self.expect_holds(clean)
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
                let lines = self.world.counter().lock().unwrap()[at..].to_vec();
                format!(
                    "(b) {} request(s) reached the world after the break: {lines:?}",
                    lines.len()
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
            self.live_other_than(id)
        } else {
            None
        };
        let result = match inside {
            Some(victim) if submitted.is_ok() => {
                let (hook, kept) = self.breaker(victim);
                let slot = HookSlot::default();
                slot.arm(0, hook);
                let owner = HookConfirmer {
                    hook: slot.clone(),
                    yes,
                };
                self.note_runs();
                let open_before = !open_requests(self.control, &self.runs).is_empty();
                let was_broken = mother.broken();
                let r = mother.request_consent(id, &owner);
                self.covered(3, was_broken, !slot.armed(), open_before);
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
        // Aimed at the session record: the next open fails at its first
        // write, right after a clean close.
        let open_now = !open_requests(self.control, &self.runs).is_empty();
        if r.is_ok() && self.rng.percent(if open_now { 80 } else { 20 }) {
            let poison = self.rng.percent(50);
            self.disk.fail_after(Some(0), poison);
            self.log.push(format!(
                "fault: the next open's first append fails, poison fails {poison}"
            ));
            let open_before = !open_requests(self.control, &self.runs).is_empty();
            let failures = self.disk.failures();
            let opened = self.open();
            if open_before && self.disk.failures() > failures {
                self.cov.failed_opens_after_clean_close += 1;
            }
            return opened;
        }
        self.open()
    }

    /// Advance B while a Stop of A whose append and poisoning both fail
    /// runs inside B's call: before a perform, inside a lane, or before an
    /// approval query.
    fn interleave(&mut self) -> Check {
        if self.mother().broken() {
            return self.advance();
        }
        // B where the point can be reached: a perform in Building or
        // Verifying, a lane in any phase that runs lanes, an approval query
        // while waiting; never a held mission. A is a mission that has not
        // ended, so that its Stop appends.
        let mother = self.mother().clone();
        let reaches = |point: usize, state: MissionState| match point {
            0 => matches!(state, MissionState::Building | MissionState::Verifying),
            1 => matches!(
                state,
                MissionState::Investigating | MissionState::Building | MissionState::Verifying
            ),
            _ => state == MissionState::AwaitingApproval,
        };
        // Waiting missions are rarer: prefer the approval query when one waits.
        let waits = self.missions.iter().any(|m| {
            mother.held(*m).is_none() && mother.state(*m) == Some(MissionState::AwaitingApproval)
        });
        let first = if waits && self.rng.percent(60) {
            2
        } else {
            self.rng.below(3)
        };
        let mut chosen = None;
        for point in [first, (first + 1) % 3, (first + 2) % 3] {
            let candidates: Vec<MissionId> = self
                .missions
                .iter()
                .copied()
                .filter(|m| {
                    mother.held(*m).is_none() && mother.state(*m).is_some_and(|s| reaches(point, s))
                })
                .collect();
            if !candidates.is_empty() {
                chosen = Some((point, candidates[self.rng.below(candidates.len())]));
                break;
            }
        }
        let Some((point, b)) = chosen else {
            return self.advance();
        };
        let Some(a) = self.live_other_than(b) else {
            return self.advance();
        };
        let skip = if point == 1 { self.rng.below(3) } else { 0 };
        self.note_runs();
        let open_before = !open_requests(self.control, &self.runs).is_empty();
        let was_broken = mother.broken();
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
        let _ = rig;
        self.covered(point, was_broken, fired, open_before);
        let rig = self.rig.as_ref().expect("a coordinator");
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
    let mut coverage = Coverage::default();
    for seed in 0..sequences {
        let mut seq = Seq::new(seed, &home.control, origin, &world);
        let outcome = catch_unwind(AssertUnwindSafe(|| seq.run()));
        coverage.add(seq.cov);
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
    eprintln!(
        "M2a-S class test: {sequences} sequences; breaks inside a perform {}, a lane {}, an approval query {}, a consent dialog {}; with a request open {}; failed opens after a clean close with a request open {}",
        coverage.breaks[0],
        coverage.breaks[1],
        coverage.breaks[2],
        coverage.breaks[3],
        coverage.breaks_with_open_request,
        coverage.failed_opens_after_clean_close
    );
    // The class test must reach what it exists for (minimums for the
    // default 500 fixed seeds, scaled for more).
    let scale = sequences as usize;
    let floor = |per_500: usize| per_500 * scale / 500;
    let reached = [
        ("a perform", coverage.breaks[0], floor(MIN_PERFORM)),
        ("a lane", coverage.breaks[1], floor(MIN_LANE)),
        ("an approval query", coverage.breaks[2], floor(MIN_APPROVAL)),
        ("a consent dialog", coverage.breaks[3], floor(MIN_CONSENT)),
        (
            "a request open",
            coverage.breaks_with_open_request,
            floor(MIN_WITH_REQUEST),
        ),
        (
            "a failed open after a clean close",
            coverage.failed_opens_after_clean_close,
            floor(MIN_FAILED_OPEN),
        ),
    ];
    for (what, got, min) in reached {
        assert!(
            got >= min,
            "class test coverage: breaks with {what}: {got} < {min}"
        );
    }
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
