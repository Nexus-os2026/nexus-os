//! Panel layout — status bar, chat, input, optional sidebar.

use super::theme::Theme;
use super::TuiApp;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

/// Draw the complete TUI layout.
pub fn draw(frame: &mut Frame, state: &TuiApp) {
    let size = frame.area();

    // Main layout: status bar (1) + chat (min 5) + input (3)
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(5),
            Constraint::Length(3),
        ])
        .split(size);

    // Split chat area if sidebar is shown
    let (chat_area, sidebar_area) = if state.show_sidebar {
        let h_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
            .split(main_chunks[1]);
        (h_chunks[0], Some(h_chunks[1]))
    } else {
        (main_chunks[1], None)
    };

    // Draw components
    super::status_bar::draw(frame, main_chunks[0], state);
    super::chat_panel::draw(frame, chat_area, state);
    super::input_area::draw(frame, main_chunks[2], state);

    if let Some(sidebar) = sidebar_area {
        draw_sidebar(frame, sidebar, state);
    }

    // Overlays (drawn last, on top)
    if state.show_help {
        super::help_overlay::draw(frame, size);
    }

    if state.pending_consent.is_some() {
        super::consent_modal::draw(frame, size, state);
    }
}

/// Governance sidebar panel.
fn draw_sidebar(frame: &mut Frame, area: Rect, state: &TuiApp) {
    let block = Block::default()
        .title(Span::styled(" Governance ", Theme::title()))
        .borders(Borders::ALL)
        .border_style(Theme::dim());

    let envelope_style = if state.envelope_similarity > 70.0 {
        Theme::success()
    } else if state.envelope_similarity > 50.0 {
        Theme::warning()
    } else {
        Theme::error()
    };

    let lines = vec![
        Line::from(vec![
            Span::styled("Session: ", Theme::dim()),
            Span::styled(state.session_id_short.clone(), Theme::text()),
        ]),
        Line::from(vec![
            Span::styled("Provider: ", Theme::dim()),
            Span::styled(format!("{}/{}", state.provider, state.model), Theme::text()),
        ]),
        Line::from(""),
        Line::from(Span::styled("\u{2500} Fuel \u{2500}", Theme::bold())),
        Line::from(vec![
            Span::styled("  Remaining: ", Theme::dim()),
            Span::styled(format!("{}", state.fuel_remaining), Theme::text()),
        ]),
        Line::from(vec![
            Span::styled("  Total: ", Theme::dim()),
            Span::styled(format!("{}", state.fuel_total), Theme::text()),
        ]),
        Line::from(""),
        Line::from(Span::styled("\u{2500} Audit \u{2500}", Theme::bold())),
        Line::from(vec![
            Span::styled("  Entries: ", Theme::dim()),
            Span::styled(format!("{}", state.audit_len), Theme::text()),
        ]),
        Line::from(""),
        Line::from(Span::styled("\u{2500} Envelope \u{2500}", Theme::bold())),
        Line::from(vec![
            Span::styled("  Similarity: ", Theme::dim()),
            Span::styled(format!("{:.0}%", state.envelope_similarity), envelope_style),
        ]),
    ];

    let paragraph = Paragraph::new(lines).block(block);
    frame.render_widget(paragraph, area);
}
