//! Mutation receipts name their exact protected subject and use fresh reachability.
use super::*;

#[test]
fn scanner_inventory_loads_nested_docs_with_an_already_registered_module_name() {
    let stage = 22;
    let mut tree = clean_tree(stage);
    tree.insert(
        "kernel/src/privacy.rs".into(),
        read("kernel/src/privacy.rs"),
    );
    tree.get_mut("kernel/src/lib.rs")
        .unwrap()
        .push_str("pub mod privacy;");
    tree.prove_fixture();
    let fixture = tree.write_fixture();
    let baseline = load_tree_in(fixture.path());
    let before = successor(stage, &baseline);
    assert!(before.is_empty(), "{before:?}");
    assert!(module_edges(stage)["kernel/src/privacy.rs"].contains("tests"));
    let parent = fixture.path().join("kernel/src/privacy.rs");
    let raw = std::fs::read_to_string(&parent).unwrap();
    assert!(raw.contains("#[cfg(test)]\nmod tests"));
    std::fs::write(
        parent,
        format!("{raw}\n#[cfg(not(test))] #[path=\"docs/runner.rs\"] mod tests;\n"),
    )
    .unwrap();
    let subject = "kernel/src/docs/runner.rs";
    let target = fixture.path().join(subject);
    std::fs::create_dir_all(target.parent().unwrap()).unwrap();
    std::fs::write(&target, "fn run(){ std::process::Command::new(\"sh\"); }").unwrap();
    let production: BTreeSet<_> = source::production_files_in(fixture.path())
        .into_iter()
        .map(|f| f.rel)
        .collect();
    assert!(
        production.contains(subject),
        "plant must be production reachable"
    );
    let loaded = load_tree_in(fixture.path());
    assert!(loaded.contains_key(subject));
    assert!(!loaded.test_only.contains(subject));
    assert!(production.iter().all(|p| loaded.contains_key(p)));
    let added: Findings = successor(stage, &loaded)
        .difference(&before)
        .cloned()
        .collect();
    for oracle in ["f7-dependency", "raw-process"] {
        assert!(added.contains(&(oracle, subject.into())), "{added:?}");
    }
    // The already registered module name alone does not flag the parent.
    assert!(!added.contains(&("f7-dependency", "kernel/src/privacy.rs".into())));
}

#[test]
fn harmless_mutations_remain_green_after_recomputing_reachability() {
    for stage in 2..=22 {
        let baseline = clean_tree(stage);
        let before = successor(stage, &baseline);
        assert!(before.is_empty());
        for path in [
            "kernel/src/privacy.rs",
            "kernel/src/audit/mod.rs",
            "kernel/src/workspace_authority/tests.rs",
        ] {
            let mut changed = baseline.clone();
            changed
                .get_mut(path)
                .unwrap()
                .push_str("\n// harmless mutation\n");
            changed.prove_fixture();
            assert_eq!(successor(stage, &changed), before, "K{stage}: {path}");
            assert_eq!(changed.test_only, baseline.test_only, "K{stage}: {path}");
        }
    }
}

