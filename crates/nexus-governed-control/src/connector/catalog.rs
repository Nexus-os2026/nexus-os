//! The production connector catalog: the Phase Zero S6 email and messaging
//! endpoints, migrated to typed operations.
//!
//! Reads are R1; every send is R2 and is approved natively with its exact
//! recipient, subject and body digest. Credentials come only from the vault
//! (scope `http`, the verified HTTP-connector scope) through broker leases,
//! and always travel in a header. Telegram is not migrated: its Bot API
//! carries the token in the URL path, which no Phase Three request may do,
//! so it stays closed.
//!
//! A Slack or Discord post names its channel by id; its destination is
//! identified by the connector's own API before it is approved and again
//! immediately before it is sent (see the module above): Slack's
//! `auth.test` (the workspace) and `conversations.info` (the conversation:
//! its id, name and kind, a direct message's peer), Discord's channel
//! object (its id, name and type, its server, a direct message's peer).

use super::{
    only_fields, text_field, Connector, ConnectorOperation, DestinationIdentity,
    DestinationResolver, OperationRequest,
};
use crate::authority::effect::EffectClass;
use crate::authority::evidence::{escaped, is_plain, quoted, wrapped};
use crate::authority::AuthorityError;
use crate::broker::{CredentialSpec, Placement};
use crate::control::EffectOutput;
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
                    "slack.conversations.info",
                    EffectClass::R1,
                    Method::Get,
                    SLACK,
                    slack_info,
                ),
                post(
                    "slack.chat.post",
                    SLACK,
                    slack_post,
                    DestinationResolver {
                        lookups: slack_lookups,
                        identify: slack_destination,
                    },
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
                    "discord.channel.get",
                    EffectClass::R1,
                    Method::Get,
                    DISCORD,
                    discord_channel,
                ),
                post(
                    "discord.channel.post",
                    DISCORD,
                    discord_post,
                    DestinationResolver {
                        lookups: discord_lookups,
                        identify: discord_destination,
                    },
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
        destination: None,
    }
}

