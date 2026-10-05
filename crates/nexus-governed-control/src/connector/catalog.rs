//! The production connector catalog: the Phase Zero S6 email and messaging
//! endpoints, migrated to typed operations.
//!
//! Reads are R1; every send is R2 and is approved natively with its exact
//! recipient, subject and body digest. Credentials come only from the vault
//! (scope `http`, the verified HTTP-connector scope) through broker leases,
//! and always travel in a header. Telegram is not migrated: its Bot API
//! carries the token in the URL path, which no Phase Three request may do,
//! so it stays closed.

use super::{only_fields, text_field, Connector, ConnectorOperation, OperationRequest};
use crate::authority::effect::EffectClass;
use crate::authority::AuthorityError;
use crate::broker::{CredentialSpec, Placement};
use crate::egress::transport::Method;
use base64::Engine;
use serde_json::{json, Value};

/// The vault scope every connector credential is stored under.
pub const CONNECTOR_SCOPE: &str = "http";

const GMAIL: CredentialSpec = CredentialSpec {
    service: "Gmail",
    scope: CONNECTOR_SCOPE,
    name: "gmail.access_token",
    placement: Placement::Authorization("Bearer"),
};
const OUTLOOK: CredentialSpec = CredentialSpec {
    service: "Outlook",
    scope: CONNECTOR_SCOPE,
    name: "outlook.access_token",
    placement: Placement::Authorization("Bearer"),
};
const SLACK: CredentialSpec = CredentialSpec {
    service: "Slack",
    scope: CONNECTOR_SCOPE,
    name: "slack.bot_token",
    placement: Placement::Authorization("Bearer"),
};
const DISCORD: CredentialSpec = CredentialSpec {
    service: "Discord",
    scope: CONNECTOR_SCOPE,
    name: "discord.bot_token",
    placement: Placement::Authorization("Bot"),
};

/// Every production connector.
pub fn production() -> Vec<Connector> {
    vec![
        Connector {
            id: "gmail",
            origin: "https://gmail.googleapis.com".into(),
            allow_private: false,
            operations: vec![
                op(
                    "gmail.messages.list",
                    EffectClass::R1,
                    Method::Get,
                    GMAIL,
                    gmail_list,
                ),
                op(
                    "gmail.messages.search",
                    EffectClass::R1,
                    Method::Get,
                    GMAIL,
                    gmail_search,
                ),
                op(
                    "gmail.messages.get",
                    EffectClass::R1,
                    Method::Get,
                    GMAIL,
                    gmail_get,
                ),
                op(
                    "gmail.messages.send",
                    EffectClass::R2,
                    Method::Post,
                    GMAIL,
                    gmail_send,
                ),
            ],
        },
        Connector {
            id: "outlook",
            origin: "https://graph.microsoft.com".into(),
            allow_private: false,
            operations: vec![
                op(
                    "outlook.messages.list",
                    EffectClass::R1,
                    Method::Get,
                    OUTLOOK,
                    outlook_list,
                ),
                op(
                    "outlook.messages.search",
                    EffectClass::R1,
                    Method::Get,
                    OUTLOOK,
                    outlook_search,
                ),
                op(
                    "outlook.messages.send",
                    EffectClass::R2,
                    Method::Post,
                    OUTLOOK,
                    outlook_send,
                ),
            ],
        },
        Connector {
            id: "slack",
            origin: "https://slack.com".into(),
            allow_private: false,
            operations: vec![
                op(
                    "slack.auth.test",
                    EffectClass::R1,
                    Method::Get,
                    SLACK,
                    slack_auth_test,
                ),
                op(
                    "slack.conversations.history",
                    EffectClass::R1,
                    Method::Get,
                    SLACK,
                    slack_history,
                ),
                op(
                    "slack.chat.post",
                    EffectClass::R2,
                    Method::Post,
                    SLACK,
                    slack_post,
                ),
            ],
        },
        Connector {
            id: "discord",
            origin: "https://discord.com".into(),
            allow_private: false,
            operations: vec![
                op(
                    "discord.users.me",
                    EffectClass::R1,
                    Method::Get,
                    DISCORD,
                    discord_me,
                ),
                op(
                    "discord.channel.messages",
                    EffectClass::R1,
                    Method::Get,
                    DISCORD,
                    discord_messages,
                ),
                op(
                    "discord.channel.post",
                    EffectClass::R2,
                    Method::Post,
                    DISCORD,
                    discord_post,
                ),
            ],
        },
    ]
}

fn op(
    id: &'static str,
    class: EffectClass,
    method: Method,
    credential: CredentialSpec,
    build: fn(&Value) -> Result<OperationRequest, AuthorityError>,
) -> ConnectorOperation {
    ConnectorOperation {
        id,
        class,
        method,
        credential: Some(credential),
        build,
    }
}

