//! The backend side of the verifier sandbox helper (Linux).
//!
//! The backend spawns the trusted helper with a cleared environment, no
//! arguments, `/` as its working directory, the private control socket as
//! its stdin and fresh pipes as its stdout and stderr. It never runs code in
//! the child before exec. It then drives the typed protocol: it checks the
//! helper enforces the expected policy, sends the backend-constructed launch
//! with its descriptors, writes the identity uid/gid maps for exactly its
//! own unreaped child when the namespaces exist, and waits, with bounded
//! waits, for the verifier to start and finish.
//!
//! The helper's process id is used only as an operating-system locator for
//! the retained, unreaped child; it is never an identity or authority.

use std::io;
use std::os::fd::{AsFd, AsRawFd, BorrowedFd, OwnedFd};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use crate::helper::enforced_policy_hash;
use crate::policy::Role;
use crate::protocol::{
    FdRole, FromHelper, LaunchMessage, SetupStage, ToHelper, VerifierStatus, MAX_MESSAGE_BYTES,
    PROTOCOL_VERSION,
};
use crate::sys;

/// Bound on each setup step (handshake, namespaces, start).
pub const SETUP_STEP_TIMEOUT: Duration = Duration::from_secs(30);

/// Where the Nexus package installs the application and the helper.
const INSTALLED_BIN_DIR: &str = "/usr/bin";
const INSTALLED_HELPER: &str = "nexus-verifier-sandbox";

/// Why no installed helper is available.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HelperUnavailable {
    /// The application is not running from its installed package, or the
    /// package has no helper.
    NotInstalled,
    /// The helper or its directory is not root-owned and unwritable by
    /// others.
    NotProtected,
}

/// The trusted helper executable.
#[derive(Debug, Clone)]
pub struct HelperProgram {
    path: PathBuf,
}

