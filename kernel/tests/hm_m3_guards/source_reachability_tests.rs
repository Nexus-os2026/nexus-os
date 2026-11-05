//! Fix round 1 restores the intended whole-file oracle.
//! Old: "any test-only declaration excludes the resolved file globally".
//! New: "every incoming declaration must provably require test; uncertainty scans".

// R-4 amendment. Old: "new reachability alone may exclude whole files".
// New: "exclude only if fe5328af excluded AND new proof establishes test-only;
// every independent Cargo production root stays production".
use super::{fe5328af, production_files_in};
use std::collections::BTreeSet;
use std::path::Path;

fn assert_monotonic(root: &Path) -> BTreeSet<String> {
    let legacy: BTreeSet<_> = fe5328af::production_files_in(root)
        .into_iter()
        .map(|file| file.rel)
        .collect();
    let current: BTreeSet<_> = production_files_in(root)
        .into_iter()
        .map(|file| file.rel)
        .collect();
    assert!(
        legacy.is_subset(&current),
        "lost fe5328af files: {:?}",
        legacy.difference(&current).collect::<Vec<_>>()
    );
    current
}

fn inventory(files: &[(&str, &str)]) -> BTreeSet<String> {
    let root = tempfile::tempdir().unwrap();
    for (rel, text) in files {
        let path = root.path().join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }
    assert_monotonic(root.path())
}

