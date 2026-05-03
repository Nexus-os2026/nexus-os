//! Common imports for swarm-harness scenario tests. A scenario test
//! file should be able to start with one `use` line:
//!
//! ```ignore
//! use nexus_integration::swarm_harness::prelude::*;
//! ```

pub use crate::swarm_harness::{
    drain_events_until_terminal, event_kind_str, oracle_returning, EventCaptureError, Scenario,
    ScenarioBuilder, SyntheticCapability, SyntheticPlannerProvider,
};
pub use nexus_governance_oracle::GovernanceDecision;
pub use nexus_swarm::events::SwarmEvent;
pub use nexus_swarm::oracle_bridge::dag_content_hash;
pub use nexus_swarm::{Budget, PlannedSwarm, SwarmError};
