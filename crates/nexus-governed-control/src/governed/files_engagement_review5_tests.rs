//! Broker-only sites: pause at the actual atomic write, before later work.
//! Mother separately checks that each broker tag records its exact terminal.
use super::*;
use crate::authority::approval::*;
use crate::authority::clock::ManualClock;
use crate::authority::evidence::MemoryEvidence;
use crate::broker::Vault;
use std::sync::mpsc;
use std::time::Duration;
struct Owner;
impl ControlConfirmer for Owner {
    fn confirm_action(&self, _: &ActionConfirmation) -> bool {
        true
    }
    fn confirm_grant(&self, _: &GrantConfirmation) -> bool {
        true
    }
    fn confirm_resume(&self, _: &ResumeConfirmation) -> bool {
        true
    }
    fn confirm_acknowledgement(&self, _: &AcknowledgementConfirmation) -> bool {
        true
    }
    fn confirm_agent_door(&self, _: &AgentDoorConfirmation) -> bool {
        true
    }
}
#[test]
fn lite_review5_broker_engagement_sites() {
    let mut failures = vec![];
    for site in [
        "history-poison",
        "replacement-port",
        "replacement-attachment",
        "door-claim",
        "run-attachment",
        "door-halt",
        "emergency",
    ] {
        let root = crate::harness_tests::temp_root("lite-review5");
        let control = Arc::new(
            GovernedControl::with_secrets(
                root.0.path(),
                Vault::Disabled.source(),
                Arc::new(MemoryEvidence::new(1024)),
                Arc::new(ManualClock::default()),
            )
            .unwrap(),
        );
        let door = Arc::new(
            control
                .issue_agent_door(AgentId::new("mother").unwrap(), Arc::new(Owner))
                .unwrap(),
        );
        let port = door.file_port().unwrap();
        let attached = port.attach().unwrap();
        let cell = control.file_sessions.review5_cell();
        match site {
            "door-claim" => {
                door.core.file_cell.set(cell.clone()).unwrap();
            }
            "run-attachment" => {
                let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    control.authority().runs().panic_holding_the_runs_lock()
                }));
            }
            "history-poison" => {
                control.file_sessions.review5_poison();
            }
            _ => {
                door.claim_file_session(&attached).unwrap();
            }
        }
        let (tx, rx) = mpsc::sync_channel(1);
        let (release_tx, release_rx) = mpsc::sync_channel(1);
        cell.1.arm("file.engaged", move || {
            tx.send(()).unwrap();
            release_rx.recv_timeout(Duration::from_secs(10)).unwrap();
        });
        let worker = {
            let door = door.clone();
            let control = control.clone();
            std::thread::spawn(move || match site {
                "history-poison" | "replacement-port" => {
                    let _ = door.file_port();
                }
                "replacement-attachment" => {
                    let _ = port.attach();
                }
                "door-claim" | "run-attachment" => {
                    let _ = door.claim_file_session(&attached);
                }
                "door-halt" => {
                    let _ = door.halt(HaltReason::JournalFailed).write();
                }
                "emergency" => {
                    control.emergency_stop();
                }
                _ => unreachable!(),
            })
        };
        rx.recv_timeout(Duration::from_secs(5)).unwrap();
        let first = cell.reason().unwrap();
        // A competing Stop cannot replace the first tagged outcome.
        cell.engage();
        let after = cell.reason().unwrap();
        release_tx.send(()).unwrap();
        worker.join().unwrap();
        let cause = if site == "emergency" {
            HaltCause::Emergency
        } else {
            HaltCause::Fence
        };
        if first
            != (
                cause,
                std::num::NonZeroU16::new(if site == "emergency" {
                    u16::MAX - 1
                } else {
                    u16::MAX - 3
                }),
            )
            || first != after
        {
            failures.push(format!(
                "{site}: exact tagged outcome required, got {first:?} -> {after:?}"
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
