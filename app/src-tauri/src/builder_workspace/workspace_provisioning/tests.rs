//! P0-002C4D1B governed React/runtime provisioning tests. Synthetic scaffolds
//! and the real deterministic scaffold generator only; no process execution.
use super::super::tests::{registered, registered_only, Fixture};
use super::*;
use nexus_kernel::workspace_authority::WorkspaceAuthorityRegistry;
use serde_json::Value;
use std::sync::Mutex;

// ── Helpers ───────────────────────────────────────────────────────────────

fn scaffold() -> Vec<(&'static str, &'static [u8])> {
    vec![
        ("index.html", &b"<!doctype html><div id=\"root\"></div>"[..]),
        (
            "src/App.tsx",
            &b"export default function App() { return null }\n"[..],
        ),
        (
            "src/components/Hero.tsx",
            &b"export const Hero = () => null\n"[..],
        ),
        ("public/favicon.svg", &b"<svg/>"[..]),
        ("package.json", &b"{\"scripts\":{\"dev\":\"vite\"}}"[..]),
        ("tsconfig.json", &b"{}"[..]),
        ("vite.config.ts", &b"export default {}"[..]),
        ("tailwind.config.ts", &b"export default {}"[..]),
        ("postcss.config.js", &b"export default {}"[..]),
        ("README.md", &b"readme"[..]),
    ]
}

fn project(f: &Fixture, id: &str) -> Arc<RegisteredBuilderProject> {
    f.authority
        .catalog
        .lookup(Uuid::parse_str(id).unwrap())
        .unwrap()
}

fn provisioned(f: &Fixture, id: &str) -> bool {
    f.authority
        .catalog
        .lookup(Uuid::parse_str(id).unwrap())
        .is_ok_and(|project| project.workspace.get().is_some())
}

/// Every entry below `root` (no link is followed); directories end in `/`.
fn tree(root: &Path) -> BTreeSet<String> {
    fn walk(base: &Path, dir: &Path, into: &mut BTreeSet<String>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let relative = path
                .strip_prefix(base)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            let metadata = std::fs::symlink_metadata(&path).unwrap();
            if metadata.is_dir() {
                into.insert(format!("{relative}/"));
                walk(base, &path, into);
            } else {
                into.insert(relative);
            }
        }
    }
    let mut entries = BTreeSet::new();
    walk(root, root, &mut entries);
    entries
}

fn set(entries: &[&str]) -> BTreeSet<String> {
    entries.iter().map(|entry| (*entry).to_owned()).collect()
}

const RUNTIME_TREE: [&str; 5] = [
    "runtime/",
    "runtime/env/",
    "runtime/home/",
    "runtime/tmp/",
    "runtime/vite-cache/",
];

/// An audit sink that runs `hook` on each event and records it.
fn hooked(
    f: &Fixture,
    hook: impl Fn(&Value) + Send + Sync + 'static,
) -> (Audit, Arc<Mutex<Vec<Value>>>) {
    let events = Arc::clone(&f.events);
    let audit: Audit = Arc::new(move |event| {
        hook(&event);
        events.lock().unwrap().push(event);
    });
    (audit, Arc::clone(&f.events))
}

fn is_event(event: &Value, operation: &str, outcome: &str) -> bool {
    event["operation"] == operation && event["outcome"] == outcome
}

// ── Success ───────────────────────────────────────────────────────────────

#[test]
fn p0_002c4d1b_fresh_registration_provisions_governed_react_and_private_runtime() {
    let f = Fixture::new();
    let (id, root) = registered_only(&f);
    assert!(!root.join(REACT).exists() && !root.join(RUNTIME).exists());
    f.authority
        .provision_workspace(&id, &scaffold(), f.audit())
        .unwrap();
    let mut expected = set(&[
        "react/",
        "react/index.html",
        "react/public/",
        "react/public/favicon.svg",
        "react/src/",
        "react/src/App.tsx",
        "react/src/components/",
        "react/src/components/Hero.tsx",
    ]);
    expected.extend(set(&RUNTIME_TREE));
    let mut observed = tree(&root);
    observed.retain(|entry| entry.starts_with("react/") || entry.starts_with("runtime/"));
    assert_eq!(observed, expected);
    assert_eq!(
        std::fs::read(root.join("react/src/components/Hero.tsx")).unwrap(),
        b"export const Hero = () => null\n"
    );
    // Retained governed identities validate; nothing else was persisted.
    let registration = project(&f, &id);
    let workspace = registration.workspace.get().unwrap();
    workspace.validate_react(&root).unwrap();
    workspace.validate_runtime(&root).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(root.join(RUNTIME))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o077, 0, "runtime must be private: {mode:o}");
    }
    // The governed workspace is now usable by C3 and C4A (launch still denied).
    f.authority
        .write_file(&id, "src/new.ts", b"export {}", f.audit())
        .unwrap();
    assert_eq!(
        f.authority.dev_server_start(&id, f.audit()),
        "launch unavailable"
    );
}

