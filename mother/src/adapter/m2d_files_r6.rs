//! M2d-C round 6 (the Architect's deep-diagnostic note R6-1 to R6-4 with
//! its binding X1-X5 and X2″, after Codex review 5), against the real
//! broker, pre-registered before the repair. They run in
//! `real_broker_m1_oracles` after `m2d_files_r5b`, before its emergency
//! finale (the emergencies here are lifted by the Owner). Deterministic:
//! held journal appends, held owed writes, held final checks (the broker's
//! clock hook), a poisoned coordinator; no sleeps.
//!
//! The engaged cell is the only "stopped" fact. A journal's identity is the
//! opened file's own (`fstat`), never its path's text; a replayed reference
//! to a surviving run must carry this journal's cell or that run is stopped
//! and the open typed (X2″); a captured execution token is reached through a replacement
//! coordinator opened by any accepted alias of the journal, and through a
//! permitted adoption; the emergency's cohort is the cells the broker's
//! ingress engaged, recorded on the first recording-capable touch; every
//! Stop path is typed on poison, end to end.

use super::m2as_support::Disk;
use super::m2c_support::{consent, port_for, Home};
use super::{mother_agent, FileJournal, GovernedPort};
use crate::coordinator::{Coordinator, CoordinatorError, IdentityRefusal, Lanes};
use crate::effect::EffectPort;
use crate::evidence::SettlementOutcome;
use crate::journal::{Event, JournalError, JournalStore, MemoryJournal, MissionId};
use crate::oracles::m2as_hooks::HookSlot;
use crate::oracles::m2c_owner::OwnerLiftsTheStop;
use crate::oracles::m2d_c::{files_lanes, FilesOwner};
use crate::oracles::{contract_with, Yes};
use crate::state::{MissionState, Trigger};
use nexus_governed_control::authority::clock::{Clock, ManualClock};
use nexus_governed_control::authority::halt_cell::HaltCause;
use nexus_governed_control::authority::ids::{CommitmentId, RunId};
use nexus_governed_control::files::FileCreate;
use nexus_governed_control::governed::{AgentOutcome, Intent};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

const BOUND: Duration = Duration::from_secs(30);
/// Past the test contracts' time budget (one hour).
const PAST_BUDGET: Duration = Duration::from_millis(3_600_001);

type Hold = Arc<Mutex<Option<(mpsc::Sender<()>, mpsc::Receiver<()>)>>>;

/// A journal whose next append, once armed, waits for its release.
struct Gated {
    inner: FileJournal,
    hold: Hold,
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
    fn anchor(&self) -> Option<Box<dyn std::any::Any + Send + Sync>> {
        self.inner.anchor()
    }
}

struct Rig<'a> {
    home: &'a Home,
    ws: PathBuf,
    folder: String,
    port: Arc<GovernedPort>,
    mother: Coordinator<GovernedPort>,
    clock: Arc<ManualClock>,
    owner: FilesOwner,
    hold: Hold,
}

impl<'a> Rig<'a> {
    fn new(home: &'a Home, journal: &str) -> Self {
        Self::at(home, &home.dir.join(journal))
    }

    /// Over the journal file at `path` (any path the file journal accepts).
    fn at(home: &'a Home, path: &Path) -> Self {
        Self::try_at(home, path).expect("the coordinator opens")
    }

    fn try_at(home: &'a Home, path: &Path) -> Result<Self, CoordinatorError> {
        let port = Arc::new(port_for(&home.control, true).with_workspace(folder_of(home)));
        let hold: Hold = Arc::default();
        let store = Gated {
            inner: FileJournal::open(path).unwrap(),
            hold: hold.clone(),
        };
        Self::over(home, port, store, hold)
    }

