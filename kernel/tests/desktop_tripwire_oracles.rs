//! M2b-A PR-1 oracles (design §7.1 G0, G0-r, G11; §11 A2 and A4),
//! registered before the shared desktop tripwire they judge.
//!
//! - G0: the tripwire reads the resolved dependency graph (`cargo metadata
//!   --locked --offline`), with the default features and with all features,
//!   and fails on any banned package by name or by `links`, under any
//!   dependency kind and target, through aliases and wrappers.
//! - The planted forms: one metadata document per form, each of which must
//!   turn the tripwire red.
//! - A2/A4: a real offline Cargo fixture whose banned package is reachable
//!   only through a disabled optional dependency.
//! - G0-r: every tripwire call site still calls the shared helper.
//! - G11: a webview family fails everywhere, including under the desktop.

#[path = "desktop_tripwire/mod.rs"]
mod desktop_tripwire;

use desktop_tripwire::{
    findings, findings_of, metadata_args, resolve, tree_findings, workspace_root, Features,
    CONFIGURATIONS,
};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

// ── Planted metadata documents ──────────────────────────────────────────────

/// A planted root: it does not exist, so nothing on disk can answer for it.
fn planted_root() -> PathBuf {
    PathBuf::from("/nexus-planted-workspace")
}

/// A handwritten `cargo metadata` document: workspace members by directory,
/// registry packages, and resolved edges with their kinds.
struct Doc {
    root: PathBuf,
    ids: BTreeMap<String, String>,
    packages: Vec<Value>,
    members: Vec<String>,
    deps: BTreeMap<String, Vec<Value>>,
}

impl Doc {
    fn new() -> Self {
        Self {
            root: planted_root(),
            ids: BTreeMap::new(),
            packages: Vec::new(),
            members: Vec::new(),
            deps: BTreeMap::new(),
        }
    }

    /// A workspace member named `name` in `dir` (relative to the root).
    fn member(mut self, name: &str, dir: &str) -> Self {
        let path = self.root.join(dir);
        let id = format!("path+file://{}#{name}@0.1.0", path.display());
        self.packages.push(json!({
            "name": name,
            "version": "0.1.0",
            "id": id,
            "links": null,
            "manifest_path": path.join("Cargo.toml"),
        }));
        self.members.push(id.clone());
        self.ids.insert(name.to_string(), id.clone());
        self.deps.insert(id, Vec::new());
        self
    }

    /// A registry package; `links` is its `links` value, if any.
    fn package(mut self, name: &str, links: Option<&str>) -> Self {
        let id = format!("registry+https://github.com/rust-lang/crates.io-index#{name}@1.0.0");
        self.packages.push(json!({
            "name": name,
            "version": "1.0.0",
            "id": id,
            "links": links,
            "manifest_path": format!("/registry/{name}-1.0.0/Cargo.toml"),
        }));
        self.ids.insert(name.to_string(), id.clone());
        self.deps.insert(id, Vec::new());
        self
    }

    /// `from` depends on `to`: `kind` is `None` (normal), `"build"` or
    /// `"dev"`; `target` a platform condition; `rename` the alias under
    /// which `from` names it (`package = …`).
    fn edge(
        mut self,
        from: &str,
        to: &str,
        kind: Option<&str>,
        target: Option<&str>,
        rename: Option<&str>,
    ) -> Self {
        let pkg = self.ids[to].clone();
        let name = rename.unwrap_or(to).replace('-', "_");
        let from = self.ids[from].clone();
        self.deps.get_mut(&from).unwrap().push(json!({
            "name": name,
            "pkg": pkg,
            "dep_kinds": [{ "kind": kind, "target": target }],
        }));
        self
    }

    fn normal(self, from: &str, to: &str) -> Self {
        self.edge(from, to, None, None, None)
    }

