//! The store's I/O boundary (design sections 8, 10.1 and 17): the
//! [`StoreIo`] trait every store algorithm is written against, the bounded
//! retry helpers of the append protocol (W3 and W4, section 9.4), a
//! controlled fault adapter, and the Linux implementation.
//!
//! This is the only file of the store that contains `unsafe` code. Every
//! foreign call is wrapped exactly once, in a small function whose safety
//! argument is written beside it; nothing else in the store calls `libc`.
//!
//! What the Linux implementation is for, in this mission: unprivileged,
//! isolated primitive tests beneath `CARGO_TARGET_TMPDIR` only (no-follow,
//! descriptor-relative opening; symbolic- and hard-link refusal; a
//! non-blocking FIFO open; independent and duplicated `flock` descriptions;
//! complete and short writes; file and directory syncs and `futimens` on the
//! test's own files; descriptor identity and same-filesystem comparison).
//! [`LinuxIo`] implements [`StoreIo`] only. The opening, activation, claim
//! and maintenance algorithms need a [`super::open::Platform`] (the host
//! view: mount table, effective profile, kernel identity, storage
//! observation), and no Linux platform exists: the real configured-store
//! entry is closed ([`super::open::open_configured_store`]), so no code here
//! reads a mount table, `/proc/fs`, sysfs, a device attribute or any host
//! qualification data, and an unprivileged fixture cannot be opened as a
//! store.

use std::collections::VecDeque;
use std::fmt;
use std::sync::{Arc, Mutex};

/// The error numbers the store tells apart. Anything else is kept as its
/// number and is never success.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Errno {
    Intr,
    Io,
    Rofs,
    NoSpc,
    Dquot,
    NoEnt,
    Exist,
    Loop,
    Again,
    Xdev,
    BadF,
    NotDir,
    IsDir,
    Perm,
    Acces,
    Inval,
    NotEmpty,
    MLink,
    OpNotSupp,
    NxIo,
    Other(i32),
}

impl Errno {
    #[cfg(target_os = "linux")]
    fn from_raw(raw: i32) -> Errno {
        match raw {
            libc::EINTR => Errno::Intr,
            libc::EIO => Errno::Io,
            libc::EROFS => Errno::Rofs,
            libc::ENOSPC => Errno::NoSpc,
            libc::EDQUOT => Errno::Dquot,
            libc::ENOENT => Errno::NoEnt,
            libc::EEXIST => Errno::Exist,
            libc::ELOOP => Errno::Loop,
            libc::EAGAIN => Errno::Again,
            libc::EXDEV => Errno::Xdev,
            libc::EBADF => Errno::BadF,
            libc::ENOTDIR => Errno::NotDir,
            libc::EISDIR => Errno::IsDir,
            libc::EPERM => Errno::Perm,
            libc::EACCES => Errno::Acces,
            libc::EINVAL => Errno::Inval,
            libc::ENOTEMPTY => Errno::NotEmpty,
            libc::EMLINK => Errno::MLink,
            libc::EOPNOTSUPP => Errno::OpNotSupp,
            libc::ENXIO => Errno::NxIo,
            other => Errno::Other(other),
        }
    }
}

/// One failed I/O call: which operation, and its error number.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IoError {
    pub op: &'static str,
    pub errno: Errno,
}

impl IoError {
    pub fn new(op: &'static str, errno: Errno) -> Self {
        Self { op, errno }
    }
}

impl fmt::Display for IoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {:?}", self.op, self.errno)
    }
}

/// An inode's type, as `fstat` and `fstatat` report it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FileType {
    Regular,
    Directory,
    Symlink,
    Fifo,
    Socket,
    CharDevice,
    BlockDevice,
    Unknown,
}

/// The facts of one inode the store checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stat {
    pub file_type: FileType,
    pub uid: u32,
    pub gid: u32,
    /// Permission bits (`0o7777`).
    pub mode: u32,
    pub nlink: u64,
    pub ino: u64,
    pub dev: u64,
    pub size: u64,
}

impl Stat {
    /// The same inode of the same filesystem.
    pub fn same_inode(&self, other: &Stat) -> bool {
        self.dev == other.dev && self.ino == other.ino
    }
}

/// A non-blocking `flock` request. There is no unlock: a store lock is
/// released only when its description closes (design section 8.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LockRequest {
    /// `LOCK_EX | LOCK_NB`.
    Exclusive,
    /// `LOCK_SH | LOCK_NB`.
    Shared,
}

/// A directory's entry names, or the fact that there are more than the
/// bound (in which case no name was examined).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Listing {
    Names(Vec<String>),
    TooMany,
}

/// The operations the store performs, on descriptors only. Every open is
/// `O_CLOEXEC`, `O_NOFOLLOW` and resolved beneath the directory given, with
/// no symbolic link, magic link or mount crossing (design section 10.1).
/// Handles close when dropped; dropping a description is the only way its
/// advisory lock is released.
pub trait StoreIo: Send + Sync {
    /// A directory handle (`O_PATH | O_DIRECTORY`): a base for `*at` calls.
    type Dir: Send + Sync;
    /// An open file description (read-only, read-write, or a directory
    /// opened for a sync).
    type File: Send + Sync;

