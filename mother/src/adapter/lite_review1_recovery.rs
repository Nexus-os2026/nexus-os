//! Review 1 MEDIUM: native directory-sync failure through broker to journal.
//! The fault is child-process-local, one-shot and restricted to the fixture workspace.
//! All other fsync calls use libc; no publication class is fabricated.
use super::*;
use crate::adapter::lite_clock::{past_the_start, LiteHome};
use crate::evidence::PublicationCause;
#[test]
fn lite_review1_native_recovery_required_reaches_journal() {
    let Ok(case) = std::env::var("NEXUS_LITE_REVIEW1_RECOVERY") else {
        let fault_dir =
            std::env::temp_dir().join(format!("lite-review1-fsync-{}", std::process::id()));
        create_private(&fault_dir).unwrap();
        let source = fault_dir.join("fault.c");
        let library = fault_dir.join("fault.so");
        std::fs::write(&source, include_str!("lite_review1_fsync.c")).unwrap();
        let compiled = std::process::Command::new("cc")
            .args(["-shared", "-fPIC", "-Wall", "-Wextra", "-Werror", "-o"])
            .arg(&library)
            .arg(&source)
            .arg("-ldl")
            .output()
            .unwrap();
        assert!(
            compiled.status.success(),
            "{}",
            String::from_utf8_lossy(&compiled.stderr)
        );
        for case in ["recovery", "recovery-interrupted"] {
            let output = std::process::Command::new(std::env::current_exe().unwrap())
                .args(["--exact", "adapter::lite::review1_recovery::lite_review1_native_recovery_required_reaches_journal", "--nocapture"])
                .env("NEXUS_LITE_REVIEW1_RECOVERY", case)
                .env("LD_PRELOAD", &library)
                .output().unwrap();
            assert!(
                output.status.success(),
                "{case}: {}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            assert!(String::from_utf8_lossy(&output.stdout)
                .contains(&format!("native-recovery:{case}")));
        }
        return;
    };
    let dir = std::env::temp_dir().join(format!(
        "lite-review1-recovery-{}-{}",
        std::process::id(),
        case
    ));
    let home = LiteHome::open(&dir).unwrap();
    let ws = dir.join("workspace");
    create_private(&ws).unwrap();
    let info = home.control.register_workspace(&ws, &[]).unwrap();
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
    let m = Coordinator::open(
        journal.clone(),
        port.clone(),
        Lanes::new(files_lanes()).unwrap(),
        Arc::new(SystemClock::default()),
        home.private(),
    )
    .unwrap()
    .into_file_session()
    .unwrap();
    let mut contract = contract_with("MKFILE:one.txt MKFILE:two.txt");
    contract.requested_folder = Some(info.name);
    let id = m.draft(contract).unwrap();
    m.submit(id).unwrap();
    answering(&Yes, || m.request_consent(id)).unwrap();
    for _ in 0..3 {
        m.advance(id).unwrap();
    }
    let pending = port.pending(id).unwrap();
    let run = RunId::parse(&pending.run).unwrap();
    let commitment = CommitmentId::parse(&pending.commitment).unwrap();
    home.control
        .authorize(commitment, &mother_agent(), run, &owner)
        .unwrap();
    let interrupted = case == "recovery-interrupted";
    if interrupted {
        let broker = home.control.clone();
        home.clock.arm(past_the_start(&home, run), move || {
            broker.cancel_run(run).unwrap()
        });
    }
    let receipt = dir.join("sync-failed");
    std::env::set_var("NEXUS_LITE_SYNC_FAILURE", &ws);
    std::env::set_var("NEXUS_LITE_SYNC_RECEIPT", &receipt);
    let output = home
        .control
        .execute(commitment, &mother_agent(), run)
        .unwrap();
    std::env::remove_var("NEXUS_LITE_SYNC_FAILURE");
    std::env::remove_var("NEXUS_LITE_SYNC_RECEIPT");
    assert_eq!(
        std::fs::read_to_string(receipt).unwrap(),
        "workspace-fsync:EIO\n",
        "the native workspace sync failed exactly once"
    );
    assert!(
        output
            .meta
            .contains(&("reason".into(), "recovery_required".into())),
        "native publication reports recovery: {:?}",
        output.meta
    );
    assert_eq!(
        output
            .meta
            .contains(&("interrupted".into(), "cancelled".into())),
        interrupted
    );
    assert!(!home.clock.disarm(), "any interruption hook fired");
    assert_eq!(
        std::fs::read_to_string(ws.join("one.txt")).unwrap(),
        content_of("MKFILE", "one.txt").unwrap()
    );
    let result = m.advance(id).unwrap();
    assert_eq!(
        result.to,
        MissionState::Blocked,
        "Mother must block a native recovery-required publication"
    );
    assert_eq!(
        m.reason(id).unwrap().as_deref(),
        Some("file_publication_recovery_required")
    );
    let entries = crate::journal::verify(&journal.lines()).unwrap();
    let terminals: Vec<_> = entries
        .iter()
        .filter_map(|e| match &e.event {
            Event::Transition {
                trigger:
                    Trigger::Settled {
                        effect,
                        action,
                        outcome,
                        ..
                    },
            } => Some((*effect, action.clone(), outcome.clone())),
            _ => None,
        })
        .collect();
    assert_eq!(
        terminals,
        vec![(
            0,
            Some(pending),
            SettlementOutcome::Published {
                recovery_required: true,
                interrupted: if interrupted {
                    vec![PublicationCause::Cancelled]
                } else {
                    vec![]
                },
            }
        )],
        "journal preserves native recovery and interruption together"
    );
    assert!(
        !entries.iter().any(|e| matches!(
            e.event,
            Event::Transition {
                trigger: Trigger::ActionApproved { .. }
            } | Event::EffectDispatched { effect: 1, .. }
        )),
        "no ordinary success or remaining dispatch"
    );
    assert_eq!(port.acks_tried(), 1);
    assert!(!ws.join("two.txt").exists());
    println!("native-recovery:{case}");
}
