use social_poster_agent::run_social_poster_from_manifest;
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

    match run_social_poster_from_manifest(manifest_path.as_path(), dry_run) {
        Ok(report) => {
            println!(
                "social-poster completed: generated={}, published={}, dry_run={}",
                report.generated_posts.len(),
                report.published_post_ids.len(),
                report.dry_run
            );
            if report.dry_run {
                for (idx, post) in report.generated_posts.iter().enumerate() {
                    println!(
                        "\n--- dry-run post {} [{}] ---\n{}\n",
                        idx + 1,
                        "x",
                        post.text
                    );
                }
            }
        }
        Err(error) => {
            eprintln!("social-poster failed: {error}");
            std::process::exit(1);
        }
    }
}

fn default_manifest_path() -> PathBuf {
    if let Ok(cwd) = env::current_dir() {
        let candidate = cwd.join("agents/social-poster/manifest.toml");
        if candidate.exists() {
            return candidate;
        }
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("manifest.toml")
}

fn print_help() {
    println!("Usage: social-poster-agent [--manifest <path>] [--dry-run]");
}
