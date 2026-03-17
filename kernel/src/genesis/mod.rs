//! Genesis Protocol — autonomous agent creation engine.
//!
//! When a user request reveals a capability gap in the agent pool, the Genesis
//! engine designs, generates, tests, and hot-deploys a new agent — all governed
//! by HITL approval. The OS grows its own organs.
//!
//! # Pipeline
//!
//! 1. **Gap analysis** — compare user request against existing agents
//! 2. **Design** — specify the new agent's capabilities, autonomy, and prompt
//! 3. **Generate** — produce a complete agent manifest + genome
//! 4. **Test** — score the agent on domain-specific tasks
//! 5. **Deploy** — register the agent in the kernel without restart
//! 6. **Learn** — store creation patterns for future reuse

pub mod deployer;
pub mod engine;
pub mod gap_analysis;
pub mod generator;
pub mod memory;
pub mod tester;

pub use engine::{GenesisEngine, GenesisResult};
pub use gap_analysis::{AgentMatch, GapAnalysis};
pub use generator::AgentSpec;
pub use memory::CreationPattern;
pub use tester::TestResult;
