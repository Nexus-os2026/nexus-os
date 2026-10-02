//! P2-V1-R3B-I3-I1-R3 baseline probes: the Architect's administrative
//! recovery findings R3-B1 to R3-B8, run against the exact base
//! 66a245c183b04ff1e2ae30fbbd065a57381df09c (tree
//! 13c689357c5781ebd048f4cee5081028329a729d) through ordinary safe-Rust call
//! sites of the store's public API and the simulator's fixture interface,
//! from outside the store module. This file is the only addition to an
//! isolated copy of the base tree; every other source is the base's blob.
//! Each probe prints what it observed and asserts only what it observed.

#![cfg(target_os = "linux")]

#[path = "support/custody/mod.rs"]
mod custody;

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use custody::store::format::{self, parse_provision};
use custody::store::io::{FileType, Listing, StoreIo};
use custody::store::maintenance::{self as maint, Session};
use custody::store::open::{open_owner, NoHooks};
use custody::store::owner::{start_owner, StartRefused, StoreOwner};
use custody::store::sim::{self, Fixture, Schedule, SimIo, Tear};
use custody::*;

#[derive(Debug)]
struct Token(SlotKind);

impl Resource for Token {
    fn kind(&self) -> SlotKind {
        self.0
    }
}

const SMALL: Config = Config {
    automatic_attempts: 2,
    recovery_budget: 3,
    recovery_spacing_millis: 1,
    lease_millis: 1_000_000,
    record_capacity: 16,
    control_reserve: 2,
    receipt_limit: 4,
    late_limit: 1,
    failure_detail_limit: 8,
    detail_chars: 64,
    incident_limit: 4,
};

const PROVDIR: &str = "/var/lib/nexus-os/phase2-custody/provision";

type Shared = Rc<RefCell<Session<SimIo>>>;

fn clock() -> Arc<dyn Fn() -> Tick + Send + Sync> {
    let counter = Arc::new(AtomicU64::new(0));
    Arc::new(move || Tick(counter.fetch_add(1, Ordering::SeqCst)))
}

struct Ticks(u64);

impl Ticks {
    fn tick(&mut self) -> Tick {
        self.0 += 1;
        Tick(self.0)
    }
}

fn pump(owner: &mut StoreOwner<Token, SimIo>, ticks: &mut Ticks) {
    for _ in 0..8 {
        let now = ticks.tick();
        let _ = owner.flush(now);
        owner.run_worker_until_idle();
        let now = ticks.tick();
        owner.apply(now);
    }
}

fn quiet() -> impl FnMut(Option<&'static str>) {
    |_| {}
}

/// A store whose pool file 0 holds an unsealed generation that recorded an
/// action start (its owner died): one current, blocking, undispositioned
/// incident. Returns its binding, as a fresh owner's refusal names it.
fn store_with_incident() -> (Fixture, [u8; 32]) {
    let fixture = Fixture::provisioned(2, 16);
    let io = fixture.store_process("dying-owner");
    let mut owner: StoreOwner<Token, SimIo> =
        match start_owner(&io, &fixture.path, &SMALL, Tick(1), clock(), &mut NoHooks) {
            Ok(owner) => owner,
            Err(refused) => panic!("the first owner starts: {refused:?}"),
        };
    owner.run_worker_until_idle();
    let mut ticks = Ticks(10);
    owner.start_run(ticks.tick()).expect("the run starts");
    pump(&mut owner, &mut ticks);
    owner
        .begin_case(CaseId(1), Expectation::Clean, ticks.tick())
        .expect("the case begins");
    pump(&mut owner, &mut ticks);
    let reservation = owner
        .reserve(SlotKind::Process, ticks.tick())
        .expect("reserved");
    pump(&mut owner, &mut ticks);
    let ticket = owner
        .admit(reservation, ticks.tick())
        .expect("admitted through the gate");
    let _ = owner.complete(
        &ticket,
        NativeOutcome::Created(Token(SlotKind::Process)),
        ticks.tick(),
    );
    fixture.world.kill(&io);
    drop(owner);
    let binding = match start_owner::<Token, _>(
        &fixture.store_process("probe"),
        &fixture.path,
        &SMALL,
        Tick(1),
        clock(),
        &mut NoHooks,
    ) {
        Err(StartRefused::PriorUnresolved(blocking)) => {
            assert_eq!(blocking.len(), 1);
            blocking[0]
        }
        Err(other) => panic!("expected PriorUnresolved: {other:?}"),
        Ok(_) => panic!("expected PriorUnresolved: the owner started"),
    };
    (fixture, binding)
}

