use nexus_connectors_core::validation::{
    validate_anthropic_key, validate_brave_key, validate_telegram_token,
};
use nexus_kernel::config::{
    load_config, load_config_from_path, save_config, AgentLlmConfig, HardwareConfig, ModelsConfig,
    NexusConfig, OllamaConfig,
};
use nexus_kernel::hardware::{recommend_agent_configs, HardwareProfile};
use nexus_connectors_llm::providers::OllamaProvider;
use std::io::{self, Write};
use std::path::Path;

pub fn run_setup(check: bool) -> Result<String, String> {
    if check {
        return run_setup_check();
    }
    run_setup_interactive()
}

pub fn run_setup_check() -> Result<String, String> {
    let config = load_config().map_err(|error| format!("failed to load config: {error}"))?;
    Ok(render_setup_check(&config))
}

pub fn run_setup_check_with_path(path: &Path) -> Result<String, String> {
    let config =
        load_config_from_path(path).map_err(|error| format!("failed to load config: {error}"))?;
    Ok(render_setup_check(&config))
}

pub fn render_setup_check(config: &NexusConfig) -> String {
    let mut lines = vec![
        "NEXUS setup status".to_string(),
        String::new(),
        "--- Hardware ---".to_string(),
        format!("GPU: {}", if config.hardware.gpu.is_empty() { "not detected" } else { config.hardware.gpu.as_str() }),
        format!("VRAM: {} MB", config.hardware.vram_mb),
        format!("RAM: {} MB", config.hardware.ram_mb),
        String::new(),
        "--- Ollama ---".to_string(),
        format!("URL: {}", config.ollama.base_url),
        format!("Status: {}", if config.ollama.status.is_empty() { "unknown" } else { config.ollama.status.as_str() }),
        String::new(),
        "--- Models ---".to_string(),
        format!("Primary: {}", if config.models.primary.is_empty() { "not set" } else { config.models.primary.as_str() }),
        format!("Fast: {}", if config.models.fast.is_empty() { "not set" } else { config.models.fast.as_str() }),
    ];

    if !config.agents.is_empty() {
        lines.push(String::new());
        lines.push("--- Agent Configs ---".to_string());
        for (name, agent_config) in &config.agents {
            lines.push(format!(
                "{}: model={}, temp={}, max_tokens={}",
                name, agent_config.model, agent_config.temperature, agent_config.max_tokens
            ));
        }
    }

    lines.push(String::new());
    lines.push("--- API Keys ---".to_string());
    lines.push(format!("Anthropic: {}", status(&config.llm.anthropic_api_key)));
    lines.push(format!("OpenAI: {}", status(&config.llm.openai_api_key)));
    lines.push(format!("Brave: {}", status(&config.search.brave_api_key)));
    lines.push(format!("X: {}", status(&config.social.x_api_key)));
    lines.push(format!("Telegram: {}", status(&config.messaging.telegram_bot_token)));

    lines.join("\n")
}

