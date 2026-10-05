//! The production tool catalog.
//!
//! Two local tools, both typed and bounded: a SHA-256 digest of text (R0,
//! local computation) and speech synthesis with the system `espeak-ng`
//! (R1: the audio is meant to be played aloud). Neither takes a command
//! line, a path or a program name from its caller. There is deliberately no
//! shell, interpreter, container or code runner here: ShellCommand,
//! DockerCommand and CodeExecute stay closed.

use super::{ToolDefinition, ToolInvocation, ToolOutput};
use crate::authority::effect::EffectClass;
use crate::authority::AuthorityError;
use crate::connector::{only_fields, text_field};
use crate::executable::Trust;
use serde_json::Value;
use std::time::Duration;

/// Every production tool.
pub fn production() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            key: "text.sha256",
            executable: "/usr/bin/sha256sum",
            class: EffectClass::R0,
            build: sha256,
            env: &[("LC_ALL", "C")],
            timeout: Duration::from_secs(10),
            max_output: 4 * 1024,
            output: ToolOutput::Text,
            trust: Trust::System,
        },
        ToolDefinition {
            key: "speech.synthesize",
            executable: "/usr/bin/espeak-ng",
            class: EffectClass::R1,
            build: speech,
            env: &[("LC_ALL", "C.UTF-8")],
            timeout: Duration::from_secs(30),
            max_output: 16 * 1024 * 1024,
            output: ToolOutput::Bytes,
            trust: Trust::System,
        },
    ]
}

fn sha256(input: &Value) -> Result<ToolInvocation, AuthorityError> {
    only_fields(input, &["text"])?;
    let text = text_field(input, "text", 1024 * 1024)?;
    Ok(ToolInvocation {
        args: vec!["--".into(), "input.txt".into()],
        files: vec![("input.txt", text.as_bytes().to_vec())],
        summary: vec![format!(
            "Compute the SHA-256 digest of {} characters of text",
            text.chars().count()
        )],
    })
}

/// Voices the speech tool may use.
pub const VOICES: [&str; 10] = [
    "en", "en-us", "en-gb", "de", "fr", "es", "it", "pt", "nl", "hi",
];

fn speech(input: &Value) -> Result<ToolInvocation, AuthorityError> {
    only_fields(input, &["text", "voice"])?;
    let text = text_field(input, "text", 2000)?;
    let voice = match input.get("voice") {
        None | Some(Value::Null) => "en",
        Some(Value::String(voice)) => VOICES
            .iter()
            .find(|v| **v == voice.as_str())
            .copied()
            .ok_or(AuthorityError::InvalidAction("unknown voice"))?,
        Some(_) => return Err(AuthorityError::InvalidAction("a voice is text")),
    };
    Ok(ToolInvocation {
        args: vec![
            "--stdout".into(),
            "-v".into(),
            voice.into(),
            "-f".into(),
            "input.txt".into(),
        ],
        files: vec![("input.txt", text.as_bytes().to_vec())],
        summary: vec![
            format!("Speak {} characters in voice {voice}", text.chars().count()),
            format!(
                "Text begins: {}",
                text.chars().take(120).collect::<String>()
            ),
        ],
    })
}
