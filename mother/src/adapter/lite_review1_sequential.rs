//! Review 1 HIGH 2: a new dispatch cannot inherit the prior publication.
use super::*;
use std::sync::mpsc;

#[test]
fn lite_review1_b_before_perform_never_settles_as_a() {
    if std::env::var_os("NEXUS_LITE_REVIEW1_SEQUENTIAL").is_none() {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "adapter::lite::review1_sequential::lite_review1_b_before_perform_never_settles_as_a", "--nocapture"])
            .env("NEXUS_LITE_REVIEW1_SEQUENTIAL", "1")
            .output().unwrap();
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
    let id = f.waiting(&m, "MKFILE:one.txt MKFILE:two.txt");
    let a = f.port.pending(id).unwrap();
    f.publish(id);
    let (entered_tx, entered_rx) = mpsc::sync_channel(1);
    let (release_tx, release_rx) = mpsc::sync_channel(1);
    f.port.before_perform.arm(0, move || {
        entered_tx.send(()).unwrap();
        release_rx.recv_timeout(Duration::from_secs(10)).unwrap();
    });
    let worker = {
        let m = m.clone();
        std::thread::spawn(move || m.advance(id))
    };
    entered_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    assert_eq!(
        f.port.acks_tried(),
        1,
        "A was recognized and acknowledged before B's perform"
    );
    let before = m.journal().unwrap();
    assert!(before.iter().any(|e| matches!(
        e.event,
        Event::Transition {
            trigger: Trigger::ActionApproved { effect: 0, .. }
        }
    )));
    assert!(before
        .iter()
        .any(|e| matches!(e.event, Event::EffectDispatched { effect: 1, .. })));
    let stopped = m.stop(id);
    release_tx.send(()).unwrap();
    assert_eq!(worker.join().unwrap(), Err(CoordinatorError::Stopped));
    let entries = m.journal().unwrap();
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
        vec![(1, None, SettlementOutcome::NotPublished)],
        "B was not proposed: its settlement must never borrow A's action or publication ({a:?})"
    );
    assert_eq!(stopped, Ok(MissionState::Stopped));
    assert_eq!(f.port.acks_tried(), 1, "A must not be acknowledged twice");
    assert_eq!(
        std::fs::read_to_string(f.ws.join("one.txt")).unwrap(),
        content_of("MKFILE", "one.txt").unwrap()
    );
    assert!(!f.ws.join("two.txt").exists());
}
