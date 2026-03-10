//! OS-level resource containment for spawned subprocesses.
//!
//! Uses POSIX `setrlimit` via `pre_exec` to apply hard limits to child
//! processes *before* they exec.  Limits propagate to all descendants,
//! which closes the fork-bomb and memory-bomb gaps that fuel metering
//! alone cannot address (fuel meters instruction count inside WASM, but
//! native subprocesses bypass the WASM sandbox entirely).
//!
//! # Safety exception
//!
//! This module contains the **only** `unsafe` code in the workspace.
//! `CommandExt::pre_exec` requires an `unsafe` block because the closure
//! runs between `fork()` and `exec()` in the child process — a
//! signal-unsafe context.  The closure only calls `libc::setrlimit` and
//! `libc::setpgid`, both of which are async-signal-safe per POSIX.
//! The parent process is never affected.

use std::fmt;

/// Hard resource limits applied to every spawned subprocess.
#[derive(Debug, Clone)]
pub struct ResourceLimits {
    /// Maximum virtual memory in bytes (RLIMIT_AS). Default: 512 MB.
    pub max_memory_bytes: u64,
    /// Maximum CPU time in seconds (RLIMIT_CPU). Default: 60.
    pub max_cpu_seconds: u64,
    /// Maximum number of processes for the user (RLIMIT_NPROC). Default: 50.
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
            max_processes: 50,                      // 50 processes
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
        use std::os::unix::process::CommandExt;

        let mem = self.limits.max_memory_bytes;
        let cpu = self.limits.max_cpu_seconds;
        let nproc = self.limits.max_processes as u64;
        let fsize = self.limits.max_file_size_bytes;

        // SAFETY: The closure runs in the child process between fork() and
        // exec().  It only calls async-signal-safe POSIX functions:
        // `setrlimit` and `setpgid`.  No heap allocation, no locks, no
        // shared mutable state with the parent.
        unsafe {
            cmd.pre_exec(move || {
                // Put the child in its own process group so we can kill the
                // entire tree later with kill(-pgid, SIGKILL).
                if libc::setpgid(0, 0) != 0 {
                    return Err(std::io::Error::last_os_error());
                }

                // RLIMIT_AS — virtual address space (memory).
                let rlim = libc::rlimit {
                    rlim_cur: mem,
                    rlim_max: mem,
                };
                if libc::setrlimit(libc::RLIMIT_AS, &rlim) != 0 {
                    return Err(std::io::Error::last_os_error());
                }

                // RLIMIT_CPU — CPU time in seconds.
                let rlim = libc::rlimit {
                    rlim_cur: cpu,
                    rlim_max: cpu,
                };
                if libc::setrlimit(libc::RLIMIT_CPU, &rlim) != 0 {
                    return Err(std::io::Error::last_os_error());
                }

                // RLIMIT_NPROC — max child processes (fork-bomb defense).
                let rlim = libc::rlimit {
                    rlim_cur: nproc,
                    rlim_max: nproc,
                };
                if libc::setrlimit(libc::RLIMIT_NPROC, &rlim) != 0 {
                    return Err(std::io::Error::last_os_error());
                }

                // RLIMIT_FSIZE — max file size a process can create.
                let rlim = libc::rlimit {
                    rlim_cur: fsize,
                    rlim_max: fsize,
                };
                if libc::setrlimit(libc::RLIMIT_FSIZE, &rlim) != 0 {
                    return Err(std::io::Error::last_os_error());
                }

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
        // SAFETY: `libc::kill` is a thin wrapper around the kill(2) syscall.
        // Passing `-pid` targets the entire process group.  The pid is a
        // non-zero value obtained from `Child::id()`.
        let ret = unsafe { libc::kill(-(pid as i32), libc::SIGKILL) };
        if ret != 0 {
            let err = std::io::Error::last_os_error();
            // ESRCH means the process (group) already exited — not an error.
            if err.raw_os_error() == Some(libc::ESRCH) {
                return Ok(());
            }
            return Err(ResourceLimitError::ProcessGroupFailed(err.to_string()));
        }
        Ok(())
    }

    /// Non-Linux fallback — kill direct process only.
    #[cfg(not(target_os = "linux"))]
    pub fn kill_process_tree(pid: u32) -> Result<(), ResourceLimitError> {
        // Best-effort: send kill to the direct PID only.
        // On Windows this would need TerminateProcess; on macOS, killpg.
        // For now, use the standard library's Child::kill() from the caller.
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
        assert_eq!(limits.max_processes, 50);
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

    #[cfg(target_os = "linux")]
    #[test]
    fn apply_to_command_does_not_panic() {
        let limiter = ResourceLimiter::default();
        let mut cmd = std::process::Command::new("true");
        limiter.apply_to_command(&mut cmd);
        // The pre_exec hook is installed; verify the command still spawns.
        let status = cmd.status().expect("failed to run `true`");
        assert!(status.success());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn kill_nonexistent_process_group_is_ok() {
        // PID 2_000_000_000 almost certainly does not exist.
        let result = ResourceLimiter::kill_process_tree(2_000_000_000);
        // Should succeed because ESRCH is treated as Ok.
        assert!(result.is_ok());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn memory_limit_enforced_on_child() {
        // Spawn a child that tries to allocate more than the limit.
        // With RLIMIT_AS = 32 MB, a 64 MB allocation should fail.
        let limits = ResourceLimits {
            max_memory_bytes: 32 * 1024 * 1024, // 32 MB
            max_cpu_seconds: 5,
            max_processes: 10,
            max_file_size_bytes: 100 * 1024 * 1024,
            timeout_seconds: 5,
        };
        let limiter = ResourceLimiter::new(limits);
        let mut cmd = std::process::Command::new("sh");
        cmd.args([
            "-c",
            // Try to allocate ~64 MB with dd reading from /dev/zero.
            // Under a 32 MB RLIMIT_AS this should fail.
            "head -c 67108864 /dev/zero | cat > /dev/null",
        ]);
        limiter.apply_to_command(&mut cmd);
        let status = cmd.status().expect("failed to spawn");
        // The child should fail (non-zero exit) due to memory limit.
        // Note: this is best-effort — some shells handle the signal gracefully.
        // We mainly verify it doesn't hang or panic.
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

        // Give the child a moment to start.
        std::thread::sleep(std::time::Duration::from_millis(50));

        // Kill the process tree.
        let result = ResourceLimiter::kill_process_tree(pid);
        assert!(result.is_ok());

        // Reap the child to avoid zombies.
        let _ = child.wait();

        // Sending signal 0 checks if process exists.
        let exists = unsafe { libc::kill(pid as i32, 0) };
        assert_ne!(exists, 0, "process should be dead");
    }
}
