//! The DevTools reader keeps only the events a session uses, within bounds.

use super::{Cdp, CdpError, MAX_EVENTS, MAX_EVENT_BYTES};
use crate::harness_tests::harness;
use serde_json::json;
use std::io::{Read, Write};
use std::time::{Duration, Instant};

/// Once the session's run is cancelled nothing more is sent to the
/// browser: the command is refused before it is written, so no further step
/// reaches the page after a stop.
#[test]
fn nothing_is_sent_after_a_stop() {
    let (mut commands, command_writer) = std::io::pipe().unwrap();
    let (reply_reader, _replies) = std::io::pipe().unwrap();
    let cdp = Cdp::new(command_writer, reply_reader);
    let h = harness();
    let token = h.control.authority().runs().check(h.run, &h.agent).unwrap();
    h.control.authority().cancel_run(h.run).unwrap();
    assert!(matches!(
        cdp.call(
            "Runtime.evaluate",
            json!({ "expression": "1" }),
            None,
            Duration::from_secs(1),
            &token
        ),
        Err(CdpError::Cancelled)
    ));
    drop(cdp);
    let mut sent = Vec::new();
    commands.read_to_end(&mut sent).unwrap();
    assert!(sent.is_empty(), "{}", String::from_utf8_lossy(&sent));
}

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
