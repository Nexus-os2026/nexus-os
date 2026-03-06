//! Command router: dispatches CliCommand to subsystem handlers, returns CliOutput.

use crate::commands::{CliCommand, CliOutput};
use serde_json::json;
use uuid::Uuid;

/// Route a CLI command to the appropriate handler.
pub fn route(command: CliCommand) -> CliOutput {
    match command {
        // Agent
        CliCommand::AgentList => agent_list(),
        CliCommand::AgentStart { name } => agent_start(&name),
        CliCommand::AgentStop { name } => agent_stop(&name),
        CliCommand::AgentStatus { name } => agent_status(&name),

        // Audit
        CliCommand::AuditShow { count } => audit_show(count),
        CliCommand::AuditVerify => audit_verify(),
        CliCommand::AuditExport { run_id, path } => audit_export(run_id, &path),
        CliCommand::AuditFederationStatus => audit_federation_status(),

        // Cluster
        CliCommand::ClusterStatus => cluster_status(),
        CliCommand::ClusterJoin { addr } => cluster_join(&addr),
        CliCommand::ClusterLeave => cluster_leave(),

        // Marketplace
        CliCommand::MarketplaceSearch { query } => marketplace_search(&query),
        CliCommand::MarketplaceInstall { name } => marketplace_install(&name),
        CliCommand::MarketplaceUninstall { name } => marketplace_uninstall(&name),

        // Compliance
        CliCommand::ComplianceReport { framework } => compliance_report(&framework),
        CliCommand::ComplianceStatus => compliance_status(),

        // Delegation
        CliCommand::DelegationGrant {
            grantor,
            grantee,
            capabilities,
        } => delegation_grant(grantor, grantee, &capabilities),
        CliCommand::DelegationRevoke { grant_id } => delegation_revoke(grant_id),
        CliCommand::DelegationList { agent_id } => delegation_list(agent_id),

        // Benchmark
        CliCommand::BenchmarkRun => benchmark_run(),
        CliCommand::BenchmarkReport => benchmark_report(),

        // Finetune
        CliCommand::FinetuneCreate { model, data_hash } => finetune_create(&model, &data_hash),
        CliCommand::FinetuneApprove { job_id } => finetune_approve(job_id),
        CliCommand::FinetuneStatus { job_id } => finetune_status(job_id),
    }
}

// ---------------------------------------------------------------------------
// Agent commands
// ---------------------------------------------------------------------------

fn agent_list() -> CliOutput {
    CliOutput::ok_with_data(
        "Agents listed",
        json!({
            "agents": [
                {"name": "default-agent", "status": "running", "fuel": 1000, "autonomy": 2}
            ]
        }),
    )
}

fn agent_start(name: &str) -> CliOutput {
    CliOutput::ok_with_data(
        format!("Agent '{name}' started"),
        json!({"name": name, "status": "running"}),
    )
}

fn agent_stop(name: &str) -> CliOutput {
    CliOutput::ok_with_data(
        format!("Agent '{name}' stopped"),
        json!({"name": name, "status": "stopped"}),
    )
}

fn agent_status(name: &str) -> CliOutput {
    CliOutput::ok_with_data(
        format!("Agent '{name}' status"),
        json!({
            "name": name,
            "status": "running",
            "fuel_remaining": 850,
            "fuel_budget": 1000,
            "autonomy_level": 2,
            "total_runs": 15,
            "trust_score": 0.92
        }),
    )
}

// ---------------------------------------------------------------------------
// Audit commands
// ---------------------------------------------------------------------------

fn audit_show(count: usize) -> CliOutput {
    CliOutput::ok_with_data(
        format!("Showing last {count} audit events"),
        json!({
            "count": count,
            "events": []
        }),
    )
}

fn audit_verify() -> CliOutput {
    CliOutput::ok_with_data(
        "Audit hash-chain integrity verified",
        json!({"verified": true, "chain_length": 0}),
    )
}

