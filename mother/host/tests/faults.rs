#![deny(unsafe_code)]
#![cfg(target_os = "linux")]
mod support;
use support::faults::Fault;
use support::{Fixture, Pty};

#[test]
fn journal_directory_sync_child() {
    let Some(path) = std::env::var_os("M2DD_JOURNAL") else {
        return;
    };
    assert!(
        nexus_mother::adapter::FileJournal::open(std::path::Path::new(&path)).is_err(),
        "FileJournal must propagate parent directory sync failure, including an existing file"
    );
}

#[test]
fn file_journal_new_and_existing_entries_require_directory_sync() {
    for existing in [false, true] {
        let f = Fixture::new();
        let path = f.root.join("journal.jsonl");
        if existing {
            std::fs::write(&path, b"").unwrap();
        }
        let fault = Fault::new(&f, "journal-dir-fail");
        let mut child = std::process::Command::new(std::env::current_exe().unwrap());
        child
            .args(["--exact", "journal_directory_sync_child", "--nocapture"])
            .env("M2DD_JOURNAL", &path)
            .env("M2DD_JOURNAL_PARENT", &f.root);
        fault.attach(&f, &mut child);
        fault.arm();
        let output = child.output().unwrap();
        assert!(
            output.status.success(),
            "new/existing={existing}: {}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn d_o20_stop_ctrl_d_and_master_close_before_publication() {
    for ingress in ["stop", "ctrl-d", "master-close"] {
        let f = Fixture::new();
        let fault = Fault::new(&f, "before-publication");
        let mut command = f.command("held exact bytes\n");
        fault.attach(&f, &mut command);
        command.args(["--hold-before-publish", "20"]);
        let mut p = Pty::start(command);
        p.ready();
        p.mission();
        fault.arm();
        p.line("approve");
        p.yes();
        let receipt = fault.reached();
        assert!(
            receipt.path.contains("(deleted)"),
            "anonymous file synced before final check"
        );
        match ingress {
            "stop" => {
                p.line("stop");
                p.expect("kernel_stopped=true broker_stopped=true");
            }
            "ctrl-d" => p.send(&[4]),
            _ => {
                p.master.take();
            }
        }
        let stopped = fault.reached_kind("stop-recording");
        assert!(stopped.path.contains("/p3-evidence/evidence.sqlite3"));
        stopped.release();
        receipt.release();
        if ingress == "stop" {
            p.expect("STOP RECORDING COMPLETED");
            p.line("quit");
        }
        let status = p.finish();
        if ingress != "stop" {
            assert!(!status.success(), "EOF is an emergency exit");
        }
        f.empty();
    }
}

#[test]
fn emergency_ordering_b_preserves_publication_and_refuses_restoration() {
    let f = Fixture::new();
    let fault = Fault::new(&f, "after-final-check");
    let mut command = f.command("one handed-off publication\n");
    fault.attach(&f, &mut command);
    let mut p = Pty::start(command);
    p.ready();
    p.mission();
    fault.arm();
    p.line("approve");
    p.yes();
    let receipt = fault.reached();
    f.empty();
    assert_eq!(receipt.path.trim_end(), f.workspace.to_str().unwrap());
    p.line("stop");
    p.expect("kernel_stopped=true broker_stopped=true");
    p.expect("STOP RECORDING COMPLETED");
    receipt.release();
    p.expect("ACTION FINISHED");
    assert_eq!(
        std::fs::read(f.workspace.join("hello.txt")).unwrap(),
        b"one handed-off publication\n"
    );
    p.line("status");
    p.expect("STATUS END\n");
    let before = p.output.matches("LANE invoked").count();
    for command in [
        "lift-for-undo",
        "undo",
        "cleanup",
        "run",
        "approve",
        "detail",
    ] {
        let dialogues = p.output.matches("Answer code:").count();
        p.line(command);
        p.expect("REFUSED");
        assert_eq!(p.output.matches("Answer code:").count(), dialogues);
    }
    assert_eq!(p.output.matches("LANE invoked").count(), before);
    assert_eq!(
        std::fs::read(f.workspace.join("hello.txt")).unwrap(),
        b"one handed-off publication\n"
    );
    p.line("status");
    p.expect("kernel_stopped=true broker_stopped=true");
    p.expect("STATUS END\n");
    p.line("quit");
    p.finish();
}

#[test]
fn h1_four_storage_blocked_workers_fifth_ingress_and_resume_refusal() {
    let f = Fixture::new();
    let fault = Fault::new(&f, "evidence");
    let mut command = f.command("text\n");
    fault.attach(&f, &mut command);
    let mut p = Pty::start(command);
    p.ready();
    fault.arm();
    p.line("stop");
    let receipt = fault.reached();
    assert!(receipt.path.contains("/p3-evidence/evidence.sqlite3"));
    p.expect("kernel_stopped=true broker_stopped=true");
    for epoch in 2..=5 {
        p.line("stop");
        p.expect(&format!("STOP REQUEST RECEIVED epoch={epoch}"));
    }
    p.expect("STOP COALESCED");
    p.line("lift-for-undo");
    p.expect("REFUSED unavailable\n");
    receipt.release();
    p.expect("STOP WORKERS outstanding=0\n");
    p.line("evidence");
    p.expect("RECENT EVIDENCE bounded\n");
    p.expect("emergency_stopped");
    p.expect("EVIDENCE HEALTH\n");
    p.quit();
}

#[test]
fn h1_shutdown_with_storage_held_is_bounded_and_unclean() {
    let f = Fixture::new();
    let fault = Fault::new(&f, "evidence");
    let mut command = f.command("text\n");
    fault.attach(&f, &mut command);
    let mut p = Pty::start(command);
    p.ready();
    fault.arm();
    p.line("stop");
    let _receipt = fault.reached();
    p.expect("kernel_stopped=true broker_stopped=true");
    p.line("quit");
    p.expect("UNCLEAN EXIT: this process did not complete the full clean-close protocol. Some session-end records may already be durable; any unverified recording or resource obligations are listed separately. No cleanup or recording is promised after this process exits.\n");
    p.expect("MOTHER END Unknown (not attempted)\n");
    p.expect("PRIMARY END Unknown (not attempted)\n");
    p.expect("FINAL DELIVERY Unknown (not attempted)\n");
    p.expect("CLOSE OBLIGATIONS resource_snapshot=unknown in_flight_or_unknown=true\n");
    p.expect("PRODUCERS outstanding=1\n");
    assert!(!p.finish().success());
    assert!(!p.output.contains("SESSION CLOSED recorded"));
}

#[test]
fn d_o4_display_boundary_rejects_queued_complete_and_partial_answers() {
    for partial in [false, true] {
        let f = Fixture::new();
        let fault = Fault::new(&f, "display");
        let mut command = f.command("data\n");
        fault.attach(&f, &mut command);
        let mut p = Pty::start(command);
        p.ready();
        p.mission();
        fault.arm();
        p.line("approve");
        let receipt = fault.reached();
        let display = String::from_utf8(receipt.bytes.clone()).unwrap();
        let code = display
            .split("Answer code: ")
            .nth(1)
            .expect("dialog display receipt includes the code")
            .lines()
            .next()
            .unwrap();
        assert_eq!(code.len(), 8);
        if partial {
            p.send(&code.as_bytes()[..4]);
        } else {
            p.line(code);
        }
        receipt.release();
        p.code();
        if partial {
            p.send(&code.as_bytes()[4..]);
            p.send(b"\n");
        }
        p.line("no");
        p.expect("ACTION DENIED\n");
        f.empty();
        p.quit();
    }
}

#[test]
fn d_o4_input_error_and_output_failure_end_hands() {
    for kind in ["read-fail", "write-fail"] {
        let f = Fixture::new();
        let fault = Fault::new(&f, kind);
        let mut command = f.command("data\n");
        fault.attach(&f, &mut command);
        let mut p = Pty::start(command);
        p.ready();
        fault.arm();
        p.line("status");
        assert!(
            !p.finish().success(),
            "fatal terminal failure is not a clean exit"
        );
        f.empty();
    }
}

#[test]
fn h1_store_already_held_before_stop_ingress_and_eventual_records() {
    let f = Fixture::new();
    let fault = Fault::new(&f, "evidence");
    let mut command = f.command("no approval\n");
    fault.attach(&f, &mut command);
    let mut p = Pty::start(command);
    p.ready();
    p.line("run");
    let grant = p.code();
    fault.arm();
    p.line(&grant);
    let receipt = fault.reached();
    assert!(receipt.path.contains("/p3-evidence/evidence.sqlite3"));
    p.line("stop");
    p.expect("kernel_stopped=true broker_stopped=true");
    p.line("stop");
    p.expect("STOP REQUEST RECEIVED epoch=2");
    p.line("lift-for-undo");
    p.expect("REFUSED unavailable\n");
    receipt.release();
    p.expect("STOP WORKERS outstanding=0\n");
    p.line("evidence");
    p.expect("emergency_stopped");
    p.expect("EVIDENCE HEALTH\n");
    f.empty();
    p.quit();
}

#[test]
fn emergency_composition_preserves_file_and_rejects_queued_commands() {
    let f = Fixture::new();
    let fault = Fault::new(&f, "after-final-check");
    let mut command = f.command("one publication\n");
    fault.attach(&f, &mut command);
    let mut p = Pty::start(command);
    p.ready();
    p.mission();
    fault.arm();
    p.line("approve");
    p.yes();
    let receipt = fault.reached();
    let before = p.output.matches("LANE invoked").count();
    p.line("run");
    p.line("approve");
    p.line("stop");
    p.expect("STOP RECORDING COMPLETED");
    receipt.release();
    p.expect("ACTION FINISHED");
    for _ in 0..2 {
        p.line("stop");
        p.expect("STOP RECORDING COMPLETED");
        for command in ["lift-for-undo", "undo", "cleanup", "run", "approve"] {
            p.line(command);
            p.expect("REFUSED");
        }
        assert_eq!(p.output.matches("LANE invoked").count(), before);
        assert_eq!(
            std::fs::read(f.workspace.join("hello.txt")).unwrap(),
            b"one publication\n"
        );
    }
    p.line("quit");
    p.finish();
}

#[test]
fn h1_cleanup_stall_does_not_hold_reader_or_repeated_stop() {
    let f = Fixture::new();
    let fault = Fault::new(&f, "cleanup");
    let mut command = f.command("pending candidate\n");
    fault.attach(&f, &mut command);
    let mut p = Pty::start(command);
    p.ready();
    p.mission();
    // Denial retires the preparation without removing its owned staging.
    p.line("approve");
    p.code();
    p.line("no");
    p.expect("ACTION DENIED\n");
    fault.arm();
    p.line("cleanup");
    let receipt = fault.reached();
    assert!(receipt.path.contains("/coding-runs/"));
    for _ in 0..2 {
        p.line("stop");
        p.expect("STOP RECORDING COMPLETED");
    }
    p.line("status");
    p.expect("STATUS END\n");
    receipt.release();
    p.expect("CLEANUP");
    p.line("cleanup");
    p.expect("REFUSED");
    f.empty();
    p.line("quit");
    p.finish();
}

#[test]
fn owner_cleanup_route_retries_real_retained_staging() {
    let f = Fixture::new();
    let fault = Fault::new(&f, "cleanup-fail");
    let mut command = f.command("pending candidate\n");
    fault.attach(&f, &mut command);
    let mut p = Pty::start(command);
    p.ready();
    p.mission();
    p.line("approve");
    p.code();
    p.line("no");
    p.expect("ACTION DENIED\n");
    fault.arm();
    p.line("cleanup");
    p.expect("CLEANUP");
    p.line("cleanup");
    p.expect("CLEANUP settled=1 still_owed=0\n");
    f.empty();
    p.quit();
}

#[test]
fn recovery_restart_reports_crashed_preparation_without_sweeping_staging() {
    let f = Fixture::new();
    let fault = Fault::new(&f, "before-publication");
    let mut command = f.command("retained crash candidate\n");
    fault.attach(&f, &mut command);
    let mut p = Pty::start(command);
    p.ready();
    p.mission();
    fault.arm();
    p.line("approve");
    p.yes();
    let _receipt = fault.reached();
    p.child.kill().unwrap();
    assert!(!p.finish().success());
    f.empty();
    let staging = f.home.join("broker/files/coding-runs");
    let before = support::privacy::files_under(&staging, None);
    assert!(!before.is_empty(), "real retained staging");
    let mut restarted = Pty::start(f.command("no restoration\n"));
    restarted.expect("HANDS REPORT ONLY");
    assert!(!restarted.output.contains("HANDS READY"));
    assert!(restarted
        .output
        .contains("prepared; publication or temp status unknown"));
    assert_eq!(
        support::privacy::files_under(&staging, None),
        before,
        "startup never sweeps"
    );
    restarted.line("undo");
    restarted.expect("UNDO REFUSED");
    restarted.line("quit");
    restarted.finish();
    f.empty();
}

#[test]
fn d_o20_read_error_during_held_execution_engages_stop_before_exit() {
    let f = Fixture::new();
    let fault = Fault::new(&f, "held-read-error");
    let mut command = f.command("must not publish\n");
    fault.attach(&f, &mut command);
    let mut p = Pty::start(command);
    p.ready();
    p.mission();
    fault.arm();
    p.line("approve");
    p.yes();
    let held = fault.reached_kind("before-publication");
    p.line("status");
    let stop = fault.reached_kind("stop-recording");
    assert!(stop.path.contains("/p3-evidence/evidence.sqlite3"));
    stop.release();
    held.release();
    assert!(!p.finish().success());
    f.empty();
}

#[test]
fn review1_real_cr_lf_crlf_stop_during_publish_hold() {
    for end in [b"\r".as_slice(), b"\n", b"\r\n"] {
        let f = Fixture::new();
        let fault = Fault::new(&f, "before-publication");
        let mut command = f.command("must never publish\n");
        command.args(["--hold-before-publish", "20"]);
        fault.attach(&f, &mut command);
        let mut p = Pty::start(command);
        p.ready();
        p.mission();
        fault.arm();
        p.line("approve");
        p.yes();
        let before_hold = fault.reached();
        assert!(before_hold.path.contains("(deleted)"));
        // Release the real pre-final-check boundary into the configured hold.
        before_hold.release();
        p.send(b"stop");
        p.send(end);
        p.expect("STOP REQUEST RECEIVED epoch=1");
        p.expect("kernel_stopped=true broker_stopped=true");
        fault.reached_kind("stop-recording").release();
        p.expect("STOP RECORDING COMPLETED");
        // Execution and recording are independent workers. Require both
        // receipts within the deadline without inventing their relative order.
        let deadline = std::time::Instant::now() + support::DEADLINE;
        while !p.output.contains("ACTION REFUSED") {
            assert!(
                std::time::Instant::now() < deadline && p.read_once(deadline),
                "execution finished: {}",
                p.output
            );
        }
        assert!(
            !p.output.contains("ACTION FINISHED"),
            "interrupted pre-publication call is not successful approval"
        );
        p.line("status");
        p.expect("STATUS END\n");
        assert_eq!(p.output.matches("STOP REQUEST RECEIVED").count(), 1);
        assert!(
            !p.output.contains("unknown_command"),
            "CRLF must not submit an empty command"
        );
        f.empty();
        p.line("quit");
        p.finish();
        f.empty();
    }
}
