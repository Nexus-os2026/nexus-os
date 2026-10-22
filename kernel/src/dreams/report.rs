//! Morning briefing — summary of overnight dream results.

use super::engine::{dream_unit, DreamLlm};
use super::scheduler::DreamScheduler;
use super::types::{DreamOutcome, DreamResult};
use crate::hm::{DispatchContext, DispatchIssuer, EntryLabel, PromptBuilder};
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
    /// Generate a briefing from the scheduler's completed dreams. The
    /// briefing is a derived item of every contributing entry (§11.3): its
    /// unit starts from the ⊔ of their labels when they share one principal,
    /// and is `LOCAL_ONLY_ORIGIN` (never non-local) otherwise.
    pub fn generate(
        scheduler: &DreamScheduler,
        issuer: &DispatchIssuer,
        llm: &dyn DreamLlm,
    ) -> Self {
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
            let labels: Option<Vec<&EntryLabel>> = dreams.iter().map(|d| d.label()).collect();
            let combined = labels.and_then(|labels| EntryLabel::combine(&labels));
            let briefing = dream_unit(issuer, combined.as_ref()).and_then(|mut ctx| {
                llm.query(
                    &mut ctx,
                    &mut |c: &mut DispatchContext| {
                        let mut b = PromptBuilder::new(c);
                        b.text(&format!("[System: {system}]\n\n{user}"));
                        Some(b.build())
                    },
                    300,
                )
            });
            match briefing {
                Ok((text, _)) => text.text().to_string(),
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
    use crate::hm::testing::{ScriptedLlm, TestHm};
    use crate::hm::RouteClass;

    fn mock() -> (TestHm, ScriptedLlm) {
        let hm = TestHm::new();
        let llm = ScriptedLlm::answering(&hm.issuer, RouteClass::Local, |prompt| {
            format!("Mock dream response for: {prompt}")
        });
        (hm, llm)
    }
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
            label: None,
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
            label: None,
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
            label: None,
        });
        s
    }

    #[test]
    fn briefing_categorizes_results() {
        let scheduler = make_scheduler_with_dreams();
        let (hm, llm) = mock();
        let briefing = MorningBriefing::generate(&scheduler, &hm.issuer, &llm);

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
        let (hm, llm) = mock();
        let briefing = MorningBriefing::generate(&scheduler, &hm.issuer, &llm);
        assert!(!briefing.summary.is_empty());
    }

    #[test]
    fn empty_briefing() {
        let scheduler = DreamScheduler::new();
        let (hm, llm) = mock();
        let briefing = MorningBriefing::generate(&scheduler, &hm.issuer, &llm);
        assert_eq!(briefing.total_dreams, 0);
        assert!(briefing.summary.contains("No dreams"));
    }

    /// P3-C11-HM-M4 N1 C-5: a morning briefing over the dream results of two
    /// principals is never non-local. With only a non-local transport it is
    /// refused (the briefing keeps its fallback text) and no request is made.
    #[test]
    fn hm_m4_n1_c5_a_briefing_over_two_principals_is_refused_without_a_local_transport() {
        use crate::hm::{OriginLabel, Principal, Unit};
        let hm = TestHm::new();
        let agent = hm.open_agent("c5-briefing-agent");
        let label_of = |principal| {
            hm.issuer
                .begin_unit(
                    principal,
                    Unit::ChatTurn {
                        turn_id: uuid::Uuid::new_v4(),
                    },
                )
                .unwrap()
                .entry_label()
                .unwrap()
        };
        let owner = label_of(Principal::OwnerDirect);
        let agent = label_of(Principal::Agent(agent));
        assert_eq!(
            (owner.label(), agent.label()),
            (
                OriginLabel::UnrestrictedOrigin,
                OriginLabel::UnrestrictedOrigin
            ),
            "precondition: each entry alone would be unrestricted"
        );
        let mut scheduler = make_scheduler_with_dreams();
        scheduler.completed_dreams[0].label = Some(owner.clone());
        scheduler.completed_dreams[1].label = Some(agent);
        scheduler.completed_dreams[2].label = Some(owner);
        let remote = ScriptedLlm::replying(&hm.issuer, RouteClass::NonLocal, "a cloud briefing");
        let briefing = MorningBriefing::generate(&scheduler, &hm.issuer, &remote);
        assert!(
            remote.sent().is_empty(),
            "C-5: PROHIBITED NON-LOCAL BRIEFING DISPATCH: {:?}",
            remote.sent()
        );
        assert!(
            briefing.summary.starts_with("Good morning. Overnight"),
            "C-5: the briefing was not refused: {}",
            briefing.summary
        );
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
            label: None,
        }];
        let text = build_bullet_points(&improvements, &[], &[], &[]);
        assert!(text.contains("Improved: Code quality"));
        assert!(text.contains("5 → 8"));
    }
}
