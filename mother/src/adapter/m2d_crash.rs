//! M2d-C round 5, decision D8 (pre-registered before it runs): oracle 5 of
//! the settlement design with real crash children, the `m2c_crash` pattern
//! (that file is unchanged). Child 1 builds its own broker and Mother in a
//! fresh home with a Files workspace, publishes a File the mission waits
//! on, then the Owner's Stop, and dies by `abort` (never by unwinding) at
//! one settlement boundary:
//!
//! - `started`: once `SettlementStarted` is durable;
//! - `before_write`, `torn`, `after_sync`: inside the `Settled` append,
//!   before the line is written, with half a line written, and once it is
//!   written and synced;
//! - `fence`: the `SettlementStarted` append fails and the fence has run;
//! - `poison`: the same failure, inside the poisoning;
//! - `before_ack`: after the terminal step, before its acknowledgement.
//!
//! Child 2 reopens the home in a new process: a fresh broker (the kernel's
//! recovery report read through it) and Mother on the same journal. A
//! commitment the new broker does not know is uncertain, never "not
//! published"; only a conclusive outcome is acknowledged again; a torn
//! terminal line is refused; a held mission's Stop goes through the
//! settlement. The parent waits for each child within 60 s, polling its own
//! child handle and killing that handle on timeout. No new dependency.

use super::m2c_support::{consent, port_for, Home};
use super::{create_private, mother_agent, FileJournal, GovernedPort};
use crate::coordinator::{Coordinator, Lanes};
use crate::effect::EffectPort;
use crate::journal::{Event, JournalError, JournalStore, MissionId};
use crate::oracles::m2d_c::{files_lanes, FilesOwner};
use crate::oracles::{contract_with, Yes};
use crate::state::{MissionState, Trigger};
use nexus_governed_control::authority::clock::SystemClock;
use nexus_governed_control::authority::ids::{CommitmentId, RunId};
use nexus_governed_control::governed::GrantRequest;
use serde_json::{json, Value};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const POINTS: [&str; 7] = [
    "started",
    "before_write",
    "torn",
    "after_sync",
    "fence",
    "poison",
    "before_ack",
];

/// The journal child 1 writes: it dies inside the `Settled` append, or
/// fails the `SettlementStarted` append, as its point says.
struct Dying {
    file: FileJournal,
    path: PathBuf,
    point: String,
}

impl JournalStore for Dying {
    fn load(&self) -> Result<Vec<String>, JournalError> {
        self.file.load()
    }
    fn append(&mut self, line: &str) -> Result<(), JournalError> {
        let settled = line.contains("\"trigger\":\"settled\"");
        let started = line.contains("\"event\":\"settlement_started\"");
        match self.point.as_str() {
            "before_write" if settled => std::process::abort(),
            "torn" if settled => {
                let mut file = std::fs::OpenOptions::new()
                    .append(true)
                    .open(&self.path)
                    .unwrap();
                file.write_all(&line.as_bytes()[..line.len() / 2]).unwrap();
                file.sync_all().unwrap();
                std::process::abort();
            }
            "after_sync" if settled => {
                self.file.append(line).unwrap();
                std::process::abort();
            }
            "fence" | "poison" if started => {
                return Err(JournalError::Store(crate::journal::StoreFailure::Injected))
            }
            _ => {}
        }
        self.file.append(line)
    }
    fn poison(&mut self) -> Result<(), JournalError> {
        if self.point == "poison" {
            std::process::abort();
        }
        self.file.poison()
    }
    fn identity(&self) -> Option<String> {
        self.file.identity()
    }
}

fn workspace_path(dir: &Path) -> PathBuf {
    PathBuf::from(std::fs::read_to_string(dir.join("workspace.path")).unwrap())
}

fn mother(
    home: &Home,
    port: &Arc<GovernedPort>,
    journal: impl JournalStore + 'static,
) -> Result<Coordinator<GovernedPort>, crate::coordinator::CoordinatorError> {
    Coordinator::open(
        journal,
        port.clone(),
        Lanes::new(files_lanes()).unwrap(),
        Arc::new(SystemClock::default()),
        home.private(),
    )
}

