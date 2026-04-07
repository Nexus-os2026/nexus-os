//! Help screen overlay (F1) — updated with all keybindings.

use super::theme::Theme;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;

pub fn draw(frame: &mut Frame, area: Rect) {
    let width = 62u16.min(area.width.saturating_sub(4));
    let height = 28u16.min(area.height.saturating_sub(4));
    let help_area = super::consent_modal::centered_rect(width, height, area);

    frame.render_widget(Clear, help_area);

    let block = Block::default()
        .title(Span::styled(" Nexus Code \u{2014} Help ", Theme::title()))
        .borders(Borders::ALL)
        .border_style(Theme::panel_border());

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
        Line::from("  /screenshot      Capture screen"),
        Line::from("  /quit            Exit"),
        Line::from(""),
        Line::from(Span::styled("  Keys:", Theme::bold())),
        Line::from("  Enter        Send message"),
        Line::from("  Shift+Ctrl+V Paste"),
        Line::from("  F1           Toggle help"),
        Line::from("  F2           Governance panel"),
        Line::from("  F3           Computer use panel"),
        Line::from("  F4           Patterns panel"),
        Line::from("  F5           Memory panel"),
        Line::from("  Tab          Complete slash command"),
        Line::from("  PgUp/PgDn   Scroll conversation"),
        Line::from("  Ctrl+Up/Dn  Scroll (1 line)"),
        Line::from("  Ctrl+L      Clear conversation"),
        Line::from("  Ctrl+C      Cancel / Quit"),
        Line::from("  Esc          Close overlay"),
    ];

    let paragraph = Paragraph::new(lines).block(block);
    frame.render_widget(paragraph, help_area);
}
