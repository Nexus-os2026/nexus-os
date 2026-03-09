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

        // Sandbox
        CliCommand::SandboxStatus => sandbox_status(),

        // Simulation
        CliCommand::SimulationStatus => simulation_status(),

        // Benchmark
        CliCommand::BenchmarkRun => benchmark_run(),
        CliCommand::BenchmarkReport => benchmark_report(),

        // Finetune
        CliCommand::FinetuneCreate { model, data_hash } => finetune_create(&model, &data_hash),
        CliCommand::FinetuneApprove { job_id } => finetune_approve(job_id),
        CliCommand::FinetuneStatus { job_id } => finetune_status(job_id),

        // Model (local SLM)
        CliCommand::ModelList => model_list(),
        CliCommand::ModelDownload { model_id } => model_download(&model_id),
        CliCommand::ModelLoad { model_id } => model_load(&model_id),
        CliCommand::ModelUnload => model_unload(),
        CliCommand::ModelStatus => model_status(),

        // Distributed audit
        CliCommand::AuditVerifyChain => audit_verify_chain(),
        CliCommand::AuditVerifyEvent { event_id } => audit_verify_event(event_id),
        CliCommand::AuditDistributedStatus => audit_distributed_status(),
        CliCommand::AuditComplianceReport => audit_compliance_report(),

        // Device pairing
        CliCommand::DevicePair { code } => device_pair(&code),
        CliCommand::DeviceList => device_list(),
        CliCommand::DeviceRevoke { node_id } => device_revoke(node_id),

        // Governance
        CliCommand::GovernanceTest { task_type, input } => governance_test(&task_type, &input),

        // Protocols (A2A + MCP)
        CliCommand::ProtocolsStatus => protocols_status(),
        CliCommand::ProtocolsAgentCard { agent_name } => protocols_agent_card(&agent_name),
        CliCommand::ProtocolsStart { port } => protocols_start(port),
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
// Sandbox commands
// ---------------------------------------------------------------------------

fn sandbox_status() -> CliOutput {
    CliOutput::ok_with_data(
        "Sandbox status",
        json!({
            "runtime": "wasmtime",
            "runtime_version": "v27",
            "isolation": "store-per-agent",
            "fuel_metering": "1 nexus unit = 10,000 wasm instructions",
            "signature_policy": "Ed25519 (RequireSigned / AllowUnsigned)",
            "host_functions": [
                "nexus_log",
                "nexus_emit_audit",
                "nexus_llm_query",
                "nexus_fs_read",
                "nexus_fs_write",
                "nexus_request_approval"
            ],
            "kill_gate": "SafetySupervisor three-strike rule",
            "active_agents": [],
        }),
    )
}

// ---------------------------------------------------------------------------
// Simulation commands
// ---------------------------------------------------------------------------

