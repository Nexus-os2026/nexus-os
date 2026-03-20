use crate::hardware_security::types::{
    deterministic_attestation, AttestationReport, KeyBackend, KeyError, KeyHandle, KeyPurpose,
    PublicKeyBytes, SignatureBytes,
};

#[derive(Debug, Clone, Copy, Default)]
pub struct TpmBackend {
    configured: bool,
}

impl TpmBackend {
    pub fn new(configured: bool) -> Self {
        Self { configured }
    }

    fn unavailable_error(&self) -> KeyError {
        KeyError::NotAvailable(self.backend_name())
    }
}

impl KeyBackend for TpmBackend {
    fn backend_name(&self) -> &'static str {
        "tpm2-stub"
    }

    fn is_available(&self) -> bool {
        cfg!(feature = "hardware-tpm") && self.configured
    }

    fn generate_ed25519(&mut self, _purpose: KeyPurpose) -> Result<KeyHandle, KeyError> {
        if !self.is_available() {
            return Err(self.unavailable_error());
        }
        Err(KeyError::BackendFailure(
            "TPM hardware not detected. This feature requires a physical TPM 2.0 module."
                .to_string(),
        ))
    }

    fn public_key(&self, _handle: &KeyHandle) -> Result<PublicKeyBytes, KeyError> {
        if !self.is_available() {
            return Err(self.unavailable_error());
        }
        Err(KeyError::BackendFailure(
            "TPM hardware not detected. This feature requires a physical TPM 2.0 module."
                .to_string(),
        ))
    }

    fn sign(&self, _handle: &KeyHandle, _msg: &[u8]) -> Result<SignatureBytes, KeyError> {
        if !self.is_available() {
            return Err(self.unavailable_error());
        }
        Err(KeyError::BackendFailure(
            "TPM hardware not detected. This feature requires a physical TPM 2.0 module."
                .to_string(),
        ))
    }

    fn rotate(&mut self, _handle: &KeyHandle) -> Result<KeyHandle, KeyError> {
        if !self.is_available() {
            return Err(self.unavailable_error());
        }
        Err(KeyError::BackendFailure(
            "TPM hardware not detected. This feature requires a physical TPM 2.0 module."
                .to_string(),
        ))
    }

    fn attest(&self, nonce: &str) -> Result<AttestationReport, KeyError> {
        Ok(deterministic_attestation(
            self.backend_name(),
            self.is_available(),
            nonce,
        ))
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct SecureEnclaveBackend {
    configured: bool,
}

impl SecureEnclaveBackend {
    pub fn new(configured: bool) -> Self {
        Self { configured }
    }

    fn unavailable_error(&self) -> KeyError {
        KeyError::NotAvailable(self.backend_name())
    }
}

impl KeyBackend for SecureEnclaveBackend {
    fn backend_name(&self) -> &'static str {
        "secure-enclave-stub"
    }

    fn is_available(&self) -> bool {
        cfg!(feature = "hardware-secure-enclave") && self.configured
    }

    fn generate_ed25519(&mut self, _purpose: KeyPurpose) -> Result<KeyHandle, KeyError> {
        if !self.is_available() {
            return Err(self.unavailable_error());
        }
        Err(KeyError::BackendFailure(
            "Secure Enclave hardware not detected. This feature requires Apple Secure Enclave or equivalent.".to_string(),
        ))
    }

    fn public_key(&self, _handle: &KeyHandle) -> Result<PublicKeyBytes, KeyError> {
        if !self.is_available() {
            return Err(self.unavailable_error());
        }
        Err(KeyError::BackendFailure(
            "Secure Enclave hardware not detected. This feature requires Apple Secure Enclave or equivalent.".to_string(),
        ))
    }

    fn sign(&self, _handle: &KeyHandle, _msg: &[u8]) -> Result<SignatureBytes, KeyError> {
        if !self.is_available() {
            return Err(self.unavailable_error());
        }
        Err(KeyError::BackendFailure(
            "Secure Enclave hardware not detected. This feature requires Apple Secure Enclave or equivalent.".to_string(),
        ))
    }

    fn rotate(&mut self, _handle: &KeyHandle) -> Result<KeyHandle, KeyError> {
        if !self.is_available() {
            return Err(self.unavailable_error());
        }
        Err(KeyError::BackendFailure(
            "Secure Enclave hardware not detected. This feature requires Apple Secure Enclave or equivalent.".to_string(),
        ))
    }

    fn attest(&self, nonce: &str) -> Result<AttestationReport, KeyError> {
        Ok(deterministic_attestation(
            self.backend_name(),
            self.is_available(),
            nonce,
        ))
    }
}
