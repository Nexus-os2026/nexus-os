//! Owned OS containment for governed subprocesses.
//!
//! Linux/macOS create a process group before exec; Linux additionally applies
//! four hard rlimits. Windows assigns a private kill-on-close Job Object during
//! process creation. Always explicitly finalize, even after natural root exit.
//! Unix groups contain descendants that remain in the group; this is not a
//! sandbox against a workload deliberately calling setsid/setpgid.

use serde::{Deserialize, Serialize};
use std::ffi::OsString;
use std::io::{self, Read};
use std::path::PathBuf;
use std::process::ExitStatus;
#[cfg(any(target_os = "linux", target_os = "macos", windows))]
use std::time::Duration;
use std::time::Instant;

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[path = "resource_limiter/unix.rs"]
mod platform;
#[cfg(windows)]
#[path = "resource_limiter/windows.rs"]
mod platform;

/// Linux hard resource limits; other supported platforms provide tree containment.
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

/// Failures preserve their OS error and distinguish cleanup from command failure.
#[derive(Debug, thiserror::Error)]
pub enum ResourceLimitError {
    #[error("process setup/spawn failed: {0}")]
    SpawnFailed(#[source] io::Error),
    #[error("process group/job setup failed: {0}")]
    ContainmentSetupFailed(#[source] io::Error),
    #[error("setrlimit failed: {0}")]
    SetLimitFailed(#[source] io::Error),
    #[error("exit observation failed: {0}")]
    ObservationFailed(#[source] io::Error),
    #[error("tree termination/reap failed: {0}")]
    TerminationFailed(#[source] io::Error),
    #[error("process cleanup deadline exceeded")]
    CleanupDeadlineExceeded,
    #[error("resource containment is unsupported on this platform")]
    UnsupportedPlatform,
}

/// Only the two execution forms required by the terminal and native helpers.
#[derive(Debug, Clone)]
pub enum ResourceProgram {
    /// Use an explicit executable path for portability (Windows does not search PATH).
    Executable {
        program: OsString,
        args: Vec<OsString>,
    },
    /// `sh -lc` on Unix; system-directory `cmd.exe /C` on Windows.
    Shell(String),
}

#[derive(Debug, Clone, Copy)]
pub enum ResourceStdin {
    Inherit,
    Null,
}

#[derive(Debug, Clone, Copy)]
pub enum ResourceOutput {
    Inherit,
    Null,
    Piped,
}

/// Environment is inherited unchanged. No arbitrary pre-exec or handle hooks.
#[derive(Debug, Clone)]
pub struct ResourceSpawnSpec {
    pub program: ResourceProgram,
    pub current_dir: PathBuf,
    pub stdin: ResourceStdin,
    pub stdout: ResourceOutput,
    pub stderr: ResourceOutput,
}

pub type ResourceReader = Box<dyn Read + Send>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminationReport {
    pub status: ExitStatus,
    pub already_finalized: bool,
}

#[derive(Debug, Clone, Default)]
pub struct ResourceLimiter {
    limits: ResourceLimits,
}

impl ResourceLimiter {
    pub fn new(limits: ResourceLimits) -> Self {
        Self { limits }
    }

    pub fn limits(&self) -> &ResourceLimits {
        &self.limits
    }

    /// Fail closed if containment cannot be established before execution.
    pub fn spawn(
        &self,
        spec: &ResourceSpawnSpec,
    ) -> Result<ResourceLimitedChild, ResourceLimitError> {
        #[cfg(any(target_os = "linux", target_os = "macos", windows))]
        {
            let child = platform::Child::spawn(spec, &self.limits)?;
            Ok(ResourceLimitedChild {
                id: child.id(),
                child: Some(child),
                status: None,
            })
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
        {
            let _ = spec;
            Err(ResourceLimitError::UnsupportedPlatform)
        }
    }
}

/// Sole owner of process identity and containment. Intentionally not Clone.
///
/// `poll_exit` never releases that identity. After successful finalization only
/// the historical ID/status remain; subsequent calls never signal that ID.
/// On Unix the application must not independently reap this owner's child
/// (including via a global SIGCHLD reaper or SA_NOCLDWAIT).
pub struct ResourceLimitedChild {
    id: u32,
    #[cfg(any(target_os = "linux", target_os = "macos", windows))]
    child: Option<platform::Child>,
    status: Option<ExitStatus>,
}

impl ResourceLimitedChild {
    pub fn id(&self) -> u32 {
        self.id
    }

    pub fn take_stdout(&mut self) -> Option<ResourceReader> {
        #[cfg(any(target_os = "linux", target_os = "macos", windows))]
        {
            self.child.as_mut()?.take_stdout()
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
        {
            None
        }
    }

    pub fn take_stderr(&mut self) -> Option<ResourceReader> {
        #[cfg(any(target_os = "linux", target_os = "macos", windows))]
        {
            self.child.as_mut()?.take_stderr()
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
        {
            None
        }
    }

    /// Observe root exit without reaping or closing the containment identity.
    pub fn poll_exit(&mut self) -> Result<Option<ExitStatus>, ResourceLimitError> {
        if let Some(status) = self.status {
            return Ok(Some(status));
        }
        #[cfg(any(target_os = "linux", target_os = "macos", windows))]
        {
            self.child.as_mut().expect("unfinalized owner").poll_exit()
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
        {
            Err(ResourceLimitError::UnsupportedPlatform)
        }
    }

    /// Request tree termination, then finalize the root within `deadline`.
    /// Failure retains ownership for retry/Drop unless the root was finalized
    /// just as the deadline expired. Never join pipe readers after failure.
    /// An expired deadline still requests termination, but cannot
    /// report successful cleanup. Repeated successful calls are deterministic.
    pub fn terminate_and_reap(
        &mut self,
        deadline: Instant,
    ) -> Result<TerminationReport, ResourceLimitError> {
        if let Some(status) = self.status {
            return Ok(TerminationReport {
                status,
                already_finalized: true,
            });
        }
        #[cfg(any(target_os = "linux", target_os = "macos", windows))]
        {
            let child = self.child.as_mut().expect("unfinalized owner");
            child.request_termination()?;
            loop {
                if Instant::now() >= deadline {
                    return Err(ResourceLimitError::CleanupDeadlineExceeded);
                }
                if let Some(status) = child.try_finalize()? {
                    self.status = Some(status);
                    // OS identity is released only after termination and root reap.
                    self.child.take();
                    if Instant::now() >= deadline {
                        return Err(ResourceLimitError::CleanupDeadlineExceeded);
                    }
                    return Ok(TerminationReport {
                        status,
                        already_finalized: false,
                    });
                }
                std::thread::sleep(
                    Duration::from_millis(5)
                        .min(deadline.saturating_duration_since(Instant::now())),
                );
            }
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
        {
            let _ = deadline;
            Err(ResourceLimitError::UnsupportedPlatform)
        }
    }
}

impl Drop for ResourceLimitedChild {
    fn drop(&mut self) {
        #[cfg(any(target_os = "linux", target_os = "macos", windows))]
        if let Some(child) = self.child.as_mut() {
            // Defense in depth only: no blocking wait, no reportable success.
            if child.request_termination().is_ok() {
                let _ = child.try_finalize();
            }
        }
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
        let e = ResourceLimitError::SetLimitFailed(std::io::Error::from_raw_os_error(12));
        assert!(std::error::Error::source(&e).is_some());

        let e = ResourceLimitError::ContainmentSetupFailed(std::io::Error::from_raw_os_error(3));
        assert!(std::error::Error::source(&e).is_some());
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
}