/// Child 1: a published File, the Owner's Stop, death at `point`.
fn child_one(point: &str, dir: &Path) -> ! {
    let home = Home::open(dir).unwrap();
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let ws = std::env::temp_dir().join(format!("m2dc5-crash-ws-{point}-{nanos}"));
    create_private(&ws).unwrap();
    let ws = std::fs::canonicalize(&ws).unwrap();
    std::fs::write(dir.join("workspace.path"), ws.to_string_lossy().as_bytes()).unwrap();
    let info = home
        .control
        .register_workspace(&ws, &[home.dir.as_path()])
        .unwrap();
    let owner = FilesOwner::default();
    home.control
        .request_grant(
            &GrantRequest::Files {
                workspace: info.id.clone(),
            },
            Duration::from_secs(24 * 3600),
            &owner,
        )
        .unwrap();
    let port = Arc::new(port_for(&home.control, true).with_workspace(info.name.clone()));
    let journal = Dying {
        file: home.journal().unwrap(),
        path: home.journal_path(),
        point: point.to_string(),
    };
    let mother = mother(&home, &port, journal).unwrap();
    let mut contract = contract_with("MKFILE:crash-a.txt MKFILE:crash-b.txt VFILE:crash-c.txt");
    contract.requested_folder = Some(info.name.clone());
    let id = mother.draft(contract).unwrap();
    mother.submit(id).unwrap();
    assert_eq!(consent(&mother, id, &Yes), Ok(MissionState::Approved));
    for _ in 0..3 {
        mother.advance(id).unwrap();
    }
    let pending = port.pending(id).unwrap();
    let (run, commitment) = (
        RunId::parse(&pending.run).unwrap(),
        CommitmentId::parse(&pending.commitment).unwrap(),
    );
    home.control
        .authorize(commitment, &mother_agent(), run, &owner)
        .unwrap();
    home.control
        .execute(commitment, &mother_agent(), run)
        .expect("published");
    std::fs::write(dir.join("mission.id"), id.0.to_string()).unwrap();
    match point {
        // The Stop's first step journals the start; its owed write follows.
        "started" => mother
            .hooks
            .before_owed_write
            .arm(0, || std::process::abort()),
        // The fence ran, before the poison marker.
        "fence" => mother
            .hooks
            .fence_to_marker
            .arm(0, || std::process::abort()),
        // The terminal step's owed write, before the acknowledgement.
        "before_ack" => mother
            .hooks
            .before_owed_write
            .arm(1, || std::process::abort()),
        _ => {}
    }
    let _ = mother.stop(id);
    // Every point dies before this.
    println!("M2D-CRASH missed {point}");
    std::process::exit(7);
}

/// Child 2: a fresh broker on the same home, Mother reopened; one JSON
/// line of what it finds.
fn child_two(dir: &Path) {
    let home = Home::open(dir).unwrap();
    let id = MissionId(
        std::fs::read_to_string(dir.join("mission.id"))
            .unwrap()
            .parse()
            .unwrap(),
    );
    // The Owner's workspace registered again with the fresh broker (the
    // kernel's recovery reads it).
    let registered = home
        .control
        .register_workspace(&workspace_path(dir), &[home.dir.as_path()])
        .map_or_else(|e| format!("err:{}", e.class()), |_| "ok".to_string());
    let recovery = home.control.files_recovery_report().map_or_else(
        |e| format!("err:{}", e.class()),
        |r| format!("ok:{}", r.len()),
    );
    let port = Arc::new(port_for(&home.control, true));
    let mut obs = serde_json::Map::new();
    obs.insert("registered".into(), json!(registered));
    obs.insert("recovery".into(), json!(recovery));
    match mother(&home, &port, home.journal().unwrap()) {
        Err(e) => {
            obs.insert("open".into(), json!(format!("{e:?}")));
        }
        Ok(mother) => {
            obs.insert("open".into(), json!("ok"));
            obs.insert("held".into(), json!(mother.held(id).is_some()));
            if mother.held(id).is_some() {
                // The held mission's Stop goes through its settlement.
                let _ = mother.stop(id);
            }
            obs.insert(
                "state".into(),
                json!(mother.state(id).map(|s| s.as_str().to_string())),
            );
            obs.insert("reason".into(), json!(mother.reason(id)));
            let outcomes: Vec<String> = mother
                .journal()
                .unwrap()
                .into_iter()
                .filter(|e| e.mission == id)
                .filter_map(|e| match e.event {
                    Event::Transition {
                        trigger: Trigger::Settled { outcome, .. },
                    } => Some(format!("{outcome:?}")),
                    _ => None,
                })
                .collect();
            obs.insert("outcomes".into(), json!(outcomes));
            obs.insert("acks".into(), json!(port.acks_tried()));
        }
    }
    println!("M2D-OBS {}", Value::Object(obs));
}

