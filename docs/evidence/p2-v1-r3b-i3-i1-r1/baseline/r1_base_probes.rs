//! P2-V1-R3B-I3-I1-R1 baseline probes: the Architect's counterexamples
//! B1 to B6, run against the exact base 72ffc4fcc0141e2e5ae77a927481017ca711ab7c
//! through ordinary safe-Rust call sites of the store's public API, from
//! outside the store module. This file is the only addition to an isolated
//! copy of the base tree; every other source is the base's blob. Each probe
//! prints what it observed and asserts that the counterexample reproduces.

#![cfg(target_os = "linux")]

#[path = "support/custody/mod.rs"]
mod custody;

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use custody::store::classify::{self, classify_bytes};
use custody::store::disposition::StoreValidator;
use custody::store::exchange::{Cause, ClaimState, SealState};
use custody::store::format::BLOCK;
use custody::store::maintenance;
use custody::store::open::{open_owner, NoHooks};
use custody::store::recorder::{start_owner, OwnerStart, StartRefused};
use custody::store::sim::{self, Fixture, SimIo};
use custody::*;

#[derive(Debug)]
struct Token(SlotKind);

impl Resource for Token {
    fn kind(&self) -> SlotKind {
        self.0
    }
}

struct Confirm;

impl Cleanup<Token> for Confirm {
    fn attempt(&mut self, _owner: &mut Token, _entry: &EntryView) -> CleanupReport {
        CleanupReport {
            subtree: Observed::Confirmed,
            reaped: Observed::Confirmed,
            output: OutputObserved::Complete,
            removed: Observed::Confirmed,
        }
    }
}

const fn config(record_capacity: u32) -> Config {
    Config {
        automatic_attempts: 2,
        recovery_budget: 4,
        recovery_spacing_millis: 1,
        lease_millis: 1_000_000,
        record_capacity,
        control_reserve: 2,
        receipt_limit: 4,
        late_limit: 1,
        failure_detail_limit: 8,
        detail_chars: 64,
        incident_limit: 4,
    }
}

const WIDE: Config = config(64);
const SMALL: Config = config(16);

fn clock() -> Arc<dyn Fn() -> Tick + Send + Sync> {
    let counter = Arc::new(AtomicU64::new(0));
    Arc::new(move || Tick(counter.fetch_add(1, Ordering::SeqCst)))
}

/// One owner through the base's public API.
struct Probe {
    s: OwnerStart<Token, SimIo>,
    owner: SimIo,
    now: u64,
}

impl Probe {
    fn start(fixture: &Fixture, name: &str, config: Config) -> Probe {
        let owner = fixture.store_process(name);
        let mut s =
            start_owner::<Token, _>(&owner, &fixture.path, &config, Tick(1), clock(), &mut NoHooks)
                .expect("the owner starts");
        s.worker.run_until_idle();
        assert_eq!(s.recorder.claim_state(), ClaimState::Claimed);
        Probe { s, owner, now: 10 }
    }

    fn tick(&mut self) -> Tick {
        self.now += 1;
        Tick(self.now)
    }

    fn pump(&mut self) {
        for _ in 0..16 {
            let now = self.tick();
            let _ = self.s.recorder.flush(&mut self.s.custody, now);
            self.s.worker.run_until_idle();
            let now = self.tick();
            self.s.recorder.apply(&mut self.s.custody, now);
        }
    }

    /// A clean, passed, finalized run with one admitted process action.
    fn clean_pass(&mut self) -> OpTicket {
        let now = self.tick();
        self.s.custody.start_run(now).expect("the run starts");
        self.pump();
        let now = self.tick();
        self.s
            .custody
            .begin_case(CaseId(1), Expectation::Clean, now)
            .expect("the case begins");
        self.pump();
        let now = self.tick();
        let reservation = self.s.custody.reserve(SlotKind::Process, now).expect("reserved");
        self.pump();
        let now = self.tick();
        let ticket = self
            .s
            .recorder
            .admit(&mut self.s.custody, reservation, now)
            .expect("admitted through the gate");
        let now = self.tick();
        assert!(matches!(
            self.s
                .custody
                .complete(&ticket, NativeOutcome::Created(Token(SlotKind::Process)), now),
            Ok(Completion::Deposited(_))
        ));
        let now = self.tick();
        let mut clean = Confirm;
        self.s.custody.end_case(&mut clean, now).expect("the case ends");
        self.s.custody.take_released();
        self.pump();
        let now = self.tick();
        let _ = self.s.custody.finish_run(now);
        self.pump();
        assert_eq!(self.s.custody.snapshot().phase, RunPhase::Finalized);
        ticket
    }
}

