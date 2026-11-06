//! The approved K02–K22 inventory, checked before any production extraction.
#[allow(dead_code)]
#[path = "retirement/coverage.rs"]
mod coverage;
#[allow(dead_code)]
mod retirement;
#[allow(dead_code)]
#[path = "hm_m3_guards/source.rs"]
mod source;
use retirement::*;
use std::collections::BTreeSet;

fn check_extraction_anchors(stage: u8, row: &Stage, read_home: impl Fn(&str) -> String) {
    for extraction in &row.extract {
        let path = extraction_home(stage, row.stage, extraction);
        let text = read_home(path);
        for anchor in &extraction.anchors {
            assert!(
                coverage::active_anchor(&text, anchor),
                "{path}: missing {anchor}"
            );
        }
    }
}

fn module_attributes(declaration: &str) -> Vec<String> {
    if declaration == "pub mod test_support;" {
        vec!["#[cfg(any(test,feature=\"test-utils\"))]".into()]
    } else {
        Vec::new()
    }
}

fn rust_files(path: &std::path::Path, out: &mut BTreeSet<String>) {
    let meta = std::fs::symlink_metadata(path).expect("registered path readable");
    assert!(
        !meta.file_type().is_symlink(),
        "manifest paths cannot alias"
    );
    if meta.is_dir() {
        for entry in std::fs::read_dir(path).unwrap() {
            rust_files(&entry.unwrap().path(), out);
        }
    } else if path.extension().is_some_and(|e| e == "rs") {
        out.insert(
            path.strip_prefix(root())
                .unwrap()
                .to_str()
                .unwrap()
                .replace('\\', "/"),
        );
    }
}

#[test]
fn exact_retirement_manifest_matches_current_tree_and_keep_boundary() {
    let rows = stages();
    assert_eq!(
        rows.iter().map(|r| r.stage).collect::<Vec<_>>(),
        (2..=22).collect::<Vec<_>>()
    );
    let keep: Vec<_> = include_str!("retirement/keep.txt").lines().collect();
    assert_eq!(keep.len(), 21);
    let mut seen = BTreeSet::new();
    let mut roots = 0;
    for row in rows {
        let mut actual = BTreeSet::new();
        for rel in &row.roots {
            assert!(rel.starts_with("kernel/src/"));
            assert!(!rel.contains(".."));
            assert!(!keep.iter().any(|k| within(rel, k)), "KEEP removal: {rel}");
            if row.stage > LIVE_STAGE {
                rust_files(&root().join(rel), &mut actual);
            } else {
                assert!(!root().join(rel).exists(), "retired path restored: {rel}");
            }
            roots += 1;
        }
        if row.stage > LIVE_STAGE {
            assert_eq!(
                actual,
                row.paths.iter().cloned().collect(),
                "K{:02} file inventory",
                row.stage
            );
        }
        for rel in &row.paths {
            assert!(seen.insert(rel.clone()), "duplicate retirement: {rel}");
        }
        assert_eq!(
            row.declarations.len(),
            row.roots
                .iter()
                .map(|p| p.rsplit('/').next().unwrap().trim_end_matches(".rs"))
                .collect::<BTreeSet<_>>()
                .len()
        );
        let lib = read("kernel/src/lib.rs");
        for declaration in &row.declarations {
            assert_eq!(
                coverage::root_module(
                    &lib,
                    declaration
                        .trim_start_matches("pub mod ")
                        .trim_end_matches(';')
                )
                .is_some_and(|c| c.attributes == module_attributes(declaration)
                    && c.head == compact(declaration)),
                row.stage > LIVE_STAGE,
                "{declaration}"
            );
        }
        for export in &row.reexports {
            if row.stage > LIVE_STAGE {
                assert!(
                    compact(&source::mask(&read("kernel/src/cognitive/mod.rs"), false))
                        .contains(&compact(export))
                );
            }
        }
        for item in &row.items {
            assert_eq!(
                coverage::active_anchor(&read(&item.path), &item.anchor),
                row.stage > LIVE_STAGE,
                "{}",
                item.anchor
            );
        }
    }
    assert_eq!(roots, 34, "every §4 physical candidate is registered");
    assert_eq!(
        seen.len(),
        76,
        "no silent omission from a candidate directory"
    );
    assert!(rows
        .last()
        .unwrap()
        .closure
        .as_ref()
        .unwrap()
        .contains("No old-path"));
    for rel in keep {
        assert!(root().join(rel).exists(), "KEEP omitted: {rel}");
    }
}

