//! P2-V1-R3B-I3-I1-R2 baseline probes: the Architect's maintenance findings
//! B-S1 to B-S6, run against the exact base
//! ed7c7088247badf87b3b1d483ee57360f867e2f1 (tree
//! 08c20843335507210b9401239ad9c8472e980d66) through ordinary safe-Rust call
//! sites of the store's public API, from outside the store module. This file
//! is the only addition to an isolated copy of the base tree; every other
//! source is the base's blob. Each probe prints what it observed and asserts
//! only what it observed.

#![cfg(target_os = "linux")]

#[path = "support/custody/mod.rs"]
mod custody;

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use custody::store::classify::{classify_bytes, FileClass};
use custody::store::format::{
    self, encode_header, parse_provision, render_disposition, Disposition, HeaderFields,
    IncidentClass, IncidentFacts, IncidentKind, BLOCK,
};
use custody::store::io::FileType;
use custody::store::maintenance::{self as maint, Session, SessionEvent};
use custody::store::open::{open_owner, NoHooks, Refused};
use custody::store::owner::{start_owner, StartRefused, StoreOwner};
use custody::store::sim::{self, Fixture, SimIo};
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
    layout.state_name = format!(
        "uid-{}-{}",
        sim::STORE_UID,
        hex::encode(sim::SUCCESSOR_ID)
    );
    layout
}

fn session(fixture: &Fixture) -> Rc<RefCell<Session<SimIo>>> {
    match maint::begin(fixture.root_process("owner-session"), &fixture.path, false) {
        Ok(session) => Rc::new(RefCell::new(session)),
        Err(refusal) => panic!("a maintenance session: {refusal:?}"),
    }
}

fn end(session: Rc<RefCell<Session<SimIo>>>) {
    match Rc::try_unwrap(session) {
        Ok(session) => session.into_inner().end(),
        Err(_) => panic!("the session is still shared"),
    }
}

fn provision_now(fixture: &Fixture) -> format::Provision {
    let ino = fixture
        .world
        .lookup(sim::PROVISION_PATH)
        .expect("PROVISION");
    parse_provision(&fixture.world.visible(ino)).expect("PROVISION parses")
}

/// A header-only pool image (a valid header, no record): AbandonedClaim.
fn header_only(fixture: &Fixture, index: u32, claim: u64, generation: [u8; 16]) {
    let fields = HeaderFields {
        root_id: sim::ROOT_ID,
        generation,
        claim,
        pool_index: index,
        c_pool: fixture.layout.c_pool,
        capacity: 16,
        created_ms: 7,
        boot_id: [9; 16],
        applied: Vec::new(),
    };
    let mut image = vec![0u8; (fixture.layout.c_pool as usize + 2) * BLOCK];
    image[..BLOCK].copy_from_slice(&encode_header(&fields).expect("a header")[..]);
    fixture
        .world
        .install(fixture.pool_ino(index).expect("pool"), &image);
}

/// A disposition file for a claim gap of another root: it parses, its name
/// matches its binding, and its root is not this store's.
fn foreign_root_disposition(fixture: &Fixture) -> String {
    let other_root = [0x77u8; 16];
    let facts = IncidentFacts {
        kind: IncidentKind::ClaimGap,
        claim: Some(5),
        generation: None,
        pool_index: None,
        content: None,
        class: IncidentClass::Malformed,
        recorded_unsettled: 0,
    };
    let binding = facts.binding(&other_root).expect("a gap binding");
    let disposition = Disposition {
        root: other_root,
        binding,
        facts,
        reason: DispositionReason::Other,
        statement: "a disposition of another root".into(),
        operator: "probe".into(),
        at: "2026-10-02T00:00:00Z".into(),
    };
    let name = format::disposition_name(&binding);
    let dir = fixture.store_ino(&["dispositions"]).expect("dispositions/");
    let ino = fixture
        .world
        .fixture_entry(dir, &name, FileType::Regular, (0, 0, 0o444));
    fixture
        .world
        .install(ino, &render_disposition(&disposition).expect("renders"));
    name
}

fn events(session: &Rc<RefCell<Session<SimIo>>>) -> Vec<SessionEvent> {
    session.borrow().events().to_vec()
}

