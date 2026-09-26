//! P0-002C4D2 production verification tests: root derivation from the
//! installed executable layout, the packaged verification path and its
//! backend-only accessors (synthetic installed layouts), the build/bundle
//! contract and, in packaged builds only, the real assembled toolchain.
use super::*;

// ── Fixtures ──────────────────────────────────────────────────────────────

fn at(root: &Path, relative: &str) -> PathBuf {
    relative
        .split('/')
        .fold(root.to_path_buf(), |path, part| path.join(part))
}

/// A synthetic installed layout for this OS. Windows and macOS layouts
/// include the executable; Linux derives only from `/usr/bin`, so its
/// synthetic layout exercises the root-level packaged verification.
struct Layout {
    base: PathBuf,
    executable: Option<PathBuf>,
    root: PathBuf,
}

impl Layout {
    fn new() -> Self {
        let dir = std::env::temp_dir().join(format!("nexus-c4d2-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&dir).unwrap();
        let base = dir.canonicalize().unwrap();
        let (executable, root) = if cfg!(windows) {
            let app = base.join("NexusOS");
            (Some(app.join("NexusOS.exe")), app.join("toolchain"))
        } else if cfg!(target_os = "macos") {
            let contents = base.join("NexusOS.app").join("Contents");
            (
                Some(contents.join("MacOS").join("NexusOS")),
                contents.join("Resources").join("toolchain"),
            )
        } else {
            (None, base.join("toolchain"))
        };
        if let Some(executable) = &executable {
            std::fs::create_dir_all(executable.parent().unwrap()).unwrap();
            std::fs::write(executable, b"synthetic Nexus executable").unwrap();
        }
        std::fs::create_dir_all(&root).unwrap();
        Self {
            base,
            executable,
            root,
        }
    }

    fn with(files: &[(&str, &[u8])]) -> Self {
        let layout = Self::new();
        for (path, content) in files {
            let target = at(&layout.root, path);
            std::fs::create_dir_all(target.parent().unwrap()).unwrap();
            std::fs::write(target, content).unwrap();
        }
        layout
    }

    fn verify(
        &self,
        manifest: &ToolchainManifest<'_>,
    ) -> Result<VerifiedToolchain, ToolchainError> {
        match &self.executable {
            Some(executable) => verify_packaged(manifest, executable),
            None => verify_packaged_root(manifest, &self.root),
        }
    }
}

impl Drop for Layout {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.base);
    }
}

const ADDON: &str = "node_modules/native/binding.node";

fn synthetic() -> Vec<(&'static str, &'static [u8])> {
    let mut files: Vec<(&'static str, &'static [u8])> = vec![
        (ENTRY_MODULE, b"export {};\n"),
        (NODE_EXECUTABLE, b"synthetic node runtime"),
        ("node/LICENSE", b"license"),
        (ADDON, b"synthetic native addon"),
        ("node_modules/react/index.js", b"module.exports = {};\n"),
    ];
    files.sort();
    files
}

fn entries<'a>(files: &[(&'a str, &[u8])]) -> Vec<ManifestFile<'a>> {
    files
        .iter()
        .map(|(path, content)| ManifestFile {
            path,
            size: content.len() as u64,
            sha256: Sha256::digest(content).into(),
        })
        .collect()
}

fn manifest<'a>(files: &'a [ManifestFile<'a>]) -> ToolchainManifest<'a> {
    ToolchainManifest {
        schema: SCHEMA,
        target: CURRENT_TARGET,
        files,
    }
}

fn rejected(layout: &Layout, manifest: &ToolchainManifest<'_>) -> ToolchainError {
    match layout.verify(manifest) {
        Ok(_) => panic!("packaged verification must fail"),
        Err(error) => error,
    }
}

// Same-size replacement: only the digest can reveal it.
fn swap_bytes(path: &Path) -> Vec<u8> {
    let original = std::fs::read(path).unwrap();
    let mut changed = original.clone();
    let last = changed.len() - 1;
    changed[last] ^= 0x01;
    std::fs::write(path, changed).unwrap();
    original
}

fn code(source: &str) -> String {
    source
        .replace("\r\n", "\n")
        .lines()
        .map(|line| line.split("//").next().unwrap_or(""))
        .collect::<Vec<_>>()
        .join("\n")
}

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
    panic!("unterminated function");
}

