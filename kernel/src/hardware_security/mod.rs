mod manager;
mod software;
mod stubs;
#[cfg(test)]
mod tests;
mod types;

pub use manager::{KeyBackendKind, KeyManager, KeyManagerConfig, RotationApproval};
pub use software::SoftwareBackend;
pub use stubs::{SecureEnclaveBackend, TeeBackend, TpmBackend};
pub use types::{
    AttestationReport, KeyBackend, KeyError, KeyHandle, KeyPurpose, PublicKeyBytes, SignatureBytes,
};
