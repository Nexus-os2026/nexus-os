//! Session 6 tests — TUI state, markdown rendering, theme, chat messages.

use nexus_code::tui::markdown::render_markdown;
use nexus_code::tui::theme::Theme;
use nexus_code::tui::{ChatMessage, MessageRole, ToolActivityEntry, ToolActivityStatus, TuiApp};

// ═══════════════════════════════════════════════════════
// TuiApp State Tests (8)
// ═══════════════════════════════════════════════════════

#[test]
fn test_tui_app_creation() {
    let app = nexus_code::app::App::new(nexus_code::config::NxConfig::default()).unwrap();
    let tui = TuiApp::new(&app);

    assert!(tui.messages.is_empty());
    assert!(tui.streaming_text.is_empty());
    assert!(!tui.is_streaming);
    assert!(tui.input.is_empty());
    assert_eq!(tui.cursor_pos, 0);
    assert!(!tui.should_quit);
    assert!(!tui.show_help);
    assert_eq!(tui.active_panel, nexus_code::tui::layout::BottomPanel::None);
    assert!(tui.fuel_total > 0);
    assert!(tui.tool_count > 0);
}

#[test]
fn test_tui_take_input() {
    let app = nexus_code::app::App::new(nexus_code::config::NxConfig::default()).unwrap();
    let mut tui = TuiApp::new(&app);

    tui.input = "hello world".to_string();
    tui.cursor_pos = 11;

    let taken = tui.take_input();
    assert_eq!(taken, "hello world");
    assert!(tui.input.is_empty());
    assert_eq!(tui.cursor_pos, 0);
    assert_eq!(tui.input_history.len(), 1);
    assert_eq!(tui.input_history[0], "hello world");
}

#[test]
fn test_tui_take_empty_input() {
    let app = nexus_code::app::App::new(nexus_code::config::NxConfig::default()).unwrap();
    let mut tui = TuiApp::new(&app);

    let taken = tui.take_input();
    assert!(taken.is_empty());
    assert!(tui.input_history.is_empty()); // Empty input not added to history
}

#[test]
fn test_tui_finalize_streaming() {
    let app = nexus_code::app::App::new(nexus_code::config::NxConfig::default()).unwrap();
    let mut tui = TuiApp::new(&app);

    tui.is_streaming = true;
    tui.streaming_text = "Hello from the assistant".to_string();

    tui.finalize_streaming();

    assert!(!tui.is_streaming);
    assert!(tui.streaming_text.is_empty());
    assert_eq!(tui.messages.len(), 1);
    assert_eq!(tui.messages[0].role, MessageRole::Assistant);
    assert_eq!(tui.messages[0].content, "Hello from the assistant");
}

#[test]
fn test_tui_finalize_empty_streaming() {
    let app = nexus_code::app::App::new(nexus_code::config::NxConfig::default()).unwrap();
    let mut tui = TuiApp::new(&app);

    tui.is_streaming = true;
    tui.finalize_streaming();

    assert!(!tui.is_streaming);
    assert!(tui.messages.is_empty()); // No message created for empty stream
}

#[test]
fn test_tui_sync_governance() {
    let app = nexus_code::app::App::new(nexus_code::config::NxConfig::default()).unwrap();
    let mut tui = TuiApp::new(&app);

    // Initial state matches
    assert_eq!(tui.fuel_total, app.governance.fuel.budget().total);
    assert_eq!(tui.fuel_remaining, app.governance.fuel.remaining());
    assert_eq!(tui.audit_len, app.governance.audit.len());

    // After sync, values are consistent
    tui.sync_governance(&app);
    assert_eq!(tui.fuel_total, app.governance.fuel.budget().total);
}