fn successor_layout(fixture: &Fixture) -> maint::Layout {
    let mut layout = fixture.layout.clone();
    layout.root_id = sim::SUCCESSOR_ID;
    layout.state_name = format!("uid-{}-{}", sim::STORE_UID, format::hex(&sim::SUCCESSOR_ID));
    layout
}

fn session(fixture: &Fixture, name: &str) -> Shared {
    match maint::begin(fixture.root_process(name), &fixture.path, false) {
        Ok(session) => Rc::new(RefCell::new(session)),
        Err(refusal) => panic!("a session: {refusal:?}"),
    }
}

fn end(session: Shared) {
    match Rc::try_unwrap(session) {
        Ok(session) => session.into_inner().end(),
        Err(_) => panic!("the session is still shared"),
    }
}

/// F1: the session's process dies; its descriptors close and its locks are
/// released.
fn crash(fixture: &Fixture, session: Shared) {
    let io = session.borrow().io().clone();
    fixture.world.kill(&io);
    drop(session);
}

fn provision_now(fixture: &Fixture) -> (Vec<u8>, format::Provision) {
    let ino = fixture
        .world
        .lookup(sim::PROVISION_PATH)
        .expect("PROVISION");
    let bytes = fixture.world.visible(ino);
    let parsed = parse_provision(&bytes).expect("PROVISION parses");
    (bytes, parsed)
}

/// A fresh root process's description of a directory tree: each entry's
/// type, owner, mode and link count, and for a regular file its size and
/// whether it holds a non-zero byte.
fn describe(fixture: &Fixture, path: &str) -> Vec<String> {
    let io = fixture.root_process("inspect");
    let components: Vec<String> = path
        .split('/')
        .filter(|part| !part.is_empty())
        .map(str::to_string)
        .collect();
    let mut out = Vec::new();
    let Ok(dir) = maint::dir_ref(&io, &components) else {
        out.push(format!("{path}: absent"));
        return out;
    };
    describe_dir(&io, fixture, &dir.dir, path, &mut out);
    out
}

fn describe_dir(
    io: &SimIo,
    fixture: &Fixture,
    dir: &<SimIo as StoreIo>::Dir,
    path: &str,
    out: &mut Vec<String>,
) {
    let names = match io.list_dir(dir, 8192) {
        Ok(Listing::Names(names)) => names,
        other => {
            out.push(format!("{path}: listing {other:?}"));
            return;
        }
    };
    for name in names {
        let Ok(stat) = io.stat_at(dir, &name) else {
            out.push(format!("{path}/{name}: no stat"));
            continue;
        };
        let mut line = format!(
            "{path}/{name}: {:?} {}:{} {:o} nlink {}",
            stat.file_type, stat.uid, stat.gid, stat.mode, stat.nlink
        );
        if stat.file_type == FileType::Regular {
            let bytes = fixture.world.visible(stat.ino);
            line.push_str(&format!(
                " size {} nonzero {}",
                bytes.len(),
                bytes.iter().any(|byte| *byte != 0)
            ));
        }
        out.push(line);
        if stat.file_type == FileType::Directory {
            if let Ok(child) = io.open_dir(dir, &name) {
                describe_dir(io, fixture, &child, &format!("{path}/{name}"), out);
            }
        }
    }
}

