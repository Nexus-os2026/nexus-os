//! Custom webhook integration — configurable HTTP delivery with retry and HMAC signing.

use crate::config::{WebhookAuth, WebhookConfig};
use crate::error::IntegrationError;
use crate::events::Notification;
use crate::providers::{Integration, ProviderType};
use reqwest::blocking::Client;
use sha2::{Digest, Sha256};

pub struct WebhookIntegration {
    id: String,
    config: WebhookConfig,
    http: Client,
}

impl WebhookIntegration {
    pub fn new(id: String, config: WebhookConfig) -> Result<Self, IntegrationError> {
        if config.url.is_empty() {
            return Err(IntegrationError::MissingCredential {
                env_var: format!("webhook '{id}' url"),
            });
        }
        let timeout = std::time::Duration::from_millis(config.timeout_ms.max(1000));
        let http = Client::builder().timeout(timeout).build().map_err(|e| {
            IntegrationError::ConnectionError {
                provider: format!("webhook:{id}"),
                message: e.to_string(),
            }
        })?;
        Ok(Self { id, config, http })
    }

    fn resolve_auth_header(&self) -> Result<Option<(String, String)>, IntegrationError> {
        match &self.config.auth {
            None => Ok(None),
            Some(WebhookAuth::Bearer { token_env }) => {
                let token =
                    std::env::var(token_env).map_err(|_| IntegrationError::MissingCredential {
                        env_var: token_env.clone(),
                    })?;
                Ok(Some(("Authorization".into(), format!("Bearer {token}"))))
            }
            Some(WebhookAuth::Basic {
                username,
                password_env,
            }) => {
                let password = std::env::var(password_env).map_err(|_| {
                    IntegrationError::MissingCredential {
                        env_var: password_env.clone(),
                    }
                })?;
                use base64::Engine;
                let encoded = base64::engine::general_purpose::STANDARD
                    .encode(format!("{username}:{password}"));
                Ok(Some(("Authorization".into(), format!("Basic {encoded}"))))
            }
            Some(WebhookAuth::ApiKey { header, key_env }) => {
                let key =
                    std::env::var(key_env).map_err(|_| IntegrationError::MissingCredential {
                        env_var: key_env.clone(),
                    })?;
                Ok(Some((header.clone(), key)))
            }
            Some(WebhookAuth::HmacSignature { .. }) => {
                // HMAC is applied per-request to the body, not as a static header
                Ok(None)
            }
        }
    }

    fn compute_hmac_signature(&self, body: &str) -> Option<String> {
        if let Some(WebhookAuth::HmacSignature { secret_env }) = &self.config.auth {
            if let Ok(secret) = std::env::var(secret_env) {
                let mut hasher = Sha256::new();
                hasher.update(secret.as_bytes());
                hasher.update(body.as_bytes());
                let result = hasher.finalize();
                return Some(format!("sha256={}", hex::encode(result)));
            }
        }
        None
    }

    fn deliver_with_retry(&self, payload: &serde_json::Value) -> Result<(), IntegrationError> {
        let body_str = serde_json::to_string(payload)
            .map_err(|e| IntegrationError::Serialization(e.to_string()))?;

        let auth = self.resolve_auth_header()?;
        let hmac_sig = self.compute_hmac_signature(&body_str);

        let max_attempts = self.config.retry_count.max(1);
        let mut last_error = String::new();

        for attempt in 0..max_attempts {
            let mut request = match self.config.method.to_uppercase().as_str() {
                "PUT" => self.http.put(&self.config.url),
                "PATCH" => self.http.patch(&self.config.url),
                _ => self.http.post(&self.config.url),
            };

            request = request
                .header("Content-Type", "application/json")
                .header("X-Nexus-Delivery-Attempt", attempt.to_string())
                .body(body_str.clone());

            for (k, v) in &self.config.headers {
                request = request.header(k.as_str(), v.as_str());
            }

            if let Some((header, value)) = &auth {
                request = request.header(header.as_str(), value.as_str());
            }

            if let Some(sig) = &hmac_sig {
                request = request.header("X-Nexus-Signature", sig.as_str());
            }

            match request.send() {
                Ok(response) if response.status().is_success() => return Ok(()),
                Ok(response) => {
                    last_error = format!(
                        "HTTP {} — {}",
                        response.status(),
                        response.text().unwrap_or_default()
                    );
                }
                Err(e) => {
                    last_error = e.to_string();
                }
            }

            if attempt + 1 < max_attempts {
                std::thread::sleep(std::time::Duration::from_millis(200 * 2u64.pow(attempt)));
            }
        }

        Err(IntegrationError::WebhookDeliveryFailed {
            attempts: max_attempts,
            message: last_error,
        })
    }
}

impl Integration for WebhookIntegration {
    fn name(&self) -> &str {
        &self.id
    }

    fn provider_type(&self) -> ProviderType {
        ProviderType::CustomWebhook
    }

    fn send_notification(&self, message: &Notification) -> Result<(), IntegrationError> {
        let payload = serde_json::to_value(message)
            .map_err(|e| IntegrationError::Serialization(e.to_string()))?;
        self.deliver_with_retry(&payload)
    }

    fn send_webhook(&self, payload: &serde_json::Value) -> Result<(), IntegrationError> {
        self.deliver_with_retry(payload)
    }

    fn health_check(&self) -> Result<(), IntegrationError> {
        if self.config.url.is_empty() {
            return Err(IntegrationError::NotConfigured {
                provider: format!("webhook:{}", self.id),
            });
        }
        Ok(())
    }
}

/// Hex encoding helper (avoids adding the `hex` crate).
mod hex {
    pub fn encode(bytes: impl AsRef<[u8]>) -> String {
        bytes.as_ref().iter().map(|b| format!("{b:02x}")).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn test_config() -> WebhookConfig {
        WebhookConfig {
            enabled: true,
            url: "https://example.com/hook".into(),
            method: "POST".into(),
            headers: HashMap::new(),
            auth: None,
            events: vec!["*".into()],
            retry_count: 1,
            timeout_ms: 5000,
        }
    }

    #[test]
    fn webhook_health_check_ok() {
        let wh = WebhookIntegration::new("test".into(), test_config()).unwrap();
        assert!(wh.health_check().is_ok());
    }

    #[test]
    fn webhook_health_check_empty_url() {
        let mut cfg = test_config();
        cfg.url = String::new();
        let result = WebhookIntegration::new("test".into(), cfg);
        assert!(result.is_err());
    }

    #[test]
    fn hmac_signature_computed() {
        std::env::set_var("TEST_HMAC_SECRET", "mysecret");
        let mut cfg = test_config();
        cfg.auth = Some(WebhookAuth::HmacSignature {
            secret_env: "TEST_HMAC_SECRET".into(),
        });
        let wh = WebhookIntegration::new("hmac-test".into(), cfg).unwrap();
        let sig = wh.compute_hmac_signature("test body");
        assert!(sig.is_some());
        assert!(sig.unwrap().starts_with("sha256="));
        std::env::remove_var("TEST_HMAC_SECRET");
    }
}
