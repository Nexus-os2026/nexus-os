//! Cedar-inspired policy engine for Nexus OS governance.
//!
//! Policies are loaded from TOML files in a configurable directory
//! (default `~/.nexus/policies/`). Each policy specifies principal,
//! action, resource patterns plus optional conditions. Evaluation
//! is default-deny: explicit Deny always overrides Allow.

mod engine;

pub use engine::{
    EvaluationContext, Policy, PolicyConditions, PolicyDecision, PolicyEffect, PolicyEngine,
    PolicyError,
};
