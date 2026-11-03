//! G0 (M2b-A PR-1, design §7.1 and §11 A2/A4): the one desktop tripwire,
//! shared by every guard that used to match `app/` only. Included with
//! `#[path]` by `kernel/tests/hm_m3_guards.rs`,
//! `persistence/tests/hm_a1_storage.rs`, `persistence/tests/hm_m2_guards.rs`,
//! `kernel/src/coding_run/tests.rs` and `mother/src/adapter/scan.rs`.
//!
//! It checks package identity in the resolved graph, never names in text:
//! `cargo metadata --locked --offline` over every target, once with the
//! default features and once with all features, the findings united. A
//! package is banned by its name or by its `links` value. Every dependency
//! kind (normal, build, dev) and every target table is followed; aliases
//! resolve to the real package; wrappers are followed to any depth.
//!
//! The one exemption: the registered `nexus-mother-desktop` (the workspace
//! member at `mother/desktop`) may bring the GTK families (`gtk*`, `gdk*`,
//! `gsk*`) with their own tree, and nothing reaches them except through it.
//! The webview families fail everywhere, the desktop included (G11).
//!
//! Anything the helper cannot establish (no `cargo`, an unreadable graph, a
//! `cargo metadata` that fails or overruns its deadline) fails the guard.

#![allow(dead_code)]

use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

/// One feature configuration `cargo metadata` resolves.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Features {
    Default,
    All,
}

/// Every configuration the tripwire checks; their findings are united.
pub const CONFIGURATIONS: [Features; 2] = [Features::Default, Features::All];

/// The families the registered desktop may bring.
const DESKTOP_FAMILIES: [&str; 3] = ["gtk", "gdk", "gsk"];

/// The webview families: banned everywhere (G11).
const WEBVIEW_FAMILIES: [&str; 5] = ["webkit", "javascriptcore", "soup", "wry", "tauri"];

/// The registered desktop: its package name and its directory.
const DESKTOP_NAME: &str = "nexus-mother-desktop";
const DESKTOP_DIR: &str = "mother/desktop";

/// How long one `cargo metadata` may take.
const METADATA_DEADLINE: Duration = Duration::from_secs(180);

/// The arguments of one `cargo metadata` run.
pub fn metadata_args(features: Features) -> Vec<&'static str> {
    let mut args = vec!["metadata", "--locked", "--offline", "--format-version", "1"];
    if features == Features::All {
        args.push("--all-features");
    }
    args
}

/// The `cargo` that runs this test: the one Cargo gave the test process,
/// else the one that compiled it.
fn cargo() -> PathBuf {
    std::env::var_os("CARGO")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO")))
}

/// The resolved graph of the workspace at `manifest`, for `features`. Any
/// failure panics: the guard fails closed.
pub fn resolve(manifest: &Path, features: Features) -> Value {
    let mut child = Command::new(cargo())
        .args(metadata_args(features))
        .arg("--manifest-path")
        .arg(manifest)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|e| panic!("G0: cargo metadata could not start: {e}"));
    let mut stdout = child.stdout.take().expect("piped stdout");
    let mut stderr = child.stderr.take().expect("piped stderr");
    let out = std::thread::spawn(move || {
        let mut text = Vec::new();
        stdout.read_to_end(&mut text).map(|_| text)
    });
    let err = std::thread::spawn(move || {
        let mut text = Vec::new();
        let _ = stderr.read_to_end(&mut text);
        text
    });
    let deadline = Instant::now() + METADATA_DEADLINE;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(20));
            }
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                panic!("G0: cargo metadata overran {METADATA_DEADLINE:?} for {manifest:?}");
            }
            Err(e) => panic!("G0: waiting for cargo metadata: {e}"),
        }
    };
    let text = out
        .join()
        .expect("stdout reader")
        .unwrap_or_else(|e| panic!("G0: reading cargo metadata: {e}"));
    let err = err.join().expect("stderr reader");
    assert!(
        status.success(),
        "G0: cargo metadata {features:?} failed for {manifest:?}: {}",
        String::from_utf8_lossy(&err)
    );
    serde_json::from_slice(&text)
        .unwrap_or_else(|e| panic!("G0: cargo metadata printed no graph: {e}"))
}

/// `name` or `links` in one of `families`. A `links` value may carry a
/// `lib` prefix.
fn in_family(name: &str, links: Option<&str>, families: &[&str]) -> bool {
    let named = |text: &str| {
        let text = text.to_ascii_lowercase();
        let text = text.strip_prefix("lib").unwrap_or(&text).to_string();
        families.iter().any(|family| text.starts_with(family))
    };
    named(name) || links.is_some_and(named)
}

