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
fn recovery_ordering_b_publication_then_separate_lift_and_undo() {
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
    p.line("lift-for-undo");
    p.yes();
    p.expect("LIFTED recovery only; missions remain stopped\n");
    p.line("undo");
    p.yes();
    p.expect("UNDO REMOVED interrupted=none\n");
    f.empty();
    for command in ["run", "approve"] {
        p.line(command);
        p.expect("REFUSED recovery_only\n");
    }
    assert_eq!(p.output.matches("LANE invoked").count(), before);
    p.line("status");
    p.expect("host_recovery_only=true");
    p.expect("kernel_stopped=false broker_stopped=false");
    p.expect("STATUS END\n");
    p.quit();
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
    p.expect("REFUSED stop_workers_outstanding\n");
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
    p.expect("UNCLEAN outstanding=");
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