    fn over(
        home: &'a Home,
        port: Arc<GovernedPort>,
        store: impl JournalStore + 'static,
        hold: Hold,
    ) -> Result<Self, CoordinatorError> {
        let folder = folder_of(home);
        let ws = std::fs::canonicalize(std::env::temp_dir())
            .unwrap()
            .join(&folder);
        let clock = Arc::new(ManualClock::default());
        let mother = Coordinator::open(
            store,
            port.clone(),
            Lanes::new(files_lanes()).unwrap(),
            clock.clone() as Arc<dyn Clock>,
            home.private(),
        )?;
        Ok(Self {
            home,
            ws,
            folder,
            port,
            mother,
            clock,
            owner: FilesOwner::default(),
            hold,
        })
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
    fn waiting(&self, tag: &str) -> MissionId {
        let id = self.approved(tag);
        for _ in 0..3 {
            self.mother.advance(id).unwrap();
        }
        assert_eq!(self.mother.state(id), Some(MissionState::AwaitingApproval));
        id
    }

    /// The Owner approves the waiting File natively: authorized, poised.
    fn approve(&self, id: MissionId) -> (RunId, CommitmentId) {
        let pending = self.port.pending(id).expect("the waiting action");
        let action = (
            RunId::parse(&pending.run).unwrap(),
            CommitmentId::parse(&pending.commitment).unwrap(),
        );
        self.authorize(action);
        action
    }

    fn authorize(&self, (run, commitment): (RunId, CommitmentId)) {
        self.home
            .control
            .authorize(commitment, &mother_agent(), run, &self.owner)
            .unwrap();
    }

    fn execute(&self, (run, commitment): (RunId, CommitmentId)) -> bool {
        self.home
            .control
            .execute(commitment, &mother_agent(), run)
            .is_ok()
    }

    /// The mission's halt cell (the broker's), by this journal's identity.
    fn cause(&self, id: MissionId) -> Option<HaltCause> {
        self.port
            .halt_cells()
            .get(self.mother.journal_identity(), id.0)
            .and_then(|cell| cell.cause())
    }

    fn engaged(&self, id: MissionId) -> bool {
        self.port
            .halt_cells()
            .get(self.mother.journal_identity(), id.0)
            .is_none_or(|cell| cell.is_engaged())
    }

    fn hold_next_append(&self) -> (mpsc::Receiver<()>, mpsc::Sender<()>) {
        let (entered_tx, entered) = mpsc::channel();
        let (release, release_rx) = mpsc::channel();
        *self.hold.lock().unwrap() = Some((entered_tx, release_rx));
        (entered, release)
    }

    /// The mission's journaled settlement outcomes.
    fn terminal(&self, id: MissionId) -> Vec<SettlementOutcome> {
        self.mother
            .journal()
            .unwrap()
            .into_iter()
            .filter(|e| e.mission == id)
            .filter_map(|e| match e.event {
                Event::Transition {
                    trigger: Trigger::Settled { outcome, .. },
                } => Some(outcome),
                _ => None,
            })
            .collect()
    }

    /// Whether the journal has an ordinary `ActionApproved` for `id`.
    fn action_approved(&self, id: MissionId) -> bool {
        self.mother.journal().unwrap().into_iter().any(|e| {
            e.mission == id
                && matches!(
                    e.event,
                    Event::Transition {
                        trigger: Trigger::ActionApproved { .. }
                    }
                )
        })
    }

    /// How many `Stop` transitions the journal has for `id`.
    fn stops(&self, id: MissionId) -> usize {
        self.mother
            .journal()
            .unwrap()
            .into_iter()
            .filter(|e| {
                e.mission == id
                    && matches!(
                        e.event,
                        Event::Transition {
                            trigger: Trigger::Stop { .. }
                        }
                    )
            })
            .count()
    }
}

fn folder_of(home: &Home) -> String {
    home.control.workspace().expect("the workspace").name
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
    a_captured_token_is_reached_through_every_alias_of_the_journal(home);
    an_identity_less_store_binds_its_replayed_runs_or_stops_them(home);
    a_captured_token_is_reached_through_a_permitted_adoption(home);
    retirement_racing_the_stop_all_snapshot(home);
    the_emergency_cohort_is_the_cells_the_ingress_engaged(home);
    an_emergency_recording_interrupted_by_poison_is_drained_on_reopen(home);
    with_the_stop_append_held_no_admission_passes(home);
    every_stop_path_is_typed_on_poison_end_to_end(home);
    a_journal_copied_to_another_inode_stops_the_surviving_run(home);
}

/// The journal's path relative to the current directory (`..` up to the
/// root, then down to the home).
fn relative_to_cwd(path: &Path) -> PathBuf {
    let cwd = std::env::current_dir().unwrap();
    let ups = cwd.components().count() - 1;
    let mut relative = PathBuf::new();
    for _ in 0..ups {
        relative.push("..");
    }
    relative.join(path.strip_prefix("/").unwrap())
}

/// R6-2 / X2 / X4: a poised execution (authorized under the original
/// coordinator and port) is reached by a Stop through a replacement opened
/// over the surviving broker through `./journal`, a relative path and a
/// parent-directory symlink: the file's own identity names the same cell
/// (the adoption is refused: D4(a)); with the replacement's
/// `SettlementStarted` append held, the execution's final check refuses,
/// nothing is published and the settlement is truthful. (The frozen
/// final-component symlink refusal stays. A token held before its final
/// check is the broker's `m2d_c_r6_a_token_held_before_its_final_check_sees_the_cell_engaged_later`.)
fn a_captured_token_is_reached_through_every_alias_of_the_journal(home: &Home) {
    let canonical = home.dir.join("m2d-c-r6-alias.jsonl");
    let link = std::env::temp_dir().join(format!(
        "m2d-c-r6-parent-link-{}",
        home.dir.file_name().unwrap().to_string_lossy()
    ));
    let _ = std::fs::remove_file(&link);
    std::os::unix::fs::symlink(&home.dir, &link).unwrap();
    let aliases: Vec<(&str, PathBuf)> = vec![
        ("dot", home.dir.join(".").join("m2d-c-r6-alias.jsonl")),
        ("relative", relative_to_cwd(&canonical)),
        ("parent-link", link.join("m2d-c-r6-alias.jsonl")),
    ];
    for (name, alias) in aliases {
        let s = Rig::at(home, &canonical);
        let tag = format!("r6-alias-{name}");
        let a = s.waiting(&tag);
        let poised = s.approve(a);
        let identity = s.mother.journal_identity().to_string();
        drop(s);
        let again = Rig::at(home, &alias);
        assert_eq!(
            again.mother.journal_identity(),
            identity,
            "{name}: the same file, the same identity"
        );
        assert!(!again.engaged(a), "{name}: the original cell, not engaged");
        assert_eq!(
            again.port.run_of(a),
            None,
            "{name}: the adoption was refused"
        );
        let (entered, release_append) = again.hold_next_append();
        let state = std::thread::scope(|scope| {
            let stopping = scope.spawn(|| again.mother.stop(a));
            entered.recv_timeout(BOUND).expect("SettlementStarted held");
            assert!(again.engaged(a), "{name}: engaged through the replacement");
            assert!(!again.execute(poised), "{name}: the final check refused");
            release_append.send(()).unwrap();
            stopping.join().unwrap()
        });
        assert!(
            state.as_ref().is_ok_and(|s| s.is_terminal()),
            "{name}: the settlement ended the mission: {state:?}"
        );
        let terminal = again.terminal(a);
        assert_eq!(terminal.len(), 1, "{name}: one terminal line");
        assert!(
            !matches!(terminal[0], SettlementOutcome::Published { .. }),
            "{name}: truthful: {terminal:?}"
        );
        assert!(
            !again.ws.join(format!("{tag}-a.txt")).exists(),
            "{name}: nothing published"
        );
    }
    let _ = std::fs::remove_file(&link);
}

/// X2″: in a test build an identity-less store takes a coordinator-local
/// identity even while other scenarios' runs are live at the shared broker
/// (work this journal never names); for every store, a replayed reference
/// to a surviving run must carry this journal's cell after the adoption:
/// an authorized File's run (its adoption refused by D4(a)) does not, so
/// the open stops that run (its original cells engaged `Fence`, the run
/// cancelled), adopts nothing and fails typed; the surviving execution's
/// final check then refuses. (Production refuses an identity-less store:
/// the guard in `oracles/m2d_c_r6.rs`.)
fn an_identity_less_store_binds_its_replayed_runs_or_stops_them(home: &Home) {
    fn anonymous(
        home: &Home,
        disk: Disk,
    ) -> Result<(Rig<'_>, Arc<GovernedPort>), CoordinatorError> {
        let port = Arc::new(port_for(&home.control, true).with_workspace(folder_of(home)));
        Rig::over(home, port.clone(), disk, Arc::default()).map(|rig| (rig, port))
    }
    // Another scenario's live run of an unhalted door: not this journal's.
    let other = Rig::new(home, "m2d-c-r6-anon-other.jsonl");
    let unrelated = other.waiting("r6-anon-other");
    assert!(other.port.run_of(unrelated).is_some(), "live at the broker");
    let disk = Disk::over(MemoryJournal::new());
    let (s, port) = anonymous(home, disk.clone()).expect("a test build's fallback");
    assert!(!port.fenced());
    let a = s.waiting("r6-anon-bound");
    let poised = s.approve(a);
    let cell = port
        .halt_cells()
        .get(s.mother.journal_identity(), a.0)
        .unwrap();
    drop(s);
    // The same lines, a new coordinator: its cell for A is fresh; A's run
    // survives with the original cell only, and D4(a) refuses the adoption.
    let refused = anonymous(home, disk.clone()).err();
    assert_eq!(
        refused,
        Some(CoordinatorError::Identity(
            IdentityRefusal::SurvivingRunMismatch
        ))
    );
    assert_eq!(cell.cause(), Some(HaltCause::Fence), "its original cell");
    let view = home.control.authority().runs().view(poised.0).unwrap();
    assert!(view.cancelled, "the surviving run is stopped");
    assert!(!other.execute(poised), "its final check refuses");
    assert!(!other.ws.join("r6-anon-bound-a.txt").exists());
    // The unrelated run is untouched: its mission goes on.
    assert!(
        other.execute(other.approve(unrelated)),
        "not this journal's work"
    );
    assert!(other.ws.join("r6-anon-other-a.txt").exists());
}

/// C5 (review 5, MEDIUM 5), the adoption case as the product has it: a
/// waiting File's run is never adopted by a replacement (D4(a): a pinned
/// Files obligation refuses any adoption, from the proposal on; the
/// replacement tries and is refused), so no Files token ever rests on an
/// adopted run; the replacement reaches the poised File by identity, and
/// after its Stop the execution's final check refuses. (The cell an
/// adoption attaches, and a token held before its final check, are the
/// broker's oracles.)
fn a_captured_token_is_reached_through_a_permitted_adoption(home: &Home) {
    let journal = "m2d-c-r6-adopt.jsonl";
    let s = Rig::new(home, journal);
    let a = s.waiting("r6-adopt");
    let pending = s.port.pending(a).unwrap();
    let action = (
        RunId::parse(&pending.run).unwrap(),
        CommitmentId::parse(&pending.commitment).unwrap(),
    );
    drop(s);
    let again = Rig::new(home, journal);
    assert_eq!(again.port.adopts_tried(), 1, "the adoption was tried");
    assert_eq!(again.port.run_of(a), None, "and refused: a pinned File");
    again.authorize(action);
    let (entered, release_append) = again.hold_next_append();
    std::thread::scope(|scope| {
        let stopping = scope.spawn(|| again.mother.stop(a));
        entered.recv_timeout(BOUND).expect("SettlementStarted held");
        assert!(again.engaged(a));
        assert!(!again.execute(action), "the final check refused");
        release_append.send(()).unwrap();
        let _ = stopping.join().unwrap();
    });
    assert!(
        !again.ws.join("r6-adopt-a.txt").exists(),
        "nothing published"
    );
    assert_eq!(again.terminal(a).len(), 1, "one truthful terminal line");
}

/// C5 (review 5, MEDIUM 5): a cell's retirement races the `stop_all`
/// snapshot (a hook between the snapshot and the engage): the retired
/// mission is recorded once (by its own Stop), the snapshot's other mission
/// is engaged and recorded, and a mission drafted after the snapshot is
/// ordered after `stop_all`: neither engaged nor stopped.
fn retirement_racing_the_stop_all_snapshot(home: &Home) {
    let s = Rig::new(home, "m2d-c-r6-retire.jsonl");
    let a = s.approved("r6-retire-a");
    let b = s.approved("r6-retire-b");
    let later: Arc<Mutex<Option<MissionId>>> = Arc::default();
    std::thread::scope(|scope| {
        let (snapshotted, go) = held_at(&s.mother.hooks.after_snapshot, 0);
        let stopping = scope.spawn(|| s.mother.stop_all());
        snapshotted
            .recv_timeout(BOUND)
            .expect("the cohort snapshotted");
        // B's own Stop retires its cell while the snapshot holds it.
        assert_eq!(s.mother.stop(b), Ok(MissionState::Stopped));
        *later.lock().unwrap() = Some(s.mother.draft(contract_with("")).unwrap());
        go.send(()).unwrap();
        assert_eq!(stopping.join().unwrap(), Ok(1), "A alone is counted");
    });
    let later = later.lock().unwrap().unwrap();
    assert_eq!(s.mother.state(a), Some(MissionState::Stopped));
    assert!(s.engaged(a));
    assert_eq!(s.stops(a), 1);
    assert_eq!(s.stops(b), 1, "recorded once");
    assert_eq!(s.mother.state(later), Some(MissionState::Draft), "after it");
    assert!(!s.engaged(later));
}

/// R6-3 / X1: Codex's scenario and the ingress rule. A is held (an unclean
/// reopen), B awaits recognition of an already-published File. (1) The
/// emergency, `advance(A)`, the Owner's lift, `advance(B)`: B is never
/// `ActionApproved`, runs no lane, and records its settlement (published,
/// blocked for the emergency), as does A. (2) Publish B, the emergency,
/// the lift with no coordinator call in between, `advance(B)`: the same.
fn the_emergency_cohort_is_the_cells_the_ingress_engaged(home: &Home) {
    let journal = "m2d-c-r6-emergency.jsonl";
    let s = Rig::new(home, journal);
    let a = s.waiting("r6-emg-a");
    drop(s);
    let s = Rig::new(home, journal);
    assert!(s.mother.held(a).is_some(), "A is held");
    let b = s.waiting("r6-emg-b");
    assert!(s.execute(s.approve(b)), "B's File is published");
    let performed = s.port.performed();
    home.control.emergency_stop();
    assert_eq!(
        s.cause(a),
        Some(HaltCause::Emergency),
        "engaged at the ingress"
    );
    assert_eq!(s.cause(b), Some(HaltCause::Emergency));
    let touched = s.mother.advance(a);
    assert_ne!(touched, Err(CoordinatorError::Held), "the cell comes first");
    home.control.resume(&OwnerLiftsTheStop).unwrap();
    let after = s.mother.advance(b);
    assert!(
        !matches!(&after, Ok(step) if step.to == MissionState::Verifying),
        "B never goes on: {after:?}"
    );
    assert!(!s.action_approved(b), "never ActionApproved");
    assert_eq!(s.port.performed(), performed, "no lane's effect ran");
    assert_eq!(s.mother.state(b), Some(MissionState::Blocked));
    assert_eq!(
        s.mother.reason(b).as_deref(),
        Some("file_publication_interrupted:emergency_stop")
    );
    assert!(matches!(
        s.terminal(b).as_slice(),
        [SettlementOutcome::Published { .. }]
    ));
    // A too: its waiting File is a predecessor's (a pinned File is never
    // adopted: D4(a)), so its settlement is truthfully uncertain.
    assert_eq!(s.mother.state(a), Some(MissionState::Blocked), "A too");
    assert_eq!(s.terminal(a), vec![SettlementOutcome::Uncertain]);
    assert_eq!(
        s.mother.reason(a).as_deref(),
        Some("file_publication_uncertain")
    );

    // (2) No coordinator call between the emergency and the lift.
    let s = Rig::new(home, "m2d-c-r6-emergency-lift.jsonl");
    let b = s.waiting("r6-lift-b");
    assert!(s.execute(s.approve(b)));
    home.control.emergency_stop();
    home.control.resume(&OwnerLiftsTheStop).unwrap();
    assert!(!home.control.authority().runs().is_stopped());
    assert_eq!(
        s.cause(b),
        Some(HaltCause::Emergency),
        "kept across the lift"
    );
    // A mission created after the lift is not of the cohort: the first
    // touch records the cells the ingress engaged, and takes no second
    // snapshot.
    let c = s.approved("r6-lift-c");
    let after = s.mother.advance(b);
    assert!(
        !matches!(&after, Ok(step) if step.to == MissionState::Verifying),
        "B never goes on: {after:?}"
    );
    assert!(!s.action_approved(b), "never ActionApproved");
    assert_eq!(
        s.mother.reason(b).as_deref(),
        Some("file_publication_interrupted:emergency_stop")
    );
    // Touched again: idempotent, the one terminal line.
    assert!(s.mother.advance(b).is_err());
    assert_eq!(s.terminal(b).len(), 1);
    assert!(!s.engaged(c), "not of the cohort");
    assert_eq!(
        s.mother.advance(c).map(|step| step.to),
        Ok(MissionState::Investigating),
        "runnable"
    );
}

/// R6-3 / X3: an emergency whose recording is interrupted by poison: the
/// cohort stays non-runnable, nothing panics, and the next healthy open of
/// the journal drains the pending recordings: each cohort mission's
/// terminal is recorded with the cell's first cause before anything of it
/// is exposed (never held and resumable).
fn an_emergency_recording_interrupted_by_poison_is_drained_on_reopen(home: &Home) {
    let journal = "m2d-c-r6-emergency-poison.jsonl";
    let s = Rig::new(home, journal);
    let a = s.approved("r6-poison-a");
    let b = s.waiting("r6-poison-b");
    let poised = s.approve(b);
    home.control.emergency_stop();
    s.mother.poison_for_test();
    let touched = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| s.mother.advance(a)));
    assert_eq!(
        touched.expect("never a panic").err(),
        Some(CoordinatorError::Poisoned)
    );
    assert!(!s.execute(poised), "refused");
    assert_eq!(
        s.mother.stop_all(),
        Err(CoordinatorError::Poisoned),
        "never Ok(0)"
    );
    home.control.resume(&OwnerLiftsTheStop).unwrap();
    drop(s);
    let again = Rig::new(home, journal);
    for id in [a, b] {
        assert_eq!(
            again.mother.held(id),
            None,
            "drained at the open, never held"
        );
        assert!(
            again
                .mother
                .state(id)
                .is_some_and(MissionState::is_terminal),
            "{id}: recorded"
        );
    }
    assert_eq!(again.mother.state(a), Some(MissionState::Stopped));
    assert_eq!(
        again.mother.reason(b).as_deref(),
        Some("file_publication_uncertain")
    );
    assert!(!again.ws.join("r6-poison-b-a.txt").exists());
}

