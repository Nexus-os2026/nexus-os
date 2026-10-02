# P2 custody store: implementation boundary (P2-V1-R3B-I3-I1, R1)

This document records what the custody store implementation is, what it is
not, and what stays open. It belongs to the candidate on
`review/p2-v1-custody-store-implementation`, based on the R5 design commit
`7689e599ed8fc89ec7720d13869ef58cab767ecb` (custody and codec source baseline
`45898e05178a56efaadeb1f8f9ee7a0a521c72c9`). The design is
`docs/architecture/p2-custody-durable-recorder-design.md` (revision R5). The
I3-I1 evidence is in `docs/evidence/p2-v1-r3b-i3-i1/`.

P2-V1-R3B-I3-I1-R1 (repair base `72ffc4fcc0141e2e5ae77a927481017ca711ab7c`)
corrects the store's authority and closure boundary (section 7). Its evidence
is in `docs/evidence/p2-v1-r3b-i3-i1-r1/`.

## 1. Acceptance status

- **R5 is a conditional design baseline.** It is not host acceptance,
  qualification, production integration, authorization for real verifier
  jobs, or Phase Two completion. A-S5 remains rejected.
- **This implementation is test infrastructure.** It is the test-side module
  that R5 section 17 proposed, under `crates/nexus-verifier-sandbox/tests/`.
  It runs against simulated storage and a fixture host, and against the real
  custody core.
- **Nothing here is approved for use.** No runtime exception or new trust
  assumption is approved. The implementation awaits independent Architect
  review.

## 2. What is implemented

All paths are under `crates/nexus-verifier-sandbox/tests/`.

| Path | Content |
|---|---|
| `support/custody/store/format.rs` | The container (4096-byte header, closure seal and one record block per record, around the unchanged version-1 frames); the `PROVISION` and disposition grammars; entry and archive names; content-addressed incident bindings; the storage identity record. Every byte is checked, with full consumption. |
| `support/custody/store/classify.rs` | The record grammar G1 to G17 and GId; the streaming per-file classifier; the conservative refusal (every malformed file and every unsealed generation with an action start); pool-level and archive checks with arithmetic gap bounds; current and history incidents; the decision over the complete set; the bounded report (64 in detail, with a partial flag). |
| `support/custody/store/io.rs` | The `StoreIo` trait; bounded write and sync retries (a failed sync is never retried into success); a controlled fault adapter; the Linux implementation (`openat2` with `RESOLVE_BENEATH`, `RESOLVE_NO_SYMLINKS`, `RESOLVE_NO_MAGICLINKS` and `RESOLVE_NO_XDEV`, one component at a time). This is the store's only `unsafe` code. Each call has one wrapper, with a safety argument. |
| `support/custody/store/open.rs` | Selection and post-lock revalidation by fresh walks; safe opening; the mount, effective-profile, journal-location, kernel and storage checks; the opening-bound `StorageAdmission`; durable activation (A1 to A5); the scan; the decision; the opening `Opened`, whose state is private (R1); the internal claim handoff; `StoreGuard` and `StartupReport`; the standalone verifier; the closed real entry (section 3). |
| `support/custody/store/exchange.rs` | The bounded, idempotent exchange with one fatal latch (first cause and P); `ExchangeSink`; the owner's apply step; failure delivery at a record the core really issued; the store admission gate around `Custody::admit`; the one-way seal request; `RecorderStatus` (a stall is never a failure). Internal to the store (R1), apart from its plain data types. |
| `support/custody/store/recorder.rs` | The worker's steps W1 to W6, claim and seal, with explicit interleaving points; the drop guard that latches worker loss; the threaded runner, which keeps the owner's guard; the Linux-only native wiring check (R1). The worker is internal to the store (R1). |
| `support/custody/store/owner.rs` | (R1) `StoreOwner`, the one value that retains the custody, recorder, claimed header and worker, and exclusion guard; `start_owner`, its only constructor; `StoreOwner::close`, the only path from a closure to a seal; `ClosedStore`. |
| `support/custody/store/faults.rs` | (R1) The narrow fault and interleaving interface the tests use. Each operation only makes the recorder fail closed or interleaves its real steps. |
| `support/custody/store/disposition.rs` | The pure exact-restatement comparison, and the `DispositionValidator` that exists only as a borrow of one opening's verified state (R1): only a root-owned disposition that restates exactly the incident the store computed. |
| `support/custody/store/maintenance.rs` | The maintenance session (the store lock held for the session's life; verification through that retained authority; R1: the session's own latest verification, consumed once and lapsed by any later procedure step); the fixture qualification; every procedure of design section 13 as labelled, crash-injectable steps. |
| `support/custody/store/sim.rs` | The simulated storage, error, persistence, journal, device and host model; the fixture host. |
| `support/custody/store/mod.rs` | The module's documentation: what it is and is not. |
| `support/custody/mod.rs` | Linux-only wiring of `store`, and one paragraph on the integration obligations the store addresses. |
| `phase2_custody_store.rs` | The test target: 84 store tests (75 from I3-I1, adapted to the R1 interface, and 9 R1 regressions), plus the core's 3 commitment-boundary tests that every custody target compiles. |

