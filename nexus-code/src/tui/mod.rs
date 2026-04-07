//! Ratatui terminal UI — production-grade interactive experience.
//!
//! Replaces the rustyline REPL as the default interactive mode.
//! The rustyline REPL is still available via `nx chat --no-tui`.

pub mod chat_panel;
pub mod computer_use_panel;
pub mod consent_modal;
pub mod governance_panel;
pub mod help_overlay;
pub mod input_area;
pub mod keybindings;
pub mod layout;
pub mod markdown;
pub mod patterns_panel;
pub mod status_bar;
pub mod theme;
pub mod tool_activity;

use crossterm::event::{self, Event};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use std::io::Stdout;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};

/// A message displayed in the chat panel.
#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub role: MessageRole,
    pub content: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Role of a chat message.
#[derive(Debug, Clone, PartialEq)]
pub enum MessageRole {
    User,
    Assistant,
    Tool {
        name: String,
        success: bool,
        duration_ms: u64,
    },
    System,
}

/// Pending consent request displayed as a modal.
#[derive(Debug)]
pub struct PendingConsent {
    pub request: crate::governance::ConsentRequest,
    pub tool_name: String,
    pub response_tx: Option<tokio::sync::oneshot::Sender<bool>>,
}

/// Active tool execution entry.
#[derive(Debug, Clone)]
pub struct ToolActivityEntry {
    pub name: String,
    pub started_at: chrono::DateTime<chrono::Utc>,
    pub status: ToolActivityStatus,
}

/// Tool execution status.
#[derive(Debug, Clone, PartialEq)]
pub enum ToolActivityStatus {
    Running,
    Completed { success: bool, duration_ms: u64 },
    Denied { reason: String },
}

/// A learned pattern for the patterns panel.
#[derive(Debug, Clone)]
pub struct PatternEntry {
    pub name: String,
    pub confidence: f64,
}

/// A recent run entry for the patterns panel.
#[derive(Debug, Clone)]
pub struct RecentRun {
    pub name: String,
    pub success: bool,
    pub duration_ms: u64,
}

/// The TUI application state.
pub struct TuiApp {
    pub messages: Vec<ChatMessage>,
    pub streaming_text: String,
    pub is_streaming: bool,
    pub input: String,
    pub cursor_pos: usize,
    pub input_history: Vec<String>,
    pub history_index: Option<usize>,
    pub scroll_offset: u16,
    pub pending_consent: Option<PendingConsent>,
    pub active_tools: Vec<ToolActivityEntry>,
    pub show_help: bool,
    pub active_panel: layout::BottomPanel,
    pub should_quit: bool,
    pub status_message: Option<(String, chrono::DateTime<chrono::Utc>)>,

    // Governance state (synced from App)
    pub session_id_short: String,
    pub public_key_short: String,
    pub provider: String,
    pub model: String,
    pub fuel_remaining: u64,
    pub fuel_total: u64,
    pub audit_len: usize,
    pub envelope_similarity: f64,
    pub tool_count: usize,

    // Computer use state
    pub computer_use_active: bool,
    pub computer_use_backend: String,
    pub computer_use_resolution: String,
    pub last_screenshot_hash: String,
    pub cu_screenshots: u32,
    pub cu_interactions: u32,
    pub cu_analyses: u32,
    pub cu_fixes_verified: u32,
    pub cu_fixes_total: u32,
    pub cu_current_page: String,
    pub cu_last_action: String,
    pub cu_last_action_time: String,

    // Patterns state
    pub patterns: Vec<PatternEntry>,
    pub recent_runs: Vec<RecentRun>,

    // Memory state
    pub memory_entries: usize,
    pub memory_fuel_used: u64,
}

