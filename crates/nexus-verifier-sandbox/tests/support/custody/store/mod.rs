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
//!   exchange with one fatal latch (first cause and P), the
//!   [`exchange::ExchangeSink`], the owner's apply step and failure delivery
//!   at a record the core really issued, the store admission gate around
//!   `Custody::admit`, [`exchange::RecorderStatus`], the worker's W1 to W6,
//!   claim and seal, its drop guard, and the owner's start through the real
//!   core ([`recorder::start_owner`]).
//! - **Dispositions** ([`disposition`]): the validator that accepts only a
//!   root-owned disposition restating exactly the incident computed.
//! - **Maintenance** ([`maintenance`]): the session type (the store lock held
//!   for its whole life, verification through that retained authority), the
//!   fixture qualification, and every retained procedure as labelled,
//!   crash-injectable protocol steps.
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
//! - **No authority against the store uid** (gate G-AUTH stays open), no
//!   integration with an owner service, no live validation, no native
//!   cleanup proof and no Phase Two completion.

pub mod classify;
pub mod disposition;
pub mod exchange;
pub mod format;
pub mod io;
pub mod maintenance;
pub mod open;
pub mod recorder;
pub mod sim;