#[test]
fn f3_search_plants_and_coverage_omissions_stay_red_through_k11() {
    let path = "kernel/src/actuators/web.rs";
    for stage in [2, 3, 10, 11, 12] {
        for plant in [
            "curl_get(_query)",
            "Command::new(\"curl\").arg(_query).output()",
            "Command::new(\"sh\").arg(_query).output()",
            "get(\"https://html.duckduckgo.com\")",
            "Err(\"use ShellCommand with curl\".into())",
            "std::env::var(\"SEARXNG_URL\")",
        ] {
            let mut tree = clean_tree(stage);
            let original = WEB_SOURCE_FIXTURE;
            let mutant = original.replacen("Err(WEB_SEARCH_CLOSED.to_string())", plant, 1);
            assert_ne!(original, mutant);
            tree.insert(path.into(), mutant);
            rejected(
                stage,
                &tree,
                if stage < 12 {
                    "f3-search"
                } else {
                    "f7-absence"
                },
                path,
            );
        }
        if stage >= 12 {
            continue;
        }
        let mut tree = clean_tree(stage);
        tree.remove(WEB_TEST_PATH).unwrap();
        rejected(stage, &tree, "f3-coverage", WEB_TEST_PATH);
        for name in [
            "f3_source_prohibition_survives_without_the_cognitive_executor",
            "f3_direct_default_search_refuses_without_a_connection",
            "f3_injected_hermetic_search_still_works_directly",
            "f3_injected_backend_failure_preserves_the_original_error",
        ] {
            let mut tree = clean_tree(stage);
            let raw = tree.get_mut(WEB_TEST_PATH).unwrap();
            let declaration = coverage::declarations(raw)
                .into_iter()
                .find(|d| d.name == name)
                .unwrap();
            raw.replace_range(declaration.span.0..declaration.span.1, "/* omitted */");
            rejected(stage, &tree, "f3-coverage", WEB_TEST_PATH);
        }
        // Removing either assertion alone must fail, even with the failing
        // backend fixture, expect_err, and test declaration still present.
        for assertion in [
            r#"assert!(error.contains("backend down"), "{error}");"#,
            r#"assert!(!error.contains("web search is closed"), "{error}");"#,
        ] {
            let mut tree = clean_tree(stage);
            let raw = tree.get_mut(WEB_TEST_PATH).unwrap();
            assert_eq!(raw.matches(assertion).count(), 1);
            *raw = raw.replace(assertion, "/* omitted assertion */");
            rejected(stage, &tree, "f3-coverage", WEB_TEST_PATH);
        }
    }
}

#[test]
fn current_reachability_promotion_never_hides_registered_test_source() {
    for stage in [2, 11, 13, 22] {
        let tree = clean_tree(stage);
        let fixture = tree.write_fixture();
        let target = "kernel/src/workspace_authority/tests.rs";
        let loaded = load_tree_in(fixture.path());
        assert!(loaded.test_only.contains(target));
        let baseline = successor(stage, &loaded);
        assert!(baseline.is_empty());
        let parent = fixture.path().join("kernel/src/workspace_authority.rs");
        let raw = std::fs::read_to_string(&parent).unwrap();
        let promoted = raw.replace("#[cfg(test)]\nmod tests;", "mod tests;");
        assert_ne!(raw, promoted);
        std::fs::write(parent, promoted).unwrap();
        std::fs::OpenOptions::new()
            .append(true)
            .open(fixture.path().join(target))
            .and_then(|mut f| {
                std::io::Write::write_all(
                    &mut f,
                    br#"
fn promoted() {
    std::process::Command::new("sh"); reqwest::Client::new();
    outside::Orchestrator::new();
}
"#,
                )
            })
            .unwrap();
        let promoted = load_tree_in(fixture.path());
        assert!(!promoted.test_only.contains(target));
        let added: Findings = successor(stage, &promoted)
            .difference(&baseline)
            .cloned()
            .collect();
        for (oracle, subject) in [
            ("f7-dependency", target.to_owned()),
            ("raw-process", target.to_owned()),
            ("raw-network", target.to_owned()),
            ("acceptance-inventory", format!("{target}::load")),
        ] {
            assert!(
                added.contains(&(oracle, subject)),
                "K{stage:02} {oracle}: {added:?}"
            );
        }
    }
}

#[test]
fn original_f7_plants_remain_red_at_every_applicable_transition() {
    let plants = [
        "    if crate::free_api_registry::is_known_free_api(url) {",
        "use crate::orchestration::orchestrator::Orchestrator;",
        "let e = crate::delegation::DelegationEngine::new();",
        "use crate::speculative::SpeculativeEngine;",
        "use crate::time_machine::{Checkpoint, TimeMachine};",
        "use crate::policy_engine::PolicyEngine;",
    ];
    for stage in 2..=22 {
        for (until, path) in [
            (11, "kernel/src/cognitive/planner.rs"),
            (12, "kernel/src/actuators/shell.rs"),
        ] {
            for plant in plants {
                let mut tree = clean_tree(stage);
                tree.insert(path.into(), plant.into());
                rejected(
                    stage,
                    &tree,
                    if stage < until {
                        "f7-dependency"
                    } else {
                        "f7-absence"
                    },
                    path,
                );
            }
        }
        let mut tree = clean_tree(stage);
        tree.insert(
            "kernel/src/actuators/computer_use.rs".into(),
            plants[3].into(),
        );
        rejected(
            stage,
            &tree,
            if stage < 12 {
                "f7-dependency"
            } else {
                "f7-absence"
            },
            "kernel/src/actuators/computer_use.rs",
        );
    }
    // The predecessor's one checkpoint exception still applies while that
    // actuator exists. Re-prove the changed fixture before a green assertion.
    let mut tree = clean_tree(11);
    tree.insert(
        "kernel/src/actuators/computer_use.rs".into(),
        plants[4].into(),
    );
    tree.prove_fixture();
    assert_eq!(successor(11, &tree), Findings::new());
}