impl TuiApp {
    pub fn new(app: &crate::app::App) -> Self {
        let session_id = app.governance.identity.session_id();
        let pubkey = hex::encode(app.governance.identity.public_key_bytes());
        Self {
            messages: Vec::new(),
            streaming_text: String::new(),
            is_streaming: false,
            input: String::new(),
            cursor_pos: 0,
            input_history: Vec::new(),
            history_index: None,
            scroll_offset: 0,
            pending_consent: None,
            active_tools: Vec::new(),
            show_help: false,
            active_panel: layout::BottomPanel::None,
            should_quit: false,
            status_message: None,
            session_id_short: session_id[..8.min(session_id.len())].to_string(),
            public_key_short: if pubkey.len() >= 16 {
                format!("{}...{}", &pubkey[..8], &pubkey[pubkey.len() - 8..])
            } else {
                pubkey
            },
            provider: app.config.default_provider.clone(),
            model: app.config.default_model.clone(),
            fuel_remaining: app.governance.fuel.remaining(),
            fuel_total: app.governance.fuel.budget().total,
            audit_len: app.governance.audit.len(),
            envelope_similarity: 100.0,
            tool_count: app.tool_registry.list().len(),
            computer_use_active: app.is_computer_use_active(),
            computer_use_backend: "none".to_string(),
            computer_use_resolution: "n/a".to_string(),
            last_screenshot_hash: "n/a".to_string(),
            cu_screenshots: 0,
            cu_interactions: 0,
            cu_analyses: 0,
            cu_fixes_verified: 0,
            cu_fixes_total: 0,
            cu_current_page: "n/a".to_string(),
            cu_last_action: "n/a".to_string(),
            cu_last_action_time: "n/a".to_string(),
            patterns: Vec::new(),
            recent_runs: Vec::new(),
            memory_entries: app.memory.len(),
            memory_fuel_used: 0,
        }
    }

    /// Take the current input and clear the buffer.
    pub fn take_input(&mut self) -> String {
        let input = self.input.clone();
        if !input.is_empty() {
            self.input_history.push(input.clone());
        }
        self.input.clear();
        self.cursor_pos = 0;
        self.history_index = None;
        input
    }

    /// Update governance state from App.
    pub fn sync_governance(&mut self, app: &crate::app::App) {
        self.fuel_remaining = app.governance.fuel.remaining();
        self.fuel_total = app.governance.fuel.budget().total;
        self.audit_len = app.governance.audit.len();
        self.memory_entries = app.memory.len();
        self.tool_count = app.tool_registry.list().len();
        self.computer_use_active = app.is_computer_use_active();
    }

    /// Finalize streaming — move streaming_text into a completed message.
    pub fn finalize_streaming(&mut self) {
        if !self.streaming_text.is_empty() {
            self.messages.push(ChatMessage {
                role: MessageRole::Assistant,
                content: std::mem::take(&mut self.streaming_text),
                timestamp: chrono::Utc::now(),
            });
        }
        self.is_streaming = false;
    }

    /// Toggle a bottom panel. If already showing that panel, close it.
    pub fn toggle_panel(&mut self, panel: layout::BottomPanel) {
        if self.active_panel == panel {
            self.active_panel = layout::BottomPanel::None;
        } else {
            self.active_panel = panel;
        }
    }

    /// Record a completed run for the patterns panel.
    pub fn record_run(&mut self, name: String, success: bool, duration_ms: u64) {
        self.recent_runs.push(RecentRun {
            name,
            success,
            duration_ms,
        });
        // Keep only last 10 runs
        if self.recent_runs.len() > 10 {
            self.recent_runs.remove(0);
        }
    }