#[test]
fn every_extraction_payload_and_temporary_export_has_a_named_home() {
    let rows = stages();
    let mut definitions = BTreeSet::new();
    let mut payloads = 0;
    let mut temporary_helpers = 0;
    for row in rows {
        check_extraction_anchors(LIVE_STAGE, row, read);
        for extraction in &row.extract {
            assert!(extraction.destination.starts_with("kernel/src/"));
            assert!(!extraction.destination.contains(".."));
            assert_ne!(extraction.source, extraction.destination);
            assert!(!extraction.payload.is_empty());
            assert!(!extraction.anchors.is_empty());
            let path = extraction_home(LIVE_STAGE, row.stage, extraction);
            let text = read(path);
            if row.stage <= LIVE_STAGE && root().join(&extraction.source).is_file() {
                let old = compact(&source::mask(&read(&extraction.source), false));
                for export in &extraction.transitional_exports {
                    assert!(
                        old.contains(&compact(export)),
                        "missing same-type export: {export}"
                    );
                }
            }
            for temporary in &extraction.temporary {
                temporary_helpers += 1;
                assert_eq!(temporary.anchor, "pub fn required_capabilities");
                assert_eq!(
                    temporary.until, 12,
                    "last helper consumer retires with actuators"
                );
                assert_eq!(
                    coverage::active_anchor(&text, &temporary.anchor),
                    LIVE_STAGE < temporary.until
                );
            }
            for name in &extraction.payload {
                payloads += 1;
                if name.chars().next().unwrap().is_uppercase()
                    && !name.contains(' ')
                    && !name.contains("::")
                {
                    definitions.insert(name.as_str());
                    assert!(extraction
                        .transitional_exports
                        .iter()
                        .any(|e| e.ends_with(&format!("::{name};"))));
                }
            }
        }
    }
    assert_eq!(
        temporary_helpers, 1,
        "the temporary helper cannot be omitted"
    );
    assert_eq!(
        definitions,
        BTreeSet::from([
            "AgentId",
            "FsPermissionLevel",
            "BrowserAction",
            "PlannedAction",
            "ScreenRegion",
            "FuelViolation",
            "AgentState",
            "BackupScheduleConfig",
            "RateLimitConfig",
            "ApiHardeningConfig"
        ])
    );
    assert_eq!(
        payloads, 34,
        "all codecs/default helpers/temporary capability helper named"
    );
}

#[test]
fn retained_production_seeds_and_dependency_manifests_are_present() {
    let paths = production_paths();
    // K02 registration: 327 K01 seeds plus the three one-line desktop-tripwire
    // fixture crate sources the production scanner has always loaded (see
    // README, "K02 registration"). No seed was removed or replaced.
    assert_eq!(paths.len(), 330);
    for path in paths_at(LIVE_STAGE) {
        assert!(root().join(&path).is_file(), "missing stage seed: {path}");
    }
    assert_eq!(cargo_manifests().len(), 10);
    for path in cargo_manifests() {
        assert!(root().join(&path).is_file(), "missing manifest: {path}");
    }
    dependencies::assert_live();
}

