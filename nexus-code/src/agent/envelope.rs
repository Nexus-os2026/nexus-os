//! Behavioral envelope — mathematical drift detection using cosine similarity.
//!
//! Monitors the agent's action distribution and detects drift from a baseline.
//! The envelope config is IMMUTABLE from the agent's perspective.

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

/// Action categories for behavioral tracking.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ActionCategory {
    Read,
    Write,
    Execute,
    LlmCall,
    Search,
}

impl ActionCategory {
    /// Classify a tool name into an action category.
    pub fn from_tool(tool_name: &str) -> Self {
        match tool_name {
            "file_read" => Self::Read,
            "file_write" | "file_edit" => Self::Write,
            "bash" | "shell" => Self::Execute,
            "search" | "glob" => Self::Search,
            "llm_call" | "sub_agent" => Self::LlmCall,
            _ => Self::Execute, // conservative default
        }
    }
}

/// Configuration for the behavioral envelope.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvelopeConfig {
    /// Sliding window size for action tracking (default: 50).
    pub window_size: usize,
    /// Cosine similarity threshold for warning (default: 0.7).
    pub warn_threshold: f64,
    /// Cosine similarity threshold for alert (default: 0.5).
    pub alert_threshold: f64,
    /// Cosine similarity threshold for auto-termination (default: 0.3).
    pub terminate_threshold: f64,
    /// Baseline action distribution (normalized vector).
    /// Order: [Read, Write, Execute, LlmCall, Search]
    pub baseline: [f64; 5],
    /// Whether the envelope is active.
    pub enabled: bool,
}

impl Default for EnvelopeConfig {
    fn default() -> Self {
        Self {
            window_size: 50,
            warn_threshold: 0.7,
            alert_threshold: 0.5,
            terminate_threshold: 0.3,
            baseline: [0.4, 0.2, 0.15, 0.15, 0.1],
            enabled: true,
        }
    }
}

/// The result of an envelope check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EnvelopeStatus {
    Normal,
    Warning { similarity: u32 },
    Alert { similarity: u32 },
    Terminate { similarity: u32 },
}

/// Behavioral envelope tracker.
pub struct BehavioralEnvelope {
    config: EnvelopeConfig,
    window: VecDeque<ActionCategory>,
    total_counts: [u64; 5],
}

impl BehavioralEnvelope {
    pub fn new(config: EnvelopeConfig) -> Self {
        Self {
            config,
            window: VecDeque::new(),
            total_counts: [0; 5],
        }
    }

    /// Record an action and check the envelope.
    pub fn record_action(&mut self, category: ActionCategory) -> EnvelopeStatus {
        self.window.push_back(category);
        if self.window.len() > self.config.window_size {
            self.window.pop_front();
        }

        let idx = Self::category_index(category);
        self.total_counts[idx] += 1;

        if !self.config.enabled || self.window.len() < self.config.window_size / 2 {
            return EnvelopeStatus::Normal;
        }

        let similarity = self.compute_similarity();
        let sim_pct = (similarity * 100.0) as u32;

        if similarity < self.config.terminate_threshold {
            EnvelopeStatus::Terminate {
                similarity: sim_pct,
            }
        } else if similarity < self.config.alert_threshold {
            EnvelopeStatus::Alert {
                similarity: sim_pct,
            }
        } else if similarity < self.config.warn_threshold {
            EnvelopeStatus::Warning {
                similarity: sim_pct,
            }
        } else {
            EnvelopeStatus::Normal
        }
    }

    fn compute_similarity(&self) -> f64 {
        let current = self.current_distribution();
        cosine_similarity(&current, &self.config.baseline)
    }

    fn current_distribution(&self) -> [f64; 5] {
        let mut counts = [0u64; 5];
        for action in &self.window {
            counts[Self::category_index(*action)] += 1;
        }
        let total = counts.iter().sum::<u64>() as f64;
        if total == 0.0 {
            return [0.0; 5];
        }
        [
            counts[0] as f64 / total,
            counts[1] as f64 / total,
            counts[2] as f64 / total,
            counts[3] as f64 / total,
            counts[4] as f64 / total,
        ]
    }

    fn category_index(cat: ActionCategory) -> usize {
        match cat {
            ActionCategory::Read => 0,
            ActionCategory::Write => 1,
            ActionCategory::Execute => 2,
            ActionCategory::LlmCall => 3,
            ActionCategory::Search => 4,
        }
    }

    /// Get a summary of the current state.
    pub fn summary(&self) -> String {
        let dist = self.current_distribution();
        let sim = if self.window.len() >= self.config.window_size / 2 {
            self.compute_similarity()
        } else {
            1.0
        };
        format!(
            "Envelope: sim={:.2} [R={:.0}% W={:.0}% X={:.0}% L={:.0}% S={:.0}%] window={}/{}",
            sim,
            dist[0] * 100.0,
            dist[1] * 100.0,
            dist[2] * 100.0,
            dist[3] * 100.0,
            dist[4] * 100.0,
            self.window.len(),
            self.config.window_size,
        )
    }

    /// Get the config (read-only).
    pub fn config(&self) -> &EnvelopeConfig {
        &self.config
    }

    /// Get total action counts.
    pub fn total_counts(&self) -> &[u64; 5] {
        &self.total_counts
    }
}

/// Compute cosine similarity between two 5-element vectors.
pub fn cosine_similarity(a: &[f64; 5], b: &[f64; 5]) -> f64 {
    let dot: f64 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let mag_a: f64 = a.iter().map(|x| x * x).sum::<f64>().sqrt();
    let mag_b: f64 = b.iter().map(|x| x * x).sum::<f64>().sqrt();

    if mag_a == 0.0 || mag_b == 0.0 {
        return 0.0;
    }

    (dot / (mag_a * mag_b)).clamp(0.0, 1.0)
}
