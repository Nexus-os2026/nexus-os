use super::*;

#[cfg(any(target_os = "macos", test))]
#[path = "darwin_group.rs"]
mod darwin_group;
use nix::errno::Errno;
use nix::libc;
use nix::sys::signal::{killpg, Signal};
use nix::unistd::{setpgid, Pid};
use std::os::fd::AsRawFd;
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::process::{Command, Stdio};

pub(super) struct Child {
    process: std::process::Child,
    identity_valid: bool,
    termination_requested: bool,
}

impl Child {
    pub(super) fn spawn(
        spec: &ResourceSpawnSpec,
        limits: &ResourceLimits,
    ) -> Result<Self, ResourceLimitError> {
        let mut command = match &spec.program {
            ResourceProgram::Executable { program, args } => {
                let mut command = Command::new(program);
                command.args(args);
                command
            }
            ResourceProgram::Shell(script) => {
                let mut command = Command::new("sh");
                command.args(["-lc", script]);
                command
            }
        };
        command
            .current_dir(&spec.current_dir)
            .stdin(match spec.stdin {
                ResourceStdin::Inherit => Stdio::inherit(),
                ResourceStdin::Null => Stdio::null(),
            })
            .stdout(output(spec.stdout))
            .stderr(output(spec.stderr));
        // Legacy short-lived limits: Linux applies all four; macOS none.
        #[cfg(target_os = "linux")]
        let rlimits = vec![
            (Rlimit::AddressSpace, limits.max_memory_bytes),
            (Rlimit::Cpu, limits.max_cpu_seconds),
            (Rlimit::Processes, u64::from(limits.max_processes)),
            (Rlimit::FileSize, limits.max_file_size_bytes),
        ];
        #[cfg(target_os = "macos")]
        let rlimits = {
            let _ = limits;
            Vec::new()
        };
        spawn_contained(command, rlimits)
    }

    /// Sealed long-lived spawn: the parent environment is cleared before the
    /// validated sealed entries are applied, the absolute program is never
    /// PATH-resolved, and the fixed long-lived rlimits are installed (with the
    /// process group) before exec. Any setup failure prevents execution.
    pub(super) fn spawn_sealed(spec: &SealedSpawnSpec) -> Result<Self, ResourceLimitError> {
        let mut command = Command::new(&spec.program);
        command
            .args(&spec.args)
            .env_clear()
            .env("HOME", spec.environment.home())
            .env("TMPDIR", spec.environment.temp())
            .envs(spec.environment.variables())
            .current_dir(&spec.current_dir)
            .stdin(Stdio::null())
            .stdout(output(spec.stdout))
            .stderr(output(spec.stderr));
        spawn_contained(command, sealed_rlimits())
    }
    pub(super) fn id(&self) -> u32 {
        self.process.id()
    }
    pub(super) fn take_stdout(&mut self) -> Option<ResourceReader> {
        self.process
            .stdout
            .take()
            .map(|pipe| Box::new(pipe) as ResourceReader)
    }
    pub(super) fn take_stderr(&mut self) -> Option<ResourceReader> {
        self.process
            .stderr
            .take()
            .map(|pipe| Box::new(pipe) as ResourceReader)
    }