fn banned(name: &str, links: Option<&str>) -> bool {
    in_family(name, links, &DESKTOP_FAMILIES) || in_family(name, links, &WEBVIEW_FAMILIES)
}

fn webview(name: &str, links: Option<&str>) -> bool {
    in_family(name, links, &WEBVIEW_FAMILIES)
}

/// The same path, canonical when it exists.
fn canonical(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

struct Package {
    name: String,
    links: Option<String>,
    manifest: PathBuf,
}

struct Graph {
    packages: BTreeMap<String, Package>,
    members: Vec<String>,
    /// Each package's resolved dependencies: the package each edge reaches.
    edges: BTreeMap<String, Vec<String>>,
}

fn graph(doc: &Value) -> Result<Graph, String> {
    let mut packages = BTreeMap::new();
    for package in doc["packages"].as_array().ok_or("no packages")? {
        let id = package["id"].as_str().ok_or("a package without an id")?;
        packages.insert(
            id.to_string(),
            Package {
                name: package["name"]
                    .as_str()
                    .ok_or("a package without a name")?
                    .to_string(),
                links: package["links"].as_str().map(str::to_string),
                manifest: PathBuf::from(
                    package["manifest_path"]
                        .as_str()
                        .ok_or("a package without a manifest")?,
                ),
            },
        );
    }
    let members = doc["workspace_members"]
        .as_array()
        .ok_or("no workspace members")?
        .iter()
        .map(|m| {
            m.as_str()
                .map(str::to_string)
                .ok_or("a member without an id")
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut edges = BTreeMap::new();
    for node in doc["resolve"]["nodes"]
        .as_array()
        .ok_or("no resolved graph")?
    {
        let id = node["id"].as_str().ok_or("a node without an id")?;
        let mut reached = Vec::new();
        for dep in node["deps"].as_array().ok_or("a node without deps")? {
            let kinds = dep["dep_kinds"].as_array().ok_or("an edge without kinds")?;
            // Every kind (normal, build, dev) under every target.
            if kinds.is_empty() {
                return Err(format!("{id}: an edge with no kind"));
            }
            let pkg = dep["pkg"].as_str().ok_or("an edge without a package")?;
            reached.push(pkg.to_string());
        }
        edges.insert(id.to_string(), reached);
    }
    for (id, reached) in &edges {
        for pkg in reached.iter().chain(std::iter::once(id)) {
            if !packages.contains_key(pkg) {
                return Err(format!("{pkg}: in the graph but not a package"));
            }
        }
    }
    Ok(Graph {
        packages,
        members,
        edges,
    })
}

/// Every package reachable from `from`, never entering `fence`, with the
/// path that first reached it.
fn reach(graph: &Graph, from: &[String], fence: Option<&str>) -> BTreeMap<String, Vec<String>> {
    let mut seen: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut queue = VecDeque::new();
    for id in from {
        if Some(id.as_str()) != fence && !seen.contains_key(id) {
            seen.insert(id.clone(), vec![id.clone()]);
            queue.push_back(id.clone());
        }
    }
    while let Some(id) = queue.pop_front() {
        let path = seen[&id].clone();
        for next in graph.edges.get(&id).into_iter().flatten() {
            if seen.contains_key(next) {
                continue;
            }
            let mut through = path.clone();
            through.push(next.clone());
            seen.insert(next.clone(), through);
            if Some(next.as_str()) != fence {
                queue.push_back(next.clone());
            }
        }
    }
    seen
}

fn show(graph: &Graph, path: &[String]) -> String {
    path.iter()
        .map(|id| graph.packages[id].name.as_str())
        .collect::<Vec<_>>()
        .join(" -> ")
}

/// The registered desktop in `graph`: the member named
/// `nexus-mother-desktop` whose manifest is `<root>/mother/desktop`.
fn registered_desktop(root: &Path, graph: &Graph) -> Option<String> {
    let manifest = canonical(&root.join(DESKTOP_DIR).join("Cargo.toml"));
    graph
        .members
        .iter()
        .find(|id| {
            let package = &graph.packages[*id];
            package.name == DESKTOP_NAME && canonical(&package.manifest) == manifest
        })
        .cloned()
}

/// What one metadata document of the workspace at `root` breaks.
pub fn findings_of(root: &Path, doc: &Value) -> Vec<String> {
    let graph = match graph(doc) {
        Ok(graph) => graph,
        Err(e) => return vec![format!("unreadable dependency graph: {e}")],
    };
    let desktop = registered_desktop(root, &graph);
    let mut found = BTreeSet::new();
    // Outside the desktop: nothing banned, and never the desktop itself.
    let others: Vec<String> = graph
        .members
        .iter()
        .filter(|id| Some(*id) != desktop.as_ref())
        .cloned()
        .collect();
    for (id, path) in reach(&graph, &others, desktop.as_deref()) {
        let package = &graph.packages[&id];
        if Some(&id) == desktop.as_ref() {
            found.insert(format!(
                "a workspace member depends on {DESKTOP_NAME}: {}",
                show(&graph, &path)
            ));
        } else if banned(&package.name, package.links.as_deref()) {
            found.insert(format!(
                "banned package {} outside the desktop: {}",
                package.name,
                show(&graph, &path)
            ));
        }
    }
    // Under the desktop: the GTK families only, never a webview (G11).
    if let Some(desktop) = &desktop {
        for (id, path) in reach(&graph, std::slice::from_ref(desktop), None) {
            let package = &graph.packages[&id];
            if webview(&package.name, package.links.as_deref()) {
                found.insert(format!(
                    "webview package {} under the desktop: {}",
                    package.name,
                    show(&graph, &path)
                ));
            }
        }
    }
    // A webview anywhere in the document, reached or not (G11).
    for package in graph.packages.values() {
        if webview(&package.name, package.links.as_deref()) {
            found.insert(format!("webview package {} in the graph", package.name));
        }
    }
    found.into_iter().collect()
}

/// Every directory under `dir`, outside build output and VCS metadata.
fn directories(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        let is_dir = entry.file_type().is_ok_and(|kind| kind.is_dir());
        if is_dir && !matches!(name.as_str(), "target" | ".git" | "node_modules") {
            out.push(path.clone());
            directories(&path, out);
        }
    }
}

/// What the tree at `root` breaks: `app/`, any `src-tauri`, and a
/// `mother/desktop` that is not the registered package in `docs`.
pub fn tree_findings(root: &Path, docs: &[Value]) -> Vec<String> {
    let mut found = Vec::new();
    if root.join("app").exists() {
        found.push("app/ exists: the old desktop returned".to_string());
    }
    let mut dirs = Vec::new();
    directories(root, &mut dirs);
    for dir in dirs {
        if dir.file_name().is_some_and(|name| name == "src-tauri") {
            let rel = dir.strip_prefix(root).unwrap_or(&dir);
            found.push(format!("{} exists: a src-tauri tree", rel.display()));
        }
    }
    if root.join(DESKTOP_DIR).exists() {
        let registered = !docs.is_empty()
            && docs.iter().all(|doc| {
                graph(doc)
                    .ok()
                    .is_some_and(|graph| registered_desktop(root, &graph).is_some())
            });
        if !registered {
            found.push(format!(
                "{DESKTOP_DIR} exists but is not the registered {DESKTOP_NAME} member"
            ));
        }
    }
    found
}

/// Every finding for the workspace at `root`, over every configuration.
pub fn findings(root: &Path) -> Vec<String> {
    let manifest = root.join("Cargo.toml");
    let docs: Vec<Value> = CONFIGURATIONS
        .iter()
        .map(|features| resolve(&manifest, *features))
        .collect();
    let mut found = BTreeSet::new();
    for doc in &docs {
        found.extend(findings_of(root, doc));
    }
    found.extend(tree_findings(root, &docs));
    found.into_iter().collect()
}

/// The repository's workspace root: the nearest directory at or above the
/// including crate whose `Cargo.toml` declares `[workspace]`. (A lock file
/// alone is no sign: `kernel/` keeps a stale one.)
pub fn workspace_root() -> PathBuf {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    crate_dir
        .ancestors()
        .find(|dir| {
            std::fs::read_to_string(dir.join("Cargo.toml"))
                .is_ok_and(|text| text.lines().any(|line| line.trim() == "[workspace]"))
        })
        .map(canonical)
        .unwrap_or_else(|| panic!("G0: no workspace above {}", crate_dir.display()))
}

/// The tripwire every guard calls. The workspace is read once per test
/// process.
pub fn assert_no_desktop() {
    static FOUND: OnceLock<Vec<String>> = OnceLock::new();
    let found = FOUND.get_or_init(|| findings(&workspace_root()));
    assert!(
        found.is_empty(),
        "a desktop or webview returned; restore and re-pin its checks: {found:#?}"
    );
}