#[test]
fn bs1_unverified_succession() {
    let (fixture, binding) = store_with_incident();
    println!(
        "PROBE B-S1: the predecessor's unresolved, undispositioned incident {}",
        hex::encode(binding)
    );
    let s = session(&fixture);
    println!(
        "PROBE B-S1: session events before successor() (no verify): {:?}",
        events(&s)
    );
    let built = maint::successor(
        Rc::clone(&s),
        &successor_layout(&fixture),
        "predecessor fully dispositioned",
    );
    println!(
        "PROBE B-S1: successor() without any verification: {}",
        match &built {
            Ok(procedure) => format!("Ok, a procedure of {} steps", procedure.len()),
            Err(why) => format!("Err({why})"),
        }
    );
    let mut procedure = built.expect("B-S1: successor() refused");
    let ran = procedure.run_all(&mut |_| {});
    println!("PROBE B-S1: run_all: {ran:?}");
    drop(procedure);
    println!("PROBE B-S1: session events after: {:?}", events(&s));
    end(s);
    let selected = provision_now(&fixture);
    println!(
        "PROBE B-S1: PROVISION now selects root {} (revision {}), predecessor {:?}",
        hex::encode(selected.root_id),
        selected.revision,
        selected
            .predecessor
            .as_ref()
            .map(|(id, statement)| (hex::encode(id), statement.clone()))
    );
    let io = fixture.store_process("fresh-owner");
    let opened = open_owner(&io, &fixture.path, &SMALL, &mut NoHooks).expect("B-S1: opens");
    let decision = opened.decision().clone();
    println!(
        "PROBE B-S1: a fresh opening: root {}, current {}, blocking {}, the predecessor's binding present: {}",
        hex::encode(opened.selection().provision.root_id),
        decision.current.len(),
        decision.blocking.len(),
        decision.current.contains(&binding) || opened.scan().level.incidents.contains_key(&binding)
    );
    drop(opened);
    let owner = start_owner::<Token, _>(&io, &fixture.path, &SMALL, Tick(1), clock(), &mut NoHooks);
    println!(
        "PROBE B-S1: a fresh owner: {}",
        match &owner {
            Ok(owner) => format!(
                "STARTED (Ready): claim {} in pool file {} of root {}",
                owner.claim(),
                owner.index(),
                hex::encode(owner.report().root_id)
            ),
            Err(refused) => format!("refused {refused:?}"),
        }
    );
    drop(owner);
    let predecessor_pool = fixture.pool_ino(0).expect("the predecessor's pool file 0");
    let report = classify_bytes(
        &fixture.world.visible(predecessor_pool),
        sim::ROOT_ID,
        fixture.layout.c_pool,
        0,
    );
    println!(
        "PROBE B-S1: the predecessor's pool file 0, unchanged as evidence: class {:?}, refused {}",
        report.class, report.refused
    );
    assert!(ran.is_ok(), "B-S1 did not reproduce: the procedure failed");
    assert_eq!(selected.root_id, sim::SUCCESSOR_ID);
    assert_eq!(report.class, FileClass::UnsealedAction);
}

#[test]
fn bs2_freeform_statement() {
    let fixture = Fixture::provisioned(2, 16);
    let name = foreign_root_disposition(&fixture);
    let s = session(&fixture);
    let verified = s.borrow_mut().verify(&SMALL);
    println!(
        "PROBE B-S2: the predecessor's in-session verification: {:?}",
        verified.as_ref().map(|_| "Ok").map_err(|refusal| refusal.clone())
    );
    let statement = "predecessor fully dispositioned";
    let mut procedure = maint::successor(Rc::clone(&s), &successor_layout(&fixture), statement)
        .expect("B-S2: successor() refused");
    let ran = procedure.run_all(&mut |_| {});
    println!("PROBE B-S2: run_all: {ran:?}");
    drop(procedure);
    end(s);
    let selected = provision_now(&fixture);
    let persisted = selected
        .predecessor
        .as_ref()
        .map(|(_, statement)| statement.clone());
    println!(
        "PROBE B-S2: the successor's predecessor-statement: {persisted:?}; it names the condition on {name}: {}",
        persisted.as_deref().is_some_and(|text| text.contains(&name))
    );
    assert!(matches!(
        verified,
        Err(ref refusal) if matches!(refusal.refused, Refused::Invalid(_))
    ));
    assert!(ran.is_ok());
    assert_eq!(persisted.as_deref(), Some(statement));
}