fn ops(trace: &[sim::TraceOp]) -> Vec<String> {
    trace
        .iter()
        .map(|op| format!("{} {}", op.op, op.name.clone().unwrap_or_default()))
        .collect()
}

fn publish(fixture: &Fixture, binding: [u8; 32], statement: &str) {
    let s = session(fixture, "publish");
    let report = s.borrow_mut().verify(&SMALL).expect("verifies");
    assert!(report.current.iter().any(|(found, _)| *found == binding));
    maint::publish_disposition(
        Rc::clone(&s),
        binding,
        &maint::OwnerWords {
            reason: DispositionReason::OwnerDestroyed,
            statement: statement.into(),
            operator: "owner".into(),
            at: "2026-10-02T12:00:00Z".into(),
        },
    )
    .expect("a procedure")
    .run_all(&mut quiet())
    .expect("published");
    end(s);
}

/// R3-B1: an authorized succession crashed (F1) after its operation 6 (the
/// predecessor's PROVISION kept and its directory synced), before the
/// successor's root; the leftover recovery, an honest verification and the
/// same succession again.
#[test]
fn r3b1_repeat_after_successor_step_2a() {
    let fixture = Fixture::provisioned(2, 16);
    let layout = successor_layout(&fixture);
    let (verified_bytes, verified) = provision_now(&fixture);
    let s = session(&fixture, "first");
    s.borrow_mut().verify(&SMALL).expect("the predecessor verifies");
    let mut procedure = maint::successor(Rc::clone(&s), &layout, &[]).expect("authorized");
    let labels = procedure.labels();
    println!(
        "PROBE R3-B1: the authorized succession has {} steps; steps 1 to 6: {:?}",
        labels.len(),
        &labels[..6]
    );
    procedure.run(6, &mut quiet()).expect("operations 1 to 6");
    drop(procedure);
    crash(&fixture, s);
    let copy_name = fixture.path.predecessor_name(&verified.root_id);
    let provdir = fixture.world.lookup(PROVDIR).expect("the PROVISION directory");
    println!(
        "PROBE R3-B1: after F1 at operation 6, the PROVISION directory holds {:?}; durably {:?}",
        fixture.world.entries(provdir).keys().collect::<Vec<_>>(),
        fixture.world.durable_entries(provdir).keys().collect::<Vec<_>>()
    );
    let copy_ino = fixture.world.entries(provdir)[&copy_name];
    println!(
        "PROBE R3-B1: {copy_name} equals the PROVISION the session verified: {}",
        fixture.world.visible(copy_ino) == verified_bytes
    );
    let (_, now) = provision_now(&fixture);
    println!(
        "PROBE R3-B1: PROVISION selects root {} (revision {}); the successor's root exists: {}",
        format::hex(&now.root_id),
        now.revision,
        fixture.world.lookup(&layout.state_root()).is_some()
    );
    let s = session(&fixture, "second");
    let mut recovery = maint::leftover(Rc::clone(&s)).expect("R-LEFTOVER");
    println!(
        "PROBE R3-B1: R-LEFTOVER has {} step(s): {:?}",
        recovery.len(),
        recovery.labels()
    );
    println!("PROBE R3-B1: R-LEFTOVER: {:?}", recovery.run_all(&mut quiet()));
    drop(recovery);
    let verified_again = s.borrow_mut().verify(&SMALL);
    println!(
        "PROBE R3-B1: a fresh in-session verification: {:?}",
        verified_again
            .as_ref()
            .map(|report| (report.current.len(), report.blocking.len()))
            .map_err(|refusal| refusal.refused.clone())
    );
    match maint::successor(Rc::clone(&s), &layout, &[]) {
        Err(why) => println!("PROBE R3-B1: the repeat is refused at authorization: {why}"),
        Ok(mut repeat) => {
            fixture.world.trace_start();
            let result = repeat.run_all(&mut quiet());
            let trace = fixture.world.trace_take();
            println!(
                "PROBE R3-B1: the repeat: {result:?}; steps done {} of {}",
                repeat.steps_done(),
                repeat.len()
            );
            println!("PROBE R3-B1: the repeat's operations: {:?}", ops(&trace));
            drop(repeat);
        }
    }
    println!(
        "PROBE R3-B1: the PROVISION directory after the repeat: {:?}",
        fixture.world.entries(provdir).keys().collect::<Vec<_>>()
    );
    let (_, after) = provision_now(&fixture);
    println!(
        "PROBE R3-B1: PROVISION still selects root {} (revision {})",
        format::hex(&after.root_id),
        after.revision
    );
    end(s);
}