#[test]
fn manifest_presence_requires_real_ungated_declarations() {
    assert!(coverage::root_module(
        "#[cfg(any(test, feature = \"test - utils\"))] pub mod test_support;",
        "test_support"
    )
    .is_none_or(|c| c.attributes != module_attributes("pub mod test_support;")));
    let baseline = "#[cfg(target_os = \"linux\")] #[test] fn protected() {}";
    assert_ne!(
        coverage::test_contract(baseline, "protected"),
        coverage::test_contract(&baseline.replace("linux", "lin ux"), "protected")
    );
    for row in stages() {
        check_extraction_anchors(LIVE_STAGE, row, read);
        for declaration in &row.declarations {
            let name = declaration
                .trim_start_matches("pub mod ")
                .trim_end_matches(';');
            let present = |raw: &str| {
                coverage::root_module(raw, name).is_some_and(|c| {
                    c.attributes == module_attributes(declaration) && c.head == compact(declaration)
                })
            };
            let live = format!("{} {declaration}", module_attributes(declaration).join(" "));
            assert!(present(&live));
            for mutant in [
                format!("// {declaration}"),
                format!("/* {declaration} */"),
                format!("#[cfg(any())] {declaration}"),
                format!("#![cfg(any())] {declaration}"),
                format!("const NOTE: &str = \"{declaration}\";"),
                format!("macro_rules! unused {{ () => {{ {declaration} }} }}"),
            ] {
                assert!(!present(&mutant), "{mutant}");
            }
        }
    }
}

#[test]
fn extraction_anchor_controls_accept_k02_same_type_exports_and_later_source_deletion() {
    let row = &stages()[0];
    assert_eq!(row.stage, 2);
    // Independent synthetic declarations remain usable after the old sources
    // are deleted; their serialized behavior has separate live goldens.
    let permissions = "pub enum FsPermissionLevel { ReadOnly, ReadWrite, Deny }";
    let mut tree = std::collections::BTreeMap::from([
        (
            "kernel/src/supervisor.rs",
            "pub use crate::agent_identity::AgentId;".to_owned(),
        ),
        (
            "kernel/src/agent_identity.rs",
            "use uuid::Uuid;\npub type AgentId = Uuid;".to_owned(),
        ),
        (
            "kernel/src/manifest.rs",
            "pub use crate::workspace_permissions::FsPermissionLevel;".to_owned(),
        ),
        (
            "kernel/src/workspace_permissions.rs",
            permissions.to_owned(),
        ),
    ]);
    check_extraction_anchors(2, row, |p| tree[p].clone());
    for extraction in &row.extract {
        let old = compact(&source::mask(&tree[extraction.source.as_str()], false));
        for export in &extraction.transitional_exports {
            assert!(old.contains(&compact(export)));
        }
        for anchor in &extraction.anchors {
            assert!(!coverage::active_anchor(
                &tree[extraction.source.as_str()],
                anchor
            ));
        }
    }
    tree.remove("kernel/src/supervisor.rs");
    tree.remove("kernel/src/manifest.rs");
    check_extraction_anchors(19, row, |p| tree[p].clone());
    // The same shared oracle rejects an empty destination even if the original
    // declaration is restored in its old source home.
    tree.insert(
        "kernel/src/supervisor.rs",
        "pub type AgentId = Uuid;".into(),
    );
    tree.insert("kernel/src/agent_identity.rs", String::new());
    assert!(
        std::panic::catch_unwind(|| check_extraction_anchors(2, row, |p| tree[p].clone())).is_err()
    );
}

