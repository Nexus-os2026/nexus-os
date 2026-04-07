//! F3 — Computer Use panel widget.
//!
//! Shows screen capture status, input control, safety guards, session stats,
//! and last action. Works WITHOUT nexus-computer-use compiled in — shows
//! "not available" if absent.

use super::theme::Theme;
use super::TuiApp;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

pub fn draw(frame: &mut Frame, area: Rect, state: &TuiApp) {
    let block = Block::default()
        .title(Span::styled(" Computer Use [F3] ", Theme::title()))
        .borders(Borders::ALL)
        .border_style(Theme::panel_border())
        .style(Theme::panel_bg());

    let lines = if state.computer_use_active {
        let status_indicator = Span::styled("\u{25cf} Active", Theme::success());
        let fixes_display = format!(
            "{}/{} verified",
            state.cu_fixes_verified, state.cu_fixes_total
        );

        vec![
            Line::from(vec![
                Span::styled(" Status:    ", Theme::dim()),
                status_indicator,
            ]),
            Line::from(vec![
                Span::styled(" Tools:     ", Theme::dim()),
                Span::styled(
                    "screen_capture, screen_interact, screen_analyze",
                    Theme::text(),
                ),
            ]),
            Line::from(""),
            Line::from(Span::styled(
                " \u{2500} Current Target \u{2500}",
                Theme::bold(),
            )),
            Line::from(vec![
                Span::styled("  App:       ", Theme::dim()),
                Span::styled("Nexus OS", Theme::text()),
            ]),
            Line::from(vec![
                Span::styled("  Page:      ", Theme::dim()),
                Span::styled(&state.cu_current_page, Theme::text()),
            ]),
            Line::from(""),
            Line::from(Span::styled(
                " \u{2500} Session Stats \u{2500}",
                Theme::bold(),
            )),
            Line::from(vec![
                Span::styled("  Screenshots:  ", Theme::dim()),
                Span::styled(state.cu_screenshots.to_string(), Theme::text()),
            ]),
            Line::from(vec![
                Span::styled("  Interactions: ", Theme::dim()),
                Span::styled(state.cu_interactions.to_string(), Theme::text()),
            ]),
            Line::from(vec![
                Span::styled("  Analyses:     ", Theme::dim()),
                Span::styled(state.cu_analyses.to_string(), Theme::text()),
            ]),
            Line::from(vec![
                Span::styled("  Fixes:        ", Theme::dim()),
                Span::styled(fixes_display, Theme::text()),
            ]),
            Line::from(""),
            Line::from(Span::styled(
                " \u{2500} Last Action \u{2500}",
                Theme::bold(),
            )),
            Line::from(vec![
                Span::styled("  ", Theme::dim()),
                Span::styled(&state.cu_last_action_time, Theme::dim()),
                Span::styled("  ", Theme::dim()),
                Span::styled(&state.cu_last_action, Theme::text()),
            ]),
        ]
    } else {
        vec![
            Line::from(""),
            Line::from(""),
            Line::from(Span::styled("  Computer Use not available", Theme::muted())),
            Line::from(""),
            Line::from(Span::styled(
                "  Enable with --computer-use flag",
                Theme::muted(),
            )),
            Line::from(Span::styled(
                "  or /computer-use on command.",
                Theme::muted(),
            )),
            Line::from(""),
            Line::from(Span::styled("  Tools: screen_capture,", Theme::muted())),
            Line::from(Span::styled(
                "  screen_interact, screen_analyze",
                Theme::muted(),
            )),
        ]
    };

    let paragraph = Paragraph::new(lines).block(block);
    frame.render_widget(paragraph, area);
}
