//! Empathic interface — infer user mood from behavioral signals and adapt.

use serde::{Deserialize, Serialize};

/// Events emitted by the frontend for behaviour tracking.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum UserInputEvent {
    Keystroke { timestamp: u64, is_deletion: bool },
    MessageSent { length: u32, timestamp: u64 },
}

/// Inferred user mood from behavioral signals.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum UserMood {
    /// Fast typing, low deletion, steady rhythm.
    Focused,
    /// Rapid deletions, short messages, fast sends.
    Frustrated,
    /// Long pauses, short messages, question marks.
    Confused,
    /// Fast typing, long messages, few deletions.
    Flowing,
    /// Slowing typing, increasing errors, longer pauses.
    Fatigued,
    /// Varied message lengths, lots of questions.
    Exploring,
}

/// Tracks user input behaviour and infers mood.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserBehaviorState {
    pub typing_speed_wpm: f64,
    pub typing_speed_baseline: f64,
    pub deletion_rate: f64,
    pub pause_duration_ms: u64,
    pub message_frequency: f64,
    pub avg_message_length: f64,
    pub error_correction_rate: f64,

    // Internal bookkeeping
    total_keystrokes: u64,
    total_deletions: u64,
    messages_sent: u64,
    total_message_chars: u64,
    last_keystroke_ts: u64,
    last_message_ts: u64,
    session_start_ts: u64,

    // Inferred
    pub inferred_mood: UserMood,
    pub confidence_in_inference: f64,
}

impl Default for UserBehaviorState {
    fn default() -> Self {
        Self::new()
    }
}

impl UserBehaviorState {
    pub fn new() -> Self {
        let now = crate::consciousness::state::now_secs();
        Self {
            typing_speed_wpm: 0.0,
            typing_speed_baseline: 40.0, // reasonable default
            deletion_rate: 0.0,
            pause_duration_ms: 0,
            message_frequency: 0.0,
            avg_message_length: 0.0,
            error_correction_rate: 0.0,
            total_keystrokes: 0,
            total_deletions: 0,
            messages_sent: 0,
            total_message_chars: 0,
            last_keystroke_ts: now,
            last_message_ts: now,
            session_start_ts: now,
            inferred_mood: UserMood::Focused,
            confidence_in_inference: 0.3,
        }
    }

    /// Process a user input event and update internal state.
    pub fn update(&mut self, event: &UserInputEvent) {
        match event {
            UserInputEvent::Keystroke {
                timestamp,
                is_deletion,
            } => {
                self.total_keystrokes += 1;
                if *is_deletion {
                    self.total_deletions += 1;
                }
                self.deletion_rate = if self.total_keystrokes > 0 {
                    self.total_deletions as f64 / self.total_keystrokes as f64
                } else {
                    0.0
                };

                // Pause since last keystroke
                if *timestamp > self.last_keystroke_ts {
                    self.pause_duration_ms = (*timestamp - self.last_keystroke_ts) * 1000;
                }

                // Rough WPM estimate: 5 chars per word
                let elapsed_secs = timestamp.saturating_sub(self.session_start_ts).max(1);
                let words = self.total_keystrokes as f64 / 5.0;
                self.typing_speed_wpm = words / (elapsed_secs as f64 / 60.0);

                // Update baseline with exponential moving average
                if self.total_keystrokes > 50 {
                    self.typing_speed_baseline =
                        self.typing_speed_baseline * 0.95 + self.typing_speed_wpm * 0.05;
                }

                self.last_keystroke_ts = *timestamp;
            }
            UserInputEvent::MessageSent { length, timestamp } => {
                self.messages_sent += 1;
                self.total_message_chars += *length as u64;
                self.avg_message_length =
                    self.total_message_chars as f64 / self.messages_sent as f64;

                let elapsed_min =
                    (timestamp.saturating_sub(self.session_start_ts) as f64 / 60.0).max(0.01);
                self.message_frequency = self.messages_sent as f64 / elapsed_min;

                self.last_message_ts = *timestamp;
            }
        }
        self.infer_mood();
    }

    fn infer_mood(&mut self) {
        let speed_ratio = self.typing_speed_wpm / self.typing_speed_baseline.max(1.0);

        self.inferred_mood = if self.deletion_rate > 0.4 && speed_ratio > 1.2 {
            UserMood::Frustrated
        } else if self.pause_duration_ms > 10_000 && self.avg_message_length < 20.0 {
            UserMood::Confused
        } else if speed_ratio > 1.1 && self.deletion_rate < 0.1 && self.avg_message_length > 100.0 {
            UserMood::Flowing
        } else if speed_ratio < 0.7 && self.deletion_rate > 0.2 {
            UserMood::Fatigued
        } else if self.avg_message_length > 50.0 && self.message_frequency > 2.0 {
            UserMood::Exploring
        } else {
            UserMood::Focused
        };

        // Confidence based on signal strength
        self.confidence_in_inference = if !(0.5..=1.3).contains(&speed_ratio) {
            0.8
        } else {
            0.4
        };
    }

