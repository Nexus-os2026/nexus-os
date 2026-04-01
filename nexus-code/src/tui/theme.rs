//! Nexus Code TUI color theme.

use ratatui::style::{Color, Modifier, Style};

/// Consistent color palette for the TUI.
pub struct Theme;

impl Theme {
    // ─── Brand ───
    pub const BRAND: Color = Color::Cyan;

    // ─── Text ───
    pub const TEXT: Color = Color::White;
    pub const TEXT_DIM: Color = Color::Gray;
    pub const TEXT_MUTED: Color = Color::DarkGray;

    // ─── Status ───
    pub const SUCCESS: Color = Color::Green;
    pub const ERROR: Color = Color::Red;
    pub const WARNING: Color = Color::Yellow;

    // ─── Governance ───
    pub const CONSENT_BORDER: Color = Color::Yellow;

    // ─── Code ───
    pub const CODE_BG: Color = Color::Rgb(30, 30, 46);
    pub const CODE_FG: Color = Color::Rgb(205, 214, 244);

    // ─── Roles ───
    pub const USER_ROLE: Color = Color::Cyan;
    pub const ASSISTANT_ROLE: Color = Color::Magenta;

    // ─── Styles ───
    pub fn title() -> Style {
        Style::default()
            .fg(Self::BRAND)
            .add_modifier(Modifier::BOLD)
    }
    pub fn text() -> Style {
        Style::default().fg(Self::TEXT)
    }
    pub fn dim() -> Style {
        Style::default().fg(Self::TEXT_DIM)
    }
    pub fn muted() -> Style {
        Style::default().fg(Self::TEXT_MUTED)
    }
    pub fn success() -> Style {
        Style::default().fg(Self::SUCCESS)
    }
    pub fn error() -> Style {
        Style::default().fg(Self::ERROR)
    }
    pub fn warning() -> Style {
        Style::default().fg(Self::WARNING)
    }
    pub fn bold() -> Style {
        Style::default().add_modifier(Modifier::BOLD)
    }
    pub fn code() -> Style {
        Style::default().fg(Self::CODE_FG).bg(Self::CODE_BG)
    }
    pub fn user_label() -> Style {
        Style::default()
            .fg(Self::USER_ROLE)
            .add_modifier(Modifier::BOLD)
    }
    pub fn assistant_label() -> Style {
        Style::default()
            .fg(Self::ASSISTANT_ROLE)
            .add_modifier(Modifier::BOLD)
    }
}