fn get(path: String, summary: Vec<String>) -> Result<OperationRequest, AuthorityError> {
    Ok(OperationRequest {
        path,
        body: None,
        content_type: None,
        summary,
    })
}

fn post_json(
    path: String,
    body: Value,
    summary: Vec<String>,
) -> Result<OperationRequest, AuthorityError> {
    Ok(OperationRequest {
        path,
        body: Some(body.to_string()),
        content_type: Some("application/json"),
        summary,
    })
}

fn encoded(text: &str) -> String {
    url::form_urlencoded::byte_serialize(text.as_bytes()).collect()
}

/// An optional field that must be one of `allowed` when present.
fn choice<'a>(
    input: &'a Value,
    name: &'static str,
    allowed: &[&'a str],
    default: &'a str,
) -> Result<&'a str, AuthorityError> {
    match input.get(name) {
        None | Some(Value::Null) => Ok(default),
        Some(Value::String(value)) => allowed
            .iter()
            .find(|candidate| **candidate == value.as_str())
            .copied()
            .ok_or(AuthorityError::InvalidAction(
                "a field is not one of its choices",
            )),
        Some(_) => Err(AuthorityError::InvalidAction("a field is not text")),
    }
}

/// An identifier of `[A-Za-z0-9]` (and `extra`), 1 to `max` characters.
fn identifier<'a>(
    input: &'a Value,
    name: &'static str,
    max: usize,
    extra: &[char],
) -> Result<&'a str, AuthorityError> {
    let value = text_field(input, name, max)?;
    if value.is_empty()
        || !value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || extra.contains(&c))
    {
        return Err(AuthorityError::InvalidAction("an identifier is malformed"));
    }
    Ok(value)
}

/// One recipient address: no line breaks, one `@`, plain.
fn address<'a>(input: &'a Value, name: &'static str) -> Result<&'a str, AuthorityError> {
    let value = text_field(input, name, 254)?;
    let ok = value.matches('@').count() == 1
        && !value.starts_with('@')
        && !value.ends_with('@')
        && value.chars().all(|c| c.is_ascii_graphic());
    if !ok {
        return Err(AuthorityError::InvalidAction(
            "a recipient address is malformed",
        ));
    }
    Ok(value)
}

/// A one-line header value.
fn line<'a>(input: &'a Value, name: &'static str, max: usize) -> Result<&'a str, AuthorityError> {
    let value = text_field(input, name, max)?;
    if value.contains('\n') {
        return Err(AuthorityError::InvalidAction(
            "a one-line field has a line break",
        ));
    }
    Ok(value)
}

fn body_lines(body: &str) -> Vec<String> {
    let first = body.lines().next().unwrap_or_default();
    vec![
        format!("Body: {} characters", body.chars().count()),
        format!(
            "Body begins: {}",
            first.chars().take(120).collect::<String>()
        ),
    ]
}

fn gmail_list(input: &Value) -> Result<OperationRequest, AuthorityError> {
    only_fields(input, &["folder"])?;
    let folder = choice(
        input,
        "folder",
        &["inbox", "sent", "drafts", "trash", "starred"],
        "inbox",
    )?;
    let label = match folder {
        "sent" => "SENT",
        "drafts" => "DRAFT",
        "trash" => "TRASH",
        "starred" => "STARRED",
        _ => "INBOX",
    };
    get(
        format!("/gmail/v1/users/me/messages?labelIds={label}&maxResults=20"),
        vec![format!("List 20 messages in {folder}")],
    )
}

fn gmail_search(input: &Value) -> Result<OperationRequest, AuthorityError> {
    only_fields(input, &["query"])?;
    let query = line(input, "query", 256)?;
    get(
        format!(
            "/gmail/v1/users/me/messages?q={}&maxResults=20",
            encoded(query)
        ),
        vec![format!("Search messages for: {query}")],
    )
}

fn gmail_get(input: &Value) -> Result<OperationRequest, AuthorityError> {
    only_fields(input, &["id"])?;
    let id = identifier(input, "id", 64, &[])?;
    get(
        format!(
            "/gmail/v1/users/me/messages/{id}?format=metadata&metadataHeaders=From&metadataHeaders=To&metadataHeaders=Subject&metadataHeaders=Date"
        ),
        vec![format!("Read the headers of message {id}")],
    )
}

fn gmail_send(input: &Value) -> Result<OperationRequest, AuthorityError> {
    only_fields(input, &["to", "subject", "body"])?;
    let to = address(input, "to")?;
    let subject = line(input, "subject", 200)?;
    let body = text_field(input, "body", 32 * 1024)?;
    let raw = format!(
        "To: {to}\r\nSubject: {subject}\r\nContent-Type: text/plain; charset=UTF-8\r\n\r\n{body}"
    );
    let raw = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(raw.as_bytes());
    let mut summary = vec![
        format!("Send an email to: {to}"),
        format!("Subject: {subject}"),
    ];
    summary.extend(body_lines(body));
    post_json(
        "/gmail/v1/users/me/messages/send".into(),
        json!({ "raw": raw }),
        summary,
    )
}

