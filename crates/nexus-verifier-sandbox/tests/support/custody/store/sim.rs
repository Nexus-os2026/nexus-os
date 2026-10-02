//! Simulated storage and host (design sections 4, 4.8, 5.5 to 5.8, 10.7 and
//! 16.2), reachable from the store's tests only. It is a bounded model of
//! the states the store's protocols depend on, not a kernel and not a
//! line-for-line port of the design model:
//!
//! - **Bytes.** Each file keeps its kernel-visible bytes (**K**), its
//!   durable bytes (**D**) and its pending (dirty) blocks apart. Process
//!   death (F1) makes nothing durable; power loss (F2) turns **K** into
//!   **D**, with every pending block resolved old, new, torn within its own
//!   4096-byte block, or garbage.
//! - **Writeback errors.** Each file has the kernel's exact `errseq_t`
//!   (`lib/errseq.c`: a 12-bit error, the seen flag, a 19-bit counter), and
//!   each open file description its own cursor, sampled at open. A failed
//!   writeback leaves the page clean with **D** unchanged; it is reported
//!   once per description; page reclaim may then revert **K** while a
//!   descriptor is open; inode eviction (no descriptor open) drops the error.
//!   The counter's finite width is modelled, not hidden: see [`Errseq`].
//! - **Metadata.** `create`, `mkdir`, `link`, `unlink`, `rename` and (R3)
//!   `rmdir` change the visible entries at once and append halves to a
//!   pending log (`rmdir` only of an empty directory; a removed directory
//!   takes no new entry; a `rename` between two names of one file changes
//!   nothing, as POSIX specifies). A directory sync guarantees that
//!   directory's halves. At F2 one permitted schedule decides what became
//!   durable: an ordered prefix (assumption A-M1, the supported profile) or
//!   the per-directory over-approximation, which can split a rename (not
//!   ext4 behaviour; it shows what does not depend on A-M1).
//! - **The journal behaviour activation relies on (A-M2).** A directory
//!   sync through a new descriptor; a read-only superblock; a commit that
//!   fails silently in the commit thread (an abort ext4 has not noticed);
//!   emergency read-only once a handle start notices it; the timestamp
//!   probe, whose transaction runs or has completed, so that the probe's
//!   `fsync` waits (and tests the abort flag) only for a running one; an
//!   abort that lands between ext4's and jbd2's handle tests.
//! - **The storage below the journal.** `Stable` is admitted storage: a
//!   completed write is stable, and no flush is sent. `Volatile` is a
//!   volatile write-back cache that receives flushes; `Unflushed` is a
//!   volatile cache the kernel was told is absent (the configuration A-S1
//!   excludes). A checkpoint's home writes, its discarded flush and its
//!   superblock writes follow the transitions of [`Jbd2`], which models the
//!   transaction windows of design section 10.7 in detail.
//! - **The host.** A fixture host view: the mount table, block and page
//!   sizes, the link sysctls, the effective ext4 option listing, the jbd2
//!   entry names, the kernel identity, and the `/sys/dev/block` link and
//!   sysfs attributes of design section 6.4 step 12, with the Owner's
//!   root-only facts (the controller's Identify data, a virtual machine
//!   guest) as fields only qualification reads. Every value is a fixture in
//!   the form Linux prints it; none was observed on any host or device, and
//!   a passing test qualifies nothing.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::sync::{Arc, Mutex, MutexGuard};

use super::format::BLOCK;
use super::io::{Errno, FileType, IoError, Listing, LockRequest, Stat, StoreIo};
use super::open::Platform;

pub type Ino = u64;

/// `errno` values the simulator records.
pub const EIO: u32 = 5;

// ---------------------------------------------------------------------------
// errseq (lib/errseq.c, Linux v6.17)
// ---------------------------------------------------------------------------

/// The kernel's `errseq_t`: bits 0-11 the last error (`ERRNO_MASK`), bit 12
/// the seen flag (`ERRSEQ_SEEN`), bits 13-31 a counter (`ERRSEQ_CTR_INC`),
/// incremented only when an error is set after the previous one was seen.
/// Collisions are possible when errors are recorded frequently (the source
/// says so): after 2^19 seen errors the counter returns to its earlier value,
/// and a check against a sample taken then reports nothing. A sample of zero
/// never recurs, because every set value carries a non-zero error. The store
/// does not detect such a collision and does not claim to (design section
/// 5.6: an error storm of that size is a storage fault, domain S).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Errseq(pub u32);

impl Errseq {
    pub const ERRNO_MASK: u32 = (1 << 12) - 1;
    pub const SEEN: u32 = 1 << 12;
    pub const CTR_INC: u32 = 1 << 13;

    /// `errseq_set(eseq, -errno)`; zero and out-of-range errors are ignored,
    /// as the kernel does (with a warning).
    pub fn set(&mut self, errno: u32) {
        if errno == 0 || errno > Self::ERRNO_MASK {
            return;
        }
        let old = self.0;
        let mut new = (old & !(Self::ERRNO_MASK | Self::SEEN)) | errno;
        if old & Self::SEEN != 0 {
            new = new.wrapping_add(Self::CTR_INC);
        }
        self.0 = new;
    }

    /// `errseq_sample`: zero if the newest error was not seen yet, so that
    /// the new sampler reports it.
    pub fn sample(&self) -> u32 {
        if self.0 & Self::SEEN == 0 {
            0
        } else {
            self.0
        }
    }

    /// `errseq_check`: the error if anything changed since `since`, without
    /// advancing anything.
    pub fn check(&self, since: u32) -> u32 {
        if self.0 == since {
            0
        } else {
            self.0 & Self::ERRNO_MASK
        }
    }

    /// `errseq_check_and_advance`: report and mark seen.
    pub fn check_and_advance(&mut self, since: &mut u32) -> u32 {
        let old = self.0;
        if old == *since {
            return 0;
        }
        let new = old | Self::SEEN;
        self.0 = new;
        *since = new;
        new & Self::ERRNO_MASK
    }
}

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

/// One directory effect of one metadata operation: an entry added
/// (`name -> ino`) or removed. A rename contributes two halves with the same
/// operation number.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Half {
    pub op: u64,
    pub dir: Ino,
    pub name: String,
    pub ino: Ino,
    pub add: bool,
}

/// A metadata schedule at power loss (design section 4.8).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Schedule {
    /// Every half of the operations numbered below `cut`; `cut` is at least
    /// one past the last guaranteed operation (assumption A-M1).
    Ordered(u64),
    /// For each directory, a prefix of its own halves, at least its
    /// guaranteed prefix (the over-approximation).
    PerDirectory(BTreeMap<Ino, usize>),
}

/// How a pending data block resolves at power loss, within its own block.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tear {
    Old,
    New,
    /// The first `k` bytes new, the rest old.
    Prefix(usize),
    /// Neither old nor new.
    Garbage,
}

/// The storage below the filesystem (design sections 5.5 to 5.8).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Device {
    /// Admitted storage: a completed write is stable; no flush is sent.
    Stable,
    /// A volatile write-back cache that receives flushes.
    Volatile,
    /// A volatile cache the kernel was told is absent: no flush is sent and a
    /// completed write may still be volatile (the configuration A-S1
    /// excludes).
    Unflushed,
}

/// The probe inode's sync transaction (design section 10.7).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProbeTxn {
    Running,
    Completed,
}

/// One durability operation, recorded while tracing (protocol parity).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraceOp {
    pub step: Option<String>,
    pub op: &'static str,
    pub dir: Option<Ino>,
    pub name: Option<String>,
    pub ino: Option<Ino>,
}

/// One interaction, for order checks (a startup's lock before any sync or
/// read of a journal, for instance).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Interaction {
    pub pid: u32,
    pub op: &'static str,
    pub ino: Option<Ino>,
    pub name: Option<String>,
}

#[derive(Debug, Clone)]
struct Inode {
    kind: FileType,
    uid: u32,
    gid: u32,
    mode: u32,
    dev: u64,
    mount_id: u64,
    k: Vec<u8>,
    d: Vec<u8>,
    pending: BTreeSet<u64>,
    errseq: Errseq,
    ents_k: BTreeMap<String, Ino>,
    ents_d: BTreeMap<String, Ino>,
    touched: u64,
}

#[derive(Debug, Clone)]
struct Ofd {
    ino: Ino,
    writable: bool,
    cursor: u32,
    refs: u32,
}

#[derive(Debug, Clone)]
struct Proc {
    uid: u32,
    gid: u32,
    alive: bool,
    name: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LockMode {
    Exclusive,
    Shared,
}

#[derive(Debug, Clone)]
struct Lock {
    mode: LockMode,
    holders: BTreeSet<u64>,
}

type Entries = BTreeMap<Ino, BTreeMap<String, Ino>>;

#[derive(Debug, Clone)]
struct SimState {
    inodes: BTreeMap<Ino, Inode>,
    next_ino: Ino,
    ofds: BTreeMap<u64, Ofd>,
    next_ofd: u64,
    fds: BTreeMap<u64, (u64, u32)>,
    next_fd: u64,
    procs: BTreeMap<u32, Proc>,
    next_pid: u32,
    locks: BTreeMap<Ino, Lock>,
    log: Vec<Half>,
    guaranteed: BTreeSet<usize>,
    op_count: u64,
    aborted: bool,
    emergency_ro: bool,
    probe: Option<(Ino, ProbeTxn)>,
    abort_in_probe_handle: bool,
    dir_sync_plan: VecDeque<Option<Errno>>,
    file_sync_plan: VecDeque<Option<Errno>>,
    write_plan: VecDeque<WritePlan>,
    writeback_fail: BTreeSet<(Ino, u64)>,
    device: Device,
    home_stable: Option<Entries>,
    home_cached: Option<Entries>,
    released: bool,
    host: HostFixture,
    trace: Option<Vec<TraceOp>>,
    step: Option<String>,
    interactions: Vec<Interaction>,
    random_state: u64,
    root: Ino,
}

/// A planned result of one `pwrite` (fault injection).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WritePlan {
    Ok,
    Short(usize),
    Zero,
    Interrupted,
    Fail(Errno),
}

/// The default filesystem device (`makedev(259, 3)`), the fixture NVMe
/// partition's `st_dev`.
pub const FIXTURE_DEV: u64 = makedev(259, 3);
/// The default mount ID of the fixture filesystem.
pub const FIXTURE_MOUNT_ID: u64 = 31;

/// glibc's `makedev`.
pub const fn makedev(major: u32, minor: u32) -> u64 {
    let major = major as u64;
    let minor = minor as u64;
    ((major & 0xffff_f000) << 32)
        | ((major & 0x0000_0fff) << 8)
        | ((minor & 0xffff_ff00) << 12)
        | (minor & 0x0000_00ff)
}

/// One simulated host: its filesystems, processes, journal, storage and
/// host view. Cloning the handle shares the world; [`SimWorld::fork`] copies
/// it (for enumerating crash outcomes from one state).
#[derive(Clone)]
pub struct SimWorld {
    state: Arc<Mutex<SimState>>,
}

impl SimWorld {
    /// An empty filesystem (only `/`) on admitted storage, with the given
    /// host view.
    pub fn new(host: HostFixture) -> SimWorld {
        let mut state = SimState {
            inodes: BTreeMap::new(),
            next_ino: 2,
            ofds: BTreeMap::new(),
            next_ofd: 1,
            fds: BTreeMap::new(),
            next_fd: 3,
            procs: BTreeMap::new(),
            next_pid: 1000,
            locks: BTreeMap::new(),
            log: Vec::new(),
            guaranteed: BTreeSet::new(),
            op_count: 0,
            aborted: false,
            emergency_ro: false,
            probe: None,
            abort_in_probe_handle: false,
            dir_sync_plan: VecDeque::new(),
            file_sync_plan: VecDeque::new(),
            write_plan: VecDeque::new(),
            writeback_fail: BTreeSet::new(),
            device: Device::Stable,
            home_stable: None,
            home_cached: None,
            released: false,
            host,
            trace: None,
            step: None,
            interactions: Vec::new(),
            random_state: 0x9e37_79b9_7f4a_7c15,
            root: 0,
        };
        state.root = state.new_inode(FileType::Directory, 0, 0, 0o755);
        SimWorld {
            state: Arc::new(Mutex::new(state)),
        }
    }

    fn lock(&self) -> MutexGuard<'_, SimState> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// A copy of the whole world, independent of this one. Handles held for
    /// this world do not refer to the copy.
    pub fn fork(&self) -> SimWorld {
        let copy = self.lock().clone();
        SimWorld {
            state: Arc::new(Mutex::new(copy)),
        }
    }

    /// A new process with the given credentials.
    pub fn process(&self, name: &str, uid: u32, gid: u32) -> SimIo {
        let mut state = self.lock();
        let pid = state.next_pid;
        state.next_pid += 1;
        state.procs.insert(
            pid,
            Proc {
                uid,
                gid,
                alive: true,
                name: name.to_string(),
            },
        );
        SimIo {
            world: self.clone(),
            pid,
        }
    }

    /// F1 for one process: its descriptors close (releasing their locks);
    /// nothing becomes durable.
    pub fn kill(&self, io: &SimIo) {
        self.lock().kill(io.pid);
    }

    /// F2 under `schedule` (the guaranteed minimum when `None`), pending data
    /// resolving as `data` unless `tears` says otherwise for a block. Every
    /// process dies.
    pub fn power_loss(
        &self,
        schedule: Option<&Schedule>,
        data: Tear,
        tears: &BTreeMap<(Ino, u64), Tear>,
    ) {
        self.lock().power_loss(schedule, data, tears);
    }

    /// Every permitted schedule of the ordered family (`true`) or of the
    /// per-directory over-approximation (`false`) for the pending log.
    pub fn schedules(&self, ordered: bool) -> Vec<Schedule> {
        self.lock().schedules(ordered)
    }

