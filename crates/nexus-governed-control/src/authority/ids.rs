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
pub(crate) fn random16() -> [u8; 16] {
    let mut bytes = [0u8; 16];
    getrandom::getrandom(&mut bytes).expect("the OS random generator is available");
    bytes
}

/// The operating system's generator could not supply a fresh page marker.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MarkerUnavailable;

/// A fresh page-framing marker: 16 new bytes from the operating system's
/// generator with UUID version 4 bits, as 32 lowercase hex digits. A marker
/// is framing data only; it proves no consent, identity or authority.
/// Generator failure is an error, never a panic or a substitute value.
pub fn fresh_page_marker() -> Result<String, MarkerUnavailable> {
    let mut bytes = [0u8; 16];
    let filled = getrandom::getrandom(&mut bytes);
    #[cfg(test)]
    let filled = marker_fault::inject(filled);
    filled.map_err(|_| MarkerUnavailable)?;
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    Ok(hex::encode(bytes))
}

/// Test-only, one-shot generator fault for this module's marker oracles.
#[cfg(test)]
mod marker_fault {
    use std::cell::Cell;

    thread_local! {
        static FAIL_NEXT: Cell<bool> = const { Cell::new(false) };
    }

    pub(super) fn fail_next() {
        FAIL_NEXT.with(|fail| fail.set(true));
    }

    /// Replaces the generator's result with a failure, once, if armed.
    pub(super) fn inject(filled: Result<(), getrandom::Error>) -> Result<(), getrandom::Error> {
        if FAIL_NEXT.with(|fail| fail.replace(false)) {
            return Err(getrandom::Error::UNSUPPORTED);
        }
        filled
    }
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

minted!(CommitmentId, RunId, GrantId, LeaseId);

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

