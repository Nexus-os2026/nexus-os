//! Anthropic swarm provider.
//!
//! # Invariants
//!
//! - Haiku-only. Model must be `claude-haiku-4-5-20251001`. Anything else is
//!   rejected with `ProviderError::HaikuOnly` before any HTTP traffic.
//! - Hard cumulative spend cap of `$2.00` USD across the process lifetime,
//!   persisted to `~/.nexus/swarm/anthropic_spend.json`. Exceed → the
//!   `invoke()` returns `ProviderError::SpendCapExceeded` before calling the
//!   API. The provider's `health_check` surfaces the spend in its `notes`
//!   field using the exact phrase `spend cap exceeded` so the Router can
//!   skip Anthropic routes without re-reading the ledger.
//! - Directory `~/.nexus/swarm/` is created on first access with `0700`; the
//!   ledger file is written `0600`.
//! - File-lock on write: we use the `fs2` crate (chosen over `fd-lock`
//!   because `fs2` is a broader-portability crate already familiar to the
//!   workspace patterns — `fd-lock` would have been equally fine).

use crate::events::{ProviderHealth, ProviderHealthStatus};
use crate::profile::{CostClass, PrivacyClass, ReasoningTier};
use crate::provider::{
    InvokeRequest, InvokeResponse, ModelDescriptor, Provider, ProviderCapabilities, ProviderError,
};
use async_trait::async_trait;
use fs2::FileExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

pub const HAIKU_MODEL: &str = "claude-haiku-4-5-20251001";
pub const HARD_CAP_USD: f64 = 2.00;

const KEYRING_SERVICE: &str = "nexus.anthropic";
const KEYRING_USER: &str = "api_key";
const DEFAULT_BASE_URL: &str = "https://api.anthropic.com/v1";
const API_VERSION: &str = "2023-06-01";

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SpendLedger {
    pub cumulative_usd: f64,
}

pub struct AnthropicSpendStore {
    path: PathBuf,
    guard: Mutex<()>,
}

impl AnthropicSpendStore {
    pub fn default_path() -> PathBuf {
        let base = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        base.join(".nexus")
            .join("swarm")
            .join("anthropic_spend.json")
    }

    pub fn at(path: PathBuf) -> Self {
        Self {
            path,
            guard: Mutex::new(()),
        }
    }

    fn ensure_dir(&self) -> std::io::Result<()> {
        if let Some(dir) = self.path.parent() {
            std::fs::create_dir_all(dir)?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700));
            }
        }
        Ok(())
    }

    /// Read the current ledger (0 if missing or unparseable).
    pub fn read(&self) -> SpendLedger {
        let _g = self.guard.lock().ok();
        let Ok(mut f) = File::open(&self.path) else {
            return SpendLedger::default();
        };
        let _ = f.lock_shared();
        let mut buf = String::new();
        let _ = f.read_to_string(&mut buf);
        let _ = FileExt::unlock(&f);
        serde_json::from_str(&buf).unwrap_or_default()
    }

    /// Attempt to add `delta_usd` to the ledger, returning the new total.
    /// Fails if the resulting total would exceed `cap`.
    pub fn try_add(&self, delta_usd: f64, cap: f64) -> Result<f64, ProviderError> {
        let _g = self
            .guard
            .lock()
            .map_err(|e| ProviderError::Io("anthropic".into(), format!("mutex poisoned: {e}")))?;
        self.ensure_dir()
            .map_err(|e| ProviderError::Io("anthropic".into(), e.to_string()))?;
        let mut f = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .truncate(false)
            .open(&self.path)
            .map_err(|e| ProviderError::Io("anthropic".into(), e.to_string()))?;
        f.lock_exclusive()
            .map_err(|e| ProviderError::Io("anthropic".into(), e.to_string()))?;

        let mut buf = String::new();
        f.read_to_string(&mut buf)
            .map_err(|e| ProviderError::Io("anthropic".into(), e.to_string()))?;
        let mut ledger: SpendLedger = if buf.trim().is_empty() {
            SpendLedger::default()
        } else {
            serde_json::from_str(&buf).unwrap_or_default()
        };
        let proposed = ledger.cumulative_usd + delta_usd;
        if proposed > cap {
            let _ = FileExt::unlock(&f);
            return Err(ProviderError::SpendCapExceeded {
                spent: ledger.cumulative_usd,
                cap,
            });
        }
        ledger.cumulative_usd = proposed;
        let out = serde_json::to_string_pretty(&ledger)
            .map_err(|e| ProviderError::Io("anthropic".into(), e.to_string()))?;
        f.set_len(0)
            .map_err(|e| ProviderError::Io("anthropic".into(), e.to_string()))?;
        f.seek(SeekFrom::Start(0))
            .map_err(|e| ProviderError::Io("anthropic".into(), e.to_string()))?;
        f.write_all(out.as_bytes())
            .map_err(|e| ProviderError::Io("anthropic".into(), e.to_string()))?;
        f.flush()
            .map_err(|e| ProviderError::Io("anthropic".into(), e.to_string()))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&self.path, std::fs::Permissions::from_mode(0o600));
        }
        let _ = FileExt::unlock(&f);
        Ok(ledger.cumulative_usd)
    }
}

