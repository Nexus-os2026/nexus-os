//! M2d-C round 5 (decisions D1 and D4(c), amendment H1-H4 with C1-C5),
//! against the real broker, pre-registered before the repair. They run in
//! `real_broker_m1_oracles` after `m2d_files_r4`, before its emergency
//! finale (one emergency here is lifted by the Owner). Each scenario has its
//! own door. Deterministic: held journal appends, held owed writes, held
//! port calls, a poisoned coordinator, Mother's own manual clock; no sleeps.
//!
//! The mission's halt cell, read through the broker's table, is the
//! observable: engaged before any coordinator lock or write on every Stop
//! path, it refuses the mission's poised File (authorized, not executed).

use super::m2c_support::{consent, port_for, Home};
use super::{mother_agent, FileJournal, GovernedPort};
use crate::coordinator::{Coordinator, CoordinatorError, Lanes};
use crate::effect::EffectPort;
use crate::journal::{JournalError, JournalStore, MissionId};
use crate::lane::{Lane, LaneIdentity, LaneInput, LaneOutput};
use crate::oracles::m2as_hooks::HookSlot;
use crate::oracles::m2c_owner::OwnerLiftsTheStop;
use crate::oracles::m2d_c::{files_lanes, FilesOwner};
use crate::oracles::{contract_with, Yes};
use crate::state::MissionState;
use nexus_governed_control::authority::clock::{Clock, ManualClock};
use nexus_governed_control::authority::ids::{CommitmentId, RunId};
use std::path::PathBuf;
use std::sync::mpsc;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

const BOUND: Duration = Duration::from_secs(30);
/// Past the test contracts' time budget (one hour).
const PAST_BUDGET: Duration = Duration::from_millis(3_600_001);

type Hold = Arc<Mutex<Option<(mpsc::Sender<()>, mpsc::Receiver<()>)>>>;

/// A lane's one hook, set once a scenario is built.
type Tick = Arc<OnceLock<Mutex<Option<Box<dyn FnOnce() + Send>>>>>;

/// A journal whose next append, once armed, waits for its release.
struct Gated {
    inner: FileJournal,
    pub(super) hold: Hold,
}

impl JournalStore for Gated {
    fn load(&self) -> Result<Vec<String>, JournalError> {
        self.inner.load()
    }
    fn append(&mut self, line: &str) -> Result<(), JournalError> {
        let held = self.hold.lock().unwrap().take();
        if let Some((entered, release)) = held {
            entered.send(()).unwrap();
            release.recv_timeout(BOUND).expect("released");
        }
        self.inner.append(line)
    }
    fn poison(&mut self) -> Result<(), JournalError> {
        self.inner.poison()
    }
    fn identity(&self) -> Option<String> {
        self.inner.identity()
    }
}

/// A lane that, once armed, runs a hook as it answers (the 1695 budget
/// check: the next lane's reservation).
struct Ticking {
    inner: Arc<dyn Lane>,
    hook: Tick,
}

impl Lane for Ticking {
    fn identity(&self) -> LaneIdentity {
        self.inner.identity()
    }
    fn invoke(&self, input: &LaneInput) -> LaneOutput {
        if let Some(slot) = self.hook.get() {
            if let Some(hook) = slot.lock().unwrap().take() {
                hook();
            }
        }
        self.inner.invoke(input)
    }
}

pub(super) struct Scenario<'a> {
    pub(super) home: &'a Home,
    pub(super) ws: PathBuf,
    pub(super) folder: String,
    pub(super) port: Arc<GovernedPort>,
    pub(super) mother: Coordinator<GovernedPort>,
    pub(super) clock: Arc<ManualClock>,
    pub(super) owner: FilesOwner,
    hold: Hold,
    pub(super) tick: Tick,
}

impl<'a> Scenario<'a> {
    pub(super) fn new(home: &'a Home, journal: &str) -> Self {
        Self::with_port(
            home,
            journal,
            Arc::new(port_for(&home.control, true).with_workspace(Self::folder_of(home))),
        )
    }

