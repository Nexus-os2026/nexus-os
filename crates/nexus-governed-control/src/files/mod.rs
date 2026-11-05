//! M2d-B1: the broker's Files domain, create-only (design v2 §6.1-6.2,
//! amended by the Owner's D1 with Codex's conditions): in the one empty
//! folder the Owner registered in this process, an agent run admitted to a
//! mission the Owner consented to may ask to create one new small UTF-8
//! text file at the folder's top level. Nothing here replaces, deletes,
//! renames or makes a folder.
//!
//! - Workspace selection ([`Files::register`]): the Owner's own selection,
//!   from the front door only. Exactly `/`, `$HOME` or the identity home is
//!   refused; a reserved tree (overlap in either direction) is refused; the
//!   folder must exist, be its own canonical path (no link), be the current
//!   user's and be empty. One per process; nothing is persisted.
//! - Preparation ([`Files::prepare`]): the run's admission (never the
//!   intent), the admitted workspace still the same directory, a live Files
//!   grant on it, the name and content rules, the name absent, and the
//!   complete escaped display, refused whole if it cannot be shown.
//!
//! M2d-B2 (§6.1 steps 4-6, §6.3-§6.7): this module is the one broker source
//! that names the kernel's coding-run primitive (P1A-NC-30's exact-path
//! exception, M2d-K).
//! - Registration also registers the folder with a kernel project registry
//!   that reserves the broker's state.
//! - Preparation stages the file in a kernel run (`FilesEffect`), whose
//!   ledger and staging parent are in the broker's own runtime root.
//! - Execution (one at a time) carries the Owner's approval of the
//!   commitment into the kernel through `BrokerApproved`. It publishes
//!   with the kernel's observed apply, whose final check asks the
//!   `Observer`: the broker's kernel stop, the run's cancellation, the
//!   commitment's deadline on both clocks and its coverage. A cause landing
//!   before that check publishes nothing; one after it lets only that one
//!   publication complete, recorded as it happened, and nothing waits for
//!   it.
//! - The last file is kept for the Owner's undo (once, this process only,
//!   fresh authority).
//! - The startup report reads the ledger and sweeps nothing.
//! - Nothing that a Stop path drops does filesystem or ledger work.
//!
//! Overlap with a protected or reserved location is decided by directory
//! identity (`st_dev`, `st_ino`) along each directory's ancestor chain, not
//! by its spelling (Codex review 2 of PR #57): a bind mount or a casefolded
//! name gives a directory a second canonical spelling, never a second
//! identity. The spelling checks stay as defense in depth.
//!
//! Supported topology, and only it (Codex reviews 3 to 5 of PR #57, the
//! Owner's Option 1): the workspace and every protected and reserved
//! location are each on a plain local ext4, xfs or tmpfs filesystem,
//! mounted once, whole, on real hardware, including dm-crypt, LVM and md
//! (its device listed exactly once in `/proc/self/mountinfo`, with root
//! `/`, no other mount between it and the location, and, for ext4 or xfs,
//! its block device and everything under it in sysfs real disks or
//! partitions, never a loop, nbd, zram, ublk or ram device). Everything else
//! is refused (`unsupported_mount_topology`): bind mounts of every kind,
//! stacked or hidden mounts, overlay, btrfs, fuse, network and image
//! filesystems, virtual block devices, and a mount table or sysfs node that
//! cannot be read or resolved. Both are read at selection.
//!
//! Not claimed, finally: any other storage arrangement made by root or the
//! Owner is not protected (a mount, link, rename or casefold change after
//! the selection, a device's storage rearranged underneath it, an inode
//! number reused after its directory is deleted, and whatever else the
//! rules above do not name). No further topology rules will be added (the
//! Owner's decision, Option 1).
//!
//! Path validation is not an OS sandbox: these checks bound what the broker
//! asks the kernel for; they are not confinement.

#[cfg(all(test, target_os = "linux"))]
mod b2_tests;
#[cfg(test)]
mod tests;

use crate::authority::approval::ControlConfirmer;
#[cfg(target_os = "linux")]
use crate::authority::approval::UndoConfirmation;
use crate::authority::commitment::MAX_SUMMARY_LINES;
#[cfg(target_os = "linux")]
use crate::authority::commitment::{ExecutionGuard, FailureClass, PreparedAction, TargetIdentity};
use crate::authority::effect::CapabilityKind;
#[cfg(target_os = "linux")]
use crate::authority::effect::EffectClass;
use crate::authority::evidence::{hint, is_plain, quoted, wrapped};
use crate::authority::file_session::HaltCell;
#[cfg(target_os = "linux")]
use crate::authority::file_session::RecoveryCheck;
use crate::authority::ids::{random16, CommitmentId, Digest, GrantId, RunId};
use crate::authority::policy::GrantScope;
use crate::authority::run::{AdmittedWorkspace, FilesAdmission};
use crate::authority::{Authority, AuthorityError};
use crate::control::Preparation;
#[cfg(target_os = "linux")]
use crate::control::{EffectOutput, PendingEffect};
#[cfg(target_os = "linux")]
use nexus_kernel::coding_run::ProjectGrant;
#[cfg(target_os = "linux")]
use nexus_kernel::coding_run::{
    analyze_apply, ApplyError, ApplyFinding, ApplyInterrupt, ApplyState, CandidateEdit, ChangeKind,
    CleanupStatus, CodingRun, ConfirmationKind, ConfirmationRequest, FolderPicker, InertPageError,
    InertPageResult, InterruptCause, LedgerFailure, LedgerFile, ManifestEntry, OwnerConfirmer,
    PreparedFile, ProjectError, ProjectInfo, ProjectRegistry, Refusal, RelPath, RevocationReason,
    RunError, RunScopes, ScopeEntry, ScopeSet, StagingParent, TempLeft, VerifierMarker,
};
use nexus_kernel::emergency_stop::StopAuthority;
#[cfg(target_os = "linux")]
use nexus_kernel::emergency_stop::StopWatch;
#[cfg(any(test, target_os = "linux"))]
use nexus_kernel::inert_page::ApprovedFacts;
#[cfg(target_os = "linux")]
use nexus_kernel::inert_page::{LABEL_PASSED, MOTHER_CSS};
#[cfg(target_os = "linux")]
use nexus_kernel::workspace_authority::{
    WorkspaceAuthorityRegistry, WorkspaceBinding, WorkspaceGrantId,
};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
#[cfg(target_os = "linux")]
use std::sync::PoisonError;
use std::sync::{Arc, Mutex};
use std::time::Duration;
#[cfg(target_os = "linux")]
use std::time::Instant;

/// Establish the kernel's no-dump invariant before registering a File workspace.
/// This only narrows process access; it grants no workspace authority.
#[cfg(target_os = "linux")]
pub fn make_non_dumpable() -> bool {
    nexus_kernel::coding_run::make_non_dumpable()
}

/// The largest file Files creates.
pub const MAX_FILE_BYTES: usize = 8 * 1024;
/// The most lines a created file may have.
pub const MAX_FILE_LINES: usize = 150;
/// The longest file name.
pub const MAX_NAME_CHARS: usize = 64;
/// The kernel's staging prefix: no file name may start with it.
const TEMP_PREFIX: &str = ".nexus-coding-run-tmp-";
/// The digest domain of a Files commitment's parameters (I9).
pub(crate) const PARAMETERS_DOMAIN: &str = "nexus.files.parameters.v1";
const IDENTITY_DOMAIN: &str = "nexus.files.workspace.identity.v1";

/// An agent's request to create one file, as data. Its `Debug` shows only
/// sizes: neither the name nor the content.
#[derive(Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FileCreate {
    pub name: String,
    pub content: String,
}

impl fmt::Debug for FileCreate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileCreate")
            .field("name_bytes", &self.name.len())
            .field("content_bytes", &self.content.len())
            .finish()
    }
}

/// S1a-D (O4): what Files may create, by the name's extension. A page
/// (`*.html`, lowercase) is checked with the inert page check; plain text
/// (`*.txt`, lowercase) is not; any other name (another extension, an
/// upper-case or short spelling of a page, or none) is refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileClass {
    Page,
    Plain,
    Refused,
}

impl FileClass {
    /// The class of the file name `name` (one root-level component): by
    /// its last extension, exactly and case-sensitively.
    pub fn of(name: &str) -> Self {
        match name.rsplit_once('.') {
            Some((stem, "html")) if !stem.is_empty() => FileClass::Page,
            Some((stem, "txt")) if !stem.is_empty() => FileClass::Plain,
            _ => FileClass::Refused,
        }
    }
}

/// S1a-D's line beside every checked page's label (S1a design v2 §7 step
/// 3): what the inert page check does not cover.
pub const PAGE_NOT_CHECKED: &str =
    "not checked: how the page looks in a browser (375 px and 1366 px are OPEN)";
/// What a checked page's dialog says in place of S1a's facts line when its
/// admission holds no facts.
#[cfg(target_os = "linux")]
const NO_FACTS_LINE: &str = "facts: not checked (none in the consent)";

/// S1a-D+B (M3 §6.2): the broker's receipt for a Files create that
/// succeeded: the published bytes' SHA-256 and size, the policy hash of the
/// kernel's inert page verdict on them (`None` for plain text), and whether
/// a cause overtook the publication after its final check (the effect's
/// own interruption codes, as its end record carries them). Digests and a
/// size only.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FileReceipt {
    pub sha256: [u8; 32],
    pub bytes: u64,
    pub policy_hash: Option<[u8; 32]>,
    pub overtaken: bool,
}

/// What the interface and Mother's port may know of the workspace.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct WorkspaceInfo {
    pub id: String,
    pub name: String,
}

/// What a workspace may not be (§6.2): exactly `/`, `home` or
/// `identity_home`; within or containing any of `reserved`; or owned by
/// another user than `uid`.
pub(crate) struct Exclusions {
    pub(crate) home: PathBuf,
    pub(crate) identity_home: PathBuf,
    pub(crate) reserved: Vec<PathBuf>,
    pub(crate) uid: u32,
}

/// §6.1 step 3: `[A-Za-z0-9_-][A-Za-z0-9._-]{0,63}`, not the kernel's
/// staging prefix: one component at the workspace's top level.
pub(crate) fn name_ok(name: &str) -> bool {
    let first = |b: u8| b.is_ascii_alphanumeric() || b == b'_' || b == b'-';
    let bytes = name.as_bytes();
    !bytes.is_empty()
        && bytes.len() <= MAX_NAME_CHARS
        && first(bytes[0])
        && bytes[1..].iter().all(|&b| first(b) || b == b'.')
        && !name.starts_with(TEMP_PREFIX)
}

/// The lines of `content`, as an editor counts them (a final newline ends
/// the last line; it does not start another).
pub(crate) fn line_count(content: &str) -> usize {
    content.split_inclusive('\n').count()
}

/// §6.1 step 3: text (UTF-8 by type, no NUL), at most `MAX_FILE_BYTES`
/// and `MAX_FILE_LINES`.
pub(crate) fn content_ok(content: &str) -> Result<(), AuthorityError> {
    if content.len() > MAX_FILE_BYTES {
        return Err(AuthorityError::InvalidAction(
            "the file is larger than 8 KiB",
        ));
    }
    if line_count(content) > MAX_FILE_LINES {
        return Err(AuthorityError::InvalidAction(
            "the file has more than 150 lines",
        ));
    }
    if content.contains('\0') {
        return Err(AuthorityError::InvalidAction("the file is not text"));
    }
    Ok(())
}

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest as _, Sha256};
    hex::encode(Sha256::digest(bytes))
}

/// `wall_ms` as a UTC date and time (days to civil date, proleptic
/// Gregorian).
fn utc(wall_ms: u64) -> String {
    let secs = wall_ms / 1000;
    let days = i64::try_from(secs / 86_400).unwrap_or(i64::MAX / 2);
    let rem = secs % 86_400;
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02} {:02}:{:02}:{:02} UTC",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

/// §6.1 step 4, what the Owner reads before approving: the mission's
/// contract hash in full, the consent's time, record and kind, the request,
/// the workspace's name and id, the name, the exact bytes and lines and the
/// full sha256, then the complete file, escaped and `│`-prefixed
/// ([`quoted`]). Refused whole (`change_too_large_to_show`) when it needs
/// more than `MAX_SUMMARY_LINES` lines or a line is not plain: never cut.
pub(crate) fn display(
    admission: &FilesAdmission,
    workspace: &WorkspaceInfo,
    name: &str,
    content: &str,
) -> Result<Vec<String>, AuthorityError> {
    let mut lines = vec![
        format!("Mission contract: {}", admission.contract_hash),
        format!(
            "Consented: {} (record {}, {})",
            utc(admission.consented_wall_ms),
            admission.evidence_seq,
            admission.kind.as_str()
        ),
    ];
    lines.extend(wrapped(&format!("Request: {}", admission.request)));
    lines.extend(wrapped(&format!(
        "Workspace: {} ({})",
        workspace.name, workspace.id
    )));
    lines.extend(wrapped(&format!(
        "File: {name} (new, at the workspace's top level)"
    )));
    lines.push(format!(
        "Size: {} bytes, {} lines; ends with a newline: {}",
        content.len(),
        line_count(content),
        if content.ends_with('\n') { "yes" } else { "no" }
    ));
    lines.push(format!("SHA-256: {}", sha256_hex(content.as_bytes())));
    lines.extend(quoted("New file, complete", content));
    if lines.len() > MAX_SUMMARY_LINES || !lines.iter().all(|line| is_plain(line)) {
        return Err(AuthorityError::ChangeTooLargeToShow);
    }
    Ok(lines)
}

/// What a Files commitment's parameters digest binds (I9): the admission
/// (every field; S1a-D+B: its page facts by their hash), the broker run,
/// the kernel coding run, the Files grant,
/// both halves of the kernel workspace binding, the full review hash and
/// the name.
pub(crate) struct Components<'a> {
    pub(crate) admission: &'a FilesAdmission,
    pub(crate) broker_run: RunId,
    pub(crate) kernel_run: [u8; 16],
    pub(crate) grant: GrantId,
    pub(crate) binding_agent: [u8; 16],
    pub(crate) binding_run: [u8; 16],
    pub(crate) review_hash: [u8; 32],
    pub(crate) name: &'a str,
}

