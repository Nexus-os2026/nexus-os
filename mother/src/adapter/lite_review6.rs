//! Review 6: a recognised File is already settled. Later lane failures keep
//! their exact ordinary terminal, while unresolved Files still settle safely.
use super::*;
use crate::adapter::lite_clock::LiteHome;
use crate::adapter::m2c_support::port_for;
use crate::adapter::{create_private, mother_agent};
use crate::journal::MemoryJournal;
use crate::lane::{Lane, LaneIdentity, LaneOutput, Verdict};
use crate::oracles::lite_support::{file, FilesOwner};
use crate::oracles::m2c_confirmers::answering;
use crate::oracles::{contract_with, Yes};
use nexus_governed_control::authority::clock::ManualClock;
use nexus_governed_control::governed::GrantRequest;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

const CASE: &str = "NEXUS_LITE_REVIEW6";
const TEST: &str = "coordinator::lite::review6::lite_failure_after_recognised_file";

struct FileThenFailure {
    role: LaneRole,
    failing: LaneRole,
    missing_verdict: bool,
    first: AtomicBool,
}
impl Lane for FileThenFailure {
    fn identity(&self) -> LaneIdentity {
        LaneIdentity {
            role: self.role,
            name: format!("review6-{}", self.role.as_str()),
            version: "1".into(),
        }
    }
    fn invoke(&self, _: &LaneInput) -> LaneOutput {
        // Sentinel runs before the researchers and again in Building. Let its
        // first call publish A so EVERY role can fail after A is recognised.
        if self.role == LaneRole::Sentinel && self.first.swap(false, Ordering::SeqCst) {
            return LaneOutput {
                text: "publish File A".into(),
                effects: vec![file("a.txt", "A\n")],
                verdict: Verdict::Continue,
            };
        }
        LaneOutput {
            text: "lane result after File A".into(),
            effects: vec![],
            verdict: if self.role == self.failing {
                if self.missing_verdict {
                    Verdict::Continue
                } else {
                    Verdict::Fail {
                        reason: "review6 failure detail".into(),
                    }
                }
            } else if self.role == LaneRole::Verifier {
                Verdict::Pass
            } else {
                Verdict::Continue
            },
        }
    }
}

fn replayed(lines: &[String], id: MissionId) -> (MissionState, Option<String>, Vec<Trigger>) {
    let mut missions = BTreeMap::new();
    let mut next = 1;
    let mut session = Session::Never;
    let mut terminals = Vec::new();
    for entry in verify(lines).unwrap() {
        check(&entry.event).unwrap();
        if entry.mission == id {
            if let Event::Transition { trigger } = &entry.event {
                let from = missions
                    .get(&id)
                    .map(|m: &Mission| m.state)
                    .unwrap_or(MissionState::Draft);
                if transition(from, trigger).unwrap().is_terminal() {
                    terminals.push(trigger.clone());
                }
            }
        }
        replay(&mut missions, &mut next, &mut session, &entry).unwrap();
    }
    let m = &missions[&id];
    (m.state, m.reason.clone(), terminals)
}