impl HelperProgram {
    /// The helper at `path`: the live sandbox harness's own build of the
    /// helper. Compiled only for this crate's tests and the harness feature,
    /// never into a normal build; production code obtains the helper from
    /// the installed layout ([`Self::installed`]) alone.
    #[cfg(any(test, feature = "live-sandbox-harness"))]
    pub fn at(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    /// The helper the Nexus package installs beside the running application:
    /// `/usr/bin/nexus-verifier-sandbox`, derived only from this process's
    /// own executable (which must itself be in `/usr/bin`), and a root-owned
    /// regular executable that no one else can write, in the root-owned
    /// `/usr/bin`: the installed-package invariant. A development build is
    /// not installed and has none.
    pub fn installed() -> Result<Self, HelperUnavailable> {
        let executable = std::env::current_exe()
            .and_then(|path| path.canonicalize())
            .map_err(|_| HelperUnavailable::NotInstalled)?;
        let bin = Path::new(INSTALLED_BIN_DIR);
        if executable.parent() != Some(bin) {
            return Err(HelperUnavailable::NotInstalled);
        }
        Self::in_bin(bin, 0)
    }

    /// The helper of an extracted, not installed, Nexus package beneath
    /// `root`, held to exactly the installed layout's checks except that the
    /// owner is the user who extracted it (who cannot create root-owned
    /// files): package evidence only, never authority. The archive's own
    /// ownership is checked separately (root).
    #[cfg(any(test, feature = "live-sandbox-harness"))]
    pub fn in_extracted_package(root: &Path) -> Result<Self, HelperUnavailable> {
        // SAFETY: getuid has no preconditions.
        let owner = unsafe { libc::getuid() };
        Self::in_bin(&root.join(INSTALLED_BIN_DIR.trim_start_matches('/')), owner)
    }

    /// The package's helper in `bin`: a regular executable that `owner`
    /// owns and no one else can write, in a directory with the same
    /// protection.
    fn in_bin(bin: &Path, owner: u32) -> Result<Self, HelperUnavailable> {
        use std::os::unix::fs::MetadataExt;
        let protected = |meta: &std::fs::Metadata| meta.uid() == owner && meta.mode() & 0o022 == 0;
        let dir = std::fs::symlink_metadata(bin).map_err(|_| HelperUnavailable::NotInstalled)?;
        let path = bin.join(INSTALLED_HELPER);
        let file = std::fs::symlink_metadata(&path).map_err(|_| HelperUnavailable::NotInstalled)?;
        if !dir.is_dir()
            || !protected(&dir)
            || !file.file_type().is_file()
            || !protected(&file)
            || file.mode() & 0o100 == 0
        {
            return Err(HelperUnavailable::NotProtected);
        }
        Ok(Self { path })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

/// A backend-constructed launch.
#[derive(Debug)]
pub struct LaunchSpec {
    pub generation: u64,
    /// The verified executable (an `O_PATH` or read descriptor).
    pub executable: OwnedFd,
    pub working_directory: OwnedFd,
    pub argv: Vec<Vec<u8>>,
    pub env: Vec<Vec<u8>>,
    pub rules: Vec<(Role, OwnedFd)>,
}

#[derive(Debug)]
pub enum LaunchError {
    /// The helper could not be started.
    Spawn(io::Error),
    /// The control channel failed or timed out.
    Control(io::Error),
    /// The helper sent something unexpected.
    Protocol,
    /// The helper enforces a different protocol or policy.
    PolicyMismatch,
    /// The launch is not well formed.
    InvalidLaunch,
    /// The identity maps could not be written.
    Mapping(io::Error),
    /// The helper reported where setup stopped; nothing untrusted ran.
    SetupFailed { stage: SetupStage, errno: i32 },
    /// Every helper identity of this backend process is used: no helper is
    /// spawned again (P2-V1-R3B-I4-R1).
    IdentitiesExhausted,
}

/// How a launched execution ended, as the helper reported it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Finished(VerifierStatus),
    /// Setup failed after the launch was accepted; nothing untrusted ran.
    SetupFailed {
        stage: SetupStage,
        errno: i32,
    },
    /// The verifier ran, but its init or the helper ended without a final
    /// report: the result is unknown.
    Lost,
}

/// Read ends of the helper's stdout and stderr, which the verifier inherits.
#[derive(Debug)]
pub struct HelperOutput {
    pub stdout: OwnedFd,
    pub stderr: OwnedFd,
}

/// A running helper, owned by the backend until it is reaped.
#[derive(Debug)]
pub struct Helper {
    child: Child,
    control: OwnedFd,
    /// This value's identity within the backend process, nonzero and never
    /// reused (P2-V1-R3B-I4, -R1): what a pending scope operation is bound
    /// to. Unlike the process id, it cannot come to name another child once
    /// this one is reaped. It is allocated before the child is spawned, and
    /// none is spawned once they are exhausted. It is no authority by
    /// itself.
    serial: u64,
}

/// The next helper's [`Helper::serial`].
static NEXT_SERIAL: AtomicU64 = AtomicU64::new(1);

/// A helper identity from `next`: nonzero, increasing, never reused. Once
/// the next one would not fit, none is ever issued again: the counter never
/// wraps to an identity already issued.
pub(crate) fn allocate_serial(next: &AtomicU64) -> Option<u64> {
    next.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |serial| {
        serial.checked_add(1).filter(|_| serial != 0)
    })
    .ok()
}

#[cfg(test)]
thread_local! {
    /// Helper children this thread has spawned (unit tests only).
    pub(crate) static SPAWNED: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

impl Helper {
    /// Spawn the helper. Its environment is empty, it has no arguments, its
    /// working directory is `/`, stdin is the control socket and stdout and
    /// stderr are fresh pipes.
    pub fn spawn(program: &HelperProgram) -> Result<(Helper, HelperOutput), LaunchError> {
        Self::spawn_with(program, &NEXT_SERIAL)
    }

    /// [`Self::spawn`], with its identity from `serials`.
    pub(crate) fn spawn_with(
        program: &HelperProgram,
        serials: &AtomicU64,
    ) -> Result<(Helper, HelperOutput), LaunchError> {
        // The identity first: no child exists without one, and none is
        // spawned once they are exhausted.
        let serial = allocate_serial(serials).ok_or(LaunchError::IdentitiesExhausted)?;
        let (control, helper_end) = sys::seqpacket_pair().map_err(LaunchError::Spawn)?;
        let (stdout_read, stdout_write) = sys::pipe().map_err(LaunchError::Spawn)?;
        let (stderr_read, stderr_write) = sys::pipe().map_err(LaunchError::Spawn)?;
        let child = Command::new(program.path())
            .env_clear()
            .current_dir("/")
            .stdin(Stdio::from(helper_end))
            .stdout(Stdio::from(stdout_write))
            .stderr(Stdio::from(stderr_write))
            .spawn()
            .map_err(LaunchError::Spawn)?;
        #[cfg(test)]
        SPAWNED.with(|spawned| spawned.set(spawned.get() + 1));
        // The Command and its copies of the child's ends are gone here, so
        // the output pipes reach end of file when the verifier tree exits.
        let mut helper = Helper {
            child,
            control,
            serial,
        };
        if let Err(error) = helper.set_receive_timeout(Some(SETUP_STEP_TIMEOUT)) {
            // The helper is never left running or unreaped.
            let _ = helper.child.kill();
            let _ = helper.child.wait();
            return Err(LaunchError::Control(error));
        }
        Ok((
            helper,
            HelperOutput {
                stdout: stdout_read,
                stderr: stderr_read,
            },
        ))
    }

    /// The helper's process id: an operating-system locator for this owned,
    /// unreaped child only.
    pub fn pid(&self) -> u32 {
        self.child.id()
    }

    /// This retained helper's identity within the backend process (see the
    /// field): binds a pending scope operation to exactly this child.
    pub(crate) fn serial(&self) -> u64 {
        self.serial
    }

    fn set_receive_timeout(&self, timeout: Option<Duration>) -> io::Result<()> {
        let timeout = timeout.unwrap_or(Duration::ZERO);
        let value = libc::timeval {
            tv_sec: timeout.as_secs() as libc::time_t,
            tv_usec: timeout.subsec_micros() as libc::suseconds_t,
        };
        // SAFETY: SO_RCVTIMEO takes one timeval.
        let rc = unsafe {
            libc::setsockopt(
                self.control.as_raw_fd(),
                libc::SOL_SOCKET,
                libc::SO_RCVTIMEO,
                (&value as *const libc::timeval).cast(),
                std::mem::size_of::<libc::timeval>() as libc::socklen_t,
            )
        };
        if rc < 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }

    fn receive(&self) -> Result<Option<FromHelper>, LaunchError> {
        let mut buf = vec![0u8; MAX_MESSAGE_BYTES];
        let (n, fds) =
            sys::recv_message(self.control.as_fd(), &mut buf, 0).map_err(LaunchError::Control)?;
        if !fds.is_empty() {
            return Err(LaunchError::Protocol);
        }
        if n == 0 {
            return Ok(None);
        }
        FromHelper::decode(&buf[..n])
            .map(Some)
            .map_err(|_| LaunchError::Protocol)
    }

    fn send(&self, message: &ToHelper, fds: &[BorrowedFd<'_>]) -> Result<(), LaunchError> {
        sys::send_message(self.control.as_fd(), &message.encode(), fds)
            .map_err(LaunchError::Control)
    }

    /// Receive the helper's greeting and confirm it speaks this protocol and
    /// enforces exactly the expected sandbox policy.
    pub fn handshake(&self) -> Result<(), LaunchError> {
        match self.receive()? {
            Some(FromHelper::Hello {
                version,
                policy_hash,
            }) if version == PROTOCOL_VERSION && policy_hash == enforced_policy_hash() => Ok(()),
            Some(FromHelper::Hello { .. }) => Err(LaunchError::PolicyMismatch),
            _ => Err(LaunchError::Protocol),
        }
    }

    /// Send the launch and see it through to a running verifier: write the
    /// identity maps when the namespaces exist, then wait for the verifier
    /// to start.
    pub fn launch(&self, spec: LaunchSpec) -> Result<(), LaunchError> {
        let mut roles = vec![FdRole::Executable, FdRole::WorkingDirectory];
        let mut fds: Vec<BorrowedFd<'_>> =
            vec![spec.executable.as_fd(), spec.working_directory.as_fd()];
        for (role, fd) in &spec.rules {
            roles.push(FdRole::Rule(*role));
            fds.push(fd.as_fd());
        }
        let message = LaunchMessage {
            generation: spec.generation,
            policy_hash: enforced_policy_hash(),
            argv: spec.argv.clone(),
            env: spec.env.clone(),
            fds: roles,
        };
        if !message.is_well_formed() {
            return Err(LaunchError::InvalidLaunch);
        }
        self.send(&ToHelper::Launch(message), &fds)?;
        drop(fds);
        drop(spec);
        match self.receive()? {
            Some(FromHelper::NamespacesReady) => {}
            Some(FromHelper::SetupFailed { stage, errno }) => {
                return Err(LaunchError::SetupFailed { stage, errno })
            }
            _ => return Err(LaunchError::Protocol),
        }
        self.write_identity_maps().map_err(LaunchError::Mapping)?;
        self.send(&ToHelper::Mapped, &[])?;
        match self.receive()? {
            Some(FromHelper::Running) => Ok(()),
            Some(FromHelper::SetupFailed { stage, errno }) => {
                Err(LaunchError::SetupFailed { stage, errno })
            }
            _ => Err(LaunchError::Protocol),
        }
    }

    /// Map exactly this process's uid and gid onto themselves in the
    /// helper's new user namespace, with setgroups denied.
    fn write_identity_maps(&self) -> io::Result<()> {
        // SAFETY: getuid and getgid have no preconditions.
        let (uid, gid) = unsafe { (libc::getuid(), libc::getgid()) };
        let dir = PathBuf::from(format!("/proc/{}", self.child.id()));
        std::fs::write(dir.join("setgroups"), b"deny")?;
        std::fs::write(dir.join("uid_map"), format!("{uid} {uid} 1"))?;
        std::fs::write(dir.join("gid_map"), format!("{gid} {gid} 1"))?;
        Ok(())
    }

    /// Wait up to `timeout` for the verifier's final report. `Ok(None)`
    /// means the timeout passed first (the caller ends the execution). After
    /// its final report the helper holds its scope until it is killed or
    /// the control channel is written to or closed.
    pub fn wait_report(&self, timeout: Duration) -> Result<Option<Outcome>, LaunchError> {
        self.set_receive_timeout(Some(timeout.max(Duration::from_millis(1))))
            .map_err(LaunchError::Control)?;
        match self.receive() {
            Ok(Some(FromHelper::Finished(status))) => Ok(Some(Outcome::Finished(status))),
            Ok(Some(FromHelper::SetupFailed { stage, errno })) => {
                Ok(Some(Outcome::SetupFailed { stage, errno }))
            }
            Ok(Some(FromHelper::InitLost)) | Ok(None) => Ok(Some(Outcome::Lost)),
            Ok(Some(_)) => Err(LaunchError::Protocol),
            Err(LaunchError::Control(error))
                if matches!(
                    error.raw_os_error(),
                    Some(libc::EAGAIN) | Some(libc::ETIMEDOUT)
                ) =>
            {
                Ok(None)
            }
            Err(error) => Err(error),
        }
    }

    /// Kill the helper. Its death signal chain ends the namespace init, and
    /// with it every process in the PID namespace.
    pub fn kill(&mut self) -> io::Result<()> {
        self.child.kill()
    }

    /// Release the helper and reap it, returning its exit status. Closing
    /// the control channel ends a helper's hold on its scope after its
    /// final report.
    pub fn reap(self) -> io::Result<std::process::ExitStatus> {
        let Self {
            mut child, control, ..
        } = self;
        drop(control);
        child.wait()
    }

    /// Reap the helper if it has exited, without waiting.
    pub fn try_reap(&mut self) -> io::Result<Option<std::process::ExitStatus>> {
        self.child.try_wait()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn p2h_only_an_installed_application_has_an_installed_helper() {
        // A test executable is never the installed application.
        assert_eq!(
            HelperProgram::installed().err(),
            Some(HelperUnavailable::NotInstalled)
        );
    }
}