`core.rs`, `model.rs`, `codec.rs` and their tests are byte-identical to the
baseline. So are the cleanup observer, the live harness, production `src/`,
the manifests, `Cargo.lock`, workflows and the design.

## 3. The real configured-store entry is permanently closed

```rust
pub fn open_configured_store(
    request: &ConfiguredStoreRequest,
) -> Result<Infallible, IntegrationUnavailable> {
    let _ = request;
    Err(IntegrationUnavailable::DeploymentNotAuthorized)
}
```

- **It refuses before anything else.** It returns
  `DeploymentNotAuthorized` before it opens, reads, syncs, locks, claims or
  modifies anything. Its success type is uninhabited.
- **Nothing enables it.** No environment variable, configuration value,
  feature, serialized approval or caller argument can enable it.
- **Nothing can open a real store.** Only the simulator implements
  `Platform`, the host view that opening, activation, the claim and
  maintenance need. So no code path in this module can open a real store.
- **What the tests pin.** Test `o01` checks the refusal for several paths. It
  checks, in the source, that the entry's whole body is the refusal, that
  no store source names `std::env`, `env!`, `option_env!` or `cfg(feature`,
  and that only the simulator implements `Platform`. Controls
  `A-ENTRY-CALLS` and `A-ENTRY-SWITCH` show that the test detects both
  kinds of change.

## 4. Fixture and native-primitive boundaries

- **Simulated storage.** Every opening, activation, claim, recorder, scan
  and maintenance test runs on `SimIo` over `SimWorld`. The world models:
  - kernel-visible, durable and pending blocks, kept separate;
  - process death (F1) apart from power loss (F2);
  - the per-description `errseq` cursor, with its finite counter;
  - page reclaim with descriptors open, and inode eviction;
  - the metadata log, under the ordered and the per-directory schedule
    families;
  - tears within one 4096-byte block;
  - the jbd2 transaction windows and the stable-completion device paths;
  - a fixture host view.

  It is a model, not a kernel. Group J tests that it reproduces the
  behaviour the design derived from the cited Linux v6.17 paths. That is
  conformance to the design's reading of the sources; no kernel was
  observed or tested.
- **Fixture host.** Every mount, effective-profile, jbd2, kernel and storage
  value is a fixture. A passing test qualifies no host, filesystem, kernel
  build or device.
- **The Owner and root in the simulator.** The "Owner" and "root" processes
  are simulated identities in `SimWorld`. An unprivileged fixture is never a
  root-provisioned store.
- **Native primitives.** `LinuxIo` implements `StoreIo` only. Tests `n01` to
  `n08` exercise it, unprivileged, on files the test creates beneath
  `CARGO_TARGET_TMPDIR`. Test `n08` reaches the worker's write and sync path
  only through `recorder::native_wiring` (R1): a check on the test's own
  file, on an exchange of its own that never leaves it, with no seal and no
  store. Each fixture records the identity of every entry
  it creates and removes only those, bounded, when it ends. They test:
  - descriptor-relative, no-follow opening;
  - link counts;
  - non-blocking FIFO opens;
  - `flock` across descriptions and duplicates;
  - write and sync wiring through a controlled fault adapter;
  - directory syncs and `futimens`;
  - identity and one-filesystem checks.
