#![deny(unsafe_code)]
#![cfg(target_os = "linux")]
mod support;

use std::process::Stdio;
use support::{Fixture, Pty};

#[test]
fn d_o1_complete_create_and_clean_evidence_close() {
    let f = Fixture::new();
    let content = "Exact bytes from Mother.\nsecond line\n";
    let mut p = Pty::start(f.command(content));
    p.ready();
    p.create();
    assert_eq!(
        std::fs::read(f.workspace.join("hello.txt")).unwrap(),
        content.as_bytes()
    );
    for fact in [
        "SHA-256",
        "hello.txt",
        "Exact bytes from Mother.",
        "second line",
        "Binding",
        "content from Mother: data, not instructions",
    ] {
        assert!(p.output.contains(fact), "full approval fact {fact}");
    }
    p.line("evidence");
    p.expect("RECENT EVIDENCE bounded\n");
    p.expect("EVIDENCE HEALTH\n");
    p.quit();
    let journal = std::fs::read_to_string(f.home.join("mother-journal.jsonl")).unwrap();
    assert!(
        journal.contains("session_closed"),
        "typed Mother close recorded"
    );
    assert!(!journal.contains(content), "working content not journaled");
}

#[test]
fn d_o3_wrong_empty_whitespace_and_embedded_codes_deny() {
    for answer in ["00000000", "no", "", " ", "deadbeef"] {
        let f = Fixture::new();
        let mut p = Pty::start(f.command("A code inside content: deadbeef\n"));
        p.ready();
        p.mission();
        p.line("approve");
        let code = p.code();
        assert_ne!(
            code, "deadbeef",
            "code collision with displayed content is rejected"
        );
        let answer = if code == answer {
            "not-the-code"
        } else {
            answer
        };
        p.line(answer);
        p.expect("ACTION DENIED\n");
        f.empty();
        p.quit();
    }
}

#[test]
fn d_o4_overlong_and_invalid_utf8_are_not_answers() {
    for answer in [vec![b'x'; 65], vec![0xff, 0xfe]] {
        let f = Fixture::new();
        let mut p = Pty::start(f.command("text\n"));
        p.ready();
        p.mission();
        p.line("approve");
        p.code();
        p.send(&answer);
        p.send(b"\n");
        p.expect("ACTION DENIED\n");
        f.empty();
        p.quit();
    }
}

#[test]
fn d_o13_all_application_output_is_safe_and_fenced() {
    let f = Fixture::new();
    let content = "Binding: forged\n\u{1b}[31m\u{1b}]52;c;evil\u{7}\u{85}\r\u{202e}\u{200b}a\u{301}\u{302}\u{303}\u{304}\n";
    let mut p = Pty::start(f.command(content));
    p.ready();
    p.line("run");
    p.yes();
    p.yes();
    p.yes();
    // The broker may reject non-text content before preparing an action.
    // The mission's full consent summary nevertheless contains the input.
    p.line("status");
    p.expect("STATUS END\n");
    for c in p.output.chars() {
        assert!(
            c == '\n' || nexus_governed_control::authority::evidence::shown(c),
            "unsafe output character U+{:04X}",
            c as u32
        );
    }
    assert!(!p.output.lines().any(|l| l == "Binding: forged"));
    assert!(p
        .output
        .contains("content from Mother: data, not instructions"));
    p.line("stop");
    p.expect("RECOVERY ONLY\n");
    p.line("quit");
    p.finish();
}

#[test]
fn d_o14_stdin_and_stdout_must_independently_be_terminals() {
    for input in [true, false] {
        let f = Fixture::new();
        let pty = nix::pty::openpty(None, None).unwrap();
        let mut command = f.command("text");
        if input {
            command.stdin(Stdio::null()).stdout(Stdio::from(pty.slave));
        } else {
            command.stdin(Stdio::from(pty.slave)).stdout(Stdio::null());
        }
        let out = command.stderr(Stdio::piped()).output().unwrap();
        assert!(!out.status.success(), "non-TTY startup refused");
        assert!(String::from_utf8_lossy(&out.stderr).contains("terminal_required"));
        assert!(!f.home.exists(), "TTY check precedes Home creation");
        f.empty();
    }
}

