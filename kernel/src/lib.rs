//! Core runtime primitives and governance enforcement surfaces for NEXUS OS.

pub mod adaptive_policy;
pub mod audit;
pub mod autonomy;
pub mod compliance;
pub mod config;
pub mod consent;
pub mod delegation;
pub mod distributed;
pub mod errors;
pub mod firewall;
pub mod fuel_hardening;
pub mod hardware;
pub mod hardware_security;
pub mod identity;
pub mod kill_gates;
pub mod lifecycle;
pub mod manifest;
pub mod orchestration;
pub mod permissions;
pub mod privacy;
pub mod protocols;
pub mod redaction;
pub mod replay;
// SAFETY EXCEPTION: resource_limiter uses `unsafe` for `pre_exec` + `setrlimit`
// to impose OS-level limits on child processes.  This is the only unsafe code
// in the workspace.  See module-level docs for full justification.
#[allow(unsafe_code)]
pub mod resource_limiter;
pub mod safety_supervisor;
pub mod speculative;
pub mod supervisor;
