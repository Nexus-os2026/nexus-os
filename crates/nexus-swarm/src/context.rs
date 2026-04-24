//! Re-export of `AgentExecutionContext` and `NodeBudget` from
//! `nexus-swarm-core`. The types moved out so agent crates can implement
//! the swarm execution contract without a circular dep on this crate.
//! Existing callers continue to use the `nexus_swarm::AgentExecutionContext`
//! path unchanged.

pub use nexus_swarm_core::context::{AgentExecutionContext, NodeBudget};
