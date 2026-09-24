use super::*;
use serde_json::json;
use std::sync::mpsc;
use std::time::Duration;

fn bounded(test: impl FnOnce() + Send + 'static) {
    let (done, wait) = mpsc::channel();
    std::thread::spawn(move || {
        test();
        done.send(()).unwrap();
    });
    wait.recv_timeout(Duration::from_secs(5))
        .expect("audit writer failed or deadlocked");
}

#[test]
fn shared_writer_preserves_interleaved_events_and_hash_chain() {
    bounded(|| {
        let shared = Arc::new(Mutex::new(AuditTrail::new()));
        let mut writer = shared.clone();
        let first = writer
            .append_event(Uuid::nil(), EventType::UserAction, json!(0))
            .unwrap();
        let (done, wait) = mpsc::channel();
        let callback_audit = shared.clone();
        let callback = std::thread::spawn(move || {
            // Same shape as a secrets/Warden callback: a separate owner appends
            // to the canonical trail between two execution-owned appends.
            let id = callback_audit
                .lock()
                .unwrap()
                .append_event(Uuid::nil(), EventType::UserAction, json!(1))
                .unwrap();
            done.send(id).unwrap();
        });
        let second = wait.recv_timeout(Duration::from_secs(2)).unwrap();
        let third = writer
            .append_event(Uuid::nil(), EventType::UserAction, json!(2))
            .unwrap();
        callback.join().unwrap();
        let trail = shared.lock().unwrap();
        assert!(trail.verify_integrity());
        assert_eq!(
            trail
                .events()
                .iter()
                .map(|e| e.event_id)
                .collect::<Vec<_>>(),
            vec![first, second, third]
        );
        assert_eq!(
            trail
                .events()
                .iter()
                .map(|e| e.payload.clone())
                .collect::<Vec<_>>(),
            vec![json!(0), json!(1), json!(2)]
        );
    });
}

#[test]
fn shared_writer_returns_append_failure_synchronously() {
    bounded(|| {
        struct Sink;
        impl BlockBatchSink for Sink {
            fn seal_batch(&mut self, _: Vec<AuditEvent>) {}
        }
        let mut trail = AuditTrail::new();
        trail.enable_distributed_audit(BatcherConfig::default(), Box::new(Sink));
        let batcher = trail.batcher.inner.as_ref().unwrap().clone();
        let poison = std::thread::spawn(move || {
            let _guard = batcher.lock().unwrap();
            panic!("poison batcher to exercise existing fail-closed error");
        });
        assert!(poison.join().is_err());
        let mut writer = Arc::new(Mutex::new(trail));
        assert_eq!(
            writer.append_event(Uuid::nil(), EventType::UserAction, json!({})),
            Err(AuditError::BatcherPoisoned)
        );
        assert!(
            writer.try_lock().is_ok(),
            "writer leaked the audit guard on error"
        );
    });
}