/// A `Closed` value assembled from public fields.
fn fabricated_closed(records: u64) -> Closed<Token> {
    Closed {
        terminal: None,
        verdict: Verdict::Passed,
        resolved_by: None,
        records,
        failures: Vec::new(),
        failure_counts: [0; FailureClass::COUNT],
        failure_overflow: 0,
        faults: Vec::new(),
        fault_counts: [0; FailureClass::COUNT],
        fault_overflow: 0,
        released: Vec::new(),
    }
}

/// Kill the owner, then start the next one: Ready or refused.
fn restart(fixture: &Fixture, probe: Probe, config: Config, label: &str) -> String {
    let index = probe.s.index;
    fixture.world.kill(&probe.owner);
    drop(probe);
    let pool = fixture.pool_ino(index).expect("pool");
    let report = classify_bytes(
        &fixture.world.visible(pool),
        sim::ROOT_ID,
        fixture.layout.c_pool,
        index,
    );
    let next = fixture.store_process(&format!("{label}-next"));
    let outcome =
        match start_owner::<Token, _>(&next, &fixture.path, &config, Tick(1), clock(), &mut NoHooks) {
            Ok(_) => "the next owner STARTED (Ready): the generation is not refused".to_string(),
            Err(StartRefused::PriorUnresolved(blocking)) => {
                format!("the next owner was refused: PriorUnresolved ({})", blocking.len())
            }
            Err(other) => format!("the next owner was refused: {other:?}"),
        };
    format!(
        "pool file {index}: class {:?}, refused {}; {outcome}",
        report.class, report.refused
    )
}

#[test]
fn b1_fabricated_closure_data() {
    let fixture = Fixture::provisioned(2, 64);
    let mut p = Probe::start(&fixture, "b1", WIDE);
    let ticket = p.clean_pass();
    let now = p.tick();
    let late = p
        .s
        .custody
        .complete(&ticket, NativeOutcome::Created(Token(SlotKind::Process)), now);
    println!("PROBE B1: late owner delivered after the terminal record: {:?}", late.as_ref().map(|c| format!("{c:?}")).map_err(|_| "rejected"));
    let status = p.s.recorder.status();
    let snapshot = p.s.custody.snapshot();
    println!(
        "PROBE B1: custody phase {:?}, records issued {}, acknowledged {}, unsent {}, held entries {}; recorder durable_through {}, submitted_through {}",
        snapshot.phase,
        snapshot.evidence.issued,
        snapshot.evidence.acknowledged,
        snapshot.evidence.unsent,
        snapshot.entries.len(),
        status.durable_through,
        status.submitted_through
    );
    let durable = status.durable_through;
    // The actual custody refuses closure and keeps the late owner.
    let custody = std::mem::replace(
        &mut p.s.custody,
        Custody::<Token>::new(WIDE, Generation::new([0x5e; 16]), Vec::new(), Tick(1))
            .expect("a placeholder custody")
            .0,
    );
    let actual = custody.close(p.tick());
    let refused = actual.is_err();
    match actual {
        Ok(_) => println!("PROBE B1: the actual custody CLOSED"),
        Err(back) => {
            let snapshot = back.snapshot();
            println!(
                "PROBE B1: the actual custody REFUSED closure: shutdown decision {:?}; held entries {}",
                back.shutdown_decision(),
                snapshot.entries.len()
            );
            p.s.custody = *back;
        }
    }
    // Fabricated closure data with the old durable count.
    let fabricated = fabricated_closed(durable);
    let accepted = p.s.recorder.request_seal(&p.s.header, &fabricated, 0);
    println!("PROBE B1: request_seal with fabricated Closed {{ records: {durable} }}: {accepted:?}");
    p.s.worker.run_until_idle();
    let seal = p.s.recorder.seal_state();
    println!("PROBE B1: seal state after the worker ran: {}", match &seal {
        SealState::Sealed => "Sealed (the worker wrote the seal)".to_string(),
        other => format!("{other:?}"),
    });
    let outcome = restart(&fixture, p, WIDE, "b1");
    println!("PROBE B1: restart: {outcome}");
    assert!(refused, "the actual custody must refuse");
    assert!(
        accepted.is_ok() && seal == SealState::Sealed,
        "B1 did not reproduce: the fabricated closure was not accepted"
    );
}