    /// `/`, opened `O_PATH | O_DIRECTORY`.
    fn root_dir(&self) -> Result<Self::Dir, IoError>;
    /// One component, `O_PATH | O_DIRECTORY | O_NOFOLLOW`.
    fn open_dir(&self, at: &Self::Dir, name: &str) -> Result<Self::Dir, IoError>;
    /// `fstatat(at, name, AT_SYMLINK_NOFOLLOW)`.
    fn stat_at(&self, at: &Self::Dir, name: &str) -> Result<Stat, IoError>;
    fn stat_dir(&self, dir: &Self::Dir) -> Result<Stat, IoError>;
    fn stat_file(&self, file: &Self::File) -> Result<Stat, IoError>;
    /// The entry names (without `.` and `..`), or `TooMany` once more than
    /// `limit` are seen: the count is taken before any entry is examined.
    fn list_dir(&self, dir: &Self::Dir, limit: usize) -> Result<Listing, IoError>;
    /// `O_RDONLY | O_NONBLOCK | O_NOCTTY | O_NOFOLLOW | O_CLOEXEC`: a FIFO
    /// that replaced the entry after its type check opens without blocking.
    fn open_read(&self, at: &Self::Dir, name: &str) -> Result<Self::File, IoError>;
    /// `O_RDWR | O_NONBLOCK | O_NOCTTY | O_NOFOLLOW | O_CLOEXEC`.
    fn open_write(&self, at: &Self::Dir, name: &str) -> Result<Self::File, IoError>;
    /// `O_RDONLY | O_DIRECTORY | O_NOFOLLOW | O_CLOEXEC`, for a directory
    /// sync (an `O_PATH` descriptor cannot be synced).
    fn open_dir_for_sync(&self, at: &Self::Dir, name: &str) -> Result<Self::File, IoError>;
    /// Non-blocking `flock`; `Errno::Again` when another description holds
    /// a conflicting lock.
    fn flock(&self, file: &Self::File, request: LockRequest) -> Result<(), IoError>;
    fn pread(&self, file: &Self::File, offset: u64, buf: &mut [u8]) -> Result<usize, IoError>;
    /// One `pwrite`: it may write fewer bytes than given, or none.
    fn pwrite(&self, file: &Self::File, offset: u64, buf: &[u8]) -> Result<usize, IoError>;
    fn fdatasync(&self, file: &Self::File) -> Result<(), IoError>;
    fn fsync(&self, file: &Self::File) -> Result<(), IoError>;
    /// `futimens(fd, NULL)`: both timestamps to the current time.
    fn touch(&self, file: &Self::File) -> Result<(), IoError>;
    /// `getrandom`, filling the buffer.
    fn random(&self, buf: &mut [u8]) -> Result<(), IoError>;
    /// `O_CREAT | O_EXCL | O_NOFOLLOW | O_RDWR | O_CLOEXEC`, owned by the
    /// caller, with `mode`.
    fn create_exclusive(
        &self,
        at: &Self::Dir,
        name: &str,
        mode: u32,
    ) -> Result<Self::File, IoError>;
    fn make_dir(&self, at: &Self::Dir, name: &str, mode: u32) -> Result<(), IoError>;
    /// `linkat`: publication without replacement (`Errno::Exist`).
    fn link(
        &self,
        from: &Self::Dir,
        from_name: &str,
        to: &Self::Dir,
        to_name: &str,
    ) -> Result<(), IoError>;
    /// `renameat`: atomic replacement of the target.
    fn rename(
        &self,
        from: &Self::Dir,
        from_name: &str,
        to: &Self::Dir,
        to_name: &str,
    ) -> Result<(), IoError>;
    fn unlink(&self, at: &Self::Dir, name: &str) -> Result<(), IoError>;
    /// `unlinkat(at, name, AT_REMOVEDIR)` (P2-V1-R3B-I3-I1-R3): remove one
    /// empty directory's entry, relative to an open directory. Never
    /// recursive: a directory that is not empty refuses (`ENOTEMPTY`), and so
    /// does an entry that is not a directory (`ENOTDIR`).
    fn remove_dir(&self, at: &Self::Dir, name: &str) -> Result<(), IoError>;
    /// `fallocate` mode 0 over `0 .. len`.
    fn allocate(&self, file: &Self::File, len: u64) -> Result<(), IoError>;
    /// `fchown` of a file the caller just created (never one another process
    /// may hold open, design section 13.1).
    fn set_owner(&self, file: &Self::File, uid: u32, gid: u32) -> Result<(), IoError>;
}

/// How many times a zero-progress `pwrite` or an `EINTR` is retried before
/// the write fails (design section 9.4, W3), and how many times an `EINTR`
/// sync is retried (W4, and activation's A2).
pub const RETRIES: u32 = 3;

/// Why a whole-block write did not complete.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WriteFailure {
    /// An error other than `EINTR`.
    Error(IoError),
    /// More than `RETRIES` zero-progress results or interruptions.
    NoProgress,
}

