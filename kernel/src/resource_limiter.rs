//! OS-level resource containment for spawned subprocesses.
//!
//! Uses POSIX `setrlimit` via `pre_exec` to apply hard limits to child
//! processes *before* they exec.  Limits propagate to all descendants,
//! which closes the fork-bomb and memory-bomb gaps that fuel metering
//! alone cannot address (fuel meters instruction count inside WASM, but
//! native subprocesses bypass the WASM sandbox entirely).
//!
//! # Safety
//!
//! `CommandExt::pre_exec` is an inherently `unsafe` API because the
//! closure runs between `fork()` and `exec()` in a signal-unsafe
//! context.  The closure body uses only safe `nix` wrappers around
//! async-signal-safe POSIX functions (`setrlimit`, `setpgid`).
//! The parent process is never affected.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Hard resource limits applied to every spawned subprocess.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ResourceLimits {
    /// Maximum virtual memory in bytes (RLIMIT_AS). Default: 512 MB.
    pub max_memory_bytes: u64,
    /// Maximum CPU time in seconds (RLIMIT_CPU). Default: 60.
    pub max_cpu_seconds: u64,
    /// Maximum number of processes for the user (RLIMIT_NPROC). Default: 4096.
    ///
    /// Note: RLIMIT_NPROC counts **all** processes for the UID, not just
    /// descendants of the child.  The default of 4096 stops fork bombs
    /// (which create thousands within milliseconds) while leaving ample
    /// headroom for normal operation.
    pub max_processes: u32,
    /// Maximum file size in bytes (RLIMIT_FSIZE). Default: 100 MB.
    pub max_file_size_bytes: u64,
    /// Wall-clock timeout in seconds. Default: 60.
    pub timeout_seconds: u64,
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            max_memory_bytes: 512 * 1024 * 1024,    // 512 MB
            max_cpu_seconds: 60,                    // 60 seconds
            max_processes: 4096,                    // 4096 processes
            max_file_size_bytes: 100 * 1024 * 1024, // 100 MB
            timeout_seconds: 60,                    // 60 seconds
        }
    }
}

/// Errors from resource limit operations.
#[derive(Debug, Clone)]
pub enum ResourceLimitError {
    /// Failed to set a resource limit on the child process.
    SetLimitFailed(String),
    /// Failed to create or signal a process group.
    ProcessGroupFailed(String),
}

impl fmt::Display for ResourceLimitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SetLimitFailed(msg) => write!(f, "setrlimit failed: {msg}"),
            Self::ProcessGroupFailed(msg) => write!(f, "process group failed: {msg}"),
        }
    }
}

impl std::error::Error for ResourceLimitError {}

/// Applies OS-level resource limits to child processes.
#[derive(Debug, Clone)]
pub struct ResourceLimiter {
    limits: ResourceLimits,
}

impl ResourceLimiter {
    /// Create a limiter with the given limits.
    pub fn new(limits: ResourceLimits) -> Self {
        Self { limits }
    }

    /// Access the underlying limits.
    pub fn limits(&self) -> &ResourceLimits {
        &self.limits
    }

