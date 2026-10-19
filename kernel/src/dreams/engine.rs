//! Dream engine — the core loop that processes dream tasks.
//!
//! Each dream method takes a task, calls the LLM within budget, and
//! produces a [`DreamResult`].

use super::report::MorningBriefing;
use super::scheduler::DreamScheduler;
use super::types::{DreamOutcome, DreamResult, DreamTask, DreamType};
use crate::consciousness::state::now_secs;
use crate::hm::{DerivedItem, DispatchContext, DispatchIssuer, PromptBuilder, PromptSource, Unit};
use serde_json::json;

/// The dream model (P3-HM-CONTRACT-02 §11.3, §11.4): every call runs under
/// the dream unit's context (started from the entry's label); the reply is
/// a derived item, with the tokens it used.
pub trait DreamLlm: Send + Sync {
    fn query(
        &self,
        ctx: &mut DispatchContext,
        prompt: &mut PromptSource<'_>,
        max_tokens: u32,
    ) -> Result<(DerivedItem, u64), String>;
}

/// One dream call: the system instruction and the user text as backend
/// text of the dream unit, whose taint already carries the entry's label.
fn dream_query(
    llm: &dyn DreamLlm,
    ctx: &mut DispatchContext,
    system: &str,
    user: &str,
    max_tokens: u32,
) -> Result<(String, u64), String> {
    let (reply, tokens) = llm.query(
        ctx,
        &mut |c: &mut DispatchContext| {
            let mut b = PromptBuilder::new(c);
            b.text(&format!("[System: {system}]\n\n{user}"));
            Some(b.build())
        },
        max_tokens,
    )?;
    Ok((reply.text().to_string(), tokens))
}

/// The dream unit of one queued entry (O1, §11.3, §12.1): an agent-owned
/// entry is a dream unit of that agent (AR); an `OwnerDirect` entry is HR;
/// an entry without a label is `LOCAL_ONLY_ORIGIN` and never non-local.
///
/// Every dream unit is then made local-only (mission P3-C11-HM-M4 C-2,
/// F-N6-2): an entry holds the human's raw chat text, which a non-local
/// chat once sent through the gateway's redaction and a dream would now
/// re-send unredacted. Dreams (and the morning briefing) dispatch only on
/// an attested local transport, or are refused. Tightening is always
/// permitted (§7.5).
pub(crate) fn dream_unit(
    issuer: &DispatchIssuer,
    label: Option<&crate::hm::EntryLabel>,
) -> Result<DispatchContext, String> {
    let unit = Unit::Dream {
        entry_id: uuid::Uuid::new_v4(),
    };
    let mut ctx = match label {
        Some(label) => issuer.begin_entry_unit(label, unit),
        None => issuer.begin_unlabelled_entry_unit(unit),
    }
    .map_err(|e| e.to_string())?;
    ctx.tighten_local_only();
    Ok(ctx)
}

/// The core dream engine that processes the dream queue.
pub struct DreamEngine {
    pub scheduler: DreamScheduler,
}

/// One dream session's entries and budgets, taken from the engine under its
/// lock (P3-HM-CONTRACT-02 §13.2 dream LK-1); its dreams then run with the
/// lock released.
#[derive(Debug, Clone)]
pub struct DreamSession {
    pub tasks: Vec<DreamTask>,
    pub budget_tokens: u64,
    pub budget_calls: u32,
}

impl DreamSession {
    /// LK-2 and LK-3 of a session: each entry within the budgets is dreamed
    /// with no engine borrowed (no lock held across its model call), then
    /// handed to `record` (which re-locks and records the result, or drops it
    /// when the entry was cleared meanwhile). Returns the results recorded.
    pub fn run(
        self,
        issuer: &DispatchIssuer,
        llm: &dyn DreamLlm,
        mut record: impl FnMut(DreamResult) -> bool,
    ) -> Vec<DreamResult> {
        let mut results = Vec::new();
        let mut tokens_used: u64 = 0;
        // Every entry dreamed uses one call, a failed one included.
        for (calls_made, task) in (0_u32..).zip(self.tasks.iter()) {
            // Budget check — strict enforcement
            if tokens_used >= self.budget_tokens || calls_made >= self.budget_calls {
                break;
            }
            let result =
                DreamEngine::dream_entry(issuer, llm, task, self.budget_tokens - tokens_used);
            tokens_used += result.tokens_used;
            if record(result.clone()) {
                results.push(result);
            }
        }
        results
    }
}

