//! Lite D8 registration: adapted from #78; adds before-sync and lost in-flight
//! broker state. Recovery success and exact findings are mandatory even for torn journals.
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
//! - `before_ack`: after the terminal step, before its acknowledgement;
//! - round 6 (D8 completed): `fence_terminal` and `poison_terminal`: the
//!   `Settled` append fails after a durable `SettlementStarted`; the fence
//!   child dies inside the fence (at its first cancellation, before the
//!   fence returns), the poison child inside the poisoning.
//!
//! Every child writes a receipt naming the boundary it dies at (and that
//! `SettlementStarted` was durable, where it was), which the parent reads
//! back exactly. Child 2 reopens the home in a new process: a fresh broker,
//! the kernel's recovery read through the product's own report path
//! (`GovernedControl::files_recovery_report`, which reopens the Files
//! ledger and runs the kernel's analysis with no workspace registration:
//! its findings are exactly none, `ok:0`, because the one publication
//! finished before every crash point) and Mother on the same journal. A
//! commitment the new broker does not know is uncertain, never "not
//! published"; only a conclusive outcome is acknowledged again; a torn
//! terminal line is refused; a held mission's Stop goes through the
//! settlement. The parent waits for each child within 60 s, polling its own
//! child handle and killing that handle on timeout. No new dependency.

use super::lite_clock::{past_the_start, LiteHome as Home};
use super::m2c_support::port_for;
use super::{create_private, mother_agent, FileJournal, GovernedPort};
use crate::coordinator::{Coordinator, FileSession, Lanes};
use crate::effect::EffectPort;
use crate::journal::{Event, JournalError, JournalStore, MissionId};
use crate::oracles::lite_support::{files_lanes, FilesOwner};
use crate::oracles::m2c_confirmers::answering;
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

const POINTS: [&str; 11] = [
    "started",
    "before_write",
    "before_sync",
    "inflight",
    "torn",
    "after_sync",
    "fence",
    "poison",
    "before_ack",
    "fence_terminal",
    "poison_terminal",
];

/// Round 6: the child's receipt of the boundary it dies at, appended and
/// synced before it dies (read back exactly by the parent).
fn receipt(dir: &Path, line: &str) {
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join("receipt"))
        .unwrap();
    file.write_all(format!("{line}\n").as_bytes()).unwrap();
    file.sync_all().unwrap();
}

/// The journal child 1 writes: it dies inside the `Settled` append, or
/// fails the `SettlementStarted` append, as its point says.
struct Dying {
    file: FileJournal,
    path: PathBuf,
    point: String,
    /// The home: where the receipt goes.
    dir: PathBuf,
}

