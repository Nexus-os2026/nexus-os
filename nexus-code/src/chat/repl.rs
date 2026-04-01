use std::sync::Arc;

use colored::Colorize;
use tokio::sync::{mpsc, Mutex};

use crate::app::App;
use crate::error::NxError;
use crate::governance::ConsentTier;
use crate::llm::types::{Message, Role};
use crate::llm::ModelSlot;

/// Interactive chat REPL with governance integration.
pub struct ChatRepl {
    app: Arc<Mutex<App>>,
    history: Vec<Message>,
}

impl ChatRepl {
    /// Create a new chat REPL.
    pub fn new(app: Arc<Mutex<App>>) -> Self {
        Self {
            app,
            history: Vec::new(),
        }
    }

    /// Run the interactive REPL loop.
    pub async fn run(&mut self) -> Result<(), NxError> {
        self.print_banner().await;

        let mut rl = rustyline::DefaultEditor::new()
            .map_err(|e| NxError::ConfigError(format!("readline: {}", e)))?;

        loop {
            let readline = rl.readline(&format!("{} ", "❯".cyan()));
            match readline {
                Ok(line) => {
                    let input = line.trim().to_string();
                    if input.is_empty() {
                        continue;
                    }

                    let _ = rl.add_history_entry(&input);

                    if input.starts_with('/') && self.handle_slash_command(&input).await? {
                        continue;
                    }

                    if let Err(e) = self.process_message(&input).await {
                        println!("{} {}", "Error:".red().bold(), e);
                    }
                }
                Err(
                    rustyline::error::ReadlineError::Interrupted
                    | rustyline::error::ReadlineError::Eof,
                ) => {
                    self.shutdown().await;
                    break;
                }
                Err(e) => {
                    println!("{} {}", "Readline error:".red(), e);
                    break;
                }
            }
        }

        Ok(())
    }

    /// Print the welcome banner.
    async fn print_banner(&self) {
        let app = self.app.lock().await;
        let session_id = app.governance.identity.session_id();
        let short_id = &session_id[..session_id.len().min(12)];
        let provider = &app.config.default_provider;
        let model = &app.config.default_model;
        let fuel = app.governance.fuel.budget();

        println!();
        println!(
            "{}",
            "╭─────────────────────────────────────────────╮".cyan()
        );
        println!(
            "{}",
            format!(
                "│  {} v0.1.0                          │",
                "Nexus Code".bold()
            )
            .cyan()
        );
        println!(
            "{}",
            format!("│  Session: {}...                   │", short_id).cyan()
        );
        println!(
            "{}",
            format!(
                "│  Provider: {} / {}│",
                provider,
                Self::pad_right(model, 23)
            )
            .cyan()
        );
        println!(
            "{}",
            format!(
                "│  Fuel: {:>6} / {:>6}                      │",
                fuel.total - fuel.consumed,
                fuel.total
            )
            .cyan()
        );
        println!(
            "{}",
            format!(
                "│  Governance: {} Identity {} Audit {} ACL       │",
                "✓".green(),
                "✓".green(),
                "✓".green()
            )
            .cyan()
        );
        println!(
            "{}",
            "╰─────────────────────────────────────────────╯".cyan()
        );
        println!();
        println!(
            "  Type {} for commands, {} to exit.",
            "/help".bold(),
            "/quit".bold()
        );
        println!();
    }

    /// Pad or truncate a string to the right.
    fn pad_right(s: &str, width: usize) -> String {
        if s.len() >= width {
            s[..width].to_string()
        } else {
            format!("{}{}", s, " ".repeat(width - s.len()))
        }
    }

