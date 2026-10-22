//! The governed session-process launcher (Linux).
//!
//! The kernel's sealed spawn runs short tool processes. Long-lived session
//! processes (the agent display server, the governed browser) need two
//! things it deliberately does not offer: to die with Nexus, and, for the
//! browser's DevTools pipe, to inherit exactly two extra descriptors. This
//! launcher adds only those, under the same discipline: an identity-pinned
//! canonical program, a cleared environment with explicit variables, a
//! private working directory, its own process group, no standard input and
//! no output, and an end that kills the whole group and reaps the leader
//! (after a short `SIGTERM` grace only for a process that must clean up
//! after itself and has no external effect, like the X server).
//!
//! Launches happen on one thread that lives as long as the process,
//! because the parent-death signal follows the thread that forked.

#![allow(unsafe_code)]

#[cfg(not(target_os = "linux"))]
use crate::authority::AuthorityError;
use std::ffi::OsString;
use std::path::PathBuf;

/// What to launch.
pub(crate) struct SessionSpec {
    /// Canonical and pinned by the caller.
    pub program: PathBuf,
    pub args: Vec<OsString>,
    /// The whole environment; nothing is inherited.
    pub env: Vec<(String, OsString)>,
    pub current_dir: PathBuf,
    /// How long the leader may take to exit after `SIGTERM` before the
    /// group is killed; `None` kills at once. An X server removes its lock
    /// and socket on `SIGTERM`; a browser gets no grace, since nothing it
    /// does may run after a stop.
    pub stop_grace: Option<std::time::Duration>,
    /// Descriptors the child receives: (descriptor in Nexus, number in the
    /// child). Every other descriptor is closed by `exec`.
    #[cfg(target_os = "linux")]
    pub inherit: Vec<(std::os::fd::OwnedFd, i32)>,
}

/// A launched session process; it ends (group killed, leader reaped) when
/// dropped.
pub(crate) struct SessionProcess {
    #[cfg(target_os = "linux")]
    child: std::process::Child,
    #[cfg(target_os = "linux")]
    stop_grace: Option<std::time::Duration>,
    ended: bool,
}

/// This process's effective user id: whom the files it creates belong to.
#[cfg(target_os = "linux")]
pub(crate) fn effective_uid() -> u32 {
    // SAFETY: geteuid has no preconditions and cannot fail.
    unsafe { libc::geteuid() }
}

#[cfg(target_os = "linux")]
mod linux {
    use super::{SessionProcess, SessionSpec};
    use crate::authority::AuthorityError;
    use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
    use std::os::unix::process::CommandExt;
    use std::process::{Child, Command, Stdio};
    use std::sync::mpsc;
    use std::sync::{Mutex, OnceLock};
    use std::time::{Duration, Instant};

    type Request = (SessionSpec, mpsc::Sender<std::io::Result<Child>>);

