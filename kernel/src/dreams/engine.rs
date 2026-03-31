//! Dream engine — the core loop that processes dream tasks.
//!
//! Each dream method takes a task, calls the LLM within budget, and
//! produces a [`DreamResult`].

use super::report::MorningBriefing;
use super::scheduler::DreamScheduler;
use super::types::{DreamOutcome, DreamResult, DreamTask, DreamType};
use crate::consciousness::state::now_secs;
use serde_json::json;

/// Trait for LLM calls during dream state (injectable for testing).
pub trait DreamLlm: Send + Sync {
    /// Query the LLM with a prompt. Returns the response text and token count.
    fn query(&self, system: &str, user: &str, max_tokens: u32) -> Result<(String, u64), String>;
}

/// A no-op LLM for unit tests.
pub struct MockDreamLlm;
impl DreamLlm for MockDreamLlm {
    fn query(&self, _system: &str, user: &str, _max_tokens: u32) -> Result<(String, u64), String> {
        Ok((format!("Mock dream response for: {user}"), 50))
    }
}

/// The core dream engine that processes the dream queue.
pub struct DreamEngine {
    pub scheduler: DreamScheduler,
}

impl DreamEngine {
    pub fn new(scheduler: DreamScheduler) -> Self {
        Self { scheduler }
    }

    /// Main dream loop — processes priority queue within budget.
    /// Returns all completed dream results.
    pub fn enter_dream_state(&mut self, llm: &dyn DreamLlm) -> Vec<DreamResult> {
        let mut results = Vec::new();

        self.scheduler.sort_queue();
        self.scheduler.last_dream_at = Some(now_secs());

        let mut tokens_used: u64 = 0;
        let mut calls_made: u32 = 0;

        // Snapshot task IDs to process (avoid borrow issues).
        let task_snapshot: Vec<DreamTask> = self.scheduler.priority_queue.clone();

        for task in &task_snapshot {
            // Budget check — strict enforcement
            if tokens_used >= self.scheduler.dream_budget_tokens
                || calls_made >= self.scheduler.dream_budget_api_calls
            {
                break;
            }

            let remaining_tokens = self.scheduler.dream_budget_tokens - tokens_used;

            let result = match task.task_type {
                DreamType::Replay => self.dream_replay(llm, task, remaining_tokens),
                DreamType::Experiment => self.dream_experiment(llm, task, remaining_tokens),
                DreamType::Consolidate => self.dream_consolidate(llm, task, remaining_tokens),
                DreamType::Explore => self.dream_explore(llm, task, remaining_tokens),
                DreamType::Precompute => self.dream_precompute(llm, task, remaining_tokens),
                DreamType::Create => self.dream_create(llm, task, remaining_tokens),
                DreamType::Optimize => self.dream_optimize(llm, task, remaining_tokens),
            };

            match result {
                Ok(r) => {
                    tokens_used += r.tokens_used;
                    calls_made += 1;
                    self.scheduler.record_result(r.clone());
                    results.push(r);
                }
                Err(e) => {
                    // Record failure as NoResult, still consume a call slot.
                    let fail_result = DreamResult {
                        task_id: task.id.clone(),
                        dream_type: task.task_type.clone(),
                        agent_id: task.source_agent.clone(),
                        started_at: now_secs(),
                        completed_at: now_secs(),
                        tokens_used: 0,
                        outcome: DreamOutcome::NoResult { reason: e },
                    };
                    calls_made += 1;
                    self.scheduler.record_result(fail_result.clone());
                    results.push(fail_result);
                }
            }
        }

        results
    }

    /// Generate a morning briefing from the last dream session.
    pub fn generate_morning_briefing(&self, llm: &dyn DreamLlm) -> MorningBriefing {
        MorningBriefing::generate(&self.scheduler, llm)
    }

    // ── Individual dream implementations ────────────────────────────

