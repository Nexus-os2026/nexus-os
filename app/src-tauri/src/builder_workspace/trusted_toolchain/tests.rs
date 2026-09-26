//! P0-002C4D1A verifier tests. Synthetic fixture trees and borrowed fixture
//! manifests only: no real toolchain, no process execution.
use super::contract::{MAX_FILE_BYTES, MAX_PATH_BYTES};
use super::*;

// ── Fixtures ──────────────────────────────────────────────────────────────

struct Fixture {
    root: PathBuf,
    // Test-only: the absolute path the fixture was created through, before
    // canonicalization. On Windows it is a non-verbatim spelling of `root`.
    #[cfg(windows)]
    created: PathBuf,
}

impl Fixture {
    fn new(files: &[(&str, &[u8])]) -> Self {
        let dir = std::env::temp_dir().join(format!("nexus-c4d1a-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&dir).unwrap();
        let root = dir.canonicalize().unwrap();
        for (path, content) in files {
            let target = at(&root, path);
            std::fs::create_dir_all(target.parent().unwrap()).unwrap();
            std::fs::write(target, content).unwrap();
        }
        Self {
            root,
            #[cfg(windows)]
            created: dir,
        }
    }
}

// Component-wise join: a Windows verbatim root does not normalize '/'.
fn at(root: &Path, relative: &str) -> PathBuf {
    relative
        .split('/')
        .fold(root.to_path_buf(), |path, part| path.join(part))
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

const TREE: &[(&str, &[u8])] = &[
    ("bin/node", b"synthetic node runtime"),
    ("entry.mjs", b"export {};\n"),
    ("node_modules/vite/package.json", b"{\"name\":\"vite\"}"),
    (
        "node_modules/vite/dist/index.js",
        b"export const vite = 1;\n",
    ),
];

fn entries<'a>(files: &[(&'a str, &[u8])]) -> Vec<ManifestFile<'a>> {
    let mut entries: Vec<_> = files
        .iter()
        .map(|(path, content)| ManifestFile {
            path,
            size: content.len() as u64,
            sha256: Sha256::digest(content).into(),
        })
        .collect();
    entries.sort_by(|a, b| a.path.cmp(b.path));
    entries
}

fn manifest<'a>(files: &'a [ManifestFile<'a>]) -> ToolchainManifest<'a> {
    ToolchainManifest {
        schema: SCHEMA,
        target: CURRENT_TARGET,
        files,
    }
}

fn file<'a>(path: &'a str) -> ManifestFile<'a> {
    ManifestFile {
        path,
        size: 0,
        sha256: [0; 32],
    }
}