#[test]
fn test_tui_scroll() {
    let app = nexus_code::app::App::new(nexus_code::config::NxConfig::default()).unwrap();
    let mut tui = TuiApp::new(&app);

    assert_eq!(tui.scroll_offset, 0);

    // Scroll via keybindings module (PageUp/PageDown now handled there)
    tui.scroll_offset = tui.scroll_offset.saturating_add(10);
    assert_eq!(tui.scroll_offset, 10);

    tui.scroll_offset = tui.scroll_offset.saturating_sub(10);
    assert_eq!(tui.scroll_offset, 0);
}

#[test]
fn test_tui_input_editing() {
    let app = nexus_code::app::App::new(nexus_code::config::NxConfig::default()).unwrap();
    let mut tui = TuiApp::new(&app);

    // Type "abc"
    for c in ['a', 'b', 'c'] {
        tui.handle_key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Char(c),
            crossterm::event::KeyModifiers::NONE,
        ));
    }
    assert_eq!(tui.input, "abc");
    assert_eq!(tui.cursor_pos, 3);

    // Backspace
    tui.handle_key(crossterm::event::KeyEvent::new(
        crossterm::event::KeyCode::Backspace,
        crossterm::event::KeyModifiers::NONE,
    ));
    assert_eq!(tui.input, "ab");
    assert_eq!(tui.cursor_pos, 2);

    // Home
    tui.handle_key(crossterm::event::KeyEvent::new(
        crossterm::event::KeyCode::Home,
        crossterm::event::KeyModifiers::NONE,
    ));
    assert_eq!(tui.cursor_pos, 0);

    // End
    tui.handle_key(crossterm::event::KeyEvent::new(
        crossterm::event::KeyCode::End,
        crossterm::event::KeyModifiers::NONE,
    ));
    assert_eq!(tui.cursor_pos, 2);
}

// ═══════════════════════════════════════════════════════
// Markdown Rendering Tests (6)
// ═══════════════════════════════════════════════════════

#[test]
fn test_markdown_plain_text() {
    let lines = render_markdown("Hello world");
    assert_eq!(lines.len(), 1);
}

#[test]
fn test_markdown_code_block() {
    let md = "```rust\nfn main() {}\nlet x = 1;\n```";
    let lines = render_markdown(md);
    assert_eq!(lines.len(), 2); // Two code lines
}

#[test]
fn test_markdown_headers() {
    let md = "# H1\n## H2\n### H3";
    let lines = render_markdown(md);
    assert_eq!(lines.len(), 3);
}

#[test]
fn test_markdown_list() {
    let md = "- item 1\n- item 2\n* item 3";
    let lines = render_markdown(md);
    assert_eq!(lines.len(), 3);
}

#[test]
fn test_markdown_mixed() {
    let md = "# Title\n\nSome text with `code`.\n\n- list item\n\n```\ncode block\n```";
    let lines = render_markdown(md);
    assert!(lines.len() >= 4);
}

#[test]
fn test_markdown_empty() {
    let lines = render_markdown("");
    assert!(lines.is_empty());
}

// ═══════════════════════════════════════════════════════
// Theme Tests (3)
// ═══════════════════════════════════════════════════════

#[test]
fn test_theme_styles_exist() {
    // Verify all style constructors don't panic
    let _ = Theme::title();
    let _ = Theme::text();
    let _ = Theme::dim();
    let _ = Theme::muted();
    let _ = Theme::success();
    let _ = Theme::error();
    let _ = Theme::warning();
    let _ = Theme::bold();
    let _ = Theme::code();
    let _ = Theme::user_label();
    let _ = Theme::assistant_label();
    let _ = Theme::info();
    let _ = Theme::status_bar();
    let _ = Theme::panel_border();
    let _ = Theme::panel_bg();
    let _ = Theme::fuel_style(50);
    let _ = Theme::envelope_style(75.0);
}

#[test]
fn test_theme_colors_defined() {
    assert_ne!(Theme::BRAND, ratatui::style::Color::Reset);
    assert_ne!(Theme::SUCCESS, ratatui::style::Color::Reset);
    assert_ne!(Theme::ERROR, ratatui::style::Color::Reset);
}

