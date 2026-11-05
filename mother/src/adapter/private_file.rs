//! P0: the private store in Mother's home (design §3 I2, §5.2): one file
//! per blob, `<dir>/<kind>/<hex>`, directories 0700 and files 0600, each
//! written to a temporary file, synced, renamed into place, and its
//! directory synced.
//!
//! Durability, by one stateless rule (Architect decision after Codex
//! review 3; it replaces an in-memory record of owed links, which a
//! concurrent opener could race and another spelling of a path could
//! miss). At open the store's directory is made canonical, and a fixed
//! chain is taken: that directory, each directory above it up to Mother's
//! home root (the top of the unbroken run of private directories, 0700
//! or stricter and of the store's owner: every directory Mother creates
//! is one), and that root's parent. Every put, after its blob's rename,
//! syncs every directory of the chain, from the top down, and then the
//! kind directory. The chain stops only at a verified boundary (a
//! directory read successfully and not private to the store's owner); a
//! level whose metadata cannot be read fails the open. Whoever created a directory, and however the path was
//! spelled, its link is durable before the put returns. Nothing about
//! durability is remembered between calls or shared between stores. A
//! missing level is created on its own (0700) and synced with its parent
//! as it is created. Any failure fails the call, so no reference to the
//! content is journaled; no lock is held across a sync.
//!
//! Cost: a put syncs the chain's directories (the store's, those above it
//! that are private to Mother, and one more) on top of the blob and its
//! directory: a few directory syncs per put, accepted. Mother puts content
//! only when a mission is drafted, revised, or pauses for the Owner.
//!
//! Privacy: every put checks the store's directory and the kind directory
//! (a directory, not a link, no access for group or others, the store's
//! owner) before it writes. A read is bounded and returned only if its
//! content id verifies.
//!
//! Non-claim: the checks and the write go by path, so someone with write
//! access to Mother's home who swaps a directory between the check and the
//! write is not stopped (design §4: out of scope).

use crate::journal::IoKind;
use crate::private::{ContentId, ContentKind, PrivateError, PrivateStore, MAX_CONTENT_BYTES};
use nexus_governed_control::host_session::{MotherStep, PrivateObligation, SessionContext};
use std::fs::OpenOptions;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

enum PrivateMode {
    Legacy,
    Session(SessionContext),
}
impl PrivateMode {
    fn reserve(
        &self,
        temporary: &Path,
        retained: Option<&Path>,
    ) -> std::io::Result<Option<PrivateObligation>> {
        match self {
            Self::Legacy => Ok(None),
            Self::Session(context) => context
                .reserve_private(temporary, retained)
                .map(Some)
                .map_err(|_| std::io::Error::other("session private inventory unavailable")),
        }
    }
    fn admit(&self, step: MotherStep) -> std::io::Result<()> {
        match self {
            Self::Legacy => Ok(()),
            Self::Session(context) => context.admit(step).map_err(|_| {
                std::io::Error::new(
                    std::io::ErrorKind::PermissionDenied,
                    "session private sink refused",
                )
            }),
        }
    }
}

/// Mother's private store on disk.
pub struct FilePrivate {
    mode: PrivateMode,
    /// The store's directory, canonical.
    dir: PathBuf,
    /// The owner of the store's directory (Unix), which every directory
    /// the store writes in must have.
    owner: u32,
    /// Fixed at open, canonical, bottom up: the store's directory, each
    /// directory above it up to Mother's home root, and that root's
    /// parent. Every put syncs all of it.
    chain: Vec<PathBuf>,
}

fn io(e: std::io::Error) -> PrivateError {
    PrivateError::Io(IoKind::of(e.kind()))
}

/// Sync a directory (its entries durable). Tests see every call and can
/// make one fail ([`seam`]).
fn sync_dir(dir: &Path, mode: &PrivateMode) -> std::io::Result<()> {
    #[cfg(test)]
    session_probe::before("private_parent_sync");
    #[cfg(test)]
    seam::sync(dir)?;
    #[cfg(unix)]
    let directory = std::fs::File::open(dir)?;
    mode.admit(MotherStep::PrivateParentSync)?;
    #[cfg(test)]
    session_probe::entered("private_parent_sync");
    #[cfg(unix)]
    directory.sync_all()?;
    #[cfg(test)]
    session_probe::after("private_parent_sync");
    Ok(())
}

/// A directory's metadata (no link followed). Tests can make the read of
/// one directory fail ([`seam`]).
fn metadata(dir: &Path) -> std::io::Result<std::fs::Metadata> {
    #[cfg(test)]
    seam::metadata(dir)?;
    std::fs::symlink_metadata(dir)
}

