use super::*;
use std::os::unix::fs::{symlink, PermissionsExt};

/// A disposable toolchain-shaped tree owned by this user, and its manifest.
struct Fixture {
    base: PathBuf,
    root: PathBuf,
    files: Vec<(String, Vec<u8>, bool)>,
}

impl Fixture {
    fn new(tag: &str) -> Self {
        let base = std::env::temp_dir().join(format!(
            "nexus-p2f-{tag}-{}-{}",
            std::process::id(),
            sys::random_hex(4).unwrap()
        ));
        let root = base.join("toolchain");
        let mut files: Vec<(String, Vec<u8>, bool)> = contract::REQUIRED
            .iter()
            .map(|(path, executable)| {
                (
                    path.to_string(),
                    format!("{path}\n").into_bytes(),
                    *executable,
                )
            })
            .collect();
        files.push(("lib/libextra.so".into(), b"library".to_vec(), false));
        files.push((
            "share/doc/rustc/COPYRIGHT".into(),
            b"notice".to_vec(),
            false,
        ));
        files.sort();
        for (path, content, executable) in &files {
            let full = root.join(path);
            std::fs::create_dir_all(full.parent().unwrap()).unwrap();
            std::fs::write(&full, content).unwrap();
            let mode = if *executable { 0o755 } else { 0o644 };
            std::fs::set_permissions(&full, std::fs::Permissions::from_mode(mode)).unwrap();
        }
        set_dir_modes(&root);
        Self { base, root, files }
    }

    /// The manifest of the fixture as created, leaked for a `'static` use.
    fn manifest(&self) -> &'static Manifest<'static> {
        let files: Vec<ManifestFile<'static>> = self
            .files
            .iter()
            .map(|(path, content, executable)| ManifestFile {
                path: Box::leak(path.clone().into_boxed_str()),
                size: content.len() as u64,
                executable: *executable,
                sha256: Sha256::digest(content).into(),
            })
            .collect();
        Box::leak(Box::new(Manifest {
            schema: contract::SCHEMA,
            rust_version: contract::RUST_VERSION,
            host: contract::HOST_TARGET,
            target: contract::VERIFIER_TARGET,
            files: Box::leak(files.into_boxed_slice()),
        }))
    }

    fn open_root(&self) -> Dir {
        open_path(&self.root, false).unwrap()
    }

    fn check(&self) -> Result<TreeFacts, ToolchainError> {
        verify_tree(self.manifest(), &self.open_root(), Owner::CurrentUser)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.base);
    }
}

fn set_dir_modes(dir: &Path) {
    std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o755)).unwrap();
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if std::fs::symlink_metadata(&path).unwrap().is_dir() {
            set_dir_modes(&path);
        }
    }
}

