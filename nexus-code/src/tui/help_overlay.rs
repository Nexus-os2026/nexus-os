//! Help screen overlay (F1).

use super::theme::Theme;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;

pub fn draw(frame: &mut Frame, area: Rect) {
    let width = 60u16.min(area.width.saturating_sub(4));
    let height = 24u16.min(area.height.saturating_sub(4));
    let help_area = super::consent_modal::centered_rect(width, height, area);

    frame.render_widget(Clear, help_area);

    let block = Block::default()
        .title(Span::styled(" Nexus Code \u{2014} Help ", Theme::title()))
        .borders(Borders::ALL)
        .border_style(Theme::dim());

    let lines = vec![
        Line::from(""),
        Line::from(Span::styled("  Commands:", Theme::bold())),
        Line::from("  /status          Governance status"),
        Line::from("  /cost            Fuel usage"),
        Line::from("  /plan <task>     Plan + execute"),
        Line::from("  /commit <msg>    Governed git commit"),
        Line::from("  /diff            Uncommitted changes"),
        Line::from("  /test            Run project tests"),
        Line::from("  /fix             Auto-fix last error"),
        Line::from("  /search <pat>    Search codebase"),
        Line::from("  /review          Code review"),
        Line::from("  /refactor <desc> Refactor code"),
        Line::from("  /compact         Compact context"),
        Line::from("  /memory          Manage memories"),
        Line::from("  /explain <topic> Explain code/concept"),
        Line::from("  /quit            Exit"),
        Line::from(""),
        Line::from(Span::styled("  Keys:", Theme::bold())),
        Line::from("  F1           Toggle help"),
        Line::from("  Tab          Toggle sidebar"),
        Line::from("  PgUp/PgDn   Scroll chat"),
        Line::from("  Ctrl+C      Cancel / Quit"),
        Line::from("  Esc          Close overlay"),
    ];

    let paragraph = Paragraph::new(lines).block(block);
    frame.render_widget(paragraph, help_area);
}
