use std::sync::Arc;

use colored::Colorize;

use crate::config::NxConfig;
use crate::error::NxError;
use crate::governance::GovernanceKernel;
use crate::llm::providers::anthropic::AnthropicProvider;
use crate::llm::providers::claude_cli::ClaudeCliProvider;
use crate::llm::providers::google::GoogleProvider;
use crate::llm::{ModelRouter, ModelSlot, ProviderRegistry, SlotConfig};

/// Application state holding config, governance, routing, tools, and session-level state.
pub struct App {
    /// Application configuration.
    pub config: NxConfig,
    /// Governance kernel (identity, audit, ACL, consent, fuel).
    pub governance: GovernanceKernel,
    /// Model router with provider registry.
    pub router: ModelRouter,
    /// Registry of all available tools.
    pub tool_registry: crate::tools::ToolRegistry,
    /// Behavioral envelope (action drift detection).
    pub envelope: crate::agent::envelope::BehavioralEnvelope,
    /// Self-improvement engine (prompt versioning).
    pub self_improve: crate::self_improve::SelfImproveEngine,
    /// Cross-session memory store.
    pub memory: crate::persistence::memory::MemoryStore,
    /// MCP connection manager.
    pub mcp_manager: crate::mcp::McpManager,
}

impl App {
    /// Create a new application instance.
    pub fn new(config: NxConfig) -> Result<Self, NxError> {
        let governance = GovernanceKernel::new(config.fuel_budget)?;

        // Build provider registry
        let mut registry = ProviderRegistry::new();
        registry.register(Box::new(ClaudeCliProvider::new()));
        registry.register(Box::new(AnthropicProvider::new()));
        registry.register(Box::new(crate::llm::providers::create_openai_provider()));
        registry.register(Box::new(GoogleProvider::new()));
        registry.register(Box::new(crate::llm::providers::create_ollama_provider()));
        registry.register(Box::new(crate::llm::providers::create_openrouter_provider()));
        registry.register(Box::new(crate::llm::providers::create_groq_provider()));
        registry.register(Box::new(crate::llm::providers::create_deepseek_provider()));

        let registry = Arc::new(registry);
        let mut router = ModelRouter::new(registry);

        // Set default execution slot from config
        router.set_slot(
            ModelSlot::Execution,
            SlotConfig {
                provider: config.default_provider.clone(),
                model: config.default_model.clone(),
            },
        );

        let tool_registry = crate::tools::ToolRegistry::with_defaults();

        let envelope = crate::agent::envelope::BehavioralEnvelope::new(
            crate::agent::envelope::EnvelopeConfig::default(),
        );

        let self_improve = crate::self_improve::SelfImproveEngine::new(
            "You are Nexus Code, a governed terminal coding agent.",
        );

        let memory_path = dirs::data_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join("nexus-code")
            .join("memory.json");
        let memory = crate::persistence::memory::MemoryStore::load(memory_path);

        let mcp_manager = crate::mcp::McpManager::new();

        Ok(Self {
            config,
            governance,
            router,
            tool_registry,
            envelope,
            self_improve,
            memory,
            mcp_manager,
        })
    }

    /// Enable computer use capabilities: register 3 screen tools + grant ComputerUse capability.
    pub fn enable_computer_use(&mut self) {
        self.tool_registry.register_computer_use_tools();
        self.governance.capabilities.grant(
            crate::governance::Capability::ComputerUse,
            crate::governance::CapabilityScope::Full,
        );
    }

    /// Disable computer use capabilities: remove 3 screen tools + revoke ComputerUse capability.
    pub fn disable_computer_use(&mut self) {
        self.tool_registry.unregister_computer_use_tools();
        self.governance
            .capabilities
            .revoke(crate::governance::Capability::ComputerUse);
    }

    /// Check if computer use is active (ComputerUse capability granted).
    pub fn is_computer_use_active(&self) -> bool {
        self.governance
            .capabilities
            .granted()
            .iter()
            .any(|g| g.capability == crate::governance::Capability::ComputerUse)
    }

    /// Run the init command — create NEXUSCODE.md in the current directory.
    pub fn init(&self) -> Result<(), NxError> {
        let path = std::path::Path::new("NEXUSCODE.md");
        if path.exists() {
            println!("{} NEXUSCODE.md already exists", "✓".green());
            return Ok(());
        }

        let content = format!(
            r#"# NEXUSCODE.md — Nexus Code Project Configuration

provider: {}
model: {}
fuel_budget: {}

## Governance

This project is governed by Nexus Code. All actions are recorded in a
hash-chained audit trail with Ed25519 signatures.

## Capabilities

Default capabilities: file_read, git_read, env_read, llm_call
Additional capabilities must be granted via the REPL.

## Blocked Paths

# blocked_paths: .env, secrets/, credentials/
"#,
            self.config.default_provider, self.config.default_model, self.config.fuel_budget,
        );

        std::fs::write(path, content)?;
        println!(
            "{} Created NEXUSCODE.md with default configuration",
            "✓".green()
        );
        Ok(())
    }

