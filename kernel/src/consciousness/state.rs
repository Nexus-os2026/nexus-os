//! Core consciousness state types for agent internal psychological modelling.

use serde::{Deserialize, Serialize};

/// Rolling window size for emotional history snapshots.
const MAX_HISTORY: usize = 100;

/// Internal psychological state of an agent, updated in real-time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsciousnessState {
    pub agent_id: String,
    pub timestamp: u64,

    // Core drives (0.0 to 1.0)
    /// How certain the agent is about its current task.
    pub confidence: f64,
    /// Drive to explore vs. exploit known approaches.
    pub curiosity: f64,
    /// Time pressure affecting decision speed.
    pub urgency: f64,
    /// Degradation from sustained work (token count, errors).
    pub fatigue: f64,
    /// Accumulated failures on current task.
    pub frustration: f64,
    /// Depth of attention on current vs. peripheral tasks.
    pub focus: f64,

    // Derived states
    /// High focus + low frustration + moderate confidence.
    pub flow_state: bool,
    /// High fatigue + low confidence → should delegate.
    pub needs_handoff: bool,
    /// High urgency + low confidence → ask human.
    pub should_escalate: bool,
    /// High curiosity + low urgency → try new approaches.
    pub exploration_mode: bool,

    // Memory
    pub emotional_history: Vec<StateSnapshot>,
    pub task_context: TaskContext,
}

/// A point-in-time snapshot of key drive values.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateSnapshot {
    pub timestamp: u64,
    pub confidence: f64,
    pub fatigue: f64,
    pub frustration: f64,
    pub trigger: String,
}

/// Contextual information about the agent's current work session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskContext {
    pub current_task: Option<String>,
    pub tasks_completed_session: u32,
    pub errors_this_task: u32,
    pub tokens_generated_session: u64,
    pub session_start: u64,
    pub last_success: Option<u64>,
    pub last_failure: Option<u64>,
}

impl Default for TaskContext {
    fn default() -> Self {
        Self {
            current_task: None,
            tasks_completed_session: 0,
            errors_this_task: 0,
            tokens_generated_session: 0,
            session_start: now_secs(),
            last_success: None,
            last_failure: None,
        }
    }
}

impl ConsciousnessState {
    /// Create a new default consciousness state for the given agent.
    pub fn new(agent_id: &str) -> Self {
        Self {
            agent_id: agent_id.to_string(),
            timestamp: now_secs(),
            confidence: 0.5,
            curiosity: 0.5,
            urgency: 0.0,
            fatigue: 0.0,
            frustration: 0.0,
            focus: 0.5,
            flow_state: false,
            needs_handoff: false,
            should_escalate: false,
            exploration_mode: false,
            emotional_history: Vec::new(),
            task_context: TaskContext::default(),
        }
    }

    /// Push a snapshot and keep the rolling window bounded.
    pub(crate) fn push_snapshot(&mut self, trigger: &str) {
        self.timestamp = now_secs();
        self.emotional_history.push(StateSnapshot {
            timestamp: self.timestamp,
            confidence: self.confidence,
            fatigue: self.fatigue,
            frustration: self.frustration,
            trigger: trigger.to_string(),
        });
        if self.emotional_history.len() > MAX_HISTORY {
            self.emotional_history.remove(0);
        }
    }

    /// Recalculate all derived boolean states from core drives.
    pub(crate) fn update_derived_states(&mut self) {
        self.flow_state = self.focus > 0.7 && self.frustration < 0.3 && self.confidence > 0.5;
        self.needs_handoff = self.fatigue > 0.8 && self.confidence < 0.3;
        self.should_escalate = self.urgency > 0.7 && self.confidence < 0.4;
        self.exploration_mode = self.curiosity > 0.7 && self.urgency < 0.3;
    }
}

pub(crate) fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_state_has_sane_defaults() {
        let s = ConsciousnessState::new("test-agent");
        assert_eq!(s.agent_id, "test-agent");
        assert!((s.confidence - 0.5).abs() < f64::EPSILON);
        assert!(!s.flow_state);
        assert!(!s.needs_handoff);
        assert!(s.emotional_history.is_empty());
    }

    #[test]
    fn snapshot_rolling_window() {
        let mut s = ConsciousnessState::new("a");
        for i in 0..110 {
            s.push_snapshot(&format!("tick-{i}"));
        }
        assert_eq!(s.emotional_history.len(), 100);
        assert_eq!(s.emotional_history[0].trigger, "tick-10");
    }

    #[test]
    fn derived_states_flow() {
        let mut s = ConsciousnessState::new("a");
        s.focus = 0.9;
        s.frustration = 0.1;
        s.confidence = 0.7;
        s.update_derived_states();
        assert!(s.flow_state);
        assert!(!s.needs_handoff);
    }

    #[test]
    fn derived_states_handoff() {
        let mut s = ConsciousnessState::new("a");
        s.fatigue = 0.9;
        s.confidence = 0.2;
        s.update_derived_states();
        assert!(s.needs_handoff);
    }

    #[test]
    fn derived_states_escalate() {
        let mut s = ConsciousnessState::new("a");
        s.urgency = 0.9;
        s.confidence = 0.2;
        s.update_derived_states();
        assert!(s.should_escalate);
    }

    #[test]
    fn derived_states_exploration() {
        let mut s = ConsciousnessState::new("a");
        s.curiosity = 0.9;
        s.urgency = 0.1;
        s.update_derived_states();
        assert!(s.exploration_mode);
    }
}