    /// Handle a key event for input editing.
    pub fn handle_key(&mut self, key: event::KeyEvent) {
        use crossterm::event::KeyCode;
        match key.code {
            KeyCode::Char(c)
                if !key
                    .modifiers
                    .contains(crossterm::event::KeyModifiers::CONTROL) =>
            {
                self.input.insert(self.cursor_pos, c);
                self.cursor_pos += 1;
            }
            KeyCode::Backspace => {
                if self.cursor_pos > 0 {
                    self.cursor_pos -= 1;
                    self.input.remove(self.cursor_pos);
                }
            }
            KeyCode::Delete => {
                if self.cursor_pos < self.input.len() {
                    self.input.remove(self.cursor_pos);
                }
            }
            KeyCode::Left => {
                self.cursor_pos = self.cursor_pos.saturating_sub(1);
            }
            KeyCode::Right => {
                self.cursor_pos = (self.cursor_pos + 1).min(self.input.len());
            }
            KeyCode::Home => {
                self.cursor_pos = 0;
            }
            KeyCode::End => {
                self.cursor_pos = self.input.len();
            }
            KeyCode::Up => {
                if let Some(idx) = self.history_index {
                    if idx > 0 {
                        self.history_index = Some(idx - 1);
                        self.input = self.input_history[idx - 1].clone();
                        self.cursor_pos = self.input.len();
                    }
                } else if !self.input_history.is_empty() {
                    self.history_index = Some(self.input_history.len() - 1);
                    self.input = self.input_history.last().cloned().unwrap_or_default();
                    self.cursor_pos = self.input.len();
                }
            }
            KeyCode::Down => {
                if let Some(idx) = self.history_index {
                    if idx < self.input_history.len() - 1 {
                        self.history_index = Some(idx + 1);
                        self.input = self.input_history[idx + 1].clone();
                    } else {
                        self.history_index = None;
                        self.input.clear();
                    }
                    self.cursor_pos = self.input.len();
                }
            }
            _ => {}
        }
    }
}

// ─── Terminal Setup/Restore ───

pub fn setup_terminal() -> Result<Terminal<CrosstermBackend<Stdout>>, crate::error::NxError> {
    crossterm::terminal::enable_raw_mode().map_err(crate::error::NxError::Io)?;
    let mut stdout = std::io::stdout();
    crossterm::execute!(
        stdout,
        crossterm::terminal::EnterAlternateScreen,
        crossterm::event::EnableMouseCapture,
    )
    .map_err(crate::error::NxError::Io)?;
    let backend = CrosstermBackend::new(stdout);
    Terminal::new(backend).map_err(crate::error::NxError::Io)
}

pub fn restore_terminal(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
) -> Result<(), crate::error::NxError> {
    crossterm::terminal::disable_raw_mode().map_err(crate::error::NxError::Io)?;
    crossterm::execute!(
        terminal.backend_mut(),
        crossterm::terminal::LeaveAlternateScreen,
        crossterm::event::DisableMouseCapture,
    )
    .map_err(crate::error::NxError::Io)?;
    terminal.show_cursor().map_err(crate::error::NxError::Io)?;
    Ok(())
}

// ─── Main TUI Event Loop ───

/// Run the TUI interactive mode.
pub async fn run_tui(app: Arc<Mutex<crate::app::App>>) -> Result<(), crate::error::NxError> {
    let mut terminal = setup_terminal()?;

    let tui_state = {
        let app_lock = app.lock().await;
        TuiApp::new(&app_lock)
    };
    let tui_state = Arc::new(Mutex::new(tui_state));

    let (_agent_event_tx, mut agent_event_rx) =
        mpsc::unbounded_channel::<crate::agent::AgentEvent>();

    let result = run_event_loop(&mut terminal, tui_state, app, &mut agent_event_rx).await;

    restore_terminal(&mut terminal)?;
    result
}

