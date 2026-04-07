//! Centralized key binding handler.
//!
//! All F-keys, Ctrl combos, and scrolling are handled here.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// Result of processing a key event.
pub enum KeyAction {
    /// No action needed — key was handled internally (e.g., input editing).
    Handled,
    /// Quit the application.
    Quit,
    /// Cancel streaming.
    CancelStream,
    /// Submit the current input.
    Submit,
    /// Close overlay (help, consent modal).
    CloseOverlay,
    /// Toggle help overlay.
    ToggleHelp,
    /// Toggle governance panel (F2).
    ToggleGovernance,
    /// Toggle computer use panel (F3).
    ToggleComputerUse,
    /// Toggle patterns panel (F4).
    TogglePatterns,
    /// Toggle memory panel (F5).
    ToggleMemory,
    /// Clear conversation (Ctrl+L).
    ClearConversation,
    /// Approve consent.
    ApproveConsent,
    /// Deny consent.
    DenyConsent,
    /// Tab completion for slash commands.
    TabComplete,
    /// Paste (Shift+Ctrl+V).
    Paste,
    /// Scroll up by N lines.
    ScrollUp(u16),
    /// Scroll down by N lines.
    ScrollDown(u16),
    /// Input editing — delegate to TuiApp::handle_key.
    InputKey(KeyEvent),
    /// Unhandled key.
    None,
}

/// Process a key event and return the action to take.
///
/// `is_streaming`: whether the agent is currently streaming.
/// `has_consent`: whether a consent modal is open.
/// `has_overlay`: whether help overlay is open.
pub fn process_key(
    key: KeyEvent,
    is_streaming: bool,
    has_consent: bool,
    has_overlay: bool,
) -> KeyAction {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    let shift = key.modifiers.contains(KeyModifiers::SHIFT);

    match key.code {
        // ─── Global ───
        KeyCode::Char('c') if ctrl => {
            if is_streaming {
                KeyAction::CancelStream
            } else {
                KeyAction::Quit
            }
        }
        KeyCode::Char('l') if ctrl => KeyAction::ClearConversation,

        // ─── Paste: Shift+Ctrl+V ───
        KeyCode::Char('v') if ctrl && shift => KeyAction::Paste,

        // ─── Escape ───
        KeyCode::Esc => {
            if has_overlay {
                KeyAction::CloseOverlay
            } else if has_consent {
                KeyAction::DenyConsent
            } else if is_streaming {
                KeyAction::CancelStream
            } else {
                KeyAction::None
            }
        }

        // ─── Enter ───
        KeyCode::Enter if !is_streaming && !has_consent => KeyAction::Submit,

        // ─── F-keys ───
        KeyCode::F(1) => KeyAction::ToggleHelp,
        KeyCode::F(2) => KeyAction::ToggleGovernance,
        KeyCode::F(3) => KeyAction::ToggleComputerUse,
        KeyCode::F(4) => KeyAction::TogglePatterns,
        KeyCode::F(5) => KeyAction::ToggleMemory,

        // ─── Tab ───
        KeyCode::Tab if !is_streaming => KeyAction::TabComplete,

        // ─── Consent keys ───
        KeyCode::Char('a') | KeyCode::Char('A') if has_consent => KeyAction::ApproveConsent,
        KeyCode::Char('d') | KeyCode::Char('D') if has_consent => KeyAction::DenyConsent,

        // ─── Scrolling ───
        KeyCode::PageUp => KeyAction::ScrollUp(10),
        KeyCode::PageDown => KeyAction::ScrollDown(10),
        KeyCode::Up if ctrl => KeyAction::ScrollUp(1),
        KeyCode::Down if ctrl => KeyAction::ScrollDown(1),

        // ─── Input editing ───
        _ if !is_streaming && !has_consent => KeyAction::InputKey(key),

        _ => KeyAction::None,
    }
}
