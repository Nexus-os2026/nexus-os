//! Opaque identities and digests.
//!
//! An identifier names a backend record; it never carries authority. Every
//! operation that takes one looks the record up in backend state and checks
//! the caller's bound agent and run against it, so a guessed, forged,
//! replayed or deserialized identifier grants nothing.

use sha2::{Digest as _, Sha256};
use std::fmt;

/// A SHA-256 digest.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Digest([u8; 32]);

impl Digest {
    /// The digest of `parts`, each length-prefixed so that no two different
    /// part lists collide by concatenation, under a domain tag.
    pub fn of(domain: &str, parts: &[&[u8]]) -> Self {
        let mut hasher = Sha256::new();
        hasher.update((domain.len() as u64).to_be_bytes());
        hasher.update(domain.as_bytes());
        for part in parts {
            hasher.update((part.len() as u64).to_be_bytes());
            hasher.update(part);
        }
        Self(hasher.finalize().into())
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    pub fn to_hex(&self) -> String {
        hex::encode(self.0)
    }

    /// The first twelve hex digits, for display only.
    pub fn short(&self) -> String {
        self.to_hex()[..12].to_string()
    }
}

impl fmt::Debug for Digest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Digest({})", self.short())
    }
}

impl fmt::Display for Digest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_hex())
    }
}

/// 128 bits from the operating system's generator.
fn random16() -> [u8; 16] {
    let mut bytes = [0u8; 16];
    getrandom::getrandom(&mut bytes).expect("the OS random generator is available");
    bytes
}

macro_rules! opaque_id {
    ($(#[$doc:meta])* $name:ident, $prefix:literal) => {
        $(#[$doc])*
        #[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub struct $name([u8; 16]);

        impl $name {
            /// Parse the display form (`<prefix>-<32 hex>`), for looking up a
            /// record. A parsed identity is only a name.
            pub fn parse(text: &str) -> Option<Self> {
                let hex_part = text.strip_prefix(concat!($prefix, "-"))?;
                if hex_part.len() != 32 {
                    return None;
                }
                let mut bytes = [0u8; 16];
                hex::decode_to_slice(hex_part, &mut bytes).ok()?;
                Some(Self(bytes))
            }

            pub fn as_bytes(&self) -> &[u8; 16] {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}-{}", $prefix, hex::encode(self.0))
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}-{}", $prefix, &hex::encode(self.0)[..8])
            }
        }
    };
}

opaque_id!(
    /// One action commitment.
    CommitmentId,
    "cmt"
);
opaque_id!(
    /// One governed run: the scope every commitment of one command or agent
    /// goal belongs to.
    RunId,
    "run"
);
opaque_id!(
    /// One owner grant.
    GrantId,
    "grant"
);
opaque_id!(
    /// One credential lease.
    LeaseId,
    "lease"
);

/// The identities this crate's registries mint. A fresh identity grants
/// nothing: only the registry that records it gives it meaning.
macro_rules! minted {
    ($($name:ident),*) => {
        $(
            impl $name {
                pub(crate) fn fresh() -> Self {
                    Self(random16())
                }
            }
        )*
    };
}

minted!(CommitmentId, RunId, GrantId);

/// The identity of the agent (or the owner's own command session) a run
/// acts for: a bounded identifier, compared exactly.
#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct AgentId(String);

/// The owner's own command session: commands submitted through the unified
/// front door act for this identity.
pub const OWNER_SESSION: &str = "owner-session";

impl AgentId {
    /// An agent identity: 1 to 64 characters of `[A-Za-z0-9_-]`.
    pub fn new(text: &str) -> Option<Self> {
        let ok = !text.is_empty()
            && text.len() <= 64
            && text
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_');
        ok.then(|| Self(text.to_string()))
    }

    pub fn owner_session() -> Self {
        Self(OWNER_SESSION.to_string())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for AgentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl fmt::Debug for AgentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "AgentId({})", self.0)
    }
}
