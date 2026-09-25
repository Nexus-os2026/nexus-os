//! P0-002C4C2 native sealed-spawn evidence. Helpers are ignored libtest
//! entries invoked only by exact name through the current (absolute) test
//! executable. Synthetic sentinels only: no real credential is ever used, the
//! test process's own environment is never mutated, and values of unexpected
//! variables are never printed.
#![cfg(any(target_os = "linux", target_os = "macos", windows))]

use nexus_kernel::resource_limiter::{
    ResourceLimitError, ResourceLimitedChild, ResourceLimiter, ResourceOutput, ResourceProgram,
    ResourceSpawnSpec, ResourceStdin, SealedEnvironment, SealedEnvironmentError, SealedSpawnSpec,
};
use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

const WAIT: Duration = Duration::from_secs(30);
const CLEANUP: Duration = Duration::from_secs(10);
const HOLD: Duration = Duration::from_secs(60);
const VISIBLE: &str = "C4C2_VISIBLE";
const GIB: u64 = 1024 * 1024 * 1024;
const MIB: u64 = 1024 * 1024;

// Synthetic sentinels placed in the middle helper's (the sealed spawner's)
// environment. None may reach a sealed child.
const SENTINELS: &[(&str, &str)] = &[
    ("FAKE_PROVIDER_API_KEY", "c4c2-sentinel-provider"),
    ("NODE_OPTIONS", "--require=/c4c2-sentinel"),
    ("NODE_PATH", "/c4c2-sentinel"),
    ("NPM_CONFIG_FAKE", "c4c2-sentinel"),
    ("HTTP_PROXY", "http://c4c2-sentinel.invalid:1"),
    ("HTTPS_PROXY", "http://c4c2-sentinel.invalid:1"),
    ("SSH_AUTH_SOCK", "/c4c2-sentinel.sock"),
    ("NEXUS_FAKE_SECRET", "c4c2-sentinel"),
    ("C4C2_ARBITRARY_PARENT", "c4c2-sentinel"),
];

fn helper_args(name: &str) -> Vec<OsString> {
    [
        "--exact",
        name,
        "--ignored",
        "--nocapture",
        "--test-threads=1",
    ]
    .into_iter()
    .map(Into::into)
    .collect()
}

struct Dirs {
    _root: tempfile::TempDir,
    home: PathBuf,
    temp: PathBuf,
    cwd: PathBuf,
}

fn dirs() -> Dirs {
    let root = tempfile::tempdir().unwrap();
    let make = |name: &str| {
        let dir = root.path().join(name);
        std::fs::create_dir(&dir).unwrap();
        dir.canonicalize().unwrap()
    };
    let (home, temp, cwd) = (make("home"), make("temp"), make("cwd"));
    Dirs {
        _root: root,
        home,
        temp,
        cwd,
    }
}

fn sealed_environment(home: &Path, temp: &Path, marker: &str) -> SealedEnvironment {
    SealedEnvironment::builder()
        .home_dir(home)
        .unwrap()
        .temp_dir(temp)
        .unwrap()
        .set(VISIBLE, marker)
        .unwrap()
        .build()
        .unwrap()
}

fn sealed_spec(program: PathBuf, cwd: &Path, environment: SealedEnvironment) -> SealedSpawnSpec {
    SealedSpawnSpec {
        program,
        args: Vec::new(),
        current_dir: cwd.to_path_buf(),
        environment,
        stdout: ResourceOutput::Piped,
        stderr: ResourceOutput::Null,
    }
}

fn marked(line: &str) -> Option<&str> {
    line.find("C4C2|").map(|at| &line[at + "C4C2|".len()..])
}

