//! Opening a store (design sections 6.4, 10 and 13.11): the closed real
//! entry; `PROVISION` selection and post-lock revalidation; safe opening;
//! the mount, effective-profile, kernel and storage checks of section 6.4
//! steps 1 to 12; the opening-bound [`StorageAdmission`] and its one
//! constructor; durable activation (A1 to A5); the scan of steps 4 to 8; the
//! decision; and the claim handoff (section 10.3).
//!
//! Every algorithm here is written against a [`Platform`]: the store's I/O
//! and a host view. In this mission only the simulator implements
//! [`Platform`], so the whole opening algorithm runs, through the same code,
//! against simulated storage and a fixture host only. The real configured
//! store is never opened: [`open_configured_store`] refuses before it
//! touches anything, and no environment variable, configuration value,
//! feature, serialized approval or caller argument changes that.

use std::collections::{BTreeMap, BTreeSet};
use std::convert::Infallible;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use super::super::{
    Config, DispositionReason, Generation, IncidentBinding, PriorIncident, PriorOutcome,
};
use super::classify::{
    self, bounded_report, decide, pool_level, ArchiveEntry, BoundedReport, Decision, FileClass,
    FileClassifier, FileReport, Incident, PoolFacts, PoolLevel, PoolRefusal,
};
use super::disposition::StoreValidator;
use super::format::{
    self, encode_header, hex, parse_disposition, parse_header, parse_provision, pool_file_size,
    pool_name, sha256, storage_identity, ArchiveName, Disposition, HeaderFields, HeaderParse,
    Provision, BLOCK, BLOCK_U64, DISPOSITIONS_ENTRY_LIMIT, DISPOSITION_LIMIT, MAX_APPLIED,
    MAX_CLAIM, PROVISION_LIMIT, REVOKED_ENTRY_LIMIT, ROOT_ENTRIES, STORAGE_READ_LIMIT, TMP_PREFIX,
};
use super::io::{sync_with_retries, Errno, FileType, IoError, Listing, LockRequest, Stat, StoreIo};

// ---------------------------------------------------------------------------
// The host view and the closed real entry
// ---------------------------------------------------------------------------

/// The store's I/O plus the host view an opening observes (design section
/// 6.4): the mount table, block and page sizes, the link sysctls, the
/// effective ext4 option listing, the jbd2 entries, the kernel identity, and
/// the storage's `/sys/dev/block` link and sysfs attributes. Only the
/// simulator implements it in this mission.
pub trait Platform: StoreIo {
    /// This process's id, for `/proc/<pid>/mountinfo` (never the `self`
    /// link).
    fn process_id(&self) -> u32;
    /// `statx(dir, "", AT_EMPTY_PATH, STATX_MNT_ID)`.
    fn mount_id(&self, dir: &Self::Dir) -> Result<u64, IoError>;
    /// `/proc/<pid>/mountinfo`, at most `limit + 1` bytes.
    fn read_mountinfo(&self, pid: u32, limit: usize) -> Result<Vec<u8>, IoError>;
    /// `fstatvfs`: `f_bsize`, `f_frsize`.
    fn fs_block_sizes(&self, dir: &Self::Dir) -> Result<(u64, u64), IoError>;
    /// `sysconf(_SC_PAGESIZE)`.
    fn page_size(&self) -> Result<u64, IoError>;
    /// `fs.protected_hardlinks`, `fs.protected_symlinks`.
    fn link_protection(&self) -> Result<(u32, u32), IoError>;
    /// `readlink("/sys/dev/block/<major>:<minor>")`: the whole target.
    fn block_device_link(&self, major: u32, minor: u32) -> Result<Option<String>, IoError>;
    /// `/proc/fs/ext4/<device>/options`, at most `limit + 1` bytes.
    fn ext4_options(&self, device: &str, limit: usize) -> Result<Option<Vec<u8>>, IoError>;
    /// Whether `/proc/fs/jbd2/<entry>` exists.
    fn jbd2_entry_exists(&self, entry: &str) -> Result<bool, IoError>;
    /// `uname(2)`: release and version.
    fn kernel_identity(&self) -> Result<(String, String), IoError>;
    /// One world-readable attribute below `/sys`, found by a fresh lookup
    /// that follows no symbolic link, at most `limit + 1` bytes.
    fn sysfs_attribute(&self, path: &str, limit: usize) -> Result<Option<Vec<u8>>, IoError>;
}

/// Why the real configured-store entry refuses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntegrationUnavailable {
    /// No deployment of the store is authorized: no host, device or
    /// configuration is qualified (design sections 5.9 and 18).
    DeploymentNotAuthorized,
}

/// What a request to open a configured store names. It carries no switch:
/// nothing in it can make the entry proceed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfiguredStoreRequest {
    pub provision_path: String,
    pub config: Config,
}

/// The real configured-store entry: unconditionally unavailable. It refuses
/// with [`IntegrationUnavailable::DeploymentNotAuthorized`] before it opens,
/// reads, syncs, locks, claims or modifies anything, and its success type is
/// uninhabited, so no path through it can ever return a store.
pub fn open_configured_store(
    request: &ConfiguredStoreRequest,
) -> Result<Infallible, IntegrationUnavailable> {
    let _ = request;
    Err(IntegrationUnavailable::DeploymentNotAuthorized)
}

// ---------------------------------------------------------------------------
// Refusals
// ---------------------------------------------------------------------------

/// Why an opening, a verification or a session refused (design sections
/// 10.2, 10.5, 10.6, 11.2, 13.1 and 13.11). Every refusal is a refusal: no
/// state here is success but `Ready`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refused {
    Unprovisioned,
    Invalid(String),
    Lost(String),
    Unsupported(String),
    /// Another description holds the store lock: nothing was synced or read.
    Busy,
    SelectionChanged(String),
    Unreliable(String),
    MaintenanceIncomplete(String),
    /// A pool file's lock is held: a live writer.
    Live(String),
    Capacity(String),
    /// These current incidents have no disposition that restates them.
    PriorUnresolved(Vec<[u8; 32]>),
    PoolExhausted,
    ClaimExhausted,
    ClaimFailed(String),
    /// A session object without its retained lock.
    NotAuthorized,
}

/// A refusal and the `PROVISION` revision it was made against, if a
/// selection was read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refusal {
    pub refused: Refused,
    pub revision: Option<u64>,
}

impl Refusal {
    fn new(refused: Refused, revision: Option<u64>) -> Refusal {
        Refusal { refused, revision }
    }
}

fn invalid(why: impl Into<String>) -> Refused {
    Refused::Invalid(why.into())
}

fn unsupported(why: impl Into<String>) -> Refused {
    Refused::Unsupported(why.into())
}

fn lost(why: impl Into<String>) -> Refused {
    Refused::Lost(why.into())
}

// ---------------------------------------------------------------------------
// Openings and the storage admission (design section 10.8)
// ---------------------------------------------------------------------------

static OPENINGS: AtomicU64 = AtomicU64::new(1);

/// One opening (an owner's, a verifier's or a session's): an admission
/// belongs to exactly one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct OpeningId(u64);

impl OpeningId {
    /// A new opening's serial. It names an opening; it grants nothing (an
    /// admission is bound to one, and only the storage check makes one).
    pub(super) fn fresh() -> OpeningId {
        OpeningId(OPENINGS.fetch_add(1, Ordering::SeqCst))
    }

    fn next() -> OpeningId {
        OpeningId::fresh()
    }

    pub fn serial(&self) -> u64 {
        self.0
    }
}

/// What step 12 observed of the storage. Plain data: an observation is never
/// authority by itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorageObservation {
    pub pci_function: String,
    pub partition: Option<u32>,
    pub identity: [u8; 32],
    pub controller_dir: String,
    pub disk_dir: String,
}

/// A verified storage qualification (design section 10.8): made only by
/// [`admit_storage`], from a fresh observation, for one opening and the
/// `PROVISION` digest of that opening's selection. It has private fields and
/// no other constructor, and it is not `Clone`, `Copy` or `Default` and has
/// no serialization or decoding. It is never stored beyond its opening, and
/// the claim accepts only its own opening's (A4's) admission.
#[derive(Debug)]
pub struct StorageAdmission {
    opening: OpeningId,
    provision_digest: [u8; 32],
    observation: StorageObservation,
}

impl StorageAdmission {
    pub fn opening(&self) -> OpeningId {
        self.opening
    }

    pub fn provision_digest(&self) -> [u8; 32] {
        self.provision_digest
    }

    pub fn observation(&self) -> &StorageObservation {
        &self.observation
    }
}

/// The claim's test: an admission the storage check made for this opening
/// and this selection.
pub(super) fn verify_admission(
    admission: Option<&StorageAdmission>,
    opening: OpeningId,
    provision_digest: &[u8; 32],
) -> bool {
    admission.is_some_and(|admission| {
        admission.opening == opening && admission.provision_digest == *provision_digest
    })
}

// ---------------------------------------------------------------------------
// Paths and safe walks (design section 10.1)
// ---------------------------------------------------------------------------

/// `<PROVISION_PATH>`, the Owner's choice: its directory's components and
/// its own name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProvisionPath {
    pub directory: Vec<String>,
    pub name: String,
}

impl ProvisionPath {
    pub fn parse(path: &str) -> Result<ProvisionPath, format::ParseError> {
        let path = format::path(path)?;
        let mut components: Vec<String> = format::path_components(path)
            .into_iter()
            .map(str::to_string)
            .collect();
        let name = components
            .pop()
            .ok_or_else(|| format::ParseError("bad path".into()))?;
        Ok(ProvisionPath {
            directory: components,
            name,
        })
    }

    /// The temporary names that belong to the store beside `PROVISION`.
    pub fn tmp_name(&self) -> String {
        format!("{}.tmp", self.name)
    }