#[test]
fn p2f_the_manifest_grammar_is_enforced() {
    let fixture = Fixture::new("grammar");
    let good = fixture.manifest();
    assert_eq!(validate(good), Ok(()));
    let with = |files: Vec<ManifestFile<'static>>| -> Manifest<'static> {
        Manifest {
            files: Box::leak(files.into_boxed_slice()),
            ..*good
        }
    };
    let copy = |file: &ManifestFile<'static>| ManifestFile {
        path: file.path,
        size: file.size,
        executable: file.executable,
        sha256: file.sha256,
    };
    let base: Vec<ManifestFile<'static>> = good.files.iter().map(copy).collect();
    for bad in ["../escape", "a//b", "has space", "/absolute", "", "a/./b"] {
        let mut files: Vec<_> = good.files.iter().map(copy).collect();
        files.push(ManifestFile {
            path: bad,
            size: 1,
            executable: false,
            sha256: [0; 32],
        });
        files.sort_by(|a, b| a.path.cmp(b.path));
        assert_eq!(
            validate(&with(files)),
            Err(ToolchainError::InvalidManifest),
            "{bad:?}"
        );
    }
    // Unsorted, duplicated, a file that is also a directory.
    let mut unsorted: Vec<_> = good.files.iter().map(copy).collect();
    unsorted.reverse();
    assert_eq!(
        validate(&with(unsorted)),
        Err(ToolchainError::InvalidManifest)
    );
    let mut duplicated: Vec<_> = good.files.iter().map(copy).collect();
    duplicated.insert(1, copy(&good.files[0]));
    assert_eq!(
        validate(&with(duplicated)),
        Err(ToolchainError::InvalidManifest)
    );
    let mut shadowing: Vec<_> = good.files.iter().map(copy).collect();
    shadowing.push(ManifestFile {
        path: "lib",
        size: 1,
        executable: false,
        sha256: [0; 32],
    });
    shadowing.sort_by(|a, b| a.path.cmp(b.path));
    assert_eq!(
        validate(&with(shadowing)),
        Err(ToolchainError::InvalidManifest)
    );
    // Every required file, with its mode.
    for (required, _) in contract::REQUIRED {
        let without: Vec<_> = base
            .iter()
            .filter(|f| f.path != *required)
            .map(copy)
            .collect();
        assert_eq!(
            validate(&with(without)),
            Err(ToolchainError::InvalidManifest)
        );
        let flipped: Vec<_> = base
            .iter()
            .map(|f| ManifestFile {
                executable: if f.path == *required {
                    !f.executable
                } else {
                    f.executable
                },
                ..copy(f)
            })
            .collect();
        assert_eq!(
            validate(&with(flipped)),
            Err(ToolchainError::InvalidManifest)
        );
    }
    // Schema, release and platform are exact.
    assert_eq!(
        validate(&Manifest { schema: 2, ..*good }),
        Err(ToolchainError::InvalidManifest)
    );
    assert_eq!(
        validate(&Manifest {
            rust_version: "1.93.0",
            ..*good
        }),
        Err(ToolchainError::InvalidManifest)
    );
    assert_eq!(
        validate(&Manifest {
            host: "aarch64-unknown-linux-gnu",
            ..*good
        }),
        Err(ToolchainError::PlatformMismatch)
    );
    assert_eq!(
        validate(&Manifest {
            target: "x86_64-unknown-linux-gnu",
            ..*good
        }),
        Err(ToolchainError::PlatformMismatch)
    );
}

#[test]
fn p2f_the_exact_tree_verifies_and_every_deviation_is_rejected() {
    assert!(Fixture::new("exact").check().is_ok());
    type Mutation = fn(&Path);
    let deviations: [(&str, Mutation, ToolchainError); 11] = [
        (
            "missing file",
            |r| std::fs::remove_file(r.join("lib/libextra.so")).unwrap(),
            ToolchainError::Missing,
        ),
        (
            "extra file",
            |r| std::fs::write(r.join("lib/extra"), b"x").unwrap(),
            ToolchainError::Unexpected,
        ),
        (
            "extra directory",
            |r| std::fs::create_dir(r.join("lib/extra")).unwrap(),
            ToolchainError::Unexpected,
        ),
        (
            "symlinked file",
            |r| {
                std::fs::remove_file(r.join("lib/libextra.so")).unwrap();
                symlink(
                    "/usr/lib/x86_64-linux-gnu/libc.so.6",
                    r.join("lib/libextra.so"),
                )
                .unwrap();
            },
            ToolchainError::Redirected,
        ),
        (
            "symlinked directory",
            |r| {
                std::fs::remove_dir_all(r.join("share")).unwrap();
                symlink("/usr/share", r.join("share")).unwrap();
            },
            ToolchainError::Redirected,
        ),
        (
            "special file",
            |r| {
                std::fs::remove_file(r.join("lib/libextra.so")).unwrap();
                let path = CString::new(r.join("lib/libextra.so").as_os_str().as_bytes()).unwrap();
                // SAFETY: a NUL-terminated path.
                assert_eq!(unsafe { libc::mkfifo(path.as_ptr(), 0o644) }, 0);
            },
            ToolchainError::UnsupportedKind,
        ),
        (
            "same-size change",
            |r| std::fs::write(r.join("lib/libextra.so"), b"LIBRARY").unwrap(),
            ToolchainError::DigestMismatch,
        ),
        (
            "size change",
            |r| std::fs::write(r.join("lib/libextra.so"), b"library!").unwrap(),
            ToolchainError::SizeMismatch,
        ),
        (
            "mode change",
            |r| {
                std::fs::set_permissions(
                    r.join("bin/cargo"),
                    std::fs::Permissions::from_mode(0o644),
                )
                .unwrap()
            },
            ToolchainError::ModeMismatch,
        ),
        (
            "writable file",
            |r| {
                std::fs::set_permissions(
                    r.join("lib/libextra.so"),
                    std::fs::Permissions::from_mode(0o664),
                )
                .unwrap()
            },
            ToolchainError::OwnershipRejected,
        ),
        (
            "writable directory",
            |r| {
                std::fs::set_permissions(r.join("lib"), std::fs::Permissions::from_mode(0o775))
                    .unwrap()
            },
            ToolchainError::OwnershipRejected,
        ),
    ];
    for (what, mutate, expected) in deviations {
        let fixture = Fixture::new("deviation");
        let manifest = fixture.manifest();
        mutate(&fixture.root);
        let result = verify_tree(manifest, &fixture.open_root(), Owner::CurrentUser);
        assert_eq!(result.err(), Some(expected), "{what}");
    }
    // The installed-package invariant: a tree this user owns is not
    // root-owned.
    let fixture = Fixture::new("owner");
    assert_eq!(
        verify_tree(fixture.manifest(), &fixture.open_root(), Owner::Root).err(),
        Some(ToolchainError::OwnershipRejected)
    );
}