#[test]
fn payload_presence_rejects_comments_literals_macros_and_disabling_cfg() {
    for (declaration, anchor) in [
        ("pub enum IdentityExtra {}", "pub enum Identity"),
        ("fn deserialize_disabled() {}", "fn deserialize"),
        ("impl Default for RegionExtra {}", "impl Default for Region"),
    ] {
        assert!(!coverage::active_anchor(declaration, anchor));
    }
    for (declaration, anchor) in [
        ("pub enum Identity { A }", "pub enum Identity"),
        ("pub struct Region { x: u32 }", "pub struct Region"),
        ("pub type AgentId = Uuid;", "pub type AgentId = Uuid;"),
        (
            "impl Default for Region { fn default() -> Self { todo!() } }",
            "impl Default for Region",
        ),
        (
            "pub(crate) mod codec { pub fn deserialize() {} }",
            "pub fn deserialize",
        ),
    ] {
        assert!(coverage::active_anchor(declaration, anchor));
        for mutant in [
            format!("/* {declaration} */"),
            format!("#[cfg(any())] {declaration}"),
            format!("#![cfg(any())] {declaration}"),
            format!("const NOTE: &str = {declaration:?};"),
            format!("macro_rules! unused {{ () => {{ {declaration} }} }}"),
        ] {
            assert!(!coverage::active_anchor(&mutant, anchor), "{mutant}");
        }
    }
}

// Cargo, rather than these controls, decides which packages are implicit
// members and what renamed/target-specific edges resolve to. Fixtures model
// that OUTPUT; they do not parse or simulate Cargo.toml membership rules.
fn metadata_fixture() -> serde_json::Value {
    use serde_json::json;
    json!({
        "packages": [
            {"id":"kernel", "name":"nexus-kernel", "manifest_path":root().join("kernel/Cargo.toml"),
             "dependencies":[{"name":"serde", "rename":null}]},
            {"id":"host", "name":"nexus-mother-host", "manifest_path":root().join("mother/host/Cargo.toml"),
             "dependencies":[]}
        ],
        "workspace_members":["kernel", "host"],
        "resolve":{"nodes":[{"id":"kernel", "deps":[]}, {"id":"host", "deps":[]}]}
    })
}

fn add_metadata_package(doc: &mut serde_json::Value, name: &str, path: &str) {
    use serde_json::json;
    doc["packages"].as_array_mut().unwrap().push(json!({
        "id":"added", "name":name, "manifest_path":root().join(path), "dependencies":[]
    }));
    doc["resolve"]["nodes"]
        .as_array_mut()
        .unwrap()
        .push(json!({"id":"added", "deps":[]}));
}

#[test]
fn cargo_metadata_real_graph_and_synthetic_baseline_pass() {
    dependencies::assert_live();
    assert!(dependencies::findings_of(&root(), &metadata_fixture()).is_empty());
}

#[test]
fn cargo_metadata_rejects_retired_packages_including_implicit_members_and_renamed_target_edges() {
    use serde_json::json;
    for (name, _) in dependencies::RETIRED {
        for kind in [json!(null), json!("dev"), json!("build")] {
            for target in [json!(null), json!("cfg(windows)")] {
                let mut doc = metadata_fixture();
                // Cargo's implicit helper member is present in workspace_members
                // even though no explicit workspace.members entry named it.
                doc["packages"].as_array_mut().unwrap().push(json!({
                    "id":"helper", "name":"helper", "manifest_path":root().join("crates/helper/Cargo.toml"),
                    "dependencies":[{"name":name, "rename":"innocent_alias", "kind":kind, "target":target}]
                }));
                doc["workspace_members"]
                    .as_array_mut()
                    .unwrap()
                    .push(json!("helper"));
                doc["resolve"]["nodes"].as_array_mut().unwrap().push(json!({"id":"helper", "deps":[
                    {"name":"innocent_alias", "pkg":"added", "dep_kinds":[{"kind":kind, "target":target}]}
                ]}));
                doc["resolve"]["nodes"][1]["deps"] = json!([
                    {"name":"helper", "pkg":"helper", "dep_kinds":[{"kind":null, "target":null}]}
                ]);
                assert!(dependencies::member_manifests_of(&root(), &doc)
                    .contains("crates/helper/Cargo.toml"));
                // A normal-looking manifest path ensures package identity alone
                // catches a retired crate behind the resolved alias.
                add_metadata_package(&mut doc, name, "crates/innocent/Cargo.toml");
                assert_eq!(
                    dependencies::findings_of(&root(), &doc),
                    BTreeSet::from([format!(
                        "retired package {name} at {}",
                        root().join("crates/innocent/Cargo.toml").display()
                    )])
                );
                // Retired identity is also prohibited as an implicit member.
                doc["workspace_members"]
                    .as_array_mut()
                    .unwrap()
                    .push(json!("added"));
                assert_eq!(dependencies::findings_of(&root(), &doc).len(), 1);
            }
        }
    }
}