    pub fn predecessor_tmp_name(&self) -> String {
        format!("{}.predecessor.tmp", self.name)
    }

    pub fn predecessor_name(&self, old_root_id: &[u8; 16]) -> String {
        format!("{}.predecessor-{}", self.name, hex(old_root_id))
    }
}

fn io_refusal(error: IoError, what: &str) -> Refused {
    match error.errno {
        Errno::NoEnt => lost(format!("{what}: no such entry")),
        Errno::Loop => invalid(format!("{what}: symbolic link")),
        errno => invalid(format!("{what}: {:?} ({})", errno, error.op)),
    }
}

/// A root-owned directory that neither group nor others can write.
fn trusted_dir(stat: &Stat) -> bool {
    stat.file_type == FileType::Directory && stat.uid == 0 && stat.mode & 0o022 == 0
}

/// Walk `components` from `/`, one `O_PATH` directory at a time, refusing a
/// symbolic link before opening it, checking every component root-owned and
/// not writable by group or others, and rechecking each opened descriptor's
/// identity.
fn walk<P: Platform>(io: &P, components: &[&str]) -> Result<(P::Dir, Stat), Refused> {
    let mut dir = io.root_dir().map_err(|error| io_refusal(error, "/"))?;
    let mut stat = io.stat_dir(&dir).map_err(|error| io_refusal(error, "/"))?;
    if !trusted_dir(&stat) {
        return Err(invalid("/ is not a root-owned directory closed to others"));
    }
    for component in components {
        let seen = io
            .stat_at(&dir, component)
            .map_err(|error| io_refusal(error, component))?;
        if seen.file_type != FileType::Directory {
            return Err(invalid(format!("{component} is not a directory")));
        }
        if !trusted_dir(&seen) {
            return Err(invalid(format!(
                "{component} is not root-owned or is writable by others"
            )));
        }
        let next = io
            .open_dir(&dir, component)
            .map_err(|error| io_refusal(error, component))?;
        let opened = io
            .stat_dir(&next)
            .map_err(|error| io_refusal(error, component))?;
        if !opened.same_inode(&seen) || opened.file_type != FileType::Directory {
            return Err(invalid(format!("{component} replaced during the walk")));
        }
        dir = next;
        stat = opened;
    }
    Ok((dir, stat))
}

/// The expected type and ownership of one entry (design section 10.1 step 1).
#[derive(Debug, Clone, Copy)]
pub struct Expect {
    pub file_type: FileType,
    pub uid: u32,
    pub gid: u32,
    pub mode: u32,
    /// The inode recorded in `PROVISION`, where one is.
    pub ino: Option<u64>,
}

/// Type-check an entry without opening it: a special file or a symbolic link
/// is refused here, never opened.
fn check_entry<P: Platform>(
    io: &P,
    dir: &P::Dir,
    name: &str,
    expect: Expect,
) -> Result<Stat, Refused> {
    let stat = io
        .stat_at(dir, name)
        .map_err(|error| io_refusal(error, name))?;
    if stat.file_type != expect.file_type {
        return Err(invalid(format!("{name} is a {:?}", stat.file_type)));
    }
    if (stat.uid, stat.gid, stat.mode) != (expect.uid, expect.gid, expect.mode) {
        return Err(invalid(format!("{name} ownership or mode")));
    }
    if expect.file_type == FileType::Regular && stat.nlink != 1 {
        return Err(invalid(format!("{name} link count {}", stat.nlink)));
    }
    if let Some(ino) = expect.ino {
        if stat.ino != ino {
            return Err(lost(format!("{name} replaced")));
        }
    }
    Ok(stat)
}

/// Safe open of a regular file after its type check: non-blocking,
/// no-follow, then its identity rechecked by `fstat`.
fn open_checked<P: Platform>(
    io: &P,
    dir: &P::Dir,
    name: &str,
    seen: &Stat,
) -> Result<P::File, Refused> {
    let file = io
        .open_read(dir, name)
        .map_err(|error| io_refusal(error, name))?;
    let opened = io
        .stat_file(&file)
        .map_err(|error| io_refusal(error, name))?;
    if !opened.same_inode(seen) || opened.file_type != seen.file_type {
        return Err(lost(format!("{name} replaced before it was opened")));
    }
    Ok(file)
}

/// Read a whole small file with a bound: `None` when it is larger.
fn read_bounded<P: Platform>(
    io: &P,
    file: &P::File,
    limit: usize,
) -> Result<Option<Vec<u8>>, IoError> {
    let mut data = Vec::new();
    let mut buf = vec![0u8; 4096];
    loop {
        let got = io.pread(file, data.len() as u64, &mut buf)?;
        if got == 0 {
            return Ok(Some(data));
        }
        data.extend_from_slice(&buf[..got]);
        if data.len() > limit {
            return Ok(None);
        }
    }
}

// ---------------------------------------------------------------------------
// Selection and revalidation (design section 10.5)
// ---------------------------------------------------------------------------

/// A `PROVISION` selection: what was read, from which inode, and the
/// identities it records. Reading yields a candidate; only revalidation under
/// the store lock makes it authoritative for the process holding that lock.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Selection {
    pub provision: Provision,
    pub provision_stat: Stat,
    /// The SHA-256 of the complete bytes read.
    pub digest: [u8; 32],
    pub provdir_ino: u64,
    pub parent_ino: u64,
    /// The state root's parent's components and its own name.
    pub parent: Vec<String>,
    pub state_name: String,
}

impl Selection {
    pub fn revision(&self) -> u64 {
        self.provision.revision
    }
}

fn components(path: &[String]) -> Vec<&str> {
    path.iter().map(String::as_str).collect()
}

/// Read `PROVISION` through a fresh walk and safe open: a candidate.
pub(super) fn select<P: Platform>(io: &P, path: &ProvisionPath) -> Result<Selection, Refused> {
    let (provdir, provdir_stat) = match walk(io, &components(&path.directory)) {
        Ok(found) => found,
        Err(Refused::Lost(_)) => return Err(Refused::Unprovisioned),
        Err(other) => return Err(other),
    };
    let stat = match io.stat_at(&provdir, &path.name) {
        Ok(stat) => stat,
        Err(IoError {
            errno: Errno::NoEnt,
            ..
        }) => return Err(Refused::Unprovisioned),
        Err(error) => return Err(io_refusal(error, "PROVISION")),
    };
    if stat.file_type != FileType::Regular
        || (stat.uid, stat.gid, stat.mode, stat.nlink) != (0, 0, 0o444, 1)
    {
        return Err(invalid("PROVISION type"));
    }
    let file = open_checked(io, &provdir, &path.name, &stat)?;
    let data = read_bounded(io, &file, PROVISION_LIMIT)
        .map_err(|error| io_refusal(error, "PROVISION"))?
        .ok_or_else(|| invalid("PROVISION too large"))?;
    drop(file);
    let provision =
        parse_provision(&data).map_err(|error| invalid(format!("PROVISION: {}", error.0)))?;
    let (parent, state_name) = {
        let all = format::path_components(&provision.state_root);
        let (last, rest) = all
            .split_last()
            .ok_or_else(|| invalid("PROVISION: state root"))?;
        (
            rest.iter()
                .map(|part| part.to_string())
                .collect::<Vec<String>>(),
            last.to_string(),
        )
    };
    let (_, parent_stat) = walk(io, &components(&parent)).map_err(|refused| match refused {
        Refused::Lost(why) => lost(format!("state root parent: {why}")),
        other => other,
    })?;
    Ok(Selection {
        provision,
        provision_stat: stat,
        digest: sha256(&data),
        provdir_ino: provdir_stat.ino,
        parent_ino: parent_stat.ino,
        parent,
        state_name,
    })
}

/// Post-lock revalidation by fresh lookups (design section 10.5 steps 1 to
/// 4): never through a descriptor kept from the read.
pub(super) fn revalidate<P: Platform>(
    io: &P,
    path: &ProvisionPath,
    selection: &Selection,
    lock: &P::File,
) -> bool {
    let fresh = || -> Result<bool, Refused> {
        let (provdir, _) = walk(io, &components(&path.directory))?;
        let stat = io
            .stat_at(&provdir, &path.name)
            .map_err(|error| io_refusal(error, "PROVISION"))?;
        if !stat.same_inode(&selection.provision_stat) {
            return Ok(false);
        }
        let file = open_checked(io, &provdir, &path.name, &stat)?;
        let data = read_bounded(io, &file, PROVISION_LIMIT)
            .map_err(|error| io_refusal(error, "PROVISION"))?;
        if data.map(|bytes| sha256(&bytes)) != Some(selection.digest) {
            return Ok(false);
        }
        let (parent, _) = walk(io, &components(&selection.parent))?;
        let root = io
            .stat_at(&parent, &selection.state_name)
            .map_err(|error| io_refusal(error, "state root"))?;
        if root.ino != selection.provision.root_inode || root.file_type != FileType::Directory {
            return Ok(false);
        }
        let root_dir = io
            .open_dir(&parent, &selection.state_name)
            .map_err(|error| io_refusal(error, "state root"))?;
        let entry = io
            .stat_at(&root_dir, "LOCK")
            .map_err(|error| io_refusal(error, "LOCK"))?;
        let locked = io
            .stat_file(lock)
            .map_err(|error| io_refusal(error, "LOCK"))?;
        Ok(entry.ino == selection.provision.lock_inode && locked.same_inode(&entry))
    };
    fresh().unwrap_or(false)
}

// ---------------------------------------------------------------------------
// Mount, profile and storage checks (design section 6.4)
// ---------------------------------------------------------------------------

