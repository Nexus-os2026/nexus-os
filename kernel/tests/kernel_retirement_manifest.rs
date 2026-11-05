//! The approved K02–K22 inventory, checked before any production extraction.
#[allow(dead_code)]
mod retirement;
use retirement::*;
use std::collections::BTreeSet;

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
        let lib = compact(&read("kernel/src/lib.rs"));
        for declaration in &row.declarations {
            assert_eq!(
                lib.contains(&compact(declaration)),
                row.stage > LIVE_STAGE,
                "{declaration}"
            );
        }
        for export in &row.reexports {
            if row.stage > LIVE_STAGE {
                assert!(compact(&read("kernel/src/cognitive/mod.rs")).contains(&compact(export)));
            }
        }
        for item in &row.items {
            assert_eq!(
                read(&item.path).contains(&item.anchor),
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
        for extraction in &row.extract {
            assert!(extraction.destination.starts_with("kernel/src/"));
            assert!(!extraction.destination.contains(".."));
            assert_ne!(extraction.source, extraction.destination);
            assert!(!extraction.payload.is_empty());
            assert!(!extraction.anchors.is_empty());
            let path = if row.stage <= LIVE_STAGE {
                &extraction.destination
            } else {
                &extraction.source
            };
            let text = read(path);
            if row.stage <= LIVE_STAGE && root().join(&extraction.source).is_file() {
                let old = compact(&read(&extraction.source));
                for export in &extraction.transitional_exports {
                    assert!(
                        old.contains(&compact(export)),
                        "missing same-type export: {export}"
                    );
                }
            }
            for anchor in &extraction.anchors {
                assert!(text.contains(anchor), "{path}: missing {anchor}");
            }
            for temporary in &extraction.temporary {
                temporary_helpers += 1;
                assert_eq!(temporary.anchor, "pub fn required_capabilities");
                assert_eq!(
                    temporary.until, 12,
                    "last helper consumer retires with actuators"
                );
                assert_eq!(
                    text.contains(&temporary.anchor),
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
    let manifests = cargo_manifests();
    assert_eq!(manifests.len(), 10);
    for (path, expected) in manifests {
        assert_eq!(
            digest(read(&path).as_bytes()),
            expected,
            "dependency policy changed: {path}"
        );
    }
}