- **What the native tests do not do.** They inject no real I/O error and no
  power loss. They mount nothing, run nothing privileged, start no
  subprocess owner and touch no systemd, network sandbox or verifier.
- **What the native tests do not prove.** Durability of real storage,
  native cleanup, and any property of the configured store's host.
- **Reads that never happen.** No code reads NVMe attributes, Identify data,
  device cache state, firmware revisions, `/proc/fs/jbd2` or any host
  qualification data.

## 5. Decisions that stay open

### 5.1 Supported runtime (G-HOST)

Nothing is qualified, and the store cannot qualify anything itself. A real
store needs all of the following:

- Owner qualification and provisioning on a named host.
- The running kernel's exact build, qualified against the source claims of
  design sections 5.1, 5.5 to 5.8 and 10.7.
- The superblock facts: an internal journal, and no `fast_commit`.
- The effective ext4 profile of section 6.4.
- The storage qualification of section 5.9: the admitted class, the
  controller's own report, that the host is not a virtual machine guest,
  the evidence for stable completion, and the identity pinned in
  `PROVISION`.
- An Architect-authorized step for the qualification. No such step exists.

### 5.2 Assumptions that stay assumptions

These are assumptions, not properties the store establishes:

- **A-S1, stable completion.** The store observes the kernel's registration
  of a device, never the device.
- **A-S2 to A-S4, containment and read stability.**
- **A-M1 and A-M2.** Directory metadata behaviour and the jbd2 paths, read
  from the v6.17 sources and modelled, not tested on a kernel.

A device that breaks A-S1 can lose acknowledged records with nothing
reported (the design's C31 witness, rerun here as test `j03`'s A-S1 case).

### 5.3 Error observation

- **What the store does.** A sync reports a writeback error once per open
  file description. The recorder never retries a failed sync into success.
- **Why a new description proves nothing.** A sync through a new
  description returning 0 says nothing about records an earlier description
  saw fail (test `s02`).
- **The counter limit.** The kernel's error-sequence counter is finite.
  After 2^19 seen errors it returns to an earlier sample, and a check
  against that sample reports nothing. The store does not detect such a
  collision and does not claim to (test `j06`; control `S-ERRSEQ-NO-WRAP`
  shows that an invented detection would fail the test).
- **What the limit is and is not.** It is a storage fault (domain S) of the
  kind design section 5.6 names. It is not a claim that every reported
  failure means dishonest hardware. Stable completion does not remove it.
- **Still to be decided:** how an integration surfaces such faults to an
  operator, beyond the refusal the store already makes.

### 5.4 Authority, integration and live use

The following are not implemented:

- **G-AUTH.** An authority model against tampering by the store uid
  remains unresolved. In domain A an unkeyed seal or binding proves
  nothing.
- **G-LIVE.** The owner process or service, its transport, its threads and
  the live harness's use of this store are not integrated. Nothing here was
  run live.
- **G-NATIVE.** There are no native-layer guarantees for the residuals of
  design section 14.4.
- **G-PWR.** No empirical power-loss qualification exists.
- **Maintenance and provisioning executables.** No such executable or
  privileged helper exists. The procedures exist only as test-driven steps
  on simulated storage.
- **API-5.** Not proposed and not used.

## 6. Verification, in brief

This section records I3-I1's verification. R1's is in section 7.5.

- **Tests.** The store target had 78 tests: 75 store tests and the core's 3
  commitment-boundary tests. The custody core (47) and codec (21) suites
  and the cleanup observation regression (36) are unchanged and pass. The
  live harness was built, never run.
- **Store mutation controls.** Each of the 90 restores one wrong behaviour.
  Each must compile, fail its intended marked assertion and have its exact
  bytes restored. They are counted by category, never in one figure:
  - implementation safety: 69, of which 3 exercise the Linux primitives;
  - simulator and source conformance: 12;
  - authority and API surface: 9.
- **The I2-R1 suite.** Its 32 counted controls and 4 informational runs were
  rerun unchanged. Every result is as I2-R1 recorded it.
