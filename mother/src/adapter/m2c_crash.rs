//! M2c-B (PR-B) crash-point oracles, pre-registered before the halt exists
//! (design v3 §6, §7.3, §11 C4): MC1–MC4 in-process over the shared broker,
//! and CR1–CR7 across real processes (N4b). CR8's property (an Executing
//! member, the whole batch durable, a crash before its end: unsettled) is
//! the broker's R6 (Architect decision 3): no record is written between
//! Executing and Finished, so no Mother test can hold a worker there.
//!
//! The child-process harness: a CR test re-runs this test binary with
//! `--exact adapter::m2c_crash::m2c_crash_child`, which does nothing unless
//! `MOTHER_M2C_CHILD` is set. Child 1 builds its own broker and Mother in a
//! fresh home (one front door per process), runs to the crash point and
//! aborts; just before, it writes down every broker record committed so far
//! (a projection sees a record only after its commit), then its receipt:
//! the point it is at, whether its coordinator broke, the halt records
//! counted since the break began and how many records the gate saw. The
//! parent accepts only that abort with a well-formed receipt naming the
//! expected point. Child 2 reopens the
//! same home in a new process, answers the dialogs by script, and prints its
//! observations as one JSON line. The parent waits for each child with a
//! 60 s bound, polling its own child handle and killing that handle on
//! timeout (never a PID or a name). No new dependency.

use super::m2as_support::{rig, Disk, Rig};
use super::m2c_support::{
    arm_call_point, consent, door_open_run, halt_lost, port_for, resume_with, try_port_for,
    CallPoint, HeldOwner, Home,
};
use super::{FileJournal, GovernedPort};
use crate::coordinator::{Coordinator, CoordinatorError, Lanes};
use crate::journal::{JournalError, JournalStore, MemoryJournal, MissionId};
use crate::oracles::m2c_owner::{OwnerGrants, OwnerRecovers};
use crate::oracles::{contract_with, keyed_lanes, Yes};
use crate::state::MissionState;
use nexus_governed_control::authority::clock::SystemClock;
use nexus_governed_control::governed::{GovernedControl, GrantRequest};
use serde_json::{json, Value};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, Weak};
use std::time::{Duration, Instant};

// ---------------------------------------------------------------- shared

fn approved(r: &Rig, markers: &str) -> MissionId {
    let id = r.mother.draft(contract_with(markers)).unwrap();
    r.mother.submit(id).unwrap();
    assert_eq!(consent(&r.mother, id, &Yes), Ok(MissionState::Approved));
    id
}

fn waiting(r: &Rig, origin: &str, path: &str) -> MissionId {
    let id = approved(r, &format!("POST-BUILDER:{origin}/{path}"));
    for _ in 0..3 {
        r.mother.advance(id).unwrap();
    }
    assert_eq!(r.mother.state(id), Some(MissionState::AwaitingApproval));
    id
}

/// Break the coordinator: a Stop of `victim` whose append fails (and whose
/// poisoning fails too unless `marker`).
fn break_it(disk: &Disk, mother: &Coordinator<GovernedPort>, victim: MissionId, marker: bool) {
    disk.fail(true, !marker);
    assert!(mother.stop(victim).is_err());
    assert!(mother.broken());
    disk.fail(false, false);
}

fn doors_asked(home: &Home) -> usize {
    home.gate.count("agent_door_opened") + home.gate.count("agent_door_declined")
}

// ---------------------------------------------------------------- MC1-MC4

/// MC1 (P1/P4): the appends and the poisoning fail. Mother's restart (over
/// the live broker): the next door is asked for; the journal reopens with
/// the mission held; its resume asks.
fn m2c_mc1_a_failed_poison_reopens_held(home: &Home, origin: &str) {
    let control = &home.control;
    let disk = Disk::over(MemoryJournal::new());
    let r = rig(control, disk.clone()).unwrap();
    let id = waiting(&r, origin, "m2c-mc1");
    let victim = approved(&r, "");
    break_it(&disk, &r.mother, victim, false);
    assert_eq!(door_open_run(&r.port), Err("halted".to_string()));
    drop(r);
    let before = doors_asked(home);
    let r = rig(control, disk).unwrap();
    assert_eq!(
        doors_asked(home),
        before + 1,
        "MC1: the next door is asked for"
    );
    assert!(r.mother.held(id).is_some(), "MC1: held after the break");
    let owner = HeldOwner::new(true);
    assert!(resume_with(&r.mother, id, &owner).is_ok());
    assert_eq!(owner.shown().len(), 1, "MC1: the resume asks");
}

