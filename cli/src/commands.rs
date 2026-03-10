//! CLI command definitions covering every Nexus OS subsystem.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Unified CLI command enum with a variant for each subcommand.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CliCommand {
    // Agent commands
    AgentList,
    AgentStart {
        name: String,
    },
    AgentStop {
        name: String,
    },
    AgentStatus {
        name: String,
    },

    // Audit commands
    AuditShow {
        count: usize,
    },
    AuditVerify,
    AuditExport {
        run_id: Uuid,
        path: String,
    },
    AuditFederationStatus,

    // Cluster commands
    ClusterStatus,
    ClusterJoin {
        addr: String,
    },
    ClusterLeave,

    // Marketplace commands
    MarketplaceSearch {
        query: String,
    },
    MarketplaceInstall {
        name: String,
    },
    MarketplaceUninstall {
        name: String,
    },
    MarketplacePublish {
        bundle_path: String,
    },
    MarketplaceInfo {
        agent_id: String,
    },
    MarketplaceMyAgents {
        author: String,
    },

    // Compliance commands
    ComplianceReport {
        framework: String,
    },
    ComplianceStatus,
    ComplianceClassify {
        agent_id: Uuid,
    },
    ComplianceEraseAgentData {
        agent_id: Uuid,
    },
    ComplianceRetentionCheck,
    ComplianceProvenance {
        agent_id: Uuid,
    },

    // Delegation commands
    DelegationGrant {
        grantor: Uuid,
        grantee: Uuid,
        capabilities: Vec<String>,
    },
    DelegationRevoke {
        grant_id: Uuid,
    },
    DelegationList {
        agent_id: Uuid,
    },

    // Sandbox commands
    SandboxStatus,

    // Simulation commands
    SimulationStatus,

    // Benchmark commands
    BenchmarkRun,
    BenchmarkReport,

    // Finetune commands
    FinetuneCreate {
        model: String,
        data_hash: String,
    },
    FinetuneApprove {
        job_id: Uuid,
    },
    FinetuneStatus {
        job_id: Uuid,
    },

    // Model commands (local SLM management)
    ModelList,
    ModelDownload {
        model_id: String,
    },
    ModelLoad {
        model_id: String,
    },
    ModelUnload,
    ModelStatus,

    // Distributed audit commands
    AuditVerifyChain,
    AuditVerifyEvent {
        event_id: Uuid,
    },
    AuditDistributedStatus,
    AuditComplianceReport,

    // Device pairing commands
    DevicePair {
        code: String,
    },
    DeviceList,
    DeviceRevoke {
        node_id: Uuid,
    },

    // Governance commands
    GovernanceTest {
        task_type: String,
        input: String,
    },

    // Protocol commands (A2A + MCP)
    ProtocolsStatus,
    ProtocolsAgentCard {
        agent_name: String,
    },
    ProtocolsStart {
        port: u16,
    },

    // Identity commands
    IdentityShow {
        agent_id: Uuid,
    },
    IdentityList,
    TokenIssue {
        agent_id: Uuid,
        scopes: Vec<String>,
        ttl: Option<u64>,
    },

    // Firewall commands
    FirewallStatus,
    FirewallPatterns,
}

/// Structured output for every CLI command, supporting JSON mode.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CliOutput {
    pub success: bool,
    pub message: String,
    pub data: Option<serde_json::Value>,
}

impl CliOutput {
    pub fn ok(message: impl Into<String>) -> Self {
        Self {
            success: true,
            message: message.into(),
            data: None,
        }
    }

    pub fn ok_with_data(message: impl Into<String>, data: serde_json::Value) -> Self {
        Self {
            success: true,
            message: message.into(),
            data: Some(data),
        }
    }

