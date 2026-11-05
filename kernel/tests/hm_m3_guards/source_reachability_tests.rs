//! Fix round 1 restores the intended whole-file oracle.
//! Old: "any test-only declaration excludes the resolved file globally".
//! New: "every incoming declaration must provably require test; uncertainty scans".

use super::production_files_in;
use std::collections::BTreeSet;

fn inventory(files: &[(&str, &str)]) -> BTreeSet<String> {
    let root = tempfile::tempdir().unwrap();
    for (rel, text) in files {
        let path = root.path().join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }
    production_files_in(root.path())
        .into_iter()
        .map(|file| file.rel)
        .collect()
}

#[test]
fn whole_file_attributes_are_complete_in_both_orders() {
    for attrs in [
        "#[path = \"fixture.rs\"] #[cfg(all(unix, test))]",
        "#[cfg(all(unix, test))] #[path = \"fixture.rs\"]",
    ] {
        let files = inventory(&[
            ("lib.rs", &format!("{attrs} mod config;")),
            ("fixture.rs", "fn test_only() {}"),
            ("config.rs", "fn production() {}"),
        ]);
        assert!(!files.contains("fixture.rs"), "{attrs}: {files:?}");
        assert!(files.contains("config.rs"), "{attrs}: {files:?}");
    }
}

#[test]
fn whole_file_shared_production_and_test_declarations_stay_scanned() {
    for test in [
        "#[path = \"config.rs\"] #[cfg(all(unix, test))] mod fixture;",
        "#[cfg(all(unix, test))] #[path = \"config.rs\"] mod fixture;",
    ] {
        for declarations in [
            format!("#[path = \"config.rs\"] mod live; {test}"),
            format!("{test} #[path = \"config.rs\"] mod live;"),
        ] {
            assert!(
                inventory(&[("lib.rs", &declarations), ("config.rs", "")]).contains("config.rs")
            );
        }
    }
    // Path-based test classification cannot overrule a production declaration.
    assert!(inventory(&[
        ("lib.rs", "#[path = \"shared_tests.rs\"] mod live;"),
        ("shared_tests.rs", ""),
    ])
    .contains("shared_tests.rs"));
}

#[test]
fn whole_file_test_only_reachability_propagates_without_hiding_shared_children() {
    let files = inventory(&[
        ("lib.rs", "#[cfg(test)] mod fixture; mod live;"),
        (
            "fixture.rs",
            "#[path = \"shared.rs\"] mod shared; mod child;",
        ),
        ("fixture/child.rs", ""),
        ("live.rs", "#[path = \"shared.rs\"] mod shared;"),
        ("shared.rs", ""),
    ]);
    assert!(!files.contains("fixture.rs"));
    assert!(!files.contains("fixture/child.rs"));
    assert!(files.contains("shared.rs"));
}

#[test]
fn whole_file_default_resolution_and_ambiguous_targets_fail_closed() {
    let files = inventory(&[
        (
            "lib.rs",
            "#[cfg(test)] mod flat; #[cfg(test)] mod nested; #[cfg(test)] mod ambiguous;",
        ),
        ("flat.rs", ""),
        ("nested/mod.rs", ""),
        ("ambiguous.rs", ""),
        ("ambiguous/mod.rs", ""),
    ]);
    assert!(!files.contains("flat.rs"));
    assert!(!files.contains("nested/mod.rs"));
    assert!(files.contains("ambiguous.rs"));
    assert!(files.contains("ambiguous/mod.rs"));
}