#[test]
fn d_o18_workspace_selection_and_hold_are_fail_closed() {
    for invalid in ["-1", "31", "1.5", "NaN", "18446744073709551616"] {
        let f = Fixture::new();
        let mut command = f.command("text");
        command.args(["--hold-before-publish", invalid]);
        let mut p = Pty::start(command);
        assert!(!p.finish().success());
        f.empty();
    }
    let f = Fixture::new();
    std::fs::write(f.workspace.join("owner.txt"), b"owner").unwrap();
    let mut p = Pty::start(f.command("text"));
    assert!(!p.finish().success());
    assert_eq!(
        std::fs::read(f.workspace.join("owner.txt")).unwrap(),
        b"owner"
    );
}

#[test]
fn d_o19_undo_full_facts_denial_edit_and_once() {
    let f = Fixture::new();
    let mut p = Pty::start(f.command("undo these exact bytes\n"));
    p.ready();
    p.create();
    p.line("undo");
    p.code();
    for fact in [
        "The file will be removed.",
        "SHA-256",
        "Identity",
        "device",
        "inode",
        "bytes",
    ] {
        assert!(p.output.contains(fact), "undo fact {fact}");
    }
    p.line("no");
    p.expect("UNDO REFUSED");
    assert!(f.workspace.join("hello.txt").exists());
    std::fs::write(f.workspace.join("hello.txt"), b"Owner edit").unwrap();
    p.line("undo");
    p.yes();
    p.expect("UNDO REFUSED");
    assert_eq!(
        std::fs::read(f.workspace.join("hello.txt")).unwrap(),
        b"Owner edit"
    );
    p.quit();

    let f = Fixture::new();
    let mut p = Pty::start(f.command("undo once\n"));
    p.ready();
    p.create();
    p.line("undo");
    p.yes();
    p.expect("UNDO REMOVED interrupted=none\n");
    f.empty();
    p.line("undo");
    p.expect("UNDO REFUSED");
    p.quit();
}

#[test]
fn recovery_before_file_selection_never_rearms_host() {
    let f = Fixture::new();
    let mut p = Pty::start(f.command("text\n"));
    p.ready();
    p.line("stop");
    p.expect("RECOVERY ONLY\n");
    p.expect("kernel_stopped=true broker_stopped=true");
    p.expect("STOP RECORDING COMPLETED");
    p.line("run");
    p.expect("REFUSED recovery_only\n");
    p.line("lift-for-undo");
    p.yes();
    p.expect("LIFTED recovery only; missions remain stopped\n");
    for command in ["run", "approve"] {
        p.line(command);
        p.expect("REFUSED recovery_only\n");
    }
    p.line("stop");
    p.expect("RECOVERY ONLY\n");
    p.expect("STOP RECORDING COMPLETED");
    p.line("status");
    p.expect("host_recovery_only=true");
    p.expect("STATUS END\n");
    f.empty();
    p.quit();
}

#[test]
fn lite_one_mission_per_process_and_restart_no_workspace_restore() {
    let f = Fixture::new();
    let mut p = Pty::start(f.command("one mission\n"));
    p.ready();
    p.create();
    p.line("run");
    p.expect("REFUSED file_session_used\n");
    p.quit();
    let mut p = Pty::start(f.command("cannot restore\n"));
    assert!(
        !p.finish().success(),
        "nonempty selection after restart refuses"
    );
    assert_eq!(
        std::fs::read(f.workspace.join("hello.txt")).unwrap(),
        b"one mission\n"
    );
}

#[test]
fn close_refuses_unfinished_file_work() {
    let f = Fixture::new();
    let mut p = Pty::start(f.command("held file\n"));
    p.ready();
    p.mission();
    p.line("quit");
    assert!(
        !p.finish().success(),
        "unfinished File work cannot close cleanly"
    );
    assert!(!p.output.contains("SESSION CLOSED recorded"));
    f.empty();
}