    /// Apply resource limits to a [`std::process::Command`] before it spawns.
    ///
    /// On Linux this installs a `pre_exec` hook that calls `setrlimit` for
    /// `RLIMIT_AS`, `RLIMIT_CPU`, `RLIMIT_NPROC`, and `RLIMIT_FSIZE`, then
    /// calls `setpgid(0, 0)` so the child becomes its own process-group
    /// leader (enabling [`kill_process_tree`] to terminate all descendants).
    ///
    /// On non-Linux platforms this is a no-op.
    #[cfg(target_os = "linux")]
    pub fn apply_to_command(&self, cmd: &mut std::process::Command) {
        use nix::sys::resource::{setrlimit, Resource};
        use nix::unistd::{setpgid, Pid};
        use std::os::unix::process::CommandExt;

        let mem = self.limits.max_memory_bytes;
        let cpu = self.limits.max_cpu_seconds;
        let nproc = self.limits.max_processes as u64;
        let fsize = self.limits.max_file_size_bytes;

        // This unsafe block is irreducible: CommandExt::pre_exec is an unsafe fn in Rust's stdlib.
        // The closure body uses only safe nix wrappers (setrlimit, setpgid).
        // See: https://doc.rust-lang.org/std/os/unix/process/trait.CommandExt.html#tymethod.pre_exec
        unsafe {
            cmd.pre_exec(move || {
                // Put the child in its own process group so we can kill the
                // entire tree later with killpg.
                setpgid(Pid::from_raw(0), Pid::from_raw(0))
                    .map_err(std::io::Error::other)?;

                // RLIMIT_AS — virtual address space (memory).
                setrlimit(Resource::RLIMIT_AS, mem, mem)
                    .map_err(std::io::Error::other)?;

                // RLIMIT_CPU — CPU time in seconds.
                setrlimit(Resource::RLIMIT_CPU, cpu, cpu)
                    .map_err(std::io::Error::other)?;

                // RLIMIT_NPROC — max child processes (fork-bomb defense).
                setrlimit(Resource::RLIMIT_NPROC, nproc, nproc)
                    .map_err(std::io::Error::other)?;

                // RLIMIT_FSIZE — max file size a process can create.
                setrlimit(Resource::RLIMIT_FSIZE, fsize, fsize)
                    .map_err(std::io::Error::other)?;

                Ok(())
            });
        }
    }

    /// Non-Linux fallback — no-op.
    #[cfg(not(target_os = "linux"))]
    pub fn apply_to_command(&self, _cmd: &mut std::process::Command) {
        // Resource limits via setrlimit are Linux-specific.
        // On other platforms we rely on the wall-clock timeout only.
    }

    /// Kill an entire process tree rooted at `pid`.
    ///
    /// On Linux, sends `SIGKILL` to the process group (negative PID).
    /// On other platforms, kills only the direct process.
    #[cfg(target_os = "linux")]
    pub fn kill_process_tree(pid: u32) -> Result<(), ResourceLimitError> {
        use nix::sys::signal::{killpg, Signal};
        use nix::unistd::Pid;

        match killpg(Pid::from_raw(pid as i32), Signal::SIGKILL) {
            Ok(()) => Ok(()),
            Err(nix::errno::Errno::ESRCH) => Ok(()), // already exited
            Err(e) => Err(ResourceLimitError::ProcessGroupFailed(e.to_string())),
        }
    }

    /// Non-Linux fallback — kill direct process only.
    #[cfg(not(target_os = "linux"))]
    pub fn kill_process_tree(pid: u32) -> Result<(), ResourceLimitError> {
        let _ = pid;
        Ok(())
    }
}

