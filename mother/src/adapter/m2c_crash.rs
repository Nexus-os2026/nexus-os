//! M2c-B (PR-B) crash-point oracles, pre-registered before the halt exists
//! (design v3 §6, §7.3, §11 C4): MC1–MC4 in-process over the shared broker,
//! and CR1–CR8 across real processes (N4b).
//!
//! The child-process harness: a CR test re-runs this test binary with
//! `--exact adapter::m2c_crash::m2c_crash_child`, which does nothing unless
//! `MOTHER_M2C_CHILD` is set. Child 1 builds its own broker and Mother in a
//! fresh home (one front door per process), runs to the crash point and
//! aborts; just before, it writes down every broker record committed so far
//! (a projection sees a record only after its commit). Child 2 reopens the
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
use std::process::{Command, Stdio};
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
/// holds `/hang`, which it takes and never answers (its action stays
/// Executing). Says when it took a hanging request.
fn world(listener: TcpListener) -> std::sync::mpsc::Receiver<()> {
    let (took, taken) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut kept = Vec::new();
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let mut request = [0u8; 4096];
            let n = stream.read(&mut request).unwrap_or(0);
            let line = String::from_utf8_lossy(&request[..n]).to_string();
            if line.contains("/hang") {
                kept.push(stream);
                let _ = took.send(());
            } else {
                let _ = stream.write_all(
                    b"HTTP/1.1 200 OK\r\nContent-Length: 6\r\nConnection: close\r\n\r\nmother",
                );
            }
        }
    });
    taken
}

/// Write down every broker record committed so far (with where the break
/// began), then abort the process.
fn crash_now(home: &Home, dir: &Path, break_at: usize) -> ! {
    let seen = home.gate.seen();
    let mut text = format!("break_at={break_at}\n");
    for record in &seen {
        text.push_str(&record.phase);
        text.push('\n');
    }
    let mut file = std::fs::File::create(dir.join("seen.log")).unwrap();
    file.write_all(text.as_bytes()).unwrap();
    file.sync_all().unwrap();
    std::process::abort();
}

struct Child {
    home: Arc<Home>,
    dir: PathBuf,
    origin: String,
    taken: std::sync::mpsc::Receiver<()>,
    appends: Arc<AtomicBool>,
    poison: Arc<AtomicBool>,
}

