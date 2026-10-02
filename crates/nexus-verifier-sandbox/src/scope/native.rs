//! The kernel observations the scope module trusts (P2-V1-R3B-I4): the
//! cgroup the kernel reports for the backend's own retained, unreaped
//! helper, and cgroup directories retained by descriptor.
//!
//! A cgroup path is used once, to open the directory the kernel reported
//! the helper in; from then on every read, `cgroup.kill` and the removal
//! check go through the retained descriptor, never through the path or a
//! unit name. The helper's process id is only the locator of that retained
//! child.
//!
//! Production uses [`Kernel`]: `/proc/<pid>/cgroup` and `/sys/fs/cgroup`.
//! The interface is private to the crate; the deterministic test cgroups
//! exist only in test builds.

use std::ffi::CString;
use std::fmt::Debug;
use std::io;
use std::os::fd::{AsFd, AsRawFd, FromRawFd, OwnedFd};

use super::ScopeError;
use crate::launcher::Helper;
use crate::sys;

const CGROUP2_SUPER_MAGIC: i64 = 0x6367_7270;

/// A cgroup directory retained by descriptor.
pub(crate) trait CgroupDir: Send + Sync + Debug {
    /// The text of the file `name` in the directory.
    fn read(&self, name: &str) -> io::Result<String>;
    /// End every process in the cgroup (`cgroup.kill`).
    fn kill(&self) -> io::Result<()>;
    /// Whether the directory lists no entry (a removed cgroup lists none).
    fn listing_is_empty(&self) -> io::Result<bool>;
}

/// The kernel observations.
pub(crate) trait Native: Send + Sync {
    /// The cgroup v2 path (relative to the cgroup root) the kernel reports
    /// for the retained, unreaped `helper`; `None` without a cgroup v2 line.
    fn membership(&self, helper: &Helper) -> io::Result<Option<String>>;
    /// Open the cgroup at `path`, relative to the cgroup root, by a retained
    /// descriptor that must be a cgroup v2 directory.
    fn open(&self, path: &str) -> Result<Box<dyn CgroupDir>, ScopeError>;
}

/// `/proc` and the cgroup v2 hierarchy at `/sys/fs/cgroup`.
pub(crate) struct Kernel;

impl Native for Kernel {
    fn membership(&self, helper: &Helper) -> io::Result<Option<String>> {
        // Exactly the unified hierarchy's single line, as before.
        let text = std::fs::read_to_string(format!("/proc/{}/cgroup", helper.pid()))?;
        Ok(text.trim_end().strip_prefix("0::").map(str::to_string))
    }

    fn open(&self, path: &str) -> Result<Box<dyn CgroupDir>, ScopeError> {
        let full = CString::new(format!("/sys/fs/cgroup{path}"))
            .map_err(|_| ScopeError::Mismatch("cgroup path"))?;
        // SAFETY: a NUL-terminated path; open returns a new descriptor.
        let fd = unsafe {
            libc::open(
                full.as_ptr(),
                libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_RDONLY | libc::O_CLOEXEC,
            )
        };
        if fd < 0 {
            return Err(ScopeError::Io(io::Error::last_os_error()));
        }
        // SAFETY: open succeeded, so fd is new and owned here.
        let dir = unsafe { OwnedFd::from_raw_fd(fd) };
        // SAFETY: statfs is plain data; fstatfs fills it.
        let mut fs: libc::statfs = unsafe { std::mem::zeroed() };
        // SAFETY: fstatfs writes one statfs structure.
        if unsafe { libc::fstatfs(dir.as_raw_fd(), &mut fs) } != 0
            || fs.f_type != CGROUP2_SUPER_MAGIC
        {
            return Err(ScopeError::Mismatch("not a cgroup v2 directory"));
        }
        Ok(Box::new(KernelDir { dir }))
    }
}

/// A cgroup v2 directory, retained by descriptor.
#[derive(Debug)]
struct KernelDir {
    dir: OwnedFd,
}

impl KernelDir {
    fn open_file(&self, name: &str, flags: libc::c_int) -> io::Result<std::fs::File> {
        let name = CString::new(name).map_err(|_| io::Error::from_raw_os_error(libc::EINVAL))?;
        // SAFETY: openat relative to the retained directory; a new
        // descriptor is returned on success.
        let fd = unsafe {
            libc::openat(
                self.dir.as_raw_fd(),
                name.as_ptr(),
                flags | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            )
        };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: openat succeeded, so fd is new and owned here.
        Ok(unsafe { std::fs::File::from_raw_fd(fd) })
    }
}

impl CgroupDir for KernelDir {
    fn read(&self, name: &str) -> io::Result<String> {
        use std::io::Read;
        let mut text = String::new();
        self.open_file(name, libc::O_RDONLY)?
            .read_to_string(&mut text)?;
        Ok(text)
    }

    fn kill(&self) -> io::Result<()> {
        use std::io::Write;
        self.open_file("cgroup.kill", libc::O_WRONLY)?
            .write_all(b"1")
    }

    fn listing_is_empty(&self) -> io::Result<bool> {
        sys::directory_is_empty(self.dir.as_fd())
    }
}