impl JournalStore for Dying {
    fn load(&self) -> Result<Vec<String>, JournalError> {
        self.file.load()
    }
    fn append(&mut self, line: &str) -> Result<(), JournalError> {
        let settled = line.contains("\"trigger\":\"settled\"");
        let started = line.contains("\"event\":\"settlement_started\"");
        match self.point.as_str() {
            "before_sync" if settled => {
                let mut file = std::fs::OpenOptions::new()
                    .append(true)
                    .open(&self.path)
                    .unwrap();
                file.write_all(format!("{line}\n").as_bytes()).unwrap();
                receipt(&self.dir, "terminal_append:before_sync");
                std::process::abort();
            }
            "before_write" if settled => {
                receipt(&self.dir, "terminal_append:before_write");
                std::process::abort()
            }
            "torn" if settled => {
                let mut file = std::fs::OpenOptions::new()
                    .append(true)
                    .open(&self.path)
                    .unwrap();
                file.write_all(&line.as_bytes()[..line.len() / 2]).unwrap();
                file.sync_all().unwrap();
                receipt(&self.dir, "terminal_append:torn");
                std::process::abort();
            }
            "after_sync" if settled => {
                self.file.append(line).unwrap();
                receipt(&self.dir, "terminal_append:after_sync");
                std::process::abort();
            }
            "fence" | "poison" if started => {
                receipt(&self.dir, "started_append:refused");
                return Err(JournalError::Store(crate::journal::StoreFailure::Injected));
            }
            // Round 6: the terminal append fails after the durable start.
            "fence_terminal" | "poison_terminal" if settled => {
                receipt(&self.dir, "terminal_append:refused");
                return Err(JournalError::Store(crate::journal::StoreFailure::Injected));
            }
            _ => {}
        }
        self.file.append(line)?;
        if started {
            receipt(&self.dir, "started_durable");
        }
        Ok(())
    }
    fn poison(&mut self) -> Result<(), JournalError> {
        if self.point == "poison" || self.point == "poison_terminal" {
            receipt(&self.dir, "inside_poison");
            std::process::abort();
        }
        self.file.poison()
    }
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
        dir: dir.to_path_buf(),
    };
    let mother: FileSession<_> = mother(&home, &port, journal)
        .unwrap()
        .into_file_session()
        .unwrap();
    let mut contract = contract_with("MKFILE:crash-a.txt MKFILE:crash-b.txt VFILE:crash-c.txt");
    contract.requested_folder = Some(info.name.clone());
    let id = mother.draft(contract).unwrap();
    mother.submit(id).unwrap();
    assert_eq!(
        answering(&Yes, || mother.request_consent(id)),
        Ok(MissionState::Approved)
    );
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
    std::fs::write(dir.join("mission.id"), id.0.to_string()).unwrap();
    if point == "inflight" {
        let home_dir = dir.to_path_buf();
        home.clock.arm(past_the_start(&home, run), move || {
            receipt(&home_dir, "inflight:apply_prepared");
            std::process::abort();
        });
    }
    home.control
        .execute(commitment, &mother_agent(), run)
        .expect("published");
    std::fs::write(dir.join("mission.id"), id.0.to_string()).unwrap();
    let home_dir = dir.to_path_buf();
    match point {
        // The Stop's first step journals the start; its owed write follows.
        "started" => mother.hooks_for_test().before_owed_write.arm(0, move || {
            receipt(&home_dir, "after_first_step");
            std::process::abort()
        }),
        // The fence ran, before the poison marker.
        "fence" => mother.hooks_for_test().fence_to_marker.arm(0, move || {
            receipt(&home_dir, "after_fence_before_marker");
            std::process::abort()
        }),
        // Round 6: inside the fence, at its first cancellation, before it
        // returns (under the port's mutex and the coordinator lock).
        "fence_terminal" => port.first_fence_cancel.arm(move |fenced| {
            receipt(&home_dir, &format!("inside_fence:fenced={fenced}"));
            std::process::abort()
        }),
        // The terminal step's owed write, before the acknowledgement.
        "before_ack" => mother.hooks_for_test().file_before_ack.arm(0, move || {
            receipt(&home_dir, "after_terminal_before_ack");
            std::process::abort()
        }),
        _ => {}
    }
    let _ = mother.stop(id);
    // Every point dies before this.
    println!("LITE-CRASH missed {point}");
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
    // Round 6 (D8 completed): the product's own recovery path for an
    // already-registered, non-empty folder: the fresh broker reopens the
    // Files ledger and runs the kernel's analysis; no workspace is
    // registered again (registration refuses a non-empty folder, and the
    // report needs none).
    let report = home.control.files_recovery_report();
    let findings: Vec<String> = report
        .as_ref()
        .map(|entries| {
            entries
                .iter()
                .flat_map(|e| e.findings.iter())
                .map(|f| match f {
                    nexus_governed_control::files::RecoveryFinding::PreparedWithoutOutcome {
                        files,
                    } => format!("prepared:{}", files.len()),
                    other => format!("unexpected:{other:?}"),
                })
                .collect()
        })
        .unwrap_or_default();
    let recovery = report.map_or_else(
        |e| format!("err:{}", e.class()),
        |r| format!("ok:{}", r.len()),
    );
    let receipt = std::fs::read_to_string(dir.join("receipt")).unwrap_or_default();
    let inflight = receipt
        .lines()
        .any(|line| line == "inflight:apply_prepared");
    assert_eq!(
        home.control.authority().recovery_required(),
        inflight,
        "only the killed executing action requires native acknowledgement"
    );
    if inflight {
        assert!(
            super::m2c_support::try_port_for(&home.control, true).is_none(),
            "a new door must refuse before recovery acknowledgement"
        );
        let owner = crate::oracles::m2c_owner::OwnerRecovers::default();
        home.control
            .resume(&owner)
            .expect("the Owner acknowledges the lost action");
        assert_eq!(owner.unsettled.load(std::sync::atomic::Ordering::SeqCst), 1);
    }
    let port = Arc::new(port_for(&home.control, true));
    let mut obs = serde_json::Map::new();
    obs.insert("recovery".into(), json!(recovery));
    obs.insert("findings".into(), json!(findings));
    obs.insert(
        "receipt".into(),
        json!(receipt.lines().map(str::to_string).collect::<Vec<_>>()),
    );
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
    println!("LITE-OBS {}", Value::Object(obs));
}