    fn folder_of(home: &Home) -> String {
        home.control.workspace().expect("the workspace").name
    }

    fn with_port(home: &'a Home, journal: &str, port: Arc<GovernedPort>) -> Self {
        let folder = Self::folder_of(home);
        let ws = std::fs::canonicalize(std::env::temp_dir())
            .unwrap()
            .join(&folder);
        let hold: Hold = Arc::default();
        let tick: Tick = Arc::default();
        // The Sentinel ticks: every lane call after it is a reservation.
        let lanes: Vec<Arc<dyn Lane>> = files_lanes()
            .into_iter()
            .map(|lane| {
                if lane.identity().role == crate::lane::LaneRole::ResearcherA {
                    Arc::new(Ticking {
                        inner: lane,
                        hook: tick.clone(),
                    }) as Arc<dyn Lane>
                } else {
                    lane
                }
            })
            .collect();
        let clock = Arc::new(ManualClock::default());
        let mother = Coordinator::open(
            Gated {
                inner: FileJournal::open(&home.dir.join(journal)).unwrap(),
                hold: hold.clone(),
            },
            port.clone(),
            Lanes::new(lanes).unwrap(),
            clock.clone() as Arc<dyn Clock>,
            home.private(),
        )
        .unwrap();
        Self {
            home,
            ws,
            folder,
            port,
            mother,
            clock,
            owner: FilesOwner::default(),
            hold,
            tick,
        }
    }

    fn approved(&self, tag: &str) -> MissionId {
        let mut contract = contract_with(&format!(
            "MKFILE:{tag}-a.txt MKFILE:{tag}-b.txt VFILE:{tag}-c.txt"
        ));
        contract.requested_folder = Some(self.folder.clone());
        let id = self.mother.draft(contract).unwrap();
        self.mother.submit(id).unwrap();
        assert_eq!(consent(&self.mother, id, &Yes), Ok(MissionState::Approved));
        id
    }

    /// Approved and advanced to the Builder's File A, waiting.
    pub(super) fn waiting(&self, tag: &str) -> MissionId {
        let id = self.approved(tag);
        for _ in 0..3 {
            self.mother.advance(id).unwrap();
        }
        assert_eq!(self.mother.state(id), Some(MissionState::AwaitingApproval));
        id
    }

    /// The Owner approves the waiting File natively: authorized, poised.
    pub(super) fn approve(&self, id: MissionId) -> (RunId, CommitmentId) {
        let pending = self.port.pending(id).expect("the waiting action");
        let run = RunId::parse(&pending.run).unwrap();
        let commitment = CommitmentId::parse(&pending.commitment).unwrap();
        self.home
            .control
            .authorize(commitment, &mother_agent(), run, &self.owner)
            .unwrap();
        (run, commitment)
    }

    pub(super) fn execute(&self, (run, commitment): (RunId, CommitmentId)) -> bool {
        self.home
            .control
            .execute(commitment, &mother_agent(), run)
            .is_ok()
    }

    /// Whether the mission's halt cell (the broker's) is engaged.
    fn engaged(&self, id: MissionId) -> bool {
        self.port
            .halt_cells()
            .get(self.mother.journal_identity(), id.0)
            .is_none_or(|cell| cell.is_engaged())
    }

    pub(super) fn hold_next_append(&self) -> (mpsc::Receiver<()>, mpsc::Sender<()>) {
        let (entered_tx, entered) = mpsc::channel();
        let (release, release_rx) = mpsc::channel();
        *self.hold.lock().unwrap() = Some((entered_tx, release_rx));
        (entered, release)
    }
}

/// A slot's hook that signals, then waits for its release.
fn held_at(slot: &HookSlot, skip: usize) -> (mpsc::Receiver<()>, mpsc::Sender<()>) {
    let (entered_tx, entered) = mpsc::channel();
    let (release, release_rx) = mpsc::channel::<()>();
    slot.arm(skip, move || {
        entered_tx.send(()).unwrap();
        let _ = release_rx.recv_timeout(BOUND);
    });
    (entered, release)
}

