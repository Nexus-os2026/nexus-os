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

pub mod applicability;
mod hash;
pub mod policy;
pub mod profile;
pub mod protocol;
pub mod seccomp_policy;

// The sandbox itself exists only on x86_64 Linux: the seccomp program is
// compiled for that architecture. Everywhere else verification is
// unavailable.
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
pub mod execution;
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
pub mod helper;
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
pub mod landlock_rules;
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
pub mod launcher;
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
pub mod profile_launch;
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
pub mod scope;
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
pub mod seccomp;
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
mod sys;
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
pub mod toolchain;
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
pub mod workspace;

pub use hash::{ProfileHash, ResourcePolicyHash, SandboxPolicyHash};

/// Whether this build can run the verifier sandbox at all.
pub const SANDBOX_SUPPORTED_PLATFORM: bool = cfg!(all(target_os = "linux", target_arch = "x86_64"));
