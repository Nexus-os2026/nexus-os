use super::{Fixture, DEADLINE};
use nix::poll::{poll, PollFd, PollFlags};
use std::io::{BufRead, BufReader, Read, Write};
use std::os::fd::AsFd;
use std::os::linux::net::SocketAddrExt;
use std::os::unix::net::{SocketAddr, UnixListener, UnixStream};
use std::path::PathBuf;
use std::process::Command;
use std::sync::OnceLock;

fn library() -> &'static PathBuf {
    static LIBRARY: OnceLock<PathBuf> = OnceLock::new();
    LIBRARY.get_or_init(|| {
        let dir = std::env::temp_dir().join(format!("m2dd-interpose-{}", std::process::id()));
        std::fs::create_dir(&dir).unwrap();
        let lib = dir.join("faults.so");
        let output = Command::new("cc")
            .args([
                "-shared", "-fPIC", "-std=c11", "-Wall", "-Wextra", "-Werror", "-o",
            ])
            .arg(&lib)
            .arg(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/support/faults.c"))
            .args(["-ldl"])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "native fixture: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        lib
    })
}

pub struct Fault {
    listener: UnixListener,
    pub arm: PathBuf,
    name: String,
    kind: &'static str,
}
impl Fault {
    pub fn new(f: &Fixture, kind: &'static str) -> Self {
        let name = format!(
            "m2dd.{}.{}",
            std::process::id(),
            super::NEXT.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
        );
        let listener =
            UnixListener::bind_addr(&SocketAddr::from_abstract_name(name.as_bytes()).unwrap())
                .unwrap();
        Self {
            listener,
            arm: f.root.join("arm"),
            name,
            kind,
        }
    }
    pub fn attach(&self, f: &Fixture, command: &mut Command) {
        command
            .env("LD_PRELOAD", library())
            .env("M2DD_FAULT", self.kind)
            .env("M2DD_ARM", &self.arm)
            .env("M2DD_SOCKET", &self.name)
            .env("M2DD_WORKSPACE", &f.workspace);
    }
    pub fn arm(&self) {
        std::fs::write(&self.arm, b"armed").unwrap();
    }
    pub fn reached(&self) -> Receipt {
        let mut fds = [PollFd::new(self.listener.as_fd(), PollFlags::POLLIN)];
        assert_eq!(
            poll(&mut fds, DEADLINE.as_millis() as u16).unwrap(),
            1,
            "exact {} boundary receipt",
            self.kind
        );
        let (stream, _) = self.listener.accept().unwrap();
        stream.set_read_timeout(Some(DEADLINE)).unwrap();
        stream.set_write_timeout(Some(DEADLINE)).unwrap();
        let mut stream = BufReader::new(stream);
        let mut kind = String::new();
        stream.read_line(&mut kind).unwrap();
        let mut path = String::new();
        stream.read_line(&mut path).unwrap();
        let mut n = String::new();
        stream.read_line(&mut n).unwrap();
        assert_eq!(kind.trim_end(), self.kind);
        let n: usize = n.trim_end().parse().unwrap();
        assert!(n <= 1_000_000);
        let mut bytes = vec![0; n];
        stream.read_exact(&mut bytes).unwrap();
        Receipt {
            stream: stream.into_inner(),
            path,
            bytes,
        }
    }
}
pub struct Receipt {
    stream: UnixStream,
    pub path: String,
    pub bytes: Vec<u8>,
}
impl Receipt {
    pub fn release(mut self) {
        self.stream.write_all(b"R").unwrap();
    }
}
