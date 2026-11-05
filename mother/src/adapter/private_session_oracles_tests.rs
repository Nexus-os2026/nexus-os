//! R37's API-call boundaries, including the error-path removal. The baseline
//! fixture invokes the old store. Migration changes its adapter, not these
//! expected filesystem facts. Probe callbacks are test-only and thread-local.
use super::*;
use nexus_governed_control::host_session::{CommandKind, HostSession, SessionSeed};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

type Callback = Box<dyn FnOnce()>;
#[derive(Default)]
struct Probe {
    active: bool,
    point: Option<(&'static str, &'static str, Callback)>,
    calls: Vec<&'static str>,
    fail_write: bool,
}
thread_local! { static PROBE: RefCell<Probe> = RefCell::default(); }

fn fire(when: &'static str, sink: &'static str) {
    let callback = PROBE.with(|probe| {
        let mut p = probe.borrow_mut();
        if p.point
            .as_ref()
            .is_some_and(|(w, s, _)| *w == when && *s == sink)
        {
            p.point.take().map(|(_, _, f)| f)
        } else {
            None
        }
    });
    if let Some(callback) = callback {
        callback();
    }
}
pub(super) fn before(sink: &'static str) {
    fire("before", sink);
}
pub(super) fn after(sink: &'static str) {
    fire("after", sink);
}
pub(super) fn entered(sink: &'static str) {
    PROBE.with(|probe| {
        let mut p = probe.borrow_mut();
        if p.active {
            p.calls.push(sink);
        }
    });
}
pub(super) fn write_result() -> std::io::Result<()> {
    if PROBE.with(|probe| probe.borrow().fail_write) {
        Err(std::io::Error::other("registered private-write fault"))
    } else {
        Ok(())
    }
}