#[test]
fn historical_f7_absence_includes_empty_roots_files_and_restored_children() {
    let fixture = tempfile::tempdir().unwrap();
    for stem in F7_ABSENT {
        std::fs::create_dir_all(fixture.path().join(stem)).unwrap();
        std::fs::write(fixture.path().join(format!("{stem}.rs")), "").unwrap();
        std::fs::write(fixture.path().join(stem).join("note.txt"), "plant").unwrap();
    }
    let loaded = load_tree_in(fixture.path());
    for stage in 2..=22 {
        let mut tree = clean_tree(stage);
        for (path, raw) in loaded.iter() {
            tree.insert(path.clone(), raw.clone());
        }
        tree.prove_fixture();
        let baseline = successor(stage, &clean_tree(stage));
        let findings: Findings = successor(stage, &tree)
            .difference(&baseline)
            .cloned()
            .collect();
        assert!(baseline.is_empty());
        for stem in F7_ABSENT {
            assert!(loaded.contains_key(stem));
            assert!(loaded.contains_key(&format!("{stem}.rs")));
            assert!(loaded.contains_key(&format!("{stem}/note.txt")));
            assert!(
                findings.contains(&("f7-absence", stem.into())),
                "K{stage}: {stem}"
            );
        }
    }
}

#[test]
fn f7_guard_presence_and_contacts_survive_until_their_subject_retires() {
    for stage in 2..=22 {
        for (until, module) in [
            (19, "policy_engine"),
            (17, "time_machine"),
            (15, "speculative"),
            (19, "consent"),
            (19, "autonomy"),
            (14, "supervisor"),
        ] {
            if stage >= until {
                continue;
            }
            let mut tree = clean_tree(stage);
            let lib = tree.get_mut("kernel/src/lib.rs").unwrap();
            *lib = lib.replace(&format!("pub mod {module};"), "");
            rejected(
                stage,
                &tree,
                "f7-presence",
                &format!("kernel/src/lib.rs::{module}"),
            );
        }
        for (until, path) in [
            (19, "kernel/src/consent.rs"),
            (19, "kernel/src/autonomy.rs"),
            (14, "kernel/src/supervisor.rs"),
        ] {
            if stage >= until {
                continue;
            }
            let mut tree = clean_tree(stage);
            tree.insert(path.into(), String::new());
            rejected(stage, &tree, "f7-presence", path);
        }
    }
}

#[test]
fn no_handoff_preserves_identifiers_literals_and_sql_reads() {
    for stage in 2..=22 {
        let path = if stage < 11 {
            "kernel/src/cognitive/algorithms/world_model.rs"
        } else {
            "kernel/src/privacy.rs"
        };
        for plant in [
            r#"fn handoff(){ connection.query_row("SELECT value_json FROM agent_memory", [], read); }"#,
            r#"fn handoff(){ let route = r"task_context"; outside.read(route); }"#,
            "fn handoff(){ outside::control_records(); }",
        ] {
            let mut tree = clean_tree(stage);
            tree.insert(path.into(), plant.into());
            rejected(stage, &tree, "hm-no-handoff", path);
        }
    }
    for route in NO_HANDOFF {
        let mut tree = clean_tree(22);
        tree.insert(
            "kernel/src/privacy.rs".into(),
            format!("fn handoff(){{outside.read(\"{route}\");}}"),
        );
        rejected(22, &tree, "hm-no-handoff", "kernel/src/privacy.rs");
    }
}

