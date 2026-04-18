//! The Governance Oracle — the ONLY interface agents interact with.
//!
//! Agents submit capability requests and receive sealed tokens. They learn
//! nothing about the decision process, timing, or governance rules.

use nexus_crypto::{CryptoIdentity, SignatureAlgorithm};
use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, oneshot};
use uuid::Uuid;

use std::time::Duration;

use crate::timing::TimingConfig;

/// What an agent submits.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityRequest {
    pub agent_id: String,
    pub capability: String,
    pub parameters: serde_json::Value,
    pub budget_hash: String,
    pub request_nonce: String,
}

/// Opaque sealed token returned to the agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SealedToken {
    pub payload: Vec<u8>,
    pub signature: Vec<u8>,
    pub token_id: String,
}

/// Contents of a sealed token (only readable by authorized verifiers).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenPayload {
    pub decision: GovernanceDecision,
    pub nonce: String,
    pub timestamp: u64,
    pub governance_version: String,
    pub request_nonce: String,
    pub agent_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum GovernanceDecision {
    Approved {
        capability_token: String,
    },
    /// Denied — NO reason provided (reasons would leak governance logic).
    Denied,
}

/// Internal request wrapper with response channel.
pub struct OracleRequest {
    pub request: CapabilityRequest,
    pub response_tx: oneshot::Sender<GovernanceDecision>,
}

/// Oracle error.
#[derive(Debug, thiserror::Error)]
pub enum OracleError {
    #[error("Decision engine unavailable")]
    EngineUnavailable,
    #[error("Decision timed out within response ceiling")]
    DecisionTimeout,
    #[error("Token sealing error: {0}")]
    SealingError(String),
    #[error("Invalid token signature")]
    InvalidSignature,
    #[error("Invalid token payload: {0}")]
    InvalidPayload(String),
}

/// The Governance Oracle — sealed submission interface.
pub struct GovernanceOracle {
    request_tx: mpsc::Sender<OracleRequest>,
    timing: TimingConfig,
    identity: CryptoIdentity,
    requests_processed: std::sync::atomic::AtomicU64,
    started_at: std::time::Instant,
}

impl GovernanceOracle {
    /// Create a new oracle with a freshly generated Ed25519 identity.
    /// Behavior preserved byte-for-byte with the pre-1.5a.1 API.
    pub fn new(request_tx: mpsc::Sender<OracleRequest>, response_ceiling: Duration) -> Self {
        let identity = CryptoIdentity::generate(SignatureAlgorithm::Ed25519)
            .expect("Ed25519 key generation should never fail");
        Self::with_identity(request_tx, response_ceiling, identity)
    }

    /// Create a new oracle with a caller-supplied identity. The caller owns
    /// identity lifecycle (persistence, rotation, ephemeral-vs-durable). This
    /// is the constructor `OracleRuntime` uses to bind the oracle to the
    /// keypair stored at `~/.nexus/oracle_identity.key`, so sealed tokens
    /// remain verifiable across app restarts.
    pub fn with_identity(
        request_tx: mpsc::Sender<OracleRequest>,
        response_ceiling: Duration,
        identity: CryptoIdentity,
    ) -> Self {
        Self {
            request_tx,
            timing: TimingConfig {
                response_ceiling,
                ..TimingConfig::default()
            },
            identity,
            requests_processed: std::sync::atomic::AtomicU64::new(0),
            started_at: std::time::Instant::now(),
        }
    }

    /// Submit a capability request. ALWAYS takes >= `response_ceiling` duration.
    pub async fn submit_request(
        &self,
        request: CapabilityRequest,
    ) -> Result<SealedToken, OracleError> {
        let start = tokio::time::Instant::now();

        let (response_tx, response_rx) = oneshot::channel();

        self.request_tx
            .send(OracleRequest {
                request: request.clone(),
                response_tx,
            })
            .await
            .map_err(|_| OracleError::EngineUnavailable)?;

        let decision_timeout = self
            .timing
            .response_ceiling
            .checked_sub(Duration::from_millis(5))
            .unwrap_or(self.timing.response_ceiling);

        let decision = tokio::time::timeout(decision_timeout, response_rx)
            .await
            .map_err(|_| OracleError::DecisionTimeout)?
            .map_err(|_| OracleError::EngineUnavailable)?;

        let token = self.seal_decision(decision, &request)?;

        // Pad to constant time
        let elapsed = start.elapsed();
        let wait = self.timing.wait_duration(elapsed);
        tokio::time::sleep(wait).await;

        self.requests_processed
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        Ok(token)
    }

    /// Seal a decision into an opaque, signed token.
    fn seal_decision(
        &self,
        decision: GovernanceDecision,
        request: &CapabilityRequest,
    ) -> Result<SealedToken, OracleError> {
        let payload = TokenPayload {
            decision,
            nonce: Uuid::new_v4().to_string(),
            timestamp: epoch_secs(),
            governance_version: String::new(),
            request_nonce: request.request_nonce.clone(),
            agent_id: request.agent_id.clone(),
        };

        let payload_bytes =
            serde_json::to_vec(&payload).map_err(|e| OracleError::SealingError(e.to_string()))?;

        let signature = self
            .identity
            .sign(&payload_bytes)
            .map_err(|e| OracleError::SealingError(e.to_string()))?;

        Ok(SealedToken {
            payload: payload_bytes,
            signature,
            token_id: Uuid::new_v4().to_string(),
        })
    }

