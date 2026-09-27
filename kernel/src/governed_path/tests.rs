use super::*;

const HOSTILE_COMPONENTS: &[&str] = &[
    "",
    ".",
    "..",
    "a/b",
    "a\\b",
    "C:",
    "C:x",
    "file.txt:stream",
    "trailing.",
    "trailing ",
    "CON",
    "con.json",
    "Nul",
    "COM1",
    "lpt9.log",
    "COM¹",
    "CONIN$",
    "bell\u{7}",
    "tab\t",
    "<x>",
    "pipe|x",
    "star*",
    "quote\"",
    "ask?",
];

#[test]
fn components_reject_separators_prefixes_streams_devices_and_controls() {
    for component in HOSTILE_COMPONENTS {
        assert_eq!(
            validate_component(component),
            Err(PathDenied::InvalidName),
            "{component:?}"
        );
    }
    assert!(validate_component(&"x".repeat(MAX_COMPONENT_BYTES + 1)).is_err());
    for component in [
        "file.json",
        "Llama-3.2-1B.Q4_K_M.gguf",
        "résumé",
        "COM10",
        "console",
    ] {
        assert_eq!(validate_component(component), Ok(()), "{component:?}");
    }
}

#[test]
fn relative_paths_reject_absolute_prefixed_parent_and_empty_forms() {
    for relative in [
        "",
        "/",
        "/etc/passwd",
        "a//b",
        "a/./b",
        "a/../b",
        "../escape",
        "../../escape",
        "a/",
        "C:\\Windows",
        "C:/Windows",
        "\\\\server\\share\\x",
        "//server/share/x",
        "\\\\?\\C:\\x",
        "a\\b",
        "a/b:ads",
        "dir/CON",
        "dir/aux.txt",
    ] {
        assert_eq!(
            validate_relative(relative),
            Err(PathDenied::InvalidRelative),
            "{relative:?}"
        );
    }
    let deep = vec!["d"; MAX_DEPTH + 1].join("/");
    assert!(validate_relative(&deep).is_err());
    let long = format!("{}/{}/{}", "x".repeat(80), "y".repeat(80), "z".repeat(80));
    assert!(long.len() > MAX_RELATIVE_BYTES);
    assert!(validate_relative(&long).is_err());
    assert_eq!(validate_relative("notes/2026/n-1.json"), Ok(()));
    assert_eq!(validate_relative(&vec!["d"; MAX_DEPTH].join("/")), Ok(()));
}

#[test]
fn identifiers_are_a_narrow_ascii_grammar() {
    for id in [
        "",
        ".hidden",
        "-flag",
        "_x",
        "a/b",
        "a\\b",
        "..",
        "../../escape",
        "x.",
        "CON",
        "con.x",
        "a b",
        "ü",
        "a:b",
        "a=b",
        "%2e%2e",
    ] {
        assert_eq!(
            validate_identifier(id, 64),
            Err(PathDenied::InvalidName),
            "{id:?}"
        );
    }
    assert!(validate_identifier(&"a".repeat(65), 64).is_err());
    for id in ["n-1712345678901", "default", "em-1", "abc.def_1-2", "0"] {
        assert_eq!(validate_identifier(id, 64), Ok(()), "{id:?}");
    }
}

#[test]
fn storage_stems_are_valid_deterministic_and_distinct() {
    assert_eq!(storage_stem("em-1712"), "em-1712");
    let hostile = [
        "../../escape",
        "/etc/passwd",
        "C:\\x",
        "AAMkAG+/base64==",
        "h-abc",
        "CON",
        "",
        &"x".repeat(300),
    ];
    let mut seen = std::collections::HashSet::new();
    for id in hostile {
        let stem = storage_stem(id);
        assert!(
            stem.starts_with("h-") && stem.len() == 66,
            "{id:?} -> {stem}"
        );
        assert_eq!(validate_identifier(&stem, 66), Ok(()));
        assert_eq!(stem, storage_stem(id));
        assert!(seen.insert(stem));
    }
    // A caller cannot pick an identifier that lands on another's hashed stem.
    let hashed = storage_stem("../../escape");
    assert_ne!(storage_stem(&hashed), hashed);
}

#[test]
fn join_relative_stays_beneath_the_root() {
    let root = Path::new("/nexus/root");
    assert_eq!(
        join_relative(root, "a/b.json").unwrap(),
        root.join("a").join("b.json")
    );
    assert!(join_relative(root, "../b.json").is_err());
    assert!(join_relative(root, "/etc/passwd").is_err());
}

#[test]
fn roots_must_be_existing_canonical_directories() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    assert_eq!(existing_root(&root), Ok(()));
    assert!(existing_root(Path::new("relative/root")).is_err());
    assert!(existing_root(&root.join("missing")).is_err());
    std::fs::write(root.join("file"), b"x").unwrap();
    assert!(existing_root(&root.join("file")).is_err());
    std::fs::create_dir(root.join("child")).unwrap();
    assert_eq!(existing_root(&root.join("child")), Ok(()));
    let dotted = dotted_spelling(&root, "child");
    assert!(existing_root(&dotted).is_err(), "{dotted:?}");
}

