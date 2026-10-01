//! P2-V1-R1 fixture controls for the live suite's checked cleanup
//! observations (`support/cleanup_observation.rs`): a failed, unbounded,
//! redirected or malformed query is never "no scopes", and an observation
//! failure at a retained boundary never skips its retry.
//!
//! These are not live evidence. No user manager is contacted and no unit,
//! scope or cgroup is created: the stand-in queries are `/bin/sh` scripts
//! this test owns, bounded and reaped by the observation itself, and the
//! runtime directories are fixture trees under the temporary directory, in
//! which this test's uid stands in for root.

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[path = "support/cleanup_observation.rs"]
mod cleanup_observation;

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
mod controls {
    use super::cleanup_observation::*;
    use std::cell::Cell;
    use std::collections::BTreeMap;
    use std::fs;
    use std::os::unix::fs::{symlink, PermissionsExt};
    use std::os::unix::net::UnixListener;
    use std::os::unix::process::ExitStatusExt;
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::{Duration, Instant};

    /// A bus address no stand-in ever connects to.
    const BUS: &str = "/nonexistent/nexus-p2v1r1/run/user/4242/bus";
    const AMBIENT_CHILD: &str = "NEXUS_P2V1R1_AMBIENT_CHILD";

    fn uid() -> u32 {
        // SAFETY: getuid has no preconditions.
        unsafe { libc::getuid() }
    }

    fn mode(path: &Path, mode: u32) {
        fs::set_permissions(path, fs::Permissions::from_mode(mode)).unwrap();
    }

    /// A fixture root this test owns, removed when dropped.
    struct Root(PathBuf);

    impl Root {
        fn new(tag: &str) -> Self {
            static NEXT: AtomicUsize = AtomicUsize::new(0);
            let root = std::env::temp_dir().join(format!(
                "p2v1r1-{tag}-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::SeqCst)
            ));
            fs::create_dir(&root).unwrap();
            mode(&root, 0o755);
            Root(root)
        }

        /// `run/user/<uid>` as a host has it: `run` and `run/user` 0755,
        /// the runtime directory 0700 holding a listening bus socket.
        fn runtime(&self, uid: u32) -> (PathBuf, UnixListener) {
            let user = self.0.join("run/user");
            fs::create_dir_all(&user).unwrap();
            mode(&self.0.join("run"), 0o755);
            mode(&user, 0o755);
            let runtime = user.join(uid.to_string());
            fs::create_dir(&runtime).unwrap();
            mode(&runtime, 0o700);
            let bus = UnixListener::bind(runtime.join("bus")).unwrap();
            (runtime, bus)
        }

