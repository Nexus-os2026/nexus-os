//! Review 1 HIGH 1: Stop wins after perform's last halt check.
use super::*;
use std::sync::mpsc;

#[test]
fn lite_review1_dispatch_after_stop_replays() {
    if let Some(path) = std::env::var_os("NEXUS_LITE_REVIEW1_REOPEN") {
        let home = Home::open(std::path::Path::new(&path)).unwrap();
        let reopened = Coordinator::open(
            home.journal().unwrap(),
            Arc::new(port_for(&home.control, true)),
            Lanes::new(files_lanes()).unwrap(),
            Arc::new(SystemClock::default()),
            home.private(),
        );
        assert!(
            reopened.is_ok(),
            "replay after Stop must succeed: {:?}",
            reopened.err()
        );
        return;
    }
    if std::env::var_os("NEXUS_LITE_REVIEW1_DISPATCH").is_none() {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "adapter::lite::review1_dispatch::lite_review1_dispatch_after_stop_replays",
                "--nocapture",
            ])
            .env("NEXUS_LITE_REVIEW1_DISPATCH", "1")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        return;
    }
    let f = Fx::new();
    let m = Arc::new(f.file());
    let id = m.draft(f.contract("MKFILE:one.txt")).unwrap();
    m.submit(id).unwrap();
    answering(&Yes, || m.request_consent(id)).unwrap();
    m.advance(id).unwrap();
    m.advance(id).unwrap();
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
    assert_eq!(m.stop(id), Ok(MissionState::Stopped));
    let stopped = f.journal.lines();
    release_tx.send(()).unwrap();
    assert_eq!(worker.join().unwrap(), Err(CoordinatorError::Stopped));
    // A fresh broker models restart; surviving-broker File replacement remains forbidden.
    std::fs::write(f.home.journal_path(), f.journal.lines().join("\n") + "\n").unwrap();
    let dir = f.home.dir.clone();
    let lines = f.journal.lines();
    let workspace = f.ws.clone();
    drop(m);
    drop(f);
    let reopen = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "adapter::lite::review1_dispatch::lite_review1_dispatch_after_stop_replays",
            "--nocapture",
        ])
        .env("NEXUS_LITE_REVIEW1_REOPEN", &dir)
        .output()
        .unwrap();
    assert!(
        reopen.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&reopen.stdout),
        String::from_utf8_lossy(&reopen.stderr)
    );
    let entries = crate::journal::verify(&lines).unwrap();
    let terminal = entries
        .iter()
        .position(|e| {
            matches!(
                e.event,
                Event::Transition {
                    trigger: Trigger::Settled { .. }
                }
            )
        })
        .unwrap();
    assert!(
        !entries[terminal + 1..]
            .iter()
            .any(|e| matches!(e.event, Event::EffectSettled { .. })),
        "no EffectSettled after terminal Settled"
    );
    assert_eq!(&lines[..stopped.len()], stopped.as_slice());
    assert!(!workspace.join("one.txt").exists());
}
