//! F09: entered-lane Stop plus readable-prefix append/poison failure.
use super::*;
use crate::adapter::FileJournal;
use crate::lane::{Lane, LaneIdentity, LaneOutput};
use crate::oracles::lite_support::files_lanes;
use crate::oracles::m2c_confirmers::answering;
use crate::oracles::{contract_with, Yes};
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::mpsc::{sync_channel, Receiver, SyncSender};
use std::time::Duration;

const TEST: &str =
    "coordinator::lite::session_result_fix1::fix1_terminal_record_is_not_a_readable_prefix";
const CASE: &str = "M2DD_FIX1_TERMINAL_RECORD";
struct FaultJournal {
    journal: FileJournal,
    mode: Arc<AtomicU8>,
}
impl JournalStore for FaultJournal {
    fn load(&self) -> Result<Vec<String>, JournalError> {
        self.journal.load()
    }
    fn append(&mut self, line: &str) -> Result<(), JournalError> {
        match self.mode.load(Ordering::SeqCst) {
            0 => self.journal.append(line),
            2 => {
                self.journal.append(line)?;
                Err(JournalError::Store(StoreFailure::Injected))
            }
            _ => Err(JournalError::Store(StoreFailure::Injected)),
        }
    }
    fn poison(&mut self) -> Result<(), JournalError> {
        if self.mode.load(Ordering::SeqCst) == 0 {
            self.journal.poison()
        } else {
            Err(JournalError::Store(StoreFailure::Injected))
        }
    }
}
struct HeldResearch {
    entered: SyncSender<()>,
    release: Mutex<Receiver<()>>,
}
impl Lane for HeldResearch {
    fn identity(&self) -> LaneIdentity {
        LaneIdentity {
            role: LaneRole::ResearcherA,
            name: "fix1-held-research".into(),
            version: "1".into(),
        }
    }
    fn invoke(&self, _: &LaneInput) -> LaneOutput {
        self.entered.send(()).unwrap();
        self.release
            .lock()
            .unwrap()
            .recv_timeout(Duration::from_secs(5))
            .unwrap();
        LaneOutput {
            verification: None,
            text: "bounded entered lane".into(),
            effects: vec![],
            verdict: Verdict::Continue,
        }
    }
}
#[test]
fn fix1_terminal_record_is_not_a_readable_prefix() {
    let Ok(case) = std::env::var(CASE) else {
        for case in ["prefix", "readable-unsynced", "control", "later-failure"] {
            let output = child(case);
            assert!(
                output.status.success(),
                "{case}: {}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            assert!(
                String::from_utf8_lossy(&output.stdout)
                    .contains(&format!("FIX1 F09 VERIFIED {case}")),
                "child must execute exact registered case"
            );
        }
        return;
    };
    assert!(nexus_governed_control::files::make_non_dumpable());
    let home = crate::adapter::session_support::Fixture::new("fix1-terminal");
    let mode = Arc::new(AtomicU8::new(0));
    let (entered_tx, entered_rx) = sync_channel(1);
    let (release_tx, release_rx) = sync_channel(1);
    let mut lanes = files_lanes();
    lanes.retain(|lane| lane.identity().role != LaneRole::ResearcherA);
    lanes.push(Arc::new(HeldResearch {
        entered: entered_tx,
        release: Mutex::new(release_rx),
    }));
    let m = Arc::new(
        Coordinator::open(
            FaultJournal {
                journal: home.journal(),
                mode: mode.clone(),
            },
            home.port.clone(),
            Lanes::new(lanes).unwrap(),
            home.clock.clone(),
            home.private(),
        )
        .unwrap()
        .into_file_session()
        .unwrap(),
    );
    let mut contract = contract_with("bounded terminal record fixture");
    contract.requested_folder = Some(home.session.workspace().unwrap().name);
    let id = m.draft(contract).unwrap();
    m.submit(id).unwrap();
    answering(&Yes, || m.request_consent(id)).unwrap();
    m.advance(id).unwrap();
    let worker = {
        let m = m.clone();
        std::thread::spawn(move || m.advance(id))
    };
    entered_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    assert_eq!(
        m.coordinator.inner.lock().unwrap().missions[&id].state,
        MissionState::Investigating
    );
    let before = verify(&home.lines()).unwrap();
    assert!(matches!(
        before.iter().rev().find_map(|entry| match &entry.event {
            Event::Transition { trigger } => Some(trigger),
            _ => None,
        }),
        Some(Trigger::Start)
    ));
    mode.store(
        match case.as_str() {
            "prefix" => 1,
            "readable-unsynced" => 2,
            "control" | "later-failure" => 0,
            _ => panic!("unknown case"),
        },
        Ordering::SeqCst,
    );
    m.cell_for_test().engage_emergency();
    release_tx.send(()).unwrap();
    assert!(worker.join().unwrap().is_err());
    if case == "later-failure" {
        mode.store(1, Ordering::SeqCst);
        assert!(m
            .coordinator
            .inner
            .lock()
            .unwrap()
            .write(crate::journal::SESSION, Event::SessionClosed)
            .is_err());
    }
    let facts = m.terminal_facts(id).unwrap();
    assert_eq!(facts.state, Some(MissionState::Stopped));
    assert_eq!(
        facts.first_code(),
        "stop",
        "exact emergency terminal transition is not an earlier Start"
    );
    if matches!(case.as_str(), "control" | "later-failure") {
        assert!(!facts.recording_unknown);
        let seq = facts
            .recorded_sequence
            .expect("successful terminal append has an exact receipt");
        assert!(verify(&home.lines())
            .unwrap()
            .iter()
            .any(|entry| entry.seq == seq
                && matches!(
                    entry.event,
                    Event::Transition {
                        trigger: Trigger::Stop {
                            reason: StopReason::EmergencyStop
                        }
                    }
                )));
    } else {
        assert!(
            facts.recording_unknown,
            "known failed append and poison cannot be repaired by readable bytes"
        );
        assert_eq!(
            facts.recorded_sequence, None,
            "neither Start nor a readable failed append is a durable terminal receipt"
        );
    }
    println!(
        "FIX1 F09 VERIFIED {case} first={} record={:?} unknown={}",
        facts.first_code(),
        facts.recorded_sequence,
        facts.recording_unknown
    );
}

fn child(case: &str) -> std::process::Output {
    let mut process = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", TEST, "--nocapture"])
        .env(CASE, case)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let deadline = std::time::Instant::now() + Duration::from_secs(30);
    while process.try_wait().unwrap().is_none() {
        if std::time::Instant::now() >= deadline {
            process.kill().unwrap();
            process.wait().unwrap();
            panic!("bounded fix1 oracle child timed out: {case}");
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    process.wait_with_output().unwrap()
}
