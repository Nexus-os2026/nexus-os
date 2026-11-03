//! The request and reply of one model call.

use std::fmt;

use serde::de::{self, Deserialize, Deserializer, MapAccess, SeqAccess, Visitor};

use super::ModelError;

/// The bound on the complete rendered content of a request (system plus
/// user message, in bytes): `num_ctx / 2 − 96 − 64` at the qualified
/// `num_ctx` of 8,192. Under byte-level BPE a text has at most as many
/// tokens as bytes, so with the chat template's ≤96 tokens the prompt stays
/// under half the context, where the runtime would otherwise truncate.
pub const MAX_RENDERED_CONTENT_BYTES: usize = 3_936;

/// The template allowance and the margin of the byte bound (tokens).
const TEMPLATE_ALLOWANCE: usize = 96;
const MARGIN: usize = 64;

/// The smallest context under which the byte bound holds.
pub(super) const MIN_NUM_CTX: u32 =
    (2 * (MAX_RENDERED_CONTENT_BYTES + TEMPLATE_ALLOWANCE + MARGIN)) as u32;

/// The bound on a response schema's compact text.
const MAX_SCHEMA_BYTES: usize = 16 * 1024;

/// Keys a response schema may not hold at any depth: each lets prose in.
const PROSE_KEYS: [&str; 5] = ["description", "title", "examples", "default", "$comment"];

/// A closed JSON schema for the reply, sent as the request's `format`. It
/// keeps its key order (constrained decoding follows it) and refuses
/// duplicate keys, a top level that is not an object, and the prose keys
/// `description`, `title`, `examples`, `default` and `$comment` (in any
/// case) at any depth.
#[derive(Clone, PartialEq, Eq)]
pub struct ResponseSchema {
    text: String,
}

impl ResponseSchema {
    pub fn new(json: &str) -> Result<Self, ModelError> {
        let node: Node = serde_json::from_str(json).map_err(|_| ModelError::Malformed)?;
        if !matches!(node, Node::Object(_)) || node.has_prose_key() {
            return Err(ModelError::Malformed);
        }
        let mut text = String::new();
        node.write(&mut text);
        if text.len() > MAX_SCHEMA_BYTES {
            return Err(ModelError::RequestTooLarge);
        }
        Ok(Self { text })
    }

    /// The compact schema text, in its original key order.
    pub(super) fn as_json(&self) -> &str {
        &self.text
    }
}

impl fmt::Debug for ResponseSchema {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ResponseSchema")
            .field("bytes", &self.text.len())
            .finish()
    }
}

/// One request: exactly a system and a user message, and the reply schema.
/// The rendered content is at most [`MAX_RENDERED_CONTENT_BYTES`]. `Debug`
/// shows sizes only.
pub struct ModelRequest {
    system: String,
    user: String,
    format: ResponseSchema,
}

impl ModelRequest {
    pub fn new(system: String, user: String, format: ResponseSchema) -> Result<Self, ModelError> {
        let request = Self {
            system,
            user,
            format,
        };
        request.check()?;
        Ok(request)
    }

    /// The bytes of the rendered content (system plus user message).
    pub fn rendered_bytes(&self) -> usize {
        self.system.len().saturating_add(self.user.len())
    }

    /// The byte bound, checked at construction and again before a call.
    pub(super) fn check(&self) -> Result<(), ModelError> {
        if self.rendered_bytes() > MAX_RENDERED_CONTENT_BYTES {
            return Err(ModelError::RequestTooLarge);
        }
        Ok(())
    }

    pub(super) fn system(&self) -> &str {
        &self.system
    }

    pub(super) fn user(&self) -> &str {
        &self.user
    }

    pub(super) fn format(&self) -> &ResponseSchema {
        &self.format
    }

    /// A request made without the construction check (the call's own check
    /// is exercised with it).
    #[cfg(test)]
    pub(super) fn unchecked(system: String, user: String, format: ResponseSchema) -> Self {
        Self {
            system,
            user,
            format,
        }
    }
}

impl fmt::Debug for ModelRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ModelRequest")
            .field("system_bytes", &self.system.len())
            .field("user_bytes", &self.user.len())
            .field("format_bytes", &self.format.text.len())
            .finish()
    }
}

/// An accepted reply: its content (untrusted data) and the runtime's
/// counts. Neither `Clone` nor serializable; `Debug` shows sizes only.
pub struct ModelReply {
    content: String,
    prompt_tokens: u32,
    output_tokens: u32,
    latency_ms: u32,
}

