//! Panel layout — 2-line status bar, conversation, toggleable bottom panels, input.
//!
//! Layout:
//!   [Status Bar — 2 lines]
//!   [Conversation area — fills remaining space]
//!   [Bottom panel (F2/F3/F4) — 12 lines, if toggled]
//!   [Input area — 3 lines]

use super::theme::Theme;
use super::TuiApp;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::Frame;

/// Which bottom panel is currently shown (if any).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BottomPanel {
    None,
    Governance,
    ComputerUse,
    Patterns,
    Memory,
}

/// Draw the complete TUI layout.
pub fn draw(frame: &mut Frame, state: &TuiApp) {
    let size = frame.area();

    // Determine constraints based on whether a bottom panel is shown
    let has_panel = state.active_panel != BottomPanel::None;

    let main_chunks = if has_panel {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(2),  // status bar (2 lines)
                Constraint::Min(5),     // conversation
                Constraint::Length(14), // bottom panel
                Constraint::Length(3),  // input
            ])
            .split(size)
    } else {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(2), // status bar (2 lines)
                Constraint::Min(5),    // conversation
                Constraint::Length(3), // input
            ])
            .split(size)
    };

    // Draw status bar (always)
    super::status_bar::draw(frame, main_chunks[0], state);

    // Draw conversation area
    super::chat_panel::draw(frame, main_chunks[1], state);

    // Draw bottom panel if active
    if has_panel {
        draw_bottom_panel(frame, main_chunks[2], state);
        super::input_area::draw(frame, main_chunks[3], state);
    } else {
        super::input_area::draw(frame, main_chunks[2], state);
    }

    // Overlays (drawn last, on top)
    if state.show_help {
        super::help_overlay::draw(frame, size);
    }

    if state.pending_consent.is_some() {
        super::consent_modal::draw(frame, size, state);
    }
}

/// Draw the active bottom panel.
fn draw_bottom_panel(frame: &mut Frame, area: Rect, state: &TuiApp) {
    match state.active_panel {
        BottomPanel::Governance => {
            super::governance_panel::draw(frame, area, state);
        }
        BottomPanel::ComputerUse => {
            super::computer_use_panel::draw(frame, area, state);
        }
        BottomPanel::Patterns => {
            super::patterns_panel::draw(frame, area, state);
        }
        BottomPanel::Memory => {
            draw_memory_panel(frame, area, state);
        }
        BottomPanel::None => {}
    }
}

/// F5 — Memory panel (inline, simple).
fn draw_memory_panel(frame: &mut Frame, area: Rect, state: &TuiApp) {
    use ratatui::text::{Line, Span};
    use ratatui::widgets::{Block, Borders, Paragraph};

    let block = Block::default()
        .title(Span::styled(" Memory [F5] ", Theme::title()))
        .borders(Borders::ALL)
        .border_style(Theme::panel_border())
        .style(Theme::panel_bg());

    let mut lines = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled(" Entries: ", Theme::dim()),
            Span::styled(format!("{}", state.memory_entries), Theme::text()),
        ]),
        Line::from(vec![
            Span::styled(" Fuel used: ", Theme::dim()),
            Span::styled(format!("{}", state.memory_fuel_used), Theme::text()),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            " \u{2500} Cross-session Memory \u{2500}",
            Theme::bold(),
        )),
    ];

    if state.memory_entries == 0 {
        lines.push(Line::from(Span::styled(
            "  No memories stored yet",
            Theme::muted(),
        )));
        lines.push(Line::from(Span::styled(
            "  Use /memory to manage",
            Theme::muted(),
        )));
    } else {
        lines.push(Line::from(Span::styled(
            "  Use /memory to view entries",
            Theme::dim(),
        )));
    }

    let paragraph = Paragraph::new(lines).block(block);
    frame.render_widget(paragraph, area);
}