/// The parameters digest of a Files commitment: a pure function of its
/// components, each length-prefixed under `PARAMETERS_DOMAIN`; what the
/// staged effect digests (M2d-B2).
#[cfg_attr(not(any(test, target_os = "linux")), allow(dead_code))]
pub(crate) fn parameters_digest(components: &Components<'_>) -> Digest {
    let admission = components.admission;
    // S1a-D+B (S1a O8): tag 0 with no facts, tag 1 and their hash with.
    let facts = admission.page_facts.as_ref().map_or_else(
        || vec![0u8],
        |facts| [&[1u8][..], &facts.hash()[..]].concat(),
    );
    Digest::of(
        PARAMETERS_DOMAIN,
        &[
            admission.contract_hash.as_bytes(),
            admission.request.as_bytes(),
            &admission.evidence_seq.to_be_bytes(),
            admission.kind.as_str().as_bytes(),
            &admission.consented_wall_ms.to_be_bytes(),
            admission.workspace.id.as_bytes(),
            admission.workspace.identity.as_bytes(),
            &facts,
            components.broker_run.as_bytes(),
            &components.kernel_run,
            components.grant.as_bytes(),
            &components.binding_agent,
            &components.binding_run,
            &components.review_hash,
            components.name.as_bytes(),
        ],
    )
}

// ---------------------------------------------------------------- M2d-B2

/// The agent half of the kernel workspace binding of every Files run: the
/// Files consumer is Mother, named by these fixed 16 bytes (the ASCII of
/// `MotherNexusFiles`). The run half is the broker run's own 16 bytes, so
/// a kernel grant resolves only for the broker run it was issued to.
pub(crate) const KERNEL_AGENT: [u8; 16] = *b"MotherNexusFiles";
/// How long a Files commitment may wait to start. Its deadline also bounds
/// the publication (§6.3): it is checked again at the final check.
pub(crate) const FILE_TTL: Duration = Duration::from_secs(10 * 60);
/// The longest startup hold before a publication (R3).
pub const MAX_HOLD: Duration = Duration::from_secs(30);
/// The slice a hold waits in before it looks again.
#[cfg(target_os = "linux")]
const HOLD_SLICE: Duration = Duration::from_millis(50);
/// Most working details kept; the oldest goes first.
const MAX_DETAILS: usize = 64;
/// Most publication receipts kept; the oldest goes first.
const MAX_RECEIPTS: usize = 64;
/// The broker's Files state under its runtime root (reserved from every
/// workspace): the coding-run ledger and the staging parent.
const STATE_DIR: &str = "files";
#[cfg(target_os = "linux")]
const LEDGER_FILE: &str = "ledger.db";
/// The digest domain of a Files commitment's target (the workspace and the
/// name).
#[cfg(target_os = "linux")]
const TARGET_DOMAIN: &str = "nexus.files.target.v1";

/// The approval bridge (§6.4): the Owner's R2 approval of a Files
/// commitment, carried into the kernel's confirmation. It says yes only to
/// an apply of exactly the committed review (all 32 bytes of its hash).
/// It is built at one site, inside [`FilesEffect`]'s `execute`, which runs
/// only under the commitment's execution guard: `begin` refuses a
/// commitment that is not authorized, so a record of an approval alone
/// gives nothing. It cannot be copied or duplicated.
#[cfg(target_os = "linux")]
pub(crate) struct BrokerApproved {
    review_hash: [u8; 32],
}

#[cfg(target_os = "linux")]
impl OwnerConfirmer for BrokerApproved {
    fn confirm(&self, request: &ConfirmationRequest) -> bool {
        request.kind == ConfirmationKind::Apply && request.review_hash == self.review_hash
    }
}

/// One run's findings in the startup recovery report (§6.7): ids and
/// digests only.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecoveryEntry {
    /// The kernel coding run's id.
    pub run: String,
    pub findings: Vec<RecoveryFinding>,
}

/// What the Files ledger says is uncertain or owed about one run. Reporting
/// only: nothing is swept, and nothing is called disposable.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RecoveryFinding {
    /// A publication was prepared and has no recorded outcome: whether the
    /// file was published, and where its temporary file is, is unknown.
    PreparedWithoutOutcome { files: Vec<TempRef> },
    /// An undo was prepared and has no recorded outcome.
    RestorePreparedWithoutOutcome { files: Vec<TempRef> },
    /// Temporary files whose removal was not established.
    CleanupOwed { temps: Vec<TempRef> },
    /// The Owner's own versions kept under temporary names: never cleanup.
    OwnerVersionKept { versions: Vec<TempRef> },
    /// Recovery is required for another recorded reason (a code).
    RecoveryRequired { failed: &'static str },
    /// A verified file awaited approval; its staging may remain.
    StagingAwaitingApproval,
    /// In this process: the run's staging could not be removed; Files
    /// keeps owning it until the Owner's retry removes it (Codex review 1
    /// of PR #64, M1).
    StagingNotDiscarded,
    /// In this process: a kernel write grant issued for this run's undo
    /// could not be confirmed closed, whether the undo was refused, its
    /// restore returned, or it unwound; it stays live until it expires
    /// (Codex re-check 2 of PR #64, HIGH 2; re-check 3).
    WriteGrantNotClosed,
}

/// What a completed undo came to (§6.6, M4′): `interrupted` names the
/// kernel write grant's lapse after U1 (`write_grant_revoked` or
/// `write_grant_expired`), if it lapsed; the file was removed either way.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct UndoReport {
    pub interrupted: Option<&'static str>,
}

/// What one Owner retry of owed Files cleanup came to (§6.6, Codex review
/// 1 of PR #64, M1): each owed run got one bounded attempt.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CleanupRetry {
    /// Runs whose cleanup is now established (no longer owned).
    pub settled: usize,
    /// Runs still owed (their cleanup still fails, or recovery is required
    /// for another reason): still owned and reported.
    pub still_owed: usize,
}

/// A file of a finding: its path digest and the backend-random id of its
/// temporary name, as recorded.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TempRef {
    pub path_sha256: Option<String>,
    pub temp_id: Option<String>,
}

/// Where the Owner reads a working detail (the host's screen).
pub trait OwnerView {
    fn line(&mut self, label: &str, value: &str);
}

/// The Owner-only detail behind a Files failure code (I4): held in memory,
/// taken once, shown only through an [`OwnerView`]. It has no `Debug`,
/// `Display`, `Clone` or `Serialize`, and nothing persists it.
pub struct WorkingDetail {
    lines: Lines,
}

impl WorkingDetail {
    pub fn show(self, owner: &mut dyn OwnerView) {
        for (label, value) in self.lines {
            owner.line(label, &value);
        }
    }
}

/// The workspace as the kernel registered it: its own project registry
/// (with the broker's state reserved) and the one project in it.
#[cfg(target_os = "linux")]
struct Kernel {
    projects: ProjectRegistry,
    /// The registry the project's grants come from: an undo closes a write
    /// grant it issued and did not use (HIGH 2).
    authority: Arc<WorkspaceAuthorityRegistry>,
    project: ProjectInfo,
}

#[cfg(target_os = "linux")]
struct Picked(PathBuf);

#[cfg(target_os = "linux")]
impl FolderPicker for Picked {
    fn pick_folder(&self) -> Option<PathBuf> {
        Some(self.0.clone())
    }
}

#[cfg(target_os = "linux")]
impl Kernel {
    /// Register `root` with a fresh kernel registry that reserves the
    /// broker's runtime root (its Files state included) and `reserved`.
    fn register(
        root: &Path,
        runtime_root: &Path,
        reserved: &[PathBuf],
        halt: Option<&HaltCell>,
    ) -> Result<Self, AuthorityError> {
        let authority = Arc::new(WorkspaceAuthorityRegistry::new());
        let mut trees: Vec<&Path> = reserved.iter().map(PathBuf::as_path).collect();
        trees.push(runtime_root);
        let projects = ProjectRegistry::reserving(Arc::clone(&authority), &trees);
        let picked = Picked(root.to_path_buf());
        crate::authority::file_session::admit_host(halt)?;
        let project = projects
            .select(&picked)
            .map_err(|_| refused("the kernel refused the workspace"))?;
        Ok(Self {
            projects,
            authority,
            project,
        })
    }
}

/// The broker's Files ledger and staging parent, opened once.
#[cfg(target_os = "linux")]
struct Backend {
    ledger: LedgerFile,
    staging: StagingParent,
}

/// Open (creating owner-only) `<runtime root>/files`, its ledger and its
/// staging parent; anything that is not a real private directory of ours
/// refuses.
#[cfg(target_os = "linux")]
fn open_backend(runtime_root: &Path) -> Result<Backend, AuthorityError> {
    use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};
    let unavailable =
        || AuthorityError::Unavailable("the Files ledger or staging cannot be opened");
    let dir = runtime_root.join(STATE_DIR);
    match std::fs::DirBuilder::new().mode(0o700).create(&dir) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(_) => return Err(unavailable()),
    }
    let meta = std::fs::symlink_metadata(&dir).map_err(|_| unavailable())?;
    if !meta.is_dir()
        || meta.uid() != crate::launcher::effective_uid()
        || meta.permissions().mode() & 0o077 != 0
    {
        return Err(unavailable());
    }
    Ok(Backend {
        ledger: LedgerFile::open(&dir.join(LEDGER_FILE)).map_err(|_| unavailable())?,
        staging: StagingParent::within(&dir).map_err(|_| unavailable())?,
    })
}

/// The last file created in this process, the undo slot (§6.6).
#[cfg(target_os = "linux")]
struct Applied {
    run: CodingRun,
    /// The broker run that created it (closed for Files if its undo's
    /// write grant lapsed after U1).
    broker_run: RunId,
    workspace: Arc<Workspace>,
    binding: WorkspaceBinding,
    name: String,
    bytes: u64,
    sha256: String,
    review_hash: [u8; 32],
}

/// A working detail's labelled lines.
type Lines = Vec<(&'static str, String)>;

/// What Files executions, the undo and the report share. Every lock here
/// is held only to move a value in or out: never across the ledger, the
/// filesystem, a dialog or evidence.
#[derive(Default)]
struct Shared {
    /// The ledger and staging parent, opened at first use.
    #[cfg(target_os = "linux")]
    backend: Mutex<Option<Arc<Backend>>>,
    /// The undo slot.
    #[cfg(target_os = "linux")]
    slot: Mutex<Option<Applied>>,
    /// Runs that owe cleanup (temps, or their staging) or require recovery:
    /// kept, never dropped, until the Owner's retry establishes their
    /// cleanup.
    #[cfg(target_os = "linux")]
    kept: Mutex<Vec<CodingRun>>,
    /// Kernel runs whose undo left a write grant that could not be
    /// confirmed closed (HIGH 2): reported until the process ends.
    #[cfg(target_os = "linux")]
    unclosed: Mutex<Vec<String>>,
    /// Broker runs a cause interrupted after a publication's final check:
    /// no further Files request of theirs is accepted (Codex review 1 of PR
    /// #64, H3; M4′).
    closed: Mutex<std::collections::HashSet<RunId>>,
    /// Runs of effects dropped elsewhere (a Stop, a cancellation, a sweep):
    /// owned here until the next Files call discards their staging.
    #[cfg(target_os = "linux")]
    deferred: Mutex<Vec<CodingRun>>,
    /// The Owner-only details of failed executions.
    details: Mutex<VecDeque<(CommitmentId, Lines)>>,
    /// S1a-D+B: the receipts of the latest published creates.
    receipts: Mutex<VecDeque<(CommitmentId, FileReceipt)>>,
    /// S-O22's hook (tests): the facts the next preparation checks its page
    /// with, instead of its admission's.
    #[cfg(test)]
    prepare_facts: Mutex<Option<Option<ApprovedFacts>>>,
    /// One Files execution or undo at a time (Codex D2 condition 3).
    busy: AtomicBool,
    /// The startup hold before each publication, in milliseconds.
    hold_ms: AtomicU64,
    #[cfg(test)]
    hooks: Arc<crate::test_hooks::Hooks>,
    /// Tests: the closure of an undo's issued write grant fails.
    #[cfg(all(test, target_os = "linux"))]
    closure_fails: AtomicBool,
    /// Tests: the last write grant issued, to revoke at a hook.
    #[cfg(all(test, target_os = "linux"))]
    write_grant: Mutex<
        Option<(
            Arc<WorkspaceAuthorityRegistry>,
            WorkspaceGrantId,
            WorkspaceBinding,
        )>,
    >,
}

/// The one Files operation under way; another is refused, never queued.
struct Busy<'a>(&'a AtomicBool);

impl<'a> Busy<'a> {
    fn take(flag: &'a AtomicBool) -> Option<Self> {
        flag.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .ok()
            .map(|_| Self(flag))
    }
}

impl Drop for Busy<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

impl Shared {
    fn hold(&self) -> Duration {
        Duration::from_millis(self.hold_ms.load(Ordering::SeqCst))
    }

    fn detail(&self, id: CommitmentId, lines: Lines) {
        let mut details = self.details.lock().expect("files details");
        details.push_back((id, lines));
        while details.len() > MAX_DETAILS {
            details.pop_front();
        }
    }

    #[cfg_attr(not(target_os = "linux"), allow(dead_code))]
    fn receipt(&self, id: CommitmentId, receipt: FileReceipt) {
        let mut receipts = self.receipts.lock().expect("files receipts");
        receipts.push_back((id, receipt));
        while receipts.len() > MAX_RECEIPTS {
            receipts.pop_front();
        }
    }

    /// A run Files no longer needs: one that owes cleanup or requires
    /// recovery is kept; any other has its staging discarded now (recorded)
    /// and is dropped only once that removal is established; otherwise it
    /// is kept too (A6, Codex review 1 of PR #64, M1). Returns whether it
    /// was kept.
    #[cfg(target_os = "linux")]
    fn retire(&self, mut run: CodingRun) -> bool {
        if owes(&run) {
            self.keep(run);
            return true;
        }
        if !discard(&mut run, self) {
            self.keep(run);
            return true;
        }
        false
    }

    #[cfg(target_os = "linux")]
    fn keep(&self, run: CodingRun) {
        self.kept.lock().expect("files kept").push(run);
    }

    fn is_closed(&self, run: RunId) -> bool {
        self.closed.lock().expect("files closed").contains(&run)
    }

    /// Put `applied` in the undo slot; the one it replaces is retired.
    #[cfg(target_os = "linux")]
    fn applied(&self, applied: Applied) {
        let replaced = self.slot.lock().expect("files slot").replace(applied);
        if let Some(old) = replaced {
            let _kept = self.retire(old.run);
        }
    }

