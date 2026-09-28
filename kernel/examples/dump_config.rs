//! Tiny helper that reports whether the Nexus configuration holds an NVIDIA
//! NIM API key. It prints only presence, never the value: a stored credential
//! is not written to standard output, where terminals, logs and scripts would
//! keep it. Scripts that need the key take it from `NVIDIA_NIM_API_KEY` in
//! their own environment; the output line deliberately has no `NVIDIA_KEY=`
//! form, so a script that parsed the old line finds no key rather than
//! mistaking the presence word for one.

fn main() {
    match nexus_kernel::config::load_config() {
        Ok(config) => {
            let presence = if config.llm.nvidia_api_key.is_empty() {
                "absent"
            } else {
                "present"
            };
            println!("nvidia_api_key: {presence}");
        }
        Err(e) => {
            eprintln!("Failed to load config: {e}");
            std::process::exit(1);
        }
    }
}
