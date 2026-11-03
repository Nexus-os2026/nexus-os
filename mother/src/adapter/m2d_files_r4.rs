//! M2d-C round 4: the settlement design v2 and its binding addendum against
//! the real broker, pre-registered before the repair. They run inside
//! `real_broker_m1_oracles`, after `m2d_files_r2`, before its emergency-stop
//! finale; each scenario has its own door (a Stop with a File live halts
//! it, for good). Deterministic: a held clock reading at the execution's
//! final check, a held owed write, a held journal append; no sleeps.
//!
//! - (2) Executing at a Stop: pending, so uncertain and Blocked; the other
//!   mission's File, confirmed not published, ends Stopped; the execution
//!   then finishes (the D2 one) and never reopens its mission, whose File is
//!   never acknowledged.
//! - Addendum 1: with the first mission's Stop held at its journal append,
//!   and separately at its owed write, another mission's File poised before
//!   its final check (authorized) does not publish.
//! - Addendum 4: a crash after the terminal line and before the ack; a
//!   replacement door's reopen acknowledges it (ack-only ownership).
//!
//! Not here (no seam from Mother): the broker's registries poisoned or at
//! capacity, and the kernel's uncertain publication: the broker's own
//! round-4 oracles hold them.

use super::m2c_support::{consent, port_for, Home};
use super::{mother_agent, FileJournal, GovernedPort};
use crate::coordinator::{Coordinator, Lanes};
use crate::effect::EffectPort;
use crate::evidence::SettlementOutcome;
use crate::journal::{Event, JournalError, JournalStore, MissionId};
use crate::oracles::m2d_c::{files_lanes, FilesOwner};
use crate::oracles::{contract_with, Yes};
use crate::state::{MissionState, Trigger};
use nexus_governed_control::authority::clock::SystemClock;
use nexus_governed_control::authority::ids::{CommitmentId, RunId};
use std::path::PathBuf;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

const BOUND: Duration = Duration::from_secs(30);

/// A journal append's hold: its entry signal and its release.
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
}

struct Scenario<'a> {
    home: &'a Home,
    ws: PathBuf,
    folder: String,
    port: Arc<GovernedPort>,
    mother: Coordinator<GovernedPort>,
    owner: FilesOwner,
    hold: Hold,
}

impl<'a> Scenario<'a> {
    fn new(home: &'a Home, journal: &str) -> Self {
        let folder = home.control.workspace().expect("the workspace").name;
        let ws = std::fs::canonicalize(std::env::temp_dir())
            .unwrap()
            .join(&folder);
        let port = Arc::new(port_for(&home.control, true).with_workspace(folder.clone()));
        let hold: Hold = Arc::default();
        let mother = Coordinator::open(
            Gated {
                inner: FileJournal::open(&home.dir.join(journal)).unwrap(),
                hold: hold.clone(),
            },
            port.clone(),
            Lanes::new(files_lanes()).unwrap(),
            Arc::new(SystemClock::default()),
            home.private(),
        )
        .unwrap();
        Self {
            home,
            ws,
            folder,
            port,
            mother,
            owner: FilesOwner::default(),
            hold,
        }
    }

    /// Approved and advanced to the Builder's File A, waiting.
    fn waiting(&self, tag: &str) -> MissionId {
        let mut contract = contract_with(&format!(
            "MKFILE:{tag}-a.txt MKFILE:{tag}-b.txt VFILE:{tag}-c.txt"
        ));
        contract.requested_folder = Some(self.folder.clone());
        let id = self.mother.draft(contract).unwrap();
        self.mother.submit(id).unwrap();
        assert_eq!(consent(&self.mother, id, &Yes), Ok(MissionState::Approved));
        for _ in 0..3 {
            self.mother.advance(id).unwrap();
        }
        assert_eq!(self.mother.state(id), Some(MissionState::AwaitingApproval));
        id
    }

    /// The Owner approves the waiting File natively: authorized, poised
    /// before its execution.
    fn approve(&self, id: MissionId) -> (RunId, CommitmentId) {
        let pending = self.port.pending(id).expect("the waiting action");
        let run = RunId::parse(&pending.run).unwrap();
        let commitment = CommitmentId::parse(&pending.commitment).unwrap();
        self.home
            .control
            .authorize(commitment, &mother_agent(), run, &self.owner)
            .unwrap();
        (run, commitment)
    }