#[test]
fn p0_002c4d1b_real_builder_scaffold_persists_only_project_content() {
    let f = Fixture::new();
    let (id, root) = registered_only(&f);
    let built = web_builder_agent::build_orchestrator::run_build_pipeline(
        "a saas landing page with pricing",
        web_builder_agent::react_gen::OutputMode::React,
        "Demo",
        &|_| {},
    )
    .unwrap()
    .react_project
    .unwrap();
    let files: Vec<(&str, &[u8])> = built
        .files
        .iter()
        .map(|file| (file.path.as_str(), file.content.as_bytes()))
        .collect();
    f.authority
        .provision_workspace(&id, &files, f.audit())
        .unwrap();
    let react = tree(&root.join(REACT));
    for required in [
        "index.html",
        "src/main.tsx",
        "src/App.tsx",
        "public/favicon.svg",
    ] {
        assert!(react.contains(required), "{required}: {react:?}");
    }
    for entry in &react {
        assert!(
            entry == "index.html" || entry.starts_with("src/") || entry.starts_with("public/"),
            "unexpected persisted entry {entry}"
        );
    }
    for dropped in DROPPED_GENERATOR_METADATA {
        assert!(!react.contains(dropped), "{dropped} persisted");
    }
    assert!(built.files.iter().any(|file| file.path == "package.json"));
}

// ── Registration failures ─────────────────────────────────────────────────

#[test]
fn p0_002c4d1b_selectors_without_a_current_registration_never_provision() {
    let f = Fixture::new();
    let before = tree(&f.path);
    let unknown = Uuid::new_v4().to_string();
    for selector in ["", "not-a-uuid", "../react", unknown.as_str()] {
        assert_eq!(
            f.authority
                .provision_workspace(selector, &scaffold(), f.audit()),
            Err("project not registered"),
            "{selector:?}"
        );
    }
    assert_eq!(tree(&f.path), before);
    // Tombstoned registration.
    let (id, root) = registered_only(&f);
    let registration = project(&f, &id);
    f.authority.catalog.invalidate(&registration).unwrap();
    assert_eq!(
        f.authority.provision_workspace(&id, &scaffold(), f.audit()),
        Err("project not registered")
    );
    assert!(!root.join(REACT).exists() && !root.join(RUNTIME).exists());
    // An already provisioned workspace is never provisioned again.
    let (id, root) = registered(&f);
    let provisioned_tree = tree(&root);
    assert_eq!(
        f.authority.provision_workspace(&id, &scaffold(), f.audit()),
        Err("workspace already provisioned")
    );
    assert_eq!(tree(&root), provisioned_tree);
}

#[test]
fn p0_002c4d1b_provisioning_binds_only_the_selected_registration() {
    let f = Fixture::new();
    let (first, first_root) = registered_only(&f);
    let (second, second_root) = registered_only(&f);
    let untouched = tree(&second_root);
    f.authority
        .provision_workspace(&first, &scaffold(), f.audit())
        .unwrap();
    assert!(first_root.join(REACT).is_dir());
    assert_eq!(tree(&second_root), untouched);
    assert!(provisioned(&f, &first) && !provisioned(&f, &second));
    assert_eq!(
        f.authority
            .write_file(&second, "src/x.ts", b"denied", f.audit()),
        Err("workspace not provisioned")
    );
}