impl Child {
    fn open(dir: &Path) -> Self {
        let home = Arc::new(Home::open(dir).unwrap());
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let taken = world(listener);
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
            taken,
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

    /// A mission whose Builder's GET is taken by the world and never
    /// answered: it stays Executing.
    fn executing(&self, mother: &Arc<Coordinator<GovernedPort>>) -> MissionId {
        let id = self.approved(mother, &format!("FETCH-BUILDER:{}/hang", self.origin));
        mother.advance(id).unwrap();
        mother.advance(id).unwrap();
        let m = mother.clone();
        std::thread::spawn(move || {
            let _ = m.advance(id);
        });
        self.taken
            .recv_timeout(Duration::from_secs(10))
            .expect("the world took the hanging request");
        id
    }

    fn crash(&self, break_at: usize) -> ! {
        crash_now(&self.home, &self.dir, break_at)
    }

    /// Break the coordinator; `marker` decides whether its poisoning works.
    fn break_it(&self, mother: &Coordinator<GovernedPort>, marker: bool) -> usize {
        let victim = mother.draft(contract_with("")).unwrap();
        let at = self.home.gate.seen().len();
        self.poison.store(!marker, Ordering::SeqCst);
        self.appends.store(true, Ordering::SeqCst);
        let _ = mother.stop(victim);
        at
    }
}

/// Child 1: run to `point`, then abort.
fn child_one(point: &str, dir: &Path) -> ! {
    let c = Child::open(dir);
    let mother = c.mother();
    match point {
        "cr1" => {
            c.executing(&mother);
            c.approved(&mother, "");
            c.crash(0);
        }
        "cr2" | "cr3" | "cr5" => {
            let x = c.approved(&mother, &format!("POST-BUILDER:{}/cr-wait", c.origin));
            for _ in 0..3 {
                mother.advance(x).unwrap();
            }
            let at = c.home.gate.seen().len();
            let (point_at, marker) = match point {
                "cr2" => (CallPoint::FenceToMarker, false),
                "cr3" => (CallPoint::BeforeOwedWrite, true),
                _ => (CallPoint::AfterOwedWrite, true),
            };
            let (home, dir) = (c.home.clone(), c.dir.clone());
            arm_call_point(&mother, point_at, move || crash_now(&home, &dir, at));
            c.break_it(&mother, marker);
            c.crash(at);
        }
        "cr4" => {
            c.executing(&mother);
            let z = c.approved(&mother, &format!("POST-BUILDER:{}/cr-wait", c.origin));
            for _ in 0..3 {
                mother.advance(z).unwrap();
            }
            let at = c.home.gate.seen().len();
            let (home, dir) = (c.home.clone(), c.dir.clone());
            c.home
                .gate
                .arm_with("run_cancelled", None, move || crash_now(&home, &dir, at));
            c.break_it(&mother, true);
            c.crash(at);
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
            c.crash(0);
        }
        "cr7" => {
            let x = c.approved(&mother, &format!("FETCH-BUILDER:{}/cr-done", c.origin));
            for _ in 0..3 {
                mother.advance(x).unwrap();
            }
            let at = c.home.gate.seen().len();
            let (home, dir) = (c.home.clone(), c.dir.clone());
            arm_call_point(&mother, CallPoint::BeforeOwedWrite, move || {
                crash_now(&home, &dir, at)
            });
            c.break_it(&mother, true);
            c.crash(at);
        }
        "cr8" => {
            c.executing(&mother);
            let at = c.home.gate.seen().len();
            let (home, dir) = (c.home.clone(), c.dir.clone());
            arm_call_point(&mother, CallPoint::AfterOwedWrite, move || {
                crash_now(&home, &dir, at)
            });
            c.break_it(&mother, true);
            c.crash(at);
        }
        other => panic!("no crash point {other}"),
    }
}

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
fn run_child(point: &str, phase: &str, dir: &Path) -> (bool, String) {
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
    (status.success(), text)
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

/// Crash child 1 at `point`, reopen with child 2: child 1's records and
/// child 2's observations.
fn crash_case(point: &str) -> (Seen, Value) {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let dir = std::env::temp_dir().join(format!("nexus-mother-crash-{point}-{nanos}"));
    let (clean, _) = run_child(point, "1", &dir);
    assert!(!clean, "{point}: child 1 must crash");
    let log = std::fs::read_to_string(dir.join("seen.log")).unwrap_or_default();
    let mut lines = log.lines();
    let break_at: usize = lines
        .next()
        .and_then(|l| l.strip_prefix("break_at="))
        .and_then(|n| n.parse().ok())
        .unwrap_or(0);
    let seen = Seen {
        after_break: lines.skip(break_at).map(str::to_string).collect(),
    };
    let (clean, text) = run_child(point, "2", &dir);
    assert!(clean, "{point}: child 2 ran to its end:\n{text}");
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

fn mission<'a>(obs: &'a Value, n: u64) -> &'a Value {
    &obs["missions"][format!("msn-{n:06}")]
}

/// CR1 (P0, §11 C4 corrected): an action Executing at the crash. Recovery
/// is required; the door issue is refused until the Owner acknowledges and
/// lifts the stop; then the door is asked; the interrupted mission is
/// Blocked (`effect_outcome_unknown`); a separate, unstarted mission comes
/// back held and its resume asks.
#[test]
fn m2c_cr1_an_executing_action_at_the_crash() {
    let (_, obs) = crash_case("cr1");
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
    let (seen, obs) = crash_case("cr2");
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
    let (seen, obs) = crash_case("cr3");
    assert_eq!(seen.count("door_halted"), 0, "S0");
    assert_eq!(obs["door_asked"], json!(1));
    assert_eq!(obs["open"], json!("poisoned"));
}

/// CR4 (P2-k): a projection aborts on the batch's 2nd record (its first
/// run_cancelled), after its commit: S1, exactly those 2 records durable;
/// the started member is unsettled, the never-started one is not.
#[test]
fn m2c_cr4_a_prefix_of_the_batch() {
    let (seen, obs) = crash_case("cr4");
    assert_eq!(seen.count("door_halted"), 1, "S1");
    assert_eq!(seen.count("run_cancelled"), 1, "S1: exactly k = 2 records");
    assert_eq!(seen.count("revoked"), 0, "S1");
    assert_eq!(obs["recovery_required"], json!(true));
    assert_eq!(obs["unsettled"], json!(1));
}

/// CR5 (P3): after the records: J3, S2.
#[test]
fn m2c_cr5_after_the_records() {
    let (seen, obs) = crash_case("cr5");
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
    let (_, obs) = crash_case("cr6");
    assert_eq!(obs["door_asked"], json!(1));
    assert_eq!(obs["open"], json!("ok"));
    assert_eq!(obs["held"], json!([]));
}

/// CR7 (§11 C4): a member action whose Started and Finished are durable,
/// and the halt's records lost (S0): the action is not unsettled.
#[test]
fn m2c_cr7_a_finished_member_is_settled_without_halt_records() {
    let (seen, obs) = crash_case("cr7");
    assert_eq!(seen.count("door_halted"), 0, "S0");
    assert_eq!(obs["recovery_required"], json!(false));
    assert_eq!(obs["unsettled"], json!(0));
}

/// CR8 (§11 C4): a member action Executing, the whole batch durable (S2),
/// then a crash before the action's end: it is unsettled, and recovery is
/// required. (Plant (j): recovery that treats a durable halt record as
/// settling a member.)
#[test]
fn m2c_cr8_a_durable_halt_does_not_settle_an_executing_member() {
    let (seen, obs) = crash_case("cr8");
    assert_eq!(seen.count("door_halted"), 1, "S2");
    assert_eq!(obs["recovery_required"], json!(true));
    assert_eq!(obs["unsettled"], json!(1));
    assert_eq!(obs["door_before_recovery"], json!(false));
}