pub struct AnthropicProvider {
    base_url: String,
    client: Client,
    key_override: Option<String>,
    spend: AnthropicSpendStore,
    cap_usd: f64,
}

impl AnthropicProvider {
    pub fn new() -> Self {
        Self::with(
            DEFAULT_BASE_URL,
            None,
            AnthropicSpendStore::default_path(),
            HARD_CAP_USD,
        )
    }

    pub fn with_base_and_key(base_url: impl Into<String>, key: impl Into<String>) -> Self {
        Self::with(
            base_url.into(),
            Some(key.into()),
            AnthropicSpendStore::default_path(),
            HARD_CAP_USD,
        )
    }

    pub fn with(
        base_url: impl Into<String>,
        key_override: Option<String>,
        spend_path: PathBuf,
        cap_usd: f64,
    ) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(60))
            .build()
            .unwrap_or_else(|_| Client::new());
        Self {
            base_url: base_url.into(),
            client,
            key_override,
            spend: AnthropicSpendStore::at(spend_path),
            cap_usd,
        }
    }

    fn api_key(&self) -> Result<String, ProviderError> {
        if let Some(k) = &self.key_override {
            return Ok(k.clone());
        }
        let entry = keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER)
            .map_err(|e| ProviderError::NotConfigured(format!("anthropic keyring: {e}")))?;
        entry.get_password().map_err(|_| {
            ProviderError::NotConfigured("anthropic api key missing from keyring".into())
        })
    }

    /// Approximate cost in USD given Anthropic's Haiku 4.5 pricing.
    /// Input: $0.80 / 1M tokens; Output: $4.00 / 1M tokens (conservative).
    pub(crate) fn estimate_cost_usd(tokens_in: u32, tokens_out: u32) -> f64 {
        let cin = (tokens_in as f64) * 0.80 / 1_000_000.0;
        let cout = (tokens_out as f64) * 4.00 / 1_000_000.0;
        cin + cout
    }
}