fn repository(relative: &str) -> String {
    let path = at(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap(),
        relative,
    );
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("{relative}: {e}"))
        .replace("\r\n", "\n")
}

// ── Root derivation ───────────────────────────────────────────────────────

#[test]
fn p0_002c4d2_installed_root_is_derived_only_from_the_executable_layout() {
    #[cfg(target_os = "linux")]
    {
        for executable in ["/usr/bin/NexusOS", "/usr/bin/nexus-desktop-backend"] {
            assert_eq!(
                installed_root(Path::new(executable)),
                Some(PathBuf::from("/usr/lib/NexusOS/toolchain")),
                "{executable}"
            );
        }
        for unsupported in [
            "/usr/local/bin/NexusOS",
            "/opt/NexusOS/NexusOS",
            "/tmp/.mount_nexus/usr/bin/NexusOS",
            "/home/user/usr/bin/NexusOS",
            "/usr/bin",
            "/",
            "usr/bin/NexusOS",
            "NexusOS",
        ] {
            assert_eq!(
                installed_root(Path::new(unsupported)),
                None,
                "{unsupported}"
            );
        }
    }
    #[cfg(target_os = "macos")]
    {
        assert_eq!(
            installed_root(Path::new(
                "/Applications/NexusOS.app/Contents/MacOS/NexusOS"
            )),
            Some(PathBuf::from(
                "/Applications/NexusOS.app/Contents/Resources/toolchain"
            ))
        );
        for unsupported in [
            "/Applications/NexusOS.app/Contents/Resources/NexusOS",
            "/Applications/NexusOS/Contents/MacOS/NexusOS",
            "/Applications/NexusOS.app/MacOS/NexusOS",
            "/usr/local/bin/NexusOS",
            "NexusOS.app/Contents/MacOS/NexusOS",
        ] {
            assert_eq!(
                installed_root(Path::new(unsupported)),
                None,
                "{unsupported}"
            );
        }
    }
    #[cfg(windows)]
    {
        assert_eq!(
            installed_root(Path::new(r"C:\Program Files\NexusOS\NexusOS.exe")),
            Some(PathBuf::from(r"C:\Program Files\NexusOS\toolchain"))
        );
        assert_eq!(
            installed_root(Path::new(r"\\?\C:\Program Files\NexusOS\NexusOS.exe")),
            Some(PathBuf::from(r"\\?\C:\Program Files\NexusOS\toolchain"))
        );
        for unsupported in [r"NexusOS.exe", r"NexusOS\NexusOS.exe", r"\NexusOS.exe"] {
            assert_eq!(
                installed_root(Path::new(unsupported)),
                None,
                "{unsupported}"
            );
        }
    }
}

#[test]
fn p0_002c4d2_production_root_uses_only_the_executable_path() {
    let module = code(include_str!("../trusted_toolchain.rs"));
    // The single executable-path query, in the single production helper.
    assert_eq!(module.matches("current_exe").count(), 1);
    let executable = function(&module, "fn installed_executable(");
    assert!(executable.contains("std::env::current_exe()"));
    assert!(executable.contains("is_absolute()"));
    assert!(executable.contains("canonicalize()"));
    let installed = function(&module, "pub(super) fn verify_installed(");
    assert!(installed.contains("installed_executable()?"));
    assert!(installed.contains("verify_packaged("));
    for body in [
        executable,
        installed,
        function(&module, "fn installed_root("),
        function(&module, "fn verify_packaged("),
        function(&module, "fn verify_packaged_root("),
    ] {
        for forbidden in [
            "env::var",
            "var_os",
            "vars(",
            "current_dir",
            "home_dir",
            "temp_dir",
            "APPDIR",
            "resource_dir",
            "PATH",
            "HOME",
            "which",
            "Command",
        ] {
            assert!(!body.contains(forbidden), "{forbidden} in:\n{body}");
        }
    }
}

// ── Packaged verification (synthetic installed layouts) ───────────────────

