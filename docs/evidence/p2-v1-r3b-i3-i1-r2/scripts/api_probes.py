#!/usr/bin/env python3
"""P2-V1-R3B-I3-I1-R2 compile-time API probes: the store's authority
boundary, checked from outside it.

Adapted from docs/evidence/p2-v1-r3b-i3-i1-r1/scripts/api_probes.py (which
stays unchanged); api_probes.adaptation.diff is the difference. R1's probes
keep their ids and intentions: P-SESSION-VERIFIED reopens the R2 field
(the session now retains a bound `Verification`), and the positive control
uses the R2 procedure interface. R2 adds three guards of the session's
retained verification: it cannot be moved between sessions
(P-VERIFICATION-TRANSPLANT, the intention of NC-SUCC-FOREIGN), built
outside the store (P-VERIFICATION-FORGE), or taken by a caller
(P-TAKE-ASSESSMENT).

Each probe is a small test target, written into a scratch checkout as
crates/nexus-verifier-sandbox/tests/r2_api_probe.rs. It reaches the store
only as an external caller does, through `custody::store::...` paths, and
attempts one of the authority paths the R1 and R2 repairs close. Each
counted probe must:

- fail to build, with exactly its expected error lines (the distinct
  `error[...]` header lines of the build, "could not compile" excluded):
  the failure is the intended privacy or type guard and nothing else;
- self-check: once the guard is reopened in the scratch checkout's store
  sources (each reopen edit's anchor occurs exactly once), the same probe
  builds with no error. The sources are then restored byte for byte and
  verified by SHA-256.

Two probes are not visibility guards and are labelled so: the one-way
closure (a moved owner cannot be closed twice, E0382; its self-check is the
same probe closing the owner that a refusal returned, which builds), and
the harness check (a deliberate E0308, proving the harness sees errors).
A positive control builds the public API's ordinary use with no error.

These are API/type guards. They are reported apart from the behavioural
controls and never added into one figure with them. They prove the safe
API surface only; they do not defend against `unsafe` code.

Usage:
  api_probes.py <scratch checkout> <log directory>

The scratch checkout must hold exactly the candidate's tree (it is never
the worktree under review). CARGO_TARGET_DIR should name a target directory
for the scratch builds.
"""
import hashlib
import json
import os
import pathlib
import subprocess
import sys

TESTS = "crates/nexus-verifier-sandbox/tests"
STORE = f"{TESTS}/support/custody/store"
PROBE = f"{TESTS}/r2_api_probe.rs"
OWN = f"{STORE}/owner.rs"
EXC = f"{STORE}/exchange.rs"
OPN = f"{STORE}/open.rs"
DSP = f"{STORE}/disposition.rs"
REC = f"{STORE}/recorder.rs"
MNT = f"{STORE}/maintenance.rs"
SOURCES = [OWN, EXC, OPN, DSP, REC, MNT, f"{STORE}/faults.rs", f"{STORE}/mod.rs"]

HEADER = r'''//! P2-V1-R3B-I3-I1-R2 API probe: an external caller of the store.
#![cfg(target_os = "linux")]
#![allow(unused, dead_code, unreachable_code, private_interfaces)]

#[path = "support/custody/mod.rs"]
mod custody;

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use custody::store::maintenance as maint;
use custody::store::open::{open_owner, NoHooks};
use custody::store::owner::{start_owner, ClosedStore, StoreOwner};
use custody::store::sim::{Fixture, SimIo};
use custody::*;

#[derive(Debug)]
struct Token(SlotKind);

impl Resource for Token {
    fn kind(&self) -> SlotKind {
        self.0
    }
}

fn clock() -> Arc<dyn Fn() -> Tick + Send + Sync> {
    Arc::new(|| Tick(0))
}

fn owner(fixture: &Fixture, config: &Config) -> StoreOwner<Token, SimIo> {
    let io = fixture.store_process("probe");
    match start_owner(&io, &fixture.path, config, Tick(1), clock(), &mut NoHooks) {
        Ok(owner) => owner,
        Err(_) => panic!("never run"),
    }
}

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

#[allow(clippy::all)]
fn probe(fixture: &Fixture, config: &Config) {
'''

