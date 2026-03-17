//! Automatic dream task generation from chat interactions and consciousness state.

use super::scheduler::DreamScheduler;
use super::types::{DreamTask, DreamType};
use crate::consciousness::state::ConsciousnessState;
use serde_json::json;

/// Context from a completed chat interaction.
#[derive(Debug, Clone)]
pub struct ChatInteraction {
    pub user_message: String,
    pub agent_response: String,
    pub was_error: bool,
    pub error_message: Option<String>,
    pub topic_detected: Option<String>,
    pub token_count: u64,
}

/// Queue dream tasks based on the agent's consciousness state after an interaction.
pub fn queue_dreams_from_interaction(
    scheduler: &mut DreamScheduler,
    agent_id: &str,
    consciousness: &ConsciousnessState,
    interaction: &ChatInteraction,
) {
    // High frustration → queue Experiment dream
    if consciousness.frustration > 0.5 {
        scheduler.enqueue(DreamTask {
            task_type: DreamType::Experiment,
            priority: consciousness.frustration,
            source_agent: agent_id.to_string(),
            context: json!({
                "task": interaction.user_message,
                "failures": interaction.error_message.as_deref().unwrap_or("")
            }),
            estimated_tokens: 800,
            ..DreamTask::default()
        });
    }

    // High curiosity → queue Explore dream
    if consciousness.curiosity > 0.7 {
        if let Some(ref topic) = interaction.topic_detected {
            scheduler.enqueue(DreamTask {
                task_type: DreamType::Explore,
                priority: 0.3,
                source_agent: agent_id.to_string(),
                context: json!({"topic": topic}),
                estimated_tokens: 600,
                ..DreamTask::default()
            });
        }
    }

    // Task failure → queue Replay dream for later improvement
    if interaction.was_error {
        scheduler.enqueue(DreamTask {
            task_type: DreamType::Replay,
            priority: 0.6,
            source_agent: agent_id.to_string(),
            context: json!({
                "task": interaction.user_message,
                "original_response": interaction.agent_response,
                "error": interaction.error_message.as_deref().unwrap_or("")
            }),
            estimated_tokens: 600,
            ..DreamTask::default()
        });
    }

    // Needs handoff → queue Create dream (capability gap)
    if consciousness.needs_handoff {
        scheduler.enqueue(DreamTask {
            task_type: DreamType::Create,
            priority: 0.7,
            source_agent: agent_id.to_string(),
            context: json!({
                "gap": format!(
                    "Agent {} is fatigued and unable to handle: {}",
                    agent_id, interaction.user_message
                ),
                "missing_capabilities": []
            }),
            estimated_tokens: 600,
            ..DreamTask::default()
        });
    }

    // Record activity on scheduler to reset idle timer
    scheduler.touch_activity();
}

/// Queue end-of-session dreams (consolidation + precompute).
/// Call when the user says goodnight or idle threshold is about to trigger.
pub fn queue_end_of_session_dreams(
    scheduler: &mut DreamScheduler,
    agent_id: &str,
    session_summary: &str,
    conversation_context: &str,
) {
    // Consolidate the day's learnings
    scheduler.enqueue(DreamTask {
        task_type: DreamType::Consolidate,
        priority: 0.8,
        source_agent: agent_id.to_string(),
        context: json!({"session_summary": session_summary}),
        estimated_tokens: 600,
        ..DreamTask::default()
    });

    // Precompute likely next requests
    scheduler.enqueue(DreamTask {
        task_type: DreamType::Precompute,
        priority: 0.5,
        source_agent: agent_id.to_string(),
        context: json!({"conversation": conversation_context}),
        estimated_tokens: 800,
        ..DreamTask::default()
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_consciousness(
        frustration: f64,
        curiosity: f64,
        needs_handoff: bool,
    ) -> ConsciousnessState {
        let mut s = ConsciousnessState::new("test");
        s.frustration = frustration;
        s.curiosity = curiosity;
        s.needs_handoff = needs_handoff;
        s
    }

    fn make_interaction(was_error: bool) -> ChatInteraction {
        ChatInteraction {
            user_message: "How do I sort a linked list?".into(),
            agent_response: "Use merge sort.".into(),
            was_error,
            error_message: if was_error {
                Some("timeout".into())
            } else {
                None
            },
            topic_detected: Some("algorithms".into()),
            token_count: 100,
        }
    }

    #[test]
    fn high_frustration_queues_experiment() {
        let mut sched = DreamScheduler::new();
        let cons = make_consciousness(0.7, 0.3, false);
        let interaction = make_interaction(false);

        queue_dreams_from_interaction(&mut sched, "agent-a", &cons, &interaction);

        assert_eq!(sched.queue_len(), 1);
        assert_eq!(sched.priority_queue[0].task_type, DreamType::Experiment);
        assert!((sched.priority_queue[0].priority - 0.7).abs() < f64::EPSILON);
    }

    #[test]
    fn high_curiosity_queues_explore() {
        let mut sched = DreamScheduler::new();
        let cons = make_consciousness(0.0, 0.9, false);
        let interaction = make_interaction(false);

        queue_dreams_from_interaction(&mut sched, "agent-a", &cons, &interaction);

        assert_eq!(sched.queue_len(), 1);
        assert_eq!(sched.priority_queue[0].task_type, DreamType::Explore);
    }

    #[test]
    fn error_queues_replay() {
        let mut sched = DreamScheduler::new();
        let cons = make_consciousness(0.0, 0.3, false);
        let interaction = make_interaction(true);

        queue_dreams_from_interaction(&mut sched, "agent-a", &cons, &interaction);

        assert_eq!(sched.queue_len(), 1);
        assert_eq!(sched.priority_queue[0].task_type, DreamType::Replay);
    }

    #[test]
    fn needs_handoff_queues_create() {
        let mut sched = DreamScheduler::new();
        let cons = make_consciousness(0.0, 0.3, true);
        let interaction = make_interaction(false);

        queue_dreams_from_interaction(&mut sched, "agent-a", &cons, &interaction);

        assert_eq!(sched.queue_len(), 1);
        assert_eq!(sched.priority_queue[0].task_type, DreamType::Create);
    }

    #[test]
    fn multiple_triggers_queue_multiple_dreams() {
        let mut sched = DreamScheduler::new();
        let cons = make_consciousness(0.8, 0.9, true);
        let interaction = make_interaction(true);

        queue_dreams_from_interaction(&mut sched, "agent-a", &cons, &interaction);

        // Frustration → Experiment, Curiosity → Explore, Error → Replay, Handoff → Create
        assert_eq!(sched.queue_len(), 4);
    }

    #[test]
    fn end_of_session_queues_consolidate_and_precompute() {
        let mut sched = DreamScheduler::new();
        queue_end_of_session_dreams(&mut sched, "agent-a", "did a lot today", "built REST API");

        assert_eq!(sched.queue_len(), 2);
        let types: Vec<_> = sched
            .priority_queue
            .iter()
            .map(|t| t.task_type.clone())
            .collect();
        assert!(types.contains(&DreamType::Consolidate));
        assert!(types.contains(&DreamType::Precompute));
    }

    #[test]
    fn no_triggers_no_dreams() {
        let mut sched = DreamScheduler::new();
        let cons = make_consciousness(0.0, 0.3, false);
        let interaction = make_interaction(false);

        queue_dreams_from_interaction(&mut sched, "agent-a", &cons, &interaction);

        assert_eq!(sched.queue_len(), 0);
    }
}
