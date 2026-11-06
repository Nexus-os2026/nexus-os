pub use crate::errors::compat::AgentState;
use crate::errors::AgentError;

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
    use super::transition_state;
    use crate::errors::compat::AgentState;
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