/// `dir/<name>/../<name>` spelled from text: a non-canonical spelling of an
/// existing directory. It cannot come from `join`, because on Windows
/// pushing `..` onto the verbatim (`\\?\`) path `canonicalize` returns
/// normalizes the `..` away; the plain form keeps it.
fn dotted_spelling(dir: &Path, name: &str) -> PathBuf {
    let text = dir.to_string_lossy();
    let text = text.strip_prefix(r"\\?\").unwrap_or(&text);
    let sep = std::path::MAIN_SEPARATOR;
    PathBuf::from(format!("{text}{sep}{name}{sep}..{sep}{name}"))
}

#[test]
fn identity_is_stable_for_an_object_and_changes_when_replaced() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let first = root.join("first");
    std::fs::write(&first, b"one").unwrap();
    std::fs::write(root.join("second"), b"two").unwrap();
    let identity = identity_of(&first).unwrap();
    assert_eq!(identity_of(&first).unwrap(), identity);
    assert_ne!(identity_of(&root.join("second")).unwrap(), identity);
    std::fs::rename(&first, root.join("moved")).unwrap();
    std::fs::write(&first, b"one").unwrap();
    assert_ne!(identity_of(&first).unwrap(), identity);

    let governed = root.join("governed");
    std::fs::create_dir(&governed).unwrap();
    let before = identity_of(&governed).unwrap();
    std::fs::rename(&governed, root.join("governed.old")).unwrap();
    std::fs::create_dir(&governed).unwrap();
    assert_ne!(identity_of(&governed).unwrap(), before);
    assert_eq!(
        identity_of(&root.join("missing")),
        Err(PathDenied::Unavailable)
    );
}

#[cfg(unix)]
#[test]
fn unix_symlinks_are_redirects_not_roots_or_identities() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let target = root.join("target");
    std::fs::create_dir(&target).unwrap();
    std::os::unix::fs::symlink(&target, root.join("link")).unwrap();
    assert_eq!(
        existing_root(&root.join("link")),
        Err(PathDenied::Redirected)
    );
    assert_eq!(identity_of(&root.join("link")), Err(PathDenied::Redirected));
    std::os::unix::fs::symlink(root.join("absent"), root.join("dangling")).unwrap();
    assert_eq!(
        identity_of(&root.join("dangling")),
        Err(PathDenied::Redirected)
    );
}

#[cfg(windows)]
#[test]
fn windows_reparse_points_are_redirects_not_roots_or_identities() {
    use std::os::windows::fs::{symlink_dir, symlink_file};
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let target = root.join("target");
    std::fs::create_dir(&target).unwrap();
    std::fs::write(root.join("file"), b"x").unwrap();
    symlink_dir(&target, root.join("link"))
        .expect("native Windows test requires symlink creation privilege");
    symlink_file(root.join("file"), root.join("file-link"))
        .expect("native Windows test requires symlink creation privilege");
    assert_eq!(
        existing_root(&root.join("link")),
        Err(PathDenied::Redirected)
    );
    assert_eq!(identity_of(&root.join("link")), Err(PathDenied::Redirected));
    assert_eq!(
        identity_of(&root.join("file-link")),
        Err(PathDenied::Redirected)
    );
}

#[test]
fn private_temp_dirs_are_unpredictable_owner_only_and_removed() {
    let first = private_temp_dir("nexus-test-").unwrap();
    let second = private_temp_dir("nexus-test-").unwrap();
    assert_ne!(first.path(), second.path());
    assert!(first.path().is_dir());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(first.path())
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o700);
    }
    let path = first.path().to_path_buf();
    drop(first);
    assert!(!path.exists());
}

#[test]
fn regular_files_beneath_a_root_follow_no_link_and_leave_no_tree() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    std::fs::create_dir(root.join("models")).unwrap();
    std::fs::write(root.join("models").join("m.gguf"), b"x").unwrap();
    std::fs::write(root.join("secret"), b"s").unwrap();
    assert_eq!(
        regular_file_beneath(&root, "models/m.gguf").unwrap(),
        root.join("models").join("m.gguf")
    );
    for hostile in [
        "../secret",
        "/etc/passwd",
        "models/../secret",
        "models",
        "",
        "C:\\x",
    ] {
        assert!(regular_file_beneath(&root, hostile).is_err(), "{hostile:?}");
    }
    assert_eq!(
        regular_file_beneath(&root, "models"),
        Err(PathDenied::WrongKind)
    );
    assert!(regular_file_beneath(Path::new("relative"), "models/m.gguf").is_err());
}

#[cfg(unix)]
#[test]
fn unix_links_on_the_way_to_a_regular_file_are_refused() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let outside = root.join("outside");
    std::fs::create_dir(&outside).unwrap();
    std::fs::write(outside.join("secret"), b"s").unwrap();
    let models = root.join("models");
    std::fs::create_dir(&models).unwrap();
    std::os::unix::fs::symlink(outside.join("secret"), models.join("link.gguf")).unwrap();
    std::os::unix::fs::symlink(&outside, models.join("dir")).unwrap();
    assert_eq!(
        regular_file_beneath(&models, "link.gguf"),
        Err(PathDenied::Redirected)
    );
    assert_eq!(
        regular_file_beneath(&models, "dir/secret"),
        Err(PathDenied::Redirected)
    );
    std::os::unix::fs::symlink(&outside, root.join("root-link")).unwrap();
    assert_eq!(
        regular_file_beneath(&root.join("root-link"), "secret"),
        Err(PathDenied::Redirected)
    );
}