/// R3-B2: an authorized succession crashed (F1) during step 2b, after its
/// operation 23 (the successor's directories, LOCK and pool file 0 made and
/// synced; pool file 1 not yet), before any successor PROVISION.
#[test]
fn r3b2_partially_created_successor_root() {
    let fixture = Fixture::provisioned(2, 16);
    let layout = successor_layout(&fixture);
    let s = session(&fixture, "first");
    s.borrow_mut().verify(&SMALL).expect("the predecessor verifies");
    let mut procedure = maint::successor(Rc::clone(&s), &layout, &[]).expect("authorized");
    let labels = procedure.labels();
    println!(
        "PROBE R3-B2: operations 7 to 23 are {:?}",
        &labels[6..23]
    );
    procedure.run(23, &mut quiet()).expect("operations 1 to 23");
    drop(procedure);
    crash(&fixture, s);
    let (_, now) = provision_now(&fixture);
    println!(
        "PROBE R3-B2: PROVISION selects root {} (revision {}): the predecessor stays selected: {}",
        format::hex(&now.root_id),
        now.revision,
        now.root_id == fixture.layout.root_id
    );
    for line in describe(&fixture, &layout.state_root()) {
        println!("PROBE R3-B2: successor {line}");
    }
    let opened = open_owner(
        &fixture.store_process("owner"),
        &fixture.path,
        &SMALL,
        &mut NoHooks,
    );
    println!(
        "PROBE R3-B2: an owner's opening of the selected store: {}",
        match &opened {
            Ok(_) => "opens".to_string(),
            Err(refusal) => format!("{:?}", refusal.refused),
        }
    );
    drop(opened);
    let s = session(&fixture, "second");
    let mut recovery = maint::leftover(Rc::clone(&s)).expect("R-LEFTOVER");
    println!(
        "PROBE R3-B2: R-LEFTOVER has {} step(s) {:?}: {:?}",
        recovery.len(),
        recovery.labels(),
        recovery.run_all(&mut quiet())
    );
    drop(recovery);
    s.borrow_mut().verify(&SMALL).expect("the predecessor verifies");
    match maint::successor(Rc::clone(&s), &layout, &[]) {
        Err(why) => println!("PROBE R3-B2: the repeat is refused at authorization: {why}"),
        Ok(mut repeat) => {
            fixture.world.trace_start();
            let result = repeat.run_all(&mut quiet());
            println!(
                "PROBE R3-B2: the repeat with the same layout: {result:?}; operations {:?}",
                ops(&fixture.world.trace_take())
            );
        }
    }
    println!(
        "PROBE R3-B2: the successor's root still exists: {}",
        fixture.world.lookup(&layout.state_root()).is_some()
    );
    let io_source = include_str!("support/custody/store/io.rs");
    let maintenance_source = include_str!("support/custody/store/maintenance.rs");
    let trait_start = io_source.find("pub trait StoreIo").expect("the StoreIo trait");
    let trait_text = &io_source[trait_start..];
    let trait_text = &trait_text[..trait_text.find("\n}\n").expect("the trait's end")];
    println!(
        "PROBE R3-B2: the StoreIo trait declares a directory removal (AT_REMOVEDIR, remove_dir, rmdir): {}",
        ["AT_REMOVEDIR", "remove_dir", "rmdir"]
            .iter()
            .any(|token| trait_text.contains(token))
    );
    println!(
        "PROBE R3-B2: io.rs's only AT_REMOVEDIR use is the Linux fixture helper for a test's own entries (linux::fixture_remove): {}",
        io_source.matches("AT_REMOVEDIR").count() == 2 && io_source.contains("pub fn fixture_remove(dir: &LinuxDir")
    );
    println!(
        "PROBE R3-B2: maintenance names any removal of a successor root (remove_dir, rmdir, incomplete_successor): {}",
        ["remove_dir", "rmdir", "incomplete_successor"]
            .iter()
            .any(|token| maintenance_source.contains(token))
    );
    end(s);
}

