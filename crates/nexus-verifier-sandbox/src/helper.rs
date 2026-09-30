//! The trusted verifier sandbox helper (Linux).
//!
//! Three roles, one binary:
//!
//! - the **helper** talks to the backend over its private socket (fd 0),
//!   validates the launch, creates the six namespaces in one `unshare`,
//!   waits while the backend writes the identity uid/gid maps, verifies the
//!   namespaces, forks the namespace init and relays its reports;
//! - the **init** (PID 1 of the new PID namespace, trusted) replaces fd 0
//!   with `/dev/null`, verifies it is PID 1 and that the network namespace
//!   has only a down loopback, forks the verifier child, reports whether the
//!   verifier started, reaps, reports the verifier's status and exits, so
//!   the kernel kills anything left in the namespace;
//! - the **verifier child** starts a new session, enters the working
//!   directory, sets `no_new_privs`, applies strict Landlock, installs the
//!   seccomp allow-list, closes every descriptor except the executable,
//!   re-checks the layers and `execveat`s the verified executable by
//!   descriptor. Any failure before exec is reported and nothing untrusted
//!   runs.
//!
//! The helper reads no arguments and no environment: its only input is the
//! backend's typed launch message.

use std::ffi::CString;
use std::os::fd::{AsFd, AsRawFd, BorrowedFd, OwnedFd};

use seccompiler::sock_filter;

use crate::landlock_rules;
use crate::policy::{Role, SandboxPolicy};
use crate::protocol::{
    FdRole, FromHelper, LaunchMessage, SetupStage, ToHelper, VerifierStatus, MAX_FDS,
    MAX_MESSAGE_BYTES, PROTOCOL_VERSION,
};
use crate::seccomp;
use crate::sys::{self, Forked, Waited};

/// `CLONE_NEWUSER | CLONE_NEWPID | CLONE_NEWNET | CLONE_NEWIPC |
/// CLONE_NEWUTS | CLONE_NEWCGROUP`.
pub const NAMESPACE_FLAGS: libc::c_int = libc::CLONE_NEWUSER
    | libc::CLONE_NEWPID
    | libc::CLONE_NEWNET
    | libc::CLONE_NEWIPC
    | libc::CLONE_NEWUTS
    | libc::CLONE_NEWCGROUP;

/// Exit statuses of the helper process itself.
pub mod exit {
    pub const OK: i32 = 0;
    pub const USAGE: i32 = 64;
    pub const NOT_SINGLE_THREADED: i32 = 65;
    pub const DESCRIPTORS: i32 = 66;
    pub const PARENT_GONE: i32 = 67;
    pub const CONTROL: i32 = 68;
    pub const SETUP_FAILED: i32 = 70;
    pub const INIT_LOST: i32 = 71;
}

/// Status the verifier child exits with when setup fails before exec.
const CHILD_SETUP_FAILED: i32 = 126;

/// The sandbox policy this helper enforces.
pub fn enforced_policy_hash() -> [u8; 32] {
    SandboxPolicy::V1.hash().bytes()
}

/// Entry point of the helper binary.
pub fn main() -> ! {
    let code = run();
    std::process::exit(code)
}

fn run() -> i32 {
    if std::env::args_os().len() != 1 {
        return exit::USAGE;
    }
    if !matches!(sys::thread_count(), Ok(1)) {
        return exit::NOT_SINGLE_THREADED;
    }
    // SAFETY: getppid has no preconditions.
    let parent = unsafe { libc::getppid() };
    if sys::close_all_above_stdio_except(&[]).is_err() {
        return exit::DESCRIPTORS;
    }
    if sys::set_parent_death_signal(libc::SIGKILL).is_err() {
        return exit::PARENT_GONE;
    }
    // SAFETY: getppid has no preconditions.
    if unsafe { libc::getppid() } != parent {
        return exit::PARENT_GONE;
    }
    // SAFETY: descriptors 0 to 2 exist for the life of the process (the
    // close above kept them); they are only borrowed here.
    let (control, stdout, stderr) = unsafe {
        (
            BorrowedFd::borrow_raw(0),
            BorrowedFd::borrow_raw(1),
            BorrowedFd::borrow_raw(2),
        )
    };
    if sys::socket_type(control).ok() != Some((libc::AF_UNIX, libc::SOCK_SEQPACKET)) {
        return exit::CONTROL;
    }
    if sys::fstat(stdout).is_err() || sys::fstat(stderr).is_err() {
        return exit::DESCRIPTORS;
    }
    Session { control }.run()
}