    fn build(&self) -> Value {
        let nodes: Vec<Value> = self
            .deps
            .iter()
            .map(|(id, deps)| {
                let ids: Vec<&Value> = deps.iter().map(|d| &d["pkg"]).collect();
                json!({ "id": id, "dependencies": ids, "deps": deps, "features": [] })
            })
            .collect();
        json!({
            "packages": self.packages,
            "workspace_members": self.members,
            "resolve": { "nodes": nodes, "root": null },
            "workspace_root": self.root,
            "version": 1,
        })
    }
}

/// A workspace like the repository's: a few members and a clean tree.
fn base() -> Doc {
    Doc::new()
        .member("nexus-kernel", "kernel")
        .member("nexus-mother", "mother")
        .package("serde", None)
        .normal("nexus-kernel", "serde")
        .normal("nexus-mother", "nexus-kernel")
}

/// The same workspace with the registered desktop.
fn with_desktop() -> Doc {
    base().member("nexus-mother-desktop", "mother/desktop")
}

fn red(doc: &Doc, banned: &str) {
    let found = findings_of(&doc.root, &doc.build());
    assert!(
        found.iter().any(|f| f.contains(banned)),
        "the tripwire must name {banned}: {found:?}"
    );
}

fn green(doc: &Doc) {
    let found = findings_of(&doc.root, &doc.build());
    assert!(found.is_empty(), "no false red: {found:?}");
}

#[test]
fn g0_control_a_clean_graph_is_green() {
    green(&base());
}

#[test]
fn g0_planted_direct_dependency() {
    red(
        &base().package("gtk4", None).normal("nexus-kernel", "gtk4"),
        "gtk4",
    );
}

#[test]
fn g0_planted_alias() {
    red(
        &base()
            .package("gtk4", None)
            .edge("nexus-kernel", "gtk4", None, None, Some("ui")),
        "gtk4",
    );
}

#[test]
fn g0_planted_target_specific_table() {
    red(
        &base().package("gtk4", None).edge(
            "nexus-kernel",
            "gtk4",
            None,
            Some("cfg(target_os = \"linux\")"),
            None,
        ),
        "gtk4",
    );
}

#[test]
fn g0_planted_build_dependency() {
    red(
        &base()
            .package("gtk4", None)
            .edge("nexus-kernel", "gtk4", Some("build"), None, None),
        "gtk4",
    );
}

#[test]
fn g0_planted_dev_dependency() {
    red(
        &base()
            .package("gtk4", None)
            .edge("nexus-kernel", "gtk4", Some("dev"), None, None),
        "gtk4",
    );
}

#[test]
fn g0_planted_sys_crate() {
    red(
        &base()
            .package("gtk4-sys", None)
            .normal("nexus-kernel", "gtk4-sys"),
        "gtk4-sys",
    );
}

#[test]
fn g0_planted_webkit6() {
    red(
        &base()
            .package("webkit6", None)
            .normal("nexus-kernel", "webkit6"),
        "webkit6",
    );
}

#[test]
fn g0_planted_links_only() {
    red(
        &base()
            .package("display-sys", Some("gtk-4"))
            .normal("nexus-kernel", "display-sys"),
        "display-sys",
    );
}

#[test]
fn g0_planted_wrapper_into_a_non_desktop_member() {
    red(
        &base()
            .package("ui-wrapper", None)
            .package("gtk4", None)
            .normal("nexus-mother", "ui-wrapper")
            .normal("ui-wrapper", "gtk4"),
        "gtk4",
    );
}

// ── The exemption: the registered desktop only, the gtk families only ──────

#[test]
fn g0_the_registered_desktop_may_bring_the_gtk_families() {
    green(
        &with_desktop()
            .package("gtk4", None)
            .package("gtk4-sys", Some("gtk4"))
            .package("gdk4", None)
            .package("gsk4", None)
            .package("glib", Some("glib-2.0"))
            .normal("nexus-mother-desktop", "gtk4")
            .normal("nexus-mother-desktop", "nexus-mother")
            .normal("gtk4", "gtk4-sys")
            .normal("gtk4", "gdk4")
            .normal("gtk4", "gsk4")
            .normal("gtk4", "glib"),
    );
}