/// W3: write all of `data` at `offset`, looping on short counts. A
/// zero-progress result or `EINTR` is retried at most `RETRIES` times in
/// all; any other error is final. A block counts as written only once every
/// byte is.
pub fn write_fully<I: StoreIo>(
    io: &I,
    file: &I::File,
    offset: u64,
    data: &[u8],
) -> Result<(), WriteFailure> {
    let mut done = 0usize;
    let mut stalls = 0u32;
    while done < data.len() {
        let Some(at) = offset.checked_add(done as u64) else {
            return Err(WriteFailure::Error(IoError::new("pwrite", Errno::Inval)));
        };
        match io.pwrite(file, at, &data[done..]) {
            Ok(0)
            | Err(IoError {
                errno: Errno::Intr, ..
            }) => {
                stalls += 1;
                if stalls > RETRIES {
                    return Err(WriteFailure::NoProgress);
                }
            }
            Ok(count) => done += count.min(data.len() - done),
            Err(error) => return Err(WriteFailure::Error(error)),
        }
    }
    Ok(())
}

/// W4 (and A2): a sync whose `EINTR` is retried at most `RETRIES` times; any
/// other result is final, and a failed sync is never retried into success.
pub fn sync_with_retries<I: StoreIo>(
    io: &I,
    file: &I::File,
    data_only: bool,
) -> Result<(), IoError> {
    let mut attempts = 0u32;
    loop {
        let result = if data_only {
            io.fdatasync(file)
        } else {
            io.fsync(file)
        };
        match result {
            Err(IoError {
                errno: Errno::Intr, ..
            }) if attempts < RETRIES => attempts += 1,
            other => return other,
        }
    }
}

/// A fault one call of [`Faulty`] injects instead of (or into) the real call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fault {
    /// `pwrite` writes at most this many bytes (then returns that count).
    Short(usize),
    /// `pwrite` writes nothing and returns 0.
    Zero,
    /// The call returns `EINTR` without doing anything.
    Interrupted,
    /// The call returns this error without doing anything.
    Fail(Errno),
}

/// A controlled adapter: it delegates every call to the inner I/O, except
/// that `pwrite`, `fdatasync` and `fsync` first consume one planned fault
/// each, if any is queued. It is how tests drive short writes, zero
/// progress, interruptions and errors through the store's own code against
/// the simulator or a native temporary file; it never claims an error the
/// inner I/O did not get unless one was planned.
pub struct Faulty<I: StoreIo> {
    pub inner: I,
    writes: Arc<Mutex<VecDeque<Fault>>>,
    syncs: Arc<Mutex<VecDeque<Fault>>>,
}

impl<I: StoreIo> Faulty<I> {
    pub fn new(inner: I) -> Self {
        Self {
            inner,
            writes: Arc::default(),
            syncs: Arc::default(),
        }
    }

    pub fn plan_writes(&self, faults: impl IntoIterator<Item = Fault>) {
        if let Ok(mut plan) = self.writes.lock() {
            plan.extend(faults);
        }
    }

    pub fn plan_syncs(&self, faults: impl IntoIterator<Item = Fault>) {
        if let Ok(mut plan) = self.syncs.lock() {
            plan.extend(faults);
        }
    }

    fn next(queue: &Mutex<VecDeque<Fault>>) -> Option<Fault> {
        queue.lock().ok().and_then(|mut plan| plan.pop_front())
    }
}

impl<I: StoreIo> StoreIo for Faulty<I> {
    type Dir = I::Dir;
    type File = I::File;

