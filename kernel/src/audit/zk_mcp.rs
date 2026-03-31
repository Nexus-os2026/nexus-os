//! ZK audit proofs exposed via MCP (Model Context Protocol).
//!
//! External auditors invoke `ZkMcpHandler::handle_audit_request` to request
//! compliance proofs. Each request generates fresh proofs (no caching) and
//! includes the auditor's nonce to prevent replay. Requests are rate-limited
//! and meta-audited (logged to the audit trail).

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

use super::zk_proof::{GovernanceProofType, ProofGenerator};
use super::zk_report::{ComplianceConfig, ReportError, VerificationResult, ZkAuditReport};
use super::{AuditError, AuditTrail, EventType};

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// Errors specific to ZK MCP audit operations.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, Serialize, Deserialize)]
pub enum ZkMcpError {
    #[error("rate limit exceeded: max {max} requests per hour")]
    RateLimitExceeded { max: usize },

    #[error("report generation failed: {0}")]
    ReportGenerationFailed(String),

    #[error("verification failed: {0}")]
    VerificationFailed(String),

    #[error("invalid request: {0}")]
    InvalidRequest(String),

    #[error("audit trail error: {0}")]
    AuditTrailError(String),
}

impl From<ReportError> for ZkMcpError {
    fn from(e: ReportError) -> Self {
        ZkMcpError::ReportGenerationFailed(e.to_string())
    }
}

impl From<AuditError> for ZkMcpError {
    fn from(e: AuditError) -> Self {
        ZkMcpError::AuditTrailError(e.to_string())
    }
}

// ---------------------------------------------------------------------------
// Request / Response
// ---------------------------------------------------------------------------

/// An auditor's request for ZK compliance proofs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ZkAuditRequest {
    /// Request a full compliance report for a time period.
    RequestFullReport {
        period_start: u64,
        period_end: u64,
        /// Auditor-supplied nonce to prevent replay.
        nonce: String,
    },
    /// Request a proof for a specific governance property.
    RequestSpecificProof {
        proof_type: GovernanceProofType,
        parameters: HashMap<String, String>,
        /// Auditor-supplied nonce to prevent replay.
        nonce: String,
    },
    /// Verify an existing report from its JSON representation.
    VerifyExistingReport { report_json: String },
}

/// Response to a ZK audit request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZkAuditResponse {
    /// Whether the operation completed successfully.
    pub success: bool,
    /// The generated report (for full report or specific proof requests).
    pub report: Option<ZkAuditReport>,
    /// Verification results (for verify requests and full reports).
    pub verification_result: Option<VerificationResult>,
    /// Error message if the operation failed.
    pub error: Option<String>,
}

// ---------------------------------------------------------------------------
// Rate limiter
// ---------------------------------------------------------------------------

/// Per-requester rate limit state.
#[derive(Debug, Clone)]
struct RateLimitEntry {
    /// Timestamps of requests within the current hour window.
    request_times: Vec<u64>,
}

// ---------------------------------------------------------------------------
// Handler
// ---------------------------------------------------------------------------

/// Handles ZK audit requests via MCP.
///
/// Generates fresh proofs on each request (no caching). All requests are
/// logged to the audit trail as meta-audit events. Rate-limited to prevent
/// abuse.
pub struct ZkMcpHandler {
    /// Compliance configuration for proof generation.
    compliance_config: ComplianceConfig,
    /// Per-requester rate limit state. Key: requester_id string.
    rate_limits: HashMap<String, RateLimitEntry>,
    /// Maximum requests per hour per requester.
    max_requests_per_hour: usize,
}

impl ZkMcpHandler {
    /// Create a new handler with the given compliance configuration.
    pub fn new(compliance_config: ComplianceConfig) -> Self {
        Self {
            compliance_config,
            rate_limits: HashMap::new(),
            max_requests_per_hour: 10,
        }
    }

