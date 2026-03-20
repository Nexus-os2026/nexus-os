//! Teach Mode — learn-by-building mentor that explains each step in plain English.
//!
//! Instead of building FOR the user, Nexus builds WITH them, explaining
//! concepts along the way and adapting language to their skill level.

use serde::{Deserialize, Serialize};

/// A single step in the teaching flow.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeachStep {
    pub step_number: u32,
    pub total_steps: u32,
    pub title: String,
    /// Plain-English explanation of what this step does and why.
    pub explanation: String,
    /// Real-world analogy to ground the concept.
    pub analogy: Option<String>,
    /// The actual code/config that will be created.
    pub implementation_preview: Option<String>,
    /// Concepts the user will learn from this step.
    pub concepts: Vec<String>,
    /// Whether this step was auto-completed ("do it for me").
    pub skipped: bool,
}

/// Tracks teach-mode state for a project.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeachMode {
    pub active: bool,
    pub project_id: String,
    pub current_step: u32,
    pub total_steps: u32,
    pub user_skill_level: f64,
    pub concepts_learned: Vec<String>,
    pub skipped_steps: Vec<u32>,
    pub steps: Vec<TeachStep>,
}

impl TeachMode {
    pub fn new(project_id: &str, total_steps: u32) -> Self {
        Self {
            active: true,
            project_id: project_id.to_string(),
            current_step: 0,
            total_steps,
            user_skill_level: 0.0,
            concepts_learned: Vec::new(),
            skipped_steps: Vec::new(),
            steps: Vec::new(),
        }
    }

    /// Present the next step.  `llm_explanation` is the LLM-generated
    /// plain-English explanation; `concepts` are the new ideas introduced.
    pub fn next_step(
        &mut self,
        title: &str,
        llm_explanation: &str,
        analogy: Option<&str>,
        implementation_preview: Option<&str>,
        concepts: Vec<String>,
    ) -> TeachStep {
        self.current_step += 1;
        let step = TeachStep {
            step_number: self.current_step,
            total_steps: self.total_steps,
            title: title.to_string(),
            explanation: llm_explanation.to_string(),
            analogy: analogy.map(String::from),
            implementation_preview: implementation_preview.map(String::from),
            concepts: concepts.clone(),
            skipped: false,
        };
        self.steps.push(step.clone());
        // Learning concepts increases skill level.
        for c in &concepts {
            if !self.concepts_learned.contains(c) {
                self.concepts_learned.push(c.clone());
                self.user_skill_level = (self.user_skill_level + 0.05).min(1.0);
            }
        }
        step
    }

    /// User said "do it for me" — skip the current step.
    pub fn skip_step(&mut self, title: &str, implementation_preview: Option<&str>) -> TeachStep {
        self.current_step += 1;
        self.skipped_steps.push(self.current_step);
        let step = TeachStep {
            step_number: self.current_step,
            total_steps: self.total_steps,
            title: title.to_string(),
            explanation: "No worries — I've done this step for you!".to_string(),
            analogy: None,
            implementation_preview: implementation_preview.map(String::from),
            concepts: Vec::new(),
            skipped: true,
        };
        self.steps.push(step.clone());
        step
    }

    /// Respond to user input: "next", "skip"/"do it for me", or "explain more".
    pub fn respond(&self, user_input: &str) -> TeachModeAction {
        let lower = user_input.to_lowercase();
        if lower.contains("skip") || lower.contains("do it for me") {
            TeachModeAction::Skip
        } else if lower.contains("explain") || lower.contains("more") || lower.contains("why") {
            TeachModeAction::ExplainMore
        } else {
            TeachModeAction::Next
        }
    }

    /// Whether the teaching session is complete.
    pub fn is_complete(&self) -> bool {
        self.current_step >= self.total_steps
    }

    /// Deactivate teach mode.
    pub fn deactivate(&mut self) {
        self.active = false;
    }
}

/// Action derived from user input during teach mode.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum TeachModeAction {
    Next,
    Skip,
    ExplainMore,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_teach_mode() {
        let tm = TeachMode::new("proj-1", 10);
        assert!(tm.active);
        assert_eq!(tm.current_step, 0);
        assert_eq!(tm.user_skill_level, 0.0);
    }

    #[test]
    fn test_next_step_increases_skill() {
        let mut tm = TeachMode::new("proj-1", 5);
        let step = tm.next_step(
            "Create the homepage",
            "We're going to create the main page people see first.",
            Some("Think of it like the front door of a shop."),
            Some("<div>Welcome!</div>"),
            vec!["HTML basics".into(), "Page structure".into()],
        );
        assert_eq!(step.step_number, 1);
        assert!(!step.skipped);
        assert_eq!(tm.concepts_learned.len(), 2);
        assert!(tm.user_skill_level > 0.0);
    }

    #[test]
    fn test_skip_step() {
        let mut tm = TeachMode::new("proj-1", 5);
        let step = tm.skip_step("Database setup", Some("CREATE TABLE ..."));
        assert!(step.skipped);
        assert_eq!(tm.skipped_steps, vec![1]);
        // Skill doesn't increase on skip.
        assert_eq!(tm.user_skill_level, 0.0);
    }

    #[test]
    fn test_respond_actions() {
        let tm = TeachMode::new("proj-1", 5);
        assert_eq!(tm.respond("I understand, next"), TeachModeAction::Next);
        assert_eq!(tm.respond("do it for me"), TeachModeAction::Skip);
        assert_eq!(
            tm.respond("explain more please"),
            TeachModeAction::ExplainMore
        );
    }

    #[test]
    fn test_is_complete() {
        let mut tm = TeachMode::new("proj-1", 2);
        assert!(!tm.is_complete());
        tm.next_step("Step 1", "Explanation", None, None, vec![]);
        assert!(!tm.is_complete());
        tm.next_step("Step 2", "Explanation", None, None, vec![]);
        assert!(tm.is_complete());
    }
}
