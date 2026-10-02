//! Maintenance (design section 13): the maintenance session and its
//! in-session verification (sections 13.1 and 13.11), the Owner's fixture
//! qualification (section 13.2 step 1), and every retained procedure as an
//! explicit list of protocol steps, each labelled with its section and step,
//! so that a test can stop after any one of them (a crash point) and check
//! the steps against the operation table of section 15.3.
//!
//! This is a specified future interface exercised against simulated storage
//! only: there is no executable, binary target, privileged entry point or
//! provisioning tool here, and nothing here performs real maintenance. A
//! session is one process holding the store lock (`LOCK_EX`) for its whole
//! lifetime: it never unlocks, converts or duplicates it, and verifies
//! through that retained description, never through a new lock request.

use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

use super::super::{Config, DispositionReason};
use super::classify::{FileClass, Incident};
use super::format::{
    self, encode_header, hex, pool_file_size, pool_name, render_disposition, render_provision,
    sha256, ArchiveClass, ArchiveName, Disposition, IncidentClass, IncidentFacts, IncidentKind,
    Provision, BLOCK, BLOCK_U64,
};
use super::io::{sync_with_retries, write_fully, Errno, FileType, IoError, LockRequest, StoreIo};
use super::open::{
    activate, check_mount, check_storage, open_root, open_store_dirs, revalidate, scan, select,
    Activation, NoHooks, OpenedRoot, OpeningHooks, OpeningId, Pinning, Platform, ProvisionPath,
    Refusal, Refused, ScanMode, ScanResult, Selection, StorageAdmission,
};

// ---------------------------------------------------------------------------
// The Owner's qualification (design section 13.2 step 1), as a fixture
// ---------------------------------------------------------------------------

/// What only root can read: superblock facts, the journal's location, the
/// controller's own report and whether the host is a virtual machine guest.
/// The store uid's openings never read these.
pub trait OwnerFacts {
    fn superblock_features(&self) -> BTreeSet<String>;
    /// The journal's inode and whether it is on another device.
    fn journal_location(&self) -> (u64, bool);
    /// The controller's Identify data report a volatile write cache.
    fn controller_reports_volatile_cache(&self) -> bool;
    fn virtual_machine_guest(&self) -> bool;
}

/// What a qualification pins in `PROVISION`. Recording these qualifies
/// nothing by itself: stable completion (A-S1) stays an assumption.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Qualification {
    pub mount_options: String,
    pub super_options: String,
    pub kernel: String,
    pub storage_pci_function: String,
    pub storage_partition: Option<u32>,
    pub storage_identity: [u8; 32],
}

/// The fixture qualification: the root-only facts, then every rule of design
/// section 6.4 steps 1 to 12 as the store uid will read them, with nothing
/// pinned yet. `dir` is a directory on the store's filesystem. Returns the
/// problems; provisioning proceeds only with none.
pub fn qualify<P: Platform + OwnerFacts>(
    io: &P,
    dir: &P::Dir,
) -> Result<Qualification, Vec<String>> {
    let mut problems = Vec::new();
    let features = io.superblock_features();
    if !features.contains("has_journal") {
        problems.push("no journal".to_string());
    }
    if features.contains("fast_commit") {
        problems.push("fast_commit feature".to_string());
    }
    if io.journal_location() != (super::open::JOURNAL_INODE, false) {
        problems.push("journal not internal at inode 8".to_string());
    }
    if io.virtual_machine_guest() {
        problems.push("virtual machine guest".to_string());
    }
    if io.controller_reports_volatile_cache() {
        problems.push("the controller reports a volatile write cache".to_string());
    }
    let placeholder = placeholder_provision();
    let mut record = None;
    match check_mount(io, &placeholder, dir, &[], Pinning::Requalification) {
        Err(refused) => problems.push(format!("{refused:?}")),
        Ok((device, _)) => {
            match check_storage(io, &placeholder, device, Pinning::Requalification) {
                Err(refused) => problems.push(format!("{refused:?}")),
                Ok(observation) => record = Some(observation),
            }
        }
    }
    if !problems.is_empty() {
        return Err(problems);
    }
    let Some(observation) = record else {
        return Err(vec!["storage not observed".into()]);
    };
    let mount_id = io.mount_id(dir).map_err(|error| vec![error.to_string()])?;
    let table = io
        .read_mountinfo(io.process_id(), super::open::MOUNTINFO_LIMIT)
        .map_err(|error| vec![error.to_string()])?;
    let records = super::open::parse_mountinfo(&table).map_err(|why| vec![why])?;
    let line = records
        .iter()
        .find(|record| record.mount_id == mount_id)
        .ok_or_else(|| vec!["mount not found".to_string()])?;
    let (release, version) = io
        .kernel_identity()
        .map_err(|error| vec![error.to_string()])?;
    Ok(Qualification {
        mount_options: line.mount_options.clone(),
        super_options: line.super_options.clone(),
        kernel: format!("{release} {version}"),
        storage_pci_function: observation.pci_function,
        storage_partition: observation.partition,
        storage_identity: observation.identity,
    })
}

/// A provision with nothing pinned, for the rules-only checks of a
/// qualification (its fields are never compared: the pinning is off).
fn placeholder_provision() -> Provision {
    Provision {
        uid: 0,
        gid: 0,
        root_id: [0; 16],
        state_root: "/x".into(),
        root_inode: 0,
        lock_inode: 0,
        journals_inode: 0,
        dispositions_inode: 0,
        revoked_inode: 0,
        archive_inode: 0,
        mount_options: "x".into(),
        super_options: "x".into(),
        device_logical_block_size: 512,
        device_physical_block_size: 4096,
        kernel: "x".into(),
        storage_pci_function: "0000:00:00.0".into(),
        storage_partition: None,
        storage_identity: [0; 32],
        storage_attestation: "x".into(),
        c_pool: 1,
        pool_inodes: vec![0],
        retired_through: 0,
        predecessor: None,
        operator: "x".into(),
        created: "1970-01-01T00:00:00Z".into(),
        revision: 1,
    }
}

// ---------------------------------------------------------------------------
// Procedures as protocol steps
// ---------------------------------------------------------------------------

/// One protocol step: its section/step label and its action.
pub struct ProcStep<'a> {
    pub label: &'static str,
    action: Box<dyn FnMut() -> Result<(), String> + 'a>,
    /// The mutation count of the session the step acts in, if any.
    session: Option<Rc<Cell<u64>>>,
}

/// A procedure: its steps, in order. Running stops at the first failure,
/// which is never success; a test may stop after any step instead (a crash
/// point).
pub struct Procedure<'a> {
    pub name: &'static str,
    steps: Vec<ProcStep<'a>>,
    done: usize,
    session: Option<Rc<Cell<u64>>>,
}

impl<'a> Procedure<'a> {
    fn new(name: &'static str) -> Self {
        Self {
            name,
            steps: Vec::new(),
            done: 0,
            session: None,
        }
    }

    fn add(&mut self, label: &'static str, action: impl FnMut() -> Result<(), String> + 'a) {
        self.steps.push(ProcStep {
            label,
            action: Box::new(action),
            session: self.session.clone(),
        });
    }

    /// Every step, present and later added, acts inside the session whose
    /// mutation count this is: starting any of them lapses that session's
    /// latest verification ([`Session::take_report`]).
    fn within(&mut self, mutations: Rc<Cell<u64>>) {
        for step in &mut self.steps {
            step.session = Some(Rc::clone(&mutations));
        }
        self.session = Some(mutations);
    }

    fn extend(&mut self, other: Procedure<'a>) {
        self.steps.extend(other.steps);
    }

    pub fn len(&self) -> usize {
        self.steps.len()
    }

    pub fn is_empty(&self) -> bool {
        self.steps.is_empty()
    }