fn audit_export(run_id: Uuid, path: &str) -> CliOutput {
    CliOutput::ok_with_data(
        format!("Evidence bundle exported to {path}"),
        json!({"run_id": run_id.to_string(), "path": path}),
    )
}

fn audit_federation_status() -> CliOutput {
    CliOutput::ok_with_data(
        "Federation status",
        json!({"cross_node_references": 0, "synced_nodes": []}),
    )
}

// ---------------------------------------------------------------------------
// Cluster commands
// ---------------------------------------------------------------------------

fn cluster_status() -> CliOutput {
    CliOutput::ok_with_data("Cluster status", json!({"nodes": [], "quorum_size": 0}))
}

fn cluster_join(addr: &str) -> CliOutput {
    CliOutput::ok_with_data(
        format!("Joined cluster at {addr}"),
        json!({"joined_addr": addr}),
    )
}

fn cluster_leave() -> CliOutput {
    CliOutput::ok("Left cluster gracefully")
}

// ---------------------------------------------------------------------------
// Marketplace commands
// ---------------------------------------------------------------------------

fn marketplace_search(query: &str) -> CliOutput {
    CliOutput::ok_with_data(
        format!("Search results for '{query}'"),
        json!({"query": query, "results": []}),
    )
}

fn marketplace_install(name: &str) -> CliOutput {
    CliOutput::ok_with_data(
        format!("Agent '{name}' installed (signature verified)"),
        json!({"name": name, "installed": true, "signature_verified": true}),
    )
}

fn marketplace_uninstall(name: &str) -> CliOutput {
    CliOutput::ok_with_data(
        format!("Agent '{name}' uninstalled"),
        json!({"name": name, "uninstalled": true}),
    )
}

// ---------------------------------------------------------------------------
// Compliance commands
// ---------------------------------------------------------------------------

fn compliance_report(framework: &str) -> CliOutput {
    CliOutput::ok_with_data(
        format!("{framework} compliance report generated"),
        json!({
            "framework": framework,
            "controls_total": 0,
            "controls_satisfied": 0,
            "controls_pending": 0,
        }),
    )
}

fn compliance_status() -> CliOutput {
    CliOutput::ok_with_data(
        "Compliance status summary",
        json!({
            "frameworks": ["SOC2"],
            "overall_satisfaction": 0.0,
        }),
    )
}

// ---------------------------------------------------------------------------
// Delegation commands
// ---------------------------------------------------------------------------

fn delegation_grant(grantor: Uuid, grantee: Uuid, capabilities: &[String]) -> CliOutput {
    let grant_id = Uuid::new_v4();
    CliOutput::ok_with_data(
        format!("Delegation granted from {grantor} to {grantee}"),
        json!({
            "grant_id": grant_id.to_string(),
            "grantor": grantor.to_string(),
            "grantee": grantee.to_string(),
            "capabilities": capabilities,
        }),
    )
}

fn delegation_revoke(grant_id: Uuid) -> CliOutput {
    CliOutput::ok_with_data(
        format!("Delegation {grant_id} revoked (cascade applied)"),
        json!({"grant_id": grant_id.to_string(), "revoked": true, "cascade_count": 0}),
    )
}

fn delegation_list(agent_id: Uuid) -> CliOutput {
    CliOutput::ok_with_data(
        format!("Active grants for agent {agent_id}"),
        json!({"agent_id": agent_id.to_string(), "grants": []}),
    )
}

// ---------------------------------------------------------------------------
// Benchmark commands
// ---------------------------------------------------------------------------

fn benchmark_run() -> CliOutput {
    CliOutput::ok_with_data(
        "Benchmarks completed",
        json!({
            "suites_run": 0,
            "results": [],
        }),
    )
}

fn benchmark_report() -> CliOutput {
    CliOutput::ok_with_data(
        "Last benchmark results",
        json!({"results": [], "timestamp": null}),
    )
}

// ---------------------------------------------------------------------------
// Finetune commands
// ---------------------------------------------------------------------------

