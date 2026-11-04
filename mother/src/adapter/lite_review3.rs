//! Review 3: the terminal contract across the complete File-mode state/cause
//! product. Each row has a fresh real broker; state is reached through product
//! calls. Holds use the same hold operation as recovery, without replacement.
use super::*;
use crate::adapter::m2c_support::{port_for, Home};
use crate::adapter::{create_private, mother_agent};
use crate::evidence::SettlementEnd;
use crate::journal::MemoryJournal;
use crate::lane::{Lane, LaneIdentity, LaneOutput, Verdict};
use crate::oracles::lite_support::{file, files_lanes, FilesOwner};
use crate::oracles::m2c_confirmers::answering;
use crate::oracles::{contract_with, Yes};
use nexus_governed_control::authority::clock::ManualClock;
use nexus_governed_control::authority::commitment::CommitmentState;
use nexus_governed_control::authority::effect::{CapabilityKind, EffectClass};
use nexus_governed_control::egress::EgressIntent;
use nexus_governed_control::governed::GrantRequest;
use std::io::ErrorKind;
use std::net::TcpListener;
use std::sync::mpsc;
use std::time::Duration;

const CASE: &str = "NEXUS_LITE_REVIEW3_PAIR";
const TEST: &str = "coordinator::lite::review3::lite_terminal_state_cause_matrix";
const STATES: &[(&str, MissionState)] = &[
    ("draft", MissionState::Draft),
    ("consent", MissionState::AwaitingConsent),
    ("approved", MissionState::Approved),
    ("investigating", MissionState::Investigating),
    ("building", MissionState::Building),
    ("verifying", MissionState::Verifying),
    ("explaining", MissionState::Explaining),
    ("file-wait", MissionState::AwaitingApproval),
    ("post-wait", MissionState::AwaitingApproval),
    ("file-dispatch", MissionState::Building),
    ("post-dispatch", MissionState::Building),
];
const CAUSES: &[&str] = &[
    "owner",
    "mission",
    "budget",
    "emergency",
    "fence",
    "stop-all",
    "typed",
    "internal-end",
    "journal-failure",
    "terminal-record-failure",
];

struct MixedBuilder(String);
impl Lane for MixedBuilder {
    fn identity(&self) -> LaneIdentity {
        LaneIdentity {
            role: LaneRole::Builder,
            name: "review3-mixed".into(),
            version: "1".into(),
        }
    }
    fn invoke(&self, _: &LaneInput) -> LaneOutput {
        LaneOutput {
            text: "File A, R2 POST, File B".into(),
            effects: vec![
                file("a.txt", "A\n"),
                EffectRequest::Network(EgressIntent {
                    method: "POST".into(),
                    url: format!("{}/review3", self.0),
                    headers: vec![],
                    body: None,
                }),
                file("b.txt", "B\n"),
            ],
            verdict: Verdict::Continue,
        }
    }
}

struct PostBuilder(String);
impl Lane for PostBuilder {
    fn identity(&self) -> LaneIdentity {
        MixedBuilder(self.0.clone()).identity()
    }
    fn invoke(&self, input: &LaneInput) -> LaneOutput {
        let mut output = MixedBuilder(self.0.clone()).invoke(input);
        output.effects.remove(0);
        output
    }
}