    pub fn labels(&self) -> Vec<&'static str> {
        self.steps.iter().map(|step| step.label).collect()
    }

    /// Run the next `count` steps; `on_step` sees each label first.
    pub fn run(
        &mut self,
        count: usize,
        on_step: &mut dyn FnMut(Option<&'static str>),
    ) -> Result<(), String> {
        let end = (self.done + count).min(self.steps.len());
        while self.done < end {
            let step = &mut self.steps[self.done];
            on_step(Some(step.label));
            // A step that starts may change the store even if it fails.
            if let Some(mutations) = &step.session {
                mutations.set(mutations.get() + 1);
            }
            let result = (step.action)();
            on_step(None);
            result.map_err(|why| format!("{} (step {}): {why}", step.label, self.done + 1))?;
            self.done += 1;
        }
        Ok(())
    }

    pub fn run_all(&mut self, on_step: &mut dyn FnMut(Option<&'static str>)) -> Result<(), String> {
        let rest = self.steps.len() - self.done;
        self.run(rest, on_step)
    }

    pub fn steps_done(&self) -> usize {
        self.done
    }
}

fn err(error: IoError) -> String {
    error.to_string()
}

/// One directory, reached by a fresh component walk from `/` (each
/// component root-owned and closed to others), with its parent: what a
/// procedure needs to sync it.
pub struct DirRef<P: StoreIo> {
    pub parent: P::Dir,
    pub name: String,
    pub dir: P::Dir,
}

/// Walk to `components` (the last one is the directory itself).
pub fn dir_ref<P: Platform>(io: &P, components: &[String]) -> Result<DirRef<P>, String> {
    let (last, base) = components
        .split_last()
        .ok_or_else(|| "no directory".to_string())?;
    let mut parent = io.root_dir().map_err(err)?;
    for component in base {
        let stat = io.stat_at(&parent, component).map_err(err)?;
        if stat.file_type != FileType::Directory || stat.uid != 0 || stat.mode & 0o022 != 0 {
            return Err(format!("{component} is not a trusted directory"));
        }
        parent = io.open_dir(&parent, component).map_err(err)?;
    }
    let dir = io.open_dir(&parent, last).map_err(err)?;
    Ok(DirRef {
        parent,
        name: last.clone(),
        dir,
    })
}

/// `fsync` a directory through a new `O_RDONLY | O_DIRECTORY` description.
fn sync_dir<P: Platform>(io: &P, dir: &DirRef<P>) -> Result<(), String> {
    let handle = io.open_dir_for_sync(&dir.parent, &dir.name).map_err(err)?;
    sync_with_retries(io, &handle, false).map_err(err)
}

/// Create a file exclusively, owned as given, write all of `data`, and
/// `fsync` it through the creating description (create, write and fsync are
/// three protocol steps; the caller runs them in turn).
struct NewFile<P: StoreIo> {
    file: Option<P::File>,
}

fn create_owned<P: Platform>(
    io: &P,
    dir: &P::Dir,
    name: &str,
    owner: (u32, u32, u32),
    allocate: Option<u64>,
) -> Result<P::File, String> {
    let file = io.create_exclusive(dir, name, owner.2).map_err(err)?;
    if (owner.0, owner.1) != (0, 0) {
        io.set_owner(&file, owner.0, owner.1).map_err(err)?;
    }
    if let Some(len) = allocate {
        io.allocate(&file, len).map_err(err)?;
    }
    Ok(file)
}

fn write_all<P: Platform>(io: &P, file: &P::File, data: &[u8]) -> Result<(), String> {
    write_fully(io, file, 0, data).map_err(|failure| format!("{failure:?}"))
}

fn sync_file<P: Platform>(io: &P, file: &P::File) -> Result<(), String> {
    sync_with_retries(io, file, false).map_err(err)
}

// ---------------------------------------------------------------------------
// The store's layout, and initial provisioning (design section 13.2)
// ---------------------------------------------------------------------------

/// What a new store's provisioning creates.
#[derive(Debug, Clone)]
pub struct Layout {
    pub provision: ProvisionPath,
    /// `<STATE_ROOT>`'s parent's components and its own name.
    pub parent: Vec<String>,
    pub state_name: String,
    pub root_id: [u8; 16],
    pub uid: u32,
    pub gid: u32,
    pub pool: u32,
    pub c_pool: u32,
    pub attestation: String,
    pub operator: String,
    pub created: String,
}

impl Layout {
    pub fn state_root(&self) -> String {
        let mut all = self.parent.clone();
        all.push(self.state_name.clone());
        format!("/{}", all.join("/"))
    }
}

/// The inodes a provisioning made, as `PROVISION` records them.
#[derive(Debug, Clone, Default)]
pub struct Made {
    pub root: u64,
    pub lock: u64,
    pub journals: u64,
    pub dispositions: u64,
    pub revoked: u64,
    pub archive: u64,
    pub pool: Vec<u64>,
}

/// The `PROVISION` a layout, its inodes and a qualification give.
pub fn provision_for(
    layout: &Layout,
    made: &Made,
    qualification: &Qualification,
    predecessor: Option<([u8; 16], String)>,
    revision: u64,
) -> Provision {
    Provision {
        uid: layout.uid,
        gid: layout.gid,
        root_id: layout.root_id,
        state_root: layout.state_root(),
        root_inode: made.root,
        lock_inode: made.lock,
        journals_inode: made.journals,
        dispositions_inode: made.dispositions,
        revoked_inode: made.revoked,
        archive_inode: made.archive,
        mount_options: qualification.mount_options.clone(),
        super_options: qualification.super_options.clone(),
        device_logical_block_size: 512,
        device_physical_block_size: 4096,
        kernel: qualification.kernel.clone(),
        storage_pci_function: qualification.storage_pci_function.clone(),
        storage_partition: qualification.storage_partition,
        storage_identity: qualification.storage_identity,
        storage_attestation: layout.attestation.clone(),
        c_pool: layout.c_pool,
        pool_inodes: made.pool.clone(),
        retired_through: 0,
        predecessor,
        operator: layout.operator.clone(),
        created: layout.created.clone(),
        revision,
    }
}

/// Design section 13.2 steps 2 to 4 (the three labels): the directories,
/// `LOCK` and the pool, every step synced. The inodes go into `made`.
fn store_steps<P: Platform + Clone + 'static>(
    io: Rc<P>,
    layout: Rc<Layout>,
    made: Rc<RefCell<Made>>,
    labels: (&'static str, &'static str, &'static str),
) -> Procedure<'static> {
    let mut proc = Procedure::new("store");
    let parent_path = layout.parent.clone();
    let mut root_path = parent_path.clone();
    root_path.push(layout.state_name.clone());
    let sub = |name: &str| {
        let mut path = root_path.clone();
        path.push(name.to_string());
        path
    };
    let journals_path = sub("journals");
    let dispositions_path = sub("dispositions");
    let archive_path = sub("archive");
    let mut revoked_path = dispositions_path.clone();
    revoked_path.push("revoked".into());
    fn ino_of<P: Platform>(io: &P, path: &[String]) -> Result<u64, String> {
        let dir = dir_ref(io, path)?;
        io.stat_dir(&dir.dir).map(|stat| stat.ino).map_err(err)
    }
    {
        let (io, layout, made) = (Rc::clone(&io), Rc::clone(&layout), Rc::clone(&made));
        let (parent_path, root_path) = (parent_path.clone(), root_path.clone());
        proc.add(labels.0, move || {
            let parent = dir_ref(&*io, &parent_path)?;
            io.make_dir(&parent.dir, &layout.state_name, 0o755)
                .map_err(err)?;
            made.borrow_mut().root = ino_of(&*io, &root_path)?;
            Ok(())
        });
    }
    for (name, path) in [
        ("journals", journals_path.clone()),
        ("dispositions", dispositions_path.clone()),
        ("archive", archive_path.clone()),
    ] {
        let (io, made, root_path) = (Rc::clone(&io), Rc::clone(&made), root_path.clone());
        proc.add(labels.0, move || {
            let root = dir_ref(&*io, &root_path)?;
            io.make_dir(&root.dir, name, 0o755).map_err(err)?;
            let ino = ino_of(&*io, &path)?;
            let mut made = made.borrow_mut();
            match name {
                "journals" => made.journals = ino,
                "dispositions" => made.dispositions = ino,
                _ => made.archive = ino,
            }
            Ok(())
        });
    }
    {
        let (io, made) = (Rc::clone(&io), Rc::clone(&made));
        let (dispositions_path, revoked_path) = (dispositions_path.clone(), revoked_path.clone());
        proc.add(labels.0, move || {
            let dispositions = dir_ref(&*io, &dispositions_path)?;
            io.make_dir(&dispositions.dir, "revoked", 0o755)
                .map_err(err)?;
            made.borrow_mut().revoked = ino_of(&*io, &revoked_path)?;
            Ok(())
        });
    }
    for path in [
        revoked_path,
        dispositions_path,
        journals_path.clone(),
        archive_path,
        root_path.clone(),
        parent_path,
    ] {
        let io = Rc::clone(&io);
        proc.add(labels.0, move || sync_dir(&*io, &dir_ref(&*io, &path)?));
    }
    let lock_file: Rc<RefCell<Option<P::File>>> = Rc::new(RefCell::new(None));
    {
        let (io, layout, made, lock_file) = (
            Rc::clone(&io),
            Rc::clone(&layout),
            Rc::clone(&made),
            Rc::clone(&lock_file),
        );
        let root_path = root_path.clone();
        proc.add(labels.1, move || {
            let root = dir_ref(&*io, &root_path)?;
            let file = create_owned(
                &*io,
                &root.dir,
                "LOCK",
                (layout.uid, layout.gid, 0o600),
                None,
            )?;
            made.borrow_mut().lock = io.stat_file(&file).map_err(err)?.ino;
            *lock_file.borrow_mut() = Some(file);
            Ok(())
        });
    }
    {
        let (io, lock_file) = (Rc::clone(&io), Rc::clone(&lock_file));
        proc.add(labels.1, move || {
            let file = lock_file.borrow_mut().take().ok_or("LOCK not created")?;
            sync_file(&*io, &file)
        });
    }
    {
        let io = Rc::clone(&io);
        proc.add(labels.1, move || {
            sync_dir(&*io, &dir_ref(&*io, &root_path)?)
        });
    }
    let size = pool_file_size(layout.c_pool).unwrap_or(0);
    for index in 0..layout.pool {
        let pool_file: Rc<RefCell<Option<P::File>>> = Rc::new(RefCell::new(None));
        {
            let (io, layout, made, pool_file) = (
                Rc::clone(&io),
                Rc::clone(&layout),
                Rc::clone(&made),
                Rc::clone(&pool_file),
            );
            let journals_path = journals_path.clone();
            proc.add(labels.2, move || {
                let journals = dir_ref(&*io, &journals_path)?;
                let file = create_owned(
                    &*io,
                    &journals.dir,
                    &pool_name(index),
                    (layout.uid, layout.gid, 0o600),
                    Some(size),
                )?;
                made.borrow_mut()
                    .pool
                    .push(io.stat_file(&file).map_err(err)?.ino);
                *pool_file.borrow_mut() = Some(file);
                Ok(())
            });
        }
        {
            let (io, pool_file) = (Rc::clone(&io), Rc::clone(&pool_file));
            proc.add(labels.2, move || {
                let guard = pool_file.borrow();
                let file = guard.as_ref().ok_or("pool file not created")?;
                write_all(&*io, file, &vec![0u8; size as usize])
            });
        }
        {
            let (io, pool_file) = (Rc::clone(&io), Rc::clone(&pool_file));
            proc.add(labels.2, move || {
                let file = pool_file
                    .borrow_mut()
                    .take()
                    .ok_or("pool file not created")?;
                sync_file(&*io, &file)
            });
        }
    }
    proc.add(labels.2, move || {
        sync_dir(&*io, &dir_ref(&*io, &journals_path)?)
    });
    proc
}