#[test]
fn g0_a_gtk_family_also_reached_outside_the_desktop_is_red() {
    red(
        &with_desktop()
            .package("gtk4", None)
            .normal("nexus-mother-desktop", "gtk4")
            .normal("nexus-kernel", "gtk4"),
        "gtk4",
    );
}

#[test]
fn g0_a_member_that_depends_on_the_desktop_is_red() {
    red(
        &with_desktop()
            .package("gtk4", None)
            .normal("nexus-mother-desktop", "gtk4")
            .normal("nexus-mother", "nexus-mother-desktop"),
        "nexus-mother-desktop",
    );
}

#[test]
fn g0_an_impostor_desktop_gets_no_exemption() {
    red(
        &base()
            .member("nexus-mother-desktop", "tools/desktop")
            .package("gtk4", None)
            .normal("nexus-mother-desktop", "gtk4"),
        "gtk4",
    );
}

// ── G11: the webview families fail everywhere, the desktop included ─────────

#[test]
fn g11_every_webview_family_fails_under_the_desktop() {
    for name in [
        "webkit6",
        "webkit2gtk",
        "javascriptcore-rs",
        "soup3",
        "wry",
        "tauri",
    ] {
        red(
            &with_desktop()
                .package(name, None)
                .normal("nexus-mother-desktop", name),
            name,
        );
    }
}

#[test]
fn g11_a_webview_link_fails_under_the_desktop() {
    red(
        &with_desktop()
            .package("view-sys", Some("webkit2gtk-4.1"))
            .normal("nexus-mother-desktop", "view-sys"),
        "view-sys",
    );
}

#[test]
fn g11_a_webview_behind_a_gtk_wrapper_under_the_desktop_fails() {
    red(
        &with_desktop()
            .package("gtk4", None)
            .package("webkit2gtk", None)
            .normal("nexus-mother-desktop", "gtk4")
            .normal("gtk4", "webkit2gtk"),
        "webkit2gtk",
    );
}

#[test]
fn g11_a_webview_package_anywhere_in_the_document_fails() {
    red(&base().package("wry", None), "wry");
}

// ── The tree: no app/, no src-tauri, mother/desktop only as registered ─────

struct Tree(PathBuf);

impl Tree {
    fn new(label: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "nexus-g0-tree-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(dir.join("kernel/src")).unwrap();
        Self(dir)
    }

    fn with(self, rel: &str) -> Self {
        let path = self.0.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, "planted\n").unwrap();
        self
    }

    /// The planted documents' paths, rebased onto this tree.
    fn doc(&self, doc: Doc) -> Value {
        let text = doc.build().to_string();
        let root = planted_root();
        serde_json::from_str(&text.replace(&*root.to_string_lossy(), &self.0.to_string_lossy()))
            .unwrap()
    }
}

impl Drop for Tree {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn g0_tree_control_a_clean_tree_is_green() {
    let tree = Tree::new("clean").with("kernel/src/lib.rs");
    let docs = [tree.doc(base())];
    assert_eq!(tree_findings(&tree.0, &docs), Vec::<String>::new());
}

#[test]
fn g0_tree_app_fails() {
    let tree = Tree::new("app").with("app/README.md");
    let found = tree_findings(&tree.0, &[tree.doc(base())]);
    assert!(found.iter().any(|f| f.contains("app")), "{found:?}");
}

#[test]
fn g0_tree_src_tauri_anywhere_fails() {
    let tree = Tree::new("tauri").with("crates/view/src-tauri/build.rs");
    let found = tree_findings(&tree.0, &[tree.doc(base())]);
    assert!(found.iter().any(|f| f.contains("src-tauri")), "{found:?}");
}

#[test]
fn g0_tree_an_unregistered_mother_desktop_fails() {
    let tree = Tree::new("unregistered").with("mother/desktop/Cargo.toml");
    let found = tree_findings(&tree.0, &[tree.doc(base())]);
    assert!(
        found.iter().any(|f| f.contains("mother/desktop")),
        "{found:?}"
    );
    let renamed = base().member("desktop", "mother/desktop");
    let found = tree_findings(&tree.0, &[tree.doc(renamed)]);
    assert!(
        found.iter().any(|f| f.contains("mother/desktop")),
        "{found:?}"
    );
}

#[test]
fn g0_tree_the_registered_mother_desktop_is_green() {
    let tree = Tree::new("registered").with("mother/desktop/Cargo.toml");
    let docs = [tree.doc(with_desktop())];
    assert_eq!(tree_findings(&tree.0, &docs), Vec::<String>::new());
}

// ── A2 and A4: the real offline Cargo fixture ───────────────────────────────

fn fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/desktop_tripwire/fixture")
        .canonicalize()
        .unwrap()
}

