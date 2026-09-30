//! The few Linux system interfaces the helper and launcher use, each
//! checked. Every `unsafe` block has a stated reason; nothing here decides
//! policy.

use std::ffi::CString;
use std::io;
use std::os::fd::{AsRawFd, BorrowedFd, FromRawFd, OwnedFd, RawFd};

pub(crate) fn last_errno() -> i32 {
    io::Error::last_os_error().raw_os_error().unwrap_or(-1)
}

fn check(rc: libc::c_int) -> io::Result<libc::c_int> {
    if rc < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(rc)
    }
}

fn check_long(rc: libc::c_long) -> io::Result<libc::c_long> {
    if rc < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(rc)
    }
}

/// Close every descriptor in `first..=last`.
pub(crate) fn close_range(first: u32, last: u32) -> io::Result<()> {
    // SAFETY: close_range only closes descriptors; callers own every
    // descriptor in the range they pass (it never includes one they still
    // use).
    check_long(unsafe { libc::syscall(libc::SYS_close_range, first, last, 0u32) }).map(drop)
}

/// Close every descriptor from 3 upward except `keep` (ascending, each ≥ 3).
pub(crate) fn close_all_above_stdio_except(keep: &[RawFd]) -> io::Result<()> {
    let mut next: u32 = 3;
    for fd in keep {
        let fd = u32::try_from(*fd).map_err(|_| io::Error::from_raw_os_error(libc::EBADF))?;
        if fd < next {
            return Err(io::Error::from_raw_os_error(libc::EINVAL));
        }
        if fd > next {
            close_range(next, fd - 1)?;
        }
        next = fd + 1;
    }
    close_range(next, u32::MAX)
}

fn prctl(option: libc::c_int, arg: libc::c_ulong) -> io::Result<libc::c_int> {
    // SAFETY: the prctl options used here take plain integer arguments.
    check(unsafe { libc::prctl(option, arg, 0, 0, 0) })
}

pub(crate) fn set_parent_death_signal(signal: libc::c_int) -> io::Result<()> {
    prctl(libc::PR_SET_PDEATHSIG, signal as libc::c_ulong).map(drop)
}

pub(crate) fn set_no_new_privs() -> io::Result<()> {
    prctl(libc::PR_SET_NO_NEW_PRIVS, 1).map(drop)
}

pub(crate) fn no_new_privs() -> io::Result<bool> {
    prctl(libc::PR_GET_NO_NEW_PRIVS, 0).map(|value| value == 1)
}

/// `PR_GET_SECCOMP`: 2 means a filter is installed.
pub(crate) fn seccomp_mode() -> io::Result<libc::c_int> {
    prctl(libc::PR_GET_SECCOMP, 0)
}

pub(crate) fn unshare(flags: libc::c_int) -> io::Result<()> {
    // SAFETY: unshare takes a flag word and changes only this process.
    check(unsafe { libc::unshare(flags) }).map(drop)
}

pub(crate) enum Forked {
    Child,
    Parent(libc::pid_t),
}

/// Fork. Only called from a single-threaded process (the helper and the
/// namespace init are single-threaded by construction), so the child may
/// allocate.
pub(crate) fn fork() -> io::Result<Forked> {
    // SAFETY: callers are single-threaded, so no lock can be held by
    // another thread across the fork.
    let pid = check(unsafe { libc::fork() })?;
    Ok(if pid == 0 {
        Forked::Child
    } else {
        Forked::Parent(pid)
    })
}

pub(crate) enum Waited {
    Exited(libc::pid_t, i32),
    Signalled(libc::pid_t, i32),
    NoChildren,
}