    pub fn err(message: impl Into<String>) -> Self {
        Self {
            success: false,
            message: message.into(),
            data: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn cli_output_ok() {
        let out = CliOutput::ok("done");
        assert!(out.success);
        assert_eq!(out.message, "done");
        assert!(out.data.is_none());
    }

    #[test]
    fn cli_output_ok_with_data() {
        let out = CliOutput::ok_with_data("agents listed", json!({"count": 3}));
        assert!(out.success);
        assert!(out.data.is_some());
        assert_eq!(out.data.unwrap()["count"], 3);
    }

    #[test]
    fn cli_output_err() {
        let out = CliOutput::err("not found");
        assert!(!out.success);
        assert_eq!(out.message, "not found");
    }

    #[test]
    fn cli_output_serializes_to_json() {
        let out = CliOutput::ok_with_data("ok", json!({"key": "value"}));
        let json_str = serde_json::to_string(&out).unwrap();
        assert!(json_str.contains("\"success\":true"));
        assert!(json_str.contains("\"key\":\"value\""));
    }

    #[test]
    fn cli_command_all_variants_exist() {
        // Verify every variant constructs without panic
        let _cmds: Vec<CliCommand> = vec![
            CliCommand::AgentList,
            CliCommand::AgentStart {
                name: "a".to_string(),
            },
            CliCommand::AgentStop {
                name: "a".to_string(),
            },
            CliCommand::AgentStatus {
                name: "a".to_string(),
            },
            CliCommand::AuditShow { count: 10 },
            CliCommand::AuditVerify,
            CliCommand::AuditExport {
                run_id: Uuid::new_v4(),
                path: "/tmp/out".to_string(),
            },
            CliCommand::AuditFederationStatus,
            CliCommand::ClusterStatus,
            CliCommand::ClusterJoin {
                addr: "127.0.0.1:9090".to_string(),
            },
            CliCommand::ClusterLeave,
            CliCommand::MarketplaceSearch {
                query: "code".to_string(),
            },
            CliCommand::MarketplaceInstall {
                name: "agent-x".to_string(),
            },
            CliCommand::MarketplaceUninstall {
                name: "agent-x".to_string(),
            },
            CliCommand::MarketplacePublish {
                bundle_path: "/tmp/agent.nexus-agent".to_string(),
            },
            CliCommand::MarketplaceInfo {
                agent_id: "pkg-abc123".to_string(),
            },
            CliCommand::MarketplaceMyAgents {
                author: "dev-alice".to_string(),
            },
            CliCommand::ComplianceReport {
                framework: "SOC2".to_string(),
            },
            CliCommand::ComplianceStatus,
            CliCommand::ComplianceClassify {
                agent_id: Uuid::new_v4(),
            },
            CliCommand::ComplianceEraseAgentData {
                agent_id: Uuid::new_v4(),
            },
            CliCommand::ComplianceRetentionCheck,
            CliCommand::ComplianceProvenance {
                agent_id: Uuid::new_v4(),
            },
            CliCommand::DelegationGrant {
                grantor: Uuid::new_v4(),
                grantee: Uuid::new_v4(),
                capabilities: vec!["fs_read".to_string()],
            },
            CliCommand::DelegationRevoke {
                grant_id: Uuid::new_v4(),
            },
            CliCommand::DelegationList {
                agent_id: Uuid::new_v4(),
            },
            CliCommand::SandboxStatus,
            CliCommand::SimulationStatus,
            CliCommand::BenchmarkRun,
            CliCommand::BenchmarkReport,
            CliCommand::FinetuneCreate {
                model: "llama-3".to_string(),
                data_hash: "sha256:abc".to_string(),
            },
            CliCommand::FinetuneApprove {
                job_id: Uuid::new_v4(),
            },
            CliCommand::FinetuneStatus {
                job_id: Uuid::new_v4(),
            },
            CliCommand::ModelList,
            CliCommand::ModelDownload {
                model_id: "tinyllama-1.1b".to_string(),
            },
            CliCommand::ModelLoad {
                model_id: "tinyllama-1.1b".to_string(),
            },
            CliCommand::ModelUnload,
            CliCommand::ModelStatus,
            CliCommand::AuditVerifyChain,
            CliCommand::AuditVerifyEvent {
                event_id: Uuid::new_v4(),
            },
            CliCommand::AuditDistributedStatus,
            CliCommand::AuditComplianceReport,
            CliCommand::DevicePair {
                code: "abc123".to_string(),
            },
            CliCommand::DeviceList,
            CliCommand::DeviceRevoke {
                node_id: Uuid::new_v4(),
            },
            CliCommand::GovernanceTest {
                task_type: "pii_detection".to_string(),
                input: "test input".to_string(),
            },
            CliCommand::ProtocolsStatus,
            CliCommand::ProtocolsAgentCard {
                agent_name: "test-agent".to_string(),
            },
            CliCommand::ProtocolsStart { port: 3000 },
            CliCommand::IdentityShow {
                agent_id: Uuid::new_v4(),
            },
            CliCommand::IdentityList,
            CliCommand::TokenIssue {
                agent_id: Uuid::new_v4(),
                scopes: vec!["web.search".to_string()],
                ttl: Some(3600),
            },
            CliCommand::FirewallStatus,
            CliCommand::FirewallPatterns,
        ];
        assert_eq!(_cmds.len(), 54);
    }
}