fn names(doc: &Value) -> Vec<String> {
    doc["packages"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["name"].as_str().unwrap().to_string())
        .collect()
}

fn member_names(doc: &Value) -> Vec<String> {
    let members: Vec<&str> = doc["workspace_members"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m.as_str().unwrap())
        .collect();
    doc["packages"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|p| members.contains(&p["id"].as_str().unwrap()))
        .map(|p| p["name"].as_str().unwrap().to_string())
        .collect()
}

#[test]
fn g0_metadata_is_read_locked_offline_in_both_configurations() {
    assert_eq!(CONFIGURATIONS, [Features::Default, Features::All]);
    for features in CONFIGURATIONS {
        let args = metadata_args(features);
        for required in ["metadata", "--locked", "--offline", "--format-version", "1"] {
            assert!(args.contains(&required), "{features:?}: {args:?}");
        }
        assert!(!args.contains(&"--no-default-features"), "{args:?}");
        assert!(
            !args.iter().any(|a| a.starts_with("--filter-platform")),
            "every target is checked: {args:?}"
        );
    }
    assert!(metadata_args(Features::All).contains(&"--all-features"));
    assert!(!metadata_args(Features::Default).contains(&"--all-features"));
}

#[test]
fn g0_a2_a4_a_disabled_optional_wrapper_reaches_webkit6_only_with_all_features() {
    let root = fixture_root();
    let manifest = root.join("Cargo.toml");
    let default = resolve(&manifest, Features::Default);
    let all = resolve(&manifest, Features::All);
    // A4: the wrapper and the stub are not members.
    assert_eq!(member_names(&default), ["fixture-member"]);
    assert_eq!(member_names(&all), ["fixture-member"]);
    // A4: the default-feature resolution lacks the banned path.
    assert!(
        !names(&default).iter().any(|n| n == "webkit6"),
        "{:?}",
        names(&default)
    );
    assert_eq!(findings_of(&root, &default), Vec::<String>::new());
    // A4: the all-features resolution contains it, through the wrapper.
    for name in ["fixture-wrapper", "webkit6"] {
        assert!(
            names(&all).iter().any(|n| n == name),
            "{name}: {:?}",
            names(&all)
        );
    }
    let found = findings_of(&root, &all);
    assert!(found.iter().any(|f| f.contains("webkit6")), "{found:?}");
    // A2: the tripwire's own union finds it.
    let found = findings(&root);
    assert!(found.iter().any(|f| f.contains("webkit6")), "{found:?}");
}

#[test]
fn g0_the_workspace_has_no_desktop() {
    let root = workspace_root();
    assert!(root.join("Cargo.lock").is_file(), "{}", root.display());
    assert_eq!(findings(&root), Vec::<String>::new());
}

// ── G0-r: every tripwire call site calls the shared helper ──────────────────