/// Wait for any child (blocking unless `nohang`); `None` with `nohang` means
/// none has exited yet.
pub(crate) fn wait_any(nohang: bool) -> io::Result<Option<Waited>> {
    let mut status: libc::c_int = 0;
    let flags = if nohang { libc::WNOHANG } else { 0 };
    loop {
        // SAFETY: waitpid writes only the status integer.
        let pid = unsafe { libc::waitpid(-1, &mut status, flags) };
        if pid < 0 {
            let error = io::Error::last_os_error();
            match error.raw_os_error() {
                Some(libc::EINTR) => continue,
                Some(libc::ECHILD) => return Ok(Some(Waited::NoChildren)),
                _ => return Err(error),
            }
        }
        if pid == 0 {
            return Ok(None);
        }
        if libc::WIFEXITED(status) {
            return Ok(Some(Waited::Exited(pid, libc::WEXITSTATUS(status))));
        }
        if libc::WIFSIGNALED(status) {
            return Ok(Some(Waited::Signalled(pid, libc::WTERMSIG(status))));
        }
    }
}

/// Wait for one specific child to end.
pub(crate) fn wait_pid(pid: libc::pid_t) -> io::Result<Waited> {
    let mut status: libc::c_int = 0;
    loop {
        // SAFETY: waitpid writes only the status integer.
        let rc = unsafe { libc::waitpid(pid, &mut status, 0) };
        if rc < 0 {
            let error = io::Error::last_os_error();
            if error.raw_os_error() == Some(libc::EINTR) {
                continue;
            }
            return Err(error);
        }
        if libc::WIFEXITED(status) {
            return Ok(Waited::Exited(rc, libc::WEXITSTATUS(status)));
        }
        if libc::WIFSIGNALED(status) {
            return Ok(Waited::Signalled(rc, libc::WTERMSIG(status)));
        }
    }
}

/// `execveat(fd, "", argv, envp, AT_EMPTY_PATH)`. Returns only on failure.
pub(crate) fn execveat_fd(fd: RawFd, argv: &[CString], env: &[CString]) -> io::Error {
    let mut argv_ptrs: Vec<*const libc::c_char> = argv.iter().map(|a| a.as_ptr()).collect();
    argv_ptrs.push(std::ptr::null());
    let mut env_ptrs: Vec<*const libc::c_char> = env.iter().map(|e| e.as_ptr()).collect();
    env_ptrs.push(std::ptr::null());
    let empty = c"";
    // SAFETY: every pointer array is NUL-terminated and points into CStrings
    // that outlive the call; the kernel copies them.
    unsafe {
        libc::syscall(
            libc::SYS_execveat,
            fd,
            empty.as_ptr(),
            argv_ptrs.as_ptr(),
            env_ptrs.as_ptr(),
            libc::AT_EMPTY_PATH,
        );
    }
    io::Error::last_os_error()
}

pub(crate) fn dup2(old: RawFd, new: RawFd) -> io::Result<()> {
    // SAFETY: dup2 operates on descriptor numbers only.
    check(unsafe { libc::dup2(old, new) }).map(drop)
}

pub(crate) fn setsid() -> io::Result<()> {
    // SAFETY: setsid changes only this process's session.
    check(unsafe { libc::setsid() }).map(drop)
}

pub(crate) fn fchdir(fd: BorrowedFd<'_>) -> io::Result<()> {
    // SAFETY: fchdir takes a descriptor the caller keeps open.
    check(unsafe { libc::fchdir(fd.as_raw_fd()) }).map(drop)
}

/// A close-on-exec pipe: (read end, write end).
pub(crate) fn pipe() -> io::Result<(OwnedFd, OwnedFd)> {
    let mut fds = [0 as libc::c_int; 2];
    // SAFETY: pipe2 writes two descriptors into the array.
    check(unsafe { libc::pipe2(fds.as_mut_ptr(), libc::O_CLOEXEC) })?;
    // SAFETY: pipe2 succeeded, so both descriptors are new and owned here.
    Ok(unsafe { (OwnedFd::from_raw_fd(fds[0]), OwnedFd::from_raw_fd(fds[1])) })
}

