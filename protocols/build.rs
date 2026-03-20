use std::env;
use std::path::PathBuf;

fn main() {
    let manifest_dir = PathBuf::from(match env::var("CARGO_MANIFEST_DIR") {
        Ok(d) => d,
        Err(e) => {
            eprintln!("CARGO_MANIFEST_DIR not set: {e}");
            std::process::exit(1);
        }
    });
    let frontend_dist = manifest_dir.join("../app/dist");
    println!("cargo:rerun-if-changed={}", frontend_dist.display());
}
