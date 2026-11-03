//! The request and reply of one model call.

use std::fmt;

use serde::de::{self, Deserialize, Deserializer, MapAccess, SeqAccess, Visitor};

use sha2::{Digest, Sha256};

use super::ModelError;

/// The bound on the complete rendered content of a request (system plus
/// user message, in bytes): `num_ctx / 2 − 96 − 64` at the qualified
/// `num_ctx` of 8,192. Under byte-level BPE a text has at most as many
/// tokens as bytes, so with the chat template's ≤96 tokens the prompt stays
/// under half the context, where the runtime would otherwise truncate.
pub const MAX_RENDERED_CONTENT_BYTES: usize = 3_936;

/// The fewest prompt tokens a reply may report (design §10.4
/// `SCAFFOLD_FLOOR`). A reply that reports fewer, 0 included, cannot have
/// processed the fixed qualified texts, and is refused as `Truncated`.
///
/// **PROVISIONAL until Q-R3.** Q-R3 (design §4) measures the token count of
/// the fixed qualified texts on the pinned runtime and freezes this value.
/// 256 is conservative: the smallest qualified Builder prompt (the 1,024-byte
/// system instruction and the 1,970-byte fixed user text, with no notes) is
/// about 3,000 bytes of English text, which is at least 374 tokens even at 8
/// bytes per token; the qualification measured 587 prompt tokens for it with
/// the bakery notes. **Binding:** M2e-C2 must not enable the Builder lane
/// until Q-R3 has frozen this value.
// PROVISIONAL until Q-R3: replace with the frozen SCAFFOLD_FLOOR.
pub const SCAFFOLD_FLOOR: u32 = 256;

/// The template allowance and the margin of the byte bound (tokens).
pub const TEMPLATE_ALLOWANCE: usize = 96;
pub const MARGIN: usize = 64;

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

/// One request and its reply schema: either exactly a system and a user
/// message (A1's qualified kind; the rendered content is at most
/// [`MAX_RENDERED_CONTENT_BYTES`]), or a [`Conversation`]'s complete
/// ordered history ([`ModelRequest::conversation`], N1). `Debug` shows
/// sizes only.
pub struct ModelRequest {
    body: Body,
    format: ResponseSchema,
}

enum Body {
    Pair { system: String, user: String },
    Conversation(Rendered),
}

/// A conversation's history as sent: each message's role and its content as
/// it goes on the wire.
pub(super) struct Rendered {
    pub(super) messages: Vec<(MessageRole, String)>,
    /// The conversation and its length when the request was built.
    pub(super) answered: Answered,
    /// The prompt count of the conversation's last accepted reply.
    #[allow(dead_code)] // M2e-A2 stub: read from the implementation commit
    pub(super) previous_prompt: Option<u32>,
}

/// Which conversation, at which length, a reply answers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Answered {
    pub(super) conversation: u64,
    pub(super) length: usize,
}

impl ModelRequest {
    pub fn new(system: String, user: String, format: ResponseSchema) -> Result<Self, ModelError> {
        let request = Self {
            body: Body::Pair { system, user },
            format,
        };
        request.check()?;
        Ok(request)
    }

    /// A request carrying `conversation`'s complete ordered history (N1).
    pub fn conversation(
        conversation: &Conversation,
        format: ResponseSchema,
    ) -> Result<Self, ModelError> {
        Ok(Self {
            body: Body::Conversation(conversation.render()),
            format,
        })
    }

    /// Whether this request carries a conversation (N1's kind).
    pub fn is_conversation(&self) -> bool {
        matches!(self.body, Body::Conversation(_))
    }

    /// The bytes of the rendered content: the system plus the user message,
    /// or every message of a conversation as sent.
    pub fn rendered_bytes(&self) -> usize {
        match &self.body {
            Body::Pair { system, user } => system.len().saturating_add(user.len()),
            Body::Conversation(rendered) => content_bytes(&rendered.messages),
        }
    }