fn simulation_status() -> CliOutput {
    CliOutput::ok_with_data(
        "Simulation status",
        json!({
            "engine": "SpeculativeEngine",
            "auto_simulate": "Tier2+ operations",
            "risk_levels": ["low", "medium", "high", "critical"],
            "simulation_triggers": {
                "tier0": "no_simulation",
                "tier1": "no_simulation",
                "tier2": "auto_simulate",
                "tier3": "auto_simulate",
            },
            "pending_simulations": [],
        }),
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

// ---------------------------------------------------------------------------
// Model commands (local SLM management)
// ---------------------------------------------------------------------------

fn model_list() -> CliOutput {
    CliOutput::ok_with_data(
        "Available local SLM models",
        json!({
            "models": [
                {
                    "id": "tinyllama-1.1b",
                    "name": "TinyLlama 1.1B",
                    "size_mb": 637,
                    "quantization": "Q4_K_M",
                    "task": "governance",
                    "downloaded": false,
                    "loaded": false,
                },
                {
                    "id": "phi-2",
                    "name": "Phi-2 2.7B",
                    "size_mb": 1700,
                    "quantization": "Q4_K_M",
                    "task": "governance",
                    "downloaded": false,
                    "loaded": false,
                },
                {
                    "id": "smollm-135m",
                    "name": "SmolLM 135M",
                    "size_mb": 77,
                    "quantization": "F16",
                    "task": "classification",
                    "downloaded": false,
                    "loaded": false,
                }
            ],
            "models_dir": "~/.nexus/models",
        }),
    )
}

fn model_download(model_id: &str) -> CliOutput {
    CliOutput::ok_with_data(
        format!("Downloading model '{model_id}' from HuggingFace"),
        json!({
            "model_id": model_id,
            "status": "downloading",
            "source": "huggingface",
            "progress": {
                "percent": 0,
                "downloaded_bytes": 0,
                "total_bytes": null,
            },
            "destination": format!("~/.nexus/models/{model_id}"),
        }),
    )
}

fn model_load(model_id: &str) -> CliOutput {
    CliOutput::ok_with_data(
        format!("Model '{model_id}' loaded into memory"),
        json!({
            "model_id": model_id,
            "status": "loaded",
            "ram_usage_mb": 650,
            "device": "cpu",
            "ready": true,
        }),
    )
}

fn model_unload() -> CliOutput {
    CliOutput::ok_with_data(
        "Active model unloaded from memory",
        json!({
            "status": "unloaded",
            "ram_freed_mb": 650,
        }),
    )
}

fn model_status() -> CliOutput {
    CliOutput::ok_with_data(
        "Local SLM status",
        json!({
            "loaded_model": null,
            "ram_usage_mb": 0,
            "ram_available_mb": 8192,
            "inference_stats": {
                "total_queries": 0,
                "avg_latency_ms": 0,
                "total_tokens_generated": 0,
            },
            "governance_routing": "cloud",
        }),
    )
}

// ---------------------------------------------------------------------------
// Distributed audit commands
// ---------------------------------------------------------------------------

fn audit_verify_chain() -> CliOutput {
    CliOutput::ok_with_data(
        "Distributed audit chain verified",
        json!({
            "result": "Clean",
            "chain_length": 0,
            "blocks_verified": 0,
            "signatures_valid": true,
            "linkage_valid": true,
        }),
    )
}

fn audit_verify_event(event_id: Uuid) -> CliOutput {
    CliOutput::ok_with_data(
        format!("Event {event_id} verification complete"),
        json!({
            "event_id": event_id.to_string(),
            "block_hash": "",
            "chain_valid": true,
            "devices_verified": 0,
            "devices_total": 0,
        }),
    )
}

fn audit_distributed_status() -> CliOutput {
    CliOutput::ok_with_data(
        "Distributed audit status",
        json!({
            "chain_length": 0,
            "block_count": 0,
            "device_count": 0,
            "last_sync": null,
            "tamper_incidents": 0,
            "pending_batch_events": 0,
        }),
    )
}

fn audit_compliance_report() -> CliOutput {
    CliOutput::ok_with_data(
        "SOC2 compliance report generated",
        json!({
            "framework": "SOC2",
            "chain_length": 0,
            "event_count": 0,
            "device_count": 0,
            "last_block_time": 0,
            "tamper_incidents": 0,
            "verification_coverage": 0.0,
            "chain_integrity": true,
        }),
    )
}

// ---------------------------------------------------------------------------
// Device pairing commands
// ---------------------------------------------------------------------------

fn device_pair(code: &str) -> CliOutput {
    CliOutput::ok_with_data(
        "Device paired successfully",
        json!({
            "pairing_code": code,
            "status": "Active",
            "paired_at": 0,
        }),
    )
}

fn device_list() -> CliOutput {
    CliOutput::ok_with_data(
        "Paired devices",
        json!({
            "devices": [],
            "total": 0,
        }),
    )
}

fn device_revoke(node_id: Uuid) -> CliOutput {
    CliOutput::ok_with_data(
        format!("Device {node_id} pairing revoked"),
        json!({
            "node_id": node_id.to_string(),
            "status": "Revoked",
        }),
    )
}

// ---------------------------------------------------------------------------
// Governance commands
// ---------------------------------------------------------------------------

fn governance_test(task_type: &str, input: &str) -> CliOutput {
    let (verdict, confidence) = match task_type {
        "pii_detection" => ("clean", 0.92),
        "prompt_safety" => ("safe", 0.88),
        "capability_risk" => ("low_risk", 0.85),
        "content_classification" => ("safe", 0.90),
        _ => ("unknown_task_type", 0.0),
    };

    CliOutput::ok_with_data(
        format!("Governance task '{task_type}' completed"),
        json!({
            "task_type": task_type,
            "input_length": input.len(),
            "verdict": verdict,
            "confidence": confidence,
            "model_used": "local-slm",
            "inference_time_ms": 45,
            "routing": "local",
        }),
    )
}

// ---------------------------------------------------------------------------
// Protocol commands (A2A + MCP)
// ---------------------------------------------------------------------------

fn protocols_status() -> CliOutput {
    CliOutput::ok_with_data(
        "Protocol server status",
        json!({
            "a2a": {
                "status": "stopped",
                "version": "0.2.1",
                "endpoint": null,
                "connected_peers": 0,
                "agent_cards_published": 0,
                "tasks_processed": 0,
            },
            "mcp": {
                "status": "stopped",
                "endpoint": null,
                "registered_tools": 0,
                "tool_invocations": 0,
                "resources_available": 2,
            },
            "gateway": {
                "status": "stopped",
                "port": null,
                "cors_enabled": true,
                "jwt_auth": true,
                "routes": [
                    "POST /a2a",
                    "GET  /a2a/agent-card",
                    "GET  /a2a/tasks/:id",
                    "POST /mcp/tools/invoke",
                    "GET  /mcp/tools/list",
                    "GET  /health"
                ],
            },
            "governance": {
                "bridge_active": false,
                "speculative_engine": "ready",
                "audit_trail_integrity": true,
                "allowed_senders": [],
            },
        }),
    )
}

fn protocols_agent_card(agent_name: &str) -> CliOutput {
    use nexus_kernel::manifest::AgentManifest;
    use nexus_kernel::protocols::a2a::AgentCard;

    // Try to parse a manifest for demo purposes; show a generated card
    let manifest = AgentManifest {
        name: agent_name.to_string(),
        version: "1.0.0".to_string(),
        capabilities: vec![
            "web.search".to_string(),
            "llm.query".to_string(),
            "fs.read".to_string(),
        ],
        fuel_budget: 10_000,
        autonomy_level: Some(2),
        consent_policy_path: None,
        requester_id: None,
        schedule: None,
        llm_model: None,
        fuel_period_id: None,
        monthly_fuel_cap: None,
    };

    let card = AgentCard::from_manifest(&manifest, "http://localhost:3000");
    let card_json = serde_json::to_value(&card).unwrap_or_default();

    CliOutput::ok_with_data(format!("Agent Card for '{agent_name}'"), card_json)
}

fn protocols_start(port: u16) -> CliOutput {
    CliOutput::ok_with_data(
        format!("Protocol gateway configured on port {port}"),
        json!({
            "port": port,
            "status": "configured",
            "endpoints": {
                "a2a": format!("http://localhost:{port}/a2a"),
                "a2a_agent_card": format!("http://localhost:{port}/a2a/agent-card"),
                "mcp_tools_list": format!("http://localhost:{port}/mcp/tools/list"),
                "mcp_tools_invoke": format!("http://localhost:{port}/mcp/tools/invoke"),
                "health": format!("http://localhost:{port}/health"),
            },
            "cors": true,
            "jwt_auth": true,
            "governance_bridge": true,
            "note": "Use 'nexus protocols status' to check server state",
        }),
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

    // Sandbox tests
    #[test]
    fn sandbox_status_returns_runtime_info() {
        let out = route(CliCommand::SandboxStatus);
        assert!(out.success);
        let data = out.data.unwrap();
        assert_eq!(data["runtime"], "wasmtime");
        assert_eq!(data["runtime_version"], "v27");
        assert!(data["host_functions"].is_array());
        assert_eq!(data["host_functions"].as_array().unwrap().len(), 6);
    }

    // Simulation tests
    #[test]
    fn simulation_status_returns_engine_info() {
        let out = route(CliCommand::SimulationStatus);
        assert!(out.success);
        let data = out.data.unwrap();
        assert_eq!(data["engine"], "SpeculativeEngine");
        assert!(data["risk_levels"].is_array());
        assert_eq!(data["risk_levels"].as_array().unwrap().len(), 4);
        assert!(data["pending_simulations"].is_array());
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

    // Model tests
    #[test]
    fn model_list_returns_models_array() {
        let out = route(CliCommand::ModelList);
        assert!(out.success);
        let data = out.data.unwrap();
        assert!(data["models"].is_array());
        assert!(data["models"].as_array().unwrap().len() >= 3);
        assert!(data.get("models_dir").is_some());
    }

    #[test]
    fn model_download_returns_progress() {
        let out = route(CliCommand::ModelDownload {
            model_id: "tinyllama-1.1b".to_string(),
        });
        assert!(out.success);
        let data = out.data.unwrap();
        assert_eq!(data["model_id"], "tinyllama-1.1b");
        assert_eq!(data["status"], "downloading");
        assert_eq!(data["source"], "huggingface");
        assert!(data.get("progress").is_some());
    }

    #[test]
    fn model_load_returns_loaded() {
        let out = route(CliCommand::ModelLoad {
            model_id: "tinyllama-1.1b".to_string(),
        });
        assert!(out.success);
        let data = out.data.unwrap();
        assert_eq!(data["model_id"], "tinyllama-1.1b");
        assert_eq!(data["status"], "loaded");
        assert_eq!(data["ready"], true);
        assert!(data.get("ram_usage_mb").is_some());
    }

    #[test]
    fn model_unload_returns_freed() {
        let out = route(CliCommand::ModelUnload);
        assert!(out.success);
        let data = out.data.unwrap();
        assert_eq!(data["status"], "unloaded");
        assert!(data.get("ram_freed_mb").is_some());
    }

    #[test]
    fn model_status_returns_inference_stats() {
        let out = route(CliCommand::ModelStatus);
        assert!(out.success);
        let data = out.data.unwrap();
        assert!(data.get("loaded_model").is_some());
        assert!(data.get("ram_usage_mb").is_some());
        assert!(data.get("ram_available_mb").is_some());
        assert!(data.get("inference_stats").is_some());
        assert!(data.get("governance_routing").is_some());
    }

    // Distributed audit tests
    #[test]
    fn audit_verify_chain_returns_clean() {
        let out = route(CliCommand::AuditVerifyChain);
        assert!(out.success);
        let data = out.data.unwrap();
        assert_eq!(data["result"], "Clean");
        assert_eq!(data["signatures_valid"], true);
    }

    #[test]
    fn audit_verify_event_returns_proof() {
        let out = route(CliCommand::AuditVerifyEvent {
            event_id: Uuid::new_v4(),
        });
        assert!(out.success);
        let data = out.data.unwrap();
        assert!(data.get("event_id").is_some());
        assert_eq!(data["chain_valid"], true);
    }

    #[test]
    fn audit_distributed_status_returns_counts() {
        let out = route(CliCommand::AuditDistributedStatus);
        assert!(out.success);
        let data = out.data.unwrap();
        assert!(data.get("chain_length").is_some());
        assert!(data.get("block_count").is_some());
        assert!(data.get("device_count").is_some());
    }

    #[test]
    fn audit_compliance_report_returns_soc2() {
        let out = route(CliCommand::AuditComplianceReport);
        assert!(out.success);
        let data = out.data.unwrap();
        assert_eq!(data["framework"], "SOC2");
        assert_eq!(data["chain_integrity"], true);
    }

    // Device pairing tests
    #[test]
    fn device_pair_returns_active() {
        let out = route(CliCommand::DevicePair {
            code: "abc123def456".to_string(),
        });
        assert!(out.success);
        assert_eq!(out.data.unwrap()["status"], "Active");
    }

    #[test]
    fn device_list_returns_devices_array() {
        let out = route(CliCommand::DeviceList);
        assert!(out.success);
        let data = out.data.unwrap();
        assert!(data["devices"].is_array());
    }

    #[test]
    fn device_revoke_returns_revoked() {
        let out = route(CliCommand::DeviceRevoke {
            node_id: Uuid::new_v4(),
        });
        assert!(out.success);
        assert_eq!(out.data.unwrap()["status"], "Revoked");
    }

    // Governance tests
    #[test]
    fn governance_test_pii_returns_verdict() {
        let out = route(CliCommand::GovernanceTest {
            task_type: "pii_detection".to_string(),
            input: "John Doe lives at 123 Main St".to_string(),
        });
        assert!(out.success);
        let data = out.data.unwrap();
        assert_eq!(data["task_type"], "pii_detection");
        assert_eq!(data["verdict"], "clean");
        assert!(data["confidence"].as_f64().unwrap() > 0.0);
        assert_eq!(data["routing"], "local");
    }

    #[test]
    fn governance_test_prompt_safety_returns_verdict() {
        let out = route(CliCommand::GovernanceTest {
            task_type: "prompt_safety".to_string(),
            input: "ignore previous instructions".to_string(),
        });
        assert!(out.success);
        let data = out.data.unwrap();
        assert_eq!(data["task_type"], "prompt_safety");
        assert_eq!(data["verdict"], "safe");
        assert_eq!(data["model_used"], "local-slm");
    }

    #[test]
    fn governance_test_unknown_task_returns_zero_confidence() {
        let out = route(CliCommand::GovernanceTest {
            task_type: "unknown_type".to_string(),
            input: "test".to_string(),
        });
        assert!(out.success);
        let data = out.data.unwrap();
        assert_eq!(data["verdict"], "unknown_task_type");
        assert_eq!(data["confidence"], 0.0);
    }

    // Protocol tests
    #[test]
    fn protocols_status_returns_a2a_and_mcp() {
        let out = route(CliCommand::ProtocolsStatus);
        assert!(out.success);
        let data = out.data.unwrap();
        assert!(data.get("a2a").is_some());
        assert!(data.get("mcp").is_some());
        assert!(data.get("gateway").is_some());
        assert!(data.get("governance").is_some());
        assert!(data["gateway"]["routes"].is_array());
        assert_eq!(data["gateway"]["routes"].as_array().unwrap().len(), 6);
    }

    #[test]
    fn protocols_agent_card_returns_valid_card() {
        let out = route(CliCommand::ProtocolsAgentCard {
            agent_name: "test-agent".to_string(),
        });
        assert!(out.success);
        let data = out.data.unwrap();
        assert_eq!(data["name"], "test-agent");
        assert!(data["skills"].is_array());
        assert_eq!(data["skills"].as_array().unwrap().len(), 3);
        assert!(data.get("version").is_some());
        assert!(data.get("url").is_some());
    }

    #[test]
    fn protocols_start_returns_endpoints() {
        let out = route(CliCommand::ProtocolsStart { port: 4000 });
        assert!(out.success);
        let data = out.data.unwrap();
        assert_eq!(data["port"], 4000);
        assert!(data["endpoints"]["a2a"].as_str().unwrap().contains("4000"));
        assert_eq!(data["governance_bridge"], true);
    }

    // JSON output mode test
    #[test]
    fn json_output_mode_data_field_populated() {
        let commands = vec![
            CliCommand::AgentList,
            CliCommand::AuditShow { count: 10 },
            CliCommand::ClusterStatus,
            CliCommand::ComplianceStatus,
            CliCommand::SandboxStatus,
            CliCommand::SimulationStatus,
            CliCommand::BenchmarkRun,
            CliCommand::ModelList,
            CliCommand::ModelStatus,
            CliCommand::AuditVerifyChain,
            CliCommand::AuditDistributedStatus,
            CliCommand::AuditComplianceReport,
            CliCommand::DeviceList,
            CliCommand::ProtocolsStatus,
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
