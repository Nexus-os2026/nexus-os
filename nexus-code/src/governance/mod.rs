//! Governance kernel — the trust boundary for Nexus Code.

pub mod audit;
pub mod capability;
pub mod consent;
pub mod fuel;
pub mod identity;

pub use audit::{AuditAction, AuditEntry, AuditTrail};
pub use capability::{Capability, CapabilityGrant, CapabilityManager, CapabilityScope};
pub use consent::{ConsentDecision, ConsentGate, ConsentOutcome, ConsentRequest, ConsentTier};
pub use fuel::{FuelBudget, FuelCost, FuelMeter};
pub use identity::SessionIdentity;

use std::sync::Arc;

use crate::error::NxError;

/// Result of authorize_tool: either fully authorized, or consent is needed.
pub enum AuthorizationResult {
    /// All gates passed (Tier1 auto-approved). The tool may execute.
    Authorized(ConsentDecision),
    /// Consent required. Present the ConsentRequest to the user.
    /// After user responds, call `finalize_authorization()`.
    ConsentNeeded(ConsentRequest),
}

/// The governance kernel — orchestrates all governance components for a session.
pub struct GovernanceKernel {
    /// Cryptographic session identity.
    pub identity: Arc<SessionIdentity>,
    /// Hash-chained audit trail.
    pub audit: AuditTrail,
    /// Capability-based access control.
    pub capabilities: CapabilityManager,
    /// HITL consent gates.
    pub consent: ConsentGate,
    /// Fuel metering and budgets.
    pub fuel: FuelMeter,
}

impl GovernanceKernel {
    /// Create a new governance kernel with the given fuel budget.
    pub fn new(fuel_budget: u64) -> Result<Self, NxError> {
        let identity = Arc::new(SessionIdentity::new()?);
        let mut audit = AuditTrail::new(identity.clone());
        audit.record(AuditAction::SessionStarted {
            public_key: hex::encode(identity.public_key_bytes()),
        });
        Ok(Self {
            identity,
            audit,
            capabilities: CapabilityManager::with_defaults(),
            consent: ConsentGate::new(),
            fuel: FuelMeter::new(fuel_budget),
        })
    }

    /// Phase 1: Check capability + fuel + consent classification.
    ///
    /// Pipeline:
    /// 1. Capability ACL check -> fail fast if denied
    /// 2. Fuel reservation -> fail fast if exhausted
    /// 3. Consent classification -> auto-approve (Tier1) or return ConsentNeeded
    /// 4. Audit recording
    pub fn authorize_tool(
        &mut self,
        tool_name: &str,
        context: &str,
        estimated_fuel: u64,
    ) -> Result<AuthorizationResult, NxError> {
        // 1. Capability check
        if let Some(required_cap) = Capability::for_tool(tool_name) {
            self.capabilities.check(required_cap, context)?;
            self.audit.record(AuditAction::CapabilityCheck {
                capability: required_cap.as_str().to_string(),
                granted: true,
            });
        }

        // 2. Fuel reservation
        self.fuel.reserve(estimated_fuel)?;

        // 3. Consent
        let outcome = self.consent.prepare(tool_name, context, &self.identity);
        match outcome {
            ConsentOutcome::AutoApproved(decision) => {
                self.audit.record(AuditAction::ConsentGranted {
                    action: tool_name.to_string(),
                });
                Ok(AuthorizationResult::Authorized(decision))
            }
            ConsentOutcome::Required(request) => {
                self.audit.record(AuditAction::ConsentRequested {
                    action: tool_name.to_string(),
                    tier: match request.tier {
                        ConsentTier::Tier1 => 1,
                        ConsentTier::Tier2 => 2,
                        ConsentTier::Tier3 => 3,
                    },
                });
                Ok(AuthorizationResult::ConsentNeeded(request))
            }
        }
    }

    /// Phase 2: Finalize authorization after user provides consent decision.
    pub fn finalize_authorization(
        &mut self,
        request: &ConsentRequest,
        granted: bool,
        estimated_fuel: u64,
    ) -> Result<ConsentDecision, NxError> {
        let decision = self.consent.finalize(&request.id, granted, &self.identity);

        if granted {
            self.audit.record(AuditAction::ConsentGranted {
                action: request.action.clone(),
            });
            Ok(decision)
        } else {
            self.fuel.release_reservation(estimated_fuel);
            self.audit.record(AuditAction::ConsentDenied {
                action: request.action.clone(),
            });
            Err(NxError::ConsentDenied {
                action: request.action.clone(),
            })
        }
    }

    /// Record fuel consumption after a tool/LLM call completes.
    pub fn record_fuel(&mut self, provider: &str, cost: FuelCost) {
        self.fuel.consume(provider, cost);
        self.audit.record(AuditAction::FuelConsumed {
            amount: self.fuel.budget().consumed,
            remaining: self.fuel.remaining(),
        });
    }

    /// End the session cleanly.
    pub fn end_session(&mut self, reason: &str) {
        self.audit.record(AuditAction::SessionEnded {
            reason: reason.to_string(),
        });
    }
}