    /// The launcher thread: it never exits, so a child's parent-death
    /// signal fires only when Nexus itself ends.
    fn launcher() -> &'static Mutex<mpsc::Sender<Request>> {
        static LAUNCHER: OnceLock<Mutex<mpsc::Sender<Request>>> = OnceLock::new();
        LAUNCHER.get_or_init(|| {
            let (sender, receiver) = mpsc::channel::<Request>();
            std::thread::Builder::new()
                .name("nexus-p3-launcher".into())
                .spawn(move || {
                    for (spec, reply) in receiver {
                        let _ = reply.send(spawn(spec));
                    }
                })
                .expect("the launcher thread starts");
            Mutex::new(sender)
        })
    }

    /// Move `fd` to a number of at least 10, close-on-exec, so it cannot
    /// collide with the numbers the child receives.
    fn high(fd: OwnedFd) -> std::io::Result<OwnedFd> {
        // SAFETY: F_DUPFD_CLOEXEC on a valid descriptor returns a new one
        // we take ownership of.
        let moved = unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_DUPFD_CLOEXEC, 10) };
        if moved < 0 {
            return Err(std::io::Error::last_os_error());
        }
        // SAFETY: `moved` is a fresh descriptor owned by nobody else.
        Ok(unsafe { OwnedFd::from_raw_fd(moved) })
    }

    fn spawn(spec: SessionSpec) -> std::io::Result<Child> {
        let mut inherited = Vec::with_capacity(spec.inherit.len());
        for (fd, target) in spec.inherit {
            inherited.push((high(fd)?, target));
        }
        let pairs: Vec<(i32, i32)> = inherited
            .iter()
            .map(|(fd, target)| (fd.as_raw_fd(), *target))
            .collect();
        // The inherited descriptors are exactly 3, 4, ...; every descriptor
        // above them closes at exec, whether or not it was opened
        // close-on-exec.
        if pairs
            .iter()
            .enumerate()
            .any(|(index, &(_, target))| target != 3 + index as i32)
        {
            return Err(std::io::Error::from(std::io::ErrorKind::InvalidInput));
        }
        let first_free = 3 + pairs.len() as libc::c_long;
        let parent = std::process::id() as libc::pid_t;
        let mut command = Command::new(&spec.program);
        command
            .args(&spec.args)
            .env_clear()
            .envs(spec.env.iter().map(|(k, v)| (k.as_str(), v.as_os_str())))
            .current_dir(&spec.current_dir)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        // SAFETY: the closure runs in the child between fork and exec and
        // only calls async-signal-safe functions on data prepared before
        // the fork; it allocates nothing.
        unsafe {
            command.pre_exec(move || {
                if libc::setpgid(0, 0) != 0 {
                    return Err(std::io::Error::last_os_error());
                }
                if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL) != 0 {
                    return Err(std::io::Error::last_os_error());
                }
                // Nexus ended before the signal was armed.
                if libc::getppid() != parent {
                    return Err(std::io::Error::from_raw_os_error(libc::ESRCH));
                }
                for &(from, to) in &pairs {
                    // dup2 leaves the new descriptor open across exec.
                    if libc::dup2(from, to) < 0 {
                        return Err(std::io::Error::last_os_error());
                    }
                }
                if libc::syscall(
                    libc::SYS_close_range,
                    first_free,
                    libc::c_long::from(libc::c_uint::MAX),
                    libc::c_long::from(libc::CLOSE_RANGE_CLOEXEC),
                ) != 0
                {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
        let child = command.spawn();
        drop(inherited);
        child
    }

    impl SessionProcess {
        pub(crate) fn launch(spec: SessionSpec) -> Result<Self, AuthorityError> {
            let stop_grace = spec.stop_grace;
            let (reply, answer) = mpsc::channel();
            launcher()
                .lock()
                .expect("launcher")
                .send((spec, reply))
                .map_err(|_| AuthorityError::Unavailable("the launcher is not running"))?;
            let child = answer
                .recv()
                .map_err(|_| AuthorityError::Unavailable("the launcher is not running"))?
                .map_err(|_| AuthorityError::Unavailable("the session process could not start"))?;
            Ok(Self {
                child,
                stop_grace,
                ended: false,
            })
        }

        /// Whether the process is still running, observed without reaping
        /// it, so the group id stays reserved until `end`.
        pub(crate) fn running(&mut self) -> bool {
            !self.ended && !exited(self.child.id() as libc::pid_t)
        }

        /// End the whole process group and reap the leader. With a grace,
        /// the group is first asked to stop and the leader given that long
        /// to exit; then whatever remains is killed. The group is signalled
        /// only while the leader is unreaped, so its id cannot have been
        /// reused.
        pub(crate) fn end(&mut self) {
            if self.ended {
                return;
            }
            self.ended = true;
            let group = self.child.id() as libc::pid_t;
            if let Some(grace) = self.stop_grace {
                // SAFETY: killpg on the group this launcher created; its
                // leader is not reaped.
                unsafe {
                    libc::killpg(group, libc::SIGTERM);
                }
                let until = Instant::now() + grace;
                while Instant::now() < until && !exited(group) {
                    std::thread::sleep(Duration::from_millis(10));
                }
            }
            // SAFETY: killpg on the group this launcher created; `exited`
            // observes the leader without reaping it.
            unsafe {
                libc::killpg(group, libc::SIGKILL);
            }
            let deadline = Instant::now() + Duration::from_secs(5);
            while Instant::now() < deadline {
                if !matches!(self.child.try_wait(), Ok(None)) {
                    return;
                }
                std::thread::sleep(Duration::from_millis(10));
            }
        }
    }

    /// Whether the child `pid` has exited, observed without reaping it (it
    /// stays waitable, so its id is not reused).
    fn exited(pid: libc::pid_t) -> bool {
        // SAFETY: an all-zero siginfo_t is valid; waitid with WNOWAIT only
        // reports the child's state, and with WNOHANG leaves si_pid zero
        // when it has not exited.
        unsafe {
            let mut info: libc::siginfo_t = std::mem::zeroed();
            libc::waitid(
                libc::P_PID,
                pid as libc::id_t,
                &mut info,
                libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,
            ) == 0
                && info.si_pid() != 0
        }
    }
}

#[cfg(all(test, target_os = "linux"))]
mod tests;

#[cfg(not(target_os = "linux"))]
impl SessionProcess {
    pub(crate) fn launch(_spec: SessionSpec) -> Result<Self, AuthorityError> {
        Err(AuthorityError::Unavailable(
            "governed real-world control is available on Linux only",
        ))
    }

    pub(crate) fn running(&mut self) -> bool {
        false
    }

    pub(crate) fn end(&mut self) {
        self.ended = true;
    }
}

impl Drop for SessionProcess {
    fn drop(&mut self) {
        self.end();
    }
}

/// Unused fields on other platforms.
#[cfg(not(target_os = "linux"))]
type Fields<'a> = (
    &'a PathBuf,
    &'a Vec<OsString>,
    &'a Vec<(String, OsString)>,
    &'a PathBuf,
    &'a Option<std::time::Duration>,
);
#[cfg(not(target_os = "linux"))]
const _: fn(&SessionSpec) -> Fields<'_> = |spec| {
    (
        &spec.program,
        &spec.args,
        &spec.env,
        &spec.current_dir,
        &spec.stop_grace,
    )
};