#[test]
fn b2_foreign_closure() {
    // This store's owner: finalized, then a late owner it still holds.
    let fixture = Fixture::provisioned(2, 64);
    let mut a = Probe::start(&fixture, "b2-a", WIDE);
    let ticket = a.clean_pass();
    let now = a.tick();
    let _ = a
        .s
        .custody
        .complete(&ticket, NativeOutcome::Created(Token(SlotKind::Process)), now);
    let a_records = a.s.recorder.status().durable_through;
    // Another custody, in another store, with the same record count, closed
    // for real.
    let other_fixture = Fixture::provisioned(2, 64);
    let mut b = Probe::start(&other_fixture, "b2-b", WIDE);
    b.clean_pass();
    let b_custody = std::mem::replace(
        &mut b.s.custody,
        Custody::<Token>::new(WIDE, Generation::new([0x5f; 16]), Vec::new(), Tick(1))
            .expect("a placeholder custody")
            .0,
    );
    let genuine = match b_custody.close(b.tick()) {
        Ok(closed) => closed,
        Err(back) => panic!("the other custody did not close: {:?}", back.shutdown_decision()),
    };
    println!(
        "PROBE B2: a genuine Closed from another custody (generation {:?}): records {}, verdict {:?}; this recorder's durable_through {}",
        b.s.generation, genuine.records, genuine.verdict, a_records
    );
    let accepted = a.s.recorder.request_seal(&a.s.header, &genuine, 0);
    println!("PROBE B2: this recorder's request_seal with the foreign Closed: {accepted:?}");
    a.s.worker.run_until_idle();
    let seal = a.s.recorder.seal_state();
    println!("PROBE B2: seal state after the worker ran: {seal:?}");
    let outcome = restart(&fixture, a, WIDE, "b2");
    println!("PROBE B2: restart: {outcome}");
    assert!(
        genuine.records == a_records && accepted.is_ok() && seal == SealState::Sealed,
        "B2 did not reproduce"
    );
}

/// An owner that admitted an action and died: one unresolved incident, and
/// pool file 1 still unused.
fn store_with_incident() -> Fixture {
    let fixture = Fixture::provisioned(2, 16);
    let mut p = Probe::start(&fixture, "dying", SMALL);
    let now = p.tick();
    p.s.custody.start_run(now).expect("the run starts");
    p.pump();
    let now = p.tick();
    p.s.custody
        .begin_case(CaseId(1), Expectation::Clean, now)
        .expect("the case begins");
    p.pump();
    let now = p.tick();
    let reservation = p.s.custody.reserve(SlotKind::Process, now).expect("reserved");
    p.pump();
    let now = p.tick();
    let ticket = p
        .s
        .recorder
        .admit(&mut p.s.custody, reservation, now)
        .expect("admitted");
    let now = p.tick();
    let _ = p
        .s
        .custody
        .complete(&ticket, NativeOutcome::Created(Token(SlotKind::Process)), now);
    fixture.world.kill(&p.owner);
    drop(p);
    fixture
}

