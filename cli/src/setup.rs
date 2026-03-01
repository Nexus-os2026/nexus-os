use nexus_connectors_core::validation::{
    validate_anthropic_key, validate_brave_key, validate_telegram_token,
};
use nexus_kernel::config::{load_config, load_config_from_path, save_config, NexusConfig};
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
    [
        "NEXUS setup status".to_string(),
        format!("Anthropic: {}", status(&config.llm.anthropic_api_key)),
        format!("OpenAI: {}", status(&config.llm.openai_api_key)),
        format!("Brave: {}", status(&config.search.brave_api_key)),
        format!("X: {}", status(&config.social.x_api_key)),
        format!("Facebook: {}", status(&config.social.facebook_page_token)),
        format!(
            "Instagram: {}",
            status(&config.social.instagram_access_token)
        ),
        format!("Telegram: {}", status(&config.messaging.telegram_bot_token)),
        format!("WhatsApp: {}", status(&config.messaging.whatsapp_api_token)),
        format!("Discord: {}", status(&config.messaging.discord_bot_token)),
        format!("Slack: {}", status(&config.messaging.slack_bot_token)),
    ]
    .join("\n")
}

fn run_setup_interactive() -> Result<String, String> {
    let mut config = load_config().map_err(|error| format!("failed to load config: {error}"))?;

    if ask_yes_no("Do you have an Anthropic API key? (y/n)")? {
        let key = ask_value("Paste Anthropic API key")?;
        if validate_anthropic_key(key.as_str()) {
            config.llm.anthropic_api_key = key;
            println!("Anthropic key validated and saved.");
        } else {
            println!("Anthropic key validation failed, skipping.");
        }
    }

    if ask_yes_no("Do you have an OpenAI API key? (y/n)")? {
        config.llm.openai_api_key = ask_value("Paste OpenAI API key")?;
    }

    if ask_yes_no("Use a custom Ollama URL? (y/n)")? {
        config.llm.ollama_url = ask_value("Paste Ollama URL")?;
    }

    if ask_yes_no("Do you have a Brave Search API key? (y/n)")? {
        let key = ask_value("Paste Brave API key")?;
        if validate_brave_key(key.as_str()) {
            config.search.brave_api_key = key;
            println!("Brave key validated and saved.");
        } else {
            println!("Brave key validation failed, skipping.");
        }
    }

    if ask_yes_no("Do you have an X API key? (y/n)")? {
        config.social.x_api_key = ask_value("Paste X API key")?;
        config.social.x_api_secret = ask_value("Paste X API secret")?;
        config.social.x_access_token = ask_value("Paste X access token")?;
        config.social.x_access_secret = ask_value("Paste X access secret")?;
    }

    if ask_yes_no("Do you have a Facebook page token? (y/n)")? {
        config.social.facebook_page_token = ask_value("Paste Facebook page token")?;
    }

    if ask_yes_no("Do you have an Instagram access token? (y/n)")? {
        config.social.instagram_access_token = ask_value("Paste Instagram access token")?;
    }

    if ask_yes_no("Do you have a Telegram bot token? (y/n)")? {
        let token = ask_value("Paste Telegram bot token")?;
        if validate_telegram_token(token.as_str()) {
            config.messaging.telegram_bot_token = token;
            println!("Telegram token validated and saved.");
        } else {
            println!("Telegram token validation failed, skipping.");
        }
    }

    if ask_yes_no("Do you have a WhatsApp Business ID + API token? (y/n)")? {
        config.messaging.whatsapp_business_id = ask_value("Paste WhatsApp Business ID")?;
        config.messaging.whatsapp_api_token = ask_value("Paste WhatsApp API token")?;
    }

    if ask_yes_no("Do you have a Discord bot token? (y/n)")? {
        config.messaging.discord_bot_token = ask_value("Paste Discord bot token")?;
    }

    if ask_yes_no("Do you have a Slack bot token? (y/n)")? {
        config.messaging.slack_bot_token = ask_value("Paste Slack bot token")?;
    }

    if ask_yes_no("Use a custom Whisper model setting? (y/n)")? {
        config.voice.whisper_model = ask_value("Set whisper model (auto/tiny/base/medium)")?;
    }

    if ask_yes_no("Use a custom wake word? (y/n)")? {
        config.voice.wake_word = ask_value("Set wake word")?;
    }

    if ask_yes_no("Use a custom TTS voice? (y/n)")? {
        config.voice.tts_voice = ask_value("Set TTS voice")?;
    }

    if ask_yes_no("Enable telemetry? (y/n)")? {
        config.privacy.telemetry = true;
    }

    save_config(&config).map_err(|error| format!("failed to save config: {error}"))?;
    Ok("Setup complete! Run 'nexus agent create' to start.".to_string())
}

fn status(value: &str) -> &'static str {
    if value.trim().is_empty() {
        "✗ not configured"
    } else {
        "✓ configured"
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
        assert!(output.contains("Anthropic: ✓ configured"));
    }
}