FOOTER = r'''
}

#[test]
fn builds() {}
'''


def probe(pid, category, boundary, base, attempt, body, expect, reopen, why):
    return dict(id=pid, category=category, boundary=boundary, base=base,
                attempt=attempt, body=body, expect=list(expect),
                reopen=[dict(file=f, anchor=a, replacement=r) for (f, a, r) in reopen],
                why=why)


PROBES = [
    probe(
        "P-B1-CLOSURE-DATA", "api-type-guard", "closure to seal",
        "B1: Recorder::request_seal(&header, &fabricated_closed, 0) sealed a journal whose custody refused closure",
        "request a seal with fabricated closure data through the owner's recorder",
        r'''    let owner = owner(fixture, config);
    let fabricated = fabricated_closed(6);
    let _ = owner.recorder.request_seal(owner.header(), fabricated.records, 0);''',
        ["error[E0616]: field `recorder` of struct `StoreOwner` is private"],
        [(OWN, "    pub(super) control: Control,\n    pub(super) recorder: Recorder,",
          "    pub(super) control: Control,\n    pub recorder: Recorder,"),
         (EXC, "pub(super) struct Recorder {", "pub struct Recorder {"),
         (EXC, "    pub(super) fn request_seal(", "    pub fn request_seal(")],
        "the owner's recorder is a private field; only StoreOwner::close requests the seal, from the actual closure",
    ),
    probe(
        "P-B2-CUSTODY-SWAP", "api-type-guard", "closure to seal",
        "B2: another custody's genuine closure sealed this journal",
        "exchange the custodies of two owners, so that one closure seals the other's journal",
        r'''    let mut a = owner(fixture, config);
    let mut b = owner(fixture, config);
    std::mem::swap(&mut a.custody, &mut b.custody);''',
        ["error[E0616]: field `custody` of struct `StoreOwner` is private"],
        [(OWN, "    pub(super) custody: Custody<R>,", "    pub custody: Custody<R>,")],
        "the custody is retained with its recorder, header and guard in one value; there is no &mut Custody",
    ),
    probe(
        "P-B2-HEADER-SWAP", "api-type-guard", "closure to seal",
        "B2: the seal was requested with a caller's header",
        "replace the claimed header an owner seals with",
        r'''    let mut owner = owner(fixture, config);
    let header = owner.header().clone();
    owner.header = header;''',
        ["error[E0616]: field `header` of struct `StoreOwner` is private"],
        [(OWN, "    pub(super) guard: Arc<StoreGuard<P>>,\n    pub(super) header: Header,\n    pub(super) claim: u64,",
          "    pub(super) guard: Arc<StoreGuard<P>>,\n    pub header: Header,\n    pub(super) claim: u64,")],
        "the claimed header is retained internally",
    ),
    probe(
        "P-SEAL-AGAIN", "api-type-guard", "one-way seal",
        "(R1) a closed store requesting a second seal",
        "request a seal again through a closed store's recorder",
        r'''    let owner = owner(fixture, config);
    if let Ok(closed) = owner.close(Tick(2)) {
        let _ = closed.recorder.request_seal(closed.header(), 0, 0);
    }''',
        ["error[E0616]: field `recorder` of struct `ClosedStore` is private"],
        [(OWN, "pub struct ClosedStore<R, P: Platform> {\n    pub(super) recorder: Recorder,",
          "pub struct ClosedStore<R, P: Platform> {\n    pub recorder: Recorder,"),
         (EXC, "pub(super) struct Recorder {", "pub struct Recorder {"),
         (EXC, "    pub(super) fn request_seal(", "    pub fn request_seal(")],
        "a closed store has no close and no seal request; its recorder is private (and refuses AlreadyRequested)",
    ),
    probe(
        "P-B3-DECISION-EDIT", "api-type-guard", "opening immutability",
        "B3: opened.decision.blocking.clear() then opened.claim(...) accepted",
        "clear the opening's blocking incidents in place",
        r'''    let io = fixture.store_process("probe");
    let mut opened = open_owner(&io, &fixture.path, config, &mut NoHooks).expect("opens");
    opened.decision.blocking.clear();''',
        ["error[E0616]: field `decision` of struct `Opened` is private"],
        [(OPN, "    scan: ScanResult,\n    decision: Decision,\n    admission: StorageAdmission,\n}",
          "    scan: ScanResult,\n    pub decision: Decision,\n    admission: StorageAdmission,\n}")],
        "the opening's decision is retained; a caller gets only a borrow or a copy",
    ),
    probe(
        "P-B3-SCAN-EDIT", "api-type-guard", "opening immutability",
        "B3/B5: the opening's scan was a public field",
        "remove the opening's verified incidents in place",
        r'''    let io = fixture.store_process("probe");
    let mut opened = open_owner(&io, &fixture.path, config, &mut NoHooks).expect("opens");
    opened.scan.level.incidents.clear();''',
        ["error[E0616]: field `scan` of struct `Opened` is private"],
        [(OPN, "    scan: ScanResult,\n    decision: Decision,\n    admission: StorageAdmission,\n}",
          "    pub scan: ScanResult,\n    decision: Decision,\n    admission: StorageAdmission,\n}")],
        "the verified scan is retained; editing a copy changes nothing",
    ),
    probe(
        "P-B3-CLAIM-CALL", "api-type-guard", "opening immutability",
        "B3: Opened::claim(io, config, generation, applied) took the caller's applied list",
        "claim through an opening from outside the store",
        r'''    let io = fixture.store_process("probe");
    let opened = open_owner(&io, &fixture.path, config, &mut NoHooks).expect("opens");
    let _ = opened.claim(&io, config, Generation::new([1; 16]));''',
        ["error[E0624]: method `claim` is private"],
        [(OPN, "    pub(super) fn claim(\n        self,", "    pub fn claim(\n        self,"),
         (OPN, "pub(super) struct Claim<P: Platform> {", "pub struct Claim<P: Platform> {")],
        "only start_owner claims, through the opening's own claim, which derives what it needs internally (its result type is internal too)",
    ),
    probe(
        "P-B4-IDENTITY-EDIT", "api-type-guard", "admission binding",
        "B4: opened.opening and opened.selection.digest were overwritten with another opening's",
        "overwrite the opening's identity and selection digest",
        r'''    let io = fixture.store_process("probe");
    let mut a = open_owner(&io, &fixture.path, config, &mut NoHooks).expect("A");
    let b = open_owner(&io, &fixture.path, config, &mut NoHooks).expect("B");
    a.opening = b.admission().opening();
    a.selection.digest = b.admission().provision_digest();''',
        ["error[E0616]: field `opening` of struct `Opened` is private",
         "error[E0616]: field `selection` of struct `Opened` is private"],
        [(OPN, "pub struct Opened<P: Platform> {\n    opening: OpeningId,\n    selection: Selection,",
          "pub struct Opened<P: Platform> {\n    pub opening: OpeningId,\n    pub selection: Selection,")],
        "the opening's identity and revalidated selection are retained",
    ),
    probe(
        "P-B4-OPENING-MINT", "api-type-guard", "admission binding",
        "B4: OpeningId::fresh() was public",
        "mint a new opening identity outside the store",
        r'''    let id = custody::store::open::OpeningId::fresh();''',
        ["error[E0624]: associated function `fresh` is private"],
        [(OPN, "    pub(super) fn fresh() -> OpeningId {", "    pub fn fresh() -> OpeningId {")],
        "only the store's openings mint identities",
    ),
    probe(
        "P-B4-ADMISSION-FORGE", "api-type-guard", "admission binding",
        "(I3-I1 guard, re-verified) an admission built from parts",
        "construct a StorageAdmission for another opening",
        r'''    let io = fixture.store_process("probe");
    let b = open_owner(&io, &fixture.path, config, &mut NoHooks).expect("B");
    let forged = custody::store::open::StorageAdmission {
        opening: b.opening(),
        provision_digest: [0; 32],
        observation: b.admission().observation().clone(),
    };''',
        ["error[E0451]: fields `opening`, `provision_digest` and `observation` of struct `StorageAdmission` are private"],
        [(OPN, "pub struct StorageAdmission {\n    opening: OpeningId,\n    provision_digest: [u8; 32],\n    observation: StorageObservation,\n}",
          "pub struct StorageAdmission {\n    pub opening: OpeningId,\n    pub provision_digest: [u8; 32],\n    pub observation: StorageObservation,\n}")],
        "an admission has one constructor, the storage check",
    ),
    probe(
        "P-B5-VALIDATOR-FROM-COPY", "api-type-guard", "disposition provenance",
        "B5: StoreValidator::new(&root, &edited_scan) validated a fabricated disposition",
        "build a validator over an edited copy of a scan",
        r'''    let io = fixture.store_process("probe");
    let opened = open_owner(&io, &fixture.path, config, &mut NoHooks).expect("opens");
    let edited = opened.scan().clone();
    let validator = custody::store::disposition::StoreValidator::of(
        &[0; 16],
        &edited.level.incidents,
        &edited.dispositions,
    );''',
        ["error[E0624]: associated function `of` is private"],
        [(DSP, "    pub(super) fn of(", "    pub fn of(")],
        "a validator exists only as a borrow of an opening's verified state",
    ),
    probe(
        "P-B6-EXCHANGE-TYPE", "api-type-guard", "exchange containment",
        "B6: Exchange::acquire().state.{fatal, durable_through, claim, seal} were writable",
        "name the exchange to reach its state",
        r'''    fn reach(exchange: &custody::store::exchange::Exchange) {}''',
        ["error[E0603]: struct `Exchange` is private"],
        [(EXC, "pub(super) struct Exchange {", "pub struct Exchange {")],
        "the exchange, its state and its guard are internal; a caller reads copies",
    ),
    probe(
        "P-B6-WORKER-TYPE", "api-type-guard", "exchange containment",
        "B6: Worker::new pointed a worker at any file and exchange",
        "name the worker to construct one",
        r'''    fn reach(worker: custody::store::recorder::Worker<SimIo>) {}''',
        ["error[E0603]: struct `Worker` is private"],
        [(REC, "pub(super) struct Worker<P: StoreIo> {", "pub struct Worker<P: StoreIo> {")],
        "only start_owner constructs an owner's worker",
    ),
    probe(
        "P-SESSION-VERIFIED", "api-type-guard", "session verification",
        "(R1) maintenance procedures took a caller's report, name or refusal",
        "replace the session's retained verification",
        r'''    let mut session = maint::begin(fixture.root_process("probe"), &fixture.path, false)
        .expect("a session");
    session.verified = None;''',
        ["error[E0616]: field `verified` of struct `Session` is private"],
        [(MNT, "    verified: Option<Verification>,",
          "    pub verified: Option<Verification>,"),
         (MNT, "#[derive(Clone)]\nstruct Verification {", "#[derive(Clone)]\npub struct Verification {")],
        "a procedure decides from the session's own latest verification only (R2: the field and its type are both private; the self-check reopens both)",
    ),
    probe(
        "P-PROVISION-REWRITE", "api-type-guard", "session verification",
        "(R1) a public PROVISION rewrite skipped retirement's and re-qualification's preconditions",
        "rewrite PROVISION's retirement directly in a session",
        r'''    let session = maint::begin(fixture.root_process("probe"), &fixture.path, false)
        .expect("a session");
    let _ = maint::rewrite_provision(Rc::new(RefCell::new(session)), |provision| {
        provision.retired_through = 9;
    });''',
        ["error[E0603]: function `rewrite_provision` is private"],
        [(MNT, "pub(super) fn rewrite_provision<P: Platform + Clone + 'static>(",
          "pub fn rewrite_provision<P: Platform + Clone + 'static>(")],
        "section 13.3 uses the rewrite only inside recycling, retirement and re-qualification",
    ),
    probe(
        "P-VERIFICATION-TRANSPLANT", "api-type-guard", "session verification",
        "(R2, NC-SUCC-FOREIGN) a verification of one session or store authorizing another's succession",
        "move one session's retained verification into another session",
        r'''    let mut a = maint::begin(fixture.root_process("a"), &fixture.path, false).expect("A");
    let mut b = maint::begin(fixture.root_process("b"), &fixture.path, false).expect("B");
    let _ = a.verify(config);
    b.verified = a.verified.take();''',
        ["error[E0616]: field `verified` of struct `Session` is private"],
        [(MNT, "    verified: Option<Verification>,",
          "    pub verified: Option<Verification>,"),
         (MNT, "#[derive(Clone)]\nstruct Verification {", "#[derive(Clone)]\npub struct Verification {")],
        "a session's verification is private to it (the field and its type), bound to its opening, selection and mutation count, and taken only by its own procedures",
    ),
    probe(
        "P-VERIFICATION-FORGE", "api-type-guard", "session verification",
        "(R2) a caller-built verification standing in for the session's own",
        "build a verification from a caller's assessment and an opening's identity",
        r'''    let io = fixture.store_process("probe");
    let opened = open_owner(&io, &fixture.path, config, &mut NoHooks).expect("opens");
    let session = maint::begin(fixture.root_process("probe"), &fixture.path, false)
        .expect("a session");
    let forged = maint::Verification {
        epoch: 0,
        opening: opened.opening(),
        root_id: [0; 16],
        revision: 1,
        digest: [0; 32],
        state: String::new(),
        assessment: session.assessment().expect("assessed"),
    };''',
        ["error[E0603]: struct `Verification` is private"],
        [(MNT, '''struct Verification {
    epoch: u64,
    opening: OpeningId,
    root_id: [u8; 16],
    revision: u64,
    digest: [u8; 32],
    state: String,
    assessment: Assessment,
}''', '''pub struct Verification {
    pub epoch: u64,
    pub opening: OpeningId,
    pub root_id: [u8; 16],
    pub revision: u64,
    pub digest: [u8; 32],
    pub state: String,
    pub assessment: Assessment,
}''')],
        "the verification type is internal to the store; a caller holds only Assessment copies, which no procedure accepts",
    ),
    probe(
        "P-TAKE-ASSESSMENT", "api-type-guard", "session verification",
        "(R2) a caller consuming or reading the session's retained verify-before",
        "take the session's retained verification from outside the store",
        r'''    let mut session = maint::begin(fixture.root_process("probe"), &fixture.path, false)
        .expect("a session");
    let _ = session.verify(config);
    let taken = session.take_assessment();''',
        ["error[E0624]: method `take_assessment` is private"],
        [(MNT, "    fn take_assessment(&mut self) -> Result<Assessment, String> {",
          "    pub fn take_assessment(&mut self) -> Result<Assessment, String> {")],
        "only the store's procedures take the verification, once",
    ),
]