    fn dream_replay(
        &self,
        llm: &dyn DreamLlm,
        task: &DreamTask,
        _remaining_budget: u64,
    ) -> Result<DreamResult, String> {
        let started = now_secs();

        let original_task = task
            .context
            .get("task")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown task");
        let original_response = task
            .context
            .get("original_response")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let system = "You are replaying a past task to find improvements. \
                       Score the original response 0-10, then produce an improved version. \
                       Format: ORIGINAL_SCORE: N\nIMPROVED_SCORE: N\nIMPROVED:\n<response>";
        let user = format!(
            "Original task: {original_task}\nOriginal response: {original_response}\n\nReplay and improve."
        );

        let (response, tokens) = llm.query(system, &user, 600)?;
        let (before, after) = parse_scores(&response);

        Ok(DreamResult {
            task_id: task.id.clone(),
            dream_type: DreamType::Replay,
            agent_id: task.source_agent.clone(),
            started_at: started,
            completed_at: now_secs(),
            tokens_used: tokens,
            outcome: DreamOutcome::Improvement {
                description: format!("Replay of: {original_task}"),
                before_score: before,
                after_score: after,
                artifact: None,
            },
        })
    }

    fn dream_experiment(
        &self,
        llm: &dyn DreamLlm,
        task: &DreamTask,
        _remaining_budget: u64,
    ) -> Result<DreamResult, String> {
        let started = now_secs();

        let failed_task = task
            .context
            .get("task")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown task");
        let failures = task
            .context
            .get("failures")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let system = "You are experimenting with different approaches to a failed task. \
                       Generate 3 different strategies. For each, give a confidence score 0-10. \
                       Format: STRATEGY_1: <desc> SCORE: N\nSTRATEGY_2: <desc> SCORE: N\nSTRATEGY_3: <desc> SCORE: N\n\
                       BEST_APPROACH:\n<detailed best approach>";
        let user = format!(
            "Task: {failed_task}\nPrevious failures: {failures}\n\nExperiment with new approaches."
        );

        let (response, tokens) = llm.query(system, &user, 800)?;
        let best_score = parse_best_score(&response);

        Ok(DreamResult {
            task_id: task.id.clone(),
            dream_type: DreamType::Experiment,
            agent_id: task.source_agent.clone(),
            started_at: started,
            completed_at: now_secs(),
            tokens_used: tokens,
            outcome: DreamOutcome::Improvement {
                description: format!("Experiment on: {failed_task}"),
                before_score: 0.0,
                after_score: best_score,
                artifact: None,
            },
        })
    }

    fn dream_consolidate(
        &self,
        llm: &dyn DreamLlm,
        task: &DreamTask,
        _remaining_budget: u64,
    ) -> Result<DreamResult, String> {
        let started = now_secs();

        let session_summary = task
            .context
            .get("session_summary")
            .and_then(|v| v.as_str())
            .unwrap_or("No session data");

        let system = "Analyze the day's work session. Identify patterns, lessons learned, \
                       and concrete improvements. Output a compressed lesson that could be \
                       added to the agent's system prompt. Format: LESSON:\n<lesson text>";
        let user = format!("Session summary:\n{session_summary}");

        let (response, tokens) = llm.query(system, &user, 600)?;
        let lesson = extract_after_marker(&response, "LESSON:");

        Ok(DreamResult {
            task_id: task.id.clone(),
            dream_type: DreamType::Consolidate,
            agent_id: task.source_agent.clone(),
            started_at: started,
            completed_at: now_secs(),
            tokens_used: tokens,
            outcome: DreamOutcome::Discovery {
                description: lesson,
                relevance: 0.8,
                shared_with: vec![task.source_agent.clone()],
            },
        })
    }

