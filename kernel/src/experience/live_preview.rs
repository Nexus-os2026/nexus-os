//! Live Preview Engine — real-time visual progress while the autopilot builds.
//!
//! Emits human-friendly status frames so users see their product taking shape
//! instead of watching a log scroll.

use serde::{Deserialize, Serialize};

/// A single point-in-time snapshot of build progress.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreviewFrame {
    pub timestamp: u64,
    pub screenshot_url: Option<String>,
    pub progress_percent: f64,
    pub completed_features: Vec<String>,
    pub current_feature: String,
    pub upcoming_features: Vec<String>,
    /// One plain-English sentence describing what just happened.
    pub human_status: String,
}

/// Tracks preview frames for a project build in progress.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LivePreviewEngine {
    pub project_id: String,
    pub preview_frames: Vec<PreviewFrame>,
    pub total_tasks: u32,
    pub completed_tasks: u32,
}

impl LivePreviewEngine {
    pub fn new(project_id: &str, total_tasks: u32) -> Self {
        Self {
            project_id: project_id.to_string(),
            preview_frames: Vec::new(),
            total_tasks,
            completed_tasks: 0,
        }
    }

    /// Record a new preview frame after a build step completes.
    pub fn push_frame(
        &mut self,
        feature_completed: &str,
        current_feature: &str,
        upcoming: Vec<String>,
        human_status: &str,
        screenshot_url: Option<String>,
    ) -> PreviewFrame {
        self.completed_tasks += 1;
        let progress = if self.total_tasks == 0 {
            100.0
        } else {
            (self.completed_tasks as f64 / self.total_tasks as f64) * 100.0
        };

        let mut completed: Vec<String> = self
            .preview_frames
            .iter()
            .flat_map(|f| f.completed_features.clone())
            .collect();
        completed.push(feature_completed.to_string());

        let frame = PreviewFrame {
            timestamp: now_secs(),
            screenshot_url,
            progress_percent: progress,
            completed_features: completed,
            current_feature: current_feature.to_string(),
            upcoming_features: upcoming,
            human_status: human_status.to_string(),
        };
        self.preview_frames.push(frame.clone());
        frame
    }

    /// Get the latest preview frame, if any.
    pub fn latest(&self) -> Option<&PreviewFrame> {
        self.preview_frames.last()
    }

    /// Current progress as a percentage.
    pub fn progress_percent(&self) -> f64 {
        if self.total_tasks == 0 {
            100.0
        } else {
            (self.completed_tasks as f64 / self.total_tasks as f64) * 100.0
        }
    }
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_push_frame_progress() {
        let mut engine = LivePreviewEngine::new("proj-1", 4);
        assert_eq!(engine.progress_percent(), 0.0);

        let frame = engine.push_frame(
            "Homepage",
            "Product page",
            vec!["Cart".into(), "Checkout".into()],
            "Built the homepage with your brand colors.",
            None,
        );
        assert!((frame.progress_percent - 25.0).abs() < 0.01);
        assert_eq!(frame.completed_features, vec!["Homepage"]);
        assert_eq!(frame.current_feature, "Product page");
    }

    #[test]
    fn test_latest_frame() {
        let mut engine = LivePreviewEngine::new("proj-2", 2);
        assert!(engine.latest().is_none());
        engine.push_frame("A", "B", vec![], "Did A.", None);
        engine.push_frame("B", "", vec![], "Did B.", None);
        assert_eq!(engine.latest().unwrap().human_status, "Did B.");
    }

    #[test]
    fn test_zero_tasks() {
        let engine = LivePreviewEngine::new("empty", 0);
        assert_eq!(engine.progress_percent(), 100.0);
    }
}
