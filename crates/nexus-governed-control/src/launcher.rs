//! The governed session-process launcher (Linux).
//!
//! The kernel's sealed spawn runs short tool processes. Long-lived session
//! processes (the agent display server, the governed browser) need two
//! things it deliberately does not offer: to die with Nexus, and, for the
//! browser's DevTools pipe, to inherit exactly two extra descriptors. This
//! launcher adds only those, under the same discipline: an identity-pinned
//! canonical program, a cleared environment with explicit variables, a
//! private working directory, its own process group, no standard input and
//! no output, and an end that kills the whole group and reaps the leader.
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
    ended: bool,
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
                Ok(())
            });
        }
        let child = command.spawn();
        drop(inherited);
        child
    }

    impl SessionProcess {
        pub(crate) fn launch(spec: SessionSpec) -> Result<Self, AuthorityError> {
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
                ended: false,
            })
        }

        /// Whether the process is still running.
        pub(crate) fn running(&mut self) -> bool {
            !self.ended && matches!(self.child.try_wait(), Ok(None))
        }

        /// Kill the whole process group and reap the leader.
        pub(crate) fn end(&mut self) {
            if self.ended {
                return;
            }
            self.ended = true;
            let group = self.child.id() as libc::pid_t;
            // SAFETY: killpg on the group this launcher created.
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
}

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
const _: fn(&SessionSpec) -> (&PathBuf, &Vec<OsString>, &Vec<(String, OsString)>, &PathBuf) =
    |spec| (&spec.program, &spec.args, &spec.env, &spec.current_dir);