    #[cfg(all(test, target_os = "linux"))]
    fn note_write_grant(&self, authority: &Arc<WorkspaceAuthorityRegistry>, grant: &ProjectGrant) {
        *self.write_grant.lock().unwrap() =
            Some((Arc::clone(authority), grant.grant_id(), grant.binding()));
    }

    #[cfg(test)]
    fn fire(&self, at: &'static str) {
        self.hooks.fire(at);
    }
}

/// A staged Files create (§6.1 steps 4-6), held as a pending effect until
/// the Owner's approved commitment executes it once.
#[cfg(target_os = "linux")]
struct FilesEffect {
    /// The staged kernel run; taken by `execute`, or handed to the deferred
    /// list when the effect is dropped (never cleaned on the dropping path,
    /// which may be a Stop).
    run: Mutex<Option<CodingRun>>,
    workspace: Arc<Workspace>,
    backend: Arc<Backend>,
    shared: Arc<Shared>,
    stop: Arc<StopAuthority>,
    admission: FilesAdmission,
    broker_run: RunId,
    kernel_run: [u8; 16],
    grant: GrantId,
    review_hash: [u8; 32],
    name: String,
    bytes: u64,
    lines: usize,
    sha256: String,
    /// The reviewed candidate's manifest entry: its SHA-256 and size, equal
    /// to the input shown (checked at prepare).
    candidate: ManifestEntry,
    /// S1a-D: a page's recorded inert page pass, as prepare's check
    /// returned it (`None` for plain text). Its binding hash is the review's
    /// marker, so the Owner's approval covers it.
    page: Option<InertPageResult>,
    target: Digest,
}

#[cfg(target_os = "linux")]
impl FilesEffect {
    fn components(&self) -> Components<'_> {
        Components {
            admission: &self.admission,
            broker_run: self.broker_run,
            kernel_run: self.kernel_run,
            grant: self.grant,
            binding_agent: KERNEL_AGENT,
            binding_run: *self.broker_run.as_bytes(),
            review_hash: self.review_hash,
            name: &self.name,
        }
    }

    fn binding(&self) -> WorkspaceBinding {
        WorkspaceBinding::from_bytes(KERNEL_AGENT, *self.broker_run.as_bytes())
    }

    /// A failed execution: the Owner's detail is kept; the caller gets the
    /// class and a code.
    fn failed(
        &self,
        guard: &ExecutionGuard,
        class: FailureClass,
        code: String,
        detail: String,
    ) -> (FailureClass, String) {
        self.failed_with(guard, (class, code), detail, false)
    }

    /// [`Self::failed`], saying whether the run's cleanup is owed (M1).
    fn failed_with(
        &self,
        guard: &ExecutionGuard,
        (class, code): (FailureClass, String),
        detail: String,
        owed: bool,
    ) -> (FailureClass, String) {
        let mut lines = vec![
            ("File", self.name.clone()),
            ("Outcome", code.clone()),
            ("Detail", detail),
        ];
        if owed {
            lines.push((
                "Cleanup",
                "owed: Files keeps this run until the Owner's retry establishes it".to_string(),
            ));
        }
        self.shared.detail(guard.commitment(), lines);
        (class, code)
    }

    /// Nothing was published: the run is retired (kept if its cleanup is
    /// owed) and the failure is a class and a code.
    fn not_published(
        &self,
        guard: &ExecutionGuard,
        run: CodingRun,
        error: &ApplyError,
    ) -> (FailureClass, String) {
        let kept = self.shared.retire(run);
        self.failed_with(guard, apply_failure(error), error.to_string(), kept)
    }
}

/// Whether a run still owes cleanup of temps or requires recovery.
#[cfg(target_os = "linux")]
fn owes(run: &CodingRun) -> bool {
    !run.cleanup_owed().is_empty() || run.apply_state() == ApplyState::RecoveryRequired
}

/// Discard a run's staging (cancelling it first if it is still live);
/// whether its removal is established. Never called on a Stop path: a
/// dropped effect only hands its run over (H1 decision, requirement 2).
#[cfg(target_os = "linux")]
fn discard(run: &mut CodingRun, _shared: &Shared) -> bool {
    if run.cleanup_status() == CleanupStatus::Discarded {
        return true;
    }
    #[cfg(test)]
    _shared.fire(crate::test_hooks::at::FILE_DISCARD);
    if !run.state().is_terminal() {
        let _ = run.cancel();
    }
    #[cfg(test)]
    _shared.fire("session.before.discard_staging");
    #[cfg(test)]
    _shared.fire("session.entered.discard_staging");
    let result = run.discard_staging();
    #[cfg(test)]
    _shared.fire("session.after.discard_staging");
    matches!(result, Ok(CleanupStatus::Discarded))
}

/// What a publication attempt came to for the file at the name (Codex
/// review 1 of PR #64, H2), decided from every kernel outcome with no
/// catch-all: a new kernel outcome must be classified here.
#[cfg(target_os = "linux")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Publication {
    /// The file is at the name; `recovery`: its durability, or the write
    /// grant's closure, is not established.
    Published { recovery: bool },
    /// Nothing was published, or the file was removed again, established.
    NotPublished,
    /// The file may be present: recovery is required. Never reported as a
    /// plain failure.
    Uncertain,
}

/// The outcome of an attempt that ended in `error` (a successful one is
/// a publication).
#[cfg(target_os = "linux")]
fn publication(error: &ApplyError) -> Publication {
    match error {
        // Refused, or rolled back with the removal established, before any
        // file stayed at the name.
        ApplyError::InvalidState => Publication::NotPublished,
        ApplyError::Refused(_) => Publication::NotPublished,
        ApplyError::RolledBack { .. } => Publication::NotPublished,
        ApplyError::Interrupted { .. } => Publication::NotPublished,
        // Published, and only its directory sync failed: the file is in
        // place (and undoable).
        ApplyError::RecoveryRequired { failed, .. } if failed == "dir_sync" => {
            Publication::Published { recovery: true }
        }
        // A failed verification, or a compensating unpublication that did
        // not complete: the file may be at the name.
        ApplyError::RecoveryRequired { .. } => Publication::Uncertain,
        // The filesystem work is done but not recorded: not established.
        ApplyError::CompletionUnrecorded { .. } => Publication::Uncertain,
        // Moved aside by a compensation whose removal is not established.
        ApplyError::CleanupOwed { .. } => Publication::Uncertain,
        ApplyError::AuthorityNotClosed { after } => match after.as_ref() {
            Ok(_) => Publication::Published { recovery: true },
            Err(inner) => match publication(inner) {
                Publication::Published { .. } => Publication::Published { recovery: true },
                Publication::NotPublished => Publication::NotPublished,
                Publication::Uncertain => Publication::Uncertain,
            },
        },
    }
}

/// The kernel write grant's lapse that M2d-K2 found when the kernel closed
/// it at the end of `run`'s last attempt, as an interruption code. K2 also
/// reports a refused attempt's grant, so it is read only together with a
/// publication or a completed undo, whose final check (U1) required the
/// grant to resolve: then a lapse came after that check.
#[cfg(target_os = "linux")]
fn write_grant_lapse(run: &CodingRun) -> Option<&'static str> {
    match run.write_grant_at_close()? {
        InterruptCause::Expired => Some("write_grant_expired"),
        _ => Some("write_grant_revoked"),
    }
}

/// A cause's code in a success's controlled interruption status (H3).
#[cfg(target_os = "linux")]
fn interruption(cause: InterruptCause) -> &'static str {
    match cause {
        InterruptCause::EmergencyStop => "emergency_stop",
        InterruptCause::Cancelled => "cancelled",
        InterruptCause::Revoked => "revoked",
        InterruptCause::Expired => "expired",
        InterruptCause::CommitmentExpired => "commitment_expired",
    }
}

/// An apply's failure as a class and a code (never a name or a path).
#[cfg(target_os = "linux")]
fn apply_failure(error: &ApplyError) -> (FailureClass, String) {
    match error {
        ApplyError::Interrupted { cause } => (
            FailureClass::Refused,
            format!("interrupted:{}", cause.code()),
        ),
        ApplyError::Refused(
            refusal @ (Refusal::EmergencyStopped
            | Refusal::Interrupted(_)
            | Refusal::AuthorityDenied
            | Refusal::ProcessDumpable),
        ) => (FailureClass::Refused, format!("refused:{}", refusal.code())),
        ApplyError::Refused(refusal) => (
            FailureClass::TargetChanged,
            format!("refused:{}", refusal.code()),
        ),
        other => (FailureClass::Actuator, other.code().to_string()),
    }
}

/// S1a-D: an inert page check refused at prepare, as the caller's refusal:
/// S1a's code, never content. Exhaustive: a new kernel outcome must choose.
#[cfg(target_os = "linux")]
fn inert_refusal(error: InertPageError) -> AuthorityError {
    match error {
        InertPageError::Refused(code) => AuthorityError::PageNotInert(code.as_str()),
        InertPageError::FactMissing(code) => AuthorityError::PageFactMissing(code.as_str()),
        InertPageError::EmergencyStopped => AuthorityError::EmergencyStopped,
        InertPageError::Unrecorded => {
            AuthorityError::Unavailable("the inert page check could not be recorded")
        }
        InertPageError::InvalidState
        | InertPageError::Spent
        | InertPageError::NotChecked
        | InertPageError::FactsChanged
        | InertPageError::Stale
        | InertPageError::CandidateUnavailable
        | InertPageError::NotSingleFile => {
            AuthorityError::Unavailable("the inert page check could not be completed")
        }
    }
}

/// S1a-D (§7 step 4): a failed execute-time recheck as a class and the code
/// `page_recheck:<code>`.
#[cfg(target_os = "linux")]
fn recheck_failure(error: InertPageError) -> (FailureClass, String) {
    let class = match error {
        InertPageError::EmergencyStopped => FailureClass::Refused,
        InertPageError::Unrecorded => FailureClass::Unavailable,
        InertPageError::InvalidState
        | InertPageError::Spent
        | InertPageError::NotChecked
        | InertPageError::FactsChanged
        | InertPageError::Stale
        | InertPageError::CandidateUnavailable
        | InertPageError::NotSingleFile
        | InertPageError::Refused(_)
        | InertPageError::FactMissing(_) => FailureClass::TargetChanged,
    };
    (class, format!("page_recheck:{}", error.code()))
}

/// S1a-D (§7 step 3): what a checked page's dialog says of its check, after
/// the file: S1a's label, the policy and stylesheet hashes, S1a's facts line
/// (or that no facts were consented), and what the check does not cover.
#[cfg(target_os = "linux")]
fn page_lines(page: &InertPageResult) -> Vec<String> {
    vec![
        LABEL_PASSED.to_string(),
        format!(
            "policy {}  css {}",
            hex::encode(page.page.policy_hash),
            sha256_hex(MOTHER_CSS.as_bytes())
        ),
        page.page
            .facts_line()
            .unwrap_or_else(|| NO_FACTS_LINE.to_string()),
        PAGE_NOT_CHECKED.to_string(),
    ]
}

#[cfg(target_os = "linux")]
impl PendingEffect for FilesEffect {
    /// The workspace is still the admitted directory, the name still
    /// absent, and the staged candidate still reviews to exactly the
    /// committed review hash (§6.1 step 6).
    fn revalidate(&self) -> Result<Digest, AuthorityError> {
        if self.shared.is_closed(self.broker_run) {
            return Err(AuthorityError::RunNotActive);
        }
        if identity_of(&self.workspace.root)? != self.admission.workspace.identity {
            return Err(AuthorityError::TargetChanged);
        }
        match std::fs::symlink_metadata(self.workspace.root.join(&self.name)) {
            Ok(_) => return Err(AuthorityError::FileExists),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err(AuthorityError::Unavailable("the workspace cannot be read")),
        }
        let mut run = self.run.lock().unwrap_or_else(PoisonError::into_inner);
        let run = run
            .as_mut()
            .ok_or(AuthorityError::Unavailable("the staged file is gone"))?;
        #[cfg(test)]
        self.shared.fire("session.before.revalidation_review");
        #[cfg(test)]
        self.shared.fire("session.entered.revalidation_review");
        let review = run.review().map_err(|_| AuthorityError::TargetChanged)?;
        #[cfg(test)]
        self.shared.fire("session.after.revalidation_review");
        if review.binding.hash() != self.review_hash || review.single_complete().is_none() {
            return Err(AuthorityError::TargetChanged);
        }
        Ok(self.target)
    }

    fn parameters(&self) -> Digest {
        parameters_digest(&self.components())
    }

