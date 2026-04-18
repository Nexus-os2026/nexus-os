//! # nexus-swarm
//!
//! Swarm orchestration layer for Nexus OS.
//!
//! A `Director` decomposes a user intent into an `ExecutionDag` of capability
//! invocations. A `SwarmCoordinator` runs ready DAG nodes in parallel as tokio
//! actors, resolving each node's (provider, model) through a `Router` that
//! enforces privacy class, budget, and provider-health constraints. Every
//! outbound provider call is wrapped through the existing
//! [`nexus_governance_oracle::GovernanceOracle`] before execution.
//!
//! ## Invariants
//!
//! - No Claude CLI provider. The Claude Max interactive CLI is not used in
//!   the autonomous swarm.
//! - Anthropic is Haiku-only with a hard cumulative $2.00 USD cap persisted
//!   to `~/.nexus/swarm/anthropic_spend.json`.
//! - `StrictLocal` and `Sensitive` privacy classes are hard-deny against
//!   non-local providers. Never downgraded.
//! - No `todo!()`, `unimplemented!()`, or `.unwrap()` outside test code.

#![cfg_attr(not(test), deny(clippy::unwrap_used))]

pub mod adapters;
pub mod budget;
pub mod capability;
pub mod coordinator;
pub mod dag;
pub mod director;
pub mod error;
pub mod events;
pub mod profile;
pub mod provider;
pub mod providers;
pub mod registry;
pub mod routing;
pub mod routing_defaults;

pub use budget::Budget;
pub use capability::{AgentCapabilityDescriptor, SwarmCapability};
pub use coordinator::{SwarmCoordinator, SwarmRunHandle};
pub use dag::{DagEdge, DagNode, DagNodeStatus, ExecutionDag};
pub use director::{Director, SwarmDirector};
pub use error::SwarmError;
pub use events::{ProviderHealth, ProviderHealthStatus, SwarmEvent};
pub use profile::{
    ContextSize, CostClass, LatencyClass, PrivacyClass, ReasoningTier, TaskProfile, ToolUseLevel,
};
pub use provider::{
    InvokeRequest, InvokeResponse, ModelDescriptor, Provider, ProviderCapabilities, ProviderError,
};
pub use registry::CapabilityRegistry;
pub use routing::{RouteCandidate, RouteDenied, Router, RoutingPolicy};
