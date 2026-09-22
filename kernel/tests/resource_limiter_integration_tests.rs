//! Finite native helpers: no external shell, sleep command, or PID probing.
//! Parent-process tests serialize internally for deterministic resource/handle-count checks.
#![cfg(any(target_os = "linux", target_os = "macos", windows))]

#[cfg(target_os = "linux")]
use nexus_kernel::resource_limiter::ResourceLimits;
use nexus_kernel::resource_limiter::{
    ResourceLimitError, ResourceLimitedChild, ResourceLimiter, ResourceOutput, ResourceProgram,
    ResourceSpawnSpec, ResourceStdin,
};
use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::{Duration, Instant};

const STARTUP: Duration = Duration::from_secs(10);
const CLEANUP: Duration = Duration::from_secs(5);
const EXPIRY: Duration = Duration::from_secs(15);

fn helper_args(name: &str) -> Vec<std::ffi::OsString> {
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
fn spec(name: &str) -> ResourceSpawnSpec {
    ResourceSpawnSpec {
        program: ResourceProgram::Executable {
            program: std::env::current_exe().unwrap().into_os_string(),
            args: helper_args(name),
        },
        current_dir: std::env::current_dir().unwrap(),
        stdin: ResourceStdin::Null,
        stdout: ResourceOutput::Piped,
        stderr: ResourceOutput::Piped,
    }
}

enum Output {
    Line(String),
    Eof,
    Error(std::io::Error),
}
struct Capture {
    receiver: Receiver<Output>,
    reader: thread::JoinHandle<()>,
}
impl Capture {
    fn new(reader: impl Read + Send + 'static) -> Self {
        let (sender, receiver) = mpsc::channel();
        let reader = thread::spawn(move || {
            for line in BufReader::new(reader).lines() {
                let output = match line {
                    Ok(line) => Output::Line(line),
                    Err(e) => Output::Error(e),
                };
                if sender.send(output).is_err() {
                    return;
                }
            }
            let _ = sender.send(Output::Eof);
        });
        Self { receiver, reader }
    }
    fn ready(&self, marker: &str) {
        let deadline = Instant::now() + STARTUP;
        loop {
            match self
                .receiver
                .recv_timeout(deadline.saturating_duration_since(Instant::now()))
                .expect("helper readiness deadline")
            {
                Output::Line(line) if line.contains(marker) => return,
                Output::Line(_) => {}
                Output::Eof => panic!("helper exited before {marker}"),
                Output::Error(e) => panic!("helper output: {e}"),
            }
        }
    }
    fn eof(self, deadline: Instant) {
        self.finish(deadline);
    }
    fn finish(self, deadline: Instant) -> Vec<String> {
        let mut lines = Vec::new();
        loop {
            match self
                .receiver
                .recv_timeout(deadline.saturating_duration_since(Instant::now()))
                .expect("descendant still holds pipe after cleanup deadline")
            {
                Output::Eof => break,
                Output::Line(line) => lines.push(line),
                Output::Error(e) => panic!("helper output: {e}"),
            }
        }
        // EOF was sent; also bound thread completion instead of a blind join.
        while !self.reader.is_finished() {
            assert!(Instant::now() < deadline, "output reader deadline");
            thread::yield_now();
        }
        self.reader.join().unwrap();
        lines
    }
}
fn spawn(name: &str) -> (ResourceLimitedChild, Capture, Capture) {
    let mut child = ResourceLimiter::default().spawn(&spec(name)).unwrap();
    let stdout = Capture::new(child.take_stdout().unwrap());
    let stderr = Capture::new(child.take_stderr().unwrap());
    (child, stdout, stderr)
}
fn exited(child: &mut ResourceLimitedChild) -> std::process::ExitStatus {
    let deadline = Instant::now() + STARTUP;
    loop {
        if let Some(status) = child.poll_exit().unwrap() {
            return status;
        }
        assert!(Instant::now() < deadline, "root exit deadline");
        thread::sleep(Duration::from_millis(5));
    }
}
fn cleanup(child: &mut ResourceLimitedChild, stdout: Capture, stderr: Capture) {
    let deadline = Instant::now() + CLEANUP;
    let report = child.terminate_and_reap(deadline).unwrap();
    assert!(!report.already_finalized);
    stdout.eof(deadline);
    stderr.eof(deadline);
}

#[test]
#[serial_test::serial]
fn direct_owned_child_terminates_promptly() {
    let (mut child, stdout, stderr) = spawn("helper_direct");
    stdout.ready("DIRECT_READY");
    assert!(child.poll_exit().unwrap().is_none());
    cleanup(&mut child, stdout, stderr);
}

#[test]
#[serial_test::serial]
fn startup_descendant_is_contained_and_terminated() {
    // The root's first fixture action spawns a descendant, before readiness.
    // On Windows this exercises creation-time job membership, not assignment
    // after a running process has already had an opportunity to spawn children.
    let (mut child, stdout, stderr) = spawn("helper_tree");
    stdout.ready("DIRECT_READY");
    cleanup(&mut child, stdout, stderr);
}

#[test]
#[serial_test::serial]
fn naturally_exited_root_retains_descendant_ownership() {
    let (mut child, stdout, stderr) = spawn("helper_root_exit");
    stdout.ready("DIRECT_READY");
    let status = exited(&mut child);
    assert!(status.success());
    // Repeated observation must not reap the leader or discard the job.
    assert_eq!(child.poll_exit().unwrap(), Some(status));
    cleanup(&mut child, stdout, stderr);
    assert_eq!(child.poll_exit().unwrap(), Some(status));
}

#[test]
#[serial_test::serial]
fn finalization_is_idempotent_and_retains_original_exit_status() {
    let (mut child, stdout, stderr) = spawn("helper_exit");
    assert!(exited(&mut child).success());
    cleanup(&mut child, stdout, stderr);
    let id = child.id();
    for _ in 0..3 {
        let report = child.terminate_and_reap(Instant::now()).unwrap();
        assert!(report.already_finalized);
        assert!(report.status.success());
        assert_eq!(child.id(), id);
    }
}

#[test]
#[serial_test::serial]
fn expired_cleanup_deadline_is_reported_and_can_be_retried() {
    let (mut child, stdout, stderr) = spawn("helper_direct");
    stdout.ready("DIRECT_READY");
    assert!(matches!(
        child.terminate_and_reap(Instant::now()),
        Err(ResourceLimitError::CleanupDeadlineExceeded)
    ));
    cleanup(&mut child, stdout, stderr);
}

// Also cleans a failed sentinel assertion without ever waiting unboundedly.
struct Sentinel(std::process::Child);
impl Drop for Sentinel {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let deadline = Instant::now() + CLEANUP;
        while Instant::now() < deadline {
            if !matches!(self.0.try_wait(), Ok(None)) {
                break;
            }
            thread::sleep(Duration::from_millis(5));
        }
    }
}
#[test]
#[serial_test::serial]
fn unrelated_sentinel_is_not_killed() {
    let mut sentinel = Sentinel(
        Command::new(std::env::current_exe().unwrap())
            .args(helper_args("helper_direct"))
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    let output = Capture::new(sentinel.0.stdout.take().unwrap());
    output.ready("DIRECT_READY");
    let (mut child, stdout, stderr) = spawn("helper_tree");
    stdout.ready("DIRECT_READY");
    cleanup(&mut child, stdout, stderr);
    assert!(
        sentinel.0.try_wait().unwrap().is_none(),
        "unrelated process killed"
    );
    drop(sentinel);
    output.eof(Instant::now() + CLEANUP);
}

#[test]
#[serial_test::serial]
fn spawn_failures_release_resources_and_allow_subsequent_execution() {
    let directory = tempfile::tempdir().unwrap();
    let mut missing = spec("helper_direct");
    missing.program = ResourceProgram::Executable {
        program: directory.path().join("missing-executable").into_os_string(),
        args: Vec::new(),
    };
    let mut bad_cwd = spec("helper_direct");
    bad_cwd.current_dir = directory.path().join("missing-directory");
    #[cfg(windows)]
    let before = handle_count();
    #[cfg(unix)]
    let before = next_pipe_descriptors();
    #[cfg(windows)]
    eprintln!("HANDLE_DIAG baseline={before}");
    for iteration in 0..32 {
        let _ = iteration;
        assert!(matches!(
            ResourceLimiter::default().spawn(&missing),
            Err(ResourceLimitError::SpawnFailed(_))
        ));
        assert!(matches!(
            ResourceLimiter::default().spawn(&bad_cwd),
            Err(ResourceLimitError::SpawnFailed(_))
        ));
        #[cfg(windows)]
        eprintln!("HANDLE_DIAG iteration={iteration} count={}", handle_count());
    }
    #[cfg(windows)]
    eprintln!(
        "HANDLE_DIAG after_failures={} before={before}",
        handle_count()
    );
    #[cfg(windows)]
    assert!(
        handle_count() <= before + 2,
        "spawn failure leaked native handles"
    );
    #[cfg(unix)]
    assert_eq!(
        next_pipe_descriptors(),
        before,
        "spawn failure leaked pipe descriptors"
    );
    let (mut child, stdout, stderr) = spawn("helper_exit");
    assert!(exited(&mut child).success());
    cleanup(&mut child, stdout, stderr);
    #[cfg(windows)]
    eprintln!(
        "HANDLE_DIAG after_finalize={} before={before}",
        handle_count()
    );
    #[cfg(windows)]
    assert!(
        handle_count() <= before + 2,
        "finalization leaked native handles"
    );
    #[cfg(unix)]
    assert_eq!(
        next_pipe_descriptors(),
        before,
        "finalization leaked descriptors"
    );
}

#[cfg(windows)]
#[test]
#[serial_test::serial]
fn windows_exit_code_259_is_an_exit_not_a_live_process() {
    let (mut child, stdout, stderr) = spawn("helper_exit_259");
    assert_eq!(exited(&mut child).code(), Some(259));
    cleanup(&mut child, stdout, stderr);
    assert_eq!(child.poll_exit().unwrap().unwrap().code(), Some(259));
}

#[cfg(windows)]
#[test]
#[ignore]
fn helper_exit_259() {
    std::process::exit(259);
}

#[cfg(unix)]
fn next_pipe_descriptors() -> (i32, i32) {
    use std::os::fd::AsRawFd;
    let (reader, writer) = std::io::pipe().unwrap();
    (reader.as_raw_fd(), writer.as_raw_fd())
}

#[cfg(windows)]
fn handle_count() -> u32 {
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, GetProcessHandleCount};
    let mut count = 0;
    assert_ne!(
        unsafe { GetProcessHandleCount(GetCurrentProcess(), &mut count) },
        0
    );
    count
}