struct Scenario(PathBuf);
impl Scenario {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "mother-private-session-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::SeqCst)
        ));
        crate::adapter::create_private(&root).unwrap();
        PROBE.with(|probe| *probe.borrow_mut() = Probe::default());
        Self(root)
    }
}
impl Drop for Scenario {
    fn drop(&mut self) {
        PROBE.with(|probe| *probe.borrow_mut() = Probe::default());
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}

type Snapshot = BTreeMap<PathBuf, Option<Vec<u8>>>;
fn snapshot(root: &Path) -> Snapshot {
    fn walk(root: &Path, at: &Path, out: &mut Snapshot) {
        for entry in std::fs::read_dir(at).unwrap() {
            let path = entry.unwrap().path();
            let metadata = std::fs::symlink_metadata(&path).unwrap();
            assert!(
                !metadata.file_type().is_symlink(),
                "owned fixture contains no links"
            );
            let value = if metadata.is_dir() {
                None
            } else {
                Some(std::fs::read(&path).unwrap())
            };
            out.insert(path.strip_prefix(root).unwrap().to_path_buf(), value);
            if metadata.is_dir() {
                walk(root, &path, out);
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(root, root, &mut out);
    out
}

#[test]
fn session_r37_each_private_call_and_error_removal_has_its_own_boundary() {
    let selected = std::env::var("M2DD_PRIVATE_CASE").ok();
    let mut failures = Vec::new();
    let mut executed = 0;
    for kind in [ContentKind::Contract, ContentKind::PausedPhase] {
        for sink in [
            "private_mkdir",
            "private_temp_create",
            "private_write",
            "private_sync",
            "private_rename",
            "private_parent_sync",
            "private_error_remove",
        ] {
            for when in ["before", "after"] {
                for ingresses in 0..=3 {
                    let key = format!("{kind:?}/{sink}/{when}/{ingresses}");
                    if let Some(selected) = &selected {
                        if selected != &key {
                            continue;
                        }
                    } else {
                        let passed = run_child(&key);
                        if !passed {
                            failures.push(key);
                        }
                        executed += 1;
                        continue;
                    }
                    let dir = Scenario::new();
                    let seed = SessionSeed::new().unwrap();
                    let ingress = seed.ingress();
                    let broker = dir.0.join("broker");
                    let evidence = nexus_governed_control::evidence_store::PrimaryEvidence::open(
                        &broker.join("p3-evidence/evidence.sqlite3"),
                        vec![],
                    )
                    .unwrap();
                    let session = HostSession::open(
                        seed,
                        &broker,
                        evidence,
                        Arc::new(nexus_governed_control::authority::clock::SystemClock::default()),
                        Arc::new(crate::oracles::lite_support::FilesOwner::default()),
                    )
                    .unwrap();
                    let workspace = dir.0.join("workspace");
                    crate::adapter::create_private(&workspace).unwrap();
                    session.bootstrap_workspace(&workspace, &[]).unwrap();
                    session.finish_bootstrap().unwrap();
                    let request = ingress.accept(CommandKind::Run, 1).unwrap();
                    let store = FilePrivate::open_session(
                        &dir.0.join("private"),
                        session.context(&request).unwrap(),
                    )
                    .unwrap();
                    let observed = Arc::new(Mutex::new(None));
                    let captured = observed.clone();
                    let root = dir.0.clone();
                    let fail_write = sink == "private_error_remove";
                    PROBE.with(|probe| {
                        *probe.borrow_mut() = Probe {
                            active: true,
                            point: Some((
                                when,
                                sink,
                                Box::new(move || {
                                    let calls = PROBE.with(|p| p.borrow().calls.len());
                                    *captured.lock().unwrap() = Some((snapshot(&root), calls));
                                    for _ in 0..ingresses {
                                        ingress.emergency().unwrap();
                                    }
                                }),
                            )),
                            fail_write,
                            ..Probe::default()
                        };
                    });
                    let result = store.put(kind, b"independent private content\n");
                    let (before, calls) = observed
                        .lock()
                        .unwrap()
                        .take()
                        .expect("real store reached the registered API boundary");
                    let remaining = PROBE.with(|p| p.borrow().calls[calls..].to_vec());
                    let after = snapshot(&dir.0);
                    let obligations = session.resource_obligations().unwrap();
                    // Literal obligation rule: a refusal before the first
                    // creation creates no resource. Every other interrupted
                    // creation/write/rename/removal still lacks a required
                    // completion or parent-sync result, even if now absent.
                    let expected_unresolved = usize::from(
                        ingresses != 0
                            && !matches!(
                                (sink, when),
                                ("private_mkdir" | "private_temp_create", "before")
                            ),
                    );
                    assert_eq!(
                        obligations.private_unresolved, expected_unresolved,
                        "R37/{key}: independent private finalization obligations"
                    );
                    let correct = if ingresses == 0 {
                        if fail_write {
                            result.is_err()
                                && after.keys().all(|p| {
                                    !p.file_name()
                                        .unwrap()
                                        .to_string_lossy()
                                        .starts_with(".tmp-")
                                })
                        } else {
                            result.as_ref().is_ok_and(|id| {
                                store.get(kind, id).unwrap() == b"independent private content\n"
                            })
                        }
                    } else {
                        remaining.is_empty() && before == after
                    };
                    println!("R37/{kind:?}/{sink}/{when}/ingresses={ingresses}: new_calls={remaining:?} unchanged={} conforms={correct}", before == after);
                    if !correct {
                        failures.push(format!("{kind:?}/{sink}/{when}/{ingresses}"));
                    }
                    executed += 1;
                }
            }
        }
    }
    assert_eq!(executed, if selected.is_some() { 1 } else { 112 });
    assert!(
        failures.is_empty(),
        "executed {executed} private-store schedules; forbidden continuations: {}",
        failures.join(", ")
    );
}

fn run_child(case: &str) -> bool {
    use std::io::Read;
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "adapter::private_file::session_probe::session_r37_each_private_call_and_error_removal_has_its_own_boundary", "--nocapture"])
        .env("M2DD_PRIVATE_CASE", case)
        .stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
    let mut stdout = child.stdout.take().unwrap();
    let mut stderr = child.stderr.take().unwrap();
    let out = std::thread::spawn(move || {
        let mut text = String::new();
        stdout.read_to_string(&mut text).unwrap();
        text
    });
    let err = std::thread::spawn(move || {
        let mut text = String::new();
        stderr.read_to_string(&mut text).unwrap();
        text
    });
    let deadline = Instant::now() + Duration::from_secs(20);
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break Some(status);
        }
        if Instant::now() >= deadline {
            child.kill().unwrap();
            child.wait().unwrap();
            break None;
        }
        std::thread::sleep(Duration::from_millis(5));
    };
    let stdout = out.join().unwrap();
    let stderr = err.join().unwrap();
    let reached = stdout.lines().any(|line| line.starts_with("R37/"));
    let passed = status.is_some_and(|status| status.success()) && reached;
    print!("{stdout}");
    if !passed {
        eprintln!("private child {case} failed: {stderr}");
    }
    passed
}

