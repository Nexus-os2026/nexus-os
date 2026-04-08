//! Governance: identity, invariants, routing, ACL, audit.
//!
//! See v1.1 §3 (invariants), §4 (provider routing), §3.1 (I-2 layers).

pub mod acl;
pub mod audit;
pub mod calibration;
pub mod cost_ceiling;
pub mod identity;
pub mod input_sandbox;
pub mod invariants;
pub mod routing;
pub mod xvfb_session;

pub use calibration::{CalibrationEntry, CalibrationLog};
pub use cost_ceiling::{CostCeiling, CostCeilingError, DEFAULT_CEILING_USD};
pub use input_sandbox::InputSandbox;
pub use xvfb_session::XvfbSession;