// Bounded outer wait on the middle helper (a plain test process).
fn run_middle(mode: &str, dirs: &Dirs) -> Vec<String> {
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .args(helper_args("sealed_middle"))
        .env("C4C2_TEST_MODE", mode)
        .env("C4C2_TEST_HOME", &dirs.home)
        .env("C4C2_TEST_TEMP", &dirs.temp)
        .env("C4C2_TEST_CWD", &dirs.cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (name, value) in SENTINELS {
        command.env(name, value);
    }
    let mut child = command.spawn().unwrap();
    let mut stdout = child.stdout.take().unwrap();
    let mut stderr = child.stderr.take().unwrap();
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let (mut out, mut err) = (String::new(), String::new());
        let _ = stdout.read_to_string(&mut out);
        let _ = stderr.read_to_string(&mut err);
        let _ = sender.send((out, err));
    });
    let deadline = Instant::now() + WAIT;
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("middle helper deadline");
        }
        thread::sleep(Duration::from_millis(10));
    };
    let (out, err) = receiver.recv_timeout(WAIT).unwrap();
    assert!(
        status.success(),
        "middle helper failed: {status}\n{out}\n{err}"
    );
    out.lines()
        .filter_map(|line| marked(line).map(str::to_owned))
        .collect()
}

struct Report {
    names: BTreeSet<String>,
    values: BTreeMap<String, String>,
    limits: BTreeMap<String, (u64, u64)>,
    parent_limits: BTreeMap<String, (u64, u64)>,
    other: Vec<String>,
}

fn parse(lines: &[String]) -> Report {
    let mut report = Report {
        names: BTreeSet::new(),
        values: BTreeMap::new(),
        limits: BTreeMap::new(),
        parent_limits: BTreeMap::new(),
        other: Vec::new(),
    };
    for line in lines {
        let parts: Vec<&str> = line.splitn(4, '|').collect();
        match parts.as_slice() {
            ["NAME", name] => {
                report.names.insert((*name).to_owned());
            }
            ["VALUE", name, value] => {
                report
                    .values
                    .insert((*name).to_owned(), (*value).to_owned());
            }
            [kind @ ("RLIMIT" | "PARENT_RLIMIT"), name, soft, hard] => {
                let value = (soft.parse().unwrap(), hard.parse().unwrap());
                let map = if *kind == "RLIMIT" {
                    &mut report.limits
                } else {
                    &mut report.parent_limits
                };
                map.insert((*name).to_owned(), value);
            }
            _ => report.other.push(line.clone()),
        }
    }
    report
}

#[test]
fn sealed_child_receives_only_the_sealed_environment_and_fixed_policy() {
    let dirs = dirs();
    let report = parse(&run_middle("sealed", &dirs));
    assert!(
        report.other.iter().any(|line| line == "DONE"),
        "{:?}",
        report.other
    );
    #[cfg(unix)]
    let expected = ["HOME", "TMPDIR", VISIBLE];
    #[cfg(windows)]
    let expected = [
        "SystemRoot",
        "TEMP",
        "TMP",
        "USERPROFILE",
        VISIBLE,
        "windir",
    ];
    let expected: BTreeSet<String> = expected.iter().map(|name| (*name).to_owned()).collect();
    // Exactly the sealed entries: no PATH, no sentinel, no parent variable.
    assert_eq!(report.names, expected);
    for (name, _) in SENTINELS {
        assert!(!report.names.contains(*name), "{name} leaked");
    }
    assert!(!report
        .names
        .iter()
        .any(|name| name.eq_ignore_ascii_case("PATH")));
    assert_eq!(report.values[VISIBLE], "report");
    let canonical = |value: &str| PathBuf::from(value).canonicalize().unwrap();
    #[cfg(unix)]
    {
        assert_eq!(PathBuf::from(&report.values["HOME"]), dirs.home);
        assert_eq!(PathBuf::from(&report.values["TMPDIR"]), dirs.temp);
    }
    #[cfg(windows)]
    {
        assert_eq!(canonical(&report.values["USERPROFILE"]), dirs.home);
        assert_eq!(canonical(&report.values["TEMP"]), dirs.temp);
        assert_eq!(canonical(&report.values["TMP"]), dirs.temp);
        assert_eq!(report.values["SystemRoot"], report.values["windir"]);
        assert!(canonical(&report.values["SystemRoot"])
            .join("System32")
            .is_dir());
        // An ordinary local operation works in the sealed child.
        assert!(
            report.other.iter().any(|line| line == "LOOPBACK|ok"),
            "{:?}",
            report.other
        );
        let job = report
            .other
            .iter()
            .find_map(|line| line.strip_prefix("JOB|"))
            .expect("sealed child job report");
        let fields: Vec<u64> = job.split('|').map(|field| field.parse().unwrap()).collect();
        use windows_sys::Win32::System::JobObjects::{
            JOB_OBJECT_LIMIT_ACTIVE_PROCESS, JOB_OBJECT_LIMIT_BREAKAWAY_OK,
            JOB_OBJECT_LIMIT_JOB_MEMORY, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
            JOB_OBJECT_LIMIT_SILENT_BREAKAWAY_OK,
        };
        let flags = fields[0] as u32;
        for required in [
            JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
            JOB_OBJECT_LIMIT_JOB_MEMORY,
            JOB_OBJECT_LIMIT_ACTIVE_PROCESS,
        ] {
            assert_eq!(flags & required, required, "flags {flags:#x}");
        }
        assert_eq!(flags & JOB_OBJECT_LIMIT_BREAKAWAY_OK, 0);
        assert_eq!(flags & JOB_OBJECT_LIMIT_SILENT_BREAKAWAY_OK, 0);
        assert_eq!(fields[1], 2 * GIB, "job memory limit");
        assert_eq!(fields[2], 32, "active process limit");
    }
    #[cfg(unix)]
    {
        let _ = canonical;
        let inherited = |name: &str| {
            assert_eq!(
                report.limits[name], report.parent_limits[name],
                "{name} must be inherited, not installed by the sealed policy"
            );
        };
        assert_eq!(report.limits["FSIZE"], (100 * MIB, 100 * MIB));
        #[cfg(target_os = "linux")]
        assert_eq!(report.limits["DATA"], (2 * GIB, 2 * GIB));
        // macOS: no memory bound is installed or claimed.
        #[cfg(target_os = "macos")]
        inherited("DATA");
        inherited("AS");
        inherited("CPU");
        inherited("NPROC");
    }
}

