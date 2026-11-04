//! Review 4: first-cause races and terminal obligations. Every child owns a
//! fresh real broker. Channels order competing calls; no timing assumptions.
use super::*;
use crate::adapter::lite_clock::{past_the_start, LiteHome};
use crate::adapter::m2c_support::port_for;
use crate::adapter::{create_private, mother_agent, GovernedPort};
use crate::journal::MemoryJournal;
use crate::oracles::lite_support::{files_lanes, FilesOwner};
use crate::oracles::m2c_confirmers::answering;
use crate::oracles::{contract_with, Yes};
use nexus_governed_control::authority::clock::ManualClock;
use nexus_governed_control::governed::GrantRequest;
use std::sync::mpsc;
use std::time::Duration;

const RACE: &str = "NEXUS_LITE_REVIEW4_RACE";
const RACE_TEST: &str = "coordinator::lite::review4::lite_first_cause_competing_touch";

struct Fixture {
    home: LiteHome,
    port: Arc<GovernedPort>,
    journal: MemoryJournal,
    clock: Arc<ManualClock>,
    folder: String,
    owner: FilesOwner,
}
impl Fixture {
    fn new() -> Self {
        assert!(nexus_governed_control::files::make_non_dumpable());
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let home =
            LiteHome::open(&std::env::temp_dir().join(format!("nexus-lite-review4-{unique}")))
                .unwrap();
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
        Self {
            port: Arc::new(port_for(&home.control, true).with_workspace(info.name.clone())),
            home,
            journal: MemoryJournal::default(),
            clock: Arc::new(ManualClock::default()),
            folder: info.name,
            owner,
        }
    }
    fn open(&self) -> Arc<FileSession<GovernedPort>> {
        Arc::new(
            Coordinator::open(
                self.journal.clone(),
                self.port.clone(),
                Lanes::new(files_lanes()).unwrap(),
                self.clock.clone(),
                self.home.private(),
            )
            .unwrap()
            .into_file_session()
            .unwrap(),
        )
    }
    fn approved(&self, m: &FileSession<GovernedPort>, markers: &str, calls: u32) -> MissionId {
        let mut contract = contract_with(markers);
        contract.requested_folder = Some(self.folder.clone());
        contract.budget.max_model_calls = calls;
        let id = m.draft(contract).unwrap();
        m.submit(id).unwrap();
        assert_eq!(
            answering(&Yes, || m.request_consent(id)),
            Ok(MissionState::Approved)
        );
        id
    }
    fn wait(&self, m: &FileSession<GovernedPort>, markers: &str) -> MissionId {
        let id = self.approved(m, markers, 50);
        for _ in 0..3 {
            m.advance(id).unwrap();
        }
        assert_eq!(m.state(id).unwrap(), Some(MissionState::AwaitingApproval));
        id
    }
}