/// A bound on `/proc/<pid>/mountinfo`.
pub const MOUNTINFO_LIMIT: usize = 1 << 20;
/// The option listing's bound (design section 6.4 step 9).
pub const OPTIONS_LIMIT: usize = 4096;
/// Options the effective listing must contain.
pub const REQUIRED_OPTIONS: [&str; 2] = ["data=ordered", "barrier"];
/// Options the effective listing must not contain.
pub const EXCLUDED_OPTIONS: [&str; 9] = [
    "nobarrier",
    "data=journal",
    "data=writeback",
    "journal_async_commit",
    "norecovery",
    "noload",
    "fc_debug_force",
    "emergency_ro",
    "shutdown",
];
/// Options the pinned strings must not contain (step 6).
pub const PINNED_EXCLUDED: [&str; 4] = ["ro", "nobarrier", "barrier=0", "data=writeback"];
/// The qualified journal inode: jbd2 names an internal journal
/// "<device>-<inode>".
pub const JOURNAL_INODE: u64 = 8;
/// The admitted cache registration: `queue/write_cache`, `queue/fua`.
pub const ADMITTED_CACHE: (&str, &str) = ("write through\n", "0\n");

/// glibc's `major` and `minor` of an `st_dev`.
pub fn major_minor(dev: u64) -> (u32, u32) {
    let major = ((dev >> 32) & 0xffff_f000) | ((dev >> 8) & 0x0000_0fff);
    let minor = ((dev >> 12) & 0xffff_ff00) | (dev & 0x0000_00ff);
    (major as u32, minor as u32)
}

/// One `mountinfo` record: the fields the store reads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MountRecord {
    pub mount_id: u64,
    pub major: u32,
    pub minor: u32,
    pub mount_options: String,
    pub fstype: String,
    pub super_options: String,
}

/// Parse `mountinfo`: field 1 the mount ID, 3 `major:minor`, 6 the per-mount
/// options, then optional fields up to `-`, then the type, the source and
/// the per-superblock options. Any unparsable line refuses the whole table.
pub fn parse_mountinfo(text: &[u8]) -> Result<Vec<MountRecord>, String> {
    let text = std::str::from_utf8(text).map_err(|_| "mountinfo not text".to_string())?;
    let mut records = Vec::new();
    for line in text.lines() {
        let fields: Vec<&str> = line.split(' ').collect();
        let dash = fields
            .iter()
            .enumerate()
            .skip(6)
            .find(|(_, field)| **field == "-")
            .map(|(at, _)| at)
            .ok_or_else(|| format!("mountinfo line without separator: {line}"))?;
        if fields.len() != dash + 4 {
            return Err(format!("mountinfo line fields: {line}"));
        }
        let mount_id: u64 = fields[0]
            .parse()
            .map_err(|_| format!("mountinfo mount id: {line}"))?;
        let (major, minor) = fields[2]
            .split_once(':')
            .and_then(|(major, minor)| Some((major.parse().ok()?, minor.parse().ok()?)))
            .ok_or_else(|| format!("mountinfo device: {line}"))?;
        records.push(MountRecord {
            mount_id,
            major,
            minor,
            mount_options: fields[5].to_string(),
            fstype: fields[dash + 1].to_string(),
            super_options: fields[dash + 3].to_string(),
        });
    }
    Ok(records)
}

/// Whether the requalification skips the pinned comparisons (design section
/// 13.3): the option strings, the kernel identity and the storage identity
/// being replaced. It never skips a rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pinning {
    Pinned,
    Requalification,
}

/// Steps 1 to 11: the mount through the retained root descriptor, the
/// pinned options, sizes and link protection, one filesystem, the effective
/// options, the journal's location and the kernel. Returns the device's
/// major and minor numbers and its name.
pub(super) fn check_mount<P: Platform>(
    io: &P,
    provision: &Provision,
    root: &P::Dir,
    other_dirs: &[&P::Dir],
    pinning: Pinning,
) -> Result<((u32, u32), String), Refused> {
    let root_stat = io
        .stat_dir(root)
        .map_err(|error| io_refusal(error, "state root"))?;
    let mount_id = io
        .mount_id(root)
        .map_err(|error| unsupported(format!("statx: {error}")))?;
    let table = io
        .read_mountinfo(io.process_id(), MOUNTINFO_LIMIT)
        .map_err(|error| unsupported(format!("mountinfo: {error}")))?;
    if table.len() > MOUNTINFO_LIMIT {
        return Err(unsupported("mountinfo too large"));
    }
    let records = parse_mountinfo(&table).map_err(unsupported)?;
    let matches: Vec<&MountRecord> = records
        .iter()
        .filter(|record| record.mount_id == mount_id)
        .collect();
    let [record] = matches.as_slice() else {
        return Err(unsupported("mount id not unique"));
    };
    let device = major_minor(root_stat.dev);
    if (record.major, record.minor) != device {
        return Err(unsupported("device mismatch"));
    }
    if record.fstype != "ext4" {
        return Err(unsupported(format!("filesystem {}", record.fstype)));
    }
    if pinning == Pinning::Pinned
        && (record.mount_options != provision.mount_options
            || record.super_options != provision.super_options)
    {
        return Err(unsupported("options changed"));
    }
    for options in [&record.mount_options, &record.super_options] {
        if options
            .split(',')
            .any(|option| PINNED_EXCLUDED.contains(&option))
        {
            return Err(unsupported("unsupported option"));
        }
    }
    let sizes = io
        .fs_block_sizes(root)
        .map_err(|error| unsupported(format!("fstatvfs: {error}")))?;
    let page = io
        .page_size()
        .map_err(|error| unsupported(format!("page size: {error}")))?;
    if sizes != (4096, 4096) || page != 4096 {
        return Err(unsupported("block or page size"));
    }
    let protection = io
        .link_protection()
        .map_err(|error| unsupported(format!("link protection: {error}")))?;
    if protection != (1, 1) {
        return Err(unsupported("link protection"));
    }
    for dir in other_dirs {
        let stat = io
            .stat_dir(dir)
            .map_err(|error| io_refusal(error, "directory"))?;
        if stat.dev != root_stat.dev {
            return Err(unsupported(
                "PROVISION directory and state root on different filesystems",
            ));
        }
    }
    let name = check_profile(io, provision, record, device, pinning)?;
    Ok((device, name))
}

/// Steps 9 to 11: the effective profile, never inferred from an absent
/// string in `mountinfo`.
fn check_profile<P: Platform>(
    io: &P,
    provision: &Provision,
    record: &MountRecord,
    device: (u32, u32),
    pinning: Pinning,
) -> Result<String, Refused> {
    let link = io
        .block_device_link(device.0, device.1)
        .map_err(|error| unsupported(format!("device link: {error}")))?
        .ok_or_else(|| unsupported("device name unresolved"))?;
    let name = link
        .rsplit('/')
        .next()
        .filter(|name| !name.is_empty())
        .ok_or_else(|| unsupported("device name unresolved"))?
        .to_string();
    let listing = io
        .ext4_options(&name, OPTIONS_LIMIT)
        .map_err(|error| unsupported(format!("effective options: {error}")))?
        .ok_or_else(|| unsupported("effective options unreadable"))?;
    if listing.len() > OPTIONS_LIMIT {
        return Err(unsupported("effective options unreadable"));
    }
    let listing =
        std::str::from_utf8(&listing).map_err(|_| unsupported("effective options unreadable"))?;
    let lines: Vec<&str> = listing.lines().collect();
    if lines.first() != Some(&"rw") {
        return Err(unsupported("read-only"));
    }
    let listed: BTreeSet<&str> = lines.iter().copied().collect();
    let relevant: BTreeSet<&str> = REQUIRED_OPTIONS
        .iter()
        .chain(EXCLUDED_OPTIONS.iter())
        .copied()
        .collect();
    if record
        .super_options
        .split(',')
        .filter(|option| relevant.contains(option))
        .any(|option| !listed.contains(option))
    {
        return Err(unsupported("contradictory profile information"));
    }
    if REQUIRED_OPTIONS
        .iter()
        .any(|option| !listed.contains(option))
        || EXCLUDED_OPTIONS
            .iter()
            .any(|option| listed.contains(option))
    {
        return Err(unsupported("effective profile"));
    }
    let journal = format!("{name}-{JOURNAL_INODE}");
    if !io
        .jbd2_entry_exists(&journal)
        .map_err(|error| unsupported(format!("journal location: {error}")))?
    {
        return Err(unsupported("journal not internal"));
    }
    if pinning == Pinning::Pinned {
        let (release, version) = io
            .kernel_identity()
            .map_err(|error| unsupported(format!("kernel identity: {error}")))?;
        if format!("{release} {version}") != provision.kernel {
            return Err(unsupported("kernel not qualified"));
        }
    }
    Ok(name)
}

fn nvme_controller(name: &str) -> bool {
    let Some(number) = name.strip_prefix("nvme") else {
        return false;
    };
    canonical_number(number, 5)
}

fn canonical_number(text: &str, max_digits: usize) -> bool {
    !text.is_empty()
        && text.len() <= max_digits
        && text.bytes().all(|byte| byte.is_ascii_digit())
        && (text == "0" || !text.starts_with('0'))
}

fn nvme_disk(name: &str) -> bool {
    let Some(rest) = name.strip_prefix("nvme") else {
        return false;
    };
    let Some((controller, namespace)) = rest.split_once('n') else {
        return false;
    };
    canonical_number(controller, 5) && canonical_number(namespace, 10) && namespace != "0"
}

