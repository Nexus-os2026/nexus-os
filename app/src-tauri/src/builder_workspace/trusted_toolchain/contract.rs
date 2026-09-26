//! The packaged-toolchain contract shared by the runtime verifier and the
//! P0-002C4D2 build script: the P0-002C4D1A manifest grammar and limits, and
//! the Nexus-owned files every packaged toolchain contains. A packaged tree is
//! accepted at build time exactly when the verifier would accept its manifest.
use std::collections::{BTreeMap, BTreeSet};

/// The Nexus-owned Builder runtime entry.
pub(super) const ENTRY_MODULE: &str = "entry/nexus-builder.mjs";

/// The packaged Nexus-owned Node executable.
pub(super) const fn node_executable(windows: bool) -> &'static str {
    if windows {
        "node/node.exe"
    } else {
        "node/node"
    }
}

pub(super) const MAX_FILES: usize = 100_000;
pub(super) const MAX_FILE_BYTES: u64 = 1 << 30;
pub(super) const MAX_PATH_BYTES: usize = 200;
const MAX_COMPONENT_BYTES: usize = 255;

/// A complete `(path, size)` listing: non-empty, bounded, strictly ordered,
/// grammatical and free of ASCII case-insensitive aliases.
pub(super) fn valid_listing(files: &[(&str, u64)]) -> bool {
    if files.is_empty() || files.len() > MAX_FILES {
        return false;
    }
    if files.windows(2).any(|pair| pair[0].0 >= pair[1].0) {
        return false;
    }
    let mut folded_files = BTreeSet::new();
    for (path, size) in files {
        if !valid_path(path)
            || *size > MAX_FILE_BYTES
            || !folded_files.insert(path.to_ascii_lowercase())
        {
            return false;
        }
    }
    // Every implied directory has one spelling under ASCII case folding, and a
    // file may not also be (case-insensitively) an implied directory.
    let mut folded_dirs = BTreeMap::new();
    for (path, _) in files {
        for prefix in directory_prefixes(path) {
            let lower = prefix.to_ascii_lowercase();
            if folded_files.contains(&lower)
                || *folded_dirs.entry(lower).or_insert(prefix) != prefix
            {
                return false;
            }
        }
    }
    true
}

/// Normalized relative `/`-separated ASCII path under the strict grammar.
pub(super) fn valid_path(path: &str) -> bool {
    !path.is_empty()
        && path.len() <= MAX_PATH_BYTES
        && path.is_ascii()
        && path.split('/').all(valid_component)
}

/// One component: `[A-Za-z0-9._@+-]+`, not `.`/`..`, no trailing dot or
/// space, and no Windows DOS device stem (case-insensitive).
pub(super) fn valid_component(component: &str) -> bool {
    const DEVICES: [&str; 22] = [
        "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
        "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
    ];
    let stem = component.split('.').next().unwrap_or_default();
    !component.is_empty()
        && component.len() <= MAX_COMPONENT_BYTES
        && component
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._@+-".contains(&b))
        && component != "."
        && component != ".."
        && !component.ends_with(['.', ' '])
        && !DEVICES
            .iter()
            .any(|device| stem.eq_ignore_ascii_case(device))
}

pub(super) fn directory_prefixes(path: &str) -> impl Iterator<Item = &str> {
    path.match_indices('/').map(move |(at, _)| &path[..at])
}
