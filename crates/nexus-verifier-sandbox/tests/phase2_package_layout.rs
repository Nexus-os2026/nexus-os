//! P2-R1 package layout: the Linux package's verifier runtime, evaluated
//! where it was extracted rather than installed.
//!
//! Production derives the helper (`/usr/bin/nexus-verifier-sandbox`) and the
//! verifier toolchain (`/usr/lib/NexusOS/verifier-toolchain`) from the
//! application installed in `/usr/bin`, and requires root ownership. An
//! ordinary user who extracts the package owns what is extracted, so here
//! the same checks run beneath the extracted root with that user as owner;
//! the archive's root ownership is asserted from its own metadata by
//! `packaging/verifier-toolchain/scripts/inspect-deb.mjs`.
//!
//! A synthetic root is always evaluated. With
//! `NEXUS_EXTRACTED_PACKAGE_ROOT=<dpkg-deb -x output>`, the real package is
//! evaluated too (the release job), and it must hold the verifier toolchain
//! this build embeds (`NEXUS_VERIFIER_TOOLCHAIN=packaged`,
//! `--features development-toolchain`).

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
mod layout {
    use std::fs;
    #[cfg(feature = "development-toolchain")]
    use std::os::unix::fs::MetadataExt;
    use std::os::unix::fs::{symlink, PermissionsExt};
    use std::path::{Path, PathBuf};

    use nexus_verifier_sandbox::launcher::{HelperProgram, HelperUnavailable};
    use nexus_verifier_sandbox::toolchain::{ToolchainError, VerifiedVerifierToolchain};

    const HELPER: &str = env!("CARGO_BIN_EXE_nexus-verifier-sandbox");
    const EXTRACTED: &str = "NEXUS_EXTRACTED_PACKAGE_ROOT";
    const BIN: &str = "usr/bin";
    #[cfg(feature = "development-toolchain")]
    const TOOLCHAIN: &str = "usr/lib/NexusOS/verifier-toolchain";

    fn mode(path: &Path, mode: u32) {
        fs::set_permissions(path, fs::Permissions::from_mode(mode)).unwrap();
    }

