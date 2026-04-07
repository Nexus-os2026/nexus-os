//! Hole A Layer 2 enforcement — per-app input governance.
//!
//! Wraps `nexus_computer_use::governance::AppGrantManager` to whitelist
//! exactly one window (the Nexus OS main window) for the scout's input
//! ACL. Phase 1.3 ships this as **structural only** — no actual input
//! events cross this boundary because Phase 1.3 specialists parse
//! fixture HTML files, not live windows. Real input gating wires up in
//! Phase 1.3.5 alongside Xvfb. See v1.1 amendment §3.1.
//!
//! ## Strategy
//!
//! `AppGrantManager` doesn't expose a bare "is this wm_class in my
//! allowlist" method — its public denial surface is
//! `validate_action(focused_app, action)`, which returns a
//! `CapabilityDenied` error when the action is not permitted for the
//! given app. We lean on that:
//!
//! 1. At construction, we install an explicit `GrantLevel::Full` grant
//!    for the whitelisted wm_class. This overrides any category
//!    auto-grant for that specific window.
//! 2. At validation, we probe the manager with a benign
//!    `AgentAction::Click` against the target `AppInfo`. If the target
//!    is the whitelisted window, the explicit Full grant matches and
//!    the click is permitted. If the target is a different window, the
//!    grant does not match (substring check) and the action falls
//!    through to the target's category auto-grant, which — for the
//!    categories we care about in the scout's real environment
//!    (Communication, Unknown, System → ReadOnly) — denies the click.
//! 3. We translate `Ok(_)` → `Ok(())` and `Err(CapabilityDenied)` →
//!    `Err(crate::Error::InvariantViolation(..))`.
//!
//! The validation path **actually calls** `AppGrantManager::validate_action`,
//! so this is not a tautological `self.allowed_wm_class == target.wm_class`
//! comparison. The denial test (`tests/input_sandbox.rs`) exercises the
//! real `find_grant` + category fallback code path.
//!
//! Phase 1.3.5 will add PID matching on top of wm_class matching and
//! wire this into the real input event path.

use nexus_computer_use::agent::action::AgentAction;
use nexus_computer_use::error::ComputerUseError;
use nexus_computer_use::governance::app_grant::{AppGrant, AppGrantManager, GrantLevel};
use nexus_computer_use::governance::app_registry::{AppCategory, AppInfo};

use crate::Error;

/// Per-window input sandbox for the scout.
///
/// Holds an `AppGrantManager` seeded with exactly one explicit Full
/// grant for the whitelisted wm_class. The stored `allowed_pid` is
/// recorded for Phase 1.3.5 use and is not yet consulted during
/// validation.
pub struct InputSandbox {
    manager: AppGrantManager,
    allowed_wm_class: String,
    allowed_pid: Option<u32>,
}

impl InputSandbox {
    /// Construct a sandbox that whitelists exactly one window.
    ///
    /// `wm_class` identifies the window class (e.g., `"nexus-os"`).
    /// `pid` is stored but not consulted until Phase 1.3.5.
    pub fn for_nexus_os_window(wm_class: &str, pid: Option<u32>) -> Self {
        let mut manager = AppGrantManager::new();
        let grant = AppGrant::new(
            wm_class,
            AppCategory::NexusOS,
            GrantLevel::Full,
            Vec::new(), // unused when GrantLevel::Full
            "nexus-ui-repair",
            None, // no expiry for the scout's session
        );
        manager.add_grant(grant);
        Self {
            manager,
            allowed_wm_class: wm_class.to_string(),
            allowed_pid: pid,
        }
    }

    /// Validate that `target` is the whitelisted window.
    ///
    /// Probes `AppGrantManager::validate_action` with a benign click
    /// action. If the action is permitted, the window is whitelisted.
    /// If the action is denied, the window is not whitelisted and we
    /// surface `Error::InvariantViolation` with a message containing
    /// the target's wm_class and the phrase `"not whitelisted"`.
    ///
    /// Phase 1.3: this method is called by tests only. Phase 1.3.5
    /// wires it into the real input event path.
    pub fn validate_target_window(&mut self, target: &AppInfo) -> crate::Result<()> {
        let probe = AgentAction::Click {
            x: 0,
            y: 0,
            button: "left".to_string(),
        };
        match self.manager.validate_action(target, &probe) {
            Ok(_) => Ok(()),
            Err(ComputerUseError::CapabilityDenied { capability }) => {
                Err(Error::InvariantViolation(format!(
                    "InputSandbox: window wm_class={} is not whitelisted (manager denied probe: {})",
                    target.wm_class, capability
                )))
            }
            Err(other) => Err(Error::InvariantViolation(format!(
                "InputSandbox: unexpected grant manager error for wm_class={}: {:?}",
                target.wm_class, other
            ))),
        }
    }

    /// The wm_class this sandbox was constructed for.
    pub fn allowed_wm_class(&self) -> &str {
        &self.allowed_wm_class
    }

    /// The PID this sandbox was constructed for (stored for Phase 1.3.5).
    pub fn allowed_pid(&self) -> Option<u32> {
        self.allowed_pid
    }
}
