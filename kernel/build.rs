use std::env;
use std::path::PathBuf;
use std::process::Command;

fn run(command: &mut Command) {
    let status = command.status().expect("execute macOS SDK build tool");
    assert!(status.success(), "macOS SDK build tool failed: {command:?}");
}

fn main() {
    println!("cargo:rerun-if-changed=src/resource_limiter/darwin_proc.c");
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("macos") {
        return;
    }
    // libc 0.2.183 omits kinfo_proc. Let the target SDK define its ABI rather
    // than duplicating a private Rust layout. No runtime subprocess is used.
    let arch = match env::var("CARGO_CFG_TARGET_ARCH").as_deref() {
        Ok("aarch64") => "arm64",
        Ok("x86_64") => "x86_64",
        other => panic!("unsupported macOS architecture: {other:?}"),
    };
    let out = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo OUT_DIR"));
    let object = out.join("darwin_proc.o");
    let archive = out.join("libnexus_darwin_proc.a");
    run(Command::new("xcrun")
        .args([
            "--sdk",
            "macosx",
            "clang",
            "-arch",
            arch,
            "-std=c11",
            "-Wall",
            "-Wextra",
            "-Werror",
            "-c",
            "src/resource_limiter/darwin_proc.c",
            "-o",
        ])
        .arg(&object));
    run(Command::new("xcrun")
        .args(["--sdk", "macosx", "libtool", "-static", "-o"])
        .arg(&archive)
        .arg(&object));
    println!("cargo:rustc-link-search=native={}", out.display());
    println!("cargo:rustc-link-lib=static=nexus_darwin_proc");
}
