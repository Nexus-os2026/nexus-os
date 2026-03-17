//! Generate genome files for all prebuilt agents.
//! Run: cargo run -p nexus-kernel --example generate_genomes

use nexus_kernel::genome::{genome_from_manifest, JsonAgentManifest};
use std::path::PathBuf;

fn main() {
    let prebuilt_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../agents/prebuilt");
    let genome_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../agents/genomes");

    std::fs::create_dir_all(&genome_dir).expect("create genomes dir");

    let mut generated = 0;
    let mut errors = 0;

    let entries: Vec<_> = std::fs::read_dir(&prebuilt_dir)
        .expect("read prebuilt dir")
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map(|x| x == "json").unwrap_or(false))
        .collect();

    for entry in &entries {
        let path = entry.path();
        let raw = match std::fs::read_to_string(&path) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("  SKIP {}: {e}", path.display());
                errors += 1;
                continue;
            }
        };
        let manifest: JsonAgentManifest = match serde_json::from_str(&raw) {
            Ok(m) => m,
            Err(e) => {
                eprintln!("  SKIP {}: {e}", path.display());
                errors += 1;
                continue;
            }
        };

        let genome = genome_from_manifest(&manifest);
        let out_path = genome_dir.join(format!("{}.genome.json", genome.agent_id));
        let json = serde_json::to_string_pretty(&genome).expect("serialize");
        std::fs::write(&out_path, json).expect("write genome");
        println!("  OK  {} → {}", manifest.name, out_path.display());
        generated += 1;
    }

    println!(
        "\nGenerated: {generated}, Errors: {errors}, Total: {}",
        entries.len()
    );
}
