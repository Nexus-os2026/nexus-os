//! P2C live controls for the verifier sandbox helper.
//!
//! `harness = false`: this executable is the test driver and, copied into a
//! fixture "toolchain", also the untrusted probe that runs inside the real
//! helper and attempts every escape. The driver also runs the probe with a
//! single layer (or none) applied, proving that each check can observe the
//! escape its layer prevents.
//!
//! On a host that cannot provide the sandbox the suite asserts that the
//! helper fails closed (nothing runs), unless
//! `NEXUS_PHASE2_REQUIRE_LIVE_SANDBOX=1` demands a supported host, in which
//! case it fails.

fn main() {
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    live::main();
    #[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
    println!("p2c live sandbox: not an x86_64 Linux build; the helper is unavailable here");
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
mod live {
    use std::collections::{BTreeMap, BTreeSet};
    use std::ffi::CString;
    use std::fs;
    use std::io::{Read, Write};
    use std::net::{Shutdown, TcpListener, TcpStream, UdpSocket};
    use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
    use std::os::linux::net::SocketAddrExt;
    use std::os::unix::fs::{symlink, PermissionsExt};
    use std::os::unix::net::{SocketAddr, UnixListener, UnixStream};
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    use nexus_verifier_sandbox::launcher::{
        Helper, HelperProgram, LaunchError, LaunchSpec, Outcome,
    };
    use nexus_verifier_sandbox::policy::Role;
    use nexus_verifier_sandbox::protocol::{SetupStage, VerifierStatus};

    const REQUIRE_LIVE: &str = "NEXUS_PHASE2_REQUIRE_LIVE_SANDBOX";
    const HELPER: &str = env!("CARGO_BIN_EXE_nexus-verifier-sandbox");
    const RUNTIME_DIR: &str = "/usr/lib/x86_64-linux-gnu";
    const RUNTIME_LIBRARIES: [&str; 7] = [
        "libc.so.6",
        "libm.so.6",
        "libdl.so.2",
        "librt.so.1",
        "libpthread.so.0",
        "libgcc_s.so.1",
        "libz.so.1",
    ];
    const LOADER: &str = "ld-linux-x86-64.so.2";

    pub fn main() {
        let args: Vec<String> = std::env::args().collect();
        match args.get(1).map(String::as_str) {
            Some("--probe") => probe::run(&args[2..]),
            Some("--probe-daemon") => probe::daemon(&args[2..]),
            Some("--ablation") => ablation::run(&args[2..]),
            Some("--deny-unshare-driver") => drivers::deny_unshare(&args[2..]),
            Some("--parent-death-driver") => drivers::parent_death(&args[2..]),
            _ => suite(),
        }
    }

    fn open_path(path: &Path) -> OwnedFd {
        let c = CString::new(path.as_os_str().as_encoded_bytes()).unwrap();
        // SAFETY: a NUL-terminated path; open returns a new descriptor.
        let fd = unsafe { libc::open(c.as_ptr(), libc::O_PATH | libc::O_CLOEXEC) };
        assert!(
            fd >= 0,
            "open {}: {}",
            path.display(),
            std::io::Error::last_os_error()
        );
        // SAFETY: open succeeded.
        unsafe { OwnedFd::from_raw_fd(fd) }
    }

    /// A disposable fixture: a toolchain holding a copy of this executable,
    /// a candidate input, scratch directories, and host content outside every
    /// allowed root.
    pub struct Fixture {
        pub root: PathBuf,
        pub toolchain: PathBuf,
        pub probe: PathBuf,
        pub input: PathBuf,
        pub target: PathBuf,
        pub home: PathBuf,
        pub tmp: PathBuf,
        pub outside: PathBuf,
    }

    impl Fixture {
        pub fn new(tag: &str) -> Self {
            let nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let root = std::env::temp_dir()
                .join(format!("nexus-p2c-{tag}-{}-{nanos}", std::process::id()));
            let fixture = Fixture {
                toolchain: root.join("toolchain"),
                probe: root.join("toolchain").join("probe"),
                input: root.join("input"),
                target: root.join("scratch").join("target"),
                home: root.join("scratch").join("home"),
                tmp: root.join("scratch").join("tmp"),
                outside: root.join("outside"),
                root,
            };
            for dir in [
                &fixture.toolchain,
                &fixture.input,
                &fixture.target,
                &fixture.home,
                &fixture.tmp,
                &fixture.outside.join(".ssh"),
            ] {
                fs::create_dir_all(dir).unwrap();
            }
            fs::copy(std::env::current_exe().unwrap(), &fixture.probe).unwrap();
            fs::set_permissions(&fixture.probe, fs::Permissions::from_mode(0o755)).unwrap();
            fs::write(fixture.input.join("data.txt"), b"candidate input\n").unwrap();
            symlink(
                "../outside/sentinel.txt",
                fixture.input.join("escape-relative"),
            )
            .unwrap();
            symlink(
                fixture.outside.join("sentinel.txt"),
                fixture.input.join("escape-absolute"),
            )
            .unwrap();
            fs::write(fixture.outside.join("sentinel.txt"), b"P2C-SENTINEL\n").unwrap();
            fs::write(
                fixture.outside.join(".git-credentials"),
                b"https://synthetic:not-a-real-token@example.invalid\n",
            )
            .unwrap();
            fs::write(
                fixture.outside.join(".ssh").join("id_ed25519"),
                b"-----BEGIN SYNTHETIC TEST KEY-----\n",
            )
            .unwrap();
            fs::write(
                fixture.outside.join("api-key.env"),
                b"API_KEY=synthetic-not-real\n",
            )
            .unwrap();
            let chmod_target = fixture.outside.join("chmod-target");
            fs::write(&chmod_target, b"x").unwrap();
            fs::set_permissions(&chmod_target, fs::Permissions::from_mode(0o600)).unwrap();
            fixture
        }

        /// A fixture another process created and owns.
        pub fn at(root: &Path) -> std::mem::ManuallyDrop<Self> {
            std::mem::ManuallyDrop::new(Fixture {
                toolchain: root.join("toolchain"),
                probe: root.join("toolchain").join("probe"),
                input: root.join("input"),
                target: root.join("scratch").join("target"),
                home: root.join("scratch").join("home"),
                tmp: root.join("scratch").join("tmp"),
                outside: root.join("outside"),
                root: root.to_path_buf(),
            })
        }

        pub fn env(&self) -> Vec<Vec<u8>> {
            vec![
                format!("HOME={}", self.home.display()).into_bytes(),
                format!("TMPDIR={}", self.tmp.display()).into_bytes(),
                b"LC_ALL=C".to_vec(),
                b"PROBE_ENV_OK=1".to_vec(),
            ]
        }

        /// Rules exactly as the production policy roles would be granted.
        pub fn rules(&self) -> Vec<(Role, OwnedFd)> {
            let mut rules = vec![
                (Role::ToolchainRoot, open_path(&self.toolchain)),
                (Role::CandidateInput, open_path(&self.input)),
                (Role::Target, open_path(&self.target)),
                (Role::Scratch, open_path(&self.home)),
                (Role::Scratch, open_path(&self.tmp)),
                (Role::DevNull, open_path(Path::new("/dev/null"))),
                (Role::DevUrandom, open_path(Path::new("/dev/urandom"))),
                (
                    Role::RuntimeLoader,
                    open_path(&Path::new(RUNTIME_DIR).join(LOADER)),
                ),
            ];
            for library in RUNTIME_LIBRARIES {
                let path = Path::new(RUNTIME_DIR).join(library);
                if path.exists() {
                    rules.push((Role::RuntimeLibrary, open_path(&path)));
                }
            }
            rules
        }

        pub fn spec(&self, generation: u64, argv: Vec<String>) -> LaunchSpec {
            LaunchSpec {
                generation,
                executable: open_path(&self.probe),
                working_directory: open_path(&self.input),
                argv: argv.into_iter().map(String::into_bytes).collect(),
                env: self.env(),
                rules: self.rules(),
            }
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    /// Host-side listeners outside the sandbox; they count every contact.
    pub struct Listeners {
        pub tcp_port: u16,
        pub udp_port: u16,
        pub abstract_name: String,
        pub pathname: PathBuf,
        pub contacts: Arc<AtomicUsize>,
    }

    impl Listeners {
        pub fn start(outside: &Path) -> Self {
            let contacts = Arc::new(AtomicUsize::new(0));
            let tcp = TcpListener::bind("127.0.0.1:0").unwrap();
            let udp = UdpSocket::bind("127.0.0.1:0").unwrap();
            let abstract_name = format!("nexus-p2c-{}-{}", std::process::id(), outside.display());
            let abstract_listener =
                UnixListener::bind_addr(&SocketAddr::from_abstract_name(&abstract_name).unwrap())
                    .unwrap();
            let pathname = outside.join("host.sock");
            let path_listener = UnixListener::bind(&pathname).unwrap();
            let listeners = Listeners {
                tcp_port: tcp.local_addr().unwrap().port(),
                udp_port: udp.local_addr().unwrap().port(),
                abstract_name,
                pathname,
                contacts: contacts.clone(),
            };
            let c = contacts.clone();
            std::thread::spawn(move || {
                for stream in tcp.incoming().flatten() {
                    c.fetch_add(1, Ordering::SeqCst);
                    let _ = stream.shutdown(Shutdown::Both);
                }
            });
            for listener in [abstract_listener, path_listener] {
                let c = contacts.clone();
                std::thread::spawn(move || {
                    for stream in listener.incoming().flatten() {
                        c.fetch_add(1, Ordering::SeqCst);
                        drop(stream);
                    }
                });
            }
            let c = contacts;
            std::thread::spawn(move || {
                let mut buf = [0u8; 64];
                while udp.recv_from(&mut buf).is_ok() {
                    c.fetch_add(1, Ordering::SeqCst);
                }
            });
            listeners
        }

        pub fn probe_args(&self) -> Vec<String> {
            vec![
                format!("tcp={}", self.tcp_port),
                format!("udp={}", self.udp_port),
                format!("abstract={}", self.abstract_name),
                format!("pathname={}", self.pathname.display()),
            ]
        }
    }

    fn real_home() -> PathBuf {
        // SAFETY: getpwuid returns a pointer into static storage or null.
        let pw = unsafe { libc::getpwuid(libc::getuid()) };
        assert!(!pw.is_null());
        // SAFETY: pw_dir is a NUL-terminated string owned by libc.
        let dir = unsafe { std::ffi::CStr::from_ptr((*pw).pw_dir) };
        PathBuf::from(dir.to_str().unwrap())
    }

    fn probe_argv(
        fixture: &Fixture,
        listeners: &Listeners,
        marker: &str,
        extra: &[String],
    ) -> Vec<String> {
        let home = real_home();
        let mut argv = vec![
            "probe".to_string(),
            "--probe".to_string(),
            format!(
                "sentinel={}",
                fixture.outside.join("sentinel.txt").display()
            ),
            format!("outside={}", fixture.outside.display()),
            format!("input={}", fixture.input.display()),
            format!("target={}", fixture.target.display()),
            format!("real_home={}", home.display()),
            format!("nexus_dir={}", home.join(".nexus").display()),
            format!("driver_pid={}", std::process::id()),
            format!("self_exe={}", fixture.probe.display()),
            format!("marker={marker}"),
            "env_keys=HOME,LC_ALL,PROBE_ENV_OK,TMPDIR".to_string(),
        ];
        argv.extend(listeners.probe_args());
        argv.extend(extra.iter().cloned());
        argv
    }

    /// Parse `check NAME ALLOWED|DENIED ERRNO` lines.
    fn parse(output: &str) -> BTreeMap<String, (bool, i32)> {
        output
            .lines()
            .filter_map(|line| {
                let mut parts = line.split_whitespace();
                (parts.next()? == "check").then_some(())?;
                let name = parts.next()?.to_string();
                let allowed = parts.next()? == "ALLOWED";
                let errno = parts.next()?.parse().ok()?;
                Some((name, (allowed, errno)))
            })
            .collect()
    }

    fn marker_processes(marker: &str) -> usize {
        let mut count = 0;
        for entry in fs::read_dir("/proc").unwrap().flatten() {
            let name = entry.file_name();
            let Some(pid) = name.to_str().and_then(|n| n.parse::<u32>().ok()) else {
                continue;
            };
            if pid == std::process::id() {
                continue;
            }
            if let Ok(cmdline) = fs::read(format!("/proc/{pid}/cmdline")) {
                let text = String::from_utf8_lossy(&cmdline);
                if text.contains(marker) && text.contains("--probe-daemon") {
                    count += 1;
                }
            }
        }
        count
    }

    fn wait_until(deadline: Duration, mut done: impl FnMut() -> bool) -> bool {
        let start = Instant::now();
        while start.elapsed() < deadline {
            if done() {
                return true;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        done()
    }

    fn read_all(fd: OwnedFd) -> std::thread::JoinHandle<String> {
        std::thread::spawn(move || {
            let mut text = String::new();
            let _ = fs::File::from(fd).read_to_string(&mut text);
            text
        })
    }

    pub enum Launched {
        Ran {
            outcome: Outcome,
            stdout: String,
            stderr: String,
        },
        Refused(LaunchError),
    }

    /// Spawn the real helper and run one launch to completion.
    pub fn launch(spec: LaunchSpec) -> Launched {
        let (mut helper, output) = Helper::spawn(&HelperProgram::at(HELPER)).expect("spawn helper");
        let stdout = read_all(output.stdout);
        let stderr = read_all(output.stderr);
        let result = helper.handshake().and_then(|()| helper.launch(spec));
        match result {
            Err(error) => {
                let _ = helper.kill();
                let _ = helper.reap();
                let _ = (stdout.join(), stderr.join());
                Launched::Refused(error)
            }
            Ok(()) => {
                let outcome = helper
                    .wait_report(Duration::from_secs(120))
                    .expect("report")
                    .expect("finished in time");
                let status = helper.reap().expect("reap helper");
                assert!(status.success(), "helper exit status {status:?}");
                Launched::Ran {
                    outcome,
                    stdout: stdout.join().unwrap(),
                    stderr: stderr.join().unwrap(),
                }
            }
        }
    }

    fn required() -> bool {
        std::env::var(REQUIRE_LIVE).as_deref() == Ok("1")
    }

    fn run_case(name: &str, case: fn()) {
        print!("test {name} ... ");
        let _ = std::io::stdout().flush();
        case();
        println!("ok");
    }

    fn suite() {
        // Establish whether this host can run the sandbox at all.
        let fixture = Fixture::new("probe-host");
        let listeners = Listeners::start(&fixture.outside);
        let spec = fixture.spec(
            1,
            probe_argv(&fixture, &listeners, "host-check", &["only=noop".into()]),
        );
        match launch(spec) {
            Launched::Ran { .. } => {}
            Launched::Refused(error) => {
                assert!(
                    !required(),
                    "{REQUIRE_LIVE}=1 but the sandbox is unavailable on this host: {error:?}"
                );
                assert!(
                    matches!(error, LaunchError::SetupFailed { .. }),
                    "an unsupported host must fail closed at setup: {error:?}"
                );
                assert!(
                    fs::read_dir(&fixture.target).unwrap().next().is_none(),
                    "nothing ran"
                );
                println!(
                    "p2c live sandbox: this host cannot provide the sandbox ({error:?}); \
                     verified that the helper fails closed and nothing ran"
                );
                return;
            }
        }
        drop((fixture, listeners));
        run_case(
            "p2c_live_escape_attempts_are_all_denied",
            cases::escapes_denied,
        );
        run_case("p2c_live_each_layer_is_necessary", cases::layer_ablation);
        run_case(
            "p2c_live_descendants_do_not_survive_init_exit",
            cases::descendants_die,
        );
        run_case(
            "p2c_live_parent_death_ends_the_sandbox",
            cases::parent_death,
        );
        run_case(
            "p2c_live_missing_namespace_fails_closed",
            cases::missing_namespace,
        );
        run_case(
            "p2c_live_malformed_launch_runs_nothing",
            cases::malformed_launch,
        );
        run_case("p2d_live_missing_scope_fails_closed", p2d::missing_scope);
        run_case(
            "p2f_live_packaged_toolchain_verifies",
            p2f::packaged_toolchain,
        );
        let mut passed = 8;
        match p2d::ScopeManager::connect() {
            Err(error) => {
                assert!(
                    !required(),
                    "{REQUIRE_LIVE}=1 but no user-manager cgroup scope is available: {error:?}"
                );
                println!(
                    "phase2 live sandbox: no user-manager cgroup scope on this host ({error:?}); \
                     verification is unavailable here: no execution can start without a scope"
                );
            }
            Ok(scopes) => {
                type ScopedCase = (&'static str, fn(&p2d::ScopeManager));
                let scoped: [ScopedCase; 10] = [
                    (
                        "p2d_live_execution_runs_in_a_verified_scope",
                        p2d::scoped_run_passes,
                    ),
                    (
                        "p2d_live_helper_holds_the_scope_until_released",
                        p2d::helper_holds_the_scope,
                    ),
                    (
                        "p2d_live_deadline_kill_reaches_every_descendant",
                        p2d::deadline_kill_reaches_descendants,
                    ),
                    ("p2d_live_pids_limit_is_enforced", p2d::pids_limit),
                    ("p2d_live_memory_limit_is_enforced", p2d::memory_limit),
                    (
                        "p2d_live_descendant_oom_is_never_a_pass",
                        p2d::descendant_oom,
                    ),
                    ("p2d_live_output_flood_is_bounded", p2d::output_flood),
                    (
                        "p2d_live_unmovable_process_fails_closed",
                        p2d::unmovable_process,
                    ),
                    (
                        "p2e_live_workspace_is_private_and_path_bound",
                        p2e::workspace_is_private,
                    ),
                    (
                        "p2e_live_workspace_confines_the_verifier",
                        p2e::workspace_confines,
                    ),
                ];
                for (name, case) in scoped {
                    print!("test {name} ... ");
                    let _ = std::io::stdout().flush();
                    case(&scopes);
                    println!("ok");
                    passed += 1;
                }
            }
        }
        println!("test result: ok. {passed} live sandbox cases passed");
    }

    mod p2d {
        use super::*;
        pub use nexus_verifier_sandbox::execution::{self, EndedBy, ExitClass};
        pub use nexus_verifier_sandbox::policy::ResourcePolicy;
        pub use nexus_verifier_sandbox::scope::{Occupancy, ScopeError, ScopeManager};

        fn limits() -> ResourcePolicy {
            ResourcePolicy {
                wall_timeout_secs: 60,
                runtime_backstop_secs: 120,
                ..ResourcePolicy::RUST_OFFLINE_V1
            }
        }

        fn run_probe(
            scopes: &ScopeManager,
            tag: &str,
            only: &str,
            marker: &str,
            limits: &ResourcePolicy,
        ) -> (execution::ExecutionReport, BTreeMap<String, (bool, i32)>) {
            let fixture = Fixture::new(tag);
            let listeners = Listeners::start(&fixture.outside);
            let spec = fixture.spec(
                10,
                probe_argv(&fixture, &listeners, marker, &[format!("only={only}")]),
            );
            let report = execution::run(scopes, &HelperProgram::at(HELPER), spec, limits);
            let checks = parse(&String::from_utf8_lossy(&report.stdout.excerpt));
            (report, checks)
        }

        pub fn scoped_run_passes(scopes: &ScopeManager) {
            let (report, checks) = run_probe(scopes, "scoped", "noop", "scoped", &limits());
            assert!(report.not_run.is_none(), "{:?}", report.not_run);
            assert_eq!(report.classify(0), ExitClass::Passed, "{report:?}");
            assert!(report.cleanup.is_confirmed());
            assert!(checks.is_empty());
        }

        pub fn helper_holds_the_scope(scopes: &ScopeManager) {
            let fixture = Fixture::new("hold");
            let listeners = Listeners::start(&fixture.outside);
            let spec = fixture.spec(
                11,
                probe_argv(&fixture, &listeners, "hold", &["only=noop".into()]),
            );
            let (helper, output) = Helper::spawn(&HelperProgram::at(HELPER)).unwrap();
            let stdout = read_all(output.stdout);
            let stderr = read_all(output.stderr);
            let scope = scopes.start(&helper, &limits()).unwrap();
            helper.handshake().unwrap();
            helper.launch(spec).unwrap();
            let outcome = helper.wait_report(Duration::from_secs(60)).unwrap();
            assert_eq!(outcome, Some(Outcome::Finished(VerifierStatus::Exited(0))));
            // After its final report the helper keeps the scope, and with it
            // the kernel's counters, until it is released.
            std::thread::sleep(Duration::from_secs(1));
            assert_eq!(scope.occupancy().unwrap(), Occupancy::Populated);
            let events = scope.events().unwrap();
            assert_eq!((events.oom_kills, events.pids_max), (0, 0));
            scope.kill().unwrap();
            helper.reap().unwrap();
            assert!(scope.wait_empty(Duration::from_secs(10)).unwrap());
            // The manager then removes the emptied scope; the removal is
            // recognised through the retained descriptor alone.
            assert!(wait_until(Duration::from_secs(10), || {
                scope.occupancy().ok() == Some(Occupancy::Removed)
            }));
            assert!(scope.events().is_err(), "no counters outlive the scope");
            let _ = (stdout.join(), stderr.join());
        }

        pub fn deadline_kill_reaches_descendants(scopes: &ScopeManager) {
            let marker = format!("p2d-deadline-{}", std::process::id());
            let limits = ResourcePolicy {
                wall_timeout_secs: 3,
                ..limits()
            };
            let started = Instant::now();
            let (report, _) = run_probe(scopes, "deadline", "hang", &marker, &limits);
            assert_eq!(report.ended_by, Some(EndedBy::Deadline), "{report:?}");
            assert_eq!(report.classify(0), ExitClass::TimedOut);
            assert!(report.cleanup.is_confirmed());
            assert!(started.elapsed() < Duration::from_secs(30));
            assert_eq!(
                marker_processes(&marker),
                0,
                "cgroup.kill reached every detached descendant"
            );
        }

        pub fn pids_limit(scopes: &ScopeManager) {
            let limits = ResourcePolicy {
                pids_max: 16,
                ..limits()
            };
            let (report, checks) = run_probe(scopes, "pids", "threads", "pids", &limits);
            assert!(report.events.is_some_and(|e| e.pids_max > 0), "{report:?}");
            assert_eq!(report.classify(0), ExitClass::ProcessLimit);
            let refused = checks.get("threads_refused").map(|c| c.1).unwrap_or(0);
            assert!(refused > 0, "{checks:?}");
            assert!(report.cleanup.is_confirmed());
        }

        pub fn memory_limit(scopes: &ScopeManager) {
            let limits = ResourcePolicy {
                memory_max_bytes: 64 * 1024 * 1024,
                ..limits()
            };
            let (report, checks) = run_probe(scopes, "memory", "alloc", "memory", &limits);
            assert!(report.events.is_some_and(|e| e.oom_kills > 0), "{report:?}");
            assert_eq!(report.classify(0), ExitClass::OomKilled);
            assert!(
                !checks.contains_key("allocated"),
                "the allocation never completed"
            );
            assert!(report.cleanup.is_confirmed());
        }

        pub fn descendant_oom(scopes: &ScopeManager) {
            // A descendant is killed for memory while the verifier carries
            // on and exits normally, as a compiler under a build tool would.
            // The manager must not end the scope for it (the counters would
            // be lost) and the run must never be reported as passed.
            let limits = ResourcePolicy {
                memory_max_bytes: 64 * 1024 * 1024,
                ..limits()
            };
            let (report, checks) =
                run_probe(scopes, "oom-child", "alloc-child", "oom-child", &limits);
            assert_eq!(
                report.outcome,
                Some(Outcome::Finished(VerifierStatus::Exited(0))),
                "{report:?}"
            );
            assert_eq!(
                checks.get("child_killed_by_signal").map(|c| c.1),
                Some(libc::SIGKILL),
                "{checks:?}"
            );
            assert!(report.events.is_some_and(|e| e.oom_kills > 0), "{report:?}");
            assert_eq!(report.classify(0), ExitClass::OomKilled);
            assert!(report.cleanup.is_confirmed());
        }

        pub fn output_flood(scopes: &ScopeManager) {
            let limits = ResourcePolicy {
                output_ceiling_bytes: 64 * 1024,
                excerpt_bytes: 1024,
                ..limits()
            };
            let (report, _) = run_probe(scopes, "flood", "flood", "flood", &limits);
            assert_eq!(
                report.classify(0),
                ExitClass::OutputLimitExceeded,
                "{report:?}"
            );
            assert!(report.stdout.truncated);
            assert!(report.stdout.bytes > 64 * 1024);
            assert!(report.stdout.excerpt.len() <= 1024);
            assert!(report.cleanup.is_confirmed());
        }

        /// Transient verifier scopes the user manager currently has loaded
        /// (observation only).
        fn loaded_scopes() -> usize {
            let out = Command::new("systemctl")
                .args(["--user", "list-units", "--all", "--plain", "--no-legend"])
                .arg("nexus-verifier-*.scope")
                .output()
                .unwrap();
            String::from_utf8_lossy(&out.stdout).lines().count()
        }

        pub fn unmovable_process(scopes: &ScopeManager) {
            // A helper that is no longer running cannot be placed in a
            // scope: no scope is confirmed for it, and the empty unit made
            // for it is not left behind.
            let before = loaded_scopes();
            let (mut helper, _output) = Helper::spawn(&HelperProgram::at(HELPER)).unwrap();
            helper.kill().unwrap();
            let refused = scopes.start(&helper, &limits());
            assert!(refused.is_err(), "{refused:?}");
            helper.reap().unwrap();
            assert!(
                wait_until(Duration::from_secs(10), || loaded_scopes() <= before),
                "the refused scope was stopped"
            );
        }

        pub fn missing_scope() {
            assert!(matches!(
                ScopeManager::connect_at("/nonexistent/nexus-p2d/bus"),
                Err(ScopeError::BusUnavailable(_))
            ));
            let fixture = Fixture::new("nobus");
            let file = fixture.outside.join("bus-file");
            fs::write(&file, b"").unwrap();
            assert!(matches!(
                ScopeManager::connect_at(file.to_str().unwrap()),
                Err(ScopeError::BusUnavailable(_))
            ));
            // A socket that does not speak D-Bus is not a user manager.
            let path = fixture.outside.join("not-dbus.sock");
            let listener = UnixListener::bind(&path).unwrap();
            std::thread::spawn(move || {
                for stream in listener.incoming().flatten() {
                    drop(stream);
                }
            });
            assert!(ScopeManager::connect_at(path.to_str().unwrap()).is_err());
        }
    }

    mod p2f {
        use super::*;
        use nexus_verifier_sandbox::toolchain::{ToolchainError, VerifiedVerifierToolchain};

        /// The assembled development tree (`packaging/verifier-toolchain`).
        pub fn development_root() -> PathBuf {
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../app/src-tauri/verifier-toolchain")
                .canonicalize()
                .expect("an assembled verifier toolchain (packaging/verifier-toolchain)")
        }

        /// The development toolchain, when this build can verify one.
        #[cfg(feature = "development-toolchain")]
        pub fn development() -> Option<VerifiedVerifierToolchain> {
            nexus_verifier_sandbox::toolchain::is_packaged().then(|| {
                VerifiedVerifierToolchain::verify_development(&development_root()).unwrap()
            })
        }

        #[cfg(not(feature = "development-toolchain"))]
        pub fn development() -> Option<VerifiedVerifierToolchain> {
            None
        }

        pub fn packaged_toolchain() {
            // Whatever this build is, production verification fails closed
            // here: this test is not the installed application.
            assert_eq!(
                VerifiedVerifierToolchain::verify_installed().err(),
                Some(ToolchainError::Unavailable)
            );
            let Some(toolchain) = development() else {
                assert!(
                    !required(),
                    "{REQUIRE_LIVE}=1 requires the packaged verifier toolchain: assemble \
                     packaging/verifier-toolchain and build with \
                     NEXUS_VERIFIER_TOOLCHAIN=packaged --features development-toolchain"
                );
                println!("(no packaged verifier toolchain in this build: verified unavailable)");
                return;
            };
            toolchain.reverify().unwrap();
            assert_eq!(toolchain.rust_version(), "1.94.0");
            let launch = toolchain.launch().unwrap();
            assert_eq!(launch.rules.len(), 9);
            assert_eq!(launch.rustc, development_root().join("bin/rustc"));
            let again = development().expect("verified again");
            assert_eq!(again.digest(), toolchain.digest());
            assert!(again.generation() > toolchain.generation());
        }
    }

    mod p2e {
        use super::p2d::{execution, ExitClass, ResourcePolicy, ScopeManager};
        use super::*;
        use nexus_verifier_sandbox::workspace::{Area, Workspace, WorkspaceError, WorkspaceRoot};
        use std::os::unix::fs::MetadataExt;

        fn limits() -> ResourcePolicy {
            ResourcePolicy {
                wall_timeout_secs: 60,
                runtime_backstop_secs: 120,
                ..ResourcePolicy::RUST_OFFLINE_V1
            }
        }

        /// File bytes by relative path, from the backend's view.
        fn contents(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
            fn walk(root: &Path, dir: &Path, out: &mut BTreeMap<PathBuf, Vec<u8>>) {
                for entry in fs::read_dir(dir).unwrap().flatten() {
                    let path = entry.path();
                    let kind = fs::symlink_metadata(&path).unwrap().file_type();
                    if kind.is_dir() {
                        walk(root, &path, out);
                    } else {
                        let bytes = if kind.is_file() {
                            fs::read(&path).unwrap()
                        } else {
                            b"<not a regular file>".to_vec()
                        };
                        out.insert(path.strip_prefix(root).unwrap().to_path_buf(), bytes);
                    }
                }
            }
            let mut out = BTreeMap::new();
            walk(root, root, &mut out);
            out
        }

        pub fn workspace_is_private(_: &ScopeManager) {
            let root = WorkspaceRoot::derive().unwrap();
            let ws = Workspace::create(&root).unwrap();
            // SAFETY: getuid has no preconditions.
            let uid = unsafe { libc::getuid() };
            let base = PathBuf::from(format!("/run/user/{uid}/nexus-verifier"));
            assert_eq!(fs::metadata(&base).unwrap().mode() & 0o7777, 0o700);
            for area in Area::ALL {
                let path = ws.env_path(area);
                assert!(path.starts_with(&base), "{path:?}");
                let meta = fs::symlink_metadata(&path).unwrap();
                assert!(meta.is_dir());
                assert_eq!((meta.uid(), meta.mode() & 0o7777), (uid, 0o700));
            }
            ws.verify_paths().unwrap();
            // A path that no longer names its retained directory is refused,
            // and the directory now at that name is never removed.
            let scratch = ws.env_path(Area::Scratch);
            let moved = scratch.with_file_name("scratch-moved");
            fs::rename(&scratch, &moved).unwrap();
            fs::create_dir(&scratch).unwrap();
            fs::write(scratch.join("sentinel"), b"keep").unwrap();
            assert!(matches!(
                ws.verify_paths(),
                Err(WorkspaceError::PathChanged(Area::Scratch))
            ));
            let workspace_dir = scratch.parent().unwrap().to_path_buf();
            let retained = ws.remove().unwrap_err();
            assert_eq!(fs::read(scratch.join("sentinel")).unwrap(), b"keep");
            // Put back, the retained workspace is removed on retry.
            fs::remove_dir_all(&scratch).unwrap();
            fs::rename(&moved, &scratch).unwrap();
            retained.retry().unwrap();
            assert!(!workspace_dir.exists());
        }

        pub fn workspace_confines(scopes: &ScopeManager) {
            let fixture = Fixture::new("workspace");
            let listeners = Listeners::start(&fixture.outside);
            let root = WorkspaceRoot::derive().unwrap();
            let ws = Workspace::create(&root).unwrap();
            let input = ws.env_path(Area::Input);
            fs::create_dir(input.join("src")).unwrap();
            fs::write(input.join("src/lib.rs"), b"pub fn answer() -> u32 { 42 }\n").unwrap();
            let before = contents(&input);
            ws.verify_paths().unwrap();
            let mut rules: Vec<(Role, OwnedFd)> = fixture
                .rules()
                .into_iter()
                .filter(|(role, _)| {
                    !matches!(role, Role::CandidateInput | Role::Target | Role::Scratch)
                })
                .collect();
            rules.extend(ws.rules().unwrap());
            let mut extra = vec!["only=workspace".to_string()];
            for area in Area::ALL {
                extra.push(format!(
                    "ws_{}={}",
                    area.name().replace('-', "_"),
                    ws.env_path(area).display()
                ));
            }
            let env = vec![
                format!("HOME={}", ws.env_path(Area::Home).display()).into_bytes(),
                format!("TMPDIR={}", ws.env_path(Area::Tmp).display()).into_bytes(),
                format!("CARGO_HOME={}", ws.env_path(Area::CargoHome).display()).into_bytes(),
                b"LC_ALL=C".to_vec(),
            ];
            let spec = LaunchSpec {
                generation: 20,
                executable: open_path(&fixture.probe),
                working_directory: ws.directory(Area::Input).unwrap(),
                argv: probe_argv(&fixture, &listeners, "workspace", &extra)
                    .into_iter()
                    .map(String::into_bytes)
                    .collect(),
                env,
                rules,
            };
            let report = execution::run(scopes, &HelperProgram::at(HELPER), spec, &limits());
            assert_eq!(report.classify(0), ExitClass::Passed, "{report:?}");
            let checks = parse(&String::from_utf8_lossy(&report.stdout.excerpt));
            for name in ["ws_input_read", "ws_input_list"] {
                assert_eq!(
                    checks.get(name).map(|c| c.0),
                    Some(true),
                    "{name}: {checks:?}"
                );
            }
            for name in [
                "ws_input_write",
                "ws_input_create",
                "ws_input_remove",
                "ws_input_mkdir",
                "ws_input_rename",
                "ws_input_link_out",
                "ws_scratch_symlink",
                "ws_scratch_fifo",
            ] {
                assert_eq!(
                    checks.get(name).map(|c| c.0),
                    Some(false),
                    "{name}: {checks:?}"
                );
            }
            for name in [
                "ws_scratch_write",
                "ws_home_write",
                "ws_tmp_write",
                "ws_cargo_home_write",
                "ws_target_write",
                "ws_left_deep",
                "ws_left_locked",
                "ws_left_hidden",
                "ws_left_odd_name",
                "ws_left_hard_link",
                "ws_left_no_mode",
            ] {
                assert_eq!(
                    checks.get(name).map(|c| c.0),
                    Some(true),
                    "{name}: {checks:?}"
                );
            }
            // The candidate is exactly as the backend wrote it.
            assert_eq!(contents(&input), before);
            let dir = input.parent().unwrap().to_path_buf();
            ws.remove().unwrap();
            assert!(!dir.exists(), "everything the verifier left was removed");
        }
    }

    mod cases {
        use super::*;

        pub fn escapes_denied() {
            let fixture = Fixture::new("escape");
            let listeners = Listeners::start(&fixture.outside);
            // An inheritable, non-close-on-exec descriptor in the backend.
            let leaked = fs::File::open(fixture.outside.join("sentinel.txt")).unwrap();
            // SAFETY: clears FD_CLOEXEC on a descriptor this test owns.
            unsafe { libc::fcntl(leaked.as_raw_fd(), libc::F_SETFD, 0) };
            // A synthetic secret in the backend's own environment.
            std::env::set_var("NEXUS_P2C_PARENT_SECRET", "synthetic-not-real");
            let before_input = fs::read(fixture.input.join("data.txt")).unwrap();
            let spec = fixture.spec(2, probe_argv(&fixture, &listeners, "escape", &[]));
            let Launched::Ran {
                outcome,
                stdout,
                stderr,
            } = launch(spec)
            else {
                panic!("launch refused on a supported host");
            };
            std::env::remove_var("NEXUS_P2C_PARENT_SECRET");
            drop(leaked);
            assert_eq!(
                outcome,
                Outcome::Finished(VerifierStatus::Exited(0)),
                "{stdout}\n{stderr}"
            );
            let checks = parse(&stdout);
            let denied_any = [
                "read_sentinel",
                "write_outside",
                "list_real_home",
                "list_nexus_dir",
                "read_git_credentials",
                "read_ssh_key",
                "read_api_key",
                "read_etc_passwd",
                "symlink_relative",
                "symlink_absolute",
                "write_input_new",
                "modify_input",
                "exec_shell",
                "read_proc_status",
                "list_proc",
                "tcp_host",
                "udp_socket",
                "abstract_socket",
                "pathname_socket",
                "tcp_ollama",
                "socketpair_inet",
            ];
            for name in denied_any {
                let (allowed, _) = checks
                    .get(name)
                    .unwrap_or_else(|| panic!("{name} ran\n{stdout}"));
                assert!(!allowed, "{name} must be denied\n{stdout}");
            }
            // Denied by the seccomp layer itself (ENOSYS), not merely by a
            // missing privilege.
            for name in [
                "unshare_user",
                "setns",
                "mount",
                "chroot",
                "pivot_root",
                "io_uring_setup",
                "ptrace",
                "process_vm_readv",
                "pidfd_open",
                "pidfd_getfd",
                "clone3",
                "bpf",
                "perf_event_open",
                "keyctl",
                "userfaultfd",
                "chmod_outside",
                "utimensat_path",
                "socket_raw",
            ] {
                assert_eq!(
                    checks.get(name),
                    Some(&(false, libc::ENOSYS)),
                    "{name} must be refused by seccomp\n{stdout}"
                );
            }
            for name in [
                "read_input",
                "write_target",
                "socketpair_unix",
                "no_inherited_fds",
                "exact_environment",
                "identity_uid",
            ] {
                assert_eq!(
                    checks.get(name).map(|c| c.0),
                    Some(true),
                    "{name} must hold\n{stdout}"
                );
            }
            assert_eq!(
                checks.get("kill_driver"),
                Some(&(false, libc::ESRCH)),
                "host processes are not visible\n{stdout}"
            );
            assert_eq!(
                listeners.contacts.load(Ordering::SeqCst),
                0,
                "no host socket was reached"
            );
            assert_eq!(
                fs::read(fixture.input.join("data.txt")).unwrap(),
                before_input
            );
            let mode = fs::metadata(fixture.outside.join("chmod-target"))
                .unwrap()
                .permissions()
                .mode();
            assert_eq!(mode & 0o777, 0o600, "host metadata unchanged");
        }

        pub fn layer_ablation() {
            // No sandbox: every probe observes its escape (the checks work).
            let fixture = Fixture::new("none");
            let listeners = Listeners::start(&fixture.outside);
            let checks = ablation::observe(&fixture, &listeners, "none");
            for name in [
                "read_sentinel",
                "tcp_host",
                "udp_socket",
                "pathname_socket",
                "read_proc_status",
            ] {
                assert_eq!(
                    checks.get(name).map(|c| c.0),
                    Some(true),
                    "unsandboxed {name}: {checks:?}"
                );
            }
            assert!(listeners.contacts.load(Ordering::SeqCst) >= 3);
            drop((fixture, listeners));

            // Landlock alone: filesystem and TCP denied, but UDP and pathname
            // sockets remain reachable, so seccomp's socket policy is needed.
            let fixture = Fixture::new("landlock");
            let listeners = Listeners::start(&fixture.outside);
            let checks = ablation::observe(&fixture, &listeners, "landlock");
            for name in [
                "read_sentinel",
                "read_etc_passwd",
                "tcp_host",
                "abstract_socket",
                "read_proc_status",
            ] {
                assert_eq!(
                    checks.get(name).map(|c| c.0),
                    Some(false),
                    "landlock-only {name}: {checks:?}"
                );
            }
            for name in ["udp_socket", "pathname_socket"] {
                assert_eq!(
                    checks.get(name).map(|c| c.0),
                    Some(true),
                    "landlock-only {name}: {checks:?}"
                );
            }
            assert_eq!(
                checks.get("chmod_outside").map(|c| c.0),
                Some(true),
                "Landlock does not mediate chmod"
            );
            drop((fixture, listeners));

            // seccomp alone: sockets and escape syscalls denied, but host
            // files remain readable, so Landlock is needed.
            let fixture = Fixture::new("seccomp");
            let listeners = Listeners::start(&fixture.outside);
            let checks = ablation::observe(&fixture, &listeners, "seccomp");
            for name in [
                "tcp_host",
                "udp_socket",
                "pathname_socket",
                "abstract_socket",
                "mount",
            ] {
                assert_eq!(
                    checks.get(name).map(|c| c.0),
                    Some(false),
                    "seccomp-only {name}: {checks:?}"
                );
            }
            for name in ["read_sentinel", "read_etc_passwd", "read_proc_status"] {
                assert_eq!(
                    checks.get(name).map(|c| c.0),
                    Some(true),
                    "seccomp-only {name}: {checks:?}"
                );
            }
            assert_eq!(listeners.contacts.load(Ordering::SeqCst), 0);
            drop((fixture, listeners));

            // Namespaces alone: host loopback unreachable, but pathname
            // sockets and host files remain reachable.
            let fixture = Fixture::new("namespaces");
            let listeners = Listeners::start(&fixture.outside);
            let checks = ablation::observe(&fixture, &listeners, "namespaces");
            for name in ["tcp_host", "udp_socket", "abstract_socket"] {
                assert_eq!(
                    checks.get(name).map(|c| c.0),
                    Some(false),
                    "namespaces-only {name}: {checks:?}"
                );
            }
            for name in ["pathname_socket", "read_sentinel"] {
                assert_eq!(
                    checks.get(name).map(|c| c.0),
                    Some(true),
                    "namespaces-only {name}: {checks:?}"
                );
            }
        }

        pub fn descendants_die() {
            let fixture = Fixture::new("daemon");
            let listeners = Listeners::start(&fixture.outside);
            let marker = format!("p2c-descendant-{}", std::process::id());
            let spec = fixture.spec(
                3,
                probe_argv(&fixture, &listeners, &marker, &["only=daemons".into()]),
            );
            let Launched::Ran {
                outcome, stdout, ..
            } = launch(spec)
            else {
                panic!("launch refused on a supported host");
            };
            assert_eq!(
                outcome,
                Outcome::Finished(VerifierStatus::Exited(0)),
                "{stdout}"
            );
            let checks = parse(&stdout);
            assert_eq!(
                checks.get("spawned_daemons").map(|c| c.0),
                Some(true),
                "{stdout}"
            );
            assert!(
                wait_until(Duration::from_secs(5), || marker_processes(&marker) == 0),
                "a setsid/setpgid descendant or grandchild survived init exit"
            );
        }

        pub fn parent_death() {
            let marker = format!("p2c-parent-death-{}", std::process::id());
            let fixture = Fixture::new("parent-death");
            let mut driver = Command::new(std::env::current_exe().unwrap())
                .args([
                    "--parent-death-driver",
                    &marker,
                    &fixture.root.display().to_string(),
                ])
                .stdout(std::process::Stdio::piped())
                .spawn()
                .unwrap();
            let mut ready = String::new();
            let mut stdout = driver.stdout.take().unwrap();
            let mut byte = [0u8; 1];
            while !ready.ends_with('\n') && stdout.read(&mut byte).unwrap() == 1 {
                ready.push(byte[0] as char);
            }
            assert_eq!(ready.trim(), "READY", "the inner driver launched");
            assert!(
                wait_until(Duration::from_secs(5), || marker_processes(&marker) >= 1),
                "the sandboxed process is running"
            );
            driver.kill().unwrap();
            let _ = driver.wait();
            assert!(
                wait_until(Duration::from_secs(5), || marker_processes(&marker) == 0),
                "the sandbox outlived its backend"
            );
        }

        pub fn missing_namespace() {
            let fixture = Fixture::new("nons");
            let status = Command::new(std::env::current_exe().unwrap())
                .args(["--deny-unshare-driver", &fixture.root.display().to_string()])
                .status()
                .unwrap();
            assert!(
                status.success(),
                "a host without namespaces must fail closed"
            );
        }

        pub fn malformed_launch() {
            let fixture = Fixture::new("malformed");
            let listeners = Listeners::start(&fixture.outside);
            let base = || fixture.spec(4, probe_argv(&fixture, &listeners, "malformed", &[]));
            // A working directory that is a regular file.
            let mut file_cwd = base();
            file_cwd.working_directory = open_path(&fixture.input.join("data.txt"));
            // Directory rules a backend mistake might pass: the filesystem
            // root, /proc and /sys must never be granted.
            let mut root_rule = base();
            root_rule
                .rules
                .push((Role::Scratch, open_path(Path::new("/"))));
            let mut proc_rule = base();
            proc_rule
                .rules
                .push((Role::CandidateInput, open_path(Path::new("/proc"))));
            let mut sys_rule = base();
            sys_rule
                .rules
                .push((Role::ToolchainRoot, open_path(Path::new("/sys"))));
            for (what, spec) in [
                ("file working directory", file_cwd),
                ("root directory rule", root_rule),
                ("/proc rule", proc_rule),
                ("/sys rule", sys_rule),
            ] {
                match launch(spec) {
                    Launched::Refused(LaunchError::SetupFailed {
                        stage: SetupStage::Protocol,
                        ..
                    }) => {}
                    Launched::Refused(other) => panic!("{what}: unexpected refusal {other:?}"),
                    Launched::Ran { .. } => panic!("{what}: a malformed launch ran"),
                }
            }
            assert!(
                fs::read_dir(&fixture.target).unwrap().next().is_none(),
                "nothing ran"
            );
        }
    }

    mod drivers {
        use super::*;
        use seccompiler::{BpfProgram, SeccompAction, SeccompFilter, TargetArch};

        /// Runs with `unshare` denied (as on a host without user
        /// namespaces): the helper must stop at `Unshare` and run nothing.
        pub fn deny_unshare(args: &[String]) {
            let root = PathBuf::from(&args[0]);
            let filter = SeccompFilter::new(
                [(libc::SYS_unshare, vec![])].into_iter().collect(),
                SeccompAction::Allow,
                SeccompAction::Errno(libc::EPERM as u32),
                TargetArch::x86_64,
            )
            .unwrap();
            let program: BpfProgram = filter.try_into().unwrap();
            seccompiler::apply_filter(&program).unwrap();
            // The outer test owns the directory.
            let fixture = Fixture::at(&root);
            let listeners = Listeners::start(&fixture.outside);
            let spec = fixture.spec(
                5,
                probe_argv(&fixture, &listeners, "nons", &["only=noop".into()]),
            );
            match launch(spec) {
                Launched::Refused(LaunchError::SetupFailed {
                    stage: SetupStage::Unshare,
                    errno,
                }) if errno == libc::EPERM => {}
                Launched::Refused(other) => panic!("expected an Unshare refusal, got {other:?}"),
                Launched::Ran { .. } => panic!("the helper ran without namespaces"),
            }
            assert!(fs::read_dir(root.join("scratch").join("target"))
                .unwrap()
                .next()
                .is_none());
        }

        /// Launches a long-running sandboxed process, reports READY, then
        /// waits to be killed by the outer test.
        pub fn parent_death(args: &[String]) {
            let marker = &args[0];
            let fixture = Fixture::at(Path::new(&args[1]));
            let listeners = Listeners::start(&fixture.outside);
            let (helper, output) = Helper::spawn(&HelperProgram::at(HELPER)).unwrap();
            let _stdout = read_all(output.stdout);
            let _stderr = read_all(output.stderr);
            helper.handshake().unwrap();
            let mut argv = probe_argv(&fixture, &listeners, marker, &[]);
            argv[1] = "--probe-daemon".to_string();
            helper.launch(fixture.spec(6, argv)).unwrap();
            println!("READY");
            let _ = std::io::stdout().flush();
            std::thread::sleep(Duration::from_secs(300));
        }
    }

    pub mod ablation {
        use super::*;

        /// Run the probe outside the helper with only `layer` applied.
        pub fn observe(
            fixture: &Fixture,
            listeners: &Listeners,
            layer: &str,
        ) -> BTreeMap<String, (bool, i32)> {
            let probe = probe_argv(fixture, listeners, &format!("ablation-{layer}"), &[]);
            let mut args = vec!["--ablation".to_string(), layer.to_string()];
            args.extend(probe[2..].iter().cloned());
            let output = Command::new(std::env::current_exe().unwrap())
                .args(&args)
                .env_clear()
                .envs(fixture.env().iter().map(|e| {
                    let text = String::from_utf8(e.clone()).unwrap();
                    let (k, v) = text.split_once('=').unwrap();
                    (k.to_string(), v.to_string())
                }))
                .output()
                .unwrap();
            assert!(output.status.success(), "ablation {layer}: {output:?}");
            parse(&String::from_utf8_lossy(&output.stdout))
        }

        /// In this child: apply one layer, then exec the probe.
        pub fn run(args: &[String]) {
            let layer = args[0].clone();
            let config: BTreeMap<String, String> = args[1..]
                .iter()
                .filter_map(|a| {
                    a.split_once('=')
                        .map(|(k, v)| (k.to_string(), v.to_string()))
                })
                .collect();
            let root = PathBuf::from(&config["input"])
                .parent()
                .unwrap()
                .to_path_buf();
            // The driver owns the directory.
            let fixture = Fixture::at(&root);
            match layer.as_str() {
                "none" => {}
                "landlock" => {
                    let rules = fixture.rules();
                    let borrowed: Vec<_> = rules
                        .iter()
                        .map(|(role, fd)| (*role, std::os::fd::AsFd::as_fd(fd)))
                        .collect();
                    nexus_verifier_sandbox::landlock_rules::restrict_self(&borrowed)
                        .expect("landlock");
                }
                "seccomp" => {
                    let program = nexus_verifier_sandbox::seccomp::program().unwrap();
                    nexus_verifier_sandbox::seccomp::install(&program).expect("seccomp");
                }
                "namespaces" => {
                    // SAFETY: unshare changes only this process.
                    let rc = unsafe { libc::unshare(libc::CLONE_NEWUSER | libc::CLONE_NEWNET) };
                    assert_eq!(rc, 0, "unshare: {}", std::io::Error::last_os_error());
                }
                other => panic!("unknown layer {other}"),
            }
            let probe = CString::new(root_probe(&config)).unwrap();
            let mut argv: Vec<CString> = vec![
                CString::new("probe").unwrap(),
                CString::new("--probe").unwrap(),
            ];
            argv.extend(args[1..].iter().map(|a| CString::new(a.as_str()).unwrap()));
            let mut argv_ptrs: Vec<_> = argv.iter().map(|a| a.as_ptr()).collect();
            argv_ptrs.push(std::ptr::null());
            let env: Vec<CString> = std::env::vars()
                .map(|(k, v)| CString::new(format!("{k}={v}")).unwrap())
                .collect();
            let mut env_ptrs: Vec<_> = env.iter().map(|e| e.as_ptr()).collect();
            env_ptrs.push(std::ptr::null());
            // SAFETY: NUL-terminated argument and environment arrays.
            unsafe { libc::execve(probe.as_ptr(), argv_ptrs.as_ptr(), env_ptrs.as_ptr()) };
            panic!("exec probe: {}", std::io::Error::last_os_error());
        }

        fn root_probe(config: &BTreeMap<String, String>) -> String {
            config["self_exe"].clone()
        }
    }

    /// Runs inside the sandbox (or an ablation). Prints one line per check.
    pub mod probe {
        use super::*;
        use std::os::unix::process::{CommandExt, ExitStatusExt};

        fn report(name: &str, result: std::io::Result<()>) {
            match result {
                Ok(()) => println!("check {name} ALLOWED 0"),
                Err(error) => {
                    println!("check {name} DENIED {}", error.raw_os_error().unwrap_or(-1))
                }
            }
        }

        fn raw(name: &str, rc: libc::c_long) {
            if rc < 0 {
                println!(
                    "check {name} DENIED {}",
                    std::io::Error::last_os_error().raw_os_error().unwrap_or(-1)
                );
            } else {
                println!("check {name} ALLOWED 0");
            }
        }

        fn holds(name: &str, ok: bool) {
            println!("check {name} {} 0", if ok { "ALLOWED" } else { "DENIED" });
        }

        fn config(args: &[String]) -> BTreeMap<String, String> {
            args.iter()
                .filter_map(|a| {
                    a.split_once('=')
                        .map(|(k, v)| (k.to_string(), v.to_string()))
                })
                .collect()
        }

        pub fn daemon(args: &[String]) {
            // A detached descendant that tries to outlive the sandbox.
            // SAFETY: setsid changes only this process's session.
            unsafe { libc::setsid() };
            let config = config(args);
            if config.get("stage").map(String::as_str) != Some("grandchild") {
                let mut child_args: Vec<String> = args.to_vec();
                child_args.push("stage=grandchild".into());
                let _ = Command::new(&config["self_exe"])
                    .arg("--probe-daemon")
                    .args(&child_args)
                    .process_group(0)
                    .spawn();
            }
            std::thread::sleep(Duration::from_secs(300));
        }

        pub fn run(args: &[String]) {
            let c = config(args);
            match c.get("only").map(String::as_str) {
                Some("noop") => {
                    let _ = fs::write(Path::new(&c["target"]).join("ran"), b"1");
                    return;
                }
                Some("hang") => {
                    for _ in 0..2 {
                        let _ = Command::new(&c["self_exe"])
                            .arg("--probe-daemon")
                            .args(args)
                            .process_group(0)
                            .spawn();
                    }
                    std::thread::sleep(Duration::from_secs(300));
                    return;
                }
                Some("threads") => {
                    let (mut started, mut refused) = (0, 0);
                    let mut handles = Vec::new();
                    for _ in 0..64 {
                        match std::thread::Builder::new()
                            .spawn(|| std::thread::sleep(Duration::from_millis(500)))
                        {
                            Ok(handle) => {
                                started += 1;
                                handles.push(handle);
                            }
                            Err(_) => refused += 1,
                        }
                    }
                    println!("check threads_started ALLOWED {started}");
                    println!("check threads_refused ALLOWED {refused}");
                    for handle in handles {
                        let _ = handle.join();
                    }
                    return;
                }
                Some("alloc") => {
                    let mut block = vec![0u8; 512 * 1024 * 1024];
                    for i in (0..block.len()).step_by(4096) {
                        block[i] = 1;
                    }
                    println!("check allocated ALLOWED {}", block.len() / (1024 * 1024));
                    return;
                }
                Some("workspace") => {
                    use std::ffi::OsString;
                    use std::os::unix::ffi::OsStringExt;
                    use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
                    let input = PathBuf::from(&c["ws_input"]);
                    let scratch = PathBuf::from(&c["ws_scratch"]);
                    let lib = input.join("src/lib.rs");
                    report("ws_input_read", fs::read(&lib).map(|_| ()));
                    report("ws_input_list", fs::read_dir(&input).map(|_| ()));
                    report(
                        "ws_input_write",
                        fs::OpenOptions::new().write(true).open(&lib).map(|_| ()),
                    );
                    report("ws_input_create", fs::write(input.join("new.rs"), b"x"));
                    report("ws_input_remove", fs::remove_file(&lib));
                    report("ws_input_mkdir", fs::create_dir(input.join("dir")));
                    report(
                        "ws_input_rename",
                        fs::rename(&lib, input.join("src/moved.rs")),
                    );
                    report(
                        "ws_input_link_out",
                        fs::hard_link(&lib, scratch.join("linked-input.rs")),
                    );
                    for area in ["scratch", "home", "tmp", "cargo_home", "target"] {
                        let dir = PathBuf::from(&c[&format!("ws_{area}")]);
                        report(
                            &format!("ws_{area}_write"),
                            fs::write(dir.join("written"), b"x"),
                        );
                    }
                    report(
                        "ws_scratch_symlink",
                        symlink("/etc/hostname", scratch.join("link")),
                    );
                    let fifo =
                        std::ffi::CString::new(scratch.join("fifo").into_os_string().into_vec())
                            .unwrap();
                    // SAFETY: a NUL-terminated path.
                    raw("ws_scratch_fifo", unsafe {
                        libc::mkfifo(fifo.as_ptr(), 0o600) as libc::c_long
                    });
                    // What a verifier can leave behind for the backend.
                    let mut deep = scratch.join("deep");
                    for _ in 0..100 {
                        deep = deep.join("d");
                    }
                    report("ws_left_deep", fs::create_dir_all(&deep));
                    report(
                        "ws_left_locked",
                        fs::DirBuilder::new()
                            .mode(0o000)
                            .create(scratch.join("locked")),
                    );
                    let hidden = scratch.join("hidden");
                    report(
                        "ws_left_hidden",
                        fs::DirBuilder::new()
                            .mode(0o300)
                            .create(&hidden)
                            .and_then(|()| fs::write(hidden.join("file"), b"x")),
                    );
                    report(
                        "ws_left_odd_name",
                        fs::write(scratch.join(OsString::from_vec(b"\xff\xfe".to_vec())), b"x"),
                    );
                    report(
                        "ws_left_hard_link",
                        fs::hard_link(scratch.join("written"), scratch.join("written-too")),
                    );
                    report(
                        "ws_left_no_mode",
                        fs::OpenOptions::new()
                            .write(true)
                            .create_new(true)
                            .mode(0o000)
                            .open(scratch.join("no-mode"))
                            .map(|_| ()),
                    );
                    return;
                }
                Some("alloc-child") => {
                    let child_args: Vec<String> = args
                        .iter()
                        .map(|arg| {
                            if arg.starts_with("only=") {
                                "only=alloc".to_string()
                            } else {
                                arg.clone()
                            }
                        })
                        .collect();
                    let status = Command::new(&c["self_exe"])
                        .arg("--probe")
                        .args(&child_args)
                        .status();
                    // Carry on well after the kill, as a build tool would.
                    std::thread::sleep(Duration::from_secs(3));
                    let signal = status.ok().and_then(|s| s.signal()).unwrap_or(0);
                    println!("check child_killed_by_signal ALLOWED {signal}");
                    return;
                }
                Some("flood") => {
                    let chunk = vec![b'x'; 64 * 1024];
                    let mut out = std::io::stdout().lock();
                    for _ in 0..256 {
                        if out.write_all(&chunk).is_err() {
                            break;
                        }
                    }
                    return;
                }
                Some("daemons") => {
                    for _ in 0..2 {
                        let spawned = Command::new(&c["self_exe"])
                            .arg("--probe-daemon")
                            .args(args)
                            .process_group(0)
                            .spawn()
                            .is_ok();
                        holds("spawned_daemons", spawned);
                    }
                    std::thread::sleep(Duration::from_millis(300));
                    return;
                }
                _ => {}
            }
            // Before the probe opens anything itself: every descriptor above
            // stdio must be closed.
            let inherited_fds_were_absent = (3..1024)
                // SAFETY: F_GETFD only queries a descriptor number.
                .all(|fd| unsafe { libc::fcntl(fd, libc::F_GETFD) } < 0);
            let path = |key: &str| PathBuf::from(&c[key]);
            let outside = path("outside");
            report("read_sentinel", fs::read(path("sentinel")).map(drop));
            report("write_outside", fs::write(outside.join("written"), b"x"));
            report("list_real_home", fs::read_dir(path("real_home")).map(drop));
            report("list_nexus_dir", fs::read_dir(path("nexus_dir")).map(drop));
            report(
                "read_git_credentials",
                fs::read(outside.join(".git-credentials")).map(drop),
            );
            report(
                "read_ssh_key",
                fs::read(outside.join(".ssh/id_ed25519")).map(drop),
            );
            report(
                "read_api_key",
                fs::read(outside.join("api-key.env")).map(drop),
            );
            report("read_etc_passwd", fs::read("/etc/passwd").map(drop));
            let input = path("input");
            report(
                "symlink_relative",
                fs::read(input.join("escape-relative")).map(drop),
            );
            report(
                "symlink_absolute",
                fs::read(input.join("escape-absolute")).map(drop),
            );
            report("write_input_new", fs::write(input.join("new-file"), b"x"));
            report(
                "modify_input",
                fs::OpenOptions::new()
                    .append(true)
                    .open(input.join("data.txt"))
                    .and_then(|mut f| f.write_all(b"x")),
            );
            report("read_input", fs::read(input.join("data.txt")).map(drop));
            report("write_target", fs::write(path("target").join("out"), b"x"));
            report(
                "exec_shell",
                Command::new("/bin/sh")
                    .args(["-c", "true"])
                    .status()
                    .map(drop),
            );
            report("read_proc_status", fs::read("/proc/self/status").map(drop));
            report("list_proc", fs::read_dir("/proc").map(drop));
            let tcp: u16 = c["tcp"].parse().unwrap();
            report(
                "tcp_host",
                TcpStream::connect_timeout(&([127, 0, 0, 1], tcp).into(), Duration::from_secs(2))
                    .map(drop),
            );
            let udp: u16 = c["udp"].parse().unwrap();
            report(
                "udp_socket",
                UdpSocket::bind("127.0.0.1:0")
                    .and_then(|s| s.send_to(b"p2c", ("127.0.0.1", udp)).map(drop)),
            );
            report(
                "abstract_socket",
                SocketAddr::from_abstract_name(&c["abstract"])
                    .and_then(|a| UnixStream::connect_addr(&a))
                    .map(drop),
            );
            report(
                "pathname_socket",
                UnixStream::connect(&c["pathname"]).map(drop),
            );
            report(
                "tcp_ollama",
                TcpStream::connect_timeout(&([127, 0, 0, 1], 11434).into(), Duration::from_secs(2))
                    .map(drop),
            );
            let mut pair = [0i32; 2];
            // SAFETY: socketpair writes two descriptors into the array.
            raw("socketpair_inet", unsafe {
                libc::socketpair(libc::AF_INET, libc::SOCK_STREAM, 0, pair.as_mut_ptr())
            } as libc::c_long);
            // SAFETY: as above.
            raw("socketpair_unix", unsafe {
                libc::socketpair(libc::AF_UNIX, libc::SOCK_STREAM, 0, pair.as_mut_ptr())
            } as libc::c_long);
            // SAFETY: each call below only asks the kernel for an operation;
            // failures are the expected outcome and are reported.
            unsafe {
                raw(
                    "socket_raw",
                    libc::socket(libc::AF_INET, libc::SOCK_RAW, 1) as libc::c_long,
                );
                raw(
                    "unshare_user",
                    libc::unshare(libc::CLONE_NEWUSER) as libc::c_long,
                );
                raw("setns", libc::setns(0, 0) as libc::c_long);
                raw(
                    "mount",
                    libc::mount(
                        c"none".as_ptr(),
                        c"/".as_ptr(),
                        c"tmpfs".as_ptr(),
                        0,
                        std::ptr::null(),
                    ) as libc::c_long,
                );
                raw("chroot", libc::chroot(c"/".as_ptr()) as libc::c_long);
                raw(
                    "pivot_root",
                    libc::syscall(libc::SYS_pivot_root, c"/".as_ptr(), c"/".as_ptr()),
                );
                let mut params = [0u8; 120];
                raw(
                    "io_uring_setup",
                    libc::syscall(libc::SYS_io_uring_setup, 1u32, params.as_mut_ptr()),
                );
                let driver: libc::pid_t = c["driver_pid"].parse().unwrap();
                raw("ptrace", libc::ptrace(libc::PTRACE_ATTACH, driver, 0, 0));
                raw(
                    "process_vm_readv",
                    libc::syscall(libc::SYS_process_vm_readv, 1, 0, 0, 0, 0, 0),
                );
                raw("pidfd_open", libc::syscall(libc::SYS_pidfd_open, 1, 0));
                raw("pidfd_getfd", libc::syscall(libc::SYS_pidfd_getfd, 0, 0, 0));
                raw(
                    "clone3",
                    libc::syscall(libc::SYS_clone3, std::ptr::null::<u8>(), 0usize),
                );
                raw("bpf", libc::syscall(libc::SYS_bpf, 0, 0, 0));
                raw(
                    "perf_event_open",
                    libc::syscall(libc::SYS_perf_event_open, 0, 0, -1, -1, 0),
                );
                raw("keyctl", libc::syscall(libc::SYS_keyctl, 0, 0, 0, 0, 0));
                raw("userfaultfd", libc::syscall(libc::SYS_userfaultfd, 0));
                let chmod_target =
                    CString::new(outside.join("chmod-target").as_os_str().as_encoded_bytes())
                        .unwrap();
                raw(
                    "chmod_outside",
                    libc::chmod(chmod_target.as_ptr(), 0o777) as libc::c_long,
                );
                let times = [libc::timespec {
                    tv_sec: 0,
                    tv_nsec: 0,
                }; 2];
                raw(
                    "utimensat_path",
                    libc::utimensat(libc::AT_FDCWD, chmod_target.as_ptr(), times.as_ptr(), 0)
                        as libc::c_long,
                );
                raw("kill_driver", libc::kill(driver, 0) as libc::c_long);
            }
            holds("no_inherited_fds", inherited_fds_were_absent);
            let keys: BTreeSet<String> = std::env::vars().map(|(k, _)| k).collect();
            let expected: BTreeSet<String> = c["env_keys"].split(',').map(String::from).collect();
            holds("exact_environment", keys == expected);
            // SAFETY: getuid has no preconditions.
            holds("identity_uid", unsafe { libc::getuid() } != 0);
        }
    }
}