fn outlook_folder(input: &Value) -> Result<(&'static str, &'static str), AuthorityError> {
    let folder = choice(
        input,
        "folder",
        &["inbox", "sent", "drafts", "trash"],
        "inbox",
    )?;
    Ok(match folder {
        "sent" => ("sent", "sentitems"),
        "drafts" => ("drafts", "drafts"),
        "trash" => ("trash", "deleteditems"),
        _ => ("inbox", "inbox"),
    })
}

fn outlook_list(input: &Value) -> Result<OperationRequest, AuthorityError> {
    only_fields(input, &["folder"])?;
    let (folder, path) = outlook_folder(input)?;
    get(
        format!("/v1.0/me/mailFolders/{path}/messages?$top=20&$orderby=receivedDateTime+desc"),
        vec![format!("List 20 messages in {folder}")],
    )
}

fn outlook_search(input: &Value) -> Result<OperationRequest, AuthorityError> {
    only_fields(input, &["query"])?;
    let query = line(input, "query", 256)?;
    let quoted = format!("\"{}\"", query.replace('"', ""));
    get(
        format!("/v1.0/me/messages?$search={}&$top=20", encoded(&quoted)),
        vec![format!("Search messages for: {query}")],
    )
}

fn outlook_send(input: &Value) -> Result<OperationRequest, AuthorityError> {
    only_fields(input, &["to", "subject", "body"])?;
    let to = address(input, "to")?;
    let subject = line(input, "subject", 200)?;
    let body = text_field(input, "body", 32 * 1024)?;
    let mut summary = vec![
        format!("Send an email to: {to}"),
        format!("Subject: {subject}"),
    ];
    summary.extend(body_lines(body));
    post_json(
        "/v1.0/me/sendMail".into(),
        json!({
            "message": {
                "subject": subject,
                "body": { "contentType": "Text", "content": body },
                "toRecipients": [{ "emailAddress": { "address": to } }]
            },
            "saveToSentItems": true
        }),
        summary,
    )
}

fn slack_auth_test(input: &Value) -> Result<OperationRequest, AuthorityError> {
    only_fields(input, &[])?;
    get(
        "/api/auth.test".into(),
        vec!["Check the Slack connection".into()],
    )
}

fn slack_history(input: &Value) -> Result<OperationRequest, AuthorityError> {
    only_fields(input, &["channel"])?;
    let channel = identifier(input, "channel", 24, &[])?;
    get(
        format!("/api/conversations.history?channel={channel}&limit=20"),
        vec![format!("Read the 20 latest messages of channel {channel}")],
    )
}

fn slack_post(input: &Value) -> Result<OperationRequest, AuthorityError> {
    only_fields(input, &["channel", "text"])?;
    let channel = identifier(input, "channel", 24, &[])?;
    let text = text_field(input, "text", 4000)?;
    let mut summary = vec![format!("Post to Slack channel {channel}")];
    summary.extend(body_lines(text));
    post_json(
        "/api/chat.postMessage".into(),
        json!({ "channel": channel, "text": text }),
        summary,
    )
}

fn snowflake<'a>(input: &'a Value, name: &'static str) -> Result<&'a str, AuthorityError> {
    let value = text_field(input, name, 20)?;
    if value.is_empty() || !value.bytes().all(|b| b.is_ascii_digit()) {
        return Err(AuthorityError::InvalidAction("a Discord id is malformed"));
    }
    Ok(value)
}

fn discord_me(input: &Value) -> Result<OperationRequest, AuthorityError> {
    only_fields(input, &[])?;
    get(
        "/api/v10/users/@me".into(),
        vec!["Check the Discord connection".into()],
    )
}

fn discord_messages(input: &Value) -> Result<OperationRequest, AuthorityError> {
    only_fields(input, &["channel"])?;
    let channel = snowflake(input, "channel")?;
    get(
        format!("/api/v10/channels/{channel}/messages?limit=20"),
        vec![format!("Read the 20 latest messages of channel {channel}")],
    )
}

fn discord_post(input: &Value) -> Result<OperationRequest, AuthorityError> {
    only_fields(input, &["channel", "text"])?;
    let channel = snowflake(input, "channel")?;
    let text = text_field(input, "text", 2000)?;
    let mut summary = vec![format!("Post to Discord channel {channel}")];
    summary.extend(body_lines(text));
    post_json(
        format!("/api/v10/channels/{channel}/messages"),
        json!({ "content": text }),
        summary,
    )
}