#[test]
fn whole_file_new_test_proof_cannot_exclude_legacy_scanned_files_in_either_order() {
    for attrs in [
        "#[path = \"fixture.rs\"] #[cfg(all(unix, test))]",
        "#[cfg(all(unix, test))] #[path = \"fixture.rs\"]",
    ] {
        let files = inventory(&[
            ("lib.rs", &format!("{attrs} mod config;")),
            ("fixture.rs", "fn test_only() {}"),
            ("config.rs", "fn production() {}"),
        ]);
        // fe5328af did not recognize all(unix, test): R-4 retains it.
        assert!(files.contains("fixture.rs"), "{attrs}: {files:?}");
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
    // The historical scanner kept this child (its same-line path lookup
    // reused the earlier attribute); R-4 preserves that coverage too.
    assert!(files.contains("fixture/child.rs"));
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
        // fe5328af did not resolve this inline target: R-4 retains it.
        assert!(files.contains(target), "{source}: {files:?}");
        assert!(files.contains("fixture.rs"));
    }
    let files = inventory(&[
        ("ordinary.rs", "#[path = \"thread_files\"] mod thread { #[cfg(test)] #[path = \"tls.rs\"] mod local; }"),
        ("thread_files/tls.rs", ""),
        ("ordinary/thread/tls.rs", ""),
    ]);
    assert!(files.contains("thread_files/tls.rs"));
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
fn whole_file_crate_roots_remain_production_when_also_loaded_by_tests() {
    let files = inventory(&[
        ("lib.rs", "fn production() {}"),
        ("main.rs", "fn main() {}"),
        ("test_loader.rs", "#[cfg(test)] #[path = \"lib.rs\"] mod library; #[cfg(test)] #[path = \"main.rs\"] mod binary;"),
    ]);
    assert!(files.contains("lib.rs"));
    assert!(files.contains("main.rs"));
}

#[test]
fn whole_file_path_loaded_children_keep_ambiguous_directory_contexts() {
    // rustc resolves default children of a #[path]-loaded renamed.rs from
    // its containing directory. The scanner keeps both possible contexts
    // when the physical file can also be loaded by its default name.
    let files = inventory(&[
        (
            "lib.rs",
            "#[path = \"renamed.rs\"] mod alias; #[cfg(test)] #[path = \"child.rs\"] mod fixture;",
        ),
        ("renamed.rs", "pub(crate) mod child;"),
        ("child.rs", ""),
        ("renamed/child.rs", ""),
    ]);
    assert!(files.contains("child.rs"));
    assert!(files.contains("renamed/child.rs"));
}

#[test]
fn whole_file_raw_identifiers_whitespace_paths_and_uncertain_cfg_stay_scanned() {
    let files = inventory(&[
        ("lib.rs", "# [ path = r#\"shared.rs\"# ] pub ( crate ) mod r#live; #[cfg(test)] #[path = \"shared.rs\"] mod fixture; #[cfg(any(test, unix))] mod uncertain;"),
        ("shared.rs", ""),
        ("uncertain.rs", ""),
    ]);
    assert!(files.contains("shared.rs"));
    assert!(files.contains("uncertain.rs"));
}

#[cfg(unix)]
#[test]
fn whole_file_symlink_path_alias_keeps_its_production_child_scanned() {
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir(root.path().join("real")).unwrap();
    std::fs::write(
        root.path().join("lib.rs"),
        "#[path = \"alias.rs\"] mod live; #[cfg(test)] #[path = \"child.rs\"] mod fixture;",
    )
    .unwrap();
    std::fs::write(root.path().join("real/renamed.rs"), "mod child;").unwrap();
    std::fs::write(root.path().join("child.rs"), "").unwrap();
    std::os::unix::fs::symlink("real/renamed.rs", root.path().join("alias.rs")).unwrap();
    assert!(assert_monotonic(root.path()).contains("child.rs"));
}

#[test]
fn real_tree_production_set_is_a_superset_of_fe5328af() {
    // Keep the exact 242-path inventory, independently of the live old scanner.
    let fixture: BTreeSet<_> = include_str!("source_production_fe5328af.txt")
        .lines()
        .map(str::to_string)
        .collect();
    assert_eq!(fixture.len(), 242);
    let root = super::workspace_root();
    let legacy: BTreeSet<_> = fe5328af::production_files_in(&root)
        .into_iter()
        .map(|file| file.rel)
        .collect();
    assert_eq!(legacy, fixture, "historical oracle drifted");
    let current = assert_monotonic(&root);
    assert!(fixture.is_subset(&current));
    assert!(current.contains("mother/src/contract.rs"));
    println!(
        "production inventory: legacy={} current={}",
        fixture.len(),
        current.len()
    );
}

#[test]
fn cargo_roots_and_their_children_survive_test_aliases_in_both_attribute_orders() {
    let manifest = "[package]\nname = \"fixture\"\nversion = \"0.0.0\"\n[lib]\npath = \"library.rs\"\n[[bin]]\nname = \"explicit\"\npath = \"app.rs\"\n";
    for cfg in ["all(unix, test)", "test"] {
        for path_first in [true, false] {
            let mut loader = String::new();
            for (i, path) in [
                "library.rs",
                "app.rs",
                "src/bin/helper.rs",
                "src/bin/nested/main.rs",
            ]
            .iter()
            .enumerate()
            {
                let attrs = if path_first {
                    format!("#[path = {path:?}] #[cfg({cfg})]")
                } else {
                    format!("#[cfg({cfg})] #[path = {path:?}]")
                };
                loader.push_str(&format!("{attrs} mod alias{i};\n"));
            }
            // Exercise the same auto-bin alias alongside explicit targets.
            let files = inventory(&[
                ("Cargo.toml", manifest),
                ("loader.rs", &loader),
                ("src/lib.rs", &if path_first {
                    format!("#[path = \"bin/helper.rs\"] #[cfg({cfg})] mod test_cli;")
                } else {
                    format!("#[cfg({cfg})] #[path = \"bin/helper.rs\"] mod test_cli;")
                }),
                ("library.rs", "mod child;"),
                ("app.rs", "mod child; fn main() {}"),
                ("child.rs", "pub fn production() {}"),
                ("src/bin/helper.rs", "mod child; fn main() {}"),
                ("src/bin/child.rs", "pub fn production() {}"),
                ("src/bin/nested/main.rs", "mod child; fn main() {}"),
                ("src/bin/nested/child.rs", "pub fn production() {}"),
                ("test_loader.rs", "#[cfg(test)] #[path = \"child.rs\"] mod shared; #[cfg(test)] #[path = \"src/bin/child.rs\"] mod bin_child; #[cfg(test)] #[path = \"src/bin/nested/child.rs\"] mod nested_child;"),
            ]);
            for path in [
                "library.rs",
                "app.rs",
                "child.rs",
                "src/bin/helper.rs",
                "src/bin/child.rs",
                "src/bin/nested/main.rs",
                "src/bin/nested/child.rs",
            ] {
                assert!(
                    files.contains(path),
                    "{cfg}, path_first={path_first}: lost {path}"
                );
            }
        }
    }
}

#[test]
fn custom_build_root_remains_production_with_its_children() {
    let files = inventory(&[
        ("Cargo.toml", "[package]\nname = \"fixture\"\nversion = \"0.0.0\"\nbuild = \"compile.rs\"\n"),
        ("src/lib.rs", "#[cfg(test)] #[path = \"../compile.rs\"] mod fixture; #[cfg(test)] #[path = \"../child.rs\"] mod fixture_child;"),
        ("compile.rs", "mod child; fn main() {}"),
        ("child.rs", "pub fn production() {}"),
        ("build.rs", "fn main() {}"),
    ]);
    assert!(files.contains("compile.rs"));
    assert!(files.contains("child.rs"));
    // fe5328af never covered the conventional build.rs path.
    assert!(!files.contains("build.rs"));
}

#[cfg(unix)]
#[test]
fn cargo_root_symlinks_preserve_both_spelling_and_load_directory() {
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(root.path().join("src/bin")).unwrap();
    std::fs::create_dir(root.path().join("real")).unwrap();
    std::fs::write(
        root.path().join("Cargo.toml"),
        "[package]\nname = \"fixture\"\nversion = \"0.0.0\"\n",
    )
    .unwrap();
    std::fs::write(root.path().join("src/lib.rs"), "#[cfg(test)] #[path = \"../real/program.rs\"] mod fixture; #[cfg(test)] #[path = \"bin/child.rs\"] mod fixture_child;").unwrap();
    std::fs::write(
        root.path().join("real/program.rs"),
        "mod child; fn main() {}",
    )
    .unwrap();
    std::fs::write(
        root.path().join("src/bin/child.rs"),
        "pub fn production() {}",
    )
    .unwrap();
    std::os::unix::fs::symlink(
        "../../real/program.rs",
        root.path().join("src/bin/helper.rs"),
    )
    .unwrap();
    let files = assert_monotonic(root.path());
    for path in ["real/program.rs", "src/bin/helper.rs", "src/bin/child.rs"] {
        assert!(files.contains(path), "lost Cargo root or child: {path}");
    }
}

#[test]
fn reviewer_standard_cargo_auto_bin_remains_scanned_with_protected_content() {
    for attrs in [
        "#[path = \"bin/helper.rs\"] #[cfg(all(unix, test))]",
        "#[cfg(all(unix, test))] #[path = \"bin/helper.rs\"]",
    ] {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("src/bin")).unwrap();
        std::fs::write(
            root.path().join("Cargo.toml"),
            "[package]\nname = \"fixture\"\nversion = \"0.0.0\"\n",
        )
        .unwrap();
        std::fs::write(
            root.path().join("src/lib.rs"),
            format!("{attrs} mod test_cli;"),
        )
        .unwrap();
        std::fs::write(
            root.path().join("src/bin/helper.rs"),
            "fn main() {} pub fn name_is_cloud() -> bool { false } pub const P: &str = \"cloud\";",
        )
        .unwrap();
        assert!(assert_monotonic(root.path()).contains("src/bin/helper.rs"));
        let files = production_files_in(root.path());
        let binary = files.iter().find(|f| f.rel == "src/bin/helper.rs").unwrap();
        assert!(binary.code.contains("name_is_cloud"));
        assert!(binary.text.contains("\"cloud\""));
    }
}

