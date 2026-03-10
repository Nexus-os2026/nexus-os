//! JWT token issuance, validation, refresh, and revocation using EdDSA (Ed25519).
//!
//! Replaces HS256 shared-secret JWTs with asymmetric EdDSA tokens whose
//! signatures can be verified by anyone holding the public key (exposed via
//! a JWKS endpoint).

use crate::identity::agent_identity::AgentIdentity;
use ed25519_dalek::{Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use uuid::Uuid;

/// Default token TTL: 1 hour (3600 seconds).
pub const DEFAULT_TTL_SECS: u64 = 3600;

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

#[derive(Debug, thiserror::Error)]
pub enum TokenError {
    #[error("token expired")]
    Expired,

    #[error("token revoked")]
    Revoked,

    #[error("invalid signature")]
    InvalidSignature,

    #[error("malformed token: {0}")]
    Malformed(String),

    #[error("identity error: {0}")]
    Identity(#[from] crate::identity::IdentityError),
}

// ---------------------------------------------------------------------------
// OIDC-A Claims
// ---------------------------------------------------------------------------

/// OIDC-A JWT claims for agent-to-agent authentication.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OidcAClaims {
    /// Issuer — the system or gateway DID.
    pub iss: String,
    /// Subject — the agent's DID.
    pub sub: String,
    /// Audience.
    pub aud: String,
    /// Expiration (unix timestamp).
    pub exp: u64,
    /// Issued-at (unix timestamp).
    pub iat: u64,
    /// Unique token identifier.
    pub jti: String,
    /// Scopes derived from agent capabilities.
    pub scope: String,
    /// Agent DID (same as sub, explicit per OIDC-A).
    pub agent_did: String,
    /// Delegator subject (from consent records), if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delegator_sub: Option<String>,
}

// ---------------------------------------------------------------------------
// Compact JWT structure  (header.payload.signature)
// ---------------------------------------------------------------------------

/// Minimal JWT header for EdDSA.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct JwtHeader {
    alg: String,
    typ: String,
    /// Key ID — the agent's DID, for JWKS lookup.
    kid: String,
}

// ---------------------------------------------------------------------------
// TokenManager
// ---------------------------------------------------------------------------

/// Issues, validates, refreshes, and revokes EdDSA-signed JWTs.
#[derive(Debug, Clone)]
pub struct TokenManager {
    /// Issuer string embedded in every token.
    issuer: String,
    /// Audience string embedded in every token.
    audience: String,
    /// Set of revoked JTIs.
    revoked: HashSet<String>,
}

impl TokenManager {
    pub fn new(issuer: impl Into<String>, audience: impl Into<String>) -> Self {
        Self {
            issuer: issuer.into(),
            audience: audience.into(),
            revoked: HashSet::new(),
        }
    }

    /// Issue a new EdDSA-signed JWT for the given agent identity.
    ///
    /// * `identity` — the agent's cryptographic identity (holds the signing key).
    /// * `scopes` — capability-derived scopes (e.g. `["web.search", "llm.query"]`).
    /// * `ttl_secs` — token lifetime in seconds (0 → [`DEFAULT_TTL_SECS`]).
    /// * `delegator_sub` — optional delegator DID from consent records.
    pub fn issue_token(
        &self,
        identity: &AgentIdentity,
        scopes: &[String],
        ttl_secs: u64,
        delegator_sub: Option<String>,
    ) -> String {
        let now = now_secs();
        let ttl = if ttl_secs == 0 {
            DEFAULT_TTL_SECS
        } else {
            ttl_secs
        };

        let claims = OidcAClaims {
            iss: self.issuer.clone(),
            sub: identity.did.clone(),
            aud: self.audience.clone(),
            exp: now + ttl,
            iat: now,
            jti: Uuid::new_v4().to_string(),
            scope: scopes.join(" "),
            agent_did: identity.did.clone(),
            delegator_sub,
        };

        self.encode_and_sign(identity, &claims)
    }

