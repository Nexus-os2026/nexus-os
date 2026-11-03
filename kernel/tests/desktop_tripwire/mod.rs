//! G0 (M2b-A PR-1, design §7.1 and §11 A2/A4): the one desktop tripwire,
//! shared by every guard that used to match `app/` only. Included with
//! `#[path]` by `kernel/tests/hm_m3_guards.rs`,
//! `persistence/tests/hm_a1_storage.rs`, `persistence/tests/hm_m2_guards.rs`,
//! `kernel/src/coding_run/tests.rs` and `mother/src/adapter/scan.rs`.
//!
//! Oracle commit: the signatures only. The bodies come with the helper.

#![allow(dead_code)]

use serde_json::Value;
use std::path::{Path, PathBuf};

/// One feature configuration `cargo metadata` resolves.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Features {
    Default,
    All,
}

/// Every configuration the tripwire checks; their findings are united.
pub const CONFIGURATIONS: [Features; 2] = [Features::Default, Features::All];

/// The arguments of one `cargo metadata` run.
pub fn metadata_args(features: Features) -> Vec<&'static str> {
    let _ = features;
    unimplemented!("G0 helper: PR-1 step 2")
}

/// The resolved graph of the workspace at `manifest`, for `features`.
pub fn resolve(manifest: &Path, features: Features) -> Value {
    let _ = (manifest, features);
    unimplemented!("G0 helper: PR-1 step 2")
}

/// What one metadata document of the workspace at `root` breaks.
pub fn findings_of(root: &Path, doc: &Value) -> Vec<String> {
    let _ = (root, doc);
    unimplemented!("G0 helper: PR-1 step 2")
}

/// What the tree at `root` breaks: `app/`, any `src-tauri`, and a
/// `mother/desktop` that is not the registered package in `docs`.
pub fn tree_findings(root: &Path, docs: &[Value]) -> Vec<String> {
    let _ = (root, docs);
    unimplemented!("G0 helper: PR-1 step 2")
}

/// Every finding for the workspace at `root`, over every configuration.
pub fn findings(root: &Path) -> Vec<String> {
    let _ = root;
    unimplemented!("G0 helper: PR-1 step 2")
}

/// The repository's workspace root.
pub fn workspace_root() -> PathBuf {
    unimplemented!("G0 helper: PR-1 step 2")
}

/// The tripwire every guard calls.
pub fn assert_no_desktop() {
    unimplemented!("G0 helper: PR-1 step 2")
}