    fn dream_explore(
        &self,
        llm: &dyn DreamLlm,
        task: &DreamTask,
        _remaining_budget: u64,
    ) -> Result<DreamResult, String> {
        let started = now_secs();

        let topic = task
            .context
            .get("topic")
            .and_then(|v| v.as_str())
            .unwrap_or("general AI improvements");

        let system = "Research the given topic. Provide key insights, practical applications, \
                       and relevance to software engineering. Be concise but thorough.";
        let user = format!("Research topic: {topic}");

        let (response, tokens) = llm.query(system, &user, 600)?;

        Ok(DreamResult {
            task_id: task.id.clone(),
            dream_type: DreamType::Explore,
            agent_id: task.source_agent.clone(),
            started_at: started,
            completed_at: now_secs(),
            tokens_used: tokens,
            outcome: DreamOutcome::Discovery {
                description: response,
                relevance: 0.5,
                shared_with: vec![],
            },
        })
    }

    fn dream_precompute(
        &self,
        llm: &dyn DreamLlm,
        task: &DreamTask,
        _remaining_budget: u64,
    ) -> Result<DreamResult, String> {
        let started = now_secs();

        let conversation_context = task
            .context
            .get("conversation")
            .and_then(|v| v.as_str())
            .unwrap_or("general programming session");

        let system = "Based on the conversation context, predict the most likely next request \
                       the user will make. Then provide a high-quality response to that predicted request. \
                       Format: PREDICTION: <predicted request>\nCONFIDENCE: <0.0-1.0>\nRESPONSE:\n<response>";
        let user = format!("Conversation so far:\n{conversation_context}");

        let (response, tokens) = llm.query(system, &user, 800)?;
        let prediction = extract_after_marker(&response, "PREDICTION:");
        let confidence = parse_confidence(&response);
        let prepared = extract_after_marker(&response, "RESPONSE:");

        Ok(DreamResult {
            task_id: task.id.clone(),
            dream_type: DreamType::Precompute,
            agent_id: task.source_agent.clone(),
            started_at: started,
            completed_at: now_secs(),
            tokens_used: tokens,
            outcome: DreamOutcome::Precomputed {
                predicted_request: prediction,
                prepared_response: prepared,
                confidence,
            },
        })
    }

    fn dream_create(
        &self,
        llm: &dyn DreamLlm,
        task: &DreamTask,
        _remaining_budget: u64,
    ) -> Result<DreamResult, String> {
        let started = now_secs();

        let gap_description = task
            .context
            .get("gap")
            .and_then(|v| v.as_str())
            .unwrap_or("unspecified capability gap");
        let missing_caps = task
            .context
            .get("missing_capabilities")
            .cloned()
            .unwrap_or_else(|| json!([]));

        let system = "Design a new agent to fill a capability gap. Provide: \
                       NAME: <agent-name>\nDESCRIPTION: <what it does>\n\
                       CAPABILITIES: <comma-separated list>\nTEST_SCORE: <0.0-1.0>\n\
                       REASON: <why this agent is needed>";
        let user = format!("Gap: {gap_description}\nMissing capabilities: {missing_caps}");

        let (response, tokens) = llm.query(system, &user, 600)?;
        let name = extract_after_marker(&response, "NAME:");
        let reason = extract_after_marker(&response, "REASON:");
        let test_score = parse_test_score(&response);

        Ok(DreamResult {
            task_id: task.id.clone(),
            dream_type: DreamType::Create,
            agent_id: task.source_agent.clone(),
            started_at: started,
            completed_at: now_secs(),
            tokens_used: tokens,
            outcome: DreamOutcome::Creation {
                new_agent_id: name,
                reason,
                test_score,
            },
        })
    }

    fn dream_optimize(
        &self,
        llm: &dyn DreamLlm,
        task: &DreamTask,
        _remaining_budget: u64,
    ) -> Result<DreamResult, String> {
        let started = now_secs();

        let work_product = task
            .context
            .get("work_product")
            .and_then(|v| v.as_str())
            .unwrap_or("no work product available");

        let system = "Review this work product and find concrete improvements. \
                       Score the original 0-10, then provide an optimized version scored 0-10. \
                       Format: ORIGINAL_SCORE: N\nIMPROVED_SCORE: N\nIMPROVEMENTS:\n<list>";
        let user = format!("Work product to optimize:\n{work_product}");

        let (response, tokens) = llm.query(system, &user, 600)?;
        let (before, after) = parse_scores(&response);

        Ok(DreamResult {
            task_id: task.id.clone(),
            dream_type: DreamType::Optimize,
            agent_id: task.source_agent.clone(),
            started_at: started,
            completed_at: now_secs(),
            tokens_used: tokens,
            outcome: DreamOutcome::Improvement {
                description: extract_after_marker(&response, "IMPROVEMENTS:"),
                before_score: before,
                after_score: after,
                artifact: None,
            },
        })
    }
}

