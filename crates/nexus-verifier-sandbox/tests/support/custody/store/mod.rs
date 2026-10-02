//! The custody store (P2-V1-R3B-I3-I1): the durable recorder, the refusal
//! store and the disposition reader specified by
//! `docs/architecture/p2-custody-durable-recorder-design.md` (revision R5),
//! implemented as test infrastructure and validated against the real
//! custody core (`super::Custody`, its ledger, admission gate and codec)
//! over simulated storage and a fixture host.
//!
//! What it is:
//! - **Formats** ([`format`]): the journal container (a 4096-byte header,
//!   closure seal and record block per record, around the unchanged
//!   version-1 frames), the `PROVISION` and disposition grammars, the entry
//!   names, the content-addressed incident bindings and the storage identity
//!   record, all checked byte for byte with full consumption.
//! - **Classification** ([`classify`]): the record grammar audited against
//!   the core (G1 to G17), the per-file classifier, the conservative refusal
//!   rule, the pool-level and archive checks, current and history incidents,
//!   and the bounded report.
//! - **I/O** ([`io`]): the [`io::StoreIo`] trait the algorithms use, the
//!   bounded write and sync retries, a controlled fault adapter, and the Linux
//!   implementation (the only `unsafe` code of the store).
//! - **Opening** ([`open`]): selection and post-lock revalidation, safe
//!   opening, the mount, effective-profile, kernel and storage checks, the
//!   opening-bound [`open::StorageAdmission`] (one constructor, no `Clone`,
//!   `Default` or decoding), durable activation (A1 to A5), the scan, the
//!   decision, the claim handoff, the owner's [`open::StoreGuard`] and
//!   [`open::StartupReport`], and the standalone verifier.
//! - **Recorder** ([`exchange`], [`recorder`]): the bounded, idempotent
//!   exchange with one fatal latch (first cause and P), the owner's apply
//!   step and failure delivery at a record the core really issued, the store
//!   admission gate around `Custody::admit`, [`exchange::RecorderStatus`],
//!   and the worker's W1 to W6, claim and seal, with its drop guard. All of
//!   it is internal to the store. The one exception is
//!   [`recorder::native_wiring`] (Linux only): a check of the write and sync
//!   path on a test's own native file, on an exchange of its own that never
//!   leaves it. It has no seal and confers nothing on a store.
//! - **Owner** ([`owner`]): [`owner::StoreOwner`], the one value that
//!   retains the custody, the recorder, the claimed journal's header and
//!   worker, and the exclusion guard; [`owner::start_owner`], its only
//!   constructor, through the real core; and [`owner::ClosedStore`]. Only
//!   the actual closure of the retained custody requests the seal of its
//!   own journal, once.
//! - **Dispositions** ([`disposition`]): the exact-restatement comparison,
//!   and the validator that exists only as a borrow of one opening's
//!   verified state.
//! - **Faults** ([`faults`]): the narrow fault and interleaving injection
//!   the tests use. Each injection can only make the recorder fail closed or
//!   interleave its real steps.
//! - **Maintenance** ([`maintenance`]): the session type (the store lock held
//!   for its whole life, verification through that retained authority), the
//!   fixture qualification, and every retained procedure as labelled,
//!   crash-injectable protocol steps. A procedure that decides from a
//!   verification consumes the session's own latest one, which lapses at the
//!   session's next procedure step. The `PROVISION` rewrite is internal to
//!   recycling, retirement and re-qualification.
//!   (P2-V1-R3B-I3-I1-R2) The session's verification reports every
//!   store-level condition it can establish, canonically named, and is
//!   complete only when nothing was left unread; what the session retains is
//!   bound to its opening and selection. Every session procedure passes a
//!   gate before its first step and ends in its in-session verify-after.
//!   Succession consumes a complete verification of the predecessor, needs
//!   every incident dispositioned and the Owner's acceptance of exactly the
//!   verified Invalid conditions, writes a statement generated from them,
//!   and is complete only once the selected successor verifies. A
//!   disposition restates only an incident the verification reported.
//!   Provisioning and re-publication never replace a `PROVISION`.
//! - **Simulation** ([`sim`]): the bounded storage, error, persistence,
//!   journal and host model the tests run everything against.
//!
//! What it is not, and what it does not claim:
//! - **No real store is ever opened.** [`open::open_configured_store`]
//!   refuses with `DeploymentNotAuthorized` before it opens, syncs, locks,
//!   claims or modifies anything; its success type is uninhabited. No
//!   environment variable, configuration value, feature, serialized approval
//!   or caller argument enables it. Only the simulator implements
//!   [`open::Platform`], so the opening, activation, claim and maintenance
//!   algorithms run against simulated storage only; [`io::linux::LinuxIo`]
//!   implements the I/O trait alone and is exercised only by unprivileged
//!   primitive tests beneath `CARGO_TARGET_TMPDIR`. Nothing reads a device
//!   attribute, Identify data, a cache setting, firmware, `/proc/fs` or any
//!   host qualification data.
//! - **Nothing is qualified.** Every host, profile and storage value the
//!   tests use is a fixture; a passing test is a fixture qualification of no
//!   machine. Stable completion (A-S1), containment and read stability stay
//!   assumptions; the store observes the kernel's registration of a device,
//!   never the device.
//! - **Error observation has limits.** A sync reports a writeback error once
//!   per open file description, and the kernel's error sequence counter is
//!   finite: after 2^19 seen errors it can return to an earlier sample, and a
//!   check against that sample reports nothing. The store does not detect
//!   such a collision and does not claim to; it is a storage fault (domain S),
//!   not something every reported failure is assumed to be.
//! - **A safe-API boundary only** (P2-V1-R3B-I3-I1-R1, R2). The owner, the
//!   closure-to-seal transition, the opening's retained state, the
//!   validator's provenance, the exchange's containment and the session's
//!   verification and succession authority rest on Rust visibility and
//!   ownership in safe code. They
//!   do not defend against `unsafe` code in the same process. The core's own
//!   public types (`Closed`, `ValidatedDisposition`, `Custody::new`,
//!   `DispositionValidator`) stay public: a caller may build and run a
//!   custody of its own, and the store's guarantees concern only the custody
//!   it retains.
//! - **No authority against the store uid** (gate G-AUTH stays open), no
//!   integration with an owner service, no live validation, no native
//!   cleanup proof and no Phase Two completion.

pub mod classify;
pub mod disposition;
pub mod exchange;
pub mod faults;
pub mod format;
pub mod io;
pub mod maintenance;
pub mod open;
pub mod owner;
pub mod recorder;
pub mod sim;
