//! Memory access control tied to agent autonomy levels.
//!
//! ACL enforcement is what makes memory a kernel subsystem — access is
//! enforced, not advisory.  An L0 agent cannot write semantic memory
//! regardless of how hard it tries.

use std::collections::HashMap;

use crate::types::*;

/// Memory access control list for an agent's memory space.
pub struct MemoryAcl {
    agent_id: String,
    autonomy_level: u8,
    grants: HashMap<String, MemoryAccess>,
}

impl MemoryAcl {
    /// Creates a new ACL for the given agent and autonomy level.
    pub fn new(agent_id: String, autonomy_level: u8) -> Self {
        Self {
            agent_id,
            autonomy_level,
            grants: HashMap::new(),
        }
    }

    /// Returns `true` if `accessor_id` can read the given memory type.
    pub fn can_read(&self, accessor_id: &str, memory_type: MemoryType) -> bool {
        if accessor_id == self.agent_id {
            return true; // owner can always read all types
        }
        self.grants
            .get(accessor_id)
            .is_some_and(|g| g.read.contains(&memory_type))
    }

    /// Returns `true` if `accessor_id` can write the given memory type.
    pub fn can_write(&self, accessor_id: &str, memory_type: MemoryType) -> bool {
        if accessor_id == self.agent_id {
            // Owner write rules by autonomy level
            return match memory_type {
                MemoryType::Working | MemoryType::Episodic => true,
                MemoryType::Semantic => self.autonomy_level >= 2,
                MemoryType::Procedural => self.autonomy_level >= 4,
            };
        }
        // Non-owner: explicit grant required AND never Procedural
        if memory_type == MemoryType::Procedural {
            return false; // safety invariant
        }
        self.grants
            .get(accessor_id)
            .is_some_and(|g| g.write.contains(&memory_type))
    }

    /// Returns `true` if `accessor_id` can search the memory space.
    pub fn can_search(&self, accessor_id: &str) -> bool {
        if accessor_id == self.agent_id {
            return true;
        }
        self.grants.get(accessor_id).is_some_and(|g| g.search)
    }

    /// Returns `true` if `accessor_id` can share memory.
    pub fn can_share(&self, accessor_id: &str) -> bool {
        if accessor_id == self.agent_id {
            return self.autonomy_level >= 6;
        }
        self.grants.get(accessor_id).is_some_and(|g| g.share)
    }

    /// Grants access to another agent.
    pub fn grant_access(
        &mut self,
        grantee_id: &str,
        mut access: MemoryAccess,
    ) -> Result<(), MemoryError> {
        // Strip Procedural from write grants (safety invariant)
        access.write.retain(|t| *t != MemoryType::Procedural);

        self.grants.insert(grantee_id.to_string(), access);
        Ok(())
    }

    /// Revokes access for another agent.
    pub fn revoke_access(&mut self, grantee_id: &str) -> Result<(), MemoryError> {
        if self.grants.remove(grantee_id).is_none() {
            return Err(MemoryError::ValidationError(format!(
                "No grant exists for {grantee_id}"
            )));
        }
        Ok(())
    }

    /// Returns all current grants.
    pub fn get_grants(&self) -> &HashMap<String, MemoryAccess> {
        &self.grants
    }

    /// Updates the autonomy level (e.g. after promotion or governance).
    pub fn update_autonomy_level(&mut self, new_level: u8) {
        self.autonomy_level = new_level;
    }

    /// Returns the current autonomy level.
    pub fn autonomy_level(&self) -> u8 {
        self.autonomy_level
    }

