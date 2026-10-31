//! Core runtime primitives and governance enforcement surfaces for NEXUS OS.

pub mod actuators;
pub mod adaptive_policy;
pub mod audit;

pub mod autonomy;
pub mod backup;
pub mod behavioral_profile;
pub mod capabilities;
pub mod checkpoint;
/// Governed coding run primitive (Phase One P1A-002; Linux support profile).
#[cfg(target_os = "linux")]
pub mod coding_run;
pub mod cognitive;
pub mod computer_control;
pub mod config;
pub mod consent;
pub mod consent_display;
pub mod crypto;
pub mod drift_detector;
pub mod errors;
pub mod firewall;
pub mod fuel_hardening;
pub mod governed_http;
pub mod governed_path;
pub mod hm;
pub mod identity_home;
pub mod kill_gates;
pub mod lifecycle;
pub mod manifest;
pub mod permissions;
pub mod persistence;
pub mod policy_engine;
pub mod privacy;
pub mod rate_limit;
pub mod redaction;
#[allow(unsafe_code)]
pub mod resource_limiter;
pub mod safety_supervisor;
pub mod scheduler;
pub mod secrets;
pub mod speculative;
pub mod startup;
pub mod supervisor;
pub mod time_machine;
pub mod typed_tools;
pub mod web_conduct;
pub mod workspace;
pub mod workspace_authority;

#[cfg(any(test, feature = "test-utils"))]
pub mod test_support;
