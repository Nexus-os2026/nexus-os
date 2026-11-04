#![allow(dead_code)]

pub mod faults;
pub mod privacy;

use nix::poll::{poll, PollFd, PollFlags};
use nix::pty::openpty;
use std::fs::File;
use std::io::{Read, Write};
use std::os::fd::{AsFd, OwnedFd};
use std::path::PathBuf;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

pub const DEADLINE: Duration = Duration::from_secs(10);
static NEXT: AtomicU64 = AtomicU64::new(0);

pub fn binary() -> PathBuf {
    let path = option_env!("CARGO_BIN_EXE_mother-hands").map(PathBuf::from);
    assert!(
        path.as_ref().is_some_and(|p| p.is_file()),
        "M2d-D baseline: mother-hands executable is unavailable; behavior cannot start"
    );
    path.unwrap()
}

pub struct Fixture {
    pub root: PathBuf,
    pub home: PathBuf,
    pub workspace: PathBuf,
}

impl Fixture {
    pub fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "m2dd-{}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::SeqCst),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        nexus_mother::adapter::create_private(&root).unwrap();
        let workspace = root.join("workspace");
        std::fs::create_dir(&workspace).unwrap();
        Self {
            home: root.join("home"),
            workspace,
            root,
        }
    }
    pub fn command(&self, text: &str) -> Command {
        let mut command = Command::new(binary());
        command
            .args(["--home"])
            .arg(&self.home)
            .arg("--workspace")
            .arg(&self.workspace)
            .args(["--name", "hello.txt", "--text", text]);
        command
    }
    pub fn empty(&self) {
        assert_eq!(
            std::fs::read_dir(&self.workspace).unwrap().count(),
            0,
            "no publication or named temporary file"
        );
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        // Only the uniquely owned fixture, never an arbitrary caller path.
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

pub struct Pty {
    pub child: Child,
    pub master: Option<File>,
    pub output: String,
    cursor: usize,
}

impl Pty {
    pub fn start(mut command: Command) -> Self {
        let pty = openpty(None, None).unwrap();
        // std's duplicate is close-on-exec; the raw openpty master is not.
        // Drop that original before spawn so the child cannot keep its own
        // master alive and defeat the master-close EOF oracle.
        let original = File::from(pty.master);
        let master = original.try_clone().unwrap();
        drop(original);
        let input: OwnedFd = pty.slave.try_clone().unwrap();
        let output: OwnedFd = pty.slave.try_clone().unwrap();
        let child = command
            .stdin(Stdio::from(input))
            .stdout(Stdio::from(output))
            .stderr(Stdio::from(pty.slave))
            .spawn()
            .unwrap();
        Self {
            child,
            master: Some(master),
            output: String::new(),
            cursor: 0,
        }
    }
    pub fn send(&mut self, bytes: &[u8]) {
        let file = self.master.as_mut().unwrap();
        let mut written = 0;
        let deadline = Instant::now() + DEADLINE;
        while written < bytes.len() {
            assert!(Instant::now() < deadline, "bounded PTY write");
            let mut fds = [PollFd::new(file.as_fd(), PollFlags::POLLOUT)];
            assert!(poll(&mut fds, 1000u16).unwrap() > 0, "PTY writable");
            written += file.write(&bytes[written..]).unwrap();
        }
    }
    pub fn line(&mut self, line: &str) {
        self.send(format!("{line}\n").as_bytes());
    }
    pub fn read_once(&mut self, until: Instant) -> bool {
        let file = self.master.as_mut().unwrap();
        let left = until.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return false;
        }
        let mut fds = [PollFd::new(file.as_fd(), PollFlags::POLLIN)];
        let ms = left.as_millis().clamp(1, 1000) as u16;
        if poll(&mut fds, ms).unwrap() == 0 {
            return true;
        }
        let mut bytes = [0u8; 4096];
        match file.read(&mut bytes) {
            Ok(0) | Err(_) => false,
            Ok(n) => {
                self.output.push_str(&String::from_utf8_lossy(&bytes[..n]));
                true
            }
        }
    }
    pub fn expect(&mut self, text: &str) -> String {
        let deadline = Instant::now() + DEADLINE;
        loop {
            if let Some(at) = self.output[self.cursor..].find(text) {
                let end = self.cursor + at + text.len();
                let seen = self.output[self.cursor..end].to_string();
                self.cursor = end;
                return seen;
            }
            assert!(
                Instant::now() < deadline && self.read_once(deadline),
                "expected {text:?}; transcript: {}",
                self.output
            );
        }
    }
    pub fn code(&mut self) -> String {
        self.expect("Answer code: ");
        let line = self.expect("\n");
        let code = line.trim_end_matches('\n');
        assert_eq!(code.len(), 8, "fresh eight-hex code");
        assert!(code
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)));
        let code = code.to_string();
        self.expect("INPUT READY\n");
        code
    }
    pub fn yes(&mut self) -> String {
        let code = self.code();
        self.line(&code);
        code
    }
    pub fn ready(&mut self) {
        self.expect("HANDS READY\n");
    }
    pub fn mission(&mut self) {
        self.line("run");
        let grant = self.yes();
        let door = self.yes();
        let consent = self.yes();
        assert_ne!(grant, door);
        assert_ne!(door, consent);
        self.expect("FILE PENDING\n");
    }
    pub fn create(&mut self) {
        self.mission();
        self.line("approve");
        self.yes();
        self.expect("MISSION done\n");
    }
    pub fn finish(&mut self) -> ExitStatus {
        let deadline = Instant::now() + DEADLINE;
        loop {
            if let Some(status) = self.child.try_wait().unwrap() {
                return status;
            }
            assert!(
                Instant::now() < deadline,
                "bounded child exit; {}",
                self.output
            );
            if self.master.is_some() {
                self.read_once(deadline);
            } else {
                std::thread::yield_now();
            }
        }
    }
    pub fn quit(&mut self) {
        self.line("quit");
        self.expect("SESSION CLOSED recorded\n");
        assert!(self.finish().success(), "clean Quit");
    }
}

impl Drop for Pty {
    fn drop(&mut self) {
        if self.child.try_wait().ok().flatten().is_none() {
            let _ = self.child.kill();
        }
        let _ = self.child.wait();
    }
}