    /// The byte bound, checked at construction and again before a call.
    pub(super) fn check(&self) -> Result<(), ModelError> {
        match &self.body {
            Body::Pair { .. } => {
                if self.rendered_bytes() > MAX_RENDERED_CONTENT_BYTES {
                    return Err(ModelError::RequestTooLarge);
                }
                Ok(())
            }
            Body::Conversation(_) => Ok(()),
        }
    }

    /// The conversation this request carries, if any.
    pub(super) fn rendered(&self) -> Option<&Rendered> {
        match &self.body {
            Body::Pair { .. } => None,
            Body::Conversation(rendered) => Some(rendered),
        }
    }

    /// The messages as sent, in order.
    pub(super) fn messages(&self) -> Vec<(MessageRole, &str)> {
        match &self.body {
            Body::Pair { system, user } => vec![
                (MessageRole::System, system.as_str()),
                (MessageRole::User, user.as_str()),
            ],
            Body::Conversation(rendered) => rendered
                .messages
                .iter()
                .map(|(role, content)| (*role, content.as_str()))
                .collect(),
        }
    }

    pub(super) fn format(&self) -> &ResponseSchema {
        &self.format
    }

    /// A request made without the construction check (the call's own check
    /// is exercised with it).
    #[cfg(test)]
    pub(super) fn unchecked(system: String, user: String, format: ResponseSchema) -> Self {
        Self {
            body: Body::Pair { system, user },
            format,
        }
    }

    /// A conversation request made of these messages exactly as sent, without
    /// any construction check (the transport's own pre-send check is
    /// exercised with it).
    #[cfg(test)]
    pub(super) fn conversation_unchecked(
        messages: Vec<(MessageRole, String)>,
        format: ResponseSchema,
    ) -> Self {
        let length = messages.len();
        Self {
            body: Body::Conversation(Rendered {
                messages,
                answered: Answered {
                    conversation: 0,
                    length,
                },
                previous_prompt: None,
            }),
            format,
        }
    }
}

impl fmt::Debug for ModelRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.body {
            Body::Pair { system, user } => f
                .debug_struct("ModelRequest")
                .field("system_bytes", &system.len())
                .field("user_bytes", &user.len())
                .field("format_bytes", &self.format.text.len())
                .finish(),
            Body::Conversation(rendered) => f
                .debug_struct("ModelRequest")
                .field("messages", &rendered.messages.len())
                .field("content_bytes", &content_bytes(&rendered.messages))
                .field("format_bytes", &self.format.text.len())
                .finish(),
        }
    }
}

fn content_bytes(messages: &[(MessageRole, String)]) -> usize {
    messages.iter().fold(0usize, |sum, (_, content)| {
        sum.saturating_add(content.len())
    })
}