/// The children's entry point: nothing unless the parent set the variables.
#[test]
fn lite_crash_child() {
    let Ok(point) = std::env::var("MOTHER_LITE_CHILD") else {
        return;
    };
    let dir = PathBuf::from(std::env::var("MOTHER_LITE_HOME").unwrap());
    match std::env::var("MOTHER_LITE_PHASE").as_deref() {
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
            "adapter::lite_crash::lite_crash_child",
            "--nocapture",
            "--test-threads=1",
        ])
        .env("MOTHER_LITE_CHILD", point)
        .env("MOTHER_LITE_PHASE", phase)
        .env("MOTHER_LITE_HOME", dir)
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
        std::thread::yield_now();
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
        .find_map(|l| l.find("LITE-OBS ").map(|at| &l[at + "LITE-OBS ".len()..]))
        .unwrap_or_else(|| panic!("{point}: no observation: {out}"));
    let _ = std::fs::remove_dir_all(&dir);
    serde_json::from_str(line).unwrap()
}

/// The receipt each child must leave: the boundary it died at, exactly,
/// after a durable `SettlementStarted` where the point is past it.
fn receipt_of(point: &str) -> Value {
    let lines: Vec<&str> = match point {
        "started" => vec!["started_durable", "after_first_step"],
        "before_write" => vec!["started_durable", "terminal_append:before_write"],
        "before_sync" => vec!["started_durable", "terminal_append:before_sync"],
        "inflight" => vec!["inflight:apply_prepared"],
        "torn" => vec!["started_durable", "terminal_append:torn"],
        "after_sync" => vec!["started_durable", "terminal_append:after_sync"],
        "fence" => vec!["started_append:refused", "after_fence_before_marker"],
        "poison" => vec!["started_append:refused", "inside_poison"],
        "before_ack" => vec!["started_durable", "after_terminal_before_ack"],
        "fence_terminal" => vec![
            "started_durable",
            "terminal_append:refused",
            "inside_fence:fenced=true",
        ],
        "poison_terminal" => vec![
            "started_durable",
            "terminal_append:refused",
            "inside_poison",
        ],
        other => panic!("no receipt for {other}"),
    };
    json!(lines)
}

/// D8: every settlement boundary, a real crash, a fresh broker.
#[cfg(target_os = "linux")]
#[test]
fn lite_d8_crash_at_every_settlement_boundary() {
    for point in POINTS {
        let obs = crash(point);
        let text = obs.to_string();
        assert_eq!(
            obs["recovery"],
            if point == "inflight" { "ok:1" } else { "ok:0" },
            "{point}: production recovery must succeed, including torn journals: {text}"
        );
        assert_eq!(
            obs["findings"],
            if point == "inflight" {
                json!(["prepared:1"])
            } else {
                json!([])
            },
            "{point}: exact recovery findings: {text}"
        );
        if point == "torn" {
            assert!(
                obs["open"].as_str().is_some_and(|o| o != "ok"),
                "torn: refused: {text}"
            );
            assert_eq!(obs["receipt"], receipt_of(point), "torn: the boundary");
            continue;
        }
        assert_eq!(obs["open"], "ok", "{point}: {text}");
        // The real kernel recovery, through the product's report path:
        // exactly no finding (the one publication finished before every
        // crash point), never an error and never any string.

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
        assert_eq!(obs["receipt"], receipt_of(point), "{point}: the boundary");
        match point {
            "before_sync" | "after_sync" | "before_ack" => {
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