    /// §6.1 step 6: one at a time; the bridge (`request_approval` with
    /// [`BrokerApproved`]); a fresh kernel write grant; then the kernel's
    /// observed apply with this execution's [`Observer`] at its final check.
    fn execute(
        mut self: Box<Self>,
        guard: &ExecutionGuard,
    ) -> Result<EffectOutput, (FailureClass, String)> {
        let shared = Arc::clone(&self.shared);
        #[cfg(test)]
        shared.fire(crate::test_hooks::at::FILE_EXECUTE_START);
        let Some(_busy) = Busy::take(&shared.busy) else {
            return Err(self.failed(
                guard,
                FailureClass::Refused,
                "busy".into(),
                "another Files operation is under way".into(),
            ));
        };
        let taken = self
            .run
            .get_mut()
            .unwrap_or_else(PoisonError::into_inner)
            .take();
        let Some(mut run) = taken else {
            return Err(self.failed(
                guard,
                FailureClass::Abandoned,
                "staging_gone".into(),
                "the staged file is gone".into(),
            ));
        };
        // H3's closure, again once the slot is held (Codex re-check 2 of PR
        // #64, HIGH 1): an execution already past its revalidation must
        // still see a run that another execution closed meanwhile. Only an
        // execution holding the slot closes a run, so this answer holds
        // through the handoff.
        if shared.is_closed(self.broker_run) {
            let kept = shared.retire(run);
            return Err(self.failed_with(
                guard,
                (FailureClass::Refused, "run_closed".into()),
                "Files is closed for this run: an earlier publication's authority ended after its handoff".into(),
                kept,
            ));
        }
        let kernel = &self.workspace.kernel;
        let binding = self.binding();
        // The bridge's one construction site (§6.4).
        let hash = self.review_hash;
        let approved = BrokerApproved { review_hash: hash };
        #[cfg(test)]
        shared.fire("session.before.request_approval");
        #[cfg(test)]
        shared.fire("session.entered.request_approval");
        let approval = match run.request_approval(&kernel.project.name, &approved) {
            Ok(approval) => approval,
            Err(error) => return Err(self.not_published(guard, run, &error)),
        };
        #[cfg(test)]
        shared.fire("session.after.request_approval");
        // S1a-D (§7 step 4): a page is rechecked with its admission's facts
        // (never prepare's) immediately before its publication: only the
        // kernel write grant is issued between them. Its pass must still be
        // the one the Owner approved. Any failure publishes nothing, spends
        // the run's check and discards its staging.
        if let Some(page) = &self.page {
            #[cfg(test)]
            shared.fire("session.before.page_recheck");
            #[cfg(test)]
            shared.fire("session.entered.page_recheck");
            let rechecked = run
                .recheck_inert_page(self.admission.page_facts.as_ref())
                .and_then(|()| match run.latest_inert_page() {
                    Some(now) if now == page => Ok(()),
                    Some(_) | None => Err(InertPageError::Stale),
                });
            #[cfg(test)]
            shared.fire("session.after.page_recheck");
            if let Err(error) = rechecked {
                let kept = shared.retire(run);
                return Err(self.failed_with(
                    guard,
                    recheck_failure(error),
                    error.to_string(),
                    kept,
                ));
            }
        }
        #[cfg(test)]
        shared.fire("session.before.write_grant");
        #[cfg(test)]
        shared.fire("session.entered.write_grant");
        let Ok(write) = kernel.projects.grant_for_apply(kernel.project.id, binding) else {
            let kept = shared.retire(run);
            return Err(self.failed_with(
                guard,
                (FailureClass::Unavailable, "write_grant".into()),
                "the kernel issued no write grant".into(),
                kept,
            ));
        };
        #[cfg(test)]
        shared.fire("session.after.write_grant");
        #[cfg(test)]
        shared.note_write_grant(&kernel.authority, &write);
        let observer = Observer::new(guard, &self.stop, shared.hold(), &shared);
        guard.handing_off();
        #[cfg(test)]
        shared.fire("session.before.apply");
        #[cfg(test)]
        shared.fire("session.entered.apply");
        let result = run.apply_observed(approval, write, &self.backend.staging, &observer);
        #[cfg(test)]
        shared.fire("session.after.apply");
        let recovery = match result {
            Ok(_) => false,
            Err(error) => match publication(&error) {
                Publication::Published { recovery } => {
                    self.failed(
                        guard,
                        FailureClass::Actuator,
                        "recovery_required".into(),
                        error.to_string(),
                    );
                    recovery
                }
                Publication::NotPublished => {
                    guard.proven_not_published();
                    return Err(self.not_published(guard, run, &error));
                }
                Publication::Uncertain => {
                    // The file may be present: never a plain failure, and
                    // the run is kept (never retired or discarded).
                    shared.keep(run);
                    return Err(self.failed(
                        guard,
                        FailureClass::Uncertain,
                        "uncertain".into(),
                        format!("the file may be present: recovery required ({error})"),
                    ));
                }
            },
        };
        // H3 (M4′): a cause that arrived after the handoff is reported
        // beside the truthful success, and Files ends for this run.
        // M4′ (HIGH 3 of Codex re-check 2): the kernel write grant's lapse
        // after the final check, as the kernel found it at its close; read
        // only here, together with the publication.
        let interrupted: Vec<&'static str> = observer
            .cause()
            .map(interruption)
            .into_iter()
            .chain(write_grant_lapse(&run))
            .collect();
        if !interrupted.is_empty() {
            shared
                .closed
                .lock()
                .expect("files closed")
                .insert(self.broker_run);
        }
        // S1a-D+B (M3 §6.2): the receipt, from the verified candidate the
        // kernel published (for either class; a page's verdict names the
        // same bytes) and, for a page, its verdict's policy; overtaken
        // exactly when this effect reports an interruption beside its
        // success (the same codes its end record carries).
        shared.receipt(
            guard.commitment(),
            FileReceipt {
                sha256: self.candidate.sha256,
                bytes: self.candidate.size,
                policy_hash: self.page.as_ref().map(|page| page.page.policy_hash),
                overtaken: !interrupted.is_empty(),
            },
        );
        shared.applied(Applied {
            run,
            broker_run: self.broker_run,
            workspace: Arc::clone(&self.workspace),
            binding,
            name: self.name.clone(),
            bytes: self.bytes,
            sha256: self.sha256.clone(),
            review_hash: self.review_hash,
        });
        let mut meta = vec![
            ("op".to_string(), "create".to_string()),
            ("bytes".to_string(), self.bytes.to_string()),
            ("lines".to_string(), self.lines.to_string()),
        ];
        if recovery {
            meta.push(("reason".to_string(), "recovery_required".to_string()));
        }
        for code in interrupted {
            meta.push(("interrupted".to_string(), code.to_string()));
        }
        Ok(EffectOutput {
            meta,
            ..EffectOutput::default()
        })
    }
}

#[cfg(target_os = "linux")]
impl Drop for FilesEffect {
    /// Hand an unexecuted run over, in memory only: whoever drops the
    /// effect (a Stop, a cancellation, a door halt, a sweep) does no
    /// filesystem or ledger work; the next Files call discards the staging.
    fn drop(&mut self) {
        let taken = self
            .run
            .get_mut()
            .unwrap_or_else(PoisonError::into_inner)
            .take();
        if let Some(run) = taken {
            self.shared
                .deferred
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .push(run);
        }
    }
}

/// The execution guard's view for the kernel's final check (I8): asked
/// once, after the candidate is written and synced and after the Owner's
/// startup hold, if any. Memory only: no lock is held, nothing is written.
#[cfg(target_os = "linux")]
struct Observer<'a> {
    guard: &'a ExecutionGuard,
    /// The broker's kernel stop, watched from this execution's start
    /// (`None`: it was in force then).
    watch: Option<StopWatch>,
    hold: Duration,
    #[cfg_attr(not(test), allow(dead_code))]
    shared: &'a Shared,
}

#[cfg(target_os = "linux")]
impl<'a> Observer<'a> {
    fn new(
        guard: &'a ExecutionGuard,
        stop: &Arc<StopAuthority>,
        hold: Duration,
        shared: &'a Shared,
    ) -> Self {
        Self {
            guard,
            watch: stop.admit().ok(),
            hold,
            shared,
        }
    }

    /// What interrupts the publication, in order: the broker's kernel stop
    /// (engaged since this execution started, or in force), the run's
    /// cancellation, the commitment's deadline on either clock, then its
    /// coverage (a grant expired, or revoked, or the policy generation
    /// moved).
    fn cause(&self) -> Option<InterruptCause> {
        match &self.watch {
            Some(watch) if !watch.halted() && !watch.authority().is_stopped() => {}
            _ => return Some(InterruptCause::EmergencyStop),
        }
        if self.guard.is_cancelled() {
            return Some(InterruptCause::Cancelled);
        }
        if self.guard.expired() {
            return Some(InterruptCause::CommitmentExpired);
        }
        if !self.guard.still_authorized() {
            return Some(if self.guard.grant_expired() {
                InterruptCause::Expired
            } else {
                InterruptCause::Revoked
            });
        }
        None
    }

    /// R3: wait out the startup hold in bounded slices, ending at once when
    /// any cause appears (a stop wakes it immediately).
    fn hold(&self) {
        if self.hold.is_zero() {
            return;
        }
        let until = Instant::now() + self.hold;
        while self.cause().is_none() {
            let now = Instant::now();
            let Some(watch) = self.watch.as_ref().filter(|_| now < until) else {
                return;
            };
            watch.wait_halted_timeout((until - now).min(HOLD_SLICE));
        }
    }
}

#[cfg(target_os = "linux")]
impl ApplyInterrupt for Observer<'_> {
    fn interrupted(&self) -> Option<InterruptCause> {
        #[cfg(test)]
        self.shared.fire(crate::test_hooks::at::FILE_BEFORE_FINAL);
        self.hold();
        let cause = self.cause();
        // The real refusal boundary: a cause here refuses the handoff
        // before anything is published, recorded or cleaned.
        #[cfg(test)]
        if cause.is_some() {
            self.shared
                .fire(crate::test_hooks::at::FILE_HANDOFF_REFUSED);
        }
        cause
    }

    fn passed_final_check(&self) {
        #[cfg(test)]
        self.shared.fire(crate::test_hooks::at::FILE_AFTER_FINAL);
    }
}

/// A kernel write grant an undo was issued, armed for the grant's whole life
/// (Codex re-check 2 of PR #64, HIGH 2; re-check 3, the Architect's design:
/// "armed for the grant's whole life, outcome always recorded"). It is armed
/// from issue across the kernel's restore until [`IssuedGrant::close`]
/// settles it explicitly; any other exit, an unwind included, settles it in
/// `Drop`. Settling always attempts the closure and always records a closure
/// that is not established as [`RecoveryFinding::WriteGrantNotClosed`] in the
/// list the recovery report reads; `Drop` never panics.
///
/// The kernel's restore takes the `ProjectGrant` by value (it closes it
/// itself), so the guard keeps the grant's identity, its id and binding
/// (both `Copy`), which is all a closure needs: the registry's `revoke` and
/// `resolve` take only these. Closing after the kernel's own closure is
/// harmless: `revoke` is idempotent (it keeps the first `revoked_at` and
/// answers `Ok` for a grant already revoked or expired), so the second
/// closure only confirms the first.
#[cfg(target_os = "linux")]
struct IssuedGrant<'a> {
    registry: &'a WorkspaceAuthorityRegistry,
    id: WorkspaceGrantId,
    binding: WorkspaceBinding,
    /// The kernel run, for a recorded closure that is not established.
    run: String,
    armed: bool,
    shared: &'a Shared,
}

#[cfg(target_os = "linux")]
impl<'a> IssuedGrant<'a> {
    fn new(
        registry: &'a WorkspaceAuthorityRegistry,
        grant: &ProjectGrant,
        run: String,
        shared: &'a Shared,
    ) -> Self {
        Self {
            registry,
            id: grant.grant_id(),
            binding: grant.binding(),
            run,
            armed: true,
            shared,
        }
    }

    /// Settle it now: whether its closure is established (a failure is
    /// already recorded; the caller answers recovery required).
    fn close(mut self) -> bool {
        self.settle()
    }

    /// Close it, revoked or confirmed not to resolve any more (revoked or
    /// expired already), and record a closure that is not established. The
    /// list's lock is held only to push, and a poisoned list is still
    /// written: nothing here panics.
    fn settle(&mut self) -> bool {
        if !std::mem::replace(&mut self.armed, false) {
            return true;
        }
        let closed = self.closure();
        if !closed {
            self.shared
                .unclosed
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .push(std::mem::take(&mut self.run));
        }
        closed
    }

    fn closure(&self) -> bool {
        #[cfg(test)]
        if self.shared.closure_fails.load(Ordering::SeqCst) {
            return false;
        }
        self.registry.revoke(self.id, self.binding).is_ok()
            || self.registry.resolve(self.id, self.binding).is_err()
    }
}

#[cfg(target_os = "linux")]
impl Drop for IssuedGrant<'_> {
    fn drop(&mut self) {
        let _ = self.settle();
    }
}

/// An undo's lifetime, taken when it starts, before any cleanup or dialog
/// (Codex review 1 of PR #64, H4; H2′): the broker's kernel stop, watched
/// from then, and the policy generation then. It ends at any Stop since
/// (even one already resumed) or any policy change; checks are memory only.
#[cfg(target_os = "linux")]
struct UndoLifetime {
    recovery: RecoveryCheck,
    watch: StopWatch,
    generation: u64,
}

#[cfg(target_os = "linux")]
impl UndoLifetime {
    fn take(
        authority: &Authority,
        stop: &Arc<StopAuthority>,
        cell: Option<&HaltCell>,
    ) -> Result<Self, AuthorityError> {
        let recovery = RecoveryCheck::take(cell)?;
        let watch = stop.admit().map_err(|_| AuthorityError::EmergencyStopped)?;
        if authority.runs().is_stopped() {
            return Err(AuthorityError::EmergencyStopped);
        }
        Ok(Self {
            recovery,
            watch,
            generation: authority.policy_generation(),
        })
    }

    fn check(&self, authority: &Authority) -> Result<(), AuthorityError> {
        self.recovery.check()?;
        if self.watch.halted()
            || self.watch.authority().is_stopped()
            || authority.runs().is_stopped()
        {
            return Err(AuthorityError::EmergencyStopped);
        }
        if authority.policy_generation() != self.generation {
            return Err(AuthorityError::Stale);
        }
        Ok(())
    }
}

/// The undo's confirmation (§6.5, §6.6): the kernel's restore request,
/// checked against the broker's own record of the file, shown to the Owner
/// with every fact.
#[cfg(target_os = "linux")]
struct UndoBridge<'a> {
    confirmer: &'a dyn ControlConfirmer,
    workspace: &'a str,
    name: &'a str,
    bytes: u64,
    sha256: &'a str,
    review_hash: [u8; 32],
    /// Whether the undo's lifetime still holds: checked before the Owner
    /// is asked (H4).
    live: &'a dyn Fn() -> bool,
}

#[cfg(target_os = "linux")]
impl OwnerConfirmer for UndoBridge<'_> {
    fn confirm(&self, request: &ConfirmationRequest) -> bool {
        let Some(identity) = request.identity else {
            return false;
        };
        if !(self.live)()
            || request.kind != ConfirmationKind::Restore
            || request.review_hash != self.review_hash
            || request.bytes != Some(self.bytes)
            || request.sha256_full.as_deref() != Some(self.sha256)
        {
            return false;
        }
        self.confirmer.confirm_undo(&UndoConfirmation {
            workspace: self.workspace.to_string(),
            file: self.name.to_string(),
            bytes: self.bytes,
            sha256: self.sha256.to_string(),
            device: identity.dev,
            inode: identity.ino,
        })
    }
}

