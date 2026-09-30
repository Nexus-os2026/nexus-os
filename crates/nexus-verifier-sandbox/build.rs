//! Verifier sandbox build script.
//!
//! Phase Two P2F: embeds the packaged verifier toolchain manifest. Only when
//! release/CI assembly sets NEXUS_VERIFIER_TOOLCHAIN=packaged is the
//! assembled tree at `app/src-tauri/verifier-toolchain/` (the directory Tauri
//! bundles as the `verifier-toolchain` resource) validated and its exact
//! files rendered into the library; otherwise the manifest is absent and the
//! verifier toolchain stays unavailable. The library never reads a manifest
//! or the assembled tree at runtime.
#[path = "src/toolchain/contract.rs"]
#[allow(dead_code)]
mod contract;

use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::{Path, PathBuf};

const SWITCH: &str = "NEXUS_VERIFIER_TOOLCHAIN";
const ABSENT: &str = "const PACKAGED_MANIFEST: Option<Manifest<'static>> = None;\n";

fn main() {
    println!("cargo::rerun-if-env-changed={SWITCH}");
    println!("cargo::rerun-if-changed=src/toolchain/contract.rs");
    let source = match std::env::var(SWITCH) {
        Err(std::env::VarError::NotPresent) => ABSENT.to_owned(),
        Ok(value) if value == "packaged" => packaged(),
        _ => panic!("{SWITCH} must be unset or \"packaged\""),
    };
    let out = PathBuf::from(std::env::var_os("OUT_DIR").expect("OUT_DIR"))
        .join("verifier_toolchain_manifest.rs");
    // Rewritten only on change, so an unchanged manifest never forces a rebuild.
    if std::fs::read_to_string(&out).ok().as_deref() != Some(source.as_str()) {
        std::fs::write(&out, source).expect("write verifier toolchain manifest");
    }
}

struct File {
    path: String,
    size: u64,
    executable: bool,
    sha256: [u8; 32],
}

fn packaged() -> String {
    let cfg = |key: &str| std::env::var(key).unwrap_or_default();
    if cfg("CARGO_CFG_TARGET_OS") != "linux" || cfg("CARGO_CFG_TARGET_ARCH") != "x86_64" {
        panic!("the packaged verifier toolchain exists only for x86_64 Linux");
    }
    let root = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"))
        .join("../../app/src-tauri/verifier-toolchain");
    println!("cargo::rerun-if-changed={}", root.display());
    let mut files = Vec::new();
    walk(&root, "", &mut files);
    files.sort_by(|a, b| a.path.cmp(&b.path));
    assert!(
        !files.is_empty() && files.len() <= contract::MAX_FILES,
        "packaged verifier toolchain: file count out of bounds"
    );
    for (path, executable) in contract::REQUIRED {
        let file = files
            .iter()
            .find(|file| file.path == *path)
            .unwrap_or_else(|| panic!("packaged verifier toolchain lacks {path}"));
        assert_eq!(
            file.executable, *executable,
            "packaged verifier toolchain: wrong mode for {path}"
        );
    }
    let mut source =
        String::from("const PACKAGED_MANIFEST: Option<Manifest<'static>> = Some(Manifest {\n");
    source += &format!(
        "    schema: {},\n    rust_version: {:?},\n    host: {:?},\n    target: {:?},\n    files: &[\n",
        contract::SCHEMA,
        contract::RUST_VERSION,
        contract::HOST_TARGET,
        contract::VERIFIER_TARGET
    );
    for file in &files {
        source += &format!(
            "        ManifestFile {{ path: {:?}, size: {}, executable: {}, sha256: {:?} }},\n",
            file.path, file.size, file.executable, file.sha256
        );
    }
    source += "    ],\n});\n";
    source
}

/// Every regular file beneath `dir`; links, special files and empty
/// directories are refused.
fn walk(dir: &Path, prefix: &str, files: &mut Vec<File>) {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .unwrap_or_else(|error| panic!("packaged verifier toolchain unreadable: {error}"))
        .map(|entry| {
            entry
                .expect("directory entry")
                .file_name()
                .into_string()
                .expect("a UTF-8 name")
        })
        .collect();
    names.sort();
    assert!(
        !names.is_empty(),
        "packaged verifier toolchain: empty directory {prefix:?}"
    );
    for name in names {
        let path = if prefix.is_empty() {
            name.clone()
        } else {
            format!("{prefix}/{name}")
        };
        assert!(
            contract::valid_path(&path),
            "packaged verifier toolchain: path outside the manifest grammar: {path}"
        );
        let full = dir.join(&name);
        let metadata = std::fs::symlink_metadata(&full).expect("entry metadata");
        let kind = metadata.file_type();
        if kind.is_dir() {
            walk(&full, &path, files);
        } else if kind.is_file() {
            assert!(
                metadata.len() <= contract::MAX_FILE_BYTES,
                "packaged verifier toolchain: {path} too large"
            );
            let mut hasher = Sha256::new();
            let mut reader = std::fs::File::open(&full).expect("open toolchain file");
            let mut buffer = vec![0u8; 1 << 20];
            let mut size = 0u64;
            loop {
                let read = reader.read(&mut buffer).expect("read toolchain file");
                if read == 0 {
                    break;
                }
                size += read as u64;
                hasher.update(&buffer[..read]);
            }
            assert_eq!(size, metadata.len(), "{path} changed while hashing");
            files.push(File {
                path,
                size,
                executable: executable(&metadata),
                sha256: hasher.finalize().into(),
            });
        } else {
            panic!("packaged verifier toolchain: unsupported entry {path}");
        }
    }
}

#[cfg(unix)]
fn executable(metadata: &std::fs::Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt;
    metadata.permissions().mode() & 0o100 != 0
}

#[cfg(not(unix))]
fn executable(_: &std::fs::Metadata) -> bool {
    panic!("the packaged verifier toolchain is assembled on Unix hosts only")
}
