//! Tiny helper to extract API keys from the encrypted Nexus config.
//! Used by Python test scripts that need the NVIDIA NIM API key.

fn main() {
    match nexus_kernel::config::load_config() {
        Ok(config) => {
            if !config.llm.nvidia_api_key.is_empty() {
                println!("NVIDIA_KEY={}", config.llm.nvidia_api_key);
            }
        }
        Err(e) => {
            eprintln!("Failed to load config: {e}");
            std::process::exit(1);
        }
    }
}