/// An undo's refusal or failure, as a broker class.
#[cfg(target_os = "linux")]
fn undo_error(error: ApplyError) -> AuthorityError {
    match error {
        ApplyError::Refused(Refusal::Declined) => AuthorityError::Declined,
        ApplyError::Refused(
            Refusal::EmergencyStopped | Refusal::Interrupted(InterruptCause::EmergencyStop),
        ) => AuthorityError::EmergencyStopped,
        ApplyError::Refused(Refusal::Interrupted(_) | Refusal::AuthorityDenied) => {
            AuthorityError::GrantNotLive
        }
        ApplyError::Refused(
            Refusal::Stale(_) | Refusal::Redirect(_) | Refusal::IdentityChanged,
        ) => AuthorityError::TargetChanged,
        ApplyError::Refused(Refusal::Unrecorded) => {
            AuthorityError::Unavailable("the Files ledger refused the record")
        }
        ApplyError::InvalidState => {
            AuthorityError::Unavailable("there is no file to undo in this session")
        }
        ApplyError::Refused(_) => AuthorityError::Unavailable("the undo was refused"),
        _ => AuthorityError::RecoveryRequired,
    }
}

/// A kernel run's failure while staging, as a broker class.
#[cfg(target_os = "linux")]
fn run_error(error: RunError) -> AuthorityError {
    match error {
        RunError::Revoked(RevocationReason::EmergencyStop) => AuthorityError::EmergencyStopped,
        _ => AuthorityError::Unavailable("the file cannot be staged"),
    }
}

#[cfg(target_os = "linux")]
fn project_error(error: ProjectError) -> AuthorityError {
    match error {
        ProjectError::IdentityChanged => AuthorityError::TargetChanged,
        _ => AuthorityError::Unavailable("the kernel issued no workspace grant"),
    }
}

#[cfg(target_os = "linux")]
fn ledger_error(error: LedgerFailure) -> AuthorityError {
    match error {
        LedgerFailure::Integrity => AuthorityError::RecoveryRequired,
        _ => AuthorityError::Unavailable("the Files ledger cannot be read"),
    }
}

#[cfg(target_os = "linux")]
fn finding(finding: ApplyFinding) -> RecoveryFinding {
    let prepared = |files: Vec<PreparedFile>| {
        files
            .into_iter()
            .map(|file| TempRef {
                path_sha256: Some(file.path_sha256),
                temp_id: file.temp_id,
            })
            .collect()
    };
    let left = |temps: Vec<TempLeft>| {
        temps
            .into_iter()
            .map(|temp| TempRef {
                path_sha256: temp.path_sha256,
                temp_id: Some(temp.temp_id),
            })
            .collect()
    };
    match finding {
        ApplyFinding::PreparedWithoutOutcome { files } => RecoveryFinding::PreparedWithoutOutcome {
            files: prepared(files),
        },
        ApplyFinding::RestorePreparedWithoutOutcome { files } => {
            RecoveryFinding::RestorePreparedWithoutOutcome {
                files: prepared(files),
            }
        }
        ApplyFinding::CleanupOwed { temps } => RecoveryFinding::CleanupOwed { temps: left(temps) },
        ApplyFinding::OwnerVersionKept { versions } => RecoveryFinding::OwnerVersionKept {
            versions: left(versions),
        },
        ApplyFinding::RecoveryRequired { failed } => RecoveryFinding::RecoveryRequired { failed },
        ApplyFinding::StagingAwaitingApproval => RecoveryFinding::StagingAwaitingApproval,
    }
}

/// §6.1 step 4's kernel half: grant, snapshot, the one created file,
/// structural verification, and a review that shows exactly that file whole.
/// Returns the review's full hash. S1a-D (§7 step 2): a page is checked
/// once, after structural verification, with `facts` (its admission's);
/// its review must then name exactly that pass, and plain text's no
/// verification. The page's pass is returned with the hash. Codex review 1
/// of PR #74: for either class, the reviewed candidate (the bytes the
/// kernel verified and would publish) must be exactly the input the Owner
/// is shown, its SHA-256 and size; its entry is returned too, for the
/// receipt.
#[cfg(target_os = "linux")]
fn stage_one(
    run: &mut CodingRun,
    (staging, _shared): (&StagingParent, &Shared),
    path: &RelPath,
    content: &[u8],
    (class, facts): (FileClass, Option<ApprovedFacts>),
) -> Result<([u8; 32], Option<InertPageResult>, ManifestEntry), AuthorityError> {
    #[cfg(test)]
    _shared.fire("session.before.staging_grant");
    #[cfg(test)]
    _shared.fire("session.entered.staging_grant");
    run.grant(staging).map_err(run_error)?;
    #[cfg(test)]
    _shared.fire("session.after.staging_grant");
    #[cfg(test)]
    _shared.fire("session.before.snapshot");
    #[cfg(test)]
    _shared.fire("session.entered.snapshot");
    run.snapshot().map_err(run_error)?;
    #[cfg(test)]
    _shared.fire("session.after.snapshot");
    #[cfg(test)]
    _shared.fire("session.before.edit");
    #[cfg(test)]
    _shared.fire("session.entered.edit");
    run.edit(CandidateEdit::Create {
        path: path.clone(),
        content: content.to_vec(),
    })
    .map_err(run_error)?;
    #[cfg(test)]
    _shared.fire("session.after.edit");
    #[cfg(test)]
    _shared.fire(crate::test_hooks::at::FILE_STAGED);
    #[cfg(test)]
    _shared.fire("session.before.structural_verify");
    #[cfg(test)]
    _shared.fire("session.entered.structural_verify");
    if !run.verify_structural().map_err(run_error)?.passed() {
        return Err(refused("the file is not plain new text"));
    }
    #[cfg(test)]
    _shared.fire("session.after.structural_verify");
    let page = match class {
        FileClass::Page => {
            #[cfg(test)]
            _shared.fire("session.before.page_verify");
            #[cfg(test)]
            _shared.fire("session.entered.page_verify");
            let result = run
                .check_inert_page(&path.as_string(), facts)
                .map_err(inert_refusal)?;
            #[cfg(test)]
            _shared.fire("session.after.page_verify");
            Some(result)
        }
        FileClass::Plain => None,
        FileClass::Refused => return Err(AuthorityError::FileTypeNotAllowed),
    };
    // The kernel checked exactly the bytes the Owner is shown.
    let digest: [u8; 32] = {
        use sha2::{Digest as _, Sha256};
        Sha256::digest(content).into()
    };
    if page
        .as_ref()
        .is_some_and(|page| page.page.candidate_sha256 != digest)
    {
        return Err(AuthorityError::TargetChanged);
    }
    #[cfg(test)]
    _shared.fire("session.before.review");
    #[cfg(test)]
    _shared.fire("session.entered.review");
    let review = run.review().map_err(run_error)?;
    #[cfg(test)]
    _shared.fire("session.after.review");
    let bound = match (&page, review.binding.verification) {
        (Some(page), VerifierMarker::InertPage(hash)) => page.binding_hash() == hash,
        (Some(_), VerifierMarker::NoResult | VerifierMarker::Result(_)) => false,
        (None, VerifierMarker::NoResult) => true,
        (None, VerifierMarker::Result(_) | VerifierMarker::InertPage(_)) => false,
    };
    if !bound {
        return Err(AuthorityError::Unavailable(
            "the review does not name the file's check",
        ));
    }
    let only = match review.single_complete() {
        Some(only) if &only.path == path && only.kind == ChangeKind::Create => only,
        _ => return Err(AuthorityError::ChangeTooLargeToShow),
    };
    // The verified, reviewed candidate is the input shown, for either
    // class: a same-UID edit of the staging before its verification is
    // refused here, before any commitment.
    match &only.new {
        Some(entry) if entry.sha256 == digest && entry.size == content.len() as u64 => {
            Ok((review.binding.hash(), page, entry.clone()))
        }
        Some(_) | None => Err(AuthorityError::TargetChanged),
    }
}

/// The registered workspace.
struct Workspace {
    info: WorkspaceInfo,
    root: PathBuf,
    identity: Digest,
    /// M2d-B2: the same directory, registered with the kernel.
    #[cfg(target_os = "linux")]
    kernel: Kernel,
}

/// The Files domain's state: at most one workspace per process.
pub(crate) struct Files {
    runtime_root: PathBuf,
    /// Serializes registrations (nothing else takes it).
    registering: Mutex<()>,
    workspace: Mutex<Option<Arc<Workspace>>>,
    /// M2d-B2: what executions, the undo and the report share.
    shared: Arc<Shared>,
}

fn refused(why: &'static str) -> AuthorityError {
    AuthorityError::InvalidAction(why)
}

/// `path`'s canonical spelling (`std::fs::canonicalize`). Tests can make a
/// directory reachable under a second canonical spelling, as a bind mount
/// or a case-insensitive name makes it ([`mount_alias_for_test`]).
fn canonical(path: &Path) -> std::io::Result<PathBuf> {
    #[cfg(test)]
    if let Some(alias) = MOUNTED.with(|mounted| {
        mounted
            .borrow()
            .iter()
            .find(|alias| path.starts_with(alias))
            .cloned()
    }) {
        let target = std::fs::canonicalize(&alias)?;
        let below = path.strip_prefix(&alias).unwrap_or(Path::new(""));
        let real = std::fs::canonicalize(target.join(below))?;
        return Ok(match real.strip_prefix(&target) {
            Ok(rest) if rest.as_os_str().is_empty() => alias,
            Ok(rest) => alias.join(rest),
            Err(_) => real,
        });
    }
    std::fs::canonicalize(path)
}

#[cfg(test)]
thread_local! {
    /// Tests only: symlinks this thread treats as a second canonical
    /// spelling of the directory they name (an unprivileged stand-in for a
    /// bind mount or a casefolded name): the link's own spelling is kept by
    /// [`canonical`], while opening it reaches the same directory.
    static MOUNTED: std::cell::RefCell<Vec<PathBuf>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// Tests only: treat the symlink at `link` (an absolute, canonical-parent
/// path) as a second canonical spelling of its directory, on this thread.
#[cfg(test)]
pub(crate) fn mount_alias_for_test(link: PathBuf) {
    MOUNTED.with(|mounted| mounted.borrow_mut().push(link));
}

/// One mount, as `/proc/self/mountinfo` describes it (proc(5)): its id and
/// its parent's, its device (`major:minor`), its root within that filesystem
/// (field 4) and its mount point (field 5), both unescaped, and its
/// filesystem type.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Mount {
    pub(crate) id: u64,
    /// Parsed and kept (Codex review 4): the allowlist does not need it.
    #[allow(dead_code)]
    pub(crate) parent: u64,
    pub(crate) major: u64,
    pub(crate) minor: u64,
    pub(crate) root: std::ffi::OsString,
    pub(crate) mount_point: PathBuf,
    pub(crate) fstype: std::ffi::OsString,
}

/// The filesystem types a workspace, or a protected or reserved location,
/// may be on: plain local filesystems with no backing directories elsewhere
/// (Codex review 4 of PR #57; overlay, btrfs, fuse, network and image
/// filesystems are refused).
#[cfg(target_os = "linux")]
const SUPPORTED_FILESYSTEMS: &[&str] = &["ext4", "xfs", "tmpfs"];

/// `st_dev` as (`major`, `minor`), in the encoding Linux and glibc use.
#[cfg(target_os = "linux")]
fn split_dev(dev: u64) -> (u64, u64) {
    let major = ((dev >> 8) & 0xfff) | ((dev >> 32) & !0xfff);
    let minor = (dev & 0xff) | ((dev >> 12) & !0xff);
    (major, minor)
}

/// A mountinfo field as the kernel escapes it (space, tab, newline and
/// backslash as `\ooo`, three octal digits), unescaped; `None` if an
/// escape is malformed.
#[cfg(target_os = "linux")]
fn unescape(field: &[u8]) -> Option<std::ffi::OsString> {
    use std::os::unix::ffi::OsStringExt;
    let mut out = Vec::with_capacity(field.len());
    let mut at = 0;
    while at < field.len() {
        if field[at] == b'\\' {
            let digits = field.get(at + 1..at + 4)?;
            if !digits.iter().all(|d| (b'0'..=b'7').contains(d)) {
                return None;
            }
            let value = digits
                .iter()
                .fold(0u32, |value, d| value * 8 + u32::from(d - b'0'));
            out.push(u8::try_from(value).ok()?);
            at += 4;
        } else {
            out.push(field[at]);
            at += 1;
        }
    }
    Some(std::ffi::OsString::from_vec(out))
}

/// Every mount `/proc/self/mountinfo`'s `text` lists (proc(5): mount id,
/// parent id, `major:minor`, root, mount point, options, optional fields up
/// to `-`, then exactly the filesystem type, the source and the superblock
/// options). Every field must be present and non-empty (Codex review 5: a
/// doubled or trailing space leaves an empty one, such as empty superblock
/// options) and well formed. `None` if any line is malformed or none is
/// listed: callers refuse the whole table (fail closed).
#[cfg(target_os = "linux")]
pub(crate) fn parse_mountinfo(text: &[u8]) -> Option<Vec<Mount>> {
    let number = |field: &[u8]| std::str::from_utf8(field).ok()?.parse::<u64>().ok();
    let mut mounts = Vec::new();
    for line in text.split(|b| *b == b'\n').filter(|line| !line.is_empty()) {
        let fields: Vec<&[u8]> = line.split(|b| *b == b' ').collect();
        if fields.iter().any(|field| field.is_empty()) {
            return None;
        }
        let separator = 6 + fields.get(6..)?.iter().position(|field| *field == b"-")?;
        if fields.len() != separator + 4 {
            return None;
        }
        let (major, minor) = std::str::from_utf8(fields[2]).ok()?.split_once(':')?;
        let mount_point = PathBuf::from(unescape(fields[4])?);
        if !mount_point.is_absolute() {
            return None;
        }
        mounts.push(Mount {
            id: number(fields[0])?,
            parent: number(fields[1])?,
            major: major.parse().ok()?,
            minor: minor.parse().ok()?,
            root: unescape(fields[3])?,
            mount_point,
            fstype: unescape(fields[separator + 1])?,
        });
    }
    (!mounts.is_empty()).then_some(mounts)
}

/// The mount table this process sees.
#[cfg(target_os = "linux")]
fn mountinfo() -> Option<Vec<u8>> {
    #[cfg(test)]
    if let Some(given) = MOUNTINFO.with(|mountinfo| mountinfo.borrow().clone()) {
        return given.ok().map(String::into_bytes);
    }
    std::fs::read("/proc/self/mountinfo").ok()
}