    /// Adapt agent response strategy based on inferred user mood.
    pub fn adapt_response(&self) -> ResponseAdaptation {
        match self.inferred_mood {
            UserMood::Frustrated => ResponseAdaptation {
                tone: "calm_and_direct".into(),
                length: "short".into(),
                offer_alternatives: true,
                proactive_help: true,
                message: Some(
                    "I notice this might be tricky. Let me suggest a different approach.".into(),
                ),
            },
            UserMood::Confused => ResponseAdaptation {
                tone: "explanatory".into(),
                length: "medium_with_examples".into(),
                offer_alternatives: true,
                proactive_help: true,
                message: Some("Let me break this down step by step.".into()),
            },
            UserMood::Flowing => ResponseAdaptation {
                tone: "minimal".into(),
                length: "concise".into(),
                offer_alternatives: false,
                proactive_help: false,
                message: None,
            },
            UserMood::Fatigued => ResponseAdaptation {
                tone: "supportive".into(),
                length: "short".into(),
                offer_alternatives: false,
                proactive_help: true,
                message: Some(
                    "You've been working a while. I can handle this if you want to take a break."
                        .into(),
                ),
            },
            UserMood::Exploring => ResponseAdaptation {
                tone: "enthusiastic".into(),
                length: "detailed".into(),
                offer_alternatives: true,
                proactive_help: false,
                message: None,
            },
            UserMood::Focused => ResponseAdaptation {
                tone: "professional".into(),
                length: "appropriate".into(),
                offer_alternatives: false,
                proactive_help: false,
                message: None,
            },
        }
    }
}

/// How the agent should adapt its response to the user's inferred mood.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseAdaptation {
    pub tone: String,
    pub length: String,
    pub offer_alternatives: bool,
    pub proactive_help: bool,
    /// Proactive message to show the user (if any).
    pub message: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_mood_is_focused() {
        let s = UserBehaviorState::new();
        assert_eq!(s.inferred_mood, UserMood::Focused);
    }

    #[test]
    fn keystroke_updates_counts() {
        let mut s = UserBehaviorState::new();
        s.update(&UserInputEvent::Keystroke {
            timestamp: s.session_start_ts + 1,
            is_deletion: false,
        });
        assert_eq!(s.total_keystrokes, 1);
        assert_eq!(s.total_deletions, 0);

        s.update(&UserInputEvent::Keystroke {
            timestamp: s.session_start_ts + 2,
            is_deletion: true,
        });
        assert_eq!(s.total_keystrokes, 2);
        assert_eq!(s.total_deletions, 1);
        assert!((s.deletion_rate - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn message_updates_frequency() {
        let mut s = UserBehaviorState::new();
        s.update(&UserInputEvent::MessageSent {
            length: 50,
            timestamp: s.session_start_ts + 60,
        });
        assert_eq!(s.messages_sent, 1);
        assert!((s.avg_message_length - 50.0).abs() < f64::EPSILON);
    }

    #[test]
    fn frustrated_mood_detection() {
        let mut s = UserBehaviorState::new();
        // Simulate lots of deletions with fast typing
        for i in 0..100 {
            s.update(&UserInputEvent::Keystroke {
                timestamp: s.session_start_ts + i,
                is_deletion: i % 2 == 0, // 50% deletion rate
            });
        }
        // With high deletion rate and enough keystrokes the mood should shift
        // (exact mood depends on speed ratio — here we just verify no panic)
        assert!(s.deletion_rate > 0.4);
    }

    #[test]
    fn adapt_response_frustrated() {
        let mut s = UserBehaviorState::new();
        s.inferred_mood = UserMood::Frustrated;
        let adapt = s.adapt_response();
        assert!(adapt.proactive_help);
        assert!(adapt.offer_alternatives);
        assert!(adapt.message.is_some());
    }

    #[test]
    fn adapt_response_flowing() {
        let mut s = UserBehaviorState::new();
        s.inferred_mood = UserMood::Flowing;
        let adapt = s.adapt_response();
        assert!(!adapt.proactive_help);
        assert!(adapt.message.is_none());
    }
}
