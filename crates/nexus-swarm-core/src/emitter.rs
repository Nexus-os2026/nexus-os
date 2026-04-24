//! EventEmitter trait. Implementations live in the consuming crate:
//!
//! - `nexus-swarm::CoordinatorEmitter` ships emissions through the
//!   broadcast channel as `SwarmEvent` variants.
//! - `recording::RecordingEmitter` (this file) captures calls in a
//!   `tokio::sync::Mutex<Vec<Recorded>>` for tests.

use crate::context::NodeBudget;
use async_trait::async_trait;
use serde_json::Value;

#[async_trait]
pub trait EventEmitter: Send + Sync {
    /// Emit a `NodeEvent` with the given phase + payload.
    async fn emit_phase(&self, phase: &str, payload: Value);

    /// Emit a per-node `BudgetUpdate`.
    async fn emit_budget_update(&self, delta: NodeBudget);
}

/// In-memory recording emitter. Exposed unconditionally so integration
/// tests in separate crates (which can't see `cfg(test)`) can use it.
pub mod recording {
    use super::*;
    use std::sync::Arc;
    use tokio::sync::Mutex;

    #[derive(Debug, Clone, PartialEq)]
    pub enum Recorded {
        Phase { phase: String, payload: Value },
        Budget { delta: NodeBudget },
    }

    #[derive(Default)]
    pub struct RecordingEmitter {
        pub log: Arc<Mutex<Vec<Recorded>>>,
    }

    impl RecordingEmitter {
        pub fn new() -> Self {
            Self {
                log: Arc::new(Mutex::new(Vec::new())),
            }
        }

        pub async fn snapshot(&self) -> Vec<Recorded> {
            self.log.lock().await.clone()
        }
    }

    #[async_trait]
    impl EventEmitter for RecordingEmitter {
        async fn emit_phase(&self, phase: &str, payload: Value) {
            self.log.lock().await.push(Recorded::Phase {
                phase: phase.to_string(),
                payload,
            });
        }

        async fn emit_budget_update(&self, delta: NodeBudget) {
            self.log.lock().await.push(Recorded::Budget { delta });
        }
    }
}