/// The one mount of the directory at `path` (canonical), if its topology is
/// supported (Codex review 4 of PR #57: a strict allowlist, decided by the
/// whole table, never by choosing among candidates):
/// 1. the path's device (`major:minor` of its `st_dev`) is listed exactly
///    once (refusing every bind, whole, subtree, hidden or stacked, and
///    btrfs layouts);
/// 2. that entry's root is exactly `/` and its type is in
///    `SUPPORTED_FILESYSTEMS`;
/// 3. its mount point is the path or a component-wise ancestor of it, and no
///    other entry's mount point lies strictly between them or is the path.
///
/// Anything else, a path that cannot be stated, or a table that cannot be
/// read or parsed in full, is `unsupported_mount_topology`.
#[cfg(target_os = "linux")]
pub(crate) fn mount_of(path: &Path) -> Result<Mount, AuthorityError> {
    use std::os::unix::fs::MetadataExt;
    let unsupported = || AuthorityError::UnsupportedMountTopology;
    let (major, minor) = split_dev(std::fs::metadata(path).map_err(|_| unsupported())?.dev());
    let mounts = mountinfo()
        .and_then(|text| parse_mountinfo(&text))
        .ok_or_else(unsupported)?;
    let mut listed = mounts
        .iter()
        .filter(|mount| mount.major == major && mount.minor == minor);
    let (Some(mount), None) = (listed.next(), listed.next()) else {
        return Err(unsupported());
    };
    let supported_type = SUPPORTED_FILESYSTEMS
        .iter()
        .any(|fstype| mount.fstype == std::ffi::OsStr::new(fstype));
    if mount.root != std::ffi::OsStr::new("/")
        || !supported_type
        || !path.starts_with(&mount.mount_point)
    {
        return Err(unsupported());
    }
    let between = mounts
        .iter()
        .filter(|other| other.id != mount.id)
        .any(|other| {
            other.mount_point.as_path() == path
                || (other.mount_point != mount.mount_point
                    && other.mount_point.starts_with(&mount.mount_point)
                    && path.starts_with(&other.mount_point))
        });
    if between {
        return Err(unsupported());
    }
    // The Owner's Option 1: an ext4 or xfs mount's device on real hardware
    // (tmpfs has no block device).
    if mount.fstype != std::ffi::OsStr::new("tmpfs") && !hardware_backed(mount.major, mount.minor) {
        return Err(unsupported());
    }
    Ok(mount.clone())
}

#[cfg(not(target_os = "linux"))]
pub(crate) fn mount_of(_path: &Path) -> Result<Mount, AuthorityError> {
    Err(AuthorityError::UnsupportedMountTopology)
}

/// The directory at `path` is on a supported topology ([`mount_of`]), or
/// refused as unsupported.
fn whole_filesystem(path: &Path) -> Result<(), AuthorityError> {
    mount_of(path).map(|_| ())
}

#[cfg(all(test, target_os = "linux"))]
thread_local! {
    /// Tests only: the text this thread reads in place of
    /// `/proc/self/mountinfo` (`Err`: unreadable), when set.
    static MOUNTINFO: std::cell::RefCell<Option<Result<String, ()>>> =
        const { std::cell::RefCell::new(None) };
}

/// Tests only: read `text` in place of `/proc/self/mountinfo` on this
/// thread (`Some(Err(()))`: it cannot be read; `None`: the real one).
#[cfg(all(test, target_os = "linux"))]
pub(crate) fn mountinfo_for_test(text: Option<Result<String, ()>>) {
    MOUNTINFO.with(|mountinfo| *mountinfo.borrow_mut() = text);
}

/// The deepest level, and the most devices, a backing walk visits.
#[cfg(target_os = "linux")]
const MAX_BACKING_DEPTH: usize = 16;
#[cfg(target_os = "linux")]
const MAX_BACKING_NODES: usize = 256;
/// Block devices that are not real hardware, by name: loop, network block,
/// compressed RAM, userspace block and RAM disks.
#[cfg(target_os = "linux")]
const VIRTUAL_BLOCK_DEVICES: &[&str] = &["loop", "nbd", "zram", "ublkb", "ublkc", "ram"];
/// The loop driver's major number.
#[cfg(target_os = "linux")]
const LOOP_MAJOR: u64 = 7;

/// The sysfs this process reads.
#[cfg(target_os = "linux")]
fn sysfs_root() -> PathBuf {
    #[cfg(test)]
    if let Some(root) = SYSFS.with(|sysfs| sysfs.borrow().clone()) {
        return root;
    }
    PathBuf::from("/sys")
}

/// Whether block device `major:minor` is backed only by real hardware (the
/// Owner's decision, Option 1): `/sys/dev/block/<major:minor>` resolves,
/// and neither it nor anything it rests on (a partition's disk, a disk's
/// `slaves/`, followed only through those sysfs links, at most
/// `MAX_BACKING_DEPTH` levels and `MAX_BACKING_NODES` devices) is a loop
/// device (by name or major 7), nbd, zram, ublk or ram, and every node's
/// `dev` and `slaves/` can be read and resolved. Anything else is false
/// (fail closed). Whole disks, their partitions, dm-crypt and LVM (`dm-*`
/// over real disks) and md RAID are real hardware.
#[cfg(target_os = "linux")]
pub(crate) fn hardware_backed(major: u64, minor: u64) -> bool {
    let start = sysfs_root()
        .join("dev/block")
        .join(format!("{major}:{minor}"));
    let Ok(start) = std::fs::canonicalize(start) else {
        return false;
    };
    let mut pending = vec![(start, 0usize)];
    let mut visited = 0usize;
    while let Some((node, depth)) = pending.pop() {
        visited += 1;
        if depth > MAX_BACKING_DEPTH || visited > MAX_BACKING_NODES {
            return false;
        }
        let Some(name) = node.file_name().and_then(|name| name.to_str()) else {
            return false;
        };
        if VIRTUAL_BLOCK_DEVICES
            .iter()
            .any(|prefix| name.starts_with(prefix))
        {
            return false;
        }
        let Ok(dev) = std::fs::read_to_string(node.join("dev")) else {
            return false;
        };
        let Some((node_major, node_minor)) = dev.trim().split_once(':') else {
            return false;
        };
        let (Ok(node_major), Ok(_)) = (node_major.parse::<u64>(), node_minor.parse::<u64>()) else {
            return false;
        };
        if node_major == LOOP_MAJOR {
            return false;
        }
        if node.join("partition").is_file() {
            let Some(disk) = node.parent() else {
                return false;
            };
            pending.push((disk.to_path_buf(), depth + 1));
            continue;
        }
        let Ok(slaves) = std::fs::read_dir(node.join("slaves")) else {
            return false;
        };
        for slave in slaves {
            let Ok(target) = slave.and_then(|slave| std::fs::canonicalize(slave.path())) else {
                return false;
            };
            pending.push((target, depth + 1));
        }
    }
    true
}

#[cfg(all(test, target_os = "linux"))]
thread_local! {
    /// Tests only: the directory this thread reads in place of `/sys`.
    static SYSFS: std::cell::RefCell<Option<PathBuf>> = const { std::cell::RefCell::new(None) };
}

/// Tests only: read `root` in place of `/sys` on this thread (`None`: the
/// real one).
#[cfg(all(test, target_os = "linux"))]
pub(crate) fn sysfs_for_test(root: Option<PathBuf>) {
    SYSFS.with(|sysfs| *sysfs.borrow_mut() = root);
}

/// Where a protected or reserved location is: its longest existing
/// ancestor, canonical (`existing`), the location itself (`real`: `existing`
/// with the missing components appended), and whether it exists completely
/// (`existing == real`).
struct Located {
    existing: PathBuf,
    real: PathBuf,
    complete: bool,
}

/// Where `path` really is (Codex review 1 of PR #57): its longest existing
/// ancestor resolved canonically (links included), with the components
/// below it appended, none of which exists yet. `None` when that cannot be
/// established: a relative path; the first missing component present after
/// all (a dangling or looping link, an entry that cannot be read); or a `.`
/// or `..` below the existing part. A caller refuses on `None`.
fn established(path: &Path) -> Option<Located> {
    use std::path::Component;
    if !path.is_absolute() {
        return None;
    }
    let mut missing = Vec::new();
    let mut existing = path;
    loop {
        if let Ok(canonical) = canonical(existing) {
            if let Some(first) = missing.last() {
                match std::fs::symlink_metadata(canonical.join(first)) {
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                    _ => return None,
                }
            }
            let mut real = canonical.clone();
            for part in missing.iter().rev() {
                real.push(part);
            }
            return Some(Located {
                existing: canonical,
                real,
                complete: missing.is_empty(),
            });
        }
        match existing.components().next_back()? {
            Component::Normal(part) => missing.push(part),
            _ => return None,
        }
        existing = existing.parent()?;
    }
}

/// Most directories an ancestry walk visits.
#[cfg(target_os = "linux")]
const MAX_ANCESTRY: usize = 4096;

/// The identities (`st_dev`, `st_ino`) of the directory `dir` opens and of
/// every directory above it, walking `..` from the open directory itself
/// (`/proc/self/fd/<fd>/..`, as `openat(fd, "..")` does) up to the root,
/// whose `..` is itself. `None` if an open or a stat fails, an entry is not
/// a directory, or the walk does not end: callers refuse (fail closed).
#[cfg(target_os = "linux")]
fn ancestry(dir: &Path) -> Option<Vec<(u64, u64)>> {
    use std::os::fd::AsRawFd;
    use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
    let open = |path: &Path| {
        std::fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_PATH | libc::O_DIRECTORY)
            .open(path)
            .ok()
    };
    let mut current = open(dir)?;
    let mut chain: Vec<(u64, u64)> = Vec::new();
    for _ in 0..MAX_ANCESTRY {
        let meta = current.metadata().ok()?;
        if !meta.is_dir() {
            return None;
        }
        let id = (meta.dev(), meta.ino());
        if chain.last() == Some(&id) {
            return Some(chain);
        }
        if chain.contains(&id) {
            return None;
        }
        chain.push(id);
        let parent = format!("/proc/self/fd/{}/..", current.as_raw_fd());
        current = open(Path::new(&parent))?;
    }
    None
}

/// The identity of the directory `root` names now: it must still be a real
/// directory, named by its own canonical path.
#[cfg(target_os = "linux")]
fn identity_of(root: &Path) -> Result<Digest, AuthorityError> {
    use std::os::unix::fs::MetadataExt;
    let meta = std::fs::symlink_metadata(root).map_err(|_| AuthorityError::TargetChanged)?;
    if !meta.is_dir() || canonical(root).ok().as_deref() != Some(root) {
        return Err(AuthorityError::TargetChanged);
    }
    Ok(Digest::of(
        IDENTITY_DOMAIN,
        &[&meta.dev().to_be_bytes(), &meta.ino().to_be_bytes()],
    ))
}

#[cfg(not(target_os = "linux"))]
fn identity_of(_root: &Path) -> Result<Digest, AuthorityError> {
    Err(AuthorityError::Unavailable(
        "Files are available on Linux only",
    ))
}

/// §6.2: the broker's checks of the Owner's selection, in order: absolute,
/// its own canonical path (no link), a directory; not `/`, `$HOME` or the
/// identity home, and no overlap in either direction with a reserved tree,
/// each decided by directory identity ([`ancestry`]: the workspace is a
/// protected location if their identities are equal; a reserved location
/// is inside or equal to the workspace if the workspace's identity is in
/// the ancestor-or-self chain of the location's longest existing ancestor;
/// the workspace is inside or equal to a reserved location that exists
/// completely if the location's identity is in the workspace's chain), and
/// by spelling too (as given, or where it really is, [`established`]); a
/// location that cannot be established or opened refuses; the current
/// user's; empty. Returns the identity of the directory it checked.
#[cfg(target_os = "linux")]
fn selectable(
    path: &Path,
    exclusions: &Exclusions,
    runtime_root: &Path,
) -> Result<Digest, AuthorityError> {
    use std::os::unix::fs::MetadataExt;
    if !path.is_absolute() {
        return Err(refused("the workspace is not an absolute path"));
    }
    let canonical =
        canonical(path).map_err(|_| refused("the workspace folder cannot be opened"))?;
    if canonical != path {
        return Err(refused(
            "the workspace is reached through a link; choose the real folder",
        ));
    }
    let meta = std::fs::symlink_metadata(path)
        .map_err(|_| refused("the workspace folder cannot be opened"))?;
    if !meta.is_dir() {
        return Err(refused("the workspace is not a folder"));
    }
    let unresolved = || refused("a protected or reserved location cannot be resolved");
    // The workspace and every directory above it, by identity, on a
    // whole-filesystem mount.
    let workspace = ancestry(path).ok_or_else(unresolved)?;
    whole_filesystem(path)?;
    let exact = [
        Path::new("/"),
        exclusions.home.as_path(),
        exclusions.identity_home.as_path(),
    ];
    for forbidden in exact {
        let located = established(forbidden).ok_or_else(unresolved)?;
        whole_filesystem(&located.existing)?;
        let same = located.complete
            && ancestry(&located.existing).ok_or_else(unresolved)?[0] == workspace[0];
        if same || forbidden == path || located.real == path {
            return Err(refused("this folder cannot be the workspace"));
        }
    }
    let reserved = exclusions
        .reserved
        .iter()
        .map(PathBuf::as_path)
        .chain(std::iter::once(runtime_root));
    for tree in reserved {
        let located = established(tree).ok_or_else(unresolved)?;
        whole_filesystem(&located.existing)?;
        let above = ancestry(&located.existing).ok_or_else(unresolved)?;
        let inside_workspace = above.contains(&workspace[0]);
        let contains_workspace = located.complete && workspace.contains(&above[0]);
        let by_spelling = [tree, located.real.as_path()]
            .iter()
            .any(|tree| path.starts_with(tree) || tree.starts_with(path));
        if inside_workspace || contains_workspace || by_spelling {
            return Err(refused("the workspace overlaps a reserved location"));
        }
    }
    if meta.uid() != exclusions.uid {
        return Err(refused("the workspace folder is not the current user's"));
    }
    let mut entries =
        std::fs::read_dir(path).map_err(|_| refused("the workspace folder cannot be opened"))?;
    if entries.next().is_some() {
        return Err(refused("the workspace folder is not empty"));
    }
    Ok(Digest::of(
        IDENTITY_DOMAIN,
        &[&meta.dev().to_be_bytes(), &meta.ino().to_be_bytes()],
    ))
}

#[cfg(not(target_os = "linux"))]
fn selectable(_: &Path, _: &Exclusions, _: &Path) -> Result<Digest, AuthorityError> {
    Err(AuthorityError::Unavailable(
        "Files are available on Linux only",
    ))
}

