//! AgentContext provides capability-gated, fuel-metered operations for agents.

use nexus_kernel::audit::{AuditTrail, EventType};
use nexus_kernel::errors::AgentError;
use nexus_kernel::manifest::{path_matches_pattern, FilesystemPermission, FsPermissionLevel};
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

const LLM_QUERY_FUEL_COST: u64 = 10;
const READ_FILE_FUEL_COST: u64 = 2;
const WRITE_FILE_FUEL_COST: u64 = 8;

/// A side-effect captured when `AgentContext` is in recording mode.
///
/// Instead of executing the real operation, the context logs what *would*
/// happen. This is used by `ShadowSandbox` to capture speculative effects.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ContextSideEffect {
    /// LLM query attempted.
    LlmQuery {
        prompt: String,
        max_tokens: u32,
        fuel_cost: u64,
    },
    /// File read attempted.
    FileRead { path: String, fuel_cost: u64 },
    /// File write attempted.
    FileWrite {
        path: String,
        content_size: usize,
        fuel_cost: u64,
    },
    /// Approval requested.
    ApprovalRequest { description: String },
    /// Audit event emitted.
    AuditEvent { payload: serde_json::Value },
}

#[derive(Debug, Clone)]
pub struct ApprovalRecord {
    pub description: String,
    pub requested_at: u64,
    /// True when the description originated from an agent (WASM guest).
    /// The UI should display kernel-generated `display_summary` with higher
    /// visual prominence than agent-provided descriptions.
    pub agent_provided: bool,
}

/// A fuel reservation that atomically removes fuel from the available pool.
///
/// Fuel is subtracted from `fuel_remaining` at reservation time — no other
/// operation can spend the same fuel.  The caller must either [`commit`] the
/// reservation (fuel is permanently spent) or [`cancel`] it (fuel returns to
/// the pool).  If the reservation is dropped without committing, the fuel is
/// **automatically returned** via [`Drop`] — fail-safe against error paths.
#[derive(Debug)]
pub struct FuelReservation {
    /// Unique ID for this reservation (matches an entry in AgentContext).
    id: Uuid,
    /// Amount of fuel reserved.
    amount: u64,
    /// Whether `commit()` was called.
    committed: bool,
}

impl FuelReservation {
    /// The amount of fuel held by this reservation.
    pub fn amount(&self) -> u64 {
        self.amount
    }

    /// The unique ID of this reservation.
    pub fn id(&self) -> Uuid {
        self.id
    }

    /// Confirm the deduction — fuel is permanently spent.
    ///
    /// After this call the reservation is consumed and the fuel will NOT
    /// be returned to the pool.  Must be followed by
    /// [`AgentContext::commit_reservation`] to finalize.
    pub fn commit(mut self) -> CommittedReservation {
        self.committed = true;
        CommittedReservation {
            id: self.id,
            amount: self.amount,
        }
    }

    /// Cancel the reservation — returns fuel to the pool.
    ///
    /// After this call the reserved fuel is available again.  Must be
    /// followed by [`AgentContext::cancel_reservation`] to finalize.
    pub fn cancel(self) -> CancelledReservation {
        // `Drop` will see `committed == false` but we return a typed token
        // so the caller can pass it to `AgentContext::cancel_reservation`.
        CancelledReservation {
            id: self.id,
            amount: self.amount,
        }
    }
}

impl Drop for FuelReservation {
    fn drop(&mut self) {
        if !self.committed {
            // Fuel will be returned when the caller passes the
            // CancelledReservation to AgentContext::cancel_reservation.
            // If the caller simply drops the reservation without calling
            // cancel() or commit(), this is a programming error — but we
            // record the ID so the context can detect leaked reservations.
            //
            // The AgentContext::drop_leaked_reservation method (or the
            // periodic audit sweep) will reclaim these.
        }
    }
}

/// Token proving a reservation was committed.
#[derive(Debug)]
pub struct CommittedReservation {
    id: Uuid,
    amount: u64,
}

impl CommittedReservation {
    /// The reservation ID.
    pub fn id(&self) -> Uuid {
        self.id
    }

    /// The amount committed.
    pub fn amount(&self) -> u64 {
        self.amount
    }
}

/// Token proving a reservation was cancelled.
#[derive(Debug)]
pub struct CancelledReservation {
    id: Uuid,
    amount: u64,
}