    pub(super) fn poll_exit(&mut self) -> Result<Option<ExitStatus>, ResourceLimitError> {
        if !self.identity_valid {
            return Err(ResourceLimitError::ObservationFailed(
                io::Error::from_raw_os_error(libc::ECHILD),
            ));
        }
        // nix 0.29 does not expose waitid on macOS, so use its libc re-export
        // on both platforms. WNOWAIT reserves the unreaped leader's PID/PGID.
        // SAFETY: initialized, correctly sized siginfo and our own child PID.
        let mut info: libc::siginfo_t = unsafe { std::mem::zeroed() };
        let result = unsafe {
            libc::waitid(
                libc::P_PID,
                self.id() as libc::id_t,
                &mut info,
                libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,
            )
        };
        if result < 0 {
            let error = io::Error::last_os_error();
            if error.raw_os_error() == Some(libc::ECHILD) {
                // An external reaper violated sole ownership. Never signal a
                // potentially recycled identity, including from Drop.
                self.identity_valid = false;
            }
            if error.kind() == io::ErrorKind::Interrupted {
                return Ok(None);
            }
            return Err(ResourceLimitError::ObservationFailed(error));
        }
        // SAFETY: waitid initialized these SIGCHLD fields (zero means no exit).
        if unsafe { info.si_pid() } == 0 {
            return Ok(None);
        }
        let code = unsafe { info.si_status() };
        let raw = match info.si_code {
            libc::CLD_EXITED => code << 8,
            libc::CLD_KILLED => code,
            libc::CLD_DUMPED => code | 0x80,
            _ => {
                return Err(ResourceLimitError::ObservationFailed(io::Error::other(
                    "unexpected waitid exit state",
                )))
            }
        };
        Ok(Some(ExitStatus::from_raw(raw)))
    }

    pub(super) fn request_termination(&mut self) -> Result<(), ResourceLimitError> {
        if !self.identity_valid {
            return Err(ResourceLimitError::TerminationFailed(
                io::Error::from_raw_os_error(libc::ECHILD),
            ));
        }
        if self.termination_requested {
            return Ok(());
        }
        let root_exited = self.poll_exit()?.is_some();
        match killpg(Pid::from_raw(self.id() as i32), Signal::SIGKILL) {
            Ok(()) => {}
            Err(Errno::ESRCH) if root_exited => {}
            #[cfg(target_os = "macos")]
            Err(Errno::EPERM) if root_exited => {
                // Keep the leader unreaped while inspecting every group member.
                // Root-only ESRCH cannot rule out a live, unsignalable descendant.
                darwin_group::confirm_terminal(self.id() as i32).map_err(|error| {
                    ResourceLimitError::TerminationFailed(io::Error::from_raw_os_error(
                        error as i32,
                    ))
                })?;
                // Detect an external reaper before accepting the observation.
                // poll_exit invalidates ownership on ECHILD; never signal again.
                if self.poll_exit()?.is_none() {
                    return Err(ResourceLimitError::ObservationFailed(io::Error::other(
                        "owned root exit changed during group observation",
                    )));
                }
            }
            Err(error) => {
                return Err(ResourceLimitError::TerminationFailed(
                    io::Error::from_raw_os_error(error as i32),
                ))
            }
        }
        self.termination_requested = true;
        Ok(())
    }

    pub(super) fn try_finalize(&mut self) -> Result<Option<ExitStatus>, ResourceLimitError> {
        debug_assert!(self.termination_requested);
        match self.process.try_wait() {
            Ok(Some(status)) => {
                self.identity_valid = false; // no further PGID use after reap
                Ok(Some(status))
            }
            Ok(None) => Ok(None),
            Err(error) => {
                if error.raw_os_error() == Some(libc::ECHILD) {
                    self.identity_valid = false;
                }
                Err(ResourceLimitError::TerminationFailed(error))
            }
        }
    }
}

/// The fixed P0-002C4C2 long-lived policy: per-process committed private
/// memory (Linux RLIMIT_DATA) and file size. Deliberately no RLIMIT_AS (V8
/// reserves far more address space than it commits), no RLIMIT_CPU and no new
/// RLIMIT_NPROC. macOS: file size only; no memory bound is claimed there.
pub(super) fn sealed_rlimits() -> Vec<(Rlimit, u64)> {
    const FILE_SIZE_BYTES: u64 = 100 * 1024 * 1024;
    #[cfg(target_os = "linux")]
    {
        const DATA_BYTES: u64 = 2 * 1024 * 1024 * 1024;
        vec![
            (Rlimit::Data, DATA_BYTES),
            (Rlimit::FileSize, FILE_SIZE_BYTES),
        ]
    }
    #[cfg(target_os = "macos")]
    {
        vec![(Rlimit::FileSize, FILE_SIZE_BYTES)]
    }
}

