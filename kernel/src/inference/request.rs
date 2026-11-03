//! The request and reply of one model call.
//!
//! M2e-A1 interface stub: the inherited behaviour. The coding-run transport
//! sends any messages whatever their size, with `format: "json"`, and its
//! messages derive `Debug` (F3); this stub keeps a schema's text as given,
//! bounds nothing and derives `Debug`.

use super::ModelError;

/// The bound on the complete rendered content of a request (not applied by
/// this stub).
pub const MAX_RENDERED_CONTENT_BYTES: usize = 3_936;

/// A JSON schema for the reply (stub: the text as given).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResponseSchema {
    text: String,
}

impl ResponseSchema {
    pub fn new(json: &str) -> Result<Self, ModelError> {
        Ok(Self {
            text: json.to_string(),
        })
    }

    pub(super) fn as_json(&self) -> &str {
        &self.text
    }
}

/// One request: a system and a user message, and the reply schema.
#[derive(Debug)]
pub struct ModelRequest {
    system: String,
    user: String,
    format: ResponseSchema,
}

impl ModelRequest {
    pub fn new(system: String, user: String, format: ResponseSchema) -> Result<Self, ModelError> {
        Ok(Self {
            system,
            user,
            format,
        })
    }

    pub fn rendered_bytes(&self) -> usize {
        self.system.len().saturating_add(self.user.len())
    }

    pub(super) fn system(&self) -> &str {
        &self.system
    }

    pub(super) fn user(&self) -> &str {
        &self.user
    }

    #[cfg(test)]
    pub(super) fn unchecked(system: String, user: String, format: ResponseSchema) -> Self {
        Self {
            system,
            user,
            format,
        }
    }
}

/// A reply.
#[derive(Debug)]
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
