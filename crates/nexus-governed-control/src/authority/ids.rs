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
    let filled = ::getrandom::getrandom(&mut bytes);
    #[cfg(test)]
    let filled = marker_fault::inject(filled);
    filled.map_err(|_| MarkerUnavailable)?;
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    Ok(::hex::encode(bytes))
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
        let filled = ::getrandom::getrandom(&mut bytes);
        #[cfg(test)]
        let filled = marker_fault::inject(filled);
        filled.map_err(|_| MarkerUnavailable)?;
        bytes[6] = (bytes[6] & 0x0f) | 0x40;
        bytes[8] = (bytes[8] & 0x3f) | 0x80;
        Ok(::hex::encode(bytes))
    }";
    const SIGNATURE: &str = "pub fn fresh_page_marker() -> Result<String, MarkerUnavailable> {";

    fn approved(body: &str) -> bool {
        let tokens = |code: &str| -> String { code_only(code).split_whitespace().collect() };
        tokens(body) == tokens(APPROVED)
    }

    /// The length of the string or char literal starting `code`, if any.
    fn literal_len(code: &[u8]) -> Option<usize> {
        match code {
            [b'\'', c, b'\'', ..] if *c != b'\\' => Some(3),
            [b'"', rest @ ..] => {
                let mut i = 0;
                while i < rest.len() && rest[i] != b'"' {
                    i += if rest[i] == b'\\' { 2 } else { 1 };
                }
                Some((i + 2).min(code.len()))
            }
            _ => None,
        }
    }

    /// `code` with every comment replaced by one space; literals kept.
    fn code_only(code: &str) -> String {
        let b = code.as_bytes();
        let (mut out, mut i) = (Vec::with_capacity(b.len()), 0);
        while i < b.len() {
            if b[i..].starts_with(b"//") {
                while i < b.len() && b[i] != b'\n' {
                    i += 1;
                }
                out.push(b' ');
            } else if b[i..].starts_with(b"/*") {
                let mut depth = 0usize;
                while i < b.len() {
                    if b[i..].starts_with(b"/*") {
                        (depth, i) = (depth + 1, i + 2);
                    } else if b[i..].starts_with(b"*/") {
                        (depth, i) = (depth - 1, i + 2);
                        if depth == 0 {
                            break;
                        }
                    } else {
                        i += 1;
                    }
                }
                out.push(b' ');
            } else {
                let n = literal_len(&b[i..]).unwrap_or(1);
                out.extend_from_slice(&b[i..i + n]);
                i += n;
            }
        }
        String::from_utf8(out).unwrap_or_default()
    }

    /// Declarations that could rebind `::getrandom`/`::hex` or shadow them.
    const REBINDINGS: &[&str] = &[
        "extern crate",
        "mod getrandom",
        "mod hex",
        " as getrandom",
        " as hex",
    ];

    /// The unique, ungated production definition's body, comments removed.
    fn production_body(source: &str) -> Option<String> {
        let source = source.replace("\r\n", "\n");
        let raw = &source[..source.find("\n#[cfg(test)]\nmod marker_tests {")?];
        let production = code_only(raw);
        let ok = production.matches("fresh_page_marker").count() == 1
            && production.matches("mod marker_fault").count() == 1
            && production
                .matches("\n#[cfg(test)]\nmod marker_fault {")
                .count()
                == 1
            && !REBINDINGS.iter().any(|r| production.contains(r));
        let at = production.find(SIGNATURE).filter(|_| ok)?;
        if production[..at].trim_end().ends_with(']') {
            return None;
        }
        let (b, open) = (production.as_bytes(), at + SIGNATURE.len() - 1);
        let (mut i, mut depth) = (open, 0usize);
        while i < b.len() {
            if let Some(n) = literal_len(&b[i..]) {
                i += n;
                continue;
            }
            match b[i] {
                b'{' => depth += 1,
                b'}' if depth == 1 => return production.get(open..=i).map(str::to_string),
                b'}' => depth -= 1,
                _ => {}
            }
            i += 1;
        }
        None
    }

    /// The pre-review body: `getrandom`/`hex` resolved relative to this module.
    const UNPINNED: &str = "{
        let mut bytes = [0u8; 16];
        let filled = getrandom::getrandom(&mut bytes);
        #[cfg(test)]
        let filled = marker_fault::inject(filled);
        filled.map_err(|_| MarkerUnavailable)?;
        bytes[6] = (bytes[6] & 0x0f) | 0x40;
        bytes[8] = (bytes[8] & 0x3f) | 0x80;
        Ok(hex::encode(bytes))
    }";
    /// Production-only modules that would capture a relative path.
    const SHADOWS: &[&str] = &[
        "#[cfg(not(test))]\nmod getrandom {\n    pub fn getrandom(b: &mut [u8]) \
         -> Result<(), ::getrandom::Error> {\n        b.fill(0x5a);\n        Ok(())\n    }\n}\n\n",
        "#[cfg(not(test))]\nmod hex {\n    pub fn encode(_: [u8; 16]) -> String {\n        \
         \"5a\".repeat(16)\n    }\n}\n\n",
    ];
    /// Harmless comments inside the production body.
    const COMMENTS: &[&str] = &["\n    // } temporary buffer", "\n    /* } */"];
    /// Rebinding declarations whose tokens are split by whitespace or comments.
    const REBINDS: &[&str] = &[
        "extern\ncrate self as getrandom;\n\n",
        "extern\ncrate self as\ngetrandom;\n\n",
        "extern /* */ crate self as /* */ hex;\n\n",
        "use ::std as\ngetrandom;\n\n",
        "pub mod\thex {}\n\n",
        "mod /**/ getrandom {}\n\n",
        "mod r#hex {}\n\n",
    ];
    /// Literals a comment stripper can mis-lex, hiding later declarations.
    const MISLEXED: &[&str] = &[
        "const _: &str = r#\"/*\"#;\n\n",
        "const _: &str = r\"\\\";\nconst _: &str = \"/*\";\nextern crate self as getrandom;\n// */\n\n",
        "const _: &[u8] = br#\"x\"#;\n\n",
        "const _: char = '\\\"';\nconst _: &str = \"/*\";\nextern crate self as hex;\n// */\n\n",
        "const _: (char, char) = ('é','\"');\nconst _: &str = \"/*\";\nextern crate self as hex;\n// */\n\n",
    ];
    /// A second test-module header in production, truncating the checked section.
    const FAKE_CUT: &str =
        "\nconst _: &str = \"\n#[cfg(test)]\nmod marker_tests {\";\nextern crate self as hex;\n";

    /// The source with `insert` placed after the first `at` (the production one).
    fn edited(source: &str, at: &str, insert: &str, before: bool) -> String {
        let with = if before {
            format!("{insert}{at}")
        } else {
            format!("{at}{insert}")
        };
        source.replacen(at, &with, 1)
    }

    fn approved_source(source: &str) -> bool {
        production_body(source).is_some_and(|body| approved(&body))
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
        let source = include_str!("ids.rs");
        let body = production_body(source);
        assert!(
            body.as_deref().is_some_and(approved),
            "non-test fresh_page_marker is not the approved fallible getrandom path: {body:?}"
        );
        let mut failures = Vec::new();
        if approved(UNPINNED) {
            failures.push("relative getrandom/hex paths accepted".to_string());
        }
        let doc = "/// A fresh page-framing marker";
        for shadow in SHADOWS {
            if approved_source(&edited(source, doc, shadow, true)) {
                failures.push(format!("shadow accepted: {shadow:?}"));
            }
        }
        for plant in REBINDS {
            if approved_source(&edited(source, doc, plant, true)) {
                failures.push(format!("split rebinding accepted: {plant:?}"));
            }
        }
        for plant in MISLEXED {
            if approved_source(&edited(source, doc, plant, true)) {
                failures.push(format!("mis-lexable literal accepted: {plant:?}"));
            }
        }
        let cut = "\n#[cfg(test)]\nmod marker_tests {";
        if approved_source(&edited(source, cut, FAKE_CUT, true)) {
            failures.push("second test-module header accepted".to_string());
        }
        for comment in COMMENTS {
            if !approved_source(&edited(source, SIGNATURE, comment, false)) {
                failures.push(format!("harmless comment rejected: {comment:?}"));
            }
        }
        assert!(failures.is_empty(), "{failures:#?}");
        let crate_root = code_only(include_str!("../lib.rs"));
        assert!(
            !crate_root.contains("extern crate"),
            "no crate-root rebinding"
        );
        let manifest = include_str!("../../Cargo.toml").replace("\r\n", "\n");
        for dependency in ["\ngetrandom = \"0.2\"\n", "\nhex = \"0.4\"\n"] {
            assert!(manifest.contains(dependency), "unrenamed {dependency:?}");
        }
    }
}