/// A close-on-exec `AF_UNIX` `SOCK_SEQPACKET` socketpair.
pub fn seqpacket_pair() -> io::Result<(OwnedFd, OwnedFd)> {
    let mut fds = [0 as libc::c_int; 2];
    // SAFETY: socketpair writes two descriptors into the array.
    check(unsafe {
        libc::socketpair(
            libc::AF_UNIX,
            libc::SOCK_SEQPACKET | libc::SOCK_CLOEXEC,
            0,
            fds.as_mut_ptr(),
        )
    })?;
    // SAFETY: socketpair succeeded, so both descriptors are new and owned.
    Ok(unsafe { (OwnedFd::from_raw_fd(fds[0]), OwnedFd::from_raw_fd(fds[1])) })
}

pub(crate) fn socket_type(fd: BorrowedFd<'_>) -> io::Result<(libc::c_int, libc::c_int)> {
    let get = |option: libc::c_int| -> io::Result<libc::c_int> {
        let mut value: libc::c_int = 0;
        let mut len = std::mem::size_of::<libc::c_int>() as libc::socklen_t;
        // SAFETY: getsockopt writes one int and its length.
        check(unsafe {
            libc::getsockopt(
                fd.as_raw_fd(),
                libc::SOL_SOCKET,
                option,
                (&mut value as *mut libc::c_int).cast(),
                &mut len,
            )
        })?;
        Ok(value)
    };
    Ok((get(libc::SO_DOMAIN)?, get(libc::SO_TYPE)?))
}

/// Send one whole message, with descriptors as `SCM_RIGHTS`.
pub fn send_message(fd: BorrowedFd<'_>, bytes: &[u8], fds: &[BorrowedFd<'_>]) -> io::Result<()> {
    let raw: Vec<RawFd> = fds.iter().map(|f| f.as_raw_fd()).collect();
    let space = if raw.is_empty() {
        0
    } else {
        // SAFETY: CMSG_SPACE is a pure size computation.
        unsafe { libc::CMSG_SPACE(std::mem::size_of_val(raw.as_slice()) as u32) as usize }
    };
    let mut control = vec![0u8; space];
    let mut iov = libc::iovec {
        iov_base: bytes.as_ptr() as *mut libc::c_void,
        iov_len: bytes.len(),
    };
    // SAFETY: msghdr is plain data; zeroed is a valid initial value.
    let mut msg: libc::msghdr = unsafe { std::mem::zeroed() };
    msg.msg_iov = &mut iov;
    msg.msg_iovlen = 1;
    if !raw.is_empty() {
        msg.msg_control = control.as_mut_ptr().cast();
        msg.msg_controllen = space;
        // SAFETY: the control buffer is CMSG_SPACE bytes for this many
        // descriptors, so the first header and its data fit.
        unsafe {
            let header = libc::CMSG_FIRSTHDR(&msg);
            (*header).cmsg_level = libc::SOL_SOCKET;
            (*header).cmsg_type = libc::SCM_RIGHTS;
            (*header).cmsg_len = libc::CMSG_LEN(std::mem::size_of_val(raw.as_slice()) as u32) as _;
            std::ptr::copy_nonoverlapping(
                raw.as_ptr().cast::<u8>(),
                libc::CMSG_DATA(header),
                std::mem::size_of_val(raw.as_slice()),
            );
        }
    }
    loop {
        // SAFETY: msg points at live buffers for the duration of the call.
        let sent = unsafe { libc::sendmsg(fd.as_raw_fd(), &msg, libc::MSG_NOSIGNAL) };
        if sent < 0 {
            let error = io::Error::last_os_error();
            if error.raw_os_error() == Some(libc::EINTR) {
                continue;
            }
            return Err(error);
        }
        if sent as usize != bytes.len() {
            return Err(io::Error::from_raw_os_error(libc::EMSGSIZE));
        }
        return Ok(());
    }
}