/// R3-B3: as R3-B2, with a non-zero byte in the candidate successor's pool
/// file 0.
#[test]
fn r3b3_nonzero_incomplete_successor() {
    let fixture = Fixture::provisioned(2, 16);
    let layout = successor_layout(&fixture);
    let s = session(&fixture, "first");
    s.borrow_mut().verify(&SMALL).expect("the predecessor verifies");
    let mut procedure = maint::successor(Rc::clone(&s), &layout, &[]).expect("authorized");
    procedure.run(23, &mut quiet()).expect("operations 1 to 23");
    drop(procedure);
    crash(&fixture, s);
    let pool = fixture
        .world
        .lookup(&format!("{}/journals/{}", layout.state_root(), format::pool_name(0)))
        .expect("the successor's pool file 0");
    fixture.world.overwrite_visible(pool, 4096 * 3 + 17, &[0x5a]);
    for line in describe(&fixture, &layout.state_root()) {
        println!("PROBE R3-B3: successor {line}");
    }
    let found = maint::roots_with_history(
        &fixture.root_process("history"),
        &fixture.layout.parent,
    );
    println!("PROBE R3-B3: roots that may hold history: {found:?}");
    let s = session(&fixture, "second");
    let mut recovery = maint::leftover(Rc::clone(&s)).expect("R-LEFTOVER");
    println!(
        "PROBE R3-B3: R-LEFTOVER {:?}: {:?}",
        recovery.labels(),
        recovery.run_all(&mut quiet())
    );
    drop(recovery);
    println!(
        "PROBE R3-B3: the non-zero successor root still exists after every available procedure: {}",
        fixture.world.lookup(&layout.state_root()).is_some()
    );
    end(s);
}

/// R3-B4: a kept predecessor copy whose bytes differ (one byte) from the
/// PROVISION the session verifies, then an authorized succession.
#[test]
fn r3b4_wrong_kept_predecessor_copy() {
    let fixture = Fixture::provisioned(2, 16);
    let layout = successor_layout(&fixture);
    let (verified_bytes, verified) = provision_now(&fixture);
    let copy_name = fixture.path.predecessor_name(&verified.root_id);
    let mut wrong = verified_bytes.clone();
    let at = wrong
        .iter()
        .position(|byte| *byte == b'=')
        .expect("a key")
        + 1;
    wrong[at] ^= 0x01;
    let provdir = fixture.world.lookup(PROVDIR).expect("the PROVISION directory");
    let planted = fixture
        .world
        .fixture_entry(provdir, &copy_name, FileType::Regular, (0, 0, 0o444));
    fixture.world.install(planted, &wrong);
    let s = session(&fixture, "first");
    s.borrow_mut().verify(&SMALL).expect("the predecessor verifies");
    match maint::successor(Rc::clone(&s), &layout, &[]) {
        Err(why) => println!("PROBE R3-B4: the succession is refused at authorization: {why}"),
        Ok(mut procedure) => {
            fixture.world.trace_start();
            let result = procedure.run_all(&mut quiet());
            println!(
                "PROBE R3-B4: the succession: {result:?}; steps done {} of {}; operations {:?}",
                procedure.steps_done(),
                procedure.len(),
                ops(&fixture.world.trace_take())
            );
        }
    }
    println!(
        "PROBE R3-B4: the mismatched copy is unchanged (same inode, same bytes): {}",
        fixture.world.entries(provdir).get(&copy_name) == Some(&planted)
            && fixture.world.visible(planted) == wrong
    );
    println!(
        "PROBE R3-B4: the PROVISION directory now holds {:?}",
        fixture.world.entries(provdir).keys().collect::<Vec<_>>()
    );
    let (_, after) = provision_now(&fixture);
    println!(
        "PROBE R3-B4: PROVISION still selects root {} (revision {})",
        format::hex(&after.root_id),
        after.revision
    );
    end(s);
}

