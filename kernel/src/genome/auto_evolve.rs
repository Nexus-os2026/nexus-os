//! Auto-Evolution Engine — background self-improvement for every agent.
//!
//! After every chat response, agents are automatically scored. When scores
//! drop below a configurable threshold, the system mutates the agent's system
//! prompt via LLM and tests the mutation before committing.
//!
//! All scoring and evolution happens in the background — the user just sees
//! better responses over time.

use super::dna::AgentGenome;
use super::operations::mutate_with_prompt;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

// ── Traits ──────────────────────────────────────────────────────────────────

/// LLM backend for scoring and prompt mutation.
pub trait AutoEvolveLlm: Send + Sync {
    /// Score a response on a 1–10 scale.
    fn score_response(&self, user_message: &str, agent_response: &str) -> Result<f64, String>;

    /// Generate an improved system prompt given the current prompt and weak responses.
    fn mutate_prompt(
        &self,
        current_prompt: &str,
        weak_responses: &[(String, String, f64)], // (user_msg, agent_response, score)
    ) -> Result<String, String>;

    /// Generate a response using a given system prompt and user message (for testing mutations).
    fn generate_with_prompt(
        &self,
        system_prompt: &str,
        user_message: &str,
    ) -> Result<String, String>;
}

// ── Performance Tracker ─────────────────────────────────────────────────────

/// Tracks per-agent performance scores and controls evolution timing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentPerformanceTracker {
    pub agent_id: String,
    pub recent_scores: Vec<f64>,
    pub running_average: f64,
    pub improvement_threshold: f64,
    pub evolution_cooldown_secs: u64,
    pub last_evolution_attempt: u64,
    pub total_tasks: u64,
    pub total_evolutions: u32,
    pub successful_evolutions: u32,
    pub evolution_enabled: bool,
    /// Recent task data for evolution analysis: (user_msg, agent_response, score).
    pub recent_tasks: Vec<(String, String, f64)>,
}

impl AgentPerformanceTracker {
    pub fn new(agent_id: impl Into<String>) -> Self {
        Self {
            agent_id: agent_id.into(),
            recent_scores: Vec::new(),
            running_average: 0.0,
            improvement_threshold: 6.0,
            evolution_cooldown_secs: 300,
            last_evolution_attempt: 0,
            total_tasks: 0,
            total_evolutions: 0,
            successful_evolutions: 0,
            evolution_enabled: true,
            recent_tasks: Vec::new(),
        }
    }

    /// Record a score and the associated task data.
    pub fn record_score(&mut self, score: f64, user_msg: &str, agent_response: &str) {
        self.recent_scores.push(score);
        if self.recent_scores.len() > 20 {
            self.recent_scores.remove(0);
        }
        self.running_average =
            self.recent_scores.iter().sum::<f64>() / self.recent_scores.len() as f64;
        self.total_tasks += 1;

        // Keep last 10 task entries for evolution analysis
        self.recent_tasks
            .push((user_msg.to_string(), agent_response.to_string(), score));
        if self.recent_tasks.len() > 10 {
            self.recent_tasks.remove(0);
        }
    }

    /// Whether this agent should attempt evolution now.
    pub fn should_evolve(&self) -> bool {
        if !self.evolution_enabled {
            return false;
        }
        let now = epoch_secs();
        self.running_average < self.improvement_threshold
            && self.recent_scores.len() >= 3
            && (now.saturating_sub(self.last_evolution_attempt)) > self.evolution_cooldown_secs
    }

    /// Return the weakest recent tasks (below threshold).
    pub fn weak_tasks(&self) -> Vec<(String, String, f64)> {
        self.recent_tasks
            .iter()
            .filter(|(_, _, score)| *score < self.improvement_threshold)
            .cloned()
            .collect()
    }

    /// Mark an evolution attempt (success or failure).
    pub fn record_evolution_attempt(&mut self, success: bool) {
        self.last_evolution_attempt = epoch_secs();
        self.total_evolutions += 1;
        if success {
            self.successful_evolutions += 1;
        }
    }
}

// ── Evolution Event Log ─────────────────────────────────────────────────────

/// A single evolution attempt record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvolutionEvent {
    pub agent_id: String,
    pub timestamp: u64,
    pub old_score: f64,
    pub new_score: f64,
    pub success: bool,
    pub prompt_diff_summary: String,
}

