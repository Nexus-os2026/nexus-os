//! P0-002C4D2 build-time manifest generation tests: synthetic assembled
//! trees with synthetic native headers for every release target.
use super::super::{verify_tree, ManifestFile, ToolchainError, ToolchainManifest, CURRENT_TARGET};
use super::*;
use std::fs;
use std::path::PathBuf;

const TARGETS: [Target; 3] = [
    Target::LinuxX64Gnu,
    Target::WindowsX64Msvc,
    Target::MacosArm64,
];
const EM_AARCH64: u16 = 183;
const IMAGE_FILE_MACHINE_ARM64: u16 = 0xaa64;
const CPU_TYPE_X86_64: u32 = 0x0100_0007;

struct Tree {
    root: PathBuf,
}

impl Tree {
    fn new(files: &[(&str, Vec<u8>)]) -> Self {
        let dir = std::env::temp_dir().join(format!("nexus-c4d2-pkg-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&dir).unwrap();
        let root = dir.canonicalize().unwrap();
        for (path, bytes) in files {
            let target = at(&root, path);
            fs::create_dir_all(target.parent().unwrap()).unwrap();
            fs::write(target, bytes).unwrap();
        }
        Self { root }
    }
}

impl Drop for Tree {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn at(root: &Path, relative: &str) -> PathBuf {
    relative
        .split('/')
        .fold(root.to_path_buf(), |path, part| path.join(part))
}

fn elf(machine: u16, kind: u16) -> Vec<u8> {
    let mut h = vec![0u8; 64];
    h[..4].copy_from_slice(&[0x7f, b'E', b'L', b'F']);
    h[4] = 2;
    h[5] = 1;
    h[16..18].copy_from_slice(&kind.to_le_bytes());
    h[18..20].copy_from_slice(&machine.to_le_bytes());
    h
}

fn pe(machine: u16, dll: bool) -> Vec<u8> {
    let mut h = vec![0u8; 0x200];
    h[..2].copy_from_slice(b"MZ");
    h[0x3c..0x40].copy_from_slice(&0x80u32.to_le_bytes());
    h[0x80..0x84].copy_from_slice(b"PE\0\0");
    h[0x84..0x86].copy_from_slice(&machine.to_le_bytes());
    let characteristics: u16 = 0x0022 | if dll { 0x2000 } else { 0 };
    h[0x96..0x98].copy_from_slice(&characteristics.to_le_bytes());
    h[0x98..0x9a].copy_from_slice(&0x20bu16.to_le_bytes());
    h
}

fn macho(cpu: u32, kind: u32) -> Vec<u8> {
    let mut h = vec![0u8; 32];
    h[..4].copy_from_slice(&[0xcf, 0xfa, 0xed, 0xfe]);
    h[4..8].copy_from_slice(&cpu.to_le_bytes());
    h[12..16].copy_from_slice(&kind.to_le_bytes());
    h
}

fn universal(cpus: &[u32]) -> Vec<u8> {
    let mut h = vec![0xca, 0xfe, 0xba, 0xbe];
    h.extend_from_slice(&(cpus.len() as u32).to_be_bytes());
    for cpu in cpus {
        let mut arch = [0u8; 20];
        arch[..4].copy_from_slice(&cpu.to_be_bytes());
        h.extend_from_slice(&arch);
    }
    h
}

// A well-formed assembled closure for `target`.
fn closure(target: Target) -> Vec<(&'static str, Vec<u8>)> {
    let (node, executable, addon) = match target {
        Target::LinuxX64Gnu => ("node/node", elf(EM_X86_64, ET_EXEC), elf(EM_X86_64, ET_DYN)),
        Target::WindowsX64Msvc => (
            "node/node.exe",
            pe(IMAGE_FILE_MACHINE_AMD64, false),
            pe(IMAGE_FILE_MACHINE_AMD64, true),
        ),
        Target::MacosArm64 => (
            "node/node",
            macho(CPU_TYPE_ARM64, MH_EXECUTE),
            macho(CPU_TYPE_ARM64, MH_BUNDLE),
        ),
    };
    vec![
        (node, executable),
        ("node/LICENSE", b"license".to_vec()),
        ("entry/nexus-builder.mjs", b"export {};\n".to_vec()),
        (
            "node_modules/react/index.js",
            b"module.exports = {};\n".to_vec(),
        ),
        ("node_modules/@scope/pkg/a-b.js", b"1".to_vec()),
        ("node_modules/@scope/pkg/a/b.js", b"2".to_vec()),
        ("node_modules/native/binding.node", addon),
        (
            "node_modules/native/MZ.txt",
            b"MZ is only text here".to_vec(),
        ),
    ]
}

fn replaced(target: Target, path: &str, bytes: Vec<u8>) -> Vec<(&'static str, Vec<u8>)> {
    let mut files = closure(target);
    files.iter_mut().find(|(p, _)| *p == path).unwrap().1 = bytes;
    files
}

fn generation_error(target: Target, files: &[(&str, Vec<u8>)]) -> String {
    let tree = Tree::new(files);
    generate(&tree.root, target).expect_err("generation must be rejected")
}

fn compact(text: &str) -> String {
    text.split_whitespace().collect()
}

#[test]
fn p0_002c4d2_manifest_generation_is_exact_and_deterministic() {
    for target in TARGETS {
        let files = closure(target);
        let tree = Tree::new(&files);
        let mut expected: Vec<(String, u64, [u8; 32])> = files
            .iter()
            .map(|(path, bytes)| {
                (
                    (*path).to_owned(),
                    bytes.len() as u64,
                    Sha256::digest(bytes).into(),
                )
            })
            .collect();
        expected.sort();
        let collected: Vec<_> = collect(&tree.root)
            .unwrap()
            .into_iter()
            .map(|f| (f.path, f.size, f.sha256))
            .collect();
        assert_eq!(collected, expected, "{target:?}");
        // Byte order: "a-b.js" (0x2d) precedes the directory "a/" (0x2f).
        let order: Vec<_> = collected.iter().map(|f| f.0.as_str()).collect();
        let dash = order.iter().position(|p| p.ends_with("a-b.js")).unwrap();
        assert!(dash < order.iter().position(|p| p.ends_with("a/b.js")).unwrap());

        let source = generate(&tree.root, target).unwrap();
        assert_eq!(source, generate(&tree.root, target).unwrap(), "{target:?}");
        let (os, arch, env) = target.identity();
        assert!(source.contains("schema: 1,"));
        assert!(source.contains(&format!(
            "target: ToolchainTarget {{ os: {os:?}, arch: {arch:?}, env: {env:?} }},"
        )));
        assert_eq!(source.matches("ManifestFile {").count(), files.len());
        for (path, size, sha256) in &expected {
            let bytes: Vec<String> = sha256.iter().map(|b| format!("0x{b:02x}")).collect();
            assert!(source.contains(&format!(
                "ManifestFile {{ path: {path:?}, size: {size}, sha256: [{}] }},",
                bytes.join(", ")
            )));
        }
    }
}

#[test]
fn p0_002c4d2_generated_manifest_verifies_exactly_the_tree_it_describes() {
    let files = closure(Target::LinuxX64Gnu);
    let tree = Tree::new(&files);
    let collected = collect(&tree.root).unwrap();
    let entries: Vec<ManifestFile<'_>> = collected
        .iter()
        .map(|f| ManifestFile {
            path: &f.path,
            size: f.size,
            sha256: f.sha256,
        })
        .collect();
    let manifest = ToolchainManifest {
        schema: SCHEMA,
        target: CURRENT_TARGET,
        files: &entries,
    };
    assert!(verify_tree(&manifest, &tree.root).is_ok());
    // A same-size change after generation is caught by the verifier.
    fs::write(at(&tree.root, "node_modules/@scope/pkg/a-b.js"), b"9").unwrap();
    assert!(matches!(
        verify_tree(&manifest, &tree.root),
        Err(ToolchainError::DigestMismatch)
    ));
}

#[test]
fn p0_002c4d2_generation_rejects_links_special_entries_empty_directories_and_bad_names() {
    let target = Target::LinuxX64Gnu;
    let tree = Tree::new(&closure(target));
    fs::create_dir(at(&tree.root, "node_modules/empty")).unwrap();
    assert!(generate(&tree.root, target)
        .unwrap_err()
        .contains("empty directory"));

    let mut named = closure(target);
    named.push(("node_modules/react/a b.js", b"x".to_vec()));
    assert!(generation_error(target, &named).contains("grammar"));
    let mut long = closure(target);
    let deep: &'static str =
        Box::leak(format!("node_modules/{}", "a".repeat(200)).into_boxed_str());
    long.push((deep, b"x".to_vec()));
    assert!(generation_error(target, &long).contains("grammar"));

    let tree = Tree::new(&closure(target));
    let link = at(&tree.root, "node_modules/react/link.js");
    let destination = at(&tree.root, "node_modules/react/index.js");
    #[cfg(unix)]
    std::os::unix::fs::symlink(&destination, &link).unwrap();
    #[cfg(windows)]
    std::os::windows::fs::symlink_file(&destination, &link)
        .expect("native Windows test requires symlink creation privilege");
    assert!(generate(&tree.root, target)
        .unwrap_err()
        .contains("link or reparse point"));

    #[cfg(unix)]
    {
        #[cfg(target_os = "macos")]
        type Mode = u16;
        #[cfg(not(target_os = "macos"))]
        type Mode = u32;
        unsafe extern "C" {
            fn mkfifo(path: *const std::ffi::c_char, mode: Mode) -> std::ffi::c_int;
        }
        use std::os::unix::ffi::OsStrExt;
        let tree = Tree::new(&closure(target));
        let fifo = at(&tree.root, "node_modules/react/fifo");
        let fifo = std::ffi::CString::new(fifo.as_os_str().as_bytes()).unwrap();
        // SAFETY: NUL-terminated owned pathname, valid POSIX mode, no retained pointer.
        assert_eq!(unsafe { mkfifo(fifo.as_ptr(), 0o600) }, 0);
        assert!(generate(&tree.root, target)
            .unwrap_err()
            .contains("unsupported entry"));
        // A linked root is refused before traversal.
        let linked = tree.root.with_extension("link");
        std::os::unix::fs::symlink(&tree.root, &linked).unwrap();
        assert!(generate(&linked, target)
            .unwrap_err()
            .contains("plain directory"));
        let _ = fs::remove_file(&linked);
    }
}

#[test]
fn p0_002c4d2_generation_applies_the_verifier_contract() {
    // The build script and the verifier share one listing rule.
    assert!(valid_listing(&[("a-b.js", 1), ("a/b.js", 1)]));
    assert!(!valid_listing(&[]));
    assert!(!valid_listing(&[("a/B.js", 1), ("a/b.js", 1)]));
    assert!(!valid_listing(&[("A/x.js", 1), ("a/y.js", 1)]));
    assert!(!valid_listing(&[("a", 1), ("a/b", 1)]));
    assert!(!valid_listing(&[("b", 1), ("a", 1)]));
    assert!(!valid_listing(&[("con.js", 1)]));
    assert!(!valid_listing(&[(
        "x",
        super::super::contract::MAX_FILE_BYTES + 1
    )]));
    assert_eq!(SCHEMA, super::super::SCHEMA);
    assert_eq!(
        compact(ABSENT),
        compact("const PRODUCTION_MANIFEST: Option<&ToolchainManifest<'static>> = None;")
    );
}

#[test]
fn p0_002c4d2_generation_requires_node_and_entry_and_rejects_package_managers() {
    for target in TARGETS {
        for required in [target.node(), ENTRY_MODULE] {
            let files: Vec<_> = closure(target)
                .into_iter()
                .filter(|(path, _)| *path != required)
                .collect();
            assert!(
                generation_error(target, &files).contains("missing"),
                "{target:?} {required}"
            );
        }
        for manager in [
            "node_modules/npm/index.js",
            "node_modules/.bin/npx",
            "node/corepack",
        ] {
            let mut files = closure(target);
            files.push((manager, b"x".to_vec()));
            assert!(
                generation_error(target, &files).contains("package manager"),
                "{target:?} {manager}"
            );
        }
    }
    let mut musl = closure(Target::LinuxX64Gnu);
    musl.push((
        "node_modules/lightningcss-linux-x64-musl/index.js",
        b"x".to_vec(),
    ));
    assert!(generation_error(Target::LinuxX64Gnu, &musl).contains("musl"));
}

#[test]
fn p0_002c4d2_wrong_platform_closure_is_rejected() {
    let addon = "node_modules/native/binding.node";
    let foreign: Vec<(Target, &str, Vec<u8>)> = vec![
        // Linux x86_64 glibc: ELF x86-64 only; the executable an EXEC/DYN.
        (
            Target::LinuxX64Gnu,
            "node/node",
            pe(IMAGE_FILE_MACHINE_AMD64, false),
        ),
        (Target::LinuxX64Gnu, "node/node", elf(EM_AARCH64, ET_EXEC)),
        (Target::LinuxX64Gnu, addon, macho(CPU_TYPE_ARM64, MH_BUNDLE)),
        (Target::LinuxX64Gnu, addon, elf(EM_AARCH64, ET_DYN)),
        (Target::LinuxX64Gnu, addon, elf(EM_X86_64, ET_EXEC)),
        (Target::LinuxX64Gnu, addon, b"not a binary".to_vec()),
        (Target::LinuxX64Gnu, addon, {
            let mut h = elf(EM_X86_64, ET_DYN);
            h[4] = 1; // 32-bit
            h
        }),
        // Windows x86_64: PE32+ AMD64; the executable not a DLL, addons DLLs.
        (
            Target::WindowsX64Msvc,
            "node/node.exe",
            pe(IMAGE_FILE_MACHINE_AMD64, true),
        ),
        (
            Target::WindowsX64Msvc,
            "node/node.exe",
            pe(IMAGE_FILE_MACHINE_ARM64, false),
        ),
        (
            Target::WindowsX64Msvc,
            "node/node.exe",
            elf(EM_X86_64, ET_EXEC),
        ),
        (
            Target::WindowsX64Msvc,
            addon,
            pe(IMAGE_FILE_MACHINE_AMD64, false),
        ),
        (
            Target::WindowsX64Msvc,
            addon,
            pe(IMAGE_FILE_MACHINE_ARM64, true),
        ),
        // macOS arm64: thin arm64 executable; arm64 bundles or universal addons.
        (
            Target::MacosArm64,
            "node/node",
            macho(CPU_TYPE_X86_64, MH_EXECUTE),
        ),
        (
            Target::MacosArm64,
            "node/node",
            universal(&[CPU_TYPE_X86_64, CPU_TYPE_ARM64]),
        ),
        (
            Target::MacosArm64,
            "node/node",
            macho(CPU_TYPE_ARM64, MH_BUNDLE),
        ),
        (Target::MacosArm64, addon, universal(&[CPU_TYPE_X86_64])),
        (Target::MacosArm64, addon, macho(CPU_TYPE_X86_64, MH_BUNDLE)),
        (Target::MacosArm64, addon, elf(EM_X86_64, ET_DYN)),
    ];
    for (target, path, bytes) in foreign {
        let error = generation_error(target, &replaced(target, path, bytes));
        assert!(
            error.contains("not built for the target"),
            "{target:?} {path}: {error}"
        );
    }
    // Universal addons containing the arm64 slice are accepted on macOS.
    let fat = replaced(
        Target::MacosArm64,
        addon,
        universal(&[CPU_TYPE_X86_64, CPU_TYPE_ARM64]),
    );
    let tree = Tree::new(&fat);
    assert!(generate(&tree.root, Target::MacosArm64).is_ok());
    // A closure is only ever valid for the target it was built for.
    for built in TARGETS {
        let tree = Tree::new(&closure(built));
        for target in TARGETS {
            assert_eq!(
                generate(&tree.root, target).is_ok(),
                built == target,
                "{built:?} closure for {target:?}"
            );
        }
    }
}

// Correctly targeted native binaries at paths that are neither the exact
// packaged Node executable nor a `.node` addon beneath `node_modules/`.
fn stray_natives(target: Target) -> Vec<(&'static str, Vec<u8>)> {
    match target {
        Target::LinuxX64Gnu => vec![
            ("node_modules/tool/libevil.so", elf(EM_X86_64, ET_DYN)),
            ("node_modules/tool/bin/tool", elf(EM_X86_64, ET_EXEC)),
            ("node/node2", elf(EM_X86_64, ET_EXEC)),
            ("entry/native.node", elf(EM_X86_64, ET_DYN)),
            ("node_modules/tool/libold.so", {
                let mut h = elf(EM_X86_64, ET_DYN);
                h[4] = 1; // 32-bit: still a native binary
                h
            }),
        ],
        Target::WindowsX64Msvc => vec![
            (
                "node_modules/tool/evil.dll",
                pe(IMAGE_FILE_MACHINE_AMD64, true),
            ),
            (
                "node_modules/tool/tool.exe",
                pe(IMAGE_FILE_MACHINE_AMD64, false),
            ),
            ("node/node", pe(IMAGE_FILE_MACHINE_AMD64, false)),
            ("entry/native.node", pe(IMAGE_FILE_MACHINE_AMD64, true)),
        ],
        Target::MacosArm64 => vec![
            (
                "node_modules/tool/libevil.dylib",
                macho(CPU_TYPE_ARM64, MH_DYLIB),
            ),
            (
                "node_modules/tool/libuniversal.dylib",
                universal(&[CPU_TYPE_X86_64, CPU_TYPE_ARM64]),
            ),
            (
                "node_modules/tool/bin/tool",
                macho(CPU_TYPE_ARM64, MH_EXECUTE),
            ),
            ("entry/native.node", macho(CPU_TYPE_ARM64, MH_BUNDLE)),
        ],
    }
}

#[test]
fn p0_002c4d2_native_binaries_only_as_the_node_executable_or_node_modules_addons() {
    // A matching architecture never makes an arbitrary native path trusted.
    for target in TARGETS {
        for (path, bytes) in stray_natives(target) {
            let mut files = closure(target);
            files.push((path, bytes));
            let error = generation_error(target, &files);
            assert_eq!(
                error,
                format!("native binary at an unexpected path: {path}"),
                "{target:?}"
            );
        }
    }
}

#[test]
fn p0_002c4d2_node_executable_node_addons_and_ordinary_files_are_accepted() {
    for target in TARGETS {
        let addon = closure(target)
            .into_iter()
            .find(|(path, _)| *path == "node_modules/native/binding.node")
            .unwrap()
            .1;
        let mut files = closure(target);
        files.extend([
            ("node_modules/a/node_modules/b/binding.node", addon),
            (
                "node_modules/tool/index.js",
                b"module.exports = 1;\n".to_vec(),
            ),
            (
                "node_modules/tool/style.css",
                b"body { color: red; }\n".to_vec(),
            ),
            ("node_modules/tool/package.json", b"{}".to_vec()),
            ("node_modules/tool/data.wasm", b"\0asm\x01\0\0\0".to_vec()),
        ]);
        let tree = Tree::new(&files);
        let source = generate(&tree.root, target).expect("valid closure");
        assert_eq!(source.matches("ManifestFile {").count(), files.len());
    }
    // A `.node` addon beneath node_modules must itself be a target addon.
    let text = replaced(
        Target::LinuxX64Gnu,
        "node_modules/native/binding.node",
        b"not a binary".to_vec(),
    );
    assert!(generation_error(Target::LinuxX64Gnu, &text).contains("not built for the target"));
}

#[test]
fn p0_002c4d2_release_targets_match_the_verifier_identity() {
    assert_eq!(
        Target::from_cargo("linux", "x86_64", "gnu"),
        Some(Target::LinuxX64Gnu)
    );
    assert_eq!(
        Target::from_cargo("windows", "x86_64", "msvc"),
        Some(Target::WindowsX64Msvc)
    );
    assert_eq!(
        Target::from_cargo("macos", "aarch64", ""),
        Some(Target::MacosArm64)
    );
    for (os, arch, env) in [
        ("linux", "x86_64", "musl"),
        ("linux", "aarch64", "gnu"),
        ("macos", "x86_64", ""),
        ("windows", "aarch64", "msvc"),
        ("windows", "x86_64", "gnu"),
        ("freebsd", "x86_64", ""),
    ] {
        assert_eq!(Target::from_cargo(os, arch, env), None, "{os} {arch} {env}");
    }
    // The generated target identity is exactly the verifier's for this build.
    let env = if cfg!(target_env = "gnu") {
        "gnu"
    } else if cfg!(target_env = "msvc") {
        "msvc"
    } else {
        ""
    };
    if let Some(target) = Target::from_cargo(std::env::consts::OS, std::env::consts::ARCH, env) {
        let (os, arch, env) = target.identity();
        assert_eq!(
            (os, arch, env),
            (CURRENT_TARGET.os, CURRENT_TARGET.arch, CURRENT_TARGET.env)
        );
        assert_eq!(target.node(), super::super::NODE_EXECUTABLE);
    }
}