/// Step 12's form: `../../devices/pci.../<PCI function>/nvme/nvme<k>/<disk>`
/// with an optional `/<disk>p<i>`: the controller's and the disk's
/// directories below `/sys`, the PCI function and the partition.
pub fn parse_storage_link(link: &str) -> Option<(String, String, String, Option<u32>)> {
    let rest = link.strip_prefix("../../devices/")?;
    let mut parts: Vec<&str> = rest.split('/').collect();
    if parts
        .iter()
        .any(|part| part.is_empty() || *part == "." || *part == "..")
    {
        return None;
    }
    let mut partition = None;
    if let Some(last) = parts.last() {
        if let Some((disk, number)) = last.rsplit_once('p') {
            if nvme_disk(disk) && canonical_number(number, 3) {
                let number: u32 = number.parse().ok()?;
                if parts.len() < 2 || parts[parts.len() - 2] != disk || !(1..=255).contains(&number)
                {
                    return None;
                }
                partition = Some(number);
                parts.pop();
            }
        }
    }
    let n = parts.len();
    if n < 5
        || !nvme_disk(parts[n - 1])
        || !nvme_controller(parts[n - 2])
        || parts[n - 3] != "nvme"
        || format::pci_function(parts[n - 4]).is_err()
        || !parts[0].starts_with("pci")
    {
        return None;
    }
    let disk_dir = format!("/sys/devices/{}", parts.join("/"));
    let controller_dir = format!("/sys/devices/{}", parts[..n - 1].join("/"));
    Some((
        controller_dir,
        disk_dir,
        parts[n - 4].to_string(),
        partition,
    ))
}

/// Step 12: the storage observed afresh, its rules (constants of the code,
/// relaxed by no field), and, pinned, the identity `PROVISION` records.
pub(super) fn check_storage<P: Platform>(
    io: &P,
    provision: &Provision,
    device: (u32, u32),
    pinning: Pinning,
) -> Result<StorageObservation, Refused> {
    let link = io
        .block_device_link(device.0, device.1)
        .map_err(|error| unsupported(format!("storage link: {error}")))?;
    let Some((controller_dir, disk_dir, pci_function, partition)) =
        link.as_deref().and_then(parse_storage_link)
    else {
        return Err(unsupported(
            "storage not a direct NVMe namespace on PCI Express",
        ));
    };
    let read = |path: String| -> Result<Vec<u8>, Refused> {
        match io.sysfs_attribute(&path, STORAGE_READ_LIMIT) {
            Ok(Some(bytes)) if bytes.len() <= STORAGE_READ_LIMIT => Ok(bytes),
            _ => Err(unsupported("storage unreadable")),
        }
    };
    let transport = read(format!("{controller_dir}/transport"))?;
    let model = read(format!("{controller_dir}/model"))?;
    let serial = read(format!("{controller_dir}/serial"))?;
    let firmware = read(format!("{controller_dir}/firmware_rev"))?;
    let wwid = read(format!("{disk_dir}/wwid"))?;
    let write_cache = read(format!("{disk_dir}/queue/write_cache"))?;
    let fua = read(format!("{disk_dir}/queue/fua"))?;
    if transport != b"pcie\n" {
        return Err(unsupported("storage transport"));
    }
    let cache = (write_cache.as_slice(), fua.as_slice());
    if cache == (&b"write back\n"[..], &b"1\n"[..]) {
        return Err(unsupported("volatile write cache"));
    }
    if cache == (&b"write through\n"[..], &b"1\n"[..]) {
        return Err(unsupported(
            "volatile write cache, flushes disabled in the kernel's view only",
        ));
    }
    if cache != (ADMITTED_CACHE.0.as_bytes(), ADMITTED_CACHE.1.as_bytes()) {
        return Err(unsupported("contradictory storage information"));
    }
    let identity = storage_identity([&model, &serial, &firmware, &wwid])
        .ok_or_else(|| unsupported("storage unreadable"))?;
    let observation = StorageObservation {
        pci_function,
        partition,
        identity,
        controller_dir,
        disk_dir,
    };
    if pinning == Pinning::Pinned
        && (observation.pci_function != provision.storage_pci_function
            || observation.partition != provision.storage_partition
            || observation.identity != provision.storage_identity)
    {
        return Err(unsupported("storage not qualified"));
    }
    Ok(observation)
}

/// The only constructor of a [`StorageAdmission`]: step 12 afresh, for this
/// opening and this selection.
pub(super) fn admit_storage<P: Platform>(
    io: &P,
    selection: &Selection,
    device: (u32, u32),
    opening: OpeningId,
    pinning: Pinning,
) -> Result<StorageAdmission, Refused> {
    let observation = check_storage(io, &selection.provision, device, pinning)?;
    Ok(StorageAdmission {
        opening,
        provision_digest: selection.digest,
        observation,
    })
}

// ---------------------------------------------------------------------------
// The opened root and activation (design sections 10.2 step 2 and 10.6)
// ---------------------------------------------------------------------------

/// The directories an opening keeps: the state root, its parent and the
/// `PROVISION` directory, each from a fresh walk.
pub(super) struct RootDirs<P: StoreIo> {
    pub(super) provdir: P::Dir,
    pub(super) parent: P::Dir,
    pub(super) root: P::Dir,
}

/// What step 2 opened and admitted.
pub(super) struct OpenedRoot<P: StoreIo> {
    pub(super) dirs: RootDirs<P>,
    pub(super) admission: StorageAdmission,
    pub(super) device: (u32, u32),
}

/// Step 2: the state root through its parent, the mount, profile and
/// one-filesystem checks, then the storage and this opening's admission.
pub(super) fn open_root<P: Platform>(
    io: &P,
    path: &ProvisionPath,
    selection: &Selection,
    opening: OpeningId,
    pinning: Pinning,
) -> Result<OpenedRoot<P>, Refused> {
    let (provdir, _) = walk(io, &components(&path.directory))?;
    let (parent, _) = walk(io, &components(&selection.parent))?;
    let stat = match io.stat_at(&parent, &selection.state_name) {
        Ok(stat) => stat,
        Err(_) => return Err(lost("state root")),
    };
    if stat.file_type != FileType::Directory || stat.ino != selection.provision.root_inode {
        return Err(lost("state root"));
    }
    if (stat.uid, stat.gid, stat.mode) != (0, 0, 0o755) {
        return Err(invalid("state root ownership"));
    }
    let root = io
        .open_dir(&parent, &selection.state_name)
        .map_err(|error| io_refusal(error, "state root"))?;
    let opened = io
        .stat_dir(&root)
        .map_err(|error| io_refusal(error, "state root"))?;
    if !opened.same_inode(&stat) {
        return Err(lost("state root replaced"));
    }
    let (device, _) = check_mount(
        io,
        &selection.provision,
        &root,
        &[&provdir, &parent],
        pinning,
    )?;
    let admission = admit_storage(io, selection, device, opening, pinning)?;
    Ok(OpenedRoot {
        dirs: RootDirs {
            provdir,
            parent,
            root,
        },
        admission,
        device,
    })
}

/// `LOCK`'s type and identity, before it is opened.
fn check_lock_entry<P: Platform>(
    io: &P,
    root: &P::Dir,
    provision: &Provision,
) -> Result<Stat, Refused> {
    let stat = io.stat_at(root, "LOCK").map_err(|_| lost("LOCK"))?;
    if stat.file_type != FileType::Regular || stat.ino != provision.lock_inode {
        return Err(lost("LOCK"));
    }
    if (stat.uid, stat.gid, stat.mode, stat.nlink) != (provision.uid, provision.gid, 0o600, 1) {
        return Err(invalid("LOCK type"));
    }
    Ok(stat)
}

/// The store lock's description: `LOCK` type-checked, then opened safely.
pub(super) fn open_lock<P: Platform>(
    io: &P,
    root: &P::Dir,
    provision: &Provision,
) -> Result<P::File, Refused> {
    let stat = check_lock_entry(io, root, provision)?;
    open_checked(io, root, "LOCK", &stat)
}

/// The activation directories, in sync order (design section 10.6 A2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ActivationDir {
    Journals,
    Revoked,
    Dispositions,
    Archive,
    Root,
    Parent,
    ProvDir,
}

pub const ACTIVATION_DIRECTORIES: [ActivationDir; 7] = [
    ActivationDir::Journals,
    ActivationDir::Revoked,
    ActivationDir::Dispositions,
    ActivationDir::Archive,
    ActivationDir::Root,
    ActivationDir::Parent,
    ActivationDir::ProvDir,
];

impl ActivationDir {
    pub fn label(self) -> &'static str {
        match self {
            ActivationDir::Journals => "journals",
            ActivationDir::Revoked => "dispositions/revoked",
            ActivationDir::Dispositions => "dispositions",
            ActivationDir::Archive => "archive",
            ActivationDir::Root => "root",
            ActivationDir::Parent => "parent",
            ActivationDir::ProvDir => "provdir",
        }
    }
}

/// Where an activation is (a test's deterministic interleaving point).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivationPoint {
    /// Before the sync of this directory.
    BeforeSync(ActivationDir),
    BeforeProvisionSync,
    /// Before the probe's `futimens`.
    Probe,
    /// Between the probe's `futimens` and its `fsync`.
    ProbeFsync,
    /// Before the revalidation of A4.
    Revalidate,
}

/// A test's interleaving points in an opening, called from the store's own
/// code. They let a test act on the simulated world at an exact point; they
/// decide nothing and change no store state.
pub trait OpeningHooks {
    /// After the `LOCK` entry's check, before the lock request, on each
    /// selection attempt.
    fn before_lock(&mut self, _attempt: u32) {}
    fn activation(&mut self, _point: ActivationPoint) {}
    /// After activation, before the scan.
    fn before_scan(&mut self) {}
}

/// No interleaving.
pub struct NoHooks;

impl OpeningHooks for NoHooks {}