#[test]
fn legacy_spawn_still_inherits_the_parent_environment() {
    let dirs = dirs();
    let lines = run_middle("legacy", &dirs);
    // Legacy ResourceLimiter::spawn behaviour is unchanged.
    for (name, _) in SENTINELS {
        assert!(lines.contains(&format!("HAS|{name}")), "{name}: {lines:?}");
    }
}

#[test]
fn sealed_spawn_rejects_invalid_specifications() {
    let dirs = dirs();
    let environment = || sealed_environment(&dirs.home, &dirs.temp, "unused");
    let limiter = ResourceLimiter::default();
    // PATH lookup is never attempted for a bare or relative program.
    for program in ["node", "bin/node"] {
        assert!(matches!(
            limiter.spawn_sealed(&sealed_spec(program.into(), &dirs.cwd, environment())),
            Err(ResourceLimitError::InvalidSealedSpawn(_))
        ));
    }
    // An absolute but missing executable is an OS spawn failure.
    assert!(matches!(
        limiter.spawn_sealed(&sealed_spec(
            dirs.cwd.join("missing-executable"),
            &dirs.cwd,
            environment()
        )),
        Err(ResourceLimitError::SpawnFailed(_))
    ));
    let program = std::env::current_exe().unwrap();
    std::fs::create_dir(dirs.cwd.join("sub")).unwrap();
    let mut non_canonical = dirs.cwd.as_os_str().to_owned();
    non_canonical.push(std::path::MAIN_SEPARATOR_STR);
    non_canonical.push("sub");
    non_canonical.push(std::path::MAIN_SEPARATOR_STR);
    non_canonical.push("..");
    for cwd in [
        PathBuf::from("relative"),
        dirs.cwd.join("missing"),
        PathBuf::from(non_canonical),
    ] {
        assert!(
            matches!(
                limiter.spawn_sealed(&sealed_spec(program.clone(), &cwd, environment())),
                Err(ResourceLimitError::InvalidSealedSpawn(_))
            ),
            "{cwd:?}"
        );
    }
    for dir in [PathBuf::from("relative"), dirs.home.join("missing")] {
        assert_eq!(
            SealedEnvironment::builder().home_dir(&dir).err(),
            Some(SealedEnvironmentError::InvalidDirectory)
        );
        assert_eq!(
            SealedEnvironment::builder().temp_dir(&dir).err(),
            Some(SealedEnvironmentError::InvalidDirectory)
        );
    }
}

