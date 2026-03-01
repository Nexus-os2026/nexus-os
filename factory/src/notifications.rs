use crate::intent::ParsedIntent;
use nexus_connectors_messaging::messaging::{MessageId, MessagingPlatform, RichMessage};
use nexus_kernel::errors::AgentError;

pub fn send_creation_confirmation(
    platform: &mut dyn MessagingPlatform,
    chat_id: &str,
    request_id: &str,
    intent: &ParsedIntent,
) -> Result<MessageId, AgentError> {
    platform.send_message(
        chat_id,
        format!(
            "Factory request {request_id} received. task={:?}, platforms={:?}, schedule={}",
            intent.task_type, intent.platforms, intent.schedule
        )
        .as_str(),
    )
}

pub fn send_capability_review(
    platform: &mut dyn MessagingPlatform,
    chat_id: &str,
    request_id: &str,
    capabilities: &[String],
    fuel_budget: u64,
) -> Result<MessageId, AgentError> {
    let capability_list = if capabilities.is_empty() {
        "none".to_string()
    } else {
        capabilities.join(", ")
    };

    platform.send_rich_message(
        chat_id,
        RichMessage {
            text: format!(
                "Review request {request_id}: capabilities [{capability_list}], fuel_budget={fuel_budget}."
            ),
            buttons: vec![
                format!("approve {request_id}"),
                format!("reject {request_id}"),
            ],
            images: Vec::new(),
            attachments: Vec::new(),
        },
    )
}

pub fn send_step_up_challenge(
    platform: &mut dyn MessagingPlatform,
    chat_id: &str,
    request_id: &str,
    challenge_id: &str,
) -> Result<MessageId, AgentError> {
    platform.send_rich_message(
        chat_id,
        RichMessage {
            text: format!(
                "Step-up challenge required for request {request_id}. challenge_id={challenge_id}"
            ),
            buttons: vec![format!("reject {request_id}")],
            images: Vec::new(),
            attachments: Vec::new(),
        },
    )
}

pub fn send_deployment_success(
    platform: &mut dyn MessagingPlatform,
    chat_id: &str,
    request_id: &str,
    agent_id: &str,
    state: &str,
) -> Result<MessageId, AgentError> {
    platform.send_message(
        chat_id,
        format!("Request {request_id} deployed. agent_id={agent_id}, status={state}").as_str(),
    )
}
