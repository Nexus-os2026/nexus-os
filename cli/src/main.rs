use clap::Parser;
use nexus_cli::{execute_command, Cli};

fn main() {
    let cli = Cli::parse();
    match execute_command(cli) {
        Ok(message) => println!("{message}"),
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    }
}