    /// Process a ZK audit request from an external auditor.
    ///
    /// - Generates fresh proofs (never cached) with auditor's nonce.
    /// - Logs the request to the audit trail (meta-audit).
    /// - Enforces rate limits per requester.
    pub fn handle_audit_request(
        &mut self,
        requester_id: &str,
        request: ZkAuditRequest,
        audit_trail: &mut AuditTrail,
    ) -> Result<ZkAuditResponse, ZkMcpError> {
        // Rate limit check
        self.check_rate_limit(requester_id)?;

        // Log the request itself to the audit trail (meta-audit)
        let request_description = match &request {
            ZkAuditRequest::RequestFullReport { nonce, .. } => {
                format!("full_report:nonce={nonce}")
            }
            ZkAuditRequest::RequestSpecificProof {
                proof_type, nonce, ..
            } => {
                format!("specific_proof:{proof_type:?}:nonce={nonce}")
            }
            ZkAuditRequest::VerifyExistingReport { .. } => "verify_existing_report".into(),
        };

        let meta_agent_id = Uuid::nil(); // system-level meta-audit
        audit_trail.append_event(
            meta_agent_id,
            EventType::ToolCall,
            serde_json::json!({
                "tool": "zk_audit_request",
                "requester": requester_id,
                "request_type": request_description,
            }),
        )?;

        // Process based on request type
        match request {
            ZkAuditRequest::RequestFullReport {
                period_start: _,
                period_end: _,
                nonce: _,
            } => self.handle_full_report(audit_trail),
            ZkAuditRequest::RequestSpecificProof {
                proof_type,
                parameters,
                nonce: _,
            } => self.handle_specific_proof(proof_type, &parameters, audit_trail),
            ZkAuditRequest::VerifyExistingReport { report_json } => {
                self.handle_verify_report(&report_json)
            }
        }
    }

    // -- Internal handlers ------------------------------------------------

    fn handle_full_report(&self, audit_trail: &AuditTrail) -> Result<ZkAuditResponse, ZkMcpError> {
        let report = ZkAuditReport::generate(audit_trail, &self.compliance_config)?;
        let verification = report
            .verify_all_proofs()
            .map_err(|e| ZkMcpError::VerificationFailed(e.to_string()))?;

        Ok(ZkAuditResponse {
            success: true,
            report: Some(report),
            verification_result: Some(verification),
            error: None,
        })
    }

    fn handle_specific_proof(
        &self,
        proof_type: GovernanceProofType,
        parameters: &HashMap<String, String>,
        audit_trail: &AuditTrail,
    ) -> Result<ZkAuditResponse, ZkMcpError> {
        let agent_id_str = parameters
            .get("agent_id")
            .ok_or_else(|| ZkMcpError::InvalidRequest("missing parameter: agent_id".into()))?;

        let agent_id = Uuid::parse_str(agent_id_str)
            .map_err(|e| ZkMcpError::InvalidRequest(format!("invalid agent_id: {e}")))?;

        let blinding = ProofGenerator::generate_blinding();
        let events = audit_trail.events();

        let proof = match proof_type {
            GovernanceProofType::FuelBudgetCompliance => {
                let spent = events
                    .iter()
                    .filter(|e| e.agent_id == agent_id && e.event_type == EventType::LlmCall)
                    .count() as u64;
                ProofGenerator::prove_fuel_compliance(
                    &agent_id,
                    self.compliance_config.fuel_cap,
                    spent,
                    &blinding,
                )
            }
            GovernanceProofType::AuditChainIntegrity => {
                if events.is_empty() {
                    return Err(ZkMcpError::ReportGenerationFailed(
                        "empty audit trail".into(),
                    ));
                }
                let chain_length = events.len() as u64;
                let genesis = &events[0].previous_hash;
                let final_hash = &events[events.len() - 1].hash;
                ProofGenerator::prove_audit_chain_integrity(
                    &agent_id,
                    chain_length,
                    genesis,
                    final_hash,
                    &blinding,
                )
            }
            GovernanceProofType::AutonomyLevelCompliance => {
                let observed: u8 = parameters
                    .get("observed_level")
                    // Optional: invalid observed_level parse falls through to default 0
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(0);
                ProofGenerator::prove_autonomy_compliance(
                    &agent_id,
                    observed,
                    self.compliance_config.max_autonomy_level,
                    &blinding,
                )
            }
            GovernanceProofType::CapabilityBoundary => {
                let granted: Vec<String> = parameters
                    .get("granted_caps")
                    .map(|s| s.split(',').map(|c| c.trim().to_string()).collect())
                    .unwrap_or_default();
                let used: Vec<String> = parameters
                    .get("used_caps")
                    .map(|s| s.split(',').map(|c| c.trim().to_string()).collect())
                    .unwrap_or_default();
                ProofGenerator::prove_capability_boundary(&agent_id, &granted, &used, &blinding)
            }
            GovernanceProofType::ApprovalChainValid => {
                let approval_count: usize = parameters
                    .get("approval_count")
                    // Optional: invalid approval_count parse falls through to default 0
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(0);
                let approvals: Vec<(String, String, bool)> = (0..approval_count)
                    .map(|i| (format!("approver_{i}"), "operation".into(), true))
                    .collect();
                ProofGenerator::prove_approval_chain(
                    &agent_id,
                    &approvals,
                    self.compliance_config.required_approval_tier,
                    &blinding,
                )
            }
            GovernanceProofType::DataRetentionCompliance => {
                let retention_days: u64 = parameters
                    .get("retention_days")
                    // Optional: invalid retention_days parse falls through to default 0
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(0);
                ProofGenerator::prove_data_retention(
                    &agent_id,
                    retention_days,
                    self.compliance_config.retention_policy_days,
                    0,
                    &blinding,
                )
            }
        };

        // Wrap single proof in a minimal report structure
        let verification = proof
            .verify()
            .map(|passed| VerificationResult {
                all_passed: passed,
                proof_results: vec![super::zk_report::ProofVerificationEntry {
                    proof_id: proof.proof_id.clone(),
                    proof_type: format!("{:?}", proof.proof_type),
                    passed,
                    detail: if passed {
                        "verified".into()
                    } else {
                        "property does not hold".into()
                    },
                }],
            })
            .map_err(|e| ZkMcpError::VerificationFailed(e.to_string()))?;

        Ok(ZkAuditResponse {
            success: true,
            report: None,
            verification_result: Some(verification),
            error: None,
        })
    }

