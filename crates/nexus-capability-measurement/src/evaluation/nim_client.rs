//! Minimal chat client for capability measurement.
//! Uses curl subprocess (same pattern as the benchmark).
//!
//! Despite its historical name, `NimClient` is a Groq client: every request
//! goes to the fixed Groq endpoint below, so its key must be a Groq key.

use std::sync::Arc;

const NIM_ENDPOINT: &str = "https://api.groq.com/openai/v1/chat/completions";

/// The only environment variable a [`NimClient`] key may come from.
pub const GROQ_API_KEY_VAR: &str = "GROQ_API_KEY";

/// P0-FINAL-GATE C5: the key for a [`NimClient`], which only ever posts to the
/// fixed Groq endpoint, comes from `GROQ_API_KEY` alone. Another provider's
/// key (NVIDIA NIM, OpenRouter) is never a fallback: sent to Groq it would
/// disclose that credential to a third party. `lookup` is asked for exactly
/// one variable, the Groq one.
pub fn groq_api_key(lookup: impl FnOnce(&str) -> Option<String>) -> Option<String> {
    lookup(GROQ_API_KEY_VAR)
}

/// [`groq_api_key`] read from the process environment.
pub fn groq_api_key_from_env() -> Option<String> {
    groq_api_key(|name| std::env::var(name).ok())
}

/// Groq chat client (historically named for NIM) — thread-safe via Arc.
pub struct NimClient {
    api_key: String,
    model: String,
}

impl NimClient {
    pub fn new(api_key: String, model: String) -> Self {
        Self { api_key, model }
    }

    /// Create an Arc-wrapped instance for sharing across adapters.
    pub fn shared(api_key: String, model: String) -> Arc<Self> {
        Arc::new(Self::new(api_key, model))
    }

    /// Send a system + user prompt to the Groq endpoint. Retries with backoff.
    pub fn query(
        &self,
        system_prompt: &str,
        user_prompt: &str,
        max_tokens: u32,
    ) -> Result<String, String> {
        for attempt in 0..5u32 {
            match self.query_inner(system_prompt, user_prompt, max_tokens) {
                Ok(text) => return Ok(text),
                Err(e) if e.contains("429") && attempt < 4 => {
                    // Rate limited — wait 3-12 seconds with exponential backoff
                    let wait = 3000 * 2u64.pow(attempt.min(2));
                    std::thread::sleep(std::time::Duration::from_millis(wait));
                    continue;
                }
                Err(_) if attempt < 4 => {
                    // Other error — short backoff
                    std::thread::sleep(std::time::Duration::from_millis(500 * 2u64.pow(attempt)));
                    continue;
                }
                Err(e) => return Err(e),
            }
        }
        Err("Exhausted retries".into())
    }

    fn query_inner(
        &self,
        system_prompt: &str,
        user_prompt: &str,
        max_tokens: u32,
    ) -> Result<String, String> {
        let body = serde_json::json!({
            "model": self.model,
            "messages": [
                {"role": "system", "content": system_prompt},
                {"role": "user", "content": user_prompt}
            ],
            "max_tokens": max_tokens,
            "temperature": 0.7,
            "stream": false
        });

        let encoded = serde_json::to_string(&body).map_err(|e| format!("json: {e}"))?;

        let marker = "__NX_CM__:";
        // P0-002C5B: HTTPS only, no URL globbing, a literal body, and the
        // fixed endpoint after `--`.
        let out = std::process::Command::new("curl")
            .args([
                "-q",
                "--globoff",
                "--proto",
                "=https",
                "--proto-redir",
                "=https",
            ])
            .args(["-sS", "-L", "-m", "60", "--max-filesize", "10485760"])
            .arg("-H")
            .arg(format!("authorization: Bearer {}", self.api_key))
            .arg("-H")
            .arg("content-type: application/json")
            .arg("--data-raw")
            .arg(&encoded)
            .arg("-w")
            .arg(format!("\n{marker}%{{http_code}}"))
            .arg("--")
            .arg(NIM_ENDPOINT)
            .output()
            .map_err(|e| format!("curl: {e}"))?;

        if !out.status.success() {
            return Err("curl failed".into());
        }

        let raw = String::from_utf8(out.stdout).map_err(|e| format!("utf8: {e}"))?;
        let (body_raw, status_raw) = raw.rsplit_once(marker).ok_or("no status marker")?;
        let status: u16 = status_raw
            .trim()
            .parse()
            .map_err(|e| format!("status: {e}"))?;

        if !(200..300).contains(&status) {
            return Err(format!("Groq status {status}"));
        }

        let payload: serde_json::Value =
            serde_json::from_str(body_raw.trim()).map_err(|e| format!("parse: {e}"))?;

        let text = payload
            .get("choices")
            .and_then(|v| v.as_array())
            .and_then(|a| a.first())
            .and_then(|c| c.get("message"))
            .and_then(|m| m.get("content"))
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();

        if text.trim().is_empty() {
            return Err("Empty response".into());
        }

        Ok(text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// P0-FINAL-GATE C5: a client for the Groq endpoint takes only the Groq
    /// key. With only an NVIDIA NIM or OpenRouter key available there is no
    /// key at all, and the lookup is never asked for another provider's.
    #[test]
    fn p0_fg_groq_client_key_never_falls_back_to_another_provider() {
        let other_providers = |name: &str| match name {
            "NVIDIA_NIM_API_KEY" => Some("nvapi-sentinel".to_string()),
            "OPENROUTER_API_KEY" => Some("sk-or-sentinel".to_string()),
            _ => None,
        };
        let mut asked = Vec::new();
        let key = groq_api_key(|name| {
            asked.push(name.to_string());
            other_providers(name)
        });
        assert_eq!(key, None);
        assert_eq!(asked, [GROQ_API_KEY_VAR]);
        assert_eq!(GROQ_API_KEY_VAR, "GROQ_API_KEY");

        // The Groq key itself is used when present, and only it.
        let key = groq_api_key(|name| match name {
            "GROQ_API_KEY" => Some("gsk-sentinel".to_string()),
            other => other_providers(other),
        });
        assert_eq!(key.as_deref(), Some("gsk-sentinel"));

        // The client posts nowhere but the Groq endpoint.
        assert_eq!(
            NIM_ENDPOINT,
            "https://api.groq.com/openai/v1/chat/completions"
        );
    }

    /// Sends a real request to a Groq-hosted NIM model and verifies the response.
    /// Ignored in fast CI: requires a live GROQ_API_KEY and network access.
    /// Run manually: GROQ_API_KEY=... cargo test -p nexus-capability-measurement -- test_real_nim --ignored --nocapture
    #[test]
    #[ignore]
    fn test_real_nim_adapter_single_agent() {
        let api_key = std::env::var("GROQ_API_KEY").expect("Set GROQ_API_KEY");
        let nim = NimClient::new(api_key, "llama-3.1-8b-instant".into());

        let response = nim
            .query(
                "You are a helpful assistant.",
                "What is 2 + 2? Answer in one word.",
                50,
            )
            .expect("NIM call failed");

        assert!(!response.trim().is_empty());
        eprintln!("NIM response: {}", response.trim());
    }
}
