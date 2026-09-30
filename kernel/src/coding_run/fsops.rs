//! Descriptor-anchored filesystem operations for coding runs (Linux).
//!
//! Every operation starts from an open directory handle, never from a path
//! string. A directory is opened with `O_DIRECTORY | O_NOFOLLOW`; its children
//! are reached through the handle's `/proc/self/fd/<fd>` entry, which the
//! kernel resolves to the open directory itself, not by re-walking a path.
//! Final components are never followed:
//!
//! - `lstat` (`symlink_metadata`) classifies an entry without following it;
//! - files are opened with `O_NOFOLLOW` (and `O_NONBLOCK`, so a FIFO swapped
//!   in cannot block), then re-checked with `fstat` on the open descriptor;
//! - new files are created with `O_CREAT | O_EXCL | O_NOFOLLOW`;
//! - replacement is a `rename` inside one directory handle; creation publishes
//!   with `link` (which never replaces) and then removes the temporary name.
//!
//! Each handle checks, when opened, that its `/proc/self/fd` entry resolves to
//! the same device and inode as the descriptor, so a missing or substituted
//! procfs fails closed. Entries on another device than the root (a mount point
//! inside a project) and regular files with more than one hard link are
//! refused. This is containment against same-user filesystem races on the
//! Linux support profile; it is not a mount-namespace or hostile-root claim.

use std::fs::{File, Metadata, OpenOptions};
use std::io::{self, Read, Write};
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt};
use std::os::unix::io::AsRawFd;
use std::path::{Path, PathBuf};

use nix::libc;

use super::scope::validate_component;

/// Device and inode of a filesystem object.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct NodeIdentity {
    pub dev: u64,
    pub ino: u64,
}

impl NodeIdentity {
    fn of(metadata: &Metadata) -> Self {
        Self {
            dev: metadata.dev(),
            ino: metadata.ino(),
        }
    }
}

/// What an entry is, classified without following it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EntryKind {
    Directory,
    Regular,
    Symlink,
    Special,
    Missing,
}

#[cfg(test)]
thread_local! {
    /// Test hook run just before an identity-bound subdirectory removal
    /// during recursive cleanup, with the parent's anchor and the entry name.
    #[allow(clippy::type_complexity)]
    pub(crate) static BEFORE_IDENTITY_REMOVAL: std::cell::RefCell<Option<Box<dyn FnMut(&Path, &str)>>> =
        const { std::cell::RefCell::new(None) };
}

/// An open directory with a retained identity.
#[derive(Debug)]
pub(crate) struct DirHandle {
    file: File,
    identity: NodeIdentity,
}

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}

impl DirHandle {
    /// Open an absolute directory path, refusing a symlink at the final
    /// component. Used only for backend-resolved grant roots.
    pub(crate) fn open_absolute(path: &Path) -> io::Result<Self> {
        if !path.is_absolute() {
            return Err(invalid("directory path must be absolute"));
        }
        let file = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(path)?;
        Self::from_directory_file(file)
    }

    fn from_directory_file(file: File) -> io::Result<Self> {
        let metadata = file.metadata()?;
        if !metadata.is_dir() {
            return Err(invalid("not a directory"));
        }
        let identity = NodeIdentity::of(&metadata);
        let handle = Self { file, identity };
        // The /proc anchor must resolve to this very directory.
        let anchored = std::fs::metadata(handle.anchor())?;
        if NodeIdentity::of(&anchored) != identity {
            return Err(io::Error::other("procfs anchoring unavailable"));
        }
        Ok(handle)
    }

    pub(crate) fn identity(&self) -> NodeIdentity {
        self.identity
    }

    /// A second handle on the same open directory (a duplicated descriptor,
    /// not a path re-open), so an owner can outlive the original handle.
    pub(crate) fn try_clone(&self) -> io::Result<DirHandle> {
        Ok(Self {
            file: self.file.try_clone()?,
            identity: self.identity,
        })
    }

    fn anchor(&self) -> PathBuf {
        PathBuf::from(format!("/proc/self/fd/{}", self.file.as_raw_fd()))
    }

    fn child(&self, name: &str) -> io::Result<PathBuf> {
        validate_component(name).map_err(|_| invalid("invalid entry name"))?;
        Ok(self.anchor().join(name))
    }

