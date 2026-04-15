use super::filesystem::GovernedFilesystem;
use super::types::{ActionResult, Actuator, ActuatorContext, ActuatorError, SideEffect};
use crate::capabilities::has_capability;
use crate::cognitive::types::PlannedAction;
use std::env;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};

const FUEL_COST_TTS: f64 = 4.0;

/// Once piper is detected as missing, disable it for the rest of the session.
/// This prevents a crash loop from repeatedly trying to spawn a binary that doesn't exist.
static PIPER_DISABLED: AtomicBool = AtomicBool::new(false);

#[derive(Debug, Clone, Default)]
pub struct TtsActuator;

impl TtsActuator {
    fn resolve_output_path(
        context: &ActuatorContext,
        output_path: &str,
    ) -> Result<PathBuf, ActuatorError> {
        let safe_path = GovernedFilesystem::resolve_safe_path(&context.working_dir, output_path)?;
        if let Some(parent) = safe_path.parent() {
            std::fs::create_dir_all(parent).map_err(|error| {
                ActuatorError::IoError(format!("create tts output dir: {error}"))
            })?;
        }
        Ok(safe_path)
    }

    /// Check if piper binary exists in PATH before trying to use it.
    fn is_piper_available() -> bool {
        if PIPER_DISABLED.load(Ordering::Relaxed) {
            return false;
        }
        // Check if piper binary exists using `which piper`
        let available = Command::new("which")
            .arg("piper")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if !available {
            eprintln!("[tts] piper CLI not found in PATH — disabling piper TTS for this session");
            PIPER_DISABLED.store(true, Ordering::Relaxed);
        }
        available
    }

    fn select_provider(provider: Option<&str>) -> Result<String, ActuatorError> {
        if let Some(provider) = provider {
            let p = provider.to_lowercase();
            // If explicitly requesting piper but it's not available, fail immediately
            if (p == "piper" || p == "piper-tts") && !Self::is_piper_available() {
                return Err(ActuatorError::IoError(
                    "piper TTS not installed — skipping voice synthesis".to_string(),
                ));
            }
            return Ok(p);
        }
        if env::var("PIPER_MODEL").is_ok() && Self::is_piper_available() {
            return Ok("piper".to_string());
        }
        if env::var("OPENAI_API_KEY").is_ok() {
            return Ok("openai".to_string());
        }
        Err(ActuatorError::IoError(
            "no TTS provider available (piper not installed, no OPENAI_API_KEY set)".to_string(),
        ))
    }

