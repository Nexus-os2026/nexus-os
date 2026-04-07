//! F2 — Governance panel widget.
//!
//! Shows session ID, public key, active grants, audit chain, fuel budget.

use super::theme::Theme;
use super::TuiApp;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

pub fn draw(frame: &mut Frame, area: Rect, state: &TuiApp) {
    let block = Block::default()
        .title(Span::styled(" Governance [F2] ", Theme::title()))
        .borders(Borders::ALL)
        .border_style(Theme::panel_border())
        .style(Theme::panel_bg());

    let fuel_pct = if state.fuel_total > 0 {
        (state.fuel_remaining as f64 / state.fuel_total as f64 * 100.0) as u32
    } else {
        0
    };
    let (fuel_bar, fuel_color) = Theme::fuel_bar(fuel_pct);

    let envelope_style = Theme::envelope_style(state.envelope_similarity);

    let integrity_str = if state.audit_len > 0 {
        "\u{2713} valid"
    } else {
        "\u{2014} empty"
    };
    let integrity_style = if state.audit_len > 0 {
        Theme::success()
    } else {
        Theme::muted()
    };

    let lines = vec![
        Line::from(vec![
            Span::styled(" Session: ", Theme::dim()),
            Span::styled(&state.session_id_short, Theme::text()),
        ]),
        Line::from(vec![
            Span::styled(" PubKey:  ", Theme::dim()),
            Span::styled(&state.public_key_short, Theme::text()),
        ]),
        Line::from(""),
        Line::from(Span::styled(" \u{2500} App Grants \u{2500}", Theme::bold())),
        Line::from(vec![
            Span::styled("  file_read   ", Theme::dim()),
            Span::styled("\u{2713}", Theme::success()),
        ]),
        Line::from(vec![
            Span::styled("  file_write  ", Theme::dim()),
            Span::styled("\u{2713}", Theme::success()),
        ]),
        Line::from(vec![
            Span::styled("  shell_exec  ", Theme::dim()),
            Span::styled("\u{2713}", Theme::warning()),
        ]),
        Line::from(vec![
            Span::styled("  network     ", Theme::dim()),
            Span::styled("\u{2713}", Theme::warning()),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            " \u{2500} Audit Chain \u{2500}",
            Theme::bold(),
        )),
        Line::from(vec![
            Span::styled("  Length:    ", Theme::dim()),
            Span::styled(format!("{}", state.audit_len), Theme::text()),
        ]),
        Line::from(vec![
            Span::styled("  Integrity: ", Theme::dim()),
            Span::styled(integrity_str, integrity_style),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            " \u{2500} Fuel Budget \u{2500}",
            Theme::bold(),
        )),
        Line::from(vec![
            Span::styled("  ", Theme::dim()),
            Span::styled(fuel_bar, ratatui::style::Style::default().fg(fuel_color)),
            Span::styled(format!(" {}%", fuel_pct), Theme::fuel_style(fuel_pct)),
        ]),
        Line::from(vec![
            Span::styled("  Remaining: ", Theme::dim()),
            Span::styled(format!("{}", state.fuel_remaining), Theme::text()),
            Span::styled(format!(" / {}", state.fuel_total), Theme::dim()),
        ]),
        Line::from(""),
        Line::from(Span::styled(" \u{2500} Envelope \u{2500}", Theme::bold())),
        Line::from(vec![
            Span::styled("  Similarity: ", Theme::dim()),
            Span::styled(format!("{:.1}%", state.envelope_similarity), envelope_style),
        ]),
    ];

    let paragraph = Paragraph::new(lines).block(block);
    frame.render_widget(paragraph, area);
}