    fn root_dir(&self) -> Result<Self::Dir, IoError> {
        self.inner.root_dir()
    }
    fn open_dir(&self, at: &Self::Dir, name: &str) -> Result<Self::Dir, IoError> {
        self.inner.open_dir(at, name)
    }
    fn stat_at(&self, at: &Self::Dir, name: &str) -> Result<Stat, IoError> {
        self.inner.stat_at(at, name)
    }
    fn stat_dir(&self, dir: &Self::Dir) -> Result<Stat, IoError> {
        self.inner.stat_dir(dir)
    }
    fn stat_file(&self, file: &Self::File) -> Result<Stat, IoError> {
        self.inner.stat_file(file)
    }
    fn list_dir(&self, dir: &Self::Dir, limit: usize) -> Result<Listing, IoError> {
        self.inner.list_dir(dir, limit)
    }
    fn open_read(&self, at: &Self::Dir, name: &str) -> Result<Self::File, IoError> {
        self.inner.open_read(at, name)
    }
    fn open_write(&self, at: &Self::Dir, name: &str) -> Result<Self::File, IoError> {
        self.inner.open_write(at, name)
    }
    fn open_dir_for_sync(&self, at: &Self::Dir, name: &str) -> Result<Self::File, IoError> {
        self.inner.open_dir_for_sync(at, name)
    }
    fn flock(&self, file: &Self::File, request: LockRequest) -> Result<(), IoError> {
        self.inner.flock(file, request)
    }
    fn pread(&self, file: &Self::File, offset: u64, buf: &mut [u8]) -> Result<usize, IoError> {
        self.inner.pread(file, offset, buf)
    }
    fn pwrite(&self, file: &Self::File, offset: u64, buf: &[u8]) -> Result<usize, IoError> {
        match Self::next(&self.writes) {
            None => self.inner.pwrite(file, offset, buf),
            Some(Fault::Short(limit)) => {
                self.inner
                    .pwrite(file, offset, &buf[..limit.min(buf.len())])
            }
            Some(Fault::Zero) => Ok(0),
            Some(Fault::Interrupted) => Err(IoError::new("pwrite", Errno::Intr)),
            Some(Fault::Fail(errno)) => Err(IoError::new("pwrite", errno)),
        }
    }
    fn fdatasync(&self, file: &Self::File) -> Result<(), IoError> {
        match Self::next(&self.syncs) {
            None | Some(Fault::Short(_)) | Some(Fault::Zero) => self.inner.fdatasync(file),
            Some(Fault::Interrupted) => Err(IoError::new("fdatasync", Errno::Intr)),
            Some(Fault::Fail(errno)) => Err(IoError::new("fdatasync", errno)),
        }
    }
    fn fsync(&self, file: &Self::File) -> Result<(), IoError> {
        match Self::next(&self.syncs) {
            None | Some(Fault::Short(_)) | Some(Fault::Zero) => self.inner.fsync(file),
            Some(Fault::Interrupted) => Err(IoError::new("fsync", Errno::Intr)),
            Some(Fault::Fail(errno)) => Err(IoError::new("fsync", errno)),
        }
    }
    fn touch(&self, file: &Self::File) -> Result<(), IoError> {
        self.inner.touch(file)
    }
    fn random(&self, buf: &mut [u8]) -> Result<(), IoError> {
        self.inner.random(buf)
    }
    fn create_exclusive(
        &self,
        at: &Self::Dir,
        name: &str,
        mode: u32,
    ) -> Result<Self::File, IoError> {
        self.inner.create_exclusive(at, name, mode)
    }
    fn make_dir(&self, at: &Self::Dir, name: &str, mode: u32) -> Result<(), IoError> {
        self.inner.make_dir(at, name, mode)
    }
    fn link(
        &self,
        from: &Self::Dir,
        from_name: &str,
        to: &Self::Dir,
        to_name: &str,
    ) -> Result<(), IoError> {
        self.inner.link(from, from_name, to, to_name)
    }
    fn rename(
        &self,
        from: &Self::Dir,
        from_name: &str,
        to: &Self::Dir,
        to_name: &str,
    ) -> Result<(), IoError> {
        self.inner.rename(from, from_name, to, to_name)
    }
    fn unlink(&self, at: &Self::Dir, name: &str) -> Result<(), IoError> {
        self.inner.unlink(at, name)
    }
    fn remove_dir(&self, at: &Self::Dir, name: &str) -> Result<(), IoError> {
        self.inner.remove_dir(at, name)
    }
    fn allocate(&self, file: &Self::File, len: u64) -> Result<(), IoError> {
        self.inner.allocate(file, len)
    }
    fn set_owner(&self, file: &Self::File, uid: u32, gid: u32) -> Result<(), IoError> {
        self.inner.set_owner(file, uid, gid)
    }
}

// ---------------------------------------------------------------------------
// Linux
// ---------------------------------------------------------------------------