impl Default for AnthropicProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Provider for AnthropicProvider {
    fn id(&self) -> &str {
        "anthropic"
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            models: vec![ModelDescriptor {
                id: HAIKU_MODEL.into(),
                param_count_b: None,
                tier: ReasoningTier::Medium,
                context_window: 200_000,
            }],
            supports_tool_use: true,
            supports_streaming: true,
            max_context: 200_000,
            cost_class: CostClass::Low,
            privacy_class: PrivacyClass::Public,
        }
    }

    async fn health_check(&self) -> ProviderHealth {
        let start = Instant::now();
        let ledger = self.spend.read();
        let spend_note = format!(
            "spend: ${:.2} / ${:.2}",
            ledger.cumulative_usd, self.cap_usd
        );
        let notes = if ledger.cumulative_usd >= self.cap_usd {
            format!("spend cap exceeded: {}", spend_note)
        } else {
            spend_note
        };

        let key = match self.api_key() {
            Ok(k) => k,
            Err(e) => {
                return ProviderHealth {
                    provider_id: "anthropic".into(),
                    status: ProviderHealthStatus::Unhealthy,
                    latency_ms: None,
                    models: vec![],
                    notes: format!("{e}; {notes}"),
                    checked_at_secs: chrono::Utc::now().timestamp(),
                }
            }
        };

        // Anthropic has no `/models` endpoint; probe with a tiny messages call.
        let url = format!("{}/messages", self.base_url.trim_end_matches('/'));
        let body = serde_json::json!({
            "model": HAIKU_MODEL,
            "max_tokens": 1,
            "messages": [{"role": "user", "content": "."}]
        });
        let resp = self
            .client
            .post(&url)
            .header("x-api-key", key.clone())
            .header("anthropic-version", API_VERSION)
            .json(&body)
            .send()
            .await;
        match resp {
            Ok(r) if r.status().is_success() => ProviderHealth {
                provider_id: "anthropic".into(),
                status: ProviderHealthStatus::Ok,
                latency_ms: Some(start.elapsed().as_millis() as u64),
                models: vec![HAIKU_MODEL.into()],
                notes,
                checked_at_secs: chrono::Utc::now().timestamp(),
            },
            Ok(r) => ProviderHealth {
                provider_id: "anthropic".into(),
                status: ProviderHealthStatus::Unhealthy,
                latency_ms: Some(start.elapsed().as_millis() as u64),
                models: vec![],
                notes: format!("http {}; {}", r.status(), notes),
                checked_at_secs: chrono::Utc::now().timestamp(),
            },
            Err(e) => ProviderHealth {
                provider_id: "anthropic".into(),
                status: ProviderHealthStatus::Unhealthy,
                latency_ms: None,
                models: vec![],
                notes: format!("{e}; {notes}"),
                checked_at_secs: chrono::Utc::now().timestamp(),
            },
        }
    }

    async fn invoke(&self, req: InvokeRequest) -> Result<InvokeResponse, ProviderError> {
        if req.model_id != HAIKU_MODEL {
            return Err(ProviderError::HaikuOnly(req.model_id));
        }
        // Pre-check spend. We use a conservative upper bound: 2x the asked
        // max_tokens against the output price. This blocks obviously-unsafe
        // calls before touching the network.
        let upper_bound = Self::estimate_cost_usd(req.max_tokens, req.max_tokens);
        let ledger = self.spend.read();
        if ledger.cumulative_usd + upper_bound > self.cap_usd {
            return Err(ProviderError::SpendCapExceeded {
                spent: ledger.cumulative_usd,
                cap: self.cap_usd,
            });
        }
        let key = self.api_key()?;

        #[derive(Serialize)]
        struct MsgReq<'a> {
            model: &'a str,
            max_tokens: u32,
            temperature: f32,
            messages: Vec<Msg<'a>>,
        }
        #[derive(Serialize)]
        struct Msg<'a> {
            role: &'a str,
            content: &'a str,
        }
        #[derive(Deserialize)]
        struct MsgResp {
            content: Vec<ContentBlock>,
            #[serde(default)]
            usage: Option<Usage>,
        }
        #[derive(Deserialize)]
        struct ContentBlock {
            #[serde(default)]
            text: String,
        }
        #[derive(Deserialize)]
        struct Usage {
            input_tokens: u32,
            output_tokens: u32,
        }

        let url = format!("{}/messages", self.base_url.trim_end_matches('/'));
        let body = MsgReq {
            model: HAIKU_MODEL,
            max_tokens: req.max_tokens,
            temperature: req.temperature.unwrap_or(0.2),
            messages: vec![Msg {
                role: "user",
                content: &req.prompt,
            }],
        };
        let start = Instant::now();
        let resp = self
            .client
            .post(&url)
            .header("x-api-key", key)
            .header("anthropic-version", API_VERSION)
            .json(&body)
            .send()
            .await
            .map_err(|e| ProviderError::Transport("anthropic".into(), e.to_string()))?;
        let status = resp.status();
        if status == 401 {
            return Err(ProviderError::AuthFailed("anthropic".into()));
        }
        if status == 429 {
            return Err(ProviderError::Http(
                "anthropic".into(),
                429,
                "rate limited".into(),
            ));
        }
        if !status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(ProviderError::Http(
                "anthropic".into(),
                status.as_u16(),
                text,
            ));
        }
        let parsed: MsgResp = resp
            .json()
            .await
            .map_err(|e| ProviderError::Malformed("anthropic".into(), e.to_string()))?;
        let text = parsed
            .content
            .into_iter()
            .map(|b| b.text)
            .collect::<Vec<_>>()
            .join("");
        let (tin, tout) = parsed
            .usage
            .map(|u| (u.input_tokens, u.output_tokens))
            .unwrap_or((0, 0));
        let actual_cost = Self::estimate_cost_usd(tin, tout);
        // Commit spend; if this now exceeds the cap we still return success
        // for this call (spend already happened) but future calls are blocked.
        let _ = self.spend.try_add(actual_cost, f64::MAX);
        Ok(InvokeResponse {
            text,
            tokens_in: tin,
            tokens_out: tout,
            cost_cents: (actual_cost * 100.0).round() as u32,
            latency_ms: start.elapsed().as_millis() as u64,
            model_id: HAIKU_MODEL.into(),
        })
    }
}