    /// Refresh a token: validate the old one (ignoring expiry), then issue a
    /// new token with a fresh `iat`/`exp` and new `jti`.
    pub fn refresh_token(
        &mut self,
        old_token: &str,
        identity: &AgentIdentity,
        ttl_secs: u64,
    ) -> Result<String, TokenError> {
        // Decode without expiry check.
        let old_claims = self.decode_and_verify(old_token, identity, true)?;

        // Revoke the old JTI so it cannot be replayed.
        self.revoked.insert(old_claims.jti.clone());

        let now = now_secs();
        let ttl = if ttl_secs == 0 {
            DEFAULT_TTL_SECS
        } else {
            ttl_secs
        };

        let new_claims = OidcAClaims {
            iss: old_claims.iss,
            sub: old_claims.sub,
            aud: old_claims.aud,
            exp: now + ttl,
            iat: now,
            jti: Uuid::new_v4().to_string(),
            scope: old_claims.scope,
            agent_did: old_claims.agent_did,
            delegator_sub: old_claims.delegator_sub,
        };

        Ok(self.encode_and_sign(identity, &new_claims))
    }

    /// Revoke a token by its JTI.
    pub fn revoke_token(&mut self, jti: &str) {
        self.revoked.insert(jti.to_string());
    }

    /// Validate a token: check signature, expiration, and revocation.
    pub fn validate_token(
        &self,
        token: &str,
        identity: &AgentIdentity,
    ) -> Result<OidcAClaims, TokenError> {
        self.decode_and_verify(token, identity, false)
    }

    /// Return the JWKS JSON representation for the given identity's public key.
    pub fn jwks_json(identity: &AgentIdentity) -> serde_json::Value {
        let pub_bytes = identity.public_key_bytes();
        let x = base64_url_encode(&pub_bytes);
        serde_json::json!({
            "keys": [{
                "kty": "OKP",
                "crv": "Ed25519",
                "use": "sig",
                "alg": "EdDSA",
                "kid": identity.did,
                "x": x,
            }]
        })
    }

    // -- internal -------------------------------------------------------------

    fn encode_and_sign(&self, identity: &AgentIdentity, claims: &OidcAClaims) -> String {
        let header = JwtHeader {
            alg: "EdDSA".to_string(),
            typ: "JWT".to_string(),
            kid: identity.did.clone(),
        };

        let header_b64 = base64_url_encode(&serde_json::to_vec(&header).expect("serialize header"));
        let payload_b64 = base64_url_encode(&serde_json::to_vec(claims).expect("serialize claims"));

        let signing_input = format!("{header_b64}.{payload_b64}");
        let signature = identity.sign(signing_input.as_bytes());
        let sig_b64 = base64_url_encode(&signature);

        format!("{signing_input}.{sig_b64}")
    }

    fn decode_and_verify(
        &self,
        token: &str,
        identity: &AgentIdentity,
        allow_expired: bool,
    ) -> Result<OidcAClaims, TokenError> {
        let parts: Vec<&str> = token.split('.').collect();
        if parts.len() != 3 {
            return Err(TokenError::Malformed(
                "expected 3 dot-separated parts".into(),
            ));
        }

        let payload_bytes = base64_url_decode(parts[1])
            .map_err(|_| TokenError::Malformed("invalid base64 in payload".into()))?;
        let sig_bytes = base64_url_decode(parts[2])
            .map_err(|_| TokenError::Malformed("invalid base64 in signature".into()))?;

        // Verify signature over "header.payload".
        let signing_input = format!("{}.{}", parts[0], parts[1]);

        let vk = VerifyingKey::from_bytes(&identity.public_key_bytes())
            .map_err(|_| TokenError::InvalidSignature)?;
        let sig = ed25519_dalek::Signature::from_slice(&sig_bytes)
            .map_err(|_| TokenError::InvalidSignature)?;
        vk.verify(signing_input.as_bytes(), &sig)
            .map_err(|_| TokenError::InvalidSignature)?;

        let claims: OidcAClaims = serde_json::from_slice(&payload_bytes)
            .map_err(|e| TokenError::Malformed(e.to_string()))?;

        // Check revocation.
        if self.revoked.contains(&claims.jti) {
            return Err(TokenError::Revoked);
        }

        // Check expiration.
        if !allow_expired && claims.exp <= now_secs() {
            return Err(TokenError::Expired);
        }

        Ok(claims)
    }
}