    fn run_piper(
        text: &str,
        voice: Option<&str>,
        output_path: &PathBuf,
    ) -> Result<(), ActuatorError> {
        // Double-check piper is available (should have been checked by select_provider,
        // but be defensive)
        if !Self::is_piper_available() {
            return Err(ActuatorError::IoError(
                "piper not available — TTS disabled for this session".to_string(),
            ));
        }
        let model = voice
            .map(str::to_string)
            // Optional: missing PIPER_MODEL env var means no model override, fall through to error
            .or_else(|| env::var("PIPER_MODEL").ok())
            .ok_or_else(|| ActuatorError::IoError("PIPER_MODEL is not set".to_string()))?;
        let mut child = Command::new("piper")
            .args(["--model", &model, "--output_file"])
            .arg(output_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| {
                // If spawn fails, disable piper for the session
                PIPER_DISABLED.store(true, Ordering::Relaxed);
                ActuatorError::IoError(format!(
                    "spawn piper failed (disabling for session): {error}"
                ))
            })?;
        if let Some(mut stdin) = child.stdin.take() {
            stdin
                .write_all(text.as_bytes())
                .map_err(|error| ActuatorError::IoError(format!("write piper stdin: {error}")))?;
        }
        let output = child
            .wait_with_output()
            .map_err(|error| ActuatorError::IoError(format!("wait for piper: {error}")))?;
        if !output.status.success() {
            return Err(ActuatorError::IoError(format!(
                "piper failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }
        Ok(())
    }

    fn run_openai(
        text: &str,
        voice: Option<&str>,
        model: Option<&str>,
        output_path: &PathBuf,
    ) -> Result<(), ActuatorError> {
        let api_key = env::var("OPENAI_API_KEY")
            .map_err(|_| ActuatorError::IoError("OPENAI_API_KEY is not set".to_string()))?;
        let body = serde_json::json!({
            "model": model.unwrap_or("gpt-4o-mini-tts"),
            "voice": voice.unwrap_or("alloy"),
            "input": text,
            "format": "wav",
        });
        let encoded = serde_json::to_string(&body)
            .map_err(|error| ActuatorError::IoError(format!("encode tts request: {error}")))?;
        let output = Command::new("curl")
            .args([
                "-sS",
                "-L",
                "https://api.openai.com/v1/audio/speech",
                "-H",
                &format!("Authorization: Bearer {api_key}"),
                "-H",
                "Content-Type: application/json",
                "-d",
                &encoded,
                "-o",
            ])
            .arg(output_path)
            .output()
            .map_err(|error| ActuatorError::IoError(format!("curl openai tts: {error}")))?;
        if !output.status.success() {
            return Err(ActuatorError::IoError(format!(
                "openai tts failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }
        Ok(())
    }
}

impl Actuator for TtsActuator {
    fn name(&self) -> &str {
        "tts_actuator"
    }

    fn required_capabilities(&self) -> Vec<String> {
        vec!["tts.generate".into()]
    }

    fn execute(
        &self,
        action: &PlannedAction,
        context: &ActuatorContext,
    ) -> Result<ActionResult, ActuatorError> {
        let (text, output_path, provider, voice, model) = match action {
            PlannedAction::TextToSpeech {
                text,
                output_path,
                provider,
                voice,
                model,
            } => (
                text.as_str(),
                output_path.as_str(),
                provider.as_deref(),
                voice.as_deref(),
                model.as_deref(),
            ),
            _ => return Err(ActuatorError::ActionNotHandled),
        };

        if !has_capability(
            context.capabilities.iter().map(String::as_str),
            "tts.generate",
        ) {
            return Err(ActuatorError::CapabilityDenied("tts.generate".into()));
        }

        let safe_path = Self::resolve_output_path(context, output_path)?;
        let existed = safe_path.exists();
        match Self::select_provider(provider)?.as_str() {
            "piper" | "piper-tts" => Self::run_piper(text, voice, &safe_path)?,
            "openai" | "cloud" => Self::run_openai(text, voice, model, &safe_path)?,
            other => {
                return Err(ActuatorError::IoError(format!(
                    "unsupported tts provider '{other}'"
                )))
            }
        }

        let side_effect = if existed {
            SideEffect::FileModified {
                path: safe_path.clone(),
            }
        } else {
            SideEffect::FileCreated {
                path: safe_path.clone(),
            }
        };

        Ok(ActionResult {
            success: true,
            output: format!("audio written to {}", safe_path.display()),
            fuel_cost: FUEL_COST_TTS,
            side_effects: vec![side_effect],
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::autonomy::AutonomyLevel;
    use std::collections::HashSet;
    use tempfile::TempDir;

    fn make_context(tempdir: &TempDir) -> ActuatorContext {
        let mut capabilities = HashSet::new();
        capabilities.insert("tts.generate".to_string());
        ActuatorContext {
            agent_id: "agent".into(),
            agent_name: "agent".into(),
            working_dir: tempdir.path().to_path_buf(),
            autonomy_level: AutonomyLevel::L2,
            capabilities,
            fuel_remaining: 100.0,
            egress_allowlist: vec![],
            action_review_engine: None,
            hitl_approved: false,
        }
    }

    #[test]
    fn resolves_audio_output_inside_workspace() {
        let tempdir = TempDir::new().unwrap();
        let context = make_context(&tempdir);
        let resolved = TtsActuator::resolve_output_path(&context, "audio/test.wav").unwrap();
        assert!(resolved.starts_with(tempdir.path()));
    }

    #[test]
    fn select_provider_prefers_explicit_value() {
        // "openai" should always succeed as an explicit provider (doesn't require binary check)
        assert_eq!(
            TtsActuator::select_provider(Some("openai")).unwrap(),
            "openai"
        );
    }

    #[test]
    fn select_provider_rejects_missing_piper() {
        // If piper is not installed, requesting it explicitly should fail gracefully
        if !TtsActuator::is_piper_available() {
            let result = TtsActuator::select_provider(Some("piper"));
            assert!(result.is_err(), "should fail when piper is not installed");
        }
    }
}
