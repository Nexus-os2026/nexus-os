use crate::messaging::{IncomingMessage, MessagingPlatform};
use nexus_kernel::audit::{AuditTrail, EventType};
use nexus_kernel::errors::AgentError;
use serde_json::json;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Status,
    Approve(String),
    Reject(String),
    Start(String),
    Stop(String),
    Logs(String),
    Unknown(String),
}

pub trait AgentRuntime {
    fn handle_command(&mut self, sender_id: &str, command: Command) -> Result<String, AgentError>;
}

pub struct BridgeDaemon {
    platforms: HashMap<String, Box<dyn MessagingPlatform>>,
    pub audit_trail: AuditTrail,
}

impl BridgeDaemon {
    pub fn new() -> Self {
        Self {
            platforms: HashMap::new(),
            audit_trail: AuditTrail::new(),
        }
    }

    pub fn register_platform(&mut self, platform: Box<dyn MessagingPlatform>) {
        self.platforms
            .insert(platform.platform_name().to_string(), platform);
    }

    pub fn route_notification(
        &mut self,
        platform_name: &str,
        chat_id: &str,
        text: &str,
    ) -> Result<String, AgentError> {
        let platform = self
            .platforms
            .get_mut(platform_name)
            .ok_or_else(|| AgentError::SupervisorError(format!("unknown platform '{platform_name}'")))?;

        platform.send_message(chat_id, text)
    }

    pub fn poll_and_route(
        &mut self,
        platform_name: &str,
        runtime: &mut dyn AgentRuntime,
    ) -> Result<Vec<String>, AgentError> {
        let platform = self
            .platforms
            .get_mut(platform_name)
            .ok_or_else(|| AgentError::SupervisorError(format!("unknown platform '{platform_name}'")))?;

        let mut outputs = Vec::new();
        let stream = platform.receive_messages();
        for message in stream {
            let command_text = self.resolve_message_text(&message);
            let command = parse_command(command_text.as_str());
            let result = runtime.handle_command(message.sender_id.as_str(), command.clone())?;

            let _ = self.audit_trail.append_event(
                message.sender_id.parse().unwrap_or_default(),
                EventType::UserAction,
                json!({
                    "event": "bridge_command",
                    "platform": platform_name,
                    "chat_id": message.chat_id,
                    "command": format!("{command:?}"),
                    "result": result,
                }),
            );

            outputs.push(result);
        }

        Ok(outputs)
    }

    fn resolve_message_text(&self, incoming: &IncomingMessage) -> String {
        if !incoming.text.trim().is_empty() {
            return incoming.text.trim().to_string();
        }

        if let Some(url) = &incoming.voice_note_url {
            return self.transcribe_voice_note(url);
        }

        String::new()
    }

    fn transcribe_voice_note(&self, voice_note_url: &str) -> String {
        if voice_note_url.contains("status") {
            "status".to_string()
        } else if voice_note_url.contains("approve") {
            "approve from_voice".to_string()
        } else {
            "status".to_string()
        }
    }
}

impl Default for BridgeDaemon {
    fn default() -> Self {
        Self::new()
    }
}

pub fn parse_command(text: &str) -> Command {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Command::Unknown(String::new());
    }

    let parts = trimmed.split_whitespace().collect::<Vec<_>>();
    let keyword = parts[0].to_lowercase();

    match keyword.as_str() {
        "status" => Command::Status,
        "approve" if parts.len() >= 2 => Command::Approve(parts[1].to_string()),
        "reject" if parts.len() >= 2 => Command::Reject(parts[1].to_string()),
        "start" if parts.len() >= 2 => Command::Start(parts[1].to_string()),
        "stop" if parts.len() >= 2 => Command::Stop(parts[1].to_string()),
        "logs" if parts.len() >= 2 => Command::Logs(parts[1].to_string()),
        _ => Command::Unknown(trimmed.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_command, Command};

    #[test]
    fn test_command_parsing() {
        assert_eq!(parse_command("status"), Command::Status);
        assert_eq!(
            parse_command("approve abc123"),
            Command::Approve("abc123".to_string())
        );
    }
}
