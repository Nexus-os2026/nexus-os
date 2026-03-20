//! Voice Project Builder — continuous speech-to-intent accumulation for hands-free app creation.

use serde::{Deserialize, Serialize};

/// A single chunk of transcribed speech with a timestamp.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptChunk {
    pub text: String,
    pub timestamp_secs: u64,
}

/// Accumulated intent extracted from a stream of speech.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntentAccumulator {
    pub detected_features: Vec<String>,
    pub detected_entities: Vec<String>,
    pub project_type: Option<String>,
    pub confidence: f64,
    pub ambiguities: Vec<String>,
    pub core_idea: Option<String>,
    pub tech_preferences: Vec<String>,
}

impl Default for IntentAccumulator {
    fn default() -> Self {
        Self {
            detected_features: Vec::new(),
            detected_entities: Vec::new(),
            project_type: None,
            confidence: 0.0,
            ambiguities: Vec::new(),
            core_idea: None,
            tech_preferences: Vec::new(),
        }
    }
}

/// Status of the voice project builder.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum VoiceProjectStatus {
    Idle,
    Listening,
    Analyzing,
    BuildingStarted,
    Completed,
}

/// The voice-to-project engine that accumulates intent from speech and triggers builds.
#[derive(Debug, Clone)]
pub struct VoiceProjectBuilder {
    pub transcript_buffer: Vec<TranscriptChunk>,
    pub intent: IntentAccumulator,
    pub confidence_threshold: f64,
    pub reanalyze_interval_secs: u64,
    pub status: VoiceProjectStatus,
    pub last_analysis_at: Option<u64>,
    pub autopilot_triggered: bool,
}

impl Default for VoiceProjectBuilder {
    fn default() -> Self {
        Self {
            transcript_buffer: Vec::new(),
            intent: IntentAccumulator::default(),
            confidence_threshold: 0.8,
            reanalyze_interval_secs: 30,
            status: VoiceProjectStatus::Idle,
            last_analysis_at: None,
            autopilot_triggered: false,
        }
    }
}

impl VoiceProjectBuilder {
    pub fn new(confidence_threshold: f64) -> Self {
        Self {
            confidence_threshold,
            ..Default::default()
        }
    }

    /// Start listening mode.
    pub fn start_listening(&mut self) {
        self.status = VoiceProjectStatus::Listening;
        self.transcript_buffer.clear();
        self.intent = IntentAccumulator::default();
        self.autopilot_triggered = false;
        self.last_analysis_at = None;
    }

    /// Stop listening and return the accumulated intent.
    pub fn stop_listening(&mut self) -> IntentAccumulator {
        self.status = VoiceProjectStatus::Idle;
        self.intent.clone()
    }

    /// Add a new transcript chunk.
    pub fn add_chunk(&mut self, text: String, timestamp_secs: u64) {
        self.transcript_buffer.push(TranscriptChunk {
            text,
            timestamp_secs,
        });
    }