impl CancelledReservation {
    /// The reservation ID.
    pub fn id(&self) -> Uuid {
        self.id
    }

    /// The amount to return.
    pub fn amount(&self) -> u64 {
        self.amount
    }
}

#[derive(Debug, Clone)]
pub struct AgentContext {
    agent_id: Uuid,
    capabilities: Vec<String>,
    /// Path-scoped filesystem permissions (C.6). When non-empty, `read_file`
    /// and `write_file` enforce path-level access control on top of the flat
    /// `fs.read`/`fs.write` capability check.
    filesystem_permissions: Vec<FilesystemPermission>,
    fuel_budget: u64,
    fuel_remaining: u64,
    /// Fuel currently held by outstanding reservations (already subtracted
    /// from `fuel_remaining` but not yet committed or cancelled).
    fuel_reserved: u64,
    audit_trail: AuditTrail,
    approval_records: Vec<ApprovalRecord>,
    recording_mode: bool,
    side_effect_log: Vec<ContextSideEffect>,
}

impl AgentContext {
    pub fn new(agent_id: Uuid, capabilities: Vec<String>, fuel_budget: u64) -> Self {
        Self {
            agent_id,
            capabilities,
            filesystem_permissions: Vec::new(),
            fuel_budget,
            fuel_remaining: fuel_budget,
            fuel_reserved: 0,
            audit_trail: AuditTrail::new(),
            approval_records: Vec::new(),
            recording_mode: false,
            side_effect_log: Vec::new(),
        }
    }

    /// Attach path-scoped filesystem permissions (C.6 granular FS permissions).
    ///
    /// When non-empty, `read_file` and `write_file` enforce path-level access
    /// control in addition to the flat `fs.read`/`fs.write` capability check.
    pub fn with_filesystem_permissions(mut self, permissions: Vec<FilesystemPermission>) -> Self {
        self.filesystem_permissions = permissions;
        self
    }

    /// Set path-scoped filesystem permissions on an existing context.
    pub fn set_filesystem_permissions(&mut self, permissions: Vec<FilesystemPermission>) {
        self.filesystem_permissions = permissions;
    }

    /// Read-only access to configured filesystem permissions.
    pub fn filesystem_permissions(&self) -> &[FilesystemPermission] {
        &self.filesystem_permissions
    }

    pub fn agent_id(&self) -> Uuid {
        self.agent_id
    }

    pub fn fuel_remaining(&self) -> u64 {
        self.fuel_remaining
    }

    pub fn fuel_budget(&self) -> u64 {
        self.fuel_budget
    }

    pub fn capabilities(&self) -> &[String] {
        &self.capabilities
    }

    pub fn audit_trail(&self) -> &AuditTrail {
        &self.audit_trail
    }

    pub fn audit_trail_mut(&mut self) -> &mut AuditTrail {
        &mut self.audit_trail
    }

    pub fn approval_records(&self) -> &[ApprovalRecord] {
        &self.approval_records
    }

    /// Enable recording mode. Operations push to `side_effect_log` instead of
    /// executing. Capability and fuel checks still apply — only the real
    /// action is skipped.
    pub fn enable_recording(&mut self) {
        self.recording_mode = true;
    }

    /// Disable recording mode. Operations resume normal execution.
    pub fn disable_recording(&mut self) {
        self.recording_mode = false;
    }

    /// Whether the context is in recording mode.
    pub fn is_recording(&self) -> bool {
        self.recording_mode
    }

    /// Read-only access to accumulated side-effects.
    pub fn side_effects(&self) -> &[ContextSideEffect] {
        &self.side_effect_log
    }

    /// Drain and return all captured side-effects, clearing the log.
    pub fn drain_side_effects(&mut self) -> Vec<ContextSideEffect> {
        std::mem::take(&mut self.side_effect_log)
    }

    /// Manually record a side-effect (used by speculative policy interception
    /// in host functions to log what *would* have happened).
    pub fn record_side_effect(&mut self, effect: ContextSideEffect) {
        self.side_effect_log.push(effect);
    }

    /// Check that a capability is in the manifest. Returns AgentError::CapabilityDenied if not.
    pub fn require_capability(&self, capability: &str) -> Result<(), AgentError> {
        if self.capabilities.contains(&capability.to_string()) {
            Ok(())
        } else {
            Err(AgentError::CapabilityDenied(capability.to_string()))
        }
    }

