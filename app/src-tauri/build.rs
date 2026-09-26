//! Desktop backend build script.
//!
//! P0-002C4D2: embeds the packaged Builder toolchain manifest. Only when
//! release/CI assembly sets NEXUS_BUILDER_TOOLCHAIN=packaged is the assembled
//! tree at `builder-toolchain/` (the directory Tauri bundles as the
//! `toolchain` resource) validated for this target and its exact bytes
//! rendered into the executable; otherwise the manifest is absent and the
//! trusted toolchain stays unavailable. The binary never reads either.
#[path = "src/builder_workspace/trusted_toolchain/contract.rs"]
mod contract;
#[path = "src/builder_workspace/trusted_toolchain/packaging.rs"]
mod packaging;

use std::path::PathBuf;

const SWITCH: &str = "NEXUS_BUILDER_TOOLCHAIN";

fn main() {
    println!("cargo::rerun-if-env-changed={SWITCH}");
    println!("cargo::rustc-check-cfg=cfg(nexus_packaged_toolchain)");
    let source = match std::env::var(SWITCH) {
        Err(std::env::VarError::NotPresent) => packaging::ABSENT.to_owned(),
        Ok(value) if value == "packaged" => packaged(),
        _ => panic!("{SWITCH} must be unset or \"packaged\""),
    };
    let out = PathBuf::from(std::env::var_os("OUT_DIR").expect("OUT_DIR"))
        .join("builder_toolchain_manifest.rs");
    // Rewritten only on change, so an unchanged manifest never forces a rebuild.
    if std::fs::read_to_string(&out).ok().as_deref() != Some(source.as_str()) {
        std::fs::write(&out, source).expect("write Builder toolchain manifest");
    }
    tauri_build::build();
}

fn packaged() -> String {
    let root = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"))
        .join("builder-toolchain");
    println!("cargo::rerun-if-changed={}", root.display());
    let cfg = |key: &str| std::env::var(key).unwrap_or_default();
    let target = packaging::Target::from_cargo(
        &cfg("CARGO_CFG_TARGET_OS"),
        &cfg("CARGO_CFG_TARGET_ARCH"),
        &cfg("CARGO_CFG_TARGET_ENV"),
    )
    .expect("no packaged Builder toolchain exists for this target");
    let source = packaging::generate(&root, target)
        .unwrap_or_else(|error| panic!("packaged Builder toolchain rejected: {error}"));
    println!("cargo::rustc-cfg=nexus_packaged_toolchain");
    source
}