#[cfg(windows)]
#[test]
#[serial_test::serial]
fn windows_shell_preserves_legacy_cmd_tail() {
    // Compare with the previous production invocation, including quoted spaces
    // and trailing backslashes. cmd is resolved from the system directory in
    // the implementation; this baseline only compares argument behavior.
    for command in [
        "echo hello world",
        "echo \"quoted text\"",
        r"echo C:\some path\",
        "exit /B 7",
    ] {
        let mut baseline = Sentinel(
            Command::new("cmd")
                .args(["/C", command])
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap(),
        );
        let baseline_out = Capture::new(baseline.0.stdout.take().unwrap());
        let baseline_err = Capture::new(baseline.0.stderr.take().unwrap());
        let deadline = Instant::now() + STARTUP;
        let baseline_status = loop {
            if let Some(status) = baseline.0.try_wait().unwrap() {
                break status;
            }
            assert!(Instant::now() < deadline, "legacy cmd deadline");
            thread::sleep(Duration::from_millis(5));
        };
        let baseline_out = baseline_out.finish(deadline);
        let baseline_err = baseline_err.finish(deadline);
        let mut shell = spec("helper_exit");
        shell.program = ResourceProgram::Shell(command.to_owned());
        let mut child = ResourceLimiter::default().spawn(&shell).unwrap();
        let stdout = Capture::new(child.take_stdout().unwrap());
        let stderr = Capture::new(child.take_stderr().unwrap());
        let status = exited(&mut child);
        let deadline = Instant::now() + CLEANUP;
        child.terminate_and_reap(deadline).unwrap();
        assert_eq!(status, baseline_status);
        assert_eq!(stdout.finish(deadline), baseline_out);
        assert_eq!(stderr.finish(deadline), baseline_err);
    }
}