/// The 11 desktop tripwire call sites (design F22), the coding-run guard and
/// Mother's scan: each named test must call the shared helper.
const SITES: [(&str, &str); 13] = [
    (
        "kernel/tests/hm_m3_guards.rs",
        "hm_m3_n5_a_s1_no_ipc_writes_hm_no_replace_and_foreign_keys_on",
    ),
    (
        "kernel/tests/hm_m3_guards.rs",
        "hm_m3_n5_a_s3_single_issuer_attestation_admission_and_store",
    ),
    (
        "kernel/tests/hm_m3_guards.rs",
        "hm_m3_n5_a_s6_separated_subsystems_receive_no_hm_content",
    ),
    (
        "kernel/tests/hm_m3_guards.rs",
        "hm_m3_n5_r7_no_unlabelled_in_scope_content_write",
    ),
    (
        "kernel/tests/hm_m3_guards.rs",
        "hm_m3_n5_i4_no_checkpoint_memory_is_written_back",
    ),
    (
        "kernel/tests/hm_m3_guards.rs",
        "hm_m4_n2_the_only_production_route_policy_confirmer_is_the_native_dialog",
    ),
    (
        "kernel/tests/hm_m3_guards.rs",
        "hm_m3_n5_a_s6_xs_crates_reach_no_hm_content",
    ),
    (
        "kernel/tests/hm_m3_guards.rs",
        "hm_m3_n5_a3_2_the_desktop_store_fixture_module_is_test_only",
    ),
    (
        "persistence/tests/hm_a1_storage.rs",
        "hm_n5_hm_tables_are_named_only_in_the_hm_modules",
    ),
    (
        "persistence/tests/hm_m2_guards.rs",
        "hm_m2_n7_hm_store_is_called_only_from_the_hm_modules",
    ),
    (
        "persistence/tests/hm_m2_guards.rs",
        "hm_m2_n7_every_identity_home_connection_enables_foreign_keys",
    ),
    (
        "kernel/src/coding_run/tests.rs",
        "p1a_nc_30_no_production_ipc_exposes_the_coding_run_primitive",
    ),
    (
        "mother/src/adapter/scan.rs",
        "scan_m2b_no_desktop_outside_the_registered_package",
    ),
];

/// The text of the top-level test `name` in `text`: from its `#[test]` to
/// its closing brace at column 0, comment lines removed.
fn test_body(text: &str, name: &str) -> Option<String> {
    let start = text.find(&format!("\nfn {name}()"))?;
    let before = &text[..start];
    let attribute = before.trim_end().ends_with("#[test]");
    let rest = &text[start + 1..];
    let end = rest.find("\n}\n")?;
    let body: Vec<&str> = rest[..end]
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect();
    attribute.then(|| body.join("\n"))
}

const CALL: &str = "desktop_tripwire::assert_no_desktop();";

#[test]
fn g0_r_every_tripwire_site_calls_the_shared_helper() {
    let root = workspace_root();
    let mut missing = Vec::new();
    for (file, test) in SITES {
        let text = std::fs::read_to_string(root.join(file)).unwrap();
        let included = text.lines().any(|line| {
            line.trim_start().starts_with("#[path")
                && line.trim_end().ends_with("desktop_tripwire/mod.rs\"]")
        });
        match test_body(&text, test) {
            Some(body) if included && body.contains(CALL) => {}
            Some(_) => missing.push(format!("{file}: {test} does not call the helper")),
            None => missing.push(format!("{file}: no #[test] fn {test}")),
        }
    }
    assert!(missing.is_empty(), "{missing:#?}");
}

#[test]
fn g0_r_the_body_reader_sees_one_test_at_a_time() {
    let text = "use x;\n\n#[test]\nfn a() {\n    // desktop_tripwire::assert_no_desktop();\n    x();\n}\n\n#[test]\nfn b() {\n    desktop_tripwire::assert_no_desktop();\n}\n\nfn c() {\n    desktop_tripwire::assert_no_desktop();\n}\n";
    assert!(!test_body(text, "a").unwrap().contains(CALL));
    assert!(test_body(text, "b").unwrap().contains(CALL));
    assert_eq!(test_body(text, "c"), None, "not a test");
    assert_eq!(test_body(text, "d"), None);
}

#[test]
fn g0_r_no_local_tripwire_survives() {
    let root = workspace_root();
    let old = ["fn assert_desktop", "_removed("].concat();
    for (file, _) in SITES {
        let text = std::fs::read_to_string(root.join(file)).unwrap();
        assert!(!text.contains(&old), "{file} keeps its own tripwire");
    }
}