    /// The label of the owner's command runs (display and evidence only:
    /// an agent may carry the same text, and gains nothing by it; see
    /// [`super::run::RunClass`]).
    pub(crate) fn owner_session() -> Self {
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

#[cfg(test)]
mod marker_tests {
    use super::*;
    use std::cell::Cell;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn is_v4_marker(marker: &str) -> bool {
        let b = marker.as_bytes();
        b.len() == 32
            && b.iter().all(|c| matches!(c, b'0'..=b'9' | b'a'..=b'f'))
            && b[12] == b'4'
            && matches!(b[16], b'8' | b'9' | b'a' | b'b')
    }

    #[test]
    fn m3cb_broker_marker_is_fallible_v4() {
        let normal: Vec<_> = (0..32).map(|_| fresh_page_marker()).collect();
        assert!(
            normal.iter().all(|m| m.as_deref().is_ok_and(is_v4_marker)),
            "default helper: 32 lowercase hex, v4 bits: {normal:?}"
        );
        assert!(normal.windows(2).all(|w| w[0] != w[1]), "{normal:?}");
        marker_fault::fail_next();
        let faulted = std::panic::catch_unwind(fresh_page_marker);
        assert!(
            matches!(faulted, Ok(Err(MarkerUnavailable))),
            "entropy failure: exactly Err(MarkerUnavailable), no panic or fallback: {:?}",
            faulted.as_ref().ok()
        );
        let after = fresh_page_marker();
        assert!(after.as_deref().is_ok_and(is_v4_marker), "{after:?}");
        assert!(!normal.contains(&after), "no previous marker after a fault");
    }

    /// The approved non-test generation path, compared token for token.
    const APPROVED: &str = "{
        let mut bytes = [0u8; 16];
        let filled = getrandom::getrandom(&mut bytes);
        #[cfg(test)]
        let filled = marker_fault::inject(filled);
        filled.map_err(|_| MarkerUnavailable)?;
        bytes[6] = (bytes[6] & 0x0f) | 0x40;
        bytes[8] = (bytes[8] & 0x3f) | 0x80;
        Ok(hex::encode(bytes))
    }";
    const SIGNATURE: &str = "pub fn fresh_page_marker() -> Result<String, MarkerUnavailable> {";

    fn approved(body: &str) -> bool {
        let tokens = |code: &str| -> String {
            let code: String = code
                .lines()
                .map(|l| l.split("//").next().unwrap_or(""))
                .collect();
            code.split_whitespace().collect()
        };
        tokens(body) == tokens(APPROVED)
    }

    /// The unique, ungated production definition's body.
    fn production_body(source: &str) -> Option<&str> {
        let production = &source[..source.find("\n#[cfg(test)]\nmod marker_tests {")?];
        let gated_seam = "\n#[cfg(test)]\nmod marker_fault {";
        let ok = production.matches("fn fresh_page_marker").count() == 1
            && production.matches("mod marker_fault").count() == 1
            && production.matches(gated_seam).count() == 1;
        let at = production.find(SIGNATURE).filter(|_| ok)?;
        let line_before = production[..at].trim_end().rsplit('\n').next()?;
        if !line_before.trim_start().starts_with("///") {
            return None;
        }
        let start = at + SIGNATURE.len() - 1;
        let mut depth = 0usize;
        for (i, c) in production[start..].char_indices() {
            match c {
                '{' => depth += 1,
                '}' if depth == 1 => return Some(&production[start..=start + i]),
                '}' => depth -= 1,
                _ => {}
            }
        }
        None
    }

    // Deterministic substitutes whose outputs look like fresh v4 markers.
    macro_rules! substitutes {
        ($($name:ident $body:block)*) => {
            $(fn $name() -> Result<String, MarkerUnavailable> $body)*
            type Generate = fn() -> Result<String, MarkerUnavailable>;
            const SUBSTITUTES: &[(&str, Generate, &str)] =
                &[$((stringify!($name), $name, stringify!($body))),*];
        };
    }
    substitutes! {
        fixed {
            let mut bytes = [0x5a_u8; 16];
            bytes[6] = (bytes[6] & 0x0f) | 0x40;
            bytes[8] = (bytes[8] & 0x3f) | 0x80;
            Ok(hex::encode(bytes))
        }
        counter {
            static NEXT: AtomicU64 = AtomicU64::new(1);
            let mut bytes = [0u8; 16];
            bytes[8..].copy_from_slice(&NEXT.fetch_add(1, Ordering::Relaxed).to_be_bytes());
            bytes[6] = (bytes[6] & 0x0f) | 0x40;
            bytes[8] = (bytes[8] & 0x3f) | 0x80;
            Ok(hex::encode(bytes))
        }
        timestamp {
            let now = SystemTime::now().duration_since(UNIX_EPOCH);
            let mut bytes = now.map_err(|_| MarkerUnavailable)?.as_nanos().to_be_bytes();
            bytes[6] = (bytes[6] & 0x0f) | 0x40;
            bytes[8] = (bytes[8] & 0x3f) | 0x80;
            Ok(hex::encode(bytes))
        }
        seeded {
            thread_local! {
                static STATE: Cell<u64> = const { Cell::new(0x9e37_79b9_7f4a_7c15) };
            }
            let mut bytes = [0u8; 16];
            for chunk in bytes.chunks_mut(8) {
                let mut x = STATE.with(Cell::get);
                x ^= x << 13;
                x ^= x >> 7;
                x ^= x << 17;
                STATE.with(|state| state.set(x));
                chunk.copy_from_slice(&x.to_le_bytes());
            }
            bytes[6] = (bytes[6] & 0x0f) | 0x40;
            bytes[8] = (bytes[8] & 0x3f) | 0x80;
            Ok(hex::encode(bytes))
        }
        overwritten {
            static NEXT: AtomicU64 = AtomicU64::new(1);
            let mut bytes = [0u8; 16];
            let filled = getrandom::getrandom(&mut bytes);
            filled.map_err(|_| MarkerUnavailable)?;
            bytes[..8].copy_from_slice(&NEXT.fetch_add(1, Ordering::Relaxed).to_be_bytes());
            bytes[6] = (bytes[6] & 0x0f) | 0x40;
            bytes[8] = (bytes[8] & 0x3f) | 0x80;
            Ok(hex::encode(bytes))
        }
    }

    #[test]
    fn m3cb_broker_marker_uses_approved_entropy() {
        for (name, generate, body) in SUBSTITUTES {
            let (a, b) = (generate(), generate());
            let shaped = [&a, &b]
                .iter()
                .all(|m| m.as_deref().is_ok_and(is_v4_marker));
            assert!(shaped, "{name}: v4-shaped {a:?} {b:?}");
            if !matches!(*name, "fixed" | "timestamp") {
                assert_ne!(a, b, "{name}: distinct");
            }
            assert!(
                !approved(body),
                "{name}: deterministic substitute must fail"
            );
        }
        let body = production_body(include_str!("ids.rs"));
        assert!(
            body.is_some_and(approved),
            "non-test fresh_page_marker is not the approved fallible getrandom path: {body:?}"
        );
    }
}
