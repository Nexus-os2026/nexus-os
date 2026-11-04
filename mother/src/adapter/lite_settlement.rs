//! Registered before settlement implementation. Every scenario owns a real
//! fresh broker; hooks establish order, and the assertions count both forms
//! of publication success (ordinary approval and terminal publication).
use super::*;
use crate::journal::{JournalError, JournalStore};
use crate::oracles::m2as_hooks::HookSlot;
use nexus_governed_control::authority::clock::ManualClock;
use nexus_governed_control::authority::file_session::HaltCause;
use std::sync::mpsc;

const SETTLE_CASE: &str = "NEXUS_LITE_SETTLEMENT_CASE";
#[test]
fn lite_settlement_scenarios() {
    for scenario in [
        "published-stop",
        "budget-end",
        "append-internal-budget",
        "own-append-stop",
        "own-append-all",
        "own-append-budget",
        "own-append-emergency",
        "own-owed-stop",
        "own-owed-all",
        "own-owed-budget",
        "own-owed-emergency",
        "budget",
        "lane-stop",
        "coordinator-poison",
        "port-poison",
        "terminal-failure",
        "ack-retry",
        "ack-a-then-b",
        "append-stop",
        "append-all",
        "append-budget",
        "append-emergency",
        "owed-stop",
        "owed-all",
        "owed-budget",
        "owed-emergency",
    ] {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "adapter::lite::settlement::lite_settlement_child",
                "--nocapture",
            ])
            .env(SETTLE_CASE, scenario)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{scenario}: {}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(String::from_utf8_lossy(&output.stdout)
            .contains(&format!("lite-settlement-receipt:{scenario}")));
    }
}

struct HookJournal {
    memory: MemoryJournal,
    hook: HookSlot,
}
impl JournalStore for HookJournal {
    fn load(&self) -> Result<Vec<String>, JournalError> {
        self.memory.load()
    }
    fn append(&mut self, line: &str) -> Result<(), JournalError> {
        self.hook.fire();
        self.memory.append(line)
    }
    fn poison(&mut self) -> Result<(), JournalError> {
        self.memory.poison()
    }
}

fn assert_terminal(m: &FileSession<GovernedPort>, id: MissionId, published: bool) {
    let entries = m.journal().unwrap();
    let success = entries
        .iter()
        .filter(|e| {
            matches!(
                &e.event,
                Event::Transition {
                    trigger: Trigger::ActionApproved { .. }
                        | Trigger::Settled {
                            outcome: SettlementOutcome::Published { .. },
                            ..
                        }
                }
            )
        })
        .count();
    assert_eq!(
        success,
        usize::from(published),
        "ordinary and settlement success both count"
    );
    let ends: Vec<_> = entries
        .iter()
        .filter_map(|e| match &e.event {
            Event::Transition {
                trigger: Trigger::Settled { outcome, .. },
            } => Some(outcome),
            _ => None,
        })
        .collect();
    assert_eq!(ends.len(), 1, "one indivisible terminal journal line");
    assert_eq!(
        matches!(ends[0], SettlementOutcome::Published { .. }),
        published
    );
    assert!(m.state(id).unwrap().unwrap().is_terminal());
}