/// Write `PROVISION` to its temporary name, sync it, rename it into place
/// and sync the directory (design sections 13.2 step 5, 13.3 and 13.6 2c):
/// five steps. `make` produces the content when the second step runs;
/// `before` runs first in the first step.
type Make = Rc<dyn Fn() -> Result<Vec<u8>, String>>;

fn publish_provision_steps<P: Platform + Clone + 'static>(
    io: Rc<P>,
    path: Rc<ProvisionPath>,
    make: Make,
    labels: (&'static str, &'static str),
    before: Option<Box<dyn FnMut() -> Result<(), String>>>,
) -> Procedure<'static> {
    let mut proc = Procedure::new("publish PROVISION");
    let file: Rc<RefCell<Option<P::File>>> = Rc::new(RefCell::new(None));
    let mut before = before;
    {
        let (io, path, file) = (Rc::clone(&io), Rc::clone(&path), Rc::clone(&file));
        proc.add(labels.0, move || {
            if let Some(before) = before.as_mut() {
                before()?;
            }
            let provdir = dir_ref(&*io, &path.directory)?;
            *file.borrow_mut() = Some(create_owned(
                &*io,
                &provdir.dir,
                &path.tmp_name(),
                (0, 0, 0o444),
                None,
            )?);
            Ok(())
        });
    }
    {
        let (io, file) = (Rc::clone(&io), Rc::clone(&file));
        proc.add(labels.0, move || {
            let guard = file.borrow();
            let handle = guard.as_ref().ok_or("PROVISION.tmp not created")?;
            write_all(&*io, handle, &make()?)
        });
    }
    {
        let (io, file) = (Rc::clone(&io), Rc::clone(&file));
        proc.add(labels.0, move || {
            let handle = file
                .borrow_mut()
                .take()
                .ok_or("PROVISION.tmp not created")?;
            sync_file(&*io, &handle)
        });
    }
    {
        let (io, path) = (Rc::clone(&io), Rc::clone(&path));
        proc.add(labels.1, move || {
            let provdir = dir_ref(&*io, &path.directory)?;
            io.rename(&provdir.dir, &path.tmp_name(), &provdir.dir, &path.name)
                .map_err(err)
        });
    }
    proc.add(labels.1, move || {
        sync_dir(&*io, &dir_ref(&*io, &path.directory)?)
    });
    proc
}

/// P-PROV (design section 13.2 steps 2 to 5): no session (nothing can select
/// the store before its `PROVISION` exists), root only, `PROVISION` last.
pub fn provision<P: Platform + Clone + 'static>(
    io: &P,
    layout: &Layout,
    qualification: &Qualification,
) -> (Procedure<'static>, Rc<RefCell<Made>>) {
    let io = Rc::new(io.clone());
    let layout = Rc::new(layout.clone());
    let qualification = qualification.clone();
    let made = Rc::new(RefCell::new(Made::default()));
    let mut proc = Procedure::new("P-PROV");
    proc.extend(store_steps(
        Rc::clone(&io),
        Rc::clone(&layout),
        Rc::clone(&made),
        ("13.2/2", "13.2/3", "13.2/4"),
    ));
    let (made_for_content, layout_for_content) = (Rc::clone(&made), Rc::clone(&layout));
    let make: Make = Rc::new(move || {
        let provision = provision_for(
            &layout_for_content,
            &made_for_content.borrow(),
            &qualification,
            None,
            1,
        );
        render_provision(&provision).map_err(|error| error.0.to_string())
    });
    proc.extend(publish_provision_steps(
        io,
        Rc::new(layout.provision.clone()),
        make,
        ("13.2/5", "13.2/5"),
        None,
    ));
    (proc, made)
}

/// After an Unprovisioned refusal (design section 13.2, recovery): every
/// state root under the parent whose pool holds a non-zero byte. Such a
/// root may hold history: it is re-published, never replaced.
pub fn roots_with_history<P: Platform>(io: &P, parent: &[String]) -> Result<Vec<String>, String> {
    let parent = dir_ref(io, parent)?;
    let names = match io.list_dir(&parent.dir, 4096).map_err(err)? {
        super::io::Listing::Names(names) => names,
        super::io::Listing::TooMany => return Err("too many state roots".into()),
    };
    let mut found = Vec::new();
    for name in names {
        let Ok(root) = io.open_dir(&parent.dir, &name) else {
            continue;
        };
        let Ok(journals) = io.open_dir(&root, "journals") else {
            continue;
        };
        let pool = match io.list_dir(&journals, 4096).map_err(err)? {
            super::io::Listing::Names(pool) => pool,
            super::io::Listing::TooMany => return Err("too many pool files".into()),
        };
        let mut history = false;
        for file_name in pool {
            let Ok(file) = io.open_read(&journals, &file_name) else {
                continue;
            };
            let mut buf = vec![0u8; BLOCK];
            let mut offset = 0u64;
            loop {
                let got = io.pread(&file, offset, &mut buf).map_err(err)?;
                if got == 0 {
                    break;
                }
                if buf[..got].iter().any(|byte| *byte != 0) {
                    history = true;
                    break;
                }
                offset += got as u64;
            }
            if history {
                break;
            }
        }
        if history {
            found.push(name);
        }
    }
    Ok(found)
}

/// R-REPUBLISH (design section 13.2, recovery): `PROVISION` for an existing
/// root, with its recorded root id and the inodes it actually has; a
/// leftover temporary is removed first.
pub fn republish<P: Platform + Clone + 'static>(
    io: &P,
    layout: &Layout,
    qualification: &Qualification,
) -> Result<Procedure<'static>, String> {
    let mut root_path = layout.parent.clone();
    root_path.push(layout.state_name.clone());
    let root = dir_ref(io, &root_path)?;
    let ino = |dir: &P::Dir, name: &str| io.stat_at(dir, name).map(|stat| stat.ino).map_err(err);
    let journals = io.open_dir(&root.dir, "journals").map_err(err)?;
    let dispositions = io.open_dir(&root.dir, "dispositions").map_err(err)?;
    let made = Made {
        root: io.stat_dir(&root.dir).map_err(err)?.ino,
        lock: ino(&root.dir, "LOCK")?,
        journals: ino(&root.dir, "journals")?,
        dispositions: ino(&root.dir, "dispositions")?,
        revoked: ino(&dispositions, "revoked")?,
        archive: ino(&root.dir, "archive")?,
        pool: (0..layout.pool)
            .map(|index| ino(&journals, &pool_name(index)))
            .collect::<Result<_, _>>()?,
    };
    let rc_io = Rc::new(io.clone());
    let path = Rc::new(layout.provision.clone());
    let mut proc = Procedure::new("R-REPUBLISH");
    let provdir = dir_ref(io, &layout.provision.directory)?;
    if io
        .stat_at(&provdir.dir, &layout.provision.tmp_name())
        .is_ok()
    {
        let (io, path) = (Rc::clone(&rc_io), Rc::clone(&path));
        proc.add("13.2/recover", move || {
            let provdir = dir_ref(&*io, &path.directory)?;
            io.unlink(&provdir.dir, &path.tmp_name()).map_err(err)
        });
    }
    let (layout, qualification) = (layout.clone(), qualification.clone());
    let make: Make = Rc::new(move || {
        render_provision(&provision_for(&layout, &made, &qualification, None, 1))
            .map_err(|error| error.0.to_string())
    });
    proc.extend(publish_provision_steps(
        rc_io,
        path,
        make,
        ("13.2/recover", "13.2/recover"),
        None,
    ));
    Ok(proc)
}