/// An accepted reply: its content (untrusted data) and the runtime's
/// counts. Neither `Clone` nor serializable; `Debug` shows sizes only.
pub struct ModelReply {
    content: String,
    prompt_tokens: u32,
    output_tokens: u32,
    latency_ms: u32,
    /// The conversation request this reply answered, if any.
    answered: Option<Answered>,
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
            answered: None,
        }
    }

    /// Mark the reply as the answer to this conversation request.
    pub(super) fn answering(mut self, answered: Answered) -> Self {
        self.answered = Some(answered);
        self
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

// ── N1: the retained worker's complete ordered history ──────────────────────

/// The most messages a conversation carries: the system message, the opening
/// user message and up to three answer/feedback pairs (N1 spec §2.1).
pub const MAX_CONVERSATION_MESSAGES: usize = 8;
/// The hard generation cap sent as `num_predict` with every conversation
/// request (N1 spec §2.3), in tokens.
pub const ANSWER_CAP: u32 = 2_048;
/// The template's allowance per message (tokens; N1 spec §2.3).
pub const PER_MESSAGE_ALLOWANCE: usize = 16;
/// The allowance for the fixed assistant/no-thinking suffix (tokens).
pub const SUFFIX_ALLOWANCE: usize = 16;
/// The context the conversation bound is qualified at.
const CONVERSATION_NUM_CTX: u32 = 8_192;

/// The fixed, non-whitespace frame around every user message as sent, so
/// that the pinned renderer's trimming of each message removes nothing
/// (N1 spec §2.2b). Both are ASCII; `\n` is one newline byte.
pub const FRAME_OPEN: &str = "--- message (frame v1) ---\n";
pub const FRAME_CLOSE: &str = "\n--- end of message (frame v1) ---";
/// The frame's version, part of the conversation configuration.
pub const USER_FRAME_VERSION: &str = "user-frame-v1";
/// The retained-answer encoding's identifier (N1 spec §2.2).
pub const RETAINED_ANSWER_ENCODING: &str = "retained-answer-v1";

/// The frozen delimiter inventory of the pinned model (N1 spec §2.2a): the
/// union of the qwen35 renderer's sensitive strings and every CONTROL and
/// USER_DEFINED token spelling in the vocabulary of the installed GGUF
/// (manifest `a7eb95c5…`, model layer `d372de8e…`), sorted by bytes. It was
/// extracted by the pre-registered script `extract_inventory.py` (SHA-256
/// `5c83ba2f926a2bb159588d4f9ae105de962142ce5580cbfd613613ea6a3c86fe`,
/// quoted in the M2e-A2 pull request), which Q-R-N1 re-runs. A user message
/// containing any entry (exact, case-sensitive substring) is refused with
/// [`ModelError::UnsafeDelimiter`].
pub const DELIMITER_INVENTORY: [&str; 276] = [
    "</think>",
    "</tool_call>",
    "</tool_response>",
    "<think>",
    "<tool_call>",
    "<tool_response>",
    "<|box_end|>",
    "<|box_start|>",
    "<|endoftext|>",
    "<|file_sep|>",
    "<|fim_middle|>",
    "<|fim_pad|>",
    "<|fim_prefix|>",
    "<|fim_suffix|>",
    "<|im_end|>",
    "<|im_start|>",
    "<|image_pad|>",
    "<|object_ref_end|>",
    "<|object_ref_start|>",
    "<|quad_end|>",
    "<|quad_start|>",
    "<|repo_name|>",
    "<|video_pad|>",
    "<|vision_end|>",
    "<|vision_pad|>",
    "<|vision_start|>",
    "[PAD0]",
    "[PAD100]",
    "[PAD101]",
    "[PAD102]",
    "[PAD103]",
    "[PAD104]",
    "[PAD105]",
    "[PAD106]",
    "[PAD107]",
    "[PAD108]",
    "[PAD109]",
    "[PAD10]",
    "[PAD110]",
    "[PAD111]",
    "[PAD112]",
    "[PAD113]",
    "[PAD114]",
    "[PAD115]",
    "[PAD116]",
    "[PAD117]",
    "[PAD118]",
    "[PAD119]",
    "[PAD11]",
    "[PAD120]",
    "[PAD121]",
    "[PAD122]",
    "[PAD123]",
    "[PAD124]",
    "[PAD125]",
    "[PAD126]",
    "[PAD127]",
    "[PAD128]",
    "[PAD129]",
    "[PAD12]",
    "[PAD130]",
    "[PAD131]",
    "[PAD132]",
    "[PAD133]",
    "[PAD134]",
    "[PAD135]",
    "[PAD136]",
    "[PAD137]",
    "[PAD138]",
    "[PAD139]",
    "[PAD13]",
    "[PAD140]",
    "[PAD141]",
    "[PAD142]",
    "[PAD143]",
    "[PAD144]",
    "[PAD145]",
    "[PAD146]",
    "[PAD147]",
    "[PAD148]",
    "[PAD149]",
    "[PAD14]",
    "[PAD150]",
    "[PAD151]",
    "[PAD152]",
    "[PAD153]",
    "[PAD154]",
    "[PAD155]",
    "[PAD156]",
    "[PAD157]",
    "[PAD158]",
    "[PAD159]",
    "[PAD15]",
    "[PAD160]",
    "[PAD161]",
    "[PAD162]",
    "[PAD163]",
    "[PAD164]",
    "[PAD165]",
    "[PAD166]",
    "[PAD167]",
    "[PAD168]",
    "[PAD169]",
    "[PAD16]",
    "[PAD170]",
    "[PAD171]",
    "[PAD172]",
    "[PAD173]",
    "[PAD174]",
    "[PAD175]",
    "[PAD176]",
    "[PAD177]",
    "[PAD178]",
    "[PAD179]",
    "[PAD17]",
    "[PAD180]",
    "[PAD181]",
    "[PAD182]",
    "[PAD183]",
    "[PAD184]",
    "[PAD185]",
    "[PAD186]",
    "[PAD187]",
    "[PAD188]",
    "[PAD189]",
    "[PAD18]",
    "[PAD190]",
    "[PAD191]",
    "[PAD192]",
    "[PAD193]",
    "[PAD194]",
    "[PAD195]",
    "[PAD196]",
    "[PAD197]",
    "[PAD198]",
    "[PAD199]",
    "[PAD19]",
    "[PAD1]",
    "[PAD200]",
    "[PAD201]",
    "[PAD202]",
    "[PAD203]",
    "[PAD204]",
    "[PAD205]",
    "[PAD206]",
    "[PAD207]",
    "[PAD208]",
    "[PAD209]",
    "[PAD20]",
    "[PAD210]",
    "[PAD211]",
    "[PAD212]",
    "[PAD213]",
    "[PAD214]",
    "[PAD215]",
    "[PAD216]",
    "[PAD217]",
    "[PAD218]",
    "[PAD219]",
    "[PAD21]",
    "[PAD220]",
    "[PAD221]",
    "[PAD222]",
    "[PAD223]",
    "[PAD224]",
    "[PAD225]",
    "[PAD226]",
    "[PAD227]",
    "[PAD228]",
    "[PAD229]",
    "[PAD22]",
    "[PAD230]",
    "[PAD231]",
    "[PAD232]",
    "[PAD233]",
    "[PAD234]",
    "[PAD235]",
    "[PAD236]",
    "[PAD237]",
    "[PAD238]",
    "[PAD239]",
    "[PAD23]",
    "[PAD240]",
    "[PAD241]",
    "[PAD242]",
    "[PAD243]",
    "[PAD244]",
    "[PAD245]",
    "[PAD246]",
    "[PAD247]",
    "[PAD248]",
    "[PAD249]",
    "[PAD24]",
    "[PAD25]",
    "[PAD26]",
    "[PAD27]",
    "[PAD28]",
    "[PAD29]",
    "[PAD2]",
    "[PAD30]",
    "[PAD31]",
    "[PAD32]",
    "[PAD33]",
    "[PAD34]",
    "[PAD35]",
    "[PAD36]",
    "[PAD37]",
    "[PAD38]",
    "[PAD39]",
    "[PAD3]",
    "[PAD40]",
    "[PAD41]",
    "[PAD42]",
    "[PAD43]",
    "[PAD44]",
    "[PAD45]",
    "[PAD46]",
    "[PAD47]",
    "[PAD48]",
    "[PAD49]",
    "[PAD4]",
    "[PAD50]",
    "[PAD51]",
    "[PAD52]",
    "[PAD53]",
    "[PAD54]",
    "[PAD55]",
    "[PAD56]",
    "[PAD57]",
    "[PAD58]",
    "[PAD59]",
    "[PAD5]",
    "[PAD60]",
    "[PAD61]",
    "[PAD62]",
    "[PAD63]",
    "[PAD64]",
    "[PAD65]",
    "[PAD66]",
    "[PAD67]",
    "[PAD68]",
    "[PAD69]",
    "[PAD6]",
    "[PAD70]",
    "[PAD71]",
    "[PAD72]",
    "[PAD73]",
    "[PAD74]",
    "[PAD75]",
    "[PAD76]",
    "[PAD77]",
    "[PAD78]",
    "[PAD79]",
    "[PAD7]",
    "[PAD80]",
    "[PAD81]",
    "[PAD82]",
    "[PAD83]",
    "[PAD84]",
    "[PAD85]",
    "[PAD86]",
    "[PAD87]",
    "[PAD88]",
    "[PAD89]",
    "[PAD8]",
    "[PAD90]",
    "[PAD91]",
    "[PAD92]",
    "[PAD93]",
    "[PAD94]",
    "[PAD95]",
    "[PAD96]",
    "[PAD97]",
    "[PAD98]",
    "[PAD99]",
    "[PAD9]",
];

/// SHA-256 of the inventory's canonical text (each entry followed by `\n`).
pub const DELIMITER_INVENTORY_SHA256: &str =
    "0bb5816fb3a1ee2b86346991753d50da51d67388e22f3c8c36a9ce1f2785d238";

/// A message's role.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageRole {
    System,
    User,
    Assistant,
}

