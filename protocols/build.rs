use std::env;
use std::path::PathBuf;

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("manifest dir"));
    let frontend_dist = manifest_dir.join("../app/dist");
    println!("cargo:rerun-if-changed={}", frontend_dist.display());
}