// ---------------------------------------------------------------------------
// The maintenance session (design sections 13.1 and 13.11)
// ---------------------------------------------------------------------------

/// What a session did, in order (for the tests of lock lifetimes).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionEvent {
    Begin(String),
    Verify(Result<(), Refused>),
    Reselect(u64),
    Adopt(String),
    HoldJournal(String, u32),
    End,
}

/// One maintenance session: one root process, the store lock of each state
/// root it holds (`LOCK_EX`, retained for its whole life), the pool-file
/// locks it took, and its revalidated, activated selection.
///
/// Retained, ownership-bearing state (P2-V1-R3B-I3-I1-R1): every field is
/// private. The session also retains its own latest verification. The
/// procedures that decide from a verification (archival, recycling,
/// retirement, the recycling's resumption) consume that one, never a
/// report a caller supplies. One verification authorizes at most one such
/// procedure. It also lapses as soon as any step of any procedure of this
/// session starts, so a decision never rests on a verification older than
/// the session's last procedure step.
pub struct Session<P: Platform> {
    io: P,
    path: ProvisionPath,
    selection: Selection,
    requalify: bool,
    locks: BTreeMap<String, P::File>,
    journal_locks: BTreeMap<(String, u32), P::File>,
    opening: OpeningId,
    admission: Option<StorageAdmission>,
    active: bool,
    /// Steps started by this session's procedures.
    mutations: Rc<Cell<u64>>,
    /// The latest verification, with the mutation count it saw.
    verified: Option<(u64, Result<SessionReport, Refusal>)>,
    events: Vec<SessionEvent>,
}

/// An in-session verification's report (design section 13.11).
#[derive(Debug, Clone)]
pub struct SessionReport {
    pub revision: u64,
    pub scan: ScanResult,
    pub current: Vec<([u8; 32], Incident)>,
    pub blocking: Vec<[u8; 32]>,
    pub dispositioned: Vec<[u8; 32]>,
}

impl<P: Platform> Session<P> {
    /// Begin (design section 13.1 rule 1): select, open safely, take
    /// `LOCK_EX | LOCK_NB` on a retained description (Busy aborts),
    /// revalidate, and activate, so that every decision reads a durable
    /// namespace. Re-qualification mode skips only the pinned comparisons
    /// being replaced, never a rule.
    pub fn begin(
        io: P,
        path: &ProvisionPath,
        requalify: bool,
        hooks: &mut dyn OpeningHooks,
    ) -> Result<Session<P>, Refusal> {
        let opening = OpeningId::fresh();
        let pinning = if requalify {
            Pinning::Requalification
        } else {
            Pinning::Pinned
        };
        let mut revision = None;
        for attempt in 1..=format::SELECTION_ATTEMPTS {
            let selection = select(&io, path).map_err(|refused| Refusal { refused, revision })?;
            revision = Some(selection.revision());
            let fail = |refused: Refused| Refusal { refused, revision };
            let OpenedRoot {
                dirs,
                admission: observed,
                ..
            } = open_root(&io, path, &selection, opening, pinning).map_err(fail)?;
            let lock =
                super::open::open_lock(&io, &dirs.root, &selection.provision).map_err(fail)?;
            hooks.before_lock(attempt);
            if io.flock(&lock, LockRequest::Exclusive).is_err() {
                return Err(fail(Refused::Busy));
            }
            if !revalidate(&io, path, &selection, &lock) {
                continue;
            }
            let admission = activate(
                &io,
                &Activation {
                    path,
                    selection: &selection,
                    dirs: &dirs,
                    lock: &lock,
                    opening,
                    observed: &observed,
                    pinning,
                },
                hooks,
            )
            .map_err(fail)?;
            let state = selection.state_name.clone();
            let mut locks = BTreeMap::new();
            locks.insert(state.clone(), lock);
            return Ok(Session {
                io,
                path: path.clone(),
                selection,
                requalify,
                locks,
                journal_locks: BTreeMap::new(),
                opening,
                admission: Some(admission),
                active: true,
                mutations: Rc::new(Cell::new(0)),
                verified: None,
                events: vec![SessionEvent::Begin(state)],
            });
        }
        Err(Refusal {
            refused: Refused::SelectionChanged("after 3 attempts".into()),
            revision,
        })
    }

    pub fn io(&self) -> &P {
        &self.io
    }

    pub fn selection(&self) -> &Selection {
        &self.selection
    }

    pub fn path(&self) -> &ProvisionPath {
        &self.path
    }

    /// What the session did, in order (read-only).
    pub fn events(&self) -> &[SessionEvent] {
        &self.events
    }

    pub(super) fn admission(&self) -> Option<&StorageAdmission> {
        self.admission.as_ref()
    }

    /// The session's authority over `state`: its retained `LOCK_EX`
    /// description, on the inode the root's `LOCK` entry names now (a fresh
    /// lookup). Never a flag, a path, a PID or a descriptor number.
    pub fn authorized(&self, state: &str) -> bool {
        if !self.active {
            return false;
        }
        let Some(lock) = self.locks.get(state) else {
            return false;
        };
        let mut root_path = self.selection.parent.clone();
        root_path.push(state.to_string());
        let Ok(root) = dir_ref(&self.io, &root_path) else {
            return false;
        };
        let (Ok(entry), Ok(held)) = (self.io.stat_at(&root.dir, "LOCK"), self.io.stat_file(lock))
        else {
            return false;
        };
        entry.same_inode(&held)
    }

    /// In-session verification (design section 13.11): the standalone
    /// verifier's checks under the session's own exclusion, with no lock
    /// request on the store lock, scanning afresh every time.
    pub fn verify(&mut self, config: &Config) -> Result<SessionReport, Refusal> {
        let revision = Some(self.selection.revision());
        let fail = |refused: Refused| Refusal { refused, revision };
        let state = self.selection.state_name.clone();
        if !self.authorized(&state) {
            self.events
                .push(SessionEvent::Verify(Err(Refused::NotAuthorized)));
            self.verified = Some((self.mutations.get(), Err(fail(Refused::NotAuthorized))));
            return Err(fail(Refused::NotAuthorized));
        }
        let outcome = self.verify_inner(config, &state);
        self.events.push(SessionEvent::Verify(
            outcome
                .as_ref()
                .map(|_| ())
                .map_err(|refusal| refusal.refused.clone()),
        ));
        self.verified = Some((self.mutations.get(), outcome.clone()));
        outcome
    }

    /// The session's own latest verification, taken: one verification
    /// authorizes at most one procedure, and none once a step of this
    /// session's procedures started after it.
    fn take_verified(&mut self) -> Result<Result<SessionReport, Refusal>, String> {
        match self.verified.take() {
            Some((seen, outcome)) if seen == self.mutations.get() => Ok(outcome),
            Some(_) => Err("a procedure step ran since the session's latest verification".into()),
            None => Err("the session has not verified since its last procedure".into()),
        }
    }

    /// The session's own latest verification, if it succeeded, taken.
    pub(super) fn take_report(&mut self) -> Result<SessionReport, String> {
        self.take_verified()?.map_err(|refusal| {
            format!(
                "the session's latest verification refused: {:?}",
                refusal.refused
            )
        })
    }

    /// The session's own latest verification, if it refused, taken.
    pub(super) fn take_refusal(&mut self) -> Result<Refusal, String> {
        match self.take_verified()? {
            Err(refusal) => Ok(refusal),
            Ok(_) => Err("the session's latest verification did not refuse".into()),
        }
    }