/// R6-1 (review 5, MEDIUM 1), at Mother's port: with the mission's cell
/// engaged and its `SettlementStarted` append held, the door admits
/// nothing of its run: no opening, no proposal, no Files admission (the
/// same yes again included); nothing is bound, nothing proposed.
fn with_the_stop_append_held_no_admission_passes(home: &Home) {
    let s = Rig::new(home, "m2d-c-r6-admissions.jsonl");
    let b = s.waiting("r6-admissions");
    let run = s.port.run_of(b).unwrap();
    let views_before = home
        .control
        .authority()
        .commitments()
        .views_of_run(run)
        .len();
    // Read before the Stop holds the coordinator lock with its append.
    let hash = s.mother.consented_hash(b).unwrap().to_hex();
    let request = s.mother.consent_request_of(b).unwrap();
    let (entered, release) = s.hold_next_append();
    std::thread::scope(|scope| {
        let stopping = scope.spawn(|| s.mother.stop(b));
        entered.recv_timeout(BOUND).expect("SettlementStarted held");
        assert!(s.engaged(b));
        let door = &s.port.door;
        assert_eq!(
            door.admit_files(run, &hash, &request).unwrap_err().class(),
            "halted",
            "the same admission again"
        );
        assert_eq!(
            door.admit_files(run, &hash, "msn-000001/9/9")
                .unwrap_err()
                .class(),
            "halted"
        );
        let proposal = door.agent_action(
            run,
            &Intent::File(FileCreate {
                name: "r6-admissions-z.txt".into(),
                content: "z\n".into(),
            }),
        );
        assert!(
            !matches!(proposal, Ok(AgentOutcome::AwaitingApproval(_))),
            "no proposal: {proposal:?}"
        );
        let cell = s
            .port
            .halt_cells()
            .get(s.mother.journal_identity(), b.0)
            .unwrap();
        assert_eq!(
            door.open_run_with_halt(&cell).unwrap_err().class(),
            "halted",
            "no opening"
        );
        release.send(()).unwrap();
        let _ = stopping.join().unwrap();
    });
    assert_eq!(
        home.control
            .authority()
            .commitments()
            .views_of_run(run)
            .len(),
        views_before,
        "nothing proposed"
    );
    assert!(!s.ws.join("r6-admissions-z.txt").exists());
}