/// The parent directory and name of an activation directory, resolved afresh
/// from `/` by the component walk, with the inode it must have.
fn resolve_activation<P: Platform>(
    io: &P,
    path: &ProvisionPath,
    selection: &Selection,
    which: ActivationDir,
) -> Result<(P::Dir, String, u64), Refused> {
    let p = &selection.provision;
    let mut state_root = selection.parent.clone();
    state_root.push(selection.state_name.clone());
    let (base, name, expected): (Vec<String>, String, u64) = match which {
        ActivationDir::Journals => (state_root, "journals".into(), p.journals_inode),
        ActivationDir::Dispositions => (state_root, "dispositions".into(), p.dispositions_inode),
        ActivationDir::Revoked => {
            let mut base = state_root;
            base.push("dispositions".into());
            (base, "revoked".into(), p.revoked_inode)
        }
        ActivationDir::Archive => (state_root, "archive".into(), p.archive_inode),
        ActivationDir::Root => (
            selection.parent.clone(),
            selection.state_name.clone(),
            p.root_inode,
        ),
        ActivationDir::Parent => {
            let mut base = selection.parent.clone();
            let name = base
                .pop()
                .ok_or_else(|| invalid("state root parent is /"))?;
            (base, name, selection.parent_ino)
        }
        ActivationDir::ProvDir => {
            let mut base = path.directory.clone();
            let name = base
                .pop()
                .ok_or_else(|| invalid("PROVISION directory is /"))?;
            (base, name, selection.provdir_ino)
        }
    };
    let (dir, _) = walk(io, &components(&base)).map_err(|refused| match refused {
        Refused::Lost(why) => lost(format!("activation: {}: {why}", which.label())),
        other => other,
    })?;
    Ok((dir, name, expected))
}

/// What an activation acts on: the selection revalidated under the store
/// lock held through `lock`, the opening's directories, and the admission
/// the opening's step 2 made.
pub(super) struct Activation<'a, P: Platform> {
    pub(super) path: &'a ProvisionPath,
    pub(super) selection: &'a Selection,
    pub(super) dirs: &'a RootDirs<P>,
    pub(super) lock: &'a P::File,
    pub(super) opening: OpeningId,
    pub(super) observed: &'a StorageAdmission,
    pub(super) pinning: Pinning,
}

/// Durable activation (design section 10.6), under the store lock: A1 one
/// filesystem; A2 every dependency directory synced, then `PROVISION`; A3
/// the probe on the store lock's own description; A4 revalidation with the
/// profile and the storage checked again, and this opening's admission made
/// afresh with the identity the opening observed. Returns A4's admission; any
/// failure refuses, and no timeout or retry turns a failure into activation.
pub(super) fn activate<P: Platform>(
    io: &P,
    activation: &Activation<'_, P>,
    hooks: &mut dyn OpeningHooks,
) -> Result<StorageAdmission, Refused> {
    let Activation {
        path,
        selection,
        dirs,
        lock,
        opening,
        observed,
        pinning,
    } = *activation;
    // A1: one filesystem.
    let devs: Vec<u64> = [&dirs.provdir, &dirs.parent, &dirs.root]
        .iter()
        .map(|dir| io.stat_dir(dir).map(|stat| stat.dev))
        .collect::<Result<_, _>>()
        .map_err(|error| io_refusal(error, "activation"))?;
    if devs.windows(2).any(|pair| pair[0] != pair[1]) {
        return Err(unsupported(
            "PROVISION directory and state root on different filesystems",
        ));
    }
    // A2: each dependency directory, resolved afresh, checked, synced.
    for which in ACTIVATION_DIRECTORIES {
        hooks.activation(ActivationPoint::BeforeSync(which));
        let (base, name, expected) = resolve_activation(io, path, selection, which)?;
        let stat = io
            .stat_at(&base, &name)
            .map_err(|_| lost(format!("activation: {}", which.label())))?;
        if stat.ino != expected || stat.file_type != FileType::Directory {
            return Err(invalid(format!(
                "activation: {} is not the selected directory",
                which.label()
            )));
        }
        let handle = io
            .open_dir_for_sync(&base, &name)
            .map_err(|error| io_refusal(error, which.label()))?;
        if !io
            .stat_file(&handle)
            .map(|opened| opened.same_inode(&stat))
            .unwrap_or(false)
        {
            return Err(invalid(format!(
                "activation: {} replaced before its sync",
                which.label()
            )));
        }
        sync_with_retries(io, &handle, false).map_err(|error| {
            Refused::Unreliable(format!(
                "activation sync of {}: {:?}",
                which.label(),
                error.errno
            ))
        })?;
    }
    hooks.activation(ActivationPoint::BeforeProvisionSync);
    let provision = io
        .open_read(&dirs.provdir, &path.name)
        .map_err(|error| io_refusal(error, "PROVISION"))?;
    sync_with_retries(io, &provision, true).map_err(|error| {
        Refused::Unreliable(format!("activation sync of PROVISION: {:?}", error.errno))
    })?;
    drop(provision);
    // A3: the probe, after the last A2 sync.
    hooks.activation(ActivationPoint::Probe);
    io.touch(lock)
        .map_err(|error| Refused::Unreliable(format!("activation probe: {:?}", error.errno)))?;
    hooks.activation(ActivationPoint::ProbeFsync);
    io.fsync(lock)
        .map_err(|error| Refused::Unreliable(format!("activation probe: {:?}", error.errno)))?;
    // A4: revalidation after the syncs.
    hooks.activation(ActivationPoint::Revalidate);
    for which in ACTIVATION_DIRECTORIES {
        let (base, name, expected) =
            resolve_activation(io, path, selection, which).map_err(|_| {
                Refused::SelectionChanged(format!("after activation: {}", which.label()))
            })?;
        match io.stat_at(&base, &name) {
            Ok(stat) if stat.ino == expected => {}
            _ => {
                return Err(Refused::SelectionChanged(format!(
                    "after activation: {}",
                    which.label()
                )))
            }
        }
    }
    if !revalidate(io, path, selection, lock) {
        return Err(Refused::SelectionChanged("after activation".into()));
    }
    let (fresh_provdir, _) = walk(io, &components(&path.directory))?;
    let (fresh_parent, _) = walk(io, &components(&selection.parent))?;
    let fresh_root = io
        .open_dir(&fresh_parent, &selection.state_name)
        .map_err(|error| io_refusal(error, "state root"))?;
    let (device, _) = check_mount(
        io,
        &selection.provision,
        &fresh_root,
        &[&fresh_provdir, &fresh_parent],
        pinning,
    )?;
    let again = admit_storage(io, selection, device, opening, pinning)?;
    if again.observation.identity != observed.observation.identity
        || again.observation.pci_function != observed.observation.pci_function
        || again.observation.partition != observed.observation.partition
    {
        return Err(unsupported("storage changed during activation"));
    }
    // A5: activated.
    Ok(again)
}

// ---------------------------------------------------------------------------
// The scan (design section 10.2 steps 4 to 8)
// ---------------------------------------------------------------------------

/// Which opening scans: an owner (shared locks, the evidence-preservation
/// sync), the standalone verifier (shared locks, no sync) or a maintenance
/// session (exclusive locks, through the descriptions it already holds).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScanMode {
    Owner,
    Verifier,
    Session,
}

/// What the scan read and established, over the complete set.
#[derive(Debug, Clone)]
pub struct ScanResult {
    pub files: Vec<FileReport>,
    pub archive: Vec<ArchiveEntry>,
    pub dispositions: BTreeMap<[u8; 32], Disposition>,
    pub revoked: Vec<String>,
    pub level: PoolLevel,
    pub report: BoundedReport,
    /// Every generation in a scanned header: pool files and archive entries.
    pub generations: BTreeSet<[u8; 16]>,
    /// The owner's preservation syncs returned 0: never "evidence durable".
    pub preservation_sync_returned_zero: bool,
}

fn accept_journals_name(name: &str, pool: u32) -> bool {
    format::parse_pool_name(name, pool).is_some()
}

fn accept_dispositions_name(name: &str) -> bool {
    name == "revoked" || format::parse_disposition_name(name).is_some()
}

/// Check one directory's names: a leftover temporary refuses as
/// MaintenanceIncomplete, any other unaccepted name as Invalid.
fn check_names(names: &[String], accept: impl Fn(&str) -> bool, what: &str) -> Result<(), Refused> {
    if let Some(name) = names
        .iter()
        .find(|name| name.starts_with(TMP_PREFIX) && !accept(name))
    {
        return Err(Refused::MaintenanceIncomplete(format!("{what}/{name}")));
    }
    if let Some(name) = names.iter().find(|name| !accept(name)) {
        return Err(invalid(format!("unexpected entry {what}/{name}")));
    }
    Ok(())
}

fn listing<P: Platform>(
    io: &P,
    dir: &P::Dir,
    limit: usize,
    what: &str,
) -> Result<Vec<String>, Refused> {
    match io.list_dir(dir, limit) {
        Ok(Listing::Names(names)) => Ok(names),
        Ok(Listing::TooMany) => Err(invalid(format!("enumeration bound: {what}"))),
        Err(error) => Err(io_refusal(error, what)),
    }
}

/// The store directories the scan reads, opened by their names after their
/// type checks.
pub(super) struct StoreDirs<P: StoreIo> {
    pub(super) journals: P::Dir,
    pub(super) dispositions: P::Dir,
    pub(super) revoked: P::Dir,
    pub(super) archive: P::Dir,
}

