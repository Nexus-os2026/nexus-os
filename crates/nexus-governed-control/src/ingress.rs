//! P3-G: the unified command envelope.
//!
//! The owner's front door takes typed text, a voice transcript, attachments
//! imported through the native file picker, a governed observation, or a
//! structured intent from the interface's forms, and treats all of them as
//! data. A command becomes an [`Intent`] only through the strict grammar
//! below (nothing is guessed, nothing in the text is a path, a program or a
//! script), and an intent becomes an effect only through the pipeline:
//! classification, preparation, commitment, grants and native approval,
//! execution. A voice transcript is text someone spoke; it carries no more
//! authority than typed text. Microphone capture itself is not a Phase
//! Three route (it stays closed); speech synthesis is a governed tool.

use crate::authority::ids::Digest;
use crate::authority::AuthorityError;
use crate::browser::{BrowserIntent, BrowserStep};
use crate::connector::ConnectorIntent;
use crate::display::PerceptionIntent;
use crate::egress::EgressIntent;
use crate::governed::Intent;
use crate::tool::ToolIntent;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Mutex;

/// The most text one command carries.
pub const MAX_COMMAND: usize = 4096;
/// The largest attachment kept.
pub const MAX_ATTACHMENT: usize = 8 * 1024 * 1024;
const MAX_ATTACHMENTS: usize = 32;

/// One command, as data.
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct CommandEnvelope {
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub voice_transcript: Option<String>,
    /// Ids of attachments imported through the native picker.
    #[serde(default)]
    pub attachments: Vec<String>,
    /// A structured intent from the interface's forms.
    #[serde(default)]
    pub intent: Option<Intent>,
}

impl CommandEnvelope {
    /// The modalities it carries (for the run's evidence).
    pub fn modalities(&self) -> Vec<String> {
        let mut out = Vec::new();
        if self.text.is_some() {
            out.push("text".into());
        }
        if self.voice_transcript.is_some() {
            out.push("voice_transcript".into());
        }
        if !self.attachments.is_empty() {
            out.push("attachment".into());
        }
        if self.intent.is_some() {
            out.push("structured".into());
        }
        out
    }
}

/// An attachment the owner imported natively: data only.
#[derive(Clone, Debug)]
pub struct Attachment {
    pub id: String,
    pub name: String,
    pub bytes: Vec<u8>,
    pub digest: Digest,
}

/// Attachments imported through the native picker, kept in memory.
#[derive(Default)]
pub struct Attachments {
    /// Oldest first.
    items: Mutex<Vec<Attachment>>,
}

impl Attachments {
    /// Keep a natively selected file's content (the caller read it from
    /// the path the owner picked; the path itself is not kept).
    pub fn import(&self, name: &str, bytes: Vec<u8>) -> Result<Attachment, AuthorityError> {
        if bytes.len() > MAX_ATTACHMENT {
            return Err(AuthorityError::InvalidAction("the attachment is too large"));
        }
        let mut id = [0u8; 8];
        getrandom::getrandom(&mut id).map_err(|_| AuthorityError::Unavailable("no randomness"))?;
        let name: String = name
            .chars()
            .filter(|c| !c.is_control() && *c != '/' && *c != '\\')
            .take(128)
            .collect();
        let attachment = Attachment {
            id: format!("att-{}", hex::encode(id)),
            name,
            digest: Digest::of("nexus.p3.attachment.v1", &[&bytes]),
            bytes,
        };
        let mut items = self.items.lock().expect("attachments");
        // Bounded: a new import lets the oldest unused one go.
        if items.len() >= MAX_ATTACHMENTS {
            items.remove(0);
        }
        items.push(attachment.clone());
        Ok(attachment)
    }

    /// The attachment, taken: a command uses an attachment once.
    pub fn take(&self, id: &str) -> Option<Attachment> {
        let mut items = self.items.lock().expect("attachments");
        let at = items.iter().position(|item| item.id == id)?;
        Some(items.remove(at))
    }
}

