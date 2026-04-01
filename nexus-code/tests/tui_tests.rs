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
    assert!(!tui.show_sidebar);
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

    // PageUp increases offset
    tui.handle_key(crossterm::event::KeyEvent::new(
        crossterm::event::KeyCode::PageUp,
        crossterm::event::KeyModifiers::NONE,
    ));
    assert_eq!(tui.scroll_offset, 10);

    // PageDown decreases offset
    tui.handle_key(crossterm::event::KeyEvent::new(
        crossterm::event::KeyCode::PageDown,
        crossterm::event::KeyModifiers::NONE,
    ));
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