/// MC2 (P2, observed live): the failing call is held at its first batch
/// record. Meanwhile the journal is poisoned, a new coordinator over it is
/// refused, the door is halted, and the next door is asked for.
fn m2c_mc2_the_marker_comes_before_the_records(home: &Home, origin: &str) {
    let control = &home.control;
    let disk = Disk::over(MemoryJournal::new());
    let r = rig(control, disk.clone()).unwrap();
    waiting(&r, origin, "m2c-mc2");
    let victim = approved(&r, "");
    home.gate.arm("door_halted", None);
    let (d, mother) = (disk.clone(), Arc::downgrade(&r.mother));
    let breaking = std::thread::spawn(move || {
        let mother: Arc<Coordinator<GovernedPort>> = Weak::upgrade(&mother).unwrap();
        break_it(&d, &mother, victim, true);
    });
    let held = home.gate.wait_held();
    let poisoned = disk.inner.is_poisoned();
    let before = doors_asked(home);
    let reopened = rig(control, disk.clone()).err();
    let asked = doors_asked(home) - before;
    let halted = door_open_run(&r.port);
    home.gate.open();
    let _ = breaking.join();
    assert!(held, "MC2: the failing call's first batch record is held");
    assert!(poisoned, "MC2: the marker exists before the records");
    assert_eq!(
        reopened,
        Some(CoordinatorError::Journal(JournalError::Poisoned))
    );
    assert_eq!(asked, 1, "MC2: the next door is asked for");
    assert_eq!(halted, Err("halted".to_string()));
}

/// MC3 (P3): the same after the batch is written.
fn m2c_mc3_after_the_records(home: &Home, origin: &str) {
    let control = &home.control;
    let disk = Disk::over(MemoryJournal::new());
    let r = rig(control, disk.clone()).unwrap();
    waiting(&r, origin, "m2c-mc3");
    let victim = approved(&r, "");
    let halted_before = home.gate.count("door_halted");
    break_it(&disk, &r.mother, victim, true);
    assert_eq!(
        home.gate.count("door_halted"),
        halted_before + 1,
        "MC3: written"
    );
    assert!(disk.inner.is_poisoned());
    let before = doors_asked(home);
    assert_eq!(
        rig(control, disk.clone()).err(),
        Some(CoordinatorError::Journal(JournalError::Poisoned))
    );
    assert_eq!(doors_asked(home), before + 1);
    assert_eq!(door_open_run(&r.port), Err("halted".to_string()));
}

/// MC4: a break whose batch is dropped (a panic in the fence's sweep): the
/// loss is counted, and the halt holds.
fn m2c_mc4_a_dropped_batch_is_counted_and_the_halt_holds(home: &Home, origin: &str) {
    let control = &home.control;
    let disk = Disk::over(MemoryJournal::new());
    let r = rig(control, disk.clone()).unwrap();
    let id = waiting(&r, origin, "m2c-mc4");
    let run = r.port.run_of(id).unwrap();
    let victim = approved(&r, "");
    let lost = halt_lost(control);
    r.port
        .fence_between_cancels
        .arm(0, || panic!("the sweep panics (test)"));
    let broke = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        break_it(&disk, &r.mother, victim, true)
    }));
    disk.fail(false, false);
    assert!(broke.is_err());
    assert!(
        halt_lost(control) > lost,
        "MC4: the dropped batch is counted"
    );
    assert!(control.authority().runs().view(run).unwrap().cancelled);
    assert_eq!(door_open_run(&r.port), Err("halted".to_string()));
}

pub(super) fn in_process_on_the_real_broker(home: &Home, origin: &str) {
    m2c_mc1_a_failed_poison_reopens_held(home, origin);
    m2c_mc2_the_marker_comes_before_the_records(home, origin);
    m2c_mc3_after_the_records(home, origin);
    m2c_mc4_a_dropped_batch_is_counted_and_the_halt_holds(home, origin);
}

// ---------------------------------------------------------------- the child

