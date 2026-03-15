use std::env;
use std::path::PathBuf;

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("manifest dir"));
    let frontend_dist = manifest_dir.join("../app/dist");
    println!("cargo:rerun-if-changed={}", frontend_dist.display());
    if !frontend_dist.exists() {
        panic!(
            "embedded frontend dist not found at {}. Run `npm --prefix app run build` or `make nexus-os` first.",
            frontend_dist.display()
        );
    }
    println!(
        "cargo:rustc-env=NEXUS_FRONTEND_DIST={}",
        frontend_dist.display()
    );
}
