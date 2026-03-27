//! Minimal NIM API client for capability measurement.
//! Uses curl subprocess (same pattern as the benchmark).

use std::sync::Arc;

const NIM_ENDPOINT: &str = "https://api.groq.com/openai/v1/chat/completions";

/// NIM API client — thread-safe via Arc.
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

    /// Send a system + user prompt to NIM. Retries with backoff.
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
                Err(e) if attempt < 4 => {
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
        let out = std::process::Command::new("curl")
            .args(["-sS", "-L", "-m", "60"])
            .arg("-H")
            .arg(format!("authorization: Bearer {}", self.api_key))
            .arg("-H")
            .arg("content-type: application/json")
            .arg("-d")
            .arg(&encoded)
            .arg("-w")
            .arg(format!("\n{marker}%{{http_code}}"))
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

    /// Requires GROQ_API_KEY. Run with:
    /// GROQ_API_KEY=... cargo test -p nexus-capability-measurement -- test_real_nim --ignored --nocapture
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