async fn run_event_loop(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    tui_state: Arc<Mutex<TuiApp>>,
    app: Arc<Mutex<crate::app::App>>,
    agent_event_rx: &mut mpsc::UnboundedReceiver<crate::agent::AgentEvent>,
) -> Result<(), crate::error::NxError> {
    use std::time::Duration;

    loop {
        // 1. Draw
        {
            let state = tui_state.lock().await;
            terminal
                .draw(|frame| {
                    layout::draw(frame, &state);
                })
                .map_err(crate::error::NxError::Io)?;
        }

        // 2. Poll for terminal events (~60fps)
        if crossterm::event::poll(Duration::from_millis(16)).map_err(crate::error::NxError::Io)? {
            let evt = event::read().map_err(crate::error::NxError::Io)?;
            let mut state = tui_state.lock().await;

            if let Event::Key(key) = evt {
                let action = keybindings::process_key(
                    key,
                    state.is_streaming,
                    state.pending_consent.is_some(),
                    state.show_help,
                );

                match action {
                    keybindings::KeyAction::Quit => {
                        state.should_quit = true;
                    }
                    keybindings::KeyAction::CancelStream => {
                        state.finalize_streaming();
                    }
                    keybindings::KeyAction::CloseOverlay => {
                        if state.show_help {
                            state.show_help = false;
                        } else if let Some(mut consent) = state.pending_consent.take() {
                            if let Some(tx) = consent.response_tx.take() {
                                let _ = tx.send(false);
                            }
                        }
                    }
                    keybindings::KeyAction::ToggleHelp => {
                        state.show_help = !state.show_help;
                    }
                    keybindings::KeyAction::ToggleGovernance => {
                        state.toggle_panel(layout::BottomPanel::Governance);
                    }
                    keybindings::KeyAction::ToggleComputerUse => {
                        state.toggle_panel(layout::BottomPanel::ComputerUse);
                    }
                    keybindings::KeyAction::TogglePatterns => {
                        state.toggle_panel(layout::BottomPanel::Patterns);
                    }
                    keybindings::KeyAction::ToggleMemory => {
                        state.toggle_panel(layout::BottomPanel::Memory);
                    }
                    keybindings::KeyAction::ClearConversation => {
                        state.messages.clear();
                        state.scroll_offset = 0;
                    }
                    keybindings::KeyAction::Submit => {
                        let input = state.take_input();
                        if !input.is_empty() {
                            if input == "/quit" || input == "/exit" {
                                state.should_quit = true;
                            } else if input == "/help" {
                                state.show_help = true;
                            } else if input == "/status" || input == "/cost" {
                                let app_lock = app.lock().await;
                                state.sync_governance(&app_lock);
                                let usage_pct = app_lock.governance.fuel.usage_percentage();
                                drop(app_lock);
                                let msg = format!(
                                    "Session: {}\nProvider: {}/{}\nFuel: {}/{} ({:.1}%)\nAudit: {} entries",
                                    state.session_id_short,
                                    state.provider, state.model,
                                    state.fuel_remaining, state.fuel_total,
                                    usage_pct,
                                    state.audit_len,
                                );
                                state.messages.push(ChatMessage {
                                    role: MessageRole::System,
                                    content: msg,
                                    timestamp: chrono::Utc::now(),
                                });
                            } else {
                                state.messages.push(ChatMessage {
                                    role: MessageRole::User,
                                    content: input.clone(),
                                    timestamp: chrono::Utc::now(),
                                });
                                state.is_streaming = true;
                                state.scroll_offset = 0;

                                let app_clone = app.clone();
                                let tui_clone = tui_state.clone();
                                tokio::spawn(async move {
                                    handle_user_input(&input, app_clone, tui_clone).await;
                                });
                            }
                        }
                    }
                    keybindings::KeyAction::ApproveConsent => {
                        if let Some(mut consent) = state.pending_consent.take() {
                            if let Some(tx) = consent.response_tx.take() {
                                let _ = tx.send(true);
                            }
                        }
                    }
                    keybindings::KeyAction::DenyConsent => {
                        if let Some(mut consent) = state.pending_consent.take() {
                            if let Some(tx) = consent.response_tx.take() {
                                let _ = tx.send(false);
                            }
                        }
                    }
                    keybindings::KeyAction::TabComplete => {
                        if let Some(completed) = input_area::tab_complete(&state.input) {
                            state.input = completed;
                            state.cursor_pos = state.input.len();
                        }
                    }
                    keybindings::KeyAction::ScrollUp(n) => {
                        state.scroll_offset = state.scroll_offset.saturating_add(n);
                    }
                    keybindings::KeyAction::ScrollDown(n) => {
                        state.scroll_offset = state.scroll_offset.saturating_sub(n);
                    }
                    keybindings::KeyAction::InputKey(k) => {
                        state.handle_key(k);
                    }
                    keybindings::KeyAction::Paste | keybindings::KeyAction::Handled => {
                        // Paste: crossterm delivers pasted text as individual Char events,
                        // so Shift+Ctrl+V is handled by the terminal itself.
                    }
                    keybindings::KeyAction::None => {}
                }
            }

            if state.should_quit {
                let mut app_lock = app.lock().await;
                app_lock.governance.end_session("user exit");
                break;
            }
        }

        // 3. Drain agent events
        while let Ok(event) = agent_event_rx.try_recv() {
            let mut state = tui_state.lock().await;
            match event {
                crate::agent::AgentEvent::TextDelta(text) => {
                    state.streaming_text.push_str(&text);
                }
                crate::agent::AgentEvent::ToolCallStart { name, .. } => {
                    state.active_tools.push(ToolActivityEntry {
                        name,
                        started_at: chrono::Utc::now(),
                        status: ToolActivityStatus::Running,
                    });
                }
                crate::agent::AgentEvent::ToolCallComplete {
                    name,
                    success,
                    duration_ms,
                    summary,
                } => {
                    if let Some(entry) = state
                        .active_tools
                        .iter_mut()
                        .rev()
                        .find(|t| t.name == name && t.status == ToolActivityStatus::Running)
                    {
                        entry.status = ToolActivityStatus::Completed {
                            success,
                            duration_ms,
                        };
                    }
                    state.record_run(name.clone(), success, duration_ms);
                    state.messages.push(ChatMessage {
                        role: MessageRole::Tool {
                            name,
                            success,
                            duration_ms,
                        },
                        content: summary,
                        timestamp: chrono::Utc::now(),
                    });
                }
                crate::agent::AgentEvent::ToolCallDenied { name, reason } => {
                    if let Some(entry) = state
                        .active_tools
                        .iter_mut()
                        .rev()
                        .find(|t| t.name == name && t.status == ToolActivityStatus::Running)
                    {
                        entry.status = ToolActivityStatus::Denied {
                            reason: reason.clone(),
                        };
                    }
                    state.messages.push(ChatMessage {
                        role: MessageRole::Tool {
                            name,
                            success: false,
                            duration_ms: 0,
                        },
                        content: format!("Denied: {}", reason),
                        timestamp: chrono::Utc::now(),
                    });
                }
                crate::agent::AgentEvent::Done {
                    reason,
                    total_turns,
                } => {
                    state.finalize_streaming();
                    state.status_message = Some((
                        format!("Done: {} ({} turns)", reason, total_turns),
                        chrono::Utc::now(),
                    ));
                    let app_lock = app.lock().await;
                    state.sync_governance(&app_lock);
                }
                crate::agent::AgentEvent::TokenUsage { .. } => {
                    if let Ok(app_lock) = app.try_lock() {
                        state.sync_governance(&app_lock);
                    }
                }
                crate::agent::AgentEvent::Error(msg) => {
                    state.finalize_streaming();
                    state.messages.push(ChatMessage {
                        role: MessageRole::System,
                        content: format!("Error: {}", msg),
                        timestamp: chrono::Utc::now(),
                    });
                }
                _ => {}
            }
        }
    }

    Ok(())
}