impl Files {
    pub(crate) fn new(runtime_root: PathBuf) -> Self {
        Self {
            runtime_root,
            registering: Mutex::new(()),
            workspace: Mutex::new(None),
            shared: Arc::default(),
        }
    }

    /// The Files ledger and staging parent, opened at first use (with no
    /// lock held while they are opened).
    #[cfg(target_os = "linux")]
    fn backend(&self) -> Result<Arc<Backend>, AuthorityError> {
        if let Some(backend) = self.shared.backend.lock().expect("files backend").clone() {
            return Ok(backend);
        }
        let opened = Arc::new(open_backend(&self.runtime_root)?);
        let mut slot = self.shared.backend.lock().expect("files backend");
        Ok(Arc::clone(slot.get_or_insert(opened)))
    }

    /// Discard the staging of every run handed over by a dropped effect
    /// (Codex D2 condition 5: never on the path that dropped it). Called
    /// at the start of each Files call and when the front door settles.
    pub(crate) fn drain(&self) {
        #[cfg(target_os = "linux")]
        {
            let runs = std::mem::take(&mut *self.shared.deferred.lock().expect("files deferred"));
            for run in runs {
                #[cfg(test)]
                self.shared.fire(crate::test_hooks::at::FILE_CLEANUP);
                let _kept = self.shared.retire(run);
            }
        }
    }

    /// Preserve undo's inherited deferred cleanup, but carry its fresh
    /// recovery lifetime to every kernel handoff under the caller's Busy
    /// ownership. A refused tail stays deferred; nothing drops it into cleanup.
    #[cfg(target_os = "linux")]
    fn drain_recovery(&self, recovery: &RecoveryCheck) -> Result<(), AuthorityError> {
        recovery.check()?;
        let runs = std::mem::take(&mut *self.shared.deferred.lock().expect("files deferred"));
        let mut remaining = runs.into_iter();
        while let Some(run) = remaining.next() {
            #[cfg(test)]
            self.shared.fire(crate::test_hooks::at::FILE_CLEANUP);
            if let Err(error) = recovery.check() {
                let mut deferred = self.shared.deferred.lock().expect("files deferred");
                deferred.push(run);
                deferred.extend(remaining);
                return Err(error);
            }
            let _kept = self.shared.retire(run);
        }
        Ok(())
    }

    /// §6.6 and H2′: undo the last create of this process, once, with the
    /// Owner's dialog and fresh authority (see
    /// `GovernedControl::files_undo`). Its lifetime (the broker's kernel
    /// stop watch and the policy generation) is taken first, before any
    /// cleanup this call does (Codex review 1 of PR #64, H4), and checked
    /// after that cleanup, around the Owner's dialog and right before the
    /// kernel's U1: a Stop, even resumed, or a policy change in between
    /// refuses this undo, and a new request is needed. The slot is taken
    /// for the attempt and put back unless the undo completed; nothing is
    /// held across the dialog or the filesystem.
    #[cfg(target_os = "linux")]
    pub(crate) fn undo(
        &self,
        authority: &Authority,
        stop: &Arc<StopAuthority>,
        confirmer: &dyn ControlConfirmer,
        cell: Option<&HaltCell>,
    ) -> Result<UndoReport, AuthorityError> {
        let lifetime = UndoLifetime::take(authority, stop, cell)?;
        // Preserve the legacy drain order. The host's inherited drain is
        // guarded below under Busy, with this request's original lifetime.
        if cell.is_none() {
            self.drain();
        }
        let _busy = Busy::take(&self.shared.busy).ok_or(AuthorityError::Unavailable(
            "another Files operation is under way",
        ))?;
        if cell.is_some() {
            self.drain_recovery(&lifetime.recovery)?;
        }
        let taken = self.shared.slot.lock().expect("files slot").take();
        let mut applied = taken.ok_or(AuthorityError::Unavailable(
            "there is no file to undo in this session",
        ))?;
        let result = lifetime
            .check(authority)
            .and_then(|()| self.undo_one(&mut applied, confirmer, &lifetime, authority));
        if let Err(error) = result {
            let mut slot = self.shared.slot.lock().expect("files slot");
            if slot.is_none() {
                *slot = Some(applied);
            } else {
                drop(slot);
                let _kept = self.shared.retire(applied.run);
            }
            return Err(error);
        }
        // M4′ for an undo: its write grant's lapse after U1, read only now
        // that the restore completed; it ends Files for the creating run.
        let interrupted = write_grant_lapse(&applied.run)
            .or_else(|| lifetime.recovery.check().err().map(|_| "emergency_stop"));
        if interrupted.is_some() {
            self.shared
                .closed
                .lock()
                .expect("files closed")
                .insert(applied.broker_run);
        }
        let _kept = self.shared.retire(applied.run);
        Ok(UndoReport { interrupted })
    }

    /// The kernel's restore of `applied`: the request (its watches taken
    /// now, before the dialog), the Owner's dialog through [`UndoBridge`],
    /// a fresh write grant, then the restore and its final check U1.
    #[cfg(target_os = "linux")]
    fn undo_one(
        &self,
        applied: &mut Applied,
        confirmer: &dyn ControlConfirmer,
        lifetime: &UndoLifetime,
        authority: &Authority,
    ) -> Result<(), AuthorityError> {
        let kernel = &applied.workspace.kernel;
        let live = || lifetime.check(authority).is_ok();
        let bridge = UndoBridge {
            confirmer,
            workspace: &applied.workspace.info.name,
            name: &applied.name,
            bytes: applied.bytes,
            sha256: &applied.sha256,
            review_hash: applied.review_hash,
            live: &live,
        };
        #[cfg(test)]
        self.shared.fire("session.before.request_restore");
        #[cfg(test)]
        self.shared.fire("session.entered.request_restore");
        let requested = applied.run.request_restore(&kernel.project.name, &bridge);
        #[cfg(test)]
        self.shared.fire("session.after.request_restore");
        // A dialog refused because the lifetime ended is that, not a no.
        lifetime.check(authority)?;
        let approval = requested.map_err(undo_error)?;
        #[cfg(test)]
        self.shared.fire("session.before.undo_grant");
        #[cfg(test)]
        self.shared.fire("session.entered.undo_grant");
        let write = kernel
            .projects
            .grant_for_apply(kernel.project.id, applied.binding)
            .map_err(project_error)?;
        #[cfg(test)]
        self.shared.fire("session.after.undo_grant");
        #[cfg(test)]
        self.shared.note_write_grant(&kernel.authority, &write);
        // Armed from here until settled, across the restore and any unwind.
        let issued = IssuedGrant::new(
            &kernel.authority,
            &write,
            applied.run.id().to_string(),
            &self.shared,
        );
        #[cfg(test)]
        self.shared.fire(crate::test_hooks::at::FILE_UNDO_APPROVED);
        if let Err(error) = lifetime.check(authority) {
            // HIGH 2: the unused grant is closed now; a closure that is not
            // established is recovery required, and reported.
            return Err(if issued.close() {
                error
            } else {
                AuthorityError::RecoveryRequired
            });
        }
        #[cfg(test)]
        self.shared.fire(crate::test_hooks::at::FILE_UNDO_RESTORE);
        // Busy still owns the whole attempt. This is the last broker check;
        // kernel preflight/U1 cannot observe the cell (Decision 11, M2d-K3).
        if let Err(error) = lifetime.check(authority) {
            return Err(if issued.close() {
                error
            } else {
                AuthorityError::RecoveryRequired
            });
        }
        #[cfg(test)]
        self.shared.fire("files.undo_kernel_boundary");
        #[cfg(test)]
        self.shared.fire("session.before.restore");
        #[cfg(test)]
        self.shared.fire("session.entered.restore");
        let restored = applied.run.restore(approval, write);
        #[cfg(test)]
        self.shared.fire("session.after.restore");
        // Re-check 3: closed again (idempotent) and confirmed, whatever the
        // restore answered; not established is recovery required, reported.
        if !issued.close() {
            return Err(AuthorityError::RecoveryRequired);
        }
        restored.map(|_| ()).map_err(undo_error)
    }

    #[cfg(not(target_os = "linux"))]
    pub(crate) fn undo(
        &self,
        _authority: &Authority,
        _stop: &Arc<StopAuthority>,
        _confirmer: &dyn ControlConfirmer,
        _cell: Option<&HaltCell>,
    ) -> Result<UndoReport, AuthorityError> {
        Err(AuthorityError::Unavailable(
            "Files are available on Linux only",
        ))
    }

    /// §6.7: every Files run's findings, from the ledger, and in this
    /// process the runs whose staging Files still owns because its removal
    /// failed. It performs no cleanup (Codex review 1 of PR #64, M2):
    /// nothing on disk is removed, swept or called disposable; a ledger
    /// that cannot be read or verified is an error.
    #[cfg(target_os = "linux")]
    pub(crate) fn recovery_report(&self) -> Result<Vec<RecoveryEntry>, AuthorityError> {
        let backend = self.backend()?;
        let store = backend.ledger.store();
        let mut entries = Vec::new();
        for run in backend.ledger.run_ids().map_err(ledger_error)? {
            let findings = analyze_apply(store.as_ref(), run.ledger_key()).map_err(ledger_error)?;
            if !findings.is_empty() {
                entries.push(RecoveryEntry {
                    run: run.to_string(),
                    findings: findings.into_iter().map(finding).collect(),
                });
            }
        }
        let unclosed = self.shared.unclosed.lock().expect("files unclosed").clone();
        for run in unclosed {
            match entries.iter_mut().find(|entry| entry.run == run) {
                Some(entry) => entry.findings.push(RecoveryFinding::WriteGrantNotClosed),
                None => entries.push(RecoveryEntry {
                    run,
                    findings: vec![RecoveryFinding::WriteGrantNotClosed],
                }),
            }
        }
        let undiscarded: Vec<String> = self
            .shared
            .kept
            .lock()
            .expect("files kept")
            .iter()
            .filter(|run| run.cleanup_status() != CleanupStatus::Discarded)
            .map(|run| run.id().to_string())
            .collect();
        for run in undiscarded {
            match entries.iter_mut().find(|entry| entry.run == run) {
                Some(entry) => entry.findings.push(RecoveryFinding::StagingNotDiscarded),
                None => entries.push(RecoveryEntry {
                    run,
                    findings: vec![RecoveryFinding::StagingNotDiscarded],
                }),
            }
        }
        Ok(entries)
    }

    /// §6.6 (Codex review 1 of PR #64, M1): the Owner's bounded retry of
    /// owed Files cleanup: one attempt per owed run (the kernel's
    /// `retry_cleanup` for owed temps, then the staging's removal). A run
    /// whose cleanup is then established is released; any other stays
    /// owned and reported. One Files operation at a time.
    #[cfg(target_os = "linux")]
    pub(crate) fn retry_cleanup(
        &self,
        cell: Option<&HaltCell>,
    ) -> Result<CleanupRetry, AuthorityError> {
        let recovery = RecoveryCheck::take(cell)?;
        let _busy = Busy::take(&self.shared.busy).ok_or(AuthorityError::Unavailable(
            "another Files operation is under way",
        ))?;
        #[cfg(test)]
        self.shared.fire("files.retry_before_effect");
        recovery.check()?;
        let runs = std::mem::take(&mut *self.shared.kept.lock().expect("files kept"));
        let mut retry = CleanupRetry::default();
        let mut still = Vec::new();
        let mut remaining = runs.into_iter();
        while let Some(mut run) = remaining.next() {
            let result = (|| {
                // Each kernel call has its own last check under Busy. On a
                // refusal, return this run AND every unattempted obligation.
                recovery.check()?;
                if !run.cleanup_owed().is_empty() {
                    let _ = run.retry_cleanup();
                }
                if !owes(&run) {
                    recovery.check()?;
                    Ok(discard(&mut run, &self.shared))
                } else {
                    Ok(false)
                }
            })();
            let discarded = match result {
                Ok(discarded) => discarded,
                Err(error) => {
                    still.push(run);
                    still.extend(remaining);
                    self.shared.kept.lock().expect("files kept").extend(still);
                    return Err(error);
                }
            };
            if discarded {
                retry.settled += 1;
            } else {
                retry.still_owed += 1;
                still.push(run);
            }
        }
        self.shared.kept.lock().expect("files kept").extend(still);
        Ok(retry)
    }

    #[cfg(not(target_os = "linux"))]
    pub(crate) fn retry_cleanup(
        &self,
        _cell: Option<&HaltCell>,
    ) -> Result<CleanupRetry, AuthorityError> {
        Err(AuthorityError::Unavailable(
            "Files are available on Linux only",
        ))
    }

    #[cfg(not(target_os = "linux"))]
    pub(crate) fn recovery_report(&self) -> Result<Vec<RecoveryEntry>, AuthorityError> {
        Err(AuthorityError::Unavailable(
            "Files are available on Linux only",
        ))
    }

    /// The Owner-only detail of execution `id`'s failure, taken once.
    pub(crate) fn working_detail(&self, id: CommitmentId) -> Option<WorkingDetail> {
        let mut details = self.shared.details.lock().expect("files details");
        let at = details.iter().position(|(of, _)| *of == id)?;
        details.remove(at).map(|(_, lines)| WorkingDetail { lines })
    }

    /// R3: the Owner's startup hold, at most [`MAX_HOLD`], set before the
    /// workspace is registered.
    pub(crate) fn set_hold(&self, hold: Duration) -> Result<(), AuthorityError> {
        self.set_hold_with_halt(hold, None)
    }

    pub(crate) fn set_hold_with_halt(
        &self,
        hold: Duration,
        cell: Option<&HaltCell>,
    ) -> Result<(), AuthorityError> {
        if hold > MAX_HOLD {
            return Err(refused("the hold before publication is at most 30 seconds"));
        }
        if self.current().is_some() {
            return Err(refused(
                "the hold before publication is set at startup, before the workspace",
            ));
        }
        let ms = u64::try_from(hold.as_millis()).unwrap_or(u64::MAX);
        crate::authority::file_session::admit_host(cell)?;
        self.shared.hold_ms.store(ms, Ordering::SeqCst);
        Ok(())
    }

    /// The broker's test hooks for Files (tests).
    #[cfg(test)]
    pub(crate) fn hooks(&self) -> &Arc<crate::test_hooks::Hooks> {
        &self.shared.hooks
    }

