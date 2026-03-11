mod manager;
pub mod sealed_store;
mod software;
mod stubs;
pub mod tee_backend;
#[cfg(test)]
mod tests;
mod types;

pub use manager::{KeyBackendKind, KeyManager, KeyManagerConfig, RotationApproval};
pub use sealed_store::SealedKeyStore;
pub use software::SoftwareBackend;
pub use stubs::{SecureEnclaveBackend, TpmBackend};
pub use tee_backend::{TeeBackend, TeeProvider};
pub use types::{
    verify_attestation, verify_attestation_with_max_age, AttestationReport, KeyBackend, KeyError,
    KeyHandle, KeyPurpose, PublicKeyBytes, SignatureBytes,
};