// Each fixture is an ignored libtest entry invoked only by exact name. No
// environment mutation or platform executable is needed. Even broken cleanup
// leaves at most finite 15-second helpers, never hundreds of seconds.
#[test]
#[ignore]
fn helper_direct() {
    println!("DIRECT_READY");
    std::io::stdout().flush().unwrap();
    eprintln!("STDERR_READY");
    thread::sleep(EXPIRY);
}
#[test]
#[ignore]
fn helper_exit() {}
fn descendant() -> std::process::Child {
    Command::new(std::env::current_exe().unwrap())
        .args(helper_args("helper_direct"))
        .spawn()
        .unwrap()
}
#[test]
#[ignore]
fn helper_tree() {
    let mut child = Sentinel(descendant());
    let deadline = Instant::now() + EXPIRY;
    while Instant::now() < deadline {
        if child.0.try_wait().unwrap().is_some() {
            return;
        }
        thread::sleep(Duration::from_millis(10));
    }
}
#[test]
#[ignore]
#[allow(clippy::zombie_processes)] // Intentional orphan, finite fixture tests natural-root-exit cleanup.
fn helper_root_exit() {
    drop(descendant());
}

#[cfg(target_os = "linux")]
#[test]
#[serial_test::serial]
fn linux_all_four_limits_and_memory_file_enforcement() {
    let limits = ResourceLimits {
        max_memory_bytes: 256 * 1024 * 1024,
        max_cpu_seconds: 3,
        max_processes: 4096,
        max_file_size_bytes: 16 * 1024,
        timeout_seconds: 10,
    };
    let directory = tempfile::tempdir().unwrap();
    let mut spec = spec("helper_linux_limits");
    spec.current_dir = directory.path().to_owned();
    let mut child = ResourceLimiter::new(limits).spawn(&spec).unwrap();
    let stdout = Capture::new(child.take_stdout().unwrap());
    let stderr = Capture::new(child.take_stderr().unwrap());
    let status = exited(&mut child);
    cleanup(&mut child, stdout, stderr);
    assert!(
        status.success(),
        "rlimit verification helper failed: {status}"
    );
}
#[cfg(target_os = "linux")]
#[test]
#[ignore]
fn helper_linux_limits() {
    use nix::sys::resource::{getrlimit, Resource};
    for (resource, value) in [
        (Resource::RLIMIT_AS, 256 * 1024 * 1024),
        (Resource::RLIMIT_CPU, 3),
        (Resource::RLIMIT_NPROC, 4096),
        (Resource::RLIMIT_FSIZE, 16 * 1024),
    ] {
        assert_eq!(getrlimit(resource).unwrap(), (value, value));
    }
    let mut allocation = Vec::<u8>::new();
    assert!(
        allocation.try_reserve_exact(512 * 1024 * 1024).is_err(),
        "RLIMIT_AS not enforced"
    );
    // Ignore SIGXFSZ only in this helper, to inspect EFBIG and resulting size.
    unsafe {
        nix::sys::signal::signal(
            nix::sys::signal::Signal::SIGXFSZ,
            nix::sys::signal::SigHandler::SigIgn,
        )
        .unwrap();
    }
    let mut file = std::fs::File::create("bounded-output").unwrap();
    let error = file.write_all(&vec![1; 32 * 1024]).unwrap_err();
    assert_eq!(error.raw_os_error(), Some(nix::libc::EFBIG));
    assert_eq!(file.metadata().unwrap().len(), 16 * 1024);
}
#[cfg(target_os = "linux")]
#[test]
#[serial_test::serial]
fn linux_cpu_limit_enforced() {
    use std::os::unix::process::ExitStatusExt;
    let limits = ResourceLimits {
        max_cpu_seconds: 1,
        ..ResourceLimits::default()
    };
    let mut child = ResourceLimiter::new(limits)
        .spawn(&spec("helper_cpu"))
        .unwrap();
    let stdout = Capture::new(child.take_stdout().unwrap());
    let stderr = Capture::new(child.take_stderr().unwrap());
    stdout.ready("CPU_READY");
    let status = exited(&mut child);
    cleanup(&mut child, stdout, stderr);
    assert_eq!(status.signal(), Some(nix::libc::SIGKILL));
}
#[cfg(target_os = "linux")]
#[test]
#[ignore]
fn helper_cpu() {
    println!("CPU_READY");
    std::io::stdout().flush().unwrap();
    let deadline = Instant::now() + EXPIRY;
    while Instant::now() < deadline {
        std::hint::black_box(7u64.wrapping_mul(11));
    }
}
#[cfg(target_os = "linux")]
#[test]
#[serial_test::serial]
fn linux_nproc_prevents_additional_processes() {
    // Linux exempts UID 0 from RLIMIT_NPROC; the value check above still runs.
    if unsafe { nix::libc::geteuid() } == 0 {
        return;
    }
    let limits = ResourceLimits {
        max_processes: 1,
        ..ResourceLimits::default()
    };
    match ResourceLimiter::new(limits).spawn(&spec("helper_attempt_spawn")) {
        Err(ResourceLimitError::SpawnFailed(error))
            if error.raw_os_error() == Some(nix::libc::EAGAIN) => {}
        Ok(mut child) => {
            let stdout = Capture::new(child.take_stdout().unwrap());
            let stderr = Capture::new(child.take_stderr().unwrap());
            let status = exited(&mut child);
            cleanup(&mut child, stdout, stderr);
            // The harness itself may fail creating its test thread first.
            assert!(
                !status.success(),
                "RLIMIT_NPROC allowed additional processes"
            );
        }
        Err(error) => panic!("unexpected NPROC spawn error: {error}"),
    }
}
#[cfg(target_os = "linux")]
#[test]
#[ignore]
fn helper_attempt_spawn() {
    let mut command = Command::new(std::env::current_exe().unwrap());
    command.args(helper_args("helper_exit"));
    match command.spawn() {
        Ok(child) => {
            drop(Sentinel(child));
        }
        Err(error) => {
            assert_eq!(error.raw_os_error(), Some(nix::libc::EAGAIN));
            std::process::exit(42);
        }
    }
}
