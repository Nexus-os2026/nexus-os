//! The scout main loop. Phase 1.1 stub: walks the state machine and
//! writes one audit entry per state. No specialist calls. Real loop
//! body lands in Phase 1.2.

use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::driver::state::DriverState;
use crate::governance::acl::Acl;
use crate::governance::audit::{AuditEntry, AuditLog};
use crate::governance::identity::SessionIdentity;
use crate::governance::routing::RoutingTable;

/// The scout driver.
///
/// Owns the session identity, ACL, routing table, audit log, and
/// current state. Phase 1.1 wires these together but does no real work
/// inside `run`.
pub struct Driver {
    identity: SessionIdentity,
    acl: Acl,
    routing: RoutingTable,
    audit: AuditLog,
    state: DriverState,
}

impl Driver {
    /// Construct a new driver writing audit entries to `audit_path`.
    pub fn new(audit_path: PathBuf) -> Self {
        Self {
            identity: SessionIdentity::new(),
            acl: Acl::default_scout(),
            routing: RoutingTable::default_v1_1(),
            audit: AuditLog::new(audit_path),
            state: DriverState::Enumerate,
        }
    }

    /// Walk the state machine to completion. Phase 1.1 emits one
    /// audit entry per state and then halts.
    pub fn run(&mut self) -> crate::Result<()> {
        let mut current = Some(self.state);
        while let Some(s) = current {
            self.state = s;
            let entry = AuditEntry {
                timestamp: now_iso_secs(),
                state: format!("{:?}", s),
                action: "phase1.1-stub".to_string(),
                specialist: None,
                inputs: serde_json::json!({ "session": self.identity.session_id() }),
                output: serde_json::json!({}),
                prev_hash: String::new(),
                hash: String::new(),
            };
            self.audit.append(entry)?;
            current = s.next();
        }
        Ok(())
    }

    /// Read-only view of the session identity.
    pub fn identity(&self) -> &SessionIdentity {
        &self.identity
    }

    /// Read-only view of the ACL.
    pub fn acl(&self) -> &Acl {
        &self.acl
    }

    /// Read-only view of the routing table.
    pub fn routing(&self) -> &RoutingTable {
        &self.routing
    }
}

fn now_iso_secs() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("ts_{}", secs)
}
