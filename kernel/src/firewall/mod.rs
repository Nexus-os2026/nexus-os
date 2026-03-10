//! Unified prompt firewall: input filtering (injection + PII), output filtering
//! (schema validation + exfiltration detection), and egress governance
//! (per-agent URL allowlisting + rate limiting).
//!
//! Replaces the scattered injection patterns in `bridge.rs`, `defense.rs`,
//! `shadow_sandbox.rs`, and `messaging.rs` with a single canonical filter
//! that runs fail-closed with full audit trail.

pub mod egress;
pub mod patterns;
pub mod prompt_firewall;

pub use egress::{EgressDecision, EgressGovernor, DEFAULT_RATE_LIMIT_PER_MIN};
pub use patterns::{pattern_summary, PatternSummary};
pub use prompt_firewall::{
    FirewallAction, FirewallAuditEntry, InputFilter, OutputFilter, PromptFirewall,
};