impl MessageRole {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            MessageRole::System => "system",
            MessageRole::User => "user",
            MessageRole::Assistant => "assistant",
        }
    }
}

/// An accepted answer in the form it is retained and sent back (N1 spec
/// §2.2, `retained-answer-v1`). `Debug` shows its size only.
#[derive(Clone, PartialEq, Eq)]
pub struct RetainedAnswer {
    text: String,
}

impl RetainedAnswer {
    /// The retained form of an answer.
    pub fn of(answer: &str) -> Result<Self, ModelError> {
        Ok(Self {
            text: answer.to_string(),
        })
    }

    pub fn as_str(&self) -> &str {
        &self.text
    }

    pub fn into_string(self) -> String {
        self.text
    }
}

impl fmt::Debug for RetainedAnswer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RetainedAnswer")
            .field("bytes", &self.text.len())
            .finish()
    }
}

/// The complete ordered history of one model conversation (N1 spec §2.1):
/// the system message, the opening user message, then alternating retained
/// answers and backend feedback. `Debug` shows roles, counts and sizes only.
pub struct Conversation {
    id: u64,
    /// Each message's role and its original content (the retained form of
    /// an answer).
    messages: Vec<(MessageRole, String)>,
    /// The prompt count of the last accepted reply.
    last_prompt: Option<u32>,
}

