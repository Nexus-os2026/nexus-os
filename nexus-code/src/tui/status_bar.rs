//! 2-line status bar with fuel gradient and governance indicators.
//!
//! Line 1: nx version | provider (cost) | fuel bar (visual gradient) | model
//! Line 2: audit count | envelope % | computer use status | tools count

use super::theme::Theme;
use super::TuiApp;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

pub fn draw(frame: &mut Frame, area: Rect, state: &TuiApp) {
    let fuel_pct = if state.fuel_total > 0 {
        (state.fuel_remaining as f64 / state.fuel_total as f64 * 100.0) as u32
    } else {
        0
    };

    let (fuel_bar, fuel_color) = Theme::fuel_bar(fuel_pct);

    // ─── Line 1: version | provider | fuel bar | model ───
    let line1 = Line::from(vec![
        Span::styled(
            " nx ",
            ratatui::style::Style::default()
                .fg(ratatui::style::Color::Black)
                .bg(Theme::BRAND),
        ),
        Span::styled(format!(" v{} ", env!("CARGO_PKG_VERSION")), Theme::dim()),
        Span::styled("\u{2502} ", Theme::muted()),
        Span::styled(
            if state.provider == "claude_cli" {
                "claude_cli ($0)".to_string()
            } else {
                state.provider.clone()
            },
            Theme::text(),
        ),
        Span::styled(" \u{2502} ", Theme::muted()),
        Span::styled("fuel ", Theme::dim()),
        Span::styled(fuel_bar, ratatui::style::Style::default().fg(fuel_color)),
        Span::styled(format!(" {}%", fuel_pct), Theme::fuel_style(fuel_pct)),
        Span::styled(" \u{2502} ", Theme::muted()),
        Span::styled(&state.model, Theme::text()),
    ]);

    // ─── Line 2: audit | envelope | computer use | tools ───
    let envelope_style = Theme::envelope_style(state.envelope_similarity);

    let computer_use_status = if state.computer_use_active {
        Span::styled(
            "\u{25cf} active",
            ratatui::style::Style::default()
                .fg(Theme::BRAND)
                .add_modifier(ratatui::style::Modifier::BOLD),
        )
    } else {
        Span::styled("\u{25cb} off", Theme::muted())
    };

    let mut line2_spans = vec![
        Span::styled("  ", Theme::muted()),
        Span::styled("audit:", Theme::dim()),
        Span::styled(format!("{}", state.audit_len), Theme::text()),
        Span::styled(" \u{2502} ", Theme::muted()),
        Span::styled("env:", Theme::dim()),
        Span::styled(format!("{:.0}%", state.envelope_similarity), envelope_style),
        Span::styled(" \u{2502} ", Theme::muted()),
        Span::styled("screen:", Theme::dim()),
        computer_use_status,
        Span::styled(" \u{2502} ", Theme::muted()),
        Span::styled(format!("{}tools", state.tool_count), Theme::dim()),
    ];

    // Temporary status message (fades after 5 seconds)
    if let Some((ref msg, ref time)) = state.status_message {
        if chrono::Utc::now()
            .signed_duration_since(*time)
            .num_seconds()
            < 5
        {
            line2_spans.push(Span::styled(" \u{2502} ", Theme::muted()));
            line2_spans.push(Span::styled(msg.clone(), Theme::warning()));
        }
    }

    let line2 = Line::from(line2_spans);

    let lines = vec![line1, line2];
    let bar = Paragraph::new(lines).style(Theme::status_bar());
    frame.render_widget(bar, area);
}
