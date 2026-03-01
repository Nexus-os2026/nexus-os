use crate::errors::AgentError;
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AgentState {
    Created,
    Starting,
    Running,
    Paused,
    Stopping,
    Stopped,
    Destroyed,
}

impl Display for AgentState {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            AgentState::Created => write!(f, "Created"),
            AgentState::Starting => write!(f, "Starting"),
            AgentState::Running => write!(f, "Running"),
            AgentState::Paused => write!(f, "Paused"),
            AgentState::Stopping => write!(f, "Stopping"),
            AgentState::Stopped => write!(f, "Stopped"),
            AgentState::Destroyed => write!(f, "Destroyed"),
        }
    }
}

pub fn transition_state(from: AgentState, to: AgentState) -> Result<AgentState, AgentError> {
    if is_valid_transition(from, to) {
        Ok(to)
    } else {
        Err(AgentError::InvalidTransition { from, to })
    }
}

pub fn is_valid_transition(from: AgentState, to: AgentState) -> bool {
    match from {
        AgentState::Created => matches!(to, AgentState::Starting),
        AgentState::Starting => matches!(to, AgentState::Running | AgentState::Stopped),
        AgentState::Running => matches!(to, AgentState::Paused | AgentState::Stopping),
        AgentState::Paused => matches!(to, AgentState::Running | AgentState::Stopping),
        AgentState::Stopping => matches!(to, AgentState::Stopped),
        AgentState::Stopped => matches!(to, AgentState::Starting | AgentState::Destroyed),
        AgentState::Destroyed => false,
    }
}

#[cfg(test)]
mod tests {
    use super::{transition_state, AgentState};
    use crate::errors::AgentError;

    #[test]
    fn test_valid_transitions() {
        assert_eq!(
            transition_state(AgentState::Created, AgentState::Starting),
            Ok(AgentState::Starting)
        );
        assert_eq!(
            transition_state(AgentState::Starting, AgentState::Running),
            Ok(AgentState::Running)
        );
        assert_eq!(
            transition_state(AgentState::Starting, AgentState::Stopped),
            Ok(AgentState::Stopped)
        );
        assert_eq!(
            transition_state(AgentState::Running, AgentState::Paused),
            Ok(AgentState::Paused)
        );
        assert_eq!(
            transition_state(AgentState::Running, AgentState::Stopping),
            Ok(AgentState::Stopping)
        );
        assert_eq!(
            transition_state(AgentState::Paused, AgentState::Running),
            Ok(AgentState::Running)
        );
        assert_eq!(
            transition_state(AgentState::Paused, AgentState::Stopping),
            Ok(AgentState::Stopping)
        );
        assert_eq!(
            transition_state(AgentState::Stopping, AgentState::Stopped),
            Ok(AgentState::Stopped)
        );
        assert_eq!(
            transition_state(AgentState::Stopped, AgentState::Starting),
            Ok(AgentState::Starting)
        );
        assert_eq!(
            transition_state(AgentState::Stopped, AgentState::Destroyed),
            Ok(AgentState::Destroyed)
        );
    }

    #[test]
    fn test_invalid_transitions() {
        let invalid = transition_state(AgentState::Created, AgentState::Paused);
        assert_eq!(
            invalid,
            Err(AgentError::InvalidTransition {
                from: AgentState::Created,
                to: AgentState::Paused
            })
        );
    }
}