// ── Parsing helpers ─────────────────────────────────────────────────

fn parse_scores(response: &str) -> (f64, f64) {
    let before = extract_number_after(response, "ORIGINAL_SCORE:").unwrap_or(5.0);
    let after = extract_number_after(response, "IMPROVED_SCORE:").unwrap_or(5.0);
    (before, after)
}

fn parse_best_score(response: &str) -> f64 {
    // Find highest SCORE: N in the response
    let mut best = 0.0_f64;
    for line in response.lines() {
        if let Some(score) = extract_number_after(line, "SCORE:") {
            best = best.max(score);
        }
    }
    if best == 0.0 {
        5.0 // fallback
    } else {
        best
    }
}

fn parse_confidence(response: &str) -> f64 {
    extract_number_after(response, "CONFIDENCE:").unwrap_or(0.5)
}

fn parse_test_score(response: &str) -> f64 {
    extract_number_after(response, "TEST_SCORE:").unwrap_or(0.5)
}

fn extract_number_after(text: &str, marker: &str) -> Option<f64> {
    let lower = text.to_lowercase();
    let marker_lower = marker.to_lowercase();
    if let Some(idx) = lower.find(&marker_lower) {
        let after = &text[idx + marker.len()..];
        let trimmed = after.trim();
        // Take until non-numeric
        let num_str: String = trimmed
            .chars()
            .take_while(|c| c.is_ascii_digit() || *c == '.')
            .collect();
        num_str.parse().ok() // Optional: non-numeric text means no number found, return None
    } else {
        None
    }
}

