//! Ruleset versioning and hash comparison.

use crate::rules::GovernanceRuleset;

/// Compare two rulesets and determine if an update is needed.
pub fn has_changed(old: &GovernanceRuleset, new: &GovernanceRuleset) -> bool {
    old.version_hash() != new.version_hash()
}

/// Check if the new ruleset is a valid successor to the old one.
pub fn is_valid_successor(old: &GovernanceRuleset, new: &GovernanceRuleset) -> bool {
    new.version > old.version && new.id == old.id
}