// Fix round 3. Old: "Rust ancestors discover roots; raw directories expand".
// New: "workspace manifests discover every production root; normalized,
// bounded contexts fail closed". R-4's old AND new intersection is unchanged.
#[test]
fn reviewer_external_cargo_root_is_production_without_member_rust_files() {
    for member in ["app", "apps/*", "apps/a?p"] {
        let dir = if member == "app" { "app" } else { "apps/app" };
        let path = if member == "app" {
            "../shared.rs"
        } else {
            "../../shared.rs"
        };
        let files = inventory(&[
            ("Cargo.toml", &format!("[workspace]\nmembers = [{member:?}]\n")),
            (&format!("{dir}/Cargo.toml"), &format!("[package]\nname = \"app\"\nversion = \"0.0.0\"\n[lib]\npath = {path:?}\n")),
            ("test_loader.rs", "#[cfg(test)] #[path=\"shared.rs\"] mod fixture; #[cfg(test)] #[path=\"fixture.rs\"] mod only_test;"),
            ("shared.rs", "pub fn name_is_cloud() -> bool { false } pub const P: &str = \"cloud\";"),
            ("fixture.rs", "fn only_test() {}"),
        ]);
        assert!(files.contains("shared.rs"), "{member}: {files:?}");
        assert!(
            !files.contains("fixture.rs"),
            "ordinary test-only files still exclude"
        );
    }
}

