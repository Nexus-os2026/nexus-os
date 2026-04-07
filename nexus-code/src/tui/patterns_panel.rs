//! F4 — Patterns panel widget.
//!
//! Shows learned patterns, memory stats, recent runs.

use super::theme::Theme;
use super::TuiApp;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

pub fn draw(frame: &mut Frame, area: Rect, state: &TuiApp) {
    let block = Block::default()
        .title(Span::styled(" Patterns [F4] ", Theme::title()))
        .borders(Borders::ALL)
        .border_style(Theme::panel_border())
        .style(Theme::panel_bg());

    let mut lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            " \u{2500} Learned Patterns \u{2500}",
            Theme::bold(),
        )),
    ];

    if state.patterns.is_empty() {
        lines.push(Line::from(Span::styled(
            "  No patterns learned yet",
            Theme::muted(),
        )));
    } else {
        for pattern in &state.patterns {
            let confidence_style = if pattern.confidence >= 0.8 {
                Theme::success()
            } else if pattern.confidence >= 0.5 {
                Theme::warning()
            } else {
                Theme::muted()
            };
            lines.push(Line::from(vec![
                Span::styled("  \u{2022} ", Theme::dim()),
                Span::styled(pattern.name.clone(), Theme::text()),
                Span::styled(
                    format!(" ({:.0}%)", pattern.confidence * 100.0),
                    confidence_style,
                ),
            ]));
        }
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        " \u{2500} Memory Stats \u{2500}",
        Theme::bold(),
    )));
    lines.push(Line::from(vec![
        Span::styled("  Entries:      ", Theme::dim()),
        Span::styled(format!("{}", state.memory_entries), Theme::text()),
    ]));
    lines.push(Line::from(vec![
        Span::styled("  Fuel used:    ", Theme::dim()),
        Span::styled(format!("{}", state.memory_fuel_used), Theme::text()),
    ]));

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        " \u{2500} Recent Runs \u{2500}",
        Theme::bold(),
    )));

    if state.recent_runs.is_empty() {
        lines.push(Line::from(Span::styled("  No recent runs", Theme::muted())));
    } else {
        for run in &state.recent_runs {
            let icon = if run.success { "\u{2713}" } else { "\u{2717}" };
            let style = if run.success {
                Theme::success()
            } else {
                Theme::error()
            };
            lines.push(Line::from(vec![
                Span::styled(format!("  {} ", icon), style),
                Span::styled(run.name.clone(), Theme::text()),
                Span::styled(format!(" ({}ms)", run.duration_ms), Theme::dim()),
            ]));
        }
    }

    let paragraph = Paragraph::new(lines).block(block);
    frame.render_widget(paragraph, area);
}
