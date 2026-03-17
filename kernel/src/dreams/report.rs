//! Morning briefing — summary of overnight dream results.

use super::engine::DreamLlm;
use super::scheduler::DreamScheduler;
use super::types::{DreamOutcome, DreamResult};
use serde::{Deserialize, Serialize};

/// A human-readable morning briefing summarizing dream results.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MorningBriefing {
    pub dream_session_start: u64,
    pub dream_session_end: u64,
    pub total_dreams: u32,
    pub tokens_spent: u64,
    pub improvements: Vec<DreamResult>,
    pub discoveries: Vec<DreamResult>,
    pub new_agents: Vec<DreamResult>,
    pub precomputed: Vec<DreamResult>,
    pub failures: Vec<DreamResult>,
    /// LLM-generated natural language summary.
    pub summary: String,
}

impl MorningBriefing {
    /// Generate a briefing from the scheduler's completed dreams.
    pub fn generate(scheduler: &DreamScheduler, llm: &dyn DreamLlm) -> Self {
        let dreams = &scheduler.completed_dreams;
        let session_start = scheduler.last_dream_at.unwrap_or(0);

        let mut improvements = Vec::new();
        let mut discoveries = Vec::new();
        let mut new_agents = Vec::new();
        let mut precomputed = Vec::new();
        let mut failures = Vec::new();
        let mut tokens_spent: u64 = 0;

        for d in dreams {
            tokens_spent += d.tokens_used;
            match &d.outcome {
                DreamOutcome::Improvement { .. } => improvements.push(d.clone()),
                DreamOutcome::Discovery { .. } => discoveries.push(d.clone()),
                DreamOutcome::Creation { .. } => new_agents.push(d.clone()),
                DreamOutcome::Precomputed { .. } => precomputed.push(d.clone()),
                DreamOutcome::NoResult { .. } => failures.push(d.clone()),
            }
        }

        let total = dreams.len() as u32;
        let session_end = dreams
            .last()
            .map(|d| d.completed_at)
            .unwrap_or(session_start);

        // Build a structured summary for the LLM to humanize
        let bullet_points =
            build_bullet_points(&improvements, &discoveries, &new_agents, &precomputed);

        let summary = if total == 0 {
            "No dreams were processed in this cycle.".to_string()
        } else {
            let system = "You are a concise assistant. Generate a friendly morning briefing \
                          from the following dream results. Start with 'Good morning.' Keep it \
                          under 5 sentences. Include key metrics.";
            let user = format!(
                "Dream results ({total} dreams, {tokens_spent} tokens used):\n{bullet_points}"
            );
            match llm.query(system, &user, 300) {
                Ok((text, _)) => text,
                Err(_) => format!(
                    "Good morning. Overnight, I completed {total} dream tasks \
                     using {tokens_spent} tokens. {bullet_points}"
                ),
            }
        };

        Self {
            dream_session_start: session_start,
            dream_session_end: session_end,
            total_dreams: total,
            tokens_spent,
            improvements,
            discoveries,
            new_agents,
            precomputed,
            failures,
            summary,
        }
    }
}

fn build_bullet_points(
    improvements: &[DreamResult],
    discoveries: &[DreamResult],
    new_agents: &[DreamResult],
    precomputed: &[DreamResult],
) -> String {
    let mut lines = Vec::new();

    for r in improvements {
        if let DreamOutcome::Improvement {
            description,
            before_score,
            after_score,
            ..
        } = &r.outcome
        {
            lines.push(format!(
                "- Improved: {description} (score {before_score:.0} → {after_score:.0})"
            ));
        }
    }
    for r in discoveries {
        if let DreamOutcome::Discovery { description, .. } = &r.outcome {
            lines.push(format!("- Discovered: {description}"));
        }
    }
    for r in new_agents {
        if let DreamOutcome::Creation {
            new_agent_id,
            reason,
            ..
        } = &r.outcome
        {
            lines.push(format!("- Created agent {new_agent_id}: {reason}"));
        }
    }
    for r in precomputed {
        if let DreamOutcome::Precomputed {
            predicted_request, ..
        } = &r.outcome
        {
            lines.push(format!("- Pre-prepared response for: {predicted_request}"));
        }
    }

    if lines.is_empty() {
        "No notable results.".to_string()
    } else {
        lines.join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dreams::engine::MockDreamLlm;
    use crate::dreams::types::DreamType;

    fn make_scheduler_with_dreams() -> DreamScheduler {
        let mut s = DreamScheduler::new();
        s.last_dream_at = Some(1000);

        s.completed_dreams.push(DreamResult {
            task_id: "t1".into(),
            dream_type: DreamType::Replay,
            agent_id: "forge".into(),
            started_at: 1000,
            completed_at: 1100,
            tokens_used: 200,
            outcome: DreamOutcome::Improvement {
                description: "Code review accuracy".into(),
                before_score: 6.0,
                after_score: 9.0,
                artifact: None,
            },
        });
        s.completed_dreams.push(DreamResult {
            task_id: "t2".into(),
            dream_type: DreamType::Explore,
            agent_id: "analyst".into(),
            started_at: 1100,
            completed_at: 1200,
            tokens_used: 300,
            outcome: DreamOutcome::Discovery {
                description: "Faster sorting algorithm".into(),
                relevance: 0.8,
                shared_with: vec!["forge".into()],
            },
        });
        s.completed_dreams.push(DreamResult {
            task_id: "t3".into(),
            dream_type: DreamType::Create,
            agent_id: "genesis".into(),
            started_at: 1200,
            completed_at: 1300,
            tokens_used: 400,
            outcome: DreamOutcome::Creation {
                new_agent_id: "nexus-dataclean".into(),
                reason: "CSV cleaning tasks".into(),
                test_score: 0.85,
            },
        });
        s
    }

    #[test]
    fn briefing_categorizes_results() {
        let scheduler = make_scheduler_with_dreams();
        let briefing = MorningBriefing::generate(&scheduler, &MockDreamLlm);

        assert_eq!(briefing.total_dreams, 3);
        assert_eq!(briefing.tokens_spent, 900);
        assert_eq!(briefing.improvements.len(), 1);
        assert_eq!(briefing.discoveries.len(), 1);
        assert_eq!(briefing.new_agents.len(), 1);
        assert!(briefing.precomputed.is_empty());
    }

    #[test]
    fn briefing_has_summary() {
        let scheduler = make_scheduler_with_dreams();
        let briefing = MorningBriefing::generate(&scheduler, &MockDreamLlm);
        assert!(!briefing.summary.is_empty());
    }

    #[test]
    fn empty_briefing() {
        let scheduler = DreamScheduler::new();
        let briefing = MorningBriefing::generate(&scheduler, &MockDreamLlm);
        assert_eq!(briefing.total_dreams, 0);
        assert!(briefing.summary.contains("No dreams"));
    }

    #[test]
    fn bullet_points_format() {
        let improvements = vec![DreamResult {
            task_id: "t1".into(),
            dream_type: DreamType::Replay,
            agent_id: "a".into(),
            started_at: 0,
            completed_at: 1,
            tokens_used: 100,
            outcome: DreamOutcome::Improvement {
                description: "Code quality".into(),
                before_score: 5.0,
                after_score: 8.0,
                artifact: None,
            },
        }];
        let text = build_bullet_points(&improvements, &[], &[], &[]);
        assert!(text.contains("Improved: Code quality"));
        assert!(text.contains("5 → 8"));
    }
}