    /// Handle a slash command. Returns true if handled.
    async fn handle_slash_command(&mut self, input: &str) -> Result<bool, NxError> {
        let parts: Vec<&str> = input.splitn(2, ' ').collect();
        let cmd = parts[0];

        // Try Session 4 commands first (require mutable app access)
        {
            let mut app = self.app.lock().await;
            if let Some(result) = crate::commands::execute_command(input, &mut app).await {
                match result {
                    crate::commands::CommandResult::Output(msg) => {
                        println!("{}", msg);
                        return Ok(true);
                    }
                    crate::commands::CommandResult::Error(msg) => {
                        println!("{}: {}", "Error".red(), msg);
                        return Ok(true);
                    }
                    crate::commands::CommandResult::AgentPrompt(prompt) => {
                        drop(app);
                        if let Err(e) = self.process_message(&prompt).await {
                            println!("{}: {}", "Error".red(), e);
                        }
                        return Ok(true);
                    }
                    crate::commands::CommandResult::Silent => return Ok(true),
                }
            }
        }

        match cmd {
            "/quit" | "/exit" => {
                self.shutdown().await;
                std::process::exit(0);
            }
            "/help" => {
                println!();
                println!("{}", "Available commands:".bold());
                println!("  {}     — Show this help", "/help".cyan());
                println!("  {}   — Show governance status", "/status".cyan());
                println!("  {}     — Show fuel usage", "/cost".cyan());
                println!("  {} — List available providers", "/providers".cyan());
                println!(
                    "  {}  — Switch model (e.g. /model openai/gpt-4o)",
                    "/model".cyan()
                );
                println!(
                    "  {}     — Invoke a tool through governance pipeline",
                    "/tool".cyan()
                );
                println!("  {} — Create a governed git commit", "/commit".cyan());
                println!("  {}     — Show uncommitted changes", "/diff".cyan());
                println!("  {}     — Run project tests", "/test".cyan());
                println!("  {}      — Auto-fix last error", "/fix".cyan());
                println!("  {}  — Compact conversation context", "/compact".cyan());
                println!("  {}   — Search codebase", "/search".cyan());
                println!("  {}  — Save/list/restore sessions", "/session".cyan());
                println!("  {}  — Explain code or concept", "/explain".cyan());
                println!("  {}     — Plan then execute (dual-agent)", "/plan".cyan());
                println!("  {}  — Show context window usage", "/context".cyan());
                println!("  {}     — End session", "/quit".cyan());
                println!();
                Ok(true)
            }
            "/status" => {
                let app = self.app.lock().await;
                let identity = &app.governance.identity;
                let audit = &app.governance.audit;
                let fuel = app.governance.fuel.budget();
                let caps = app.governance.capabilities.granted();

                println!();
                println!("{}", "Governance Status".bold().underline());
                println!("  Session:    {}", identity.session_id());
                println!("  Public Key: {}", hex::encode(identity.public_key_bytes()));
                println!("  Audit Chain: {} entries", audit.len());
                if let Some(last) = audit.entries().last() {
                    println!("  Last Hash:  {}...", &last.entry_hash[..16]);
                }
                println!(
                    "  Fuel:       {} / {} ({:.1}% used)",
                    app.governance.fuel.remaining(),
                    fuel.total,
                    app.governance.fuel.usage_percentage()
                );
                println!("  Capabilities: {} granted", caps.len());
                println!();
                Ok(true)
            }
            "/cost" => {
                let app = self.app.lock().await;
                let fuel = app.governance.fuel.budget();
                let history = app.governance.fuel.cost_history();

                println!();
                println!("{}", "Fuel Usage".bold().underline());
                println!("  Total Budget:  {}", fuel.total);
                println!("  Consumed:      {}", fuel.consumed);
                println!("  Reserved:      {}", fuel.reserved);
                println!("  Remaining:     {}", app.governance.fuel.remaining());
                println!("  Est. Cost:     ${:.4}", fuel.cost_usd);
                if !history.is_empty() {
                    println!("  Requests:      {}", history.len());
                }
                println!();
                Ok(true)
            }
            "/providers" => {
                let app = self.app.lock().await;
                app.print_providers();
                Ok(true)
            }
            "/model" => {
                if let Some(model_spec) = parts.get(1) {
                    let spec_parts: Vec<&str> = model_spec.splitn(2, '/').collect();
                    if spec_parts.len() == 2 {
                        let mut app = self.app.lock().await;
                        let provider = spec_parts[0].to_string();
                        let model = spec_parts[1].to_string();
                        app.router.set_slot(
                            ModelSlot::Execution,
                            crate::llm::SlotConfig {
                                provider: provider.clone(),
                                model: model.clone(),
                            },
                        );
                        println!(
                            "  {} Switched to {}/{}",
                            "✓".green(),
                            provider.bold(),
                            model
                        );
                    } else {
                        println!("  {} Usage: /model <provider>/<model>", "✗".red());
                    }
                } else {
                    println!("  {} Usage: /model <provider>/<model>", "✗".red());
                }
                Ok(true)
            }
            "/tool" => {
                // Debug command: manually invoke a tool through governance pipeline
                // Usage: /tool file_read {"path": "src/main.rs"}
                // Usage: /tool  (lists all tools)
                if let Some(args) = parts.get(1) {
                    let tool_args: Vec<&str> = args.splitn(2, ' ').collect();
                    if tool_args.len() >= 2 {
                        let tool_name = tool_args[0].to_string();
                        match serde_json::from_str::<serde_json::Value>(tool_args[1]) {
                            Ok(input) => {
                                match self.handle_tool_call(&tool_name, input).await {
                                    Ok(result) => {
                                        let status = if result.is_success() {
                                            "✓".green()
                                        } else {
                                            "✗".red()
                                        };
                                        println!(
                                            "{} [{}ms] {}",
                                            status, result.duration_ms, result.output
                                        );
                                    }
                                    Err(crate::error::NxError::ConsentDenied { action }) => {
                                        println!("{} Consent denied for: {}", "✗".red(), action);
                                    }
                                    Err(e) => {
                                        println!("{} {}", "✗".red(), e);
                                    }
                                }
                                return Ok(true);
                            }
                            Err(e) => println!("{}: Invalid JSON: {}", "Error".red(), e),
                        }
                    } else if tool_args.len() == 1 && !tool_args[0].is_empty() {
                        println!(
                            "{}: /tool {} {{\"param\": \"value\"}}",
                            "Usage".yellow(),
                            tool_args[0]
                        );
                    } else {
                        println!(
                            "{}: /tool <tool_name> {{\"param\": \"value\"}}",
                            "Usage".yellow()
                        );
                    }
                } else {
                    // List available tools
                    let app = self.app.lock().await;
                    let tools = app.tool_registry.list();
                    println!("{}", "Available tools:".bold());
                    for name in tools {
                        println!("  {}", name);
                    }
                }
                Ok(true)
            }
            "/context" => {
                let measurement =
                    crate::context::ContextMeasurement::measure(&self.history, "system prompt");
                println!("{}", measurement.summary(200_000));
                Ok(true)
            }
            "/plan" => {
                if let Some(task) = parts.get(1) {
                    if let Err(e) = self
                        .process_message(&format!(
                            "Plan the following task step by step, then execute: {}",
                            task
                        ))
                        .await
                    {
                        println!("{}: {}", "Error".red(), e);
                    }
                } else {
                    println!("{}: /plan <task description>", "Usage".yellow());
                }
                Ok(true)
            }
            _ => {
                println!("  {} Unknown command: {}", "✗".red(), cmd);
                Ok(true)
            }
        }
    }