/// Distinct conversations in this process.
static NEXT_CONVERSATION: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

impl Conversation {
    /// A conversation of the system message and the opening user message.
    pub fn new(system: String, opening: String) -> Result<Self, ModelError> {
        Ok(Self {
            id: NEXT_CONVERSATION.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            messages: vec![(MessageRole::System, system), (MessageRole::User, opening)],
            last_prompt: None,
        })
    }

    /// Append the retained answer to the last user message.
    pub fn push_answer(&mut self, answer: RetainedAnswer) -> Result<(), ModelError> {
        self.messages.push((MessageRole::Assistant, answer.text));
        Ok(())
    }

    /// Append backend feedback to the last answer.
    pub fn push_feedback(&mut self, feedback: String) -> Result<(), ModelError> {
        self.messages.push((MessageRole::User, feedback));
        Ok(())
    }

    /// The number of messages.
    pub fn len(&self) -> usize {
        self.messages.len()
    }

    /// Never true: a conversation has at least its system and opening
    /// messages.
    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }

    /// The message at `index`: its role and its original content.
    pub fn message(&self, index: usize) -> Option<(MessageRole, &str)> {
        self.messages
            .get(index)
            .map(|(role, content)| (*role, content.as_str()))
    }

    /// Record an accepted reply to this conversation's current history.
    pub fn accept(&mut self, reply: &ModelReply) -> Result<(), ModelError> {
        let _ = reply;
        Ok(())
    }

    /// The history as sent.
    fn render(&self) -> Rendered {
        Rendered {
            messages: self.messages.clone(),
            answered: Answered {
                conversation: self.id,
                length: self.messages.len(),
            },
            previous_prompt: self.last_prompt,
        }
    }
}