// ---------------------------------------------------------------------------
// Time helper
// ---------------------------------------------------------------------------

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

// ---------------------------------------------------------------------------
// Base64-url helpers (no padding, URL-safe alphabet)
// ---------------------------------------------------------------------------

fn base64_url_encode(input: &[u8]) -> String {
    let mut out = String::new();
    let alphabet = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

    let mut i = 0;
    while i + 2 < input.len() {
        let n =
            (u32::from(input[i]) << 16) | (u32::from(input[i + 1]) << 8) | u32::from(input[i + 2]);
        out.push(alphabet[(n >> 18 & 0x3F) as usize] as char);
        out.push(alphabet[(n >> 12 & 0x3F) as usize] as char);
        out.push(alphabet[(n >> 6 & 0x3F) as usize] as char);
        out.push(alphabet[(n & 0x3F) as usize] as char);
        i += 3;
    }
    let remaining = input.len() - i;
    if remaining == 2 {
        let n = (u32::from(input[i]) << 16) | (u32::from(input[i + 1]) << 8);
        out.push(alphabet[(n >> 18 & 0x3F) as usize] as char);
        out.push(alphabet[(n >> 12 & 0x3F) as usize] as char);
        out.push(alphabet[(n >> 6 & 0x3F) as usize] as char);
    } else if remaining == 1 {
        let n = u32::from(input[i]) << 16;
        out.push(alphabet[(n >> 18 & 0x3F) as usize] as char);
        out.push(alphabet[(n >> 12 & 0x3F) as usize] as char);
    }
    out
}