/// The children's entry point: nothing unless the parent set the variables.
#[test]
fn m2d_crash_child() {
    let Ok(point) = std::env::var("MOTHER_M2D_CHILD") else {
        return;
    };
    let dir = PathBuf::from(std::env::var("MOTHER_M2D_HOME").unwrap());
    match std::env::var("MOTHER_M2D_PHASE").as_deref() {
        Ok("1") => child_one(&point, &dir),
        _ => child_two(&dir),
    }
}

/// Run one child to its end within 60 s; kill this child handle otherwise.
fn run_child(point: &str, phase: &str, dir: &Path) -> (ExitStatus, String) {
    let exe = std::env::current_exe().unwrap();
    let mut child = Command::new(exe)
        .args([
            "--exact",
            "adapter::m2d_crash::m2d_crash_child",
            "--nocapture",
            "--test-threads=1",
        ])
        .env("MOTHER_M2D_CHILD", point)
        .env("MOTHER_M2D_PHASE", phase)
        .env("MOTHER_M2D_HOME", dir)
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

/// Whether the child ended by `abort` (SIGABRT on Unix, 3 on Windows).
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

fn crash(point: &str) -> Value {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let dir = std::env::temp_dir().join(format!("nexus-mother-m2d-crash-{point}-{nanos}"));
    let (status, out) = run_child(point, "1", &dir);
    assert!(
        aborted(&status),
        "{point}: child 1 must abort: {status:?} {out}"
    );
    let (status, out) = run_child(point, "2", &dir);
    assert!(status.success(), "{point}: child 2: {status:?}");
    let line = out
        .lines()
        .find_map(|l| l.strip_prefix("M2D-OBS "))
        .unwrap_or_else(|| panic!("{point}: no observation: {out}"));
    let _ = std::fs::remove_dir_all(&dir);
    serde_json::from_str(line).unwrap()
}

/// D8: every settlement boundary, a real crash, a fresh broker.
#[cfg(target_os = "linux")]
#[test]
fn m2d_c_r5_d8_a_crash_at_every_settlement_boundary() {
    for point in POINTS {
        let obs = crash(point);
        let text = obs.to_string();
        if point == "torn" {
            assert!(
                obs["open"].as_str().is_some_and(|o| o != "ok"),
                "torn: refused: {text}"
            );
            continue;
        }
        assert_eq!(obs["open"], "ok", "{point}: {text}");
        assert!(obs["recovery"].as_str().is_some(), "{point}: {text}");
        let outcomes: Vec<String> = obs["outcomes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|o| o.as_str().unwrap().to_string())
            .collect();
        assert_eq!(
            obs["state"], "blocked",
            "{point}: never an ordinary Stop: {text}"
        );
        match point {
            "after_sync" | "before_ack" => {
                assert!(
                    outcomes.len() == 1 && outcomes[0].starts_with("Published"),
                    "{point}: {text}"
                );
                assert_eq!(
                    obs["reason"], "file_publication_interrupted:owner_stop",
                    "{point}: {text}"
                );
                assert_eq!(obs["acks"], 1, "{point}: acknowledged again: {text}");
            }
            _ => {
                assert_eq!(outcomes, vec!["Uncertain".to_string()], "{point}: {text}");
                assert_eq!(
                    obs["reason"], "file_publication_uncertain",
                    "{point}: never not-published: {text}"
                );
                assert_eq!(obs["acks"], 0, "{point}: never acknowledged: {text}");
            }
        }
    }
}
