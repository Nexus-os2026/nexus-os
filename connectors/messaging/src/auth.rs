use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthError {
    ChallengeNotFound,
    InvalidChallengeResponse,
    TokenNotFound,
    TokenExpired,
    DeviceRevoked,
    StepUpChallengeNotFound,
    StepUpChallengeExpired,
    OperationRequiresStrongVerification,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthLevel {
    Basic,
    StepUp,
    Strong,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PairingQrData {
    pub user_id: String,
    pub device_id: String,
    pub one_time_challenge: String,
    pub expires_at: u64,
    pub qr_payload: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PairingResponse {
    pub user_id: String,
    pub device_id: String,
    pub challenge_response: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceToken {
    pub token_id: String,
    pub user_id: String,
    pub device_id: String,
    pub signature: String,
    pub issued_at: u64,
    pub expires_at: u64,
    pub auth_level: AuthLevel,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Operation {
    Status,
    Logs,
    Approve,
    Start,
    Stop,
    CreateAgent,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StepUpChallenge {
    pub challenge_id: String,
    pub token_id: String,
    pub operation: Operation,
    pub expires_at: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StepUpAuthResult {
    Allowed,
    RequiresChallenge(StepUpChallenge),
}

#[derive(Debug, Clone)]
struct StoredPairing {
    user_id: String,
    device_id: String,
    expected_response: String,
    expires_at: u64,
}

#[derive(Debug, Clone)]
struct StoredStepUp {
    token_id: String,
    operation: Operation,
    expected_response: String,
    expires_at: u64,
}

#[derive(Clone)]
pub struct AuthManager {
    secret: String,
    pending_pairings: HashMap<String, StoredPairing>,
    tokens: HashMap<String, DeviceToken>,
    revoked_devices: HashSet<String>,
    step_up: HashMap<String, StoredStepUp>,
    clock: Arc<dyn Fn() -> u64 + Send + Sync>,
}

impl AuthManager {
    pub fn new(secret: &str) -> Self {
        Self {
            secret: secret.to_string(),
            pending_pairings: HashMap::new(),
            tokens: HashMap::new(),
            revoked_devices: HashSet::new(),
            step_up: HashMap::new(),
            clock: Arc::new(current_unix_timestamp),
        }
    }

    pub fn with_clock(secret: &str, clock: Arc<dyn Fn() -> u64 + Send + Sync>) -> Self {
        Self {
            secret: secret.to_string(),
            pending_pairings: HashMap::new(),
            tokens: HashMap::new(),
            revoked_devices: HashSet::new(),
            step_up: HashMap::new(),
            clock,
        }
    }

    pub fn generate_pairing_qr(&mut self, user_id: &str) -> PairingQrData {
        let now = (self.clock)();
        let device_id = Uuid::new_v4().to_string();
        let challenge = Uuid::new_v4().to_string();
        let expected = self.sign(format!("pairing:{user_id}:{device_id}:{challenge}").as_bytes());
        let expires_at = now + 300;

        self.pending_pairings.insert(
            challenge.clone(),
            StoredPairing {
                user_id: user_id.to_string(),
                device_id: device_id.clone(),
                expected_response: expected,
                expires_at,
            },
        );

        PairingQrData {
            user_id: user_id.to_string(),
            device_id: device_id.clone(),
            one_time_challenge: challenge.clone(),
            expires_at,
            qr_payload: format!("nexus://pair?u={user_id}&d={device_id}&c={challenge}"),
        }
    }

    pub fn expected_pairing_response(&self, qr: &PairingQrData) -> String {
        self.sign(
            format!(
                "pairing:{}:{}:{}",
                qr.user_id, qr.device_id, qr.one_time_challenge
            )
            .as_bytes(),
        )
    }

    pub fn verify_pairing(&mut self, response: PairingResponse) -> Result<DeviceToken, AuthError> {
        let now = (self.clock)();

        let stored = self
            .pending_pairings
            .remove(response.challenge_response.as_str())
            .ok_or(AuthError::ChallengeNotFound)?;

        if now > stored.expires_at {
            return Err(AuthError::InvalidChallengeResponse);
        }
        if self.revoked_devices.contains(stored.device_id.as_str()) {
            return Err(AuthError::DeviceRevoked);
        }
        if stored.user_id != response.user_id || stored.device_id != response.device_id {
            return Err(AuthError::InvalidChallengeResponse);
        }

        let provided_signature = self.sign(
            format!(
                "pairing:{}:{}:{}",
                response.user_id, response.device_id, response.challenge_response
            )
            .as_bytes(),
        );

        if provided_signature != stored.expected_response {
            return Err(AuthError::InvalidChallengeResponse);
        }

        let token_id = Uuid::new_v4().to_string();
        let issued_at = now;
        let expires_at = now + 3_600;
        let signature = self.sign(
            format!(
                "token:{}:{}:{}:{}",
                token_id, response.user_id, response.device_id, issued_at
            )
            .as_bytes(),
        );

        let token = DeviceToken {
            token_id: token_id.clone(),
            user_id: response.user_id,
            device_id: response.device_id,
            signature,
            issued_at,
            expires_at,
            auth_level: AuthLevel::Basic,
        };

        self.tokens.insert(token_id, token.clone());
        Ok(token)
    }

    pub fn step_up_auth(
        &mut self,
        token: &DeviceToken,
        operation: Operation,
    ) -> Result<StepUpAuthResult, AuthError> {
        let live_token = self.validate_token(token)?;

        match operation {
            Operation::Status | Operation::Logs => Ok(StepUpAuthResult::Allowed),
            Operation::Approve | Operation::Start | Operation::Stop => {
                if live_token.auth_level == AuthLevel::StepUp
                    || live_token.auth_level == AuthLevel::Strong
                {
                    Ok(StepUpAuthResult::Allowed)
                } else {
                    let challenge = self.generate_step_up_challenge(
                        &live_token.token_id,
                        &operation,
                        &live_token.device_id,
                    );
                    Ok(StepUpAuthResult::RequiresChallenge(challenge))
                }
            }
            Operation::CreateAgent => {
                if live_token.auth_level == AuthLevel::Strong {
                    Ok(StepUpAuthResult::Allowed)
                } else {
                    let challenge = self.generate_step_up_challenge(
                        &live_token.token_id,
                        &operation,
                        &live_token.device_id,
                    );
                    Ok(StepUpAuthResult::RequiresChallenge(challenge))
                }
            }
        }
    }

    pub fn expected_step_up_response(
        &self,
        challenge: &StepUpChallenge,
        device_id: &str,
    ) -> String {
        self.sign(format!("stepup:{}:{}", challenge.challenge_id, device_id).as_bytes())
    }

    pub fn verify_step_up_challenge(
        &mut self,
        token: &DeviceToken,
        challenge_id: &str,
        response_signature: &str,
    ) -> Result<DeviceToken, AuthError> {
        let live_token = self.validate_token(token)?;

        let stored = self
            .step_up
            .remove(challenge_id)
            .ok_or(AuthError::StepUpChallengeNotFound)?;

        if stored.token_id != live_token.token_id {
            return Err(AuthError::StepUpChallengeNotFound);
        }

        let now = (self.clock)();
        if now > stored.expires_at {
            return Err(AuthError::StepUpChallengeExpired);
        }

        if response_signature != stored.expected_response {
            return Err(AuthError::InvalidChallengeResponse);
        }

        let (auth_level, expires_at) = match stored.operation {
            Operation::CreateAgent => (AuthLevel::Strong, now + 900),
            _ => (AuthLevel::StepUp, now + 600),
        };

        let upgraded = DeviceToken {
            auth_level,
            expires_at,
            ..live_token
        };

        self.tokens
            .insert(upgraded.token_id.clone(), upgraded.clone());

        Ok(upgraded)
    }

    pub fn revoke_device(&mut self, device_id: &str) {
        self.revoked_devices.insert(device_id.to_string());
        self.tokens.retain(|_, token| token.device_id != device_id);
        self.step_up.retain(|_, challenge| {
            let token = self.tokens.get(challenge.token_id.as_str());
            match token {
                Some(token) => token.device_id != device_id,
                None => false,
            }
        });
    }

    fn validate_token(&self, token: &DeviceToken) -> Result<DeviceToken, AuthError> {
        if self.revoked_devices.contains(token.device_id.as_str()) {
            return Err(AuthError::DeviceRevoked);
        }

        let stored = self
            .tokens
            .get(token.token_id.as_str())
            .ok_or(AuthError::TokenNotFound)?;

        let now = (self.clock)();
        if now > stored.expires_at {
            return Err(AuthError::TokenExpired);
        }

        Ok(stored.clone())
    }

    fn generate_step_up_challenge(
        &mut self,
        token_id: &str,
        operation: &Operation,
        device_id: &str,
    ) -> StepUpChallenge {
        let now = (self.clock)();
        let challenge_id = Uuid::new_v4().to_string();
        let expires_at = now + 120;

        let expected_response = self.sign(format!("stepup:{challenge_id}:{device_id}").as_bytes());

        self.step_up.insert(
            challenge_id.clone(),
            StoredStepUp {
                token_id: token_id.to_string(),
                operation: operation.clone(),
                expected_response,
                expires_at,
            },
        );

        StepUpChallenge {
            challenge_id,
            token_id: token_id.to_string(),
            operation: operation.clone(),
            expires_at,
        }
    }

    fn sign(&self, data: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(self.secret.as_bytes());
        hasher.update(data);
        format!("{:x}", hasher.finalize())
    }
}

fn current_unix_timestamp() -> u64 {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => duration.as_secs(),
        Err(_) => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::{AuthError, AuthManager, Operation, PairingResponse, StepUpAuthResult};
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::Arc;

    #[test]
    fn test_pairing_flow() {
        let now = Arc::new(AtomicU64::new(1_000));
        let clock = Arc::clone(&now);
        let mut manager =
            AuthManager::with_clock("secret", Arc::new(move || clock.load(Ordering::SeqCst)));

        let qr = manager.generate_pairing_qr("user-123");
        let response = PairingResponse {
            user_id: qr.user_id.clone(),
            device_id: qr.device_id.clone(),
            challenge_response: qr.one_time_challenge.clone(),
        };

        let token = manager.verify_pairing(response);
        assert!(token.is_ok());

        if let Ok(token) = token {
            assert_eq!(token.user_id, "user-123");
            assert_eq!(token.device_id, qr.device_id);
            assert!(!token.signature.is_empty());
        }
    }

    #[test]
    fn test_step_up_auth() {
        let now = Arc::new(AtomicU64::new(2_000));
        let clock = Arc::clone(&now);
        let mut manager =
            AuthManager::with_clock("secret", Arc::new(move || clock.load(Ordering::SeqCst)));

        let qr = manager.generate_pairing_qr("user-xyz");
        let pairing = PairingResponse {
            user_id: qr.user_id.clone(),
            device_id: qr.device_id.clone(),
            challenge_response: qr.one_time_challenge.clone(),
        };
        let basic_token = manager
            .verify_pairing(pairing)
            .expect("pairing should succeed");

        let status = manager.step_up_auth(&basic_token, Operation::Status);
        assert_eq!(status, Ok(StepUpAuthResult::Allowed));

        let approve = manager.step_up_auth(&basic_token, Operation::Approve);
        assert!(matches!(
            approve,
            Ok(StepUpAuthResult::RequiresChallenge(_))
        ));

        let challenge = match approve {
            Ok(StepUpAuthResult::RequiresChallenge(challenge)) => challenge,
            _ => panic!("expected step-up challenge for approve"),
        };

        let response =
            manager.expected_step_up_response(&challenge, basic_token.device_id.as_str());
        let upgraded = manager.verify_step_up_challenge(
            &basic_token,
            challenge.challenge_id.as_str(),
            response.as_str(),
        );
        assert!(upgraded.is_ok());

        if let Ok(upgraded_token) = upgraded {
            let approve_after = manager.step_up_auth(&upgraded_token, Operation::Approve);
            assert_eq!(approve_after, Ok(StepUpAuthResult::Allowed));
        }
    }

    #[test]
    fn test_revoke_device() {
        let now = Arc::new(AtomicU64::new(3_000));
        let clock = Arc::clone(&now);
        let mut manager =
            AuthManager::with_clock("secret", Arc::new(move || clock.load(Ordering::SeqCst)));

        let qr = manager.generate_pairing_qr("user-revoke");
        let pairing = PairingResponse {
            user_id: qr.user_id.clone(),
            device_id: qr.device_id.clone(),
            challenge_response: qr.one_time_challenge.clone(),
        };
        let token = manager
            .verify_pairing(pairing)
            .expect("pairing should succeed");

        manager.revoke_device(token.device_id.as_str());

        let status = manager.step_up_auth(&token, Operation::Status);
        assert_eq!(status, Err(AuthError::DeviceRevoked));
    }
}
