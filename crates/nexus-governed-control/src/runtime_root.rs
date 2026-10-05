//! The runtime root: the one directory Phase Three actuators own on disk.
//!
//! Every on-disk thing an effect needs (a tool's working directory, a
//! browser profile, an agent display's authorization file) lives in a
//! scratch directory under this root: private to the user (0700), named
//! randomly, and removed when the effect or the session that owns it ends.
//! The desktop chooses the root; nothing here falls back to a home, a
//! working directory or a shared temporary directory.

use crate::authority::AuthorityError;
use std::path::{Path, PathBuf};

/// The owned root.
#[derive(Clone, Debug)]
pub struct RuntimeRoot {
    root: PathBuf,
}

impl RuntimeRoot {
    /// Create (if needed) and take `path` as the root: absolute, a real
    /// directory, ours (we can make it 0700), canonical.
    pub fn open(path: &Path) -> Result<Self, AuthorityError> {
        if !path.is_absolute() {
            return Err(AuthorityError::InvalidAction(
                "the runtime root is not absolute",
            ));
        }
        std::fs::create_dir_all(path)
            .map_err(|_| AuthorityError::Unavailable("the runtime root cannot be created"))?;
        let meta = std::fs::symlink_metadata(path)
            .map_err(|_| AuthorityError::Unavailable("the runtime root cannot be read"))?;
        if !meta.is_dir() {
            return Err(AuthorityError::Unavailable(
                "the runtime root is not a directory",
            ));
        }
        private(path)?;
        let root = std::fs::canonicalize(path)
            .map_err(|_| AuthorityError::Unavailable("the runtime root cannot be resolved"))?;
        Ok(Self { root })
    }

    pub fn path(&self) -> &Path {
        &self.root
    }

    /// A fresh private directory `<root>/<label>-<random>`, removed on drop.
    pub(crate) fn scratch(&self, label: &str) -> Result<Scratch, AuthorityError> {
        let mut bytes = [0u8; 12];
        getrandom::getrandom(&mut bytes)
            .map_err(|_| AuthorityError::Unavailable("no randomness for a scratch directory"))?;
        let path = self.root.join(format!("{label}-{}", hex::encode(bytes)));
        std::fs::create_dir(&path)
            .map_err(|_| AuthorityError::Unavailable("a scratch directory cannot be created"))?;
        let scratch = Scratch { path };
        private(&scratch.path)?;
        Ok(scratch)
    }
}

#[cfg(unix)]
fn private(path: &Path) -> Result<(), AuthorityError> {
    use std::os::unix::fs::PermissionsExt;
    // Only the owner can change the mode: this also proves the directory
    // is ours.
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))
        .map_err(|_| AuthorityError::Unavailable("a runtime directory is not ours"))
}

#[cfg(not(unix))]
fn private(_path: &Path) -> Result<(), AuthorityError> {
    Err(AuthorityError::Unavailable(
        "governed real-world control is available on Linux only",
    ))
}

/// A private scratch directory, removed with everything in it on drop.
#[derive(Debug)]
pub(crate) struct Scratch {
    path: PathBuf,
}

impl Scratch {
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// A private subdirectory, canonical.
    pub fn subdir(&self, name: &str) -> Result<PathBuf, AuthorityError> {
        let path = self.path.join(name);
        std::fs::create_dir(&path)
            .map_err(|_| AuthorityError::Unavailable("a scratch subdirectory cannot be created"))?;
        private(&path)?;
        std::fs::canonicalize(&path)
            .map_err(|_| AuthorityError::Unavailable("a scratch subdirectory cannot be resolved"))
    }

    /// Write a new private file (it must not exist).
    pub fn write_new(&self, name: &str, bytes: &[u8]) -> Result<PathBuf, AuthorityError> {
        use std::io::Write;
        let path = self.path.join(name);
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options
            .open(&path)
            .map_err(|_| AuthorityError::Unavailable("a scratch file cannot be created"))?;
        file.write_all(bytes)
            .and_then(|()| file.sync_all())
            .map_err(|_| AuthorityError::Unavailable("a scratch file cannot be written"))?;
        Ok(path)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}