    /// The inode a path names, following no symbolic link (fixture
    /// inspection, outside any process).
    pub fn lookup(&self, path: &str) -> Option<Ino> {
        let state = self.lock();
        let mut ino = state.root;
        for component in path.split('/').filter(|part| !part.is_empty()) {
            ino = *state.inodes.get(&ino)?.ents_k.get(component)?;
        }
        Some(ino)
    }

    /// The visible entries of a directory.
    pub fn entries(&self, dir: Ino) -> BTreeMap<String, Ino> {
        self.lock()
            .inodes
            .get(&dir)
            .map(|inode| inode.ents_k.clone())
            .unwrap_or_default()
    }

    /// The durable entries of a directory.
    pub fn durable_entries(&self, dir: Ino) -> BTreeMap<String, Ino> {
        self.lock()
            .inodes
            .get(&dir)
            .map(|inode| inode.ents_d.clone())
            .unwrap_or_default()
    }

    /// A file's kernel-visible bytes.
    pub fn visible(&self, ino: Ino) -> Vec<u8> {
        self.lock()
            .inodes
            .get(&ino)
            .map(|inode| inode.k.clone())
            .unwrap_or_default()
    }

    /// A file's durable bytes.
    pub fn durable(&self, ino: Ino) -> Vec<u8> {
        self.lock()
            .inodes
            .get(&ino)
            .map(|inode| inode.d.clone())
            .unwrap_or_default()
    }

    /// A file's pending blocks.
    pub fn pending_blocks(&self, ino: Ino) -> BTreeSet<u64> {
        self.lock()
            .inodes
            .get(&ino)
            .map(|inode| inode.pending.clone())
            .unwrap_or_default()
    }

    /// A file's `errseq_t`.
    pub fn errseq(&self, ino: Ino) -> Errseq {
        self.lock()
            .inodes
            .get(&ino)
            .map(|inode| inode.errseq)
            .unwrap_or_default()
    }

    /// How many times the inode's timestamps were set (`futimens`).
    pub fn touched(&self, ino: Ino) -> u64 {
        self.lock()
            .inodes
            .get(&ino)
            .map(|inode| inode.touched)
            .unwrap_or(0)
    }

    /// Fixture set-up outside any process: durable and visible bytes at
    /// once, nothing pending.
    pub fn install(&self, ino: Ino, data: &[u8]) {
        if let Some(inode) = self.lock().inodes.get_mut(&ino) {
            inode.k = data.to_vec();
            inode.d = data.to_vec();
            inode.pending.clear();
        }
    }

    /// Fixture set-up: overwrite visible bytes at `offset` as a store-uid
    /// process in domain A would (pending until written back).
    pub fn overwrite_visible(&self, ino: Ino, offset: usize, data: &[u8]) {
        let mut state = self.lock();
        if let Some(inode) = state.inodes.get_mut(&ino) {
            let end = offset + data.len();
            if inode.k.len() < end {
                inode.k.resize(end, 0);
            }
            inode.k[offset..end].copy_from_slice(data);
            for block in (offset / BLOCK) as u64..=((end.max(1) - 1) / BLOCK) as u64 {
                inode.pending.insert(block);
            }
        }
    }

    /// Fixture set-up outside any process: a new entry of `kind` in `dir`,
    /// durable at once.
    pub fn fixture_entry(
        &self,
        dir: Ino,
        name: &str,
        kind: FileType,
        owner: (u32, u32, u32),
    ) -> Ino {
        let mut state = self.lock();
        let ino = state.new_inode(kind, owner.0, owner.1, owner.2);
        let dev = state.inodes.get(&dir).map(|inode| inode.dev);
        let mount = state.inodes.get(&dir).map(|inode| inode.mount_id);
        if let Some(inode) = state.inodes.get_mut(&ino) {
            inode.dev = dev.unwrap_or(FIXTURE_DEV);
            inode.mount_id = mount.unwrap_or(FIXTURE_MOUNT_ID);
        }
        if let Some(directory) = state.inodes.get_mut(&dir) {
            directory.ents_k.insert(name.to_string(), ino);
            directory.ents_d.insert(name.to_string(), ino);
        }
        ino
    }

    /// Fixture set-up: link an existing inode under another name, durable.
    pub fn fixture_link(&self, dir: Ino, name: &str, ino: Ino) {
        if let Some(directory) = self.lock().inodes.get_mut(&dir) {
            directory.ents_k.insert(name.to_string(), ino);
            directory.ents_d.insert(name.to_string(), ino);
        }
    }

    /// Fixture set-up: remove an entry, durable.
    pub fn fixture_remove(&self, dir: Ino, name: &str) {
        if let Some(directory) = self.lock().inodes.get_mut(&dir) {
            directory.ents_k.remove(name);
            directory.ents_d.remove(name);
        }
    }

    /// Fixture set-up: an inode's owner and mode.
    pub fn fixture_owner(&self, ino: Ino, uid: u32, gid: u32, mode: u32) {
        if let Some(inode) = self.lock().inodes.get_mut(&ino) {
            inode.uid = uid;
            inode.gid = gid;
            inode.mode = mode;
        }
    }

    /// Fixture set-up: an inode's filesystem (`st_dev`) and mount.
    pub fn fixture_device(&self, ino: Ino, dev: u64, mount_id: u64) {
        if let Some(inode) = self.lock().inodes.get_mut(&ino) {
            inode.dev = dev;
            inode.mount_id = mount_id;
        }
    }

    /// Writeback of `block` of `ino` will fail (EIO recorded in errseq, the
    /// page left clean, **D** unchanged).
    pub fn fail_writeback(&self, ino: Ino, block: u64) {
        self.lock().writeback_fail.insert((ino, block));
    }

    /// Background writeback of every pending block of `ino`.
    pub fn background_writeback(&self, ino: Ino) {
        self.lock().write_back(ino);
    }

    /// Page reclaim: every clean page reverts to the medium's content,
    /// descriptors open or not.
    pub fn reclaim_pages(&self, ino: Ino) {
        let mut state = self.lock();
        if let Some(inode) = state.inodes.get_mut(&ino) {
            let blocks = inode.k.len().div_ceil(BLOCK) as u64;
            for block in 0..blocks {
                if inode.pending.contains(&block) {
                    continue;
                }
                let lo = block as usize * BLOCK;
                let hi = ((block as usize + 1) * BLOCK).min(inode.k.len());
                for at in lo..hi {
                    inode.k[at] = inode.d.get(at).copied().unwrap_or(0);
                }
            }
        }
    }

    /// Inode eviction: only with no open description and nothing pending;
    /// drops the pages and the error sequence.
    pub fn evict_inode(&self, ino: Ino) -> bool {
        let mut state = self.lock();
        let open = state.ofds.values().any(|ofd| ofd.ino == ino);
        let Some(inode) = state.inodes.get_mut(&ino) else {
            return false;
        };
        if open || !inode.pending.is_empty() {
            return false;
        }
        inode.k = inode.d.clone();
        inode.errseq = Errseq::default();
        true
    }

    /// A commit fails in the commit thread: the guaranteed operations were
    /// committed; every other pending operation is lost (still visible until
    /// power loss); the journal is aborted; ext4 has not noticed.
    pub fn abort_journal(&self) {
        self.lock().abort_journal();
    }

    pub fn journal_aborted(&self) -> bool {
        self.lock().aborted
    }

    pub fn emergency_read_only(&self) -> bool {
        self.lock().emergency_ro
    }

    /// The probe's running transaction completes in the background (`fail`:
    /// the commit fails and aborts the journal first).
    pub fn commit_probe(&self, fail: bool) {
        let mut state = self.lock();
        if let Some((ino, ProbeTxn::Running)) = state.probe {
            if fail {
                state.abort_journal();
            }
            state.probe = Some((ino, ProbeTxn::Completed));
        }
    }

    /// The next probe's handle start sees an abort land between ext4's test
    /// and jbd2's (design section 10.7).
    pub fn abort_inside_probe_handle(&self) {
        self.lock().abort_in_probe_handle = true;
    }

    pub fn probe_state(&self) -> Option<(Ino, ProbeTxn)> {
        self.lock().probe
    }

    /// The results of the next directory syncs (`None`: the real result).
    pub fn plan_dir_syncs(&self, plan: impl IntoIterator<Item = Option<Errno>>) {
        self.lock().dir_sync_plan.extend(plan);
    }

    /// The results of the next file syncs (`None`: the real result).
    pub fn plan_file_syncs(&self, plan: impl IntoIterator<Item = Option<Errno>>) {
        self.lock().file_sync_plan.extend(plan);
    }

    /// The results of the next writes.
    pub fn plan_writes(&self, plan: impl IntoIterator<Item = WritePlan>) {
        self.lock().write_plan.extend(plan);
    }

    pub fn set_device(&self, device: Device) {
        let mut state = self.lock();
        state.device = device;
        if device != Device::Stable && state.home_stable.is_none() {
            state.mark_home_stable();
        }
    }

    pub fn device(&self) -> Device {
        self.lock().device
    }

    /// A checkpoint of the committed metadata (design sections 5.7 and 5.8),
    /// in the whole model; [`Jbd2`] models each step in detail. Returns
    /// `Ok` or the error the journal reports.
    pub fn checkpoint(&self, event: Checkpoint) -> Result<(), Errno> {
        self.lock().checkpoint(event)
    }

    /// The host view, for a test to change.
    pub fn with_host<T>(&self, f: impl FnOnce(&mut HostFixture) -> T) -> T {
        f(&mut self.lock().host)
    }

    pub fn host(&self) -> HostFixture {
        self.lock().host.clone()
    }

    /// Start tracing durability operations.
    pub fn trace_start(&self) {
        self.lock().trace = Some(Vec::new());
    }

    /// Stop tracing; the operations recorded.
    pub fn trace_take(&self) -> Vec<TraceOp> {
        self.lock().trace.take().unwrap_or_default()
    }

    /// The protocol step the following operations belong to.
    pub fn set_step(&self, step: Option<&str>) {
        self.lock().step = step.map(str::to_string);
    }

    /// Every interaction so far (and clear the record).
    pub fn take_interactions(&self) -> Vec<Interaction> {
        std::mem::take(&mut self.lock().interactions)
    }

    /// Which open file descriptions hold a lock on `ino`, and how.
    pub fn lock_holders(&self, ino: Ino) -> Option<(bool, usize)> {
        self.lock()
            .locks
            .get(&ino)
            .map(|lock| (lock.mode == LockMode::Exclusive, lock.holders.len()))
    }

    /// Whether `pid` is alive.
    pub fn alive(&self, io: &SimIo) -> bool {
        self.lock()
            .procs
            .get(&io.pid)
            .is_some_and(|proc| proc.alive)
    }

    /// The name of every live process.
    pub fn live_processes(&self) -> Vec<String> {
        self.lock()
            .procs
            .values()
            .filter(|proc| proc.alive)
            .map(|proc| proc.name.clone())
            .collect()
    }

    /// The pending metadata log.
    pub fn pending_metadata(&self) -> Vec<Half> {
        self.lock().log.clone()
    }

    pub fn root(&self) -> Ino {
        self.lock().root
    }
}

/// A checkpoint event of the whole model (design sections 5.6 to 5.8).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Checkpoint {
    /// The home writes complete with success (`Ok`), with a reported error,
    /// or not yet (in flight).
    pub home: Home,
    /// The flush before the tail moves (its status is discarded; only a
    /// volatile cache receives it).
    pub flush_ok: bool,
    /// The superblock's tail write.
    pub superblock_ok: bool,
    /// The abort's rewrite of the superblock, after a failed tail write.
    pub rewrite_ok: bool,
    /// After both superblock writes failed: whether the medium holds the new
    /// tail (`true`) or the old one.
    pub new_tail_on_medium: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Home {
    Ok,
    Fail,
    InFlight,
}

impl Checkpoint {
    pub const CLEAN: Checkpoint = Checkpoint {
        home: Home::Ok,
        flush_ok: true,
        superblock_ok: true,
        rewrite_ok: true,
        new_tail_on_medium: true,
    };
}

impl SimState {
    fn new_inode(&mut self, kind: FileType, uid: u32, gid: u32, mode: u32) -> Ino {
        let ino = self.next_ino;
        self.next_ino += 1;
        self.inodes.insert(
            ino,
            Inode {
                kind,
                uid,
                gid,
                mode,
                dev: FIXTURE_DEV,
                mount_id: FIXTURE_MOUNT_ID,
                k: Vec::new(),
                d: Vec::new(),
                pending: BTreeSet::new(),
                errseq: Errseq::default(),
                ents_k: BTreeMap::new(),
                ents_d: BTreeMap::new(),
                touched: 0,
            },
        );
        ino
    }

    fn proc_of(&self, pid: u32, op: &'static str) -> Result<&Proc, IoError> {
        match self.procs.get(&pid) {
            Some(proc) if proc.alive => Ok(proc),
            _ => Err(IoError::new(op, Errno::BadF)),
        }
    }

    fn note(&mut self, pid: u32, op: &'static str, ino: Option<Ino>, name: Option<&str>) {
        self.interactions.push(Interaction {
            pid,
            op,
            ino,
            name: name.map(str::to_string),
        });
    }

    fn record(&mut self, op: &'static str, dir: Option<Ino>, name: Option<&str>, ino: Option<Ino>) {
        let step = self.step.clone();
        if let Some(trace) = self.trace.as_mut() {
            trace.push(TraceOp {
                step,
                op,
                dir,
                name: name.map(str::to_string),
                ino,
            });
        }
    }

    /// Whether a directory is still reachable: the root, or the target of a
    /// visible entry. A removed directory takes no new entry (`ENOENT`, as in
    /// Linux for a directory being deleted).
    fn reachable(&self, dir: Ino) -> bool {
        dir == self.root
            || self.inodes.values().any(|inode| {
                inode.kind == FileType::Directory
                    && inode.ents_k.values().any(|entry| *entry == dir)
            })
    }

    fn nlink(&self, ino: Ino) -> u64 {
        self.inodes
            .values()
            .filter(|inode| inode.kind == FileType::Directory)
            .map(|inode| inode.ents_k.values().filter(|entry| **entry == ino).count() as u64)
            .sum()
    }