/// R3-B5: a revocation crashed after its operation 2 (the rename and
/// `fsync revoked/`), then power loss (F2) under the per-directory
/// over-approximation keeping only each directory's guaranteed halves.
#[test]
fn r3b5_interrupted_revocation_split_state() {
    let (fixture, binding) = store_with_incident();
    publish(&fixture, binding, "the owner process was destroyed");
    let s = session(&fixture, "revoke");
    let mut revocation = maint::revoke(Rc::clone(&s), binding, "20261002T120000Z");
    println!("PROBE R3-B5: P-REVOKE's steps: {:?}", revocation.labels());
    revocation.run(2, &mut quiet()).expect("operations 1 and 2");
    drop(revocation);
    drop(s);
    println!(
        "PROBE R3-B5: per-directory schedules for the pending log: {}",
        fixture.world.schedules(false).len()
    );
    fixture.world.power_loss(
        Some(&Schedule::PerDirectory(BTreeMap::new())),
        Tear::New,
        &BTreeMap::new(),
    );
    let root = fixture.state_root().join("/");
    let dispositions = format!("/{root}/dispositions");
    for line in describe(&fixture, &dispositions) {
        println!("PROBE R3-B5: {line}");
    }
    let active = fixture
        .world
        .lookup(&format!("{dispositions}/{}", format::disposition_name(&binding)));
    let revoked_name = format::revoked_name(&binding, "20261002T120000Z");
    let revoked = fixture
        .world
        .lookup(&format!("{dispositions}/revoked/{revoked_name}"));
    println!(
        "PROBE R3-B5: both names present: {}; the same inode: {}",
        active.is_some() && revoked.is_some(),
        active.is_some() && active == revoked
    );
    let opened = open_owner(
        &fixture.store_process("owner"),
        &fixture.path,
        &SMALL,
        &mut NoHooks,
    );
    println!(
        "PROBE R3-B5: an owner's opening: {}",
        match &opened {
            Ok(_) => "opens".to_string(),
            Err(refusal) => format!("{:?}", refusal.refused),
        }
    );
    drop(opened);
    let s = session(&fixture, "after");
    println!(
        "PROBE R3-B5: the in-session verification: {:?}",
        s.borrow_mut()
            .verify(&SMALL)
            .map(|_| ())
            .map_err(|refusal| refusal.refused)
    );
    let mut recovery = maint::leftover(Rc::clone(&s)).expect("R-LEFTOVER");
    println!(
        "PROBE R3-B5: R-LEFTOVER {:?}: {:?}",
        recovery.labels(),
        recovery.run_all(&mut quiet())
    );
    drop(recovery);
    println!(
        "PROBE R3-B5: after R-LEFTOVER the verification: {:?}",
        s.borrow_mut()
            .verify(&SMALL)
            .map(|_| ())
            .map_err(|refusal| refusal.refused)
    );
    let mut again = maint::revoke(Rc::clone(&s), binding, "20261002T130000Z");
    println!(
        "PROBE R3-B5: P-REVOKE again (another time): {:?}",
        again.run_all(&mut quiet())
    );
    drop(again);
    for line in describe(&fixture, &dispositions) {
        println!("PROBE R3-B5: after the second revocation {line}");
    }
    println!(
        "PROBE R3-B5: the verification after it: {:?}",
        s.borrow_mut()
            .verify(&SMALL)
            .map(|_| ())
            .map_err(|refusal| refusal.refused)
    );
    let maintenance_source = include_str!("support/custody/store/maintenance.rs");
    println!(
        "PROBE R3-B5: maintenance names a revocation recovery (resume_revocation, recover_revocation): {}",
        ["resume_revocation", "recover_revocation"]
            .iter()
            .any(|token| maintenance_source.contains(token))
    );
    end(s);
}

