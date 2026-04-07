//! Nexus Code TUI color theme — full design system.
//!
//! All colors and styles used by the TUI are defined here.
//! No widget should hardcode colors — use Theme constants and methods.

use ratatui::style::{Color, Modifier, Style};

/// Consistent color palette for the TUI.
pub struct Theme;

impl Theme {
    // ─── Brand ───
    pub const BRAND: Color = Color::Cyan;
    pub const BRAND_DIM: Color = Color::Rgb(0, 139, 139); // dark cyan

    // ─── Text ───
    pub const TEXT: Color = Color::White;
    pub const TEXT_DIM: Color = Color::Gray;
    pub const TEXT_MUTED: Color = Color::DarkGray;

    // ─── Status ───
    pub const SUCCESS: Color = Color::Green;
    pub const ERROR: Color = Color::Red;
    pub const WARNING: Color = Color::Yellow;
    pub const INFO: Color = Color::Blue;

    // ─── Governance ───
    pub const CONSENT_BORDER: Color = Color::Yellow;

    // ─── Code ───
    pub const CODE_BG: Color = Color::Rgb(30, 30, 46);
    pub const CODE_FG: Color = Color::Rgb(205, 214, 244);

    // ─── Roles ───
    pub const USER_ROLE: Color = Color::Cyan;
    pub const ASSISTANT_ROLE: Color = Color::Magenta;

    // ─── Fuel gradient ───
    pub const FUEL_FULL: Color = Color::Cyan; // 75-100%
    pub const FUEL_GOOD: Color = Color::Rgb(0, 175, 175); // teal variant
    pub const FUEL_MEDIUM: Color = Color::Yellow; // 50-75%
    pub const FUEL_LOW: Color = Color::Rgb(255, 165, 0); // orange 25-50%
    pub const FUEL_CRITICAL: Color = Color::Red; // 0-25%

    // ─── Panel backgrounds ───
    pub const STATUS_BAR_BG: Color = Color::Rgb(30, 30, 46);
    pub const PANEL_BG: Color = Color::Rgb(24, 24, 37);
    pub const PANEL_BORDER: Color = Color::Rgb(69, 71, 90);

    // ─── HITL / Errors ───
    pub const HITL_BORDER: Color = Color::Yellow;
    pub const ERROR_BORDER: Color = Color::Red;

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

    pub fn info() -> Style {
        Style::default().fg(Self::INFO)
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

    pub fn status_bar() -> Style {
        Style::default().bg(Self::STATUS_BAR_BG)
    }

    pub fn panel_border() -> Style {
        Style::default().fg(Self::PANEL_BORDER)
    }

    pub fn panel_bg() -> Style {
        Style::default().bg(Self::PANEL_BG)
    }

    /// Get fuel color based on percentage (0-100).
    pub fn fuel_color(pct: u32) -> Color {
        if pct >= 75 {
            Self::FUEL_FULL
        } else if pct >= 50 {
            Self::FUEL_MEDIUM
        } else if pct >= 25 {
            Self::FUEL_LOW
        } else {
            Self::FUEL_CRITICAL
        }
    }

    /// Get fuel style based on percentage.
    pub fn fuel_style(pct: u32) -> Style {
        Style::default().fg(Self::fuel_color(pct))
    }

    /// Render a fuel bar with block characters.
    /// Returns (bar_string, color) for a 10-char wide bar.
    pub fn fuel_bar(pct: u32) -> (String, Color) {
        let filled = (pct as usize * 10) / 100;
        let filled = filled.min(10);
        let empty = 10 - filled;
        let bar = format!("{}{}", "\u{2588}".repeat(filled), "\u{2591}".repeat(empty),);
        (bar, Self::fuel_color(pct))
    }

    /// Envelope similarity style.
    pub fn envelope_style(similarity: f64) -> Style {
        if similarity > 70.0 {
            Self::success()
        } else if similarity > 50.0 {
            Self::warning()
        } else {
            Self::error()
        }
    }
}
