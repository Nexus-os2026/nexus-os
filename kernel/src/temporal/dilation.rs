//! Time Dilation — compressed iterative work sessions.
//!
//! Agents run rapid create→critique loops on a task.  Each iteration improves
//! the artifact until a target quality score is met or the iteration cap is
//! reached.

use crate::temporal::types::{Artifact, TemporalError, TimeDilatedSession};
use std::time::Instant;

/// Most create→critique iterations one dilated session may run.
///
/// Every iteration makes two model calls and keeps its artifact in memory, so
/// a count outside `1..=MAX_DILATED_ITERATIONS` is refused, never clamped,
/// before any model is called.
pub const MAX_DILATED_ITERATIONS: u32 = 50;

/// Accept an iteration count within `1..=MAX_DILATED_ITERATIONS`.
pub fn check_iteration_count(iterations: u32) -> Result<u32, TemporalError> {
    if (1..=MAX_DILATED_ITERATIONS).contains(&iterations) {
        Ok(iterations)
    } else {
        Err(TemporalError::InvalidIterationCount(iterations))
    }
}

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
        // Refused before either closure runs: no model call for a bad count.
        let max_iter =
            check_iteration_count(max_iterations.unwrap_or(self.default_max_iterations))?;
        let start = Instant::now();
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

    /// Runs a session whose critic never reaches the target, counting the
    /// creator and critic calls.
    fn run_counted(
        dilator: &TimeDilator,
        max_iterations: Option<u32>,
    ) -> (Result<TimeDilatedSession, TemporalError>, u32, u32) {
        let created = std::cell::Cell::new(0u32);
        let critiqued = std::cell::Cell::new(0u32);
        let result = dilator.run_dilated_session(
            "task",
            vec!["a".into()],
            max_iterations,
            Some(10.0),
            |_task: &str, _prev: &str, _fb: &str| {
                created.set(created.get() + 1);
                Ok("content".to_string())
            },
            |_task: &str, _content: &str| {
                critiqued.set(critiqued.get() + 1);
                Ok((1.0, "again".to_string()))
            },
        );
        (result, created.get(), critiqued.get())
    }

    #[test]
    fn p0_fg_k_iteration_count_is_refused_before_any_model_call() {
        assert_eq!(MAX_DILATED_ITERATIONS, 50);
        let dilator = TimeDilator::default();
        for count in [0, MAX_DILATED_ITERATIONS + 1, u32::MAX] {
            let (result, created, critiqued) = run_counted(&dilator, Some(count));
            match result {
                Err(TemporalError::InvalidIterationCount(refused)) => assert_eq!(refused, count),
                other => panic!("count {count} must be refused, got {other:?}"),
            }
            assert_eq!(
                (created, critiqued),
                (0, 0),
                "count {count} reached the model"
            );
        }
        // A configured default outside the bound is refused the same way.
        for default in [0, MAX_DILATED_ITERATIONS + 1] {
            let (result, created, critiqued) = run_counted(&TimeDilator::new(default, 8.0), None);
            assert!(matches!(
                result,
                Err(TemporalError::InvalidIterationCount(refused)) if refused == default
            ));
            assert_eq!((created, critiqued), (0, 0));
        }
        assert_eq!(
            TemporalError::InvalidIterationCount(51).to_string(),
            "invalid iteration count: 51 (allowed: 1 to 50)"
        );
    }

    #[test]
    fn p0_fg_k_iteration_count_bounds_are_inclusive() {
        let dilator = TimeDilator::default();
        for count in [1, MAX_DILATED_ITERATIONS] {
            let (result, created, critiqued) = run_counted(&dilator, Some(count));
            let session = result.unwrap();
            assert_eq!(session.simulated_iterations, count);
            assert_eq!((created, critiqued), (count, count));
        }
        assert_eq!(check_iteration_count(1).unwrap(), 1);
        assert_eq!(check_iteration_count(50).unwrap(), 50);
        assert!(check_iteration_count(0).is_err());
        assert!(check_iteration_count(51).is_err());
    }
}
