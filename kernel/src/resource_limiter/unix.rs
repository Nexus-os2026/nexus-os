use super::*;
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

        // A close-on-exec pipe distinguishes pre-exec containment/limit errors
        // from executable errors. Only a one-byte write occurs after fork.
        let (mut setup_reader, setup_writer) =
            io::pipe().map_err(ResourceLimitError::SpawnFailed)?;
        #[cfg(target_os = "linux")]
        let limits = limits.clone();
        #[cfg(target_os = "macos")]
        let _ = limits;
        // SAFETY: after fork this calls only setpgid/setrlimit/write and uses
        // from_raw_os_error (no allocation), never Error::other(errno).
        unsafe {
            command.pre_exec(move || {
                let fail = |stage: u8, errno: Errno| {
                    libc::write(setup_writer.as_raw_fd(), (&stage as *const u8).cast(), 1);
                    io::Error::from_raw_os_error(errno as i32)
                };
                setpgid(Pid::from_raw(0), Pid::from_raw(0)).map_err(|e| fail(1, e))?;
                #[cfg(target_os = "linux")]
                {
                    use nix::sys::resource::{setrlimit, Resource};
                    for (resource, value) in [
                        (Resource::RLIMIT_AS, limits.max_memory_bytes),
                        (Resource::RLIMIT_CPU, limits.max_cpu_seconds),
                        (Resource::RLIMIT_NPROC, u64::from(limits.max_processes)),
                        (Resource::RLIMIT_FSIZE, limits.max_file_size_bytes),
                    ] {
                        setrlimit(resource, value, value).map_err(|e| fail(2, e))?;
                    }
                }
                Ok(())
            });
        }
        let spawned = command.spawn();
        drop(command); // closes the parent's copy of setup_writer on all paths
        match spawned {
            Ok(process) => Ok(Self {
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

fn output(mode: ResourceOutput) -> Stdio {
    match mode {
        ResourceOutput::Inherit => Stdio::inherit(),
        ResourceOutput::Null => Stdio::null(),
        ResourceOutput::Piped => Stdio::piped(),
    }
}
