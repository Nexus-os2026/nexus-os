//! Page descriptor format + validation. Hole B Layer 3.
//! See v1.1 amendment §6.5.
//!
//! A descriptor may opt in to exercising specific destructive elements
//! (e.g., the delete-project flow against a throwaway project). Such
//! opt-ins are validated at descriptor-load time and are subject to
//! three hard rules:
//!
//! 1. **Protected routes are forbidden.** Opt-ins may never target
//!    routes under `/settings`, `/governance`, or `/memory`. These are
//!    the routes where destructive actions can permanently harm user
//!    state. Even with a throwaway fixture, the scout is not allowed
//!    to exercise them.
//! 2. **Fixture required.** Every opt-in must reference a named
//!    fixture and must set `fixture_required = true`.
//! 3. **Throwaway only.** The referenced fixture must be
//!    `FixtureKind::Throwaway`. Persistent fixtures are rejected —
//!    the only thing a destructive opt-in may destroy is a
//!    deliberately disposable test fixture.

use serde::{Deserialize, Serialize};

use crate::Error;

/// Kind of fixture a descriptor references.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FixtureKind {
    /// Deliberately disposable. Created for the test run and destroyed
    /// after. Destructive opt-ins may only target throwaway fixtures.
    Throwaway,
    /// Long-lived fixture (e.g., a team that exists for every test
    /// run). Destructive opt-ins may NOT target persistent fixtures.
    Persistent,
}

/// A single fixture the descriptor declares.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FixtureRef {
    pub id: String,
    pub kind: FixtureKind,
}

/// A destructive opt-in: a per-element allowance to exercise a
/// destructive action against a specific fixture.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DestructiveOptIn {
    /// DOM id of the element the opt-in permits exercising.
    pub element_id: String,
    /// Must be `true`. Kept as a boolean so a descriptor author
    /// cannot opt in by accident — they must explicitly acknowledge
    /// that a fixture is required.
    pub fixture_required: bool,
    /// The `id` of a `FixtureRef` in the same descriptor.
    pub fixture_id: String,
}

/// A page descriptor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageDescriptor {
    pub route: String,
    pub expected_elements: Option<Vec<String>>,
    pub critical_flows: Option<Vec<String>>,
    pub fixtures: Option<Vec<FixtureRef>>,
    pub destructive_opt_ins: Option<Vec<DestructiveOptIn>>,
}

/// Routes where destructive opt-ins are unconditionally forbidden.
/// See amendment §6.5: "Opt-ins never apply to elements outside
/// Builder (no opt-ins for Settings → Reset, Governance → Revoke
/// Identity, or Memory → Wipe — these are unconditionally skipped,
/// ever)."
const PROTECTED_ROUTE_PATTERNS: &[&str] = &["/settings", "/governance", "/memory"];

impl PageDescriptor {
    /// Validate the descriptor. Called at descriptor-load time.
    ///
    /// Rules (see module docs):
    /// 1. Opt-ins forbidden on `/settings`, `/governance`, `/memory`.
    /// 2. Every opt-in must set `fixture_required = true`.
    /// 3. Every opt-in must reference a fixture present in
    ///    `self.fixtures`.
    /// 4. The referenced fixture must be `FixtureKind::Throwaway`.
    pub fn validate(&self) -> crate::Result<()> {
        let opt_ins = match &self.destructive_opt_ins {
            Some(v) if !v.is_empty() => v,
            _ => return Ok(()),
        };

        // Rule 1: protected routes.
        for pattern in PROTECTED_ROUTE_PATTERNS {
            if self.route.contains(pattern) {
                return Err(Error::InvariantViolation(format!(
                    "PageDescriptor: destructive opt-ins forbidden on protected route '{}' (matched '{}')",
                    self.route, pattern
                )));
            }
        }

        let fixtures = self.fixtures.as_deref().unwrap_or(&[]);
        for opt_in in opt_ins {
            // Rule 2: fixture_required must be true.
            if !opt_in.fixture_required {
                return Err(Error::InvariantViolation(format!(
                    "PageDescriptor: opt-in {} must have fixture_required=true",
                    opt_in.element_id
                )));
            }

            // Rule 3: referenced fixture must exist.
            let fixture = match fixtures.iter().find(|f| f.id == opt_in.fixture_id) {
                Some(f) => f,
                None => {
                    return Err(Error::InvariantViolation(format!(
                        "PageDescriptor: opt-in {} references missing fixture {}",
                        opt_in.element_id, opt_in.fixture_id
                    )))
                }
            };

            // Rule 4: fixture must be throwaway.
            if !matches!(fixture.kind, FixtureKind::Throwaway) {
                return Err(Error::InvariantViolation(format!(
                    "PageDescriptor: opt-in {} references non-throwaway fixture {}",
                    opt_in.element_id, opt_in.fixture_id
                )));
            }
        }

        Ok(())
    }
}