/// Receive one whole message of at most `buf.len()` bytes and at most
/// `max_fds` descriptors (received close-on-exec). A truncated message or
/// descriptor list is an error; `Ok((0, _))` means the peer closed.
pub fn recv_message(
    fd: BorrowedFd<'_>,
    buf: &mut [u8],
    max_fds: usize,
) -> io::Result<(usize, Vec<OwnedFd>)> {
    // SAFETY: CMSG_SPACE is a pure size computation.
    let space =
        unsafe { libc::CMSG_SPACE((max_fds * std::mem::size_of::<RawFd>()) as u32) as usize };
    let mut control = vec![0u8; space.max(1)];
    let mut iov = libc::iovec {
        iov_base: buf.as_mut_ptr().cast(),
        iov_len: buf.len(),
    };
    // SAFETY: msghdr is plain data; zeroed is a valid initial value.
    let mut msg: libc::msghdr = unsafe { std::mem::zeroed() };
    msg.msg_iov = &mut iov;
    msg.msg_iovlen = 1;
    msg.msg_control = control.as_mut_ptr().cast();
    msg.msg_controllen = control.len();
    let received = loop {
        // SAFETY: msg points at live buffers for the duration of the call.
        let n = unsafe { libc::recvmsg(fd.as_raw_fd(), &mut msg, libc::MSG_CMSG_CLOEXEC) };
        if n < 0 {
            let error = io::Error::last_os_error();
            if error.raw_os_error() == Some(libc::EINTR) {
                continue;
            }
            return Err(error);
        }
        break n as usize;
    };
    let mut fds = Vec::new();
    // SAFETY: the kernel filled msg_control with well-formed headers within
    // msg_controllen; each SCM_RIGHTS payload is an array of descriptors now
    // owned by this process.
    unsafe {
        let mut header = libc::CMSG_FIRSTHDR(&msg);
        while !header.is_null() {
            if (*header).cmsg_level == libc::SOL_SOCKET && (*header).cmsg_type == libc::SCM_RIGHTS {
                let bytes = (*header).cmsg_len as usize - libc::CMSG_LEN(0) as usize;
                let count = bytes / std::mem::size_of::<RawFd>();
                let data = libc::CMSG_DATA(header).cast::<RawFd>();
                for i in 0..count {
                    fds.push(OwnedFd::from_raw_fd(std::ptr::read_unaligned(data.add(i))));
                }
            }
            header = libc::CMSG_NXTHDR(&msg, header);
        }
    }
    if msg.msg_flags & (libc::MSG_TRUNC | libc::MSG_CTRUNC) != 0 || fds.len() > max_fds {
        return Err(io::Error::from_raw_os_error(libc::EMSGSIZE));
    }
    Ok((received, fds))
}

pub(crate) fn fstat(fd: BorrowedFd<'_>) -> io::Result<libc::stat> {
    // SAFETY: stat is plain data; fstat fills it.
    let mut st: libc::stat = unsafe { std::mem::zeroed() };
    // SAFETY: fstat writes one stat structure.
    check(unsafe { libc::fstat(fd.as_raw_fd(), &mut st) })?;
    Ok(st)
}

pub(crate) fn file_kind(st: &libc::stat) -> libc::mode_t {
    st.st_mode & libc::S_IFMT
}

/// The filesystem type (`statfs.f_type`) of the object `fd` refers to.
pub(crate) fn filesystem_type(fd: BorrowedFd<'_>) -> io::Result<i64> {
    // SAFETY: statfs is plain data; fstatfs fills it.
    let mut fs: libc::statfs = unsafe { std::mem::zeroed() };
    // SAFETY: fstatfs writes one statfs structure.
    check(unsafe { libc::fstatfs(fd.as_raw_fd(), &mut fs) })?;
    Ok(fs.f_type)
}

/// Kernel pseudo-filesystems no rule may ever grant: process and kernel
/// state, control groups, namespaces, security and tracing interfaces.
pub(crate) const FORBIDDEN_FILESYSTEMS: [i64; 12] = [
    0x9fa0,      // proc
    0x6265_6572, // sysfs
    0x6367_7270, // cgroup2
    0x0027_e0eb, // cgroup
    0x1cd1,      // devpts
    0x6e73_6673, // nsfs
    0x7363_6673, // securityfs
    0x6462_6720, // debugfs
    0x7472_6163, // tracefs
    0xcafe_4a11, // bpf
    0x6265_6570, // configfs
    0xde5e_81e4, // efivarfs
];

