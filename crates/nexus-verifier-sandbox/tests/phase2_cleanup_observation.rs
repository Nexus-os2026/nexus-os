//! P2-V1-R1/R2 fixture controls for the live suite's checked cleanup
//! observations (`support/cleanup_observation.rs`): a failed, unbounded,
//! redirected or malformed query is never "no scopes"; a query's process
//! group is ended before any answer, and a group that cannot be confirmed
//! ended stays owned; a retained boundary's owner survives every failed
//! cleanup attempt, and no failure hides another.
//!
//! These are not live evidence. No user manager is contacted and no unit,
//! scope or cgroup is created: the stand-in queries are `/bin/sh` scripts
//! this test owns, the runtime directories are fixture trees under the
//! temporary directory (in which this test's uid stands in for root), and a
//! retained boundary is stood in for by an owner whose drop is witnessed.
//! Stand-ins that leave a process behind report it over a FIFO and wait
//! until this test holds it by pidfd: the test's own cleanup ends it, and
//! reaps what is this process's child, whatever the observer did.

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[path = "support/cleanup_observation.rs"]
mod cleanup_observation;

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
mod controls {
    use super::cleanup_observation::*;
    use std::cell::Cell;
    use std::collections::BTreeMap;
    use std::ffi::CString;
    use std::fs::{self, File, OpenOptions};
    use std::io::{self, Read};
    use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};
    use std::os::unix::ffi::OsStrExt;
    use std::os::unix::fs::{symlink, OpenOptionsExt, PermissionsExt};
    use std::os::unix::net::UnixListener;
    use std::os::unix::process::ExitStatusExt;
    use std::path::{Path, PathBuf};
    use std::process::{Command, ExitStatus};
    use std::rc::Rc;
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

    fn killed(status: ExitStatus) -> bool {
        status.signal() == Some(libc::SIGKILL)
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
            ops: query.ops,
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

    /// A FIFO at `dir/name`, for a stand-in's controlled communication.
    fn fifo(dir: &Path, name: &str) -> PathBuf {
        let path = dir.join(name);
        let c_path = CString::new(path.as_os_str().as_bytes()).unwrap();
        // SAFETY: a NUL-terminated path.
        let made = unsafe { libc::mkfifo(c_path.as_ptr(), 0o600) };
        assert_eq!(made, 0, "mkfifo: {}", io::Error::last_os_error());
        path
    }

    /// A process this test holds by pidfd: its own handle, which the
    /// observer under test never shares, so the test's cleanup never depends
    /// on that observer.
    struct Held {
        pid: libc::pid_t,
        fd: OwnedFd,
    }

    impl Held {
        fn open(pid: libc::pid_t) -> io::Result<Self> {
            // SAFETY: pidfd_open takes a pid and no flags; it returns a new
            // close-on-exec descriptor or -1.
            let fd = unsafe { libc::syscall(libc::SYS_pidfd_open, pid, 0) };
            let fd = RawFd::try_from(fd).map_err(|_| io::Error::other("pidfd_open"))?;
            if fd < 0 {
                return Err(io::Error::last_os_error());
            }
            // SAFETY: a new descriptor, owned here.
            Ok(Held {
                pid,
                fd: unsafe { OwnedFd::from_raw_fd(fd) },
            })
        }

        /// Whether the process has exited within `within`: its pidfd is
        /// readable once it has. An exit, never a reap.
        fn exited_within(&self, within: Duration) -> bool {
            let mut polled = [libc::pollfd {
                fd: self.fd.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            }];
            let millis = i32::try_from(within.as_millis()).unwrap_or(i32::MAX);
            // SAFETY: one initialized pollfd.
            let ready = unsafe { libc::poll(polled.as_mut_ptr(), 1, millis) };
            ready > 0 && polled[0].revents & libc::POLLIN != 0
        }

        /// Whether it is still this process's unreaped child: it was never
        /// reaped, by anyone.
        fn unreaped_child(&self) -> bool {
            self.wait(libc::WEXITED | libc::WNOHANG | libc::WNOWAIT)
        }

        /// SIGKILL to exactly this process, through its pidfd.
        fn kill(&self) {
            // SAFETY: signals only the process the pidfd refers to.
            unsafe {
                libc::syscall(
                    libc::SYS_pidfd_send_signal,
                    self.fd.as_raw_fd(),
                    libc::SIGKILL,
                    std::ptr::null::<libc::siginfo_t>(),
                    0,
                )
            };
        }

        /// Reap it, if it is still this process's child.
        fn reap(&self) -> bool {
            self.wait(libc::WEXITED)
        }

        fn wait(&self, options: libc::c_int) -> bool {
            // SAFETY: siginfo_t is plain data; waitid fills it.
            let mut info: libc::siginfo_t = unsafe { std::mem::zeroed() };
            let id = libc::id_t::try_from(self.fd.as_raw_fd()).unwrap_or(libc::id_t::MAX);
            // SAFETY: waits only for the child the pidfd refers to.
            unsafe { libc::waitid(libc::P_PIDFD, id, &mut info, options) == 0 }
        }
    }

    /// A stand-in query whose leader starts one descendant in its own
    /// process group (every standard stream on /dev/null unless
    /// `keep_output`, so it never holds the query's output), reports both
    /// pids over a FIFO, waits until this test holds both by pidfd, then
    /// exits with `status` without writing anything. Whatever the observer
    /// does, dropping this ends what is left: the descendant through its
    /// pidfd, and the leader, this process's child, reaped.
    struct Survivor {
        _root: Root,
        script: String,
        pids: PathBuf,
        go: PathBuf,
        held: Option<(Held, Held)>,
    }

    impl Survivor {
        fn new(tag: &str, status: i32, keep_output: bool) -> Self {
            let root = Root::new(tag);
            let (pids, go) = (fifo(&root.0, "pids"), fifo(&root.0, "go"));
            let redirect = if keep_output {
                ""
            } else {
                " </dev/null >/dev/null 2>&1"
            };
            let script = format!(
                "/bin/sleep 600{redirect} &\necho \"$$ $!\" > {}\nread go < {}\nexit {status}\n",
                pids.display(),
                go.display()
            );
            Survivor {
                _root: root,
                script,
                pids,
                go,
                held: None,
            }
        }

        /// The stand-in query, observed while this test takes both of its
        /// processes by pidfd: a handshake over the FIFOs, never a sleep.
        fn observe(&mut self, query: &Query) -> Result<Scopes, ObservationError> {
            let (pids, go) = (self.pids.clone(), self.go.clone());
            let handshake = std::thread::spawn(move || -> io::Result<(Held, Held)> {
                let mut line = String::new();
                File::open(&pids)?.read_to_string(&mut line)?;
                let ids: Vec<libc::pid_t> = line
                    .split_whitespace()
                    .filter_map(|id| id.parse().ok())
                    .collect();
                let &[leader, descendant] = ids.as_slice() else {
                    return Err(io::Error::other(format!("pids {line:?}")));
                };
                // Both are alive here: the leader waits for `go`, the
                // descendant sleeps.
                let held = (Held::open(leader)?, Held::open(descendant)?);
                fs::write(&go, b"go\n")?;
                Ok(held)
            });
            let result = observe(query);
            self.unblock();
            match handshake.join() {
                Ok(Ok(held)) => self.held = Some(held),
                Ok(Err(error)) => panic!("the handshake failed: {error}; observed {result:?}"),
                Err(_) => panic!("the handshake panicked; observed {result:?}"),
            }
            result
        }

        /// Release a handshake still waiting on a FIFO the stand-in never
        /// opened.
        fn unblock(&self) {
            let _ = OpenOptions::new()
                .write(true)
                .custom_flags(libc::O_NONBLOCK)
                .open(&self.pids);
            let _ = OpenOptions::new()
                .read(true)
                .custom_flags(libc::O_NONBLOCK)
                .open(&self.go);
        }

        fn held(&self) -> (&Held, &Held) {
            let (leader, descendant) = self.held.as_ref().expect("the handshake");
            (leader, descendant)
        }
    }

    impl Drop for Survivor {
        fn drop(&mut self) {
            self.unblock();
            if let Some((leader, descendant)) = &self.held {
                if !descendant.exited_within(Duration::ZERO) {
                    descendant.kill();
                    descendant.exited_within(Duration::from_secs(5));
                }
                if leader.unreaped_child() {
                    leader.kill();
                    leader.reap();
                }
            }
        }
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
            matches!(observe(&query), Err(ObservationError::Spawn(error)) if error.kind() == io::ErrorKind::NotFound)
        );
    }

    #[test]
    fn a_query_that_does_not_finish_is_ended_reaped_and_an_error() {
        let mut query = stand_in("exec /bin/sleep 30");
        query.timeout = Duration::from_millis(300);
        let started = Instant::now();
        match observe(&query) {
            Err(ObservationError::Stopped {
                stop: Stop::Timeout,
                status,
            }) => assert!(killed(status), "ended and reaped: {status:?}"),
            other => panic!("a query that does not finish is an error: {other:?}"),
        }
        assert!(started.elapsed() < Duration::from_secs(8));
    }

    #[test]
    fn a_query_whose_output_stays_open_is_ended_with_its_process_group() {
        // The leader exits, but the descendant it left keeps its output
        // open: no answer is complete, and the bound still holds.
        let mut survivor = Survivor::new("open-output", 0, true);
        let mut query = stand_in(&survivor.script);
        query.timeout = Duration::from_secs(3);
        match survivor.observe(&query) {
            Err(ObservationError::Stopped {
                stop: Stop::Timeout,
                status,
            }) => assert!(status.success(), "the leader's own exit: {status:?}"),
            other => panic!("an output held open is never an answer: {other:?}"),
        }
        let (leader, descendant) = survivor.held();
        assert!(
            descendant.exited_within(Duration::from_secs(5)),
            "the query's process group was ended"
        );
        assert!(!leader.unreaped_child(), "the leader was reaped");
    }

    #[test]
    fn a_descendant_left_by_a_successful_query_is_ended_before_the_answer() {
        // The leader exits 0 without output; its descendant, in its group,
        // holds none of the query's output. Neither the end of the output
        // nor the leader's exit ends the query.
        let mut survivor = Survivor::new("survivor-ok", 0, false);
        let query = stand_in(&survivor.script);
        assert_eq!(survivor.observe(&query).unwrap(), Scopes::new());
        let (leader, descendant) = survivor.held();
        assert!(
            descendant.exited_within(Duration::from_secs(5)),
            "the query's process group was ended"
        );
        assert!(!leader.unreaped_child(), "the leader was reaped");
    }

    #[test]
    fn a_descendant_left_by_a_failed_query_is_ended_with_it() {
        let mut survivor = Survivor::new("survivor-failed", 3, false);
        let query = stand_in(&survivor.script);
        match survivor.observe(&query) {
            Err(ObservationError::Failed { status, .. }) => assert_eq!(status.code(), Some(3)),
            other => panic!("a failed query is an error: {other:?}"),
        }
        let (leader, descendant) = survivor.held();
        assert!(descendant.exited_within(Duration::from_secs(5)));
        assert!(!leader.unreaped_child());
    }

    #[test]
    fn an_unreadable_query_is_ended_and_an_error() {
        let mut survivor = Survivor::new("survivor-io", 0, false);
        let mut query = stand_in(&survivor.script);
        query.ops = Ops {
            exited: |_| Err(io::Error::from_raw_os_error(libc::EIO)),
            ..PROCESS
        };
        match survivor.observe(&query) {
            Err(ObservationError::Stopped {
                stop: Stop::Io(error),
                ..
            }) => assert_eq!(error.raw_os_error(), Some(libc::EIO)),
            other => panic!("an unreadable query is an error: {other:?}"),
        }
        let (leader, descendant) = survivor.held();
        assert!(descendant.exited_within(Duration::from_secs(5)));
        assert!(!leader.unreaped_child());
    }

    #[test]
    fn an_unconfirmed_group_end_keeps_the_query_owned_for_an_explicit_retry() {
        let mut survivor = Survivor::new("survivor-unsignalled", 0, false);
        let mut query = stand_in(&survivor.script);
        query.ops = Ops {
            signal_group: |_| Err(io::Error::from_raw_os_error(libc::EPERM)),
            ..PROCESS
        };
        let result = survivor.observe(&query);
        let Err(ObservationError::Unfinalized(unfinalized)) = result else {
            panic!("never an answer while the group is not ended: {result:?}");
        };
        let Unfinalized {
            query: owned,
            first,
            failure,
        } = *unfinalized;
        assert!(
            matches!(&first, Ok(answer) if answer.status.success()),
            "{first:?}"
        );
        assert_eq!(failure.raw_os_error(), Some(libc::EPERM));
        let (leader, descendant) = survivor.held();
        assert_eq!(owned.leader(), leader.pid.unsigned_abs());
        assert!(leader.unreaped_child(), "still owned: the leader unreaped");
        assert!(
            !descendant.exited_within(Duration::ZERO),
            "nothing ended the group"
        );
        // A later explicit retry, without the injected failure, ends it.
        let status = owned
            .with_ops(PROCESS)
            .finalize()
            .map_err(|(_, error)| error)
            .unwrap();
        assert!(status.success());
        assert!(descendant.exited_within(Duration::from_secs(5)));
        assert!(!leader.unreaped_child());
    }

    #[test]
    fn an_unreaped_leader_keeps_the_query_owned_and_anchored() {
        let mut survivor = Survivor::new("survivor-unreaped", 0, false);
        let mut query = stand_in(&survivor.script);
        query.ops = Ops {
            reap: |_, _| Ok(None),
            ..PROCESS
        };
        let result = survivor.observe(&query);
        let Err(ObservationError::Unfinalized(unfinalized)) = result else {
            panic!("never an answer while the leader is unreaped: {result:?}");
        };
        let Unfinalized { query: owned, .. } = *unfinalized;
        let (leader, descendant) = survivor.held();
        assert!(
            descendant.exited_within(Duration::from_secs(5)),
            "the group was signalled, anchored"
        );
        assert!(leader.unreaped_child(), "the anchor is kept");
        let status = owned
            .with_ops(PROCESS)
            .finalize()
            .map_err(|(_, error)| error)
            .unwrap();
        assert!(status.success());
        assert!(!leader.unreaped_child());
    }

    static SIGNALLED: AtomicUsize = AtomicUsize::new(0);

    /// The real group signal, counted (one control only uses it).
    fn counted_signal(group: libc::pid_t) -> io::Result<()> {
        SIGNALLED.fetch_add(1, Ordering::SeqCst);
        (PROCESS.signal_group)(group)
    }

    #[test]
    fn a_leader_in_an_unknown_state_is_never_signalled_again() {
        let mut survivor = Survivor::new("survivor-unknown", 0, false);
        let mut query = stand_in(&survivor.script);
        query.ops = Ops {
            signal_group: counted_signal,
            reap: |_, _| Err(io::Error::from_raw_os_error(libc::ECHILD)),
            ..PROCESS
        };
        let result = survivor.observe(&query);
        let Err(ObservationError::Unfinalized(unfinalized)) = result else {
            panic!("never an answer while the leader's state is unknown: {result:?}");
        };
        assert_eq!(
            SIGNALLED.load(Ordering::SeqCst),
            1,
            "signalled once, anchored"
        );
        let (leader, descendant) = survivor.held();
        assert!(descendant.exited_within(Duration::from_secs(5)));
        // Its reap failed, so the leader's state is unknown: neither a retry
        // nor the drop signals that group id again.
        let Unfinalized { query: owned, .. } = *unfinalized;
        let Err((owned, error)) = owned.finalize() else {
            panic!("a leader in an unknown state is never confirmed");
        };
        assert!(
            error.to_string().contains("never signalled again"),
            "{error}"
        );
        drop(owned);
        assert_eq!(SIGNALLED.load(Ordering::SeqCst), 1);
        // The leader is in fact still this process's child: the fixture's
        // own cleanup reaps it.
        assert!(leader.unreaped_child());
    }

    static REFUSED: AtomicUsize = AtomicUsize::new(0);

    /// A group signal that always fails, counted (one control only uses it).
    fn refused_signal(_: libc::pid_t) -> io::Result<()> {
        REFUSED.fetch_add(1, Ordering::SeqCst);
        Err(io::Error::from_raw_os_error(libc::EPERM))
    }

    #[test]
    fn a_reported_unconfirmed_query_is_retried_then_released() {
        let mut survivor = Survivor::new("survivor-released", 0, false);
        let mut query = stand_in(&survivor.script);
        query.ops = Ops {
            signal_group: refused_signal,
            ..PROCESS
        };
        let error = survivor.observe(&query).expect_err("never an answer");
        let report = reported(error);
        assert!(
            report.contains("still not confirmed ended after 3 explicit attempts"),
            "{report}"
        );
        // The run's attempt, each explicit attempt, then the release's
        // backstop, which still held the group's anchor.
        assert_eq!(REFUSED.load(Ordering::SeqCst), 1 + EXPLICIT_ATTEMPTS + 1);
        let (leader, descendant) = survivor.held();
        assert!(!leader.unreaped_child(), "the release reaped the leader");
        assert!(
            !descendant.exited_within(Duration::ZERO),
            "nothing could end the group: the fixture does"
        );
    }

    #[test]
    fn a_reported_query_is_finalized_again_explicitly() {
        let mut survivor = Survivor::new("survivor-recovered", 0, false);
        let mut query = stand_in(&survivor.script);
        query.ops = Ops {
            signal_group: |_| Err(io::Error::from_raw_os_error(libc::EPERM)),
            ..PROCESS
        };
        let result = survivor.observe(&query);
        let Err(ObservationError::Unfinalized(unfinalized)) = result else {
            panic!("never an answer: {result:?}");
        };
        let Unfinalized {
            query: owned,
            first,
            failure,
        } = *unfinalized;
        let error = ObservationError::Unfinalized(Box::new(Unfinalized {
            query: owned.with_ops(PROCESS),
            first,
            failure,
        }));
        let report = reported(error);
        assert!(
            report.contains("confirmed ended only on explicit attempt 1"),
            "{report}"
        );
        let (leader, descendant) = survivor.held();
        assert!(descendant.exited_within(Duration::from_secs(5)));
        assert!(!leader.unreaped_child());
    }

    #[test]
    fn a_query_that_writes_too_much_is_ended_and_an_error() {
        let line = "nexus-verifier-0a1b.scope loaded active running Nexus verifier execution";
        for stream in ["", " >&2"] {
            let mut query = stand_in(&format!(
                "i=0; while [ $i -lt 64 ]; do echo '{line}'{stream}; i=$((i+1)); done; \
                 exec /bin/sleep 30"
            ));
            query.output_limit = 1024;
            match observe(&query) {
                Err(ObservationError::Stopped {
                    stop: Stop::OutputLimit,
                    status,
                }) => assert!(killed(status), "ended and reaped: {status:?}"),
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

    /// An owner standing in for a retained boundary: its cleanup fails
    /// `failures` more times, every attempt is counted, and its drop (the
    /// end of its ownership) is witnessed.
    #[derive(Debug)]
    struct Witness {
        id: usize,
        failures: usize,
        tries: Rc<Cell<usize>>,
        dropped: Rc<Cell<usize>>,
    }

    impl Witness {
        fn new(failures: usize) -> (Self, Rc<Cell<usize>>, Rc<Cell<usize>>) {
            static NEXT: AtomicUsize = AtomicUsize::new(0);
            let (tries, dropped) = (Rc::new(Cell::new(0)), Rc::new(Cell::new(0)));
            let witness = Witness {
                id: NEXT.fetch_add(1, Ordering::SeqCst),
                failures,
                tries: tries.clone(),
                dropped: dropped.clone(),
            };
            (witness, tries, dropped)
        }

        /// As a retained boundary's retry: a confirmation consumes the
        /// owner; a failure returns the very same owner.
        fn retry(mut self) -> Result<(), Self> {
            self.tries.set(self.tries.get() + 1);
            if self.failures == 0 {
                return Ok(());
            }
            self.failures -= 1;
            Err(self)
        }
    }

    impl Drop for Witness {
        fn drop(&mut self) {
            self.dropped.set(self.dropped.get() + 1);
        }
    }

    fn before_and_kept() -> (Scopes, Scopes) {
        (
            scopes(&["nexus-verifier-a.scope"]),
            scopes(&["nexus-verifier-a.scope", "nexus-verifier-b.scope"]),
        )
    }

    #[test]
    fn each_failed_attempt_returns_the_same_owner() {
        let (owner, tries, dropped) = Witness::new(2);
        let id = owner.id;
        let settled = settle(owner, EXPLICIT_ATTEMPTS, |owner: Witness| {
            assert_eq!(owner.id, id, "the same owner");
            assert_eq!(dropped.get(), 0, "never dropped between attempts");
            owner.retry()
        });
        assert!(matches!(settled, Settled::Confirmed(3)), "{settled:?}");
        assert_eq!((tries.get(), dropped.get()), (3, 1));
    }

    #[test]
    fn a_failed_retry_keeps_its_owner_for_a_later_explicit_retry() {
        let (owner, tries, dropped) = Witness::new(usize::MAX);
        let id = owner.id;
        let (before, kept) = before_and_kept();
        let verdict = judge_retained(
            Vec::new(),
            Ok(kept),
            &before,
            owner,
            EXPLICIT_ATTEMPTS,
            Witness::retry,
        );
        assert_eq!(
            dropped.get(),
            0,
            "the owner is never dropped by a failed cleanup: {verdict:?}"
        );
        let Verdict::Unconfirmed {
            mut owner,
            failures,
        } = verdict
        else {
            panic!("an unconfirmed cleanup keeps its owner: {verdict:?}");
        };
        assert_eq!(owner.id, id, "the very owner every failed attempt returned");
        assert_eq!(tries.get(), EXPLICIT_ATTEMPTS);
        assert!(
            failures
                .iter()
                .any(|failure| failure.contains("still unconfirmed after 3 explicit attempts")),
            "{failures:?}"
        );
        // A later explicit retry confirms the cleanup, ending the ownership.
        owner.failures = 0;
        assert!(matches!(
            settle(owner, 1, Witness::retry),
            Settled::Confirmed(1)
        ));
        assert_eq!((tries.get(), dropped.get()), (EXPLICIT_ATTEMPTS + 1, 1));
    }

    #[test]
    fn an_observation_failure_survives_a_confirming_retry() {
        // The real failure of an unreachable user bus, reported.
        let observed = observe(&stand_in("echo 'No medium found' >&2; exit 1")).map_err(reported);
        let (owner, tries, dropped) = Witness::new(0);
        let (before, _) = before_and_kept();
        let verdict = judge_retained(
            Vec::new(),
            observed,
            &before,
            owner,
            EXPLICIT_ATTEMPTS,
            Witness::retry,
        );
        assert_eq!(
            (tries.get(), dropped.get()),
            (1, 1),
            "the retry ran and confirmed"
        );
        let Verdict::Failed(failures) = verdict else {
            panic!("an observation failure stays a failure: {verdict:?}");
        };
        assert!(
            failures
                .iter()
                .any(|failure| failure.contains("could not be observed")
                    && failure.contains("No medium found")),
            "{failures:?}"
        );
    }

    #[test]
    fn observation_and_cleanup_failures_are_kept_together_with_the_owner() {
        let (owner, _, dropped) = Witness::new(usize::MAX);
        let (before, _) = before_and_kept();
        let verdict = judge_retained(
            Vec::new(),
            Err("the query failed".to_string()),
            &before,
            owner,
            EXPLICIT_ATTEMPTS,
            Witness::retry,
        );
        assert_eq!(
            dropped.get(),
            0,
            "the owner is never dropped by a failed cleanup: {verdict:?}"
        );
        let Verdict::Unconfirmed { owner, failures } = verdict else {
            panic!("an unconfirmed cleanup keeps its owner: {verdict:?}");
        };
        assert!(failures
            .iter()
            .any(|failure| failure.contains("the query failed")));
        assert!(failures
            .iter()
            .any(|failure| failure.contains("still unconfirmed")));
        // Released explicitly, once reported: the report keeps both.
        let report = release(owner, "case", &failures);
        assert_eq!(dropped.get(), 1);
        assert!(report.contains("the query failed") && report.contains("still unconfirmed"));
    }

    #[test]
    fn evidence_is_judged_only_after_the_cleanup() {
        let (owner, tries, dropped) = Witness::new(0);
        let (before, kept) = before_and_kept();
        let verdict = judge_retained(
            vec!["the retained tree is not alive".to_string()],
            Ok(kept),
            &before,
            owner,
            EXPLICIT_ATTEMPTS,
            Witness::retry,
        );
        assert_eq!((tries.get(), dropped.get()), (1, 1), "cleaned up first");
        let Verdict::Failed(failures) = verdict else {
            panic!("{verdict:?}");
        };
        assert_eq!(failures, ["the retained tree is not alive"]);
    }

    #[test]
    fn a_cleanup_confirmed_only_on_a_later_attempt_still_fails() {
        let (owner, tries, dropped) = Witness::new(1);
        let (before, kept) = before_and_kept();
        let verdict = judge_retained(
            Vec::new(),
            Ok(kept),
            &before,
            owner,
            EXPLICIT_ATTEMPTS,
            Witness::retry,
        );
        assert_eq!((tries.get(), dropped.get()), (2, 1));
        let Verdict::Failed(failures) = verdict else {
            panic!("{verdict:?}");
        };
        assert!(
            failures[0].contains("confirmed only on explicit attempt 2"),
            "{failures:?}"
        );
    }

    #[test]
    fn a_kept_scope_and_a_first_confirmed_cleanup_pass() {
        let (owner, tries, dropped) = Witness::new(0);
        let (before, kept) = before_and_kept();
        let verdict = judge_retained(
            Vec::new(),
            Ok(kept),
            &before,
            owner,
            EXPLICIT_ATTEMPTS,
            Witness::retry,
        );
        assert!(matches!(verdict, Verdict::Passed), "{verdict:?}");
        assert_eq!((tries.get(), dropped.get()), (1, 1));
        // Not kept: a failure, after the cleanup.
        let (owner, _, dropped) = Witness::new(0);
        let verdict = judge_retained(
            Vec::new(),
            Ok(before.clone()),
            &before,
            owner,
            EXPLICIT_ATTEMPTS,
            Witness::retry,
        );
        assert_eq!(dropped.get(), 1);
        assert!(
            matches!(&verdict, Verdict::Failed(failures) if failures[0].contains("not kept")),
            "{verdict:?}"
        );
    }
}
