//! Conversational Project Builder — chat-based, zero-jargon project creation.
//!
//! Users describe what they want in plain English.  The builder gathers
//! requirements through simple (preferably multiple-choice) questions,
//! proposes a human-readable plan, and hands off to the autopilot for
//! execution once the user approves.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// State machine
// ---------------------------------------------------------------------------

/// High-level phase of the conversational build flow.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum BuilderState {
    /// Gathering the user's intent from their initial message.
    Understanding,
    /// Asking follow-up questions (max 2 rounds).
    Clarifying,
    /// Presenting a plain-English build plan.
    ProposingPlan,
    /// Waiting for user to approve / tweak the plan.
    WaitingApproval,
    /// Autopilot is executing — live preview active.
    Building,
    /// Delivered — project is live.
    Complete,
    /// User requested post-build changes (feeds into Remix).
    Iterating,
}

// ---------------------------------------------------------------------------
// Requirements
// ---------------------------------------------------------------------------

/// Budget bracket — keeps it simple for non-technical users.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum BudgetRange {
    Free,
    Under50,
    Under200,
    Custom(u64),
}

/// Everything we know about what the user wants to build.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Requirements {
    pub user_goal: String,
    pub business_type: Option<String>,
    pub target_audience: Option<String>,
    pub budget: Option<BudgetRange>,
    pub integrations_needed: Vec<String>,
    pub timeline: Option<String>,
    pub must_haves: Vec<String>,
    pub nice_to_haves: Vec<String>,
}

// ---------------------------------------------------------------------------
// Plan
// ---------------------------------------------------------------------------

/// A single deliverable shown to the user (no tech jargon).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanItem {
    pub label: String,
    pub description: String,
}

/// The build plan presented for approval.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectPlan {
    pub title: String,
    pub items: Vec<PlanItem>,
    pub estimated_minutes: u32,
    pub monthly_cost_summary: String,
}

// ---------------------------------------------------------------------------
// Response
// ---------------------------------------------------------------------------

/// What the builder sends back to the frontend after each user message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuilderResponse {
    pub state: BuilderState,
    pub message: String,
    pub questions: Vec<String>,
    pub plan: Option<ProjectPlan>,
    pub project_id: Option<String>,
}

// ---------------------------------------------------------------------------
// Engine
// ---------------------------------------------------------------------------

/// Conversational builder engine.  Drives the state machine and produces
/// user-facing responses free of technical jargon.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationalBuilder {
    pub id: String,
    pub conversation_state: BuilderState,
    pub gathered_requirements: Requirements,
    pub clarification_round: u32,
    pub plan: Option<ProjectPlan>,
    pub project_id: Option<String>,
}