#[test]
fn lite_settlement_child() {
    let Ok(case) = std::env::var(SETTLE_CASE) else {
        return;
    };
    let f = Fx::new();
    match case.as_str() {
        "budget-end" => {
            let m = f.file();
            let id = f.waiting(&m, "MKFILE:one.txt MKFILE:two.txt");
            f.publish(id);
            assert_eq!(
                m.end_for_test(id, crate::evidence::BlockReason::BudgetTime),
                Ok(MissionState::Blocked)
            );
            assert_eq!(
                m.cell_for_test().cause(),
                Some(HaltCause::TimeBudget),
                "internal budget ending uses the original budget cause before any lock"
            );
            assert_terminal(&m, id, true);
            assert_eq!(
                m.reason(id).unwrap().as_deref(),
                Some("file_publication_interrupted:time_budget")
            );
            assert!(!f.ws.join("two.txt").exists());
        }
        case if case.starts_with("own-") => {
            let hook = HookSlot::default();
            let clock = Arc::new(ManualClock::default());
            let m = Arc::new(
                Coordinator::open(
                    HookJournal {
                        memory: f.journal.clone(),
                        hook: hook.clone(),
                    },
                    f.port.clone(),
                    Lanes::new(files_lanes()).unwrap(),
                    clock.clone(),
                    f.home.private(),
                )
                .unwrap()
                .into_file_session()
                .unwrap(),
            );
            let id = f.waiting(&m, "MKFILE:one.txt MKFILE:two.txt");
            let cause = case.rsplit('-').next().unwrap();
            if cause == "budget" {
                clock.advance(Duration::from_secs(86400));
            }
            if cause == "emergency" {
                f.home.control.emergency_stop();
                assert_eq!(
                    m.cell_for_test().cause(),
                    Some(HaltCause::Emergency),
                    "no Mother observation was required"
                );
            }
            let (held_tx, held_rx) = mpsc::sync_channel(1);
            let (release_tx, release_rx) = mpsc::sync_channel(1);
            let hold = move || {
                held_tx.send(()).unwrap();
                release_rx.recv_timeout(Duration::from_secs(10)).unwrap();
            };
            if case.starts_with("own-append-") {
                hook.arm(0, hold);
            } else {
                m.hooks_for_test().before_owed_write.arm(0, hold);
            }
            let worker = {
                let m = m.clone();
                let cause = cause.to_string();
                std::thread::spawn(move || match cause.as_str() {
                    "budget" => m.advance(id).map(|_| ()),
                    "all" => m.stop_all().map(|_| ()),
                    "emergency" => m.state(id).map(|_| ()),
                    _ => m.stop(id).map(|_| ()),
                })
            };
            held_rx.recv_timeout(Duration::from_secs(5)).unwrap();
            assert!(
                m.cell_for_test().is_engaged(),
                "the Stop call's own append/owed write is held AFTER engagement"
            );
            let pending = f.port.pending(id).unwrap();
            assert!(
                f.home
                    .control
                    .authorize(
                        CommitmentId::parse(&pending.commitment).unwrap(),
                        &mother_agent(),
                        RunId::parse(&pending.run).unwrap(),
                        &f.owner
                    )
                    .is_err(),
                "native admission fails while the Stop caller itself is held"
            );
            release_tx.send(()).unwrap();
            let result = worker.join().unwrap();
            if cause == "budget" {
                assert_eq!(result, Err(CoordinatorError::Stopped));
            } else {
                result.unwrap();
            }
            assert_terminal(&m, id, false);
            assert!(!f.ws.join("one.txt").exists());
            assert!(!f.ws.join("two.txt").exists());
        }
        "ack-a-then-b" => {
            let m = f.file();
            let id = f.waiting(&m, "MKFILE:one.txt MKFILE:two.txt");
            f.publish(id);
            f.port
                .fail_file_ack_once
                .store(true, std::sync::atomic::Ordering::SeqCst);
            m.advance(id).unwrap();
            assert_eq!(m.state(id).unwrap(), Some(MissionState::AwaitingApproval));
            assert_eq!(f.port.acks_tried(), 1);
            assert_eq!(
                std::fs::read_to_string(f.ws.join("one.txt")).unwrap(),
                content_of("MKFILE", "one.txt").unwrap()
            );
            m.stop(id).unwrap();
            assert_eq!(f.port.acks_tried(), 3, "failed A stays owed while B occupies the current wait; Stop acknowledges A and B exactly");
            assert!(!f.ws.join("two.txt").exists());
        }
        "published-stop" | "ack-retry" => {
            let m = f.file();
            let id = f.waiting(&m, "MKFILE:one.txt MKFILE:two.txt");
            f.publish(id);
            if case == "ack-retry" {
                f.port
                    .before_file_ack
                    .arm(0, || panic!("ack interrupted before release"));
                assert!(m.stop(id).is_err());
            } else {
                assert_eq!(m.stop(id), Ok(MissionState::Blocked));
            }
            // A healthy touch retries exact conclusive acknowledgements.
            assert_terminal(&m, id, true);
            assert!(!f.ws.join("two.txt").exists());
            assert_eq!(
                m.reason(id).unwrap().as_deref(),
                Some("file_publication_interrupted:owner_stop")
            );
        }
        "coordinator-poison" | "port-poison" => {
            let m = f.file();
            let id = f.waiting(&m, "MKFILE:one.txt");
            let poisoned = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                if case == "coordinator-poison" {
                    m.poison_for_test();
                } else {
                    let _held = f.port.runs.lock().unwrap();
                    panic!("port poisoned before Stop");
                }
            }));
            assert!(poisoned.is_err());
            let stop = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| m.stop(id)));
            assert!(stop.is_ok(), "typed File Stop must not unwind");
            assert!(stop.unwrap().is_err());
            assert!(m.cell_for_test().is_engaged());
            assert!(m.close().is_err());
            let p = f.port.pending(id).unwrap();
            assert!(f
                .home
                .control
                .authorize(
                    CommitmentId::parse(&p.commitment).unwrap(),
                    &mother_agent(),
                    RunId::parse(&p.run).unwrap(),
                    &f.owner
                )
                .is_err());
        }
        "terminal-failure" => {
            let m = f.file();
            let id = f.waiting(&m, "MKFILE:one.txt");
            f.publish(id);
            // Durable start, then terminal append refuses; no ack is allowed.
            f.journal.fail_after(1);
            assert!(m.stop(id).is_err());
            assert!(m.close().is_err());
            assert_eq!(f.port.acks_tried(), 0);
            assert!(m.cell_for_test().is_engaged());
        }
        "budget" => {
            let clock = Arc::new(ManualClock::default());
            let m = Coordinator::open(
                f.journal.clone(),
                f.port.clone(),
                Lanes::new(files_lanes()).unwrap(),
                clock.clone(),
                f.home.private(),
            )
            .unwrap()
            .into_file_session()
            .unwrap();
            let id = f.waiting(&m, "MKFILE:one.txt");
            f.publish(id);
            clock.advance(Duration::from_secs(86400));
            let _ = m.advance(id);
            assert_eq!(m.cell_for_test().cause(), Some(HaltCause::TimeBudget));
            assert_terminal(&m, id, true);
            assert_eq!(
                m.reason(id).unwrap().as_deref(),
                Some("file_publication_interrupted:time_budget")
            );
        }
        "lane-stop" => {
            let m = Arc::new(f.file());
            let id = m.draft(f.contract("MKFILE:one.txt")).unwrap();
            m.submit(id).unwrap();
            answering(&Yes, || m.request_consent(id)).unwrap();
            m.advance(id).unwrap();
            let cell = m.cell_for_test();
            m.hooks_for_test()
                .file_before_lane
                .arm(0, move || cell.engage());
            assert!(m.advance(id).is_err());
            assert!(
                !m.hooks_for_test().file_before_lane.armed(),
                "forced reservation-to-invocation boundary"
            );
            assert_eq!(
                m.invocations_for_test(),
                0,
                "no lane invocation after ordered engagement"
            );
            assert!(!f.ws.join("one.txt").exists());
        }
        case if case.starts_with("append-") || case.starts_with("owed-") => {
            let hook = HookSlot::default();
            let clock = Arc::new(ManualClock::default());
            let m = Arc::new(
                Coordinator::open(
                    HookJournal {
                        memory: f.journal.clone(),
                        hook: hook.clone(),
                    },
                    f.port.clone(),
                    Lanes::new(files_lanes()).unwrap(),
                    clock.clone(),
                    f.home.private(),
                )
                .unwrap()
                .into_file_session()
                .unwrap(),
            );
            let id = m.draft(f.contract("MKFILE:one.txt")).unwrap();
            m.submit(id).unwrap();
            answering(&Yes, || m.request_consent(id)).unwrap();
            let (held_tx, held_rx) = mpsc::sync_channel(1);
            let (release_tx, release_rx) = mpsc::sync_channel(1);
            let hold = move || {
                held_tx.send(()).unwrap();
                release_rx.recv_timeout(Duration::from_secs(10)).unwrap();
            };
            if case.starts_with("append-") {
                hook.arm(0, hold);
            } else {
                m.hooks_for_test().before_owed_write.arm(0, hold);
            }
            let worker = {
                let m = m.clone();
                std::thread::spawn(move || m.advance(id))
            };
            held_rx.recv_timeout(Duration::from_secs(5)).unwrap();
            let (engaged_tx, engaged_rx) = mpsc::sync_channel(1);
            let cell = m.cell_for_test();
            m.hooks_for_test().file_after_halt.arm(0, move || {
                assert!(cell.is_engaged());
                engaged_tx.send(()).unwrap();
            });
            let cause = case.split_once('-').unwrap().1;
            let stopping = match cause {
                "emergency" => {
                    f.home.control.emergency_stop();
                    assert_eq!(m.cell_for_test().cause(), Some(HaltCause::Emergency));
                    None
                }
                "internal-budget" => {
                    let m = m.clone();
                    Some(std::thread::spawn(move || {
                        m.end_for_test(id, crate::evidence::BlockReason::BudgetTime)
                            .map(|_| ())
                    }))
                }
                "budget" => {
                    clock.advance(Duration::from_secs(86400));
                    let m = m.clone();
                    Some(std::thread::spawn(move || m.advance(id).map(|_| ())))
                }
                "all" => {
                    let m = m.clone();
                    Some(std::thread::spawn(move || m.stop_all().map(|_| ())))
                }
                _ => {
                    let m = m.clone();
                    Some(std::thread::spawn(move || m.stop(id).map(|_| ())))
                }
            };
            if stopping.is_some() {
                engaged_rx.recv_timeout(Duration::from_secs(5)).unwrap();
            }
            assert!(
                m.cell_for_test().is_engaged(),
                "Stop must engage before the held journal/owed dependency"
            );
            release_tx.send(()).unwrap();
            let _ = worker.join().unwrap();
            if let Some(stopping) = stopping {
                let _ = stopping.join().unwrap();
            }
            assert!(m.state(id).unwrap().unwrap().is_terminal());
            assert_eq!(m.invocations_for_test(), 0);
            assert!(!f.ws.join("one.txt").exists());
        }
        _ => panic!("unknown settlement case"),
    }
    println!("lite-settlement-receipt:{case}");
}
