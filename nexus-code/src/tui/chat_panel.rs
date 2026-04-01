//! Chat message rendering with markdown support.

use super::theme::Theme;
use super::{MessageRole, TuiApp};
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use ratatui::Frame;

pub fn draw(frame: &mut Frame, area: Rect, state: &TuiApp) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Theme::dim());

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let mut lines: Vec<Line> = Vec::new();

    for msg in &state.messages {
        let role_line = match &msg.role {
            MessageRole::User => Line::from(Span::styled("\u{25b8} You", Theme::user_label())),
            MessageRole::Assistant => {
                Line::from(Span::styled("\u{25b8} nx", Theme::assistant_label()))
            }
            MessageRole::Tool {
                name,
                success,
                duration_ms,
            } => {
                let icon = if *success { "\u{2713}" } else { "\u{2717}" };
                let style = if *success {
                    Theme::success()
                } else {
                    Theme::error()
                };
                Line::from(vec![
                    Span::styled(format!("  {} {} ", icon, name), style),
                    Span::styled(format!("({}ms)", duration_ms), Theme::dim()),
                ])
            }
            MessageRole::System => Line::from(Span::styled("\u{25b8} system", Theme::muted())),
        };
        lines.push(role_line);

        let content_lines = super::markdown::render_markdown(&msg.content);
        lines.extend(content_lines);
        lines.push(Line::from(""));
    }

    // Streaming text
    if !state.streaming_text.is_empty() {
        lines.push(Line::from(Span::styled(
            "\u{25b8} nx",
            Theme::assistant_label(),
        )));
        let streaming_lines = super::markdown::render_markdown(&state.streaming_text);
        lines.extend(streaming_lines);
        lines.push(Line::from(Span::styled(
            "\u{2588}",
            ratatui::style::Style::default().fg(Theme::BRAND),
        )));
    }

    // Auto-scroll to bottom
    let total_lines = lines.len() as u16;
    let visible_lines = inner.height;
    let max_scroll = total_lines.saturating_sub(visible_lines);
    let scroll = if state.scroll_offset == 0 {
        max_scroll
    } else {
        max_scroll.saturating_sub(state.scroll_offset)
    };

    let paragraph = Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .scroll((scroll, 0));
    frame.render_widget(paragraph, inner);
}
