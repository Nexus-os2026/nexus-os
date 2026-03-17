//! IntentPredictor — predict user goals from screen context sequences.

use super::screen::ScreenContext;
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

// ── Types ───────────────────────────────────────────────────────────────

/// Confidence level for an intent prediction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConfidenceLevel {
    Low,
    Medium,
    High,
}

impl ConfidenceLevel {
    /// Classify a raw confidence score into a level.
    pub fn from_score(score: f64) -> Self {
        if score >= 0.8 {
            Self::High
        } else if score >= 0.5 {
            Self::Medium
        } else {
            Self::Low
        }
    }
}

/// A predicted intent with confidence and supporting evidence.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntentPrediction {
    /// Human-readable description of the predicted intent.
    pub intent: String,
    /// Confidence score in [0.0, 1.0].
    pub confidence: f64,
    /// Evidence supporting this prediction.
    pub evidence: Vec<String>,
    /// Suggested action the system could take.
    pub suggested_action: Option<String>,
    /// Unix timestamp in milliseconds when the prediction was made.
    pub timestamp: u64,
}

impl IntentPrediction {
    /// Return the confidence level for this prediction.
    pub fn confidence_level(&self) -> ConfidenceLevel {
        ConfidenceLevel::from_score(self.confidence)
    }
}

// ── IntentPredictor ─────────────────────────────────────────────────────

/// Predicts user intent from sequences of screen contexts.
///
/// Uses pattern matching on context history to recognize common scenarios
/// (e.g. error + search = "user is stuck", rapid tab switching = "comparing",
/// empty editor + hesitation = "writer's block").
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntentPredictor {
    /// Minimum confidence threshold for emitting predictions.
    min_confidence: f64,
}

impl IntentPredictor {
    /// Create a new predictor with default settings.
    pub fn new() -> Self {
        Self {
            min_confidence: 0.3,
        }
    }

    /// Create a predictor with a custom minimum confidence threshold.
    pub fn with_min_confidence(min_confidence: f64) -> Self {
        Self {
            min_confidence: min_confidence.clamp(0.0, 1.0),
        }
    }

    /// Set the minimum confidence threshold.
    pub fn set_min_confidence(&mut self, min_confidence: f64) {
        self.min_confidence = min_confidence.clamp(0.0, 1.0);
    }

    /// Predict user intent from a sequence of screen contexts.
    ///
    /// Returns predictions sorted by confidence (highest first),
    /// filtered to only include those above `min_confidence`.
    pub fn predict_intent(&self, contexts: &[ScreenContext]) -> Vec<IntentPrediction> {
        if contexts.is_empty() {
            return Vec::new();
        }

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        let mut predictions = Vec::new();

        // Pattern: error message visible → user may be stuck
        self.detect_error_pattern(contexts, now, &mut predictions);

        // Pattern: rapid app switching → user is comparing or searching
        self.detect_app_switching(contexts, now, &mut predictions);

        // Pattern: repeated same action → user is doing repetitive work
        self.detect_repetitive_actions(contexts, now, &mut predictions);

        // Pattern: search activity → user is looking for something
        self.detect_search_pattern(contexts, now, &mut predictions);

        // Pattern: idle with content → user may be reading or thinking
        self.detect_reading_pattern(contexts, now, &mut predictions);

        // Filter by minimum confidence and sort descending
        predictions.retain(|p| p.confidence >= self.min_confidence);
        predictions.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap());