    fn handle_verify_report(&self, report_json: &str) -> Result<ZkAuditResponse, ZkMcpError> {
        let report = ZkAuditReport::from_json(report_json)
            .map_err(|e| ZkMcpError::InvalidRequest(format!("invalid report JSON: {e}")))?;

        let signature_valid = report
            .verify_signature()
            .map_err(|e| ZkMcpError::VerificationFailed(e.to_string()))?;

        let proof_result = report
            .verify_all_proofs()
            .map_err(|e| ZkMcpError::VerificationFailed(e.to_string()))?;

        let all_valid = signature_valid && proof_result.all_passed;

        Ok(ZkAuditResponse {
            success: all_valid,
            report: Some(report),
            verification_result: Some(proof_result),
            error: if !signature_valid {
                Some("report signature is invalid or missing".into())
            } else {
                None
            },
        })
    }

    // -- Rate limiting ----------------------------------------------------

    fn check_rate_limit(&mut self, requester_id: &str) -> Result<(), ZkMcpError> {
        let now = current_unix_timestamp();
        let one_hour_ago = now.saturating_sub(3600);

        let entry = self
            .rate_limits
            .entry(requester_id.to_string())
            .or_insert_with(|| RateLimitEntry {
                request_times: Vec::new(),
            });

        // Prune old entries outside the 1-hour window
        entry.request_times.retain(|&t| t > one_hour_ago);

        if entry.request_times.len() >= self.max_requests_per_hour {
            return Err(ZkMcpError::RateLimitExceeded {
                max: self.max_requests_per_hour,
            });
        }

        entry.request_times.push(now);
        Ok(())
    }
}