fn extract_after_marker(text: &str, marker: &str) -> String {
    let lower = text.to_lowercase();
    let marker_lower = marker.to_lowercase();
    if let Some(idx) = lower.find(&marker_lower) {
        let after = &text[idx + marker.len()..];
        // Take until next marker or end
        let trimmed = after.trim();
        // Take first line or paragraph
        trimmed.lines().next().unwrap_or(trimmed).trim().to_string()
    } else {
        text.lines().next().unwrap_or(text).trim().to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_scores_works() {
        let text = "ORIGINAL_SCORE: 6\nIMPROVED_SCORE: 9\nIMPROVED:\nbetter code";
        let (b, a) = parse_scores(text);
        assert!((b - 6.0).abs() < f64::EPSILON);
        assert!((a - 9.0).abs() < f64::EPSILON);
    }

    #[test]
    fn parse_scores_fallback() {
        let (b, a) = parse_scores("no scores here");
        assert!((b - 5.0).abs() < f64::EPSILON);
        assert!((a - 5.0).abs() < f64::EPSILON);
    }

    #[test]
    fn extract_number_after_works() {
        assert_eq!(extract_number_after("SCORE: 7.5 foo", "SCORE:"), Some(7.5));
        assert_eq!(extract_number_after("no match", "SCORE:"), None);
    }

    #[test]
    fn extract_after_marker_works() {
        let text = "PREDICTION: user will ask about testing\nCONFIDENCE: 0.8";
        assert_eq!(
            extract_after_marker(text, "PREDICTION:"),
            "user will ask about testing"
        );
    }

    #[test]
    fn dream_engine_with_mock_llm() {
        let mut scheduler = DreamScheduler::new();
        scheduler.enqueue(DreamTask {
            id: "t1".into(),
            task_type: DreamType::Replay,
            priority: 0.8,
            source_agent: "agent-a".into(),
            context: serde_json::json!({"task": "write hello world", "original_response": "print('hello')"}),
            ..DreamTask::default()
        });
        scheduler.enqueue(DreamTask {
            id: "t2".into(),
            task_type: DreamType::Explore,
            priority: 0.3,
            source_agent: "agent-b".into(),
            context: serde_json::json!({"topic": "rust async patterns"}),
            ..DreamTask::default()
        });

        let mut engine = DreamEngine::new(scheduler);
        let results = engine.enter_dream_state(&MockDreamLlm);

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].dream_type, DreamType::Replay);
        assert_eq!(results[1].dream_type, DreamType::Explore);
        assert!(engine.scheduler.priority_queue.is_empty());
        assert_eq!(engine.scheduler.completed_dreams.len(), 2);
    }

    #[test]
    fn dream_engine_respects_call_budget() {
        let mut scheduler = DreamScheduler::new();
        scheduler.dream_budget_api_calls = 2;

        for i in 0..5 {
            scheduler.enqueue(DreamTask {
                id: format!("t{i}"),
                task_type: DreamType::Explore,
                priority: 0.5,
                source_agent: "a".into(),
                context: serde_json::json!({"topic": "test"}),
                ..DreamTask::default()
            });
        }

        let mut engine = DreamEngine::new(scheduler);
        let results = engine.enter_dream_state(&MockDreamLlm);

        assert_eq!(results.len(), 2); // budget limits to 2
        assert_eq!(engine.scheduler.queue_len(), 3); // 3 remaining
    }

    #[test]
    fn dream_engine_respects_token_budget() {
        let mut scheduler = DreamScheduler::new();
        scheduler.dream_budget_tokens = 90; // MockDreamLlm returns 50 tokens per call

        for i in 0..5 {
            scheduler.enqueue(DreamTask {
                id: format!("t{i}"),
                task_type: DreamType::Explore,
                priority: 0.5,
                source_agent: "a".into(),
                context: serde_json::json!({"topic": "test"}),
                ..DreamTask::default()
            });
        }

        let mut engine = DreamEngine::new(scheduler);
        let results = engine.enter_dream_state(&MockDreamLlm);

        // 50 tokens first call (50 < 90 → proceed), 50 second call (100 >= 90 → stop)
        assert_eq!(results.len(), 2);
        assert_eq!(engine.scheduler.queue_len(), 3);
    }

    #[test]
    fn dream_all_types() {
        let types = vec![
            (
                DreamType::Replay,
                serde_json::json!({"task": "test", "original_response": "resp"}),
            ),
            (
                DreamType::Experiment,
                serde_json::json!({"task": "test", "failures": "err"}),
            ),
            (
                DreamType::Consolidate,
                serde_json::json!({"session_summary": "did stuff"}),
            ),
            (DreamType::Explore, serde_json::json!({"topic": "rust"})),
            (
                DreamType::Precompute,
                serde_json::json!({"conversation": "we built an API"}),
            ),
            (
                DreamType::Create,
                serde_json::json!({"gap": "need data cleaner", "missing_capabilities": ["csv.parse"]}),
            ),
            (
                DreamType::Optimize,
                serde_json::json!({"work_product": "fn foo() {}"}),
            ),
        ];

        for (i, (dtype, ctx)) in types.into_iter().enumerate() {
            let mut scheduler = DreamScheduler::new();
            scheduler.enqueue(DreamTask {
                id: format!("t{i}"),
                task_type: dtype.clone(),
                priority: 0.5,
                source_agent: "a".into(),
                context: ctx,
                ..DreamTask::default()
            });
            let mut engine = DreamEngine::new(scheduler);
            let results = engine.enter_dream_state(&MockDreamLlm);
            assert_eq!(results.len(), 1, "Failed for {dtype:?}");
            assert_eq!(results[0].dream_type, dtype);
        }
    }

    #[test]
    fn parse_best_score_multi() {
        let text = "STRATEGY_1: foo SCORE: 3\nSTRATEGY_2: bar SCORE: 8\nSTRATEGY_3: baz SCORE: 5";
        assert!((parse_best_score(text) - 8.0).abs() < f64::EPSILON);
    }
}