#[test]
fn external_explicit_bin_and_build_roots_are_production() {
    let files = inventory(&[
        ("Cargo.toml", "[workspace]\nmembers = [\"app\"]\n"),
        ("app/Cargo.toml", "[package]\nname = \"app\"\nversion = \"0.0.0\"\nbuild = \"../compile.rs\"\nautobins = false\n[[bin]]\nname = \"app\"\npath = \"../program.rs\"\n"),
        ("test_loader.rs", "#[cfg(test)] #[path=\"program.rs\"] mod binary; #[cfg(test)] #[path=\"compile.rs\"] mod build; #[cfg(test)] #[path=\"app/src/bin/disabled.rs\"] mod disabled;"),
        ("program.rs", "fn main() {}"),
        ("compile.rs", "fn main() {}"),
        ("app/src/bin/disabled.rs", "fn main() {}"),
    ]);
    assert!(files.contains("program.rs"));
    assert!(files.contains("compile.rs"));
    assert!(!files.contains("app/src/bin/disabled.rs"));
}

#[test]
fn unresolved_workspace_members_fail_loudly() {
    for member in ["missing", "missing/*", "apps/**", "apps/[ab]"] {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(
            root.path().join("Cargo.toml"),
            format!("[workspace]\nmembers = [{member:?}]\n"),
        )
        .unwrap();
        std::fs::write(root.path().join("ordinary.rs"), "").unwrap();
        assert!(
            std::panic::catch_unwind(|| production_files_in(root.path())).is_err(),
            "unresolved {member} must fail loudly"
        );
    }
}

#[test]
fn reviewer_disabled_module_cycle_terminates_and_keeps_production() {
    let start = std::time::Instant::now();
    let files = inventory(&[
        (
            "lib.rs",
            "#[cfg(any())] mod outer { #[path=\"../lib.rs\"] mod child; }",
        ),
        (
            "test_loader.rs",
            "#[cfg(test)] #[path=\"lib.rs\"] mod fixture;",
        ),
    ]);
    assert!(files.contains("lib.rs"));
    assert!(start.elapsed() < std::time::Duration::from_secs(2));
    // A cycle involving ordinary, test-aliased files also stays production.
    let files = inventory(&[
        ("loader.rs", "#[cfg(test)] #[path=\"a.rs\"] mod fixture;"),
        ("a.rs", "#[cfg(test)] #[path=\"b.rs\"] mod next;"),
        ("b.rs", "#[cfg(test)] #[path=\"a.rs\"] mod next;"),
    ]);
    assert!(files.contains("a.rs"));
    assert!(files.contains("b.rs"));
}