/// Whether the directory `dir` refers to lists nothing but `.` and `..`. A
/// removed directory lists nothing (or reports `ENOENT`), which counts as
/// empty. The listing uses a new open file description, so the offset of
/// the retained descriptor is never moved.
pub(crate) fn directory_is_empty(dir: BorrowedFd<'_>) -> io::Result<bool> {
    // SAFETY: "." is a NUL-terminated constant; openat returns a new
    // descriptor owned here on success.
    let raw = match check(unsafe {
        libc::openat(
            dir.as_raw_fd(),
            c".".as_ptr(),
            libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC,
        )
    }) {
        Ok(raw) => raw,
        Err(error) if error.raw_os_error() == Some(libc::ENOENT) => return Ok(true),
        Err(error) => return Err(error),
    };
    // SAFETY: openat succeeded, so raw is new and owned here.
    let listing = unsafe { OwnedFd::from_raw_fd(raw) };
    let mut buf = [0u8; 4096];
    loop {
        // SAFETY: getdents64 writes at most buf.len() bytes of records.
        let n = unsafe {
            libc::syscall(
                libc::SYS_getdents64,
                listing.as_raw_fd(),
                buf.as_mut_ptr(),
                buf.len(),
            )
        };
        let filled = match check_long(n) {
            Ok(0) => return Ok(true),
            Ok(n) => buf.get(..n as usize).ok_or(io::ErrorKind::InvalidData)?,
            Err(error) if error.raw_os_error() == Some(libc::ENOENT) => return Ok(true),
            Err(error) => return Err(error),
        };
        // Each record: d_ino (8), d_off (8), d_reclen (2), d_type (1), then
        // the NUL-terminated name.
        let mut offset = 0;
        while offset < filled.len() {
            let malformed = || io::Error::from_raw_os_error(libc::EINVAL);
            let header = filled.get(offset..offset + 19).ok_or_else(malformed)?;
            let reclen = usize::from(u16::from_ne_bytes([header[16], header[17]]));
            let record = filled
                .get(offset..offset + reclen)
                .filter(|record| record.len() > 19)
                .ok_or_else(malformed)?;
            let name = &record[19..];
            let name = &name[..name.iter().position(|&b| b == 0).unwrap_or(name.len())];
            if name != b"." && name != b".." {
                return Ok(false);
            }
            offset += reclen;
        }
    }
}

/// Whether `st` is the root directory of this process's filesystem view.
pub(crate) fn is_root_directory(st: &libc::stat) -> io::Result<bool> {
    let root = std::fs::metadata("/")?;
    use std::os::unix::fs::MetadataExt;
    Ok(st.st_dev == root.dev() && st.st_ino == root.ino())
}

/// Threads in this process (`/proc/self/stat`, field 20).
pub(crate) fn thread_count() -> io::Result<u64> {
    let stat = std::fs::read_to_string("/proc/self/stat")?;
    let after_comm = stat
        .rfind(')')
        .map(|end| &stat[end + 1..])
        .ok_or_else(|| io::Error::from_raw_os_error(libc::EINVAL))?;
    // After the command name: state is field 3, num_threads is field 20.
    after_comm
        .split_whitespace()
        .nth(17)
        .and_then(|field| field.parse().ok())
        .ok_or_else(|| io::Error::from_raw_os_error(libc::EINVAL))
}

/// Identity (`kind:[inode]`) of this process's namespace of `kind`. The
/// link is read, never followed: following it needs access to the namespace
/// file itself, which AppArmor's unprivileged user-namespace profile denies.
pub(crate) fn namespace_id(kind: &str) -> io::Result<String> {
    let target = std::fs::read_link(format!("/proc/self/ns/{kind}"))?;
    let text = target
        .into_os_string()
        .into_string()
        .map_err(|_| io::Error::from_raw_os_error(libc::EINVAL))?;
    if text.starts_with(&format!("{kind}:["))
        || (kind == "pid_for_children" && text.starts_with("pid:["))
    {
        Ok(text)
    } else {
        Err(io::Error::from_raw_os_error(libc::EINVAL))
    }
}