fn run_setup_interactive() -> Result<String, String> {
    let mut config = load_config().map_err(|error| format!("failed to load config: {error}"))?;

    println!();
    println!("=== NEXUS OS Setup Wizard ===");
    println!();

    // Step 1: Hardware Detection
    println!("Step 1: Detecting hardware...");
    let hw = HardwareProfile::detect();
    println!("  GPU: {}", hw.gpu);
    println!("  VRAM: {} MB", hw.vram_mb);
    println!("  RAM: {} MB", hw.ram_mb);
    config.hardware = HardwareConfig {
        gpu: hw.gpu.clone(),
        vram_mb: hw.vram_mb,
        ram_mb: hw.ram_mb,
        detected_at: hw.detected_at.clone(),
    };
    println!();

    // Step 2: Ollama Connection
    println!("Step 2: Checking Ollama...");
    let ollama_url = if ask_yes_no("Use custom Ollama URL? (default: http://localhost:11434) (y/n)")? {
        ask_value("Ollama URL")?
    } else {
        "http://localhost:11434".to_string()
    };

    let provider = OllamaProvider::new(&ollama_url);
    let ollama_connected = match provider.health_check() {
        Ok(true) => {
            println!("  Ollama connected at {}", ollama_url);
            true
        }
        _ => {
            println!("  Ollama not reachable at {}. You can start it later.", ollama_url);
            false
        }
    };

    config.ollama = OllamaConfig {
        base_url: ollama_url.clone(),
        status: if ollama_connected { "connected".to_string() } else { "disconnected".to_string() },
    };
    config.llm.ollama_url = ollama_url;
    println!();

    // Step 3: Model Recommendations
    let tier = hw.recommended_tier();
    println!("Step 3: Model recommendations (tier: {})", tier.label());
    println!("  Primary model: {}", tier.primary_model());
    println!("  Fast model: {}", tier.fast_model());

    config.models = ModelsConfig {
        primary: tier.primary_model().to_string(),
        fast: tier.fast_model().to_string(),
    };

    // Set agent configs
    let agent_configs = recommend_agent_configs(tier);
    for (name, ac) in &agent_configs {
        config.agents.insert(
            name.to_string(),
            AgentLlmConfig {
                model: ac.model.clone(),
                temperature: ac.temperature,
                max_tokens: ac.max_tokens,
            },
        );
    }
    println!("  Configured {} agent model assignments", agent_configs.len());
    println!();

    // Step 4: Download models
    if ollama_connected {
        let primary = config.models.primary.clone();
        let fast = config.models.fast.clone();

        // Check installed models
        let installed = provider.list_models().unwrap_or_default();
        let installed_names: Vec<&str> = installed.iter().map(|m| m.name.as_str()).collect();

        let models_to_pull: Vec<String> = [&primary, &fast]
            .iter()
            .filter(|m| !installed_names.contains(&m.as_str()))
            .map(|m| m.to_string())
            .collect();

        if models_to_pull.is_empty() {
            println!("Step 4: Models already installed.");
        } else {
            println!("Step 4: Models to download: {}", models_to_pull.join(", "));
            for model in &models_to_pull {
                if ask_yes_no(format!("Download {}? (y/n)", model).as_str())? {
                    println!("  Pulling {}...", model);
                    match provider.pull_model(model, |status, completed, total| {
                        if total > 0 {
                            let pct = (completed as f64 / total as f64 * 100.0) as u32;
                            print!("\r  {} {}%", status, pct);
                            let _ = io::stdout().flush();
                        } else {
                            print!("\r  {}", status);
                            let _ = io::stdout().flush();
                        }
                    }) {
                        Ok(_) => println!("\n  {} downloaded.", model),
                        Err(e) => println!("\n  Failed to download {}: {}", model, e),
                    }
                }
            }
        }
        println!();

        // Update default model to use Ollama
        config.llm.default_model = config.models.primary.clone();
    } else {
        println!("Step 4: Skipping model download (Ollama not connected).");
        println!();
    }

    // Step 5: API Keys (optional)
    println!("Step 5: API Keys (optional, press Enter to skip)");
    if ask_yes_no("Configure Anthropic API key? (y/n)")? {
        let key = ask_value("Anthropic API key")?;
        if validate_anthropic_key(key.as_str()) {
            config.llm.anthropic_api_key = key;
            println!("  Validated and saved.");
        } else {
            println!("  Validation failed, skipping.");
        }
    }

    if ask_yes_no("Configure Brave Search API key? (y/n)")? {
        let key = ask_value("Brave API key")?;
        if validate_brave_key(key.as_str()) {
            config.search.brave_api_key = key;
            println!("  Validated and saved.");
        } else {
            println!("  Validation failed, skipping.");
        }
    }

    if ask_yes_no("Configure Telegram bot token? (y/n)")? {
        let token = ask_value("Telegram bot token")?;
        if validate_telegram_token(token.as_str()) {
            config.messaging.telegram_bot_token = token;
            println!("  Validated and saved.");
        } else {
            println!("  Validation failed, skipping.");
        }
    }

    save_config(&config).map_err(|error| format!("failed to save config: {error}"))?;

    println!();
    println!("Setup complete! Config saved to ~/.nexus/config.toml");
    println!("Run 'nexus setup --check' to verify your setup.");
    Ok("Setup complete.".to_string())
}

fn status(value: &str) -> &'static str {
    if value.trim().is_empty() {
        "not configured"
    } else {
        "configured"
    }
}

fn ask_yes_no(prompt: &str) -> Result<bool, String> {
    print!("{prompt} ");
    io::stdout()
        .flush()
        .map_err(|error| format!("failed to flush output: {error}"))?;
    let mut line = String::new();
    io::stdin()
        .read_line(&mut line)
        .map_err(|error| format!("failed to read input: {error}"))?;
    Ok(matches!(line.trim().to_lowercase().as_str(), "y" | "yes"))
}

fn ask_value(prompt: &str) -> Result<String, String> {
    print!("{prompt}: ");
    io::stdout()
        .flush()
        .map_err(|error| format!("failed to flush output: {error}"))?;
    let mut line = String::new();
    io::stdin()
        .read_line(&mut line)
        .map_err(|error| format!("failed to read input: {error}"))?;
    Ok(line.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::run_setup_check_with_path;
    use nexus_kernel::config::{save_config_to_path, NexusConfig};
    use std::path::PathBuf;
    use uuid::Uuid;

    fn temp_config_path() -> PathBuf {
        std::env::temp_dir()
            .join(format!("nexus-setup-test-{}", Uuid::new_v4()))
            .join(".nexus")
            .join("config.toml")
    }

    #[test]
    fn test_setup_check_shows_status() {
        let path = temp_config_path();
        let mut config = NexusConfig::default();
        config.llm.anthropic_api_key = "sk-ant-test".to_string();

        let saved = save_config_to_path(path.as_path(), &config);
        assert!(saved.is_ok());

        let output = run_setup_check_with_path(path.as_path()).unwrap_or_default();
        assert!(output.contains("Anthropic: configured"));
    }

    #[test]
    fn test_setup_check_shows_hardware() {
        let path = temp_config_path();
        let mut config = NexusConfig::default();
        config.hardware.gpu = "NVIDIA GeForce RTX 4070".to_string();
        config.hardware.vram_mb = 12288;
        config.hardware.ram_mb = 32768;

        let saved = save_config_to_path(path.as_path(), &config);
        assert!(saved.is_ok());

        let output = run_setup_check_with_path(path.as_path()).unwrap_or_default();
        assert!(output.contains("NVIDIA GeForce RTX 4070"));
        assert!(output.contains("12288"));
    }
}
