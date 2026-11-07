//! M3-E M-2 (S1a-E, rulings M3E-R1..R6): the actual `nexus-mother` binary
//! runs `check-page` headless, before any home or GTK: no display, a fresh
//! empty HOME and XDG directories that stay empty, exit codes 0/1/2, and
//! hashes that `sha256sum` computes independently (g9 pins the desktop's
//! dev-dependencies, so no hashing crate is added).

#![cfg(target_os = "linux")]

use nexus_governed_control::inert_page::{policy_hash, LABEL_PASSED, MOTHER_CSS};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const BAKERY: &str = include_str!("../../src/oracles/m3a_bakery.html");
const F_BAKERY: &str = "name: Rosa's Bakery\nhours: Tuesday to Sunday, 7am to 3pm\nhours: Closed on Mondays\nphone: 555-0142\n";
const DEADLINE: Duration = Duration::from_secs(60);

struct Dir(PathBuf);
impl Dir {
    fn new(tag: &str) -> Self {
        let path = std::env::temp_dir().join(format!("m3e-m2-cli-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        Dir(path)
    }
    fn sub(&self, name: &str) -> PathBuf {
        let path = self.0.join(name);
        std::fs::create_dir_all(&path).unwrap();
        path
    }
}
impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// `sha256sum` of a file, or of `stdin` bytes when `path` is `None`.
fn sha256sum(path: Option<&Path>, stdin: &[u8]) -> String {
    let mut cmd = Command::new("sha256sum");
    if let Some(path) = path {
        cmd.arg(path);
    }
    let mut child = cmd
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("sha256sum");
    child.stdin.take().unwrap().write_all(stdin).unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success());
    String::from_utf8(out.stdout).unwrap()[..64].to_string()
}

struct Ran {
    code: Option<i32>,
    stdout: String,
    stderr: String,
}

/// The real binary, display-less, with only `home` as HOME and XDG roots;
/// bounded by DEADLINE, killed and reaped on overrun.
fn nexus_mother(home: &Path, args: &[&Path]) -> Ran {
    let mut child = Command::new(env!("CARGO_BIN_EXE_nexus-mother"))
        .args(args)
        .env_clear()
        .env("PATH", std::env::var_os("PATH").unwrap_or_default())
        .env("HOME", home)
        .env("XDG_CONFIG_HOME", home.join(".config"))
        .env("XDG_DATA_HOME", home.join(".local/share"))
        .env("XDG_STATE_HOME", home.join(".local/state"))
        .env("XDG_CACHE_HOME", home.join(".cache"))
        .env("XDG_RUNTIME_DIR", home.join("run"))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let start = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if start.elapsed() > DEADLINE {
            let _ = child.kill();
            let _ = child.wait();
            panic!("nexus-mother check-page did not finish within {DEADLINE:?}");
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    let mut stdout = String::new();
    let mut stderr = String::new();
    child
        .stdout
        .take()
        .unwrap()
        .read_to_string(&mut stdout)
        .unwrap();
    child
        .stderr
        .take()
        .unwrap()
        .read_to_string(&mut stderr)
        .unwrap();
    Ran {
        code: status.code(),
        stdout,
        stderr,
    }
}

#[test]
fn s1ae_headless_binary_has_no_home_side_effect() {
    let d = Dir::new("headless");
    let home = d.sub("home");
    let work = d.sub("work");
    let page = work.join("index.html");
    std::fs::write(&page, BAKERY).unwrap();
    let facts = work.join("facts.txt");
    std::fs::write(&facts, F_BAKERY).unwrap();
    let check = Path::new("check-page");
    let flag = Path::new("--facts");
    let binary = sha256sum(Some(Path::new(env!("CARGO_BIN_EXE_nexus-mother"))), b"");
    let policy: String = policy_hash().iter().map(|b| format!("{b:02x}")).collect();
    let header = [
        format!("binary sha256 {binary}"),
        format!(
            "policy {policy}  css {}",
            sha256sum(None, MOTHER_CSS.as_bytes())
        ),
        format!(
            "file bytes {} sha256 {}",
            BAKERY.len(),
            sha256sum(Some(&page), b"")
        ),
    ];

    let ran = nexus_mother(&home, &[check, &page, flag, &facts]);
    assert_eq!(ran.code, Some(0), "{}", ran.stdout);
    let out: Vec<&str> = ran.stdout.lines().collect();
    assert_eq!(
        out[..3],
        header.iter().map(String::as_str).collect::<Vec<_>>()[..]
    );
    assert_eq!(out[3], LABEL_PASSED);
    assert!(out.contains(&"result: PASS"));
    assert!(out.iter().any(|l| l.starts_with(&format!(
        "facts file sha256 {}  facts hash ",
        sha256sum(Some(&facts), b"")
    ))));

    let ran = nexus_mother(&home, &[check, &page]);
    assert_eq!(ran.code, Some(0));
    assert!(ran
        .stdout
        .contains("\nfacts: not checked (no --facts given)\n"));

    let email = work.join("email.html");
    std::fs::write(
        &email,
        BAKERY.replace("carrot cake are", "carrot cake (rosa@example.com) are"),
    )
    .unwrap();
    assert_eq!(nexus_mother(&home, &[check, &email]).code, Some(1));

    let ran = nexus_mother(&home, &[check, &work.join("missing.html")]);
    assert_eq!(
        (ran.code, ran.stdout.as_str()),
        (Some(2), "ERROR input_unavailable (page)\n")
    );
    let ran = nexus_mother(&home, &[check]);
    assert_eq!(ran.code, Some(2));
    assert!(ran.stdout.starts_with("ERROR usage: "));

    // Nothing else happened: no stderr, and the home and XDG roots are
    // still empty (no journal, private store or GTK state).
    assert_eq!(nexus_mother(&home, &[check, &page]).stderr, "");
    let left: Vec<_> = std::fs::read_dir(&home).unwrap().collect();
    assert!(left.is_empty(), "home side effects: {left:?}");
    assert_eq!(
        std::fs::read_dir(&work).unwrap().count(),
        3,
        "inputs untouched"
    );
    assert_eq!(std::fs::read(&page).unwrap(), BAKERY.as_bytes());
}

// M3E-R9 (1): `check-page` anywhere but first, `--` included, is usage
// (exit 2) before any desktop start: no home, no GTK.
#[test]
fn s1ae_misplaced_check_page_never_reaches_the_desktop() {
    let d = Dir::new("misplaced");
    let home = d.sub("home");
    let page = d.sub("work").join("index.html");
    std::fs::write(&page, BAKERY).unwrap();
    let usage = "ERROR usage: nexus-mother check-page <file> [--facts <file>]\n";
    for lead in ["--", "x", "--check-page"] {
        let ran = nexus_mother(&home, &[Path::new(lead), Path::new("check-page"), &page]);
        assert_eq!((ran.code, ran.stdout.as_str()), (Some(2), usage), "{lead}");
        assert_eq!(ran.stderr, "", "{lead}");
        let left: Vec<_> = std::fs::read_dir(&home).unwrap().collect();
        assert!(left.is_empty(), "{lead}: home side effects: {left:?}");
    }
}