/// Write the whole buffer to a descriptor.
pub(crate) fn write_all(fd: BorrowedFd<'_>, mut bytes: &[u8]) -> io::Result<()> {
    while !bytes.is_empty() {
        // SAFETY: write reads from a live buffer.
        let n = unsafe { libc::write(fd.as_raw_fd(), bytes.as_ptr().cast(), bytes.len()) };
        if n < 0 {
            let error = io::Error::last_os_error();
            if error.raw_os_error() == Some(libc::EINTR) {
                continue;
            }
            return Err(error);
        }
        bytes = &bytes[n as usize..];
    }
    Ok(())
}

/// Read until `buf` is full or end of file; returns the bytes read.
pub(crate) fn read_full(fd: BorrowedFd<'_>, buf: &mut [u8]) -> io::Result<usize> {
    let mut filled = 0;
    while filled < buf.len() {
        // SAFETY: read writes into the unfilled part of a live buffer.
        let n = unsafe {
            libc::read(
                fd.as_raw_fd(),
                buf[filled..].as_mut_ptr().cast(),
                buf.len() - filled,
            )
        };
        if n < 0 {
            let error = io::Error::last_os_error();
            if error.raw_os_error() == Some(libc::EINTR) {
                continue;
            }
            return Err(error);
        }
        if n == 0 {
            break;
        }
        filled += n as usize;
    }
    Ok(filled)
}

/// Open a fixed, trusted absolute path (never caller text).
pub(crate) fn open_fixed(path: &std::ffi::CStr, flags: libc::c_int) -> io::Result<OwnedFd> {
    // SAFETY: path is a NUL-terminated constant; open returns a new
    // descriptor owned here on success.
    let fd = check(unsafe { libc::open(path.as_ptr(), flags | libc::O_CLOEXEC) })?;
    // SAFETY: open succeeded, so fd is new and owned here.
    Ok(unsafe { OwnedFd::from_raw_fd(fd) })
}

/// Interface names in this network namespace, and whether any is up.
pub(crate) fn network_interfaces() -> io::Result<(Vec<String>, bool)> {
    let dev = std::fs::read_to_string("/proc/self/net/dev")?;
    let names: Vec<String> = dev
        .lines()
        .skip(2)
        .filter_map(|line| line.split(':').next())
        .map(|name| name.trim().to_string())
        .collect();
    // SAFETY: socket returns a new descriptor owned here on success.
    let raw =
        check(unsafe { libc::socket(libc::AF_INET, libc::SOCK_DGRAM | libc::SOCK_CLOEXEC, 0) })?;
    // SAFETY: socket succeeded.
    let socket = unsafe { OwnedFd::from_raw_fd(raw) };
    let mut any_up = false;
    for name in &names {
        // SAFETY: ifreq is plain data; zeroed is a valid initial value.
        let mut request: libc::ifreq = unsafe { std::mem::zeroed() };
        let bytes = name.as_bytes();
        if bytes.len() >= request.ifr_name.len() {
            return Err(io::Error::from_raw_os_error(libc::ENAMETOOLONG));
        }
        for (slot, byte) in request.ifr_name.iter_mut().zip(bytes) {
            *slot = *byte as libc::c_char;
        }
        // SAFETY: SIOCGIFFLAGS reads the name and writes the flags of the
        // same ifreq.
        check(unsafe { libc::ioctl(socket.as_raw_fd(), libc::SIOCGIFFLAGS, &mut request) })?;
        // SAFETY: SIOCGIFFLAGS filled the flags member of the union.
        let flags = unsafe { request.ifr_ifru.ifru_flags };
        if flags & libc::IFF_UP as libc::c_short != 0 {
            any_up = true;
        }
    }
    Ok((names, any_up))
}
