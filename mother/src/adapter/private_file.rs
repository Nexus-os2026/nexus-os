//! P0: the private store in Mother's home (design §3 I2, §5.2): one file
//! per blob, `<dir>/<kind>/<hex>`, directories 0700 and files 0600, each
//! written to a temporary file, synced, renamed into place, and its
//! directory synced. A read is bounded and returned only if its content id
//! verifies. The store refuses a directory that is a link or that other
//! users can enter.

use crate::journal::IoKind;
use crate::private::{ContentId, ContentKind, PrivateError, PrivateStore, MAX_CONTENT_BYTES};
use std::fs::OpenOptions;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

/// Mother's private store on disk.
pub struct FilePrivate {
    dir: PathBuf,
}

fn io(e: std::io::Error) -> PrivateError {
    PrivateError::Io(IoKind::of(e.kind()))
}

/// Sync a directory (its entries durable). Tests see every call and can
/// make one fail ([`seam`]).
fn sync_dir(dir: &Path) -> std::io::Result<()> {
    #[cfg(test)]
    seam::sync(dir)?;
    #[cfg(unix)]
    std::fs::File::open(dir)?.sync_all()?;
    Ok(())
}

/// Tests: a fault seam over [`sync_dir`], per thread. It records every
/// directory sync attempted and can make the sync of one directory fail;
/// it changes nothing else.
#[cfg(test)]
pub(crate) mod seam {
    use std::cell::RefCell;
    use std::path::{Path, PathBuf};

    thread_local! {
        static SYNCED: RefCell<Vec<PathBuf>> = const { RefCell::new(Vec::new()) };
        static FAIL: RefCell<Option<PathBuf>> = const { RefCell::new(None) };
    }

    /// Every directory sync this thread attempted since the last take, in
    /// order.
    pub(crate) fn take_synced() -> Vec<PathBuf> {
        SYNCED.with(|s| std::mem::take(&mut *s.borrow_mut()))
    }

    /// Every later sync of `dir` on this thread fails, until cleared.
    pub(crate) fn fail_sync_of(dir: Option<PathBuf>) {
        FAIL.with(|f| *f.borrow_mut() = dir);
    }

    pub(super) fn sync(dir: &Path) -> std::io::Result<()> {
        SYNCED.with(|s| s.borrow_mut().push(dir.to_path_buf()));
        if FAIL.with(|f| f.borrow().as_deref() == Some(dir)) {
            return Err(std::io::Error::other("injected directory sync failure"));
        }
        Ok(())
    }
}

/// `dir` exists (created 0700 if absent), is a directory and not a link,
/// and (on Unix) only this user may enter it.
fn private_dir(dir: &Path) -> Result<(), PrivateError> {
    match std::fs::symlink_metadata(dir) {
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            super::create_private(dir).map_err(io)?;
        }
        Err(e) => return Err(io(e)),
    }
    let meta = std::fs::symlink_metadata(dir).map_err(io)?;
    if !meta.is_dir() {
        return Err(PrivateError::NotPrivate);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if meta.permissions().mode() & 0o077 != 0 {
            return Err(PrivateError::NotPrivate);
        }
    }
    Ok(())
}

static TEMP: AtomicU64 = AtomicU64::new(0);

impl FilePrivate {
    /// The store at `dir` (created private if absent).
    pub fn open(dir: &Path) -> Result<Self, PrivateError> {
        private_dir(dir)?;
        Ok(Self {
            dir: dir.to_path_buf(),
        })
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    fn write(dir: &Path, temp: &Path, target: &Path, bytes: &[u8]) -> std::io::Result<()> {
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(temp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        std::fs::rename(temp, target)?;
        sync_dir(dir)?;
        Ok(())
    }
}

impl PrivateStore for FilePrivate {
    fn put(&self, kind: ContentKind, bytes: &[u8]) -> Result<ContentId, PrivateError> {
        if bytes.len() as u64 > MAX_CONTENT_BYTES {
            return Err(PrivateError::TooLarge);
        }
        let id = ContentId::of(kind, bytes);
        let dir = self.dir.join(kind.as_str());
        private_dir(&dir)?;
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        let temp = dir.join(format!(
            ".tmp-{}-{nanos}-{}",
            id.to_hex(),
            TEMP.fetch_add(1, Ordering::SeqCst)
        ));
        let target = dir.join(id.to_hex());
        let written = Self::write(&dir, &temp, &target, bytes);
        if written.is_err() {
            let _ = std::fs::remove_file(&temp);
        }
        written.map_err(io)?;
        Ok(id)
    }

    fn get(&self, kind: ContentKind, id: &ContentId) -> Result<Vec<u8>, PrivateError> {
        let path = self.dir.join(kind.as_str()).join(id.to_hex());
        let meta = match std::fs::symlink_metadata(&path) {
            Ok(meta) => meta,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Err(PrivateError::Missing)
            }
            Err(e) => return Err(io(e)),
        };
        if !meta.is_file() {
            return Err(PrivateError::NotPrivate);
        }
        let file = std::fs::File::open(&path).map_err(io)?;
        let mut bytes = Vec::new();
        file.take(MAX_CONTENT_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(io)?;
        if bytes.len() as u64 > MAX_CONTENT_BYTES {
            return Err(PrivateError::TooLarge);
        }
        if ContentId::of(kind, &bytes) != *id {
            return Err(PrivateError::Mismatch);
        }
        Ok(bytes)
    }
}