struct Session<'a> {
    control: BorrowedFd<'a>,
}

impl Session<'_> {
    fn send(&self, message: &FromHelper) -> bool {
        sys::send_message(self.control, &message.encode(), &[]).is_ok()
    }

    fn fail(&self, stage: SetupStage, errno: i32) -> i32 {
        self.send(&FromHelper::SetupFailed { stage, errno });
        exit::SETUP_FAILED
    }

    fn receive(&self) -> Option<(ToHelper, Vec<OwnedFd>)> {
        let mut buf = vec![0u8; MAX_MESSAGE_BYTES];
        let (n, fds) = sys::recv_message(self.control, &mut buf, MAX_FDS).ok()?;
        if n == 0 {
            return None;
        }
        ToHelper::decode(&buf[..n])
            .ok()
            .map(|message| (message, fds))
    }

    fn run(&self) -> i32 {
        let policy = enforced_policy_hash();
        if !self.send(&FromHelper::Hello {
            version: PROTOCOL_VERSION,
            policy_hash: policy,
        }) {
            return exit::CONTROL;
        }
        let Some((ToHelper::Launch(message), fds)) = self.receive() else {
            return self.fail(SetupStage::Protocol, 0);
        };
        if message.policy_hash != policy {
            return self.fail(SetupStage::Protocol, 0);
        }
        let launch = match Launch::new(message, fds) {
            Ok(launch) => launch,
            Err(errno) => return self.fail(SetupStage::Protocol, errno),
        };
        let program = match seccomp::program() {
            Ok(program) => program,
            Err(_) => return self.fail(SetupStage::Seccomp, -1),
        };
        let before = match NamespaceIds::read() {
            Ok(ids) => ids,
            Err(errno) => return self.fail(SetupStage::NamespaceCheck, errno),
        };
        // SAFETY: getuid and getgid have no preconditions.
        let (uid, gid) = unsafe { (libc::getuid(), libc::getgid()) };
        if let Err(error) = sys::unshare(NAMESPACE_FLAGS) {
            return self.fail(SetupStage::Unshare, error.raw_os_error().unwrap_or(-1));
        }
        if !self.send(&FromHelper::NamespacesReady) {
            return exit::CONTROL;
        }
        if !matches!(self.receive(), Some((ToHelper::Mapped, fds)) if fds.is_empty()) {
            return self.fail(SetupStage::Protocol, 0);
        }
        if let Err(errno) = check_mapping(uid, gid) {
            return self.fail(SetupStage::Mapping, errno);
        }
        if let Err(errno) = before.check_changed() {
            return self.fail(SetupStage::NamespaceCheck, errno);
        }
        let (report_read, report_write) = match sys::pipe() {
            Ok(pipe) => pipe,
            Err(error) => return self.fail(SetupStage::Fork, error.raw_os_error().unwrap_or(-1)),
        };
        match sys::fork() {
            Err(error) => self.fail(SetupStage::Fork, error.raw_os_error().unwrap_or(-1)),
            Ok(Forked::Child) => {
                drop(report_read);
                init(launch, &program, report_write, &before.pid)
            }
            Ok(Forked::Parent(init_pid)) => {
                drop(report_write);
                drop(launch);
                self.relay(report_read, init_pid)
            }
        }
    }

    /// Forward the init's reports until its terminal report, then reap it.
    fn relay(&self, reports: OwnedFd, init_pid: libc::pid_t) -> i32 {
        let mut terminal = false;
        while let Some(message) = read_report(reports.as_fd()) {
            let ends = matches!(
                message,
                FromHelper::Finished(_) | FromHelper::SetupFailed { .. }
            );
            if !self.send(&message) {
                break;
            }
            if ends {
                terminal = true;
                break;
            }
        }
        let _ = sys::wait_pid(init_pid);
        if terminal {
            exit::OK
        } else {
            exit::INIT_LOST
        }
    }
}

