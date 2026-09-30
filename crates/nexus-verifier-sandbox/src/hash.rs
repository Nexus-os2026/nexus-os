//! Domain-separated SHA-256 bindings shared by the policy and profile
//! identities. Every field is length-prefixed or fixed-width, so an encoding
//! is unambiguous, and each identity uses its own domain tag.

use sha2::{Digest, Sha256};

pub(crate) fn put_bytes(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update((bytes.len() as u64).to_be_bytes());
    hasher.update(bytes);
}

pub(crate) fn put_u64(hasher: &mut Sha256, value: u64) {
    hasher.update(value.to_be_bytes());
}

pub(crate) fn domain(tag: &[u8]) -> Sha256 {
    let mut hasher = Sha256::new();
    put_bytes(&mut hasher, tag);
    hasher
}

macro_rules! digest_type {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Clone, Copy, PartialEq, Eq, Hash)]
        pub struct $name([u8; 32]);

        impl $name {
            pub(crate) fn finish(hasher: Sha256) -> Self {
                Self(hasher.finalize().into())
            }

            pub fn bytes(&self) -> [u8; 32] {
                self.0
            }

            pub fn to_hex(&self) -> String {
                hex::encode(self.0)
            }
        }

        impl std::fmt::Debug for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}({})", stringify!($name), &self.to_hex()[..12])
            }
        }
    };
}

digest_type!(
    /// Identity of one verifier profile (everything it would run and how).
    ProfileHash
);
digest_type!(
    /// Identity of the sandbox policy a verifier runs under.
    SandboxPolicyHash
);
digest_type!(
    /// Identity of the resource limits a verifier runs under.
    ResourcePolicyHash
);