impl DreamEngine {
    pub fn new(scheduler: DreamScheduler) -> Self {
        Self { scheduler }
    }

    /// Main dream loop — processes priority queue within budget.
    /// Returns all completed dream results. Each entry is dreamed in its own
    /// dream unit, minted by `issuer` from the entry's label. A caller that
    /// holds the engine behind a lock uses [`begin_dream_session`],
    /// [`DreamSession::run`] and [`record_dream`] instead, so no lock is held
    /// across a dream's model call (§13.2).
    ///
    /// [`begin_dream_session`]: Self::begin_dream_session
    /// [`record_dream`]: Self::record_dream
    pub fn enter_dream_state(
        &mut self,
        issuer: &DispatchIssuer,
        llm: &dyn DreamLlm,
    ) -> Vec<DreamResult> {
        let session = self.begin_dream_session();
        session.run(issuer, llm, |result| self.record_dream(result))
    }

    /// §13.2 dream LK-1: the session's entries, in priority order, and the
    /// budgets, taken under the engine's lock.
    pub fn begin_dream_session(&mut self) -> DreamSession {
        self.scheduler.sort_queue();
        self.scheduler.last_dream_at = Some(now_secs());
        DreamSession {
            tasks: self.scheduler.priority_queue.clone(),
            budget_tokens: self.scheduler.dream_budget_tokens,
            budget_calls: self.scheduler.dream_budget_api_calls,
        }
    }

    /// §13.2 dream LK-3: records `result`, or drops it when its entry was
    /// cleared while it was dreamed. Returns whether it was recorded.
    pub fn record_dream(&mut self, result: DreamResult) -> bool {
        let queued = self
            .scheduler
            .priority_queue
            .iter()
            .any(|t| t.id == result.task_id);
        if queued {
            self.scheduler.record_result(result);
        }
        queued
    }

    /// §13.2 dream LK-2: one entry's dream in its own dream unit, with no
    /// engine borrowed. A failure is a `NoResult` (it still uses a call).
    pub fn dream_entry(
        issuer: &DispatchIssuer,
        llm: &dyn DreamLlm,
        task: &DreamTask,
        remaining_tokens: u64,
    ) -> DreamResult {
        let result = dream_unit(issuer, task.label()).and_then(|mut ctx| {
            let ctx = &mut ctx;
            match task.task_type {
                DreamType::Replay => Self::dream_replay(llm, ctx, task, remaining_tokens),
                DreamType::Experiment => Self::dream_experiment(llm, ctx, task, remaining_tokens),
                DreamType::Consolidate => Self::dream_consolidate(llm, ctx, task, remaining_tokens),
                DreamType::Explore => Self::dream_explore(llm, ctx, task, remaining_tokens),
                DreamType::Precompute => Self::dream_precompute(llm, ctx, task, remaining_tokens),
                DreamType::Create => Self::dream_create(llm, ctx, task, remaining_tokens),
                DreamType::Optimize => Self::dream_optimize(llm, ctx, task, remaining_tokens),
            }
        });
        result.unwrap_or_else(|e| DreamResult {
            task_id: task.id.clone(),
            dream_type: task.task_type.clone(),
            agent_id: task.source_agent.clone(),
            started_at: now_secs(),
            completed_at: now_secs(),
            tokens_used: 0,
            outcome: DreamOutcome::NoResult { reason: e },
            label: task.label().cloned(),
        })
    }

    /// Generate a morning briefing from the last dream session.
    pub fn generate_morning_briefing(
        &self,
        issuer: &DispatchIssuer,
        llm: &dyn DreamLlm,
    ) -> MorningBriefing {
        MorningBriefing::generate(&self.scheduler, issuer, llm)
    }

    // ── Individual dream implementations ────────────────────────────

