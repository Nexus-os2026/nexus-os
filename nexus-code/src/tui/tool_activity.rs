//! Tool activity display helpers.

use super::theme::Theme;
use super::ToolActivityStatus;
use ratatui::text::{Line, Span};

/// Format active tools as status lines for display.
pub fn format_active_tools(tools: &[super::ToolActivityEntry]) -> Vec<Line<'static>> {
    tools
        .iter()
        .filter(|t| t.status == ToolActivityStatus::Running)
        .map(|t| {
            Line::from(vec![
                Span::styled("  \u{25cb} ", Theme::warning()),
                Span::styled(t.name.clone(), Theme::text()),
                Span::styled(" running...", Theme::dim()),
            ])
        })
        .collect()
}