    fn stat_of(&self, ino: Ino) -> Option<Stat> {
        let inode = self.inodes.get(&ino)?;
        Some(Stat {
            file_type: inode.kind,
            uid: inode.uid,
            gid: inode.gid,
            mode: inode.mode,
            nlink: if inode.kind == FileType::Directory {
                2
            } else {
                self.nlink(ino)
            },
            ino,
            dev: inode.dev,
            size: inode.k.len() as u64,
        })
    }

    /// Permission: root bypasses; otherwise owner, group or other bits.
    fn may(&self, pid: u32, ino: Ino, write: bool) -> bool {
        let (Some(proc), Some(inode)) = (self.procs.get(&pid), self.inodes.get(&ino)) else {
            return false;
        };
        if proc.uid == 0 {
            return true;
        }
        let bits = if proc.uid == inode.uid {
            inode.mode >> 6
        } else if proc.gid == inode.gid {
            inode.mode >> 3
        } else {
            inode.mode
        };
        if write {
            bits & 0o2 != 0
        } else {
            bits & 0o4 != 0
        }
    }

    fn check_journal(&mut self) -> Result<(), Errno> {
        if self.emergency_ro {
            return Err(Errno::Rofs);
        }
        if self.aborted {
            self.emergency_ro = true;
            return Err(Errno::Rofs);
        }
        if self.host.superblock_read_only() {
            return Err(Errno::Rofs);
        }
        Ok(())
    }

    fn meta(&mut self, halves: Vec<(Ino, String, Ino, bool)>) {
        let op = self.op_count;
        self.op_count += 1;
        for (dir, name, ino, add) in halves {
            self.log.push(Half {
                op,
                dir,
                name,
                ino,
                add,
            });
        }
    }

    fn apply_halves(&mut self, halves: &[Half]) {
        let mut sorted: Vec<&Half> = halves.iter().collect();
        sorted.sort_by_key(|half| half.op);
        for half in sorted {
            if let Some(directory) = self.inodes.get_mut(&half.dir) {
                if half.add {
                    directory.ents_d.insert(half.name.clone(), half.ino);
                } else if directory.ents_d.get(&half.name) == Some(&half.ino) {
                    directory.ents_d.remove(&half.name);
                }
            }
        }
    }

    /// Guarantee `dir`'s pending halves (a completed directory sync).
    fn guarantee_dir(&mut self, dir: Ino) {
        for (index, half) in self.log.iter().enumerate() {
            if half.dir == dir {
                self.guaranteed.insert(index);
            }
        }
        if !self.log.is_empty() && self.guaranteed.len() == self.log.len() {
            let log = std::mem::take(&mut self.log);
            self.apply_halves(&log);
            self.guaranteed.clear();
        }
        if self.device == Device::Volatile {
            // The commit record's PREFLUSH reaches a volatile cache.
            self.flush_ok();
        }
    }

    /// A commit fails in the commit thread. The transactions that had
    /// committed hold every operation issued before the last guaranteed one
    /// (a directory sync forces the commit of everything issued before it,
    /// in issue order: A-M1); every later pending operation is lost, still
    /// visible until power loss.
    fn abort_journal(&mut self) {
        let cut = self.min_cut();
        let kept: Vec<Half> = self
            .log
            .iter()
            .filter(|half| half.op < cut)
            .cloned()
            .collect();
        self.apply_halves(&kept);
        self.log.clear();
        self.guaranteed.clear();
        self.aborted = true;
    }

    fn min_cut(&self) -> u64 {
        let last = self
            .guaranteed
            .iter()
            .filter_map(|index| self.log.get(*index))
            .map(|half| half.op + 1)
            .max();
        match last {
            Some(cut) => cut,
            None => self.log.first().map_or(self.op_count, |half| half.op),
        }
    }

    fn schedules(&self, ordered: bool) -> Vec<Schedule> {
        if self.log.is_empty() {
            return vec![Schedule::Ordered(self.op_count)];
        }
        if ordered {
            let last = self.log.last().map_or(self.op_count, |half| half.op + 1);
            return (self.min_cut()..=last).map(Schedule::Ordered).collect();
        }
        let mut per: BTreeMap<Ino, Vec<usize>> = BTreeMap::new();
        for (index, half) in self.log.iter().enumerate() {
            per.entry(half.dir).or_default().push(index);
        }
        let mut combos: Vec<BTreeMap<Ino, usize>> = vec![BTreeMap::new()];
        for (dir, indexes) in per {
            let floor = indexes
                .iter()
                .filter(|index| self.guaranteed.contains(index))
                .count();
            let mut next = Vec::new();
            for combo in &combos {
                for keep in floor..=indexes.len() {
                    let mut choice = combo.clone();
                    choice.insert(dir, keep);
                    next.push(choice);
                }
            }
            combos = next;
        }
        combos.into_iter().map(Schedule::PerDirectory).collect()
    }

    fn mark_home_stable(&mut self) {
        let snapshot: Entries = self
            .inodes
            .iter()
            .filter(|(_, inode)| inode.kind == FileType::Directory)
            .map(|(ino, inode)| (*ino, inode.ents_d.clone()))
            .collect();
        self.home_stable = Some(snapshot);
        self.home_cached = None;
        self.released = false;
    }

    /// A flush the device completed: every home write completed before it is
    /// stable. Only a volatile cache receives one.
    fn flush_ok(&mut self) {
        if self.device == Device::Volatile {
            if let Some(cached) = self.home_cached.take() {
                self.home_stable = Some(cached);
                self.released = false;
            }
        }
    }

    /// The committed operations (every one before the last guaranteed one)
    /// leave the pending log: their transactions are written home.
    fn compact_committed(&mut self) {
        let cut = self.min_cut();
        let (committed, rest): (Vec<Half>, Vec<Half>) = std::mem::take(&mut self.log)
            .into_iter()
            .partition(|half| half.op < cut);
        self.apply_halves(&committed);
        self.log = rest;
        self.guaranteed.clear();
    }

    fn checkpoint(&mut self, event: Checkpoint) -> Result<(), Errno> {
        if self.aborted {
            return Err(Errno::Io);
        }
        match event.home {
            // A home write still in flight holds the tail: nothing moves.
            Home::InFlight => return Ok(()),
            // A failed home write aborts the journal before any new tail;
            // the log keeps the transaction.
            Home::Fail => {
                self.abort_journal();
                return Err(Errno::Io);
            }
            Home::Ok => {}
        }
        self.compact_committed();
        let snapshot: Entries = self
            .inodes
            .iter()
            .filter(|(_, inode)| inode.kind == FileType::Directory)
            .map(|(ino, inode)| (*ino, inode.ents_d.clone()))
            .collect();
        if self.device == Device::Stable {
            self.home_stable = Some(snapshot.clone());
        } else {
            if self.home_stable.is_none() {
                self.mark_home_stable();
            }
            self.home_cached = Some(snapshot.clone());
            if self.device == Device::Volatile && event.flush_ok {
                self.flush_ok();
            }
        }
        let released = if event.superblock_ok {
            true
        } else {
            self.abort_journal();
            event.rewrite_ok || event.new_tail_on_medium
        };
        if released && self.home_stable.as_ref() != Some(&snapshot) {
            self.released = true;
        }
        if event.superblock_ok {
            Ok(())
        } else {
            Err(Errno::Io)
        }
    }

    fn sample(&self, ino: Ino) -> u32 {
        self.inodes
            .get(&ino)
            .map(|inode| inode.errseq.sample())
            .unwrap_or(0)
    }

    fn write_back(&mut self, ino: Ino) {
        let blocks: Vec<u64> = self
            .inodes
            .get(&ino)
            .map(|inode| inode.pending.iter().copied().collect())
            .unwrap_or_default();
        for block in blocks {
            let fail = self.writeback_fail.contains(&(ino, block));
            if let Some(inode) = self.inodes.get_mut(&ino) {
                inode.pending.remove(&block);
                if fail {
                    inode.errseq.set(EIO);
                    continue;
                }
                let lo = block as usize * BLOCK;
                let hi = ((block as usize + 1) * BLOCK).min(inode.k.len());
                if lo >= hi {
                    continue;
                }
                if inode.d.len() < hi {
                    inode.d.resize(hi, 0);
                }
                let (k, d) = (&inode.k, &mut inode.d);
                d[lo..hi].copy_from_slice(&k[lo..hi]);
            }
        }
        if let Some(inode) = self.inodes.get_mut(&ino) {
            if inode.pending.is_empty() && inode.d.len() > inode.k.len() {
                inode.d.truncate(inode.k.len());
            }
        }
    }

    fn close_fd(&mut self, fd: u64) {
        let Some((ofd_id, _)) = self.fds.remove(&fd) else {
            return;
        };
        let last = match self.ofds.get_mut(&ofd_id) {
            Some(ofd) => {
                ofd.refs = ofd.refs.saturating_sub(1);
                ofd.refs == 0
            }
            None => false,
        };
        if last {
            if let Some(ofd) = self.ofds.remove(&ofd_id) {
                self.unlock(ofd.ino, ofd_id);
            }
        }
    }

    fn unlock(&mut self, ino: Ino, ofd: u64) {
        let empty = match self.locks.get_mut(&ino) {
            Some(lock) => {
                lock.holders.remove(&ofd);
                lock.holders.is_empty()
            }
            None => false,
        };
        if empty {
            self.locks.remove(&ino);
        }
    }

    fn kill(&mut self, pid: u32) {
        let fds: Vec<u64> = self
            .fds
            .iter()
            .filter(|(_, (_, owner))| *owner == pid)
            .map(|(fd, _)| *fd)
            .collect();
        for fd in fds {
            self.close_fd(fd);
        }
        if let Some(proc) = self.procs.get_mut(&pid) {
            proc.alive = false;
        }
    }

    fn power_loss(
        &mut self,
        schedule: Option<&Schedule>,
        data: Tear,
        tears: &BTreeMap<(Ino, u64), Tear>,
    ) {
        if self.released {
            if let Some(home) = self.home_stable.clone() {
                for (ino, entries) in home {
                    if let Some(inode) = self.inodes.get_mut(&ino) {
                        inode.ents_d = entries;
                    }
                }
            }
        }
        self.released = false;
        self.home_cached = None;
        let schedule = schedule
            .cloned()
            .unwrap_or(Schedule::Ordered(self.min_cut()));
        let durable: Vec<Half> = match &schedule {
            Schedule::Ordered(cut) => {
                let cut = (*cut).max(self.min_cut());
                self.log
                    .iter()
                    .filter(|half| half.op < cut)
                    .cloned()
                    .collect()
            }
            Schedule::PerDirectory(keep) => {
                let mut per: BTreeMap<Ino, Vec<(usize, Half)>> = BTreeMap::new();
                for (index, half) in self.log.iter().enumerate() {
                    per.entry(half.dir).or_default().push((index, half.clone()));
                }
                let mut durable = Vec::new();
                for (dir, items) in per {
                    let floor = items
                        .iter()
                        .filter(|(index, _)| self.guaranteed.contains(index))
                        .count();
                    let count = keep.get(&dir).copied().unwrap_or(0).max(floor);
                    durable.extend(items.into_iter().take(count).map(|(_, half)| half));
                }
                durable
            }
        };
        self.apply_halves(&durable);
        self.log.clear();
        self.guaranteed.clear();
        let inos: Vec<Ino> = self.inodes.keys().copied().collect();
        for ino in inos {
            let Some(inode) = self.inodes.get_mut(&ino) else {
                continue;
            };
            match inode.kind {
                FileType::Directory => inode.ents_k = inode.ents_d.clone(),
                _ => {
                    let pending: Vec<u64> = inode.pending.iter().copied().collect();
                    for block in pending {
                        let outcome = tears.get(&(ino, block)).copied().unwrap_or(data);
                        tear(inode, block, outcome);
                    }
                    inode.pending.clear();
                    inode.k = inode.d.clone();
                    inode.errseq = Errseq::default();
                }
            }
        }
        let pids: Vec<u32> = self.procs.keys().copied().collect();
        for pid in pids {
            if let Some(proc) = self.procs.get_mut(&pid) {
                proc.alive = false;
            }
        }
        self.fds.clear();
        self.ofds.clear();
        self.locks.clear();
        self.aborted = false;
        self.emergency_ro = false;
        self.probe = None;
        self.abort_in_probe_handle = false;
        self.dir_sync_plan.clear();
        self.file_sync_plan.clear();
        self.write_plan.clear();
        if self.home_stable.is_some() {
            self.mark_home_stable();
        }
    }
}

fn tear(inode: &mut Inode, block: u64, outcome: Tear) {
    let lo = block as usize * BLOCK;
    let hi = ((block as usize + 1) * BLOCK).min(inode.k.len());
    if lo >= hi {
        return;
    }
    if outcome == Tear::Old {
        return;
    }
    if inode.d.len() < hi {
        inode.d.resize(hi, 0);
    }
    let new: Vec<u8> = inode.k[lo..hi].to_vec();
    match outcome {
        Tear::Old => {}
        Tear::New => inode.d[lo..hi].copy_from_slice(&new),
        Tear::Prefix(k) => {
            let k = k.min(new.len());
            inode.d[lo..lo + k].copy_from_slice(&new[..k]);
        }
        Tear::Garbage => {
            let mut mixed = new;
            for at in (0..mixed.len()).step_by(97) {
                mixed[at] ^= 0x5a;
            }
            inode.d[lo..hi].copy_from_slice(&mixed);
        }
    }
}

// ---------------------------------------------------------------------------
// The simulated I/O of one process
// ---------------------------------------------------------------------------

/// One process's view of the simulated world: the store's I/O and the host
/// view, with that process's credentials. Its handles close when dropped,
/// and all close when the process dies.
#[derive(Clone)]
pub struct SimIo {
    world: SimWorld,
    pid: u32,
}

/// A directory handle (`O_PATH`): no lock, no content.
#[derive(Debug)]
pub struct SimDir {
    pid: u32,
    ino: Ino,
}

