fn main() {
    let runtime = tokio::runtime::Runtime::new().expect("tokio runtime");
    if let Err(error) = runtime.block_on(nexus_protocols::server_runtime::run_from_args(
        std::env::args(),
    )) {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