#[test]
fn test_theme_code_style_has_bg() {
    let style = Theme::code();
    // Code style should have a background color
    assert!(style.bg.is_some());
}

// ═══════════════════════════════════════════════════════
// Chat Message + Tool Activity Tests (4)
// ═══════════════════════════════════════════════════════

#[test]
fn test_chat_message_creation() {
    let msg = ChatMessage {
        role: MessageRole::User,
        content: "Hello".to_string(),
        timestamp: chrono::Utc::now(),
    };
    assert_eq!(msg.role, MessageRole::User);
    assert_eq!(msg.content, "Hello");
}

#[test]
fn test_message_role_variants() {
    assert_eq!(MessageRole::User, MessageRole::User);
    assert_ne!(MessageRole::User, MessageRole::Assistant);
    assert_ne!(MessageRole::User, MessageRole::System);
}

#[test]
fn test_tool_activity_entry() {
    let entry = ToolActivityEntry {
        name: "file_read".to_string(),
        started_at: chrono::Utc::now(),
        status: ToolActivityStatus::Running,
    };
    assert_eq!(entry.name, "file_read");
    assert_eq!(entry.status, ToolActivityStatus::Running);
}

#[test]
fn test_tool_activity_status_variants() {
    assert_eq!(ToolActivityStatus::Running, ToolActivityStatus::Running);
    assert_ne!(
        ToolActivityStatus::Running,
        ToolActivityStatus::Completed {
            success: true,
            duration_ms: 0
        }
    );
}

// ═══════════════════════════════════════════════════════
// New TUI v2 Tests — Panels, Keybindings, Fuel Bar (10)
// ═══════════════════════════════════════════════════════

#[test]
fn test_fuel_bar_gradient() {
    let (bar, _color) = Theme::fuel_bar(100);
    assert_eq!(bar.chars().count(), 10); // 10 blocks total

    let (bar, _) = Theme::fuel_bar(50);
    assert_eq!(bar.chars().count(), 10);

    let (bar, _) = Theme::fuel_bar(0);
    assert_eq!(bar.chars().count(), 10);
}

#[test]
fn test_fuel_color_gradient() {
    assert_eq!(Theme::fuel_color(100), Theme::FUEL_FULL);
    assert_eq!(Theme::fuel_color(75), Theme::FUEL_FULL);
    assert_eq!(Theme::fuel_color(60), Theme::FUEL_MEDIUM);
    assert_eq!(Theme::fuel_color(30), Theme::FUEL_LOW);
    assert_eq!(Theme::fuel_color(10), Theme::FUEL_CRITICAL);
}

#[test]
fn test_panel_toggle() {
    use nexus_code::tui::layout::BottomPanel;

    let app = nexus_code::app::App::new(nexus_code::config::NxConfig::default()).unwrap();
    let mut tui = TuiApp::new(&app);

    assert_eq!(tui.active_panel, BottomPanel::None);

    // Toggle governance on
    tui.toggle_panel(BottomPanel::Governance);
    assert_eq!(tui.active_panel, BottomPanel::Governance);

    // Toggle governance off (same panel)
    tui.toggle_panel(BottomPanel::Governance);
    assert_eq!(tui.active_panel, BottomPanel::None);

    // Toggle to computer use
    tui.toggle_panel(BottomPanel::ComputerUse);
    assert_eq!(tui.active_panel, BottomPanel::ComputerUse);

    // Switch directly to patterns
    tui.toggle_panel(BottomPanel::Patterns);
    assert_eq!(tui.active_panel, BottomPanel::Patterns);
}

#[test]
fn test_record_run() {
    let app = nexus_code::app::App::new(nexus_code::config::NxConfig::default()).unwrap();
    let mut tui = TuiApp::new(&app);

    assert!(tui.recent_runs.is_empty());

    tui.record_run("file_read".to_string(), true, 42);
    assert_eq!(tui.recent_runs.len(), 1);
    assert_eq!(tui.recent_runs[0].name, "file_read");
    assert!(tui.recent_runs[0].success);

    // Record 11 runs — should cap at 10
    for i in 0..11 {
        tui.record_run(format!("tool_{}", i), true, 10);
    }
    assert_eq!(tui.recent_runs.len(), 10);
}

