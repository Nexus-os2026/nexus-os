//! Conversation area — message rendering with timestamps, role colors,
//! action info lines, and markdown-lite formatting.

use super::theme::Theme;
use super::{MessageRole, TuiApp};
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use ratatui::Frame;

pub fn draw(frame: &mut Frame, area: Rect, state: &TuiApp) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Theme::panel_border());

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let mut lines: Vec<Line> = Vec::new();

    for msg in &state.messages {
        // Role label + timestamp on same line
        let timestamp = msg.timestamp.format("%H:%M:%S").to_string();
        let ts_width = timestamp.len();

        let role_line = match &msg.role {
            MessageRole::User => {
                // Calculate padding to right-align timestamp
                let label = "\u{25b6} You";
                let label_len = label.len();
                let avail = inner.width as usize;
                let pad = avail.saturating_sub(label_len + ts_width + 1);
                Line::from(vec![
                    Span::styled(label, Theme::user_label()),
                    Span::raw(" ".repeat(pad)),
                    Span::styled(timestamp, Theme::muted()),
                ])
            }
            MessageRole::Assistant => {
                let label = "\u{25c0} nx";
                let label_len = label.len();
                let avail = inner.width as usize;
                let pad = avail.saturating_sub(label_len + ts_width + 1);
                Line::from(vec![
                    Span::styled(label, Theme::assistant_label()),
                    Span::raw(" ".repeat(pad)),
                    Span::styled(timestamp, Theme::muted()),
                ])
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
            MessageRole::System => {
                let label = "\u{25b6} system";
                let label_len = label.len();
                let avail = inner.width as usize;
                let pad = avail.saturating_sub(label_len + ts_width + 1);
                Line::from(vec![
                    Span::styled(label, Theme::muted()),
                    Span::raw(" ".repeat(pad)),
                    Span::styled(timestamp, Theme::muted()),
                ])
            }
        };
        lines.push(role_line);

        // Message body with markdown rendering
        let content_lines = super::markdown::render_markdown(&msg.content);
        lines.extend(content_lines);

        // Action info line for tool messages (dim)
        if let MessageRole::Tool {
            success,
            duration_ms,
            ..
        } = &msg.role
        {
            let fuel_str = format!(
                "  fuel:-{} | {}ms | {}",
                1, // placeholder — real fuel cost tracked in governance
                duration_ms,
                if *success { "ok" } else { "FAIL" }
            );
            lines.push(Line::from(Span::styled(fuel_str, Theme::muted())));
        }

        lines.push(Line::from(""));
    }

    // Streaming text with typing indicator
    if state.is_streaming {
        lines.push(Line::from(Span::styled(
            "\u{25c0} nx",
            Theme::assistant_label(),
        )));
        if state.streaming_text.is_empty() {
            // Typing indicator with animated dots
            let dots = match (chrono::Utc::now().timestamp() % 4) as usize {
                0 => ".",
                1 => "..",
                2 => "...",
                _ => "",
            };
            lines.push(Line::from(Span::styled(
                format!("nx is thinking{}", dots),
                Theme::dim(),
            )));
        } else {
            let streaming_lines = super::markdown::render_markdown(&state.streaming_text);
            lines.extend(streaming_lines);
            lines.push(Line::from(Span::styled(
                "\u{2588}",
                ratatui::style::Style::default().fg(Theme::BRAND),
            )));
        }
    }

    // Active tools indicator
    let active_tool_lines = super::tool_activity::format_active_tools(&state.active_tools);
    if !active_tool_lines.is_empty() {
        lines.extend(active_tool_lines);
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