/// A file journal whose appends (and poisoning) fail on demand: durable
/// across processes.
struct FailingFile {
    file: FileJournal,
    appends: Arc<AtomicBool>,
    poison: Arc<AtomicBool>,
}

impl JournalStore for FailingFile {
    fn load(&self) -> Result<Vec<String>, JournalError> {
        self.file.load()
    }
    fn append(&mut self, line: &str) -> Result<(), JournalError> {
        if self.appends.load(Ordering::SeqCst) {
            return Err(JournalError::Store("injected append failure".into()));
        }
        self.file.append(line)
    }
    fn poison(&mut self) -> Result<(), JournalError> {
        if self.poison.load(Ordering::SeqCst) {
            return Err(JournalError::Store("injected poison failure".into()));
        }
        self.file.poison()
    }
}

/// The world, in the child: answers every request, except one whose path
/// holds `/hang`, which it takes and never answers.
fn world(listener: TcpListener) {
    std::thread::spawn(move || {
        let mut kept = Vec::new();
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let mut request = [0u8; 4096];
            let n = stream.read(&mut request).unwrap_or(0);
            let line = String::from_utf8_lossy(&request[..n]).to_string();
            if line.contains("/hang") {
                kept.push(stream);
            } else {
                let _ = stream.write_all(
                    b"HTTP/1.1 200 OK\r\nContent-Length: 6\r\nConnection: close\r\n\r\nmother",
                );
            }
        }
    });
}

/// Every halt record the broker has counted, in any state.
fn counted(home: &Home) -> u64 {
    let c = home.control.status().halt_records;
    c.pending + c.recorded + c.owed + c.uncertain + c.lost
}

/// Write `text` to `name` in `dir`, durably.
fn write_synced(dir: &Path, name: &str, text: &str) {
    let mut file = std::fs::File::create(dir.join(name)).unwrap();
    file.write_all(text.as_bytes()).unwrap();
    file.sync_all().unwrap();
}

/// Child 1 at its named crash point: where the break began (the records
/// the gate had seen, and the halt records counted, before it).
#[derive(Clone)]
struct At {
    home: Arc<Home>,
    dir: PathBuf,
    point: &'static str,
    break_at: usize,
    base: u64,
}

impl At {
    /// Write down every broker record committed so far (with where the
    /// break began), then the receipt (`broken`: whether the coordinator
    /// broke), then abort the process.
    fn crash(&self, broken: bool) -> ! {
        let seen = self.home.gate.seen();
        let mut text = format!("break_at={}\n", self.break_at);
        for record in &seen {
            text.push_str(&record.phase);
            text.push('\n');
        }
        write_synced(&self.dir, "seen.log", &text);
        let batch = counted(&self.home) - self.base;
        write_synced(
            &self.dir,
            "receipt.log",
            &format!(
                "point={}\nbroken={broken}\nbatch={batch}\ngate={}\n",
                self.point,
                seen.len()
            ),
        );
        std::process::abort();
    }
}

struct Child {
    home: Arc<Home>,
    dir: PathBuf,
    origin: String,
    appends: Arc<AtomicBool>,
    poison: Arc<AtomicBool>,
}

impl Child {
    fn open(dir: &Path) -> Self {
        let home = Arc::new(Home::open(dir).unwrap());
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        world(listener);
        home.control
            .request_grant(
                &GrantRequest::Egress {
                    origin: origin.clone(),
                    methods: vec!["GET".into(), "POST".into()],
                    allow_private: true,
                },
                Duration::from_secs(600),
                &OwnerGrants,
            )
            .unwrap();
        Child {
            home,
            dir: dir.to_path_buf(),
            origin,
            appends: Arc::new(AtomicBool::new(false)),
            poison: Arc::new(AtomicBool::new(false)),
        }
    }

    fn mother(&self) -> Arc<Coordinator<GovernedPort>> {
        let journal = FailingFile {
            file: self.home.journal().unwrap(),
            appends: self.appends.clone(),
            poison: self.poison.clone(),
        };
        Arc::new(
            Coordinator::open(
                journal,
                Arc::new(port_for(&self.home.control, true)),
                Lanes::new(keyed_lanes()).unwrap(),
                Arc::new(SystemClock::default()),
            )
            .unwrap(),
        )
    }