/// Step 4's checks of the root and its directories.
pub(super) fn open_store_dirs<P: Platform>(
    io: &P,
    root: &P::Dir,
    provision: &Provision,
) -> Result<StoreDirs<P>, Refused> {
    let names = listing(io, root, 64, "root")?;
    let expected: BTreeSet<&str> = ROOT_ENTRIES.iter().copied().collect();
    let present: BTreeSet<&str> = names.iter().map(String::as_str).collect();
    if present != expected {
        if present
            .iter()
            .any(|name| name.starts_with(TMP_PREFIX) && !expected.contains(name))
        {
            return Err(Refused::MaintenanceIncomplete("root".into()));
        }
        return Err(invalid("root entries"));
    }
    let dir = |name: &str, ino: u64| -> Result<P::Dir, Refused> {
        let stat = check_entry(
            io,
            root,
            name,
            Expect {
                file_type: FileType::Directory,
                uid: 0,
                gid: 0,
                mode: 0o755,
                ino: None,
            },
        )
        .map_err(|refused| match refused {
            Refused::Lost(why) | Refused::Invalid(why) => {
                invalid(format!("{name} directory: {why}"))
            }
            other => other,
        })?;
        if stat.ino != ino {
            return Err(invalid(format!("{name} directory")));
        }
        let handle = io
            .open_dir(root, name)
            .map_err(|error| io_refusal(error, name))?;
        if !io
            .stat_dir(&handle)
            .map(|opened| opened.same_inode(&stat))
            .unwrap_or(false)
        {
            return Err(invalid(format!("{name} directory replaced")));
        }
        Ok(handle)
    };
    let journals = dir("journals", provision.journals_inode)?;
    let dispositions = dir("dispositions", provision.dispositions_inode)?;
    let archive = dir("archive", provision.archive_inode)?;
    let revoked_stat = io
        .stat_at(&dispositions, "revoked")
        .map_err(|_| invalid("revoked directory"))?;
    if !trusted_dir(&revoked_stat) || revoked_stat.ino != provision.revoked_inode {
        return Err(invalid("revoked directory"));
    }
    let revoked = io
        .open_dir(&dispositions, "revoked")
        .map_err(|error| io_refusal(error, "revoked"))?;
    Ok(StoreDirs {
        journals,
        dispositions,
        revoked,
        archive,
    })
}

/// Pool-file locks a maintenance session already holds, by index.
pub(super) type HeldJournals<'a, P> = BTreeMap<u32, &'a <P as StoreIo>::File>;

/// Steps 4 to 8 (design section 10.2), and the bounded report.
pub(super) fn scan<P: Platform>(
    io: &P,
    provision: &Provision,
    dirs: &StoreDirs<P>,
    mode: ScanMode,
    held: &HeldJournals<'_, P>,
    incident_limit: usize,
) -> Result<ScanResult, Refused> {
    let pool = provision.pool();
    let size = pool_file_size(provision.c_pool).ok_or_else(|| invalid("pool size"))?;
    // Step 4: names only, each directory counted against its bound first.
    let journal_names = listing(io, &dirs.journals, (pool as usize) * 2 + 16, "journals")?;
    let disposition_names = listing(
        io,
        &dirs.dispositions,
        DISPOSITIONS_ENTRY_LIMIT,
        "dispositions",
    )?;
    let revoked_names = listing(
        io,
        &dirs.revoked,
        REVOKED_ENTRY_LIMIT,
        "dispositions/revoked",
    )?;
    let archive_names = listing(io, &dirs.archive, format::ARCHIVE_ENTRY_LIMIT, "archive")?;
    check_names(
        &journal_names,
        |name| accept_journals_name(name, pool),
        "journals",
    )?;
    check_names(&disposition_names, accept_dispositions_name, "dispositions")?;
    check_names(
        &revoked_names,
        |name| format::parse_revoked_name(name).is_some(),
        "dispositions/revoked",
    )?;
    check_names(
        &archive_names,
        |name| ArchiveName::parse(name).is_some(),
        "archive",
    )?;
    let mut pool_stats = Vec::with_capacity(pool as usize);
    for (index, inode) in provision.pool_inodes.iter().enumerate() {
        let name = pool_name(index as u32);
        let stat = match io.stat_at(&dirs.journals, &name) {
            Ok(stat) => stat,
            Err(_) => return Err(lost(name)),
        };
        if stat.file_type != FileType::Regular {
            return Err(invalid(format!("{name} is a {:?}", stat.file_type)));
        }
        if (stat.uid, stat.gid, stat.mode, stat.nlink) != (provision.uid, provision.gid, 0o600, 1) {
            return Err(invalid(format!("{name} ownership")));
        }
        if stat.ino != *inode {
            return Err(lost(format!("{name} replaced")));
        }
        pool_stats.push(stat);
    }
    // Step 5: each pool file, locked, (owner) preserved, read and classified.
    let mut files = Vec::with_capacity(pool as usize);
    let mut generations = BTreeSet::new();
    for (index, stat) in pool_stats.iter().enumerate() {
        let index = index as u32;
        let name = pool_name(index);
        let opened;
        // A retained description is used only while it still names the inode
        // `PROVISION` records (a recycled pool file is a new inode).
        let still_held = held.get(&index).copied().filter(|file| {
            io.stat_file(file)
                .map(|now| now.same_inode(stat))
                .unwrap_or(false)
        });
        let file: &P::File = match still_held {
            Some(file) => file,
            None => {
                opened = open_checked(io, &dirs.journals, &name, stat)?;
                let request = if mode == ScanMode::Session {
                    LockRequest::Exclusive
                } else {
                    LockRequest::Shared
                };
                if io.flock(&opened, request).is_err() {
                    return Err(Refused::Live(name));
                }
                &opened
            }
        };
        if mode == ScanMode::Owner && io.fdatasync(file).is_err() {
            return Err(Refused::Unreliable(name));
        }
        let actual = io
            .stat_file(file)
            .map_err(|error| io_refusal(error, &name))?;
        if actual.size != size {
            return Err(invalid(format!("{name} size")));
        }
        let mut classifier = FileClassifier::new(provision.root_id, provision.c_pool, index);
        let mut block = vec![0u8; BLOCK];
        let blocks = size / BLOCK_U64;
        for k in 0..blocks {
            let mut done = 0;
            while done < BLOCK {
                let got = io
                    .pread(file, k * BLOCK_U64 + done as u64, &mut block[done..])
                    .map_err(|error| io_refusal(error, &name))?;
                if got == 0 {
                    return Err(invalid(format!("{name} size")));
                }
                done += got;
            }
            classifier
                .feed(&block)
                .map_err(|why| invalid(format!("{name}: {why}")))?;
        }
        let report = classifier.finish();
        if let Some(header) = &report.header {
            generations.insert(header.generation);
        }
        files.push(report);
    }
    // Step 6: archive entries, blocks 0 and 1 only.
    let mut archive = Vec::with_capacity(archive_names.len());
    let mut sorted_archive = archive_names.clone();
    sorted_archive.sort();
    for name in &sorted_archive {
        let parsed = ArchiveName::parse(name).ok_or_else(|| invalid(format!("archive {name}")))?;
        let stat = check_entry(
            io,
            &dirs.archive,
            name,
            Expect {
                file_type: FileType::Regular,
                uid: 0,
                gid: 0,
                mode: 0o444,
                ino: None,
            },
        )
        .map_err(|refused| match refused {
            Refused::Lost(why) | Refused::Invalid(why) => {
                invalid(format!("archive {name} type: {why}"))
            }
            other => other,
        })?;
        let file = open_checked(io, &dirs.archive, name, &stat)?;
        let mut blocks = vec![0u8; 2 * BLOCK];
        let mut done = 0;
        while done < blocks.len() {
            let got = io
                .pread(&file, done as u64, &mut blocks[done..])
                .map_err(|error| io_refusal(error, name))?;
            if got == 0 {
                break;
            }
            done += got;
        }
        blocks.truncate(done);
        if let Some(block) = blocks.get(0..BLOCK) {
            if let HeaderParse::Valid(header) = parse_header(block, &provision.root_id, None, None)
            {
                generations.insert(header.generation);
            }
        }
        archive.push(ArchiveEntry {
            name: name.clone(),
            parsed,
            blocks,
            size: stat.size,
        });
    }
    // Step 7: dispositions read and validated; revoked entries type-checked.
    let mut dispositions = BTreeMap::new();
    let mut sorted_dispositions = disposition_names.clone();
    sorted_dispositions.sort();
    for name in sorted_dispositions.iter().filter(|name| *name != "revoked") {
        let stat = check_entry(
            io,
            &dirs.dispositions,
            name,
            Expect {
                file_type: FileType::Regular,
                uid: 0,
                gid: 0,
                mode: 0o444,
                ino: None,
            },
        )
        .map_err(|refused| match refused {
            Refused::Lost(why) | Refused::Invalid(why) => {
                invalid(format!("disposition {name} type: {why}"))
            }
            other => other,
        })?;
        let file = open_checked(io, &dirs.dispositions, name, &stat)?;
        let data = read_bounded(io, &file, DISPOSITION_LIMIT)
            .map_err(|error| io_refusal(error, name))?
            .ok_or_else(|| invalid(format!("disposition {name} too large")))?;
        let disposition = parse_disposition(&data)
            .map_err(|error| invalid(format!("disposition {name}: {}", error.0)))?;
        if *name != format::disposition_name(&disposition.binding) {
            return Err(invalid(format!("disposition {name} name")));
        }
        if disposition.root != provision.root_id {
            return Err(invalid(format!("disposition {name} root")));
        }
        dispositions.insert(disposition.binding, disposition);
    }
    for name in &revoked_names {
        check_entry(
            io,
            &dirs.revoked,
            name,
            Expect {
                file_type: FileType::Regular,
                uid: 0,
                gid: 0,
                mode: 0o444,
                ino: None,
            },
        )
        .map_err(|refused| match refused {
            Refused::Lost(why) | Refused::Invalid(why) => {
                invalid(format!("revoked {name} type: {why}"))
            }
            other => other,
        })?;
    }
    // Step 8: pool-level checks over the complete set.
    let facts = PoolFacts {
        root_id: provision.root_id,
        c_pool: provision.c_pool,
        retired_through: provision.retired_through,
    };
    let level =
        pool_level(facts, &files, &archive, &dispositions, incident_limit).map_err(|refusal| {
            match refusal {
                PoolRefusal::Invalid(why) => Refused::Invalid(why),
                PoolRefusal::Capacity(why) => Refused::Capacity(why),
            }
        })?;
    let report = bounded_report(&level);
    Ok(ScanResult {
        files,
        archive,
        dispositions,
        revoked: revoked_names,
        level,
        report,
        generations,
        preservation_sync_returned_zero: mode == ScanMode::Owner,
    })
}

