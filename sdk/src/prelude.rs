//! Convenience re-exports for agent developers.
//!
//! Agents should `use nexus_sdk::prelude::*` instead of importing from `nexus_kernel` directly.

pub use crate::agent_trait::{AgentOutput, NexusAgent};
pub use crate::context::AgentContext;
pub use crate::manifest::ManifestBuilder;
pub use crate::testing::TestHarness;

// Core kernel types re-exported through the SDK boundary.
pub use nexus_kernel::audit::{AuditEvent, AuditTrail, EventType};
pub use nexus_kernel::autonomy::{AutonomyError, AutonomyGuard, AutonomyLevel};
pub use nexus_kernel::consent::{ApprovalRequest, ConsentRuntime, GovernedOperation, RiskLevel};
pub use nexus_kernel::errors::{AgentError, ErrorStrategy};
pub use nexus_kernel::fuel_hardening::{
    AgentFuelLedger, BudgetPeriodId, BurnAnomalyDetector, FuelAuditReport, FuelViolation,
};
pub use nexus_kernel::lifecycle::AgentState;
pub use nexus_kernel::manifest::AgentManifest;
pub use nexus_kernel::redaction::{RedactionEngine, RedactionPolicy};
pub use nexus_kernel::supervisor::{AgentHandle, AgentId, Supervisor};