- **The coverage matrix.** Of the R5 model's 67 design controls, 66 have a
  counted Rust control. Four of those are source or type guards rather
  than behaviour: NC05a, NC05b, NC19 and NC21b. One, NC04's container
  geometry, is a historical Python-model control: it has no Rust form.
- **Where the evidence is.** The results, commands and hashes are in
  `docs/evidence/p2-v1-r3b-i3-i1/`.

## 7. Authority and closure boundary (P2-V1-R3B-I3-I1-R1)

The Architect's six counterexamples (B1 to B6) reproduced at the repair base
through ordinary safe-Rust calls from outside the store. Each used a public
item that let caller data stand in for retained state:

- B1: a fabricated `Closed` sealed a journal whose custody refused closure;
- B2: another custody's closure sealed this journal;
- B3: an edited decision let the claim proceed over an unresolved incident;
- B4: an opening's identity and digest were rebound, and another opening's
  admission was presented;
- B5: a validator built from an edited scan copy validated a fabricated
  disposition;
- B6: the exchange's latch, `durable_through`, claim and seal states were
  writable.

R1 removes those items. It does not add a check beside them.

### 7.1 What each kind of state is

- **Candidate input and decoded data** (category A): bytes on disk, parsed
  records, `PROVISION`, disposition files, and the saved report that
  `resume_recycle` takes from the Owner (design section 13.8).
- **Read-only snapshots** (category B): `StartupReport`, `Decision`,
  `ScanResult`, `Selection` and `SessionReport` copies, `RecorderStatus`,
  `Snapshot`, `ClaimState` and `SealState`, and a `Closed` value. They
  control nothing: editing one changes no retained state.
- **Retained, ownership-bearing state** (category C): `StoreOwner`'s
  custody, recorder, worker, guard and claimed header; `ClosedStore`'s
  recorder, worker and guard; `Opened`'s identity, selection, guard, scan,
  decision and admission; `Session`'s locks, selection, admission and latest
  verification. Every field is private to the store.
- **Internal transitions** (category D): the opening's claim, the start's
  disposition application, the closure-to-seal transition, the recorder's
  steps and the procedures' decisions. Each reads only category C.

### 7.2 The ownership graph

- **One constructor.** `start_owner` is the only way to obtain a
  `StoreOwner`. It opens the store, builds the custody with every current
  incident as a prior, and applies dispositions only through that opening's
  own validator. It refuses if anything still blocks, and cross-checks the
  dispositions the custody accepted against the ones the opening derived.
  It then claims through the opening's own claim, which consumes the opening
  and derives its applied bindings and reasons internally. Last, it builds
  the exchange, recorder and worker over that claim.
- **Nothing is handed out.** The custody, recorder, claimed header and
  journal, worker and guard are fields of that one value. None is handed
  out, replaced or borrowed mutably: there is no `&mut Custody`. Custody
  operations are forwarded one by one. `Custody::admit` is reachable only
  through the store's admission gate. `apply_disposition`, `flush_records`,
  `acknowledge`, `record_failed` and `close` are not forwarded.
- **The guard.** It is shared, as `Arc<StoreGuard>`, only with the worker's
  thread. That thread keeps it until the worker ends.

### 7.3 Closure to sealing

- **Closure authorizes the seal.** `StoreOwner::close(self, now)` consumes
  the owner and calls the core's own `close` on the custody it retains.
- **On refusal.** The same owner comes back, with that custody, every owner
  it holds, the recorder, the worker and the guard. The store stays excluded
  (Busy).
- **On success, and only then.** The seal of the owner's own claimed
  journal is requested once, with that closure's record count and the
  retained header.
- **No caller data.** No caller supplies closure data, a record count, a
  header or a journal. A grammar acceptance, an all-durable state, a
  plausible terminal record or a status snapshot authorizes nothing (test
  `b01`).
- **One-way.** `ClosedStore` has no close and no seal request, and the
  recorder refuses a second request (`SealWithheld::AlreadyRequested`).
- **Earlier failure stays.** A latch withholds the seal even after a
  successful closure (test `r08`).
- **Exclusion while unresolved.** The exclusion is held while the custody is
  unresolved, and while worker I/O may be in flight, even after the owner is
  dropped (test `b09`).