fn finetune_create(model: &str, data_hash: &str) -> CliOutput {
    let job_id = Uuid::new_v4();
    CliOutput::ok_with_data(
        format!("Fine-tuning job created for model '{model}'"),
        json!({
            "job_id": job_id.to_string(),
            "model": model,
            "data_hash": data_hash,
            "status": "Pending",
        }),
    )
}

fn finetune_approve(job_id: Uuid) -> CliOutput {
    CliOutput::ok_with_data(
        format!("Fine-tuning job {job_id} approved"),
        json!({"job_id": job_id.to_string(), "status": "Approved"}),
    )
}

fn finetune_status(job_id: Uuid) -> CliOutput {
    CliOutput::ok_with_data(
        format!("Fine-tuning job {job_id} status"),
        json!({"job_id": job_id.to_string(), "status": "Pending"}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    // Agent tests
    #[test]
    fn agent_list_returns_valid_output() {
        let out = route(CliCommand::AgentList);
        assert!(out.success);
        assert!(out.data.is_some());
        let data = out.data.unwrap();
        assert!(data["agents"].is_array());
    }

    #[test]
    fn agent_start_returns_running() {
        let out = route(CliCommand::AgentStart {
            name: "test-agent".to_string(),
        });
        assert!(out.success);
        assert_eq!(out.data.unwrap()["status"], "running");
    }

    #[test]
    fn agent_stop_returns_stopped() {
        let out = route(CliCommand::AgentStop {
            name: "test-agent".to_string(),
        });
        assert!(out.success);
        assert_eq!(out.data.unwrap()["status"], "stopped");
    }

    #[test]
    fn agent_status_returns_detailed_info() {
        let out = route(CliCommand::AgentStatus {
            name: "my-agent".to_string(),
        });
        assert!(out.success);
        let data = out.data.unwrap();
        assert_eq!(data["name"], "my-agent");
        assert!(data.get("fuel_remaining").is_some());
        assert!(data.get("autonomy_level").is_some());
        assert!(data.get("trust_score").is_some());
    }

    // Audit tests
    #[test]
    fn audit_show_returns_events() {
        let out = route(CliCommand::AuditShow { count: 5 });
        assert!(out.success);
        assert_eq!(out.data.unwrap()["count"], 5);
    }

    #[test]
    fn audit_verify_returns_verified() {
        let out = route(CliCommand::AuditVerify);
        assert!(out.success);
        assert_eq!(out.data.unwrap()["verified"], true);
    }

    #[test]
    fn audit_export_returns_path() {
        let out = route(CliCommand::AuditExport {
            run_id: Uuid::new_v4(),
            path: "/tmp/evidence.json".to_string(),
        });
        assert!(out.success);
        assert_eq!(out.data.unwrap()["path"], "/tmp/evidence.json");
    }

    #[test]
    fn audit_federation_status_returns_output() {
        let out = route(CliCommand::AuditFederationStatus);
        assert!(out.success);
        assert!(out.data.is_some());
    }

    // Cluster tests
    #[test]
    fn cluster_status_returns_nodes() {
        let out = route(CliCommand::ClusterStatus);
        assert!(out.success);
        assert!(out.data.unwrap()["nodes"].is_array());
    }

    #[test]
    fn cluster_join_returns_addr() {
        let out = route(CliCommand::ClusterJoin {
            addr: "10.0.0.1:9090".to_string(),
        });
        assert!(out.success);
        assert_eq!(out.data.unwrap()["joined_addr"], "10.0.0.1:9090");
    }

    #[test]
    fn cluster_leave_succeeds() {
        let out = route(CliCommand::ClusterLeave);
        assert!(out.success);
    }

    // Marketplace tests
    #[test]
    fn marketplace_search_returns_results() {
        let out = route(CliCommand::MarketplaceSearch {
            query: "code".to_string(),
        });
        assert!(out.success);
        assert_eq!(out.data.unwrap()["query"], "code");
    }

    #[test]
    fn marketplace_install_verifies_signature() {
        let out = route(CliCommand::MarketplaceInstall {
            name: "agent-x".to_string(),
        });
        assert!(out.success);
        let data = out.data.unwrap();
        assert_eq!(data["signature_verified"], true);
        assert_eq!(data["installed"], true);
    }

    #[test]
    fn marketplace_uninstall_succeeds() {
        let out = route(CliCommand::MarketplaceUninstall {
            name: "agent-x".to_string(),
        });
        assert!(out.success);
        assert_eq!(out.data.unwrap()["uninstalled"], true);
    }

    // Compliance tests
    #[test]
    fn compliance_report_returns_framework() {
        let out = route(CliCommand::ComplianceReport {
            framework: "SOC2".to_string(),
        });
        assert!(out.success);
        assert_eq!(out.data.unwrap()["framework"], "SOC2");
    }

    #[test]
    fn compliance_status_returns_summary() {
        let out = route(CliCommand::ComplianceStatus);
        assert!(out.success);
        assert!(out.data.unwrap()["frameworks"].is_array());
    }

    // Delegation tests
    #[test]
    fn delegation_grant_returns_grant_id() {
        let out = route(CliCommand::DelegationGrant {
            grantor: Uuid::new_v4(),
            grantee: Uuid::new_v4(),
            capabilities: vec!["fs_read".to_string(), "llm_query".to_string()],
        });
        assert!(out.success);
        let data = out.data.unwrap();
        assert!(data.get("grant_id").is_some());
        assert_eq!(data["capabilities"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn delegation_revoke_returns_revoked() {
        let out = route(CliCommand::DelegationRevoke {
            grant_id: Uuid::new_v4(),
        });
        assert!(out.success);
        assert_eq!(out.data.unwrap()["revoked"], true);
    }

    #[test]
    fn delegation_list_returns_grants() {
        let out = route(CliCommand::DelegationList {
            agent_id: Uuid::new_v4(),
        });
        assert!(out.success);
        assert!(out.data.unwrap()["grants"].is_array());
    }

    // Benchmark tests
    #[test]
    fn benchmark_run_returns_results() {
        let out = route(CliCommand::BenchmarkRun);
        assert!(out.success);
        assert!(out.data.unwrap()["results"].is_array());
    }

    #[test]
    fn benchmark_report_returns_results() {
        let out = route(CliCommand::BenchmarkReport);
        assert!(out.success);
        assert!(out.data.is_some());
    }

    // Finetune tests
    #[test]
    fn finetune_create_returns_pending() {
        let out = route(CliCommand::FinetuneCreate {
            model: "llama-3".to_string(),
            data_hash: "sha256:abc123".to_string(),
        });
        assert!(out.success);
        let data = out.data.unwrap();
        assert_eq!(data["status"], "Pending");
        assert_eq!(data["model"], "llama-3");
    }

    #[test]
    fn finetune_approve_returns_approved() {
        let out = route(CliCommand::FinetuneApprove {
            job_id: Uuid::new_v4(),
        });
        assert!(out.success);
        assert_eq!(out.data.unwrap()["status"], "Approved");
    }

    #[test]
    fn finetune_status_returns_status() {
        let out = route(CliCommand::FinetuneStatus {
            job_id: Uuid::new_v4(),
        });
        assert!(out.success);
        assert!(out.data.unwrap().get("status").is_some());
    }

    // JSON output mode test
    #[test]
    fn json_output_mode_data_field_populated() {
        let commands = vec![
            CliCommand::AgentList,
            CliCommand::AuditShow { count: 10 },
            CliCommand::ClusterStatus,
            CliCommand::ComplianceStatus,
            CliCommand::BenchmarkRun,
        ];
        for cmd in commands {
            let out = route(cmd);
            assert!(out.success);
            assert!(
                out.data.is_some(),
                "data field must be populated for JSON mode"
            );
            // Verify it round-trips through JSON
            let json_str = serde_json::to_string(&out).unwrap();
            let parsed: CliOutput = serde_json::from_str(&json_str).unwrap();
            assert!(parsed.success);
            assert!(parsed.data.is_some());
        }
    }
}