    fn execute(&self, (run, commitment): (RunId, CommitmentId)) -> bool {
        self.home
            .control
            .execute(commitment, &mother_agent(), run)
            .is_ok()
    }

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

    /// Arm the journal's next append to wait; returns its entry signal and
    /// its release.
    fn hold_next_append(&self) -> (mpsc::Receiver<()>, mpsc::Sender<()>) {
        let (entered_tx, entered) = mpsc::channel();
        let (release, release_rx) = mpsc::channel();
        *self.hold.lock().unwrap() = Some((entered_tx, release_rx));
        (entered, release)
    }
}

pub(super) fn on_the_real_broker(home: &Home) {
    executing_at_a_stop(home);
    the_memory_stop_precedes_the_journal(home);
    a_replacement_door_acknowledges_on_reopen(home);
}

/// (2), with (4)'s two missions: A's execution held at its final check
/// (the first broker clock reading once it is past its start); Stop
/// everything: A is pending, so uncertain; B's File, revoked by the
/// withdrawal, is confirmed not published. The execution then finishes and
/// A stays as it ended, its File never acknowledged.
fn executing_at_a_stop(home: &Home) {
    let s = Scenario::new(home, "m2d-c-r4-executing.jsonl");
    let a = s.waiting("r4-exec-a");
    let b = s.waiting("r4-exec-b");
    let action = s.approve(a);
    let (entered_tx, entered) = mpsc::channel();
    let (release, release_rx) = mpsc::channel::<()>();
    home.clock.arm(
        super::m2d_files::past_the_start(home, action.0),
        move || {
            entered_tx.send(()).unwrap();
            let _ = release_rx.recv_timeout(BOUND);
        },
    );
    std::thread::scope(|scope| {
        let worker = scope.spawn(|| s.execute(action));
        entered
            .recv_timeout(BOUND)
            .expect("held at its final check");
        assert_eq!(s.mother.stop_all(), 2);
        assert_eq!(s.mother.state(a), Some(MissionState::Blocked));
        assert_eq!(
            s.mother.reason(a).as_deref(),
            Some("file_publication_uncertain")
        );
        assert_eq!(s.terminal(a), vec![SettlementOutcome::Uncertain]);
        assert_eq!(s.mother.state(b), Some(MissionState::Stopped));
        assert_eq!(s.terminal(b), vec![SettlementOutcome::NotPublished]);
        release.send(()).unwrap();
        assert!(worker.join().unwrap(), "the one handed-off write finishes");
    });
    assert!(s.ws.join("r4-exec-a-a.txt").exists());
    assert!(!s.ws.join("r4-exec-b-a.txt").exists());
    assert_eq!(
        s.mother.state(a),
        Some(MissionState::Blocked),
        "never reopened"
    );
    assert_eq!(s.terminal(a), vec![SettlementOutcome::Uncertain]);
    assert!(s.mother.advance(a).is_err());
    assert_eq!(
        s.port.files_live(),
        1,
        "A stays pinned and live; B released"
    );
}

