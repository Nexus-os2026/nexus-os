//! Review-1 G/F10: actual terminal answers and verified primary evidence agree.
#![deny(unsafe_code)]
#![cfg(target_os = "linux")]
mod support;

use std::process::Command;
use support::{Fixture, Pty};

fn dialogue(stage: usize, input: &str, answer: &str, reason: &str) {
    let f = Fixture::new();
    let mut p = Pty::start(f.command("confirmation truth control"));
    p.ready();
    p.line("run");
    for _ in 0..stage {
        p.yes();
    }
    p.code();
    p.line(input);
    if matches!(input, "stop" | "no") {
        p.expect("ANSWERS ");
        p.line("quit");
    }
    let status = p.finish();
    if input == "not-an-answer" {
        assert!(
            !status.success(),
            "protocol failure cannot become a clean exit"
        );
    }
    f.empty();
    let question = ["grant", "door", "mission"][stage];
    assert!(
        p.output.contains(&format!("{question}={answer}")),
        "the real terminal retains the exact answer: {}",
        p.output
    );
    let output = Command::new("python3")
        .env("PYTHONDONTWRITEBYTECODE", "1")
        .arg(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/support/session_answer_evidence.py"
        ))
        .arg(f.home.join("broker/p3-evidence/evidence.sqlite3"))
        .arg(["grant_declined", "agent_door_declined", "mission_declined"][stage])
        .arg(reason)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "independent primary evidence verification: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let receipt = String::from_utf8(output.stdout).unwrap();
    assert!(receipt.contains("CONFIRMATION EVIDENCE VERIFIED"));
    println!("G/F10 {question} input={input} terminal={answer} {receipt}");
}

macro_rules! cases {
    ($stop:ident, $quit:ident, $protocol:ident, $no:ident, $stage:expr) => {
        #[test]
        fn $stop() {
            dialogue($stage, "stop", "Invalidated", "confirmation_invalidated");
        }
        #[test]
        fn $quit() {
            dialogue($stage, "quit", "Invalidated", "confirmation_invalidated");
        }
        #[test]
        fn $protocol() {
            dialogue(
                $stage,
                "not-an-answer",
                "ProtocolFailed",
                "confirmation_protocol_failed",
            );
        }
        #[test]
        fn $no() {
            dialogue($stage, "no", "Declined", "none");
        }
    };
}
cases!(
    f10_grant_stop,
    f10_grant_quit,
    f10_grant_protocol,
    f10_grant_explicit_no,
    0
);
cases!(
    f10_door_stop,
    f10_door_quit,
    f10_door_protocol,
    f10_door_explicit_no,
    1
);
cases!(
    f10_mission_stop,
    f10_mission_quit,
    f10_mission_protocol,
    f10_mission_explicit_no,
    2
);