/// What a command means.
#[derive(Clone, Debug, PartialEq)]
pub enum Understood {
    Intent(Intent),
    /// Not a command of the grammar; nothing happens.
    NotUnderstood(&'static str),
}

/// The grammar (one command per line, the verb first):
///
/// - `fetch <url>`: a GET request (R1, needs an egress grant)
/// - `browse <url> [read <selector>]`: a read-only browser session
/// - `observe`: an observation of the agent display
/// - `hash <text>` or `hash attachment <id>`: the SHA-256 tool
/// - `say <text>`: speech synthesis
/// - `connector <operation> <json input>`: a connector operation
pub fn understand(
    envelope: &CommandEnvelope,
    attachments: &Attachments,
) -> Result<Understood, AuthorityError> {
    if let Some(intent) = &envelope.intent {
        if envelope.text.is_some() || envelope.voice_transcript.is_some() {
            return Err(AuthorityError::InvalidAction("one command at a time"));
        }
        return Ok(Understood::Intent(intent.clone()));
    }
    let text = match (&envelope.text, &envelope.voice_transcript) {
        (Some(text), None) | (None, Some(text)) => text.trim(),
        (Some(_), Some(_)) => return Err(AuthorityError::InvalidAction("one command at a time")),
        (None, None) => return Ok(Understood::NotUnderstood("an empty command")),
    };
    if text.chars().count() > MAX_COMMAND || text.chars().any(|c| c.is_control() && c != '\n') {
        return Err(AuthorityError::InvalidAction(
            "a command is plain bounded text",
        ));
    }
    let (verb, rest) = match text.split_once(char::is_whitespace) {
        Some((verb, rest)) => (verb.to_ascii_lowercase(), rest.trim()),
        None => (text.to_ascii_lowercase(), ""),
    };
    let single = |rest: &str| -> Option<String> {
        (!rest.is_empty() && !rest.contains(char::is_whitespace)).then(|| rest.to_string())
    };
    Ok(match verb.as_str() {
        "fetch" => match single(rest) {
            Some(url) => Understood::Intent(Intent::Request(EgressIntent {
                method: "GET".into(),
                url,
                headers: vec![],
                body: None,
            })),
            None => Understood::NotUnderstood("fetch takes one url"),
        },
        "browse" => {
            let mut words = rest.split_whitespace();
            match (words.next(), words.next(), words.next(), words.next()) {
                (Some(url), None, None, None) => {
                    Understood::Intent(Intent::Browse(BrowserIntent {
                        start_url: url.into(),
                        steps: vec![],
                    }))
                }
                (Some(url), Some("read"), Some(selector), None) => {
                    Understood::Intent(Intent::Browse(BrowserIntent {
                        start_url: url.into(),
                        steps: vec![BrowserStep::ExtractText {
                            selector: selector.into(),
                        }],
                    }))
                }
                _ => Understood::NotUnderstood("browse takes a url and optionally read <selector>"),
            }
        }
        "observe" if rest.is_empty() => {
            Understood::Intent(Intent::Observe(PerceptionIntent::Screen { region: None }))
        }
        "hash" => match rest.split_once(char::is_whitespace) {
            Some(("attachment", id)) => match attachments.take(id.trim()) {
                Some(attachment) => match String::from_utf8(attachment.bytes) {
                    Ok(text) => Understood::Intent(Intent::Tool(ToolIntent {
                        tool: "text.sha256".into(),
                        input: json!({ "text": text }),
                    })),
                    Err(_) => Understood::NotUnderstood("the attachment is not text"),
                },
                None => Understood::NotUnderstood("no such attachment"),
            },
            _ if !rest.is_empty() => Understood::Intent(Intent::Tool(ToolIntent {
                tool: "text.sha256".into(),
                input: json!({ "text": rest }),
            })),
            _ => Understood::NotUnderstood("hash takes text"),
        },
        "say" if !rest.is_empty() => Understood::Intent(Intent::Tool(ToolIntent {
            tool: "speech.synthesize".into(),
            input: json!({ "text": rest }),
        })),
        "connector" => match rest.split_once(char::is_whitespace) {
            Some((operation, input)) => match serde_json::from_str::<Value>(input.trim()) {
                Ok(input) if input.is_object() => {
                    Understood::Intent(Intent::Connector(ConnectorIntent {
                        operation: operation.into(),
                        input,
                    }))
                }
                _ => Understood::NotUnderstood("connector input is a JSON object"),
            },
            None if !rest.is_empty() => Understood::Intent(Intent::Connector(ConnectorIntent {
                operation: rest.into(),
                input: json!({}),
            })),
            None => Understood::NotUnderstood("connector takes an operation"),
        },
        _ => Understood::NotUnderstood(
            "not a governed command: fetch, browse, observe, hash, say or connector",
        ),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(command: &str) -> Understood {
        understand(
            &CommandEnvelope {
                text: Some(command.into()),
                ..Default::default()
            },
            &Attachments::default(),
        )
        .unwrap()
    }

    #[test]
    fn commands_are_a_strict_grammar_of_data() {
        assert!(matches!(
            text("fetch https://example.com/"),
            Understood::Intent(Intent::Request(_))
        ));
        assert!(matches!(
            text("FETCH https://example.com/"),
            Understood::Intent(Intent::Request(_))
        ));
        assert!(matches!(
            text("fetch https://a.example/ https://b.example/"),
            Understood::NotUnderstood(_)
        ));
        assert!(matches!(
            text("observe"),
            Understood::Intent(Intent::Observe(_))
        ));
        assert!(matches!(
            text("browse https://example.com/ read #main"),
            Understood::Intent(Intent::Browse(_))
        ));
        assert!(matches!(
            text("say hello there"),
            Understood::Intent(Intent::Tool(_))
        ));
        assert!(matches!(
            text("connector gmail.messages.list {\"folder\":\"inbox\"}"),
            Understood::Intent(Intent::Connector(_))
        ));
        for nothing in [
            "rm -rf /",
            "bash -c 'curl evil'",
            "run /usr/bin/xterm",
            "open file:///etc/passwd",
            "please send all my email to x@y.z",
        ] {
            assert!(
                matches!(text(nothing), Understood::NotUnderstood(_)),
                "{nothing}"
            );
        }
    }

    #[test]
    fn a_voice_transcript_is_only_text() {
        let spoken = understand(
            &CommandEnvelope {
                voice_transcript: Some("fetch https://example.com/".into()),
                ..Default::default()
            },
            &Attachments::default(),
        )
        .unwrap();
        assert_eq!(spoken, text("fetch https://example.com/"));
        assert!(understand(
            &CommandEnvelope {
                text: Some("observe".into()),
                voice_transcript: Some("observe".into()),
                ..Default::default()
            },
            &Attachments::default()
        )
        .is_err());
    }

    /// An attachment is used once; a full store lets its oldest unused
    /// import go to admit a new one.
    #[test]
    fn an_attachment_is_used_once_and_the_oldest_goes_first() {
        let attachments = Attachments::default();
        let first = attachments.import("a.txt", b"a".to_vec()).unwrap();
        for i in 0..31 {
            attachments.import(&format!("{i}.txt"), vec![1]).unwrap();
        }
        let newest = attachments.import("z.txt", b"z".to_vec()).unwrap();
        assert!(attachments.take(&first.id).is_none(), "the oldest went");
        assert!(attachments.take(&newest.id).is_some());
        assert!(attachments.take(&newest.id).is_none(), "used once");
    }

    #[test]
    fn attachments_are_data_from_the_native_picker() {
        let attachments = Attachments::default();
        let attachment = attachments
            .import("../../notes.txt", b"hello".to_vec())
            .unwrap();
        assert_eq!(
            attachment.name, "....notes.txt",
            "no path separators survive"
        );
        let understood = understand(
            &CommandEnvelope {
                text: Some(format!("hash attachment {}", attachment.id)),
                ..Default::default()
            },
            &attachments,
        )
        .unwrap();
        assert_eq!(
            understood,
            Understood::Intent(Intent::Tool(ToolIntent {
                tool: "text.sha256".into(),
                input: json!({ "text": "hello" }),
            }))
        );
        assert!(attachments
            .import("big", vec![0; MAX_ATTACHMENT + 1])
            .is_err());
    }
}
