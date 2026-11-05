//! The kernel's agent identity type.
//!
//! Extracted unchanged from `supervisor` (K02) so the retained workspace
//! authority no longer depends on the legacy supervisor module. The identity
//! stays the plain UUID alias: no newtype, no conversion, no new validation.
//! The broker has its own, separate validated-string `AgentId`; the two are
//! deliberately not interchangeable.

use uuid::Uuid;

pub type AgentId = Uuid;