    fn approved(&self, mother: &Coordinator<GovernedPort>, markers: &str) -> MissionId {
        let id = mother.draft(contract_with(markers)).unwrap();
        mother.submit(id).unwrap();
        assert_eq!(consent(mother, id, &Yes), Ok(MissionState::Approved));
        id
    }

    /// A mission whose Builder's GET has started and is held: its worker
    /// waits at the gate in the projection of its own `Started` record (on
    /// its own thread, after the commit, with no broker or Mother lock
    /// held), so that record is durable and the worker never reaches its
    /// transport or its end before child 1 aborts. The gate holds every
    /// `started` writer; this action is the only one child 1 starts, and
    /// the held record is shown to be its worker's.
    fn executing(&self, mother: &Arc<Coordinator<GovernedPort>>) -> MissionId {
        let id = self.approved(mother, &format!("FETCH-BUILDER:{}/hang", self.origin));
        mother.advance(id).unwrap();
        mother.advance(id).unwrap();
        let before = self.home.gate.count("started");
        self.home.gate.arm("started", None);
        let m = mother.clone();
        let worker = std::thread::spawn(move || {
            let _ = m.advance(id);
        });
        assert!(
            self.home.gate.wait_held(),
            "the action's Started record reached the gate"
        );
        let started: Vec<_> = self
            .home
            .gate
            .seen()
            .into_iter()
            .filter(|s| s.phase == "started")
            .collect();
        assert_eq!(started.len(), before + 1, "one action started");
        assert_eq!(
            started.last().map(|s| s.thread),
            Some(worker.thread().id()),
            "the held Started is this action's own"
        );
        id
    }

    /// Child 1's crash point `point`, the break beginning now.
    fn at(&self, point: &'static str) -> At {
        At {
            home: self.home.clone(),
            dir: self.dir.clone(),
            point,
            break_at: self.home.gate.seen().len(),
            base: counted(&self.home),
        }
    }

    /// The mission whose Stop will break the coordinator, drafted before
    /// any crash hook is armed (Architect decision 1).
    fn victim(&self, mother: &Coordinator<GovernedPort>) -> MissionId {
        mother.draft(contract_with("")).unwrap()
    }

    /// Break the coordinator with a Stop of `victim`; `marker` decides
    /// whether its poisoning works.
    fn break_with(&self, mother: &Coordinator<GovernedPort>, victim: MissionId, marker: bool) {
        self.poison.store(!marker, Ordering::SeqCst);
        self.appends.store(true, Ordering::SeqCst);
        let _ = mother.stop(victim);
    }

    /// Arm `point` to crash on the next call that reaches it (the breaking
    /// Stop), once it proved it is at that point: the coordinator is
    /// broken, and the batch it is about to write (before the write) or has
    /// just written (after it) is non-empty. Otherwise child 1 exits with
    /// `VACUOUS` instead of aborting.
    fn arm_proven(&self, mother: &Arc<Coordinator<GovernedPort>>, point: CallPoint, at: At) {
        let (home, dir, weak) = (self.home.clone(), self.dir.clone(), Arc::downgrade(mother));
        let base = home.control.status().halt_records;
        arm_call_point(mother, point, move || {
            if !weak.upgrade().is_some_and(|m| m.broken()) {
                vacuous(&dir, "the coordinator is not broken");
            }
            let now = home.control.status().halt_records;
            let batch = match point {
                CallPoint::BeforeOwedWrite => now.pending > base.pending,
                _ => {
                    now.recorded + now.owed + now.uncertain
                        > base.recorded + base.owed + base.uncertain
                }
            };
            if !batch {
                vacuous(&dir, "the batch at the crash point is empty");
            }
            at.crash(true)
        });
    }

    /// After a breaking Stop whose proven hook never fired: not at the point.
    fn missed(&self) -> ! {
        vacuous(&self.dir, "the breaking call never reached the crash point")
    }
}

/// Child 1's exit code when it is not at its named crash point.
const VACUOUS: i32 = 86;

/// Child 1 is not where it should crash: say why, and exit with `VACUOUS`.
fn vacuous(dir: &Path, why: &str) -> ! {
    let _ = std::fs::write(dir.join("vacuous.log"), why);
    std::process::exit(VACUOUS);
}