// Every failure returns Err, so no VerifiedToolchain can exist on that path.
fn rejected(root: &Path, files: &[ManifestFile<'_>]) -> ToolchainError {
    match verify_tree(&manifest(files), root) {
        Ok(_) => panic!("verification must fail"),
        Err(error) => error,
    }
}

// ── Production fail-closed ─────────────────────────────────────────────────

// P0-002C4D2: only builds without an assembled packaged toolchain embed no
// production manifest; those remain unavailable before any filesystem access.
#[cfg(not(nexus_packaged_toolchain))]
#[test]
fn p0_002c4d1a_production_is_unavailable_without_any_filesystem_root() {
    assert!(std::hint::black_box(PRODUCTION_MANIFEST).is_none());
    assert!(matches!(
        verify_installed(),
        Err(ToolchainError::Unavailable)
    ));
    assert_eq!(
        ToolchainError::Unavailable.to_string(),
        "trusted toolchain unavailable"
    );
}

#[test]
fn p0_002c4d1a_empty_manifest_is_invalid_not_an_empty_toolchain() {
    let empty = Fixture::new(&[]);
    assert_eq!(
        validate_manifest(&manifest(&[]), &CURRENT_TARGET),
        Err(ToolchainError::InvalidManifest)
    );
    assert_eq!(rejected(&empty.root, &[]), ToolchainError::InvalidManifest);
}

// ── Manifest validation ────────────────────────────────────────────────────

#[test]
fn p0_002c4d1a_manifest_schema_and_target_must_match_exactly() {
    let files = entries(TREE);
    let fixture = Fixture::new(TREE);
    let wrong_schema = ToolchainManifest {
        schema: SCHEMA + 1,
        ..manifest(&files)
    };
    assert!(matches!(
        verify_tree(&wrong_schema, &fixture.root),
        Err(ToolchainError::InvalidManifest)
    ));
    for target in [
        ToolchainTarget {
            os: "plan9",
            ..CURRENT_TARGET
        },
        ToolchainTarget {
            arch: "sparc64",
            ..CURRENT_TARGET
        },
        ToolchainTarget {
            env: if CURRENT_TARGET.env == "musl" {
                "gnu"
            } else {
                "musl"
            },
            ..CURRENT_TARGET
        },
    ] {
        let other = ToolchainManifest {
            target,
            ..manifest(&files)
        };
        assert!(matches!(
            verify_tree(&other, &fixture.root),
            Err(ToolchainError::PlatformMismatch)
        ));
    }
    // An unsupported compile-time ABI can never match, even with itself.
    let unsupported = ToolchainTarget {
        env: UNSUPPORTED_ENV,
        ..CURRENT_TARGET
    };
    let same = ToolchainManifest {
        target: unsupported,
        ..manifest(&files)
    };
    assert_eq!(
        validate_manifest(&same, &unsupported),
        Err(ToolchainError::PlatformMismatch)
    );
}

#[test]
fn p0_002c4d1a_manifest_rejects_order_duplicates_aliases_and_prefix_conflicts() {
    for files in [
        vec![file("b"), file("a")],
        vec![file("a"), file("a")],
        vec![file("A"), file("a")],
        vec![file("Lib/x"), file("lib/y")],
        vec![file("a"), file("a/b")],
        vec![file("A"), file("a/b")],
        vec![file("x/a"), file("x/a/b")],
    ] {
        assert_eq!(
            validate_manifest(&manifest(&files), &CURRENT_TARGET),
            Err(ToolchainError::InvalidManifest),
            "{:?}",
            files.iter().map(|f| f.path).collect::<Vec<_>>()
        );
    }
    let oversized = [ManifestFile {
        size: MAX_FILE_BYTES + 1,
        ..file("a")
    }];
    assert_eq!(
        validate_manifest(&manifest(&oversized), &CURRENT_TARGET),
        Err(ToolchainError::InvalidManifest)
    );
}

#[test]
fn p0_002c4d1a_manifest_paths_follow_the_strict_grammar() {
    let long = "a".repeat(MAX_PATH_BYTES + 1);
    for path in [
        "",
        "..",
        "a/../b",
        ".",
        "./a",
        "/abs",
        "a/",
        "a//b",
        "a\\b",
        "C:",
        "C:/x",
        "a:b",
        "//server/share",
        "caf\u{e9}",
        "a b",
        "trailing.",
        "dir./x",
        "CON",
        "con.txt",
        "x/Nul.js",
        "lpt9",
        "COM1.cfg",
        "a*b",
        long.as_str(),
    ] {
        let files = [file(path)];
        assert_eq!(
            validate_manifest(&manifest(&files), &CURRENT_TARGET),
            Err(ToolchainError::InvalidManifest),
            "{path:?}"
        );
    }
    for path in [
        "node_modules/@scope/pkg+x/index.js",
        ".package-lock.json",
        "a-b_c.d/COM10",
        "console.log",
        "lpt",
    ] {
        let files = [file(path)];
        assert_eq!(
            validate_manifest(&manifest(&files), &CURRENT_TARGET),
            Ok(()),
            "{path:?}"
        );
    }
}

// ── Exact tree ─────────────────────────────────────────────────────────────

#[test]
fn p0_002c4d1a_exact_synthetic_tree_verifies() {
    let fixture = Fixture::new(TREE);
    let files = entries(TREE);
    let verified = verify_tree(&manifest(&files), &fixture.root).unwrap();
    assert_eq!(verified.root(), fixture.root);
    assert!(verified.root_still_pinned());
}

#[test]
fn p0_002c4d1a_missing_and_unexpected_entries_reject() {
    let files = entries(TREE);
    let missing = Fixture::new(&TREE[..3]);
    assert_eq!(rejected(&missing.root, &files), ToolchainError::Missing);

    let extra = Fixture::new(TREE);
    std::fs::write(at(&extra.root, "node_modules/extra.js"), b"x").unwrap();
    assert_eq!(rejected(&extra.root, &files), ToolchainError::Unexpected);

    let empty_dir = Fixture::new(TREE);
    std::fs::create_dir(at(&empty_dir.root, "node_modules/sass")).unwrap();
    assert_eq!(
        rejected(&empty_dir.root, &files),
        ToolchainError::Unexpected
    );

    let populated = Fixture::new(TREE);
    std::fs::create_dir_all(at(&populated.root, "lib/deep")).unwrap();
    std::fs::write(at(&populated.root, "lib/deep/x.js"), b"x").unwrap();
    assert_eq!(
        rejected(&populated.root, &files),
        ToolchainError::Unexpected
    );
}

#[test]
fn p0_002c4d1a_digest_and_size_mismatches_reject() {
    let files = entries(TREE);
    let tampered = Fixture::new(TREE);
    // Same length, different bytes: only the digest can catch it.
    std::fs::write(tampered.root.join("entry.mjs"), b"export {};\r").unwrap();
    assert_eq!(
        rejected(&tampered.root, &files),
        ToolchainError::DigestMismatch
    );

    let resized = Fixture::new(TREE);
    std::fs::write(at(&resized.root, "bin/node"), b"synthetic node runtime!").unwrap();
    assert_eq!(
        rejected(&resized.root, &files),
        ToolchainError::SizeMismatch
    );
}

#[test]
fn p0_002c4d1a_file_and_directory_kinds_must_match() {
    let files = entries(TREE);
    let dir_for_file = Fixture::new(&TREE[1..]);
    std::fs::create_dir_all(at(&dir_for_file.root, "bin/node")).unwrap();
    assert_eq!(
        rejected(&dir_for_file.root, &files),
        ToolchainError::Unexpected
    );

    let file_for_dir = Fixture::new(&TREE[..2]);
    std::fs::write(file_for_dir.root.join("node_modules"), b"x").unwrap();
    assert_eq!(
        rejected(&file_for_dir.root, &files),
        ToolchainError::Unexpected
    );
}

#[test]
fn p0_002c4d1a_root_must_be_absolute_existing_canonical_directory() {
    let fixture = Fixture::new(TREE);
    let files = entries(TREE);
    // A genuinely different absolute spelling of the same existing root. On
    // Windows the canonical root is verbatim (\\?\...) and std normalizes `..`
    // away when joining onto a verbatim path, so the non-verbatim creation path
    // is used; elsewhere an unresolved `bin/..` component remains in the path.
    #[cfg(windows)]
    let non_canonical = fixture.created.clone();
    #[cfg(not(windows))]
    let non_canonical = fixture.root.join("bin").join("..");
    assert!(non_canonical.is_absolute());
    assert_ne!(
        non_canonical, fixture.root,
        "fixture must actually use a distinct non-canonical spelling"
    );
    assert_eq!(
        non_canonical.canonicalize().unwrap(),
        fixture.root,
        "non-canonical spelling must resolve to the same real root"
    );
    for (case, root) in [
        ("relative", PathBuf::from("relative")),
        ("missing", fixture.root.join("missing")),
        ("regular-file", fixture.root.join("entry.mjs")),
        ("non-canonical", non_canonical),
    ] {
        match verify_tree(&manifest(&files), &root) {
            Ok(_) => panic!("{case}: verification must fail for {root:?}"),
            Err(error) => assert_eq!(error, ToolchainError::RootRejected, "{case}: {root:?}"),
        }
    }
}

#[test]
fn p0_002c4d1a_unexpected_entry_names_reject() {
    let files = entries(TREE);
    // Windows-special names (devices, ':' streams) are Unix fixtures only;
    // the manifest-grammar test covers them on every platform.
    #[cfg(unix)]
    let names = ["a b", "x:y", "CON", "caf\u{e9}", "back\\slash"];
    #[cfg(not(unix))]
    let names = ["a b", "caf\u{e9}"];
    for name in names {
        let fixture = Fixture::new(TREE);
        std::fs::write(fixture.root.join(name), b"x").unwrap();
        assert_eq!(
            rejected(&fixture.root, &files),
            ToolchainError::Unexpected,
            "{name}"
        );
    }
}

// ── Root identity binding ───────────────────────────────────────────────────

// A and B hold byte-identical trusted trees (same paths, sizes and digests),
// yet B must be refused when the native verifier is handed A's retained
// identity: the exact traversal-root handle must be the retained object.
// Deterministic on every platform: no races, sleeps or threads.
#[test]
fn p0_002c4d1a_native_root_handle_must_match_retained_identity() {
    let a = Fixture::new(TREE);
    let b = Fixture::new(TREE);
    let files = entries(TREE);
    let manifest = manifest(&files);
    let expected = Expected::new(&manifest);
    let identity_a = DirectoryIdentity::capture(&a.root).unwrap();

    // Control: A verifies natively against its own retained identity.
    let mut found = BTreeSet::new();
    assert_eq!(
        native::verify(&a.root, &identity_a, &expected, &mut found),
        Ok(())
    );
    assert_eq!(found.len(), files.len());

    // B differs only in native filesystem identity and is refused before any
    // of its entries are traversed.
    let mut found = BTreeSet::new();
    assert_eq!(
        native::verify(&b.root, &identity_a, &expected, &mut found),
        Err(ToolchainError::Changed)
    );
    assert!(found.is_empty());

    // Control: B's tree is itself valid under its own identity.
    let identity_b = DirectoryIdentity::capture(&b.root).unwrap();
    let mut found = BTreeSet::new();
    assert_eq!(
        native::verify(&b.root, &identity_b, &expected, &mut found),
        Ok(())
    );
    assert!(verify_tree(&manifest, &b.root).is_ok());
}

// ── Unix native redirection and unsupported kinds ──────────────────────────

#[cfg(unix)]
#[test]
fn p0_002c4d1a_unix_symlinks_are_never_followed() {
    use std::os::unix::fs::symlink;
    let files = entries(TREE);
    // A file symlink to identical trusted bytes is still rejected.
    let outside = Fixture::new(TREE);
    let file_link = Fixture::new(TREE);
    std::fs::remove_file(file_link.root.join("entry.mjs")).unwrap();
    symlink(
        outside.root.join("entry.mjs"),
        file_link.root.join("entry.mjs"),
    )
    .unwrap();
    assert_eq!(
        rejected(&file_link.root, &files),
        ToolchainError::Redirected
    );

    let dir_link = Fixture::new(TREE);
    std::fs::remove_dir_all(dir_link.root.join("node_modules")).unwrap();
    symlink(
        outside.root.join("node_modules"),
        dir_link.root.join("node_modules"),
    )
    .unwrap();
    assert_eq!(rejected(&dir_link.root, &files), ToolchainError::Redirected);

    let nested = Fixture::new(TREE);
    std::fs::remove_dir_all(at(&nested.root, "node_modules/vite/dist")).unwrap();
    symlink(
        at(&outside.root, "node_modules/vite/dist"),
        at(&nested.root, "node_modules/vite/dist"),
    )
    .unwrap();
    assert_eq!(rejected(&nested.root, &files), ToolchainError::Redirected);

    let extra_link = Fixture::new(TREE);
    symlink(
        outside.root.join("entry.mjs"),
        extra_link.root.join("linked.mjs"),
    )
    .unwrap();
    assert_eq!(
        rejected(&extra_link.root, &files),
        ToolchainError::Redirected
    );

    // A redirected root is rejected before traversal.
    let parent = Fixture::new(&[]);
    let linked_root = parent.root.join("root");
    symlink(&outside.root, &linked_root).unwrap();
    assert_eq!(rejected(&linked_root, &files), ToolchainError::RootRejected);
}

#[cfg(unix)]
#[test]
fn p0_002c4d1a_unix_fifo_is_rejected_without_blocking() {
    #[cfg(target_os = "macos")]
    type Mode = u16;
    #[cfg(not(target_os = "macos"))]
    type Mode = u32;
    unsafe extern "C" {
        fn mkfifo(path: *const std::ffi::c_char, mode: Mode) -> std::ffi::c_int;
    }
    use std::os::unix::ffi::OsStrExt;
    let files = entries(TREE);
    for expected_name in [true, false] {
        let fixture = Fixture::new(TREE);
        let path = if expected_name {
            std::fs::remove_file(fixture.root.join("entry.mjs")).unwrap();
            fixture.root.join("entry.mjs")
        } else {
            fixture.root.join("fifo")
        };
        let fifo = std::ffi::CString::new(path.as_os_str().as_bytes()).unwrap();
        // SAFETY: NUL-terminated owned pathname, valid POSIX mode, no retained pointer.
        assert_eq!(unsafe { mkfifo(fifo.as_ptr(), 0o600) }, 0);
        assert_eq!(
            rejected(&fixture.root, &files),
            ToolchainError::UnsupportedKind
        );
    }
}

// ── Windows native redirection ─────────────────────────────────────────────

#[cfg(windows)]
#[test]
fn p0_002c4d1a_windows_reparse_points_are_never_followed() {
    use std::os::windows::fs::{symlink_dir, symlink_file};
    let files = entries(TREE);
    let outside = Fixture::new(TREE);

    let dir_link = Fixture::new(TREE);
    std::fs::remove_dir_all(dir_link.root.join("node_modules")).unwrap();
    symlink_dir(
        outside.root.join("node_modules"),
        dir_link.root.join("node_modules"),
    )
    .expect("native Windows test requires symlink creation privilege");
    assert_eq!(rejected(&dir_link.root, &files), ToolchainError::Redirected);

    let file_link = Fixture::new(TREE);
    std::fs::remove_file(file_link.root.join("entry.mjs")).unwrap();
    symlink_file(
        outside.root.join("entry.mjs"),
        file_link.root.join("entry.mjs"),
    )
    .expect("native Windows test requires symlink creation privilege");
    assert_eq!(
        rejected(&file_link.root, &files),
        ToolchainError::Redirected
    );

    let nested = Fixture::new(TREE);
    std::fs::remove_dir_all(at(&nested.root, "node_modules/vite/dist")).unwrap();
    symlink_dir(
        at(&outside.root, "node_modules/vite/dist"),
        at(&nested.root, "node_modules/vite/dist"),
    )
    .expect("native Windows test requires symlink creation privilege");
    assert_eq!(rejected(&nested.root, &files), ToolchainError::Redirected);

    let extra_link = Fixture::new(TREE);
    symlink_file(
        outside.root.join("entry.mjs"),
        extra_link.root.join("linked.mjs"),
    )
    .expect("native Windows test requires symlink creation privilege");
    assert_eq!(
        rejected(&extra_link.root, &files),
        ToolchainError::Redirected
    );
}

// ── Source guards (only where behaviour cannot show the invariant) ─────────

// CRLF-normalized and comment-stripped; string literals are kept intact.
fn code(source: &str) -> String {
    source
        .replace("\r\n", "\n")
        .lines()
        .map(|line| line.split("//").next().unwrap_or(""))
        .collect::<Vec<_>>()
        .join("\n")
}

// Brace-depth extraction of one function (signature through matching brace).
fn function(source: &str, signature: &str) -> String {
    let start = source.find(signature).expect("function signature");
    let mut depth = 0usize;
    for (offset, c) in source[start..].char_indices() {
        match c {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return source[start..start + offset + 1].to_owned();
                }
            }
            _ => {}
        }
    }
    panic!("unbalanced function: {signature}");
}