/// Handle user input — dispatch to slash commands or agent loop.
async fn handle_user_input(
    input: &str,
    app: Arc<Mutex<crate::app::App>>,
    tui_state: Arc<Mutex<TuiApp>>,
) {
    if input.starts_with('/') {
        handle_slash_command(input, app, tui_state.clone()).await;
        let mut state = tui_state.lock().await;
        state.is_streaming = false;
    } else {
        run_agent_for_input(input, app, tui_state).await;
    }
}

/// Run the agent loop for a user message.
async fn run_agent_for_input(
    input: &str,
    app: Arc<Mutex<crate::app::App>>,
    tui_state: Arc<Mutex<TuiApp>>,
) {
    let (event_tx, mut event_rx) = mpsc::unbounded_channel();

    // Consent handler that posts to TUI state
    let tui_for_consent = tui_state.clone();
    let consent_handler: Arc<dyn Fn(&crate::governance::ConsentRequest) -> bool + Send + Sync> =
        Arc::new(move |request| {
            let (tx, rx) = tokio::sync::oneshot::channel();
            {
                let tui = tui_for_consent.clone();
                let req = request.clone();
                let rt = tokio::runtime::Handle::current();
                rt.block_on(async {
                    let mut state = tui.lock().await;
                    state.pending_consent = Some(PendingConsent {
                        request: req.clone(),
                        tool_name: req.action.clone(),
                        response_tx: Some(tx),
                    });
                });
            }
            rx.blocking_recv().unwrap_or(false)
        });

    let mut app_lock = app.lock().await;
    let mut messages = vec![crate::llm::types::Message {
        role: crate::llm::types::Role::User,
        content: input.to_string(),
    }];

    let agent_config = crate::agent::AgentConfig {
        max_turns: 10,
        system_prompt: "You are Nexus Code, a governed terminal coding agent. Be concise."
            .to_string(),
        model_slot: crate::llm::router::ModelSlot::Execution,
        auto_approve_tier2: false,
        auto_approve_tier3: false,
        computer_use_active: app_lock.is_computer_use_active(),
    };

    let tool_ctx = crate::tools::ToolContext {
        working_dir: app_lock
            .config
            .project_dir()
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_default()),
        blocked_paths: app_lock.config.blocked_paths.clone(),
        max_file_scope: app_lock.config.max_file_scope.clone(),
        non_interactive: false,
    };

    let cancel = tokio_util::sync::CancellationToken::new();
    let app_ref = &mut *app_lock;

    let _ = crate::agent::run_agent_loop(
        &mut messages,
        &app_ref.router,
        &app_ref.tool_registry,
        &tool_ctx,
        &mut app_ref.governance,
        &agent_config,
        event_tx,
        consent_handler,
        cancel,
    )
    .await;

    // Drain events into TUI state
    while let Ok(event) = event_rx.try_recv() {
        let mut state = tui_state.lock().await;
        match event {
            crate::agent::AgentEvent::TextDelta(text) => {
                state.streaming_text.push_str(&text);
            }
            crate::agent::AgentEvent::Done { .. } => {
                state.finalize_streaming();
                state.sync_governance(app_ref);
            }
            crate::agent::AgentEvent::ToolCallComplete {
                name,
                success,
                duration_ms,
                summary,
            } => {
                state.record_run(name.clone(), success, duration_ms);
                state.messages.push(ChatMessage {
                    role: MessageRole::Tool {
                        name,
                        success,
                        duration_ms,
                    },
                    content: summary,
                    timestamp: chrono::Utc::now(),
                });
            }
            crate::agent::AgentEvent::Error(msg) => {
                state.finalize_streaming();
                state.messages.push(ChatMessage {
                    role: MessageRole::System,
                    content: format!("Error: {}", msg),
                    timestamp: chrono::Utc::now(),
                });
            }
            _ => {}
        }
    }

    // Final sync
    let mut state = tui_state.lock().await;
    state.finalize_streaming();
    state.sync_governance(app_ref);
}

