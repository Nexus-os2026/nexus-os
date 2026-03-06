//! Replay evidence bundles for auditable, independently verifiable execution records.
//!
//! Produces `.nexus-evidence` files containing a hash-chained audit trail,
//! a policy snapshot at the time of export, and a bundle-level integrity digest.

pub mod bundle;
pub mod format;
pub mod verifier;