        predictions
    }

    /// Detect error messages on screen, possibly combined with search activity.
    fn detect_error_pattern(
        &self,
        contexts: &[ScreenContext],
        now: u64,
        out: &mut Vec<IntentPrediction>,
    ) {
        let error_keywords = ["error", "Error", "ERROR", "failed", "Failed", "exception"];
        let has_error = contexts.iter().any(|ctx| {
            ctx.visible_text
                .iter()
                .any(|t| error_keywords.iter().any(|kw| t.contains(kw)))
        });

        if !has_error {
            return;
        }

        let has_search = contexts.iter().any(|ctx| {
            ctx.active_app.to_lowercase().contains("browser")
                || ctx
                    .user_action
                    .as_deref()
                    .is_some_and(|a| a.contains("search"))
        });

        let (confidence, intent, action) = if has_search {
            (
                0.85,
                "user is stuck on an error and searching for help",
                Some("Offer to diagnose the error and suggest fixes".to_string()),
            )
        } else {
            (
                0.65,
                "user encountered an error",
                Some("Offer to help diagnose the error".to_string()),
            )
        };

        out.push(IntentPrediction {
            intent: intent.to_string(),
            confidence,
            evidence: vec!["error message detected on screen".to_string()],
            suggested_action: action,
            timestamp: now,
        });
    }

    /// Detect rapid switching between applications.
    fn detect_app_switching(
        &self,
        contexts: &[ScreenContext],
        now: u64,
        out: &mut Vec<IntentPrediction>,
    ) {
        if contexts.len() < 3 {
            return;
        }

        let recent = &contexts[contexts.len().saturating_sub(5)..];
        let mut switches = 0u32;
        for pair in recent.windows(2) {
            if pair[0].active_app != pair[1].active_app {
                switches += 1;
            }
        }

        if switches >= 3 {
            out.push(IntentPrediction {
                intent: "user is comparing content across applications".to_string(),
                confidence: 0.7,
                evidence: vec![format!("{switches} app switches in recent contexts")],
                suggested_action: Some(
                    "Offer to arrange windows side-by-side or summarize differences".to_string(),
                ),
                timestamp: now,
            });
        }
    }

    /// Detect repeated user actions suggesting repetitive work.
    fn detect_repetitive_actions(
        &self,
        contexts: &[ScreenContext],
        now: u64,
        out: &mut Vec<IntentPrediction>,
    ) {
        let actions: Vec<&str> = contexts
            .iter()
            .filter_map(|ctx| ctx.user_action.as_deref())
            .collect();

        if actions.len() < 3 {
            return;
        }

        // Check if the last 3+ actions are the same
        let last = actions.last().unwrap();
        let repeat_count = actions.iter().rev().take_while(|a| *a == last).count();

        if repeat_count >= 3 {
            out.push(IntentPrediction {
                intent: "user is performing repetitive actions".to_string(),
                confidence: 0.75,
                evidence: vec![format!(
                    "action '{last}' repeated {repeat_count} times consecutively"
                )],
                suggested_action: Some("Offer to automate the repetitive task".to_string()),
                timestamp: now,
            });
        }
    }

    /// Detect search-related activity (browser with search terms, etc.).
    fn detect_search_pattern(
        &self,
        contexts: &[ScreenContext],
        now: u64,
        out: &mut Vec<IntentPrediction>,
    ) {
        let search_indicators = contexts.iter().filter(|ctx| {
            ctx.active_app.to_lowercase().contains("browser")
                && ctx
                    .visible_text
                    .iter()
                    .any(|t| t.contains("search") || t.contains("Search") || t.contains("Google"))
        });

        let count = search_indicators.count();
        if count >= 2 {
            out.push(IntentPrediction {
                intent: "user is actively researching a topic".to_string(),
                confidence: 0.6,
                evidence: vec![format!("{count} search-related screens detected")],
                suggested_action: Some(
                    "Offer to summarize search results or find relevant documentation".to_string(),
                ),
                timestamp: now,
            });
        }
    }

    /// Detect idle reading / thinking pattern (no actions, lots of text visible).
    fn detect_reading_pattern(
        &self,
        contexts: &[ScreenContext],
        now: u64,
        out: &mut Vec<IntentPrediction>,
    ) {
        if contexts.len() < 2 {
            return;
        }

        let recent = &contexts[contexts.len().saturating_sub(3)..];
        let all_idle = recent.iter().all(|ctx| ctx.user_action.is_none());
        let has_text = recent
            .iter()
            .all(|ctx| ctx.visible_text.iter().any(|t| t.len() > 20));

        if all_idle && has_text {
            out.push(IntentPrediction {
                intent: "user is reading or thinking about content".to_string(),
                confidence: 0.4,
                evidence: vec!["no user actions with substantial text visible".to_string()],
                suggested_action: Some("Offer to summarize or explain the content".to_string()),
                timestamp: now,
            });
        }
    }
}

impl Default for IntentPredictor {
    fn default() -> Self {
        Self::new()
    }
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    fn ctx(app: &str, text: Vec<&str>, action: Option<&str>) -> ScreenContext {
        ScreenContext {
            timestamp: 1000,
            active_app: app.to_string(),
            visible_text: text.into_iter().map(String::from).collect(),
            ui_elements: vec![],
            user_action: action.map(String::from),
        }
    }

    #[test]
    fn empty_contexts_returns_nothing() {
        let predictor = IntentPredictor::new();
        assert!(predictor.predict_intent(&[]).is_empty());
    }

    #[test]
    fn detects_error_pattern() {
        let predictor = IntentPredictor::new();
        let contexts = vec![ctx(
            "editor",
            vec!["TypeError: undefined is not a function"],
            None,
        )];
        let preds = predictor.predict_intent(&contexts);
        assert!(!preds.is_empty());
        assert!(preds[0].intent.contains("error"));
    }