ONE_WAY = dict(
    id="P-CLOSE-TWICE", category="api-ownership-guard", boundary="one-way seal",
    base="(R1) one closure, one seal request",
    attempt="close one owner twice",
    body=r'''    let owner = owner(fixture, config);
    let first = owner.close(Tick(2));
    let second = owner.close(Tick(3));''',
    expect=["error[E0382]: use of moved value: `owner`"],
    variant=r'''    let owner = owner(fixture, config);
    if let Err(returned) = owner.close(Tick(2)) {
        let second = returned.close(Tick(3));
    }''',
    why="close consumes the owner; only the owner a refusal returns can be closed again",
)

POSITIVE = r'''    let io = fixture.store_process("probe");
    let opened = open_owner(&io, &fixture.path, config, &mut NoHooks).expect("opens");
    let _ = (
        opened.report(),
        opened.decision().clone(),
        opened.scan().clone(),
        opened.selection().clone(),
        opened.opening(),
        opened.admission().opening(),
    );
    let _ = opened.validator();
    drop(opened);
    let mut owner = owner(fixture, config);
    owner.run_worker_until_idle();
    let _ = (owner.header(), owner.claim(), owner.index(), owner.generation());
    let _ = (owner.retained().len(), owner.status(), owner.latched());
    let now = Tick(2);
    let _ = owner.start_run(now);
    let _ = owner.flush(now);
    match owner.close(Tick(3)) {
        Ok(mut closed) => {
            let _ = (closed.closed().records, closed.seal_request().clone());
            closed.run_worker_until_idle();
            let _ = closed.seal_state();
        }
        Err(owner) => {
            let _ = owner.shutdown_decision();
        }
    }
    let session = Rc::new(RefCell::new(
        maint::begin(fixture.root_process("probe"), &fixture.path, false).expect("a session"),
    ));
    let _ = session.borrow_mut().verify(config);
    if let Some(assessment) = session.borrow().assessment() {
        let _ = (
            assessment.complete,
            assessment.conditions.len(),
            assessment.invalid_conditions(),
            assessment.outcome(),
        );
    }
    let _ = maint::archive(Rc::clone(&session), 0);
    let _ = session.borrow_mut().verify(config);
    let words = maint::OwnerWords {
        reason: DispositionReason::Other,
        statement: "probe".into(),
        operator: "probe".into(),
        at: "2026-10-02T00:00:00Z".into(),
    };
    let _ = maint::publish_disposition(Rc::clone(&session), [0; 32], &words);
    let _ = maint::revoke(Rc::clone(&session), [0; 32], "20261002T000000Z");
    let _ = maint::leftover(Rc::clone(&session));
    let _ = session.borrow_mut().verify(config);
    let _ = maint::successor(Rc::clone(&session), &fixture.layout, &[]);
    let _ = maint::predecessor_statement(&[]);
    if let Ok(session) = Rc::try_unwrap(session) {
        session.into_inner().end();
    }'''