/// An open file description, through one descriptor.
pub struct SimFile {
    world: SimWorld,
    fd: u64,
}

impl Drop for SimFile {
    fn drop(&mut self) {
        self.world.lock().close_fd(self.fd);
    }
}

impl SimIo {
    pub fn world(&self) -> &SimWorld {
        &self.world
    }

    pub fn pid(&self) -> u32 {
        self.pid
    }

    /// The inode a file handle's description names (test inspection).
    pub fn file_ino(&self, file: &SimFile) -> Option<Ino> {
        let state = self.world.lock();
        state
            .fds
            .get(&file.fd)
            .and_then(|(ofd, _)| state.ofds.get(ofd))
            .map(|ofd| ofd.ino)
    }

    /// The inode a directory handle names.
    pub fn dir_ino(&self, dir: &SimDir) -> Ino {
        dir.ino
    }

    /// A second descriptor on the same open file description (fixture only:
    /// the store never duplicates a description).
    pub fn fixture_duplicate(&self, file: &SimFile) -> Result<SimFile, IoError> {
        let mut state = self.world.lock();
        state.proc_of(self.pid, "dup")?;
        let Some((ofd, _)) = state.fds.get(&file.fd).copied() else {
            return Err(IoError::new("dup", Errno::BadF));
        };
        if let Some(entry) = state.ofds.get_mut(&ofd) {
            entry.refs += 1;
        }
        let fd = state.next_fd;
        state.next_fd += 1;
        state.fds.insert(fd, (ofd, self.pid));
        Ok(SimFile {
            world: self.world.clone(),
            fd,
        })
    }

    /// `LOCK_UN` on a description (fixture only: the store never unlocks).
    pub fn fixture_unlock(&self, file: &SimFile) -> Result<(), IoError> {
        let mut state = self.world.lock();
        let Some((ofd, _)) = state.fds.get(&file.fd).copied() else {
            return Err(IoError::new("flock", Errno::BadF));
        };
        let ino = state.ofds.get(&ofd).map(|entry| entry.ino);
        if let Some(ino) = ino {
            state.unlock(ino, ofd);
        }
        Ok(())
    }

    fn with<T>(
        &self,
        op: &'static str,
        f: impl FnOnce(&mut SimState) -> Result<T, Errno>,
    ) -> Result<T, IoError> {
        let mut state = self.world.lock();
        state.proc_of(self.pid, op)?;
        f(&mut state).map_err(|errno| IoError::new(op, errno))
    }

    fn file_ofd(state: &SimState, file: &SimFile) -> Result<(u64, Ino), Errno> {
        let (ofd, _) = state.fds.get(&file.fd).copied().ok_or(Errno::BadF)?;
        let ino = state
            .ofds
            .get(&ofd)
            .map(|entry| entry.ino)
            .ok_or(Errno::BadF)?;
        Ok((ofd, ino))
    }

    fn entry(state: &SimState, dir: &SimDir, name: &str) -> Result<Ino, Errno> {
        if name.is_empty() || name.contains('/') || name == "." || name == ".." {
            return Err(Errno::Inval);
        }
        let directory = state.inodes.get(&dir.ino).ok_or(Errno::BadF)?;
        if directory.kind != FileType::Directory {
            return Err(Errno::NotDir);
        }
        directory.ents_k.get(name).copied().ok_or(Errno::NoEnt)
    }

    fn open_file(
        &self,
        at: &SimDir,
        name: &str,
        op: &'static str,
        writable: bool,
        directory: bool,
    ) -> Result<SimFile, IoError> {
        let pid = self.pid;
        let fd = self.with(op, |state| {
            let ino = Self::entry(state, at, name)?;
            state.note(pid, op, Some(ino), Some(name));
            let kind = state
                .inodes
                .get(&ino)
                .map(|inode| inode.kind)
                .ok_or(Errno::NoEnt)?;
            match kind {
                FileType::Symlink => return Err(Errno::Loop),
                FileType::Directory if !directory => {
                    if writable {
                        return Err(Errno::IsDir);
                    }
                }
                FileType::Directory => {}
                _ if directory => return Err(Errno::NotDir),
                FileType::Fifo if writable => return Err(Errno::NxIo),
                _ => {}
            }
            if !state.may(pid, ino, false) || (writable && !state.may(pid, ino, true)) {
                return Err(Errno::Acces);
            }
            let cursor = state.sample(ino);
            let ofd = state.next_ofd;
            state.next_ofd += 1;
            state.ofds.insert(
                ofd,
                Ofd {
                    ino,
                    writable,
                    cursor,
                    refs: 1,
                },
            );
            let fd = state.next_fd;
            state.next_fd += 1;
            state.fds.insert(fd, (ofd, pid));
            Ok(fd)
        })?;
        Ok(SimFile {
            world: self.world.clone(),
            fd,
        })
    }

    fn sync_regular(state: &mut SimState, ofd: u64, ino: Ino) -> Result<(), Errno> {
        if state.emergency_ro {
            return Err(Errno::Rofs);
        }
        if let Some(Some(errno)) = state.file_sync_plan.pop_front() {
            return Err(errno);
        }
        let mut result = Ok(());
        if !state.host.superblock_read_only() {
            state.write_back(ino);
            if let Some((probed, ProbeTxn::Running)) = state.probe {
                if probed == ino {
                    state.probe = Some((ino, ProbeTxn::Completed));
                    if state.aborted {
                        result = Err(Errno::Io);
                    }
                }
            }
        }
        if state.device == Device::Volatile {
            state.flush_ok();
        }
        let errseq = state.inodes.get(&ino).map(|inode| inode.errseq);
        if let (Some(mut errseq), Some(entry)) = (errseq, state.ofds.get_mut(&ofd)) {
            let error = errseq.check_and_advance(&mut entry.cursor);
            if let Some(inode) = state.inodes.get_mut(&ino) {
                inode.errseq = errseq;
            }
            if error != 0 && result.is_ok() {
                result = Err(Errno::Io);
            }
        }
        result
    }

    fn sync_directory(state: &mut SimState, ino: Ino) -> Result<(), Errno> {
        if let Some(Some(errno)) = state.dir_sync_plan.pop_front() {
            return Err(errno);
        }
        if state.emergency_ro {
            return Err(Errno::Rofs);
        }
        if state.host.superblock_read_only() || state.aborted {
            // A read-only superblock commits nothing; an aborted journal with
            // nothing to commit returns 0 untested (journal.c:513-517).
            return Ok(());
        }
        state.guarantee_dir(ino);
        Ok(())
    }
}

impl StoreIo for SimIo {
    type Dir = SimDir;
    type File = SimFile;

    fn root_dir(&self) -> Result<SimDir, IoError> {
        let pid = self.pid;
        self.with("open /", |state| {
            Ok(SimDir {
                pid,
                ino: state.root,
            })
        })
    }

    fn open_dir(&self, at: &SimDir, name: &str) -> Result<SimDir, IoError> {
        let pid = self.pid;
        self.with("openat2 dir", |state| {
            let ino = Self::entry(state, at, name)?;
            state.note(pid, "open dir", Some(ino), Some(name));
            match state.inodes.get(&ino).map(|inode| inode.kind) {
                Some(FileType::Directory) => Ok(SimDir { pid, ino }),
                Some(FileType::Symlink) => Err(Errno::Loop),
                _ => Err(Errno::NotDir),
            }
        })
    }

    fn stat_at(&self, at: &SimDir, name: &str) -> Result<Stat, IoError> {
        let pid = self.pid;
        self.with("fstatat", |state| {
            let ino = Self::entry(state, at, name)?;
            state.note(pid, "fstatat", Some(ino), Some(name));
            state.stat_of(ino).ok_or(Errno::NoEnt)
        })
    }

    fn stat_dir(&self, dir: &SimDir) -> Result<Stat, IoError> {
        self.with("fstat dir", |state| {
            state.stat_of(dir.ino).ok_or(Errno::BadF)
        })
    }

    fn stat_file(&self, file: &SimFile) -> Result<Stat, IoError> {
        self.with("fstat", |state| {
            let (_, ino) = Self::file_ofd(state, file)?;
            state.stat_of(ino).ok_or(Errno::BadF)
        })
    }

    fn list_dir(&self, dir: &SimDir, limit: usize) -> Result<Listing, IoError> {
        let pid = self.pid;
        self.with("getdents64", |state| {
            state.note(pid, "list", Some(dir.ino), None);
            if !state.may(pid, dir.ino, false) {
                return Err(Errno::Acces);
            }
            let directory = state.inodes.get(&dir.ino).ok_or(Errno::BadF)?;
            if directory.ents_k.len() > limit {
                return Ok(Listing::TooMany);
            }
            Ok(Listing::Names(directory.ents_k.keys().cloned().collect()))
        })
    }

    fn open_read(&self, at: &SimDir, name: &str) -> Result<SimFile, IoError> {
        self.open_file(at, name, "openat2 read", false, false)
    }

    fn open_write(&self, at: &SimDir, name: &str) -> Result<SimFile, IoError> {
        self.open_file(at, name, "openat2 write", true, false)
    }

    fn open_dir_for_sync(&self, at: &SimDir, name: &str) -> Result<SimFile, IoError> {
        self.open_file(at, name, "openat2 dir sync", false, true)
    }

    fn flock(&self, file: &SimFile, request: LockRequest) -> Result<(), IoError> {
        let pid = self.pid;
        self.with("flock", |state| {
            let (ofd, ino) = Self::file_ofd(state, file)?;
            let op = match request {
                LockRequest::Exclusive => "flock-ex",
                LockRequest::Shared => "flock-sh",
            };
            state.note(pid, op, Some(ino), None);
            let mode = match request {
                LockRequest::Exclusive => LockMode::Exclusive,
                LockRequest::Shared => LockMode::Shared,
            };
            let others: BTreeSet<u64> = state
                .locks
                .get(&ino)
                .map(|lock| {
                    lock.holders
                        .iter()
                        .copied()
                        .filter(|holder| *holder != ofd)
                        .collect()
                })
                .unwrap_or_default();
            let held_mode = state.locks.get(&ino).map(|lock| lock.mode);
            match mode {
                LockMode::Exclusive => {
                    if !others.is_empty() {
                        return Err(Errno::Again);
                    }
                    state.locks.insert(
                        ino,
                        Lock {
                            mode,
                            holders: BTreeSet::from([ofd]),
                        },
                    );
                }
                LockMode::Shared => {
                    if held_mode == Some(LockMode::Exclusive) && !others.is_empty() {
                        return Err(Errno::Again);
                    }
                    let mut holders = others;
                    holders.insert(ofd);
                    state.locks.insert(ino, Lock { mode, holders });
                }
            }
            Ok(())
        })
    }

    fn pread(&self, file: &SimFile, offset: u64, buf: &mut [u8]) -> Result<usize, IoError> {
        let pid = self.pid;
        self.with("pread", |state| {
            let (_, ino) = Self::file_ofd(state, file)?;
            state.note(pid, "pread", Some(ino), None);
            let inode = state.inodes.get(&ino).ok_or(Errno::BadF)?;
            if inode.kind == FileType::Directory {
                return Err(Errno::IsDir);
            }
            let start = usize::try_from(offset).map_err(|_| Errno::Inval)?;
            if start >= inode.k.len() {
                return Ok(0);
            }
            let count = buf.len().min(inode.k.len() - start);
            buf[..count].copy_from_slice(&inode.k[start..start + count]);
            Ok(count)
        })
    }

    fn pwrite(&self, file: &SimFile, offset: u64, buf: &[u8]) -> Result<usize, IoError> {
        let pid = self.pid;
        self.with("pwrite", |state| {
            let (ofd, ino) = Self::file_ofd(state, file)?;
            if !state.ofds.get(&ofd).is_some_and(|entry| entry.writable) {
                return Err(Errno::BadF);
            }
            let limit = match state.write_plan.pop_front() {
                None | Some(WritePlan::Ok) => buf.len(),
                Some(WritePlan::Short(count)) => count.min(buf.len()),
                Some(WritePlan::Zero) => return Ok(0),
                Some(WritePlan::Interrupted) => return Err(Errno::Intr),
                Some(WritePlan::Fail(errno)) => return Err(errno),
            };
            let start = usize::try_from(offset).map_err(|_| Errno::Inval)?;
            let end = start.checked_add(limit).ok_or(Errno::Inval)?;
            let step_record = state.procs.get(&pid).is_some_and(|proc| proc.uid == 0);
            {
                let inode = state.inodes.get_mut(&ino).ok_or(Errno::BadF)?;
                if inode.k.len() < end {
                    inode.k.resize(end, 0);
                }
                inode.k[start..end].copy_from_slice(&buf[..limit]);
                if limit > 0 {
                    for block in (start / BLOCK) as u64..=((end - 1) / BLOCK) as u64 {
                        inode.pending.insert(block);
                    }
                }
            }
            if step_record && limit > 0 {
                state.record("write", None, None, Some(ino));
            }
            Ok(limit)
        })
    }

    fn fdatasync(&self, file: &SimFile) -> Result<(), IoError> {
        let pid = self.pid;
        self.with("fdatasync", |state| {
            let (ofd, ino) = Self::file_ofd(state, file)?;
            state.note(pid, "fdatasync", Some(ino), None);
            if state.procs.get(&pid).is_some_and(|proc| proc.uid == 0) {
                state.record("fsync", None, None, Some(ino));
            }
            match state.inodes.get(&ino).map(|inode| inode.kind) {
                Some(FileType::Directory) => Self::sync_directory(state, ino),
                _ => Self::sync_regular(state, ofd, ino),
            }
        })
    }

    fn fsync(&self, file: &SimFile) -> Result<(), IoError> {
        let pid = self.pid;
        self.with("fsync", |state| {
            let (ofd, ino) = Self::file_ofd(state, file)?;
            let directory =
                state.inodes.get(&ino).map(|inode| inode.kind) == Some(FileType::Directory);
            state.note(
                pid,
                if directory { "fsync dir" } else { "fsync" },
                Some(ino),
                None,
            );
            if state.procs.get(&pid).is_some_and(|proc| proc.uid == 0) {
                state.record(
                    if directory { "fsync_dir" } else { "fsync" },
                    Some(ino),
                    None,
                    Some(ino),
                );
            }
            if directory {
                Self::sync_directory(state, ino)
            } else {
                Self::sync_regular(state, ofd, ino)
            }
        })
    }