/// A validated launch: every descriptor has the kind its role requires.
struct Launch {
    argv: Vec<CString>,
    env: Vec<CString>,
    executable: OwnedFd,
    workdir: OwnedFd,
    rules: Vec<(Role, OwnedFd)>,
}

fn makedev(major: u32, minor: u32) -> libc::dev_t {
    libc::makedev(major, minor)
}

impl Launch {
    fn new(message: LaunchMessage, fds: Vec<OwnedFd>) -> Result<Self, i32> {
        if !message.is_well_formed() || fds.len() != message.fds.len() {
            return Err(libc::EINVAL);
        }
        let mut executable = None;
        let mut workdir = None;
        let mut rules = Vec::new();
        for (role, fd) in message.fds.iter().zip(fds) {
            let st = sys::fstat(fd.as_fd()).map_err(|e| e.raw_os_error().unwrap_or(-1))?;
            let kind = sys::file_kind(&st);
            let ok = match role {
                FdRole::Executable => kind == libc::S_IFREG,
                FdRole::WorkingDirectory => kind == libc::S_IFDIR,
                FdRole::Rule(role) => match role {
                    role if role.is_directory() => kind == libc::S_IFDIR,
                    Role::DevNull => kind == libc::S_IFCHR && st.st_rdev == makedev(1, 3),
                    Role::DevUrandom => kind == libc::S_IFCHR && st.st_rdev == makedev(1, 9),
                    _ => kind == libc::S_IFREG,
                },
            };
            if !ok {
                return Err(libc::EINVAL);
            }
            // Defense in depth against a backend mistake: no rule, working
            // directory or executable may be the filesystem root or live on a
            // kernel pseudo-filesystem (/proc, /sys, cgroups, namespaces,
            // security or tracing interfaces).
            if !matches!(role, FdRole::Rule(Role::DevNull | Role::DevUrandom)) {
                let fs_type =
                    sys::filesystem_type(fd.as_fd()).map_err(|e| e.raw_os_error().unwrap_or(-1))?;
                let root =
                    sys::is_root_directory(&st).map_err(|e| e.raw_os_error().unwrap_or(-1))?;
                if root || sys::FORBIDDEN_FILESYSTEMS.contains(&fs_type) {
                    return Err(libc::EPERM);
                }
            }
            match role {
                FdRole::Executable => executable = Some(fd),
                FdRole::WorkingDirectory => workdir = Some(fd),
                FdRole::Rule(role) => rules.push((*role, fd)),
            }
        }
        let cstrings = |entries: Vec<Vec<u8>>| {
            entries
                .into_iter()
                .map(|entry| CString::new(entry).map_err(|_| libc::EINVAL))
                .collect::<Result<Vec<_>, _>>()
        };
        Ok(Self {
            argv: cstrings(message.argv)?,
            env: cstrings(message.env)?,
            executable: executable.ok_or(libc::EINVAL)?,
            workdir: workdir.ok_or(libc::EINVAL)?,
            rules,
        })
    }
}

/// The identity map must be exactly this uid and gid, with setgroups denied.
fn check_mapping(uid: libc::uid_t, gid: libc::gid_t) -> Result<(), i32> {
    let read = |name: &str| std::fs::read_to_string(format!("/proc/self/{name}")).map_err(|_| -1);
    let one = |text: String, id: u32| {
        let fields: Vec<&str> = text.split_whitespace().collect();
        fields == [id.to_string(), id.to_string(), "1".to_string()]
    };
    if !one(read("uid_map")?, uid) || !one(read("gid_map")?, gid) {
        return Err(-2);
    }
    if read("setgroups")?.trim() != "deny" {
        return Err(-3);
    }
    // SAFETY: getuid and getgid have no preconditions.
    if unsafe { (libc::getuid(), libc::getgid()) } != (uid, gid) {
        return Err(-4);
    }
    Ok(())
}

/// Namespace identities before `unshare`.
#[derive(Clone)]
struct NamespaceIds {
    user: String,
    net: String,
    ipc: String,
    uts: String,
    cgroup: String,
    pid: String,
}

