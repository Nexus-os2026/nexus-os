//! The packaged verifier toolchain contract, shared by the build script
//! (which renders the embedded manifest from the assembled tree) and the
//! runtime verifier. Dependency-free on purpose.

/// Manifest schema.
pub const SCHEMA: u32 = 1;
/// The packaged Rust release.
pub const RUST_VERSION: &str = "1.94.0";
/// The platform the toolchain's own executables run on.
pub const HOST_TARGET: &str = "x86_64-unknown-linux-gnu";
/// The only target the toolchain builds for.
pub const VERIFIER_TARGET: &str = "x86_64-unknown-linux-musl";

/// The entry executable (launched by descriptor).
pub const CARGO: &str = "bin/cargo";
/// The compiler (`RUSTC`).
pub const RUSTC: &str = "bin/rustc";
/// The linker (self-contained static-pie musl linking).
pub const LINKER: &str = "lib/rustlib/x86_64-unknown-linux-gnu/bin/rust-lld";

/// Files every packaged tree must contain, with whether each must be
/// executable.
pub const REQUIRED: &[(&str, bool)] = &[
    (CARGO, true),
    (RUSTC, true),
    (LINKER, true),
    (
        "lib/rustlib/x86_64-unknown-linux-musl/lib/self-contained/crt1.o",
        false,
    ),
    (
        "lib/rustlib/x86_64-unknown-linux-musl/lib/self-contained/rcrt1.o",
        false,
    ),
    (
        "lib/rustlib/x86_64-unknown-linux-musl/lib/self-contained/libc.a",
        false,
    ),
    (
        "lib/rustlib/x86_64-unknown-linux-musl/lib/self-contained/libunwind.a",
        false,
    ),
];

/// Bounds of the manifest grammar.
pub const MAX_FILES: usize = 1_000;
pub const MAX_FILE_BYTES: u64 = 1 << 30;
pub const MAX_PATH_BYTES: usize = 200;

/// A relative path of `/`-separated components, each of ASCII letters,
/// digits, `.`, `_`, `+` and `-`, none `.` or `..` and none empty.
pub fn valid_path(path: &str) -> bool {
    !path.is_empty()
        && path.len() <= MAX_PATH_BYTES
        && path.split('/').all(|part| {
            !part.is_empty()
                && part != "."
                && part != ".."
                && part.len() <= 255
                && part
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'+' | b'-'))
        })
}

/// Every directory a file path implies, outermost first.
pub fn directory_prefixes(path: &str) -> Vec<&str> {
    path.match_indices('/').map(|(at, _)| &path[..at]).collect()
}
