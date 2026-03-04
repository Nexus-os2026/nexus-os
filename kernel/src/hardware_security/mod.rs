mod software;
mod stubs;
mod types;

pub use software::SoftwareBackend;
pub use stubs::{SecureEnclaveBackend, TeeBackend, TpmBackend};
pub use types::{
    AttestationReport, KeyBackend, KeyError, KeyHandle, KeyPurpose, PublicKeyBytes, SignatureBytes,
};