        /// This root as a host whose root is this test's uid.
        fn host(&self, uid: u32, fs_magic: Option<i64>) -> Host<'_> {
            Host {
                root: &self.0,
                uid,
                root_owner: self::uid(),
                fs_magic,
            }
        }
    }

    impl Drop for Root {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    /// The scope query, answered by `script` (run by `/bin/sh` with the
    /// query's own arguments after it) instead of systemctl.
    fn stand_in(script: &str) -> Query {
        scope_query(
            PathBuf::from("/bin/sh"),
            &["-c", script, "systemctl"],
            Path::new(BUS),
        )
        .unwrap()
    }

    fn killed(status: Option<std::process::ExitStatus>) -> bool {
        status.is_some_and(|status| status.signal() == Some(libc::SIGKILL))
    }

    /// The environment a stand-in started with, from its own
    /// `/proc/<pid>/environ`.
    fn started_with(query: &Query) -> BTreeMap<String, String> {
        let probe = Query {
            program: query.program.clone(),
            args: ["-c", "/bin/cat /proc/$$/environ"]
                .iter()
                .map(Into::into)
                .collect(),
            env: query.env.clone(),
            timeout: query.timeout,
            output_limit: query.output_limit,
        };
        let answer = run(&probe).unwrap();
        assert!(answer.status.success(), "{answer:?}");
        String::from_utf8(answer.stdout)
            .unwrap()
            .split('\0')
            .filter(|entry| !entry.is_empty())
            .map(|entry| {
                let (key, value) = entry.split_once('=').unwrap();
                (key.to_string(), value.to_string())
            })
            .collect()
    }

    fn expected_environment(bus: &str) -> BTreeMap<String, String> {
        [
            ("DBUS_SESSION_BUS_ADDRESS", format!("unix:path={bus}")),
            ("LC_ALL", "C".to_string()),
            ("SYSTEMD_COLORS", "0".to_string()),
            ("SYSTEMD_URLIFY", "0".to_string()),
        ]
        .into_iter()
        .map(|(key, value)| (key.to_string(), value))
        .collect()
    }

    #[test]
    fn a_successful_empty_answer_is_no_scopes() {
        assert_eq!(observe(&stand_in("exit 0")).unwrap(), Scopes::new());
    }

    #[test]
    fn a_successful_answer_lists_each_loaded_verifier_scope() {
        let answer = "nexus-verifier-0a1b.scope loaded active running Nexus verifier execution\n\
                      \n\
                      nexus-verifier-ff00.scope loaded inactive dead Nexus verifier execution\n";
        let scopes = observe(&stand_in(&format!("printf '{answer}'"))).unwrap();
        assert_eq!(
            scopes.into_iter().collect::<Vec<_>>(),
            ["nexus-verifier-0a1b.scope", "nexus-verifier-ff00.scope"]
        );
    }

    #[test]
    fn a_failed_query_with_empty_output_is_an_error_never_no_scopes() {
        // A silent failure: nothing on either stream.
        match observe(&stand_in("exit 4")) {
            Err(ObservationError::Failed { status, .. }) => assert_eq!(status.code(), Some(4)),
            other => panic!("a failed query is an error, never no scopes: {other:?}"),
        }
        // What systemctl --user does without a reachable user bus.
        let unreachable = stand_in("echo 'Failed to connect to bus: No medium found' >&2; exit 1");
        match observe(&unreachable) {
            Err(ObservationError::Failed { status, stderr }) => {
                assert_eq!(status.code(), Some(1));
                assert!(stderr.contains("No medium found"), "{stderr}");
            }
            other => panic!("a failed query is an error, never no scopes: {other:?}"),
        }
    }

    #[test]
    fn a_successful_query_that_reports_diagnostics_is_an_error() {
        let query = stand_in("echo 'warning: something is off' >&2; exit 0");
        assert!(
            matches!(observe(&query), Err(ObservationError::Diagnostics(text)) if text.contains("something is off"))
        );
    }

    #[test]
    fn a_missing_query_program_is_an_error() {
        let query = scope_query(
            PathBuf::from("/nonexistent/nexus-p2v1r1/systemctl"),
            &[],
            Path::new(BUS),
        )
        .unwrap();
        assert!(
            matches!(observe(&query), Err(ObservationError::Spawn(error)) if error.kind() == std::io::ErrorKind::NotFound)
        );
    }

    #[test]
    fn a_query_that_does_not_finish_is_killed_reaped_and_an_error() {
        let mut query = stand_in("exec /bin/sleep 30");
        query.timeout = Duration::from_millis(300);
        let started = Instant::now();
        match observe(&query) {
            Err(ObservationError::Timeout(status)) => {
                assert!(killed(status), "killed and reaped: {status:?}")
            }
            other => panic!("a query that does not finish is an error: {other:?}"),
        }
        assert!(started.elapsed() < Duration::from_secs(8));
    }

    #[test]
    fn a_query_whose_output_stays_open_is_ended_with_its_process_group() {
        // The query exits at once, but a process it left behind keeps its
        // output open: no answer is complete, and the bound still holds.
        let root = Root::new("open-output");
        let pid_file = root.0.join("pid");
        let mut query = stand_in(&format!(
            "/bin/sleep 30 & echo $! > {}; exit 0",
            pid_file.display()
        ));
        query.timeout = Duration::from_millis(500);
        let started = Instant::now();
        let result = observe(&query);
        assert!(
            matches!(result, Err(ObservationError::Timeout(Some(_)))),
            "{result:?}"
        );
        assert!(started.elapsed() < Duration::from_secs(8));
        let pid = fs::read_to_string(&pid_file).unwrap().trim().to_string();
        // The killed process was reparented; it is gone once reaped there.
        let gone = (0..250).any(|_| {
            let done = match fs::read_to_string(format!("/proc/{pid}/stat")) {
                Err(_) => true,
                Ok(stat) => stat
                    .rsplit(") ")
                    .next()
                    .is_some_and(|rest| rest.starts_with('Z')),
            };
            if !done {
                std::thread::sleep(Duration::from_millis(20));
            }
            done
        });
        assert!(gone, "the query's process group was killed");
    }

    #[test]
    fn a_query_that_writes_too_much_is_killed_and_an_error() {
        let line = "nexus-verifier-0a1b.scope loaded active running Nexus verifier execution";
        for stream in ["", " >&2"] {
            let mut query = stand_in(&format!(
                "i=0; while [ $i -lt 64 ]; do echo '{line}'{stream}; i=$((i+1)); done; \
                 exec /bin/sleep 30"
            ));
            query.output_limit = 1024;
            match observe(&query) {
                Err(ObservationError::OutputLimit(status)) => {
                    assert!(killed(status), "killed and reaped: {status:?}")
                }
                other => panic!("too much output is an error{stream}: {other:?}"),
            }
        }
    }

    #[test]
    fn a_malformed_answer_is_an_error() {
        for answer in [
            "garbage\\n",
            "\\342\\227\\217 nexus-verifier-0a1b.scope loaded failed failed Nexus\\n",
            "nexus-verifier-0a1b.scope\\n",
            "nexus-verifier-0a1b.scope loaded active\\n",
            "nexus-verifier-0a1b.scope Loaded active running Nexus\\n",
            "nexus-verifier-0a1b.scope - active running Nexus\\n",
            "other.scope loaded active running Other\\n",
            "nexus-verifier-0a1b.service loaded active running Nexus\\n",
            "nexus-verifier-.scope loaded active running Nexus\\n",
            "nexus-verifier-0a1b.scope loaded active running A\\n\
             nexus-verifier-0a1b.scope loaded active running A\\n",
            "\\377\\n",
        ] {
            let result = observe(&stand_in(&format!("printf '{answer}'")));
            assert!(
                matches!(result, Err(ObservationError::Malformed(_))),
                "{answer}: {result:?}"
            );
        }
    }

    #[test]
    fn the_query_is_exactly_the_scope_listing_on_the_given_bus() {
        let answer = run(&stand_in("printf '%s\\n' \"$@\"")).unwrap();
        assert!(answer.status.success());
        assert_eq!(
            String::from_utf8(answer.stdout)
                .unwrap()
                .lines()
                .collect::<Vec<_>>(),
            [
                "--user",
                "--no-pager",
                "--legend=no",
                "--plain",
                "--full",
                "--all",
                "list-units",
                "nexus-verifier-*.scope"
            ]
        );
        assert_eq!(started_with(&stand_in("")), expected_environment(BUS));
        // A bus address is never built from a path that needs escaping.
        for bus in [
            "/run/user/1000/b us",
            "/run/user/1000/bus;x",
            "/run/user/1000/bus,guid=0",
        ] {
            assert!(matches!(
                scope_query(PathBuf::from("/bin/sh"), &[], Path::new(bus)),
                Err(ObservationError::Runtime(_))
            ));
        }
    }

    #[test]
    fn poisoned_or_absent_ambient_bus_and_runtime_never_reach_the_query() {
        if std::env::var_os(AMBIENT_CHILD).is_some() {
            // Re-executed below, with that ambient environment.
            assert_eq!(started_with(&stand_in("")), expected_environment(BUS));
            return;
        }
        for poisoned in [true, false] {
            let mut child = Command::new(std::env::current_exe().unwrap());
            child
                .args([
                    "--exact",
                    "controls::poisoned_or_absent_ambient_bus_and_runtime_never_reach_the_query",
                    "--nocapture",
                    "--test-threads=1",
                ])
                .env(AMBIENT_CHILD, "1");
            if poisoned {
                child
                    .env(
                        "DBUS_SESSION_BUS_ADDRESS",
                        "unix:path=/nonexistent/poisoned/bus",
                    )
                    .env("XDG_RUNTIME_DIR", "/nonexistent/poisoned");
            } else {
                child
                    .env_remove("DBUS_SESSION_BUS_ADDRESS")
                    .env_remove("XDG_RUNTIME_DIR");
            }
            let output = child.output().unwrap();
            let text = String::from_utf8_lossy(&output.stdout);
            assert!(output.status.success(), "poisoned {poisoned}: {text}");
            assert!(text.contains("1 passed"), "poisoned {poisoned}: {text}");
        }
    }

    #[test]
    fn the_bus_is_taken_only_from_a_checked_private_runtime_directory() {
        let root = Root::new("bus-ok");
        let (runtime, _bus) = root.runtime(uid());
        assert_eq!(
            user_bus(&root.host(uid(), None)).unwrap(),
            runtime.join("bus")
        );
        let magic = filesystem_type(&runtime).unwrap();
        assert!(user_bus(&root.host(uid(), Some(magic))).is_ok());
    }

    #[test]
    fn an_unusable_runtime_directory_or_bus_is_an_error() {
        type Change = fn(&Root, &Path);
        let changes: [(&str, Change); 10] = [
            ("no runtime directory", |_, runtime| {
                fs::remove_dir_all(runtime).unwrap()
            }),
            ("a symlinked runtime directory", |root, runtime| {
                let real = root.0.join("elsewhere");
                fs::rename(runtime, &real).unwrap();
                symlink(&real, runtime).unwrap();
            }),
            ("a shared runtime directory", |_, runtime| {
                mode(runtime, 0o755)
            }),
            ("no bus", |_, runtime| {
                fs::remove_file(runtime.join("bus")).unwrap()
            }),
            ("a bus that is a file", |_, runtime| {
                fs::remove_file(runtime.join("bus")).unwrap();
                fs::write(runtime.join("bus"), b"").unwrap();
            }),
            ("a symlinked bus", |root, runtime| {
                let real = root.0.join("real-bus");
                fs::rename(runtime.join("bus"), &real).unwrap();
                symlink(&real, runtime.join("bus")).unwrap();
            }),
            ("a writable /run", |root, _| {
                mode(&root.0.join("run"), 0o775)
            }),
            ("a writable /run/user", |root, _| {
                mode(&root.0.join("run/user"), 0o777)
            }),
            ("a symlinked /run", |root, _| {
                let real = root.0.join("real-run");
                fs::rename(root.0.join("run"), &real).unwrap();
                symlink(&real, root.0.join("run")).unwrap();
            }),
            ("a symlinked /run/user", |root, _| {
                let real = root.0.join("real-user");
                fs::rename(root.0.join("run/user"), &real).unwrap();
                symlink(&real, root.0.join("run/user")).unwrap();
            }),
        ];
        for (what, change) in changes {
            let root = Root::new("bus-bad");
            let (runtime, _bus) = root.runtime(uid());
            change(&root, &runtime);
            let result = user_bus(&root.host(uid(), None));
            assert!(
                matches!(result, Err(ObservationError::Runtime(_))),
                "{what}: {result:?}"
            );
        }
        // Another uid's runtime directory (here owned by this test), a
        // runtime directory not owned by root's stand-in, and one that is not
        // the required filesystem.
        let root = Root::new("bus-owner");
        let other = uid().wrapping_add(1);
        let (_runtime, _bus) = root.runtime(other);
        assert!(matches!(
            user_bus(&root.host(other, None)),
            Err(ObservationError::Runtime(_))
        ));
        let root = Root::new("bus-root-owner");
        let (_runtime, _bus) = root.runtime(uid());
        let mut host = root.host(uid(), None);
        host.root_owner = uid().wrapping_add(1);
        assert!(matches!(user_bus(&host), Err(ObservationError::Runtime(_))));
        let root = Root::new("bus-fs");
        let (runtime, _bus) = root.runtime(uid());
        let wrong = filesystem_type(&runtime).unwrap().wrapping_add(1);
        assert!(matches!(
            user_bus(&root.host(uid(), Some(wrong))),
            Err(ObservationError::Runtime(_))
        ));
    }

    fn scopes(names: &[&str]) -> Scopes {
        names.iter().map(|name| name.to_string()).collect()
    }

    #[test]
    fn a_failed_observation_never_ends_a_wait_as_done() {
        let failing = || observe(&stand_in("exit 1"));
        let result = wait_for(Duration::from_secs(5), failing, |now| now.is_empty());
        assert!(
            matches!(result, Err(ObservationError::Failed { .. })),
            "{result:?}"
        );
        // A failure part-way through a wait ends it too.
        let mut answers = vec![
            Err(ObservationError::Malformed("x".into())),
            Ok(scopes(&["nexus-verifier-a.scope"])),
        ];
        let result = wait_for(
            Duration::from_secs(5),
            || answers.pop().unwrap(),
            |now| now.is_empty(),
        );
        assert!(
            matches!(result, Err(ObservationError::Malformed(_))),
            "{result:?}"
        );
        assert!(wait_for(
            Duration::from_secs(5),
            || Ok(Scopes::new()),
            |now| now.is_empty()
        )
        .unwrap());
        let started = Instant::now();
        let kept = scopes(&["nexus-verifier-a.scope"]);
        assert!(!wait_for(
            Duration::from_millis(100),
            || Ok(kept.clone()),
            |now| now.is_empty()
        )
        .unwrap());
        assert!(started.elapsed() < Duration::from_secs(5));
    }

    #[test]
    fn an_observation_failure_at_a_retained_boundary_never_skips_its_retry() {
        let before = scopes(&["nexus-verifier-a.scope"]);
        let kept = scopes(&["nexus-verifier-a.scope", "nexus-verifier-b.scope"]);
        let retries = Cell::new(0);
        let retry = |confirms: bool| {
            let retries = &retries;
            move || {
                retries.set(retries.get() + 1);
                confirms
            }
        };
        // The real failure of an unreachable user bus: the retry still runs
        // and confirms, and the observation failure is kept beside it.
        let unreachable = observe(&stand_in("echo 'No medium found' >&2; exit 1"));
        match after_retry(unreachable, &before, retry(true)) {
            Err(Retained::Unobserved {
                error: ObservationError::Failed { .. },
                retried: true,
            }) => {}
            other => panic!("{other:?}"),
        }
        assert_eq!(retries.get(), 1);
        let timed_out = Err(ObservationError::Timeout(None));
        assert!(matches!(
            after_retry(timed_out, &before, retry(false)),
            Err(Retained::Unobserved {
                error: ObservationError::Timeout(None),
                retried: false
            })
        ));
        assert_eq!(retries.get(), 2);
        assert!(matches!(
            after_retry(Ok(kept.clone()), &before, retry(false)),
            Err(Retained::NotConfirmed { kept: true })
        ));
        assert!(matches!(
            after_retry(Ok(before.clone()), &before, retry(false)),
            Err(Retained::NotConfirmed { kept: false })
        ));
        assert!(matches!(
            after_retry(Ok(before.clone()), &before, retry(true)),
            Err(Retained::NotKept { before: 1, .. })
        ));
        assert!(after_retry(Ok(kept), &before, retry(true)).is_ok());
        assert_eq!(retries.get(), 6, "every judgement ran the retry once");
    }
}