- **No I/O under the exchange mutex.** Control and status take the exchange
  mutex only for copies. No I/O, wait or join is done under it.

### 7.4 Opening, dispositions, exchange and sessions

- **Opening.** `Opened`'s fields are private. A caller reads borrows or
  copies (`report`, `selection`, `scan`, `decision`, `opening`,
  `admission`, `validator`). Edits to copies change nothing (test `b03`).
  The claim and its admission check are internal: no admission, identity or
  digest is accepted from a caller (test `b05`, API probes).
- **Complete set.** The start derives from the complete verified incident
  set, never from the bounded report (test `b04`).
- **Disposition provenance.**
  - `exact_restatement` is a pure comparison that authorizes nothing.
  - `StoreValidator<'a>` exists only as a borrow of one opening's verified
    state. That state was read under the opening's store lock, after its
    activation, for its revalidated selection and root. The validator
    cannot outlive the opening or the lock, and no constructor takes a
    `ScanResult`.
  - Stale or foreign data is never consulted: the validator reads only its
    opening's state and checks root and binding (test `b06`).
  - This is API provenance only. It does not resolve G-AUTH.
- **Exchange.**
  - The exchange, its state and guard, the sink, the recorder, the worker
    and its handle are internal to the store. Callers read copies.
  - The narrow fault interface (`faults`) can only latch (monotonically),
    poison, interleave or stop the real worker, inject a submission the sink
    refuses, latches on or already holds, panic the next sink, or
    re-acknowledge a record that is already durable and not yet applied.
  - It offers no constructor, no state write, no unlatch, no closure, seal
    or disposition, and no way to point a worker at another file (test
    `b07`).
  - `recorder::native_wiring` is the one other construction of a worker:
    Linux only, on the test's own native file and an exchange of its own,
    with no seal.
- **Maintenance sessions.**
  - A procedure that decides from a verification (archival, recycling,
    retirement, the recycling's resumption) consumes the session's own
    latest verification, never a caller's report, name or refusal.
  - One verification authorizes one procedure. It lapses as soon as any
    step of the session's procedures starts (test `b08`).
  - The `PROVISION` rewrite is internal to recycling, retirement and
    re-qualification (design section 13.3).

### 7.5 R1 verification and remaining limits

- **Tests.** The store target has 87 tests: 84 store tests (75 adapted, 9 R1
  regressions `b01` to `b09`) and the core's 3.
- **Behavioural controls (99, by category, never one figure).**
  - simulator and source conformance: 12;
  - implementation safety: 69;
  - authority and API surface: 8;
  - R1 authority binding: 10.

  Three I3-I1 anchors moved to their equivalent defect points.
  `A-ADMISSION-UNBOUND` has no behavioural call site left; its intention is
  carried by API/type guards.
- **API/type guards (reported apart).**
  - 15 compile-time probes from outside the boundary, each failing for its
    intended privacy reason and each self-checked by reopening that access;
  - 1 ownership guard (close consumes the owner);
  - a positive control and a harness check.
- **Limits.**
  - This is a safe-API boundary within one crate's safe code. It does not
    defend against `unsafe` code in the same process.
  - The core's own public types (`Closed`, `ValidatedDisposition`,
    `Custody::new`, `DispositionValidator`) stay public. A caller may run a
    custody of its own; the store's guarantees concern only the custody it
    retains.
  - Provisioning and re-publication (design section 13.2) remain Owner acts
    without a session. The store does not itself refuse to re-publish over
    an existing `PROVISION`; that precondition rests on the Owner, as R5
    states.
  - Successor provisioning (design section 13.6) does not check its
    precondition: that the predecessor's in-session verification shows every
    bound incident dispositioned, and that the statement names each
    store-level Invalid condition. I3-I1 never checked it either. The
    session's verification yields no report beside a store-level Invalid or
    Capacity refusal, and section 13.10 names succession as a resolution of
    Capacity. A check therefore needs a design reading and a wider
    verification path; it is an open finding for the Architect, not
    something R1 decides.
  - G-AUTH, G-HOST, G-LIVE, G-NATIVE and G-PWR stay open. Nothing here
    authenticates storage, proves native cleanup or power-loss behaviour,
    or qualifies a host.