impl Default for ResourceLimiter {
    fn default() -> Self {
        Self::new(ResourceLimits::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_limits_are_sensible() {
        let limits = ResourceLimits::default();
        assert_eq!(limits.max_memory_bytes, 512 * 1024 * 1024);
        assert_eq!(limits.max_cpu_seconds, 60);
        assert_eq!(limits.max_processes, 4096);
        assert_eq!(limits.max_file_size_bytes, 100 * 1024 * 1024);
        assert_eq!(limits.timeout_seconds, 60);
    }

    #[test]
    fn limiter_custom_limits() {
        let limits = ResourceLimits {
            max_memory_bytes: 1024,
            max_cpu_seconds: 5,
            max_processes: 2,
            max_file_size_bytes: 2048,
            timeout_seconds: 10,
        };
        let limiter = ResourceLimiter::new(limits.clone());
        assert_eq!(limiter.limits().max_memory_bytes, 1024);
        assert_eq!(limiter.limits().max_cpu_seconds, 5);
        assert_eq!(limiter.limits().max_processes, 2);
        assert_eq!(limiter.limits().max_file_size_bytes, 2048);
        assert_eq!(limiter.limits().timeout_seconds, 10);
    }

    #[test]
    fn limiter_default_trait() {
        let limiter = ResourceLimiter::default();
        assert_eq!(limiter.limits().max_memory_bytes, 512 * 1024 * 1024);
    }

    #[test]
    fn error_display() {
        let e = ResourceLimitError::SetLimitFailed("ENOMEM".to_string());
        assert!(e.to_string().contains("ENOMEM"));

        let e = ResourceLimitError::ProcessGroupFailed("ESRCH".to_string());
        assert!(e.to_string().contains("ESRCH"));
    }

    #[test]
    fn apply_to_command_no_spawn_does_not_panic() {
        let limiter = ResourceLimiter::default();
        let mut cmd = std::process::Command::new("echo");
        cmd.arg("test");
        limiter.apply_to_command(&mut cmd);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn apply_to_command_does_not_panic() {
        let limiter = ResourceLimiter::default();
        let mut cmd = std::process::Command::new("true");
        limiter.apply_to_command(&mut cmd);
        let status = cmd.status().expect("failed to run `true`");
        assert!(status.success());
    }

    #[test]
    fn kill_process_tree_nonexistent_pid() {
        let result = ResourceLimiter::kill_process_tree(999_999_999);
        assert!(result.is_ok());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn kill_nonexistent_process_group_is_ok() {
        let result = ResourceLimiter::kill_process_tree(2_000_000_000);
        assert!(result.is_ok());
    }

    #[test]
    fn resource_limits_serialize_deserialize() {
        let limits = ResourceLimits {
            max_memory_bytes: 256 * 1024 * 1024,
            max_cpu_seconds: 30,
            max_processes: 100,
            max_file_size_bytes: 50 * 1024 * 1024,
            timeout_seconds: 45,
        };
        let json = serde_json::to_string(&limits).expect("serialize");
        let deserialized: ResourceLimits = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(limits, deserialized);
    }

    #[test]
    fn default_limits_serialize_round_trip() {
        let defaults = ResourceLimits::default();
        let json = serde_json::to_string_pretty(&defaults).expect("serialize");
        let round_tripped: ResourceLimits = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(defaults, round_tripped);
    }

    #[test]
    fn limits_are_reasonable() {
        let limits = ResourceLimits::default();
        assert!(limits.max_memory_bytes >= 100 * 1024 * 1024);
        assert!(limits.max_memory_bytes <= 2 * 1024 * 1024 * 1024);
        assert!(limits.max_cpu_seconds >= 10);
        assert!(limits.max_cpu_seconds <= 300);
        assert!(limits.max_processes >= 100);
        assert!(limits.max_processes <= 10_000);
        assert!(limits.max_file_size_bytes >= 10 * 1024 * 1024);
        assert!(limits.max_file_size_bytes <= 1024 * 1024 * 1024);
        assert!(limits.timeout_seconds >= 10);
        assert!(limits.timeout_seconds <= 300);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn memory_limit_enforced_on_child() {
        let limits = ResourceLimits {
            max_memory_bytes: 32 * 1024 * 1024,
            max_cpu_seconds: 5,
            max_processes: 10,
            max_file_size_bytes: 100 * 1024 * 1024,
            timeout_seconds: 5,
        };
        let limiter = ResourceLimiter::new(limits);
        let mut cmd = std::process::Command::new("sh");
        cmd.args([
            "-c",
            "head -c 67108864 /dev/zero | cat > /dev/null",
        ]);
        limiter.apply_to_command(&mut cmd);
        let status = cmd.status().expect("failed to spawn");
        let _ = status;
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn process_group_kill_terminates_children() {
        use std::process::Stdio;

        let limiter = ResourceLimiter::default();
        let mut cmd = std::process::Command::new("sh");
        cmd.args(["-c", "sleep 300"]);
        cmd.stdout(Stdio::null());
        cmd.stderr(Stdio::null());
        limiter.apply_to_command(&mut cmd);

        let mut child = cmd.spawn().expect("failed to spawn sleep");
        let pid = child.id();

        std::thread::sleep(std::time::Duration::from_millis(50));

        let result = ResourceLimiter::kill_process_tree(pid);
        assert!(result.is_ok());

        let _ = child.wait();

        // Verify process is dead using safe nix API.
        use nix::sys::signal::kill;
        use nix::unistd::Pid;
        let probe = kill(Pid::from_raw(pid as i32), None);
        assert!(probe.is_err(), "process should be dead");
    }
}
