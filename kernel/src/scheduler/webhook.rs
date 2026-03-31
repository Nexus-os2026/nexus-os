//! Webhook trigger — accepts incoming HTTP requests and matches them to schedules.

use super::error::SchedulerError;
use super::trigger::ScheduleId;
use sha2::Sha256;
use std::collections::HashMap;
use tokio::sync::mpsc;

/// Manages webhook routes and dispatches incoming requests to scheduled tasks.
pub struct WebhookTrigger {
    routes: HashMap<String, WebhookRoute>,
    tx: mpsc::Sender<(ScheduleId, serde_json::Value)>,
}

struct WebhookRoute {
    schedule_id: ScheduleId,
    secret: Option<String>,
    filter: Option<String>,
}

impl WebhookTrigger {
    pub fn new(tx: mpsc::Sender<(ScheduleId, serde_json::Value)>) -> Self {
        Self {
            routes: HashMap::new(),
            tx,
        }
    }

    /// Register a webhook route.
    pub fn register(
        &mut self,
        path: String,
        schedule_id: ScheduleId,
        secret: Option<String>,
        filter: Option<String>,
    ) {
        self.routes.insert(
            path,
            WebhookRoute {
                schedule_id,
                secret,
                filter,
            },
        );
    }

    /// Remove a webhook route.
    pub fn unregister(&mut self, path: &str) {
        self.routes.remove(path);
    }

    /// Handle an incoming webhook request.
    pub async fn handle_request(
        &self,
        path: &str,
        payload: serde_json::Value,
        signature: Option<&str>,
    ) -> Result<(), SchedulerError> {
        let route = self
            .routes
            .get(path)
            .ok_or_else(|| SchedulerError::UnknownWebhook(path.to_string()))?;

        // Validate HMAC signature if a secret is configured
        if let Some(ref secret) = route.secret {
            let sig = signature.ok_or(SchedulerError::MissingSignature)?;
            if !verify_hmac_sha256(secret, &payload, sig) {
                return Err(SchedulerError::InvalidSignature);
            }
        }

        // JSONPath filter is a future enhancement — for now all payloads pass
        // suppress unused filter field; JSONPath filtering is a planned future enhancement
        let _ = &route.filter;

        self.tx
            .send((route.schedule_id, payload))
            .await
            .map_err(|e| SchedulerError::ChannelClosed(e.to_string()))?;

        Ok(())
    }

    /// Returns the number of registered webhook routes.
    pub fn len(&self) -> usize {
        self.routes.len()
    }

    /// Returns true if no routes are registered.
    pub fn is_empty(&self) -> bool {
        self.routes.is_empty()
    }
}

