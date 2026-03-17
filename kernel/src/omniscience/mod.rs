//! Computer Omniscience — the OS understands everything on screen,
//! predicts user intent, and acts proactively.
//!
//! This module provides:
//! - **ScreenUnderstanding**: continuous screen analysis and context capture
//! - **IntentPredictor**: predict the user's goal from screen context sequences
//! - **ProactiveAssistant**: suggest and execute actions before the user asks
//! - **AppIntegration**: deep integration with common application types
//! - **ActionExecutor**: governed computer actions with kill-switch safety

pub mod apps;
pub mod assistant;
pub mod executor;
pub mod intent;
pub mod screen;

pub use apps::{AppContext, AppIntegration, AppType};
pub use assistant::{
    AssistanceConfig, NotificationStyle, ProactiveAssistant, Suggestion, SuggestionStatus,
};
pub use executor::{ActionExecutor, ActionStatus, ActionType, ComputerAction};
pub use intent::{ConfidenceLevel, IntentPrediction, IntentPredictor};
pub use screen::{ScreenContext, ScreenUnderstanding, UiElement};

use thiserror::Error;

/// Errors produced by the omniscience subsystem.
#[derive(Debug, Clone, Error, Serialize, Deserialize)]
#[non_exhaustive]
pub enum OmniscienceError {
    #[error("screen capture failed: {reason}")]
    CaptureFailed { reason: String },

    #[error("intent prediction failed: {reason}")]
    PredictionFailed { reason: String },

    #[error("action execution denied: {reason}")]
    ActionDenied { reason: String },

    #[error("action not found: {id}")]
    ActionNotFound { id: String },

    #[error("suggestion not found: {id}")]
    SuggestionNotFound { id: String },

    #[error("kill switch activated — all actions cancelled")]
    KillSwitchActivated,

    #[error("app integration error: {reason}")]
    AppError { reason: String },

    #[error("configuration error: {reason}")]
    ConfigError { reason: String },
}

use serde::{Deserialize, Serialize};

pub type OmniscienceResult<T> = Result<T, OmniscienceError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_display() {
        let err = OmniscienceError::CaptureFailed {
            reason: "no display".into(),
        };
        assert!(err.to_string().contains("no display"));
    }

    #[test]
    fn error_clone_debug() {
        let err = OmniscienceError::KillSwitchActivated;
        let cloned = err.clone();
        assert_eq!(format!("{err:?}"), format!("{cloned:?}"));
    }

    #[test]
    fn error_variants_serialize() {
        let errors = vec![
            OmniscienceError::CaptureFailed {
                reason: "test".into(),
            },
            OmniscienceError::PredictionFailed {
                reason: "test".into(),
            },
            OmniscienceError::ActionDenied {
                reason: "test".into(),
            },
            OmniscienceError::ActionNotFound { id: "abc".into() },
            OmniscienceError::SuggestionNotFound { id: "abc".into() },
            OmniscienceError::KillSwitchActivated,
            OmniscienceError::AppError {
                reason: "test".into(),
            },
            OmniscienceError::ConfigError {
                reason: "test".into(),
            },
        ];
        for err in &errors {
            let json = serde_json::to_string(err).unwrap();
            assert!(!json.is_empty());
        }
    }
}