    fn verify_inner(&mut self, config: &Config, state: &str) -> Result<SessionReport, Refusal> {
        let revision = Some(self.selection.revision());
        let fail = |refused: Refused| Refusal { refused, revision };
        let lock = self
            .locks
            .get(state)
            .ok_or_else(|| fail(Refused::NotAuthorized))?;
        if !revalidate(&self.io, &self.path, &self.selection, lock) {
            return Err(fail(Refused::SelectionChanged(
                "in-session verification".into(),
            )));
        }
        let pinning = if self.requalify {
            Pinning::Requalification
        } else {
            Pinning::Pinned
        };
        let OpenedRoot { dirs, .. } =
            open_root(&self.io, &self.path, &self.selection, self.opening, pinning)
                .map_err(fail)?;
        let store_dirs =
            open_store_dirs(&self.io, &dirs.root, &self.selection.provision).map_err(fail)?;
        let held: BTreeMap<u32, &P::File> = self
            .journal_locks
            .iter()
            .filter(|((owner, _), _)| owner == state)
            .map(|((_, index), file)| (*index, file))
            .collect();
        let scan = scan(
            &self.io,
            &self.selection.provision,
            &store_dirs,
            ScanMode::Session,
            &held,
            config.incident_limit,
        )
        .map_err(fail)?;
        let current = super::classify::current_incidents(&scan.level);
        let decision =
            super::classify::decide(&scan.level, &scan.dispositions, config.incident_limit)
                .map_err(|refusal| match refusal {
                    super::classify::PoolRefusal::Invalid(why) => fail(Refused::Invalid(why)),
                    super::classify::PoolRefusal::Capacity(why) => fail(Refused::Capacity(why)),
                })?;
        Ok(SessionReport {
            revision: self.selection.revision(),
            scan,
            current,
            blocking: decision.blocking,
            dispositioned: decision.dispositioned,
        })
    }

    /// After the session itself published `PROVISION`: read it by a fresh
    /// lookup and adopt it, under the session's own lock (design section
    /// 13.3 step 4).
    pub(super) fn reselect(&mut self) -> Result<(), String> {
        let selection = select(&self.io, &self.path).map_err(|refused| format!("{refused:?}"))?;
        if !self.locks.contains_key(&selection.state_name) {
            return Err("the session does not hold the selected store".into());
        }
        self.events
            .push(SessionEvent::Reselect(selection.revision()));
        self.selection = selection;
        Ok(())
    }

    /// Take and keep `LOCK_EX | LOCK_NB` on a pool file (Busy refuses).
    pub(super) fn hold_journal(&mut self, index: u32) -> Result<(), String> {
        let state = self.selection.state_name.clone();
        if self.journal_locks.contains_key(&(state.clone(), index)) {
            return Ok(());
        }
        let mut journals_path = self.selection.parent.clone();
        journals_path.push(state.clone());
        journals_path.push("journals".into());
        let journals = dir_ref(&self.io, &journals_path)?;
        let file = self
            .io
            .open_read(&journals.dir, &pool_name(index))
            .map_err(err)?;
        self.io
            .flock(&file, LockRequest::Exclusive)
            .map_err(|_| "Busy".to_string())?;
        self.journal_locks.insert((state.clone(), index), file);
        self.events.push(SessionEvent::HoldJournal(state, index));
        Ok(())
    }

    /// A held pool-file description, to read through.
    pub(super) fn journal(&self, index: u32) -> Option<&P::File> {
        self.journal_locks
            .get(&(self.selection.state_name.clone(), index))
    }

    /// Take the store lock of a state root this session just created.
    pub(super) fn adopt(&mut self, parent: &[String], state: &str) -> Result<(), String> {
        let mut root_path = parent.to_vec();
        root_path.push(state.to_string());
        let root = dir_ref(&self.io, &root_path)?;
        let lock = self.io.open_read(&root.dir, "LOCK").map_err(err)?;
        self.io
            .flock(&lock, LockRequest::Exclusive)
            .map_err(|_| "the successor's lock was not free".to_string())?;
        self.locks.insert(state.to_string(), lock);
        self.events.push(SessionEvent::Adopt(state.to_string()));
        Ok(())
    }

    /// End: every description closes, which releases the locks.
    pub fn end(mut self) {
        self.journal_locks.clear();
        self.locks.clear();
        self.active = false;
        self.events.push(SessionEvent::End);
    }

    /// The store's directories, reached afresh.
    pub(super) fn store_dir(&self, names: &[&str]) -> Result<DirRef<P>, String> {
        let mut path = self.selection.parent.clone();
        path.push(self.selection.state_name.clone());
        path.extend(names.iter().map(|name| name.to_string()));
        dir_ref(&self.io, &path)
    }

    /// The `PROVISION` bytes now at its path (a fresh read).
    pub(super) fn provision_bytes(&self) -> Result<Vec<u8>, String> {
        let provdir = dir_ref(&self.io, &self.path.directory)?;
        let file = self
            .io
            .open_read(&provdir.dir, &self.path.name)
            .map_err(err)?;
        read_whole(&self.io, &file, format::PROVISION_LIMIT)
    }
}

fn read_whole<P: StoreIo>(io: &P, file: &P::File, limit: usize) -> Result<Vec<u8>, String> {
    let mut data = Vec::new();
    let mut buf = vec![0u8; BLOCK];
    loop {
        let got = io.pread(file, data.len() as u64, &mut buf).map_err(err)?;
        if got == 0 {
            return Ok(data);
        }
        data.extend_from_slice(&buf[..got]);
        if data.len() > limit {
            return Err("too large".into());
        }
    }
}

// ---------------------------------------------------------------------------
// Procedures inside a session
// ---------------------------------------------------------------------------

/// The `PROVISION` rewrite (design section 13.3): revision + 1, written,
/// synced, renamed into place, the directory synced, then re-selection under
/// the session's own lock.
///
/// Internal to the store: section 13.3 uses it only inside recycling,
/// retirement and re-qualification, after their own preconditions. A public
/// rewrite of any field would skip those preconditions (a retirement without
/// its verification, pinned values changed outside re-qualification mode).
pub(super) fn rewrite_provision<P: Platform + Clone + 'static>(
    session: Rc<RefCell<Session<P>>>,
    change: impl Fn(&mut Provision) + 'static,
) -> Procedure<'static> {
    let base = session.borrow().selection().provision.clone();
    let path = Rc::new(session.borrow().path().clone());
    let io = Rc::new(session.borrow().io().clone());
    let make: Make = Rc::new(move || {
        let mut next = base.clone();
        next.revision = base.revision + 1;
        change(&mut next);
        render_provision(&next).map_err(|error| error.0.to_string())
    });
    let mut proc = publish_provision_steps(io, path, make, ("13.3/2", "13.3/3"), None);
    proc.within(Rc::clone(&session.borrow().mutations));
    proc.name = "PROVISION rewrite";
    if let Some(mut last) = proc.steps.pop() {
        proc.add(last.label, move || {
            (last.action)()?;
            session.borrow_mut().reselect()
        });
    }
    proc
}

/// P-DISP (design section 13.4, publication): the disposition written to a
/// temporary name, synced, linked (no replacement: an existing disposition
/// must be revoked first), the temporary unlinked, the directory synced.
pub fn publish_disposition<P: Platform + Clone + 'static>(
    session: &Session<P>,
    disposition: Disposition,
) -> Result<Procedure<'static>, String> {
    let text = render_disposition(&disposition).map_err(|error| error.0.to_string())?;
    let io = session.io().clone();
    let dispositions_path = {
        let mut path = session.selection().parent.clone();
        path.push(session.selection().state_name.clone());
        path.push("dispositions".into());
        path
    };
    let tmp = format!("{}{}", format::TMP_PREFIX, hex(&disposition.binding));
    let final_name = format::disposition_name(&disposition.binding);
    let file: Rc<RefCell<Option<P::File>>> = Rc::new(RefCell::new(None));
    let mut proc = Procedure::new("P-DISP");
    proc.within(Rc::clone(&session.mutations));
    let io = Rc::new(io);
    {
        let (io, path, tmp, file) = (
            Rc::clone(&io),
            dispositions_path.clone(),
            tmp.clone(),
            Rc::clone(&file),
        );
        proc.add("13.4/3", move || {
            let dir = dir_ref(&*io, &path)?;
            *file.borrow_mut() = Some(create_owned(&*io, &dir.dir, &tmp, (0, 0, 0o444), None)?);
            Ok(())
        });
    }
    {
        let (io, file) = (Rc::clone(&io), Rc::clone(&file));
        proc.add("13.4/3", move || {
            let guard = file.borrow();
            write_all(&*io, guard.as_ref().ok_or("not created")?, &text)
        });
    }
    {
        let (io, file) = (Rc::clone(&io), Rc::clone(&file));
        proc.add("13.4/3", move || {
            let handle = file.borrow_mut().take().ok_or("not created")?;
            sync_file(&*io, &handle)
        });
    }
    {
        let (io, path, tmp, final_name) = (
            Rc::clone(&io),
            dispositions_path.clone(),
            tmp.clone(),
            final_name.clone(),
        );
        proc.add("13.4/4", move || {
            let dir = dir_ref(&*io, &path)?;
            io.link(&dir.dir, &tmp, &dir.dir, &final_name).map_err(err)
        });
    }
    {
        let (io, path, tmp) = (Rc::clone(&io), dispositions_path.clone(), tmp.clone());
        proc.add("13.4/5", move || {
            let dir = dir_ref(&*io, &path)?;
            io.unlink(&dir.dir, &tmp).map_err(err)
        });
    }
    {
        let (io, path) = (Rc::clone(&io), dispositions_path);
        proc.add("13.4/6", move || sync_dir(&*io, &dir_ref(&*io, &path)?));
    }
    Ok(proc)
}