/// Addendum 1's schedules, scoped by C4 (round-5 amendment): a mission
/// Stop affects only that mission; Stop everything affects its cohort.
/// (i) Stop everything with A's first append, then (another scenario) its
/// owed write, held: B's authorized File is refused, nothing written, B
/// stopped. (ii) Stop B while an unrelated append holds the coordinator: B's
/// File is refused, B stopped. (iii) Stop A while B's File is live: B's
/// File publishes normally and B goes on.
fn the_memory_stop_precedes_the_journal(home: &Home) {
    for (journal, held_at) in [
        ("m2d-c-r4-owed.jsonl", "owed"),
        ("m2d-c-r4-append.jsonl", "append"),
    ] {
        let s = Scenario::new(home, journal);
        let _a = s.waiting(&format!("r4-{held_at}-a"));
        let b = s.waiting(&format!("r4-{held_at}-b"));
        let poised = s.approve(b);
        let (entered, release) = if held_at == "owed" {
            let (entered_tx, entered) = mpsc::channel();
            let (release, release_rx) = mpsc::channel::<()>();
            s.mother.hooks.before_owed_write.arm(0, move || {
                entered_tx.send(()).unwrap();
                let _ = release_rx.recv_timeout(BOUND);
            });
            (entered, release)
        } else {
            s.hold_next_append()
        };
        std::thread::scope(|scope| {
            let stopping = scope.spawn(|| s.mother.stop_all());
            entered.recv_timeout(BOUND).expect("the Stop held");
            assert!(!s.execute(poised), "{held_at}: the poised File is refused");
            release.send(()).unwrap();
            let _ = stopping.join().unwrap();
        });
        let name = s.ws.join(format!("r4-{held_at}-b-a.txt"));
        assert!(!name.exists(), "{held_at}: nothing published");
        assert_eq!(s.mother.state(b), Some(MissionState::Stopped), "{held_at}");
    }

    // (ii) The target is B; an unrelated append holds the coordinator. B's
    // Stop engages its cell (the hook after the halt), then waits for the
    // coordinator: B's File is refused meanwhile.
    let s = Scenario::new(home, "m2d-c-r4-target.jsonl");
    let b = s.waiting("r4-target-b");
    let poised = s.approve(b);
    let (entered, release) = s.hold_next_append();
    let (halted_tx, halted) = mpsc::channel();
    s.mother
        .hooks
        .after_halt
        .arm(0, move || halted_tx.send(()).unwrap());
    std::thread::scope(|scope| {
        let drafting = scope.spawn(|| s.mother.draft(contract_with("")));
        entered
            .recv_timeout(BOUND)
            .expect("the unrelated append held");
        let stopping = scope.spawn(|| s.mother.stop(b));
        halted.recv_timeout(BOUND).expect("B's cell engaged");
        assert!(!s.execute(poised), "(ii): the target's File is refused");
        release.send(()).unwrap();
        let _ = drafting.join().unwrap();
        let _ = stopping.join().unwrap();
    });
    assert!(
        !s.ws.join("r4-target-b-a.txt").exists(),
        "(ii): nothing published"
    );
    assert_eq!(s.mother.state(b), Some(MissionState::Stopped), "(ii)");

    // (iii) Stop A; B's live File publishes normally and B goes on.
    let s = Scenario::new(home, "m2d-c-r4-scope.jsonl");
    let a = s.waiting("r4-scope-a");
    let b = s.waiting("r4-scope-b");
    let live = s.approve(b);
    assert!(s.mother.stop(a).is_ok());
    assert!(s.execute(live), "(iii): B's File publishes");
    assert!(s.ws.join("r4-scope-b-a.txt").exists());
    let step = s.mother.advance(b).unwrap();
    assert_ne!(step.to, MissionState::Blocked, "(iii): B goes on");
    assert_ne!(s.mother.state(b), Some(MissionState::Stopped));
}

/// Addendum 4: the terminal line is written, then the coordinator crashes
/// before its acknowledgement (at the owed write after the terminal step).
/// Its door is gone with it; a replacement door's reopen acknowledges the
/// conclusive outcome, and is shown nothing more.
fn a_replacement_door_acknowledges_on_reopen(home: &Home) {
    let journal = "m2d-c-r4-reack.jsonl";
    let s = Scenario::new(home, journal);
    let id = s.waiting("r4-reack");
    let action = s.approve(id);
    assert!(s.execute(action), "published");
    s.mother
        .hooks
        .before_owed_write
        .arm(1, || panic!("a crash after the terminal line (test)"));
    let crashed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| s.mother.stop(id)));
    assert!(crashed.is_err());
    assert_eq!(s.port.acks_tried(), 0, "not acknowledged before the crash");
    drop(s);
    let again = Scenario::new(home, journal);
    assert_eq!(again.mother.state(id), Some(MissionState::Blocked));
    assert_eq!(
        again.mother.reason(id).as_deref(),
        Some("file_publication_interrupted:owner_stop")
    );
    assert_eq!(again.port.acks_tried(), 1, "acknowledged on reopen");
    assert_eq!(again.port.unacked(), 0, "by ack-only ownership");
}