    /// Temporary names bypass the project-name rules on purpose.
    fn temp_child(&self, name: &str) -> PathBuf {
        self.anchor().join(name)
    }

    /// Classify an entry without following it.
    pub(crate) fn kind(&self, name: &str) -> io::Result<EntryKind> {
        match std::fs::symlink_metadata(self.child(name)?) {
            Ok(metadata) => Ok(classify(&metadata)),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(EntryKind::Missing),
            Err(error) => Err(error),
        }
    }

    /// Open a subdirectory; refuses symlinks and other devices.
    pub(crate) fn open_subdir(&self, name: &str) -> io::Result<DirHandle> {
        let file = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(self.child(name)?)?;
        let handle = Self::from_directory_file(file)?;
        if handle.identity.dev != self.identity.dev {
            return Err(io::Error::other("directory is on another device"));
        }
        Ok(handle)
    }

    /// Create a subdirectory (never following an existing entry).
    pub(crate) fn make_subdir(&self, name: &str) -> io::Result<()> {
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(self.child(name)?)
    }

    /// Read a regular file completely, bounded by `max_bytes`. The open
    /// descriptor must be a regular file with exactly one link on this
    /// directory's device, and its size must not change while reading.
    pub(crate) fn read_regular(&self, name: &str, max_bytes: u64) -> io::Result<Vec<u8>> {
        let mut file = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC)
            .open(self.child(name)?)?;
        let metadata = file.metadata()?;
        self.check_regular(&metadata)?;
        if metadata.len() > max_bytes {
            return Err(io::Error::other("file exceeds the size cap"));
        }
        let mut content = Vec::with_capacity(metadata.len() as usize);
        (&mut file).take(max_bytes + 1).read_to_end(&mut content)?;
        let after = file.metadata()?;
        if content.len() as u64 != metadata.len() || after.len() != metadata.len() {
            return Err(io::Error::other("file changed while reading"));
        }
        Ok(content)
    }

    fn check_regular(&self, metadata: &Metadata) -> io::Result<()> {
        if !metadata.file_type().is_file() {
            return Err(io::Error::other("not a regular file"));
        }
        if metadata.nlink() != 1 {
            return Err(io::Error::other("regular file has more than one link"));
        }
        if metadata.dev() != self.identity.dev {
            return Err(io::Error::other("file is on another device"));
        }
        Ok(())
    }

    /// Metadata of an entry without following it, for classification.
    pub(crate) fn lstat(&self, name: &str) -> Option<Metadata> {
        std::fs::symlink_metadata(self.child(name).ok()?).ok()
    }

    /// Metadata of a regular file entry, checked like [`Self::read_regular`].
    pub(crate) fn regular_metadata(&self, name: &str) -> io::Result<Metadata> {
        let metadata = std::fs::symlink_metadata(self.child(name)?)?;
        self.check_regular(&metadata)?;
        Ok(metadata)
    }

    /// Write `content` to a new exclusive temporary file in this directory
    /// and fsync it. Returns the temporary name.
    fn write_temp(&self, content: &[u8]) -> io::Result<String> {
        let name = format!("{}{}", super::scope::TEMP_PREFIX, uuid::Uuid::new_v4());
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(self.temp_child(&name))?;
        let result = file.write_all(content).and_then(|()| file.sync_all());
        if let Err(error) = result {
            let _ = std::fs::remove_file(self.temp_child(&name));
            return Err(error);
        }
        Ok(name)
    }

    /// Create a new file, failing if the name exists in any form.
    pub(crate) fn create_file(&self, name: &str, content: &[u8]) -> io::Result<()> {
        let target = self.child(name)?;
        let temp = self.write_temp(content)?;
        // link(2) never replaces an existing entry, including a symlink.
        let linked = std::fs::hard_link(self.temp_child(&temp), &target);
        let removed = std::fs::remove_file(self.temp_child(&temp));
        linked?;
        removed?;
        self.sync()
    }

    /// Atomically replace an existing regular file with new content.
    pub(crate) fn replace_file(&self, name: &str, content: &[u8]) -> io::Result<()> {
        let target = self.child(name)?;
        self.regular_metadata(name)?;
        let temp = self.write_temp(content)?;
        if let Err(error) = std::fs::rename(self.temp_child(&temp), &target) {
            let _ = std::fs::remove_file(self.temp_child(&temp));
            return Err(error);
        }
        self.sync()
    }

    /// Write a snapshot file into a fresh directory (exclusive create).
    pub(crate) fn create_snapshot_file(&self, name: &str, content: &[u8]) -> io::Result<()> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(self.child(name)?)?;
        file.write_all(content)?;
        file.sync_all()
    }

    /// Entry names, sorted. A name that is not UTF-8 is an error.
    pub(crate) fn entries(&self) -> io::Result<Vec<String>> {
        let mut names = Vec::new();
        for entry in std::fs::read_dir(self.anchor())? {
            let entry = entry?;
            let name = entry
                .file_name()
                .into_string()
                .map_err(|_| io::Error::other("entry name is not UTF-8"))?;
            names.push(name);
        }
        names.sort();
        Ok(names)
    }

    /// Remove every entry beneath this directory without following links. A
    /// subdirectory is opened, emptied through its own handle, and removed
    /// only if the name still names that same directory (see
    /// [`Self::remove_subdir_if_identity`]); the handle stays open until then.
    pub(crate) fn remove_all_entries(&self) -> io::Result<()> {
        for name in self.entries()? {
            let path = self.temp_child(&name);
            let metadata = std::fs::symlink_metadata(&path)?;
            if metadata.is_dir() {
                let file = OpenOptions::new()
                    .read(true)
                    .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
                    .open(&path)?;
                let child = Self::from_directory_file(file)?;
                child.remove_all_entries()?;
                #[cfg(test)]
                BEFORE_IDENTITY_REMOVAL.with(|hook| {
                    if let Some(hook) = hook.borrow_mut().as_mut() {
                        hook(&self.anchor(), &name);
                    }
                });
                self.remove_subdir_if_identity(&name, child.identity())?;
                drop(child);
            } else {
                std::fs::remove_file(&path)?;
            }
        }
        self.sync()
    }

    /// Remove the empty subdirectory `name` only if it is still the directory
    /// with identity `expected`. The entry is classified through this retained
    /// parent without following it; a missing entry, a symlink, a special file
    /// or a directory with another device/inode is refused and nothing is
    /// removed. The parent is fsynced after a removal.
    ///
    /// Not a claim: without `unsafe` or a new dependency there is no atomic
    /// "remove if inode" system call here, so a same-user process racing a
    /// rename into the instant between the identity check and `rmdir` is not
    /// excluded. A stale or replaced name is never removed deterministically.
    pub(crate) fn remove_subdir_if_identity(
        &self,
        name: &str,
        expected: NodeIdentity,
    ) -> io::Result<()> {
        let path = self.temp_child(name);
        let metadata = std::fs::symlink_metadata(&path)?;
        if !metadata.file_type().is_dir() {
            return Err(io::Error::other("entry is not the retained directory"));
        }
        if NodeIdentity::of(&metadata) != expected {
            return Err(io::Error::other("directory identity changed"));
        }
        std::fs::remove_dir(&path)?;
        self.sync()
    }

    /// Open the direct child directory `name`, creating it (owner-only) if it
    /// is missing. An existing symlink, file or special entry is refused, as
    /// is a directory on another device.
    pub(crate) fn open_or_create_subdir(&self, name: &str) -> io::Result<DirHandle> {
        match self.kind(name)? {
            EntryKind::Directory => {}
            EntryKind::Missing => self.make_subdir(name)?,
            _ => return Err(io::Error::other("entry is not a directory")),
        }
        self.open_subdir(name)
    }

    /// fsync the directory.
    pub(crate) fn sync(&self) -> io::Result<()> {
        self.file.sync_all()
    }
}

pub(crate) fn classify(metadata: &Metadata) -> EntryKind {
    let file_type = metadata.file_type();
    if file_type.is_symlink() {
        EntryKind::Symlink
    } else if file_type.is_dir() {
        EntryKind::Directory
    } else if file_type.is_file() {
        EntryKind::Regular
    } else {
        EntryKind::Special
    }
}
