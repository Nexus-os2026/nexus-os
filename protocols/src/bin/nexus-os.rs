fn main() {
    let runtime = match tokio::runtime::Runtime::new() {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("Failed to create tokio runtime: {e}");
            std::process::exit(1);
        }
    };
    if let Err(error) = runtime.block_on(nexus_protocols::server_runtime::run_from_args(
        std::env::args(),
    )) {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