    fn touch(&self, file: &SimFile) -> Result<(), IoError> {
        let pid = self.pid;
        self.with("futimens", |state| {
            let (_, ino) = Self::file_ofd(state, file)?;
            state.note(pid, "futimens", Some(ino), None);
            let (uid, owner) = (
                state.procs.get(&pid).map(|proc| proc.uid),
                state.inodes.get(&ino).map(|inode| inode.uid),
            );
            if uid != Some(0) && uid != owner {
                return Err(Errno::Perm);
            }
            if state.emergency_ro || state.host.superblock_read_only() {
                return Err(Errno::Rofs);
            }
            if let Some(inode) = state.inodes.get_mut(&ino) {
                inode.touched += 1;
            }
            if state.aborted {
                // ext4_journal_check_start notices the abort: emergency
                // read-only; ext4_dirty_inode swallows the error.
                state.emergency_ro = true;
                return Ok(());
            }
            if state.abort_in_probe_handle {
                state.abort_in_probe_handle = false;
                state.abort_journal();
                return Ok(());
            }
            state.probe = Some((ino, ProbeTxn::Running));
            Ok(())
        })
    }

    fn random(&self, buf: &mut [u8]) -> Result<(), IoError> {
        self.with("getrandom", |state| {
            for byte in buf.iter_mut() {
                state.random_state ^= state.random_state << 13;
                state.random_state ^= state.random_state >> 7;
                state.random_state ^= state.random_state << 17;
                *byte = (state.random_state >> 24) as u8;
            }
            Ok(())
        })
    }

    fn create_exclusive(&self, at: &SimDir, name: &str, mode: u32) -> Result<SimFile, IoError> {
        let pid = self.pid;
        let fd = self.with("openat2 create", |state| {
            if name.is_empty() || name.contains('/') || name == "." || name == ".." {
                return Err(Errno::Inval);
            }
            if !state.may(pid, at.ino, true) {
                return Err(Errno::Acces);
            }
            if !state.reachable(at.ino) {
                return Err(Errno::NoEnt);
            }
            if state
                .inodes
                .get(&at.ino)
                .is_some_and(|directory| directory.ents_k.contains_key(name))
            {
                return Err(Errno::Exist);
            }
            state.check_journal()?;
            let (uid, gid) = state
                .procs
                .get(&pid)
                .map(|proc| (proc.uid, proc.gid))
                .unwrap_or((0, 0));
            let ino = state.new_inode(FileType::Regular, uid, gid, mode & 0o7777);
            let (dev, mount) = state
                .inodes
                .get(&at.ino)
                .map(|directory| (directory.dev, directory.mount_id))
                .unwrap_or((FIXTURE_DEV, FIXTURE_MOUNT_ID));
            if let Some(inode) = state.inodes.get_mut(&ino) {
                inode.dev = dev;
                inode.mount_id = mount;
            }
            if let Some(directory) = state.inodes.get_mut(&at.ino) {
                directory.ents_k.insert(name.to_string(), ino);
            }
            state.meta(vec![(at.ino, name.to_string(), ino, true)]);
            state.record("create", Some(at.ino), Some(name), Some(ino));
            state.note(pid, "create", Some(ino), Some(name));
            let ofd = state.next_ofd;
            state.next_ofd += 1;
            state.ofds.insert(
                ofd,
                Ofd {
                    ino,
                    writable: true,
                    cursor: 0,
                    refs: 1,
                },
            );
            let fd = state.next_fd;
            state.next_fd += 1;
            state.fds.insert(fd, (ofd, pid));
            Ok(fd)
        })?;
        Ok(SimFile {
            world: self.world.clone(),
            fd,
        })
    }

    fn make_dir(&self, at: &SimDir, name: &str, mode: u32) -> Result<(), IoError> {
        let pid = self.pid;
        self.with("mkdirat", |state| {
            if name.is_empty() || name.contains('/') || name == "." || name == ".." {
                return Err(Errno::Inval);
            }
            if !state.may(pid, at.ino, true) {
                return Err(Errno::Acces);
            }
            if !state.reachable(at.ino) {
                return Err(Errno::NoEnt);
            }
            if state
                .inodes
                .get(&at.ino)
                .is_some_and(|directory| directory.ents_k.contains_key(name))
            {
                return Err(Errno::Exist);
            }
            state.check_journal()?;
            let (uid, gid) = state
                .procs
                .get(&pid)
                .map(|proc| (proc.uid, proc.gid))
                .unwrap_or((0, 0));
            let ino = state.new_inode(FileType::Directory, uid, gid, mode & 0o7777);
            let (dev, mount) = state
                .inodes
                .get(&at.ino)
                .map(|directory| (directory.dev, directory.mount_id))
                .unwrap_or((FIXTURE_DEV, FIXTURE_MOUNT_ID));
            if let Some(inode) = state.inodes.get_mut(&ino) {
                inode.dev = dev;
                inode.mount_id = mount;
            }
            if let Some(directory) = state.inodes.get_mut(&at.ino) {
                directory.ents_k.insert(name.to_string(), ino);
            }
            state.meta(vec![(at.ino, name.to_string(), ino, true)]);
            state.record("mkdir", Some(at.ino), Some(name), Some(ino));
            Ok(())
        })
    }

    fn link(
        &self,
        from: &SimDir,
        from_name: &str,
        to: &SimDir,
        to_name: &str,
    ) -> Result<(), IoError> {
        let pid = self.pid;
        self.with("linkat", |state| {
            let ino = Self::entry(state, from, from_name)?;
            if to_name.is_empty() || to_name.contains('/') {
                return Err(Errno::Inval);
            }
            if !state.may(pid, to.ino, true) {
                return Err(Errno::Acces);
            }
            if !state.reachable(to.ino) {
                return Err(Errno::NoEnt);
            }
            if state
                .inodes
                .get(&to.ino)
                .is_some_and(|directory| directory.ents_k.contains_key(to_name))
            {
                return Err(Errno::Exist);
            }
            if state.inodes.get(&ino).map(|inode| inode.kind) == Some(FileType::Directory) {
                return Err(Errno::Perm);
            }
            state.check_journal()?;
            if let Some(directory) = state.inodes.get_mut(&to.ino) {
                directory.ents_k.insert(to_name.to_string(), ino);
            }
            state.meta(vec![(to.ino, to_name.to_string(), ino, true)]);
            state.record("link", Some(to.ino), Some(to_name), Some(ino));
            Ok(())
        })
    }

    fn rename(
        &self,
        from: &SimDir,
        from_name: &str,
        to: &SimDir,
        to_name: &str,
    ) -> Result<(), IoError> {
        let pid = self.pid;
        self.with("renameat", |state| {
            let ino = Self::entry(state, from, from_name)?;
            if to_name.is_empty() || to_name.contains('/') {
                return Err(Errno::Inval);
            }
            if !state.may(pid, from.ino, true) || !state.may(pid, to.ino, true) {
                return Err(Errno::Acces);
            }
            if !state.reachable(to.ino) {
                return Err(Errno::NoEnt);
            }
            state.check_journal()?;
            if state
                .inodes
                .get(&to.ino)
                .and_then(|directory| directory.ents_k.get(to_name))
                == Some(&ino)
            {
                // POSIX: both names already name one file; the call succeeds
                // and changes nothing.
                state.record("rename", Some(to.ino), Some(to_name), Some(ino));
                return Ok(());
            }
            if let Some(directory) = state.inodes.get_mut(&from.ino) {
                directory.ents_k.remove(from_name);
            }
            if let Some(directory) = state.inodes.get_mut(&to.ino) {
                directory.ents_k.insert(to_name.to_string(), ino);
            }
            state.meta(vec![
                (from.ino, from_name.to_string(), ino, false),
                (to.ino, to_name.to_string(), ino, true),
            ]);
            state.record("rename", Some(to.ino), Some(to_name), Some(ino));
            Ok(())
        })
    }

    fn unlink(&self, at: &SimDir, name: &str) -> Result<(), IoError> {
        let pid = self.pid;
        self.with("unlinkat", |state| {
            let ino = Self::entry(state, at, name)?;
            if !state.may(pid, at.ino, true) {
                return Err(Errno::Acces);
            }
            if state.inodes.get(&ino).map(|inode| inode.kind) == Some(FileType::Directory) {
                return Err(Errno::IsDir);
            }
            state.check_journal()?;
            if let Some(directory) = state.inodes.get_mut(&at.ino) {
                directory.ents_k.remove(name);
            }
            state.meta(vec![(at.ino, name.to_string(), ino, false)]);
            state.record("unlink", Some(at.ino), Some(name), Some(ino));
            Ok(())
        })
    }

    fn remove_dir(&self, at: &SimDir, name: &str) -> Result<(), IoError> {
        let pid = self.pid;
        self.with("unlinkat", |state| {
            let ino = Self::entry(state, at, name)?;
            if !state.may(pid, at.ino, true) {
                return Err(Errno::Acces);
            }
            let directory = state.inodes.get(&ino).ok_or(Errno::NoEnt)?;
            if directory.kind != FileType::Directory {
                return Err(Errno::NotDir);
            }
            if !directory.ents_k.is_empty() {
                return Err(Errno::NotEmpty);
            }
            state.check_journal()?;
            if let Some(parent) = state.inodes.get_mut(&at.ino) {
                parent.ents_k.remove(name);
            }
            state.meta(vec![(at.ino, name.to_string(), ino, false)]);
            state.record("rmdir", Some(at.ino), Some(name), Some(ino));
            Ok(())
        })
    }

    fn allocate(&self, file: &SimFile, len: u64) -> Result<(), IoError> {
        self.with("fallocate", |state| {
            let (ofd, ino) = Self::file_ofd(state, file)?;
            if !state.ofds.get(&ofd).is_some_and(|entry| entry.writable) {
                return Err(Errno::BadF);
            }
            let len = usize::try_from(len).map_err(|_| Errno::Inval)?;
            let inode = state.inodes.get_mut(&ino).ok_or(Errno::BadF)?;
            if inode.k.len() < len {
                let old = inode.k.len();
                inode.k.resize(len, 0);
                for block in (old / BLOCK) as u64..len.div_ceil(BLOCK) as u64 {
                    inode.pending.insert(block);
                }
            }
            Ok(())
        })
    }

    fn set_owner(&self, file: &SimFile, uid: u32, gid: u32) -> Result<(), IoError> {
        let pid = self.pid;
        self.with("fchown", |state| {
            let (_, ino) = Self::file_ofd(state, file)?;
            if state.procs.get(&pid).map(|proc| proc.uid) != Some(0) {
                return Err(Errno::Perm);
            }
            let inode = state.inodes.get_mut(&ino).ok_or(Errno::BadF)?;
            inode.uid = uid;
            inode.gid = gid;
            Ok(())
        })
    }
}

// ---------------------------------------------------------------------------
// The host view (design section 6.4)
// ---------------------------------------------------------------------------

/// The fixture filesystem's block device name: the last component of its
/// `/sys/dev/block` link.
pub const FIXTURE_DEVICE_NAME: &str = "nvme0n1p2";
/// The fixture controller's PCI function.
pub const FIXTURE_PCI: &str = "0000:3d:00.0";
/// The kernel identity the fixture qualification pins: `uname` release and
/// version, joined by one space.
pub const FIXTURE_KERNEL_RELEASE: &str = "6.17.0-fixture";
pub const FIXTURE_KERNEL_VERSION: &str = "#1 SMP PREEMPT_DYNAMIC";
/// The effective option listing of the qualified profile, in the form
/// `/proc/fs/ext4/<name>/options` prints it (every option of ext4's table in
/// its effective form, defaults included).
pub const QUALIFIED_LISTING: [&str; 8] = [
    "rw",
    "journal_checksum",
    "barrier",
    "user_xattr",
    "acl",
    "errors=remount-ro",
    "commit=5",
    "data=ordered",
];

/// The fixture's controller and disk directories below `/sys`.
pub fn fixture_controller_dir(pci: &str, controller: &str) -> String {
    format!("/sys/devices/pci0000:00/0000:00:1d.0/{pci}/nvme/{controller}")
}

/// A host view: what the store uid can read of the host (the mount table,
/// the effective profile, the kernel identity and the storage's sysfs form),
/// and, apart from it, the Owner's root-only qualification facts. Every value
/// is a fixture.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostFixture {
    /// `/proc/<pid>/mountinfo`, for every process.
    pub mountinfo: String,
    /// `fstatvfs`: `f_bsize`, `f_frsize`.
    pub fs_block_size: u64,
    pub fs_fragment_size: u64,
    /// `sysconf(_SC_PAGESIZE)`.
    pub page_size: u64,
    /// `fs.protected_hardlinks`, `fs.protected_symlinks`.
    pub protected_hardlinks: u32,
    pub protected_symlinks: u32,
    /// `readlink("/sys/dev/block/<major>:<minor>")`, by device.
    pub device_links: BTreeMap<(u32, u32), String>,
    /// `/proc/fs/ext4/<name>/options`, by device name.
    pub ext4_options: BTreeMap<String, String>,
    /// The entries of `/proc/fs/jbd2`.
    pub jbd2: BTreeSet<String>,
    /// `uname(2)`: release and version.
    pub kernel_release: String,
    pub kernel_version: String,
    /// World-readable sysfs files below `/sys`, by path.
    pub sysfs: BTreeMap<String, String>,
    /// The superblock is mounted read-only (an ordinary read-only mount; the
    /// listing then begins `ro`).
    pub read_only: bool,
    /// Root-only, read by qualification only: the controller's Identify data
    /// report a volatile write cache (and the namespace does not report it
    /// absent).
    pub identify_volatile_cache: bool,
    /// Root-only, read by qualification only: the host is a virtual machine
    /// guest.
    pub guest: bool,
    /// Root-only, read by qualification only: superblock features and the
    /// journal's location.
    pub features: BTreeSet<String>,
    pub journal_inode: u64,
    pub external_journal: bool,
}