    /// How many runs dropped effects handed over, not yet cleaned (tests).
    #[cfg(all(test, target_os = "linux"))]
    pub(crate) fn deferred_for_test(&self) -> usize {
        self.shared.deferred.lock().unwrap().len()
    }

    /// How many runs Files keeps owning for owed cleanup or recovery
    /// (tests).
    #[cfg(all(test, target_os = "linux"))]
    pub(crate) fn kept_for_test(&self) -> usize {
        self.shared.kept.lock().unwrap().len()
    }

    /// Registered session poison fixture: panic while the named real owning
    /// hold is held. No production path or synthetic mutex is substituted.
    #[cfg(all(test, target_os = "linux"))]
    pub(crate) fn session_poison_for_test(&self, which: &str) {
        match which {
            "slot" => {
                let _hold = self.shared.slot.lock().unwrap();
                panic!("session slot poison");
            }
            "kept" => {
                let _hold = self.shared.kept.lock().unwrap();
                panic!("session kept poison");
            }
            "deferred" => {
                let _hold = self.shared.deferred.lock().unwrap();
                panic!("session deferred poison");
            }
            "unclosed" => {
                let _hold = self.shared.unclosed.lock().unwrap();
                panic!("session unclosed poison");
            }
            "receipts" => {
                let _hold = self.shared.receipts.lock().unwrap();
                panic!("session receipt poison");
            }
            _ => panic!("unregistered session poison fixture"),
        }
    }

    /// Where the Files ledger and staging parent are (tests).
    #[cfg(test)]
    pub(crate) fn state_dir_for_test(&self) -> PathBuf {
        self.runtime_root.join(STATE_DIR)
    }

    /// Whether the last kernel write grant Files issued still resolves
    /// (tests).
    #[cfg(all(test, target_os = "linux"))]
    pub(crate) fn write_grant_live_for_test(&self) -> bool {
        let noted = self.shared.write_grant.lock().unwrap().clone();
        noted.is_some_and(|(authority, grant, binding)| authority.resolve(grant, binding).is_ok())
    }

    /// Make the closure of an undo's issued write grant fail (tests).
    #[cfg(all(test, target_os = "linux"))]
    pub(crate) fn fail_grant_closure_for_test(&self, failing: bool) {
        self.shared.closure_fails.store(failing, Ordering::SeqCst);
    }

    /// Revoke the last kernel write grant Files issued (tests).
    #[cfg(all(test, target_os = "linux"))]
    pub(crate) fn revoke_write_grant_for_test(&self) -> bool {
        let noted = self.shared.write_grant.lock().unwrap().take();
        noted.is_some_and(|(authority, grant, binding)| authority.revoke(grant, binding).is_ok())
    }

    fn current(&self) -> Option<Arc<Workspace>> {
        self.workspace.lock().expect("files").clone()
    }

    /// S1a-D+B: the receipt of the Files create `id`, if it was published
    /// (one of the latest `MAX_RECEIPTS`).
    pub(crate) fn receipt(&self, id: CommitmentId) -> Option<FileReceipt> {
        self.shared
            .receipts
            .lock()
            .expect("files receipts")
            .iter()
            .find(|(of, _)| *of == id)
            .map(|(_, receipt)| *receipt)
    }

    /// S-O22's crate-private hook (tests): the next preparation checks its
    /// page with `facts` instead of its admission's.
    #[cfg(test)]
    pub(crate) fn prepare_facts_for_test(&self, facts: Option<ApprovedFacts>) {
        *self.shared.prepare_facts.lock().expect("files hook") = Some(facts);
    }

    /// The registered workspace, if any.
    pub(crate) fn info(&self) -> Option<WorkspaceInfo> {
        self.current().map(|workspace| workspace.info.clone())
    }

    /// §6.2: register the Owner's selection as this process's one
    /// workspace: refused when one is registered or by any check of
    /// [`selectable`]; the directory must keep the identity it was checked
    /// with. Its id is backend-random (`wsp-` and 32 hex); its name is the
    /// folder's own, escaped and at most 64 characters.
    #[cfg(test)]
    pub(crate) fn register(
        &self,
        path: &Path,
        exclusions: &Exclusions,
    ) -> Result<WorkspaceInfo, AuthorityError> {
        self.register_with_halt(path, exclusions, None)
    }

    pub(crate) fn register_with_halt(
        &self,
        path: &Path,
        exclusions: &Exclusions,
        halt: Option<&crate::authority::file_session::HaltCell>,
    ) -> Result<WorkspaceInfo, AuthorityError> {
        crate::authority::file_session::check_halt(halt)?;
        let _one = self.registering.lock().expect("files registration");
        if self.current().is_some() {
            return Err(refused("a workspace is already registered in this process"));
        }
        let identity = selectable(path, exclusions, &self.runtime_root)?;
        // M2d-B2: the same directory, registered with the kernel (its own
        // canonical and overlap checks, the broker's state reserved), and
        // still the directory checked.
        #[cfg(test)]
        self.hooks().fire("session.before.project_select");
        #[cfg(target_os = "linux")]
        let kernel = Kernel::register(path, &self.runtime_root, &exclusions.reserved, halt)?;
        #[cfg(test)]
        self.hooks().fire("session.after.project_select");
        if identity_of(path)? != identity {
            return Err(AuthorityError::TargetChanged);
        }
        let name = path
            .file_name()
            .map(|name| hint(&name.to_string_lossy()))
            .filter(|name| !name.is_empty())
            .unwrap_or_else(|| "workspace".to_string());
        let info = WorkspaceInfo {
            id: format!("wsp-{}", hex::encode(random16())),
            name,
        };
        #[cfg(test)]
        self.hooks().fire("files.before_register");
        let mut workspace = self.workspace.lock().expect("files");
        crate::authority::file_session::check_halt(halt)?;
        *workspace = Some(Arc::new(Workspace {
            info: info.clone(),
            root: path.to_path_buf(),
            identity,
            #[cfg(target_os = "linux")]
            kernel,
        }));
        Ok(info)
    }

    /// The registered workspace, for a door's admission: still the same
    /// directory (checked now, with no lock held).
    pub(crate) fn admissible(&self) -> Result<AdmittedWorkspace, AuthorityError> {
        let workspace = self
            .current()
            .ok_or(AuthorityError::Unavailable("no workspace is registered"))?;
        #[cfg(test)]
        self.hooks().fire("files.validation");
        if identity_of(&workspace.root)? != workspace.identity {
            return Err(AuthorityError::TargetChanged);
        }
        Ok(AdmittedWorkspace {
            id: workspace.info.id.clone(),
            identity: workspace.identity,
        })
    }

    /// The canonical scope of a Files grant on the registered workspace
    /// `id`, its identity checked now.
    pub(crate) fn grant_scope(&self, id: &str) -> Result<GrantScope, AuthorityError> {
        let workspace = self
            .current()
            .ok_or(AuthorityError::Unavailable("no workspace is registered"))?;
        if workspace.info.id != id {
            return Err(refused("no such workspace"));
        }
        if identity_of(&workspace.root)? != workspace.identity {
            return Err(AuthorityError::TargetChanged);
        }
        Ok(GrantScope::Files {
            workspace: workspace.info.id.clone(),
            root: workspace.root.to_string_lossy().into_owned(),
            identity: workspace.identity,
        })
    }

    /// §6.1 step 4, in order: the run's admission (never the intent's), the
    /// admitted workspace still the registered one and the same directory, a
    /// live Files grant on it, the name, the content, the name absent, and
    /// the complete display; then the kernel staging ([`Self::stage`]). No
    /// lock is held across filesystem work.
    pub(crate) fn prepare(
        &self,
        authority: &Authority,
        run: RunId,
        create: &FileCreate,
        stop: &Arc<StopAuthority>,
    ) -> Result<Preparation, AuthorityError> {
        authority.runs().admit_file_stage(run)?;
        // H3 (M4′): a run whose publication a cause interrupted after its
        // final check asks for nothing more.
        if self.shared.is_closed(run) {
            return Err(AuthorityError::RunNotActive);
        }
        let admission = authority
            .runs()
            .files_admission(run)
            .ok_or(AuthorityError::NotAdmitted)?;
        let workspace = self
            .current()
            .ok_or(AuthorityError::Unavailable("no workspace is registered"))?;
        if workspace.info.id != admission.workspace.id
            || workspace.identity != admission.workspace.identity
            || identity_of(&workspace.root)? != admission.workspace.identity
        {
            return Err(AuthorityError::TargetChanged);
        }
        let grant = covering_grant(authority, &admission.workspace)?;
        if !name_ok(&create.name) {
            return Err(refused("the file name is not allowed"));
        }
        content_ok(&create.content)?;
        match std::fs::symlink_metadata(workspace.root.join(&create.name)) {
            Ok(_) => return Err(AuthorityError::FileExists),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err(AuthorityError::Unavailable("the workspace cannot be read")),
        }
        let summary = display(&admission, &workspace.info, &create.name, &create.content)?;
        // S1a-D (O4): only a page (checked) or plain text, decided before
        // any staging.
        let class = FileClass::of(&create.name);
        if class == FileClass::Refused {
            return Err(AuthorityError::FileTypeNotAllowed);
        }
        #[cfg(test)]
        self.hooks().fire("files.before_stage");
        authority.runs().admit_file_stage(run)?;
        #[cfg(target_os = "linux")]
        return self.stage(
            run,
            create,
            (admission, class),
            workspace,
            grant,
            summary,
            stop,
        );
        #[cfg(not(target_os = "linux"))]
        {
            let _ = (run, grant, summary, stop);
            Err(AuthorityError::Unavailable(
                "Files are available on Linux only",
            ))
        }
    }

    /// §6.1 step 4's kernel half (M2d-K's entry points): a kernel run over a
    /// fresh read grant bound to (Mother, this broker run), observing the
    /// broker's kernel stop, reading the workspace and writing only the
    /// name; staged, verified and reviewed as exactly one new file shown
    /// whole. A refused staging is discarded before anything is committed.
    #[cfg(target_os = "linux")]
    #[allow(clippy::too_many_arguments)]
    fn stage(
        &self,
        run: RunId,
        create: &FileCreate,
        (admission, class): (FilesAdmission, FileClass),
        workspace: Arc<Workspace>,
        grant: GrantId,
        mut summary: Vec<String>,
        stop: &Arc<StopAuthority>,
    ) -> Result<Preparation, AuthorityError> {
        self.drain();
        let backend = self.backend()?;
        let kernel = &workspace.kernel;
        let binding = WorkspaceBinding::from_bytes(KERNEL_AGENT, *run.as_bytes());
        let path =
            RelPath::parse(&create.name).map_err(|_| refused("the file name is not allowed"))?;
        let scopes = RunScopes::new(
            ScopeSet::new([ScopeEntry::WholeProject]),
            ScopeSet::new([ScopeEntry::File(path.clone())]),
            ScopeSet::default(),
        )
        .map_err(|_| refused("the file name is not allowed"))?;
        #[cfg(test)]
        self.shared.fire("session.before.read_grant");
        #[cfg(test)]
        self.shared.fire("session.entered.read_grant");
        let project = kernel
            .projects
            .grant_for_run(kernel.project.id, binding)
            .map_err(project_error)?;
        #[cfg(test)]
        self.shared.fire("session.after.read_grant");
        #[cfg(test)]
        self.shared.fire("session.before.run_construct");
        #[cfg(test)]
        self.shared.fire("session.entered.run_construct");
        let mut staged =
            CodingRun::create_for_project_observing(backend.ledger.store(), project, scopes, stop)
                .map_err(run_error)?;
        #[cfg(test)]
        self.shared.fire("session.after.run_construct");
        // S1a-D: the page is checked with its admission's facts (the S-O22
        // hook alone replaces them, in tests).
        let facts = admission.page_facts.clone();
        #[cfg(test)]
        let facts = self
            .shared
            .prepare_facts
            .lock()
            .expect("files hook")
            .take()
            .unwrap_or(facts);
        let (review_hash, page, candidate) = match stage_one(
            &mut staged,
            (&backend.staging, &self.shared),
            &path,
            create.content.as_bytes(),
            (class, facts),
        ) {
            Ok(checked) => checked,
            Err(error) => {
                self.shared.retire(staged);
                return Err(error);
            }
        };
        // S1a-D (§7 step 3): a page's dialog also says what was checked and
        // what was not; refused whole if it cannot be shown.
        if let Some(page) = &page {
            summary.extend(page_lines(page));
            if summary.len() > MAX_SUMMARY_LINES || !summary.iter().all(|line| is_plain(line)) {
                self.shared.retire(staged);
                return Err(AuthorityError::ChangeTooLargeToShow);
            }
        }
        let target = Digest::of(
            TARGET_DOMAIN,
            &[
                admission.workspace.id.as_bytes(),
                admission.workspace.identity.as_bytes(),
                create.name.as_bytes(),
            ],
        );
        let display = format!("{} in workspace {}", create.name, workspace.info.id);
        let effect = FilesEffect {
            kernel_run: staged.id().ledger_key().into_bytes(),
            run: Mutex::new(Some(staged)),
            workspace,
            backend,
            shared: Arc::clone(&self.shared),
            stop: Arc::clone(stop),
            admission,
            broker_run: run,
            grant,
            review_hash,
            name: create.name.clone(),
            bytes: create.content.len() as u64,
            lines: line_count(&create.content),
            sha256: sha256_hex(create.content.as_bytes()),
            candidate,
            page,
            target,
        };
        let action = PreparedAction {
            kind: CapabilityKind::Files,
            class: EffectClass::R2,
            operation: "create",
            target: TargetIdentity {
                display,
                digest: target,
            },
            parameters: effect.parameters(),
            grants: vec![grant],
            leases: Vec::new(),
            summary,
        };
        Ok(Preparation {
            action,
            effect: Box::new(effect),
            ttl: FILE_TTL,
        })
    }
}

/// The live Files grant on `workspace` (the lowest id, if several).
fn covering_grant(
    authority: &Authority,
    workspace: &AdmittedWorkspace,
) -> Result<GrantId, AuthorityError> {
    authority
        .grants()
        .live_of(CapabilityKind::Files)
        .into_iter()
        .find(|grant| {
            matches!(
                &grant.scope,
                GrantScope::Files { workspace: id, identity, .. }
                    if *id == workspace.id && *identity == workspace.identity
            )
        })
        .map(|grant| grant.id)
        .ok_or(AuthorityError::NoCoveringGrant)
}