    /// Verify a sealed token is authentic and untampered.
    pub fn verify_token(&self, token: &SealedToken) -> Result<TokenPayload, OracleError> {
        let valid = CryptoIdentity::verify(
            SignatureAlgorithm::Ed25519,
            self.identity.verifying_key(),
            &token.payload,
            &token.signature,
        )
        .map_err(|_| OracleError::InvalidSignature)?;

        if !valid {
            return Err(OracleError::InvalidSignature);
        }

        let payload: TokenPayload = serde_json::from_slice(&token.payload)
            .map_err(|e| OracleError::InvalidPayload(e.to_string()))?;

        Ok(payload)
    }

    /// Get the verifying (public) key bytes. Returns a byte slice rather than
    /// a library-specific type, enabling algorithm-agile consumers.
    pub fn verifying_key_bytes(&self) -> &[u8] {
        self.identity.verifying_key()
    }

    /// Alias for `verifying_key_bytes`. Mirrors `CryptoIdentity::verifying_key`
    /// for callers that prefer the shorter name when the byte-slice return
    /// type is already obvious from context.
    pub fn verifying_key(&self) -> &[u8] {
        self.identity.verifying_key()
    }

    pub fn requests_processed(&self) -> u64 {
        self.requests_processed
            .load(std::sync::atomic::Ordering::Relaxed)
    }

    pub fn uptime_secs(&self) -> u64 {
        self.started_at.elapsed().as_secs()
    }

    pub fn response_ceiling(&self) -> Duration {
        self.timing.response_ceiling
    }
}

fn epoch_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_timing_normalization() {
        let (tx, mut rx) = mpsc::channel::<OracleRequest>(16);
        let oracle = GovernanceOracle::new(tx, Duration::from_millis(100));

        // Spawn a fast decision engine
        tokio::spawn(async move {
            while let Some(req) = rx.recv().await {
                let _ = req.response_tx.send(GovernanceDecision::Approved {
                    capability_token: "tok".into(),
                });
            }
        });

        let start = std::time::Instant::now();
        let result = oracle
            .submit_request(CapabilityRequest {
                agent_id: "a".into(),
                capability: "llm.query".into(),
                parameters: serde_json::Value::Null,
                budget_hash: String::new(),
                request_nonce: "n1".into(),
            })
            .await;

        assert!(result.is_ok());
        let elapsed = start.elapsed();
        // Must take at least response_ceiling (100ms)
        assert!(
            elapsed >= Duration::from_millis(100),
            "Elapsed {elapsed:?} should be >= 100ms"
        );
    }

    #[tokio::test]
    async fn test_denied_response_no_reason() {
        let (tx, mut rx) = mpsc::channel::<OracleRequest>(16);
        let oracle = GovernanceOracle::new(tx, Duration::from_millis(10));

        tokio::spawn(async move {
            if let Some(req) = rx.recv().await {
                let _ = req.response_tx.send(GovernanceDecision::Denied);
            }
        });

        let result = oracle
            .submit_request(CapabilityRequest {
                agent_id: "a".into(),
                capability: "process.exec".into(),
                parameters: serde_json::Value::Null,
                budget_hash: String::new(),
                request_nonce: "n2".into(),
            })
            .await
            .unwrap();

        let payload = oracle.verify_token(&result).unwrap();
        // Denied response contains NO reason — just Denied
        assert_eq!(payload.decision, GovernanceDecision::Denied);
    }

    #[test]
    fn test_sealed_token_verify() {
        let identity = CryptoIdentity::generate(SignatureAlgorithm::Ed25519).unwrap();
        let payload = TokenPayload {
            decision: GovernanceDecision::Approved {
                capability_token: "t".into(),
            },
            nonce: "n".into(),
            timestamp: 0,
            governance_version: "v1".into(),
            request_nonce: "rn".into(),
            agent_id: "a".into(),
        };
        let bytes = serde_json::to_vec(&payload).unwrap();
        let sig = identity.sign(&bytes).unwrap();
        let token = SealedToken {
            payload: bytes,
            signature: sig,
            token_id: "tid".into(),
        };

        let (tx, _rx) = mpsc::channel::<OracleRequest>(1);
        let mut oracle = GovernanceOracle::new(tx, Duration::from_millis(10));
        // Replace identity with our test keypair
        oracle.identity = identity;

        let verified = oracle.verify_token(&token).unwrap();
        assert_eq!(verified.agent_id, "a");
    }

    #[test]
    fn with_identity_uses_passed_key() {
        let identity = CryptoIdentity::generate(SignatureAlgorithm::Ed25519).unwrap();
        let expected_vk = identity.verifying_key().to_vec();

        let (tx, _rx) = mpsc::channel::<OracleRequest>(1);
        let oracle =
            GovernanceOracle::with_identity(tx, Duration::from_millis(10), identity.clone());

        assert_eq!(oracle.verifying_key(), expected_vk.as_slice());
        assert_eq!(oracle.verifying_key_bytes(), expected_vk.as_slice());

        let payload = serde_json::to_vec(&TokenPayload {
            decision: GovernanceDecision::Denied,
            nonce: "n".into(),
            timestamp: 0,
            governance_version: String::new(),
            request_nonce: "r".into(),
            agent_id: "a".into(),
        })
        .unwrap();
        let signature = identity.sign(&payload).unwrap();
        let token = SealedToken {
            payload,
            signature,
            token_id: "t".into(),
        };
        assert!(oracle.verify_token(&token).is_ok());
    }
}