/// R3-B6: `dispositions/revoked/` holding 4096 accepted entries and one
/// valid active disposition; P-REVOKE.
#[test]
fn r3b6_revoked_directory_limit() {
    let (fixture, binding) = store_with_incident();
    publish(&fixture, binding, "the owner process was destroyed");
    let root = fixture.state_root().join("/");
    let revoked_dir = fixture
        .world
        .lookup(&format!("/{root}/dispositions/revoked"))
        .expect("revoked/");
    for k in 0..4096u32 {
        let mut other = [0xeeu8; 32];
        other[..4].copy_from_slice(&k.to_be_bytes());
        fixture.world.fixture_entry(
            revoked_dir,
            &format::revoked_name(&other, "20261001T000000Z"),
            FileType::Regular,
            (0, 0, 0o444),
        );
    }
    let s = session(&fixture, "revoke");
    println!(
        "PROBE R3-B6: the verification with 4096 revoked entries: {:?}",
        s.borrow_mut()
            .verify(&SMALL)
            .map(|report| report.current.len())
            .map_err(|refusal| refusal.refused)
    );
    fixture.world.trace_start();
    let mut revocation = maint::revoke(Rc::clone(&s), binding, "20261002T130000Z");
    let result = revocation.run_all(&mut quiet());
    println!(
        "PROBE R3-B6: P-REVOKE: {result:?}; complete {}; operations {:?}",
        revocation.is_complete(),
        ops(&fixture.world.trace_take())
    );
    drop(revocation);
    println!(
        "PROBE R3-B6: revoked/ now holds {} entries",
        fixture.world.entries(revoked_dir).len()
    );
    println!(
        "PROBE R3-B6: the session's last event: {:?}",
        s.borrow().events().last()
    );
    if let Some(assessment) = s.borrow().assessment() {
        println!(
            "PROBE R3-B6: the verify-after's conditions: {:?}",
            assessment
                .conditions
                .iter()
                .map(|condition| (condition.class, condition.name.clone()))
                .collect::<Vec<_>>()
        );
    }
    end(s);
    let opened = open_owner(
        &fixture.store_process("owner"),
        &fixture.path,
        &SMALL,
        &mut NoHooks,
    );
    println!(
        "PROBE R3-B6: an owner's opening afterwards: {}",
        match &opened {
            Ok(_) => "opens".to_string(),
            Err(refusal) => format!("{:?}", refusal.refused),
        }
    );
}

