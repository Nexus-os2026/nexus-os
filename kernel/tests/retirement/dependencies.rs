//! R-6: Cargo owns dependency resolution and workspace membership. K01 applies
//! retirement policy to its resolved metadata, never to hand-parsed manifests.
use super::{root, stages};
use serde::Deserialize;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

#[path = "../desktop_tripwire/mod.rs"]
mod desktop_tripwire;

// Already merged crate retirements; module roots come from manifest.jsonl.
pub const RETIRED: [(&str, &str); 6] = [
    ("nexus-outcome-eval", "crates/nexus-outcome-eval"),
    ("nexus-self-improve", "crates/nexus-self-improve"),
    ("nexus-memory", "crates/nexus-memory"),
    ("web-builder-agent", "agents/web-builder"),
    ("nexus-connectors-llm", "connectors/llm"),
    ("nexus-sdk", "sdk"),
];

#[derive(Deserialize)]
struct Graph {
    packages: Vec<Package>,
    workspace_members: BTreeSet<String>,
    resolve: Resolve,
}
#[derive(Deserialize)]
struct Package {
    id: String,
    name: String,
    manifest_path: PathBuf,
    dependencies: Vec<Dependency>,
}
#[derive(Deserialize)]
struct Dependency {
    name: String,
    rename: Option<String>,
}
#[derive(Deserialize)]
struct Resolve {
    nodes: Vec<Node>,
}
#[derive(Deserialize)]
struct Node {
    id: String,
    deps: Vec<Edge>,
}
#[derive(Deserialize)]
struct Edge {
    pkg: String,
    dep_kinds: Vec<Value>,
}

/// All packages Cargo reports, including implicit members and every dependency
/// kind/target. No target filter, alias inference or workspace glob matching.
/// Missing/malformed metadata is a finding, never an empty successful graph.
pub fn findings_of(workspace: &Path, doc: &Value) -> BTreeSet<String> {
    let graph: Graph = match serde_json::from_value(doc.clone()) {
        Ok(graph) => graph,
        Err(error) => return BTreeSet::from([format!("unreadable Cargo metadata: {error}")]),
    };
    let packages: BTreeMap<_, _> = graph.packages.iter().map(|p| (p.id.as_str(), p)).collect();
    let nodes: BTreeSet<_> = graph.resolve.nodes.iter().map(|n| n.id.as_str()).collect();
    let mut found = BTreeSet::new();
    if !workspace.is_absolute()
        || packages.len() != graph.packages.len()
        || nodes.len() != graph.resolve.nodes.len()
        || graph.workspace_members.is_empty()
        || graph
            .workspace_members
            .iter()
            .any(|id| !nodes.contains(id.as_str()))
        || nodes.iter().any(|id| !packages.contains_key(id))
    {
        found.insert(
            "unreadable Cargo metadata: inconsistent package/member/node identities".into(),
        );
    }
    for node in &graph.resolve.nodes {
        for edge in &node.deps {
            if edge.dep_kinds.is_empty() || !nodes.contains(edge.pkg.as_str()) {
                found.insert(format!("unreadable Cargo metadata: edge from {}", node.id));
            }
        }
    }
    let retired_roots: Vec<_> = RETIRED
        .iter()
        .map(|(_, path)| workspace.join(path))
        .chain(
            stages()
                .iter()
                .flat_map(|row| &row.roots)
                .map(|path| workspace.join(path)),
        )
        .collect();
    let cap: BTreeSet<String> =
        serde_json::from_str(include_str!("kernel-dependency-names.json")).unwrap();
    let kernel_manifest = workspace.join("kernel/Cargo.toml");
    let mut kernel = None;
    for package in &graph.packages {
        if !package.manifest_path.is_absolute() {
            found.insert(format!(
                "unreadable Cargo metadata: relative manifest for {}",
                package.id
            ));
        }
        // Cargo supplies resolved paths. Also inspect their canonical spelling
        // when present so a filesystem alias cannot conceal a retired root.
        let canonical = package.manifest_path.canonicalize().ok();
        if RETIRED.iter().any(|(name, _)| package.name == *name)
            || retired_roots.iter().any(|retired| {
                package.manifest_path.starts_with(retired)
                    || canonical
                        .as_ref()
                        .is_some_and(|path| path.starts_with(retired))
            })
        {
            found.insert(format!(
                "retired package {} at {}",
                package.name,
                package.manifest_path.display()
            ));
        }
        if package.manifest_path == kernel_manifest && package.name == "nexus-kernel" {
            kernel = Some(package.id.as_str());
            // Metadata keeps Cargo declaration names/renames with hyphens,
            // unlike resolve.nodes' Rust namespace names with underscores.
            // All normal/dev/build/target (and optional) declarations count.
            for dependency in &package.dependencies {
                let name = dependency.rename.as_ref().unwrap_or(&dependency.name);
                if !cap.contains(name) {
                    found.insert(format!("kernel dependency beyond frozen cap: {name}"));
                }
            }
        }
    }
    if kernel.is_none_or(|id| !graph.workspace_members.contains(id)) {
        found.insert("unreadable Cargo metadata: kernel is not a workspace member".into());
    }
    found
}

fn live_docs() -> &'static [Value] {
    static DOCS: std::sync::OnceLock<Vec<Value>> = std::sync::OnceLock::new();
    DOCS.get_or_init(|| {
        let workspace = root().canonicalize().expect("workspace root readable");
        desktop_tripwire::CONFIGURATIONS
            .into_iter()
            .map(|features| desktop_tripwire::resolve(&workspace.join("Cargo.toml"), features))
            .collect()
    })
}

/// Source fixtures need real member manifests to establish their existing HM
/// reachability proof. Cargo owns this membership too: the ten required paths
/// are a presence minimum, not a frozen selection of workspace members.
pub fn member_manifests_of(workspace: &Path, doc: &Value) -> BTreeSet<String> {
    let graph: Graph = serde_json::from_value(doc.clone()).expect("readable Cargo metadata");
    let mut paths = BTreeSet::from(["Cargo.toml".into()]);
    for package in graph.packages {
        if graph.workspace_members.contains(&package.id) {
            let path = package
                .manifest_path
                .strip_prefix(workspace)
                .expect("fixture member manifest must remain within the workspace");
            paths.insert(
                path.to_str()
                    .expect("UTF-8 fixture path")
                    .replace('\\', "/"),
            );
        }
    }
    paths
}

pub fn fixture_manifests() -> BTreeMap<String, String> {
    assert_live();
    let workspace = root().canonicalize().expect("workspace root readable");
    live_docs()
        .iter()
        .flat_map(|doc| member_manifests_of(&workspace, doc))
        .chain(super::cargo_manifests())
        .map(|path| {
            let raw = super::read(&path);
            (path, raw)
        })
        .collect()
}

pub fn assert_live() {
    static FOUND: std::sync::OnceLock<BTreeSet<String>> = std::sync::OnceLock::new();
    let found = FOUND.get_or_init(|| {
        let workspace = root().canonicalize().expect("workspace root readable");
        live_docs()
            .iter()
            .flat_map(|doc| findings_of(&workspace, doc))
            .collect()
    });
    assert!(
        found.is_empty(),
        "R-6 retirement dependency policy: {found:#?}"
    );
}