fn namespace_id(kind: &str) -> Result<String, i32> {
    sys::namespace_id(kind).map_err(|e| e.raw_os_error().unwrap_or(-1))
}

impl NamespaceIds {
    fn read() -> Result<Self, i32> {
        Ok(Self {
            user: namespace_id("user")?,
            net: namespace_id("net")?,
            ipc: namespace_id("ipc")?,
            uts: namespace_id("uts")?,
            cgroup: namespace_id("cgroup")?,
            pid: namespace_id("pid")?,
        })
    }

    /// This process's user, network, IPC, UTS and cgroup namespaces are all
    /// new, and its own PID namespace is unchanged (its children enter the
    /// new one; the init checks that it is in a different PID namespace).
    fn check_changed(&self) -> Result<(), i32> {
        let changed = namespace_id("user")? != self.user
            && namespace_id("net")? != self.net
            && namespace_id("ipc")? != self.ipc
            && namespace_id("uts")? != self.uts
            && namespace_id("cgroup")? != self.cgroup
            && namespace_id("pid")? == self.pid;
        if !changed {
            return Err(-5);
        }
        // Inside a new cgroup namespace the process sees itself at the root.
        match std::fs::read_to_string("/proc/self/cgroup") {
            Ok(text) if text == "0::/\n" => Ok(()),
            _ => Err(-6),
        }
    }
}

fn write_report(fd: BorrowedFd<'_>, message: &FromHelper) {
    let bytes = message.encode();
    let mut framed = (bytes.len() as u16).to_be_bytes().to_vec();
    framed.extend_from_slice(&bytes);
    let _ = sys::write_all(fd, &framed);
}

fn read_report(fd: BorrowedFd<'_>) -> Option<FromHelper> {
    let mut len = [0u8; 2];
    if sys::read_full(fd, &mut len).ok()? != 2 {
        return None;
    }
    let mut bytes = vec![0u8; u16::from_be_bytes(len) as usize];
    if sys::read_full(fd, &mut bytes).ok()? != bytes.len() {
        return None;
    }
    FromHelper::decode(&bytes).ok()
}

/// PID 1 of the new PID namespace. Never returns.
fn init(launch: Launch, program: &[sock_filter], reports: OwnedFd, host_pid_ns: &str) -> ! {
    let report = |message: FromHelper| write_report(reports.as_fd(), &message);
    let fail = |stage: SetupStage, errno: i32| -> ! {
        report(FromHelper::SetupFailed { stage, errno });
        // SAFETY: _exit ends this process immediately without unwinding.
        unsafe { libc::_exit(exit::SETUP_FAILED) }
    };
    if sys::set_parent_death_signal(libc::SIGKILL).is_err() {
        fail(SetupStage::InitCheck, sys::last_errno());
    }
    // SAFETY: getpid has no preconditions.
    if unsafe { libc::getpid() } != 1 {
        fail(SetupStage::InitCheck, -1);
    }
    match sys::namespace_id("pid") {
        Ok(id) if id != host_pid_ns => {}
        Ok(_) => fail(SetupStage::InitCheck, -2),
        Err(error) => fail(SetupStage::InitCheck, error.raw_os_error().unwrap_or(-1)),
    }
    // stdin becomes /dev/null; this also drops the init's copy of the
    // backend's control socket.
    let null = match sys::open_fixed(c"/dev/null", libc::O_RDWR) {
        Ok(null) => null,
        Err(error) => fail(SetupStage::Stdio, error.raw_os_error().unwrap_or(-1)),
    };
    match sys::fstat(null.as_fd()) {
        Ok(st) if sys::file_kind(&st) == libc::S_IFCHR && st.st_rdev == makedev(1, 3) => {}
        _ => fail(SetupStage::Stdio, -1),
    }
    if let Err(error) = sys::dup2(null.as_raw_fd(), 0) {
        fail(SetupStage::Stdio, error.raw_os_error().unwrap_or(-1));
    }
    drop(null);
    match sys::network_interfaces() {
        Ok((names, false)) if names == ["lo"] => {}
        Ok(_) => fail(SetupStage::NetworkCheck, -1),
        Err(error) => fail(SetupStage::NetworkCheck, error.raw_os_error().unwrap_or(-1)),
    }
    let (errors_read, errors_write) = match sys::pipe() {
        Ok(pipe) => pipe,
        Err(error) => fail(SetupStage::Fork, error.raw_os_error().unwrap_or(-1)),
    };
    let child = match sys::fork() {
        Err(error) => fail(SetupStage::Fork, error.raw_os_error().unwrap_or(-1)),
        Ok(Forked::Child) => {
            drop(errors_read);
            // The child's copy of the report pipe is closed with every other
            // inherited descriptor before exec.
            verifier_child(launch, program, errors_write)
        }
        Ok(Forked::Parent(pid)) => pid,
    };
    drop(errors_write);
    drop(launch);
    let mut setup_error = [0u8; 5];
    let setup = match sys::read_full(errors_read.as_fd(), &mut setup_error) {
        Ok(0) => None,
        Ok(5) => Some((
            SetupStage::from_code(setup_error[0]).unwrap_or(SetupStage::Exec),
            i32::from_be_bytes(setup_error[1..5].try_into().expect("4 bytes")),
        )),
        _ => Some((SetupStage::Exec, -1)),
    };
    drop(errors_read);
    if setup.is_none() {
        report(FromHelper::Running);
    }
    let status = loop {
        match sys::wait_any(false) {
            Ok(Some(Waited::Exited(pid, code))) if pid == child => {
                break VerifierStatus::Exited(code)
            }
            Ok(Some(Waited::Signalled(pid, signal))) if pid == child => {
                break VerifierStatus::Signalled(signal)
            }
            Ok(Some(Waited::NoChildren)) | Err(_) => break VerifierStatus::Signalled(0),
            _ => continue,
        }
    };
    while let Ok(Some(Waited::Exited(..) | Waited::Signalled(..))) = sys::wait_any(true) {}
    match setup {
        Some((stage, errno)) => report(FromHelper::SetupFailed { stage, errno }),
        None => report(FromHelper::Finished(status)),
    }
    // SAFETY: _exit ends PID 1 immediately; the kernel then kills every
    // process left in this PID namespace.
    unsafe { libc::_exit(exit::OK) }
}