fn run_children(env: &str, test: &str, cases: impl IntoIterator<Item = String>) {
    let mut failures = Vec::new();
    let mut count = 0;
    for case in cases {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", test, "--nocapture"])
            .env(env, &case)
            .output()
            .unwrap();
        let stdout = String::from_utf8_lossy(&output.stdout);
        if !output.status.success() || !stdout.contains(&format!("review4:{case}")) {
            failures.push(format!(
                "{case}:\n{stdout}\n{}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }
        count += 1;
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    println!("review4-matrix:{count} cases");
}

fn terminal_records(lines: &[String], id: MissionId) -> (MissionState, Vec<Trigger>) {
    let mut missions = BTreeMap::new();
    let mut next = 1;
    let mut session = Session::Never;
    let mut ends = Vec::new();
    for entry in verify(lines).unwrap() {
        if entry.mission == id {
            if let Event::Transition { trigger } = &entry.event {
                let from = missions
                    .get(&id)
                    .map(|m: &Mission| m.state)
                    .unwrap_or(MissionState::Draft);
                if transition(from, trigger).unwrap().is_terminal() {
                    ends.push(trigger.clone());
                }
            }
        }
        check(&entry.event).unwrap();
        replay(&mut missions, &mut next, &mut session, &entry).unwrap();
    }
    (missions[&id].state, ends)
}

#[test]
fn lite_first_cause_competing_touch() {
    let Ok(case) = std::env::var(RACE) else {
        let mut cases = vec!["exhausted".to_owned()];
        // Includes every internal BlockReason, including parameterized lane,
        // broker and publication reasons, and every typed Stop cause.
        for cause in SettleCause::every() {
            for stage in ["approved", "file-wait"] {
                cases.push(format!("{stage}/{}", cause.as_str()));
            }
        }
        run_children(RACE, RACE_TEST, cases);
        return;
    };
    let f = Fixture::new();
    let m = f.open();
    let exhausted = case == "exhausted";
    let (stage, cause) = if exhausted {
        (
            "approved",
            SettleCause::Ended(BlockReason::BudgetModelCalls),
        )
    } else {
        let (stage, code) = case.split_once('/').unwrap();
        (stage, SettleCause::parse(code).unwrap())
    };
    let id = if stage == "file-wait" {
        f.wait(&m, "MKFILE:a.txt MKFILE:b.txt")
    } else {
        f.approved(&m, "competing touches", if exhausted { 1 } else { 50 })
    };
    if exhausted {
        assert_eq!(m.advance(id).unwrap().to, MissionState::Investigating);
    }
    let (entered_tx, entered_rx) = mpsc::sync_channel(1);
    let (release_tx, release_rx) = mpsc::sync_channel(1);
    m.hooks_for_test().file_after_halt.arm(0, move || {
        entered_tx.send(()).unwrap();
        release_rx.recv_timeout(Duration::from_secs(10)).unwrap();
    });
    let worker = {
        let m = m.clone();
        std::thread::spawn(move || {
            if exhausted {
                m.advance(id).map(|r| r.to)
            } else {
                m.call(|c| c.file_stop(id, cause))
            }
        })
    };
    entered_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    assert!(m.cell_for_test().is_engaged());
    // The state touch must win the mission lock and journal the exact first
    // reason while the initiating caller is still held outside that lock.
    let touched = m.state(id);
    let at_touch = f.journal.lines();
    release_tx.send(()).unwrap();
    let initiated = worker.join().unwrap();
    let expected = if stage == "file-wait" {
        let action = m.coordinator.inner.lock().unwrap().missions[&id]
            .open_action
            .clone();
        assert!(action.is_none(), "conclusive File action closes");
        // The exact action is already journaled; compare the cause and end
        // independently of the identifier minted by this fresh broker.
        let entries = verify(&at_touch).unwrap();
        assert!(
            entries.iter().any(|e| matches!(e.event,
                Event::SettlementStarted { effect: 0, cause: first } if first == cause
            )),
            "{case}: first settlement cause must survive competing touch"
        );
        let (_, ends) = terminal_records(&at_touch, id);
        assert_eq!(ends.len(), 1);
        let Trigger::Settled {
            effect,
            action,
            outcome,
            end,
        } = &ends[0]
        else {
            panic!("File terminal");
        };
        assert_eq!(*effect, 0);
        assert!(action.is_some());
        assert_eq!(*outcome, SettlementOutcome::NotPublished);
        assert_eq!(
            *end,
            cause.end(&SettlementOutcome::NotPublished),
            "{case}: exact first end"
        );
        ends[0].clone()
    } else {
        match cause {
            SettleCause::Stop(reason) => Trigger::Stop { reason },
            SettleCause::TimeBudget => Trigger::Block {
                reason: BlockReason::BudgetTime,
            },
            SettleCause::Ended(reason) => Trigger::Block { reason },
        }
    };
    let (terminal, ends) = terminal_records(&at_touch, id);
    assert_eq!(
        ends,
        vec![expected],
        "{case}: competing touch must record the EXACT first cause"
    );
    assert_eq!(touched, Ok(Some(terminal)));
    assert_eq!(initiated, Ok(terminal));
    if exhausted {
        assert_eq!(
            m.coordinator.inner.lock().unwrap().missions[&id].model_calls,
            1
        );
    }
    assert_eq!(
        f.journal.lines(),
        at_touch,
        "held caller cannot append another terminal"
    );
    f.home.control.emergency_stop();
    assert_eq!(m.stop(id), Ok(terminal));
    assert_eq!(f.journal.lines(), at_touch);
    assert!(m.advance(id).is_err());
    m.close().unwrap();
    assert!(!f.home.dir.join("workspace/b.txt").exists());
    println!("review4:{case}");
}

const TERMINAL_CASE: &str = "NEXUS_LITE_REVIEW4_TERMINAL";
const TERMINAL_TEST: &str = "coordinator::lite::review4::lite_terminal_obligation_matrix";
const STOP_CAUSES: &[&str] = &[
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

// Exhaustive over the state machine, not a second manually selected inventory.
// Review 3 exercises the nonterminal rows (including held/dispatch variants).
// Failed is not a live File outcome: end_with converts Fail/VerifyFail to
// PhaseInterrupted. Its row exercises that route and requires Blocked.
fn terminal_rows(state: MissionState) -> &'static [&'static str] {
    match state {
        MissionState::Draft
        | MissionState::AwaitingConsent
        | MissionState::Approved
        | MissionState::Investigating
        | MissionState::Building
        | MissionState::Verifying
        | MissionState::Explaining
        | MissionState::AwaitingApproval => &[],
        MissionState::Done => &["done"],
        MissionState::Denied => &["denied-consent", "denied-file", "denied-ack"],
        MissionState::Stopped => &["stopped", "stopped-ack"],
        MissionState::Blocked => &["blocked", "published-ack", "uncertain"],
        MissionState::Failed => &["failed-lane"],
    }
}

struct FailingVerifier;
impl crate::lane::Lane for FailingVerifier {
    fn identity(&self) -> crate::lane::LaneIdentity {
        crate::lane::LaneIdentity {
            role: LaneRole::Verifier,
            name: "failing-verifier".into(),
            version: "1".into(),
        }
    }
    fn invoke(&self, _: &LaneInput) -> crate::lane::LaneOutput {
        crate::lane::LaneOutput {
            text: "verification failed".into(),
            effects: vec![],
            verdict: Verdict::Fail {
                reason: "fixture".into(),
            },
        }
    }
}

#[test]
fn lite_terminal_obligation_matrix() {
    let Ok(case) = std::env::var(TERMINAL_CASE) else {
        let cases = MissionState::ALL
            .into_iter()
            .flat_map(terminal_rows)
            .flat_map(|row| {
                STOP_CAUSES
                    .iter()
                    .map(move |cause| format!("{row}/{cause}"))
            });
        run_children(TERMINAL_CASE, TERMINAL_TEST, cases);
        return;
    };
    terminal_pair(&case);
}

pub(super) fn terminal_pair(case: &str) {
    let (row, cause) = case.split_once('/').unwrap();
    assert!(MissionState::ALL
        .into_iter()
        .flat_map(terminal_rows)
        .any(|r| *r == row));
    assert!(STOP_CAUSES.contains(&cause));
    let f = Fixture::new();
    let m = if row == "failed-lane" {
        let mut lanes = files_lanes();
        lanes.retain(|l| l.identity().role != LaneRole::Verifier);
        lanes.push(Arc::new(FailingVerifier));
        Arc::new(
            Coordinator::open(
                f.journal.clone(),
                f.port.clone(),
                Lanes::new(lanes).unwrap(),
                f.clock.clone(),
                f.home.private(),
            )
            .unwrap()
            .into_file_session()
            .unwrap(),
        )
    } else {
        f.open()
    };
    let owes_ack = row.ends_with("-ack");
    let uncertain = row == "uncertain";
    let mut executing = None;
    let mut release_execution = None;
    let mut action = None;
    let id;
    let expected;
    if row == "denied-consent" {
        let mut c = contract_with("deny consent");
        c.requested_folder = Some(f.folder.clone());
        id = m.draft(c).unwrap();
        m.submit(id).unwrap();
        assert_eq!(
            answering(&crate::oracles::No, || m.request_consent(id)),
            Err(CoordinatorError::Denied)
        );
        expected = MissionState::Denied;
    } else if matches!(
        row,
        "done" | "denied-file" | "denied-ack" | "stopped-ack" | "published-ack" | "uncertain"
    ) {
        id = f.wait(&m, "MKFILE:a.txt");
        let pending = f.port.pending(id).unwrap();
        let run = RunId::parse(&pending.run).unwrap();
        let commitment = CommitmentId::parse(&pending.commitment).unwrap();
        action = Some(pending);
        if row.starts_with("denied") {
            assert!(f
                .home
                .control
                .authorize(commitment, &mother_agent(), run, &crate::oracles::No)
                .is_err());
        } else if row != "stopped-ack" {
            f.home
                .control
                .authorize(commitment, &mother_agent(), run, &f.owner)
                .unwrap();
            if uncertain {
                let (entered_tx, entered_rx) = mpsc::sync_channel(1);
                let (release_tx, release_rx) = mpsc::sync_channel(1);
                f.home.clock.arm(past_the_start(&f.home, run), move || {
                    entered_tx.send(()).unwrap();
                    release_rx.recv_timeout(Duration::from_secs(10)).unwrap();
                });
                let control = f.home.control.clone();
                executing = Some(std::thread::spawn(move || {
                    control.execute(commitment, &mother_agent(), run)
                }));
                entered_rx.recv_timeout(Duration::from_secs(5)).unwrap();
                release_execution = Some(release_tx);
                assert_eq!(
                    f.port.file_publication(id).unwrap().unwrap().publication,
                    nexus_governed_control::authority::commitment::PublicationClass::Pending
                );
            } else {
                f.home
                    .control
                    .execute(commitment, &mother_agent(), run)
                    .unwrap();
            }
        }
        if owes_ack {
            m.hooks_for_test()
                .file_before_ack
                .arm(0, || panic!("interrupt terminal acknowledgement"));
        }
        let result = if row.starts_with("denied") || row == "done" {
            m.advance(id).map(|r| r.to)
        } else {
            m.stop(id)
        };
        if owes_ack {
            assert_eq!(result, Err(CoordinatorError::Poisoned));
        } else {
            result.unwrap();
        }
        expected = match row {
            "done" => {
                for _ in 0..6 {
                    if m.coordinator.state(id).unwrap().is_terminal() {
                        break;
                    }
                    m.advance(id).unwrap();
                }
                MissionState::Done
            }
            "denied-file" | "denied-ack" => MissionState::Denied,
            "stopped-ack" => MissionState::Stopped,
            _ => MissionState::Blocked,
        };
    } else {
        id = f.approved(&m, "terminal state", 50);
        expected = if row == "stopped" {
            m.stop(id).unwrap()
        } else if row == "blocked" {
            m.end_for_test(id, BlockReason::TooManyEffects).unwrap()
        } else {
            for _ in 0..6 {
                if m.coordinator.state(id).unwrap().is_terminal() {
                    break;
                }
                m.advance(id).unwrap();
            }
            assert_eq!(
                m.coordinator.reason(id).as_deref(),
                Some("phase_interrupted")
            );
            MissionState::Blocked
        };
    }
    // Finish the owned execution before assertions that might panic. The
    // original terminal uncertainty must survive even a later conclusive view.
    if let Some(worker) = executing {
        release_execution.unwrap().send(()).unwrap();
        let _ = worker.join().unwrap();
        assert!(!f.home.clock.disarm(), "native apply boundary was reached");
    }
    assert_eq!(m.coordinator.state(id), Some(expected));
    let original = terminal_records(&f.journal.lines(), id);
    assert_eq!(original.0, expected);
    assert_eq!(original.1.len(), 1, "original terminal is already durable");
    let invocations = m.invocations_for_test();
    let performed = f.port.performed();
    {
        let inner = m.coordinator.inner.lock().unwrap();
        let mission = &inner.missions[&id];
        assert_eq!(mission.terminal_uncertain, uncertain);
        assert_eq!(
            mission.acks,
            if owes_ack {
                vec![action.clone().unwrap()]
            } else {
                vec![]
            }
        );
        if uncertain {
            assert!(mission.open_action.is_some());
        }
    }
    if owes_ack {
        m.hooks_for_test()
            .file_before_ack
            .arm(0, || panic!("ack still unavailable during close"));
        assert_eq!(
            m.close(),
            Err(CoordinatorError::Poisoned),
            "close refuses an undischarged acknowledgement"
        );
        assert_eq!(
            m.coordinator.inner.lock().unwrap().missions[&id].acks,
            vec![action.clone().unwrap()]
        );
    } else if uncertain {
        assert_eq!(m.close(), Err(CoordinatorError::FileUnfinished));
    }
    let before = f.journal.lines();
    let ack_before = f.port.acks_tried();
    let broken_store = matches!(cause, "journal-failure" | "terminal-record-failure");
    let result = match cause {
        "owner" => m.stop(id).map(|_| ()),
        "mission" => m.coordinator.stop(id).map(|_| ()),
        "budget" => {
            f.clock.advance(Duration::from_secs(86400));
            m.state(id).map(|_| ())
        }
        "emergency" => {
            f.home.control.emergency_stop();
            m.state(id).map(|_| ())
        }
        "fence" => {
            f.port.fence().write_checked().unwrap();
            m.state(id).map(|_| ())
        }
        "stop-all" => m.stop_all().map(|n| assert_eq!(n, 0)),
        "typed" => m
            .call(|c| c.file_stop(id, SettleCause::Stop(StopReason::Owner)))
            .map(|_| ()),
        "internal-end" => m
            .end_for_test(id, BlockReason::PhaseInterrupted)
            .map(|_| ()),
        "journal-failure" | "terminal-record-failure" => {
            f.journal.fail_after(0);
            m.end_for_test(id, BlockReason::JournalFailed).map(|_| ())
        }
        _ => unreachable!(),
    };
    assert_eq!(
        result,
        Ok(()),
        "{case}: terminal touch discharges conclusive obligations"
    );
    // Read directly: a second touch must not accidentally repair the call
    // under test before we inspect its acknowledgements.
    assert!(
        m.coordinator.inner.lock().unwrap().missions[&id]
            .acks
            .is_empty(),
        "{case}: healthy terminal touch must acknowledge exactly the conclusive obligation"
    );
    assert_eq!(
        f.port.acks_tried(),
        ack_before + usize::from(owes_ack),
        "{case}: uncertainty never acknowledged"
    );
    assert_eq!(
        f.journal.lines(),
        before,
        "{case}: original terminal and reason preserved"
    );
    assert_eq!(terminal_records(&before, id), original);
    assert_eq!(m.invocations_for_test(), invocations);
    assert_eq!(f.port.performed(), performed);
    if uncertain {
        assert_eq!(m.notice(id).unwrap(), Some(FILE_UNCERTAIN_NOTICE));
        assert_eq!(m.close(), Err(CoordinatorError::FileUnfinished));
        assert!(
            f.home
                .control
                .authority()
                .commitments()
                .view(CommitmentId::parse(&action.as_ref().unwrap().commitment).unwrap())
                .is_some(),
            "uncertain obligation stays in broker retention"
        );
        assert_eq!(f.port.acks_tried(), ack_before);
    } else if broken_store {
        assert!(
            matches!(m.close(), Err(CoordinatorError::Journal(_))),
            "failed close recording is observable"
        );
    } else {
        m.close().expect("healthy close once no work is owed");
    }
    assert_eq!(terminal_records(&f.journal.lines(), id), original);
    assert!(m.advance(id).is_err());
    println!("review4:{case}");
}

// One explicit classification of EVERY enum variant. A future state fails to
// compile here until its reachable configurations have been assigned.
pub(super) fn rows_for_state(state: MissionState) -> Vec<(&'static str, MissionState)> {
    let names: &[&str] = match state {
        MissionState::Draft => &["draft"],
        MissionState::AwaitingConsent => &["consent"],
        MissionState::Approved => &["approved"],
        MissionState::Investigating => &["investigating"],
        MissionState::Building => &["building", "file-dispatch", "post-dispatch"],
        MissionState::Verifying => &["verifying"],
        MissionState::Explaining => &["explaining"],
        MissionState::AwaitingApproval => &["file-wait", "post-wait"],
        MissionState::Done
        | MissionState::Denied
        | MissionState::Stopped
        | MissionState::Blocked
        | MissionState::Failed => terminal_rows(state),
    };
    let reached = if state == MissionState::Failed {
        MissionState::Blocked
    } else {
        state
    };
    names.iter().map(|name| (*name, reached)).collect()
}