/// Child 1: run to `point`, then abort with its receipt; a breaking call
/// that never reached its crash point exits with `VACUOUS` instead.
fn child_one(point: &str, dir: &Path) -> ! {
    let c = Child::open(dir);
    let mother = c.mother();
    match point {
        "cr1" => {
            c.executing(&mother);
            c.approved(&mother, "");
            c.at(CR1).crash(mother.broken());
        }
        "cr2" | "cr3" | "cr5" => {
            let x = c.approved(&mother, &format!("POST-BUILDER:{}/cr-wait", c.origin));
            for _ in 0..3 {
                mother.advance(x).unwrap();
            }
            let victim = c.victim(&mother);
            if point == "cr2" {
                // The crashing call holds the coordinator lock here (after
                // the fence, before the marker): its break is shown by the
                // fence's batch, which only a break makes.
                let at = c.at(CR2);
                let (home, dir, base) = (c.home.clone(), c.dir.clone(), at.base);
                arm_call_point(&mother, CallPoint::FenceToMarker, move || {
                    if counted(&home) == base {
                        vacuous(&dir, "no fence batch at the marker point");
                    }
                    at.crash(true)
                });
                c.break_with(&mother, victim, false);
                c.missed();
            }
            let (point_at, name) = match point {
                "cr3" => (CallPoint::BeforeOwedWrite, CR3),
                _ => (CallPoint::AfterOwedWrite, CR5),
            };
            c.arm_proven(&mother, point_at, c.at(name));
            c.break_with(&mother, victim, true);
            c.missed();
        }
        "cr4" => {
            c.executing(&mother);
            let z = c.approved(&mother, &format!("POST-BUILDER:{}/cr-wait", c.origin));
            for _ in 0..3 {
                mother.advance(z).unwrap();
            }
            let victim = c.victim(&mother);
            let at = c.at(CR4);
            let weak = Arc::downgrade(&mother);
            c.home.gate.arm_with("run_cancelled", None, move || {
                at.crash(weak.upgrade().is_some_and(|m| m.broken()))
            });
            c.break_with(&mother, victim, true);
            c.missed();
        }
        "cr6" => {
            c.approved(&mother, "");
            Arc::try_unwrap(mother)
                .ok()
                .expect("one coordinator")
                .close()
                .unwrap();
            c.appends.store(true, Ordering::SeqCst);
            let journal = FailingFile {
                file: c.home.journal().unwrap(),
                appends: c.appends.clone(),
                poison: c.poison.clone(),
            };
            let opened = Coordinator::open(
                journal,
                Arc::new(port_for(&c.home.control, true)),
                Lanes::new(keyed_lanes()).unwrap(),
                Arc::new(SystemClock::default()),
            );
            assert!(opened.is_err(), "the first SessionOpened fails");
            // No coordinator is open: none broke.
            c.at(CR6).crash(false);
        }
        "cr7" => {
            let x = c.approved(&mother, &format!("FETCH-BUILDER:{}/cr-done", c.origin));
            for _ in 0..3 {
                mother.advance(x).unwrap();
            }
            let victim = c.victim(&mother);
            c.arm_proven(&mother, CallPoint::BeforeOwedWrite, c.at(CR7));
            c.break_with(&mother, victim, true);
            c.missed();
        }
        other => panic!("no crash point {other}"),
    }
}

/// The crash points, as child 1's receipt names them.
const CR1: &str = "cr1.p0_executing";
const CR2: &str = "cr2.p1_fence_to_marker";
const CR3: &str = "cr3.p2_before_owed_write";
const CR4: &str = "cr4.p2k_run_cancelled_projected";
const CR5: &str = "cr5.p3_after_owed_write";
const CR6: &str = "cr6.session_opened_failed";
const CR7: &str = "cr7.before_owed_write";

