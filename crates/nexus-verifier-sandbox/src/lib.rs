//! Phase Two governed verification execution (Linux support profile).
//!
//! This crate owns the backend's verifier profiles and the identities of the
//! sandbox and resource policies a verifier runs under.
//!
//! Nothing here is authority. A profile name only looks up a compiled-in
//! profile; a policy or profile hash is a binding that later results and
//! approvals commit to, never a capability.
//!
//! See `docs/security/phase2-governed-verification.md`.

mod hash;
pub mod policy;
pub mod profile;

pub use hash::{ProfileHash, ResourcePolicyHash, SandboxPolicyHash};