#[test]
fn p0_002c4d1b_fresh_authority_never_inherits_provisioning() {
    let f = Fixture::new();
    let (id, root) = registered(&f);
    let restarted =
        BuilderWorkspaceAuthority::provision(Arc::new(WorkspaceAuthorityRegistry::new()), &f.path)
            .unwrap();
    let before = tree(&root);
    assert_eq!(
        restarted.provision_workspace(&id, &scaffold(), f.audit()),
        Err("project not registered")
    );
    assert_eq!(
        restarted.write_file(&id, "src/x.ts", b"denied", f.audit()),
        Err("project not registered")
    );
    assert_eq!(
        restarted.dev_server_start(&id, f.audit()),
        "project not registered"
    );
    assert_eq!(tree(&root), before);
}

// ── Identity failures ─────────────────────────────────────────────────────

#[test]
fn p0_002c4d1b_storage_and_project_identity_changes_deny_before_mutation() {
    for case in ["project-removed", "project-replaced", "storage-replaced"] {
        let f = Fixture::new();
        let (id, root) = registered_only(&f);
        let detached = f.path.with_extension("project");
        let old = f.path.with_extension("old");
        match case {
            "project-removed" => std::fs::remove_dir_all(&root).unwrap(),
            "project-replaced" => {
                std::fs::rename(&root, root.with_extension("original")).unwrap();
                std::fs::create_dir(&root).unwrap();
            }
            _ => {
                // Move the observed project first (Windows share-delete rule).
                std::fs::rename(&root, &detached).unwrap();
                std::fs::rename(&f.path, &old).unwrap();
                std::fs::create_dir(&f.path).unwrap();
                std::fs::rename(&detached, &root).unwrap();
            }
        }
        assert_eq!(
            f.authority.provision_workspace(&id, &scaffold(), f.audit()),
            Err("registration identity denied"),
            "{case}"
        );
        assert!(!root.join(REACT).exists(), "{case}");
        assert!(!root.join(RUNTIME).exists(), "{case}");
        assert!(
            !root.with_extension("original").join(REACT).exists(),
            "{case}"
        );
        // Existing C3 semantics: storage/project Changed permanently tombstones.
        assert!(f
            .authority
            .catalog
            .lookup(Uuid::parse_str(&id).unwrap())
            .is_err());
        let _ = std::fs::remove_dir_all(&old);
    }
}

#[test]
fn p0_002c4d1b_provisioned_registration_change_tombstones_the_workspace() {
    let f = Fixture::new();
    let (id, root) = registered(&f);
    let weak = Arc::downgrade(&project(&f, &id));
    // Removal is possible on every OS (a rename of the project is not on Windows
    // while the registration retains React/runtime identities).
    std::fs::remove_dir_all(&root).unwrap();
    assert_eq!(
        f.authority
            .write_file(&id, "src/x.ts", b"denied", f.audit()),
        Err("registration identity denied")
    );
    assert_eq!(
        f.authority.dev_server_start(&id, f.audit()),
        "project not registered"
    );
    assert!(weak.upgrade().is_none(), "retained workspace released");
    assert!(!root.exists(), "nothing was recreated");
}

#[test]
fn p0_002c4d1b_react_and_runtime_replacement_deny_governed_use() {
    let f = Fixture::new();
    let (id, root) = registered(&f);
    let react = root.join(REACT);
    let original = root.join("react-original");
    std::fs::rename(&react, &original).unwrap();
    std::fs::create_dir(&react).unwrap();
    assert_eq!(
        f.authority.write_file(&id, "x.ts", b"denied", f.audit()),
        Err("React identity denied")
    );
    assert_ne!(
        f.authority.dev_server_start(&id, f.audit()),
        "launch unavailable"
    );
    assert!(!react.join("x.ts").exists() && !original.join("x.ts").exists());
    // Identity, not the path, is the authority: restoring the governed
    // directory restores governed use.
    std::fs::remove_dir(&react).unwrap();
    std::fs::rename(&original, &react).unwrap();
    f.authority
        .write_file(&id, "x.ts", b"governed", f.audit())
        .unwrap();
    // Runtime replacement is detected by its retained identity.
    let runtime = root.join(RUNTIME);
    std::fs::rename(&runtime, root.join("runtime-original")).unwrap();
    std::fs::create_dir(&runtime).unwrap();
    let registration = project(&f, &id);
    assert_eq!(
        registration
            .workspace
            .get()
            .unwrap()
            .validate_runtime(&root),
        Err(IdentityError::Changed)
    );
}

// ── Pre-existing state and partial failure ────────────────────────────────