/// R3-B7: a revocation at a compact time whose revoked name already exists
/// for the same binding (an earlier revocation at that time).
#[test]
fn r3b7_revoked_target_collision() {
    let (fixture, binding) = store_with_incident();
    publish(&fixture, binding, "first publication");
    let at = "20261002T140000Z";
    let s = session(&fixture, "first-revoke");
    maint::revoke(Rc::clone(&s), binding, at)
        .run_all(&mut quiet())
        .expect("the first revocation");
    end(s);
    let root = fixture.state_root().join("/");
    let revoked_path = format!(
        "/{root}/dispositions/revoked/{}",
        format::revoked_name(&binding, at)
    );
    let first = fixture.world.lookup(&revoked_path).expect("the first revoked file");
    let first_bytes = fixture.world.visible(first);
    publish(&fixture, binding, "second publication");
    let s = session(&fixture, "second-revoke");
    fixture.world.trace_start();
    let mut revocation = maint::revoke(Rc::clone(&s), binding, at);
    let result = revocation.run_all(&mut quiet());
    println!(
        "PROBE R3-B7: the second revocation at the same time: {result:?}; operations {:?}",
        ops(&fixture.world.trace_take())
    );
    drop(revocation);
    end(s);
    let now = fixture.world.lookup(&revoked_path).expect("a revoked file");
    println!(
        "PROBE R3-B7: the revoked name now names the first revoked file: {}; its bytes are the first's: {}",
        now == first,
        fixture.world.visible(now) == first_bytes
    );
    let revoked_dir = fixture
        .world
        .lookup(&format!("/{root}/dispositions/revoked"))
        .expect("revoked/");
    let entries = fixture.world.entries(revoked_dir);
    println!(
        "PROBE R3-B7: revoked/ holds {} entr(y/ies); any of them is the first revoked file: {}",
        entries.len(),
        entries.values().any(|ino| *ino == first)
    );
    let text = String::from_utf8_lossy(&fixture.world.visible(now)).to_string();
    println!(
        "PROBE R3-B7: the remaining revoked file's statement is the first publication's: {}; the second's: {}",
        text.contains("first publication"),
        text.contains("second publication")
    );
}

/// R3-B8: the repeat behaviour of the base's administrative procedures
/// after F1 at every crash point of an authorized succession (one pool
/// file): a fresh session, R-LEFTOVER, an honest verification and the same
/// succession again. (The R3 recovery procedures do not exist at the base.)
#[test]
fn r3b8_succession_repeat_at_every_crash_point() {
    let probe = Fixture::provisioned(1, 16);
    let layout = successor_layout(&probe);
    let steps = {
        let s = session(&probe, "count");
        s.borrow_mut().verify(&SMALL).expect("verifies");
        let procedure = maint::successor(Rc::clone(&s), &layout, &[]).expect("authorized");
        let steps = procedure.len();
        drop(procedure);
        end(s);
        steps
    };
    println!("PROBE R3-B8: the succession has {steps} steps on one pool file");
    for k in 0..=steps {
        let fixture = Fixture::provisioned(1, 16);
        let s = session(&fixture, "first");
        s.borrow_mut().verify(&SMALL).expect("verifies");
        let mut procedure = maint::successor(Rc::clone(&s), &layout, &[]).expect("authorized");
        let ran = procedure.run(k, &mut quiet());
        drop(procedure);
        crash(&fixture, s);
        let (_, now) = provision_now(&fixture);
        if now.root_id != fixture.layout.root_id {
            println!(
                "PROBE R3-B8: crash point {k:2}: {ran:?}; PROVISION selects the successor (revision {}); no repeat",
                now.revision
            );
            continue;
        }
        let s = session(&fixture, "second");
        let leftover = maint::leftover(Rc::clone(&s))
            .map(|mut procedure| (procedure.len(), procedure.run_all(&mut quiet())));
        let verified = s.borrow_mut().verify(&SMALL).is_ok();
        let repeat = match maint::successor(Rc::clone(&s), &layout, &[]) {
            Err(why) => format!("refused at authorization: {why}"),
            Ok(mut repeat) => match repeat.run_all(&mut quiet()) {
                Ok(()) => format!("completed ({} steps)", repeat.len()),
                Err(why) => format!("failed: {why}"),
            },
        };
        println!(
            "PROBE R3-B8: crash point {k:2}: predecessor selected; R-LEFTOVER {:?}; verifies {verified}; repeat: {repeat}",
            leftover.map(|(len, result)| (len, result.is_ok()))
        );
        end(s);
    }
    let maintenance_source = include_str!("support/custody/store/maintenance.rs");
    println!(
        "PROBE R3-B8: the base has a successor-cleanup or revocation-recovery procedure to interrupt: {}",
        ["incomplete_successor", "resume_revocation"]
            .iter()
            .any(|token| maintenance_source.contains(token))
    );
}