/// Result of a forced or automatic evolution attempt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvolutionResult {
    pub agent_id: String,
    pub improved: bool,
    pub old_score: f64,
    pub new_score: f64,
    pub message: String,
}

// ── Evolution Configuration ─────────────────────────────────────────────────

/// Per-agent evolution configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvolutionConfig {
    pub enabled: bool,
    pub threshold: f64,
    pub cooldown_seconds: u64,
}

// ── Auto-Evolution Manager ──────────────────────────────────────────────────

/// Central manager for all agent performance tracking and auto-evolution.
pub struct AutoEvolutionManager {
    trackers: Mutex<HashMap<String, AgentPerformanceTracker>>,
    evolution_log: Mutex<Vec<EvolutionEvent>>,
}

impl AutoEvolutionManager {
    pub fn new() -> Self {
        Self {
            trackers: Mutex::new(HashMap::new()),
            evolution_log: Mutex::new(Vec::new()),
        }
    }

    /// Get or create a tracker for the given agent.
    fn get_or_create_tracker<'a>(
        trackers: &'a mut HashMap<String, AgentPerformanceTracker>,
        agent_id: &'a str,
    ) -> &'a mut AgentPerformanceTracker {
        trackers
            .entry(agent_id.to_string())
            .or_insert_with(|| AgentPerformanceTracker::new(agent_id))
    }

    /// Score a response using the cheap LLM and record it.
    pub fn score_and_record(
        &self,
        agent_id: &str,
        user_message: &str,
        agent_response: &str,
        llm: &dyn AutoEvolveLlm,
    ) -> f64 {
        let score = llm
            .score_response(user_message, agent_response)
            .unwrap_or(7.0);
        let clamped = score.clamp(1.0, 10.0);

        let mut trackers = self.trackers.lock().unwrap_or_else(|p| p.into_inner());
        let tracker = Self::get_or_create_tracker(&mut trackers, agent_id);
        tracker.record_score(clamped, user_message, agent_response);

        clamped
    }

    /// Check if an agent should evolve.
    pub fn should_evolve(&self, agent_id: &str) -> bool {
        let trackers = self.trackers.lock().unwrap_or_else(|p| p.into_inner());
        trackers
            .get(agent_id)
            .map(|t| t.should_evolve())
            .unwrap_or(false)
    }

    /// Attempt auto-evolution for an agent. Returns the evolution result and
    /// optionally the mutated genome.
    pub fn attempt_evolution(
        &self,
        agent_id: &str,
        current_genome: &AgentGenome,
        llm: &dyn AutoEvolveLlm,
    ) -> EvolutionResult {
        let weak_tasks = {
            let trackers = self.trackers.lock().unwrap_or_else(|p| p.into_inner());
            match trackers.get(agent_id) {
                Some(t) => t.weak_tasks(),
                None => {
                    return EvolutionResult {
                        agent_id: agent_id.to_string(),
                        improved: false,
                        old_score: 0.0,
                        new_score: 0.0,
                        message: "No tracker data".to_string(),
                    }
                }
            }
        };

        if weak_tasks.is_empty() {
            let mut trackers = self.trackers.lock().unwrap_or_else(|p| p.into_inner());
            if let Some(t) = trackers.get_mut(agent_id) {
                t.record_evolution_attempt(false);
            }
            return EvolutionResult {
                agent_id: agent_id.to_string(),
                improved: false,
                old_score: 0.0,
                new_score: 0.0,
                message: "No weak tasks to improve on".to_string(),
            };
        }

        let current_prompt = &current_genome.genes.personality.system_prompt;
        let old_avg = weak_tasks.iter().map(|(_, _, s)| s).sum::<f64>() / weak_tasks.len() as f64;

        // Step 1: Generate mutated prompt
        let mutated_prompt = match llm.mutate_prompt(current_prompt, &weak_tasks) {
            Ok(p) => p,
            Err(e) => {
                let mut trackers = self.trackers.lock().unwrap_or_else(|p| p.into_inner());
                if let Some(t) = trackers.get_mut(agent_id) {
                    t.record_evolution_attempt(false);
                }
                return EvolutionResult {
                    agent_id: agent_id.to_string(),
                    improved: false,
                    old_score: old_avg,
                    new_score: old_avg,
                    message: format!("Prompt mutation failed: {e}"),
                };
            }
        };

        // Step 2: Test the mutation by re-running weak tasks with new prompt
        let mut new_scores = Vec::new();
        for (user_msg, _, _) in &weak_tasks {
            match llm.generate_with_prompt(&mutated_prompt, user_msg) {
                Ok(new_response) => {
                    let new_score = llm
                        .score_response(user_msg, &new_response)
                        .unwrap_or(5.0)
                        .clamp(1.0, 10.0);
                    new_scores.push(new_score);
                }
                Err(_) => {
                    new_scores.push(old_avg); // neutral on error
                }
            }
        }

        let new_avg = if new_scores.is_empty() {
            old_avg
        } else {
            new_scores.iter().sum::<f64>() / new_scores.len() as f64
        };

        let improved = new_avg > old_avg;

        // Record the attempt
        {
            let mut trackers = self.trackers.lock().unwrap_or_else(|p| p.into_inner());
            if let Some(t) = trackers.get_mut(agent_id) {
                t.record_evolution_attempt(improved);
            }
        }

        // Log the event
        {
            let mut log = self.evolution_log.lock().unwrap_or_else(|p| p.into_inner());
            log.push(EvolutionEvent {
                agent_id: agent_id.to_string(),
                timestamp: epoch_secs(),
                old_score: old_avg,
                new_score: new_avg,
                success: improved,
                prompt_diff_summary: if improved {
                    format!("Prompt evolved: {:.1} → {:.1}", old_avg, new_avg)
                } else {
                    "Mutation reverted — no improvement".to_string()
                },
            });
            // Keep last 200 events
            if log.len() > 200 {
                let drain_count = log.len() - 200;
                log.drain(..drain_count);
            }
        }

        EvolutionResult {
            agent_id: agent_id.to_string(),
            improved,
            old_score: old_avg,
            new_score: new_avg,
            message: if improved {
                format!(
                    "Agent {} improved: {:.1} → {:.1}",
                    agent_id, old_avg, new_avg
                )
            } else {
                format!(
                    "Agent {} evolution reverted (no improvement: {:.1} → {:.1})",
                    agent_id, old_avg, new_avg
                )
            },
        }
    }

    /// Apply a successful evolution to a genome, returning the mutated genome.
    pub fn apply_evolution(
        &self,
        genome: &AgentGenome,
        llm: &dyn AutoEvolveLlm,
    ) -> Option<AgentGenome> {
        let agent_id = &genome.agent_id;
        let weak_tasks = {
            let trackers = self.trackers.lock().unwrap_or_else(|p| p.into_inner());
            match trackers.get(agent_id) {
                Some(t) => t.weak_tasks(),
                None => return None,
            }
        };

        if weak_tasks.is_empty() {
            return None;
        }

        let current_prompt = &genome.genes.personality.system_prompt;
        let mutated_prompt = llm.mutate_prompt(current_prompt, &weak_tasks).ok()?;
        let mut evolved = mutate_with_prompt(genome, mutated_prompt);

        // Record fitness from the running average
        let avg = {
            let trackers = self.trackers.lock().unwrap_or_else(|p| p.into_inner());
            trackers
                .get(agent_id)
                .map(|t| t.running_average)
                .unwrap_or(5.0)
        };
        evolved.record_fitness(avg);

        Some(evolved)
    }

    /// Get a clone of an agent's performance tracker.
    pub fn get_tracker(&self, agent_id: &str) -> Option<AgentPerformanceTracker> {
        let trackers = self.trackers.lock().unwrap_or_else(|p| p.into_inner());
        trackers.get(agent_id).cloned()
    }

    /// Get evolution log for an agent.
    pub fn get_evolution_log(&self, agent_id: &str, limit: u32) -> Vec<EvolutionEvent> {
        let log = self.evolution_log.lock().unwrap_or_else(|p| p.into_inner());
        log.iter()
            .rev()
            .filter(|e| e.agent_id == agent_id)
            .take(limit as usize)
            .cloned()
            .collect()
    }

    /// Update evolution configuration for a specific agent.
    pub fn set_evolution_config(&self, agent_id: &str, config: EvolutionConfig) {
        let mut trackers = self.trackers.lock().unwrap_or_else(|p| p.into_inner());
        let tracker = Self::get_or_create_tracker(&mut trackers, agent_id);
        tracker.evolution_enabled = config.enabled;
        tracker.improvement_threshold = config.threshold;
        tracker.evolution_cooldown_secs = config.cooldown_seconds;
    }

    /// Force an evolution attempt regardless of cooldown/threshold.
    pub fn force_evolve(
        &self,
        agent_id: &str,
        genome: &AgentGenome,
        llm: &dyn AutoEvolveLlm,
    ) -> EvolutionResult {
        // Temporarily override cooldown
        {
            let mut trackers = self.trackers.lock().unwrap_or_else(|p| p.into_inner());
            if let Some(t) = trackers.get_mut(agent_id) {
                t.last_evolution_attempt = 0;
            }
        }
        self.attempt_evolution(agent_id, genome, llm)
    }
}

