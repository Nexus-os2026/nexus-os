//! Enhanced input area with paste support, slash command hints, tab completion.

use super::theme::Theme;
use super::TuiApp;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

/// Available slash commands for hints and tab completion.
pub const SLASH_COMMANDS: &[&str] = &[
    "/status",
    "/cost",
    "/plan",
    "/commit",
    "/diff",
    "/test",
    "/fix",
    "/search",
    "/review",
    "/refactor",
    "/compact",
    "/memory",
    "/explain",
    "/screenshot",
    "/qa",
    "/computer-use",
    "/quit",
    "/help",
];

pub fn draw(frame: &mut Frame, area: Rect, state: &TuiApp) {
    let border_style = if state.is_streaming {
        Theme::muted()
    } else {
        Theme::panel_border()
    };

    let title = if state.is_streaming {
        Span::styled(" working ", Theme::warning())
    } else {
        Span::styled(" input ", Theme::dim())
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(border_style)
        .title(title);

    if state.is_streaming {
        let dots = match (chrono::Utc::now().timestamp() % 4) as usize {
            0 => ".",
            1 => "..",
            2 => "...",
            _ => "",
        };
        let display = format!("nx is working{} (Ctrl+C to cancel)", dots);
        let paragraph =
            Paragraph::new(Line::from(Span::styled(display, Theme::dim()))).block(block);
        frame.render_widget(paragraph, area);
        return;
    }

    // Build input display with slash command hints
    let input_display = format!("\u{203a} {}", state.input);

    // Show slash command matches if typing a slash command
    let hint = if state.input.starts_with('/') && !state.input.contains(' ') {
        let partial = &state.input;
        let matches: Vec<&&str> = SLASH_COMMANDS
            .iter()
            .filter(|cmd| cmd.starts_with(partial) && **cmd != partial)
            .collect();
        if matches.len() == 1 {
            Some(matches[0].to_string())
        } else if matches.len() > 1 && matches.len() <= 5 {
            Some(matches.iter().map(|s| **s).collect::<Vec<_>>().join(" "))
        } else {
            None
        }
    } else {
        None
    };

    let lines = if let Some(ref hint_text) = hint {
        vec![
            Line::from(input_display),
            Line::from(Span::styled(format!("  {}", hint_text), Theme::muted())),
        ]
    } else {
        // Show command hints when input is empty
        if state.input.is_empty() {
            vec![
                Line::from(Span::styled("\u{203a} ", Theme::dim())),
                Line::from(Span::styled(
                    "  /status /cost /quit /help /screenshot",
                    Theme::muted(),
                )),
            ]
        } else {
            vec![Line::from(input_display)]
        }
    };

    let paragraph = Paragraph::new(lines).block(block).style(Theme::text());
    frame.render_widget(paragraph, area);

    // Position cursor
    #[allow(clippy::cast_possible_truncation)]
    let cursor_x = area.x + 1 + 2 + state.cursor_pos as u16; // border + "› " + cursor
    frame.set_cursor_position((cursor_x, area.y + 1));
}

/// Find the best tab-completion match for the current input.
pub fn tab_complete(input: &str) -> Option<String> {
    if !input.starts_with('/') {
        return None;
    }
    let matches: Vec<&&str> = SLASH_COMMANDS
        .iter()
        .filter(|cmd| cmd.starts_with(input) && **cmd != input)
        .collect();
    if matches.len() == 1 {
        Some(matches[0].to_string())
    } else {
        // Find common prefix
        if matches.len() > 1 {
            let first = matches[0];
            let mut prefix = first.to_string();
            for m in &matches[1..] {
                while !m.starts_with(&prefix) {
                    prefix.pop();
                }
            }
            if prefix.len() > input.len() {
                return Some(prefix);
            }
        }
        None
    }
}