/// Child 2: reopen the home; answer by script; print the observations.
fn child_two(dir: &Path) {
    let home = Home::open(dir).unwrap();
    let control: &Arc<GovernedControl> = &home.control;
    let mut obs = serde_json::Map::new();
    obs.insert(
        "recovery_required".into(),
        json!(control.authority().recovery_required()),
    );
    obs.insert(
        "emergency_stopped".into(),
        json!(control.status().emergency_stopped),
    );
    obs.insert(
        "door_before_recovery".into(),
        json!(try_port_for(control, true).is_some()),
    );
    let recovers = OwnerRecovers::default();
    if control.status().emergency_stopped {
        obs.insert("recovered".into(), json!(control.resume(&recovers).is_ok()));
    }
    obs.insert(
        "unsettled".into(),
        json!(recovers.unsettled.load(Ordering::SeqCst)),
    );
    let journal = home.journal().unwrap();
    let lines_before = std::fs::read(journal.path()).unwrap_or_default();
    obs.insert(
        "door_no_gives_no_port".into(),
        json!(try_port_for(control, false).is_none()),
    );
    obs.insert(
        "journal_unchanged_on_no".into(),
        json!(std::fs::read(journal.path()).unwrap_or_default() == lines_before),
    );
    let asked = doors_asked(&home);
    let port = try_port_for(control, true);
    obs.insert("door_asked".into(), json!(doors_asked(&home) - asked));
    obs.insert("door_yes".into(), json!(port.is_some()));
    let opened = port.map(|port| {
        Coordinator::open(
            home.journal().unwrap(),
            Arc::new(port),
            Lanes::new(keyed_lanes()).unwrap(),
            Arc::new(SystemClock::default()),
        )
    });
    match opened {
        Some(Ok(mother)) => {
            obs.insert("open".into(), json!("ok"));
            let mut states = serde_json::Map::new();
            let mut held = Vec::new();
            let mut resume_shown = 0;
            for id in mother.missions() {
                let state = mother.state(id).unwrap();
                states.insert(
                    id.to_string(),
                    json!({
                        "state": state.as_str(),
                        "reason": mother.reason(id),
                        "held": mother.held(id),
                    }),
                );
                if mother.held(id).is_some() {
                    held.push(id.to_string());
                    let owner = HeldOwner::new(true);
                    let _ = resume_with(&mother, id, &owner);
                    resume_shown += owner.shown().len();
                }
            }
            obs.insert("missions".into(), Value::Object(states));
            obs.insert("held".into(), json!(held));
            obs.insert("resume_shown".into(), json!(resume_shown));
        }
        Some(Err(CoordinatorError::Journal(JournalError::Poisoned))) => {
            obs.insert("open".into(), json!("poisoned"));
        }
        Some(Err(e)) => {
            obs.insert("open".into(), json!(format!("error: {e:?}")));
        }
        None => {
            obs.insert("open".into(), json!("no door"));
        }
    }
    println!("M2C-OBS {}", Value::Object(obs));
}

/// The child's entry point: nothing unless the parent set the variables.
#[test]
fn m2c_crash_child() {
    let Ok(point) = std::env::var("MOTHER_M2C_CHILD") else {
        return;
    };
    let dir = PathBuf::from(std::env::var("MOTHER_M2C_HOME").unwrap());
    match std::env::var("MOTHER_M2C_PHASE").as_deref() {
        Ok("1") => child_one(&point, &dir),
        _ => child_two(&dir),
    }
}

// ---------------------------------------------------------------- the parent

/// Run one child to its end, within the bound; kill this child handle if it
/// does not end.
fn run_child(point: &str, phase: &str, dir: &Path) -> (std::process::ExitStatus, String) {
    let exe = std::env::current_exe().unwrap();
    let mut child = Command::new(exe)
        .args([
            "--exact",
            "adapter::m2c_crash::m2c_crash_child",
            "--nocapture",
            "--test-threads=1",
        ])
        .env("MOTHER_M2C_CHILD", point)
        .env("MOTHER_M2C_PHASE", phase)
        .env("MOTHER_M2C_HOME", dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut stdout = child.stdout.take().unwrap();
    let output = Arc::new(Mutex::new(String::new()));
    let reader = {
        let output = output.clone();
        std::thread::spawn(move || {
            let mut text = String::new();
            let _ = stdout.read_to_string(&mut text);
            *output.lock().unwrap() = text;
        })
    };
    let deadline = Instant::now() + Duration::from_secs(60);
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("{point}: child {phase} did not end within 60 s");
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    let _ = reader.join();
    let text = output.lock().unwrap().clone();
    (status, text)
}

/// What child 1 had committed at its crash, from where the break began.
struct Seen {
    after_break: Vec<String>,
}

impl Seen {
    fn count(&self, phase: &str) -> usize {
        self.after_break.iter().filter(|p| *p == phase).count()
    }
}

/// Whether child 1 ended by `std::process::abort` (SIGABRT on Unix, exit
/// code 3 on Windows).
fn aborted(status: &ExitStatus) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        status.signal() == Some(6)
    }
    #[cfg(windows)]
    {
        status.code() == Some(3)
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = status;
        false
    }
}