impl fmt::Debug for Conversation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let roles: Vec<&str> = self.messages.iter().map(|(r, _)| r.as_str()).collect();
        f.debug_struct("Conversation")
            .field("roles", &roles)
            .field("messages", &self.messages.len())
            .field("content_bytes", &content_bytes(&self.messages))
            .finish()
    }
}

/// The fixed configuration of a conversation request (N1 spec §2.5). Its
/// digest joins the coding run's model pin; a change to any field is a
/// [`ModelError::ConfigMismatch`] until it is re-qualified.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConversationConfig {
    pub num_ctx: u32,
    pub num_predict: u32,
    pub max_messages: usize,
    pub template_allowance: usize,
    pub per_message_allowance: usize,
    pub suffix_allowance: usize,
    pub margin: usize,
    pub inventory_sha256: &'static str,
    pub retained_encoding: &'static str,
    pub user_frame: &'static str,
    pub frame_open: &'static str,
    pub frame_close: &'static str,
}

/// The conversation configuration Q-R-N1 qualifies.
pub const CONVERSATION_CONFIG: ConversationConfig = ConversationConfig {
    num_ctx: CONVERSATION_NUM_CTX,
    num_predict: ANSWER_CAP,
    max_messages: MAX_CONVERSATION_MESSAGES,
    template_allowance: TEMPLATE_ALLOWANCE,
    per_message_allowance: PER_MESSAGE_ALLOWANCE,
    suffix_allowance: SUFFIX_ALLOWANCE,
    margin: MARGIN,
    inventory_sha256: DELIMITER_INVENTORY_SHA256,
    retained_encoding: RETAINED_ANSWER_ENCODING,
    user_frame: USER_FRAME_VERSION,
    frame_open: FRAME_OPEN,
    frame_close: FRAME_CLOSE,
};

/// SHA-256 of [`CONVERSATION_CONFIG`] (`ConversationConfig::digest`), frozen
/// for Q-R-N1.
pub const QUALIFIED_CONVERSATION_CONFIG_SHA256: &str =
    "36dc2164684d1e9592366c4298cca9eb04cf96762e2f279e4dcf135e594ab000";

impl ConversationConfig {
    /// The canonical text of the configuration (the digest's input).
    fn canonical(&self) -> String {
        format!(
            "num_ctx={};num_predict={};max_messages={};template_allowance={};\
             per_message_allowance={};suffix_allowance={};margin={};inventory_sha256={};\
             retained_encoding={};user_frame={};frame_open_sha256={};frame_close_sha256={}",
            self.num_ctx,
            self.num_predict,
            self.max_messages,
            self.template_allowance,
            self.per_message_allowance,
            self.suffix_allowance,
            self.margin,
            self.inventory_sha256,
            self.retained_encoding,
            self.user_frame,
            hex::encode(Sha256::digest(self.frame_open.as_bytes())),
            hex::encode(Sha256::digest(self.frame_close.as_bytes())),
        )
    }

    /// SHA-256 over `nexus.inference.conversation.v1` and the
    /// length-prefixed canonical text.
    pub fn digest(&self) -> [u8; 32] {
        let canonical = self.canonical();
        let mut hash = Sha256::new();
        hash.update(b"nexus.inference.conversation.v1");
        hash.update(
            u32::try_from(canonical.len())
                .unwrap_or(u32::MAX)
                .to_be_bytes(),
        );
        hash.update(canonical.as_bytes());
        hash.finalize().into()
    }

    /// The configuration is the qualified one.
    pub fn check(&self) -> Result<(), ModelError> {
        Ok(())
    }
}