    /// A fresh synthetic package root: the application and the helper in
    /// `usr/bin`, as the package installs them.
    fn synthetic(tag: &str) -> PathBuf {
        let root = Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("p2r1-package-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let bin = root.join(BIN);
        fs::create_dir_all(&bin).unwrap();
        for dir in [root.join("usr"), bin.clone()] {
            mode(&dir, 0o755);
        }
        fs::copy(
            std::env::current_exe().unwrap(),
            bin.join("nexus-desktop-backend"),
        )
        .unwrap();
        fs::copy(HELPER, bin.join("nexus-verifier-sandbox")).unwrap();
        mode(&bin.join("nexus-desktop-backend"), 0o755);
        mode(&bin.join("nexus-verifier-sandbox"), 0o755);
        root
    }

    #[test]
    fn p2r1_the_package_layout_is_what_production_derives() {
        assert!(
            std::env::var_os(EXTRACTED).is_none() || cfg!(feature = "development-toolchain"),
            "{EXTRACTED} requires --features development-toolchain"
        );
        let root = synthetic("helper");
        let helper = root.join(BIN).join("nexus-verifier-sandbox");
        let program = HelperProgram::in_extracted_package(&root).unwrap();
        assert_eq!(program.path(), helper);
        // Each deviation production refuses is refused here too.
        let refused = |what: &str, expected: HelperUnavailable| {
            assert_eq!(
                HelperProgram::in_extracted_package(&root).err(),
                Some(expected),
                "{what}"
            );
        };
        mode(&helper, 0o775);
        refused("a group-writable helper", HelperUnavailable::NotProtected);
        mode(&helper, 0o644);
        refused(
            "a helper that is not executable",
            HelperUnavailable::NotProtected,
        );
        mode(&helper, 0o755);
        mode(&root.join(BIN), 0o777);
        refused("a writable bin directory", HelperUnavailable::NotProtected);
        mode(&root.join(BIN), 0o755);
        fs::rename(&helper, root.join("helper-elsewhere")).unwrap();
        refused("no helper", HelperUnavailable::NotInstalled);
        symlink(root.join("helper-elsewhere"), &helper).unwrap();
        refused("a helper that is a link", HelperUnavailable::NotProtected);
        fs::remove_file(&helper).unwrap();
        fs::create_dir(&helper).unwrap();
        refused(
            "a helper that is a directory",
            HelperUnavailable::NotProtected,
        );
        fs::remove_dir(&helper).unwrap();
        fs::rename(root.join("helper-elsewhere"), &helper).unwrap();
        HelperProgram::in_extracted_package(&root).unwrap();
        // This process is not the installed application: production has no
        // helper and no toolchain here, whatever exists elsewhere.
        assert_eq!(
            HelperProgram::installed().err(),
            Some(HelperUnavailable::NotInstalled)
        );
        assert_eq!(
            VerifiedVerifierToolchain::installed().err(),
            Some(ToolchainError::Unavailable)
        );
        fs::remove_dir_all(&root).unwrap();
    }

    /// Every directory from `root` down to the toolchain is owned by the
    /// extracting user and writable by no one else, as production requires
    /// of root.
    #[cfg(feature = "development-toolchain")]
    fn protected_ancestors(root: &Path) {
        // SAFETY: getuid has no preconditions.
        let uid = unsafe { libc::getuid() };
        let mut dir = root.to_path_buf();
        for part in TOOLCHAIN.split('/') {
            dir = dir.join(part);
            let meta = fs::symlink_metadata(&dir).unwrap();
            assert!(meta.is_dir(), "{}", dir.display());
            assert_eq!(meta.uid(), uid, "{}", dir.display());
            assert_eq!(meta.mode() & 0o022, 0, "{}", dir.display());
        }
    }

    #[cfg(feature = "development-toolchain")]
    #[test]
    fn p2r1_the_packaged_toolchain_is_where_production_derives_it() {
        let Some(assembled) = nexus_verifier_sandbox::toolchain::is_packaged().then(|| {
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../app/src-tauri/verifier-toolchain")
                .canonicalize()
                .unwrap()
        }) else {
            assert!(
                std::env::var_os(EXTRACTED).is_none(),
                "{EXTRACTED} requires a build that embeds the packaged toolchain"
            );
            println!("(no packaged verifier toolchain in this build)");
            return;
        };
        // A synthetic package root holding the assembled tree (linked, so
        // nothing is copied or changed) at the installed path.
        let root = synthetic("toolchain");
        let tree = root.join(TOOLCHAIN);
        let mut pending = vec![(assembled.clone(), tree.clone())];
        while let Some((from, to)) = pending.pop() {
            fs::create_dir_all(&to).unwrap();
            mode(&to, 0o755);
            for entry in fs::read_dir(&from).unwrap() {
                let entry = entry.unwrap();
                let target = to.join(entry.file_name());
                if entry.file_type().unwrap().is_dir() {
                    pending.push((entry.path(), target));
                } else {
                    fs::hard_link(entry.path(), target).unwrap();
                }
            }
        }
        for dir in ["usr/lib", "usr/lib/NexusOS"] {
            mode(&root.join(dir), 0o755);
        }
        protected_ancestors(&root);
        let verified = VerifiedVerifierToolchain::development(&tree).unwrap();
        assert_eq!(verified.rust_version(), "1.94.0");
        // An extra or a missing file is refused, as in production.
        fs::write(tree.join("bin/extra"), b"x").unwrap();
        assert_eq!(
            VerifiedVerifierToolchain::development(&tree).err(),
            Some(ToolchainError::Unexpected)
        );
        fs::remove_file(tree.join("bin/extra")).unwrap();
        fs::remove_file(tree.join("bin/rustc")).unwrap();
        assert_eq!(
            VerifiedVerifierToolchain::development(&tree).err(),
            Some(ToolchainError::Missing)
        );
        fs::remove_dir_all(&root).unwrap();
    }

    #[cfg(feature = "development-toolchain")]
    #[test]
    fn p2r1_the_extracted_package_satisfies_the_installed_layout() {
        let Some(root) = std::env::var_os(EXTRACTED).map(PathBuf::from) else {
            println!("({EXTRACTED} not set: no extracted package to evaluate)");
            return;
        };
        assert!(
            nexus_verifier_sandbox::toolchain::is_packaged(),
            "{EXTRACTED} requires a build that embeds the packaged toolchain"
        );
        let root = root.canonicalize().unwrap();
        // The application and the helper, and nothing else, in usr/bin.
        let mut bin: Vec<String> = fs::read_dir(root.join(BIN))
            .unwrap()
            .map(|entry| entry.unwrap().file_name().into_string().unwrap())
            .collect();
        bin.sort();
        assert_eq!(bin.len(), 2, "{bin:?}");
        assert!(
            bin.contains(&"nexus-verifier-sandbox".to_string()),
            "{bin:?}"
        );
        let application = root.join(BIN).join(
            bin.iter()
                .find(|name| *name != "nexus-verifier-sandbox")
                .unwrap(),
        );
        let meta = fs::symlink_metadata(&application).unwrap();
        assert!(meta.is_file() && meta.mode() & 0o777 == 0o755);
        // The helper production would derive, under production's checks.
        let program = HelperProgram::in_extracted_package(&root).unwrap();
        assert_eq!(
            program.path(),
            root.join(BIN).join("nexus-verifier-sandbox")
        );
        // The toolchain production would derive: exactly this build's
        // embedded manifest, under protected ancestors.
        protected_ancestors(&root);
        let verified = VerifiedVerifierToolchain::development(&root.join(TOOLCHAIN)).unwrap();
        verified.reverify().unwrap();
        assert_eq!(verified.rust_version(), "1.94.0");
        println!(
            "extracted package verified: application {}, helper {}, toolchain digest {}",
            application.display(),
            program.path().display(),
            hex(&verified.digest())
        );
    }

    #[cfg(feature = "development-toolchain")]
    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }
}