#[test]
fn b3_edited_refusal_decision() {
    let fixture = store_with_incident();
    let refused = start_owner::<Token, _>(
        &fixture.store_process("unedited"),
        &fixture.path,
        &SMALL,
        Tick(1),
        clock(),
        &mut NoHooks,
    );
    let unedited = match &refused {
        Err(StartRefused::PriorUnresolved(blocking)) => {
            format!("refused PriorUnresolved ({} blocking)", blocking.len())
        }
        Err(other) => format!("refused {other:?}"),
        Ok(_) => "STARTED".into(),
    };
    drop(refused);
    println!("PROBE B3: the unedited path: {unedited}");
    let io = fixture.store_process("editor");
    let mut opened = open_owner(&io, &fixture.path, &SMALL, &mut NoHooks).expect("opens");
    println!(
        "PROBE B3: the returned decision: current {}, blocking {}, dispositioned {}; an unused pool file: {}",
        opened.decision.current.len(),
        opened.decision.blocking.len(),
        opened.decision.dispositioned.len(),
        opened
            .scan
            .files
            .iter()
            .any(|file| file.class == classify::FileClass::Unused)
    );
    opened.decision.blocking.clear();
    opened.decision.current.clear();
    let generation = opened.draw_generation(&io).expect("a generation");
    let claimed = opened.claim(&io, &SMALL, generation, &[]);
    match &claimed {
        Ok(claim) => println!(
            "PROBE B3: after clearing blocking and current, claim ACCEPTED: pool file {}, claim {}, the header lists {} applied incidents",
            claim.index,
            claim.claim,
            claim.header.applied.len()
        ),
        Err(refusal) => println!("PROBE B3: claim refused: {refusal:?}"),
    }
    assert!(
        unedited.starts_with("refused PriorUnresolved") && claimed.is_ok(),
        "B3 did not reproduce"
    );
}

#[test]
fn b4_rebound_storage_admission() {
    // Two independent openings of two stores.
    let fixture_a = Fixture::provisioned(2, 16);
    let fixture_b = Fixture::provisioned(2, 16);
    let io_a = fixture_a.store_process("a");
    let io_b = fixture_b.store_process("b");
    let mut opened_a = open_owner(&io_a, &fixture_a.path, &SMALL, &mut NoHooks).expect("A opens");
    let opened_b = open_owner(&io_b, &fixture_b.path, &SMALL, &mut NoHooks).expect("B opens");
    println!(
        "PROBE B4: A opening {:?} digest {}; B admission opening {:?} digest {}",
        opened_a.opening,
        hex::encode(opened_a.selection.digest),
        opened_b.admission().opening(),
        hex::encode(opened_b.admission().provision_digest())
    );
    opened_a.opening = opened_b.admission().opening();
    opened_a.selection.digest = opened_b.admission().provision_digest();
    let generation = opened_a.draw_generation(&io_a).expect("a generation");
    let claimed =
        opened_a.claim_presenting(&io_a, Some(opened_b.admission()), &SMALL, generation, &[]);
    match &claimed {
        Ok(claim) => println!(
            "PROBE B4: A's claim presenting B's admission after the edit: ACCEPTED (pool file {}, claim {})",
            claim.index, claim.claim
        ),
        Err(refusal) => println!("PROBE B4: refused: {refusal:?}"),
    }
    assert!(claimed.is_ok(), "B4 did not reproduce");
}

#[test]
fn b5_fabricated_disposition_provenance() {
    let fixture = store_with_incident();
    let io = fixture.store_process("reader");
    let opened = open_owner(&io, &fixture.path, &SMALL, &mut NoHooks).expect("opens");
    let (binding, incident) = opened.current().into_iter().next().expect("one incident");
    let dispositions_dir = fixture
        .store_ino(&["dispositions"])
        .expect("the dispositions directory");
    let published: Vec<String> = fixture
        .world
        .entries(dispositions_dir)
        .into_keys()
        .filter(|name| name != "revoked")
        .collect();
    println!("PROBE B5: disposition files the store holds: {published:?}");
    let fabricated = maintenance::disposition_for(
        &sim::ROOT_ID,
        binding,
        &incident,
        DispositionReason::OwnerDestroyed,
        "fabricated, never published",
        "nobody",
        "2026-10-02T00:00:00Z",
    );
    let comparison = classify::disposition_matches(Some(&fabricated), &incident);
    println!("PROBE B5: pure data comparison (disposition_matches): {comparison}");
    let mut edited = opened.scan.clone();
    edited.dispositions.insert(binding, fabricated);
    let validator = StoreValidator::new(&sim::ROOT_ID, &edited);
    let validated = validator.validate(&IncidentBinding::new(binding));
    println!("PROBE B5: StoreValidator built from an edited scan: {validated:?}");
    let (mut custody, _control) =
        Custody::<Token>::new(SMALL, Generation::new([0x7b; 16]), opened.priors(), Tick(1))
            .expect("a custody with the prior");
    let blocked = custody.start_run(Tick(2));
    let applied = custody.apply_disposition(&IncidentBinding::new(binding), &validator, Tick(3));
    let started = custody.start_run(Tick(4));
    println!(
        "PROBE B5: before: start_run {:?}; apply_disposition with that validator: {applied:?}; then start_run: {:?}",
        blocked.map(|_| "started"),
        started.map(|_| "started")
    );
    assert!(
        published.is_empty() && validated.is_some() && applied.is_ok(),
        "B5 did not reproduce"
    );
}