/// A file child 1 must have written: its absence fails the test.
fn required(dir: &Path, name: &str, point: &str) -> String {
    std::fs::read_to_string(dir.join(name))
        .unwrap_or_else(|e| panic!("{point}: child 1 wrote no {name}: {e}"))
}

/// `key=value` on `line`, parsed; anything else fails the test.
fn field<T: std::str::FromStr>(line: Option<&str>, key: &str, point: &str) -> T {
    line.and_then(|l| l.strip_prefix(key))
        .and_then(|l| l.strip_prefix('='))
        .and_then(|v| v.parse().ok())
        .unwrap_or_else(|| panic!("{point}: malformed {key} line: {line:?}"))
}

/// Crash child 1 at `point`, reopen with child 2: child 1's records and
/// child 2's observations. Child 1 must have aborted at the crash point
/// its receipt names, `expected`, with its coordinator `broken` as given,
/// a non-empty batch exactly when it broke, and every record the gate saw
/// written down.
fn crash_case(point: &str, expected: &str, broken: bool) -> (Seen, Value) {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let dir = std::env::temp_dir().join(format!("nexus-mother-crash-{point}-{nanos}"));
    let (status, _) = run_child(point, "1", &dir);
    if status.code() == Some(VACUOUS) {
        let why = std::fs::read_to_string(dir.join("vacuous.log")).unwrap_or_default();
        let _ = std::fs::remove_dir_all(&dir);
        panic!("{point}: child 1 was not at its named crash point: {why}");
    }
    assert!(
        aborted(&status),
        "{point}: child 1 must abort at its crash point, not {status:?}"
    );
    let receipt = required(&dir, "receipt.log", point);
    let mut lines = receipt.lines();
    let named: String = field(lines.next(), "point", point);
    let was_broken: bool = field(lines.next(), "broken", point);
    let batch: u64 = field(lines.next(), "batch", point);
    let gate: usize = field(lines.next(), "gate", point);
    assert_eq!(lines.next(), None, "{point}: malformed receipt");
    assert_eq!(
        named, expected,
        "{point}: the receipt names its crash point"
    );
    assert_eq!(was_broken, broken, "{point}: the receipt's coordinator");
    assert_eq!(batch > 0, broken, "{point}: the receipt's batch ({batch})");
    let log = required(&dir, "seen.log", point);
    let mut lines = log.lines();
    let break_at: usize = field(lines.next(), "break_at", point);
    let records: Vec<String> = lines.map(str::to_string).collect();
    assert_eq!(records.len(), gate, "{point}: every record the gate saw");
    assert!(break_at <= gate, "{point}: break_at {break_at} > {gate}");
    let seen = Seen {
        after_break: records[break_at..].to_vec(),
    };
    let (status, text) = run_child(point, "2", &dir);
    assert!(status.success(), "{point}: child 2 ran to its end:\n{text}");
    let obs = text
        .lines()
        .find_map(|l| l.split_once("M2C-OBS ").map(|(_, obs)| obs))
        .unwrap_or_else(|| panic!("{point}: no observations:\n{text}"));
    let obs: Value = serde_json::from_str(obs).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    // In every row: a door answered no gives no port and no journal change.
    assert_eq!(obs["door_no_gives_no_port"], json!(true), "{point}");
    assert_eq!(obs["journal_unchanged_on_no"], json!(true), "{point}");
    (seen, obs)
}

fn mission(obs: &Value, n: u64) -> &Value {
    &obs["missions"][format!("msn-{n:06}")]
}

