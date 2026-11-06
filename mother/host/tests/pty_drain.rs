//! Oracle: `Pty::finish` keeps output the child wrote before it exited.
#![deny(unsafe_code)]
#![cfg(target_os = "linux")]
mod support;
use std::process::Command;
use support::Pty;

/// The child is reaped before `finish` runs, so `try_wait` reports exit on
/// its first poll. Any final line must still come from draining the master.
#[test]
fn pty_finish_drains_output_written_before_exit() {
    let mut command = Command::new("sh");
    command.args(["-c", "printf 'LAST WORDS\\n'; exit 3"]);
    let mut p = Pty::start(command);
    let exited = p.child.wait().unwrap();
    let status = p.finish();
    assert_eq!(status.code(), exited.code());
    assert_eq!(status.code(), Some(3));
    assert!(
        p.output.contains("LAST WORDS\n"),
        "finish dropped output written before exit: {:?}",
        p.output
    );
}
