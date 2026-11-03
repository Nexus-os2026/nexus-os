//! Identifiers and labels: `[a-z][a-z0-9_]{0,47}`, made only by this
//! crate's registry constants.

/// The longest identifier or label, in bytes.
pub const IDENT_MAX_BYTES: usize = 48;

/// Whether `s` is in the grammar `[a-z][a-z0-9_]{0,47}`.
pub const fn is_ident(s: &str) -> bool {
    let b = s.as_bytes();
    if b.is_empty() || b.len() > IDENT_MAX_BYTES || !b[0].is_ascii_lowercase() {
        return false;
    }
    let mut i = 1;
    while i < b.len() {
        if !(b[i].is_ascii_lowercase() || b[i].is_ascii_digit() || b[i] == b'_') {
            return false;
        }
        i += 1;
    }
    true
}

macro_rules! ident_type {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(pub(crate) &'static str);

        impl $name {
            /// Crate-private: only the registry's constants (and this
            /// crate's tests) make one. No public constructor takes a
            /// runtime string. (No registered set has labels yet, so only
            /// test registries make a `Label`.)
            #[allow(dead_code)]
            pub(crate) const fn new(s: &'static str) -> Self {
                assert!(is_ident(s), "identifier outside the grammar");
                Self(s)
            }

            pub const fn as_str(&self) -> &'static str {
                self.0
            }
        }
    };
}

ident_type!(
    /// A question's id, which is also its answer key.
    QuestionId
);
ident_type!(
    /// A Choice option or a Score level.
    Label
);
ident_type!(
    /// A question set's id, such as `sen_1`.
    SetId
);