pub(super) fn on_the_real_broker(home: &Home) {
    stop_paths_engage_before_any_lock(home);
    a_poisoned_coordinator_still_engages(home);
    a_paused_proposal_is_refused(home);
    replacement_over_a_surviving_broker(home);
    stop_timing(home);
    the_stop_all_cohort_is_one_snapshot(home);
    in_lock_budget_checks_engage_before_the_owed_write(home);
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Path {
    Stop,
    StopAll,
    Budget,
    Emergency,
}

/// D1 / H3: on every Stop path, with an unrelated append holding the
/// coordinator (then, separately, the path's own owed write held), the
/// target's cell is engaged and its poised File refused; nothing is
/// published.
fn stop_paths_engage_before_any_lock(home: &Home) {
    for path in [Path::Stop, Path::StopAll, Path::Budget, Path::Emergency] {
        for held in ["append", "owed"] {
            let s = Scenario::new(home, &format!("m2d-c-r5-{path:?}-{held}.jsonl"));
            let tag = format!("r5-{path:?}-{held}").to_lowercase();
            let b = s.waiting(&tag);
            let poised = s.approve(b);
            let (halted_tx, halted) = mpsc::channel();
            s.mother
                .hooks
                .after_halt
                .arm(0, move || halted_tx.send(()).unwrap());
            if path == Path::Budget {
                s.clock.advance(PAST_BUDGET);
            }
            std::thread::scope(|scope| {
                let (entered, release) = if held == "append" {
                    let (entered, release) = s.hold_next_append();
                    let drafting = scope.spawn(|| s.mother.draft(contract_with("")));
                    entered
                        .recv_timeout(BOUND)
                        .expect("the unrelated append held");
                    drop(drafting);
                    (None, Some(release))
                } else {
                    let (entered, release) = held_at(&s.mother.hooks.before_owed_write, 0);
                    (Some(entered), Some(release))
                };
                let stopping = scope.spawn(|| match path {
                    Path::Stop => s.mother.stop(b).map(|_| ()),
                    Path::StopAll => s.mother.stop_everything().map(|_| ()),
                    Path::Budget => s.mother.advance(b).map(|_| ()),
                    Path::Emergency => {
                        s.home.control.emergency_stop();
                        s.mother.advance(b).map(|_| ())
                    }
                });
                halted.recv_timeout(BOUND).expect("the halt engaged first");
                if let Some(entered) = entered {
                    entered.recv_timeout(BOUND).expect("the owed write held");
                }
                assert!(s.engaged(b), "{path:?} {held}: engaged");
                assert!(!s.execute(poised), "{path:?} {held}: refused");
                release.unwrap().send(()).unwrap();
                let _ = stopping.join().unwrap();
            });
            if path == Path::Emergency {
                home.control.resume(&OwnerLiftsTheStop).unwrap();
            }
            let name = s.ws.join(format!("{tag}-a.txt"));
            assert!(!name.exists(), "{path:?} {held}: nothing published");
        }
    }
}

/// D1: a poisoned coordinator lock: each Stop path engages the target's
/// cell and returns a typed error, never a panic; nothing is published.
fn a_poisoned_coordinator_still_engages(home: &Home) {
    for path in [Path::Stop, Path::StopAll, Path::Budget] {
        let s = Scenario::new(home, &format!("m2d-c-r5-poison-{path:?}.jsonl"));
        let tag = format!("r5-poison-{path:?}").to_lowercase();
        let b = s.waiting(&tag);
        let poised = s.approve(b);
        if path == Path::Budget {
            s.clock.advance(PAST_BUDGET);
        }
        s.mother.poison_for_test();
        let answer = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| match path {
            Path::Stop => s.mother.stop(b).map(|_| ()),
            Path::StopAll => s.mother.stop_everything().map(|_| ()),
            _ => s.mother.advance(b).map(|_| ()),
        }));
        assert_eq!(
            answer.expect("never a panic"),
            Err(CoordinatorError::Poisoned),
            "{path:?}"
        );
        assert!(s.engaged(b), "{path:?}");
        assert!(!s.execute(poised), "{path:?}");
        assert!(!s.ws.join(format!("{tag}-a.txt")).exists());
    }
}