    fn dream_replay(
        llm: &dyn DreamLlm,
        ctx: &mut DispatchContext,
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

        let (response, tokens) = dream_query(llm, ctx, system, &user, 600)?;
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
            label: ctx.product_label(),
        })
    }

    fn dream_experiment(
        llm: &dyn DreamLlm,
        ctx: &mut DispatchContext,
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

        let (response, tokens) = dream_query(llm, ctx, system, &user, 800)?;
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
            label: ctx.product_label(),
        })
    }

    fn dream_consolidate(
        llm: &dyn DreamLlm,
        ctx: &mut DispatchContext,
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

        let (response, tokens) = dream_query(llm, ctx, system, &user, 600)?;
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
            label: ctx.product_label(),
        })
    }

    fn dream_explore(
        llm: &dyn DreamLlm,
        ctx: &mut DispatchContext,
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

        let (response, tokens) = dream_query(llm, ctx, system, &user, 600)?;

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
            label: ctx.product_label(),
        })
    }

    fn dream_precompute(
        llm: &dyn DreamLlm,
        ctx: &mut DispatchContext,
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

        let (response, tokens) = dream_query(llm, ctx, system, &user, 800)?;
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
            label: ctx.product_label(),
        })
    }

    fn dream_create(
        llm: &dyn DreamLlm,
        ctx: &mut DispatchContext,
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

        let (response, tokens) = dream_query(llm, ctx, system, &user, 600)?;
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
            label: ctx.product_label(),
        })
    }

    fn dream_optimize(
        llm: &dyn DreamLlm,
        ctx: &mut DispatchContext,
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

        let (response, tokens) = dream_query(llm, ctx, system, &user, 600)?;
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
            label: ctx.product_label(),
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
    use crate::hm::testing::{ScriptedLlm, TestHm};
    use crate::hm::RouteClass;
    use std::sync::OnceLock;

    /// The synthetic store and issuer of these tests.
    fn dream_hm() -> &'static TestHm {
        static HM: OnceLock<TestHm> = OnceLock::new();
        HM.get_or_init(TestHm::new)
    }

    /// The former `MockDreamLlm`, through the issuer: 50 tokens a call, on an
    /// attested local transport (an unlabelled entry's dream unit is
    /// local-only).
    fn mock_dream_llm() -> ScriptedLlm {
        ScriptedLlm::answering(&dream_hm().issuer, RouteClass::Local, |prompt| {
            format!("Mock dream response for: {prompt}")
        })
    }

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
        let results = engine.enter_dream_state(&dream_hm().issuer, &mock_dream_llm());

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
        let results = engine.enter_dream_state(&dream_hm().issuer, &mock_dream_llm());

        assert_eq!(results.len(), 2); // budget limits to 2
        assert_eq!(engine.scheduler.queue_len(), 3); // 3 remaining
    }

    #[test]
    fn dream_engine_respects_token_budget() {
        let mut scheduler = DreamScheduler::new();
        scheduler.dream_budget_tokens = 90; // the mock dream model returns 50 tokens per call

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
        let results = engine.enter_dream_state(&dream_hm().issuer, &mock_dream_llm());

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
            let results = engine.enter_dream_state(&dream_hm().issuer, &mock_dream_llm());
            assert_eq!(results.len(), 1, "Failed for {dtype:?}");
            assert_eq!(results[0].dream_type, dtype);
        }
    }

    /// P3-C11-HM-M4 N1 C-2 (F-N6-2): every dream unit is local-only. An
    /// `OwnerDirect` entry whose chat turn went to a non-local transport
    /// (label `UNRESTRICTED_ORIGIN`) dreams on an attested local transport
    /// when one exists; with only a non-local transport the dream is refused
    /// and no request is made.
    #[test]
    fn hm_m4_n1_c2_every_dream_unit_is_local_only() {
        use crate::hm::{OriginLabel, Principal, Unit};
        const MARKER: &str = "P3-HM-M4-SYNTH-DREAM-C2-7e19";
        let hm = TestHm::new();
        let mut turn = hm
            .issuer
            .begin_unit(
                Principal::OwnerDirect,
                Unit::ChatTurn {
                    turn_id: uuid::Uuid::new_v4(),
                },
            )
            .unwrap();
        let chat = ScriptedLlm::replying(&hm.issuer, RouteClass::NonLocal, "a cloud reply");
        chat.ask(&mut turn, &mut |c: &mut DispatchContext| {
            let mut b = PromptBuilder::new(c);
            b.text("a synthetic chat turn");
            Some(b.build())
        })
        .expect("precondition: the Owner's turn goes to a non-local transport");
        let label = turn.entry_label().expect("a chat turn's entry label");
        assert_eq!(
            (label.principal(), label.label()),
            (Principal::OwnerDirect, OriginLabel::UnrestrictedOrigin),
            "precondition: an OwnerDirect entry labelled UNRESTRICTED_ORIGIN"
        );
        let queued = || {
            let mut scheduler = DreamScheduler::new();
            scheduler.enqueue(DreamTask {
                id: "c2".into(),
                task_type: DreamType::Replay,
                priority: 0.6,
                source_agent: "owner".into(),
                context: serde_json::json!({"task": MARKER, "original_response": "r"}),
                label: Some(label.clone()),
                ..DreamTask::default()
            });
            scheduler
        };

        // An attested local transport exists: the dream runs there.
        let local = ScriptedLlm::replying(
            &hm.issuer,
            RouteClass::Local,
            "ORIGINAL_SCORE: 4\nIMPROVED_SCORE: 7",
        );
        let mut engine = DreamEngine::new(queued());
        let results = engine.enter_dream_state(&hm.issuer, &local);
        assert_eq!(results.len(), 1);
        assert!(
            matches!(results[0].outcome, DreamOutcome::Improvement { .. }),
            "C-2: the dream did not run on the local transport: {:?}",
            results[0].outcome
        );
        assert!(local.sent().iter().any(|p| p.contains(MARKER)));
        assert_eq!(
            results[0].label().map(|l| l.label()),
            Some(OriginLabel::LocalOnlyOrigin)
        );

        // Only a non-local transport: refused, and nothing is sent.
        let remote = ScriptedLlm::replying(
            &hm.issuer,
            RouteClass::NonLocal,
            "ORIGINAL_SCORE: 4\nIMPROVED_SCORE: 7",
        );
        let mut engine = DreamEngine::new(queued());
        let results = engine.enter_dream_state(&hm.issuer, &remote);
        assert!(
            remote.sent().is_empty(),
            "C-2: PROHIBITED NON-LOCAL DREAM DISPATCH: {} request(s): {:?}",
            remote.sent().len(),
            remote.sent()
        );
        assert_eq!(results.len(), 1);
        assert!(
            matches!(results[0].outcome, DreamOutcome::NoResult { .. }),
            "C-2: the dream was not refused: {:?}",
            results[0].outcome
        );
    }

    /// P3-C11-M5 N1 K-3 (§13.2 dream LK-3; an M4 carry-over): a dream
    /// result whose entry was cleared from the queue while it was dreamed is
    /// dropped, not recorded: `record_dream` returns false and the scheduler
    /// holds no result for it. A result whose entry is still queued is
    /// recorded (the control).
    #[test]
    fn hm_m5_n1_k3_a_result_whose_entry_was_cleared_while_dreamed_is_dropped() {
        let mut scheduler = DreamScheduler::new();
        for id in ["k3-cleared", "k3-kept"] {
            scheduler.enqueue(DreamTask {
                id: id.into(),
                task_type: DreamType::Explore,
                priority: 0.5,
                source_agent: "a".into(),
                context: serde_json::json!({"topic": "k3"}),
                ..DreamTask::default()
            });
        }
        let mut engine = DreamEngine::new(scheduler);
        let session = engine.begin_dream_session();
        let llm = mock_dream_llm();
        let dreamed: Vec<DreamResult> = session
            .tasks
            .iter()
            .map(|task| DreamEngine::dream_entry(&dream_hm().issuer, &llm, task, 1_000))
            .collect();
        // The entry is cleared while it is dreamed (no engine borrowed).
        engine
            .scheduler
            .priority_queue
            .retain(|t| t.id != "k3-cleared");
        let result_of = |id: &str| dreamed.iter().find(|r| r.task_id == id).unwrap().clone();
        assert!(
            !engine.record_dream(result_of("k3-cleared")),
            "K-3: a cleared entry's result was reported recorded"
        );
        assert!(
            engine
                .scheduler
                .completed_dreams
                .iter()
                .all(|r| r.task_id != "k3-cleared"),
            "K-3: the scheduler holds a result for a cleared entry"
        );
        assert!(
            engine.record_dream(result_of("k3-kept")),
            "the control: a queued entry's result is recorded"
        );
        assert_eq!(
            engine
                .scheduler
                .completed_dreams
                .iter()
                .map(|r| r.task_id.as_str())
                .collect::<Vec<_>>(),
            vec!["k3-kept"]
        );
        assert_eq!(engine.scheduler.queue_len(), 0);
    }

    #[test]
    fn parse_best_score_multi() {
        let text = "STRATEGY_1: foo SCORE: 3\nSTRATEGY_2: bar SCORE: 8\nSTRATEGY_3: baz SCORE: 5";
        assert!((parse_best_score(text) - 8.0).abs() < f64::EPSILON);
    }
}
