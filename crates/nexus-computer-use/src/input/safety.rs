use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Mutex;
use std::time::Instant;

use tracing::warn;

use crate::error::ComputerUseError;
use crate::input::keyboard::{is_combo_blocked, KeyAction};
use crate::input::mouse::MouseAction;

/// Maximum actions per second before rate limiting kicks in
const MAX_ACTIONS_PER_SECOND: u32 = 10;

/// Maximum actions without a screenshot before dead man's switch triggers
const MAX_ACTIONS_WITHOUT_SCREENSHOT: u32 = 30;

/// Input safety guard that validates every action before execution
pub struct InputSafetyGuard {
    pub screen_width: u32,
    pub screen_height: u32,
    pub system_keys_allowed: bool,
    actions_without_screenshot: AtomicU32,
    rate_limiter: Mutex<RateLimiter>,
}

struct RateLimiter {
    window_start: Instant,
    count_in_window: u32,
}

impl InputSafetyGuard {
    /// Create a new safety guard with the given screen dimensions
    pub fn new(screen_width: u32, screen_height: u32) -> Self {
        Self {
            screen_width,
            screen_height,
            system_keys_allowed: false,
            actions_without_screenshot: AtomicU32::new(0),
            rate_limiter: Mutex::new(RateLimiter {
                window_start: Instant::now(),
                count_in_window: 0,
            }),
        }
    }

    /// Create a safety guard with system keys allowed
    pub fn with_system_keys(mut self) -> Self {
        self.system_keys_allowed = true;
        self
    }

    /// Reset the dead man's switch counter (called after a screenshot is taken)
    pub fn reset_screenshot_counter(&self) {
        self.actions_without_screenshot.store(0, Ordering::Relaxed);
    }

    /// Check rate limit — returns error if too many actions in current second
    fn check_rate_limit(&self) -> Result<(), ComputerUseError> {
        let mut rl = self.rate_limiter.lock().map_err(|e| {
            ComputerUseError::InputError(format!("Rate limiter lock poisoned: {e}"))
        })?;

        let now = Instant::now();
        let elapsed = now.duration_since(rl.window_start);

        if elapsed.as_secs() >= 1 {
            // New window
            rl.window_start = now;
            rl.count_in_window = 1;
            Ok(())
        } else if rl.count_in_window >= MAX_ACTIONS_PER_SECOND {
            Err(ComputerUseError::RateLimitExceeded {
                max_per_second: MAX_ACTIONS_PER_SECOND,
            })
        } else {
            rl.count_in_window += 1;
            Ok(())
        }
    }

    /// Check the dead man's switch — warns if no screenshot taken recently
    fn check_dead_man_switch(&self) -> Result<(), ComputerUseError> {
        let count = self
            .actions_without_screenshot
            .fetch_add(1, Ordering::Relaxed);
        if count >= MAX_ACTIONS_WITHOUT_SCREENSHOT {
            warn!(
                "Dead man's switch: {} actions without screenshot",
                count + 1
            );
            Err(ComputerUseError::DeadManSwitch {
                actions_without_screenshot: count + 1,
                max_actions: MAX_ACTIONS_WITHOUT_SCREENSHOT,
            })
        } else {
            Ok(())
        }
    }

    /// Validate coordinates are within screen bounds
    fn validate_coords(&self, x: u32, y: u32) -> Result<(), ComputerUseError> {
        if x >= self.screen_width || y >= self.screen_height {
            Err(ComputerUseError::CoordinatesOutOfBounds {
                x,
                y,
                width: self.screen_width,
                height: self.screen_height,
            })
        } else {
            Ok(())
        }
    }

    /// Validate a mouse action before execution
    pub fn validate_mouse_action(&self, action: &MouseAction) -> Result<(), ComputerUseError> {
        self.check_rate_limit()?;
        self.check_dead_man_switch()?;

        match action {
            MouseAction::Click { x, y, .. }
            | MouseAction::DoubleClick { x, y, .. }
            | MouseAction::Move { x, y }
            | MouseAction::Scroll { x, y, .. } => {
                self.validate_coords(*x, *y)?;
            }
            MouseAction::Drag {
                start_x,
                start_y,
                end_x,
                end_y,
            } => {
                self.validate_coords(*start_x, *start_y)?;
                self.validate_coords(*end_x, *end_y)?;
            }
            MouseAction::GetPosition => {
                // No validation needed
            }
        }
        Ok(())
    }

    /// Validate a keyboard action before execution
    pub fn validate_keyboard_action(&self, action: &KeyAction) -> Result<(), ComputerUseError> {
        self.check_rate_limit()?;
        self.check_dead_man_switch()?;

        match action {
            KeyAction::KeyCombo { keys } => {
                let combo = keys.join("+");
                if is_combo_blocked(&combo) && !self.system_keys_allowed {
                    return Err(ComputerUseError::BlockedKeyCombination { combo });
                }
            }
            KeyAction::KeyPress { key } => {
                if is_combo_blocked(key) && !self.system_keys_allowed {
                    return Err(ComputerUseError::BlockedKeyCombination { combo: key.clone() });
                }
            }
            _ => {}
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::mouse::MouseButton;

    #[test]
    fn test_bounds_check_valid() {
        let guard = InputSafetyGuard::new(1920, 1080);
        let action = MouseAction::Click {
            x: 100,
            y: 200,
            button: MouseButton::Left,
        };
        assert!(guard.validate_mouse_action(&action).is_ok());
    }

    #[test]
    fn test_bounds_check_out_of_range() {
        let guard = InputSafetyGuard::new(1920, 1080);
        let action = MouseAction::Click {
            x: 2000,
            y: 200,
            button: MouseButton::Left,
        };
        let result = guard.validate_mouse_action(&action);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ComputerUseError::CoordinatesOutOfBounds { .. }
        ));
    }

