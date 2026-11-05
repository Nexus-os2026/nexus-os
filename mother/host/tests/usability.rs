//! Owner-approved U1–U5: literal expectations registered before implementation.
#![deny(unsafe_code)]
#![cfg(target_os = "linux")]
mod support;
use support::faults::Fault;
use support::{Fixture, Pty};

const FRESH: &str = "Fresh start: no earlier session in this home; nothing to recover.\n";
const RECOVERY: &str = "Recovery report only. Mother has not removed files or staging, restored workspace permission, or restored undo. Some unfinished work may be known only from incomplete evidence.\n";
const COMMANDS: &str =
    "Valid commands: run, approve, status, evidence, detail, stop, undo, cleanup, quit\n";

#[test]
fn ux_u1_visible_command_and_answer_echo_without_kernel_echo() {
    use nix::sys::termios::{tcgetattr, LocalFlags};
    let f = Fixture::new();
    let (mut p, observer, _) = Pty::start_observing(f.command("echo oracle"));
    p.ready();
    assert!(!tcgetattr(&observer)
        .unwrap()
        .local_flags
        .intersects(LocalFlags::ECHO | LocalFlags::ICANON));
    p.send(b"run");
    p.expect("run"); // Visible before Enter, so backend output cannot satisfy it.
    p.send(b"\n");
    let code = p.code();
    p.send(code.as_bytes());
    p.expect(&code); // After INPUT READY; no Enter yet.
    p.send(b"\n");
    p.code();
    p.line("no");
    p.expect("OPERATION END\n");
    f.empty();
    p.quit();
}

#[test]
fn ux_u1_both_backspaces_and_ctrl_u_correct_an_accepted_code() {
    for edit in [b"x\x7f".as_slice(), b"x\x08", b"mistake\x15"] {
        let f = Fixture::new();
        let mut p = Pty::start(f.command("editing oracle"));
        p.ready();
        p.mission();
        p.line("approve");
        let code = p.code();
        p.send(edit);
        p.send(code.as_bytes());
        p.send(b"\n");
        p.expect("MISSION done\n");
        p.expect("OPERATION END\n");
        assert!(
            p.wire_output.contains("\x08 \x08"),
            "program erases visibly"
        );
        assert_eq!(
            std::fs::read(f.workspace.join("hello.txt")).unwrap(),
            b"editing oracle"
        );
        p.quit();
    }
}

#[test]
fn ux_u1_empty_commands_and_answers_keep_waiting_then_accept() {
    let f = Fixture::new();
    let mut p = Pty::start(f.command("empty line oracle"));
    p.ready();
    p.send(b"\n\r\r\n");
    p.mission();
    p.line("approve");
    let code = p.code();
    p.send(b"\n\r\r\n");
    p.line(&code);
    p.expect("MISSION done\n");
    p.expect("OPERATION END\n");
    assert!(!p.output.contains("unknown_command"));
    assert!(!p.output.contains("ProtocolFailed"));
    assert_eq!(p.output.matches("Answer code:").count(), 4);
    assert_eq!(
        std::fs::read(f.workspace.join("hello.txt")).unwrap(),
        b"empty line oracle"
    );
    p.quit();
}

#[test]
fn ux_u1_nonempty_wrong_space_and_overlong_still_fail_closed() {
    for answer in ["wrong".to_string(), " ".into(), "x".repeat(65)] {
        let f = Fixture::new();
        let mut p = Pty::start(f.command("must not publish"));
        p.ready();
        p.mission();
        p.line("approve");
        p.code();
        p.line(&answer);
        assert!(!p.finish().success());
        assert!(p.output.contains("ProtocolFailed"));
        assert!(!p.output.contains("ACTION DENIED"));
        f.empty();
    }
}

#[test]
fn ux_u1_typed_ahead_complete_partial_and_cleared_partial_stay_invalid() {
    for tail in ["complete", "partial", "cleared"] {
        let f = Fixture::new();
        let fault = Fault::new(&f, "display");
        let mut command = f.command("no stale approval");
        fault.attach(&f, &mut command);
        let mut p = Pty::start(command);
        p.ready();
        p.mission();
        fault.arm();
        p.line("approve");
        let receipt = fault.reached();
        let text = String::from_utf8(receipt.bytes.clone()).unwrap();
        let code = text
            .split("Answer code: ")
            .nth(1)
            .unwrap()
            .lines()
            .next()
            .unwrap();
        if tail == "complete" {
            p.line(code);
        } else {
            p.send(&code.as_bytes()[..4]);
        }
        receipt.release();
        assert_eq!(p.code(), code);
        match tail {
            "partial" => {
                p.send(&code.as_bytes()[4..]);
                p.send(b"\n");
            }
            "cleared" => {
                p.send(b"\x15");
                p.line(code);
            }
            _ => {}
        }
        p.line("no");
        p.expect("ACTION DENIED\n");
        f.empty();
        p.quit();
    }
}

#[test]
fn ux_u1_stop_during_typing_does_not_wait_for_blocked_echo() {
    let f = Fixture::new();
    let fault = Fault::new(&f, "echo-hold");
    let mut command = f.command("stop must win");
    fault.attach(&f, &mut command);
    let mut p = Pty::start(command);
    p.ready();
    p.mission();
    p.line("approve");
    p.code();
    fault.arm();
    p.send(b"mistake");
    let echo = fault.reached();
    // Renderer is held inside write. Reader must still clear and ingress Stop.
    p.send(b"\x15stop\n");
    let stopped = fault.reached_kind("stop-recording");
    f.empty();
    stopped.release();
    echo.release();
    p.expect("STOP RECORDING COMPLETED");
    p.line("quit");
    p.finish();
    assert!(!p.output.contains("ACTION FINISHED"));
    f.empty();
}