/// Codex review 4, HIGH 2: perform paused after `run_for`; the mission's
/// Stop, its `SettlementStarted` append held; the perform resumes: its
/// proposal is refused and nothing is published.
fn a_paused_proposal_is_refused(home: &Home) {
    let s = Scenario::new(home, "m2d-c-r5-paused.jsonl");
    let b = s.approved("r5-paused");
    s.mother.advance(b).unwrap();
    s.mother.advance(b).unwrap();
    let (paused, resume) = held_at(&s.port.before_broker_action, 0);
    std::thread::scope(|scope| {
        let advancing = scope.spawn(|| s.mother.advance(b));
        paused.recv_timeout(BOUND).expect("paused after run_for");
        let (entered, release) = s.hold_next_append();
        let stopping = scope.spawn(|| s.mother.stop(b));
        entered.recv_timeout(BOUND).expect("SettlementStarted held");
        assert!(s.engaged(b));
        resume.send(()).unwrap();
        // The perform's refusal comes back while the Stop's append is held.
        release.send(()).unwrap();
        let _ = advancing.join().unwrap();
        let _ = stopping.join().unwrap();
    });
    let run = s.port.run_of(b).expect("its run");
    let views = s.home.control.authority().commitments().views_of_run(run);
    assert!(views.is_empty(), "the proposal was refused: {views:?}");
    assert!(!s.ws.join("r5-paused-a.txt").exists());
    assert!(s.mother.state(b).is_some_and(MissionState::is_terminal));
}

/// C1 / C5: coordinator and port replaced over a surviving broker: the
/// rebuilt one finds the original cell (D4(a) refuses the adoption of the
/// authorized File's run), and its Stop reaches the original run.
fn replacement_over_a_surviving_broker(home: &Home) {
    let journal = "m2d-c-r5-replaced.jsonl";
    let s = Scenario::new(home, journal);
    let a = s.waiting("r5-replaced");
    let poised = s.approve(a);
    drop(s);
    let again = Scenario::new(home, journal);
    assert!(!again.engaged(a), "the original cell, not engaged");
    assert_eq!(again.port.run_of(a), None, "the adoption was refused");
    let _ = again.mother.stop(a);
    assert!(again.engaged(a));
    assert!(!again.execute(poised), "the Stop reached the original run");
    assert!(!again.ws.join("r5-replaced-a.txt").exists());
}

/// C5: Stop before the first run (none opens), and during the opening (the
/// run opens with the engaged cell: nothing is proposed).
fn stop_timing(home: &Home) {
    let s = Scenario::new(home, "m2d-c-r5-before.jsonl");
    let b = s.approved("r5-before");
    assert_eq!(s.mother.stop(b), Ok(MissionState::Stopped));
    assert!(s.mother.advance(b).is_err());
    assert_eq!(s.port.run_of(b), None);

    let s = Scenario::new(home, "m2d-c-r5-opening.jsonl");
    let b = s.approved("r5-opening");
    s.mother.advance(b).unwrap();
    s.mother.advance(b).unwrap();
    let (opening, open) = held_at(&s.port.before_open_run, 0);
    std::thread::scope(|scope| {
        let advancing = scope.spawn(|| s.mother.advance(b));
        opening.recv_timeout(BOUND).expect("held at the opening");
        let _ = s.mother.stop(b);
        open.send(()).unwrap();
        let _ = advancing.join().unwrap();
    });
    if let Some(run) = s.port.run_of(b) {
        let views = s.home.control.authority().commitments().views_of_run(run);
        assert!(views.is_empty(), "{views:?}");
    }
    assert!(!s.ws.join("r5-opening-a.txt").exists());
}

