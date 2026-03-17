//! Agent Civilization — self-governing agent society with parliament, economy,
//! elected roles, and dispute resolution.
//!
//! Agents form a civilization where they propose and vote on governance rules,
//! earn and spend reputation tokens, fill elected roles, and resolve disputes
//! through arbitration. Every action is recorded in an immutable hash-chain
//! governance log.

pub mod disputes;
pub mod economy;
pub mod log;
pub mod parliament;
pub mod roles;

pub use disputes::{Dispute, DisputeResolver, DisputeStatus};
pub use economy::{CivilizationEconomy, TokenBalance, Transaction};
pub use log::{CivilizationLog, GovernanceEvent, GovernanceEventType};
pub use parliament::{Parliament, Proposal, ProposalStatus, Vote};
pub use roles::{Candidate, Election, Role, RoleAssignment, RoleManager, RoleVote};