    /// Run the status command.
    pub fn status(&self) {
        let identity = &self.governance.identity;
        let audit = &self.governance.audit;
        let fuel = self.governance.fuel.budget();
        let caps = self.governance.capabilities.granted();

        println!();
        println!("{}", "Nexus Code — Governance Status".bold().underline());
        println!();
        println!("  {}", "Session Identity".bold());
        println!("    ID:         {}", identity.session_id());
        println!(
            "    Public Key: {}",
            hex::encode(identity.public_key_bytes())
        );
        println!(
            "    Created:    {}",
            identity.created_at().format("%Y-%m-%d %H:%M:%S UTC")
        );
        println!();
        println!("  {}", "Audit Trail".bold());
        println!("    Entries:    {}", audit.len());
        if let Some(last) = audit.entries().last() {
            println!("    Last Hash:  {}", &last.entry_hash);
            println!("    Last Sig:   {}...", &last.signature[..32]);
        }
        if audit.verify_chain().is_ok() {
            println!("    Integrity:  {} Valid", "✓".green());
        } else {
            println!("    Integrity:  {} CORRUPTED", "✗".red());
        }
        println!();
        println!("  {}", "Fuel Budget".bold());
        println!("    Total:      {}", fuel.total);
        println!("    Consumed:   {}", fuel.consumed);
        println!(
            "    Remaining:  {}",
            fuel.total.saturating_sub(fuel.consumed + fuel.reserved)
        );
        println!("    Est. Cost:  ${:.4}", fuel.cost_usd);
        println!();
        println!("  {}", "Capabilities".bold());
        println!("    Granted:    {} / 13", caps.len());
        for grant in &caps {
            println!("    - {} ({:?})", grant.capability.as_str(), grant.scope);
        }
        println!();
    }

    /// Print available providers and their configuration status.
    pub fn print_providers(&self) {
        let claude_cli_available = crate::setup::check_command_exists("claude");
        let providers: Vec<(&str, bool, &str)> = vec![
            ("claude_cli", claude_cli_available, "claude-cli"),
            (
                "anthropic",
                std::env::var("ANTHROPIC_API_KEY").is_ok(),
                "claude-sonnet-4-20250514",
            ),
            ("openai", std::env::var("OPENAI_API_KEY").is_ok(), "gpt-4o"),
            (
                "google",
                std::env::var("GOOGLE_API_KEY").is_ok(),
                "gemini-2.5-flash",
            ),
            ("ollama", true, "qwen3:8b"),
            (
                "openrouter",
                std::env::var("OPENROUTER_API_KEY").is_ok(),
                "anthropic/claude-sonnet-4",
            ),
            (
                "groq",
                std::env::var("GROQ_API_KEY").is_ok(),
                "llama-3.3-70b-versatile",
            ),
            (
                "deepseek",
                std::env::var("DEEPSEEK_API_KEY").is_ok(),
                "deepseek-chat",
            ),
        ];

        println!();
        println!("{}", "Available Providers".bold().underline());
        println!();
        for (name, configured, default_model) in &providers {
            let status = if *configured {
                "✓".green().to_string()
            } else {
                "✗".red().to_string()
            };
            let label = if *name == "claude_cli" && *configured {
                "configured (Max plan, $0)"
            } else if *configured {
                "configured"
            } else {
                "not configured"
            };
            println!(
                "  {} {:<12} {} (default: {})",
                status,
                name.bold(),
                label,
                default_model.dimmed()
            );
        }
        println!();
    }

    /// Run the info command.
    pub fn info(&self) {
        println!();
        println!("{} v{}", "Nexus Code".bold(), env!("CARGO_PKG_VERSION"));
        println!("  The world's first governed terminal coding agent");
        println!("  Binary: nx");
        println!("  Session: {}", self.governance.identity.session_id());
        println!(
            "  Governance: Ed25519 identity, hash-chained audit, capability ACL, HITL consent"
        );
        println!("  Tools: {} registered", self.tool_registry.list().len());
        println!("  Providers: 8 (Claude CLI, Anthropic, OpenAI, Google, Ollama, OpenRouter, Groq, DeepSeek)");
        println!("  Slots: 5 (Execution, Thinking, Critique, Compact, Vision)");
        println!();
    }
}