/// Handle slash commands from TUI.
async fn handle_slash_command(
    input: &str,
    app: Arc<Mutex<crate::app::App>>,
    tui_state: Arc<Mutex<TuiApp>>,
) {
    let mut app_lock = app.lock().await;

    if let Some(result) = crate::commands::execute_command(input, &mut app_lock).await {
        let mut state = tui_state.lock().await;
        match result {
            crate::commands::CommandResult::Output(msg) => {
                state.messages.push(ChatMessage {
                    role: MessageRole::System,
                    content: msg,
                    timestamp: chrono::Utc::now(),
                });
            }
            crate::commands::CommandResult::Error(msg) => {
                state.messages.push(ChatMessage {
                    role: MessageRole::System,
                    content: format!("Error: {}", msg),
                    timestamp: chrono::Utc::now(),
                });
            }
            crate::commands::CommandResult::AgentPrompt(prompt) => {
                state.messages.push(ChatMessage {
                    role: MessageRole::System,
                    content: format!("Running: {}", &prompt[..60.min(prompt.len())]),
                    timestamp: chrono::Utc::now(),
                });
                state.is_streaming = true;
                drop(state);
                drop(app_lock);
                run_agent_for_input(&prompt, app, tui_state).await;
            }
            crate::commands::CommandResult::Silent => {}
        }
    } else {
        let mut state = tui_state.lock().await;
        state.messages.push(ChatMessage {
            role: MessageRole::System,
            content: "Unknown command. Press F1 for help.".to_string(),
            timestamp: chrono::Utc::now(),
        });
    }
}