/// The disposition of one incident the in-session verification reported.
pub fn disposition_for(
    root_id: &[u8; 16],
    binding: [u8; 32],
    incident: &Incident,
    reason: DispositionReason,
    statement: &str,
    operator: &str,
    at: &str,
) -> Disposition {
    Disposition {
        root: *root_id,
        binding,
        facts: incident.facts.clone(),
        reason,
        statement: statement.into(),
        operator: operator.into(),
        at: at.into(),
    }
}

/// P-REVOKE (design section 13.4, revocation): renamed into `revoked/`, then
/// `revoked/` and `dispositions/` synced.
pub fn revoke<P: Platform + Clone + 'static>(
    session: &Session<P>,
    binding: [u8; 32],
    compact_time: &str,
) -> Procedure<'static> {
    let io = Rc::new(session.io().clone());
    let mut dispositions_path = session.selection().parent.clone();
    dispositions_path.push(session.selection().state_name.clone());
    dispositions_path.push("dispositions".into());
    let mut revoked_path = dispositions_path.clone();
    revoked_path.push("revoked".into());
    let name = format::disposition_name(&binding);
    let revoked_name = format::revoked_name(&binding, compact_time);
    let mut proc = Procedure::new("P-REVOKE");
    proc.within(Rc::clone(&session.mutations));
    {
        let (io, from, to) = (
            Rc::clone(&io),
            dispositions_path.clone(),
            revoked_path.clone(),
        );
        proc.add("13.4-revoke/2", move || {
            let from = dir_ref(&*io, &from)?;
            let to = dir_ref(&*io, &to)?;
            io.rename(&from.dir, &name, &to.dir, &revoked_name)
                .map_err(err)
        });
    }
    {
        let (io, path) = (Rc::clone(&io), revoked_path);
        proc.add("13.4-revoke/3", move || {
            sync_dir(&*io, &dir_ref(&*io, &path)?)
        });
    }
    {
        let (io, path) = (Rc::clone(&io), dispositions_path);
        proc.add("13.4-revoke/3", move || {
            sync_dir(&*io, &dir_ref(&*io, &path)?)
        });
    }
    proc
}

/// The archive entry name of a pool file the in-session report shows, if it
/// may be archived (design section 13.7): Sealed, unsealed with or without an
/// action start, Malformed (journal), or Malformed (pool-file); the last
/// three only with a valid disposition, which the caller's report shows.
pub fn archive_name(report: &super::classify::FileReport) -> Option<ArchiveName> {
    let class = match report.class {
        FileClass::Sealed => ArchiveClass::Sealed,
        FileClass::UnsealedNoAction => ArchiveClass::NoNativeWork,
        FileClass::UnsealedAction => ArchiveClass::Unresolved,
        FileClass::MalformedJournal => ArchiveClass::Malformed,
        FileClass::MalformedPoolFile => {
            return Some(ArchiveName::PoolFile {
                index: report.index,
                content: report.content,
            })
        }
        _ => return None,
    };
    let header = report.header.as_ref()?;
    Some(ArchiveName::Journal {
        claim: header.claim,
        generation: header.generation,
        content: report.content,
        class,
    })
}

/// Whether the session's verification shows the archival's preconditions:
/// an archivable class, and for an unresolved or malformed file a valid
/// disposition for its binding.
pub fn archival_allowed(report: &SessionReport, index: u32) -> Option<ArchiveName> {
    let file = report.scan.files.iter().find(|file| file.index == index)?;
    let name = archive_name(file)?;
    let root_id = report
        .scan
        .files
        .first()
        .and_then(|first| first.header.as_ref())
        .map(|header| header.root_id);
    let needs = match &name {
        ArchiveName::Journal {
            claim,
            generation,
            content,
            class: ArchiveClass::Unresolved | ArchiveClass::Malformed,
        } => {
            let class = if matches!(
                &name,
                ArchiveName::Journal {
                    class: ArchiveClass::Unresolved,
                    ..
                }
            ) {
                IncidentClass::Unresolved
            } else {
                IncidentClass::Malformed
            };
            Some((*claim, *generation, *content, class))
        }
        _ => None,
    };
    if let Some((claim, generation, content, class)) = needs {
        let found = report.scan.dispositions.values().any(|disposition| {
            disposition.facts.kind == IncidentKind::Journal
                && disposition.facts.claim == Some(claim)
                && disposition.facts.generation == Some(generation)
                && disposition.facts.content == Some(content)
                && disposition.facts.class == class
                && root_id.is_none_or(|root| disposition.root == root)
        });
        if !found {
            return None;
        }
    }
    if let ArchiveName::PoolFile { index, content } = &name {
        let found = report.scan.dispositions.values().any(|disposition| {
            disposition.facts.kind == IncidentKind::PoolFile
                && disposition.facts.pool_index == Some(*index)
                && disposition.facts.content == Some(*content)
        });
        if !found {
            return None;
        }
    }
    Some(name)
}

/// P-ARCH (design section 13.7): the pool file read through the session's own
/// journal lock, copied to a new root-owned temporary, synced, re-read and
/// compared, linked to its final name, the temporary unlinked, `archive/`
/// synced. The pool file is unchanged. The archive name and its
/// preconditions come from the session's own latest verification
/// ([`archival_allowed`] over it), never from the caller.
pub fn archive<P: Platform + Clone + 'static>(
    session: &mut Session<P>,
    index: u32,
) -> Result<Procedure<'static>, String> {
    let report = session.take_report()?;
    let name = archival_allowed(&report, index)
        .ok_or("the session's verification does not allow archiving this pool file")?;
    let name = &name;
    session.hold_journal(index)?;
    let io = Rc::new(session.io().clone());
    let content = {
        let file = session.journal(index).ok_or("journal not held")?;
        read_whole(
            &*io,
            file,
            (pool_file_size(format::MAX_C_POOL).unwrap_or(0)) as usize,
        )?
    };
    let digest = sha256(&content);
    let expected = match name {
        ArchiveName::Journal { content, .. } | ArchiveName::PoolFile { content, .. } => *content,
    };
    if digest != expected {
        return Err("the pool file changed since the verification".into());
    }
    let mut archive_path = session.selection().parent.clone();
    archive_path.push(session.selection().state_name.clone());
    archive_path.push("archive".into());
    let final_name = name.render();
    let tmp = format!("{}{}", format::TMP_PREFIX, final_name);
    let file: Rc<RefCell<Option<P::File>>> = Rc::new(RefCell::new(None));
    let content = Rc::new(content);
    let mut proc = Procedure::new("P-ARCH");
    proc.within(Rc::clone(&session.mutations));
    {
        let (io, path, tmp, file) = (
            Rc::clone(&io),
            archive_path.clone(),
            tmp.clone(),
            Rc::clone(&file),
        );
        proc.add("13.7/3", move || {
            let dir = dir_ref(&*io, &path)?;
            *file.borrow_mut() = Some(create_owned(&*io, &dir.dir, &tmp, (0, 0, 0o444), None)?);
            Ok(())
        });
    }
    {
        let (io, file, content) = (Rc::clone(&io), Rc::clone(&file), Rc::clone(&content));
        proc.add("13.7/3", move || {
            let guard = file.borrow();
            write_all(&*io, guard.as_ref().ok_or("not created")?, &content)
        });
    }
    {
        let (io, file) = (Rc::clone(&io), Rc::clone(&file));
        proc.add("13.7/3", move || {
            let handle = file.borrow_mut().take().ok_or("not created")?;
            sync_file(&*io, &handle)?;
            let copy = read_whole(&*io, &handle, usize::MAX)?;
            if sha256(&copy) != digest {
                return Err("the archive copy differs".into());
            }
            Ok(())
        });
    }
    {
        let (io, path, tmp, final_name) = (
            Rc::clone(&io),
            archive_path.clone(),
            tmp.clone(),
            final_name.clone(),
        );
        proc.add("13.7/4", move || {
            let dir = dir_ref(&*io, &path)?;
            io.link(&dir.dir, &tmp, &dir.dir, &final_name).map_err(err)
        });
    }
    {
        let (io, path, tmp) = (Rc::clone(&io), archive_path.clone(), tmp);
        proc.add("13.7/5", move || {
            let dir = dir_ref(&*io, &path)?;
            io.unlink(&dir.dir, &tmp).map_err(err)
        });
    }
    {
        let (io, path) = (Rc::clone(&io), archive_path);
        proc.add("13.7/5", move || sync_dir(&*io, &dir_ref(&*io, &path)?));
    }
    Ok(proc)
}