/// CR1 (P0, §11 C4 corrected): an action Executing at the crash. Recovery
/// is required; the door issue is refused until the Owner acknowledges and
/// lifts the stop; then the door is asked; the interrupted mission is
/// Blocked (`effect_outcome_unknown`); a separate, unstarted mission comes
/// back held and its resume asks.
#[test]
fn m2c_cr1_an_executing_action_at_the_crash() {
    let (_, obs) = crash_case("cr1", CR1, false);
    assert_eq!(obs["recovery_required"], json!(true));
    assert_eq!(obs["door_before_recovery"], json!(false));
    assert_eq!(obs["recovered"], json!(true));
    assert_eq!(obs["unsettled"], json!(1));
    assert_eq!(obs["door_asked"], json!(1));
    assert_eq!(obs["open"], json!("ok"));
    assert_eq!(mission(&obs, 1)["state"], json!("blocked"));
    assert_eq!(mission(&obs, 1)["reason"], json!("effect_outcome_unknown"));
    assert_eq!(obs["held"], json!(["msn-000002"]));
    assert_eq!(obs["resume_shown"], json!(1));
}

/// CR2 (P1): a crash after the in-memory halt, before the marker: J0 (the
/// journal reopens), S0 (no batch record durable); the door is asked, every
/// unfinished mission is held, and its resume asks.
#[test]
fn m2c_cr2_after_the_halt_before_the_marker() {
    let (seen, obs) = crash_case("cr2", CR2, true);
    assert_eq!(seen.count("door_halted"), 0, "S0");
    assert_eq!(seen.count("run_cancelled"), 0, "S0");
    assert_eq!(obs["door_asked"], json!(1));
    assert_eq!(obs["open"], json!("ok"));
    // The waiting mission, and the victim whose Stop was never journaled.
    assert_eq!(obs["held"], json!(["msn-000001", "msn-000002"]));
    assert_eq!(obs["resume_shown"], json!(2));
}

/// CR3 (P2): after the marker, before the records: J3 (the journal is
/// poisoned: no coordinator after the door's yes), S0.
#[test]
fn m2c_cr3_after_the_marker_before_the_records() {
    let (seen, obs) = crash_case("cr3", CR3, true);
    assert_eq!(seen.count("door_halted"), 0, "S0");
    assert_eq!(obs["door_asked"], json!(1));
    assert_eq!(obs["open"], json!("poisoned"));
}

/// CR4 (P2-k): a projection aborts on the batch's 2nd record (its first
/// run_cancelled), after its commit: S1, exactly those 2 records durable;
/// the started member is unsettled, the never-started one is not.
#[test]
fn m2c_cr4_a_prefix_of_the_batch() {
    let (seen, obs) = crash_case("cr4", CR4, true);
    assert_eq!(seen.count("door_halted"), 1, "S1");
    assert_eq!(seen.count("run_cancelled"), 1, "S1: exactly k = 2 records");
    assert_eq!(seen.count("revoked"), 0, "S1");
    assert_eq!(obs["recovery_required"], json!(true));
    assert_eq!(obs["unsettled"], json!(1));
}

/// CR5 (P3): after the records: J3, S2.
#[test]
fn m2c_cr5_after_the_records() {
    let (seen, obs) = crash_case("cr5", CR5, true);
    assert_eq!(seen.count("door_halted"), 1, "S2");
    assert_eq!(seen.count("run_cancelled"), 1, "S2");
    assert_eq!(seen.count("revoked"), 1, "S2: the waiting action");
    assert_eq!(obs["open"], json!("poisoned"));
    assert_eq!(obs["recovery_required"], json!(false));
}

/// CR6: the first SessionOpened after a clean close fails, then a crash:
/// J0's exception: no mission is held, and the door is still asked.
#[test]
fn m2c_cr6_a_failed_first_session_record_holds_nothing() {
    let (_, obs) = crash_case("cr6", CR6, false);
    assert_eq!(obs["door_asked"], json!(1));
    assert_eq!(obs["open"], json!("ok"));
    assert_eq!(obs["held"], json!([]));
}

/// CR7 (§11 C4): a member action whose Started and Finished are durable,
/// and the halt's records lost (S0): the action is not unsettled.
#[test]
fn m2c_cr7_a_finished_member_is_settled_without_halt_records() {
    let (seen, obs) = crash_case("cr7", CR7, true);
    assert_eq!(seen.count("door_halted"), 0, "S0");
    assert_eq!(obs["recovery_required"], json!(false));
    assert_eq!(obs["unsettled"], json!(0));
}
