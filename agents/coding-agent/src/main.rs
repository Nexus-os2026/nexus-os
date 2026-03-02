use coding_agent::run_coding_agent_from_manifest;
use std::env;
use std::path::PathBuf;

fn main() {
    let mut manifest_path = default_manifest_path();
    let mut dry_run = false;

    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--manifest" => {
                let Some(path) = args.next() else {
                    eprintln!("missing value for --manifest");
                    std::process::exit(2);
                };
                manifest_path = PathBuf::from(path);
            }
            "--dry-run" => {
                dry_run = true;
            }
            "-h" | "--help" => {
                print_help();
                return;
            }
            _ => {
                eprintln!("unknown argument: {arg}");
                print_help();
                std::process::exit(2);
            }
        }
    }

    match run_coding_agent_from_manifest(manifest_path.as_path(), dry_run) {
        Ok(report) => {
            println!(
                "coding-agent completed: success={}, iterations={}, modified_files={}, dry_run={}",
                report.success,
                report.iterations,
                report.modified_files.len(),
                report.dry_run
            );
            if !report.status.is_empty() {
                println!("status: {}", report.status);
            }
        }
        Err(error) => {
            eprintln!("coding-agent failed: {error}");
            std::process::exit(1);
        }
    }
}

fn default_manifest_path() -> PathBuf {
    if let Ok(cwd) = env::current_dir() {
        let candidate = cwd.join("agents/coding-agent/manifest.toml");
        if candidate.exists() {
            return candidate;
        }
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("manifest.toml")
}

fn print_help() {
    println!("Usage: coding-agent [--manifest <path>] [--dry-run]");
}