HARNESS = dict(
    id="H-TYPE-ERROR", category="harness-check", boundary="harness",
    body=r'''    let value: u8 = "not a byte";''',
    expect=["error[E0308]: mismatched types"],
)


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def errors(text):
    found = []
    for line in text.splitlines():
        if line.startswith("error[E") or (line.startswith("error:") and "could not compile" not in line
                                           and "aborting due to" not in line):
            if line not in found:
                found.append(line)
    return found


def build(checkout, body, log):
    (checkout / PROBE).write_text(HEADER + body + FOOTER)
    env = dict(os.environ)
    run = subprocess.run(
        ["cargo", "test", "--locked", "-p", "nexus-verifier-sandbox", "--test", "r2_api_probe", "--no-run"],
        cwd=checkout, env=env, capture_output=True, text=True)
    log.write_text(f"$ cargo test --locked -p nexus-verifier-sandbox --test r2_api_probe --no-run\n"
                   f"exit {run.returncode}\n--- probe body ---\n{body}\n--- stderr ---\n{run.stderr}"
                   f"--- stdout ---\n{run.stdout}")
    return run.returncode, errors(run.stderr)


def apply(checkout, edits):
    for edit in edits:
        path = checkout / edit["file"]
        text = path.read_text()
        if text.count(edit["anchor"]) != 1:
            raise SystemExit(f"anchor not unique in {edit['file']}: {edit['anchor']!r}")
        path.write_text(text.replace(edit["anchor"], edit["replacement"]))