#[test]
fn p0_002c4d2_packaged_layout_verifies_with_backend_only_paths() {
    let files = synthetic();
    let layout = Layout::with(&files);
    let listing = entries(&files);
    let verified = layout
        .verify(&manifest(&listing))
        .expect("packaged layout verifies");
    assert_eq!(verified.root(), layout.root);
    assert_eq!(
        verified.node_executable(),
        at(&layout.root, NODE_EXECUTABLE)
    );
    assert_eq!(verified.entry_module(), at(&layout.root, ENTRY_MODULE));
    assert!(verified.node_executable().is_file());
    assert!(verified.entry_module().is_file());
    assert!(verified.root_still_pinned());
}

#[test]
fn p0_002c4d2_packaged_layout_rejects_changed_missing_extra_redirected_and_foreign() {
    let files = synthetic();
    let listing = entries(&files);
    let valid = manifest(&listing);

    for (changed, expected) in [
        (ENTRY_MODULE, ToolchainError::DigestMismatch),
        (NODE_EXECUTABLE, ToolchainError::DigestMismatch),
        (ADDON, ToolchainError::DigestMismatch),
    ] {
        let layout = Layout::with(&files);
        swap_bytes(&at(&layout.root, changed));
        assert_eq!(rejected(&layout, &valid), expected, "{changed}");
    }

    let layout = Layout::with(&files);
    std::fs::write(at(&layout.root, NODE_EXECUTABLE), b"different length node").unwrap();
    assert_eq!(rejected(&layout, &valid), ToolchainError::SizeMismatch);

    let layout = Layout::with(&files);
    std::fs::remove_file(at(&layout.root, ADDON)).unwrap();
    assert_eq!(rejected(&layout, &valid), ToolchainError::Missing);

    let layout = Layout::with(&files);
    std::fs::write(at(&layout.root, "node_modules/react/extra.js"), b"x").unwrap();
    assert_eq!(rejected(&layout, &valid), ToolchainError::Unexpected);

    let layout = Layout::with(&files);
    let entry = at(&layout.root, ENTRY_MODULE);
    let elsewhere = layout.base.join("elsewhere.mjs");
    std::fs::write(&elsewhere, b"export {};\n").unwrap();
    std::fs::remove_file(&entry).unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(&elsewhere, &entry).unwrap();
    #[cfg(windows)]
    std::os::windows::fs::symlink_file(&elsewhere, &entry)
        .expect("native Windows test requires symlink creation privilege");
    assert_eq!(rejected(&layout, &valid), ToolchainError::Redirected);

    // The packaged manifest must contain the Node executable and the entry.
    for required in [NODE_EXECUTABLE, ENTRY_MODULE] {
        let partial: Vec<_> = files
            .iter()
            .copied()
            .filter(|(p, _)| *p != required)
            .collect();
        let layout = Layout::with(&partial);
        let listing = entries(&partial);
        assert_eq!(
            rejected(&layout, &manifest(&listing)),
            ToolchainError::InvalidManifest,
            "{required}"
        );
    }

    let layout = Layout::with(&files);
    let foreign = ToolchainManifest {
        target: ToolchainTarget {
            arch: if CURRENT_TARGET.arch == "x86_64" {
                "aarch64"
            } else {
                "x86_64"
            },
            ..CURRENT_TARGET
        },
        ..manifest(&listing)
    };
    assert_eq!(
        rejected(&layout, &foreign),
        ToolchainError::PlatformMismatch
    );

    // No installed toolchain at the derived root.
    let layout = Layout::with(&files);
    std::fs::remove_dir_all(&layout.root).unwrap();
    assert_eq!(rejected(&layout, &valid), ToolchainError::RootRejected);

    // An executable outside the supported layout never yields a root.
    let stray = Layout::with(&files);
    let unsupported = if cfg!(windows) {
        PathBuf::from("NexusOS.exe")
    } else {
        stray.base.join("NexusOS")
    };
    assert!(matches!(
        verify_packaged(&valid, &unsupported),
        Err(ToolchainError::Unavailable)
    ));
}

#[cfg(not(nexus_packaged_toolchain))]
#[test]
fn p0_002c4d2_builds_without_a_packaged_toolchain_stay_unavailable() {
    // Development and test builds embed no manifest: no system Node, PATH,
    // repository node_modules, npm or HOME fallback exists.
    assert!(std::hint::black_box(PRODUCTION_MANIFEST).is_none());
    assert!(matches!(
        verify_installed(),
        Err(ToolchainError::Unavailable)
    ));
}