#[test]
fn p0_002c4d1b_preexisting_controlled_entries_are_never_adopted() {
    for (entry, directory) in [
        (REACT, true),
        (REACT, false),
        (RUNTIME, true),
        (RUNTIME, false),
    ] {
        let f = Fixture::new();
        let (id, root) = registered_only(&f);
        let planted = root.join(entry);
        if directory {
            std::fs::create_dir(&planted).unwrap();
            std::fs::write(planted.join("planted"), b"not governed").unwrap();
        } else {
            std::fs::write(&planted, b"not governed").unwrap();
        }
        let before = tree(&root);
        assert_eq!(
            f.authority.provision_workspace(&id, &scaffold(), f.audit()),
            Err("controlled entry already exists"),
            "{entry} dir={directory}"
        );
        // Nothing adopted or created; React from this attempt was rolled back.
        assert_eq!(tree(&root), before, "{entry} dir={directory}");
        assert!(!provisioned(&f, &id));
        assert_eq!(
            f.authority.write_file(&id, "x.ts", b"denied", f.audit()),
            Err("workspace not provisioned")
        );
    }
}

#[test]
fn p0_002c4d1b_midway_failure_rolls_back_and_retry_never_adopts_partial_state() {
    let f = Fixture::new();
    let (id, root) = registered_only(&f);
    let runtime = root.join(RUNTIME);
    let planted = runtime.clone();
    // Deterministic injection: once React exists, a runtime entry appears.
    let (audit, events) = hooked(&f, move |event| {
        if is_event(event, "builder.workspace.react", "created") {
            std::fs::create_dir(&planted).unwrap();
        }
    });
    assert_eq!(
        f.authority.provision_workspace(&id, &scaffold(), audit),
        Err("controlled entry already exists")
    );
    assert!(
        !root.join(REACT).exists(),
        "partial React must be rolled back"
    );
    assert!(runtime.is_dir(), "a foreign entry is never removed");
    assert!(!provisioned(&f, &id));
    let recorded = events.lock().unwrap().clone();
    assert!(recorded.iter().any(|event| is_event(
        event,
        "builder.workspace.rollback",
        "completed"
    )));
    assert!(recorded
        .iter()
        .any(|event| is_event(event, "builder.workspace.revoke", "revoked")));
    assert_eq!(
        f.authority.write_file(&id, "x.ts", b"denied", f.audit()),
        Err("workspace not provisioned")
    );
    // A retry sees the foreign entry and still refuses to adopt it.
    assert_eq!(
        f.authority.provision_workspace(&id, &scaffold(), f.audit()),
        Err("controlled entry already exists")
    );
    assert!(!root.join(REACT).exists());
    // Only once the foreign state is gone does a fresh provisioning succeed.
    std::fs::remove_dir(&runtime).unwrap();
    f.authority
        .provision_workspace(&id, &scaffold(), f.audit())
        .unwrap();
    assert!(provisioned(&f, &id));
}

#[test]
fn p0_002c4d1b_incomplete_rollback_is_reported_and_blocks_retry() {
    let f = Fixture::new();
    let (id, root) = registered_only(&f);
    let (react, runtime) = (root.join(REACT), root.join(RUNTIME));
    let (intruded, planted) = (react.clone(), runtime.clone());
    let (audit, events) = hooked(&f, move |event| {
        if is_event(event, "builder.workspace.react", "created") {
            std::fs::write(intruded.join("intruder"), b"x").unwrap();
            std::fs::create_dir(&planted).unwrap();
        }
    });
    assert_eq!(
        f.authority.provision_workspace(&id, &scaffold(), audit),
        Err("provisioning failed; rollback incomplete")
    );
    assert!(events.lock().unwrap().iter().any(|event| is_event(
        event,
        "builder.workspace.rollback",
        "failed"
    )));
    assert!(react.join("intruder").is_file() && !provisioned(&f, &id));
    std::fs::remove_dir(&runtime).unwrap();
    // The leftover React is never adopted by a retry.
    assert_eq!(
        f.authority.provision_workspace(&id, &scaffold(), f.audit()),
        Err("controlled entry already exists")
    );
    assert_eq!(
        f.authority.write_file(&id, "x.ts", b"denied", f.audit()),
        Err("workspace not provisioned")
    );
}