def main():
    if len(sys.argv) != 3:
        raise SystemExit(__doc__)
    checkout = pathlib.Path(sys.argv[1]).resolve()
    logs = pathlib.Path(sys.argv[2]).resolve()
    logs.mkdir(parents=True, exist_ok=True)
    if (checkout / PROBE).exists():
        raise SystemExit(f"{PROBE} already exists in the checkout")
    originals = {name: (checkout / name).read_bytes() for name in SOURCES}
    hashes = {name: hashlib.sha256(data).hexdigest() for name, data in originals.items()}
    results = []

    def restore():
        for name, data in originals.items():
            (checkout / name).write_bytes(data)
        bad = [name for name in SOURCES if sha(checkout / name) != hashes[name]]
        if bad:
            raise SystemExit(f"restoration failed: {bad}")

    try:
        # The harness sees errors; the positive control builds.
        code, found = build(checkout, HARNESS["body"], logs / "H-TYPE-ERROR.log")
        results.append(dict(id=HARNESS["id"], category=HARNESS["category"], exit=code, errors=found,
                            ok=code != 0 and found == HARNESS["expect"]))
        code, found = build(checkout, POSITIVE, logs / "POSITIVE-CONTROL.log")
        results.append(dict(id="POSITIVE-CONTROL", category="positive-control", exit=code, errors=found,
                            ok=code == 0 and not found))
        for p in PROBES:
            code, found = build(checkout, p["body"], logs / f"{p['id']}.log")
            guarded = code != 0 and found == p["expect"]
            apply(checkout, p["reopen"])
            try:
                code2, found2 = build(checkout, p["body"], logs / f"{p['id']}.reopened.log")
            finally:
                restore()
            reopened = code2 == 0 and not found2
            results.append(dict(id=p["id"], category=p["category"], boundary=p["boundary"],
                                base=p["base"], attempt=p["attempt"], why=p["why"],
                                expect=p["expect"], exit=code, errors=found, guarded=guarded,
                                reopen=[dict(file=e["file"], anchor=e["anchor"],
                                             replacement=e["replacement"]) for e in p["reopen"]],
                                reopened_exit=code2, reopened_errors=found2,
                                self_check=reopened, ok=guarded and reopened))
        code, found = build(checkout, ONE_WAY["body"], logs / f"{ONE_WAY['id']}.log")
        guarded = code != 0 and found == ONE_WAY["expect"]
        code2, found2 = build(checkout, ONE_WAY["variant"], logs / f"{ONE_WAY['id']}.variant.log")
        variant = code2 == 0 and not found2
        results.append(dict(id=ONE_WAY["id"], category=ONE_WAY["category"], boundary=ONE_WAY["boundary"],
                            base=ONE_WAY["base"], attempt=ONE_WAY["attempt"], why=ONE_WAY["why"],
                            expect=ONE_WAY["expect"], exit=code, errors=found, guarded=guarded,
                            variant_exit=code2, variant_errors=found2, self_check=variant,
                            ok=guarded and variant))
    finally:
        (checkout / PROBE).unlink(missing_ok=True)
        restore()
    (logs / "api_probes.json").write_text(json.dumps(results, indent=2) + "\n")
    lines = ["| probe | category | boundary | expected error | guarded | self-check | ok |",
             "|---|---|---|---|---|---|---|"]
    for r in results:
        lines.append("| {} | {} | {} | {} | {} | {} | {} |".format(
            r["id"], r["category"], r.get("boundary", "-"),
            "; ".join(r.get("expect", r.get("errors", []))) or "(none)",
            r.get("guarded", "-"), r.get("self_check", "-"), r["ok"]))
    counts = {}
    for r in results:
        counts.setdefault(r["category"], [0, 0])
        counts[r["category"]][0] += 1
        counts[r["category"]][1] += 1 if r["ok"] else 0
    lines.append("")
    for category, (total, ok) in sorted(counts.items()):
        lines.append(f"- {category}: {ok}/{total} as required")
    (logs / "api_probes.md").write_text("\n".join(lines) + "\n")
    print("\n".join(lines))
    restored = all(sha(checkout / name) == hashes[name] for name in SOURCES)
    print(f"scratch sources restored: {restored}; probe file removed: {not (checkout / PROBE).exists()}")
    if not restored or (checkout / PROBE).exists():
        return 1
    failed = [r["id"] for r in results if not r["ok"]]
    if failed:
        print(f"FAILED: {failed}")
        return 1
    print(f"ALL {len(results)} PROBE RESULTS AS REQUIRED")
    return 0


if __name__ == "__main__":
    sys.exit(main())