    #[test]
    fn detects_error_plus_search() {
        let predictor = IntentPredictor::new();
        let contexts = vec![
            ctx("editor", vec!["Error: connection refused"], None),
            ctx("browser", vec!["search results"], Some("search")),
        ];
        let preds = predictor.predict_intent(&contexts);
        assert!(!preds.is_empty());
        assert!(preds[0].intent.contains("stuck"));
        assert!(preds[0].confidence >= 0.8);
    }

    #[test]
    fn detects_app_switching() {
        let predictor = IntentPredictor::new();
        let contexts = vec![
            ctx("editor", vec![], None),
            ctx("browser", vec![], None),
            ctx("terminal", vec![], None),
            ctx("editor", vec![], None),
            ctx("browser", vec![], None),
        ];
        let preds = predictor.predict_intent(&contexts);
        let switching = preds.iter().find(|p| p.intent.contains("comparing"));
        assert!(switching.is_some());
    }

    #[test]
    fn detects_repetitive_actions() {
        let predictor = IntentPredictor::new();
        let contexts = vec![
            ctx("editor", vec![], Some("copy-paste")),
            ctx("editor", vec![], Some("copy-paste")),
            ctx("editor", vec![], Some("copy-paste")),
            ctx("editor", vec![], Some("copy-paste")),
        ];
        let preds = predictor.predict_intent(&contexts);
        let repetitive = preds.iter().find(|p| p.intent.contains("repetitive"));
        assert!(repetitive.is_some());
    }

    #[test]
    fn detects_reading_pattern() {
        let predictor = IntentPredictor::new();
        let contexts = vec![
            ctx(
                "browser",
                vec!["This is a long article about distributed systems and consensus algorithms"],
                None,
            ),
            ctx(
                "browser",
                vec!["Continuing the discussion of Raft consensus protocol implementation details"],
                None,
            ),
            ctx(
                "browser",
                vec!["The leader election process involves randomized timeouts and heartbeat messages"],
                None,
            ),
        ];
        let preds = predictor.predict_intent(&contexts);
        let reading = preds.iter().find(|p| p.intent.contains("reading"));
        assert!(reading.is_some());
    }

    #[test]
    fn min_confidence_filters() {
        let predictor = IntentPredictor::with_min_confidence(0.9);
        let contexts = vec![
            ctx(
                "browser",
                vec!["This is a long article about distributed systems and consensus algorithms"],
                None,
            ),
            ctx(
                "browser",
                vec!["Another long paragraph discussing the finer points of something important"],
                None,
            ),
        ];
        let preds = predictor.predict_intent(&contexts);
        // Reading pattern has confidence 0.4, should be filtered
        assert!(preds.iter().all(|p| p.confidence >= 0.9));
    }

    #[test]
    fn confidence_level_classification() {
        assert_eq!(ConfidenceLevel::from_score(0.9), ConfidenceLevel::High);
        assert_eq!(ConfidenceLevel::from_score(0.8), ConfidenceLevel::High);
        assert_eq!(ConfidenceLevel::from_score(0.6), ConfidenceLevel::Medium);
        assert_eq!(ConfidenceLevel::from_score(0.5), ConfidenceLevel::Medium);
        assert_eq!(ConfidenceLevel::from_score(0.3), ConfidenceLevel::Low);
        assert_eq!(ConfidenceLevel::from_score(0.0), ConfidenceLevel::Low);
    }

    #[test]
    fn prediction_serialization() {
        let pred = IntentPrediction {
            intent: "test".into(),
            confidence: 0.75,
            evidence: vec!["evidence".into()],
            suggested_action: Some("do something".into()),
            timestamp: 1000,
        };
        let json = serde_json::to_string(&pred).unwrap();
        let deser: IntentPrediction = serde_json::from_str(&json).unwrap();
        assert_eq!(deser.intent, "test");
        assert_eq!(deser.confidence_level(), ConfidenceLevel::Medium);
    }

    #[test]
    fn confidence_level_serialize() {
        let level = ConfidenceLevel::High;
        let json = serde_json::to_string(&level).unwrap();
        let deser: ConfidenceLevel = serde_json::from_str(&json).unwrap();
        assert_eq!(deser, ConfidenceLevel::High);
    }

    #[test]
    fn ui_element_not_required_for_predictions() {
        // Predictions work without UI elements
        let predictor = IntentPredictor::new();
        let contexts = vec![ctx("editor", vec!["Error: file not found"], None)];
        let preds = predictor.predict_intent(&contexts);
        assert!(!preds.is_empty());
    }
}
