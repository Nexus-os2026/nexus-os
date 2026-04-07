use std::path::PathBuf;

use nexus_computer_use::capability::check_system_requirements;
use nexus_computer_use::capture::backend::detect_backend;
use nexus_computer_use::capture::screenshot::{
    take_screenshot_with_backend, CaptureRegion, ScreenshotOptions,
};

fn parse_region(s: &str) -> Option<CaptureRegion> {
    let parts: Vec<&str> = s.split(',').collect();
    if parts.len() != 4 {
        eprintln!("Region format: x,y,w,h (e.g., 0,0,800,600)");
        return None;
    }
    let nums: Result<Vec<u32>, _> = parts.iter().map(|p| p.trim().parse()).collect();
    match nums {
        Ok(v) => match CaptureRegion::new(v[0], v[1], v[2], v[3]) {
            Ok(r) => Some(r),
            Err(e) => {
                eprintln!("Invalid region: {e}");
                None
            }
        },
        Err(e) => {
            eprintln!("Failed to parse region numbers: {e}");
            None
        }
    }
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    println!("=== Nexus Computer Use — Screen Capture Test ===\n");

    // Check system requirements
    let req = check_system_requirements();
    println!("System Requirements:");
    println!(
        "  Display server: {}",
        req.display_server.as_deref().unwrap_or("none")
    );
    println!(
        "  Capture tool:   {}",
        req.capture_tool.as_deref().unwrap_or("none")
    );
    println!(
        "  Input tool:     {}",
        req.input_tool.as_deref().unwrap_or("none (Phase 2)")
    );
    println!("  Capture ready:  {}", req.all_capture_ready);
    println!();

    if !req.all_capture_ready {
        eprintln!("Screen capture not available. Install grim (Wayland) or scrot (X11).");
        std::process::exit(1);
    }

    // Parse args
    let args: Vec<String> = std::env::args().collect();
    let mut region: Option<CaptureRegion> = None;
    let mut output: Option<PathBuf> = None;
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--region" => {
                if i + 1 < args.len() {
                    region = parse_region(&args[i + 1]);
                    i += 2;
                } else {
                    eprintln!("--region requires a value");
                    std::process::exit(1);
                }
            }
            "--output" => {
                if i + 1 < args.len() {
                    output = Some(PathBuf::from(&args[i + 1]));
                    i += 2;
                } else {
                    eprintln!("--output requires a value");
                    std::process::exit(1);
                }
            }
            other => {
                eprintln!("Unknown argument: {other}");
                eprintln!("Usage: nx-screen [--region x,y,w,h] [--output path.png]");
                std::process::exit(1);
            }
        }
    }

    // Detect backend
    let backend = match detect_backend() {
        Ok(b) => b,
        Err(e) => {
            eprintln!("Backend detection failed: {e}");
            std::process::exit(1);
        }
    };

    println!(
        "Using backend: {} ({})",
        backend.kind, backend.display_server
    );

    // Take screenshot
    let options = ScreenshotOptions {
        region,
        ..Default::default()
    };

    match take_screenshot_with_backend(&backend, options).await {
        Ok(ss) => {
            println!("\nScreenshot captured successfully!");
            println!("  ID:         {}", ss.id);
            println!("  Dimensions: {}x{}", ss.width, ss.height);
            println!("  File size:  {} bytes", ss.file_size_bytes);
            println!("  Backend:    {}", ss.backend);
            println!("  Hash:       {}", ss.audit_hash);
            println!("  Timestamp:  {}", ss.timestamp);

            if let Some(path) = output {
                match std::fs::write(&path, &ss.png_bytes) {
                    Ok(()) => println!("\n  Saved to: {}", path.display()),
                    Err(e) => eprintln!("\n  Failed to save: {e}"),
                }
            }
        }
        Err(e) => {
            eprintln!("\nCapture failed: {e}");
            std::process::exit(1);
        }
    }
}