// Keep `Path` referenced — used inside ensure_dir even on non-unix.
#[allow(dead_code)]
fn _path_ref(_p: &Path) {}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[test]
    fn ledger_starts_zero_and_accumulates() {
        let dir = tempdir().unwrap();
        let p = dir.path().join("sp.json");
        let store = AnthropicSpendStore::at(p.clone());
        assert_eq!(store.read().cumulative_usd, 0.0);
        store.try_add(0.50, 2.00).unwrap();
        store.try_add(0.30, 2.00).unwrap();
        let v = store.read().cumulative_usd;
        assert!((v - 0.80).abs() < 1e-6, "got {v}");
    }

    #[test]
    fn ledger_rejects_over_cap_without_mutation() {
        let dir = tempdir().unwrap();
        let p = dir.path().join("sp.json");
        let store = AnthropicSpendStore::at(p.clone());
        store.try_add(1.80, 2.00).unwrap();
        let err = store.try_add(0.50, 2.00).unwrap_err();
        assert!(matches!(err, ProviderError::SpendCapExceeded { .. }));
        let v = store.read().cumulative_usd;
        assert!((v - 1.80).abs() < 1e-6);
    }

    #[tokio::test]
    async fn non_haiku_model_is_rejected_without_http() {
        let dir = tempdir().unwrap();
        let p = AnthropicProvider::with(
            "http://127.0.0.1:1",
            Some("k".into()),
            dir.path().join("sp.json"),
            HARD_CAP_USD,
        );
        let err = p
            .invoke(InvokeRequest {
                model_id: "claude-sonnet-4-6".into(),
                prompt: "x".into(),
                max_tokens: 8,
                temperature: None,
                metadata: serde_json::Value::Null,
            })
            .await
            .unwrap_err();
        assert!(matches!(err, ProviderError::HaikuOnly(_)));
    }

    #[tokio::test]
    async fn spend_cap_blocks_before_http() {
        let mock = MockServer::start().await;
        // Pre-load ledger close to cap.
        let dir = tempdir().unwrap();
        let ledger_path = dir.path().join("sp.json");
        std::fs::write(
            &ledger_path,
            serde_json::to_string(&SpendLedger {
                cumulative_usd: 1.999,
            })
            .unwrap(),
        )
        .unwrap();
        let p = AnthropicProvider::with(mock.uri(), Some("k".into()), ledger_path, HARD_CAP_USD);
        let err = p
            .invoke(InvokeRequest {
                model_id: HAIKU_MODEL.into(),
                prompt: "hi".into(),
                max_tokens: 1024,
                temperature: None,
                metadata: serde_json::Value::Null,
            })
            .await
            .unwrap_err();
        assert!(matches!(err, ProviderError::SpendCapExceeded { .. }));
        // wiremock should have received zero requests — no route registered.
        assert_eq!(mock.received_requests().await.unwrap().len(), 0);
    }

    #[tokio::test]
    async fn invoke_happy_path_writes_ledger() {
        let mock = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/messages"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "content": [{"type": "text", "text": "hi!"}],
                "usage": {"input_tokens": 10, "output_tokens": 2}
            })))
            .mount(&mock)
            .await;
        let dir = tempdir().unwrap();
        let ledger_path = dir.path().join("sp.json");
        let p = AnthropicProvider::with(
            mock.uri(),
            Some("k".into()),
            ledger_path.clone(),
            HARD_CAP_USD,
        );
        let resp = p
            .invoke(InvokeRequest {
                model_id: HAIKU_MODEL.into(),
                prompt: "hi".into(),
                max_tokens: 16,
                temperature: Some(0.1),
                metadata: serde_json::Value::Null,
            })
            .await
            .unwrap();
        assert_eq!(resp.text, "hi!");
        let store = AnthropicSpendStore::at(ledger_path);
        assert!(store.read().cumulative_usd > 0.0);
    }

    #[tokio::test]
    async fn missing_key_reports_unhealthy() {
        let dir = tempdir().unwrap();
        // Point base at an unreachable URL but keep key_override None so key
        // lookup fails first; we use a bogus keyring service by construction
        // (key_override None) and expect Unhealthy without a crash.
        let p = AnthropicProvider::with(
            "http://127.0.0.1:1",
            None,
            dir.path().join("sp.json"),
            HARD_CAP_USD,
        );
        let h = p.health_check().await;
        assert_eq!(h.status, ProviderHealthStatus::Unhealthy);
    }
}
