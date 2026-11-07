//! M3-E M-1: the SHA-256 of the running host image (design step 0).
//!
//! The digest comes from Mother's adapter (`input_files`, M3E-R3), which
//! streams `/proc/self/exe` (the executed file, even after its pathname is
//! replaced), bounded by [`SELF_IDENTITY_MAX_BYTES`]; argv[0] and pathnames
//! are never used. Anything else is [`SelfIdentityUnavailable`]: no made-up
//! hash. Linux-only. Data-only (M3E-R2 1(a)): not called from hands yet.

use nexus_mother::adapter::input_files;

/// The largest image hashed: 1 GiB (ruling M3E-R2 5).
pub const SELF_IDENTITY_MAX_BYTES: u64 = input_files::IMAGE_MAX_BYTES;

/// The running image's digest could not be established.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SelfIdentityUnavailable;

impl SelfIdentityUnavailable {
    pub fn code(&self) -> &'static str {
        "self_identity_unavailable"
    }
}

/// SHA-256 of the executable image this process runs.
pub fn running_binary_sha256() -> Result<[u8; 32], SelfIdentityUnavailable> {
    input_files::running_image_sha256().map_err(|_| SelfIdentityUnavailable)
}

#[cfg(test)]
mod tests {
    #[test]
    fn m3e_u1_self_identity_code_and_ceiling() {
        assert_eq!(
            super::SelfIdentityUnavailable.code(),
            "self_identity_unavailable"
        );
        assert_eq!(super::SELF_IDENTITY_MAX_BYTES, 1 << 30);
    }
}