#[test]
fn ux_u2_success_reports_mission_done_once_and_keeps_notice() {
    let f = Fixture::new();
    let mut p = Pty::start(f.command("once"));
    p.ready();
    p.create();
    p.expect("OPERATION END\n");
    assert_eq!(p.output.matches("MISSION done\n").count(), 1);
    assert!(p.output.contains("ACTION FINISHED"));
    assert!(p.output.contains("TERMINAL first_intent="));
    p.quit();
}

#[test]
fn ux_u3_fresh_empty_home_and_clean_or_unclean_history_are_distinct() {
    for existing_empty_home in [false, true] {
        let f = Fixture::new();
        if existing_empty_home {
            nexus_mother::adapter::create_private(&f.home).unwrap();
        }
        let mut p = Pty::start(f.command("fresh home"));
        p.ready();
        assert!(p.output.contains(FRESH), "{}", p.output);
        assert!(!p.output.contains(RECOVERY));
        assert!(!p
            .output
            .contains("RECOVERY REPORT only; no sweep or workspace restoration"));
        p.quit();
        let mut next = Pty::start(f.command("clean history"));
        next.ready();
        assert!(next.output.contains(RECOVERY));
        assert!(next
            .output
            .contains("RECOVERY REPORT only; no sweep or workspace restoration\n"));
        assert!(!next.output.contains(FRESH));
        next.child.kill().unwrap();
        next.child.wait().unwrap();
        let mut unclean = Pty::start(f.command("unclean history"));
        assert!(!unclean.finish().success());
        assert!(unclean.output.contains(RECOVERY));
        assert!(!unclean.output.contains(FRESH));
        f.empty();
    }
}

fn result(p: &mut Pty, expected: &str, kind: &str) {
    let seen = p.expect("OPERATION END\n");
    assert_eq!(seen.matches("RESULT: ").count(), 1, "{seen}");
    let at = seen
        .find(expected)
        .unwrap_or_else(|| panic!("missing {expected}: {seen}"));
    assert!(at < seen.find(&format!("OPERATION {kind} ")).unwrap());
}

#[test]
fn ux_u4_run_approve_undo_cleanup_and_quit_have_truthful_owner_results() {
    let f = Fixture::new();
    let mut p = Pty::start(f.command("owner result"));
    p.ready();
    p.mission();
    result(
        &mut p,
        "RESULT: waiting for your approval of hello.txt (type approve).\n",
        "Run",
    );
    p.line("approve");
    p.yes();
    result(
        &mut p,
        "RESULT: hello.txt was created, recorded and verified.\n",
        "Approve",
    );
    assert_eq!(
        std::fs::read(f.workspace.join("hello.txt")).unwrap(),
        b"owner result"
    );
    p.line("undo");
    p.yes();
    result(
        &mut p,
        "RESULT: the approved file was removed; restore completed, but other recording status is uncertain.\n",
        "Undo",
    );
    f.empty();
    p.line("cleanup");
    result(
        &mut p,
        "RESULT: cleanup was requested; resource and recording obligations are reported below.\n",
        "Cleanup",
    );
    p.quit();
    // Review-1 H2 amendment: Owner requires wording limited to established
    // facts before terminal.finish and seal.finish_delivery complete.
    assert!(p
        .output
        .contains("RESULT: closing; the final close status follows.\nSESSION CLOSED recorded\n"));
}

#[test]
fn ux_u4_decline_and_protocol_failure_do_not_claim_approval() {
    for answer in ["no", "wrong"] {
        let f = Fixture::new();
        let mut p = Pty::start(f.command("refused result"));
        p.ready();
        p.mission();
        p.expect("OPERATION END\n");
        p.line("approve");
        p.code();
        p.line(answer);
        result(
            &mut p,
            "RESULT: refused - nothing was created.\n",
            "Approve",
        );
        f.empty();
        if answer == "no" {
            p.quit();
        } else {
            assert!(!p.finish().success());
            assert!(p.output.contains(
                "RESULT: close is uncertain; the full clean-close protocol did not complete.\n"
            ));
        }
    }
}

#[test]
fn ux_u4_native_publication_uncertainty_never_claims_verified_success() {
    let f = Fixture::new();
    let fault = Fault::new(&f, "published-dir-sync");
    let mut command = f.command("uncertain durability");
    fault.attach(&f, &mut command);
    let mut p = Pty::start(command);
    p.ready();
    p.mission();
    p.expect("OPERATION END\n");
    fault.arm();
    p.line("approve");
    p.yes();
    let receipt = fault.reached();
    assert!(f.workspace.join("hello.txt").exists());
    receipt.release();
    result(
        &mut p,
        "RESULT: the file outcome is uncertain; recovery or recording obligations remain.\n",
        "Approve",
    );
    assert!(!p.output.contains("was created, recorded and verified"));
    p.line("quit");
    assert!(!p.finish().success());
}

#[test]
fn ux_u5_unknown_command_lists_exact_available_commands() {
    let f = Fixture::new();
    let mut p = Pty::start(f.command("help oracle"));
    p.ready();
    p.line("help");
    p.expect(&format!("REFUSED unknown_command\n{COMMANDS}"));
    p.quit();
    f.empty();
}