impl HostFixture {
    /// The qualified fixture host: ext4 on one partition of an NVMe namespace
    /// behind a PCI Express controller, registered without a volatile write
    /// cache, with the qualified profile and kernel. A fixture: it qualifies
    /// no host or device.
    pub fn qualified() -> HostFixture {
        let controller = fixture_controller_dir(FIXTURE_PCI, "nvme0");
        let disk = format!("{controller}/nvme0n1");
        let mut sysfs = BTreeMap::new();
        sysfs.insert(format!("{controller}/transport"), "pcie\n".to_string());
        sysfs.insert(
            format!("{controller}/model"),
            format!("Fixture NVMe controller{}\n", " ".repeat(17)),
        );
        sysfs.insert(
            format!("{controller}/serial"),
            "FIXTURE-0000000001  \n".to_string(),
        );
        sysfs.insert(
            format!("{controller}/firmware_rev"),
            "FW-0001 \n".to_string(),
        );
        sysfs.insert(format!("{disk}/wwid"), "eui.0025380000000001\n".to_string());
        sysfs.insert(
            format!("{disk}/queue/write_cache"),
            "write through\n".to_string(),
        );
        sysfs.insert(format!("{disk}/queue/fua"), "0\n".to_string());
        let mut device_links = BTreeMap::new();
        device_links.insert(
            (259, 3),
            format!(
                "../../devices/pci0000:00/0000:00:1d.0/{FIXTURE_PCI}/nvme/nvme0/nvme0n1/{FIXTURE_DEVICE_NAME}"
            ),
        );
        let mut ext4_options = BTreeMap::new();
        ext4_options.insert(
            FIXTURE_DEVICE_NAME.to_string(),
            QUALIFIED_LISTING
                .iter()
                .map(|line| format!("{line}\n"))
                .collect(),
        );
        HostFixture {
            mountinfo: [
                "22 1 259:2 / / rw,relatime shared:1 - ext4 /dev/nvme0n1p1 rw,errors=remount-ro",
                "23 22 0:21 / /proc rw,nosuid,nodev,noexec,relatime shared:12 - proc proc rw",
                "31 22 259:3 / /var/lib rw,relatime shared:30 - ext4 /dev/nvme0n1p2 rw,errors=remount-ro",
            ]
            .iter()
            .map(|line| format!("{line}\n"))
            .collect(),
            fs_block_size: 4096,
            fs_fragment_size: 4096,
            page_size: 4096,
            protected_hardlinks: 1,
            protected_symlinks: 1,
            device_links,
            ext4_options,
            jbd2: BTreeSet::from([format!("{FIXTURE_DEVICE_NAME}-8")]),
            kernel_release: FIXTURE_KERNEL_RELEASE.to_string(),
            kernel_version: FIXTURE_KERNEL_VERSION.to_string(),
            sysfs,
            read_only: false,
            identify_volatile_cache: false,
            guest: false,
            features: [
                "has_journal",
                "ext_attr",
                "dir_index",
                "filetype",
                "extent",
                "flex_bg",
                "sparse_super",
                "large_file",
                "metadata_csum",
            ]
            .iter()
            .map(|feature| feature.to_string())
            .collect(),
            journal_inode: 8,
            external_journal: false,
        }
    }

    pub fn superblock_read_only(&self) -> bool {
        self.read_only
    }

    /// The fixture filesystem's mount line's fields 6 and 11.
    pub fn store_mount_options(&self) -> (String, String) {
        for line in self.mountinfo.lines() {
            let fields: Vec<&str> = line.split(' ').collect();
            if fields.first() == Some(&"31") {
                let dash = fields.iter().position(|field| *field == "-");
                if let Some(dash) = dash {
                    if fields.len() > dash + 3 && fields.len() > 5 {
                        return (fields[5].to_string(), fields[dash + 3].to_string());
                    }
                }
            }
        }
        (String::new(), String::new())
    }

    /// Replace the store mount line's per-mount and per-superblock options.
    pub fn set_store_mount_options(&mut self, mount: &str, superblock: &str) {
        let lines: Vec<String> = self
            .mountinfo
            .lines()
            .map(|line| {
                let fields: Vec<&str> = line.split(' ').collect();
                if fields.first() != Some(&"31") {
                    return line.to_string();
                }
                let mut fields: Vec<String> =
                    fields.iter().map(|field| field.to_string()).collect();
                if let Some(dash) = fields.iter().position(|field| field == "-") {
                    if fields.len() > dash + 3 && fields.len() > 5 {
                        fields[5] = mount.to_string();
                        fields[dash + 3] = superblock.to_string();
                    }
                }
                fields.join(" ")
            })
            .collect();
        self.mountinfo = lines.iter().map(|line| format!("{line}\n")).collect();
    }

    /// A read-only remount, as the listing and the mount table then show it.
    pub fn remount_read_only(&mut self) {
        self.read_only = true;
        if let Some(listing) = self.ext4_options.get_mut(FIXTURE_DEVICE_NAME) {
            *listing = listing.replacen("rw\n", "ro\n", 1);
        }
        let (mount, superblock) = self.store_mount_options();
        self.set_store_mount_options(
            &mount.replacen("rw", "ro", 1),
            &superblock.replacen("rw", "ro", 1),
        );
    }

    /// The effective listing with one option replaced or added.
    pub fn set_listing(&mut self, lines: &[&str]) {
        self.ext4_options.insert(
            FIXTURE_DEVICE_NAME.to_string(),
            lines.iter().map(|line| format!("{line}\n")).collect(),
        );
    }

    fn controller_dir(&self) -> Option<String> {
        let link = self.device_links.get(&(259, 3))?;
        let parts: Vec<&str> = link.split('/').collect();
        let at = parts.iter().position(|part| *part == "nvme")?;
        Some(format!("/sys/{}", parts[2..at + 2].join("/")))
    }

    /// Set one sysfs attribute of the fixture's controller (`model`,
    /// `serial`, `firmware_rev`, `transport`) or disk (`wwid`,
    /// `queue/write_cache`, `queue/fua`).
    pub fn set_storage_attribute(&mut self, name: &str, value: Option<&str>) {
        let Some(controller) = self.controller_dir() else {
            return;
        };
        let path = match name {
            "transport" | "model" | "serial" | "firmware_rev" => format!("{controller}/{name}"),
            _ => format!("{controller}/nvme0n1/{name}"),
        };
        match value {
            Some(value) => {
                self.sysfs.insert(path, value.to_string());
            }
            None => {
                self.sysfs.remove(&path);
            }
        }
    }

    /// The fixture's storage attribute values, as `PROVISION` records them
    /// at the Owner's (fixture) qualification: the PCI function, the
    /// partition and the identity digest.
    pub fn storage_record(&self) -> Option<(String, Option<u32>, [u8; 32])> {
        let controller = self.controller_dir()?;
        let read = |path: String| self.sysfs.get(&path).map(|value| value.as_bytes().to_vec());
        let identity = super::format::storage_identity([
            &read(format!("{controller}/model"))?,
            &read(format!("{controller}/serial"))?,
            &read(format!("{controller}/firmware_rev"))?,
            &read(format!("{controller}/nvme0n1/wwid"))?,
        ])?;
        let link = self.device_links.get(&(259, 3))?;
        let last = link.rsplit('/').next()?;
        let partition = last
            .rsplit_once('p')
            .and_then(|(_, number)| number.parse::<u32>().ok())
            .filter(|_| last.starts_with("nvme") && last.contains("n1p"));
        let pci = link.split('/').nth(4)?.to_string();
        Some((pci, partition, identity))
    }
}

/// One storage configuration of design section 5.5 that every opening must
/// refuse, before any claim, with the refusal it must give.
pub struct StorageVariant {
    pub name: &'static str,
    pub host: HostFixture,
    pub refusal: &'static str,
}

/// The sixteen non-admitted storage configurations (design section 16.1,
/// C30), each from the qualified fixture by one change.
pub fn storage_variants() -> Vec<StorageVariant> {
    let base = HostFixture::qualified();
    let mut variants = Vec::new();
    let mut add = |name: &'static str, refusal: &'static str, change: &dyn Fn(&mut HostFixture)| {
        let mut host = base.clone();
        change(&mut host);
        variants.push(StorageVariant {
            name,
            host,
            refusal,
        });
    };
    // A different block device carries its own profile files: its option
    // listing and its jbd2 entry are named after it, as the kernel names
    // them, so that only the storage check (step 12) refuses it.
    let link = |host: &mut HostFixture, target: &str| {
        host.device_links.insert((259, 3), target.to_string());
        let name = target.rsplit('/').next().unwrap_or_default().to_string();
        if name != FIXTURE_DEVICE_NAME {
            if let Some(listing) = host.ext4_options.remove(FIXTURE_DEVICE_NAME) {
                host.ext4_options.insert(name.clone(), listing);
            }
            host.jbd2 = BTreeSet::from([format!("{name}-8")]);
        }
    };
    add(
        "a volatile cache hidden by queue/write_cache",
        "volatile write cache, flushes disabled in the kernel's view only",
        &|host| host.set_storage_attribute("queue/fua", Some("1\n")),
    );
    add(
        "a volatile write-back cache",
        "volatile write cache",
        &|host| {
            host.set_storage_attribute("queue/write_cache", Some("write back\n"));
            host.set_storage_attribute("queue/fua", Some("1\n"));
        },
    );
    add(
        "contradictory cache attributes",
        "contradictory storage information",
        &|host| host.set_storage_attribute("queue/write_cache", Some("write back\n")),
    );
    add("an unreadable attribute", "storage unreadable", &|host| {
        host.set_storage_attribute("wwid", None)
    });
    add(
        "a device-mapper device",
        "storage not a direct NVMe namespace on PCI Express",
        &|host| link(host, "../../devices/virtual/block/dm-1"),
    );
    add(
        "an NVMe multipath disk",
        "storage not a direct NVMe namespace on PCI Express",
        &|host| {
            link(
                host,
                "../../devices/virtual/nvme-subsystem/nvme-subsys0/nvme0n1/nvme0n1p2",
            )
        },
    );
    add(
        "NVMe over fabrics",
        "storage not a direct NVMe namespace on PCI Express",
        &|host| {
            link(
                host,
                "../../devices/virtual/nvme-fabrics/ctl/nvme0/nvme0n1/nvme0n1p2",
            )
        },
    );
    add(
        "a SATA or SAS disk",
        "storage not a direct NVMe namespace on PCI Express",
        &|host| {
            link(
                host,
                "../../devices/pci0000:00/0000:00:17.0/ata1/host0/target0:0:0/0:0:0:0/block/sda/sda2",
            )
        },
    );
    add(
        "a loop device",
        "storage not a direct NVMe namespace on PCI Express",
        &|host| link(host, "../../devices/virtual/block/loop0"),
    );
    add(
        "a virtio disk",
        "storage not a direct NVMe namespace on PCI Express",
        &|host| {
            link(
                host,
                "../../devices/pci0000:00/0000:00:04.0/virtio1/block/vda/vda2",
            )
        },
    );
    add(
        "a transport other than pcie",
        "storage transport",
        &|host| host.set_storage_attribute("transport", Some("tcp\n")),
    );
    add("another controller", "storage not qualified", &|host| {
        host.set_storage_attribute("model", Some("Another NVMe controller                 \n"))
    });
    add(
        "another firmware revision",
        "storage not qualified",
        &|host| host.set_storage_attribute("firmware_rev", Some("FW-0002 \n")),
    );
    add("another namespace", "storage not qualified", &|host| {
        host.set_storage_attribute("wwid", Some("eui.0025380000000002\n"))
    });
    add("another PCI function", "storage not qualified", &|host| {
        let moved = format!(
            "../../devices/pci0000:00/0000:00:1d.0/0000:3e:00.0/nvme/nvme0/nvme0n1/{FIXTURE_DEVICE_NAME}"
        );
        let old_controller = fixture_controller_dir(FIXTURE_PCI, "nvme0");
        let new_controller = fixture_controller_dir("0000:3e:00.0", "nvme0");
        let moved_files: BTreeMap<String, String> = host
            .sysfs
            .iter()
            .map(|(path, value)| {
                (
                    path.replacen(&old_controller, &new_controller, 1),
                    value.clone(),
                )
            })
            .collect();
        host.sysfs = moved_files;
        link(host, &moved);
    });
    add("another partition", "storage not qualified", &|host| {
        link(
            host,
            &format!(
                "../../devices/pci0000:00/0000:00:1d.0/{FIXTURE_PCI}/nvme/nvme0/nvme0n1/nvme0n1p3"
            ),
        )
    });
    variants
}

impl Platform for SimIo {
    fn process_id(&self) -> u32 {
        self.pid
    }

    fn mount_id(&self, dir: &SimDir) -> Result<u64, IoError> {
        self.with("statx", |state| {
            state
                .inodes
                .get(&dir.ino)
                .map(|inode| inode.mount_id)
                .ok_or(Errno::BadF)
        })
    }

    fn read_mountinfo(&self, _pid: u32, limit: usize) -> Result<Vec<u8>, IoError> {
        self.with("read mountinfo", |state| {
            let text = state.host.mountinfo.as_bytes();
            Ok(text[..text.len().min(limit + 1)].to_vec())
        })
    }

    fn fs_block_sizes(&self, _dir: &SimDir) -> Result<(u64, u64), IoError> {
        self.with("fstatvfs", |state| {
            Ok((state.host.fs_block_size, state.host.fs_fragment_size))
        })
    }

    fn page_size(&self) -> Result<u64, IoError> {
        self.with("sysconf", |state| Ok(state.host.page_size))
    }