enum Line {
    Text(String),
    End,
}

#[test]
fn sealed_tree_is_owned_and_finalized_with_its_descendant() {
    let dirs = dirs();
    let mut spec = sealed_spec(
        std::env::current_exe().unwrap(),
        &dirs.cwd,
        sealed_environment(&dirs.home, &dirs.temp, "tree"),
    );
    spec.args = helper_args("sealed_tree");
    let mut child = ResourceLimiter::default().spawn_sealed(&spec).unwrap();
    let stdout = child.take_stdout().unwrap();
    let (sender, lines) = mpsc::channel();
    thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            let Ok(line) = line else { break };
            if sender.send(Line::Text(line)).is_err() {
                return;
            }
        }
        let _ = sender.send(Line::End);
    });
    let deadline = Instant::now() + WAIT;
    let mut ready = BTreeSet::new();
    while ready.len() < 2 {
        match lines
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .expect("sealed tree readiness")
        {
            Line::Text(line) => {
                if let Some(marker) = marked(&line) {
                    ready.insert(marker.to_owned());
                }
            }
            Line::End => panic!("sealed tree exited before readiness: {ready:?}"),
        }
    }
    assert!(ready.contains("TREE_READY") && ready.contains("DESCENDANT_READY"));
    assert!(child.poll_exit().unwrap().is_none());
    finalize(&mut child);
    // Pipe EOF: both the root and its descendant are gone.
    let deadline = Instant::now() + CLEANUP;
    loop {
        match lines.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
            Ok(Line::End) => break,
            Ok(Line::Text(_)) => {}
            Err(_) => panic!("sealed descendant still holds the pipe"),
        }
    }
}

fn finalize(child: &mut ResourceLimitedChild) {
    let report = child
        .terminate_and_reap(Instant::now() + CLEANUP)
        .expect("sealed tree finalization");
    assert!(!report.already_finalized);
}

// ── Helpers (ignored; exact-name only) ────────────────────────────────────

#[test]
#[ignore]
fn sealed_middle() {
    let Ok(mode) = std::env::var("C4C2_TEST_MODE") else {
        return;
    };
    let path = |name: &str| PathBuf::from(std::env::var_os(name).unwrap());
    let (home, temp, cwd) = (
        path("C4C2_TEST_HOME"),
        path("C4C2_TEST_TEMP"),
        path("C4C2_TEST_CWD"),
    );
    let program = std::env::current_exe().unwrap();
    let mut child = if mode == "sealed" {
        let mut spec = sealed_spec(program, &cwd, sealed_environment(&home, &temp, "report"));
        spec.args = helper_args("sealed_report");
        ResourceLimiter::default().spawn_sealed(&spec).unwrap()
    } else {
        ResourceLimiter::default()
            .spawn(&ResourceSpawnSpec {
                program: ResourceProgram::Executable {
                    program: program.into_os_string(),
                    args: helper_args("sealed_report"),
                },
                current_dir: cwd,
                stdin: ResourceStdin::Null,
                stdout: ResourceOutput::Piped,
                stderr: ResourceOutput::Null,
            })
            .unwrap()
    };
    let mut output = String::new();
    child
        .take_stdout()
        .unwrap()
        .read_to_string(&mut output)
        .unwrap();
    child.terminate_and_reap(Instant::now() + CLEANUP).unwrap();
    for line in output.lines().filter_map(marked) {
        println!("C4C2|{line}");
    }
    #[cfg(unix)]
    for (name, soft, hard) in rlimits() {
        println!("C4C2|PARENT_RLIMIT|{name}|{soft}|{hard}");
    }
}