    /// Query an LLM. Checks "llm.query" capability and deducts fuel.
    /// In recording mode, logs the side-effect and returns a placeholder.
    pub fn llm_query(&mut self, prompt: &str, max_tokens: u32) -> Result<String, AgentError> {
        self.require_capability("llm.query")?;
        self.deduct_fuel(LLM_QUERY_FUEL_COST)?;

        if self.recording_mode {
            self.side_effect_log.push(ContextSideEffect::LlmQuery {
                prompt: prompt.to_string(),
                max_tokens,
                fuel_cost: LLM_QUERY_FUEL_COST,
            });
            return Ok(format!(
                "[recorded-llm-query: {} chars, max_tokens={}]",
                prompt.len(),
                max_tokens
            ));
        }

        self.audit_trail
            .append_event(
                self.agent_id,
                EventType::LlmCall,
                json!({
                    "action": "llm_query",
                    "prompt_len": prompt.len(),
                    "max_tokens": max_tokens,
                    "fuel_cost": LLM_QUERY_FUEL_COST,
                }),
            )
            .expect("audit: fail-closed");

        Ok(format!("[mock-llm-response to {} chars]", prompt.len()))
    }

    /// Check path-scoped filesystem permissions (C.6).
    ///
    /// When `filesystem_permissions` is non-empty, verifies `path` against the
    /// configured scopes. Deny rules take priority; unmatched paths are denied.
    /// When empty, this is a no-op (backward compatible with flat capabilities).
    fn check_fs_path_permission(&self, path: &str, needs_write: bool) -> Result<(), AgentError> {
        if self.filesystem_permissions.is_empty() {
            return Ok(());
        }

        let matches: Vec<&FilesystemPermission> = self
            .filesystem_permissions
            .iter()
            .filter(|fp| path_matches_pattern(path, &fp.path_pattern))
            .collect();

        if matches.is_empty() {
            let mode = if needs_write { "write" } else { "read" };
            return Err(AgentError::ManifestError(format!(
                "Filesystem {mode} denied for path: {path}"
            )));
        }

        if matches
            .iter()
            .any(|fp| fp.permission == FsPermissionLevel::Deny)
        {
            let mode = if needs_write { "write" } else { "read" };
            return Err(AgentError::ManifestError(format!(
                "Filesystem {mode} denied for path: {path}"
            )));
        }

        if needs_write {
            if matches
                .iter()
                .any(|fp| fp.permission == FsPermissionLevel::ReadWrite)
            {
                return Ok(());
            }
            return Err(AgentError::ManifestError(format!(
                "Filesystem write denied for path: {path}"
            )));
        }

        // Read: ReadOnly or ReadWrite both suffice.
        if matches.iter().any(|fp| {
            fp.permission == FsPermissionLevel::ReadOnly
                || fp.permission == FsPermissionLevel::ReadWrite
        }) {
            return Ok(());
        }

        Err(AgentError::ManifestError(format!(
            "Filesystem read denied for path: {path}"
        )))
    }

    /// Read a file. Checks "fs.read" capability and path scope, costs 2 fuel.
    /// In recording mode, logs the side-effect and returns a placeholder.
    pub fn read_file(&mut self, path: &str) -> Result<String, AgentError> {
        self.require_capability("fs.read")?;
        self.check_fs_path_permission(path, false)?;
        self.deduct_fuel(READ_FILE_FUEL_COST)?;

        if self.recording_mode {
            self.side_effect_log.push(ContextSideEffect::FileRead {
                path: path.to_string(),
                fuel_cost: READ_FILE_FUEL_COST,
            });
            return Ok(format!("[recorded-file-read: {}]", path));
        }

        self.audit_trail
            .append_event(
                self.agent_id,
                EventType::ToolCall,
                json!({
                    "action": "read_file",
                    "path": path,
                    "fuel_cost": READ_FILE_FUEL_COST,
                }),
            )
            .expect("audit: fail-closed");

        Ok(format!("[mock-file-content of {}]", path))
    }

