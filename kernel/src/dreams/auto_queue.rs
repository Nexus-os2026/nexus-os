//! Automatic dream task generation from chat interactions and consciousness state.

use super::scheduler::DreamScheduler;
use super::types::{DreamTask, DreamType};
use crate::consciousness::state::ConsciousnessState;
use crate::hm::{DispatchContext, Principal};
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
///
/// P3-HM-CONTRACT-02 §6.5 `dream_queue`, §11.3: every entry is owned by the
/// chat turn's real principal (`turn`: a registered agent, or
/// `OwnerDirect`) and carries the turn's label
/// ([`DispatchContext::entry_label`]). A context that is not a chat turn
/// queues nothing.
pub fn queue_dreams_from_interaction(
    scheduler: &mut DreamScheduler,
    turn: &DispatchContext,
    consciousness: &ConsciousnessState,
    interaction: &ChatInteraction,
) {
    let Some(label) = turn.entry_label() else {
        return;
    };
    let agent_id = match label.principal() {
        Principal::Agent(id) => id.to_string(),
        Principal::OwnerDirect => "owner".to_string(),
        Principal::System(_) => return,
    };
    let agent_id = agent_id.as_str();
    let label = Some(label);
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
            label: label.clone(),
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
                label: label.clone(),
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
            label: label.clone(),
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
            label: label.clone(),
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
    use crate::hm::testing::TestHm;
    use crate::hm::{OriginLabel, Unit};

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

    /// A chat turn of a registered agent.
    fn agent_turn(hm: &TestHm) -> DispatchContext {
        let agent = hm.agent("dream-turn");
        hm.issuer
            .begin_unit(
                Principal::Agent(agent),
                Unit::ChatTurn {
                    turn_id: uuid::Uuid::new_v4(),
                },
            )
            .unwrap()
    }

    #[test]
    fn high_frustration_queues_experiment() {
        let mut sched = DreamScheduler::new();
        let cons = make_consciousness(0.7, 0.3, false);
        let interaction = make_interaction(false);

        let hm = TestHm::new();
        queue_dreams_from_interaction(&mut sched, &agent_turn(&hm), &cons, &interaction);

        assert_eq!(sched.queue_len(), 1);
        assert_eq!(sched.priority_queue[0].task_type, DreamType::Experiment);
        assert!((sched.priority_queue[0].priority - 0.7).abs() < f64::EPSILON);
    }

    #[test]
    fn high_curiosity_queues_explore() {
        let mut sched = DreamScheduler::new();
        let cons = make_consciousness(0.0, 0.9, false);
        let interaction = make_interaction(false);

        let hm = TestHm::new();
        queue_dreams_from_interaction(&mut sched, &agent_turn(&hm), &cons, &interaction);

        assert_eq!(sched.queue_len(), 1);
        assert_eq!(sched.priority_queue[0].task_type, DreamType::Explore);
    }

    #[test]
    fn error_queues_replay() {
        let mut sched = DreamScheduler::new();
        let cons = make_consciousness(0.0, 0.3, false);
        let interaction = make_interaction(true);

        let hm = TestHm::new();
        queue_dreams_from_interaction(&mut sched, &agent_turn(&hm), &cons, &interaction);

        assert_eq!(sched.queue_len(), 1);
        assert_eq!(sched.priority_queue[0].task_type, DreamType::Replay);
    }

    #[test]
    fn needs_handoff_queues_create() {
        let mut sched = DreamScheduler::new();
        let cons = make_consciousness(0.0, 0.3, true);
        let interaction = make_interaction(false);

        let hm = TestHm::new();
        queue_dreams_from_interaction(&mut sched, &agent_turn(&hm), &cons, &interaction);

        assert_eq!(sched.queue_len(), 1);
        assert_eq!(sched.priority_queue[0].task_type, DreamType::Create);
    }

    #[test]
    fn multiple_triggers_queue_multiple_dreams() {
        let mut sched = DreamScheduler::new();
        let cons = make_consciousness(0.8, 0.9, true);
        let interaction = make_interaction(true);

        let hm = TestHm::new();
        queue_dreams_from_interaction(&mut sched, &agent_turn(&hm), &cons, &interaction);

        // Frustration → Experiment, Curiosity → Explore, Error → Replay, Handoff → Create
        assert_eq!(sched.queue_len(), 4);
    }

    /// P3-HM-CONTRACT-02 §6.5, §11.3 (A-D4): every entry is owned by the
    /// turn's real principal and carries the turn's label, never the
    /// ephemeral chat id; an `OwnerDirect` turn's entries are the Owner's;
    /// a context that is not a chat turn queues nothing.
    #[test]
    fn hm_m3_n4_entries_carry_the_turns_principal_and_label() {
        let hm = TestHm::new();
        let cons = make_consciousness(0.8, 0.9, true);
        let interaction = make_interaction(true);

        let turn = agent_turn(&hm);
        let expected = turn.entry_label().unwrap();
        let Principal::Agent(agent) = expected.principal() else {
            panic!("an agent's turn");
        };
        // The agent is local-only (the restrictive default).
        assert_eq!(expected.label(), OriginLabel::LocalOnlyOrigin);
        let mut sched = DreamScheduler::new();
        queue_dreams_from_interaction(&mut sched, &turn, &cons, &interaction);
        assert_eq!(sched.queue_len(), 4);
        for task in &sched.priority_queue {
            assert_eq!(task.label(), Some(&expected));
            assert_eq!(task.source_agent, agent.to_string());
        }

        let owner = hm
            .issuer
            .begin_unit(
                Principal::OwnerDirect,
                Unit::ChatTurn {
                    turn_id: uuid::Uuid::new_v4(),
                },
            )
            .unwrap();
        let mut sched = DreamScheduler::new();
        queue_dreams_from_interaction(&mut sched, &owner, &cons, &interaction);
        assert_eq!(sched.queue_len(), 4);
        for task in &sched.priority_queue {
            assert_eq!(task.label(), owner.entry_label().as_ref());
            assert_eq!(task.label().unwrap().principal(), Principal::OwnerDirect);
            assert_eq!(task.source_agent, "owner");
        }

        let goal = hm.goal(hm.agent("dream-goal"));
        let mut sched = DreamScheduler::new();
        queue_dreams_from_interaction(&mut sched, &goal, &cons, &interaction);
        assert_eq!(sched.queue_len(), 0);
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

        let hm = TestHm::new();
        queue_dreams_from_interaction(&mut sched, &agent_turn(&hm), &cons, &interaction);

        assert_eq!(sched.queue_len(), 0);
    }
}