#[test]
fn retained_boundary_fixtures_contain_real_process_and_complete_workspace_paths() {
    for stage in 2..=22 {
        let tree = clean_tree(stage);
        assert_eq!(
            tree["crates/nexus-verifier-sandbox/src/launcher.rs"],
            read("crates/nexus-verifier-sandbox/src/launcher.rs")
        );
        assert!(prod(&tree["crates/nexus-verifier-sandbox/src/launcher.rs"])
            .0
            .contains("Command::new"));
        assert!(
            tree["kernel/src/workspace_authority.rs"].contains("crate::workspace_permissions::")
        );
        assert!(tree["kernel/src/coding_run/apply.rs"].contains("crate::workspace_authority::"));
        assert_eq!(successor(stage, &tree), Findings::new(), "K{stage}");
        if stage >= 13 {
            for spelling in [
                "crate::workspace::Old",
                "nexus_kernel::workspace::Old",
                "nexus_kernel::{workspace,other}",
            ] {
                let mut tree = tree.clone();
                tree.insert("kernel/src/privacy.rs".into(), format!("use {spelling};"));
                rejected(stage, &tree, "f7-dependency", "kernel/src/privacy.rs");
            }
        }
    }
}

#[test]
fn registered_test_contracts_preserve_real_declarations_cfg_scopes_and_ignore_policy() {
    let policy = required_test_policy();
    assert_eq!(
        policy.keys().cloned().collect::<BTreeSet<_>>(),
        required_tests()
    );
    for (name, expected) in policy {
        let (path, function) = name.rsplit_once("::").unwrap();
        assert_eq!(
            coverage::test_contract(&read(path), function).as_ref(),
            Some(&expected),
            "{name}"
        );
        for attribute in ["#[cfg(any())]", "#[ignore]", "#[cfg_attr(all(), ignore)]"] {
            let mut tree = clean_tree(22);
            let raw = tree.get_mut(path).unwrap();
            let declaration = coverage::declarations(raw)
                .into_iter()
                .find(|d| d.kind == "fn" && d.name == function)
                .unwrap();
            raw.insert_str(declaration.span.0, attribute);
            rejected(22, &tree, "acceptance-inventory", &name);
        }
        let mut tree = clean_tree(22);
        let raw = tree.get_mut(path).unwrap();
        let declaration = coverage::declarations(raw)
            .into_iter()
            .find(|d| d.kind == "fn" && d.name == function)
            .unwrap();
        let commented = format!("/*{}*/", &raw[declaration.span.0..declaration.span.1]);
        raw.replace_range(declaration.span.0..declaration.span.1, &commented);
        rejected(22, &tree, "acceptance-inventory", &name);
    }
    // Ancestor attributes and missing external loading edges matter too.
    for edge in test_edges() {
        for replacement in ["// disabled", "#[cfg(any())]", "#[cfg(not(test))]"] {
            let mut tree = clean_tree(22);
            let raw = tree.get_mut(&edge.source).unwrap();
            let declaration = coverage::declarations(raw)
                .into_iter()
                .find(|d| d.kind == "mod" && d.name == edge.module)
                .unwrap();
            if replacement == "// disabled" {
                raw.replace_range(
                    declaration.span.0..declaration.span.1,
                    "/* removed module */",
                );
            } else {
                raw.insert_str(declaration.span.0, replacement);
            }
            rejected(
                22,
                &tree,
                "acceptance-inventory",
                &format!("{}::load", edge.target),
            );
        }
    }
    for prefix in ["#![cfg(any())]\n", "#[cfg(any())] mod disabled {\n"] {
        let mut tree = clean_tree(22);
        let path = "kernel/tests/m0_f5_emergency_stop.rs";
        let raw = tree.get_mut(path).unwrap();
        *raw = format!(
            "{prefix}{raw}{}",
            if prefix.starts_with("#[") { "\n}" } else { "" }
        );
        rejected(
            22,
            &tree,
            "acceptance-inventory",
            &format!("{path}::m0_f5_stop_ends_an_inflight_local_model_call"),
        );
    }
}