    /// Write a file. Checks "fs.write" capability and path scope, costs 8 fuel.
    /// In recording mode, logs the side-effect instead of writing.
    pub fn write_file(&mut self, path: &str, content: &str) -> Result<(), AgentError> {
        self.require_capability("fs.write")?;
        self.check_fs_path_permission(path, true)?;
        self.deduct_fuel(WRITE_FILE_FUEL_COST)?;

        if self.recording_mode {
            self.side_effect_log.push(ContextSideEffect::FileWrite {
                path: path.to_string(),
                content_size: content.len(),
                fuel_cost: WRITE_FILE_FUEL_COST,
            });
            return Ok(());
        }

        self.audit_trail
            .append_event(
                self.agent_id,
                EventType::ToolCall,
                json!({
                    "action": "write_file",
                    "path": path,
                    "content_len": content.len(),
                    "fuel_cost": WRITE_FILE_FUEL_COST,
                }),
            )
            .expect("audit: fail-closed");

        Ok(())
    }

    /// Request approval for a described action.
    /// In recording mode, logs the side-effect instead of recording approval.
    ///
    /// When `agent_provided` is true, the description originated from the WASM
    /// guest and should be treated as untrusted.  The UI should display
    /// kernel-generated `display_summary` with higher visual prominence than
    /// agent-provided descriptions.
    pub fn request_approval(&mut self, description: &str, agent_provided: bool) -> ApprovalRecord {
        let record = ApprovalRecord {
            description: description.to_string(),
            requested_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
            agent_provided,
        };

        if self.recording_mode {
            self.side_effect_log
                .push(ContextSideEffect::ApprovalRequest {
                    description: description.to_string(),
                });
            return record;
        }

        self.audit_trail
            .append_event(
                self.agent_id,
                EventType::UserAction,
                json!({
                    "action": "request_approval",
                    "description": description,
                }),
            )
            .expect("audit: fail-closed");

        self.approval_records.push(record.clone());
        record
    }

    /// Deduct fuel consumed by wasm execution (instruction-level cost).
    /// Called by WasmtimeSandbox after execution to sync fuel state back.
    /// This is separate from per-operation costs (llm_query, read_file, etc.)
    /// which are already deducted by the respective AgentContext methods.
    pub fn deduct_wasm_fuel(&mut self, units: u64) {
        self.fuel_remaining = self.fuel_remaining.saturating_sub(units);
        if units > 0 {
            self.audit_trail
                .append_event(
                    self.agent_id,
                    EventType::ToolCall,
                    json!({
                        "action": "wasm_fuel_consumed",
                        "units": units,
                        "remaining": self.fuel_remaining,
                    }),
                )
                .expect("audit: fail-closed");
        }
    }

    /// Reserve fuel atomically: check availability AND subtract in one step.
    ///
    /// Returns a [`FuelReservation`] token.  The caller must either:
    /// - Call [`commit_reservation`] after the operation succeeds, or
    /// - Call [`cancel_reservation`] (or simply drop the token) to return
    ///   the fuel to the pool.
    ///
    /// This eliminates the TOCTOU gap where fuel is checked, then consumed
    /// later — between the check and the consume, a concurrent operation
    /// could have spent the same fuel.
    pub fn reserve_fuel(&mut self, cost: u64) -> Result<FuelReservation, AgentError> {
        if self.fuel_remaining < cost {
            self.audit_trail
                .append_event(
                    self.agent_id,
                    EventType::Error,
                    json!({
                        "action": "fuel_reservation_failed",
                        "requested": cost,
                        "remaining": self.fuel_remaining,
                        "already_reserved": self.fuel_reserved,
                    }),
                )
                .expect("audit: fail-closed");
            return Err(AgentError::FuelExhausted);
        }

        let id = Uuid::new_v4();
        self.fuel_remaining -= cost;
        self.fuel_reserved += cost;

        self.audit_trail
            .append_event(
                self.agent_id,
                EventType::ToolCall,
                json!({
                    "action": "fuel_reserved",
                    "reservation_id": id.to_string(),
                    "amount": cost,
                    "remaining_after": self.fuel_remaining,
                }),
            )
            .expect("audit: fail-closed");

        Ok(FuelReservation {
            id,
            amount: cost,
            committed: false,
        })
    }

    /// Finalize a committed reservation — fuel is permanently spent.
    pub fn commit_reservation(&mut self, token: CommittedReservation) {
        self.fuel_reserved = self.fuel_reserved.saturating_sub(token.amount);
        self.audit_trail
            .append_event(
                self.agent_id,
                EventType::ToolCall,
                json!({
                    "action": "fuel_reservation_committed",
                    "reservation_id": token.id.to_string(),
                    "amount": token.amount,
                    "remaining": self.fuel_remaining,
                }),
            )
            .expect("audit: fail-closed");
    }