    /// Handle a tool call through the full governance pipeline,
    /// including interactive consent prompts for Tier2/3 operations.
    async fn handle_tool_call(
        &mut self,
        tool_name: &str,
        tool_input: serde_json::Value,
    ) -> Result<crate::tools::ToolResult, crate::error::NxError> {
        // Create a standalone tool instance to avoid borrow conflicts
        // between the immutable tool and mutable governance kernel.
        let tool = crate::tools::create_tool(tool_name).ok_or_else(|| {
            crate::error::NxError::ConfigError(format!("Unknown tool: {}", tool_name))
        })?;

        let tool_ctx = {
            let app = self.app.lock().await;
            crate::tools::ToolContext {
                working_dir: app
                    .config
                    .project_dir()
                    .unwrap_or_else(|| std::env::current_dir().unwrap_or_default()),
                blocked_paths: app.config.blocked_paths.clone(),
                max_file_scope: app.config.max_file_scope.clone(),
                non_interactive: false,
            }
        };

        // Try governed execution
        let result = {
            let mut app = self.app.lock().await;
            crate::tools::execute_governed(
                tool.as_ref(),
                tool_input.clone(),
                &tool_ctx,
                &mut app.governance,
            )
            .await
        };

        match result {
            Ok(result) => Ok(result),
            Err(crate::error::NxError::ConsentRequired { request }) => {
                // Tier2/3: prompt user for consent
                let tier_str = match request.tier {
                    ConsentTier::Tier1 => "Tier1 (auto)",
                    ConsentTier::Tier2 => "Tier2 (write)",
                    ConsentTier::Tier3 => "Tier3 (DESTRUCTIVE)",
                };

                println!();
                println!(
                    "{}",
                    format!("╭─ Consent Required ({}) ──────────────╮", tier_str).yellow()
                );
                println!("{}", format!("│ Tool:   {}", tool_name).yellow());
                println!("{}", format!("│ Action: {}", request.details).yellow());
                println!("{}", "╰──────────────────────────────────────╯".yellow());
                print!("{} ", "[A]pprove / [D]eny:".bold());
                use std::io::Write;
                std::io::stdout().flush().ok();

                let mut line = String::new();
                std::io::stdin().read_line(&mut line).ok();
                let granted = line.trim().to_lowercase().starts_with('a');

                if granted {
                    println!("{} Consent granted", "✓".green());
                } else {
                    println!("{} Consent denied", "✗".red());
                }

                // Finalize consent and execute if granted
                let mut app = self.app.lock().await;
                crate::tools::execute_after_consent(
                    tool.as_ref(),
                    tool_input,
                    &tool_ctx,
                    &mut app.governance,
                    &request,
                    granted,
                )
                .await
            }
            Err(e) => Err(e),
        }
    }