// ── Build, bundle and runtime contracts ────────────────────────────────────

#[test]
fn p0_002c4d2_verified_paths_reach_no_launch_or_frontend() {
    // Nothing outside this module can obtain or use the verified paths yet;
    // production START and static build stay unavailable (C4A/C4D0 tests).
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let own = src.join("builder_workspace").join("trusted_toolchain");
    let mut pending = vec![src.clone()];
    let mut checked = 0;
    while let Some(dir) = pending.pop() {
        for entry in std::fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                if path != own {
                    pending.push(path);
                }
                continue;
            }
            if path.extension().is_none_or(|e| e != "rs")
                || path == src.join("builder_workspace").join("trusted_toolchain.rs")
            {
                continue;
            }
            checked += 1;
            let text = code(&std::fs::read_to_string(&path).unwrap());
            for forbidden in [
                "verify_installed",
                "VerifiedToolchain",
                "node_executable",
                "entry_module",
                "builder-toolchain",
            ] {
                assert!(!text.contains(forbidden), "{forbidden} in {path:?}");
            }
        }
    }
    assert!(checked > 10);
}

#[test]
fn p0_002c4d2_tailwind_theme_matches_the_generator_mapping() {
    // Nexus owns the fixed theme mapping the generator's (untrusted, never
    // loaded) tailwind.config.ts expresses; the two must stay identical.
    fn pairs(text: &str) -> Vec<(String, String)> {
        let mut out = Vec::new();
        let mut section = String::new();
        for line in text.lines().map(str::trim) {
            if let Some(name) = line.strip_suffix(": {") {
                section = name.to_owned();
            } else if let Some((key, value)) = line.split_once(": '") {
                if let Some(value) = value.strip_suffix("',") {
                    if value.starts_with("var(--") || key == "darkMode" {
                        out.push((
                            format!("{section}.{}", key.trim_matches('\'')),
                            value.to_owned(),
                        ));
                    }
                }
            }
        }
        out.sort();
        out
    }
    let generated = web_builder_agent::token_tailwind::token_set_to_tailwind_config(
        &web_builder_agent::tokens::TokenSet::default(),
    );
    let trusted = repository("packaging/builder-toolchain/entry/tailwind-theme.mjs");
    let mut expected = pairs(&generated);
    // The generator's darkMode sits at the config top level; Nexus sets it in
    // the returned config object.
    expected.retain(|(key, _)| !key.ends_with("darkMode"));
    let mut actual = pairs(&trusted);
    actual.retain(|(key, _)| !key.ends_with("darkMode"));
    assert!(expected.len() >= 50, "{expected:?}");
    assert_eq!(actual, expected);
    assert!(generated.contains("darkMode: 'media',"));
    assert!(trusted.contains("darkMode: 'media',"));
    assert!(generated.contains("plugins: [],") && trusted.contains("plugins: [],"));
}

#[test]
fn p0_002c4d2_bundle_configuration_packages_the_toolchain_resource() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let base: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(manifest_dir.join("tauri.conf.json")).unwrap(),
    )
    .unwrap();
    // The Linux resource directory is named after the product.
    assert_eq!(base["productName"], PRODUCT_NAME);
    // Development builds bundle no toolchain; release merges this config.
    assert_eq!(base["bundle"]["resources"], serde_json::json!([]));
    let release: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(manifest_dir.join("tauri.builder-toolchain.conf.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(
        release,
        serde_json::json!({ "bundle": { "resources": { "builder-toolchain": TOOLCHAIN_DIR } } })
    );
    // Release assembly precedes every bundle, which embeds its manifest.
    let workflow = repository(".github/workflows/release.yml");
    for target in [
        "x86_64-pc-windows-msvc",
        "x86_64-unknown-linux-gnu",
        "aarch64-apple-darwin",
    ] {
        assert!(
            workflow.contains(&format!(
                "packaging/builder-toolchain/scripts/assemble.mjs --target {target} --out app/src-tauri/builder-toolchain"
            )),
            "{target}"
        );
    }
    assert_eq!(
        workflow
            .matches("NEXUS_BUILDER_TOOLCHAIN: packaged")
            .count(),
        3
    );
    assert_eq!(
        workflow
            .matches("--config src-tauri/tauri.builder-toolchain.conf.json")
            .count(),
        4 // Windows NSIS, Windows MSI fallback, Debian, macOS
    );
}