#[test]
fn actual_retained_logging_sink_and_unchanged_expression_rebinding_are_red() {
    let path = "mother/desktop/src/app.rs";
    let raw = read(path);
    let sink = "eprintln!(\"nexus-mother: {e}\")";
    assert!(logs()
        .iter()
        .any(|l| l.path == path && l.expression == sink));
    for mutant in [
        raw.replacen(sink, "eprintln!(\"{raw_content}\")", 1),
        raw.replacen(sink, &format!("let e = raw_content; {sink}"), 1),
    ] {
        assert_ne!(mutant, raw);
        let mut tree = clean_tree(22);
        tree.insert(path.into(), mutant);
        rejected(22, &tree, "logging", path);
    }
    let rebound = raw.replacen(sink, &format!("let e = raw_content; {sink}"), 1);
    assert_eq!(
        logging_sinks(path, &raw)
            .iter()
            .map(|s| &s.expression)
            .collect::<Vec<_>>(),
        logging_sinks(path, &rebound)
            .iter()
            .map(|s| &s.expression)
            .collect::<Vec<_>>()
    );
}

#[test]
fn module_matching_uses_complete_components_in_direct_and_grouped_paths() {
    for namespace in ["crate", "nexus_kernel"] {
        for tail in [
            "workspace::Old",
            "{workspace,other}",
            "{other,workspace::Old}",
            "{other::{Nested},workspace::{Old}}",
            "r#workspace::Old",
        ] {
            assert!(
                referenced_modules(&format!("use {namespace}::{tail};"), namespace)
                    .contains("workspace")
            );
        }
        for tail in [
            "workspace_authority::WorkspaceGrant",
            "workspace_permissions::FsPermissionLevel",
            "{workspace_authority::WorkspaceGrant,workspace_permissions::FsPermissionLevel}",
        ] {
            assert!(
                !referenced_modules(&format!("{namespace}::{tail}"), namespace)
                    .contains("workspace")
            );
        }
    }
}

#[test]
fn registered_self_spawn_scaffolds_allow_only_the_unchanged_test_source() {
    let helpers: BTreeMap<String, String> =
        serde_json::from_str(include_str!("retained-test-process-files.json")).unwrap();
    assert_eq!(helpers.len(), 4);
    for (path, hash) in helpers {
        let raw = read(&path);
        assert_eq!(digest(raw.replace("\r\n", "\n").as_bytes()), hash);
        let mut tree = clean_tree(22);
        let mutant = raw.replacen(
            "Command::new(std::env::current_exe().unwrap())",
            "Command::new(\"sh\")",
            1,
        );
        assert_ne!(raw, mutant);
        tree.insert(path.clone(), mutant);
        rejected(22, &tree, "raw-process", &path);
        let mut tree = clean_tree(22);
        tree.get_mut(&path)
            .unwrap()
            .push_str("fn planted(){ Command::new(\"sh\"); }");
        rejected(22, &tree, "raw-process", &path);
    }
}

#[test]
fn manifest_policy_is_shared_by_every_staged_successor() {
    for stage in 2..=22 {
        let mut tree = clean_tree(stage);
        let path = "mother/host/Cargo.toml";
        tree.get_mut(path)
            .unwrap()
            .push_str("\n[dev-dependencies.new_host_helper]\nversion = '1'\n");
        tree.prove_fixture();
        assert_eq!(
            successor(stage, &tree),
            Findings::new(),
            "K{stage}: legitimate host dependency"
        );
        tree.get_mut(path).unwrap().push_str("\n[target.'cfg(windows)'.build-dependencies.alias]\npackage = 'nexus-sdk'\nversion = '1'\n");
        rejected(stage, &tree, "f7-dependency", path);
        let mut tree = clean_tree(stage);
        tree.get_mut("kernel/Cargo.toml")
            .unwrap()
            .push_str("\n[build-dependencies.new_kernel_dependency]\nversion = '1'\n");
        rejected(stage, &tree, "f7-dependency", "kernel/Cargo.toml");
    }
}
