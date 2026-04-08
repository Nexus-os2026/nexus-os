//! Scout driver: state machine + heartbeat + main loop. See v1.1 §2.

pub mod heartbeat;
pub mod loop_;
pub mod state;

pub use heartbeat::{Heartbeat, HeartbeatState};
pub use loop_::{Driver, DriverConfig, DriverOutcome, HaltReason, PageWorkItem, VisionJudger};
pub use state::DriverState;
