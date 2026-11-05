//! Retirement dependency POLICY, shared by live registration and staged oracles.
//! No Cargo invocation, dependency resolution or host-target filtering is needed.
use super::{cargo_manifests, root, stages, within};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path};
use toml::Value;

const KINDS: [&str; 3] = ["dependencies", "dev-dependencies", "build-dependencies"];
// Package identities and roots from the already merged crate retirements.
// The K02–K22 module roots additionally come directly from manifest.jsonl.
pub const RETIRED: [(&str, &str); 6] = [
    ("nexus-outcome-eval", "crates/nexus-outcome-eval"),
    ("nexus-self-improve", "crates/nexus-self-improve"),
    ("nexus-memory", "crates/nexus-memory"),
    ("web-builder-agent", "agents/web-builder"),
    ("nexus-connectors-llm", "connectors/llm"),
    ("nexus-sdk", "sdk"),
];

pub fn load_manifests() -> BTreeMap<String, String> {
    fn walk(dir: &Path, out: &mut BTreeMap<String, String>) {
        for entry in std::fs::read_dir(dir).expect("manifest directory readable") {
            let entry = entry.unwrap();
            let name = entry.file_name();
            let name = name.to_str().unwrap();
            if name.starts_with("target") || matches!(name, ".git" | "node_modules" | "dist") {
                continue;
            }
            let kind = entry.file_type().unwrap();
            assert!(
                !kind.is_symlink(),
                "manifest inventory cannot alias: {:?}",
                entry.path()
            );
            if kind.is_dir() {
                walk(&entry.path(), out);
            } else if name == "Cargo.toml" {
                let rel = entry
                    .path()
                    .strip_prefix(root())
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .replace('\\', "/");
                out.insert(rel, std::fs::read_to_string(entry.path()).unwrap());
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(&root(), &mut out);
    let (retained, _) = retained_paths(&out);
    out.retain(|path, _| retained.contains(path));
    out
}

// Lexically resolve even nonexistent retired paths, including traversal and
// absolute paths inside this checkout. Live source inventory separately rejects
// symlink aliases; synthetic trees have no filesystem-dependent path authority.
pub fn dependency_path(manifest: &str, raw: &str) -> Option<String> {
    let raw = raw.replace('\\', "/");
    let path = Path::new(&raw);
    let joined = if path.is_absolute() {
        path.to_path_buf()
    } else {
        root().join(Path::new(manifest).parent()?).join(path)
    };
    let mut normalized = std::path::PathBuf::new();
    for part in joined.components() {
        match part {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            part => normalized.push(part.as_os_str()),
        }
    }
    Some(
        normalized
            .strip_prefix(root())
            .ok()?
            .to_str()?
            .replace('\\', "/"),
    )
}

fn dependency_tables(data: &Value) -> Option<Vec<&toml::map::Map<String, Value>>> {
    fn append<'a>(data: &'a Value, out: &mut Vec<&'a toml::map::Map<String, Value>>) -> Option<()> {
        for kind in KINDS {
            if let Some(table) = data.get(kind) {
                out.push(table.as_table()?);
            }
        }
        Some(())
    }
    let mut out = Vec::new();
    append(data, &mut out)?;
    if let Some(targets) = data.get("target") {
        for target in targets.as_table()?.values() {
            append(target, &mut out)?;
        }
    }
    if let Some(workspace) = data.get("workspace") {
        append(workspace, &mut out)?;
    }
    Some(out)
}

fn manifest_ok(path: &str, data: &Value) -> Option<bool> {
    static BASELINE: std::sync::OnceLock<BTreeSet<String>> = std::sync::OnceLock::new();
    let baseline = BASELINE.get_or_init(|| {
        serde_json::from_str(include_str!("kernel-dependency-names.json")).unwrap()
    });
    for table in dependency_tables(data)? {
        for (alias, spec) in table {
            // All names/kinds count, including optional and inactive targets.
            if path == "kernel/Cargo.toml" && !baseline.contains(alias) {
                return Some(false);
            }
            let package = match spec.get("package") {
                Some(value) => Some(value.as_str()?),
                None => None,
            };
            if RETIRED
                .iter()
                .any(|(name, _)| alias == name || package == Some(*name))
            {
                return Some(false);
            }
            if let Some(raw) = spec.get("path") {
                let raw = raw.as_str()?;
                if let Some(resolved) = dependency_path(path, raw) {
                    if RETIRED
                        .iter()
                        .any(|(_, retired)| within(&resolved, retired))
                        || stages()
                            .iter()
                            .flat_map(|row| &row.roots)
                            .any(|retired| within(&resolved, retired))
                    {
                        return Some(false);
                    }
                }
            }
            if !spec.is_str() && !spec.is_table() {
                return None;
            }
        }
    }
    Some(true)
}

// Match Cargo's simple component globs against the manifest inventory. Unsupported
// syntax is a policy error rather than silently omitting a workspace member.
fn member_matches(pattern: &str, member: &str) -> Option<bool> {
    if pattern.contains(['[', ']', '{', '}', '\\']) || pattern.contains("**") {
        return None;
    }
    let pattern = pattern.trim_start_matches("./").trim_end_matches('/');
    if !pattern.contains(['*', '?']) {
        return Some(pattern == member);
    }
    let mut expression = String::from("^");
    for c in pattern.chars() {
        match c {
            '*' => expression.push_str("[^/]*"),
            '?' => expression.push_str("[^/]"),
            c => expression.push_str(&regex::escape(&c.to_string())),
        }
    }
    expression.push('$');
    Some(regex::Regex::new(&expression).ok()?.is_match(member))
}

// Discover members from current root metadata, retaining the original required
// manifest paths. Nested independent Cargo test fixtures are not product members.
fn retained_paths(files: &BTreeMap<String, String>) -> (BTreeSet<String>, bool) {
    let mut paths = cargo_manifests();
    let workspace_ok = || -> Option<BTreeSet<String>> {
        let data: Value = toml::from_str(files.get("Cargo.toml")?).ok()?;
        let mut selected = BTreeSet::new();
        for member in data.get("workspace")?.get("members")?.as_array()? {
            let pattern = member.as_str()?;
            let mut matched = false;
            for path in files.keys().filter(|p| p.ends_with("Cargo.toml")) {
                let member = if path == "Cargo.toml" {
                    "."
                } else {
                    path.strip_suffix("/Cargo.toml")?
                };
                if member_matches(pattern, member)? {
                    matched = true;
                    selected.insert(path.clone());
                }
            }
            if !matched {
                return None;
            }
        }
        Some(selected)
    };
    match workspace_ok() {
        Some(selected) => {
            paths.extend(selected);
            (paths, true)
        }
        None => (paths, false),
    }
}

pub fn violations(stage: u8, files: &BTreeMap<String, String>) -> BTreeSet<String> {
    assert!((1..=22).contains(&stage));
    let mut bad = BTreeSet::new();
    let (retained, workspace_ok) = retained_paths(files);
    if !workspace_ok {
        bad.insert("Cargo.toml".into());
    }
    for path in retained {
        let valid = files
            .get(&path)
            .and_then(|raw| toml::from_str::<Value>(raw).ok())
            .and_then(|data| manifest_ok(&path, &data));
        if valid != Some(true) {
            bad.insert(path);
        }
    }
    bad
}