/// The verifier child. Establishes the process-local layers, then execs the
/// verified executable by descriptor. Never returns.
fn verifier_child(launch: Launch, program: &[sock_filter], errors: OwnedFd) -> ! {
    let fail = |stage: SetupStage, errno: i32| -> ! {
        let mut record = [0u8; 5];
        record[0] = stage as u8;
        record[1..5].copy_from_slice(&errno.to_be_bytes());
        let _ = sys::write_all(errors.as_fd(), &record);
        // SAFETY: _exit ends this process immediately without unwinding.
        unsafe { libc::_exit(CHILD_SETUP_FAILED) }
    };
    let code = |error: std::io::Error| error.raw_os_error().unwrap_or(-1);
    if let Err(error) = sys::setsid() {
        fail(SetupStage::Stdio, code(error));
    }
    if let Err(error) = sys::fchdir(launch.workdir.as_fd()) {
        fail(SetupStage::WorkingDirectory, code(error));
    }
    if let Err(error) = sys::set_no_new_privs() {
        fail(SetupStage::NoNewPrivs, code(error));
    }
    let rules: Vec<(Role, BorrowedFd<'_>)> = launch
        .rules
        .iter()
        .map(|(role, fd)| (*role, fd.as_fd()))
        .collect();
    if let Err(failure) = landlock_rules::restrict_self(&rules) {
        fail(SetupStage::Landlock, failure.code());
    }
    if let Err(error) = seccomp::install(program) {
        fail(SetupStage::Seccomp, code(error));
    }
    let mut keep = [launch.executable.as_raw_fd(), errors.as_raw_fd()];
    keep.sort_unstable();
    if let Err(error) = sys::close_all_above_stdio_except(&keep) {
        fail(SetupStage::Descriptors, code(error));
    }
    match (sys::no_new_privs(), sys::seccomp_mode()) {
        (Ok(true), Ok(2)) => {}
        _ => fail(SetupStage::LayerCheck, -1),
    }
    let error = sys::execveat_fd(launch.executable.as_raw_fd(), &launch.argv, &launch.env);
    fail(SetupStage::Exec, code(error))
}