// F11: real private API calls after obligation reservation, including EOF,
// normal closing without emergency, and fatal failure. No model counters stand
// in for the actual filesystem snapshots and successor API observations.
#[test]
fn session_r37_eof_quit_fatal_each_reserved_private_boundary() {
    let selected = std::env::var("M2DD_PRIVATE_CLOSE_CASE").ok();
    let mut failures = Vec::new();
    let mut executed = 0;
    for kind in [ContentKind::Contract, ContentKind::PausedPhase] {
        for sink in [
            "private_mkdir",
            "private_temp_create",
            "private_write",
            "private_sync",
            "private_rename",
            "private_parent_sync",
            "private_error_remove",
        ] {
            for when in ["before", "after"] {
                for restriction in ["control", "EOF", "quit", "fatal"] {
                    let restricted = restriction != "control";
                    let key = format!("{kind:?}/{sink}/{when}/{restriction}");
                    if let Some(selected) = &selected {
                        if selected != &key {
                            continue;
                        }
                    } else {
                        let passed = run_restriction_child(&key);
                        if !passed {
                            failures.push(key);
                        }
                        executed += 1;
                        continue;
                    }
                    let dir = Scenario::new();
                    let seed = SessionSeed::new().unwrap();
                    let ingress = seed.ingress();
                    let broker = dir.0.join("broker");
                    let evidence = nexus_governed_control::evidence_store::PrimaryEvidence::open(
                        &broker.join("p3-evidence/evidence.sqlite3"),
                        vec![],
                    )
                    .unwrap();
                    let session = HostSession::open(
                        seed,
                        &broker,
                        evidence,
                        Arc::new(nexus_governed_control::authority::clock::SystemClock::default()),
                        Arc::new(crate::oracles::lite_support::FilesOwner::default()),
                    )
                    .unwrap();
                    let workspace = dir.0.join("workspace");
                    crate::adapter::create_private(&workspace).unwrap();
                    session.bootstrap_workspace(&workspace, &[]).unwrap();
                    session.finish_bootstrap().unwrap();
                    let request = ingress.accept(CommandKind::Run, 1).unwrap();
                    let store = FilePrivate::open_session(
                        &dir.0.join("private"),
                        session.context(&request).unwrap(),
                    )
                    .unwrap();
                    let observed = Arc::new(Mutex::new(None));
                    let captured = observed.clone();
                    let root = dir.0.clone();
                    let fail_write = sink == "private_error_remove";
                    PROBE.with(|probe| {
                        *probe.borrow_mut() = Probe {
                            active: true,
                            point: Some((
                                when,
                                sink,
                                Box::new(move || {
                                    let calls = PROBE.with(|p| p.borrow().calls.len());
                                    *captured.lock().unwrap() = Some((snapshot(&root), calls));
                                    // The exact reader EOF sequence is emergency
                                    // denial followed by fatal restriction. Quit
                                    // has no emergency; it invalidates the original
                                    // Run snapshot through closing alone.
                                    match restriction {
                                        "control" => {}
                                        "EOF" => {
                                            ingress.emergency().unwrap();
                                            ingress.fail();
                                        }
                                        "quit" => {
                                            ingress.accept(CommandKind::Quit, 2).unwrap();
                                        }
                                        "fatal" => ingress.fail(),
                                        _ => unreachable!(),
                                    }
                                }),
                            )),
                            fail_write,
                            ..Probe::default()
                        };
                    });
                    let result = store.put(kind, b"independent private content\n");
                    let (before, calls) = observed
                        .lock()
                        .unwrap()
                        .take()
                        .expect("real store reached the registered API boundary");
                    let remaining = PROBE.with(|p| p.borrow().calls[calls..].to_vec());
                    let after = snapshot(&dir.0);
                    let obligations = session.resource_obligations().unwrap();
                    // Literal obligation rule: a refusal before the first
                    // creation creates no resource. Every other interrupted
                    // creation/write/rename/removal still lacks a required
                    // completion or parent-sync result, even if now absent.
                    let expected_unresolved = usize::from(
                        restricted
                            && !matches!(
                                (sink, when),
                                ("private_mkdir" | "private_temp_create", "before")
                            ),
                    );
                    assert_eq!(
                        obligations.private_unresolved, expected_unresolved,
                        "R37/{key}: independent private finalization obligations"
                    );
                    let correct = if !restricted {
                        if fail_write {
                            result.is_err()
                                && after.keys().all(|p| {
                                    !p.file_name()
                                        .unwrap()
                                        .to_string_lossy()
                                        .starts_with(".tmp-")
                                })
                        } else {
                            result.as_ref().is_ok_and(|id| {
                                store.get(kind, id).unwrap() == b"independent private content\n"
                            })
                        }
                    } else {
                        remaining.is_empty() && before == after
                    };
                    println!("R37-CLOSE/{kind:?}/{sink}/{when}/{restriction}: new_calls={remaining:?} unchanged={} conforms={correct}", before == after);
                    if !correct {
                        failures.push(format!("{kind:?}/{sink}/{when}/{restriction}"));
                    }
                    executed += 1;
                }
            }
        }
    }
    assert_eq!(executed, if selected.is_some() { 1 } else { 112 });
    assert!(
        failures.is_empty(),
        "executed {executed} private-store schedules; forbidden continuations: {}",
        failures.join(", ")
    );
}