#[test]
fn p0_002c4d1b_forbidden_scaffold_rejects_before_any_mutation() {
    let f = Fixture::new();
    let (id, root) = registered_only(&f);
    let before = tree(&root);
    for forbidden in [
        ".env",
        "src/.env.local",
        "package-lock.json",
        "vite.config.mjs",
        "src/postcss.config.cjs",
        "public/tailwind.config.js",
        "babel.config.js",
        "src/.babelrc",
        "public/package.json",
        "node_modules/react/index.js",
        "src/node_modules/x.js",
    ] {
        let mut files = scaffold();
        files.push((forbidden, &b"x"[..]));
        assert_eq!(
            f.authority.provision_workspace(&id, &files, f.audit()),
            Err("scaffold configuration denied"),
            "{forbidden}"
        );
        assert_eq!(tree(&root), before, "{forbidden}: mutated before policy");
    }
    assert!(!provisioned(&f, &id));
}

// ── Scaffold policy ───────────────────────────────────────────────────────

#[test]
fn p0_002c4d1b_scaffold_policy_allows_only_project_content() {
    let bytes = &b"x"[..];
    let allowed = scaffold_content(&[
        ("index.html", bytes),
        ("src/App.tsx", bytes),
        ("src/components/deep/Nested.tsx", bytes),
        ("public/favicon.svg", bytes),
        ("public/images/logo.png", bytes),
        ("package.json", bytes),
        ("vite.config.ts", bytes),
        ("README.md", bytes),
    ])
    .unwrap();
    assert_eq!(
        allowed.iter().map(|file| file.path).collect::<Vec<_>>(),
        [
            "index.html",
            "public/favicon.svg",
            "public/images/logo.png",
            "src/App.tsx",
            "src/components/deep/Nested.tsx",
        ]
    );
    let long = format!("src/{}", "a".repeat(MAX_SCAFFOLD_PATH));
    for path in [
        "",
        "robots.txt",
        "src",
        "SRC/App.tsx",
        "Public/x.svg",
        "assets/x.png",
        "../x",
        "src/../x",
        "./src/x",
        "src/./x",
        "src//x",
        "/abs",
        "/etc/passwd",
        "C:/x",
        "src/C:x",
        "\\\\server\\share",
        "src\\x",
        "src/a:b",
        "src/CON.tsx",
        "public/nul",
        "src/x.",
        "src/x ",
        "src/caf\u{e9}.ts",
        long.as_str(),
    ] {
        assert!(scaffold_content(&[(path, bytes)]).is_err(), "{path:?}");
    }
    for files in [
        vec![("src/a.ts", bytes), ("src/a.ts", bytes)],
        vec![("src/A.ts", bytes), ("src/a.ts", bytes)],
        vec![("src/a", bytes), ("src/a/b.ts", bytes)],
        vec![("src/Dir/x.ts", bytes), ("src/dir/y.ts", bytes)],
    ] {
        assert!(scaffold_content(&files).is_err(), "{files:?}");
    }
    let oversized = vec![0u8; MAX_SCAFFOLD_FILE_BYTES + 1];
    assert!(scaffold_content(&[("src/big.bin", &oversized)]).is_err());
    assert!(scaffold_content(&[]).unwrap().is_empty());
}

// ── Project/runtime separation ────────────────────────────────────────────

#[test]
fn p0_002c4d1b_c3_writes_can_never_reach_runtime() {
    let f = Fixture::new();
    let (id, root) = registered(&f);
    let (react, runtime) = (root.join(REACT), root.join(RUNTIME));
    let absolute = runtime.join("env").join("x").to_string_lossy().into_owned();
    for relative in [
        "../runtime/env/x",
        "runtime/env/x",
        "src/../../runtime/env/x",
        "./runtime/env/x",
        "RUNTIME/env/x",
        "..\\runtime\\env\\x",
        "runtime\\env\\x",
        "runtime:env",
        "C:/runtime/env/x",
        "\\\\?\\C:\\runtime",
        "//server/share/runtime",
        absolute.as_str(),
    ] {
        assert!(
            f.authority
                .write_file(&id, relative, b"x", f.audit())
                .is_err(),
            "{relative}"
        );
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;
        std::fs::write(runtime.join("env/existing"), b"original").unwrap();
        symlink(&runtime, react.join("dirlink")).unwrap();
        symlink(runtime.join("env/existing"), react.join("filelink")).unwrap();
        for relative in ["dirlink/env/x", "dirlink/env/existing", "filelink"] {
            assert!(
                f.authority
                    .write_file(&id, relative, b"x", f.audit())
                    .is_err(),
                "{relative}"
            );
        }
        assert_eq!(
            std::fs::read(runtime.join("env/existing")).unwrap(),
            b"original"
        );
        std::fs::remove_file(runtime.join("env/existing")).unwrap();
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::symlink_dir;
        symlink_dir(&runtime, react.join("dirlink"))
            .expect("native Windows test requires symlink creation privilege");
        assert!(f
            .authority
            .write_file(&id, "dirlink/env/x", b"x", f.audit())
            .is_err());
    }
    let mut runtime_tree = tree(&root);
    runtime_tree.retain(|entry| entry.starts_with("runtime/"));
    assert_eq!(runtime_tree, set(&RUNTIME_TREE));
}