/// A post (R2), bound to the destination `destination` identifies.
fn post(
    id: &'static str,
    credential: CredentialSpec,
    build: fn(&Value) -> Result<OperationRequest, AuthorityError>,
    destination: DestinationResolver,
) -> ConnectorOperation {
    ConnectorOperation {
        id,
        class: EffectClass::R2,
        method: Method::Post,
        credential: Some(credential),
        build,
        destination: Some(destination),
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

/// One bare recipient address, `local@domain` and nothing else: no display
/// name, quotes, brackets, comments, lists or spaces (any of those could
/// make the address that receives the mail differ from the one the owner
/// reads). At most 64 characters before the `@` and 250 in all, so the
/// approval shows it whole.
fn address<'a>(input: &'a Value, name: &'static str) -> Result<&'a str, AuthorityError> {
    let value = text_field(input, name, 250)?;
    let malformed = || AuthorityError::InvalidAction("a recipient is one bare address");
    let (local, domain) = value.split_once('@').ok_or_else(malformed)?;
    let atom = |c: char| c.is_ascii_alphanumeric() || "!#$%&'*+-/=?^_`{|}~".contains(c);
    let local_ok = !local.is_empty()
        && local.len() <= 64
        && local
            .split('.')
            .all(|part| !part.is_empty() && part.chars().all(atom));
    let domain_ok = domain.contains('.')
        && domain.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
        });
    if !local_ok || !domain_ok {
        return Err(malformed());
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

/// A message's text for the owner, in full.
fn body_lines(label: &str, body: &str) -> Vec<String> {
    quoted(
        &format!("{label} ({} characters)", body.chars().count()),
        body,
    )
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
        wrapped(&format!("Search messages for: {query}")),
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
    let mut summary = vec!["Send an email".to_string(), format!("To: {to}")];
    summary.extend(wrapped(&format!("Subject: {subject}")));
    summary.extend(body_lines("Message", body));
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
        wrapped(&format!("Search messages for: {query}")),
    )
}

fn outlook_send(input: &Value) -> Result<OperationRequest, AuthorityError> {
    only_fields(input, &["to", "subject", "body"])?;
    let to = address(input, "to")?;
    let subject = line(input, "subject", 200)?;
    let body = text_field(input, "body", 32 * 1024)?;
    let mut summary = vec!["Send an email".to_string(), format!("To: {to}")];
    summary.extend(wrapped(&format!("Subject: {subject}")));
    summary.extend(body_lines("Message", body));
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
    summary.extend(body_lines("Text", text));
    post_json(
        "/api/chat.postMessage".into(),
        json!({ "channel": channel, "text": text }),
        summary,
    )
}

fn slack_info(input: &Value) -> Result<OperationRequest, AuthorityError> {
    only_fields(input, &["channel"])?;
    let channel = identifier(input, "channel", 24, &[])?;
    get(
        format!("/api/conversations.info?channel={channel}"),
        vec![format!("Identify conversation {channel}")],
    )
}

fn slack_lookups(input: &Value) -> Result<Vec<(&'static str, Value)>, AuthorityError> {
    let channel = identifier(input, "channel", 24, &[])?;
    Ok(vec![
        ("slack.auth.test", json!({})),
        ("slack.conversations.info", json!({ "channel": channel })),
    ])
}

/// What an API answered: status 200 and a JSON object, else the
/// destination is not identified.
fn answer(output: &EffectOutput) -> Result<Value, AuthorityError> {
    let unidentified = AuthorityError::Unavailable("the destination could not be identified");
    let status = output
        .meta
        .iter()
        .find(|(key, _)| key == "status")
        .map(|(_, value)| value.as_str());
    if status != Some("200") {
        return Err(unidentified);
    }
    let value: Value = serde_json::from_str(output.text.as_deref().unwrap_or_default())
        .map_err(|_| unidentified.clone())?;
    if !value.is_object() {
        return Err(unidentified);
    }
    Ok(value)
}

/// An immutable id from an answer: 1 to `max` ASCII letters and digits
/// (digits only for Discord).
fn api_id(value: &Value, field: &str, max: usize, digits: bool) -> Result<String, AuthorityError> {
    let id = value.get(field).and_then(Value::as_str).unwrap_or_default();
    let ok = !id.is_empty()
        && id.len() <= max
        && id.bytes().all(|b| {
            if digits {
                b.is_ascii_digit()
            } else {
                b.is_ascii_alphanumeric()
            }
        });
    if !ok {
        return Err(AuthorityError::Unavailable(
            "the destination could not be identified",
        ));
    }
    Ok(id.to_string())
}

/// A readable name from an answer, escaped (shown as it is), at most 100
/// characters.
fn api_name(value: &Value, field: &str) -> Result<String, AuthorityError> {
    let name = value.get(field).and_then(Value::as_str).unwrap_or_default();
    if name.is_empty() || name.chars().count() > 100 {
        return Err(AuthorityError::Unavailable(
            "the destination could not be identified",
        ));
    }
    Ok(escaped(name))
}

/// The identity from its lines (each shown whole, wrapped) and a short
/// form for the target, the ids alone when the names make it too long.
fn identity(lines: &[String], display: String, ids_only: String) -> DestinationIdentity {
    DestinationIdentity {
        lines: lines.iter().flat_map(|line| wrapped(line)).collect(),
        display: if is_plain(&display) {
            display
        } else {
            ids_only
        },
    }
}

/// A Slack answer: also `"ok": true`.
fn slack_answer(output: &EffectOutput) -> Result<Value, AuthorityError> {
    let value = answer(output)?;
    if value.get("ok") != Some(&Value::Bool(true)) {
        return Err(AuthorityError::Closed(
            "Slack did not identify the destination",
        ));
    }
    Ok(value)
}

fn slack_destination(
    input: &Value,
    answers: &[EffectOutput],
) -> Result<DestinationIdentity, AuthorityError> {
    let channel = identifier(input, "channel", 24, &[])?;
    let [team, conversation] = answers else {
        return Err(AuthorityError::Unavailable(
            "the destination could not be identified",
        ));
    };
    let team = slack_answer(team)?;
    let team_id = api_id(&team, "team_id", 24, false)?;
    let team_name = api_name(&team, "team")?;
    let conversation = slack_answer(conversation)?;
    let conversation = conversation.get("channel").cloned().unwrap_or(Value::Null);
    // The id the post names is the conversation's own: never a name.
    let id = api_id(&conversation, "id", 24, false)?;
    if id != channel {
        return Err(AuthorityError::Closed(
            "the destination is named by its own id",
        ));
    }
    let flag = |name: &str| conversation.get(name) == Some(&Value::Bool(true));
    let (kind, who) = if flag("is_im") {
        let peer = api_id(&conversation, "user", 24, false)?;
        ("direct message".to_string(), format!("with user {peer}"))
    } else {
        let kind = if flag("is_mpim") {
            "group direct message"
        } else if flag("is_private") || flag("is_group") {
            "private channel"
        } else if flag("is_channel") {
            "public channel"
        } else {
            return Err(AuthorityError::Closed(
                "Slack did not say what kind of conversation it is",
            ));
        };
        let name = api_name(&conversation, "name")?;
        (kind.to_string(), format!("#{name}"))
    };
    Ok(identity(
        &[
            format!("Workspace: {team_name} ({team_id})"),
            format!("Conversation: {kind} {who} ({id})"),
        ],
        format!("Slack {kind} {who} ({id}) in {team_name} ({team_id})"),
        format!("Slack {kind} {id} in workspace {team_id}"),
    ))
}

fn discord_channel(input: &Value) -> Result<OperationRequest, AuthorityError> {
    only_fields(input, &["channel"])?;
    let channel = snowflake(input, "channel")?;
    get(
        format!("/api/v10/channels/{channel}"),
        vec![format!("Identify channel {channel}")],
    )
}

fn discord_lookups(input: &Value) -> Result<Vec<(&'static str, Value)>, AuthorityError> {
    let channel = snowflake(input, "channel")?;
    Ok(vec![("discord.channel.get", json!({ "channel": channel }))])
}

fn discord_destination(
    input: &Value,
    answers: &[EffectOutput],
) -> Result<DestinationIdentity, AuthorityError> {
    let channel = snowflake(input, "channel")?;
    let [found] = answers else {
        return Err(AuthorityError::Unavailable(
            "the destination could not be identified",
        ));
    };
    let found = answer(found)?;
    let id = api_id(&found, "id", 20, true)?;
    if id != channel {
        return Err(AuthorityError::Closed(
            "the destination is named by its own id",
        ));
    }
    // Only kinds a message is posted to; anything else (a category, a
    // forum, a kind this does not know) is refused.
    let kind = match found.get("type").and_then(Value::as_u64) {
        Some(0) => "text channel",
        Some(1) => "direct message",
        Some(2) => "voice channel",
        Some(3) => "group direct message",
        Some(5) => "announcement channel",
        Some(10) => "announcement thread",
        Some(11) => "public thread",
        Some(12) => "private thread",
        Some(13) => "stage channel",
        _ => {
            return Err(AuthorityError::Closed(
                "Discord did not say it is a channel a message is posted to",
            ))
        }
    };
    if kind == "direct message" {
        let recipients = found
            .get("recipients")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let [peer] = recipients.as_slice() else {
            return Err(AuthorityError::Unavailable(
                "the destination could not be identified",
            ));
        };
        let peer_id = api_id(peer, "id", 20, true)?;
        let peer_name = api_name(peer, "username")?;
        return Ok(identity(
            &[format!(
                "Conversation: direct message with {peer_name} (user {peer_id}) ({id})"
            )],
            format!("Discord direct message with {peer_name} (user {peer_id}) ({id})"),
            format!("Discord direct message with user {peer_id} ({id})"),
        ));
    }
    let name = api_name(&found, "name")?;
    let server = match found.get("guild_id") {
        Some(_) => format!("server {}", api_id(&found, "guild_id", 20, true)?),
        None if kind == "group direct message" => "no server".to_string(),
        None => {
            return Err(AuthorityError::Unavailable(
                "the destination could not be identified",
            ))
        }
    };
    Ok(identity(
        &[
            format!("Server: {server}"),
            format!("Channel: {kind} #{name} ({id})"),
        ],
        format!("Discord {kind} #{name} ({id}) in {server}"),
        format!("Discord {kind} {id} in {server}"),
    ))
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
    summary.extend(body_lines("Text", text));
    post_json(
        format!("/api/v10/channels/{channel}/messages"),
        json!({ "content": text }),
        summary,
    )
}