#[test]
fn p2f_verification_binds_tree_and_runtime_and_reverify_sees_changes() {
    let fixture = Fixture::new("full");
    let manifest = fixture.manifest();
    let first =
        VerifiedVerifierToolchain::verify_at(manifest, fixture.root.clone(), Owner::CurrentUser)
            .unwrap();
    let second =
        VerifiedVerifierToolchain::verify_at(manifest, fixture.root.clone(), Owner::CurrentUser)
            .unwrap();
    assert_eq!(first.digest(), second.digest());
    assert!(second.generation() > first.generation());
    assert_eq!(first.digest(), toolchain_digest(manifest, &first.runtime));
    assert_eq!(first.rust_version(), contract::RUST_VERSION);
    first.reverify().unwrap();
    // Another manifest (one byte of one file) is another digest.
    let mut other: Vec<ManifestFile<'static>> = manifest
        .files
        .iter()
        .map(|f| ManifestFile {
            path: f.path,
            size: f.size,
            executable: f.executable,
            sha256: f.sha256,
        })
        .collect();
    other[0].sha256[0] ^= 1;
    let other = Manifest {
        files: Box::leak(other.into_boxed_slice()),
        ..*manifest
    };
    assert_ne!(toolchain_digest(&other, &first.runtime), first.digest());
    // A change after verification is found by the re-verification.
    std::fs::write(fixture.root.join("lib/libextra.so"), b"LIBRARY").unwrap();
    assert_eq!(first.reverify(), Err(ToolchainError::DigestMismatch));
    std::fs::write(fixture.root.join("lib/libextra.so"), b"library").unwrap();
    first.reverify().unwrap();
    // A root that moved is no longer the verified toolchain.
    std::fs::rename(&fixture.root, fixture.base.join("moved")).unwrap();
    std::fs::create_dir(&fixture.root).unwrap();
    assert_eq!(first.reverify(), Err(ToolchainError::Changed));
}