impl Default for ConversationalBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl ConversationalBuilder {
    pub fn new() -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            conversation_state: BuilderState::Understanding,
            gathered_requirements: Requirements::default(),
            clarification_round: 0,
            plan: None,
            project_id: None,
        }
    }

    /// Process the next user message and advance the state machine.
    ///
    /// In production the `llm_response` parameter would come from the LLM
    /// gateway.  Here we accept the pre-computed LLM output so the kernel
    /// stays provider-agnostic.
    pub fn process_message(&mut self, user_message: &str, llm_response: &str) -> BuilderResponse {
        match self.conversation_state {
            BuilderState::Understanding | BuilderState::Clarifying => {
                self.gathered_requirements.user_goal =
                    if self.gathered_requirements.user_goal.is_empty() {
                        user_message.to_string()
                    } else {
                        format!(
                            "{} | {}",
                            self.gathered_requirements.user_goal, user_message
                        )
                    };
                self.clarification_round += 1;

                if self.clarification_round >= 2 || !llm_response.contains('?') {
                    // Enough info — move to plan proposal.
                    self.conversation_state = BuilderState::ProposingPlan;
                    let plan = self.generate_plan_from(llm_response);
                    self.plan = Some(plan.clone());
                    BuilderResponse {
                        state: BuilderState::ProposingPlan,
                        message: llm_response.to_string(),
                        questions: Vec::new(),
                        plan: Some(plan),
                        project_id: None,
                    }
                } else {
                    self.conversation_state = BuilderState::Clarifying;
                    let questions = extract_questions(llm_response);
                    BuilderResponse {
                        state: BuilderState::Clarifying,
                        message: llm_response.to_string(),
                        questions,
                        plan: None,
                        project_id: None,
                    }
                }
            }

            BuilderState::ProposingPlan | BuilderState::WaitingApproval => {
                let approved = is_approval(user_message);
                if approved {
                    self.conversation_state = BuilderState::Building;
                    let pid = Uuid::new_v4().to_string();
                    self.project_id = Some(pid.clone());
                    BuilderResponse {
                        state: BuilderState::Building,
                        message: "Building your project now! I'll update you as I go.".to_string(),
                        questions: Vec::new(),
                        plan: self.plan.clone(),
                        project_id: Some(pid),
                    }
                } else {
                    self.conversation_state = BuilderState::Clarifying;
                    self.clarification_round = 0;
                    BuilderResponse {
                        state: BuilderState::Clarifying,
                        message: llm_response.to_string(),
                        questions: extract_questions(llm_response),
                        plan: None,
                        project_id: None,
                    }
                }
            }

            BuilderState::Building => BuilderResponse {
                state: BuilderState::Building,
                message: "Still building — hang tight!".to_string(),
                questions: Vec::new(),
                plan: self.plan.clone(),
                project_id: self.project_id.clone(),
            },

            BuilderState::Complete => BuilderResponse {
                state: BuilderState::Complete,
                message: llm_response.to_string(),
                questions: Vec::new(),
                plan: self.plan.clone(),
                project_id: self.project_id.clone(),
            },

            BuilderState::Iterating => {
                self.conversation_state = BuilderState::Iterating;
                BuilderResponse {
                    state: BuilderState::Iterating,
                    message: llm_response.to_string(),
                    questions: Vec::new(),
                    plan: self.plan.clone(),
                    project_id: self.project_id.clone(),
                }
            }
        }
    }

    /// Mark the build as complete.
    pub fn mark_complete(&mut self, summary: &str) -> BuilderResponse {
        self.conversation_state = BuilderState::Complete;
        BuilderResponse {
            state: BuilderState::Complete,
            message: summary.to_string(),
            questions: Vec::new(),
            plan: self.plan.clone(),
            project_id: self.project_id.clone(),
        }
    }

    // --- internal helpers ---------------------------------------------------

    fn generate_plan_from(&self, llm_text: &str) -> ProjectPlan {
        let items: Vec<PlanItem> = llm_text
            .lines()
            .filter(|l| {
                l.starts_with("- ")
                    || l.starts_with("• ")
                    || l.starts_with("├")
                    || l.starts_with("└")
            })
            .map(|l| {
                let label = l
                    .trim_start_matches("- ")
                    .trim_start_matches("• ")
                    .trim_start_matches("├── ")
                    .trim_start_matches("└── ")
                    .to_string();
                PlanItem {
                    description: String::new(),
                    label,
                }
            })
            .collect();

        ProjectPlan {
            title: self
                .gathered_requirements
                .user_goal
                .chars()
                .take(80)
                .collect(),
            items: if items.is_empty() {
                vec![PlanItem {
                    label: "Custom project".into(),
                    description: llm_text.to_string(),
                }]
            } else {
                items
            },
            estimated_minutes: 30,
            monthly_cost_summary: "$0 to start".into(),
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn extract_questions(text: &str) -> Vec<String> {
    text.lines()
        .filter(|l| l.contains('?'))
        .map(|l| l.trim().to_string())
        .collect()
}

fn is_approval(msg: &str) -> bool {
    let lower = msg.to_lowercase();
    lower.contains("yes")
        || lower.contains("build it")
        || lower.contains("go ahead")
        || lower.contains("do it")
        || lower.contains("start")
        || lower.contains("approve")
        || lower.contains("let's go")
        || lower.contains("looks good")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_builder_starts_in_understanding() {
        let b = ConversationalBuilder::new();
        assert_eq!(b.conversation_state, BuilderState::Understanding);
    }

    #[test]
    fn test_approval_detection() {
        assert!(is_approval("Yes, build it!"));
        assert!(is_approval("GO AHEAD"));
        assert!(is_approval("Looks good to me"));
        assert!(!is_approval("I have more questions"));
    }

    #[test]
    fn test_clarification_round() {
        let mut b = ConversationalBuilder::new();
        let resp = b.process_message(
            "I want to sell t-shirts online",
            "Great idea! A few questions:\n1. Do you already have designs?\n2. What's your budget?",
        );
        assert_eq!(resp.state, BuilderState::Clarifying);
        assert!(!resp.questions.is_empty());
    }

    #[test]
    fn test_plan_after_two_rounds() {
        let mut b = ConversationalBuilder::new();
        let _ = b.process_message("sell t-shirts", "What budget?\n- Free?\n- $50?");
        let resp = b.process_message(
            "free",
            "Here's the plan:\n- Storefront website\n- AI design generator\n- Payment processing",
        );
        assert_eq!(resp.state, BuilderState::ProposingPlan);
        assert!(resp.plan.is_some());
    }

    #[test]
    fn test_approval_starts_building() {
        let mut b = ConversationalBuilder::new();
        b.conversation_state = BuilderState::ProposingPlan;
        b.plan = Some(ProjectPlan {
            title: "test".into(),
            items: vec![],
            estimated_minutes: 5,
            monthly_cost_summary: "$0".into(),
        });
        let resp = b.process_message("Yes, build it!", "");
        assert_eq!(resp.state, BuilderState::Building);
        assert!(resp.project_id.is_some());
    }

    #[test]
    fn test_mark_complete() {
        let mut b = ConversationalBuilder::new();
        b.conversation_state = BuilderState::Building;
        b.project_id = Some("proj-123".into());
        let resp = b.mark_complete("Your store is LIVE!");
        assert_eq!(resp.state, BuilderState::Complete);
        assert_eq!(resp.message, "Your store is LIVE!");
    }
}