#[test]
fn cargo_metadata_rejects_packages_under_every_retired_root_with_component_boundaries() {
    let roots = dependencies::RETIRED.iter().map(|(_, path)| *path).chain(
        stages()
            .iter()
            .flat_map(|row| &row.roots)
            .map(String::as_str),
    );
    for retired in roots {
        let mut doc = metadata_fixture();
        let path = format!("{retired}/nested/Cargo.toml");
        add_metadata_package(&mut doc, "innocent", &path);
        assert_eq!(
            dependencies::findings_of(&root(), &doc),
            BTreeSet::from([format!(
                "retired package innocent at {}",
                root().join(path).display()
            )])
        );
        doc["packages"][2]["manifest_path"] =
            serde_json::json!(root().join(format!("{retired}-neighbor/Cargo.toml")));
        assert!(dependencies::findings_of(&root(), &doc).is_empty());
    }
}

#[test]
fn cargo_metadata_kernel_direct_names_may_shrink_but_new_names_or_renames_fail() {
    use serde_json::json;
    for kind in [json!(null), json!("dev"), json!("build")] {
        for target in [json!(null), json!("cfg(windows)")] {
            for rename in [json!(null), json!("brand_new_kernel_name")] {
                let mut doc = metadata_fixture();
                doc["packages"][0]["dependencies"] = json!([{
                    "name": if rename.is_null() { "brand_new_kernel_name" } else { "serde" },
                    "rename":rename, "kind":kind, "target":target
                }]);
                assert_eq!(
                    dependencies::findings_of(&root(), &doc),
                    BTreeSet::from([
                        "kernel dependency beyond frozen cap: brand_new_kernel_name".into()
                    ])
                );
            }
        }
    }
    let mut doc = metadata_fixture();
    doc["packages"][0]["dependencies"] = json!([]);
    assert!(dependencies::findings_of(&root(), &doc).is_empty());
    doc["packages"][1]["dependencies"] = json!([{"name":"new_host_helper", "rename":null}]);
    assert!(dependencies::findings_of(&root(), &doc).is_empty());
}

#[test]
fn cargo_metadata_missing_malformed_or_inconsistent_graphs_fail_closed() {
    use serde_json::json;
    for key in ["packages", "workspace_members", "resolve"] {
        let mut doc = metadata_fixture();
        doc.as_object_mut().unwrap().remove(key);
        assert!(
            !dependencies::findings_of(&root(), &doc).is_empty(),
            "{key}"
        );
    }
    for (pointer, value) in [
        ("/resolve", json!(null)),
        ("/workspace_members", json!([])),
        ("/workspace_members", json!(["unknown"])),
        ("/packages/0/manifest_path", json!("kernel/Cargo.toml")),
        ("/packages/0/dependencies", json!(null)),
        ("/resolve/nodes/0/id", json!("unknown")),
        (
            "/resolve/nodes/0/deps",
            json!([{"pkg":"unknown", "dep_kinds":[{"kind":null,"target":null}]}]),
        ),
        (
            "/resolve/nodes/0/deps",
            json!([{"pkg":"host", "dep_kinds":[]}]),
        ),
    ] {
        let mut doc = metadata_fixture();
        *doc.pointer_mut(pointer).unwrap() = value;
        assert!(
            !dependencies::findings_of(&root(), &doc).is_empty(),
            "{pointer}"
        );
    }
}