fn base64_url_decode(input: &str) -> Result<Vec<u8>, ()> {
    fn val(c: u8) -> Result<u8, ()> {
        match c {
            b'A'..=b'Z' => Ok(c - b'A'),
            b'a'..=b'z' => Ok(c - b'a' + 26),
            b'0'..=b'9' => Ok(c - b'0' + 52),
            b'-' => Ok(62),
            b'_' => Ok(63),
            _ => Err(()),
        }
    }

    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len() * 3 / 4);
    let mut i = 0;
    while i + 3 < bytes.len() {
        let (a, b, c, d) = (
            val(bytes[i])?,
            val(bytes[i + 1])?,
            val(bytes[i + 2])?,
            val(bytes[i + 3])?,
        );
        let n = (u32::from(a) << 18) | (u32::from(b) << 12) | (u32::from(c) << 6) | u32::from(d);
        out.push((n >> 16) as u8);
        out.push((n >> 8) as u8);
        out.push(n as u8);
        i += 4;
    }
    let remaining = bytes.len() - i;
    if remaining == 3 {
        let (a, b, c) = (val(bytes[i])?, val(bytes[i + 1])?, val(bytes[i + 2])?);
        let n = (u32::from(a) << 18) | (u32::from(b) << 12) | (u32::from(c) << 6);
        out.push((n >> 16) as u8);
        out.push((n >> 8) as u8);
    } else if remaining == 2 {
        let (a, b) = (val(bytes[i])?, val(bytes[i + 1])?);
        let n = (u32::from(a) << 18) | (u32::from(b) << 12);
        out.push((n >> 16) as u8);
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn test_identity() -> AgentIdentity {
        AgentIdentity::generate(Uuid::new_v4())
    }

    fn test_manager() -> TokenManager {
        TokenManager::new("https://nexus.local", "nexus-agents")
    }

    #[test]
    fn token_with_correct_claims() {
        let id = test_identity();
        let mgr = test_manager();
        let scopes = vec!["web.search".into(), "llm.query".into()];
        let token = mgr.issue_token(&id, &scopes, 3600, Some("did:key:zDelegator".into()));

        let claims = mgr.validate_token(&token, &id).expect("valid token");
        assert_eq!(claims.iss, "https://nexus.local");
        assert_eq!(claims.sub, id.did);
        assert_eq!(claims.aud, "nexus-agents");
        assert_eq!(claims.scope, "web.search llm.query");
        assert_eq!(claims.agent_did, id.did);
        assert_eq!(claims.delegator_sub, Some("did:key:zDelegator".into()));
        assert!(claims.exp > claims.iat);
        assert!(!claims.jti.is_empty());
    }

    #[test]
    fn expired_token_rejected() {
        let id = test_identity();
        let mgr = test_manager();

        // Issue a token that expired 10 seconds ago.
        let now = now_secs();
        let claims = OidcAClaims {
            iss: "https://nexus.local".into(),
            sub: id.did.clone(),
            aud: "nexus-agents".into(),
            exp: now - 10,
            iat: now - 3610,
            jti: Uuid::new_v4().to_string(),
            scope: "test".into(),
            agent_did: id.did.clone(),
            delegator_sub: None,
        };
        let token = mgr.encode_and_sign(&id, &claims);

        let err = mgr.validate_token(&token, &id).unwrap_err();
        assert!(matches!(err, TokenError::Expired));
    }

    #[test]
    fn revoked_token_rejected() {
        let id = test_identity();
        let mut mgr = test_manager();
        let token = mgr.issue_token(&id, &[], 3600, None);

        // Extract JTI from the token.
        let claims = mgr.validate_token(&token, &id).unwrap();
        let jti = claims.jti.clone();

        mgr.revoke_token(&jti);

        let err = mgr.validate_token(&token, &id).unwrap_err();
        assert!(matches!(err, TokenError::Revoked));
    }

    #[test]
    fn refresh_works() {
        let id = test_identity();
        let mut mgr = test_manager();
        let old_token = mgr.issue_token(&id, &["a.b".into()], 3600, None);
        let old_claims = mgr.validate_token(&old_token, &id).unwrap();

        let new_token = mgr.refresh_token(&old_token, &id, 7200).unwrap();
        let new_claims = mgr.validate_token(&new_token, &id).unwrap();

        // Old token is now revoked.
        assert!(matches!(
            mgr.validate_token(&old_token, &id),
            Err(TokenError::Revoked)
        ));

        // New token has same scope/sub but different JTI.
        assert_eq!(new_claims.sub, old_claims.sub);
        assert_eq!(new_claims.scope, old_claims.scope);
        assert_ne!(new_claims.jti, old_claims.jti);
        assert!(new_claims.exp > old_claims.exp);
    }

    #[test]
    fn jwks_returns_valid_key() {
        let id = test_identity();
        let jwks = TokenManager::jwks_json(&id);

        let keys = jwks["keys"].as_array().expect("keys array");
        assert_eq!(keys.len(), 1);

        let key = &keys[0];
        assert_eq!(key["kty"], "OKP");
        assert_eq!(key["crv"], "Ed25519");
        assert_eq!(key["alg"], "EdDSA");
        assert_eq!(key["use"], "sig");
        assert_eq!(key["kid"], id.did);

        // Decode the x parameter and verify it matches the public key.
        let x_b64 = key["x"].as_str().unwrap();
        let x_bytes = base64_url_decode(x_b64).expect("valid base64url");
        assert_eq!(x_bytes.as_slice(), &id.public_key_bytes());
    }

    #[test]
    fn invalid_signature_rejected() {
        let id_a = test_identity();
        let id_b = test_identity();
        let mgr = test_manager();
        let token = mgr.issue_token(&id_a, &[], 3600, None);

        // Verify with wrong identity → signature mismatch.
        let err = mgr.validate_token(&token, &id_b).unwrap_err();
        assert!(matches!(err, TokenError::InvalidSignature));
    }

    #[test]
    fn default_ttl_used_when_zero() {
        let id = test_identity();
        let mgr = test_manager();
        let token = mgr.issue_token(&id, &[], 0, None);
        let claims = mgr.validate_token(&token, &id).unwrap();
        assert_eq!(claims.exp - claims.iat, DEFAULT_TTL_SECS);
    }

    #[test]
    fn base64_url_roundtrip() {
        let data = b"hello, nexus identity tokens!";
        let encoded = base64_url_encode(data);
        let decoded = base64_url_decode(&encoded).unwrap();
        assert_eq!(decoded, data);
    }
}
