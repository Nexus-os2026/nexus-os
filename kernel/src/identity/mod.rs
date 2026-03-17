//! Persistent cryptographic identity for agents.
//!
//! Each agent receives an Ed25519 keypair on spawn. The public key is encoded
//! as a `did:key:z6Mk…` DID string, providing a stable, verifiable identity
//! that survives restarts when persisted to disk.
//!
//! The [`token_manager`] sub-module provides EdDSA-signed JWT issuance,
//! validation, refresh, and revocation with OIDC-A claims.

pub mod agent_identity;
pub mod credentials;
pub mod passport;
pub mod token_manager;
pub mod zkproofs;

pub use agent_identity::{AgentIdentity, IdentityError, IdentityManager};
pub use credentials::{CredentialError, CredentialIssuer, CredentialType, VerifiableCredential};
pub use passport::{
    export_passport, import_passport, verify_passport, AgentPassport, PassportError, TestScore,
};
pub use token_manager::{OidcAClaims, TokenError, TokenManager, DEFAULT_TTL_SECS};
pub use zkproofs::{verify_proof_structure, ZkClaim, ZkProof, ZkProofError, ZkProofGenerator};