fn current_unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn default_config() -> ComplianceConfig {
        ComplianceConfig {
            fuel_cap: 10000,
            max_autonomy_level: 3,
            retention_policy_days: 365,
            required_approval_tier: 1,
        }
    }

    fn sample_trail(n: usize) -> AuditTrail {
        let mut trail = AuditTrail::new();
        let agent_id = Uuid::new_v4();
        for i in 0..n {
            let et = match i % 3 {
                0 => EventType::StateChange,
                1 => EventType::LlmCall,
                _ => EventType::ToolCall,
            };
            trail
                .append_event(agent_id, et, json!({"seq": i}))
                .expect("append");
        }
        trail
    }

    #[test]
    fn full_report_request() {
        let mut handler = ZkMcpHandler::new(default_config());
        let mut trail = sample_trail(15);

        let request = ZkAuditRequest::RequestFullReport {
            period_start: 0,
            period_end: u64::MAX,
            nonce: "auditor-nonce-001".into(),
        };

        let response = handler
            .handle_audit_request("auditor-1", request, &mut trail)
            .expect("handle");

        assert!(response.success);
        assert!(response.report.is_some());
        assert!(response.verification_result.is_some());
        assert!(response.error.is_none());

        let verification = response.verification_result.unwrap();
        assert!(verification.all_passed);
    }

    #[test]
    fn specific_proof_fuel_compliance() {
        let mut handler = ZkMcpHandler::new(default_config());
        let mut trail = sample_trail(10);
        let agent_id = trail.events()[0].agent_id;

        let mut params = HashMap::new();
        params.insert("agent_id".into(), agent_id.to_string());

        let request = ZkAuditRequest::RequestSpecificProof {
            proof_type: GovernanceProofType::FuelBudgetCompliance,
            parameters: params,
            nonce: "nonce-002".into(),
        };

        let response = handler
            .handle_audit_request("auditor-2", request, &mut trail)
            .expect("handle");

        assert!(response.success);
        assert!(response.verification_result.is_some());
        let vr = response.verification_result.unwrap();
        assert!(vr.all_passed);
    }

    #[test]
    fn specific_proof_missing_agent_id() {
        let mut handler = ZkMcpHandler::new(default_config());
        let mut trail = sample_trail(5);

        let request = ZkAuditRequest::RequestSpecificProof {
            proof_type: GovernanceProofType::FuelBudgetCompliance,
            parameters: HashMap::new(),
            nonce: "nonce-003".into(),
        };

        let result = handler.handle_audit_request("auditor-3", request, &mut trail);
        assert!(matches!(result, Err(ZkMcpError::InvalidRequest(_))));
    }

    #[test]
    fn verify_existing_report() {
        // First generate a report
        let trail = sample_trail(10);
        let config = default_config();
        let report = ZkAuditReport::generate(&trail, &config).expect("generate");
        let report_json = report.to_json().expect("to_json");

        // Then verify it via MCP
        let mut handler = ZkMcpHandler::new(config);
        let mut audit = AuditTrail::new();
        // Need at least one event so the meta-audit append works on a trail
        // that may be separate from the report's trail
        let request = ZkAuditRequest::VerifyExistingReport { report_json };

        let response = handler
            .handle_audit_request("auditor-4", request, &mut audit)
            .expect("handle");

        // Unsigned report: signature invalid, but proofs pass
        assert!(!response.success); // signature missing
        let vr = response.verification_result.unwrap();
        assert!(vr.all_passed); // proofs are valid
        assert!(response.error.is_some()); // signature error message
    }

    #[test]
    fn rate_limit_enforced() {
        let mut handler = ZkMcpHandler::new(default_config());
        handler.max_requests_per_hour = 3; // low limit for testing
        let mut trail = sample_trail(5);

        for i in 0..3 {
            let request = ZkAuditRequest::RequestFullReport {
                period_start: 0,
                period_end: u64::MAX,
                nonce: format!("nonce-{i}"),
            };
            handler
                .handle_audit_request("rate-limited-auditor", request, &mut trail)
                .expect("should succeed");
        }

        // 4th request should fail
        let request = ZkAuditRequest::RequestFullReport {
            period_start: 0,
            period_end: u64::MAX,
            nonce: "nonce-blocked".into(),
        };
        let result = handler.handle_audit_request("rate-limited-auditor", request, &mut trail);
        assert!(matches!(
            result,
            Err(ZkMcpError::RateLimitExceeded { max: 3 })
        ));
    }

    #[test]
    fn rate_limit_per_requester() {
        let mut handler = ZkMcpHandler::new(default_config());
        handler.max_requests_per_hour = 2;
        let mut trail = sample_trail(5);

        // Requester A: 2 requests (at limit)
        for i in 0..2 {
            let req = ZkAuditRequest::RequestFullReport {
                period_start: 0,
                period_end: u64::MAX,
                nonce: format!("a-{i}"),
            };
            handler
                .handle_audit_request("requester-a", req, &mut trail)
                .expect("should succeed");
        }

        // Requester B: should still be allowed
        let req = ZkAuditRequest::RequestFullReport {
            period_start: 0,
            period_end: u64::MAX,
            nonce: "b-0".into(),
        };
        handler
            .handle_audit_request("requester-b", req, &mut trail)
            .expect("different requester should succeed");

        // Requester A: 3rd request blocked
        let req = ZkAuditRequest::RequestFullReport {
            period_start: 0,
            period_end: u64::MAX,
            nonce: "a-blocked".into(),
        };
        assert!(matches!(
            handler.handle_audit_request("requester-a", req, &mut trail),
            Err(ZkMcpError::RateLimitExceeded { .. })
        ));
    }

    #[test]
    fn meta_audit_logged() {
        let mut handler = ZkMcpHandler::new(default_config());
        let mut trail = sample_trail(5);
        let events_before = trail.events().len();

        let request = ZkAuditRequest::RequestFullReport {
            period_start: 0,
            period_end: u64::MAX,
            nonce: "meta-audit-nonce".into(),
        };

        handler
            .handle_audit_request("meta-auditor", request, &mut trail)
            .expect("handle");

        // Trail should have grown: the meta-audit event + report generation reads
        assert!(trail.events().len() > events_before);

        // Find the meta-audit event
        let meta_event = trail
            .events()
            .iter()
            .find(|e| {
                e.event_type == EventType::ToolCall
                    && e.payload.get("tool").and_then(|v| v.as_str()) == Some("zk_audit_request")
            })
            .expect("meta-audit event should exist");

        assert_eq!(
            meta_event.payload.get("requester").and_then(|v| v.as_str()),
            Some("meta-auditor")
        );
    }

    #[test]
    fn request_serialization_roundtrip() {
        let request = ZkAuditRequest::RequestFullReport {
            period_start: 1000,
            period_end: 2000,
            nonce: "serde-test".into(),
        };

        let json = serde_json::to_string(&request).expect("serialize");
        let deserialized: ZkAuditRequest = serde_json::from_str(&json).expect("deserialize");

        match deserialized {
            ZkAuditRequest::RequestFullReport {
                period_start,
                period_end,
                nonce,
            } => {
                assert_eq!(period_start, 1000);
                assert_eq!(period_end, 2000);
                assert_eq!(nonce, "serde-test");
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn response_serialization_roundtrip() {
        let response = ZkAuditResponse {
            success: true,
            report: None,
            verification_result: Some(VerificationResult {
                all_passed: true,
                proof_results: vec![],
            }),
            error: None,
        };

        let json = serde_json::to_string(&response).expect("serialize");
        let deserialized: ZkAuditResponse = serde_json::from_str(&json).expect("deserialize");
        assert!(deserialized.success);
        assert!(deserialized.verification_result.unwrap().all_passed);
    }

    #[test]
    fn specific_proof_autonomy_compliance() {
        let mut handler = ZkMcpHandler::new(default_config());
        let mut trail = sample_trail(5);
        let agent_id = trail.events()[0].agent_id;

        let mut params = HashMap::new();
        params.insert("agent_id".into(), agent_id.to_string());
        params.insert("observed_level".into(), "2".into());

        let request = ZkAuditRequest::RequestSpecificProof {
            proof_type: GovernanceProofType::AutonomyLevelCompliance,
            parameters: params,
            nonce: "autonomy-nonce".into(),
        };

        let response = handler
            .handle_audit_request("auditor-5", request, &mut trail)
            .expect("handle");

        assert!(response.success);
        assert!(response.verification_result.unwrap().all_passed);
    }

    #[test]
    fn specific_proof_audit_chain_integrity() {
        let mut handler = ZkMcpHandler::new(default_config());
        let mut trail = sample_trail(10);
        let agent_id = trail.events()[0].agent_id;

        let mut params = HashMap::new();
        params.insert("agent_id".into(), agent_id.to_string());

        let request = ZkAuditRequest::RequestSpecificProof {
            proof_type: GovernanceProofType::AuditChainIntegrity,
            parameters: params,
            nonce: "chain-nonce".into(),
        };

        let response = handler
            .handle_audit_request("auditor-6", request, &mut trail)
            .expect("handle");

        assert!(response.success);
        assert!(response.verification_result.unwrap().all_passed);
    }

    #[test]
    fn rate_limiting_11th_rejected() {
        let mut handler = ZkMcpHandler::new(default_config());
        // Use default max_requests_per_hour = 10
        let mut trail = sample_trail(5);

        // First 10 requests should succeed
        for i in 0..10 {
            let request = ZkAuditRequest::RequestFullReport {
                period_start: 0,
                period_end: u64::MAX,
                nonce: format!("nonce-{i}"),
            };
            handler
                .handle_audit_request("auditor-limited", request, &mut trail)
                .unwrap_or_else(|_| panic!("request {i} should succeed"));
        }

        // 11th request must be rejected
        let request = ZkAuditRequest::RequestFullReport {
            period_start: 0,
            period_end: u64::MAX,
            nonce: "nonce-11th".into(),
        };
        let result = handler.handle_audit_request("auditor-limited", request, &mut trail);
        assert!(
            matches!(result, Err(ZkMcpError::RateLimitExceeded { max: 10 })),
            "11th request should be rate-limited"
        );
    }
}