/// The Linux implementation (see the module documentation for what it is
/// used for in this mission).
#[cfg(target_os = "linux")]
pub mod linux {
    use std::ffi::CString;
    use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};

    use super::{Errno, FileType, IoError, Listing, LockRequest, Stat, StoreIo};

    /// `struct open_how` of `linux/openat2.h`: three `u64` fields. libc's own
    /// definition is `#[non_exhaustive]`, so it cannot be built here; this
    /// mirror has its exact size and field order (checked below).
    #[repr(C)]
    struct OpenHow {
        flags: u64,
        mode: u64,
        resolve: u64,
    }

    const _: () = assert!(std::mem::size_of::<OpenHow>() == std::mem::size_of::<libc::open_how>());
    const _: () = assert!(std::mem::size_of::<OpenHow>() == 24);

    /// Resolution of every store open: beneath the directory given, no
    /// symbolic link at any component, no magic link, no mount crossing.
    const RESOLVE: u64 = libc::RESOLVE_BENEATH
        | libc::RESOLVE_NO_SYMLINKS
        | libc::RESOLVE_NO_MAGICLINKS
        | libc::RESOLVE_NO_XDEV;

    /// The Linux implementation of [`StoreIo`]: one wrapper per system call.
    #[derive(Debug, Default, Clone, Copy)]
    pub struct LinuxIo;

    /// An `O_PATH | O_DIRECTORY` descriptor, closed when dropped.
    #[derive(Debug)]
    pub struct LinuxDir(OwnedFd);

    /// An open file description, closed when dropped.
    #[derive(Debug)]
    pub struct LinuxFile(OwnedFd);

    impl LinuxDir {
        pub fn raw(&self) -> RawFd {
            self.0.as_raw_fd()
        }
    }

    impl LinuxFile {
        pub fn raw(&self) -> RawFd {
            self.0.as_raw_fd()
        }
    }

    fn last_errno() -> Errno {
        Errno::from_raw(
            std::io::Error::last_os_error()
                .raw_os_error()
                .unwrap_or(libc::EIO),
        )
    }

    /// One directory entry's name: never empty, never `.` or `..`, never
    /// containing a separator, so that every open stays one component deep.
    fn name(value: &str, op: &'static str) -> Result<CString, IoError> {
        if value.is_empty() || value == "." || value == ".." || value.contains('/') {
            return Err(IoError::new(op, Errno::Inval));
        }
        CString::new(value).map_err(|_| IoError::new(op, Errno::Inval))
    }

    /// Take ownership of a descriptor a successful call just returned.
    fn own(fd: libc::c_long, op: &'static str) -> Result<OwnedFd, IoError> {
        if fd < 0 {
            return Err(IoError::new(op, last_errno()));
        }
        let Ok(raw) = RawFd::try_from(fd) else {
            return Err(IoError::new(op, Errno::Inval));
        };
        // SAFETY: `raw` is a descriptor the kernel returned to this call just
        // now (non-negative, so the call succeeded). Nothing else owns it:
        // it is not stored anywhere else, so the `OwnedFd` is its only owner
        // and closes it exactly once.
        Ok(unsafe { OwnedFd::from_raw_fd(raw) })
    }

    /// `openat2(dirfd, name, how, sizeof how)`.
    fn openat2(
        dirfd: RawFd,
        path: &CString,
        flags: i32,
        mode: u32,
        op: &'static str,
    ) -> Result<OwnedFd, IoError> {
        let how = OpenHow {
            flags: flags as u32 as u64,
            mode: u64::from(mode),
            resolve: RESOLVE,
        };
        // SAFETY: `path` is a NUL-terminated string that outlives the call;
        // `how` is a properly initialised `struct open_how` of the size
        // passed, living on this stack frame for the whole call; the kernel
        // only reads both. `dirfd` is a descriptor the caller borrows for the
        // duration of the call. Every argument is passed as `long` or a
        // pointer, as the variadic `syscall` expects.
        let fd = unsafe {
            libc::syscall(
                libc::SYS_openat2,
                dirfd as libc::c_long,
                path.as_ptr(),
                &how as *const OpenHow,
                std::mem::size_of::<OpenHow>() as libc::c_long,
            )
        };
        own(fd, op)
    }

    /// A link count, whatever its width on this target.
    fn widen<T: Into<u64>>(value: T) -> u64 {
        value.into()
    }

    fn convert(st: &libc::stat) -> Stat {
        let file_type = match st.st_mode & libc::S_IFMT {
            libc::S_IFREG => FileType::Regular,
            libc::S_IFDIR => FileType::Directory,
            libc::S_IFLNK => FileType::Symlink,
            libc::S_IFIFO => FileType::Fifo,
            libc::S_IFSOCK => FileType::Socket,
            libc::S_IFCHR => FileType::CharDevice,
            libc::S_IFBLK => FileType::BlockDevice,
            _ => FileType::Unknown,
        };
        Stat {
            file_type,
            uid: st.st_uid,
            gid: st.st_gid,
            mode: st.st_mode & 0o7777,
            nlink: widen(st.st_nlink),
            ino: st.st_ino,
            dev: st.st_dev,
            size: st.st_size.max(0) as u64,
        }
    }

    fn fstat_raw(fd: RawFd, op: &'static str) -> Result<Stat, IoError> {
        let mut st = std::mem::MaybeUninit::<libc::stat>::uninit();
        // SAFETY: `st` is a writable buffer of exactly `struct stat`'s size on
        // this frame; the kernel fills it completely when the call returns 0,
        // and only then is it read (`assume_init`). `fd` is borrowed for the
        // call.
        let result = unsafe { libc::fstat(fd, st.as_mut_ptr()) };
        if result != 0 {
            return Err(IoError::new(op, last_errno()));
        }
        // SAFETY: the call succeeded, so the kernel initialised the buffer.
        Ok(convert(unsafe { &st.assume_init() }))
    }

    fn offset(value: u64, op: &'static str) -> Result<libc::off_t, IoError> {
        libc::off_t::try_from(value).map_err(|_| IoError::new(op, Errno::Inval))
    }

    impl LinuxIo {
        /// A fixture's own base directory, opened by path
        /// (`O_PATH | O_DIRECTORY | O_CLOEXEC`), for primitive tests
        /// beneath `CARGO_TARGET_TMPDIR` only. Every store open is
        /// descriptor-relative from it.
        pub fn open_base(path: &std::path::Path) -> Result<LinuxDir, IoError> {
            use std::os::unix::ffi::OsStrExt;
            let path = CString::new(path.as_os_str().as_bytes())
                .map_err(|_| IoError::new("open base", Errno::Inval))?;
            // SAFETY: `path` is a NUL-terminated string valid for the call;
            // `open` only reads it. The flags open no file content and create
            // nothing.
            let fd = unsafe {
                libc::open(
                    path.as_ptr(),
                    libc::O_PATH | libc::O_DIRECTORY | libc::O_CLOEXEC,
                )
            };
            own(libc::c_long::from(fd), "open base").map(LinuxDir)
        }

        /// A second descriptor on the same open file description
        /// (`F_DUPFD_CLOEXEC`). For the duplicate-lock primitive test only:
        /// the store never duplicates a description (design section 8.2
        /// rule 1).
        pub fn fixture_duplicate(file: &LinuxFile) -> Result<LinuxFile, IoError> {
            // SAFETY: `file`'s descriptor is borrowed for the call;
            // `F_DUPFD_CLOEXEC` takes one integer argument and returns a new
            // descriptor that this function then owns.
            let fd = unsafe { libc::fcntl(file.raw(), libc::F_DUPFD_CLOEXEC, 0) };
            own(libc::c_long::from(fd), "dup").map(LinuxFile)
        }

        /// `flock(fd, LOCK_UN)`. For the duplicate-lock primitive test only:
        /// the store never unlocks (design section 8.1).
        pub fn fixture_unlock(file: &LinuxFile) -> Result<(), IoError> {
            // SAFETY: `file`'s descriptor is borrowed for the call; `flock`
            // takes no pointer.
            let result = unsafe { libc::flock(file.raw(), libc::LOCK_UN) };
            if result != 0 {
                return Err(IoError::new("flock", last_errno()));
            }
            Ok(())
        }

        /// `mkfifoat(dir, name, mode)`. A fixture primitive for the FIFO
        /// test.
        pub fn fixture_fifo(dir: &LinuxDir, entry: &str, mode: u32) -> Result<(), IoError> {
            let entry = name(entry, "mkfifoat")?;
            // SAFETY: `entry` is NUL-terminated and valid for the call; `dir`
            // is borrowed for the call.
            let result = unsafe { libc::mkfifoat(dir.raw(), entry.as_ptr(), mode) };
            if result != 0 {
                return Err(IoError::new("mkfifoat", last_errno()));
            }
            Ok(())
        }

        /// `symlinkat(target, dir, name)`. A fixture primitive for the
        /// symbolic-link tests.
        pub fn fixture_symlink(dir: &LinuxDir, entry: &str, target: &str) -> Result<(), IoError> {
            let entry = name(entry, "symlinkat")?;
            let target =
                CString::new(target).map_err(|_| IoError::new("symlinkat", Errno::Inval))?;
            // SAFETY: both strings are NUL-terminated and valid for the call;
            // `dir` is borrowed for the call.
            let result = unsafe { libc::symlinkat(target.as_ptr(), dir.raw(), entry.as_ptr()) };
            if result != 0 {
                return Err(IoError::new("symlinkat", last_errno()));
            }
            Ok(())
        }

        /// `fsync` on an `O_PATH` directory handle. For the primitive test
        /// that such a handle cannot be synced (design section 16.2): the
        /// store syncs a directory only through an `O_RDONLY | O_DIRECTORY`
        /// description.
        pub fn fixture_fsync_path_handle(dir: &LinuxDir) -> Result<(), IoError> {
            // SAFETY: `dir` is borrowed for the call; `fsync` takes no
            // pointer.
            let result = unsafe { libc::fsync(dir.raw()) };
            if result != 0 {
                return Err(IoError::new("fsync", last_errno()));
            }
            Ok(())
        }

        /// Remove a directory entry the fixture made (`unlinkat` with
        /// `AT_REMOVEDIR` when `directory`). Bounded cleanup only: the
        /// caller first checks, by `fstatat`, that the entry is still the
        /// inode it created; nothing here deletes recursively or by path.
        pub fn fixture_remove(dir: &LinuxDir, entry: &str, directory: bool) -> Result<(), IoError> {
            let entry = name(entry, "unlinkat")?;
            let flags = if directory { libc::AT_REMOVEDIR } else { 0 };
            // SAFETY: `entry` is NUL-terminated and valid for the call; `dir`
            // is borrowed for the call.
            let result = unsafe { libc::unlinkat(dir.raw(), entry.as_ptr(), flags) };
            if result != 0 {
                return Err(IoError::new("unlinkat", last_errno()));
            }
            Ok(())
        }
    }

    impl StoreIo for LinuxIo {
        type Dir = LinuxDir;
        type File = LinuxFile;

        fn root_dir(&self) -> Result<LinuxDir, IoError> {
            let root = CString::new("/").map_err(|_| IoError::new("open /", Errno::Inval))?;
            // SAFETY: `root` is NUL-terminated and valid for the call.
            let fd = unsafe {
                libc::open(
                    root.as_ptr(),
                    libc::O_PATH | libc::O_DIRECTORY | libc::O_CLOEXEC,
                )
            };
            own(libc::c_long::from(fd), "open /").map(LinuxDir)
        }

        fn open_dir(&self, at: &LinuxDir, entry: &str) -> Result<LinuxDir, IoError> {
            let entry = name(entry, "openat2 dir")?;
            openat2(
                at.raw(),
                &entry,
                libc::O_PATH | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
                0,
                "openat2 dir",
            )
            .map(LinuxDir)
        }

        fn stat_at(&self, at: &LinuxDir, entry: &str) -> Result<Stat, IoError> {
            let entry = name(entry, "fstatat")?;
            let mut st = std::mem::MaybeUninit::<libc::stat>::uninit();
            // SAFETY: `entry` is NUL-terminated and valid for the call; `st`
            // is a writable `struct stat` on this frame, read only after the
            // call returned 0; `at` is borrowed for the call.
            let result = unsafe {
                libc::fstatat(
                    at.raw(),
                    entry.as_ptr(),
                    st.as_mut_ptr(),
                    libc::AT_SYMLINK_NOFOLLOW,
                )
            };
            if result != 0 {
                return Err(IoError::new("fstatat", last_errno()));
            }
            // SAFETY: the call succeeded, so the kernel initialised the buffer.
            Ok(convert(unsafe { &st.assume_init() }))
        }

        fn stat_dir(&self, dir: &LinuxDir) -> Result<Stat, IoError> {
            fstat_raw(dir.raw(), "fstat dir")
        }

        fn stat_file(&self, file: &LinuxFile) -> Result<Stat, IoError> {
            fstat_raw(file.raw(), "fstat")
        }

        fn list_dir(&self, dir: &LinuxDir, limit: usize) -> Result<Listing, IoError> {
            let dot = CString::new(".").map_err(|_| IoError::new("getdents64", Errno::Inval))?;
            let reader = openat2(
                dir.raw(),
                &dot,
                libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC,
                0,
                "open directory for listing",
            )?;
            let mut names = Vec::new();
            let mut buf = vec![0u8; 16 * 1024];
            loop {
                // SAFETY: `buf` is a writable buffer of `buf.len()` bytes that
                // outlives the call; the kernel writes at most that many bytes
                // of `linux_dirent64` records into it. `reader` is borrowed.
                let read = unsafe {
                    libc::syscall(
                        libc::SYS_getdents64,
                        reader.as_raw_fd() as libc::c_long,
                        buf.as_mut_ptr(),
                        buf.len() as libc::c_long,
                    )
                };
                if read < 0 {
                    return Err(IoError::new("getdents64", last_errno()));
                }
                if read == 0 {
                    return Ok(Listing::Names(names));
                }
                let filled = &buf[..read as usize];
                let mut at = 0usize;
                while at + 19 <= filled.len() {
                    let reclen =
                        usize::from(u16::from_ne_bytes([filled[at + 16], filled[at + 17]]));
                    if reclen < 20 || at + reclen > filled.len() {
                        return Err(IoError::new("getdents64", Errno::Io));
                    }
                    let raw_name = &filled[at + 19..at + reclen];
                    let end = raw_name
                        .iter()
                        .position(|byte| *byte == 0)
                        .unwrap_or(raw_name.len());
                    let entry = &raw_name[..end];
                    at += reclen;
                    if entry == b"." || entry == b".." {
                        continue;
                    }
                    if names.len() == limit {
                        return Ok(Listing::TooMany);
                    }
                    match std::str::from_utf8(entry) {
                        Ok(text) => names.push(text.to_string()),
                        Err(_) => names.push(String::from_utf8_lossy(entry).into_owned()),
                    }
                }
            }
        }

        fn open_read(&self, at: &LinuxDir, entry: &str) -> Result<LinuxFile, IoError> {
            let entry = name(entry, "openat2 read")?;
            openat2(
                at.raw(),
                &entry,
                libc::O_RDONLY
                    | libc::O_NONBLOCK
                    | libc::O_NOCTTY
                    | libc::O_NOFOLLOW
                    | libc::O_CLOEXEC,
                0,
                "openat2 read",
            )
            .map(LinuxFile)
        }

        fn open_write(&self, at: &LinuxDir, entry: &str) -> Result<LinuxFile, IoError> {
            let entry = name(entry, "openat2 write")?;
            openat2(
                at.raw(),
                &entry,
                libc::O_RDWR
                    | libc::O_NONBLOCK
                    | libc::O_NOCTTY
                    | libc::O_NOFOLLOW
                    | libc::O_CLOEXEC,
                0,
                "openat2 write",
            )
            .map(LinuxFile)
        }

        fn open_dir_for_sync(&self, at: &LinuxDir, entry: &str) -> Result<LinuxFile, IoError> {
            let entry = name(entry, "openat2 dir sync")?;
            openat2(
                at.raw(),
                &entry,
                libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
                0,
                "openat2 dir sync",
            )
            .map(LinuxFile)
        }

        fn flock(&self, file: &LinuxFile, request: LockRequest) -> Result<(), IoError> {
            let operation = match request {
                LockRequest::Exclusive => libc::LOCK_EX | libc::LOCK_NB,
                LockRequest::Shared => libc::LOCK_SH | libc::LOCK_NB,
            };
            // SAFETY: `file` is borrowed for the call; `flock` takes no pointer.
            let result = unsafe { libc::flock(file.raw(), operation) };
            if result != 0 {
                return Err(IoError::new("flock", last_errno()));
            }
            Ok(())
        }

        fn pread(&self, file: &LinuxFile, at: u64, buf: &mut [u8]) -> Result<usize, IoError> {
            let at = offset(at, "pread")?;
            // SAFETY: `buf` is a writable buffer of `buf.len()` bytes that
            // outlives the call; the kernel writes at most that many. `file` is
            // borrowed.
            let read = unsafe { libc::pread(file.raw(), buf.as_mut_ptr().cast(), buf.len(), at) };
            if read < 0 {
                return Err(IoError::new("pread", last_errno()));
            }
            Ok(read as usize)
        }

        fn pwrite(&self, file: &LinuxFile, at: u64, buf: &[u8]) -> Result<usize, IoError> {
            let at = offset(at, "pwrite")?;
            // SAFETY: `buf` is a readable buffer of `buf.len()` bytes that
            // outlives the call; the kernel only reads it. `file` is borrowed.
            let written = unsafe { libc::pwrite(file.raw(), buf.as_ptr().cast(), buf.len(), at) };
            if written < 0 {
                return Err(IoError::new("pwrite", last_errno()));
            }
            Ok(written as usize)
        }

        fn fdatasync(&self, file: &LinuxFile) -> Result<(), IoError> {
            // SAFETY: `file` is borrowed for the call; no pointer.
            let result = unsafe { libc::fdatasync(file.raw()) };
            if result != 0 {
                return Err(IoError::new("fdatasync", last_errno()));
            }
            Ok(())
        }

        fn fsync(&self, file: &LinuxFile) -> Result<(), IoError> {
            // SAFETY: `file` is borrowed for the call; no pointer.
            let result = unsafe { libc::fsync(file.raw()) };
            if result != 0 {
                return Err(IoError::new("fsync", last_errno()));
            }
            Ok(())
        }

        fn touch(&self, file: &LinuxFile) -> Result<(), IoError> {
            // SAFETY: a null `times` pointer is valid for `futimens` (both
            // timestamps set to the current time); `file` is borrowed.
            let result = unsafe { libc::futimens(file.raw(), std::ptr::null()) };
            if result != 0 {
                return Err(IoError::new("futimens", last_errno()));
            }
            Ok(())
        }

        fn random(&self, buf: &mut [u8]) -> Result<(), IoError> {
            let mut done = 0usize;
            while done < buf.len() {
                let rest = &mut buf[done..];
                // SAFETY: `rest` is a writable buffer of `rest.len()` bytes that
                // outlives the call; the kernel writes at most that many.
                let got = unsafe { libc::getrandom(rest.as_mut_ptr().cast(), rest.len(), 0) };
                if got < 0 {
                    let errno = last_errno();
                    if errno == Errno::Intr {
                        continue;
                    }
                    return Err(IoError::new("getrandom", errno));
                }
                done += got as usize;
            }
            Ok(())
        }

        fn create_exclusive(
            &self,
            at: &LinuxDir,
            entry: &str,
            mode: u32,
        ) -> Result<LinuxFile, IoError> {
            let entry = name(entry, "openat2 create")?;
            openat2(
                at.raw(),
                &entry,
                libc::O_RDWR
                    | libc::O_CREAT
                    | libc::O_EXCL
                    | libc::O_NOFOLLOW
                    | libc::O_NOCTTY
                    | libc::O_CLOEXEC,
                mode,
                "openat2 create",
            )
            .map(LinuxFile)
        }

        fn make_dir(&self, at: &LinuxDir, entry: &str, mode: u32) -> Result<(), IoError> {
            let entry = name(entry, "mkdirat")?;
            // SAFETY: `entry` is NUL-terminated and valid for the call; `at` is
            // borrowed.
            let result = unsafe { libc::mkdirat(at.raw(), entry.as_ptr(), mode) };
            if result != 0 {
                return Err(IoError::new("mkdirat", last_errno()));
            }
            Ok(())
        }

        fn link(
            &self,
            from: &LinuxDir,
            from_name: &str,
            to: &LinuxDir,
            to_name: &str,
        ) -> Result<(), IoError> {
            let from_name = name(from_name, "linkat")?;
            let to_name = name(to_name, "linkat")?;
            // SAFETY: both names are NUL-terminated and valid for the call; both
            // directories are borrowed. Flags 0: the source is not followed.
            let result = unsafe {
                libc::linkat(
                    from.raw(),
                    from_name.as_ptr(),
                    to.raw(),
                    to_name.as_ptr(),
                    0,
                )
            };
            if result != 0 {
                return Err(IoError::new("linkat", last_errno()));
            }
            Ok(())
        }

        fn rename(
            &self,
            from: &LinuxDir,
            from_name: &str,
            to: &LinuxDir,
            to_name: &str,
        ) -> Result<(), IoError> {
            let from_name = name(from_name, "renameat")?;
            let to_name = name(to_name, "renameat")?;
            // SAFETY: both names are NUL-terminated and valid for the call; both
            // directories are borrowed.
            let result = unsafe {
                libc::renameat(from.raw(), from_name.as_ptr(), to.raw(), to_name.as_ptr())
            };
            if result != 0 {
                return Err(IoError::new("renameat", last_errno()));
            }
            Ok(())
        }

        fn unlink(&self, at: &LinuxDir, entry: &str) -> Result<(), IoError> {
            let entry = name(entry, "unlinkat")?;
            // SAFETY: `entry` is NUL-terminated and valid for the call; `at` is
            // borrowed. Flags 0: a file entry, never a directory tree.
            let result = unsafe { libc::unlinkat(at.raw(), entry.as_ptr(), 0) };
            if result != 0 {
                return Err(IoError::new("unlinkat", last_errno()));
            }
            Ok(())
        }

        fn remove_dir(&self, at: &LinuxDir, entry: &str) -> Result<(), IoError> {
            let entry = name(entry, "unlinkat")?;
            // SAFETY: `entry` is NUL-terminated and valid for the call; `at` is
            // borrowed. `AT_REMOVEDIR`: one empty directory, never a tree.
            let result = unsafe { libc::unlinkat(at.raw(), entry.as_ptr(), libc::AT_REMOVEDIR) };
            if result != 0 {
                return Err(IoError::new("unlinkat", last_errno()));
            }
            Ok(())
        }

        fn allocate(&self, file: &LinuxFile, len: u64) -> Result<(), IoError> {
            let len = offset(len, "fallocate")?;
            // SAFETY: `file` is borrowed for the call; no pointer.
            let result = unsafe { libc::fallocate(file.raw(), 0, 0, len) };
            if result != 0 {
                return Err(IoError::new("fallocate", last_errno()));
            }
            Ok(())
        }

        fn set_owner(&self, file: &LinuxFile, uid: u32, gid: u32) -> Result<(), IoError> {
            // SAFETY: `file` is borrowed for the call; no pointer.
            let result = unsafe { libc::fchown(file.raw(), uid, gid) };
            if result != 0 {
                return Err(IoError::new("fchown", last_errno()));
            }
            Ok(())
        }
    }
}