/// P-RECYCLE (design section 13.8): a new zero-filled pool file under a
/// temporary name, synced, renamed over the pool name, `journals/` synced,
/// then the `PROVISION` rewrite with the new inode, in the same session.
/// Only for a file archived and pending recycling, or an abandoned claim.
pub fn recycle<P: Platform + Clone + 'static>(
    session: Rc<RefCell<Session<P>>>,
    index: u32,
) -> Result<Procedure<'static>, String> {
    let report = session.borrow_mut().take_report()?;
    let file_class = report
        .scan
        .files
        .iter()
        .find(|file| file.index == index)
        .map(|file| file.class)
        .ok_or("no such pool file")?;
    if !(report.scan.level.pending_recycle.contains(&index)
        || file_class == FileClass::AbandonedClaim)
    {
        return Err("not archived and pending recycling, nor an abandoned claim".into());
    }
    session.borrow_mut().hold_journal(index)?;
    let io = Rc::new(session.borrow().io().clone());
    let (uid, gid, c_pool) = {
        let s = session.borrow();
        let p = &s.selection().provision;
        (p.uid, p.gid, p.c_pool)
    };
    let size = pool_file_size(c_pool).ok_or("pool size")?;
    let mut journals_path = session.borrow().selection().parent.clone();
    journals_path.push(session.borrow().selection().state_name.clone());
    journals_path.push("journals".into());
    let tmp = format!("{}{index:05}", format::TMP_PREFIX);
    let file: Rc<RefCell<Option<P::File>>> = Rc::new(RefCell::new(None));
    let new_ino: Rc<RefCell<u64>> = Rc::new(RefCell::new(0));
    let mut proc = Procedure::new("P-RECYCLE");
    proc.within(Rc::clone(&session.borrow().mutations));
    {
        let (io, path, tmp, file, new_ino) = (
            Rc::clone(&io),
            journals_path.clone(),
            tmp.clone(),
            Rc::clone(&file),
            Rc::clone(&new_ino),
        );
        proc.add("13.8/2", move || {
            let dir = dir_ref(&*io, &path)?;
            let handle = create_owned(&*io, &dir.dir, &tmp, (uid, gid, 0o600), Some(size))?;
            *new_ino.borrow_mut() = io.stat_file(&handle).map_err(err)?.ino;
            *file.borrow_mut() = Some(handle);
            Ok(())
        });
    }
    {
        let (io, file) = (Rc::clone(&io), Rc::clone(&file));
        proc.add("13.8/2", move || {
            let guard = file.borrow();
            write_all(
                &*io,
                guard.as_ref().ok_or("not created")?,
                &vec![0u8; size as usize],
            )
        });
    }
    {
        let (io, file) = (Rc::clone(&io), Rc::clone(&file));
        proc.add("13.8/2", move || {
            let handle = file.borrow_mut().take().ok_or("not created")?;
            sync_file(&*io, &handle)
        });
    }
    {
        let (io, path, tmp) = (Rc::clone(&io), journals_path.clone(), tmp);
        proc.add("13.8/3", move || {
            let dir = dir_ref(&*io, &path)?;
            io.rename(&dir.dir, &tmp, &dir.dir, &pool_name(index))
                .map_err(err)
        });
    }
    {
        let (io, path) = (Rc::clone(&io), journals_path);
        proc.add("13.8/3", move || sync_dir(&*io, &dir_ref(&*io, &path)?));
    }
    let rewrite = rewrite_provision(Rc::clone(&session), move |provision: &mut Provision| {
        provision.pool_inodes[index as usize] = *new_ino.borrow();
    });
    proc.extend(rewrite);
    Ok(proc)
}

/// Section 13.8, resumption after a crash between the recycling's rename and
/// the end of its rewrite: only when the session's verification refused
/// exactly because that pool name holds an inode `PROVISION` does not record,
/// that inode, read through the session's journal lock, is a zero-filled
/// pool file owned by the store uid with one link, and the interrupted
/// session's saved report showed the old file archived and pending recycling,
/// or an abandoned claim.
///
/// The refusal is the session's own latest verification's, never a
/// caller's. The saved report is the interrupted session's, which no
/// running process retains: it is candidate input the Owner supplies (R5
/// section 13.8). The store checks it only against the session's own
/// refusal and its own reads of the file at the pool name.
pub fn resume_recycle<P: Platform + Clone + 'static>(
    session: Rc<RefCell<Session<P>>>,
    index: u32,
    saved: &SessionReport,
) -> Result<Procedure<'static>, String> {
    let refusal = session.borrow_mut().take_refusal()?;
    let name = pool_name(index);
    if refusal.refused != Refused::Lost(format!("{name} replaced")) {
        return Err("not the resumable refusal".into());
    }
    let saved_ok = saved.scan.level.pending_recycle.contains(&index)
        || saved
            .scan
            .files
            .iter()
            .any(|file| file.index == index && file.class == FileClass::AbandonedClaim);
    if !saved_ok {
        return Err("the saved report does not show the file archived or abandoned".into());
    }
    let (uid, gid, c_pool) = {
        let s = session.borrow();
        let p = &s.selection().provision;
        (p.uid, p.gid, p.c_pool)
    };
    let journals = session.borrow().store_dir(&["journals"])?;
    let stat = session
        .borrow()
        .io()
        .stat_at(&journals.dir, &name)
        .map_err(err)?;
    if stat.file_type != FileType::Regular
        || (stat.uid, stat.gid, stat.mode, stat.nlink) != (uid, gid, 0o600, 1)
    {
        return Err("the file at the pool name is not a store pool file".into());
    }
    session.borrow_mut().hold_journal(index)?;
    let size = pool_file_size(c_pool).ok_or("pool size")? as usize;
    let data = {
        let s = session.borrow();
        let file = s.journal(index).ok_or("journal not held")?;
        let held = s.io().stat_file(file).map_err(err)?;
        if !held.same_inode(&stat) {
            return Err("the held journal is not the file at the pool name".into());
        }
        read_whole(s.io(), file, size + 1)?
    };
    if data.len() != size || data.iter().any(|byte| *byte != 0) {
        return Err("the file at the pool name is not zero-filled".into());
    }
    let ino = stat.ino;
    let mut proc = rewrite_provision(session, move |provision: &mut Provision| {
        provision.pool_inodes[index as usize] = ino;
    });
    proc.name = "R-RESUME";
    Ok(proc)
}

/// Retirement's preconditions (design section 13.9), over the complete
/// incident set of the in-session report, never its listed detail.
pub fn retirement_allowed(report: &SessionReport, through: u64) -> bool {
    for file in &report.scan.files {
        if let Some(header) = &file.header {
            if header.claim <= through && !report.scan.level.pending_recycle.contains(&file.index) {
                return false;
            }
        }
    }
    report
        .scan
        .level
        .incidents
        .iter()
        .all(|(binding, incident)| {
            incident.facts.claim.is_none_or(|claim| claim > through)
                || report.scan.level.history.contains(binding)
        })
}

/// P-RETIRE (design section 13.9): the `PROVISION` rewrite with
/// `retired-through = through`, only when the preconditions hold over the
/// session's own latest verification.
pub fn retire<P: Platform + Clone + 'static>(
    session: Rc<RefCell<Session<P>>>,
    through: u64,
) -> Option<Procedure<'static>> {
    let report = session.borrow_mut().take_report().ok()?;
    if !retirement_allowed(&report, through) {
        return None;
    }
    let mut proc = rewrite_provision(session, move |provision: &mut Provision| {
        provision.retired_through = through;
    });
    proc.name = "P-RETIRE";
    Some(proc)
}