/// R6-4 (review 5, MEDIUM 2 and 3): the coordinator poisoned after a Stop
/// path's first locked step: (a) after `SettlementStarted`, before the
/// reconciliation; (b) an ordinary Stop before its closure; (c) the time
/// budget before its block; (d) `stop_all` on poison. Every result is
/// typed (`Poisoned`), none panics, the cells are engaged and nothing is
/// published.
fn every_stop_path_is_typed_on_poison_end_to_end(home: &Home) {
    // (a) Stop with a File: poisoned after SettlementStarted.
    let s = Rig::new(home, "m2d-c-r6-poison-settle.jsonl");
    let b = s.waiting("r6-poison-settle");
    let poised = s.approve(b);
    let (tx, rx) = mpsc::channel::<()>();
    let (stopped_tx, stopped) = mpsc::channel::<()>();
    s.mother.hooks.before_owed_write.arm(0, move || {
        stopped_tx.send(()).unwrap();
        let _ = rx.recv_timeout(BOUND);
    });
    std::thread::scope(|scope| {
        let stopping = scope
            .spawn(|| std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| s.mother.stop(b))));
        stopped
            .recv_timeout(BOUND)
            .expect("after SettlementStarted");
        s.mother.poison_for_test();
        tx.send(()).unwrap();
        let result = stopping.join().unwrap();
        assert_eq!(
            result.expect("never a panic"),
            Err(CoordinatorError::Poisoned),
            "(a)"
        );
    });
    assert!(s.engaged(b), "(a)");
    assert!(!s.execute(poised), "(a)");
    assert!(!s.ws.join("r6-poison-settle-a.txt").exists());

    // (b) An ordinary Stop (approved, no File): poisoned before its closure.
    let s = Rig::new(home, "m2d-c-r6-poison-close.jsonl");
    let a = s.approved("r6-poison-close");
    let (tx, rx) = mpsc::channel::<()>();
    let (stopped_tx, stopped) = mpsc::channel::<()>();
    s.mother.hooks.before_owed_write.arm(0, move || {
        stopped_tx.send(()).unwrap();
        let _ = rx.recv_timeout(BOUND);
    });
    std::thread::scope(|scope| {
        let stopping = scope
            .spawn(|| std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| s.mother.stop(a))));
        stopped
            .recv_timeout(BOUND)
            .expect("after the Stop's first step");
        s.mother.poison_for_test();
        tx.send(()).unwrap();
        let result = stopping.join().unwrap();
        assert_eq!(
            result.expect("never a panic"),
            Err(CoordinatorError::Poisoned),
            "(b)"
        );
    });
    assert!(s.engaged(a), "(b)");

    // (c) The time budget: poisoned after the barrier's first step.
    let s = Rig::new(home, "m2d-c-r6-poison-budget.jsonl");
    let a = s.approved("r6-poison-budget");
    s.clock.advance(PAST_BUDGET);
    let (tx, rx) = mpsc::channel::<()>();
    let (stopped_tx, stopped) = mpsc::channel::<()>();
    s.mother.hooks.before_owed_write.arm(1, move || {
        stopped_tx.send(()).unwrap();
        let _ = rx.recv_timeout(BOUND);
    });
    std::thread::scope(|scope| {
        let advancing = scope.spawn(|| {
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| s.mother.advance(a)))
        });
        stopped
            .recv_timeout(BOUND)
            .expect("after the budget's first step");
        s.mother.poison_for_test();
        tx.send(()).unwrap();
        let result = advancing.join().unwrap();
        assert_eq!(
            result.expect("never a panic").err(),
            Some(CoordinatorError::Poisoned),
            "(c)"
        );
    });
    assert_eq!(s.cause(a), Some(HaltCause::TimeBudget), "(c)");

    // (d) `stop_all` on poison.
    let s = Rig::new(home, "m2d-c-r6-poison-all.jsonl");
    let _a = s.approved("r6-poison-all");
    s.mother.poison_for_test();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| s.mother.stop_all()));
    assert_eq!(
        result.expect("never a panic"),
        Err(CoordinatorError::Poisoned),
        "(d): never Ok(0)"
    );
}

