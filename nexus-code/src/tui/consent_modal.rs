//! Tier2/3 consent dialog overlay.

use super::theme::Theme;
use super::TuiApp;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;

pub fn draw(frame: &mut Frame, area: Rect, state: &TuiApp) {
    let consent = match &state.pending_consent {
        Some(c) => c,
        None => return,
    };

    let modal_width = 50u16.min(area.width.saturating_sub(4));
    let modal_height = 10u16.min(area.height.saturating_sub(4));
    let modal_area = centered_rect(modal_width, modal_height, area);

    frame.render_widget(Clear, modal_area);

    let tier_str = match consent.request.tier {
        crate::governance::ConsentTier::Tier1 => "Tier1 (auto)",
        crate::governance::ConsentTier::Tier2 => "Tier2 (write)",
        crate::governance::ConsentTier::Tier3 => "\u{26a0} Tier3 (DESTRUCTIVE)",
    };

    let title = format!(" Consent Required \u{2014} {} ", tier_str);
    let block = Block::default()
        .title(Span::styled(title, Theme::warning()))
        .borders(Borders::ALL)
        .border_style(ratatui::style::Style::default().fg(Theme::CONSENT_BORDER));

    let details = truncate(&consent.request.details, 40);
    let lines = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("  Tool: ", Theme::dim()),
            Span::styled(consent.tool_name.clone(), Theme::bold()),
        ]),
        Line::from(vec![
            Span::styled("  Action: ", Theme::dim()),
            Span::raw(details),
        ]),
        Line::from(""),
        Line::from(""),
        Line::from(vec![
            Span::styled("  [A]", Theme::success()),
            Span::raw("pprove   "),
            Span::styled("[D]", Theme::error()),
            Span::raw("eny   "),
            Span::styled("[Esc]", Theme::dim()),
            Span::raw(" Cancel"),
        ]),
    ];

    let paragraph = Paragraph::new(lines).block(block);
    frame.render_widget(paragraph, modal_area);
}

/// Center a rectangle within an area.
pub fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    Rect::new(x, y, width, height)
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() > max {
        format!("{}...", &s[..max])
    } else {
        s.to_string()
    }
}
