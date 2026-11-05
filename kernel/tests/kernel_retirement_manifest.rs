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
    assert_eq!(paths.len(), 327);
    for path in paths_at(LIVE_STAGE) {
        assert!(root().join(&path).is_file(), "missing stage seed: {path}");
    }
    let manifests = dependencies::load_manifests();
    assert_eq!(cargo_manifests().len(), 10);
    for path in cargo_manifests() {
        assert!(manifests.contains_key(&path), "missing manifest: {path}");
    }
    assert_eq!(
        dependencies::violations(LIVE_STAGE, &manifests),
        BTreeSet::new(),
        "retirement dependency policy"
    );
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

#[test]
fn dependency_policy_rejects_retired_names_aliases_and_all_dependency_kinds() {
    let baseline = dependencies::load_manifests();
    assert!(dependencies::violations(1, &baseline).is_empty());
    for path in baseline.keys() {
        for kind in ["dependencies", "dev-dependencies", "build-dependencies"] {
            for prefix in [String::new(), "target.'cfg(target_os = \"never\")'.".into()] {
                for (alias, spec) in [
                    ("nexus-sdk", "version = '1'"),
                    ("innocent_alias", "package = 'nexus-sdk'\nversion = '1'"),
                ] {
                    let mut changed = baseline.clone();
                    changed
                        .get_mut(path)
                        .unwrap()
                        .push_str(&format!("\n[{prefix}{kind}.{alias}]\n{}\n", spec));
                    assert!(
                        dependencies::violations(1, &changed).contains(path),
                        "{path}: {prefix}{kind}.{alias}"
                    );
                }
            }
        }
    }
    // A workspace alias is resolved through the root declaration even when
    // inherited by a member; neither table may reintroduce a retired package.
    let mut changed = baseline.clone();
    changed
        .get_mut("Cargo.toml")
        .unwrap()
        .push_str("\n[workspace.dependencies.alias]\npackage = 'nexus-memory'\nversion = '1'\n");
    changed
        .get_mut("mother/host/Cargo.toml")
        .unwrap()
        .push_str("\n[dependencies.alias]\nworkspace = true\n");
    assert!(dependencies::violations(1, &changed).contains("Cargo.toml"));
}

#[test]
fn dependency_policy_checks_retired_paths_and_every_registered_retirement_root() {
    let baseline = dependencies::load_manifests();
    let path = "mother/host/Cargo.toml";
    for retired in [
        "sdk",
        "connectors/llm",
        "agents/web-builder",
        "crates/nexus-memory",
        "crates/nexus-self-improve",
        "crates/nexus-outcome-eval",
    ] {
        for dependency in [
            format!("../../{retired}/nested"),
            format!("../../mother/../{retired}/nested"),
            root()
                .join(retired)
                .join("nested")
                .to_str()
                .unwrap()
                .to_owned(),
        ] {
            let mut changed = baseline.clone();
            changed.get_mut(path).unwrap().push_str(&format!(
                "\n[build-dependencies.alias]\npath = {dependency:?}\n"
            ));
            assert!(
                dependencies::violations(1, &changed).contains(path),
                "{dependency}"
            );
        }
    }
    for row in stages() {
        for retired in &row.roots {
            let mut changed = baseline.clone();
            changed.get_mut(path).unwrap().push_str(&format!("\n[target.'cfg(windows)'.dev-dependencies.alias]\npath = '../../{retired}/nested'\n"));
            assert!(
                dependencies::violations(1, &changed).contains(path),
                "K01: {retired}"
            );
            assert!(
                dependencies::violations(row.stage, &changed).contains(path),
                "K{}: {retired}",
                row.stage
            );
        }
    }
    // Component boundaries do not prohibit an unrelated neighboring path.
    let mut changed = baseline;
    changed
        .get_mut(path)
        .unwrap()
        .push_str("\n[dependencies.alias]\npath = '../../sdk-tools'\n");
    assert!(dependencies::violations(22, &changed).is_empty());
}

#[test]
fn dependency_policy_allows_legitimate_manifest_changes_and_kernel_shrinkage() {
    let baseline = dependencies::load_manifests();
    for stage in [1, 2, 11, 22] {
        let mut changed = baseline.clone();
        // The merged #83 nix/uuid content is already in the green baseline.
        changed
            .get_mut("mother/host/Cargo.toml")
            .unwrap()
            .push_str("\n[dev-dependencies.new_host_helper]\nversion = '1'\n");
        assert!(dependencies::violations(stage, &changed).is_empty());
        let mut kernel: toml::Value = toml::from_str(&changed["kernel/Cargo.toml"]).unwrap();
        kernel["dependencies"]
            .as_table_mut()
            .unwrap()
            .remove("htmd")
            .unwrap();
        changed.insert(
            "kernel/Cargo.toml".into(),
            toml::to_string(&kernel).unwrap(),
        );
        assert!(dependencies::violations(stage, &changed).is_empty());
        for kind in ["dependencies", "dev-dependencies", "build-dependencies"] {
            let mut added = changed.clone();
            added
                .get_mut("kernel/Cargo.toml")
                .unwrap()
                .push_str(&format!(
                    "\n[target.'cfg(windows)'.{kind}.brand_new_kernel_name]\nversion = '1'\n"
                ));
            assert_eq!(
                dependencies::violations(stage, &added),
                BTreeSet::from(["kernel/Cargo.toml".into()])
            );
        }
    }
}

#[test]
fn dependency_policy_covers_new_workspace_members_and_fails_on_missing_or_invalid_manifests() {
    let mut changed = dependencies::load_manifests();
    let mut workspace: toml::Value = toml::from_str(&changed["Cargo.toml"]).unwrap();
    workspace["workspace"]["members"]
        .as_array_mut()
        .unwrap()
        .push(toml::Value::String("new-member".into()));
    changed.insert("Cargo.toml".into(), toml::to_string(&workspace).unwrap());
    assert!(dependencies::violations(1, &changed).contains("Cargo.toml"));
    let path = "new-member/Cargo.toml";
    changed.insert(
        path.into(),
        "[package]\nname = 'new-member'\nversion = '1.0.0'\n".into(),
    );
    assert!(dependencies::violations(1, &changed).is_empty());
    changed.get_mut(path).unwrap().push_str("\n[target.'cfg(windows)'.dependencies.alias]\npackage = 'nexus-connectors-llm'\nversion = '1'\n");
    assert!(dependencies::violations(1, &changed).contains(path));
    let mut globbed = changed.clone();
    let mut root_manifest: toml::Value = toml::from_str(&globbed["Cargo.toml"]).unwrap();
    *root_manifest["workspace"]["members"]
        .as_array_mut()
        .unwrap()
        .last_mut()
        .unwrap() = toml::Value::String("new-*".into());
    globbed.insert(
        "Cargo.toml".into(),
        toml::to_string(&root_manifest).unwrap(),
    );
    assert!(dependencies::violations(1, &globbed).contains(path));
    globbed.insert(
        path.into(),
        "[package]\nname = 'new-member'\nversion = '1.0.0'\n".into(),
    );
    assert!(dependencies::violations(1, &globbed).is_empty());
    changed.insert(path.into(), "invalid TOML = [".into());
    assert!(dependencies::violations(1, &changed).contains(path));
}
