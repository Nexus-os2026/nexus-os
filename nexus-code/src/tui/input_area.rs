//! Input area with cursor and streaming indicator.

use super::theme::Theme;
use super::TuiApp;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

pub fn draw(frame: &mut Frame, area: Rect, state: &TuiApp) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(if state.is_streaming {
            Theme::muted()
        } else {
            Theme::dim()
        })
        .title(Span::styled(
            if state.is_streaming {
                " working "
            } else {
                " input "
            },
            if state.is_streaming {
                Theme::warning()
            } else {
                Theme::dim()
            },
        ));

    let display_text = if state.is_streaming {
        "Agent is working... (Ctrl+C to cancel)".to_string()
    } else {
        format!("\u{203a} {}", state.input)
    };

    let paragraph = Paragraph::new(Line::from(display_text))
        .block(block)
        .style(Theme::text());

    frame.render_widget(paragraph, area);

    // Position cursor
    if !state.is_streaming {
        #[allow(clippy::cast_possible_truncation)]
        let cursor_x = area.x + 2 + state.cursor_pos as u16 + 1; // border + "› " + cursor
        frame.set_cursor_position((cursor_x, area.y + 1));
    }
}