    #[test]
    fn test_bounds_check_edge() {
        let guard = InputSafetyGuard::new(1920, 1080);
        // Exactly at boundary should fail (0-indexed, so 1920 is out)
        let action = MouseAction::Move { x: 1920, y: 0 };
        assert!(guard.validate_mouse_action(&action).is_err());

        // One less should pass
        let action = MouseAction::Move { x: 1919, y: 1079 };
        assert!(guard.validate_mouse_action(&action).is_ok());
    }

    #[test]
    fn test_blocked_combo_in_safety_guard() {
        let guard = InputSafetyGuard::new(1920, 1080);
        let action = KeyAction::KeyCombo {
            keys: vec!["alt".into(), "F4".into()],
        };
        let result = guard.validate_keyboard_action(&action);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ComputerUseError::BlockedKeyCombination { .. }
        ));
    }

    #[test]
    fn test_system_keys_grant_allows_blocked() {
        let guard = InputSafetyGuard::new(1920, 1080).with_system_keys();
        let action = KeyAction::KeyCombo {
            keys: vec!["alt".into(), "F4".into()],
        };
        assert!(guard.validate_keyboard_action(&action).is_ok());
    }

    #[test]
    fn test_allowed_combo_passes() {
        let guard = InputSafetyGuard::new(1920, 1080);
        let action = KeyAction::KeyCombo {
            keys: vec!["ctrl".into(), "s".into()],
        };
        assert!(guard.validate_keyboard_action(&action).is_ok());
    }

    #[test]
    fn test_rate_limit_enforced() {
        let guard = InputSafetyGuard::new(3440, 1440);
        // Fire MAX_ACTIONS_PER_SECOND actions — all should succeed
        for _ in 0..MAX_ACTIONS_PER_SECOND {
            let action = MouseAction::Move { x: 100, y: 100 };
            assert!(guard.validate_mouse_action(&action).is_ok());
        }
        // The next one should be rate-limited
        let action = MouseAction::Move { x: 100, y: 100 };
        let result = guard.validate_mouse_action(&action);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ComputerUseError::RateLimitExceeded { .. }
        ));
    }

    #[test]
    fn test_dead_man_switch_triggers() {
        let guard = InputSafetyGuard::new(3440, 1440);
        // Disable rate limiting by creating fresh guard for each batch
        // We need 30 actions to trigger dead man's switch
        for i in 0..MAX_ACTIONS_WITHOUT_SCREENSHOT {
            // Create a new guard each time to avoid rate limit, but share the counter
            // Actually, we need the same guard. Let's just test with a high rate limit window.
            // The rate limit resets each second, so we just need to be within bounds.
            if i > 0 && i % MAX_ACTIONS_PER_SECOND == 0 {
                // Reset the rate limiter by waiting (can't sleep in test, so hack it)
                let mut rl = guard.rate_limiter.lock().expect("lock");
                rl.window_start = Instant::now() - std::time::Duration::from_secs(2);
                rl.count_in_window = 0;
            }
            let action = MouseAction::Move { x: 100, y: 100 };
            assert!(
                guard.validate_mouse_action(&action).is_ok(),
                "action {i} should pass"
            );
        }

        // Reset rate limiter one more time
        {
            let mut rl = guard.rate_limiter.lock().expect("lock");
            rl.window_start = Instant::now() - std::time::Duration::from_secs(2);
            rl.count_in_window = 0;
        }

        // The 31st action should trigger dead man's switch
        let action = MouseAction::Move { x: 100, y: 100 };
        let result = guard.validate_mouse_action(&action);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ComputerUseError::DeadManSwitch { .. }
        ));
    }

    #[test]
    fn test_screenshot_counter_reset() {
        let guard = InputSafetyGuard::new(3440, 1440);
        guard
            .actions_without_screenshot
            .store(25, Ordering::Relaxed);
        guard.reset_screenshot_counter();
        assert_eq!(guard.actions_without_screenshot.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn test_drag_validates_both_endpoints() {
        let guard = InputSafetyGuard::new(1920, 1080);
        // Start in bounds, end out of bounds
        let action = MouseAction::Drag {
            start_x: 100,
            start_y: 100,
            end_x: 2000,
            end_y: 100,
        };
        assert!(guard.validate_mouse_action(&action).is_err());

        // Start out of bounds
        let action = MouseAction::Drag {
            start_x: 2000,
            start_y: 100,
            end_x: 100,
            end_y: 100,
        };
        assert!(guard.validate_mouse_action(&action).is_err());
    }
}