// ── The real assembled toolchain (packaged builds only) ────────────────────

#[cfg(nexus_packaged_toolchain)]
fn assembled_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("builder-toolchain")
        .canonicalize()
        .unwrap()
}

#[cfg(nexus_packaged_toolchain)]
fn copy_tree(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), target).unwrap();
        }
    }
}

#[cfg(nexus_packaged_toolchain)]
#[test]
fn p0_002c4d2_assembled_toolchain_embeds_its_exact_manifest() {
    let manifest = PRODUCTION_MANIFEST.expect("a packaged build embeds its manifest");
    assert_eq!(validate_manifest(manifest, &CURRENT_TARGET), Ok(()));
    let paths: Vec<&str> = manifest.files.iter().map(|file| file.path).collect();
    assert!(paths.contains(&NODE_EXECUTABLE) && paths.contains(&ENTRY_MODULE));
    assert!(paths.iter().all(|path| !path
        .split('/')
        .any(|c| matches!(c, "npm" | "npx" | "corepack"))));
    let marker = if cfg!(windows) {
        "win32-x64-msvc"
    } else if cfg!(target_os = "macos") {
        "darwin-arm64"
    } else {
        "linux-x64-gnu"
    };
    let natives: Vec<_> = paths.iter().filter(|p| p.ends_with(".node")).collect();
    assert!(natives.len() >= 2, "{natives:?}");
    for native in natives {
        assert!(
            native.contains(marker)
                || (cfg!(target_os = "macos") && native.ends_with("/fsevents.node")),
            "{native}"
        );
    }
    // The embedded manifest verifies exactly the assembled bytes.
    let root = assembled_root();
    let verified = verify_packaged_root(manifest, &root).expect("assembled toolchain verifies");
    assert!(verified.node_executable().is_file());
    assert!(verified.entry_module().is_file());
    // This test executable is not an installed layout: production
    // verification derives no usable root from it and never verifies.
    assert!(matches!(
        verify_installed(),
        Err(ToolchainError::Unavailable | ToolchainError::RootRejected)
    ));
}

#[cfg(nexus_packaged_toolchain)]
#[test]
fn p0_002c4d2_assembled_toolchain_verifies_installed_and_rejects_tampering() {
    let manifest = PRODUCTION_MANIFEST.expect("a packaged build embeds its manifest");
    let layout = Layout::new();
    std::fs::remove_dir(&layout.root).unwrap();
    copy_tree(&assembled_root(), &layout.root);
    let verified = layout.verify(manifest).expect("installed copy verifies");
    assert_eq!(
        verified.node_executable(),
        at(&layout.root, NODE_EXECUTABLE)
    );
    assert_eq!(verified.entry_module(), at(&layout.root, ENTRY_MODULE));
    drop(verified);

    let native = manifest
        .files
        .iter()
        .find(|file| file.path.ends_with(".node"))
        .expect("a native addon")
        .path;
    for changed in [NODE_EXECUTABLE, ENTRY_MODULE, native] {
        let path = at(&layout.root, changed);
        let original = swap_bytes(&path);
        assert!(
            matches!(layout.verify(manifest), Err(ToolchainError::DigestMismatch)),
            "{changed}"
        );
        std::fs::write(&path, original).unwrap();
    }
    let extra = at(&layout.root, "node_modules/unexpected.js");
    std::fs::write(&extra, b"x").unwrap();
    assert!(matches!(
        layout.verify(manifest),
        Err(ToolchainError::Unexpected)
    ));
    std::fs::remove_file(&extra).unwrap();
    let entry = at(&layout.root, ENTRY_MODULE);
    let original = std::fs::read(&entry).unwrap();
    std::fs::remove_file(&entry).unwrap();
    assert!(matches!(
        layout.verify(manifest),
        Err(ToolchainError::Missing)
    ));
    std::fs::write(&entry, original).unwrap();
    assert!(layout.verify(manifest).is_ok());
}