// ---------------------------------------------------------------------------
// The owner's startup (design section 10.2) and the claim (section 10.3)
// ---------------------------------------------------------------------------

/// What an owner keeps for the life of its process (design section 8.1):
/// the root and journals directories, the store lock description and, after
/// the claim, the claimed journal's lock description. Each lock is released
/// only when its description closes, which is at process exit: there is no
/// unlock and no early release here, and no description in it is ever
/// duplicated into worker code.
pub struct StoreGuard<P: StoreIo> {
    root: P::Dir,
    journals: Arc<P::Dir>,
    store_lock: P::File,
    journal_lock: Option<P::File>,
}

impl<P: StoreIo> StoreGuard<P> {
    pub(super) fn journals(&self) -> &Arc<P::Dir> {
        &self.journals
    }

    pub(super) fn root(&self) -> &P::Dir {
        &self.root
    }

    /// The store lock's description (for status and identity checks only).
    pub(super) fn store_lock(&self) -> &P::File {
        &self.store_lock
    }

    pub(super) fn journal_lock(&self) -> Option<&P::File> {
        self.journal_lock.as_ref()
    }
}

/// The bounded restart report a startup gives (design section 11.2).
#[derive(Debug, Clone)]
pub struct StartupReport {
    pub revision: u64,
    pub root_id: [u8; 16],
    pub files: Vec<FileReport>,
    pub report: BoundedReport,
    pub decision: Decision,
    pub activated: bool,
    /// Never "evidence durable" or "evidence preserved": only that the
    /// preservation syncs returned 0 (design section 4.6).
    pub preservation_sync_returned_zero: bool,
}

/// An owner's opened store: activated, scanned and decided, its store lock
/// held. Not yet claimed.
///
/// Retained, ownership-bearing state (P2-V1-R3B-I3-I1-R1): the opening's
/// identity, its revalidated selection, the guard with its store lock, the
/// verified scan, the decision over the complete incident set and the
/// storage admission are private. A caller reads them through borrows or
/// copies; editing a copy changes nothing here. Only the store's own claim
/// ([`Opened::claim`], internal) and validator ([`Opened::validator`]) use
/// them, and each derives what it needs from this state, never from a
/// caller's report, list or admission.
pub struct Opened<P: Platform> {
    opening: OpeningId,
    selection: Selection,
    guard: StoreGuard<P>,
    scan: ScanResult,
    decision: Decision,
    admission: StorageAdmission,
}

impl<P: Platform> std::fmt::Debug for Opened<P> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Opened")
            .field("opening", &self.opening)
            .field("revision", &self.selection.revision())
            .field("decision", &self.decision)
            .finish_non_exhaustive()
    }
}

/// The standalone verifier's result (design section 13.11): it never
/// activates and never claims, and what it read may not be durable.
#[derive(Debug, Clone)]
pub struct Verified {
    pub revision: u64,
    pub scan: ScanResult,
    pub decision: Decision,
    pub durability: &'static str,
}

pub const VERIFIER_DURABILITY_NOTE: &str =
    "the verifier reports the kernel-visible state; what it read may not be durable";

/// Open the store as an owner (design section 10.2 steps 0 to 8): select,
/// open the root, lock (`LOCK_EX | LOCK_NB`, before any journal is synced or
/// read), revalidate (at most three selections), activate, scan and decide.
/// Nothing is written; activation syncs directories and updates `LOCK`'s
/// timestamps, and the scan's preservation syncs make dirty pages durable.
pub fn open_owner<P: Platform>(
    io: &P,
    path: &ProvisionPath,
    config: &Config,
    hooks: &mut dyn OpeningHooks,
) -> Result<Opened<P>, Refusal> {
    let opening = OpeningId::next();
    if config.incident_limit > MAX_APPLIED {
        return Err(Refusal::new(unsupported("configuration"), None));
    }
    let mut revision = None;
    let fail = |refused: Refused, revision: Option<u64>| Refusal::new(refused, revision);
    let mut attempt = 0u32;
    let (selection, dirs, lock, observed) = loop {
        attempt += 1;
        if attempt > format::SELECTION_ATTEMPTS {
            return Err(fail(
                Refused::SelectionChanged(format!("after {} attempts", format::SELECTION_ATTEMPTS)),
                revision,
            ));
        }
        let selection = select(io, path).map_err(|refused| fail(refused, revision))?;
        revision = Some(selection.revision());
        if config.record_capacity > selection.provision.c_pool {
            return Err(fail(unsupported("configuration"), revision));
        }
        let OpenedRoot {
            dirs,
            admission: observed,
            ..
        } = open_root(io, path, &selection, opening, Pinning::Pinned)
            .map_err(|refused| fail(refused, revision))?;
        let lock_stat = check_lock_entry(io, &dirs.root, &selection.provision)
            .map_err(|refused| fail(refused, revision))?;
        let lock = open_checked(io, &dirs.root, "LOCK", &lock_stat)
            .map_err(|refused| fail(refused, revision))?;
        hooks.before_lock(attempt);
        if io.flock(&lock, LockRequest::Exclusive).is_err() {
            return Err(fail(Refused::Busy, revision));
        }
        if revalidate(io, path, &selection, &lock) {
            break (selection, dirs, lock, observed);
        }
        // A failed revalidation: every descriptor of this attempt closes,
        // which releases the lock, and selection starts again.
        drop(lock);
        drop(dirs);
    };
    let admission = activate(
        io,
        &Activation {
            path,
            selection: &selection,
            dirs: &dirs,
            lock: &lock,
            opening,
            observed: &observed,
            pinning: Pinning::Pinned,
        },
        hooks,
    )
    .map_err(|refused| fail(refused, revision))?;
    drop(observed);
    hooks.before_scan();
    let store_dirs = open_store_dirs(io, &dirs.root, &selection.provision)
        .map_err(|refused| fail(refused, revision))?;
    let scan = scan(
        io,
        &selection.provision,
        &store_dirs,
        ScanMode::Owner,
        &BTreeMap::new(),
        config.incident_limit,
    )
    .map_err(|refused| fail(refused, revision))?;
    let decision =
        decide(&scan.level, &scan.dispositions, config.incident_limit).map_err(|refusal| {
            match refusal {
                PoolRefusal::Invalid(why) => fail(Refused::Invalid(why), revision),
                PoolRefusal::Capacity(why) => fail(Refused::Capacity(why), revision),
            }
        })?;
    let StoreDirs { journals, .. } = store_dirs;
    Ok(Opened {
        opening,
        selection,
        guard: StoreGuard {
            root: dirs.root,
            journals: Arc::new(journals),
            store_lock: lock,
            journal_lock: None,
        },
        scan,
        decision,
        admission,
    })
}

/// The standalone verifier (design section 13.11): it selects and
/// revalidates, takes `LOCK_SH | LOCK_NB` on a new description (Busy when an
/// owner or a session holds the lock: never success), never activates,
/// writes and syncs nothing, and reports what is visible.
pub fn verify_standalone<P: Platform>(
    io: &P,
    path: &ProvisionPath,
    config: &Config,
) -> Result<Verified, Refusal> {
    let opening = OpeningId::next();
    let mut revision = None;
    let fail = |refused: Refused, revision: Option<u64>| Refusal::new(refused, revision);
    let mut attempt = 0u32;
    let (selection, dirs, lock) = loop {
        attempt += 1;
        if attempt > format::SELECTION_ATTEMPTS {
            return Err(fail(
                Refused::SelectionChanged("after 3 attempts".into()),
                revision,
            ));
        }
        let selection = select(io, path).map_err(|refused| fail(refused, revision))?;
        revision = Some(selection.revision());
        let OpenedRoot { dirs, .. } = open_root(io, path, &selection, opening, Pinning::Pinned)
            .map_err(|refused| fail(refused, revision))?;
        let lock_stat = check_lock_entry(io, &dirs.root, &selection.provision)
            .map_err(|refused| fail(refused, revision))?;
        let lock = open_checked(io, &dirs.root, "LOCK", &lock_stat)
            .map_err(|refused| fail(refused, revision))?;
        if io.flock(&lock, LockRequest::Shared).is_err() {
            return Err(fail(Refused::Busy, revision));
        }
        if revalidate(io, path, &selection, &lock) {
            break (selection, dirs, lock);
        }
    };
    let store_dirs = open_store_dirs(io, &dirs.root, &selection.provision)
        .map_err(|refused| fail(refused, revision))?;
    let scan = scan(
        io,
        &selection.provision,
        &store_dirs,
        ScanMode::Verifier,
        &BTreeMap::new(),
        config.incident_limit,
    )
    .map_err(|refused| fail(refused, revision))?;
    drop(lock);
    let decision =
        decide(&scan.level, &scan.dispositions, config.incident_limit).map_err(|refusal| {
            match refusal {
                PoolRefusal::Invalid(why) => fail(Refused::Invalid(why), revision),
                PoolRefusal::Capacity(why) => fail(Refused::Capacity(why), revision),
            }
        })?;
    Ok(Verified {
        revision: selection.revision(),
        scan,
        decision,
        durability: VERIFIER_DURABILITY_NOTE,
    })
}