fn compact(text: &str) -> String {
    text.split_whitespace().collect()
}

#[test]
fn p0_002c4d1a_production_manifest_and_entry_stay_unavailable() {
    // P0-002C4D2: the production manifest is only ever the build-generated
    // one (absent unless a packaged toolchain was assembled), and production
    // verification takes no path, root or manifest from any caller.
    let module = code(include_str!("../trusted_toolchain.rs"));
    assert!(compact(&module).contains(&compact(
        "include!(concat!(env!(\"OUT_DIR\"), \"/builder_toolchain_manifest.rs\"));"
    )));
    assert!(!module.contains("const PRODUCTION_MANIFEST"));
    assert_eq!(module.matches("PRODUCTION_MANIFEST").count(), 1);
    let installed = function(&module, "pub(super) fn verify_installed(");
    assert!(installed.starts_with("pub(super) fn verify_installed() ->"));
    for forbidden in [
        "verify_tree",
        "native::",
        "fs::",
        "DirectoryIdentity",
        "open",
    ] {
        assert!(
            !installed.contains(forbidden),
            "{forbidden} in:\n{installed}"
        );
    }
    assert_eq!(
        installed
            .matches("Err(ToolchainError::Unavailable)")
            .count(),
        1
    );
    // The opaque authority has exactly one constructor, at the end of verify_tree.
    assert_eq!(module.matches("VerifiedToolchain {").count(), 3); // struct, impl, constructor
    assert!(function(&module, "fn verify_tree(").contains("Ok(VerifiedToolchain {"));
}