// ── Native redirection ────────────────────────────────────────────────────

#[cfg(unix)]
#[test]
fn p0_002c4d1b_unix_symlink_destinations_and_redirects_deny() {
    use std::os::unix::fs::symlink;
    for entry in [REACT, RUNTIME] {
        let f = Fixture::new();
        let (id, root) = registered_only(&f);
        let outside = f.path.with_extension("outside");
        std::fs::create_dir(&outside).unwrap();
        symlink(&outside, root.join(entry)).unwrap();
        assert_eq!(
            f.authority.provision_workspace(&id, &scaffold(), f.audit()),
            Err("controlled entry already exists"),
            "{entry}"
        );
        assert!(tree(&outside).is_empty(), "{entry}: redirected write");
        if entry == RUNTIME {
            assert!(!root.join(REACT).exists(), "React rolled back");
        }
        assert!(!provisioned(&f, &id));
        std::fs::remove_dir_all(&outside).unwrap();
    }
    // The project path is redirected after the project was bound: creation
    // stays in the bound object and the redirect is denied and rolled back.
    let f = Fixture::new();
    let (id, root) = registered_only(&f);
    let decoy = f.path.with_extension("decoy");
    std::fs::create_dir(&decoy).unwrap();
    let (moved, redirect, target) = (root.with_extension("moved"), root.clone(), decoy.clone());
    let (audit, _) = hooked(&f, move |event| {
        if is_event(event, "builder.workspace.react", "created") {
            std::fs::rename(&redirect, &moved).unwrap();
            symlink(&target, &redirect).unwrap();
        }
    });
    assert_eq!(
        f.authority.provision_workspace(&id, &scaffold(), audit),
        Err("registration identity denied")
    );
    assert!(
        tree(&decoy).is_empty(),
        "decoy received provisioning output"
    );
    assert!(!root.with_extension("moved").join(REACT).exists());
    assert!(f
        .authority
        .catalog
        .lookup(Uuid::parse_str(&id).unwrap())
        .is_err());
    std::fs::remove_dir_all(&decoy).unwrap();
    // A content directory redirected inside React is never followed.
    let f = Fixture::new();
    let (id, root) = registered_only(&f);
    let outside = f.path.with_extension("outside");
    std::fs::create_dir(&outside).unwrap();
    let (link, into) = (root.join("react/src"), outside.clone());
    let (audit, _) = hooked(&f, move |event| {
        if is_event(event, "builder.workspace.runtime", "created") {
            symlink(&into, &link).unwrap();
        }
    });
    assert_eq!(
        f.authority.provision_workspace(&id, &scaffold(), audit),
        Err("provisioning failed; rollback incomplete")
    );
    assert!(tree(&outside).is_empty(), "redirected content write");
    assert!(!provisioned(&f, &id));
    std::fs::remove_dir_all(&outside).unwrap();
}