#[test]
fn b6_writable_exchange_state() {
    let fixture = Fixture::provisioned(2, 16);
    let mut p = Probe::start(&fixture, "b6", SMALL);
    let exchange = Arc::clone(p.s.recorder.exchange());
    // The fatal latch, cleared from outside.
    exchange.latch(Cause::WorkerLost);
    let latched = p.s.recorder.latched();
    {
        let mut held = exchange.acquire();
        held.state.fatal = None;
    }
    let cleared = p.s.recorder.latched();
    println!("PROBE B6: fatal latch via Recorder::exchange().acquire().state.fatal: {latched:?} -> {cleared:?}");
    // durable_through, set with nothing written: the core is told record 1
    // is durable.
    let now = p.tick();
    p.s.custody.start_run(now).expect("the run starts");
    let now = p.tick();
    let _ = p.s.recorder.flush(&mut p.s.custody, now);
    {
        let mut held = exchange.acquire();
        held.state.durable_through = 1;
    }
    let now = p.tick();
    p.s.recorder.apply(&mut p.s.custody, now);
    let pool = fixture.pool_ino(p.s.index).expect("pool");
    let block_written = fixture.world.visible(pool)[2 * BLOCK..3 * BLOCK]
        .iter()
        .any(|byte| *byte != 0);
    let acknowledged = p.s.custody.snapshot().evidence.acknowledged;
    println!(
        "PROBE B6: durable_through set to 1 from outside: the core acknowledged {acknowledged} record(s); record 1's block written: {block_written}"
    );
    // The claim state.
    let claim_before = p.s.recorder.claim_state();
    {
        let mut held = exchange.acquire();
        held.state.claim = ClaimState::Failed;
    }
    let claim_after = p.s.recorder.claim_state();
    println!("PROBE B6: claim state via the guard: {claim_before:?} -> {claim_after:?}");
    // The seal state, with no seal written.
    {
        let mut held = exchange.acquire();
        held.state.seal = SealState::Sealed;
    }
    let seal_block_written = fixture.world.visible(pool)[BLOCK..2 * BLOCK]
        .iter()
        .any(|byte| *byte != 0);
    println!(
        "PROBE B6: seal state via the guard: {:?}; seal block written: {seal_block_written}",
        p.s.recorder.seal_state()
    );
    println!("PROBE B6: the public call sites: Recorder::exchange() -> &Arc<Exchange>; Exchange::acquire() -> Held {{ pub state: MutexGuard<ExchangeState> }}; ExchangeState's pub fields fatal, durable_through, claim, seal, submitted_through, pending, worker_exit, stop; Exchange::latch; Exchange::fixture_poison; and the constructors Exchange::new, Recorder::new, Worker::new, WorkerHandle::new");
    assert!(latched.is_some() && cleared.is_none(), "B6: the latch was not cleared");
    assert!(acknowledged == 1 && !block_written, "B6: no acknowledgement was manufactured");
    assert_eq!(claim_after, ClaimState::Failed);
    assert!(p.s.recorder.seal_state() == SealState::Sealed && !seal_block_written);
}