    fn link_protection(&self) -> Result<(u32, u32), IoError> {
        self.with("read sysctl", |state| {
            Ok((
                state.host.protected_hardlinks,
                state.host.protected_symlinks,
            ))
        })
    }

    fn block_device_link(&self, major: u32, minor: u32) -> Result<Option<String>, IoError> {
        self.with("readlink", |state| {
            Ok(state.host.device_links.get(&(major, minor)).cloned())
        })
    }

    fn ext4_options(&self, device: &str, limit: usize) -> Result<Option<Vec<u8>>, IoError> {
        self.with("read options", |state| {
            Ok(state.host.ext4_options.get(device).map(|text| {
                let bytes = text.as_bytes();
                bytes[..bytes.len().min(limit + 1)].to_vec()
            }))
        })
    }

    fn jbd2_entry_exists(&self, entry: &str) -> Result<bool, IoError> {
        self.with("lookup jbd2", |state| Ok(state.host.jbd2.contains(entry)))
    }

    fn kernel_identity(&self) -> Result<(String, String), IoError> {
        self.with("uname", |state| {
            Ok((
                state.host.kernel_release.clone(),
                state.host.kernel_version.clone(),
            ))
        })
    }

    fn sysfs_attribute(&self, path: &str, limit: usize) -> Result<Option<Vec<u8>>, IoError> {
        let pid = self.pid;
        self.with("read sysfs", |state| {
            state.note(pid, "read sysfs", None, Some(path));
            Ok(state.host.sysfs.get(path).map(|text| {
                let bytes = text.as_bytes();
                bytes[..bytes.len().min(limit + 1)].to_vec()
            }))
        })
    }
}

// ---------------------------------------------------------------------------
// The journal transaction model (design sections 5.1, 5.5 to 5.8 and 10.7)
// ---------------------------------------------------------------------------

/// A transaction's state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum TxnState {
    Running,
    Committing,
    Completed,
}

/// How a completed transaction ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum TxnOutcome {
    Committed,
    Failed,
}

/// A committed transaction's home blocks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum HomeState {
    Unwritten,
    InFlight,
    Failed,
    /// Completed into a volatile cache: not stable yet.
    Written,
    Durable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Txn {
    pub tid: u64,
    pub ops: Vec<&'static str>,
    pub state: TxnState,
    pub outcome: Option<TxnOutcome>,
    pub log_durable: bool,
    pub in_log: bool,
    pub home: HomeState,
}

/// Which branch the probe's `fsync` took.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Branch {
    Running,
    Committing,
    Completed,
}

/// What the transaction model did, in order (the windows of design section
/// 10.7 are read from it).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JournalTrace {
    Abort(&'static str),
    ProbeAbortSeen,
    ProbePassedExt4Test,
    FsyncBranch(Branch),
    CompletedBranch { aborted: bool },
    Completed(u64, TxnOutcome),
    SuperblockWriteFailed(u64),
    SyncsReturned,
}

/// A flush's result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Flush {
    /// The block layer completed it without the device (no volatile cache
    /// advertised, `blk-core.c:809-820`).
    NotSent,
    Ok,
    Failed,
}

/// The smallest state of the Linux v6.17 jbd2 and ext4 paths the activation
/// proof and the storage contract depend on. Each method is one source
/// branch, as cited; it is not a kernel. The journal is internal or
/// external, commits synchronous or asynchronous; transactions run, commit
/// and complete, committed or failed; the abort flag; ext4's emergency
/// read-only flag; the probe inode's sync tid; and for each committed
/// transaction whether its log blocks are durable and still in the log and
/// whether its home blocks are written, durable, failed or in flight. Flush
/// failures come from `flushes`, commit results from `outcomes`, home write
/// results from `home_plan`, superblock write results from `sb_plan`.
#[derive(Debug, Clone)]
pub struct Jbd2 {
    pub external: bool,
    pub async_commit: bool,
    pub device: Device,
    pub emulated_fua: bool,
    pub txns: BTreeMap<u64, Txn>,
    next_tid: u64,
    pub running: Option<u64>,
    pub committing: Option<u64>,
    commit_sequence: u64,
    pub aborted: bool,
    pub emergency: bool,
    lock_tid: u64,
    pub outcomes: VecDeque<bool>,
    pub flushes: VecDeque<bool>,
    pub sites: BTreeMap<(&'static str, &'static str), u32>,
    pub trace: Vec<JournalTrace>,
    pub home_plan: VecDeque<Home>,
    pub sb_plan: VecDeque<bool>,
    dev_err: bool,
    mem_tail: u64,
    sb_mem: u64,
    pub sb_disk: BTreeSet<u64>,
}

impl Jbd2 {
    pub fn new(external: bool, async_commit: bool, device: Device, emulated_fua: bool) -> Jbd2 {
        Jbd2 {
            external,
            async_commit,
            device,
            emulated_fua,
            txns: BTreeMap::new(),
            next_tid: 1,
            running: None,
            committing: None,
            commit_sequence: 0,
            aborted: false,
            emergency: false,
            lock_tid: 0,
            outcomes: VecDeque::new(),
            flushes: VecDeque::new(),
            sites: BTreeMap::new(),
            trace: Vec::new(),
            home_plan: VecDeque::new(),
            sb_plan: VecDeque::new(),
            dev_err: false,
            mem_tail: 1,
            sb_mem: 1,
            sb_disk: BTreeSet::from([1]),
        }
    }

    /// The supported profile on admitted storage.
    pub fn admitted() -> Jbd2 {
        Jbd2::new(false, false, Device::Stable, false)
    }

    pub fn txn_of(&self, name: &str) -> Option<&Txn> {
        self.txns.values().find(|txn| txn.ops.contains(&name))
    }

    pub fn committed(&self, name: &str) -> bool {
        self.txn_of(name)
            .is_some_and(|txn| txn.outcome == Some(TxnOutcome::Committed))
    }

    /// After a power loss, recovery replays the committed transactions still
    /// in the log from the superblock's tail (`recovery.c:611`); one behind the
    /// tail survives only through its durable home blocks. After a failed
    /// superblock write the medium may hold either tail.
    pub fn survives(&self, name: &str) -> bool {
        let Some(txn) = self.txn_of(name) else {
            return false;
        };
        if txn.outcome != Some(TxnOutcome::Committed) {
            return false;
        }
        if txn.home == HomeState::Durable {
            return true;
        }
        txn.log_durable && self.sb_disk.iter().all(|tail| txn.tid >= *tail)
    }

    /// `jbd2_journal_abort`: permanent; it writes the superblock with its
    /// error code, carrying the in-memory tail (`journal.c:2549-2597`).
    pub fn abort(&mut self, why: &'static str) {
        if !self.aborted {
            self.aborted = true;
            self.trace.push(JournalTrace::Abort(why));
            self.write_superblock();
        }
    }

    fn join(&mut self) -> u64 {
        if let Some(tid) = self.running {
            return tid;
        }
        let tid = self.next_tid;
        self.next_tid += 1;
        self.txns.insert(
            tid,
            Txn {
                tid,
                ops: Vec::new(),
                state: TxnState::Running,
                outcome: None,
                log_durable: false,
                in_log: true,
                home: HomeState::Unwritten,
            },
        );
        self.running = Some(tid);
        tid
    }

    /// A metadata operation's handle start: an emergency state refuses, an
    /// aborted journal becomes emergency read-only (`ext4_jbd2.c:65-91`).
    pub fn op(&mut self, name: &'static str) -> Result<(), Errno> {
        if self.emergency {
            return Err(Errno::Rofs);
        }
        if self.aborted {
            self.emergency = true;
            return Err(Errno::Rofs);
        }
        let tid = self.join();
        if let Some(txn) = self.txns.get_mut(&tid) {
            txn.ops.push(name);
        }
        Ok(())
    }

    /// kjournald2 takes the running transaction to commit.
    pub fn start_commit(&mut self) -> bool {
        let (Some(tid), None) = (self.running, self.committing) else {
            return false;
        };
        self.committing = Some(tid);
        self.running = None;
        if let Some(txn) = self.txns.get_mut(&tid) {
            txn.state = TxnState::Committing;
        }
        true
    }

    /// A cache flush: only a volatile cache the kernel knows of receives it;
    /// otherwise it completes without the device and a planned failure is not
    /// consumed. `checked` records whether the caller tests the status; an
    /// unchecked failure changes nothing (the source discards it).
    fn flush(&mut self, site: &'static str, checked: bool) -> Flush {
        let _ = checked;
        if self.device != Device::Volatile {
            *self.sites.entry((site, "not sent")).or_default() += 1;
            return Flush::NotSent;
        }
        let ok = self.flushes.pop_front().unwrap_or(true);
        *self
            .sites
            .entry((site, if ok { "ok" } else { "fail" }))
            .or_default() += 1;
        if ok {
            Flush::Ok
        } else {
            Flush::Failed
        }
    }

    /// A flush the device completed makes every completed home write stable.
    fn home_flushed(&mut self, ok: bool) {
        if ok && self.device == Device::Volatile {
            for txn in self.txns.values_mut() {
                if txn.home == HomeState::Written {
                    txn.home = HomeState::Durable;
                }
            }
        }
    }

    /// A home write completes successfully (stable on admitted storage,
    /// otherwise into the cache), with a reported error recorded on the
    /// device's errseq before its buffer unlocks, or not yet.
    fn home_write(&mut self, tid: u64, outcome: Home) {
        let device = self.device;
        if outcome == Home::Fail {
            self.dev_err = true;
        }
        if let Some(txn) = self.txns.get_mut(&tid) {
            txn.home = match outcome {
                Home::InFlight => HomeState::InFlight,
                Home::Fail => HomeState::Failed,
                Home::Ok if device == Device::Stable => HomeState::Durable,
                Home::Ok => HomeState::Written,
            };
        }
    }

    /// A buffer leaves the checkpoint list only once unlocked: its write
    /// completed, with success or an error (`checkpoint.c:235-258`).
    fn home_done(txn: &Txn) -> bool {
        matches!(
            txn.home,
            HomeState::Written | HomeState::Durable | HomeState::Failed
        )
    }

    /// The oldest transaction still on the checkpoint list, else the
    /// committing or running one (`journal.c:1017-1044`).
    fn log_tail(&self) -> u64 {
        self.txns
            .values()
            .filter(|txn| {
                txn.in_log
                    && (txn.state != TxnState::Completed
                        || (txn.outcome == Some(TxnOutcome::Committed) && !Self::home_done(txn)))
            })
            .map(|txn| txn.tid)
            .min()
            .unwrap_or(self.next_tid)
    }

    /// `jbd2_write_superblock` with FUA: on admitted storage and with native
    /// FUA it is stable at completion; with emulated FUA a flush follows the
    /// write and its failure fails the write. A failed write may or may not
    /// have reached the medium, and aborts the journal, whose rewrite carries
    /// the same in-memory superblock.
    fn write_superblock(&mut self) -> bool {
        let mut ok = self.sb_plan.pop_front().unwrap_or(true);
        if ok && self.device == Device::Volatile && self.emulated_fua {
            match self.flush("blk-flush.c:398-403", true) {
                Flush::Ok => self.home_flushed(true),
                Flush::Failed => ok = false,
                Flush::NotSent => {}
            }
        }
        if ok {
            self.sb_disk = BTreeSet::from([self.sb_mem]);
            return true;
        }
        self.sb_disk.insert(self.sb_mem);
        self.trace
            .push(JournalTrace::SuperblockWriteFailed(self.sb_mem));
        if !self.aborted {
            self.abort("the superblock write failed (journal.c:1827-1837)");
        }
        false
    }

    /// `__jbd2_update_log_tail` and `jbd2_journal_update_sb_log_tail`: an
    /// aborted journal refuses; a write error recorded on the device aborts
    /// before the new tail is set; the superblock is written, and only then
    /// does the in-memory tail move.
    fn tail_update(&mut self) -> Result<(), Errno> {
        if self.aborted {
            return Err(Errno::Io);
        }
        let new = self.log_tail();
        if new <= self.mem_tail {
            return Ok(());
        }
        if self.dev_err {
            self.abort("a home write failed (journal.c:1861-1864)");
            return Err(Errno::Io);
        }
        self.sb_mem = new;
        if !self.write_superblock() {
            return Err(Errno::Io);
        }
        self.mem_tail = new;
        for txn in self.txns.values_mut() {
            if txn.tid < new {
                txn.in_log = false;
            }
        }
        Ok(())
    }

    /// The committing transaction completes (`commit.c`); `ok == false` is a
    /// checked failure of a log or commit-record write. Every abort precedes
    /// the completion.
    pub fn finish_commit(&mut self, ok: bool) -> bool {
        let Some(tid) = self.committing else {
            return false;
        };
        let tail = self.txns.values().any(|txn| {
            txn.in_log
                && txn.outcome == Some(TxnOutcome::Committed)
                && txn.home != HomeState::Unwritten
        });
        if self.external && !self.aborted {
            // commit.c:775-778: the filesystem device is flushed before the
            // record; the status is discarded.
            let flushed = self.flush("commit.c:775-778", false) == Flush::Ok;
            self.home_flushed(flushed);
        }
        if !self.aborted && !ok {
            self.abort("a commit's checked write failed");
        }
        let outcome = if self.aborted {
            TxnOutcome::Failed
        } else {
            let log_durable = if self.async_commit {
                // commit.c:781-786, 883-886: the final flush's status is
                // discarded.
                let last = self.flush("commit.c:883-886", false);
                self.device == Device::Stable || last == Flush::Ok
            } else {
                let site = if self.device == Device::Volatile {
                    "ok"
                } else {
                    "not sent"
                };
                *self.sites.entry(("commit.c:152-154", site)).or_default() += 1;
                if !self.external {
                    self.home_flushed(true);
                }
                self.device != Device::Unflushed
            };
            if let Some(txn) = self.txns.get_mut(&tid) {
                txn.log_durable = log_durable;
            }
            TxnOutcome::Committed
        };
        if let Some(txn) = self.txns.get_mut(&tid) {
            txn.outcome = Some(outcome);
        }
        if outcome == TxnOutcome::Committed && tail {
            let _ = self.tail_update();
        }
        if let Some(txn) = self.txns.get_mut(&tid) {
            txn.state = TxnState::Completed;
        }
        self.commit_sequence = tid;
        self.committing = None;
        self.trace.push(JournalTrace::Completed(tid, outcome));
        true
    }

