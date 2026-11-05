//! K01 registration. LIVE_STAGE changes only in the reviewed deletion/extraction PR.
//! Successors are exercised on synthetic trees in K01, never on the live runtime.
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

pub const LIVE_STAGE: u8 = 1;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Stage {
    pub stage: u8,
    pub roots: Vec<String>,
    pub paths: Vec<String>,
    pub declarations: Vec<String>,
    pub reexports: Vec<String>,
    pub items: Vec<Item>,
    pub extract: Vec<Extraction>,
    pub closure: Option<String>,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Item {
    pub path: String,
    pub anchor: String,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Extraction {
    pub source: String,
    pub destination: String,
    pub payload: Vec<String>,
    pub anchors: Vec<String>,
    pub transitional_exports: Vec<String>,
    pub temporary: Vec<Temporary>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Temporary {
    pub anchor: String,
    pub until: u8,
}

pub fn extraction_home(stage: u8, extraction_stage: u8, extraction: &Extraction) -> &str {
    if stage >= extraction_stage {
        &extraction.destination
    } else {
        &extraction.source
    }
}

pub fn stages() -> &'static [Stage] {
    static ROWS: std::sync::OnceLock<Vec<Stage>> = std::sync::OnceLock::new();
    ROWS.get_or_init(|| {
        include_str!("manifest.jsonl")
            .lines()
            .map(|line| serde_json::from_str(line).expect("registered stage"))
            .collect()
    })
}
pub fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_owned()
}
pub fn read(rel: &str) -> String {
    std::fs::read_to_string(root().join(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"))
}
pub fn compact(text: &str) -> String {
    text.split_whitespace().collect()
}
pub fn digest(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    hex::encode(Sha256::digest(bytes))
}
pub fn production_paths() -> BTreeSet<String> {
    include_str!("production-paths.txt")
        .lines()
        .map(str::to_owned)
        .collect()
}
pub fn paths_at(stage: u8) -> BTreeSet<String> {
    let mut paths = production_paths();
    for row in stages() {
        paths.extend(row.paths.iter().cloned());
    }
    for row in stages().iter().filter(|r| r.stage <= stage) {
        for path in &row.paths {
            paths.remove(path);
        }
        for extraction in &row.extract {
            paths.insert(extraction.destination.clone());
        }
    }
    paths
}
pub fn cargo_manifests() -> BTreeMap<String, String> {
    serde_json::from_str(include_str!("cargo-manifests.json")).unwrap()
}
pub fn within(path: &str, parent: &str) -> bool {
    path == parent
        || path
            .strip_prefix(parent)
            .is_some_and(|tail| tail.starts_with('/'))
}