impl<P: Platform> Opened<P> {
    /// The startup report (a copy).
    pub fn report(&self) -> StartupReport {
        StartupReport {
            revision: self.selection.revision(),
            root_id: self.selection.provision.root_id,
            files: self.scan.files.clone(),
            report: self.scan.report.clone(),
            decision: self.decision.clone(),
            activated: true,
            preservation_sync_returned_zero: self.scan.preservation_sync_returned_zero,
        }
    }

    /// The selection this opening revalidated (read-only).
    pub fn selection(&self) -> &Selection {
        &self.selection
    }

    /// The scan this opening verified (read-only).
    pub fn scan(&self) -> &ScanResult {
        &self.scan
    }

    /// The decision over the complete incident set (read-only).
    pub fn decision(&self) -> &Decision {
        &self.decision
    }

    /// This opening's identity (a copy: it names the opening, it grants
    /// nothing).
    pub fn opening(&self) -> OpeningId {
        self.opening
    }

    /// This opening's storage admission (read-only: it is not `Clone`, and no
    /// claim accepts an admission from a caller).
    pub fn admission(&self) -> &StorageAdmission {
        &self.admission
    }

    /// The validator over this opening's own verified dispositions: it lives
    /// no longer than the opening, so no longer than the store lock it holds.
    pub fn validator(&self) -> StoreValidator<'_> {
        StoreValidator::of(
            &self.selection.provision.root_id,
            &self.scan.level.incidents,
            &self.scan.dispositions,
        )
    }

    /// The current incidents, as the core's prior incidents, in binding
    /// order: every one of them, never a truncated list.
    pub(super) fn priors(&self) -> Vec<PriorIncident> {
        classify::current_incidents(&self.scan.level)
            .into_iter()
            .map(|(binding, incident)| PriorIncident {
                binding: IncidentBinding::new(binding),
                outcome: incident.outcome,
            })
            .collect()
    }

    /// The current incidents with their facts.
    pub(super) fn current(&self) -> Vec<([u8; 32], Incident)> {
        classify::current_incidents(&self.scan.level)
    }

    /// The bindings and reasons the claim's header lists: each dispositioned
    /// current incident's, from this opening's verified dispositions.
    pub(super) fn applied(&self) -> Vec<([u8; 32], DispositionReason)> {
        applied_reasons(&self.decision, &self.scan.dispositions)
    }

    /// 16 bytes from `getrandom`, not all zero and not any generation in a
    /// scanned header; at most eight draws (design section 7.7).
    pub(super) fn draw_generation(&self, io: &P) -> Result<Generation, Refusal> {
        for _ in 0..8 {
            let mut bytes = [0u8; 16];
            io.random(&mut bytes).map_err(|error| {
                self.refusal(Refused::ClaimFailed(format!("generation: {error}")))
            })?;
            if bytes.iter().any(|byte| *byte != 0) && !self.scan.generations.contains(&bytes) {
                return Ok(Generation::new(bytes));
            }
        }
        Err(self.refusal(Refused::ClaimFailed("generation".into())))
    }

    fn refusal(&self, refused: Refused) -> Refusal {
        Refusal::new(refused, Some(self.selection.revision()))
    }

    /// The claim handoff (design section 10.3), internal to the store: with
    /// this opening's own admission, its own decision, and the applied
    /// dispositions derived from its own verified state. It consumes the
    /// opening: one claim per opening.
    pub(super) fn claim(
        self,
        io: &P,
        config: &Config,
        generation: Generation,
    ) -> Result<Claim<P>, Refusal> {
        let applied = self.applied();
        let Opened {
            opening,
            selection,
            guard,
            scan,
            decision,
            admission,
        } = self;
        let parts = ClaimParts {
            opening,
            selection,
            guard,
            scan,
            decision,
        };
        claim_with(io, parts, &admission, config, generation, &applied)
    }
}

/// A claim ready for the recorder: the guard (with the journal lock), the
/// worker's I/O description, and the header block the worker writes.
/// Internal to the store: only [`super::owner::start_owner`] receives one.
pub(super) struct Claim<P: Platform> {
    pub(super) guard: StoreGuard<P>,
    pub(super) io_file: P::File,
    pub(super) journal_name: String,
    pub(super) journal_stat: Stat,
    pub(super) index: u32,
    pub(super) claim: u64,
    pub(super) header: HeaderFields,
    pub(super) header_block: Box<[u8; BLOCK]>,
    pub(super) selection: Selection,
}

/// An opened store's parts, without its own admission.
struct ClaimParts<P: Platform> {
    opening: OpeningId,
    selection: Selection,
    guard: StoreGuard<P>,
    scan: ScanResult,
    decision: Decision,
}

fn claim_with<P: Platform>(
    io: &P,
    parts: ClaimParts<P>,
    admission: &StorageAdmission,
    config: &Config,
    generation: Generation,
    applied: &[([u8; 32], DispositionReason)],
) -> Result<Claim<P>, Refusal> {
    let ClaimParts {
        opening,
        selection,
        mut guard,
        scan,
        decision,
    } = parts;
    let revision = Some(selection.revision());
    let fail = |refused: Refused| Refusal::new(refused, revision);
    if !verify_admission(Some(admission), opening, &selection.digest) {
        return Err(fail(unsupported("storage admission not verified")));
    }
    if !decision.blocking.is_empty() {
        return Err(fail(Refused::PriorUnresolved(decision.blocking.clone())));
    }
    let Some(index) = scan
        .files
        .iter()
        .find(|file| file.class == FileClass::Unused)
        .map(|file| file.index)
    else {
        return Err(fail(Refused::PoolExhausted));
    };
    let Some(claim) = scan
        .level
        .max_claim
        .checked_add(1)
        .filter(|claim| *claim <= MAX_CLAIM)
    else {
        return Err(fail(Refused::ClaimExhausted));
    };
    let generation_bytes = generation.bytes();
    if generation_bytes.iter().all(|byte| *byte == 0)
        || scan.generations.contains(&generation_bytes)
    {
        return Err(fail(Refused::ClaimFailed("generation".into())));
    }
    let name = pool_name(index);
    let journals = Arc::clone(&guard.journals);
    let stat = io
        .stat_at(&journals, &name)
        .map_err(|error| fail(io_refusal(error, &name)))?;
    if stat.ino != selection.provision.pool_inodes[index as usize] {
        return Err(fail(Refused::ClaimFailed("journal replaced".into())));
    }
    // A new lock description, never the scan's.
    let lock = open_checked(io, &journals, &name, &stat).map_err(fail)?;
    if io.flock(&lock, LockRequest::Exclusive).is_err() {
        return Err(fail(Refused::ClaimFailed("journal lock".into())));
    }
    // The worker's own I/O description: no lock is ever taken through it.
    let io_file = io
        .open_write(&journals, &name)
        .map_err(|error| fail(Refused::ClaimFailed(format!("I/O description: {error}"))))?;
    let io_stat = io
        .stat_file(&io_file)
        .map_err(|error| fail(Refused::ClaimFailed(format!("I/O description: {error}"))))?;
    if !io_stat.same_inode(&stat) {
        return Err(fail(Refused::ClaimFailed("identity".into())));
    }
    let size = pool_file_size(selection.provision.c_pool).unwrap_or(0);
    let mut block = vec![0u8; BLOCK];
    for k in 0..size / BLOCK_U64 {
        let mut done = 0;
        while done < BLOCK {
            let got = io
                .pread(&io_file, k * BLOCK_U64 + done as u64, &mut block[done..])
                .map_err(|error| fail(Refused::ClaimFailed(format!("re-read: {error}"))))?;
            if got == 0 {
                return Err(fail(Refused::ClaimFailed("content".into())));
            }
            done += got;
        }
        if block.iter().any(|byte| *byte != 0) {
            return Err(fail(Refused::ClaimFailed("content".into())));
        }
    }
    let mut applied: Vec<([u8; 32], DispositionReason)> = applied.to_vec();
    applied.sort_by(|a, b| a.0.cmp(&b.0));
    let current: BTreeSet<[u8; 32]> = decision.current.iter().copied().collect();
    if applied.len() != current.len()
        || applied
            .iter()
            .any(|(binding, _)| !current.contains(binding))
    {
        return Err(fail(Refused::ClaimFailed(
            "the header must list every current incident".into(),
        )));
    }
    let mut boot_id = [0u8; 16];
    io.random(&mut boot_id)
        .map_err(|error| fail(Refused::ClaimFailed(format!("boot id: {error}"))))?;
    let header = HeaderFields {
        root_id: selection.provision.root_id,
        generation: generation_bytes,
        claim,
        pool_index: index,
        c_pool: selection.provision.c_pool,
        capacity: config.record_capacity,
        created_ms: 0,
        boot_id,
        applied,
    };
    let header_block = encode_header(&header)
        .map_err(|error| fail(Refused::ClaimFailed(format!("header: {}", error.0))))?;
    guard.journal_lock = Some(lock);
    drop((scan, decision));
    Ok(Claim {
        guard,
        io_file,
        journal_name: name,
        journal_stat: stat,
        index,
        claim,
        header,
        header_block,
        selection,
    })
}

/// The reasons the header records: each current incident's disposition's.
pub(super) fn applied_reasons(
    decision: &Decision,
    dispositions: &BTreeMap<[u8; 32], Disposition>,
) -> Vec<([u8; 32], DispositionReason)> {
    decision
        .dispositioned
        .iter()
        .filter_map(|binding| {
            dispositions
                .get(binding)
                .map(|disposition| (*binding, disposition.reason))
        })
        .collect()
}

/// The outcome the core receives for each current incident (plain data).
pub fn outcome_of(incident: &Incident) -> PriorOutcome {
    incident.outcome
}
