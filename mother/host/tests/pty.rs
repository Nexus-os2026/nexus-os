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

#[test]
fn d_o21_privacy_canaries_absent_from_durable_stores_and_sqlite_values() {
    use support::privacy::{hits_in, needles, scan_home, Canaries};
    let cs = Canaries {
        c: format!("NXCANARY-{}", uuid::Uuid::new_v4().simple()),
        cl: format!(
            "nxsecret_{}",
            &uuid::Uuid::new_v4().simple().to_string()[..16]
        ),
    };
    // Nonvacuous scanner controls cover every encoding and fragment.
    for (_, c) in cs.both() {
        for (_, needle) in needles(c) {
            assert!(!hits_in(needle.as_bytes(), "plant", &cs).is_empty());
        }
    }
    let f = Fixture::new();
    let mut command = std::process::Command::new(support::binary());
    command
        .arg("--home")
        .arg(&f.home)
        .arg("--workspace")
        .arg(&f.workspace)
        .args([
            "--name",
            &format!("{}.txt", cs.cl),
            "--text",
            &format!("{} {}\n", cs.c, cs.cl),
        ]);
    let mut p = Pty::start(command);
    p.ready();
    p.create();
    assert!(
        p.output.contains(&cs.c),
        "Owner saw the real canary content"
    );
    let echo_start = p.output.len();
    p.line(&cs.c);
    p.expect("REFUSED unknown_command\n");
    assert!(hits_in(p.output[echo_start..].as_bytes(), "terminal echo", &cs).is_empty());
    let hits = scan_home(
        &f.home,
        &format!("{}.txt", cs.cl),
        format!("{} {}\n", cs.c, cs.cl).as_bytes(),
        &cs,
    );
    assert!(hits.is_empty(), "live main/WAL/SHM privacy: {hits:?}");
    p.quit();
    let hits = scan_home(
        &f.home,
        &format!("{}.txt", cs.cl),
        format!("{} {}\n", cs.c, cs.cl).as_bytes(),
        &cs,
    );
    assert!(hits.is_empty(), "closed-store privacy: {hits:?}");
    // Read SQLite's logical values too, independent of page encoding.
    let output = std::process::Command::new("python3").arg("-c").arg(r#"
import pathlib, sqlite3, sys
root = pathlib.Path(sys.argv[1])
count = 0
for p in root.rglob('*'):
    if p.is_file() and 'private' not in p.relative_to(root).parts and p.read_bytes().startswith(b'SQLite format 3'):
        con = sqlite3.connect(p.as_uri() + '?mode=ro', uri=True)
        print('\n'.join(con.iterdump()))
        con.close(); count += 1
assert count >= 2, 'ledger and primary evidence both read'
"#).arg(&f.home).output().unwrap();
    assert!(
        output.status.success(),
        "logical store scan: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(hits_in(&output.stdout, "logical SQLite", &cs).is_empty());
}

#[test]
fn d_o18_home_reserved_and_symlink_selection_refused_without_sweep() {
    for linked in [false, true] {
        let f = Fixture::new();
        let selected = if linked {
            f.root.join("link")
        } else {
            f.home.join("selected")
        };
        if linked {
            std::os::unix::fs::symlink(&f.workspace, &selected).unwrap();
        } else {
            nexus_mother::adapter::create_private(&f.home).unwrap();
            std::fs::create_dir(&selected).unwrap();
        }
        let mut command = std::process::Command::new(support::binary());
        command
            .arg("--home")
            .arg(&f.home)
            .arg("--workspace")
            .arg(&selected)
            .args(["--name", "hello.txt", "--text", "data"]);
        let mut p = Pty::start(command);
        assert!(!p.finish().success());
        assert!(p.output.contains("RECOVERY REPORT only; no sweep"));
        assert!(p.output.contains("workspace_refused"));
        assert!(selected.exists(), "selection was not swept");
        f.empty();
    }
}

#[test]
fn d_o4_eof_during_approval_never_approves() {
    let f = Fixture::new();
    let mut p = Pty::start(f.command("must remain absent\n"));
    p.ready();
    p.mission();
    p.line("approve");
    p.code();
    p.send(&[4]);
    assert!(!p.finish().success());
    f.empty();
    assert!(!p.output.contains("ACTION FINISHED evidence:"));
}

#[test]
fn recovery_repeated_stop_invalidates_lift_and_undo_consents() {
    let f = Fixture::new();
    let mut p = Pty::start(f.command("recover once\n"));
    p.ready();
    p.create();
    p.line("stop");
    p.expect("STOP RECORDING COMPLETED");
    p.line("lift-for-undo");
    let old = p.code();
    p.line("stop");
    p.expect("LIFT REFUSED");
    p.expect("STOP RECORDING COMPLETED");
    p.line("lift-for-undo");
    let fresh = p.code();
    assert_ne!(fresh, old);
    p.line(&fresh);
    p.expect("LIFTED recovery only; missions remain stopped\n");
    p.line("undo");
    let old = p.code();
    p.line("stop");
    p.expect("UNDO REFUSED");
    p.expect("STOP RECORDING COMPLETED");
    assert!(f.workspace.join("hello.txt").exists());
    p.line("lift-for-undo");
    p.yes();
    p.expect("LIFTED recovery only; missions remain stopped\n");
    p.line("undo");
    let fresh = p.code();
    assert_ne!(fresh, old);
    p.line(&fresh);
    p.expect("UNDO REMOVED interrupted=none\n");
    f.empty();
    p.quit();
}

#[test]
fn d_o21_real_leaks_outside_the_exact_private_payload_are_detected() {
    use support::privacy::{scan_home, Canaries};
    let f = Fixture::new();
    let cs = Canaries {
        c: format!("NXCANARY-{}", uuid::Uuid::new_v4().simple()),
        cl: format!("nxsecret_{}", uuid::Uuid::new_v4().simple()),
    };
    let staging = f
        .home
        .join("broker/files/coding-runs")
        .join(uuid::Uuid::new_v4().to_string());
    std::fs::create_dir_all(&staging).unwrap();
    std::fs::write(staging.join("candidate.txt"), &cs.c).unwrap();
    assert!(scan_home(&f.home, "candidate.txt", cs.c.as_bytes(), &cs).is_empty());
    for relative in [
        "log.txt",
        "mother-journal.jsonl",
        "ledger.sqlite3-wal",
        "evidence.sqlite3-shm",
        "broker/files/leak.txt",
    ] {
        let path = f.home.join(relative);
        std::fs::write(&path, &cs.c).unwrap();
        assert!(
            !scan_home(&f.home, "candidate.txt", cs.c.as_bytes(), &cs).is_empty(),
            "real leak: {relative}"
        );
        std::fs::remove_file(path).unwrap();
    }
    std::fs::write(staging.join("metadata"), &cs.cl).unwrap();
    assert!(
        !scan_home(&f.home, "candidate.txt", cs.c.as_bytes(), &cs).is_empty(),
        "staging metadata is not private payload"
    );
}

#[test]
fn quit_during_dialogue_cancels_without_waiting_for_an_answer() {
    let f = Fixture::new();
    let mut p = Pty::start(f.command("never approved\n"));
    p.ready();
    p.line("run");
    p.code();
    p.line("quit");
    assert!(p.finish().success(), "no File session was admitted");
    assert!(p.output.contains("SESSION CLOSED recorded"));
    f.empty();
}
