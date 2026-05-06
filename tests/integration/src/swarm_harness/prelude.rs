//! Common imports for swarm-harness scenario tests. A scenario test
//! file should be able to start with one `use` line:
//!
//! ```ignore
//! use nexus_integration::swarm_harness::prelude::*;
//! ```

pub use crate::swarm_harness::{
    correlate_audit_with_events, drain_events_until_terminal, drain_for_duration, event_kind_str,
    oracle_returning, CorrelationReport, EventCaptureError, HeraldHarnessCapability, Scenario,
    ScenarioBuilder, ScriptedOutcome, ScriptedPublishExecutor, SyntheticCapability,
    SyntheticPlannerProvider,
};
pub use nexus_governance_oracle::GovernanceDecision;
pub use nexus_swarm::dag::{DagNode, DagNodeStatus, ExecutionDag};
pub use nexus_swarm::events::SwarmEvent;
pub use nexus_swarm::oracle_bridge::dag_content_hash;
pub use nexus_swarm::oracle_policy::HighRiskEvent;
pub use nexus_swarm::profile::{
    ContextSize, CostClass, LatencyClass, PrivacyClass, ReasoningTier, TaskProfile, ToolUseLevel,
};
pub use nexus_swarm::{Budget, PlannedSwarm, SwarmError};
