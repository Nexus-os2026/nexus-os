//! The DevTools reader keeps only the events a session uses, within bounds.

use super::{Cdp, MAX_EVENTS, MAX_EVENT_BYTES};
use std::io::Write;
use std::time::{Duration, Instant};

/// Page console output and other events are dropped as they arrive; target
/// events are kept, at most `MAX_EVENTS` of them and `MAX_EVENT_BYTES`, the
/// oldest dropped first.
#[test]
fn only_target_events_are_kept_and_they_are_bounded() {
    let (_commands, command_writer) = std::io::pipe().unwrap();
    let (reply_reader, mut replies) = std::io::pipe().unwrap();
    let cdp = Cdp::new(command_writer, reply_reader);
    let noise = format!(
        "{{\"method\":\"Runtime.consoleAPICalled\",\"params\":{{\"text\":\"{}\"}}}}\0",
        "A".repeat(64 * 1024)
    );
    let big = format!(
        "{{\"method\":\"Target.targetInfoChanged\",\"params\":{{\"pad\":\"{}\"}}}}\0",
        "B".repeat(256 * 1024)
    );
    let writer = std::thread::spawn(move || {
        for _ in 0..64 {
            replies.write_all(noise.as_bytes()).unwrap();
        }
        for i in 0..8 {
            replies
                .write_all(
                    big.replace("\"pad\"", &format!("\"n\":{i},\"pad\""))
                        .as_bytes(),
                )
                .unwrap();
        }
        for i in 0..2000 {
            let small =
                format!("{{\"method\":\"Target.targetCreated\",\"params\":{{\"n\":{i}}}}}\0");
            replies.write_all(small.as_bytes()).unwrap();
        }
    });
    writer.join().unwrap();
    // The reader has caught up when the newest event is queued; then the
    // queue is taken once.
    let deadline = Instant::now() + Duration::from_secs(10);
    let caught_up = |cdp: &Cdp| {
        cdp.events
            .lock()
            .unwrap()
            .back()
            .is_some_and(|(_, event)| event["params"]["n"] == 1999)
    };
    while Instant::now() < deadline && !caught_up(&cdp) {
        std::thread::sleep(Duration::from_millis(20));
    }
    let events = cdp.drain_events();
    assert!(!events.is_empty());
    assert!(events
        .iter()
        .all(|e| e["method"].as_str().unwrap().starts_with("Target.")));
    assert!(events.len() <= MAX_EVENTS, "{}", events.len());
    let bytes: usize = events.iter().map(|e| e.to_string().len()).sum();
    assert!(bytes <= MAX_EVENT_BYTES, "{bytes}");
    assert_eq!(events.last().unwrap()["params"]["n"], 1999);
}