#[test]
fn bs3_no_verify_after() {
    let fixture = Fixture::provisioned(2, 16);
    let s = session(&fixture);
    s.borrow_mut().verify(&SMALL).expect("B-S3: verify-before");
    let mut procedure = maint::successor(Rc::clone(&s), &successor_layout(&fixture), "clean")
        .expect("B-S3: successor() refused");
    let steps = procedure.len();
    procedure
        .run(steps - 1, &mut |_| {})
        .expect("B-S3: every step but the last");
    // A leftover temporary in the successor's journals directory, before the
    // last step: the successor no longer verifies.
    let journals = fixture
        .world
        .lookup(&format!(
            "/{}/{}/journals",
            fixture.layout.parent.join("/"),
            successor_layout(&fixture).state_name
        ))
        .expect("the successor's journals/");
    fixture
        .world
        .fixture_entry(journals, ".tmp-00000", FileType::Regular, (0, 0, 0o600));
    let last = procedure.run_all(&mut |_| {});
    println!(
        "PROBE B-S3: the last step (publication and re-selection): {last:?}; steps done {} of {}",
        procedure.steps_done(),
        procedure.len()
    );
    drop(procedure);
    let after = events(&s);
    println!("PROBE B-S3: session events: {after:?}");
    let reselect_at = after
        .iter()
        .rposition(|event| matches!(event, SessionEvent::Reselect(_)));
    let verified_after = reselect_at.is_some_and(|at| {
        after[at..]
            .iter()
            .any(|event| matches!(event, SessionEvent::Verify(_)))
    });
    println!(
        "PROBE B-S3: re-selection at event {reselect_at:?}; a verification after it: {verified_after}"
    );
    let now = s.borrow_mut().verify(&SMALL);
    println!(
        "PROBE B-S3: an in-session verification of the selected successor, run by the probe afterwards: {:?}",
        now.as_ref().map(|_| "Ok").map_err(|refusal| refusal.clone())
    );
    end(s);
    assert!(last.is_ok());
    assert!(!verified_after);
    assert!(now.is_err());
}

#[test]
fn bs4_stale_verification() {
    let (fixture, binding) = store_with_incident();
    // Publish the exact disposition.
    let s = session(&fixture);
    let report = s.borrow_mut().verify(&SMALL).expect("verifies");
    let (_, incident) = report
        .current
        .iter()
        .find(|(found, _)| *found == binding)
        .cloned()
        .expect("the incident");
    let disposition = maint::disposition_for(
        &sim::ROOT_ID,
        binding,
        &incident,
        DispositionReason::OwnerDestroyed,
        "the owner process was destroyed",
        "owner",
        "2026-10-02T12:00:00Z",
    );
    maint::publish_disposition(&s.borrow(), disposition)
        .expect("procedure")
        .run_all(&mut |_| {})
        .expect("published");
    end(s);
    let s = session(&fixture);
    let before = s.borrow_mut().verify(&SMALL).expect("verify-before");
    println!(
        "PROBE B-S4: verify-before: current {}, blocking {}, dispositioned {}",
        before.current.len(),
        before.blocking.len(),
        before.dispositioned.len()
    );
    let mut revocation = maint::revoke(&s.borrow(), binding, "20261002T130000Z");
    revocation.run_all(&mut |_| {}).expect("revoked");
    drop(revocation);
    // What the predecessor holds now, read directly from the simulated
    // directories (the session's own lock excludes every other opening).
    let dispositions = fixture.store_ino(&["dispositions"]).expect("dispositions/");
    let revoked = fixture
        .store_ino(&["dispositions", "revoked"])
        .expect("dispositions/revoked/");
    let name = format::disposition_name(&binding);
    let still_dispositioned = fixture.world.entries(dispositions).contains_key(&name);
    let revoked_entries: Vec<String> = fixture.world.entries(revoked).into_keys().collect();
    println!(
        "PROBE B-S4: after the revocation: dispositions/{name} present: {still_dispositioned}; revoked/: {revoked_entries:?}"
    );
    let built = maint::successor(Rc::clone(&s), &successor_layout(&fixture), "dispositioned");
    println!(
        "PROBE B-S4: successor() after a mutation that followed the verification: {}",
        match &built {
            Ok(procedure) => format!("Ok, {} steps", procedure.len()),
            Err(why) => format!("Err({why})"),
        }
    );
    let ran = built.expect("B-S4").run_all(&mut |_| {});
    println!("PROBE B-S4: run_all: {ran:?}");
    end(s);
    let io = fixture.store_process("fresh-owner");
    let owner = start_owner::<Token, _>(&io, &fixture.path, &SMALL, Tick(1), clock(), &mut NoHooks);
    println!(
        "PROBE B-S4: a fresh owner after the succession: {}",
        match &owner {
            Ok(owner) => format!("STARTED (Ready) on root {}", hex::encode(owner.report().root_id)),
            Err(refused) => format!("refused {refused:?}"),
        }
    );
    assert_eq!(before.blocking.len(), 0);
    assert!(ran.is_ok());
    assert!(owner.is_ok());
}