#[cfg(windows)]
#[test]
fn p0_002c4d1b_windows_reparse_destinations_and_redirects_deny() {
    use std::os::windows::fs::symlink_dir;
    for entry in [REACT, RUNTIME] {
        let f = Fixture::new();
        let (id, root) = registered_only(&f);
        let outside = f.path.with_extension("outside");
        std::fs::create_dir(&outside).unwrap();
        symlink_dir(&outside, root.join(entry))
            .expect("native Windows test requires symlink creation privilege");
        assert_eq!(
            f.authority.provision_workspace(&id, &scaffold(), f.audit()),
            Err("controlled entry already exists"),
            "{entry}"
        );
        assert!(tree(&outside).is_empty(), "{entry}: redirected write");
        if entry == RUNTIME {
            assert!(!root.join(REACT).exists(), "React rolled back");
        }
        assert!(!provisioned(&f, &id));
        std::fs::remove_dir(root.join(entry)).unwrap();
        std::fs::remove_dir_all(&outside).unwrap();
    }
    // A reparse-redirected project (an unprovisioned registration can still be
    // renamed) is denied before any mutation.
    let f = Fixture::new();
    let (id, root) = registered_only(&f);
    let old = root.with_extension("old");
    std::fs::rename(&root, &old).unwrap();
    symlink_dir(&old, &root).expect("native Windows test requires symlink creation privilege");
    assert_eq!(
        f.authority.provision_workspace(&id, &scaffold(), f.audit()),
        Err("registration identity denied")
    );
    assert!(!old.join(REACT).exists() && !old.join(RUNTIME).exists());
}

// ── Events and authority ──────────────────────────────────────────────────

#[test]
fn p0_002c4d1b_events_are_bounded_and_every_grant_is_revoked() {
    let f = Fixture::new();
    let (id, _) = registered_only(&f);
    let (denied, _) = registered_only(&f);
    let from = f.events.lock().unwrap().len();
    f.authority
        .provision_workspace(&id, &scaffold(), f.audit())
        .unwrap();
    // Denied before any grant is issued (policy precedes authority issuance).
    let mut files = scaffold();
    files.push((".env", &b"SECRET=c4d1b-sentinel"[..]));
    assert!(f
        .authority
        .provision_workspace(&denied, &files, f.audit())
        .is_err());
    let events: Vec<Value> = f.events.lock().unwrap()[from..].to_vec();
    let storage = f.path.to_string_lossy().into_owned();
    let expected_keys = set(&["operation", "outcome", "project_id"]);
    for event in &events {
        let text = event.to_string();
        assert!(
            !text.contains(&storage) && !text.contains("sentinel"),
            "{text}"
        );
        let keys: BTreeSet<String> = event.as_object().unwrap().keys().cloned().collect();
        assert_eq!(keys, expected_keys, "{text}");
    }
    let count = |operation: &str, outcome: &str| {
        events
            .iter()
            .filter(|event| is_event(event, operation, outcome))
            .count()
    };
    assert_eq!(count("builder.workspace.issue", "authorized"), 1);
    assert_eq!(count("builder.workspace.revoke", "revoked"), 1);
    assert_eq!(count("builder.workspace", "provisioned"), 1);
    assert_eq!(count("builder.workspace", "denied"), 1);
}

// ── No launch or environment authority ────────────────────────────────────

fn code_only(source: &str) -> String {
    source
        .replace("\r\n", "\n")
        .lines()
        .map(|line| line.split("//").next().unwrap_or(""))
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn p0_002c4d1b_provisioning_has_no_launch_or_environment_authority() {
    let module = code_only(include_str!("../workspace_provisioning.rs"));
    for forbidden in [
        "Command::",
        "std::process",
        ".spawn(",
        "spawn_sealed",
        "SealedSpawn",
        "ResourceLimiter",
        "tauri::",
        "env::var",
        "var_os",
        "current_dir",
        "temp_dir",
        "home_dir",
        "create_dir_all",
        "canonicalize",
    ] {
        assert!(!module.contains(forbidden), "{forbidden}");
    }
    // Production wiring: the deterministic scaffold and governed provisioning
    // only; no Tauri command exposes provisioning.
    let adapter = code_only(include_str!("../../builder_workspace.rs"));
    let start = adapter.find("fn provision_planned_workspace(").unwrap();
    let end = start + adapter[start..].find("\n}\n").unwrap();
    let wiring = &adapter[start..end];
    assert!(wiring.contains("run_build_pipeline(") && wiring.contains("provision_workspace("));
    for forbidden in [
        "Command",
        ".spawn(",
        "std::fs",
        "std::process",
        "npm",
        "vite::",
    ] {
        assert!(!wiring.contains(forbidden), "{forbidden}");
    }
    let lib = code_only(include_str!("../../lib.rs"));
    assert!(!lib.contains("provision_workspace") && !lib.contains("workspace_provisioning"));
}