impl ModelReply {
    pub(super) fn new(
        content: String,
        prompt_tokens: u32,
        output_tokens: u32,
        latency_ms: u32,
    ) -> Self {
        Self {
            content,
            prompt_tokens,
            output_tokens,
            latency_ms,
        }
    }

    pub fn content(&self) -> &str {
        &self.content
    }

    pub fn into_content(self) -> String {
        self.content
    }

    pub fn prompt_tokens(&self) -> u32 {
        self.prompt_tokens
    }

    pub fn output_tokens(&self) -> u32 {
        self.output_tokens
    }

    pub fn latency_ms(&self) -> u32 {
        self.latency_ms
    }
}

impl fmt::Debug for ModelReply {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ModelReply")
            .field("content_bytes", &self.content.len())
            .field("prompt_tokens", &self.prompt_tokens)
            .field("output_tokens", &self.output_tokens)
            .field("latency_ms", &self.latency_ms)
            .finish()
    }
}

/// A JSON value that keeps its object keys in order and has no duplicates.
enum Node {
    Null,
    Bool(bool),
    Number(serde_json::Number),
    String(String),
    Array(Vec<Node>),
    Object(Vec<(String, Node)>),
}

impl Node {
    fn has_prose_key(&self) -> bool {
        match self {
            Node::Array(items) => items.iter().any(Node::has_prose_key),
            Node::Object(entries) => entries.iter().any(|(key, value)| {
                PROSE_KEYS
                    .iter()
                    .any(|prose| key.eq_ignore_ascii_case(prose))
                    || value.has_prose_key()
            }),
            _ => false,
        }
    }

    fn write(&self, out: &mut String) {
        match self {
            Node::Null => out.push_str("null"),
            Node::Bool(value) => out.push_str(if *value { "true" } else { "false" }),
            Node::Number(number) => out.push_str(&number.to_string()),
            Node::String(text) => out.push_str(&json_string(text)),
            Node::Array(items) => {
                out.push('[');
                for (index, item) in items.iter().enumerate() {
                    if index > 0 {
                        out.push(',');
                    }
                    item.write(out);
                }
                out.push(']');
            }
            Node::Object(entries) => {
                out.push('{');
                for (index, (key, value)) in entries.iter().enumerate() {
                    if index > 0 {
                        out.push(',');
                    }
                    out.push_str(&json_string(key));
                    out.push(':');
                    value.write(out);
                }
                out.push('}');
            }
        }
    }
}

/// A JSON string literal.
pub(super) fn json_string(text: &str) -> String {
    serde_json::Value::from(text).to_string()
}

impl<'de> Deserialize<'de> for Node {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(NodeVisitor)
    }
}

struct NodeVisitor;

impl<'de> Visitor<'de> for NodeVisitor {
    type Value = Node;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a JSON value")
    }

    fn visit_unit<E: de::Error>(self) -> Result<Node, E> {
        Ok(Node::Null)
    }

    fn visit_bool<E: de::Error>(self, value: bool) -> Result<Node, E> {
        Ok(Node::Bool(value))
    }

    fn visit_u64<E: de::Error>(self, value: u64) -> Result<Node, E> {
        Ok(Node::Number(value.into()))
    }

    fn visit_i64<E: de::Error>(self, value: i64) -> Result<Node, E> {
        Ok(Node::Number(value.into()))
    }

    fn visit_f64<E: de::Error>(self, value: f64) -> Result<Node, E> {
        serde_json::Number::from_f64(value)
            .map(Node::Number)
            .ok_or_else(|| E::custom("not a finite number"))
    }

    fn visit_str<E: de::Error>(self, value: &str) -> Result<Node, E> {
        Ok(Node::String(value.to_string()))
    }

    fn visit_string<E: de::Error>(self, value: String) -> Result<Node, E> {
        Ok(Node::String(value))
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Node, A::Error> {
        let mut items = Vec::new();
        while let Some(item) = seq.next_element()? {
            items.push(item);
        }
        Ok(Node::Array(items))
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Node, A::Error> {
        let mut entries: Vec<(String, Node)> = Vec::new();
        while let Some(key) = map.next_key::<String>()? {
            if entries.iter().any(|(seen, _)| *seen == key) {
                return Err(de::Error::custom("duplicate key"));
            }
            let value = map.next_value()?;
            entries.push((key, value));
        }
        Ok(Node::Object(entries))
    }
}