#[test]
fn p2f_the_launch_material_is_the_verified_files() {
    let fixture = Fixture::new("launch");
    let toolchain = VerifiedVerifierToolchain::verify_at(
        fixture.manifest(),
        fixture.root.clone(),
        Owner::CurrentUser,
    )
    .unwrap();
    let launch = toolchain.launch().unwrap();
    let cargo = std::fs::metadata(fixture.root.join(contract::CARGO)).unwrap();
    let opened = sys::fstat(launch.executable.as_fd()).unwrap();
    use std::os::unix::fs::MetadataExt;
    assert_eq!((opened.st_dev, opened.st_ino), (cargo.dev(), cargo.ino()));
    let roles: Vec<Role> = launch.rules.iter().map(|(role, _)| *role).collect();
    assert_eq!(roles[0], Role::ToolchainRoot);
    assert_eq!(roles[1], Role::RuntimeLoader);
    assert_eq!(&roles[2..], [Role::RuntimeLibrary; 7]);
    assert_eq!(launch.rustc, fixture.root.join("bin/rustc"));
    assert_eq!(
        launch.linker,
        fixture
            .root
            .join("lib/rustlib/x86_64-unknown-linux-gnu/bin/rust-lld")
    );
    // An entry executable replaced after verification is refused.
    std::fs::remove_file(fixture.root.join(contract::CARGO)).unwrap();
    std::fs::write(fixture.root.join(contract::CARGO), b"bin/cargo\n").unwrap();
    assert_eq!(toolchain.launch().err(), Some(ToolchainError::Changed));
}

#[test]
fn p2f_runtime_files_must_be_protected_regular_files() {
    let fixture = Fixture::new("runtime");
    let dir = fixture.base.join("runtime");
    std::fs::create_dir(&dir).unwrap();
    std::fs::write(dir.join("libz.so.1.3"), b"z").unwrap();
    std::fs::set_permissions(
        dir.join("libz.so.1.3"),
        std::fs::Permissions::from_mode(0o644),
    )
    .unwrap();
    symlink("libz.so.1.3", dir.join("libz.so.1")).unwrap();
    // An otherwise acceptable file outside the runtime directory.
    std::fs::create_dir(fixture.base.join("escape")).unwrap();
    std::fs::write(fixture.base.join("escape/libz.so.1.3"), b"z").unwrap();
    std::fs::set_permissions(
        fixture.base.join("escape/libz.so.1.3"),
        std::fs::Permissions::from_mode(0o644),
    )
    .unwrap();
    symlink("../escape/libz.so.1.3", dir.join("libz-escape.so.1")).unwrap();
    std::fs::write(dir.join("writable.so"), b"w").unwrap();
    std::fs::set_permissions(
        dir.join("writable.so"),
        std::fs::Permissions::from_mode(0o666),
    )
    .unwrap();
    std::fs::create_dir(dir.join("dir.so")).unwrap();
    let fd = sys::open_fixed(
        &CString::new(dir.as_os_str().as_bytes()).unwrap(),
        libc::O_RDONLY | libc::O_DIRECTORY,
    )
    .unwrap();
    let file = runtime_file(fd.as_fd(), c"libz.so.1", Owner::CurrentUser).unwrap();
    assert_eq!(file.sha256, <[u8; 32]>::from(Sha256::digest(b"z")));
    for bad in [
        c"libz-escape.so.1",
        c"writable.so",
        c"dir.so",
        c"missing.so",
    ] {
        assert_eq!(
            runtime_file(fd.as_fd(), bad, Owner::CurrentUser).err(),
            Some(ToolchainError::RuntimeRejected),
            "{bad:?}"
        );
    }
    // This user's files are not the root-owned host runtime.
    assert_eq!(
        runtime_file(fd.as_fd(), c"libz.so.1", Owner::Root).err(),
        Some(ToolchainError::RuntimeRejected)
    );
    // The real host runtime of a supported host.
    let host = HostRuntime::verify(Owner::Root).unwrap();
    assert_eq!(host.libraries.len(), RUNTIME_LIBRARIES.len());
}

#[test]
fn p2f_production_is_unavailable_without_an_installed_package() {
    // A test build embeds no manifest, or this test executable is not the
    // installed application: either way the toolchain is unavailable.
    assert_eq!(
        VerifiedVerifierToolchain::verify_installed().err(),
        Some(ToolchainError::Unavailable)
    );
}