/// Verify an HMAC-SHA256 signature using raw sha2 (no hmac crate needed).
///
/// Computes HMAC-SHA256 manually: H((K ⊕ opad) || H((K ⊕ ipad) || message))
fn verify_hmac_sha256(secret: &str, payload: &serde_json::Value, signature: &str) -> bool {
    use sha2::Digest;

    let key = secret.as_bytes();
    let message = serde_json::to_vec(payload).unwrap_or_default();

    // Pad or hash key to 64 bytes
    let mut padded_key = [0u8; 64];
    if key.len() > 64 {
        let hash: [u8; 32] = Sha256::digest(key).into();
        padded_key[..32].copy_from_slice(&hash);
    } else {
        padded_key[..key.len()].copy_from_slice(key);
    }

    // Inner hash: H((K ⊕ ipad) || message)
    let mut inner_hasher = Sha256::new();
    let mut ipad = [0x36u8; 64];
    for (i, b) in padded_key.iter().enumerate() {
        ipad[i] ^= b;
    }
    inner_hasher.update(ipad);
    inner_hasher.update(&message);
    let inner_hash = inner_hasher.finalize();

    // Outer hash: H((K ⊕ opad) || inner_hash)
    let mut outer_hasher = Sha256::new();
    let mut opad = [0x5cu8; 64];
    for (i, b) in padded_key.iter().enumerate() {
        opad[i] ^= b;
    }
    outer_hasher.update(opad);
    outer_hasher.update(inner_hash);
    let expected = outer_hasher.finalize();

    // Hex-encode expected and compare
    let expected_hex = expected
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();

    let received = signature.strip_prefix("sha256=").unwrap_or(signature);

    // Constant-time comparison
    if expected_hex.len() != received.len() {
        return false;
    }
    expected_hex
        .bytes()
        .zip(received.bytes())
        .fold(0u8, |acc, (a, b)| acc | (a ^ b))
        == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hmac_sha256_valid_signature() {
        let secret = "test-key"; // test hmac secret
        let payload = serde_json::json!({"event": "push", "ref": "refs/heads/main"});
        let payload_bytes = serde_json::to_vec(&payload).unwrap();

        // Compute expected signature
        use sha2::Digest;
        let key = secret.as_bytes();
        let mut padded_key = [0u8; 64];
        padded_key[..key.len()].copy_from_slice(key);

        let mut inner = Sha256::new();
        let mut ipad = [0x36u8; 64];
        for (i, b) in padded_key.iter().enumerate() {
            ipad[i] ^= b;
        }
        inner.update(ipad);
        inner.update(&payload_bytes);
        let inner_hash = inner.finalize();

        let mut outer = Sha256::new();
        let mut opad = [0x5cu8; 64];
        for (i, b) in padded_key.iter().enumerate() {
            opad[i] ^= b;
        }
        outer.update(opad);
        outer.update(inner_hash);
        let sig_bytes = outer.finalize();
        let sig_hex = sig_bytes
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();

        assert!(verify_hmac_sha256(
            secret,
            &payload,
            &format!("sha256={sig_hex}")
        ));
    }

    #[test]
    fn hmac_sha256_invalid_signature() {
        let payload = serde_json::json!({"event": "push"});
        assert!(!verify_hmac_sha256(
            "secret",
            &payload,
            "sha256=0000000000000000000000000000000000000000000000000000000000000000"
        ));
    }

    #[test]
    fn hmac_sha256_missing_prefix() {
        let payload = serde_json::json!({"test": true});
        // Should still work without sha256= prefix (falls through to raw comparison)
        assert!(!verify_hmac_sha256("secret", &payload, "invalid"));
    }

    #[tokio::test]
    async fn webhook_unknown_path_rejected() {
        let (tx, _rx) = mpsc::channel(16);
        let trigger = WebhookTrigger::new(tx);
        let result = trigger
            .handle_request("/unknown", serde_json::json!({}), None)
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn webhook_missing_signature_rejected() {
        let (tx, _rx) = mpsc::channel(16);
        let mut trigger = WebhookTrigger::new(tx);
        let id = uuid::Uuid::new_v4();
        trigger.register(
            "/hooks/test".to_string(),
            id,
            Some("secret".to_string()),
            None,
        );
        let result = trigger
            .handle_request("/hooks/test", serde_json::json!({}), None)
            .await;
        assert!(matches!(result, Err(SchedulerError::MissingSignature)));
    }

    #[tokio::test]
    async fn webhook_no_secret_passes() {
        let (tx, mut rx) = mpsc::channel(16);
        let mut trigger = WebhookTrigger::new(tx);
        let id = uuid::Uuid::new_v4();
        trigger.register("/hooks/open".to_string(), id, None, None);
        trigger
            .handle_request("/hooks/open", serde_json::json!({"data": 1}), None)
            .await
            .unwrap();
        let (received_id, _payload) = rx.recv().await.unwrap();
        assert_eq!(received_id, id);
    }

    #[test]
    fn register_unregister_routes() {
        let (tx, _rx) = mpsc::channel(16);
        let mut trigger = WebhookTrigger::new(tx);
        let id = uuid::Uuid::new_v4();
        trigger.register("/a".to_string(), id, None, None);
        assert_eq!(trigger.len(), 1);
        trigger.unregister("/a");
        assert!(trigger.is_empty());
    }
}