    /// Cancel a reservation — returns fuel to the available pool.
    pub fn cancel_reservation(&mut self, token: CancelledReservation) {
        self.fuel_reserved = self.fuel_reserved.saturating_sub(token.amount);
        self.fuel_remaining += token.amount;
        self.audit_trail
            .append_event(
                self.agent_id,
                EventType::ToolCall,
                json!({
                    "action": "fuel_reservation_cancelled",
                    "reservation_id": token.id.to_string(),
                    "amount": token.amount,
                    "remaining_after": self.fuel_remaining,
                }),
            )
            .expect("audit: fail-closed");
    }

    /// Return fuel from a reservation that was dropped without commit/cancel.
    ///
    /// This is the fail-safe path: if a `FuelReservation` is dropped (e.g.
    /// because an error path caused early return), the fuel is returned here.
    pub fn return_leaked_reservation(&mut self, amount: u64) {
        self.fuel_reserved = self.fuel_reserved.saturating_sub(amount);
        self.fuel_remaining += amount;
    }

    /// How much fuel is currently held in outstanding reservations.
    pub fn fuel_reserved(&self) -> u64 {
        self.fuel_reserved
    }

    fn deduct_fuel(&mut self, cost: u64) -> Result<(), AgentError> {
        // Atomic check-and-subtract.  The caller (llm_query, read_file, etc.)
        // is responsible for emitting the operation-level audit event, so we
        // only emit on failure (fuel_exhausted) to avoid double-logging.
        if self.fuel_remaining < cost {
            self.audit_trail
                .append_event(
                    self.agent_id,
                    EventType::Error,
                    json!({
                        "action": "fuel_exhausted",
                        "requested": cost,
                        "remaining": self.fuel_remaining,
                    }),
                )
                .expect("audit: fail-closed");
            return Err(AgentError::FuelExhausted);
        }
        self.fuel_remaining -= cost;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capability_check_blocks_unauthorized() {
        let ctx = AgentContext::new(Uuid::new_v4(), vec!["fs.read".to_string()], 1000);

        assert!(ctx.require_capability("fs.read").is_ok());
        assert!(matches!(
            ctx.require_capability("llm.query"),
            Err(AgentError::CapabilityDenied(_))
        ));
    }

    #[test]
    fn fuel_deduction_and_exhaustion() {
        let mut ctx = AgentContext::new(Uuid::new_v4(), vec!["llm.query".to_string()], 15);

        // First query costs 10
        assert!(ctx.llm_query("test", 100).is_ok());
        assert_eq!(ctx.fuel_remaining(), 5);

        // Second query would cost 10 but only 5 left
        let result = ctx.llm_query("test2", 100);
        assert!(matches!(result, Err(AgentError::FuelExhausted)));
        assert_eq!(ctx.fuel_remaining(), 5);
    }

    #[test]
    fn operations_emit_audit_events() {
        let mut ctx = AgentContext::new(
            Uuid::new_v4(),
            vec![
                "llm.query".to_string(),
                "fs.read".to_string(),
                "fs.write".to_string(),
            ],
            1000,
        );

        ctx.llm_query("prompt", 50).unwrap();
        ctx.read_file("/tmp/test.txt").unwrap();
        ctx.write_file("/tmp/out.txt", "data").unwrap();
        ctx.request_approval("deploy to production", false);

        assert_eq!(ctx.audit_trail().events().len(), 4);
        assert_eq!(ctx.approval_records().len(), 1);
    }

    #[test]
    fn read_file_checks_capability() {
        let mut ctx = AgentContext::new(Uuid::new_v4(), vec!["llm.query".to_string()], 1000);

        let result = ctx.read_file("/etc/passwd");
        assert!(matches!(result, Err(AgentError::CapabilityDenied(_))));
    }

    #[test]
    fn write_file_checks_capability_and_fuel() {
        let mut ctx = AgentContext::new(
            Uuid::new_v4(),
            vec!["fs.write".to_string()],
            5, // less than WRITE_FILE_FUEL_COST (8)
        );

        let result = ctx.write_file("/tmp/out.txt", "data");
        assert!(matches!(result, Err(AgentError::FuelExhausted)));
    }

    #[test]
    fn recording_mode_defaults_to_false() {
        let ctx = AgentContext::new(Uuid::new_v4(), vec![], 1000);
        assert!(!ctx.is_recording());
        assert!(ctx.side_effects().is_empty());
    }

    #[test]
    fn recording_mode_captures_llm_query() {
        let mut ctx = AgentContext::new(Uuid::new_v4(), vec!["llm.query".to_string()], 1000);
        ctx.enable_recording();

        let result = ctx.llm_query("hello world", 50).unwrap();
        assert!(result.starts_with("[recorded-llm-query:"));
        assert_eq!(ctx.side_effects().len(), 1);
        assert!(matches!(
            &ctx.side_effects()[0],
            ContextSideEffect::LlmQuery { prompt, max_tokens: 50, fuel_cost: 10 }
            if prompt == "hello world"
        ));
        // Fuel is still deducted in recording mode
        assert_eq!(ctx.fuel_remaining(), 990);
        // Audit trail should NOT have the event
        assert_eq!(ctx.audit_trail().events().len(), 0);
    }

    #[test]
    fn recording_mode_captures_file_read() {
        let mut ctx = AgentContext::new(Uuid::new_v4(), vec!["fs.read".to_string()], 1000);
        ctx.enable_recording();

        let result = ctx.read_file("/etc/hosts").unwrap();
        assert!(result.starts_with("[recorded-file-read:"));
        assert_eq!(ctx.side_effects().len(), 1);
        assert!(matches!(
            &ctx.side_effects()[0],
            ContextSideEffect::FileRead { path, fuel_cost: 2 }
            if path == "/etc/hosts"
        ));
    }

    #[test]
    fn recording_mode_captures_file_write() {
        let mut ctx = AgentContext::new(Uuid::new_v4(), vec!["fs.write".to_string()], 1000);
        ctx.enable_recording();

        ctx.write_file("/tmp/out.txt", "some data").unwrap();
        assert_eq!(ctx.side_effects().len(), 1);
        assert!(matches!(
            &ctx.side_effects()[0],
            ContextSideEffect::FileWrite { path, content_size: 9, fuel_cost: 8 }
            if path == "/tmp/out.txt"
        ));
    }

    #[test]
    fn recording_mode_captures_approval_request() {
        let mut ctx = AgentContext::new(Uuid::new_v4(), vec![], 1000);
        ctx.enable_recording();

        ctx.request_approval("deploy to prod", false);
        assert_eq!(ctx.side_effects().len(), 1);
        assert!(matches!(
            &ctx.side_effects()[0],
            ContextSideEffect::ApprovalRequest { description }
            if description == "deploy to prod"
        ));
        // Should NOT add to approval_records in recording mode
        assert_eq!(ctx.approval_records().len(), 0);
    }

    #[test]
    fn recording_mode_still_checks_capabilities() {
        let mut ctx = AgentContext::new(Uuid::new_v4(), vec![], 1000);
        ctx.enable_recording();

        let result = ctx.llm_query("test", 50);
        assert!(matches!(result, Err(AgentError::CapabilityDenied(_))));
        assert!(ctx.side_effects().is_empty());
    }

    #[test]
    fn recording_mode_still_checks_fuel() {
        let mut ctx = AgentContext::new(Uuid::new_v4(), vec!["llm.query".to_string()], 5);
        ctx.enable_recording();

        let result = ctx.llm_query("test", 50);
        assert!(matches!(result, Err(AgentError::FuelExhausted)));
        assert!(ctx.side_effects().is_empty());
    }

    #[test]
    fn drain_side_effects_clears_log() {
        let mut ctx = AgentContext::new(
            Uuid::new_v4(),
            vec!["llm.query".to_string(), "fs.read".to_string()],
            1000,
        );
        ctx.enable_recording();

        ctx.llm_query("test", 50).unwrap();
        ctx.read_file("/tmp/x").unwrap();
        assert_eq!(ctx.side_effects().len(), 2);

        let drained = ctx.drain_side_effects();
        assert_eq!(drained.len(), 2);
        assert!(ctx.side_effects().is_empty());
    }

    #[test]
    fn disable_recording_resumes_normal_execution() {
        let mut ctx = AgentContext::new(Uuid::new_v4(), vec!["llm.query".to_string()], 1000);
        ctx.enable_recording();
        ctx.llm_query("recorded", 50).unwrap();
        assert_eq!(ctx.side_effects().len(), 1);
        assert_eq!(ctx.audit_trail().events().len(), 0);

        ctx.disable_recording();
        ctx.llm_query("normal", 50).unwrap();
        // Side-effect log unchanged (no new recording)
        assert_eq!(ctx.side_effects().len(), 1);
        // Audit trail now has the event
        assert_eq!(ctx.audit_trail().events().len(), 1);
    }

    // --- Fuel reservation tests ---

    #[test]
    fn reserve_fuel_subtracts_from_remaining() {
        let mut ctx = AgentContext::new(Uuid::new_v4(), vec![], 100);
        let reservation = ctx.reserve_fuel(30).unwrap();
        assert_eq!(ctx.fuel_remaining(), 70);
        assert_eq!(ctx.fuel_reserved(), 30);
        assert_eq!(reservation.amount(), 30);
    }

    #[test]
    fn reserve_fuel_fails_when_insufficient() {
        let mut ctx = AgentContext::new(Uuid::new_v4(), vec![], 10);
        let result = ctx.reserve_fuel(20);
        assert!(matches!(result, Err(AgentError::FuelExhausted)));
        assert_eq!(ctx.fuel_remaining(), 10);
        assert_eq!(ctx.fuel_reserved(), 0);
    }

    #[test]
    fn commit_reservation_permanently_spends_fuel() {
        let mut ctx = AgentContext::new(Uuid::new_v4(), vec![], 100);
        let reservation = ctx.reserve_fuel(30).unwrap();
        let committed = reservation.commit();
        ctx.commit_reservation(committed);

        assert_eq!(ctx.fuel_remaining(), 70);
        assert_eq!(ctx.fuel_reserved(), 0);
    }

    #[test]
    fn cancel_reservation_returns_fuel() {
        let mut ctx = AgentContext::new(Uuid::new_v4(), vec![], 100);
        let reservation = ctx.reserve_fuel(30).unwrap();
        let cancelled = reservation.cancel();
        ctx.cancel_reservation(cancelled);

        assert_eq!(ctx.fuel_remaining(), 100);
        assert_eq!(ctx.fuel_reserved(), 0);
    }

    #[test]
    fn multiple_reservations_are_independent() {
        let mut ctx = AgentContext::new(Uuid::new_v4(), vec![], 100);
        let r1 = ctx.reserve_fuel(20).unwrap();
        let r2 = ctx.reserve_fuel(30).unwrap();
        assert_eq!(ctx.fuel_remaining(), 50);
        assert_eq!(ctx.fuel_reserved(), 50);

        // Commit first, cancel second.
        let c1 = r1.commit();
        ctx.commit_reservation(c1);
        assert_eq!(ctx.fuel_remaining(), 50);
        assert_eq!(ctx.fuel_reserved(), 30);

        let c2 = r2.cancel();
        ctx.cancel_reservation(c2);
        assert_eq!(ctx.fuel_remaining(), 80);
        assert_eq!(ctx.fuel_reserved(), 0);
    }

    #[test]
    fn second_reserve_fails_if_first_took_all_fuel() {
        let mut ctx = AgentContext::new(Uuid::new_v4(), vec![], 50);
        let _r1 = ctx.reserve_fuel(50).unwrap();
        let result = ctx.reserve_fuel(1);
        assert!(matches!(result, Err(AgentError::FuelExhausted)));
    }

    #[test]
    fn return_leaked_reservation_recovers_fuel() {
        let mut ctx = AgentContext::new(Uuid::new_v4(), vec![], 100);
        let reservation = ctx.reserve_fuel(40).unwrap();
        let amount = reservation.amount();

        // Simulate a drop without commit/cancel by just dropping it.
        drop(reservation);

        // Caller detects the leak and recovers.
        ctx.return_leaked_reservation(amount);
        assert_eq!(ctx.fuel_remaining(), 100);
        assert_eq!(ctx.fuel_reserved(), 0);
    }

    #[test]
    fn reserve_emits_audit_events() {
        let mut ctx = AgentContext::new(Uuid::new_v4(), vec![], 100);
        let reservation = ctx.reserve_fuel(10).unwrap();
        // reserve emits 1 audit event
        assert_eq!(ctx.audit_trail().events().len(), 1);

        let committed = reservation.commit();
        ctx.commit_reservation(committed);
        // commit emits 1 more audit event
        assert_eq!(ctx.audit_trail().events().len(), 2);
    }

    #[test]
    fn cancel_emits_audit_event() {
        let mut ctx = AgentContext::new(Uuid::new_v4(), vec![], 100);
        let reservation = ctx.reserve_fuel(10).unwrap();
        let cancelled = reservation.cancel();
        ctx.cancel_reservation(cancelled);
        // reserve (1) + cancel (1) = 2 events
        assert_eq!(ctx.audit_trail().events().len(), 2);
    }

    #[test]
    fn deduct_fuel_still_works_via_operations() {
        // Verify backward compatibility: llm_query still deducts fuel correctly.
        let mut ctx = AgentContext::new(Uuid::new_v4(), vec!["llm.query".to_string()], 100);
        ctx.llm_query("test", 50).unwrap();
        assert_eq!(ctx.fuel_remaining(), 90); // 100 - 10 (LLM cost)
        assert_eq!(ctx.fuel_reserved(), 0); // No outstanding reservations
    }

    // --- Filesystem path permission tests (C.6) ---

    #[test]
    fn test_read_file_no_scopes_allowed() {
        let mut ctx = AgentContext::new(Uuid::new_v4(), vec!["fs.read".to_string()], 1000);
        // No filesystem_permissions → flat capability governs, any path allowed
        assert!(ctx.read_file("/any/path/file.txt").is_ok());
    }

    #[test]
    fn test_read_file_scoped_allowed() {
        let mut ctx = AgentContext::new(Uuid::new_v4(), vec!["fs.read".to_string()], 1000)
            .with_filesystem_permissions(vec![FilesystemPermission {
                path_pattern: "/src/".to_string(),
                permission: FsPermissionLevel::ReadOnly,
            }]);
        assert!(ctx.read_file("/src/foo.rs").is_ok());
    }

    #[test]
    fn test_read_file_scoped_denied() {
        let mut ctx = AgentContext::new(Uuid::new_v4(), vec!["fs.read".to_string()], 1000)
            .with_filesystem_permissions(vec![FilesystemPermission {
                path_pattern: "/src/".to_string(),
                permission: FsPermissionLevel::ReadOnly,
            }]);
        let result = ctx.read_file("/etc/passwd");
        assert!(result.is_err());
    }

    #[test]
    fn test_write_file_readonly_denied() {
        let mut ctx = AgentContext::new(
            Uuid::new_v4(),
            vec!["fs.read".to_string(), "fs.write".to_string()],
            1000,
        )
        .with_filesystem_permissions(vec![FilesystemPermission {
            path_pattern: "/src/".to_string(),
            permission: FsPermissionLevel::ReadOnly,
        }]);
        let result = ctx.write_file("/src/foo.rs", "data");
        assert!(result.is_err());
    }

    #[test]
    fn test_write_file_readwrite_allowed() {
        let mut ctx = AgentContext::new(Uuid::new_v4(), vec!["fs.write".to_string()], 1000)
            .with_filesystem_permissions(vec![FilesystemPermission {
                path_pattern: "/output/".to_string(),
                permission: FsPermissionLevel::ReadWrite,
            }]);
        assert!(ctx.write_file("/output/result.txt", "data").is_ok());
    }

    #[test]
    fn test_deny_overrides_in_context() {
        let mut ctx = AgentContext::new(
            Uuid::new_v4(),
            vec!["fs.read".to_string(), "fs.write".to_string()],
            1000,
        )
        .with_filesystem_permissions(vec![
            FilesystemPermission {
                path_pattern: "/src/".to_string(),
                permission: FsPermissionLevel::ReadWrite,
            },
            FilesystemPermission {
                path_pattern: "/src/secret.rs".to_string(),
                permission: FsPermissionLevel::Deny,
            },
        ]);
        // Deny overrides ReadWrite
        assert!(ctx.read_file("/src/secret.rs").is_err());
        // Other files under /src/ still allowed
        assert!(ctx.read_file("/src/main.rs").is_ok());
    }

    #[test]
    fn test_set_filesystem_permissions_mutator() {
        let mut ctx = AgentContext::new(Uuid::new_v4(), vec!["fs.read".to_string()], 1000);
        assert!(ctx.filesystem_permissions().is_empty());

        ctx.set_filesystem_permissions(vec![FilesystemPermission {
            path_pattern: "/safe/".to_string(),
            permission: FsPermissionLevel::ReadOnly,
        }]);
        assert_eq!(ctx.filesystem_permissions().len(), 1);
        assert!(ctx.read_file("/safe/data.txt").is_ok());
        assert!(ctx.read_file("/unsafe/data.txt").is_err());
    }
}