    /// Get the full concatenated transcript.
    pub fn full_transcript(&self) -> String {
        self.transcript_buffer
            .iter()
            .map(|c| c.text.as_str())
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Check whether enough time has passed since last analysis.
    pub fn should_reanalyze(&self, current_time_secs: u64) -> bool {
        match self.last_analysis_at {
            None => self.transcript_buffer.len() >= 2,
            Some(last) => current_time_secs.saturating_sub(last) >= self.reanalyze_interval_secs,
        }
    }

    /// Build the LLM prompt for intent extraction from accumulated speech.
    pub fn build_intent_prompt(&self) -> (String, String) {
        let transcript = self.full_transcript();
        let system = "You are an intent extraction engine. A user is describing an app idea \
                       in a stream of consciousness. Extract the structured intent."
            .to_string();
        let user = format!(
            "A user has been describing an app idea in a stream of consciousness:\n\
             '{transcript}'\n\n\
             Extract:\n\
             1. The core app idea (one sentence)\n\
             2. Features mentioned (list)\n\
             3. Entities/data types (users, photos, votes, etc.)\n\
             4. Any technical preferences mentioned (mobile, web, language)\n\
             5. Ambiguities that need clarification\n\
             6. Confidence that you understand the full intent (0-1)\n\
             7. Project type (web app, mobile app, CLI, API, etc.)\n\n\
             Return JSON matching the IntentAccumulator schema."
        );
        (system, user)
    }

    /// Update the intent from a parsed LLM response.
    pub fn update_intent(
        &mut self,
        response: &str,
        current_time_secs: u64,
    ) -> Result<bool, String> {
        let new_intent: IntentAccumulator =
            serde_json::from_str(response).map_err(|e| format!("Failed to parse intent: {e}"))?;

        self.intent = new_intent;
        self.last_analysis_at = Some(current_time_secs);
        self.status = VoiceProjectStatus::Analyzing;

        // Check if we should trigger autopilot
        if self.intent.confidence >= self.confidence_threshold && self.intent.ambiguities.is_empty()
        {
            self.autopilot_triggered = true;
            self.status = VoiceProjectStatus::BuildingStarted;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Check if autopilot has been triggered.
    pub fn is_autopilot_triggered(&self) -> bool {
        self.autopilot_triggered
    }

    /// Get a summary of the current state.
    pub fn get_status_summary(&self) -> VoiceProjectSummary {
        VoiceProjectSummary {
            status: self.status.clone(),
            chunks_received: self.transcript_buffer.len(),
            confidence: self.intent.confidence,
            features_detected: self.intent.detected_features.len(),
            entities_detected: self.intent.detected_entities.len(),
            ambiguities: self.intent.ambiguities.len(),
            core_idea: self.intent.core_idea.clone(),
            autopilot_triggered: self.autopilot_triggered,
        }
    }
}

/// Summary of the voice project builder state for UI display.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoiceProjectSummary {
    pub status: VoiceProjectStatus,
    pub chunks_received: usize,
    pub confidence: f64,
    pub features_detected: usize,
    pub entities_detected: usize,
    pub ambiguities: usize,
    pub core_idea: Option<String>,
    pub autopilot_triggered: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_voice_builder_default() {
        let builder = VoiceProjectBuilder::default();
        assert_eq!(builder.status, VoiceProjectStatus::Idle);
        assert!((builder.confidence_threshold - 0.8).abs() < f64::EPSILON);
        assert!(!builder.autopilot_triggered);
    }

    #[test]
    fn test_start_stop_listening() {
        let mut builder = VoiceProjectBuilder::default();
        builder.start_listening();
        assert_eq!(builder.status, VoiceProjectStatus::Listening);

        let intent = builder.stop_listening();
        assert_eq!(builder.status, VoiceProjectStatus::Idle);
        assert!(intent.detected_features.is_empty());
    }

    #[test]
    fn test_add_chunks_and_transcript() {
        let mut builder = VoiceProjectBuilder::default();
        builder.start_listening();
        builder.add_chunk("I want a photo sharing app".into(), 100);
        builder.add_chunk("with voting and leaderboards".into(), 130);

        let transcript = builder.full_transcript();
        assert!(transcript.contains("photo sharing"));
        assert!(transcript.contains("voting"));
    }

    #[test]
    fn test_should_reanalyze() {
        let mut builder = VoiceProjectBuilder::default();
        // No chunks yet — should not reanalyze
        assert!(!builder.should_reanalyze(100));

        builder.add_chunk("chunk1".into(), 100);
        builder.add_chunk("chunk2".into(), 110);
        // 2 chunks, no prior analysis
        assert!(builder.should_reanalyze(120));

        builder.last_analysis_at = Some(120);
        // Too soon (only 10s since last)
        assert!(!builder.should_reanalyze(130));
        // Enough time (30s interval)
        assert!(builder.should_reanalyze(150));
    }

    #[test]
    fn test_update_intent_triggers_autopilot() {
        let mut builder = VoiceProjectBuilder::new(0.8);
        builder.start_listening();

        let response = serde_json::json!({
            "detected_features": ["photo upload", "voting", "leaderboard"],
            "detected_entities": ["User", "Photo", "Vote"],
            "project_type": "web app",
            "confidence": 0.9,
            "ambiguities": [],
            "core_idea": "Photo sharing app with voting",
            "tech_preferences": ["React", "mobile-friendly"]
        });

        let triggered = builder.update_intent(&response.to_string(), 200).unwrap();
        assert!(triggered);
        assert!(builder.is_autopilot_triggered());
        assert_eq!(builder.status, VoiceProjectStatus::BuildingStarted);
    }

    #[test]
    fn test_update_intent_no_trigger_low_confidence() {
        let mut builder = VoiceProjectBuilder::new(0.8);
        builder.start_listening();

        let response = serde_json::json!({
            "detected_features": ["photo upload"],
            "detected_entities": ["User"],
            "project_type": null,
            "confidence": 0.4,
            "ambiguities": ["What kind of voting?"],
            "core_idea": "Some kind of photo app",
            "tech_preferences": []
        });

        let triggered = builder.update_intent(&response.to_string(), 200).unwrap();
        assert!(!triggered);
        assert!(!builder.is_autopilot_triggered());
    }

    #[test]
    fn test_update_intent_no_trigger_with_ambiguities() {
        let mut builder = VoiceProjectBuilder::new(0.8);
        builder.start_listening();

        let response = serde_json::json!({
            "detected_features": ["photo upload", "voting"],
            "detected_entities": ["User", "Photo"],
            "project_type": "web app",
            "confidence": 0.9,
            "ambiguities": ["Should voting be anonymous?"],
            "core_idea": "Photo voting app",
            "tech_preferences": []
        });

        let triggered = builder.update_intent(&response.to_string(), 200).unwrap();
        assert!(!triggered);
    }

    #[test]
    fn test_build_intent_prompt() {
        let mut builder = VoiceProjectBuilder::default();
        builder.add_chunk("I want a todo app".into(), 100);
        builder.add_chunk("with categories and priorities".into(), 110);

        let (system, user) = builder.build_intent_prompt();
        assert!(system.contains("intent extraction"));
        assert!(user.contains("todo app"));
        assert!(user.contains("categories"));
    }

    #[test]
    fn test_get_status_summary() {
        let mut builder = VoiceProjectBuilder::default();
        builder.start_listening();
        builder.add_chunk("test chunk".into(), 100);

        let summary = builder.get_status_summary();
        assert_eq!(summary.status, VoiceProjectStatus::Listening);
        assert_eq!(summary.chunks_received, 1);
        assert!(!summary.autopilot_triggered);
    }
}