/// A directory's parent (`.` for a single relative component).
fn parent_of(dir: &Path) -> &Path {
    match dir.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent,
        _ => Path::new("."),
    }
}

/// Create every missing level of `dir`, each on its own (0700), each
/// synced with its parent as it is created. Every failure is returned.
fn create_levels(dir: &Path, mode: &PrivateMode) -> Result<(), PrivateError> {
    let mut missing = Vec::new();
    let mut at = dir;
    loop {
        match std::fs::symlink_metadata(at) {
            Ok(_) => break,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                missing.push(at.to_path_buf());
                match at.parent() {
                    Some(parent) if !parent.as_os_str().is_empty() => at = parent,
                    _ => break,
                }
            }
            Err(e) => return Err(io(e)),
        }
    }
    for level in missing.iter().rev() {
        let obligation = mode.reserve(level, Some(level)).map_err(io)?;
        let mut builder = std::fs::DirBuilder::new();
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        #[cfg(test)]
        session_probe::before("private_mkdir");
        if let Err(error) = mode.admit(MotherStep::PrivateDirectory) {
            if let Some(obligation) = obligation {
                obligation.complete();
            }
            return Err(io(error));
        }
        #[cfg(test)]
        session_probe::entered("private_mkdir");
        match builder.create(level) {
            Ok(()) => {}
            // Made meanwhile by another opener: synced here too.
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(io(e)),
        }
        if let Some(obligation) = &obligation {
            obligation
                .observe_created(std::fs::symlink_metadata(level).map_err(io)?)
                .map_err(|_| PrivateError::Io(IoKind::Other))?;
        }
        #[cfg(test)]
        seam::created(level);
        #[cfg(test)]
        session_probe::after("private_mkdir");
        sync_dir(level, mode).map_err(io)?;
        sync_dir(parent_of(level), mode).map_err(io)?;
        if let Some(obligation) = obligation {
            obligation.complete();
        }
    }
    Ok(())
}

/// `dir` is a directory, not a link, and (Unix) no one but its owner may
/// enter it, and its owner is `owner` (if given). Its owner.
fn private_dir(dir: &Path, owner: Option<u32>) -> Result<u32, PrivateError> {
    let meta = metadata(dir).map_err(io)?;
    if !meta.is_dir() {
        return Err(PrivateError::NotPrivate);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        if meta.permissions().mode() & 0o077 != 0 {
            return Err(PrivateError::NotPrivate);
        }
        if owner.is_some_and(|owner| owner != meta.uid()) {
            return Err(PrivateError::NotPrivate);
        }
        Ok(meta.uid())
    }
    #[cfg(not(unix))]
    {
        Ok(owner.unwrap_or(0))
    }
}

/// Whether `dir`, read successfully, is a private directory of `owner`
/// (0700 or stricter). `Ok(false)` only for a verified boundary: a
/// directory that exists, was read, and is not private to `owner`. A read
/// that fails, or anything but a directory, is an error.
fn private_and_owned(dir: &Path, owner: u32) -> Result<bool, PrivateError> {
    let meta = metadata(dir).map_err(io)?;
    if !meta.is_dir() {
        return Err(PrivateError::NotPrivate);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        Ok(meta.permissions().mode() & 0o077 == 0 && meta.uid() == owner)
    }
    #[cfg(not(unix))]
    {
        let _ = owner;
        Ok(false)
    }
}

/// The chain above the canonical store directory `dir`: `dir`, each
/// directory above it while that directory is private and `owner`'s
/// (Mother's home root is the last), and then the next one up (that
/// root's parent, a verified boundary), unless the root is the file
/// system's root. Every read error is returned: the chain is never cut
/// short by a level that could not be read.
fn chain_of(dir: &Path, owner: u32) -> Result<Vec<PathBuf>, PrivateError> {
    let mut chain = vec![dir.to_path_buf()];
    let mut at = dir;
    while let Some(parent) = at.parent() {
        chain.push(parent.to_path_buf());
        if !private_and_owned(parent, owner)? {
            break;
        }
        at = parent;
    }
    Ok(chain)
}

static TEMP: AtomicU64 = AtomicU64::new(0);

#[derive(Default)]
struct WriteState {
    creation_entered: bool,
    created: bool,
    published: bool,
    parent: Option<std::fs::Metadata>,
    temporary: Option<std::fs::Metadata>,
}