#[test]
fn test_keybindings_quit() {
    use nexus_code::tui::keybindings::{process_key, KeyAction};

    let key = crossterm::event::KeyEvent::new(
        crossterm::event::KeyCode::Char('c'),
        crossterm::event::KeyModifiers::CONTROL,
    );
    let action = process_key(key, false, false, false);
    assert!(matches!(action, KeyAction::Quit));
}

#[test]
fn test_keybindings_cancel_stream() {
    use nexus_code::tui::keybindings::{process_key, KeyAction};

    let key = crossterm::event::KeyEvent::new(
        crossterm::event::KeyCode::Char('c'),
        crossterm::event::KeyModifiers::CONTROL,
    );
    let action = process_key(key, true, false, false);
    assert!(matches!(action, KeyAction::CancelStream));
}

#[test]
fn test_keybindings_f_keys() {
    use nexus_code::tui::keybindings::{process_key, KeyAction};

    let f2 = crossterm::event::KeyEvent::new(
        crossterm::event::KeyCode::F(2),
        crossterm::event::KeyModifiers::NONE,
    );
    assert!(matches!(
        process_key(f2, false, false, false),
        KeyAction::ToggleGovernance
    ));

    let f3 = crossterm::event::KeyEvent::new(
        crossterm::event::KeyCode::F(3),
        crossterm::event::KeyModifiers::NONE,
    );
    assert!(matches!(
        process_key(f3, false, false, false),
        KeyAction::ToggleComputerUse
    ));

    let f4 = crossterm::event::KeyEvent::new(
        crossterm::event::KeyCode::F(4),
        crossterm::event::KeyModifiers::NONE,
    );
    assert!(matches!(
        process_key(f4, false, false, false),
        KeyAction::TogglePatterns
    ));

    let f5 = crossterm::event::KeyEvent::new(
        crossterm::event::KeyCode::F(5),
        crossterm::event::KeyModifiers::NONE,
    );
    assert!(matches!(
        process_key(f5, false, false, false),
        KeyAction::ToggleMemory
    ));
}

#[test]
fn test_keybindings_consent() {
    use nexus_code::tui::keybindings::{process_key, KeyAction};

    let a_key = crossterm::event::KeyEvent::new(
        crossterm::event::KeyCode::Char('a'),
        crossterm::event::KeyModifiers::NONE,
    );
    assert!(matches!(
        process_key(a_key, false, true, false),
        KeyAction::ApproveConsent
    ));

    let d_key = crossterm::event::KeyEvent::new(
        crossterm::event::KeyCode::Char('d'),
        crossterm::event::KeyModifiers::NONE,
    );
    assert!(matches!(
        process_key(d_key, false, true, false),
        KeyAction::DenyConsent
    ));
}

#[test]
fn test_keybindings_clear_conversation() {
    use nexus_code::tui::keybindings::{process_key, KeyAction};

    let key = crossterm::event::KeyEvent::new(
        crossterm::event::KeyCode::Char('l'),
        crossterm::event::KeyModifiers::CONTROL,
    );
    assert!(matches!(
        process_key(key, false, false, false),
        KeyAction::ClearConversation
    ));
}

#[test]
fn test_tab_completion() {
    use nexus_code::tui::input_area::tab_complete;

    // Single match
    assert_eq!(tab_complete("/scre"), Some("/screenshot".to_string()));

    // No match
    assert_eq!(tab_complete("/zzz"), None);

    // Not a slash command
    assert_eq!(tab_complete("hello"), None);

    // Exact match (no completion needed)
    assert_eq!(tab_complete("/quit"), None);
}