#[test]
#[ignore]
fn sealed_report() {
    let sealed = std::env::var(VISIBLE).as_deref() == Ok("report");
    let legacy = std::env::var("C4C2_TEST_MODE").as_deref() == Ok("legacy");
    if legacy {
        // Presence only: never print inherited values.
        for (name, _) in SENTINELS {
            if std::env::var_os(name).is_some() {
                println!("C4C2|HAS|{name}");
            }
        }
        return;
    }
    if !sealed {
        return;
    }
    // Every name; values only for the expected sealed keys.
    const PRINTABLE: &[&str] = &[
        "HOME",
        "TMPDIR",
        "USERPROFILE",
        "TEMP",
        "TMP",
        "SystemRoot",
        "windir",
        VISIBLE,
    ];
    for (name, value) in std::env::vars_os() {
        let name = name.to_string_lossy().into_owned();
        println!("C4C2|NAME|{name}");
        if PRINTABLE.contains(&name.as_str()) {
            println!("C4C2|VALUE|{name}|{}", value.to_string_lossy());
        }
    }
    #[cfg(unix)]
    for (name, soft, hard) in rlimits() {
        println!("C4C2|RLIMIT|{name}|{soft}|{hard}");
    }
    #[cfg(windows)]
    {
        let loopback = std::net::TcpListener::bind(("127.0.0.1", 0)).is_ok();
        println!("C4C2|LOOPBACK|{}", if loopback { "ok" } else { "failed" });
        let (flags, memory, active) = job_limits();
        println!("C4C2|JOB|{flags}|{memory}|{active}");
    }
    println!("C4C2|DONE");
    std::io::stdout().flush().unwrap();
}

#[test]
#[ignore]
fn sealed_tree() {
    if std::env::var(VISIBLE).as_deref() != Ok("tree") {
        return;
    }
    let mut descendant = Command::new(std::env::current_exe().unwrap())
        .args(helper_args("sealed_hold"))
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    println!("C4C2|TREE_READY");
    std::io::stdout().flush().unwrap();
    let deadline = Instant::now() + HOLD;
    while Instant::now() < deadline {
        if descendant.try_wait().unwrap().is_some() {
            return;
        }
        thread::sleep(Duration::from_millis(20));
    }
    let _ = descendant.kill();
    let _ = descendant.wait();
}

#[test]
#[ignore]
fn sealed_hold() {
    if std::env::var(VISIBLE).as_deref() != Ok("tree") {
        return;
    }
    println!("C4C2|DESCENDANT_READY");
    std::io::stdout().flush().unwrap();
    thread::sleep(HOLD);
}

#[cfg(unix)]
fn rlimits() -> Vec<(&'static str, u64, u64)> {
    // nix's `resource` feature is Linux-only; its libc re-export is not.
    use nix::libc;
    [
        ("DATA", libc::RLIMIT_DATA),
        ("FSIZE", libc::RLIMIT_FSIZE),
        ("AS", libc::RLIMIT_AS),
        ("CPU", libc::RLIMIT_CPU),
        ("NPROC", libc::RLIMIT_NPROC),
    ]
    .into_iter()
    .map(|(name, resource)| {
        let mut bound = libc::rlimit {
            rlim_cur: 0,
            rlim_max: 0,
        };
        // SAFETY: getrlimit writes only the initialized `bound`.
        assert_eq!(unsafe { libc::getrlimit(resource, &mut bound) }, 0);
        (name, bound.rlim_cur, bound.rlim_max)
    })
    .collect()
}

// The calling process's own (immediate) Job: its sealed long-lived policy.
#[cfg(windows)]
fn job_limits() -> (u32, usize, u32) {
    use windows_sys::Win32::System::JobObjects::{
        JobObjectExtendedLimitInformation, QueryInformationJobObject,
        JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
    };
    let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
    // SAFETY: a null job handle queries the calling process's job; the
    // output buffer is the exact, initialized structure.
    let ok = unsafe {
        QueryInformationJobObject(
            std::ptr::null_mut(),
            JobObjectExtendedLimitInformation,
            (&mut info as *mut JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
            std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            std::ptr::null_mut(),
        )
    };
    assert_ne!(ok, 0, "{}", std::io::Error::last_os_error());
    (
        info.BasicLimitInformation.LimitFlags,
        info.JobMemoryLimit,
        info.BasicLimitInformation.ActiveProcessLimit,
    )
}