/// P-REQUALIFY (design section 13.3): the `PROVISION` rewrite with the newly
/// qualified pinned values. The session must have begun in
/// re-qualification mode; nothing here relaxes a rule.
pub fn requalify<P: Platform + Clone + 'static>(
    session: Rc<RefCell<Session<P>>>,
    qualification: Qualification,
) -> Result<Procedure<'static>, String> {
    if !session.borrow().requalify {
        return Err("not a re-qualification session".into());
    }
    let mut proc = rewrite_provision(session, move |provision: &mut Provision| {
        provision.mount_options = qualification.mount_options.clone();
        provision.super_options = qualification.super_options.clone();
        provision.kernel = qualification.kernel.clone();
        provision.storage_pci_function = qualification.storage_pci_function.clone();
        provision.storage_partition = qualification.storage_partition;
        provision.storage_identity = qualification.storage_identity;
    });
    proc.name = "P-REQUALIFY";
    Ok(proc)
}

/// P-SUCCESSOR (design section 13.6): inside the predecessor's session, which
/// holds the predecessor's lock throughout: keep the predecessor's
/// `PROVISION`; create the successor's store; take the successor's lock,
/// then publish its `PROVISION` (the storage fields of the storage the
/// session's own opening admitted); re-select the successor.
pub fn successor<P: Platform + Clone + 'static>(
    session: Rc<RefCell<Session<P>>>,
    layout: &Layout,
    statement: &str,
) -> Result<Procedure<'static>, String> {
    let (old_bytes, old, path) = {
        let s = session.borrow();
        (
            s.provision_bytes()?,
            s.selection().provision.clone(),
            s.path().clone(),
        )
    };
    if format::parse_provision(&old_bytes).map_err(|error| error.0)? != old {
        return Err("PROVISION changed under the session".into());
    }
    let qualification = {
        let s = session.borrow();
        let observation = s
            .admission()
            .ok_or("no storage admission in this session")?
            .observation()
            .clone();
        Qualification {
            mount_options: old.mount_options.clone(),
            super_options: old.super_options.clone(),
            kernel: old.kernel.clone(),
            storage_pci_function: observation.pci_function,
            storage_partition: observation.partition,
            storage_identity: observation.identity,
        }
    };
    let io = Rc::new(session.borrow().io().clone());
    let mutations = Rc::clone(&session.borrow().mutations);
    let path = Rc::new(path);
    let layout = Rc::new(layout.clone());
    let mut proc = Procedure::new("P-SUCCESSOR");
    let file: Rc<RefCell<Option<P::File>>> = Rc::new(RefCell::new(None));
    let predecessor_name = path.predecessor_name(&old.root_id);
    {
        let (io, path, file) = (Rc::clone(&io), Rc::clone(&path), Rc::clone(&file));
        proc.add("13.6/2a", move || {
            let provdir = dir_ref(&*io, &path.directory)?;
            *file.borrow_mut() = Some(create_owned(
                &*io,
                &provdir.dir,
                &path.predecessor_tmp_name(),
                (0, 0, 0o444),
                None,
            )?);
            Ok(())
        });
    }
    {
        let (io, file) = (Rc::clone(&io), Rc::clone(&file));
        proc.add("13.6/2a", move || {
            let guard = file.borrow();
            write_all(&*io, guard.as_ref().ok_or("not created")?, &old_bytes)
        });
    }
    {
        let (io, file) = (Rc::clone(&io), Rc::clone(&file));
        proc.add("13.6/2a", move || {
            let handle = file.borrow_mut().take().ok_or("not created")?;
            sync_file(&*io, &handle)
        });
    }
    {
        let (io, path) = (Rc::clone(&io), Rc::clone(&path));
        proc.add("13.6/2a", move || {
            let provdir = dir_ref(&*io, &path.directory)?;
            io.link(
                &provdir.dir,
                &path.predecessor_tmp_name(),
                &provdir.dir,
                &predecessor_name,
            )
            .map_err(err)
        });
    }
    {
        let (io, path) = (Rc::clone(&io), Rc::clone(&path));
        proc.add("13.6/2a", move || {
            let provdir = dir_ref(&*io, &path.directory)?;
            io.unlink(&provdir.dir, &path.predecessor_tmp_name())
                .map_err(err)
        });
    }
    {
        let (io, path) = (Rc::clone(&io), Rc::clone(&path));
        proc.add("13.6/2a", move || {
            sync_dir(&*io, &dir_ref(&*io, &path.directory)?)
        });
    }
    let made = Rc::new(RefCell::new(Made::default()));
    proc.extend(store_steps(
        Rc::clone(&io),
        Rc::clone(&layout),
        Rc::clone(&made),
        ("13.6/2b", "13.6/2b", "13.6/2b"),
    ));
    let adopt_session = Rc::clone(&session);
    let (parent, state) = (layout.parent.clone(), layout.state_name.clone());
    let before: Box<dyn FnMut() -> Result<(), String>> =
        Box::new(move || adopt_session.borrow_mut().adopt(&parent, &state));
    let old_root = old.root_id;
    let revision = old.revision + 1;
    let statement = statement.to_string();
    let content_layout = Rc::clone(&layout);
    let make: Make = Rc::new(move || {
        let provision = provision_for(
            &content_layout,
            &made.borrow(),
            &qualification,
            Some((old_root, statement.clone())),
            revision,
        );
        render_provision(&provision).map_err(|error| error.0.to_string())
    });
    let mut publish = publish_provision_steps(io, path, make, ("13.6/2c", "13.6/2c"), Some(before));
    if let Some(mut last) = publish.steps.pop() {
        publish.add(last.label, move || {
            (last.action)()?;
            session.borrow_mut().reselect()
        });
    }
    proc.extend(publish);
    proc.within(mutations);
    Ok(proc)
}

/// R-LEFTOVER (design section 13.1, recovery): inside a session, unlink every
/// leftover temporary entry of an interrupted procedure, then sync each
/// directory changed.
pub fn leftover<P: Platform + Clone + 'static>(
    session: &Session<P>,
) -> Result<Procedure<'static>, String> {
    let io = Rc::new(session.io().clone());
    let mut proc = Procedure::new("R-LEFTOVER");
    proc.within(Rc::clone(&session.mutations));
    let mut state_root = session.selection().parent.clone();
    state_root.push(session.selection().state_name.clone());
    let mut dirs: Vec<(Vec<String>, bool)> = Vec::new();
    for names in [
        vec!["journals"],
        vec!["dispositions"],
        vec!["dispositions", "revoked"],
        vec!["archive"],
    ] {
        let mut path = state_root.clone();
        path.extend(names.iter().map(|name| name.to_string()));
        dirs.push((path, false));
    }
    dirs.push((session.path().directory.clone(), true));
    for (path, is_provdir) in dirs {
        let dir = dir_ref(&*io, &path)?;
        let names = match io.list_dir(&dir.dir, 8192).map_err(err)? {
            super::io::Listing::Names(names) => names,
            super::io::Listing::TooMany => return Err("too many entries".into()),
        };
        let leftovers: Vec<String> = names
            .into_iter()
            .filter(|name| {
                if is_provdir {
                    *name == session.path().tmp_name()
                        || *name == session.path().predecessor_tmp_name()
                } else {
                    name.starts_with(format::TMP_PREFIX)
                }
            })
            .collect();
        for name in &leftovers {
            let (io, path, name) = (Rc::clone(&io), path.clone(), name.clone());
            proc.add("13.1/recover", move || {
                let dir = dir_ref(&*io, &path)?;
                io.unlink(&dir.dir, &name).map_err(err)
            });
        }
        if !leftovers.is_empty() {
            let io = Rc::clone(&io);
            proc.add("13.1/recover", move || {
                sync_dir(&*io, &dir_ref(&*io, &path)?)
            });
        }
    }
    Ok(proc)
}

/// The header of a claim, for fixtures that build journals (design section
/// 7.2): re-exported encoder, so the tests use the store's own.
pub fn header_block(fields: &format::HeaderFields) -> Result<Box<[u8; BLOCK]>, String> {
    encode_header(fields).map_err(|error| error.0.to_string())
}

/// Offset of a record block.
pub fn record_offset(seq: u64) -> u64 {
    (seq + 1) * BLOCK_U64
}

/// An incident's facts for a claim gap (fixtures and reports).
pub fn gap_facts(claim: u64) -> IncidentFacts {
    IncidentFacts {
        kind: IncidentKind::ClaimGap,
        claim: Some(claim),
        generation: None,
        pool_index: None,
        content: None,
        class: IncidentClass::Malformed,
        recorded_unsettled: 0,
    }
}

/// An `Errno` a procedure step may meet that is never success.
pub fn never_success(errno: Errno) -> bool {
    !matches!(errno, Errno::Intr)
}

/// Open the session without interleaving hooks.
pub fn begin<P: Platform>(
    io: P,
    path: &ProvisionPath,
    requalify: bool,
) -> Result<Session<P>, Refusal> {
    Session::begin(io, path, requalify, &mut NoHooks)
}