fn run_restriction_child(case: &str) -> bool {
    use std::io::Read;
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "adapter::private_file::session_probe::session_r37_eof_quit_fatal_each_reserved_private_boundary", "--nocapture"])
        .env("M2DD_PRIVATE_CLOSE_CASE", case)
        .stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
    let mut stdout = child.stdout.take().unwrap();
    let mut stderr = child.stderr.take().unwrap();
    let out = std::thread::spawn(move || {
        let mut text = String::new();
        stdout.read_to_string(&mut text).unwrap();
        text
    });
    let err = std::thread::spawn(move || {
        let mut text = String::new();
        stderr.read_to_string(&mut text).unwrap();
        text
    });
    let deadline = Instant::now() + Duration::from_secs(20);
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break Some(status);
        }
        if Instant::now() >= deadline {
            child.kill().unwrap();
            child.wait().unwrap();
            break None;
        }
        std::thread::sleep(Duration::from_millis(5));
    };
    let stdout = out.join().unwrap();
    let stderr = err.join().unwrap();
    let reached = stdout.lines().any(|line| line.starts_with("R37-CLOSE/"));
    let passed = status.is_some_and(|status| status.success()) && reached;
    print!("{stdout}");
    if !passed {
        eprintln!("private child {case} failed: {stderr}");
    }
    passed
}
