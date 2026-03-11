//! Capability delegation with transitive trust, fuel tracking, and cascade revocation.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;
use uuid::Uuid;

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DelegationConstraints {
    pub max_fuel: u64,
    pub max_duration_secs: u64,
    pub max_depth: u8,
    pub require_approval: bool,
}

impl Default for DelegationConstraints {
    fn default() -> Self {
        Self {
            max_fuel: 1000,
            max_duration_secs: 3600,
            max_depth: 1,
            require_approval: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DelegationGrant {
    pub id: Uuid,
    pub grantor: Uuid,
    pub grantee: Uuid,
    pub capabilities: Vec<String>,
    pub constraints: DelegationConstraints,
    pub chain: Vec<Uuid>,
    pub created_at: u64,
    pub expires_at: u64,
    pub revoked: bool,
    pub fuel_used: u64,
}

impl DelegationGrant {
    fn is_active(&self) -> bool {
        !self.revoked && unix_now() < self.expires_at && self.fuel_used < self.constraints.max_fuel
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum DelegationError {
    #[error("grantor does not own capability: {0}")]
    CapabilityNotOwned(String),
    #[error("delegation depth exceeded")]
    DepthExceeded,
    #[error("delegation grant expired")]
    Expired,
    #[error("delegation grant revoked")]
    Revoked,
    #[error("delegated fuel budget exhausted")]
    FuelExhausted,
    #[error("delegation grant not found")]
    NotFound,
}

#[derive(Debug)]
pub struct DelegationEngine {
    grants: HashMap<Uuid, DelegationGrant>,
    agent_capabilities: HashMap<Uuid, Vec<String>>,
}

impl DelegationEngine {
    pub fn new() -> Self {
        Self {
            grants: HashMap::new(),
            agent_capabilities: HashMap::new(),
        }
    }

    pub fn register_agent(&mut self, id: Uuid, capabilities: Vec<String>) {
        self.agent_capabilities.insert(id, capabilities);
    }

    /// Check if an agent has a capability — either owned or via active delegation.
    ///
    /// TODO(C.6): When filesystem_permissions are delegated, the grantee's
    /// path scopes should be the intersection of the grantor's scopes and the
    /// delegation request. For now, only flat capability strings are delegated.
    pub fn has_capability(&self, agent_id: Uuid, capability: &str) -> bool {
        // Check own capabilities
        if let Some(caps) = self.agent_capabilities.get(&agent_id) {
            if caps.contains(&capability.to_string()) {
                return true;
            }
        }
        // Check active delegation grants
        self.grants.values().any(|g| {
            g.grantee == agent_id
                && g.is_active()
                && g.capabilities.contains(&capability.to_string())
        })
    }

    /// Delegate capabilities from grantor to grantee.
    /// The grantor must own (directly or via delegation) every capability being delegated.
    pub fn delegate(
        &mut self,
        grantor: Uuid,
        grantee: Uuid,
        capabilities: Vec<String>,
        constraints: DelegationConstraints,
    ) -> Result<DelegationGrant, DelegationError> {
        // Verify grantor owns all capabilities
        for cap in &capabilities {
            if !self.has_capability(grantor, cap) {
                return Err(DelegationError::CapabilityNotOwned(cap.clone()));
            }
        }

        // Build the delegation chain
        let chain = self.build_chain(grantor, &capabilities);

        // Check depth
        // chain length = number of delegation hops + 1 (the original owner)
        // So current depth = chain.len() - 1 (for direct) or chain.len() for the new grant
        // A grant from A->B has chain [A], depth 1. B->C has chain [A, B], depth 2.
        // max_depth=1 means only direct delegation (chain = [grantor] only).
        let current_depth = chain.len() as u8;
        if current_depth > constraints.max_depth {
            return Err(DelegationError::DepthExceeded);
        }

        let now = unix_now();
        let grant = DelegationGrant {
            id: Uuid::new_v4(),
            grantor,
            grantee,
            capabilities,
            constraints: constraints.clone(),
            chain,
            created_at: now,
            expires_at: now + constraints.max_duration_secs,
            revoked: false,
            fuel_used: 0,
        };

        let result = grant.clone();
        self.grants.insert(grant.id, grant);
        Ok(result)
    }

    /// Build the chain for a new delegation. If grantor is using a delegated
    /// capability, extend that chain. Otherwise start with just [grantor].
    fn build_chain(&self, grantor: Uuid, capabilities: &[String]) -> Vec<Uuid> {
        // Find if grantor has these capabilities via delegation
        for g in self.grants.values() {
            if g.grantee == grantor
                && g.is_active()
                && capabilities.iter().all(|c| g.capabilities.contains(c))
            {
                let mut chain = g.chain.clone();
                chain.push(grantor);
                return chain;
            }
        }
        // Grantor owns directly
        vec![grantor]
    }

    /// Consume fuel against a delegation grant's budget.
    pub fn consume_delegated_fuel(
        &mut self,
        grant_id: Uuid,
        amount: u64,
    ) -> Result<(), DelegationError> {
        let grant = self
            .grants
            .get_mut(&grant_id)
            .ok_or(DelegationError::NotFound)?;

        if grant.revoked {
            return Err(DelegationError::Revoked);
        }
        if unix_now() >= grant.expires_at {
            return Err(DelegationError::Expired);
        }
        if grant.fuel_used + amount > grant.constraints.max_fuel {
            return Err(DelegationError::FuelExhausted);
        }

        grant.fuel_used += amount;
        Ok(())
    }

    /// Revoke a grant and cascade to any grants derived from it.
    pub fn revoke(&mut self, grant_id: Uuid) -> Result<(), DelegationError> {
        let grant = self
            .grants
            .get_mut(&grant_id)
            .ok_or(DelegationError::NotFound)?;
        grant.revoked = true;

        let grantee = grant.grantee;
        let capabilities = grant.capabilities.clone();

        // Cascade: revoke any grant where the revoked grant's grantee is the grantor
        // and the capabilities overlap
        let cascade_ids: Vec<Uuid> = self
            .grants
            .values()
            .filter(|g| {
                !g.revoked
                    && g.grantor == grantee
                    && g.capabilities.iter().any(|c| capabilities.contains(c))
            })
            .map(|g| g.id)
            .collect();

        for id in cascade_ids {
            // Recursive cascade
            let _ = self.revoke(id);
        }

        Ok(())
    }

    /// Scan all grants, mark expired ones, return their IDs.
    pub fn expire_grants(&mut self) -> Vec<Uuid> {
        let now = unix_now();
        let mut expired = Vec::new();

        for grant in self.grants.values_mut() {
            if !grant.revoked && now >= grant.expires_at {
                grant.revoked = true;
                expired.push(grant.id);
            }
        }

        expired
    }

    /// List active (non-revoked, non-expired) grants where agent is grantee.
    pub fn active_grants_for(&self, agent_id: Uuid) -> Vec<&DelegationGrant> {
        self.grants
            .values()
            .filter(|g| g.grantee == agent_id && g.is_active())
            .collect()
    }
}

impl Default for DelegationEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_engine() -> (DelegationEngine, Uuid, Uuid, Uuid) {
        let mut engine = DelegationEngine::new();
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        let c = Uuid::new_v4();

        engine.register_agent(
            a,
            vec![
                "llm.query".to_string(),
                "file.read".to_string(),
                "file.write".to_string(),
            ],
        );
        engine.register_agent(b, vec![]);
        engine.register_agent(c, vec![]);

        (engine, a, b, c)
    }

    #[test]
    fn delegate_subset_succeeds() {
        let (mut engine, a, b, _) = setup_engine();

        let grant = engine.delegate(
            a,
            b,
            vec!["llm.query".to_string(), "file.read".to_string()],
            DelegationConstraints::default(),
        );
        assert!(grant.is_ok());

        let grant = grant.unwrap();
        assert_eq!(grant.grantor, a);
        assert_eq!(grant.grantee, b);
        assert_eq!(grant.capabilities.len(), 2);
        assert_eq!(grant.chain, vec![a]);
        assert!(!grant.revoked);
        assert_eq!(grant.fuel_used, 0);
    }

    #[test]
    fn delegate_capability_not_owned_fails() {
        let (mut engine, _, b, c) = setup_engine();
        // b has no capabilities
        let result = engine.delegate(
            b,
            c,
            vec!["llm.query".to_string()],
            DelegationConstraints::default(),
        );
        assert_eq!(
            result.unwrap_err(),
            DelegationError::CapabilityNotOwned("llm.query".to_string())
        );
    }

    #[test]
    fn transitive_delegation_within_depth() {
        let (mut engine, a, b, c) = setup_engine();

        // A -> B (depth 1, chain [A])
        let _ab = engine
            .delegate(
                a,
                b,
                vec!["llm.query".to_string()],
                DelegationConstraints {
                    max_depth: 2, // allow transitive
                    ..Default::default()
                },
            )
            .unwrap();

        // B -> C (depth 2, chain [A, B])
        let bc = engine
            .delegate(
                b,
                c,
                vec!["llm.query".to_string()],
                DelegationConstraints {
                    max_depth: 2,
                    ..Default::default()
                },
            )
            .unwrap();

        assert_eq!(bc.chain, vec![a, b]);
        assert!(engine.has_capability(c, "llm.query"));
    }

    #[test]
    fn depth_exceeded_fails() {
        let (mut engine, a, b, c) = setup_engine();

        // A -> B with max_depth=1
        engine
            .delegate(
                a,
                b,
                vec!["llm.query".to_string()],
                DelegationConstraints {
                    max_depth: 1,
                    ..Default::default()
                },
            )
            .unwrap();

        // B -> C should fail: chain would be [A, B] = depth 2, but max_depth=1
        let result = engine.delegate(
            b,
            c,
            vec!["llm.query".to_string()],
            DelegationConstraints {
                max_depth: 1,
                ..Default::default()
            },
        );
        assert_eq!(result.unwrap_err(), DelegationError::DepthExceeded);
    }

    #[test]
    fn revoke_cascades() {
        let (mut engine, a, b, c) = setup_engine();

        // A -> B
        let ab = engine
            .delegate(
                a,
                b,
                vec!["llm.query".to_string()],
                DelegationConstraints {
                    max_depth: 2,
                    ..Default::default()
                },
            )
            .unwrap();

        // B -> C
        let bc = engine
            .delegate(
                b,
                c,
                vec!["llm.query".to_string()],
                DelegationConstraints {
                    max_depth: 2,
                    ..Default::default()
                },
            )
            .unwrap();

        assert!(engine.has_capability(b, "llm.query"));
        assert!(engine.has_capability(c, "llm.query"));

        // Revoke A -> B, should cascade to B -> C
        engine.revoke(ab.id).unwrap();

        assert!(!engine.has_capability(b, "llm.query"));
        assert!(!engine.has_capability(c, "llm.query"));

        // Both grants should be revoked
        assert!(engine.grants.get(&ab.id).unwrap().revoked);
        assert!(engine.grants.get(&bc.id).unwrap().revoked);
    }

    #[test]
    fn expire_auto_revokes() {
        let (mut engine, a, b, _) = setup_engine();

        // Create a grant that expires immediately (0 duration)
        let grant = engine
            .delegate(
                a,
                b,
                vec!["file.read".to_string()],
                DelegationConstraints {
                    max_duration_secs: 0,
                    ..Default::default()
                },
            )
            .unwrap();

        // The grant is already expired since expires_at = now + 0
        let expired = engine.expire_grants();
        assert!(expired.contains(&grant.id));
        assert!(!engine.has_capability(b, "file.read"));
    }

    #[test]
    fn fuel_tracking_on_delegated_grant() {
        let (mut engine, a, b, _) = setup_engine();

        let grant = engine
            .delegate(
                a,
                b,
                vec!["llm.query".to_string()],
                DelegationConstraints {
                    max_fuel: 100,
                    ..Default::default()
                },
            )
            .unwrap();

        // Consume some fuel
        assert!(engine.consume_delegated_fuel(grant.id, 50).is_ok());
        assert_eq!(engine.grants.get(&grant.id).unwrap().fuel_used, 50);

        // Consume more
        assert!(engine.consume_delegated_fuel(grant.id, 30).is_ok());
        assert_eq!(engine.grants.get(&grant.id).unwrap().fuel_used, 80);
    }

    #[test]
    fn fuel_exhaustion_blocks_further_use() {
        let (mut engine, a, b, _) = setup_engine();

        let grant = engine
            .delegate(
                a,
                b,
                vec!["llm.query".to_string()],
                DelegationConstraints {
                    max_fuel: 100,
                    ..Default::default()
                },
            )
            .unwrap();

        assert!(engine.consume_delegated_fuel(grant.id, 90).is_ok());
        // Trying to use 20 more would exceed 100
        assert_eq!(
            engine.consume_delegated_fuel(grant.id, 20),
            Err(DelegationError::FuelExhausted)
        );

        // But 10 is fine (exactly at limit)
        assert!(engine.consume_delegated_fuel(grant.id, 10).is_ok());

        // Now at limit, 1 more fails
        assert_eq!(
            engine.consume_delegated_fuel(grant.id, 1),
            Err(DelegationError::FuelExhausted)
        );
    }

    #[test]
    fn has_capability_checks_own_and_delegated() {
        let (mut engine, a, b, _) = setup_engine();

        // A has own capabilities
        assert!(engine.has_capability(a, "llm.query"));
        assert!(engine.has_capability(a, "file.read"));

        // B has none of its own
        assert!(!engine.has_capability(b, "llm.query"));

        // Delegate to B
        engine
            .delegate(
                a,
                b,
                vec!["llm.query".to_string()],
                DelegationConstraints::default(),
            )
            .unwrap();

        // B now has llm.query via delegation
        assert!(engine.has_capability(b, "llm.query"));
        // But not file.read (wasn't delegated)
        assert!(!engine.has_capability(b, "file.read"));
    }

    #[test]
    fn active_grants_for_agent() {
        let (mut engine, a, b, _) = setup_engine();

        engine
            .delegate(
                a,
                b,
                vec!["llm.query".to_string()],
                DelegationConstraints::default(),
            )
            .unwrap();

        let revokable = engine
            .delegate(
                a,
                b,
                vec!["file.read".to_string()],
                DelegationConstraints::default(),
            )
            .unwrap();

        assert_eq!(engine.active_grants_for(b).len(), 2);

        engine.revoke(revokable.id).unwrap();
        assert_eq!(engine.active_grants_for(b).len(), 1);
    }

    #[test]
    fn revoke_nonexistent_returns_not_found() {
        let mut engine = DelegationEngine::new();
        assert_eq!(
            engine.revoke(Uuid::new_v4()),
            Err(DelegationError::NotFound)
        );
    }

    #[test]
    fn consume_fuel_on_revoked_grant_fails() {
        let (mut engine, a, b, _) = setup_engine();

        let grant = engine
            .delegate(
                a,
                b,
                vec!["llm.query".to_string()],
                DelegationConstraints::default(),
            )
            .unwrap();

        engine.revoke(grant.id).unwrap();
        assert_eq!(
            engine.consume_delegated_fuel(grant.id, 1),
            Err(DelegationError::Revoked)
        );
    }
}