#[test]
fn lite_failure_after_recognised_file() {
    let Ok(case) = std::env::var(CASE) else {
        let cases = LaneRole::ALL.into_iter().map(LaneRole::as_str).chain([
            "missing-verdict",
            "unresolved-empty",
            "unresolved-published",
        ]);
        let mut failures = Vec::new();
        let mut count = 0;
        for case in cases {
            let output = std::process::Command::new(std::env::current_exe().unwrap())
                .args(["--exact", TEST, "--nocapture"])
                .env(CASE, case)
                .output()
                .unwrap();
            let stdout = String::from_utf8_lossy(&output.stdout);
            if !output.status.success() || !stdout.contains(&format!("review6:{case}")) {
                failures.push(format!(
                    "{case}:\n{stdout}\n{}",
                    String::from_utf8_lossy(&output.stderr)
                ));
            }
            count += 1;
        }
        assert!(failures.is_empty(), "{}", failures.join("\n"));
        println!("review6-matrix:{count} cases");
        return;
    };
    assert!(nexus_governed_control::files::make_non_dumpable());
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let home =
        LiteHome::open(&std::env::temp_dir().join(format!("nexus-lite-review6-{unique}"))).unwrap();
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
    let port = Arc::new(port_for(&home.control, true).with_workspace(info.name.clone()));
    let journal = MemoryJournal::default();
    let missing_verdict = case == "missing-verdict";
    let unresolved = case.starts_with("unresolved-");
    let failing = LaneRole::ALL
        .into_iter()
        .find(|r| r.as_str() == case)
        .unwrap_or(LaneRole::Verifier);
    let lanes = LaneRole::ALL
        .into_iter()
        .map(|role| {
            Arc::new(FileThenFailure {
                role,
                failing,
                missing_verdict,
                first: AtomicBool::new(true),
            }) as Arc<dyn Lane>
        })
        .collect();
    let m = Coordinator::open(
        journal.clone(),
        port.clone(),
        Lanes::new(lanes).unwrap(),
        Arc::new(ManualClock::default()),
        home.private(),
    )
    .unwrap()
    .into_file_session()
    .unwrap();
    let mut contract = contract_with("publish A then preserve the failure");
    contract.requested_folder = Some(info.name);
    contract.budget.max_model_calls = 50;
    let id = m.draft(contract).unwrap();
    m.submit(id).unwrap();
    assert_eq!(
        answering(&Yes, || m.request_consent(id)),
        Ok(MissionState::Approved)
    );
    assert_eq!(m.advance(id).unwrap().to, MissionState::Investigating);
    assert_eq!(m.advance(id).unwrap().to, MissionState::AwaitingApproval);
    let pending = port.pending(id).unwrap();
    let run = RunId::parse(&pending.run).unwrap();
    let commitment = CommitmentId::parse(&pending.commitment).unwrap();
    if case != "unresolved-empty" {
        home.control
            .authorize(commitment, &mother_agent(), run, &owner)
            .unwrap();
        home.control
            .execute(commitment, &mother_agent(), run)
            .unwrap();
        assert_eq!(std::fs::read(workspace.join("a.txt")).unwrap(), b"A\n");
    }
    let trigger = if missing_verdict {
        Trigger::VerifyFail {
            reason: VerifyFailReason::VerifierGaveNoVerdict,
        }
    } else if failing == LaneRole::Verifier {
        Trigger::VerifyFail {
            reason: VerifyFailReason::VerifierFailed,
        }
    } else {
        Trigger::Fail {
            reason: FailReason::LaneFail(failing),
        }
    };
    if unresolved {
        // Fault control: ordinary lane execution cannot overtake its File wait,
        // but an internal terminal must still settle any unresolved obligation.
        m.call(|c| c.end_with(id, trigger)).unwrap();
        let outcome = if case == "unresolved-empty" {
            assert!(!workspace.join("a.txt").exists());
            SettlementOutcome::NotPublished
        } else {
            SettlementOutcome::Published {
                recovery_required: false,
                interrupted: vec![],
            }
        };
        let end = SettleCause::Ended(BlockReason::PhaseInterrupted).end(&outcome);
        let expected = Trigger::Settled {
            effect: 0,
            action: Some(pending),
            outcome,
            end,
        };
        assert_eq!(replayed(&journal.lines(), id).2, vec![expected]);
    } else {
        for _ in 0..6 {
            if m.state(id).unwrap().unwrap().is_terminal() {
                break;
            }
            m.advance(id).unwrap();
        }
        let lines = journal.lines();
        let entries = verify(&lines).unwrap();
        assert_eq!(
            entries
                .iter()
                .filter(|e| matches!(
                    e.event,
                    Event::Transition {
                        trigger: Trigger::ActionApproved { effect: 0, .. }
                    }
                ))
                .count(),
            1,
            "{case}: A must already be truthfully recognised"
        );
        assert!(
            !entries
                .iter()
                .any(|e| matches!(e.event, Event::SettlementStarted { .. })),
            "{case}: recognised A needs no second settlement"
        );
        let code = if missing_verdict {
            "verifier_gave_no_verdict".to_owned()
        } else if failing == LaneRole::Verifier {
            "verifier_fail".to_owned()
        } else {
            format!("lane_fail:{}", failing.as_str())
        };
        assert_eq!(
            (
                m.state(id).unwrap(),
                m.reason(id).unwrap(),
                replayed(&lines, id)
            ),
            (
                Some(MissionState::Failed),
                Some(code.clone()),
                (MissionState::Failed, Some(code), vec![trigger])
            ),
            "{case}: exact Failed state and code must survive live recording and replay"
        );
    }
    let before = journal.lines();
    let terminal = replayed(&before, id);
    let invocations = m.invocations_for_test();
    assert_eq!(m.stop(id).unwrap(), terminal.0);
    assert!(m.advance(id).is_err());
    assert_eq!(
        journal.lines(),
        before,
        "terminal touches cannot replace the failure"
    );
    assert_eq!(m.invocations_for_test(), invocations);
    m.close().unwrap();
    assert_eq!(replayed(&journal.lines(), id), terminal);
    println!("review6:{case}");
}