impl Default for AutoEvolutionManager {
    fn default() -> Self {
        Self::new()
    }
}

// ── Helpers ─────────────────────────────────────────────────────────────────

fn epoch_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    // Mock LLM that returns configurable scores and prompts
    struct MockAutoEvolveLlm {
        score: f64,
        mutated_prompt: String,
        generated_response: String,
    }

    impl MockAutoEvolveLlm {
        fn new(score: f64, mutated_prompt: &str, generated_response: &str) -> Self {
            Self {
                score,
                mutated_prompt: mutated_prompt.to_string(),
                generated_response: generated_response.to_string(),
            }
        }
    }

    impl AutoEvolveLlm for MockAutoEvolveLlm {
        fn score_response(
            &self,
            _user_message: &str,
            _agent_response: &str,
        ) -> Result<f64, String> {
            Ok(self.score)
        }

        fn mutate_prompt(
            &self,
            _current_prompt: &str,
            _weak_responses: &[(String, String, f64)],
        ) -> Result<String, String> {
            Ok(self.mutated_prompt.clone())
        }

        fn generate_with_prompt(
            &self,
            _system_prompt: &str,
            _user_message: &str,
        ) -> Result<String, String> {
            Ok(self.generated_response.clone())
        }
    }

    fn sample_genome(id: &str) -> AgentGenome {
        use super::super::dna::*;
        AgentGenome::new(
            id,
            GeneSet {
                personality: PersonalityGenes {
                    system_prompt: "You are a test agent.".to_string(),
                    tone: "professional".to_string(),
                    verbosity: 0.5,
                    creativity: 0.5,
                    assertiveness: 0.5,
                },
                capabilities: CapabilityGenes {
                    domains: vec!["testing".to_string()],
                    domain_weights: HashMap::from([("testing".to_string(), 1.0)]),
                    tools: vec!["fs.read".to_string()],
                    max_context_tokens: 128_000,
                },
                reasoning: ReasoningGenes {
                    strategy: "chain_of_thought".to_string(),
                    depth: 3,
                    temperature: 0.7,
                    self_reflection: true,
                    planning_horizon: 5,
                },
                autonomy: AutonomyGenes {
                    level: 3,
                    risk_tolerance: 0.4,
                    escalation_threshold: 0.7,
                    requires_approval: vec!["file_delete".to_string()],
                },
                evolution: EvolutionGenes {
                    mutation_rate: 0.1,
                    fitness_history: Vec::new(),
                    generation: 0,
                    lineage: Vec::new(),
                },
            },
        )
    }

    #[test]
    fn tracker_records_scores_and_computes_average() {
        let mut tracker = AgentPerformanceTracker::new("test-agent");
        tracker.record_score(8.0, "hello", "hi there");
        tracker.record_score(6.0, "help", "sure thing");
        tracker.record_score(4.0, "code", "here's code");

        assert_eq!(tracker.recent_scores.len(), 3);
        assert_eq!(tracker.total_tasks, 3);
        assert!((tracker.running_average - 6.0).abs() < 1e-9);
    }

    #[test]
    fn tracker_caps_at_20_scores() {
        let mut tracker = AgentPerformanceTracker::new("test-agent");
        for i in 0..25 {
            tracker.record_score(i as f64 % 10.0, "msg", "resp");
        }
        assert_eq!(tracker.recent_scores.len(), 20);
        assert_eq!(tracker.total_tasks, 25);
    }

    #[test]
    fn should_evolve_requires_minimum_scores() {
        let mut tracker = AgentPerformanceTracker::new("test-agent");
        tracker.record_score(3.0, "msg", "resp");
        tracker.record_score(3.0, "msg", "resp");
        assert!(!tracker.should_evolve(), "need at least 3 scores");

        tracker.record_score(3.0, "msg", "resp");
        assert!(tracker.should_evolve());
    }

    #[test]
    fn should_evolve_respects_cooldown() {
        let mut tracker = AgentPerformanceTracker::new("test-agent");
        for _ in 0..5 {
            tracker.record_score(3.0, "msg", "resp");
        }
        assert!(tracker.should_evolve());

        // Simulate recent evolution attempt
        tracker.last_evolution_attempt = epoch_secs();
        assert!(!tracker.should_evolve(), "cooldown not elapsed");
    }

    #[test]
    fn should_evolve_respects_threshold() {
        let mut tracker = AgentPerformanceTracker::new("test-agent");
        for _ in 0..5 {
            tracker.record_score(9.0, "msg", "resp");
        }
        assert!(!tracker.should_evolve(), "above threshold");
    }

    #[test]
    fn should_evolve_respects_enabled_flag() {
        let mut tracker = AgentPerformanceTracker::new("test-agent");
        for _ in 0..5 {
            tracker.record_score(3.0, "msg", "resp");
        }
        tracker.evolution_enabled = false;
        assert!(!tracker.should_evolve());
    }

    #[test]
    fn manager_score_and_record() {
        let manager = AutoEvolutionManager::new();
        let llm = MockAutoEvolveLlm::new(8.5, "", "");
        let score = manager.score_and_record("agent-1", "hello", "hi there", &llm);
        assert!((score - 8.5).abs() < 1e-9);

        let tracker = manager.get_tracker("agent-1").unwrap();
        assert_eq!(tracker.total_tasks, 1);
        assert_eq!(tracker.recent_scores.len(), 1);
    }

    #[test]
    fn manager_score_clamps_to_range() {
        let manager = AutoEvolutionManager::new();
        let llm = MockAutoEvolveLlm::new(15.0, "", "");
        let score = manager.score_and_record("agent-1", "msg", "resp", &llm);
        assert!((score - 10.0).abs() < 1e-9);

        let llm_low = MockAutoEvolveLlm::new(-5.0, "", "");
        let score2 = manager.score_and_record("agent-1", "msg", "resp", &llm_low);
        assert!((score2 - 1.0).abs() < 1e-9);
    }

    #[test]
    fn evolution_attempt_improves_score() {
        let manager = AutoEvolutionManager::new();
        // Record low scores to create weak tasks
        let low_llm = MockAutoEvolveLlm::new(3.0, "", "");
        for _ in 0..5 {
            manager.score_and_record("agent-1", "help me code", "bad response", &low_llm);
        }

        // Now attempt evolution with a high-scoring LLM
        let evolve_llm = MockAutoEvolveLlm::new(9.0, "Improved prompt", "Great response");
        let genome = sample_genome("agent-1");
        let result = manager.attempt_evolution("agent-1", &genome, &evolve_llm);

        assert!(result.improved);
        assert!(result.new_score > result.old_score);
    }

    #[test]
    fn evolution_attempt_reverts_on_no_improvement() {
        let manager = AutoEvolutionManager::new();
        let low_llm = MockAutoEvolveLlm::new(3.0, "", "");
        for _ in 0..5 {
            manager.score_and_record("agent-1", "help", "bad", &low_llm);
        }

        // Evolution also scores low — no improvement
        let still_low_llm = MockAutoEvolveLlm::new(2.0, "Worse prompt", "Still bad");
        let genome = sample_genome("agent-1");
        let result = manager.attempt_evolution("agent-1", &genome, &still_low_llm);

        assert!(!result.improved);
    }

    #[test]
    fn evolution_log_records_attempts() {
        let manager = AutoEvolutionManager::new();
        let low_llm = MockAutoEvolveLlm::new(3.0, "", "");
        for _ in 0..5 {
            manager.score_and_record("agent-1", "msg", "resp", &low_llm);
        }

        let evolve_llm = MockAutoEvolveLlm::new(9.0, "Better", "Better response");
        let genome = sample_genome("agent-1");
        manager.attempt_evolution("agent-1", &genome, &evolve_llm);

        let log = manager.get_evolution_log("agent-1", 10);
        assert_eq!(log.len(), 1);
        assert!(log[0].success);
    }

    #[test]
    fn set_evolution_config_updates_tracker() {
        let manager = AutoEvolutionManager::new();
        manager.set_evolution_config(
            "agent-1",
            EvolutionConfig {
                enabled: false,
                threshold: 8.0,
                cooldown_seconds: 600,
            },
        );

        let tracker = manager.get_tracker("agent-1").unwrap();
        assert!(!tracker.evolution_enabled);
        assert!((tracker.improvement_threshold - 8.0).abs() < 1e-9);
        assert_eq!(tracker.evolution_cooldown_secs, 600);
    }

    #[test]
    fn force_evolve_bypasses_cooldown() {
        let manager = AutoEvolutionManager::new();
        let low_llm = MockAutoEvolveLlm::new(3.0, "", "");
        for _ in 0..5 {
            manager.score_and_record("agent-1", "msg", "resp", &low_llm);
        }

        // Set recent cooldown
        {
            let mut trackers = manager.trackers.lock().unwrap();
            if let Some(t) = trackers.get_mut("agent-1") {
                t.last_evolution_attempt = epoch_secs();
            }
        }

        // Normal evolution should be blocked
        assert!(!manager.should_evolve("agent-1"));

        // Force should work
        let evolve_llm = MockAutoEvolveLlm::new(9.0, "Better", "Better response");
        let genome = sample_genome("agent-1");
        let result = manager.force_evolve("agent-1", &genome, &evolve_llm);
        assert!(result.improved);
    }

    #[test]
    fn weak_tasks_filters_below_threshold() {
        let mut tracker = AgentPerformanceTracker::new("test-agent");
        tracker.record_score(9.0, "good question", "great answer");
        tracker.record_score(3.0, "bad question", "poor answer");
        tracker.record_score(5.0, "ok question", "ok answer");

        let weak = tracker.weak_tasks();
        assert_eq!(weak.len(), 2); // scores 3.0 and 5.0 are below 6.0 threshold
        assert!((weak[0].2 - 3.0).abs() < 1e-9);
        assert!((weak[1].2 - 5.0).abs() < 1e-9);
    }

    #[test]
    fn apply_evolution_returns_mutated_genome() {
        let manager = AutoEvolutionManager::new();
        let low_llm = MockAutoEvolveLlm::new(3.0, "", "");
        for _ in 0..5 {
            manager.score_and_record("test-agent", "msg", "resp", &low_llm);
        }

        let evolve_llm = MockAutoEvolveLlm::new(9.0, "Evolved prompt", "Better response");
        let genome = sample_genome("test-agent");
        let evolved = manager.apply_evolution(&genome, &evolve_llm);
        assert!(evolved.is_some());
        let evolved = evolved.unwrap();
        assert_eq!(evolved.genes.personality.system_prompt, "Evolved prompt");
        assert!(evolved.generation > genome.generation);
    }

    #[test]
    fn manager_handles_multiple_agents() {
        let manager = AutoEvolutionManager::new();
        let llm = MockAutoEvolveLlm::new(7.0, "", "");

        manager.score_and_record("agent-a", "msg1", "resp1", &llm);
        manager.score_and_record("agent-b", "msg2", "resp2", &llm);

        let tracker_a = manager.get_tracker("agent-a").unwrap();
        let tracker_b = manager.get_tracker("agent-b").unwrap();
        assert_eq!(tracker_a.total_tasks, 1);
        assert_eq!(tracker_b.total_tasks, 1);
        assert_eq!(tracker_a.agent_id, "agent-a");
        assert_eq!(tracker_b.agent_id, "agent-b");
    }

    #[test]
    fn evolution_log_caps_at_200() {
        let manager = AutoEvolutionManager::new();
        {
            let mut log = manager.evolution_log.lock().unwrap();
            for i in 0..210 {
                log.push(EvolutionEvent {
                    agent_id: "agent-1".to_string(),
                    timestamp: i as u64,
                    old_score: 3.0,
                    new_score: 5.0,
                    success: true,
                    prompt_diff_summary: format!("event {i}"),
                });
            }
            // Trigger cap
            if log.len() > 200 {
                let drain_count = log.len() - 200;
                log.drain(..drain_count);
            }
        }
        let log = manager.get_evolution_log("agent-1", 300);
        assert!(log.len() <= 200);
    }
}
