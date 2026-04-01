//! Live governance metrics status bar.

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

    let fuel_style = if fuel_pct > 50 {
        Theme::success()
    } else if fuel_pct > 20 {
        Theme::warning()
    } else {
        Theme::error()
    };

    let envelope_style = if state.envelope_similarity > 70.0 {
        Theme::success()
    } else if state.envelope_similarity > 50.0 {
        Theme::warning()
    } else {
        Theme::error()
    };

    let mut spans = vec![
        Span::styled(
            " nx ",
            ratatui::style::Style::default()
                .fg(ratatui::style::Color::Black)
                .bg(Theme::BRAND),
        ),
        Span::raw(" "),
        Span::styled(&state.session_id_short, Theme::dim()),
        Span::styled(" \u{2502} ", Theme::muted()),
        Span::styled(format!("{}/{}", state.provider, state.model), Theme::text()),
        Span::styled(" \u{2502} ", Theme::muted()),
        Span::styled("Fuel:", Theme::dim()),
        Span::styled(format!("{}%", fuel_pct), fuel_style),
        Span::styled(" \u{2502} ", Theme::muted()),
        Span::styled("Audit:", Theme::dim()),
        Span::styled(format!("{}", state.audit_len), Theme::text()),
        Span::styled(" \u{2502} ", Theme::muted()),
        Span::styled("Env:", Theme::dim()),
        Span::styled(format!("{:.0}%", state.envelope_similarity), envelope_style),
        Span::styled(" \u{2502} ", Theme::muted()),
        Span::styled(format!("{}tools", state.tool_count), Theme::dim()),
    ];

    if let Some((ref msg, ref time)) = state.status_message {
        if chrono::Utc::now()
            .signed_duration_since(*time)
            .num_seconds()
            < 5
        {
            spans.push(Span::styled(" \u{2502} ", Theme::muted()));
            spans.push(Span::styled(msg.clone(), Theme::warning()));
        }
    }

    let line = Line::from(spans);
    let bar = Paragraph::new(line)
        .style(ratatui::style::Style::default().bg(ratatui::style::Color::Rgb(30, 30, 46)));
    frame.render_widget(bar, area);
}
