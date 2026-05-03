//! Phase B swarm-harness module tree.
//!
//! Synthetic plumbing for end-to-end swarm tests that exercise
//! `Director::plan` and `SwarmCoordinator::run` against deterministic
//! input. No network, no real LLM, no secrets. Each scenario test
//! constructs a fresh `Scenario` via `ScenarioBuilder`; nothing in this
//! module touches process-global state.

pub mod builder;
pub mod capabilities;
pub mod events;
pub mod oracles;
pub mod prelude;
pub mod providers;

pub use builder::{Scenario, ScenarioBuilder};
pub use capabilities::SyntheticCapability;
pub use events::{drain_events_until_terminal, event_kind_str, EventCaptureError};
pub use oracles::oracle_returning;
pub use providers::SyntheticPlannerProvider;