#[test]
fn whole_file_inline_modules_use_rust_relative_directory_rules() {
    // The same explicit path is relative to lib.rs's directory, but includes
    // the source stem inside a non-mod-rs file's inline module.
    for (source, target) in [
        ("lib.rs", "outer/inner/fixture.rs"),
        ("ordinary.rs", "ordinary/outer/inner/fixture.rs"),
        ("nested/mod.rs", "nested/outer/inner/fixture.rs"),
    ] {
        let files = inventory(&[
            (
                source,
                "mod outer { mod inner { #[path = \"fixture.rs\"] #[cfg(test)] mod child; } }",
            ),
            (target, ""),
            ("fixture.rs", ""),
        ]);
        assert!(!files.contains(target), "{source}: {files:?}");
        assert!(files.contains("fixture.rs"));
    }
    let files = inventory(&[
        ("ordinary.rs", "#[path = \"thread_files\"] mod thread { #[cfg(test)] #[path = \"tls.rs\"] mod local; }"),
        ("thread_files/tls.rs", ""),
        ("ordinary/thread/tls.rs", ""),
    ]);
    assert!(!files.contains("thread_files/tls.rs"));
    assert!(files.contains("ordinary/thread/tls.rs"));
    let files = inventory(&[
        ("lib.rs", "#[cfg(test)] mod outer { mod child; } mod live { #[path = \"../outer/child.rs\"] mod shared; }"),
        ("outer/child.rs", ""),
    ]);
    assert!(files.contains("outer/child.rs"));
}

#[test]
fn whole_file_cfg_attr_path_changes_are_uncertain_even_with_test_cfg() {
    for attr in [
        "#[cfg_attr(unix, path = \"other.rs\")]",
        "#[cfg_attr(test, path = \"other.rs\")]",
        "#[cfg_attr(unix, cfg_attr(windows, path = \"other.rs\"))]",
    ] {
        let files = inventory(&[
            ("lib.rs", &format!("#[cfg(test)] {attr} mod default; #[cfg(test)] #[path = \"other.rs\"] mod fixture;")),
            ("default.rs", ""),
            ("other.rs", ""),
        ]);
        assert!(files.contains("default.rs"), "{attr}: {files:?}");
        assert!(files.contains("other.rs"), "{attr}: {files:?}");
    }
}

#[test]
fn whole_file_unresolved_paths_cannot_hide_another_declarations_target() {
    let files = inventory(&[
        ("lib.rs", "#[path = concat!(\"shared\", \".rs\")] mod live; #[cfg(test)] #[path = \"shared.rs\"] mod fixture;"),
        ("shared.rs", ""),
    ]);
    assert!(files.contains("shared.rs"));
}

#[test]
fn real_tree_retains_legacy_production_inventory_except_proved_test_files() {
    // Captured by running the scanner from fe5328afdb04732b9aa9bf9eb854b01823f8d352
    // over this real tree. Check identities as well as the reviewer's count.
    let legacy: BTreeSet<_> = include_str!("source_production_fe5328af.txt")
        .lines()
        .collect();
    assert_eq!(legacy.len(), 242);
    let current: BTreeSet<_> = super::production_files()
        .into_iter()
        .map(|f| f.rel)
        .collect();
    let missing: BTreeSet<_> = legacy
        .iter()
        .filter(|p| !current.contains(**p))
        .copied()
        .collect();
    let expected: BTreeSet<_> = (3..=6)
        .map(|n| format!("mother/src/adapter/lite_review{n}.rs"))
        .collect();
    // These four complete attribute lists are now resolved correctly. Each
    // consists of path, target_os=linux, cfg(test); no production declaration
    // reaches any of the four. Synthetic shared-reachability controls above
    // must retain the file if a production declaration is added.
    for n in 3..=6 {
        let declaration = format!("#[path = \"../adapter/lite_review{n}.rs\"]\n#[cfg(target_os = \"linux\")]\n#[cfg(test)]\nmod review{n};");
        assert!(include_str!("../../../mother/src/coordinator/lite.rs").contains(&declaration));
    }
    assert!(
        missing.iter().all(|p| expected.contains(*p)),
        "unexpected lost production files: {missing:?}"
    );
    assert!(current.len() >= 242 - missing.len());
    println!(
        "production inventory: legacy=242 current={} extra exclusions={missing:?}",
        current.len()
    );
}