/// A resource the limiter bounds. The legacy-only resources exist only where
/// the legacy policy applies them (Linux).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Rlimit {
    #[cfg(target_os = "linux")]
    AddressSpace,
    #[cfg(target_os = "linux")]
    Cpu,
    #[cfg(target_os = "linux")]
    Processes,
    #[cfg(target_os = "linux")]
    Data,
    FileSize,
}

/// Installs `value` as both the soft and hard limit. Uses nix's libc re-export
/// because nix's `resource` feature is enabled only on Linux. Async-signal-safe:
/// one setrlimit call and an errno read, no allocation.
fn set_hard_limit(limit: Rlimit, value: u64) -> Result<(), Errno> {
    let resource = match limit {
        #[cfg(target_os = "linux")]
        Rlimit::AddressSpace => libc::RLIMIT_AS,
        #[cfg(target_os = "linux")]
        Rlimit::Cpu => libc::RLIMIT_CPU,
        #[cfg(target_os = "linux")]
        Rlimit::Processes => libc::RLIMIT_NPROC,
        #[cfg(target_os = "linux")]
        Rlimit::Data => libc::RLIMIT_DATA,
        Rlimit::FileSize => libc::RLIMIT_FSIZE,
    };
    let bound = libc::rlimit {
        rlim_cur: value,
        rlim_max: value,
    };
    // SAFETY: setrlimit only reads the initialized `bound`.
    Errno::result(unsafe { libc::setrlimit(resource, &bound) }).map(drop)
}

/// Fork/exec with the owned process group and the given hard=soft rlimits
/// installed in the child before exec. Shared by legacy and sealed spawns.
fn spawn_contained(
    mut command: Command,
    rlimits: Vec<(Rlimit, u64)>,
) -> Result<Child, ResourceLimitError> {
    // A close-on-exec pipe distinguishes pre-exec containment/limit errors
    // from executable errors. Only a one-byte write occurs after fork.
    let (mut setup_reader, setup_writer) = io::pipe().map_err(ResourceLimitError::SpawnFailed)?;
    // SAFETY: after fork this calls only setpgid/setrlimit/write, iterates an
    // already-allocated vector and uses from_raw_os_error (no allocation),
    // never Error::other(errno).
    unsafe {
        command.pre_exec(move || {
            let fail = |stage: u8, errno: Errno| {
                libc::write(setup_writer.as_raw_fd(), (&stage as *const u8).cast(), 1);
                io::Error::from_raw_os_error(errno as i32)
            };
            setpgid(Pid::from_raw(0), Pid::from_raw(0)).map_err(|e| fail(1, e))?;
            for &(resource, value) in &rlimits {
                set_hard_limit(resource, value).map_err(|e| fail(2, e))?;
            }
            Ok(())
        });
    }
    let spawned = command.spawn();
    drop(command); // closes the parent's copy of setup_writer on all paths
    match spawned {
        Ok(process) => Ok(Child {
            process,
            identity_valid: true,
            termination_requested: false,
        }),
        Err(error) => {
            let mut stage = [0];
            match setup_reader.read_exact(&mut stage) {
                Ok(()) => {}
                Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => {}
                Err(error) => return Err(ResourceLimitError::SpawnFailed(error)),
            }
            Err(match stage[0] {
                1 => ResourceLimitError::ContainmentSetupFailed(error),
                2 => ResourceLimitError::SetLimitFailed(error),
                _ => ResourceLimitError::SpawnFailed(error),
            })
        }
    }
}

fn output(mode: ResourceOutput) -> Stdio {
    match mode {
        ResourceOutput::Inherit => Stdio::inherit(),
        ResourceOutput::Null => Stdio::null(),
        ResourceOutput::Piped => Stdio::piped(),
    }
}
