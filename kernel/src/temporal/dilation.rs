//! Time Dilation — compressed iterative work sessions.
//!
//! Agents run rapid create→critique loops on a task.  Each iteration improves
//! the artifact until a target quality score is met or the iteration cap is
//! reached.

use crate::temporal::types::{Artifact, TemporalError, TimeDilatedSession};
use std::time::Instant;

/// Runs time-dilated work sessions where agents iterate rapidly on a task.
#[derive(Debug, Clone)]
pub struct TimeDilator {
    /// Default max iterations if caller doesn't specify.
    pub default_max_iterations: u32,
    /// Default target score (0-10) for early exit.
    pub default_target_score: f64,
}

impl Default for TimeDilator {
    fn default() -> Self {
        Self {
            default_max_iterations: 10,
            default_target_score: 8.0,
        }
    }
}

impl TimeDilator {
    pub fn new(max_iterations: u32, target_score: f64) -> Self {
        Self {
            default_max_iterations: max_iterations,
            default_target_score: target_score,
        }
    }

    /// Run a dilated session: agent creates → critic scores → feedback loop.
    ///
    /// `create_fn` takes (task, previous_artifact, feedback) → new artifact content.
    /// `critique_fn` takes (task, artifact_content) → (score, feedback).
    ///
    /// Both are closures backed by LLM calls.
    pub fn run_dilated_session<C, R>(
        &self,
        task: &str,
        agent_ids: Vec<String>,
        max_iterations: Option<u32>,
        target_score: Option<f64>,
        mut create_fn: C,
        mut critique_fn: R,
    ) -> Result<TimeDilatedSession, TemporalError>
    where
        C: FnMut(&str, &str, &str) -> Result<String, TemporalError>,
        R: FnMut(&str, &str) -> Result<(f64, String), TemporalError>,
    {
        let start = Instant::now();
        let max_iter = max_iterations.unwrap_or(self.default_max_iterations);
        let target = target_score.unwrap_or(self.default_target_score);

        let mut session = TimeDilatedSession::new(task, agent_ids);
        let mut best_content = String::new();
        let mut best_score: f64 = 0.0;
        let mut feedback = String::new();

        for iteration in 0..max_iter {
            // Creator generates/improves artifact
            let content = create_fn(task, &best_content, &feedback)?;

            // Critic scores it
            let (score, new_feedback) = critique_fn(task, &content)?;
            let clamped_score = score.clamp(0.0, 10.0);

            session.quality_progression.push(clamped_score);

            if clamped_score > best_score {
                best_score = clamped_score;
                best_content = content.clone();
            }

            session.artifacts.push(Artifact {
                name: format!(
                    "{}-v{}",
                    task.chars().take(30).collect::<String>(),
                    iteration + 1
                ),
                artifact_type: "iterative".into(),
                content,
                iteration: iteration + 1,
                score: clamped_score,
            });

            feedback = new_feedback;
            session.simulated_iterations = iteration + 1;

            // Early exit if target reached
            if clamped_score >= target {
                break;
            }
        }

        session.final_score = best_score;
        session.real_time_budget_seconds = start.elapsed().as_secs();

        Ok(session)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dilated_session_improves_over_iterations() {
        let dilator = TimeDilator::default();
        let iteration = std::cell::Cell::new(0u32);

        let create = |_task: &str, _prev: &str, _fb: &str| -> Result<String, TemporalError> {
            Ok(format!("artifact content v{}", iteration.get()))
        };

        let critique = |_task: &str, _content: &str| -> Result<(f64, String), TemporalError> {
            iteration.set(iteration.get() + 1);
            let i = iteration.get();
            // Simulate improving scores
            let score = 4.0 + i as f64;
            Ok((score, format!("improve area {i}")))
        };

        let session = dilator
            .run_dilated_session(
                "write a scraper",
                vec!["creator".into(), "critic".into()],
                Some(5),
                Some(8.0),
                create,
                critique,
            )
            .unwrap();

        assert!(!session.quality_progression.is_empty());
        assert!(session.final_score >= 7.0);
        assert!(session.simulated_iterations <= 5);
        assert!(!session.artifacts.is_empty());
    }

    #[test]
    fn dilated_session_early_exit() {
        let dilator = TimeDilator::default();

        let create = |_task: &str, _prev: &str, _fb: &str| -> Result<String, TemporalError> {
            Ok("perfect".into())
        };

        let critique = |_task: &str, _content: &str| -> Result<(f64, String), TemporalError> {
            Ok((10.0, "flawless".into())) // immediately hits target
        };

        let session = dilator
            .run_dilated_session(
                "task",
                vec!["a".into()],
                Some(10),
                Some(8.0),
                create,
                critique,
            )
            .unwrap();

        assert_eq!(session.simulated_iterations, 1);
        assert!((session.final_score - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn dilated_session_create_error() {
        let dilator = TimeDilator::default();

        let create = |_task: &str, _prev: &str, _fb: &str| -> Result<String, TemporalError> {
            Err(TemporalError::LlmError("creator failed".into()))
        };

        let critique = |_task: &str, _content: &str| -> Result<(f64, String), TemporalError> {
            Ok((5.0, "ok".into()))
        };

        let result =
            dilator.run_dilated_session("task", vec!["a".into()], Some(3), None, create, critique);
        assert!(result.is_err());
    }

    #[test]
    fn dilated_session_score_clamped() {
        let dilator = TimeDilator::default();

        let create = |_task: &str, _prev: &str, _fb: &str| -> Result<String, TemporalError> {
            Ok("content".into())
        };

        let critique = |_task: &str, _content: &str| -> Result<(f64, String), TemporalError> {
            Ok((15.0, "way too high".into())) // exceeds 10
        };

        let session = dilator
            .run_dilated_session("t", vec!["a".into()], Some(1), Some(20.0), create, critique)
            .unwrap();

        assert!((session.final_score - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn dilated_default_config() {
        let d = TimeDilator::default();
        assert_eq!(d.default_max_iterations, 10);
        assert!((d.default_target_score - 8.0).abs() < f64::EPSILON);
    }
}