    /// Returns the effective permissions for an accessor.
    pub fn effective_permissions(&self, accessor_id: &str) -> MemoryAccess {
        let all_types = [
            MemoryType::Working,
            MemoryType::Episodic,
            MemoryType::Semantic,
            MemoryType::Procedural,
        ];

        let read: Vec<MemoryType> = all_types
            .iter()
            .filter(|t| self.can_read(accessor_id, **t))
            .copied()
            .collect();

        let write: Vec<MemoryType> = all_types
            .iter()
            .filter(|t| self.can_write(accessor_id, **t))
            .copied()
            .collect();

        MemoryAccess {
            read,
            write,
            search: self.can_search(accessor_id),
            share: self.can_share(accessor_id),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn owner_can_read_all_types() {
        let acl = MemoryAcl::new("agent-1".into(), 0);
        assert!(acl.can_read("agent-1", MemoryType::Working));
        assert!(acl.can_read("agent-1", MemoryType::Episodic));
        assert!(acl.can_read("agent-1", MemoryType::Semantic));
        assert!(acl.can_read("agent-1", MemoryType::Procedural));
    }

    #[test]
    fn owner_l0_l1_write_restrictions() {
        for level in 0..=1 {
            let acl = MemoryAcl::new("a".into(), level);
            assert!(acl.can_write("a", MemoryType::Working));
            assert!(acl.can_write("a", MemoryType::Episodic));
            assert!(!acl.can_write("a", MemoryType::Semantic));
            assert!(!acl.can_write("a", MemoryType::Procedural));
        }
    }

    #[test]
    fn owner_l2_l3_write_restrictions() {
        for level in 2..=3 {
            let acl = MemoryAcl::new("a".into(), level);
            assert!(acl.can_write("a", MemoryType::Working));
            assert!(acl.can_write("a", MemoryType::Episodic));
            assert!(acl.can_write("a", MemoryType::Semantic));
            assert!(!acl.can_write("a", MemoryType::Procedural));
        }
    }

    #[test]
    fn owner_l4_l5_write_all() {
        for level in 4..=5 {
            let acl = MemoryAcl::new("a".into(), level);
            assert!(acl.can_write("a", MemoryType::Working));
            assert!(acl.can_write("a", MemoryType::Episodic));
            assert!(acl.can_write("a", MemoryType::Semantic));
            assert!(acl.can_write("a", MemoryType::Procedural));
        }
    }

    #[test]
    fn owner_l6_write_all() {
        let acl = MemoryAcl::new("a".into(), 6);
        assert!(acl.can_write("a", MemoryType::Working));
        assert!(acl.can_write("a", MemoryType::Episodic));
        assert!(acl.can_write("a", MemoryType::Semantic));
        assert!(acl.can_write("a", MemoryType::Procedural));
    }

    #[test]
    fn non_owner_denied_without_grant() {
        let acl = MemoryAcl::new("a".into(), 6);
        assert!(!acl.can_read("b", MemoryType::Working));
        assert!(!acl.can_write("b", MemoryType::Working));
        assert!(!acl.can_search("b"));
    }

    #[test]
    fn non_owner_allowed_with_grant() {
        let mut acl = MemoryAcl::new("a".into(), 6);
        acl.grant_access(
            "b",
            MemoryAccess {
                read: vec![MemoryType::Working, MemoryType::Semantic],
                write: vec![MemoryType::Working],
                search: true,
                share: false,
            },
        )
        .unwrap();

        assert!(acl.can_read("b", MemoryType::Working));
        assert!(acl.can_read("b", MemoryType::Semantic));
        assert!(!acl.can_read("b", MemoryType::Episodic));
        assert!(acl.can_write("b", MemoryType::Working));
        assert!(!acl.can_write("b", MemoryType::Semantic));
        assert!(acl.can_search("b"));
    }

    #[test]
    fn non_owner_can_never_write_procedural() {
        let mut acl = MemoryAcl::new("a".into(), 6);
        // Even if we try to grant Procedural write, it gets stripped
        acl.grant_access(
            "b",
            MemoryAccess {
                read: vec![MemoryType::Procedural],
                write: vec![MemoryType::Procedural, MemoryType::Working],
                search: true,
                share: false,
            },
        )
        .unwrap();

        assert!(acl.can_read("b", MemoryType::Procedural));
        assert!(!acl.can_write("b", MemoryType::Procedural));
        assert!(acl.can_write("b", MemoryType::Working));
    }

    #[test]
    fn share_only_l6_owner() {
        for level in 0..=5 {
            let acl = MemoryAcl::new("a".into(), level);
            assert!(!acl.can_share("a"), "L{level} should not share");
        }
        let acl = MemoryAcl::new("a".into(), 6);
        assert!(acl.can_share("a"));
    }

    #[test]
    fn grant_and_revoke() {
        let mut acl = MemoryAcl::new("a".into(), 6);
        acl.grant_access(
            "b",
            MemoryAccess {
                read: vec![MemoryType::Working],
                write: vec![],
                search: false,
                share: false,
            },
        )
        .unwrap();
        assert!(acl.can_read("b", MemoryType::Working));

        acl.revoke_access("b").unwrap();
        assert!(!acl.can_read("b", MemoryType::Working));
    }

    #[test]
    fn revoke_nonexistent_fails() {
        let mut acl = MemoryAcl::new("a".into(), 6);
        assert!(acl.revoke_access("nobody").is_err());
    }

    #[test]
    fn update_autonomy_changes_permissions() {
        let mut acl = MemoryAcl::new("a".into(), 1);
        assert!(!acl.can_write("a", MemoryType::Semantic));

        acl.update_autonomy_level(3);
        assert!(acl.can_write("a", MemoryType::Semantic));
    }

    #[test]
    fn effective_permissions_owner() {
        let acl = MemoryAcl::new("a".into(), 3);
        let perms = acl.effective_permissions("a");
        assert_eq!(perms.read.len(), 4); // can read all
        assert!(perms.write.contains(&MemoryType::Semantic));
        assert!(!perms.write.contains(&MemoryType::Procedural));
        assert!(perms.search);
        assert!(!perms.share);
    }
}
