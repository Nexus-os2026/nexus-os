use std::collections::HashSet;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::identity::SessionIdentity;

/// Consent tier based on operation risk level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConsentTier {
    /// Auto-approved (read-only operations): file_read, search, glob, lsp_query.
    Tier1,
    /// Requires explicit approval (write operations): file_write, file_edit, git_commit.
    Tier2,
    /// Requires approval + confirmation (destructive): bash, file_delete, git_push.
    Tier3,
}

/// A request for user consent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsentRequest {
    /// Unique request ID.
    pub id: String,
    /// Human-readable action description.
    pub action: String,
    /// Risk tier.
    pub tier: ConsentTier,
    /// Detailed description of what will happen.
    pub details: String,
    /// When the request was created.
    pub timestamp: DateTime<Utc>,
}

/// A decision on a consent request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsentDecision {
    /// The request this decision applies to.
    pub request_id: String,
    /// Whether consent was granted.
    pub granted: bool,
    /// When the decision was made.
    pub decided_at: DateTime<Utc>,
    /// Hex-encoded Ed25519 signature of the decision payload.
    pub signature: String,
}

/// Result of `prepare()`: either auto-approved, or consent is required.
pub enum ConsentOutcome {
    /// Tier1: auto-approved, here's the signed decision.
    AutoApproved(ConsentDecision),
    /// Tier2/3: consent required, present this request to the user.
    Required(ConsentRequest),
}

/// Manages HITL consent gates for tool invocations using a two-phase model.
pub struct ConsentGate {
    auto_approve: HashSet<String>,
    decisions: Vec<ConsentDecision>,
}

impl ConsentGate {
    /// Create a new consent gate with default auto-approve list.
    pub fn new() -> Self {
        let mut auto_approve = HashSet::new();
        for tool in ["file_read", "search", "glob", "lsp_query"] {
            auto_approve.insert(tool.to_string());
        }
        Self {
            auto_approve,
            decisions: Vec::new(),
        }
    }

    /// Classify a tool action into a consent tier.
    pub fn classify(&self, tool_name: &str) -> ConsentTier {
        if self.auto_approve.contains(tool_name) {
            ConsentTier::Tier1
        } else {
            match tool_name {
                "file_write" | "file_edit" | "git_commit" => ConsentTier::Tier2,
                "bash" | "shell" | "file_delete" | "git_push" => ConsentTier::Tier3,
                _ => ConsentTier::Tier2,
            }
        }
    }

    /// Phase 1: Prepare consent for an action.
    /// - Tier1 actions are auto-approved (returns AutoApproved with signed decision).
    /// - Tier2/3 actions return Required with a ConsentRequest for the caller.
    pub fn prepare(
        &mut self,
        tool_name: &str,
        details: &str,
        identity: &SessionIdentity,
    ) -> ConsentOutcome {
        let tier = self.classify(tool_name);
        let request_id = uuid::Uuid::new_v4().to_string();
        let now = Utc::now();

        if tier == ConsentTier::Tier1 {
            let payload = format!("{}:true:{}", request_id, now.to_rfc3339());
            let sig = identity.sign(payload.as_bytes());
            let decision = ConsentDecision {
                request_id,
                granted: true,
                decided_at: now,
                signature: hex::encode(sig.to_bytes()),
            };
            self.decisions.push(decision.clone());
            ConsentOutcome::AutoApproved(decision)
        } else {
            ConsentOutcome::Required(ConsentRequest {
                id: request_id,
                action: tool_name.to_string(),
                tier,
                details: details.to_string(),
                timestamp: now,
            })
        }
    }

    /// Phase 2: Record the user's decision on a consent request.
    /// Called by the REPL after the user approves or denies.
    pub fn finalize(
        &mut self,
        request_id: &str,
        granted: bool,
        identity: &SessionIdentity,
    ) -> ConsentDecision {
        let now = Utc::now();
        let payload = format!("{}:{}:{}", request_id, granted, now.to_rfc3339());
        let sig = identity.sign(payload.as_bytes());
        let decision = ConsentDecision {
            request_id: request_id.to_string(),
            granted,
            decided_at: now,
            signature: hex::encode(sig.to_bytes()),
        };
        self.decisions.push(decision.clone());
        decision
    }

    /// Check if a tool is auto-approved (Tier1).
    pub fn is_auto_approved(&self, tool_name: &str) -> bool {
        self.auto_approve.contains(tool_name)
    }

    /// Add a tool to the auto-approve list.
    pub fn add_auto_approve(&mut self, tool_name: &str) {
        self.auto_approve.insert(tool_name.to_string());
    }

    /// Get all decisions made this session.
    pub fn decisions(&self) -> &[ConsentDecision] {
        &self.decisions
    }
}

impl Default for ConsentGate {
    fn default() -> Self {
        Self::new()
    }
}