#[test]
fn bs5_capacity_report_loss() {
    // (a) More current incidents than incident_limit (4): four claim gaps
    // and one Malformed pool file.
    let fixture = Fixture::provisioned(3, 16);
    header_only(&fixture, 0, 1, [0x11; 16]);
    header_only(&fixture, 1, 6, [0x12; 16]);
    let malformed = vec![0xffu8; (fixture.layout.c_pool as usize + 2) * BLOCK];
    fixture
        .world
        .install(fixture.pool_ino(2).expect("pool"), &malformed);
    let s = session(&fixture);
    let verified = s.borrow_mut().verify(&SMALL);
    println!(
        "PROBE B-S5a: in-session verification of 4 claim gaps and 1 Malformed pool file (incident_limit 4): {:?}",
        verified.as_ref().map(|_| "Ok").map_err(|refusal| refusal.clone())
    );
    println!(
        "PROBE B-S5a: what the Err carries: Refusal {{ refused, revision }} only; no file classes, no incident bindings or facts, no disposition status, no history"
    );
    end(s);
    // (b) A claim-gap count above incident_limit plus the applied bindings:
    // refused before enumeration.
    let fixture = Fixture::provisioned(2, 16);
    header_only(&fixture, 0, 100, [0x21; 16]);
    let s = session(&fixture);
    let gaps = s.borrow_mut().verify(&SMALL);
    println!(
        "PROBE B-S5b: in-session verification of claim 100 alone (99 gaps): {:?}",
        gaps.as_ref().map(|_| "Ok").map_err(|refusal| refusal.clone())
    );
    end(s);
    assert!(matches!(
        verified,
        Err(ref refusal) if matches!(refusal.refused, Refused::Capacity(_))
    ));
    assert!(matches!(
        gaps,
        Err(ref refusal) if matches!(refusal.refused, Refused::Capacity(_))
    ));
}

#[test]
fn bs6_invalid_report_loss() {
    let fixture = Fixture::provisioned(2, 16);
    // Two independent store-level Invalid conditions: a disposition of
    // another root (scan step 7) and a duplicate generation (step 8).
    let name = foreign_root_disposition(&fixture);
    header_only(&fixture, 0, 1, [0x31; 16]);
    header_only(&fixture, 1, 2, [0x31; 16]);
    let s = session(&fixture);
    let first = s.borrow_mut().verify(&SMALL);
    println!(
        "PROBE B-S6: in-session verification with both conditions: {:?}",
        first.as_ref().map(|_| "Ok").map_err(|refusal| refusal.clone())
    );
    end(s);
    let dir = fixture.store_ino(&["dispositions"]).expect("dispositions/");
    fixture.world.fixture_remove(dir, &name);
    let s = session(&fixture);
    let second = s.borrow_mut().verify(&SMALL);
    println!(
        "PROBE B-S6: the same store with the first condition removed: {:?}",
        second.as_ref().map(|_| "Ok").map_err(|refusal| refusal.clone())
    );
    end(s);
    assert!(matches!(
        first,
        Err(ref refusal) if matches!(&refusal.refused, Refused::Invalid(why) if why.contains("root"))
    ));
    assert!(matches!(
        second,
        Err(ref refusal) if matches!(&refusal.refused, Refused::Invalid(why) if why.contains("duplicate generation"))
    ));
}