    /// Process a user message through the agent loop (LLM + tool execution).
    async fn process_message(&mut self, input: &str) -> Result<(), NxError> {
        self.history.push(Message {
            role: Role::User,
            content: input.to_string(),
        });

        // Build agent config
        let agent_config = crate::agent::AgentConfig {
            max_turns: 10,
            system_prompt: "You are Nexus Code, a governed terminal coding agent. \
                            Be concise and helpful. Use tools when needed to accomplish tasks."
                .to_string(),
            model_slot: crate::llm::ModelSlot::Execution,
            auto_approve_tier2: false,
            auto_approve_tier3: false,
        };

        // Create event channel
        let (event_tx, mut event_rx) = mpsc::unbounded_channel();

        // Create consent handler (interactive — prompts user)
        let consent_handler: std::sync::Arc<
            dyn Fn(&crate::governance::ConsentRequest) -> bool + Send + Sync,
        > = std::sync::Arc::new(|request| {
            let tier_str = match request.tier {
                ConsentTier::Tier1 => "Tier1",
                ConsentTier::Tier2 => "Tier2 (write)",
                ConsentTier::Tier3 => "Tier3 (DESTRUCTIVE)",
            };
            println!();
            println!(
                "{}",
                format!("╭─ Consent Required ({}) ─╮", tier_str).yellow()
            );
            println!("{}", format!("│ Tool: {}", request.action).yellow());
            println!("{}", format!("│ Details: {}", request.details).yellow());
            println!("{}", "╰─────────────────────────╯".yellow());
            print!("[A]pprove / [D]eny: ");
            use std::io::Write;
            std::io::stdout().flush().ok();

            let mut line = String::new();
            std::io::stdin().read_line(&mut line).ok();
            line.trim().to_lowercase().starts_with('a')
        });

        // Run agent loop — we need to hold the lock for the duration
        // since the agent loop needs mutable access to governance
        let cancel = tokio_util::sync::CancellationToken::new();
        let mut messages = self.history.clone();

        let mut app = self.app.lock().await;
        let tool_ctx = crate::tools::ToolContext {
            working_dir: app
                .config
                .project_dir()
                .unwrap_or_else(|| std::env::current_dir().unwrap_or_default()),
            blocked_paths: app.config.blocked_paths.clone(),
            max_file_scope: app.config.max_file_scope.clone(),
            non_interactive: false,
        };

        // Spawn event display task
        let display_max_turns = agent_config.max_turns;
        let display_handle = tokio::spawn(async move {
            while let Some(event) = event_rx.recv().await {
                match event {
                    crate::agent::AgentEvent::TextDelta(text) => {
                        print!("{}", text);
                        use std::io::Write;
                        std::io::stdout().flush().ok();
                    }
                    crate::agent::AgentEvent::ToolCallStart { name, .. } => {
                        println!("\n{}", format!("[Calling {}...]", name).dimmed());
                    }
                    crate::agent::AgentEvent::ToolCallComplete {
                        name,
                        success,
                        duration_ms,
                        summary,
                    } => {
                        let status = if success {
                            "✓".green().to_string()
                        } else {
                            "✗".red().to_string()
                        };
                        let display_summary = if summary.len() > 100 {
                            format!("{}...", &summary[..100])
                        } else {
                            summary
                        };
                        println!(
                            "{} {} ({}ms): {}",
                            status, name, duration_ms, display_summary
                        );
                    }
                    crate::agent::AgentEvent::ToolCallDenied { name, reason } => {
                        println!("{} {} denied: {}", "✗".red(), name, reason);
                    }
                    crate::agent::AgentEvent::TurnComplete { turn, .. } => {
                        println!(
                            "{}",
                            format!("[Turn {}/{}]", turn, display_max_turns).dimmed()
                        );
                    }
                    crate::agent::AgentEvent::TokenUsage {
                        input_tokens,
                        output_tokens,
                    } => {
                        tracing::debug!("Tokens: {} in / {} out", input_tokens, output_tokens);
                    }
                    crate::agent::AgentEvent::Done {
                        reason,
                        total_turns,
                    } => {
                        if total_turns > 1 || reason != "end_turn" {
                            println!(
                                "{}",
                                format!("\n[Done: {} ({} turns)]", reason, total_turns).dimmed()
                            );
                        }
                    }
                    crate::agent::AgentEvent::Error(msg) => {
                        println!("\n{}: {}", "Agent error".red(), msg);
                    }
                }
            }
        });

        // Reborrow through the MutexGuard to allow split borrows
        // of separate fields (router, tool_registry, governance).
        let app = &mut *app;
        let result = crate::agent::run_agent_loop(
            &mut messages,
            &app.router,
            &app.tool_registry,
            &tool_ctx,
            &mut app.governance,
            &agent_config,
            event_tx,
            consent_handler,
            cancel,
        )
        .await;

        // Wait for display to finish
        let _ = display_handle.await;
        println!();

        match result {
            Ok(_final_text) => {
                self.history = messages;
            }
            Err(e) => {
                eprintln!("{}: {}", "Agent loop error".red(), e);
            }
        }

        Ok(())
    }

    /// Shut down the session gracefully.
    async fn shutdown(&self) {
        let mut app = self.app.lock().await;
        app.governance.end_session("user exit");
        println!("\n{}", "Session ended. Audit trail sealed.".dimmed());
    }
}