#[test]
fn p0_002c4d1a_verifier_has_no_launch_command_or_environment_authority() {
    let module = code(include_str!("../trusted_toolchain.rs"));
    for forbidden in [
        "std::process",
        "Command",
        "spawn",
        "ResourceLimit",
        "SealedSpawn",
        "tauri",
        "env::var",
        "var_os",
        "current_dir",
        "temp_dir",
        "home_dir",
        "resource_dir",
        "APPDIR",
        "Serialize",
    ] {
        assert!(!module.contains(forbidden), "{forbidden}");
    }
    // P0-002C4D2: the one executable-path query that derives the production
    // root (see p0_002c4d2_production_root_uses_only_the_executable_path).
    assert_eq!(module.matches("current_exe").count(), 1);
    // The authority declaration carries no derive (no Clone, Copy or Default).
    let declaration = module.find("pub(super) struct VerifiedToolchain").unwrap();
    let preceding = &module[..declaration];
    let attributes = &preceding[preceding.rfind("\n\n").unwrap()..];
    assert!(!attributes.contains("derive"), "{attributes}");
    assert!(!module.contains("for VerifiedToolchain"));
    // Private to the Builder adapter: no command, re-export or frontend path.
    let adapter = code(include_str!("../../builder_workspace.rs"));
    assert!(adapter.contains("\nmod trusted_toolchain;"));
    assert!(
        !adapter.contains("pub mod trusted_toolchain")
            && !adapter.contains("pub use trusted_toolchain")
    );
    assert!(!code(include_str!("../../lib.rs")).contains("trusted_toolchain"));
}