/// C2: Stop everything takes one snapshot: a mission drafted after it (at
/// the hook after the halt) is not stopped; the snapshot's are.
fn the_stop_all_cohort_is_one_snapshot(home: &Home) {
    let s = Scenario::new(home, "m2d-c-r5-cohort.jsonl");
    let a = s.approved("r5-cohort");
    let later: Arc<Mutex<Option<MissionId>>> = Arc::default();
    std::thread::scope(|scope| {
        let (halted, go) = held_at(&s.mother.hooks.after_halt, 0);
        let stopping = scope.spawn(|| s.mother.stop_all());
        halted.recv_timeout(BOUND).expect("the cohort engaged");
        *later.lock().unwrap() = Some(s.mother.draft(contract_with("")).unwrap());
        go.send(()).unwrap();
        assert_eq!(stopping.join().unwrap(), 1);
    });
    let later = later.lock().unwrap().unwrap();
    assert_eq!(s.mother.state(a), Some(MissionState::Stopped));
    assert_eq!(s.mother.state(later), Some(MissionState::Draft), "after it");
    assert!(!s.engaged(later));
}

/// D1 / C5: the budget's precheck sees no expiry; it is then found inside
/// each in-lock check (the advance's first step, a lane's reservation, a
/// dispatch): the cell is engaged before that step's owed write.
fn in_lock_budget_checks_engage_before_the_owed_write(home: &Home) {
    // advance's first step: the clock moves after the precheck.
    let s = Scenario::new(home, "m2d-c-r5-budget-first.jsonl");
    let b = s.approved("r5-budget-first");
    let clock = s.clock.clone();
    s.mother
        .hooks
        .after_precheck
        .arm(0, move || clock.advance(PAST_BUDGET));
    let seen: Arc<Mutex<Option<bool>>> = Arc::default();
    let (cells, journal, noted) = (
        s.port.halt_cells(),
        s.mother.journal_identity().to_string(),
        seen.clone(),
    );
    s.mother.hooks.before_owed_write.arm(0, move || {
        *noted.lock().unwrap() = Some(cells.get(&journal, b.0).is_none_or(|c| c.is_engaged()));
    });
    let _ = s.mother.advance(b);
    assert_eq!(*seen.lock().unwrap(), Some(true), "advance's first step");

    // A lane's reservation: the clock moves as the lane before it answers.
    let s = Scenario::new(home, "m2d-c-r5-budget-lane.jsonl");
    let b = s.approved("r5-budget-lane");
    s.mother.advance(b).unwrap();
    let seen: Arc<Mutex<Option<bool>>> = Arc::default();
    let (cells, journal, noted, clock, slot) = (
        s.port.halt_cells(),
        s.mother.journal_identity().to_string(),
        seen.clone(),
        s.clock.clone(),
        s.mother.hooks.before_owed_write.clone(),
    );
    let _ = s.tick.set(Mutex::new(Some(Box::new(move || {
        clock.advance(PAST_BUDGET);
        // Its answer's step, then the next reservation's.
        slot.arm(1, move || {
            *noted.lock().unwrap() = Some(cells.get(&journal, b.0).is_none_or(|c| c.is_engaged()));
        });
    }))));
    let _ = s.mother.advance(b);
    assert_eq!(*seen.lock().unwrap(), Some(true), "a lane's reservation");

    // A dispatch: the clock moves at the Files admission before it.
    let s = Scenario::new(home, "m2d-c-r5-budget-dispatch.jsonl");
    let b = s.approved("r5-budget-dispatch");
    s.mother.advance(b).unwrap();
    s.mother.advance(b).unwrap();
    let seen: Arc<Mutex<Option<bool>>> = Arc::default();
    let (cells, journal, noted, clock, slot) = (
        s.port.halt_cells(),
        s.mother.journal_identity().to_string(),
        seen.clone(),
        s.clock.clone(),
        s.mother.hooks.before_owed_write.clone(),
    );
    s.port.after_admit.arm(0, move || {
        clock.advance(PAST_BUDGET);
        // The admission's keeping step, then the dispatch's.
        slot.arm(1, move || {
            *noted.lock().unwrap() = Some(cells.get(&journal, b.0).is_none_or(|c| c.is_engaged()));
        });
    });
    let _ = s.mother.advance(b);
    assert_eq!(*seen.lock().unwrap(), Some(true), "a dispatch");
}
