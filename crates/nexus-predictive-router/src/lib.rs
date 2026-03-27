pub mod cost_optimizer;
pub mod difficulty_estimator;
pub mod feedback;
pub mod model_capability;
pub mod router;
pub mod staging;
pub mod tauri_commands;

pub use difficulty_estimator::{DifficultyEstimator, TaskDifficultyEstimate};
pub use model_capability::{ModelCapabilityProfile, ModelRegistry, ModelSizeClass, VectorScores};
pub use router::{PredictiveRouter, RoutingAccuracy, RoutingDecision};
pub use tauri_commands::RouterState;
