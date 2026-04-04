use crate::approval::{ApprovalFlow, ApprovalRequest, DeploymentResult};
use crate::capabilities::map_intent_to_capabilities;
use crate::code_gen::{generate_agent_code, GeneratedAgentCode};
use crate::intent::{IntentParser, ParsedIntent};
use crate::manifest_gen::generate_manifest_toml;
use crate::notifications;
use nexus_connectors_llm::providers::LlmProvider;
use nexus_connectors_messaging::auth::{
    AuthError, AuthManager, DeviceToken, Operation, StepUpAuthResult, StepUpChallenge,
};
use nexus_connectors_messaging::messaging::{IncomingMessage, MessagingPlatform};
use nexus_kernel::errors::AgentError;
use std::collections::HashMap;
use uuid::Uuid;

pub trait VoiceTranscriber {
    fn transcribe_voice_note(&self, voice_note_url: &str) -> Result<String, AgentError>;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct BridgeVoiceTranscriber;

impl VoiceTranscriber for BridgeVoiceTranscriber {
    fn transcribe_voice_note(&self, voice_note_url: &str) -> Result<String, AgentError> {
        let transcript = voice_note_url
            .rsplit('/')
            .next()
            .unwrap_or_default()
            .replace(['-', '_'], " ")
            .trim()
            .to_string();

        if transcript.is_empty() {
            return Err(AgentError::SupervisorError(
                "voice note transcription produced empty text".to_string(),
            ));
        }

        Ok(transcript)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RemoteFlowStatus {
    AwaitingApproval,
    AwaitingStepUpChallenge,
    Rejected,
    Deployed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteFlowResult {
    pub request_id: String,
    pub status: RemoteFlowStatus,
    pub parsed_intent: Option<ParsedIntent>,
    pub challenge_id: Option<String>,
    pub deployment: Option<DeploymentResult>,
}

#[derive(Debug, Clone)]
struct PendingCreation {
    chat_id: String,
    parsed_intent: ParsedIntent,
    approval_request: ApprovalRequest,
    manifest_toml: String,
    generated_code: GeneratedAgentCode,
}

pub struct RemoteFactoryInterface<P: LlmProvider, V: VoiceTranscriber> {
    parser: IntentParser<P>,
    approval_flow: ApprovalFlow,
    transcriber: V,
    pending: HashMap<String, PendingCreation>,
    pending_step_up: HashMap<String, StepUpChallenge>,
}

impl<P: LlmProvider, V: VoiceTranscriber> RemoteFactoryInterface<P, V> {
    pub fn new(provider: P, model_name: &str, llm_fuel_budget: u64, transcriber: V) -> Self {
        Self {
            parser: IntentParser::new(provider, model_name, llm_fuel_budget),
            approval_flow: ApprovalFlow::new(),
            transcriber,
            pending: HashMap::new(),
            pending_step_up: HashMap::new(),
        }
    }

    pub fn handle_incoming_message(
        &mut self,
        platform: &mut dyn MessagingPlatform,
        auth_manager: &mut AuthManager,
        token: &DeviceToken,
        incoming: IncomingMessage,
    ) -> Result<RemoteFlowResult, AgentError> {
        let message_text = self.resolve_message_text(&incoming)?;
        let trimmed = message_text.trim();

        if let Some(request_id) = parse_simple_command(trimmed, "approve") {
            return self.handle_approve(platform, auth_manager, token, request_id.as_str());
        }

        if let Some(request_id) = parse_simple_command(trimmed, "reject") {
            return self.handle_reject(platform, request_id.as_str());
        }

        if let Some((request_id, signature)) = parse_challenge_command(trimmed) {
            return self.handle_challenge(
                platform,
                auth_manager,
                token,
                request_id.as_str(),
                signature.as_str(),
            );
        }

        self.handle_creation(platform, incoming.chat_id.as_str(), trimmed)
    }

    pub fn active_challenge(&self, request_id: &str) -> Option<&StepUpChallenge> {
        self.pending_step_up.get(request_id)
    }

    fn handle_creation(
        &mut self,
        platform: &mut dyn MessagingPlatform,
        chat_id: &str,
        request_text: &str,
    ) -> Result<RemoteFlowResult, AgentError> {
        let parsed_intent = self.parser.parse(request_text)?;
        let capabilities = map_intent_to_capabilities(&parsed_intent);
        let generated_manifest = generate_manifest_toml(&parsed_intent, &capabilities);
        let generated_code = generate_agent_code(&parsed_intent);
        let approval = self.approval_flow.present_for_review(
            capabilities.required.clone(),
            generated_manifest.fuel_budget,
        );
        let request_id = Uuid::new_v4().to_string();

        self.pending.insert(
            request_id.clone(),
            PendingCreation {
                chat_id: chat_id.to_string(),
                parsed_intent: parsed_intent.clone(),
                approval_request: approval,
                manifest_toml: generated_manifest.toml,
                generated_code,
            },
        );

        let _ = notifications::send_creation_confirmation(
            platform,
            chat_id,
            request_id.as_str(),
            &parsed_intent,
        )?;
        let _ = notifications::send_capability_review(
            platform,
            chat_id,
            request_id.as_str(),
            capabilities.required.as_slice(),
            generated_manifest.fuel_budget,
        )?;

        Ok(RemoteFlowResult {
            request_id,
            status: RemoteFlowStatus::AwaitingApproval,
            parsed_intent: Some(parsed_intent),
            challenge_id: None,
            deployment: None,
        })
    }

    fn handle_approve(
        &mut self,
        platform: &mut dyn MessagingPlatform,
        auth_manager: &mut AuthManager,
        token: &DeviceToken,
        request_id: &str,
    ) -> Result<RemoteFlowResult, AgentError> {
        let pending = self.pending.get(request_id).ok_or_else(|| {
            AgentError::SupervisorError(format!("unknown remote request '{request_id}'"))
        })?;

        match auth_manager
            .step_up_auth(token, Operation::CreateAgent)
            .map_err(map_auth_error)?
        {
            StepUpAuthResult::Allowed => {
                self.deploy_request(platform, request_id, pending.parsed_intent.clone())
            }
            StepUpAuthResult::RequiresChallenge(challenge) => {
                let challenge_id = challenge.challenge_id.clone();
                self.pending_step_up
                    .insert(request_id.to_string(), challenge.clone());
                let _ = notifications::send_step_up_challenge(
                    platform,
                    pending.chat_id.as_str(),
                    request_id,
                    challenge_id.as_str(),
                )?;

                Ok(RemoteFlowResult {
                    request_id: request_id.to_string(),
                    status: RemoteFlowStatus::AwaitingStepUpChallenge,
                    parsed_intent: Some(pending.parsed_intent.clone()),
                    challenge_id: Some(challenge_id),
                    deployment: None,
                })
            }
        }
    }

    fn handle_challenge(
        &mut self,
        platform: &mut dyn MessagingPlatform,
        auth_manager: &mut AuthManager,
        token: &DeviceToken,
        request_id: &str,
        signature: &str,
    ) -> Result<RemoteFlowResult, AgentError> {
        let challenge = self
            .pending_step_up
            .get(request_id)
            .cloned()
            .ok_or_else(|| {
                AgentError::SupervisorError(format!(
                    "no active challenge found for request '{request_id}'"
                ))
            })?;

        let _ = auth_manager
            .verify_step_up_challenge(token, challenge.challenge_id.as_str(), signature)
            .map_err(map_auth_error)?;

        self.pending_step_up.remove(request_id);
        let parsed = self
            .pending
            .get(request_id)
            .map(|pending| pending.parsed_intent.clone())
            .ok_or_else(|| {
                AgentError::SupervisorError(format!("unknown remote request '{request_id}'"))
            })?;
        self.deploy_request(platform, request_id, parsed)
    }

    fn handle_reject(
        &mut self,
        platform: &mut dyn MessagingPlatform,
        request_id: &str,
    ) -> Result<RemoteFlowResult, AgentError> {
        let pending = self.pending.remove(request_id).ok_or_else(|| {
            AgentError::SupervisorError(format!("unknown remote request '{request_id}'"))
        })?;

        let denied = self.approval_flow.approve_and_deploy(
            &pending.approval_request,
            pending.manifest_toml.as_str(),
            &pending.generated_code,
            false,
        )?;

        let _ = platform.send_message(
            pending.chat_id.as_str(),
            format!("Request {request_id} rejected.").as_str(),
        )?;

        Ok(RemoteFlowResult {
            request_id: request_id.to_string(),
            status: RemoteFlowStatus::Rejected,
            parsed_intent: Some(pending.parsed_intent),
            challenge_id: None,
            deployment: Some(denied),
        })
    }

    fn deploy_request(
        &mut self,
        platform: &mut dyn MessagingPlatform,
        request_id: &str,
        parsed_intent: ParsedIntent,
    ) -> Result<RemoteFlowResult, AgentError> {
        let pending = self.pending.remove(request_id).ok_or_else(|| {
            AgentError::SupervisorError(format!("unknown remote request '{request_id}'"))
        })?;
        let deployment = self.approval_flow.approve_and_deploy(
            &pending.approval_request,
            pending.manifest_toml.as_str(),
            &pending.generated_code,
            true,
        )?;

        if let (Some(agent_id), Some(state)) = (deployment.agent_id.as_ref(), deployment.state) {
            let _ = notifications::send_deployment_success(
                platform,
                pending.chat_id.as_str(),
                request_id,
                agent_id.as_str(),
                format!("{state}").as_str(),
            )?;
        }

        Ok(RemoteFlowResult {
            request_id: request_id.to_string(),
            status: RemoteFlowStatus::Deployed,
            parsed_intent: Some(parsed_intent),
            challenge_id: None,
            deployment: Some(deployment),
        })
    }

    fn resolve_message_text(&self, incoming: &IncomingMessage) -> Result<String, AgentError> {
        let trimmed = incoming.text.trim();
        if !trimmed.is_empty() {
            return Ok(trimmed.to_string());
        }

        if let Some(voice_note_url) = incoming.voice_note_url.as_ref() {
            return self
                .transcriber
                .transcribe_voice_note(voice_note_url.as_str());
        }

        Err(AgentError::SupervisorError(
            "incoming message contained no text or voice note".to_string(),
        ))
    }
}

fn map_auth_error(error: AuthError) -> AgentError {
    AgentError::SupervisorError(format!("step-up auth error: {error:?}"))
}

fn parse_simple_command(text: &str, keyword: &str) -> Option<String> {
    let mut parts = text.split_whitespace();
    if parts.next()? != keyword {
        return None;
    }
    parts.next().map(|value| value.to_string())
}

fn parse_challenge_command(text: &str) -> Option<(String, String)> {
    let mut parts = text.split_whitespace();
    if parts.next()? != "challenge" {
        return None;
    }
    let request_id = parts.next()?.to_string();
    let signature = parts.next()?.to_string();
    Some((request_id, signature))
}

#[cfg(test)]
mod tests {
    use super::{RemoteFactoryInterface, RemoteFlowStatus, VoiceTranscriber};
    use crate::intent::TaskType;
    use nexus_connectors_llm::providers::{LlmProvider, LlmResponse};
    use nexus_connectors_messaging::auth::{
        AuthManager, DeviceToken, Operation, PairingResponse, StepUpAuthResult,
    };
    use nexus_connectors_messaging::messaging::{
        IncomingMessage, IncomingMessageStream, MessageId, MessagingPlatform, RateLimitConfig,
        RichMessage,
    };
    use nexus_kernel::errors::AgentError;

    struct MockCreationProvider;

    impl LlmProvider for MockCreationProvider {
        fn query(
            &self,
            _prompt: &str,
            max_tokens: u32,
            model: &str,
        ) -> Result<LlmResponse, AgentError> {
            Ok(LlmResponse {
                output_text: r#"{
                    "task_type": "ContentPosting",
                    "platforms": ["twitter"],
                    "schedule": "daily",
                    "content_topic": "rust"
                }"#
                .to_string(),
                token_count: max_tokens.min(42),
                model_name: model.to_string(),
                tool_calls: Vec::new(),
                input_tokens: None,
            })
        }

        fn name(&self) -> &str {
            "mock-creation"
        }

        fn cost_per_token(&self) -> f64 {
            0.0
        }
    }

    struct FallbackProvider;

    impl LlmProvider for FallbackProvider {
        fn query(
            &self,
            _prompt: &str,
            max_tokens: u32,
            model: &str,
        ) -> Result<LlmResponse, AgentError> {
            Ok(LlmResponse {
                output_text: "not-json".to_string(),
                token_count: max_tokens.min(8),
                model_name: model.to_string(),
                tool_calls: Vec::new(),
                input_tokens: None,
            })
        }

        fn name(&self) -> &str {
            "mock-fallback"
        }

        fn cost_per_token(&self) -> f64 {
            0.0
        }
    }

    #[derive(Default)]
    struct MockMessagingPlatform {
        sent_text: Vec<String>,
        sent_rich: Vec<RichMessage>,
    }

    impl MessagingPlatform for MockMessagingPlatform {
        fn send_message(&mut self, _chat_id: &str, text: &str) -> Result<MessageId, AgentError> {
            self.sent_text.push(text.to_string());
            Ok(format!("msg-{}", self.sent_text.len()))
        }

        fn send_rich_message(
            &mut self,
            _chat_id: &str,
            message: RichMessage,
        ) -> Result<MessageId, AgentError> {
            self.sent_rich.push(message);
            Ok(format!("rich-{}", self.sent_rich.len()))
        }

        fn receive_messages(&mut self) -> IncomingMessageStream {
            IncomingMessageStream::empty()
        }

        fn platform_name(&self) -> &str {
            "telegram"
        }

        fn rate_limit(&self) -> RateLimitConfig {
            RateLimitConfig {
                max_messages: 10,
                window_seconds: 1,
                quality_tier: Some("test".to_string()),
            }
        }
    }

    #[derive(Default, Clone)]
    struct StubVoiceTranscriber {
        transcript: String,
    }

    impl VoiceTranscriber for StubVoiceTranscriber {
        fn transcribe_voice_note(&self, _voice_note_url: &str) -> Result<String, AgentError> {
            Ok(self.transcript.clone())
        }
    }

    fn paired_token(auth: &mut AuthManager, user_id: &str) -> DeviceToken {
        let qr = auth.generate_pairing_qr(user_id);
        let pairing = PairingResponse {
            user_id: qr.user_id.clone(),
            device_id: qr.device_id.clone(),
            challenge_response: qr.one_time_challenge.clone(),
        };
        auth.verify_pairing(pairing)
            .expect("pairing should return a device token")
    }

    fn strong_token_for_creation(auth: &mut AuthManager, token: &DeviceToken) -> DeviceToken {
        let challenge = match auth
            .step_up_auth(token, Operation::CreateAgent)
            .expect("create agent step-up should resolve")
        {
            StepUpAuthResult::RequiresChallenge(challenge) => challenge,
            StepUpAuthResult::Allowed => panic!("expected challenge before strong auth"),
        };

        let signature = auth.expected_step_up_response(&challenge, token.device_id.as_str());
        auth.verify_step_up_challenge(token, challenge.challenge_id.as_str(), signature.as_str())
            .expect("challenge verification should upgrade token")
    }

    #[test]
    fn test_remote_creation_flow() {
        let mut remote = RemoteFactoryInterface::new(
            MockCreationProvider,
            "mock-model",
            5_000,
            StubVoiceTranscriber::default(),
        );
        let mut platform = MockMessagingPlatform::default();
        let mut auth = AuthManager::new("remote-factory-secret");
        let basic = paired_token(&mut auth, "user-telegram");
        let strong = strong_token_for_creation(&mut auth, &basic);

        let create_result = remote
            .handle_incoming_message(
                &mut platform,
                &mut auth,
                &strong,
                IncomingMessage {
                    chat_id: "chat-1".to_string(),
                    sender_id: "user-telegram".to_string(),
                    text: "Create an agent that posts about Rust on Twitter every morning at 9am"
                        .to_string(),
                    sanitized_text: None,
                    voice_note_url: None,
                    timestamp: 1,
                },
            )
            .expect("creation request should parse");

        assert_eq!(create_result.status, RemoteFlowStatus::AwaitingApproval);
        assert!(!platform.sent_rich.is_empty());
        let review = platform
            .sent_rich
            .last()
            .expect("review message should be sent");
        assert!(review
            .buttons
            .iter()
            .any(|button| button.starts_with("approve ")));
        assert!(review
            .buttons
            .iter()
            .any(|button| button.starts_with("reject ")));

        let approve_result = remote
            .handle_incoming_message(
                &mut platform,
                &mut auth,
                &strong,
                IncomingMessage {
                    chat_id: "chat-1".to_string(),
                    sender_id: "user-telegram".to_string(),
                    text: format!("approve {}", create_result.request_id),
                    sanitized_text: None,
                    voice_note_url: None,
                    timestamp: 2,
                },
            )
            .expect("approval should deploy");

        assert_eq!(approve_result.status, RemoteFlowStatus::Deployed);
        assert!(approve_result
            .deployment
            .as_ref()
            .map(|deployment| deployment.deployed)
            .unwrap_or(false));
    }

    #[test]
    fn test_voice_note_to_agent() {
        let mut remote = RemoteFactoryInterface::new(
            FallbackProvider,
            "mock-model",
            5_000,
            StubVoiceTranscriber {
                transcript: "Back up my photos every night".to_string(),
            },
        );
        let mut platform = MockMessagingPlatform::default();
        let mut auth = AuthManager::new("voice-factory-secret");
        let basic = paired_token(&mut auth, "user-voice");
        let strong = strong_token_for_creation(&mut auth, &basic);

        let result = remote
            .handle_incoming_message(
                &mut platform,
                &mut auth,
                &strong,
                IncomingMessage {
                    chat_id: "chat-voice".to_string(),
                    sender_id: "user-voice".to_string(),
                    text: String::new(),
                    sanitized_text: None,
                    voice_note_url: Some("voice://telegram/voice-note-1".to_string()),
                    timestamp: 10,
                },
            )
            .expect("voice note should be transcribed and parsed");

        let parsed = result
            .parsed_intent
            .expect("parsed intent should be available");
        assert_eq!(parsed.task_type, TaskType::FileBackup);
        assert_eq!(parsed.schedule, "0 0 * * *");
    }

    #[test]
    fn test_step_up_auth_for_creation() {
        let mut remote = RemoteFactoryInterface::new(
            MockCreationProvider,
            "mock-model",
            5_000,
            StubVoiceTranscriber::default(),
        );
        let mut platform = MockMessagingPlatform::default();
        let mut auth = AuthManager::new("step-up-secret");
        let basic = paired_token(&mut auth, "user-stepup");

        let create_result = remote
            .handle_incoming_message(
                &mut platform,
                &mut auth,
                &basic,
                IncomingMessage {
                    chat_id: "chat-stepup".to_string(),
                    sender_id: "user-stepup".to_string(),
                    text: "Create an agent that posts about AI on Twitter daily".to_string(),
                    sanitized_text: None,
                    voice_note_url: None,
                    timestamp: 1,
                },
            )
            .expect("creation request should parse");

        let approve_result = remote
            .handle_incoming_message(
                &mut platform,
                &mut auth,
                &basic,
                IncomingMessage {
                    chat_id: "chat-stepup".to_string(),
                    sender_id: "user-stepup".to_string(),
                    text: format!("approve {}", create_result.request_id),
                    sanitized_text: None,
                    voice_note_url: None,
                    timestamp: 2,
                },
            )
            .expect("approve should request step-up challenge");

        assert_eq!(
            approve_result.status,
            RemoteFlowStatus::AwaitingStepUpChallenge
        );
        assert!(approve_result.challenge_id.is_some());
        assert!(approve_result.deployment.is_none());
    }
}