/// X2: a journal copied to another inode (a different file with the same
/// records) names fresh cells; its replayed reference to a surviving run
/// does not carry them: the open stops that run (its original cells
/// engaged, the run cancelled) and fails typed, adopting nothing; the
/// surviving execution's final check then refuses, and nothing is
/// published.
fn a_journal_copied_to_another_inode_stops_the_surviving_run(home: &Home) {
    let original = home.dir.join("m2d-c-r6-copied.jsonl");
    let copy = home.dir.join("m2d-c-r6-copied-twin.jsonl");
    let s = Rig::at(home, &original);
    let a = s.waiting("r6-copied");
    let poised = s.approve(a);
    let original_cell = s
        .port
        .halt_cells()
        .get(s.mother.journal_identity(), a.0)
        .unwrap();
    drop(s);
    std::fs::copy(&original, &copy).unwrap();
    let port = Arc::new(port_for(&home.control, true).with_workspace(folder_of(home)));
    let hold: Hold = Arc::default();
    let store = Gated {
        inner: FileJournal::open(&copy).unwrap(),
        hold,
    };
    let refused = Rig::over(home, port.clone(), store, Arc::default()).err();
    assert_eq!(
        refused,
        Some(CoordinatorError::Identity(
            IdentityRefusal::SurvivingRunMismatch
        ))
    );
    assert_eq!(port.run_of(a), None, "nothing adopted");
    assert!(port.fenced(), "fenced");
    assert_eq!(original_cell.cause(), Some(HaltCause::Fence));
    let view = home.control.authority().runs().view(poised.0).unwrap();
    assert!(view.cancelled, "the surviving run is stopped");
    let s = Rig::at(home, &original);
    assert!(!s.execute(poised), "its final check refuses");
    assert!(!s.ws.join("r6-copied-a.txt").exists());
}