fn same_resource(before: Option<&std::fs::Metadata>, path: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        before.is_some_and(|before| {
            std::fs::symlink_metadata(path).is_ok_and(|now| {
                (before.dev(), before.ino(), before.file_type())
                    == (now.dev(), now.ino(), now.file_type())
            })
        })
    }
    #[cfg(not(unix))]
    {
        let _ = (before, path);
        false
    }
}

#[path = "private_session_oracles_tests.rs"]
#[cfg(test)]
mod session_probe;

impl FilePrivate {
    /// The store at `dir` (created private if absent), its chain synced.
    pub fn open(dir: &Path) -> Result<Self, PrivateError> {
        Self::open_mode(dir, PrivateMode::Legacy)
    }

    pub fn open_session(dir: &Path, context: SessionContext) -> Result<Self, PrivateError> {
        Self::open_mode(dir, PrivateMode::Session(context))
    }

    fn open_mode(dir: &Path, mode: PrivateMode) -> Result<Self, PrivateError> {
        create_levels(dir, &mode)?;
        // The path as given: a link is refused before it is resolved.
        let owner = private_dir(dir, None)?;
        let dir = std::fs::canonicalize(dir).map_err(io)?;
        private_dir(&dir, Some(owner))?;
        let chain = chain_of(&dir, owner)?;
        for level in chain.iter().rev() {
            sync_dir(level, &mode).map_err(io)?;
        }
        Ok(Self {
            dir,
            owner,
            chain,
            mode,
        })
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// The kind's directory, created if missing, and checked (with the
    /// store's directory) on every call.
    fn kind_dir(&self, kind: ContentKind) -> Result<PathBuf, PrivateError> {
        private_dir(&self.dir, Some(self.owner))?;
        let dir = self.dir.join(kind.as_str());
        create_levels(&dir, &self.mode)?;
        private_dir(&dir, Some(self.owner))?;
        Ok(dir)
    }

    /// The blob: temp, fsync, rename; then the chain, from the top down,
    /// and the kind directory last.
    fn write(
        &self,
        dir: &Path,
        temp: &Path,
        target: &Path,
        bytes: &[u8],
        obligation: Option<&PrivateObligation>,
        state: &mut WriteState,
    ) -> std::io::Result<()> {
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        if obligation.is_some() {
            state.parent = Some(std::fs::symlink_metadata(dir)?);
        }
        #[cfg(test)]
        session_probe::before("private_temp_create");
        self.mode.admit(MotherStep::PrivateTemporary)?;
        #[cfg(test)]
        session_probe::entered("private_temp_create");
        state.creation_entered = true;
        let mut file = options.open(temp)?;
        state.created = true;
        if let Some(obligation) = obligation {
            let metadata = file.metadata()?;
            state.temporary = Some(metadata.clone());
            obligation
                .observe_created(metadata)
                .map_err(|_| std::io::Error::other("private inventory identity unavailable"))?;
        }
        #[cfg(test)]
        session_probe::after("private_temp_create");
        #[cfg(test)]
        session_probe::before("private_write");
        self.mode.admit(MotherStep::PrivateWrite)?;
        #[cfg(test)]
        session_probe::entered("private_write");
        #[cfg(test)]
        session_probe::write_result()?;
        file.write_all(bytes)?;
        #[cfg(test)]
        session_probe::after("private_write");
        #[cfg(test)]
        session_probe::before("private_sync");
        self.mode.admit(MotherStep::PrivateSync)?;
        #[cfg(test)]
        session_probe::entered("private_sync");
        file.sync_all()?;
        #[cfg(test)]
        session_probe::after("private_sync");
        drop(file);
        #[cfg(test)]
        session_probe::before("private_rename");
        self.mode.admit(MotherStep::PrivateRename)?;
        #[cfg(test)]
        session_probe::entered("private_rename");
        std::fs::rename(temp, target)?;
        state.published = true;
        #[cfg(test)]
        session_probe::after("private_rename");
        for level in self.chain.iter().rev() {
            sync_dir(level, &self.mode)?;
        }
        sync_dir(dir, &self.mode)?;
        Ok(())
    }
}

impl PrivateStore for FilePrivate {
    fn put(&self, kind: ContentKind, bytes: &[u8]) -> Result<ContentId, PrivateError> {
        if bytes.len() as u64 > MAX_CONTENT_BYTES {
            return Err(PrivateError::TooLarge);
        }
        let id = ContentId::of(kind, bytes);
        let dir = self.kind_dir(kind)?;
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        let temp = dir.join(format!(
            ".tmp-{}-{nanos}-{}",
            id.to_hex(),
            TEMP.fetch_add(1, Ordering::SeqCst)
        ));
        let target = dir.join(id.to_hex());
        let obligation = self.mode.reserve(&temp, Some(&target)).map_err(io)?;
        let mut state = WriteState::default();
        let written = self.write(&dir, &temp, &target, bytes, obligation.as_ref(), &mut state);
        let mut disposed = !state.creation_entered;
        if written.is_err() {
            let owned = matches!(self.mode, PrivateMode::Legacy)
                || (state.created
                    && !state.published
                    && same_resource(state.parent.as_ref(), &dir)
                    && same_resource(state.temporary.as_ref(), &temp));
            #[cfg(test)]
            session_probe::before("private_error_remove");
            if owned && self.mode.admit(MotherStep::PrivateErrorRemoval).is_ok() {
                #[cfg(test)]
                session_probe::entered("private_error_remove");
                let removed = std::fs::remove_file(&temp);
                #[cfg(test)]
                session_probe::after("private_error_remove");
                if matches!(self.mode, PrivateMode::Session(_)) {
                    // Rename may already have published an intentionally
                    // retained blob. Removing its old temporary name then
                    // proves nothing about the failed final synchronization.
                    if state.created && !state.published && removed.is_ok() {
                        disposed = matches!(std::fs::symlink_metadata(&temp), Err(error) if error.kind() == std::io::ErrorKind::NotFound)
                            && same_resource(state.parent.as_ref(), &dir)
                            && sync_dir(&dir, &self.mode).is_ok()
                            && same_resource(state.parent.as_ref(), &dir);
                    }
                }
            }
        }
        if written.is_ok() || disposed {
            if let Some(obligation) = obligation {
                obligation.complete();
            }
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
        static FAIL_METADATA: RefCell<Option<PathBuf>> = const { RefCell::new(None) };
    }

    /// Every later metadata read of `dir` on this thread fails, until
    /// cleared.
    pub(crate) fn fail_metadata_of(dir: Option<PathBuf>) {
        FAIL_METADATA.with(|f| *f.borrow_mut() = dir);
    }

    pub(super) fn metadata(dir: &Path) -> std::io::Result<()> {
        if FAIL_METADATA.with(|f| f.borrow().as_deref() == Some(dir)) {
            return Err(std::io::Error::other("injected metadata failure"));
        }
        Ok(())
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

    /// A process-wide pause point: the thread that creates the armed
    /// directory stops just after creating it, until released (bounded).
    struct Pause {
        armed: Option<PathBuf>,
        paused: bool,
        released: bool,
    }

    static PAUSE: std::sync::Mutex<Pause> = std::sync::Mutex::new(Pause {
        armed: None,
        paused: false,
        released: false,
    });
    static CHANGED: std::sync::Condvar = std::sync::Condvar::new();
    const BOUND: std::time::Duration = std::time::Duration::from_secs(10);

    fn pause() -> std::sync::MutexGuard<'static, Pause> {
        PAUSE.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// One scenario at a time arms the pause point: each holds this for its
    /// whole run, so parallel tests never arm or release each other's.
    static SCENARIO: std::sync::Mutex<()> = std::sync::Mutex::new(());

    /// Take the pause point for a whole scenario.
    pub(crate) fn pause_scenario() -> std::sync::MutexGuard<'static, ()> {
        SCENARIO.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// The next thread to create `dir` pauses just after creating it.
    pub(crate) fn pause_after_creating(dir: Option<PathBuf>) {
        let mut p = pause();
        p.armed = dir;
        p.paused = false;
        p.released = false;
    }

    /// Whether a thread is paused there (waits for one, bounded).
    pub(crate) fn wait_paused() -> bool {
        let p = pause();
        let (p, _) = CHANGED
            .wait_timeout_while(p, BOUND, |p| !p.paused)
            .unwrap_or_else(|e| e.into_inner());
        p.paused
    }

    /// Let the paused thread go on, and disarm.
    pub(crate) fn release() {
        let mut p = pause();
        p.released = true;
        p.armed = None;
        CHANGED.notify_all();
    }

    pub(super) fn created(level: &Path) {
        let mut p = pause();
        if p.armed.as_deref() != Some(level) {
            return;
        }
        p.paused = true;
        CHANGED.notify_all();
        let _ = CHANGED
            .wait_timeout_while(p, BOUND, |p| !p.released)
            .unwrap_or_else(|e| e.into_inner());
    }

    pub(super) fn sync(dir: &Path) -> std::io::Result<()> {
        SYNCED.with(|s| s.borrow_mut().push(dir.to_path_buf()));
        if FAIL.with(|f| f.borrow().as_deref() == Some(dir)) {
            return Err(std::io::Error::other("injected directory sync failure"));
        }
        Ok(())
    }
}