#[test]
fn lite_terminal_state_cause_matrix() {
    let Ok(pair) = std::env::var(CASE) else {
        let mut failures = Vec::new();
        let mut count = 0;
        for (state, _) in STATES {
            for held in [false, true] {
                // Recovery cannot hold a still-unsettled dispatch: it must
                // terminate it first. Test that live reservation separately.
                if held && state.ends_with("dispatch") {
                    continue;
                }
                for cause in CAUSES {
                    let pair = format!("{state}/{held}/{cause}");
                    let output = std::process::Command::new(std::env::current_exe().unwrap())
                        .args(["--exact", TEST, "--nocapture"])
                        .env(CASE, &pair)
                        .output()
                        .unwrap();
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    if !output.status.success() || !stdout.contains(&format!("review3-pair:{pair}"))
                    {
                        failures.push(format!(
                            "{pair}:\n{stdout}\n{}",
                            String::from_utf8_lossy(&output.stderr)
                        ));
                    }
                    count += 1;
                }
            }
        }
        assert!(failures.is_empty(), "{}", failures.join("\n"));
        println!("review3-matrix:{count} pairs");
        return;
    };
    let parts: Vec<_> = pair.split('/').collect();
    let (stage, held, cause) = (parts[0], parts[1] == "true", parts[2]);
    assert!(CAUSES.contains(&cause));
    let expected_state = STATES.iter().find(|(s, _)| *s == stage).unwrap().1;
    assert!(nexus_governed_control::files::make_non_dumpable());
    let home = Home::fresh_temp("review3").unwrap();
    let workspace = home.dir.join("workspace");
    create_private(&workspace).unwrap();
    let info = home.control.register_workspace(&workspace, &[]).unwrap();
    let owner = FilesOwner::default();
    home.control
        .request_grant(
            &GrantRequest::Files { workspace: info.id },
            Duration::from_secs(86400),
            &owner,
        )
        .unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    home.control
        .request_grant(
            &GrantRequest::Egress {
                origin: origin.clone(),
                methods: vec!["POST".into()],
                allow_private: true,
            },
            Duration::from_secs(600),
            &owner,
        )
        .unwrap();
    let port = Arc::new(port_for(&home.control, true).with_workspace(info.name.clone()));
    let mut lanes = files_lanes();
    if stage == "post-wait" {
        lanes.retain(|l| l.identity().role != LaneRole::Builder);
        lanes.push(Arc::new(MixedBuilder(origin)));
    } else if stage == "post-dispatch" {
        lanes.retain(|l| l.identity().role != LaneRole::Builder);
        lanes.push(Arc::new(PostBuilder(origin)));
    }
    let clock = Arc::new(ManualClock::default());
    let journal = MemoryJournal::default();
    let m = Arc::new(
        Coordinator::open(
            journal.clone(),
            port.clone(),
            Lanes::new(lanes).unwrap(),
            clock.clone(),
            home.private(),
        )
        .unwrap()
        .into_file_session()
        .unwrap(),
    );
    let mut contract = contract_with(if matches!(stage, "file-wait" | "file-dispatch") {
        "MKFILE:a.txt MKFILE:b.txt"
    } else {
        "terminal matrix"
    });
    contract.requested_folder = Some(info.name);
    let id = m.draft(contract.clone()).unwrap();
    // The budget can legitimately remain armed after Approved -> Revise.
    // Exercise that route for Draft/consent, rather than inventing a deadline.
    m.submit(id).unwrap();
    assert_eq!(
        answering(&Yes, || m.request_consent(id)),
        Ok(MissionState::Approved)
    );
    if matches!(stage, "draft" | "consent") {
        assert_eq!(m.revise(id, contract), Ok(MissionState::Draft));
        if stage == "consent" {
            m.submit(id).unwrap();
        }
    } else {
        for _ in 0..6 {
            if m.state(id).unwrap() == Some(expected_state) {
                break;
            }
            m.advance(id).unwrap();
        }
    }
    if stage == "post-wait" {
        let action = port.pending(id).unwrap();
        let run = RunId::parse(&action.run).unwrap();
        let commitment = CommitmentId::parse(&action.commitment).unwrap();
        home.control
            .authorize(commitment, &mother_agent(), run, &owner)
            .unwrap();
        home.control
            .execute(commitment, &mother_agent(), run)
            .unwrap();
        assert_eq!(
            std::fs::read_to_string(workspace.join("a.txt")).unwrap(),
            "A\n"
        );
        assert_eq!(m.advance(id).unwrap().to, MissionState::AwaitingApproval);
        let action = port.pending(id).unwrap();
        let view = home
            .control
            .authority()
            .commitments()
            .view(CommitmentId::parse(&action.commitment).unwrap())
            .unwrap();
        assert_eq!(view.kind, CapabilityKind::Egress);
        assert_eq!(view.class, EffectClass::R2);
        assert!(view.requires_approval);
        assert_eq!(view.state, CommitmentState::Prepared);
    }
    assert_eq!(
        m.state(id).unwrap(),
        Some(expected_state),
        "matrix state is reached"
    );
    if held {
        m.coordinator.hold_unfinished();
        assert!(m.coordinator.held(id).is_some());
    }
    let dispatch = if stage.ends_with("dispatch") {
        let (entered_tx, entered_rx) = mpsc::sync_channel(1);
        let (release_tx, release_rx) = mpsc::sync_channel(1);
        m.hooks_for_test().file_after_perform_check.arm(0, move || {
            entered_tx.send(()).unwrap();
            release_rx.recv_timeout(Duration::from_secs(10)).unwrap();
        });
        let worker = {
            let m = m.clone();
            std::thread::spawn(move || m.advance(id))
        };
        entered_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        assert!(m.coordinator.inner.lock().unwrap().missions[&id]
            .unsettled
            .is_some());
        Some((release_tx, worker))
    } else {
        None
    };
    let pending = port.pending(id);
    let before = journal.lines().len();
    let invocations = m.invocations_for_test();
    let first = match cause {
        "owner" | "mission" | "stop-all" | "typed" => HaltCause::Stop,
        "budget" => HaltCause::TimeBudget,
        "emergency" => HaltCause::Emergency,
        _ => HaltCause::Fence,
    };
    let result = match cause {
        "owner" => m.stop(id).map(|_| ()),
        "mission" => m.coordinator.stop(id).map(|_| ()),
        "typed" => m
            .call(|c| c.file_stop(id, SettleCause::Stop(StopReason::Owner)))
            .map(|_| ()),
        "stop-all" => m.stop_all().map(|n| assert_eq!(n, 1)),
        "budget" => {
            clock.advance(Duration::from_secs(86400));
            m.state(id).map(|_| ())
        }
        "emergency" => {
            home.control.emergency_stop();
            assert_eq!(m.cell_for_test().cause(), Some(first));
            m.state(id).map(|_| ())
        }
        "fence" => {
            port.fence().write_checked().unwrap();
            assert_eq!(m.cell_for_test().cause(), Some(first));
            m.state(id).map(|_| ())
        }
        "internal-end" => m
            .end_for_test(id, BlockReason::PhaseInterrupted)
            .map(|_| ()),
        "journal-failure" | "terminal-record-failure" => {
            // A failed journal cannot truthfully certify a clean close. Keep
            // the lite failure contract, including failure AFTER a durable
            // File settlement start, separately from healthy fence recording.
            let prefix = if cause == "terminal-record-failure" {
                usize::from(held) + usize::from(matches!(stage, "file-wait" | "file-dispatch"))
            } else {
                0
            };
            journal.fail_after(prefix);
            m.end_for_test(id, BlockReason::JournalFailed).map(|_| ())
        }
        _ => unreachable!(),
    };
    if matches!(cause, "journal-failure" | "terminal-record-failure") {
        assert!(
            matches!(result, Err(CoordinatorError::Journal(_))),
            "{pair}: report the actual recording error, never an illegal transition: {result:?}"
        );
        assert!(journal.is_poisoned());
        assert_eq!(m.cell_for_test().cause(), Some(first));
        if let Some((release, worker)) = dispatch {
            release.send(()).unwrap();
            assert!(worker.join().unwrap().is_err());
        }
        let failed = journal.lines();
        home.control.emergency_stop();
        assert!(m.stop(id).is_err());
        assert!(m.stop_all().is_err());
        assert!(m.advance(id).is_err());
        assert!(
            m.close().is_err(),
            "unfinished recording forbids clean close"
        );
        assert_eq!(m.cell_for_test().cause(), Some(first));
        assert_eq!(m.invocations_for_test(), invocations);
        assert_eq!(
            journal.lines(),
            failed,
            "no false durable completion on a broken store"
        );
        let entries = verify(&failed).unwrap();
        assert!(!entries[before..].iter().any(|e| matches!(
            e.event,
            Event::Transition { .. }
                | Event::SessionClosed
                | Event::LaneInvoked { .. }
                | Event::EffectDispatched { .. }
        )));
        if cause == "terminal-record-failure" && matches!(stage, "file-wait" | "file-dispatch") {
            assert!(
                entries[before..].iter().any(|e| matches!(
                    e.event,
                    Event::SettlementStarted {
                        cause: SettleCause::Ended(BlockReason::JournalFailed),
                        ..
                    }
                )),
                "failed terminal follows a durable File settlement start"
            );
        }
        if let Some(pending) = pending {
            assert!(home
                .control
                .authorize(
                    CommitmentId::parse(&pending.commitment).unwrap(),
                    &mother_agent(),
                    RunId::parse(&pending.run).unwrap(),
                    &owner,
                )
                .is_err());
        }
        assert!(!workspace.join("b.txt").exists());
        assert_eq!(listener.accept().unwrap_err().kind(), ErrorKind::WouldBlock);
        println!("review3-pair:{pair}");
        return;
    }
    assert_eq!(
        result,
        Ok(()),
        "{pair}: terminal route must be legal and recordable"
    );
    let terminal = m.state(id).unwrap().unwrap();
    assert!(
        terminal.is_terminal(),
        "{pair}: latched cause must terminate"
    );
    assert_eq!(m.cell_for_test().cause(), Some(first));
    if let Some((release, worker)) = dispatch {
        release.send(()).unwrap();
        assert_eq!(worker.join().unwrap(), Err(CoordinatorError::Stopped));
    }
    // Later ingresses cannot replace the first cause or append another end.
    home.control.emergency_stop();
    assert_eq!(m.stop(id), Ok(terminal));
    assert_eq!(m.stop_all(), Ok(0));
    assert_eq!(m.end_for_test(id, BlockReason::BudgetTime), Ok(terminal));
    assert!(m.advance(id).is_err());
    assert!(m.submit(id).is_err());
    assert_eq!(m.cell_for_test().cause(), Some(first));
    assert_eq!(m.invocations_for_test(), invocations);
    assert!(!workspace.join("b.txt").exists());
    assert_eq!(listener.accept().unwrap_err().kind(), ErrorKind::WouldBlock);
    if stage != "post-wait" {
        assert!(!workspace.join("a.txt").exists());
    }
    if let Some(pending) = pending {
        assert!(home
            .control
            .authorize(
                CommitmentId::parse(&pending.commitment).unwrap(),
                &mother_agent(),
                RunId::parse(&pending.run).unwrap(),
                &owner,
            )
            .is_err());
    }
    m.close()
        .expect("recorded conclusive terminal permits clean close");
    let entries = verify(&journal.lines()).unwrap();
    let mut missions = BTreeMap::new();
    let mut next = 1;
    let mut session = Session::Never;
    let mut ends = Vec::new();
    for entry in &entries {
        if let Event::Transition { trigger } = &entry.event {
            let from = missions
                .get(&id)
                .map(|m: &Mission| m.state)
                .unwrap_or(MissionState::Draft);
            if transition(from, trigger)
                .expect("every recorded transition is legal")
                .is_terminal()
            {
                ends.push(trigger);
            }
        }
        check(&entry.event).unwrap();
        replay(&mut missions, &mut next, &mut session, entry)
            .unwrap_or_else(|e| panic!("{pair}: replay refused {:?}: {e:?}", entry.event));
    }
    assert_eq!(ends.len(), 1, "{pair}: exactly one legal terminal record");
    assert!(session == Session::Closed);
    assert_eq!(missions[&id].state, terminal);
    let terminal_json = serde_json::to_string(ends[0]).unwrap();
    let expected_cause = match cause {
        "budget" => "budget_time",
        "emergency" => "emergency_stop",
        "fence" => "door_halted",
        "internal-end" => "phase_interrupted",
        _ => "owner",
    };
    assert!(
        terminal_json.contains(expected_cause),
        "{pair}: first cause in {terminal_json}"
    );
    assert!(
        !entries[before..].iter().any(|e| matches!(
            e.event,
            Event::LaneInvoked { .. }
                | Event::EffectDispatched { .. }
                | Event::Transition {
                    trigger: Trigger::ActionApproved { .. }
                }
        )),
        "{pair}: no work or success after Stop"
    );
    let settlements: Vec<_> = entries
        .iter()
        .filter_map(|e| match &e.event {
            Event::Transition {
                trigger:
                    Trigger::Settled {
                        effect,
                        outcome,
                        end,
                        ..
                    },
            } => Some((*effect, outcome, end)),
            _ => None,
        })
        .collect();
    if matches!(stage, "file-wait" | "file-dispatch") {
        assert_eq!(settlements.len(), 1);
        assert_eq!(settlements[0].0, 0);
        assert_eq!(settlements[0].1, &SettlementOutcome::NotPublished);
        let cause = match cause {
            "budget" => SettleCause::TimeBudget,
            "emergency" => SettleCause::Stop(StopReason::EmergencyStop),
            "fence" => SettleCause::Stop(StopReason::DoorHalted),
            "internal-end" => SettleCause::Ended(BlockReason::PhaseInterrupted),
            _ => SettleCause::Stop(StopReason::Owner),
        };
        assert_eq!(
            settlements[0].2,
            &cause.end(&SettlementOutcome::NotPublished)
        );
        assert!(matches!(
            settlements[0].2,
            SettlementEnd::Block(_) | SettlementEnd::Stop(_)
        ));
    } else {
        assert!(
            settlements.is_empty(),
            "non-File work cannot borrow a File settlement"
        );
    }
    println!("review3-pair:{pair}");
}
