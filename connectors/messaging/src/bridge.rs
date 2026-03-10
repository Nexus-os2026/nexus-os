use crate::messaging::{IncomingMessage, MessagingPlatform, RichMessage};
use nexus_kernel::audit::{AuditTrail, EventType};
use nexus_kernel::errors::AgentError;
use rand::{thread_rng, Rng};
use serde_json::json;
use std::collections::HashMap;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use uuid::Uuid;

const PAIRING_TTL_SECONDS: u64 = 300;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Pair,
    Status,
    Approve(String),
    Reject(String),
    Start(String),
    Stop(String),
    Logs(String),
    Unknown(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentSummary {
    pub name: String,
    pub status: String,
    pub fuel_remaining: u64,
}

pub trait AgentRuntime {
    fn list_agents(&mut self) -> Result<Vec<AgentSummary>, AgentError>;
    fn start_agent(&mut self, name: &str) -> Result<(), AgentError>;
    fn stop_agent(&mut self, name: &str) -> Result<(), AgentError>;
    fn approve(&mut self, approval_id: &str) -> Result<(), AgentError>;
    fn logs(&mut self, name: &str) -> Result<Vec<String>, AgentError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PairingCode {
    pub chat_id: String,
    pub code: String,
    pub expires_at: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PairedDevice {
    pub chat_id: String,
    pub device_token: String,
    pub paired_at: u64,
}

pub struct BridgeDaemon {
    platforms: HashMap<String, Box<dyn MessagingPlatform>>,
    pending_pairings: HashMap<String, PairingCode>,
    paired_devices: HashMap<String, PairedDevice>,
    pub audit_trail: AuditTrail,
    clock: Box<dyn Fn() -> u64 + Send + Sync>,
}

impl BridgeDaemon {
    pub fn new() -> Self {
        Self {
            platforms: HashMap::new(),
            pending_pairings: HashMap::new(),
            paired_devices: HashMap::new(),
            audit_trail: AuditTrail::new(),
            clock: Box::new(current_unix_timestamp),
        }
    }

    pub fn with_clock(clock: Box<dyn Fn() -> u64 + Send + Sync>) -> Self {
        Self {
            platforms: HashMap::new(),
            pending_pairings: HashMap::new(),
            paired_devices: HashMap::new(),
            audit_trail: AuditTrail::new(),
            clock,
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
        let platform = self.platforms.get_mut(platform_name).ok_or_else(|| {
            AgentError::SupervisorError(format!("unknown platform '{platform_name}'"))
        })?;
        platform.send_message(chat_id, text)
    }

    pub fn notify_agent_completed(
        &mut self,
        platform_name: &str,
        chat_id: &str,
        summary: &str,
    ) -> Result<String, AgentError> {
        self.route_notification(
            platform_name,
            chat_id,
            format!("Task complete: {summary}").as_str(),
        )
    }

    pub fn notify_approval_required(
        &mut self,
        platform_name: &str,
        chat_id: &str,
        approval_id: &str,
        summary: &str,
    ) -> Result<String, AgentError> {
        let platform = self.platforms.get_mut(platform_name).ok_or_else(|| {
            AgentError::SupervisorError(format!("unknown platform '{platform_name}'"))
        })?;

        let rich = RichMessage {
            text: format!("Approval needed ({approval_id}): {summary}"),
            buttons: vec![
                format!("approve {approval_id}"),
                format!("reject {approval_id}"),
            ],
            images: Vec::new(),
            attachments: Vec::new(),
        };
        platform.send_rich_message(chat_id, rich)
    }

    pub fn generate_pairing_code(&mut self, chat_id: &str) -> PairingCode {
        let now = (self.clock)();
        let code = format!("{:06}", thread_rng().gen_range(0..1_000_000_u32));
        let pairing = PairingCode {
            chat_id: chat_id.to_string(),
            code: code.clone(),
            expires_at: now + PAIRING_TTL_SECONDS,
        };
        self.pending_pairings
            .insert(chat_id.to_string(), pairing.clone());
        pairing
    }

    pub fn confirm_pairing(
        &mut self,
        chat_id: &str,
        code: &str,
    ) -> Result<PairedDevice, AgentError> {
        let now = (self.clock)();
        let pending = self
            .pending_pairings
            .get(chat_id)
            .cloned()
            .ok_or_else(|| AgentError::SupervisorError("pairing code not found".to_string()))?;

        if now > pending.expires_at {
            return Err(AgentError::SupervisorError(
                "pairing code expired".to_string(),
            ));
        }
        if pending.code != code {
            return Err(AgentError::SupervisorError(
                "pairing code mismatch".to_string(),
            ));
        }

        self.pending_pairings.remove(chat_id);
        let device = PairedDevice {
            chat_id: chat_id.to_string(),
            device_token: Uuid::new_v4().to_string(),
            paired_at: now,
        };
        self.paired_devices
            .insert(chat_id.to_string(), device.clone());
        Ok(device)
    }

    pub fn is_paired(&self, chat_id: &str) -> bool {
        self.paired_devices.contains_key(chat_id)
    }

    pub fn poll_and_route(
        &mut self,
        platform_name: &str,
        runtime: &mut dyn AgentRuntime,
    ) -> Result<Vec<String>, AgentError> {
        let incoming = {
            let platform = self.platforms.get_mut(platform_name).ok_or_else(|| {
                AgentError::SupervisorError(format!("unknown platform '{platform_name}'"))
            })?;
            platform.receive_messages().collect::<Vec<_>>()
        };
        let mut outputs = Vec::new();
        for message in incoming {
            let response = self.handle_incoming(runtime, &message)?;
            if let Some(platform) = self.platforms.get_mut(platform_name) {
                let _ = platform.send_message(message.chat_id.as_str(), response.as_str());
            }
            outputs.push(response);
        }
        Ok(outputs)
    }

    pub fn run_polling_loop(
        &mut self,
        platform_name: &str,
        runtime: &mut dyn AgentRuntime,
        interval: Duration,
        max_iterations: Option<usize>,
    ) -> Result<(), AgentError> {
        let mut iterations = 0_usize;
        loop {
            self.poll_and_route(platform_name, runtime)?;
            iterations = iterations.saturating_add(1);
            if let Some(max) = max_iterations {
                if iterations >= max {
                    return Ok(());
                }
            }
            thread::sleep(interval);
        }
    }

    fn handle_incoming(
        &mut self,
        runtime: &mut dyn AgentRuntime,
        message: &IncomingMessage,
    ) -> Result<String, AgentError> {
        let command_text = self.resolve_message_text(message);
        let command = parse_command(command_text.as_str());

        let response = match command {
            Command::Pair => {
                let pairing = self.generate_pairing_code(message.chat_id.as_str());
                format!(
                    "Pairing code: {} (expires in 5 minutes). Enter this in Desktop App or CLI.",
                    pairing.code
                )
            }
            _ if !self.is_paired(message.chat_id.as_str()) => {
                "Please pair first: /pair".to_string()
            }
            Command::Status => {
                let agents = runtime.list_agents()?;
                if agents.is_empty() {
                    "No agents found.".to_string()
                } else {
                    let lines = agents
                        .into_iter()
                        .map(|agent| {
                            format!(
                                "{} | {} | fuel {}",
                                agent.name, agent.status, agent.fuel_remaining
                            )
                        })
                        .collect::<Vec<_>>();
                    format!("Agents:\n{}", lines.join("\n"))
                }
            }
            Command::Start(name) => {
                runtime.start_agent(name.as_str())?;
                format!("Started agent '{name}'")
            }
            Command::Stop(name) => {
                runtime.stop_agent(name.as_str())?;
                format!("Stopped agent '{name}'")
            }
            Command::Approve(id) => {
                runtime.approve(id.as_str())?;
                format!("Approved '{id}'")
            }
            Command::Reject(id) => format!("Rejected '{id}'"),
            Command::Logs(name) => {
                let logs = runtime.logs(name.as_str())?;
                if logs.is_empty() {
                    format!("No logs for '{name}'")
                } else {
                    format!("Logs for '{name}':\n{}", logs.join("\n"))
                }
            }
            Command::Unknown(_) => {
                "Unknown command. Try: status, start <name>, stop <name>, approve <id>, logs <name>"
                    .to_string()
            }
        };

        self.audit_trail.append_event(
            message.sender_id.parse().unwrap_or_default(),
            EventType::UserAction,
            json!({
                "event": "bridge_command",
                "chat_id": message.chat_id,
                "sender_id": message.sender_id,
                "command_text": command_text,
                "response": response,
            }),
        )?;

        Ok(response)
    }

    fn resolve_message_text(&self, incoming: &IncomingMessage) -> String {
        if !incoming.text.trim().is_empty() {
            return incoming.text.trim().to_string();
        }

        if let Some(path) = &incoming.voice_note_url {
            if path.contains("approve") {
                return "approve voice".to_string();
            }
            if path.contains("start") {
                return "start voice".to_string();
            }
            return "status".to_string();
        }

        String::new()
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
        "/pair" | "pair" => Command::Pair,
        "status" => Command::Status,
        "approve" if parts.len() >= 2 => Command::Approve(parts[1].to_string()),
        "reject" if parts.len() >= 2 => Command::Reject(parts[1].to_string()),
        "start" if parts.len() >= 2 => Command::Start(parts[1].to_string()),
        "stop" if parts.len() >= 2 => Command::Stop(parts[1].to_string()),
        "logs" if parts.len() >= 2 => Command::Logs(parts[1].to_string()),
        _ => Command::Unknown(trimmed.to_string()),
    }
}

fn current_unix_timestamp() -> u64 {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => duration.as_secs(),
        Err(_) => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_command, AgentRuntime, AgentSummary, BridgeDaemon, Command};
    use crate::telegram::TelegramAdapter;
    use nexus_kernel::errors::AgentError;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::Arc;

    #[derive(Default)]
    struct MockRuntime {
        started: Vec<String>,
        stopped: Vec<String>,
        approved: Vec<String>,
    }

    impl AgentRuntime for MockRuntime {
        fn list_agents(&mut self) -> Result<Vec<AgentSummary>, AgentError> {
            Ok(vec![AgentSummary {
                name: "writer".to_string(),
                status: "Running".to_string(),
                fuel_remaining: 120,
            }])
        }

        fn start_agent(&mut self, name: &str) -> Result<(), AgentError> {
            self.started.push(name.to_string());
            Ok(())
        }

        fn stop_agent(&mut self, name: &str) -> Result<(), AgentError> {
            self.stopped.push(name.to_string());
            Ok(())
        }

        fn approve(&mut self, approval_id: &str) -> Result<(), AgentError> {
            self.approved.push(approval_id.to_string());
            Ok(())
        }

        fn logs(&mut self, name: &str) -> Result<Vec<String>, AgentError> {
            Ok(vec![format!("{name}: completed job")])
        }
    }

    #[test]
    fn test_command_parsing() {
        assert_eq!(parse_command("status"), Command::Status);
        assert_eq!(
            parse_command("approve abc123"),
            Command::Approve("abc123".to_string())
        );
        assert_eq!(parse_command("/pair"), Command::Pair);
    }

    #[test]
    fn test_command_routing() {
        let now = Arc::new(AtomicU64::new(1_000));
        let mut bridge = BridgeDaemon::with_clock({
            let now = Arc::clone(&now);
            Box::new(move || now.load(Ordering::SeqCst))
        });
        let mut telegram = TelegramAdapter::new();
        telegram.push_incoming(crate::messaging::IncomingMessage {
            chat_id: "chat-1".to_string(),
            sender_id: "1".to_string(),
            text: "status".to_string(),
            sanitized_text: None,
            voice_note_url: None,
            timestamp: 1_000,
        });
        bridge.register_platform(Box::new(telegram));

        let pairing = bridge.generate_pairing_code("chat-1");
        let confirmed = bridge.confirm_pairing("chat-1", pairing.code.as_str());
        assert!(confirmed.is_ok());

        let mut runtime = MockRuntime::default();
        let result = bridge.poll_and_route("telegram", &mut runtime);
        assert!(result.is_ok());
        if let Ok(outputs) = result {
            assert_eq!(outputs.len(), 1);
            assert!(outputs[0].contains("writer | Running"));
        }
    }

    #[test]
    fn test_pairing_code_generation() {
        let now = Arc::new(AtomicU64::new(1_500));
        let mut bridge = BridgeDaemon::with_clock({
            let now = Arc::clone(&now);
            Box::new(move || now.load(Ordering::SeqCst))
        });

        let pairing = bridge.generate_pairing_code("chat-42");
        assert_eq!(pairing.code.len(), 6);
        assert!(pairing.code.chars().all(|ch| ch.is_ascii_digit()));
        assert_eq!(pairing.expires_at, 1_800);
    }

    #[test]
    fn test_unpaired_device_blocked() {
        let mut bridge = BridgeDaemon::new();
        let mut telegram = TelegramAdapter::new();
        telegram.push_incoming(crate::messaging::IncomingMessage {
            chat_id: "chat-unpaired".to_string(),
            sender_id: "2".to_string(),
            text: "status".to_string(),
            sanitized_text: None,
            voice_note_url: None,
            timestamp: 1_000,
        });
        bridge.register_platform(Box::new(telegram));

        let mut runtime = MockRuntime::default();
        let result = bridge.poll_and_route("telegram", &mut runtime);
        assert!(result.is_ok());
        if let Ok(outputs) = result {
            assert_eq!(outputs.len(), 1);
            assert_eq!(outputs[0], "Please pair first: /pair");
        }
    }
}