    /// `jbd2_log_wait_commit`: until `tid` completed; commits run one at a
    /// time, in order.
    fn complete_through(&mut self, tid: u64) {
        while self.commit_sequence < tid {
            if self.committing.is_none() && !self.start_commit() {
                break;
            }
            let ok = self.outcomes.pop_front().unwrap_or(true);
            self.finish_commit(ok);
        }
    }

    /// `fsync` of a directory: an emergency state first; a full commit waits
    /// for the running transaction, else the committing one, and then tests
    /// the abort flag; with neither, "Nothing to commit" returns 0 untested.
    pub fn dir_fsync(&mut self) -> Result<(), Errno> {
        if self.emergency {
            return Err(Errno::Rofs);
        }
        let Some(tid) = self.running.or(self.committing) else {
            return Ok(());
        };
        self.complete_through(tid);
        if self.aborted {
            Err(Errno::Io)
        } else {
            Ok(())
        }
    }

    /// `futimens(LOCK)` by its owner: an emergency state first; the handle
    /// start turns an aborted journal into emergency read-only and swallows
    /// the error. `window`: the abort lands after ext4's test and before
    /// jbd2's, which sets nothing; the sync tid is not updated.
    pub fn probe_touch(&mut self, window: bool) -> Result<(), Errno> {
        if self.emergency {
            return Err(Errno::Rofs);
        }
        if self.aborted {
            self.emergency = true;
            self.trace.push(JournalTrace::ProbeAbortSeen);
            return Ok(());
        }
        self.trace.push(JournalTrace::ProbePassedExt4Test);
        if window {
            self.abort("between the probe's two handle tests");
            return Ok(());
        }
        let tid = self.join();
        if let Some(txn) = self.txns.get_mut(&tid) {
            txn.ops.push("probe");
        }
        self.lock_tid = tid;
        Ok(())
    }

    /// `fsync(LOCK)`: an emergency state first; a running or committing sync
    /// transaction is waited for and the abort flag tested; a completed one
    /// returns 0 untested (`journal.c:800-805`), after ext4's own checked
    /// flush, which admitted storage is not sent.
    pub fn probe_fsync(&mut self) -> Result<(), Errno> {
        if self.emergency {
            return Err(Errno::Rofs);
        }
        let tid = self.lock_tid;
        if Some(tid) == self.running || Some(tid) == self.committing {
            let branch = if Some(tid) == self.running {
                Branch::Running
            } else {
                Branch::Committing
            };
            self.trace.push(JournalTrace::FsyncBranch(branch));
            self.complete_through(tid);
            return if self.aborted { Err(Errno::Io) } else { Ok(()) };
        }
        self.trace
            .push(JournalTrace::FsyncBranch(Branch::Completed));
        self.trace.push(JournalTrace::CompletedBranch {
            aborted: self.aborted,
        });
        if self.flush("fsync.c:166-170", true) == Flush::Failed {
            Err(Errno::Io)
        } else {
            Ok(())
        }
    }

    /// Committed transactions' home blocks are written (checkpointing or
    /// ordinary writeback), each with the next planned outcome.
    pub fn writeback(&mut self) {
        let tids: Vec<u64> = self
            .txns
            .values()
            .filter(|txn| {
                txn.outcome == Some(TxnOutcome::Committed)
                    && txn.in_log
                    && txn.home == HomeState::Unwritten
            })
            .map(|txn| txn.tid)
            .collect();
        for tid in tids {
            let outcome = self.home_plan.pop_front().unwrap_or(Home::Ok);
            self.home_write(tid, outcome);
        }
    }

    /// Home writes still in flight complete.
    pub fn complete_inflight(&mut self, ok: bool) {
        let tids: Vec<u64> = self
            .txns
            .values()
            .filter(|txn| txn.home == HomeState::InFlight)
            .map(|txn| txn.tid)
            .collect();
        for tid in tids {
            self.home_write(tid, if ok { Home::Ok } else { Home::Fail });
        }
    }

    /// `jbd2_log_do_checkpoint` writes the home blocks;
    /// `jbd2_cleanup_journal_tail`: an aborted journal refuses; the flush
    /// before the tail moves is issued and its status discarded
    /// (`checkpoint.c:338-339`); the superblock update then moves the tail.
    pub fn checkpoint(&mut self) -> Result<(), Errno> {
        self.writeback();
        if self.aborted {
            return Err(Errno::Io);
        }
        let flushed = self.flush("checkpoint.c:338-339", false) == Flush::Ok;
        self.home_flushed(flushed);
        self.tail_update()
    }

    /// One event of the model's environment (design section 16.1): the
    /// dependency operations, unrelated operations, commits started or
    /// completed in the background, an abort from an ext4 error elsewhere,
    /// a checkpoint, and the storage's own events. Returns whether the event
    /// applied (`strict`: a commit start with nothing to commit does not).
    pub fn event(&mut self, event: JournalEvent, strict: bool) -> bool {
        match event {
            JournalEvent::D1 => self.op("d1").is_ok(),
            JournalEvent::D2 => self.op("d2").is_ok(),
            JournalEvent::U => {
                let _ = self.op("u");
                true
            }
            JournalEvent::U2 => {
                let _ = self.op("u2");
                true
            }
            JournalEvent::Start => self.start_commit() || !strict,
            JournalEvent::CommitOk => self.finish_commit(true),
            JournalEvent::CommitFail => self.finish_commit(false),
            JournalEvent::BackgroundOk | JournalEvent::BackgroundFail => {
                if self.committing.is_none() && !self.start_commit() {
                    return true;
                }
                self.finish_commit(event == JournalEvent::BackgroundOk)
            }
            JournalEvent::Abort => {
                self.abort("an ext4 error elsewhere");
                true
            }
            JournalEvent::Writeback => {
                self.writeback();
                true
            }
            JournalEvent::Checkpoint => {
                let _ = self.checkpoint();
                true
            }
            JournalEvent::FlushFails(count) => {
                self.flushes = std::iter::repeat_n(false, count).collect();
                true
            }
            JournalEvent::HomeWriteFails => {
                self.home_plan = VecDeque::from([Home::Fail]);
                true
            }
            JournalEvent::HomeInFlight => {
                self.home_plan = VecDeque::from([Home::InFlight]);
                self.writeback();
                true
            }
            JournalEvent::InFlightCompletes => {
                self.complete_inflight(true);
                true
            }
            JournalEvent::SuperblockFails => {
                self.sb_plan = VecDeque::from([false, true]);
                true
            }
            JournalEvent::SuperblockFailsTwice => {
                self.sb_plan = VecDeque::from([false, false]);
                true
            }
        }
    }
}

/// The environment's events (design section 16.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum JournalEvent {
    D1,
    D2,
    U,
    U2,
    Start,
    CommitOk,
    CommitFail,
    BackgroundOk,
    BackgroundFail,
    Abort,
    Writeback,
    Checkpoint,
    /// The next `n` flushes the device receives fail.
    FlushFails(usize),
    HomeWriteFails,
    HomeInFlight,
    InFlightCompletes,
    /// The tail write fails, the abort's rewrite succeeds.
    SuperblockFails,
    /// Both superblock writes fail: the medium holds the old tail or the new.
    SuperblockFailsTwice,
}

// ---------------------------------------------------------------------------
// The Owner's root-only facts, and fixture stores
// ---------------------------------------------------------------------------

impl super::maintenance::OwnerFacts for SimIo {
    fn superblock_features(&self) -> BTreeSet<String> {
        self.world.lock().host.features.clone()
    }

    fn journal_location(&self) -> (u64, bool) {
        let state = self.world.lock();
        (state.host.journal_inode, state.host.external_journal)
    }

    fn controller_reports_volatile_cache(&self) -> bool {
        self.world.lock().host.identify_volatile_cache
    }

    fn virtual_machine_guest(&self) -> bool {
        self.world.lock().host.guest
    }
}

/// The store uid and gid of the fixtures.
pub const STORE_UID: u32 = 1001;
pub const STORE_GID: u32 = 1001;
/// The fixture's root id.
pub const ROOT_ID: [u8; 16] = [
    0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1a, 0x1b, 0x1c, 0x1d, 0x1e, 0x1f,
];
/// A successor's root id.
pub const SUCCESSOR_ID: [u8; 16] = [
    0x20, 0x21, 0x22, 0x23, 0x24, 0x25, 0x26, 0x27, 0x28, 0x29, 0x2a, 0x2b, 0x2c, 0x2d, 0x2e, 0x2f,
];

/// The proposed forms of design section 6.2: the `PROVISION` path and the
/// state roots' parent (Owner choices; fixtures here).
pub const PROVISION_PATH: &str = "/var/lib/nexus-os/phase2-custody/provision/uid-1001.provision";
pub const ROOTS: [&str; 5] = ["var", "lib", "nexus-os", "phase2-custody", "roots"];

/// A fixture host and, optionally, a store provisioned on it by the real
/// P-PROV procedure after the fixture qualification. Every fact is a
/// fixture; nothing here qualifies a host or a device.
pub struct Fixture {
    pub world: SimWorld,
    /// The Owner, as root.
    pub root: SimIo,
    pub path: super::open::ProvisionPath,
    pub layout: super::maintenance::Layout,
    pub qualification: Option<super::maintenance::Qualification>,
}

impl Fixture {
    /// The host, with the ancestors of the `PROVISION` directory and of the
    /// state roots durable and root-owned, and nothing provisioned.
    pub fn host(host: HostFixture) -> Fixture {
        Self::host_sized(host, 2, 16)
    }

    pub fn host_sized(host: HostFixture, pool: u32, c_pool: u32) -> Fixture {
        let world = SimWorld::new(host);
        let root_ino = world.root();
        world.fixture_device(root_ino, makedev(259, 2), 22);
        let var = world.fixture_entry(root_ino, "var", FileType::Directory, (0, 0, 0o755));
        world.fixture_device(var, makedev(259, 2), 22);
        let lib = world.fixture_entry(var, "lib", FileType::Directory, (0, 0, 0o755));
        world.fixture_device(lib, FIXTURE_DEV, FIXTURE_MOUNT_ID);
        let nexus = world.fixture_entry(lib, "nexus-os", FileType::Directory, (0, 0, 0o755));
        let custody =
            world.fixture_entry(nexus, "phase2-custody", FileType::Directory, (0, 0, 0o755));
        world.fixture_entry(custody, "provision", FileType::Directory, (0, 0, 0o755));
        world.fixture_entry(custody, "roots", FileType::Directory, (0, 0, 0o755));
        let root = world.process("owner-root", 0, 0);
        let path = match super::open::ProvisionPath::parse(PROVISION_PATH) {
            Ok(path) => path,
            Err(error) => panic!("fixture: {error:?}"),
        };
        let layout = super::maintenance::Layout {
            provision: path.clone(),
            parent: ROOTS.iter().map(|part| part.to_string()).collect(),
            state_name: format!("uid-{STORE_UID}-{}", super::format::hex(&ROOT_ID)),
            root_id: ROOT_ID,
            uid: STORE_UID,
            gid: STORE_GID,
            pool,
            c_pool,
            attestation: "fixture: stable completion and 4096-byte containment stated by the Owner"
                .into(),
            operator: "owner".into(),
            created: "2026-10-01T00:00:00Z".into(),
        };
        Fixture {
            world,
            root,
            path,
            layout,
            qualification: None,
        }
    }

    /// The fixture qualification as root (design section 13.2 step 1).
    pub fn qualify(&mut self) -> Result<super::maintenance::Qualification, Vec<String>> {
        let roots: Vec<String> = ROOTS.iter().map(|part| part.to_string()).collect();
        let dir = super::maintenance::dir_ref(&self.root, &roots).map_err(|why| vec![why])?;
        let qualification = super::maintenance::qualify(&self.root, &dir.dir)?;
        self.qualification = Some(qualification.clone());
        Ok(qualification)
    }

    /// The qualified host with a store of `pool` pool files of `c_pool`
    /// blocks provisioned by P-PROV, every step run.
    pub fn provisioned(pool: u32, c_pool: u32) -> Fixture {
        Self::provisioned_on(HostFixture::qualified(), pool, c_pool)
    }

    pub fn provisioned_on(host: HostFixture, pool: u32, c_pool: u32) -> Fixture {
        let mut fixture = Self::host_sized(host, pool, c_pool);
        let qualification = match fixture.qualify() {
            Ok(qualification) => qualification,
            Err(problems) => panic!("fixture: qualification failed: {problems:?}"),
        };
        let (mut procedure, _) =
            super::maintenance::provision(&fixture.root, &fixture.layout, &qualification);
        if let Err(why) = procedure.run_all(&mut |_| {}) {
            panic!("fixture: provisioning failed: {why}");
        }
        fixture
    }

    /// A store-uid process.
    pub fn store_process(&self, name: &str) -> SimIo {
        self.world.process(name, STORE_UID, STORE_GID)
    }

    /// A root process (an Owner's maintenance session).
    pub fn root_process(&self, name: &str) -> SimIo {
        self.world.process(name, 0, 0)
    }

    /// The state root's path components.
    pub fn state_root(&self) -> Vec<String> {
        let mut path = self.layout.parent.clone();
        path.push(self.layout.state_name.clone());
        path
    }

    /// The inode of a store path below the state root.
    pub fn store_ino(&self, names: &[&str]) -> Option<Ino> {
        let mut path = format!("/{}", self.state_root().join("/"));
        for name in names {
            path.push('/');
            path.push_str(name);
        }
        self.world.lookup(&path)
    }

    /// The inode of pool file `index`.
    pub fn pool_ino(&self, index: u32) -> Option<Ino> {
        self.store_ino(&["journals", &super::format::pool_name(index)])
    }
}
