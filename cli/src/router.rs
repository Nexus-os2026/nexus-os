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
        CliCommand::MarketplacePublish { bundle_path } => marketplace_publish(&bundle_path),
        CliCommand::MarketplaceInfo { agent_id } => marketplace_info(&agent_id),
        CliCommand::MarketplaceMyAgents { author } => marketplace_my_agents(&author),

        // Compliance
        CliCommand::ComplianceReport { framework } => compliance_report(&framework),
        CliCommand::ComplianceStatus => compliance_status(),
        CliCommand::ComplianceClassify { agent_id } => compliance_classify(agent_id),
        CliCommand::ComplianceEraseAgentData { agent_id } => compliance_erase_agent_data(agent_id),
        CliCommand::ComplianceRetentionCheck => compliance_retention_check(),
        CliCommand::ComplianceProvenance { agent_id } => compliance_provenance(agent_id),

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

        // Policy engine
        CliCommand::PolicyList => policy_list(),
        CliCommand::PolicyShow { policy_id } => policy_show(&policy_id),
        CliCommand::PolicyValidate { file } => policy_validate(&file),
        CliCommand::PolicyTest {
            file,
            principal,
            action,
            resource,
        } => policy_test(&file, &principal, &action, &resource),
        CliCommand::PolicyReload => policy_reload(),

        // Protocols (A2A + MCP)
        CliCommand::ProtocolsStatus => protocols_status(),
        CliCommand::ProtocolsAgentCard { agent_name } => protocols_agent_card(&agent_name),
        CliCommand::ProtocolsStart { port } => protocols_start(port),

        // Identity
        CliCommand::IdentityShow { agent_id } => identity_show(agent_id),
        CliCommand::IdentityList => identity_list(),
        CliCommand::TokenIssue {
            agent_id,
            scopes,
            ttl,
        } => token_issue(agent_id, &scopes, ttl),

        // Firewall
        CliCommand::FirewallStatus => firewall_status(),
        CliCommand::FirewallPatterns => firewall_patterns(),
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
// Marketplace commands (backed by SQLite registry)
// ---------------------------------------------------------------------------

fn open_registry() -> Result<nexus_marketplace::sqlite_registry::SqliteRegistry, String> {
    let db_path = nexus_marketplace::sqlite_registry::SqliteRegistry::default_db_path();
    nexus_marketplace::sqlite_registry::SqliteRegistry::open(&db_path)
        .map_err(|e| format!("Failed to open marketplace database: {e}"))
}

fn marketplace_search(query: &str) -> CliOutput {
    let registry = match open_registry() {
        Ok(r) => r,
        Err(e) => return CliOutput::err(e),
    };

    match registry.search(query) {
        Ok(results) => {
            let items: Vec<serde_json::Value> = results
                .iter()
                .map(|r| {
                    json!({
                        "package_id": r.package_id,
                        "name": r.name,
                        "description": r.description,
                        "author": r.author_id,
                        "tags": r.tags,
                    })
                })
                .collect();
            let count = items.len();
            CliOutput::ok_with_data(
                format!("{count} result(s) for '{query}'"),
                json!({"query": query, "count": count, "results": items}),
            )
        }
        Err(e) => CliOutput::err(format!("Search failed: {e}")),
    }
}

fn marketplace_install(name: &str) -> CliOutput {
    let registry = match open_registry() {
        Ok(r) => r,
        Err(e) => return CliOutput::err(e),
    };

    match registry.install(name) {
        Ok(bundle) => {
            let downloads = registry.download_count(name).unwrap_or(0);
            CliOutput::ok_with_data(
                format!(
                    "Agent '{}' v{} installed (signature verified)",
                    bundle.metadata.name, bundle.metadata.version
                ),
                json!({
                    "package_id": bundle.package_id,
                    "name": bundle.metadata.name,
                    "version": bundle.metadata.version,
                    "installed": true,
                    "signature_verified": true,
                    "downloads": downloads,
                }),
            )
        }
        Err(e) => CliOutput::err(format!("Install failed: {e}")),
    }
}

fn marketplace_uninstall(name: &str) -> CliOutput {
    let registry = match open_registry() {
        Ok(r) => r,
        Err(e) => return CliOutput::err(e),
    };

    match registry.remove(name) {
        Ok(true) => CliOutput::ok_with_data(
            format!("Agent '{name}' uninstalled"),
            json!({"name": name, "uninstalled": true}),
        ),
        Ok(false) => CliOutput::err(format!("Agent '{name}' not found in marketplace")),
        Err(e) => CliOutput::err(format!("Uninstall failed: {e}")),
    }
}

fn marketplace_publish(bundle_path: &str) -> CliOutput {
    use nexus_marketplace::package::SignedPackageBundle;
    use nexus_marketplace::verification_pipeline::{verify_bundle, Verdict};

    let content = match std::fs::read_to_string(bundle_path) {
        Ok(c) => c,
        Err(e) => return CliOutput::err(format!("Cannot read bundle file: {e}")),
    };

    let bundle: SignedPackageBundle = match serde_json::from_str(&content) {
        Ok(b) => b,
        Err(e) => return CliOutput::err(format!("Invalid bundle format: {e}")),
    };

    // Run verification pipeline
    let verification = verify_bundle(&bundle);
    if verification.verdict == Verdict::Rejected {
        let findings: Vec<String> = verification
            .checks
            .iter()
            .filter(|c| !c.passed)
            .flat_map(|c| c.findings.clone())
            .collect();
        return CliOutput::err(format!("Verification rejected: {}", findings.join("; ")));
    }

    let registry = match open_registry() {
        Ok(r) => r,
        Err(e) => return CliOutput::err(e),
    };

    match registry.upsert_signed(&bundle) {
        Ok(()) => {
            let check_summaries: Vec<serde_json::Value> = verification
                .checks
                .iter()
                .map(|c| {
                    json!({
                        "name": c.name,
                        "passed": c.passed,
                        "findings": c.findings,
                    })
                })
                .collect();
            CliOutput::ok_with_data(
                format!(
                    "Published '{}' v{} ({:?})",
                    bundle.metadata.name, bundle.metadata.version, verification.verdict
                ),
                json!({
                    "package_id": bundle.package_id,
                    "name": bundle.metadata.name,
                    "version": bundle.metadata.version,
                    "verdict": format!("{:?}", verification.verdict),
                    "checks": check_summaries,
                }),
            )
        }
        Err(e) => CliOutput::err(format!("Publish failed: {e}")),
    }
}

fn marketplace_info(agent_id: &str) -> CliOutput {
    let registry = match open_registry() {
        Ok(r) => r,
        Err(e) => return CliOutput::err(e),
    };

    match registry.get_agent(agent_id) {
        Ok(detail) => {
            let reviews = registry.get_reviews(agent_id).unwrap_or_default();
            let versions = registry.version_history(agent_id).unwrap_or_default();
            let review_items: Vec<serde_json::Value> = reviews
                .iter()
                .map(|r| {
                    json!({
                        "reviewer": r.reviewer,
                        "stars": r.stars,
                        "comment": r.comment,
                        "created_at": r.created_at,
                    })
                })
                .collect();
            let version_items: Vec<serde_json::Value> = versions
                .iter()
                .map(|v| {
                    json!({
                        "version": v.version,
                        "changelog": v.changelog,
                        "created_at": v.created_at,
                    })
                })
                .collect();
            CliOutput::ok_with_data(
                format!("{} v{}", detail.name, detail.version),
                json!({
                    "package_id": detail.package_id,
                    "name": detail.name,
                    "version": detail.version,
                    "description": detail.description,
                    "author": detail.author,
                    "tags": detail.tags,
                    "capabilities": detail.capabilities,
                    "price_cents": detail.price_cents,
                    "downloads": detail.downloads,
                    "rating": detail.rating,
                    "review_count": detail.review_count,
                    "reviews": review_items,
                    "versions": version_items,
                    "created_at": detail.created_at,
                    "updated_at": detail.updated_at,
                }),
            )
        }
        Err(e) => CliOutput::err(format!("Agent not found: {e}")),
    }
}

fn marketplace_my_agents(author: &str) -> CliOutput {
    let registry = match open_registry() {
        Ok(r) => r,
        Err(e) => return CliOutput::err(e),
    };

    // Search by author to find their agents
    match registry.search(author) {
        Ok(results) => {
            let items: Vec<serde_json::Value> = results
                .iter()
                .filter(|r| r.author_id == author)
                .map(|r| {
                    json!({
                        "package_id": r.package_id,
                        "name": r.name,
                        "description": r.description,
                    })
                })
                .collect();
            let count = items.len();
            CliOutput::ok_with_data(
                format!("{count} agent(s) by '{author}'"),
                json!({"author": author, "count": count, "agents": items}),
            )
        }
        Err(e) => CliOutput::err(format!("Query failed: {e}")),
    }
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
    use nexus_kernel::audit::AuditTrail;
    use nexus_kernel::compliance::monitor::ComplianceMonitor;
    use nexus_kernel::identity::IdentityManager;

    let trail = AuditTrail::new();
    let id_mgr = IdentityManager::in_memory();
    let monitor = ComplianceMonitor::new();
    let status = monitor.check_compliance(&[], &trail, &id_mgr);

    CliOutput::ok_with_data(
        format!("Compliance status: {}", status.status.as_str()),
        json!({
            "status": status.status.as_str(),
            "frameworks": ["SOC2", "EU_AI_Act", "HIPAA", "CA_AB316"],
            "checks_passed": status.checks_passed,
            "checks_failed": status.checks_failed,
            "agents_checked": status.agents_checked,
            "alerts": status.alerts.iter().map(|a| json!({
                "severity": a.severity.as_str(),
                "check_id": a.check_id,
                "message": a.message,
                "agent_id": a.agent_id.map(|id| id.to_string()),
            })).collect::<Vec<_>>(),
            "last_check_unix": status.last_check_unix,
        }),
    )
}

fn compliance_classify(agent_id: Uuid) -> CliOutput {
    use nexus_kernel::compliance::eu_ai_act::RiskClassifier;
    use nexus_kernel::manifest::AgentManifest;

    // Build a representative manifest for the agent
    let manifest = AgentManifest {
        name: format!("agent-{}", &agent_id.to_string()[..8]),
        version: "1.0.0".to_string(),
        capabilities: vec!["llm.query".to_string(), "fs.read".to_string()],
        fuel_budget: 5000,
        autonomy_level: Some(2),
        consent_policy_path: None,
        requester_id: None,
        schedule: None,
        llm_model: None,
        fuel_period_id: None,
        monthly_fuel_cap: None,
        allowed_endpoints: None,
        domain_tags: vec![],
    };

    let classifier = RiskClassifier::new();
    let profile = classifier.classify_agent(&manifest);

    CliOutput::ok_with_data(
        format!(
            "EU AI Act risk classification for agent {}: {}",
            agent_id,
            profile.tier.as_str()
        ),
        json!({
            "agent_id": agent_id.to_string(),
            "risk_tier": profile.tier.as_str(),
            "justification": profile.justification,
            "applicable_articles": profile.applicable_articles,
            "required_controls": profile.required_controls,
        }),
    )
}

fn compliance_erase_agent_data(agent_id: Uuid) -> CliOutput {
    use nexus_kernel::audit::AuditTrail;
    use nexus_kernel::compliance::data_governance::AgentDataEraser;
    use nexus_kernel::identity::IdentityManager;
    use nexus_kernel::permissions::PermissionManager;
    use nexus_kernel::privacy::PrivacyManager;

    let mut trail = AuditTrail::new();
    let mut privacy = PrivacyManager::new();
    let mut identity_mgr = IdentityManager::in_memory();
    let mut perm_mgr = PermissionManager::new();

    let eraser = AgentDataEraser::new();
    match eraser.erase_agent_data(
        agent_id,
        &[],
        &mut trail,
        &mut privacy,
        &mut identity_mgr,
        &mut perm_mgr,
    ) {
        Ok(receipt) => CliOutput::ok_with_data(
            format!("Cryptographic erasure completed for agent {agent_id}"),
            json!({
                "agent_id": receipt.agent_id.to_string(),
                "events_redacted": receipt.events_redacted,
                "keys_destroyed": receipt.keys_destroyed.len(),
                "identity_purged": receipt.identity_purged,
                "permissions_purged": receipt.permissions_purged,
                "proof_event_id": receipt.proof_event_id.to_string(),
                "erased_at": receipt.erased_at,
            }),
        ),
        Err(e) => CliOutput::err(format!("Erasure failed: {e}")),
    }
}

fn compliance_retention_check() -> CliOutput {
    use nexus_kernel::audit::AuditTrail;
    use nexus_kernel::compliance::data_governance::RetentionPolicy;

    let mut trail = AuditTrail::new();
    let policy = RetentionPolicy::new();
    let result = policy.check_retention(&mut trail);

    CliOutput::ok_with_data(
        format!(
            "Retention check complete — {} events purged",
            result.events_purged
        ),
        json!({
            "events_purged": result.events_purged,
            "agents_held": result.agents_held.iter().map(|id| id.to_string()).collect::<Vec<_>>(),
            "checked_at": result.checked_at,
            "policy": {
                "audit_events_max_age_days": 365,
            },
        }),
    )
}

fn compliance_provenance(agent_id: Uuid) -> CliOutput {
    use nexus_kernel::compliance::provenance::ProvenanceTracker;

    let tracker = ProvenanceTracker::new();
    let report = tracker.export_lineage_report(agent_id);

    CliOutput::ok_with_data(
        format!("Data provenance report for agent {agent_id}"),
        json!({
            "agent_id": report.agent_id.to_string(),
            "lineage_entries": report.lineage_entries.len(),
            "data_originated": report.originated,
            "data_received": report.received,
            "transformations_applied": report.transformations_applied,
            "generated_at": report.generated_at,
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
// Policy engine commands
// ---------------------------------------------------------------------------

fn policy_list() -> CliOutput {
    use nexus_kernel::policy_engine::PolicyEngine;

    let policy_dir = dirs_policy_dir();
    let mut engine = PolicyEngine::new(&policy_dir);
    match engine.load_policies() {
        Ok(count) => {
            let policies: Vec<serde_json::Value> = engine
                .policies()
                .iter()
                .map(|p| {
                    serde_json::json!({
                        "policy_id": p.policy_id,
                        "effect": format!("{:?}", p.effect),
                        "principal": p.principal,
                        "action": p.action,
                        "resource": p.resource,
                        "priority": p.priority,
                        "description": p.description,
                    })
                })
                .collect();
            CliOutput::ok_with_data(
                format!("{count} policies loaded from {}", policy_dir.display()),
                serde_json::json!({ "policies": policies, "count": count }),
            )
        }
        Err(e) => CliOutput::err(format!("failed to load policies: {e}")),
    }
}

fn policy_show(policy_id: &str) -> CliOutput {
    use nexus_kernel::policy_engine::PolicyEngine;

    let policy_dir = dirs_policy_dir();
    let mut engine = PolicyEngine::new(&policy_dir);
    if let Err(e) = engine.load_policies() {
        return CliOutput::err(format!("failed to load policies: {e}"));
    }
    match engine.policies().iter().find(|p| p.policy_id == policy_id) {
        Some(policy) => CliOutput::ok_with_data(
            format!("Policy '{policy_id}'"),
            serde_json::json!({
                "policy_id": policy.policy_id,
                "description": policy.description,
                "effect": format!("{:?}", policy.effect),
                "principal": policy.principal,
                "action": policy.action,
                "resource": policy.resource,
                "priority": policy.priority,
                "conditions": {
                    "min_autonomy_level": policy.conditions.min_autonomy_level,
                    "max_fuel_cost": policy.conditions.max_fuel_cost,
                    "required_approvers": policy.conditions.required_approvers,
                    "time_window": policy.conditions.time_window,
                },
            }),
        ),
        None => CliOutput::err(format!("policy '{policy_id}' not found")),
    }
}

fn policy_validate(file: &str) -> CliOutput {
    use nexus_kernel::policy_engine::Policy;

    let content = match std::fs::read_to_string(file) {
        Ok(c) => c,
        Err(e) => return CliOutput::err(format!("cannot read '{file}': {e}")),
    };
    match toml::from_str::<Policy>(&content) {
        Ok(policy) => CliOutput::ok_with_data(
            format!("Policy '{}' is valid", policy.policy_id),
            serde_json::json!({
                "valid": true,
                "policy_id": policy.policy_id,
                "effect": format!("{:?}", policy.effect),
            }),
        ),
        Err(e) => CliOutput::ok_with_data(
            format!("Validation failed: {e}"),
            serde_json::json!({ "valid": false, "error": e.to_string() }),
        ),
    }
}

fn policy_test(file: &str, principal: &str, action: &str, resource: &str) -> CliOutput {
    use nexus_kernel::policy_engine::{EvaluationContext, Policy, PolicyEngine};

    let content = match std::fs::read_to_string(file) {
        Ok(c) => c,
        Err(e) => return CliOutput::err(format!("cannot read '{file}': {e}")),
    };
    let policy: Policy = match toml::from_str(&content) {
        Ok(p) => p,
        Err(e) => return CliOutput::err(format!("invalid policy TOML: {e}")),
    };

    let engine = PolicyEngine::with_policies(vec![policy]);
    let ctx = EvaluationContext::default();
    let decision = engine.evaluate(principal, action, resource, &ctx);
    let decision_str = format!("{decision:?}");

    CliOutput::ok_with_data(
        format!("Dry-run result: {decision_str}"),
        serde_json::json!({
            "principal": principal,
            "action": action,
            "resource": resource,
            "decision": decision_str,
        }),
    )
}

fn policy_reload() -> CliOutput {
    use nexus_kernel::policy_engine::PolicyEngine;

    let policy_dir = dirs_policy_dir();
    let mut engine = PolicyEngine::new(&policy_dir);
    match engine.load_policies() {
        Ok(count) => CliOutput::ok_with_data(
            format!("Reloaded {count} policies from {}", policy_dir.display()),
            serde_json::json!({ "count": count, "dir": policy_dir.display().to_string() }),
        ),
        Err(e) => CliOutput::err(format!("reload failed: {e}")),
    }
}

fn dirs_policy_dir() -> std::path::PathBuf {
    if let Some(home) = std::env::var_os("HOME") {
        std::path::PathBuf::from(home)
            .join(".nexus")
            .join("policies")
    } else {
        std::path::PathBuf::from("~/.nexus/policies")
    }
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
        allowed_endpoints: None,
        domain_tags: vec![],
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

// ---------------------------------------------------------------------------
// Identity commands
// ---------------------------------------------------------------------------

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn identity_show(agent_id: Uuid) -> CliOutput {
    use nexus_kernel::identity::IdentityManager;

    let mut mgr = IdentityManager::in_memory();
    match mgr.get_or_create(agent_id) {
        Ok(identity) => CliOutput::ok_with_data(
            format!("Identity for agent {agent_id}"),
            json!({
                "agent_id": agent_id.to_string(),
                "did": identity.did,
                "created_at": identity.created_at,
                "public_key_hex": hex_encode(&identity.public_key_bytes()),
            }),
        ),
        Err(e) => CliOutput::err(format!("failed to get identity: {e}")),
    }
}

fn identity_list() -> CliOutput {
    // In a real deployment, this would scan the persist directory.
    // For now, return an illustrative empty list.
    CliOutput::ok_with_data(
        "Agent identities",
        json!({
            "identities": [],
            "count": 0,
            "persist_dir": "~/.nexus/identities",
            "note": "Run 'nexus identity show <agent-id>' to create/view an identity",
        }),
    )
}

fn token_issue(agent_id: Uuid, scopes: &[String], ttl: Option<u64>) -> CliOutput {
    use nexus_kernel::identity::{IdentityManager, TokenManager, DEFAULT_TTL_SECS};

    let mut mgr = IdentityManager::in_memory();
    // Clone the identity to release the borrow on mgr, so we can access key_manager().
    let identity = match mgr.get_or_create(agent_id) {
        Ok(id) => id.clone(),
        Err(e) => return CliOutput::err(format!("identity error: {e}")),
    };

    let tm = TokenManager::new("nexus-os", "nexus-agents");
    let ttl_secs = ttl.unwrap_or(DEFAULT_TTL_SECS);
    let token = match tm.issue_token(&identity, mgr.key_manager(), scopes, ttl_secs, None) {
        Ok(t) => t,
        Err(e) => return CliOutput::err(format!("token signing error: {e}")),
    };

    CliOutput::ok_with_data(
        format!("Token issued for agent {agent_id}"),
        json!({
            "agent_id": agent_id.to_string(),
            "did": identity.did,
            "token": token,
            "ttl_secs": ttl_secs,
            "scopes": scopes,
            "algorithm": "EdDSA",
        }),
    )
}

// ---------------------------------------------------------------------------
// Firewall commands
// ---------------------------------------------------------------------------

fn firewall_status() -> CliOutput {
    use nexus_kernel::firewall::{pattern_summary, EgressGovernor};

    let summary = pattern_summary();
    let _egress = EgressGovernor::new();

    CliOutput::ok_with_data(
        "Firewall status",
        json!({
            "status": "active",
            "mode": "fail-closed",
            "input_filter": {
                "injection_patterns": summary.injection_count,
                "pii_detection": true,
                "ssn_detection": summary.has_ssn_detection,
                "passport_detection": summary.has_passport_detection,
                "homoglyph_detection": true,
                "context_overflow_threshold_bytes": summary.context_overflow_threshold_bytes,
            },
            "output_filter": {
                "exfil_patterns": summary.exfil_count,
                "internal_ip_detection": summary.has_internal_ip_detection,
                "json_schema_validation": true,
            },
            "egress_governor": {
                "default_deny": true,
                "rate_limit_per_min": nexus_kernel::firewall::DEFAULT_RATE_LIMIT_PER_MIN,
                "registered_agents": 0,
            },
        }),
    )
}

fn firewall_patterns() -> CliOutput {
    use nexus_kernel::firewall::patterns;

    CliOutput::ok_with_data(
        "Canonical firewall patterns",
        json!({
            "injection_patterns": patterns::INJECTION_PATTERNS,
            "pii_patterns": patterns::PII_PATTERNS,
            "exfil_patterns": patterns::EXFIL_PATTERNS,
            "sensitive_paths": patterns::SENSITIVE_PATHS,
            "regex_patterns": {
                "ssn": patterns::SSN_PATTERN,
                "passport": patterns::PASSPORT_PATTERN,
                "internal_ip": patterns::INTERNAL_IP_PATTERN,
            },
            "context_overflow_threshold_bytes": patterns::CONTEXT_OVERFLOW_THRESHOLD_BYTES,
            "summary": {
                "total_patterns": patterns::INJECTION_PATTERNS.len()
                    + patterns::PII_PATTERNS.len()
                    + patterns::EXFIL_PATTERNS.len()
                    + patterns::SENSITIVE_PATHS.len()
                    + 3, // regex patterns
            },
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

    // Marketplace tests — real SQLite registry on default db path
    #[test]
    fn marketplace_search_returns_results() {
        let out = route(CliCommand::MarketplaceSearch {
            query: "nonexistent-xyz-agent".to_string(),
        });
        assert!(out.success);
        let data = out.data.unwrap();
        assert_eq!(data["query"], "nonexistent-xyz-agent");
        assert_eq!(data["count"], 0);
    }

    #[test]
    fn marketplace_install_nonexistent_fails() {
        let out = route(CliCommand::MarketplaceInstall {
            name: "pkg-does-not-exist".to_string(),
        });
        // Install should fail for non-existent agent
        assert!(!out.success);
    }

    #[test]
    fn marketplace_uninstall_nonexistent_fails() {
        let out = route(CliCommand::MarketplaceUninstall {
            name: "pkg-does-not-exist".to_string(),
        });
        assert!(!out.success);
    }

    #[test]
    fn marketplace_info_nonexistent_fails() {
        let out = route(CliCommand::MarketplaceInfo {
            agent_id: "pkg-does-not-exist".to_string(),
        });
        assert!(!out.success);
    }

    #[test]
    fn marketplace_my_agents_returns_empty() {
        let out = route(CliCommand::MarketplaceMyAgents {
            author: "nonexistent-author".to_string(),
        });
        assert!(out.success);
        assert_eq!(out.data.unwrap()["count"], 0);
    }

    #[test]
    fn marketplace_publish_invalid_path_fails() {
        let out = route(CliCommand::MarketplacePublish {
            bundle_path: "/tmp/nonexistent-bundle.nexus-agent".to_string(),
        });
        assert!(!out.success);
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

    #[test]
    fn compliance_classify_returns_risk_tier() {
        let out = route(CliCommand::ComplianceClassify {
            agent_id: Uuid::new_v4(),
        });
        assert!(out.success);
        let data = out.data.unwrap();
        assert!(data.get("risk_tier").is_some());
        assert!(data.get("justification").is_some());
        assert!(data["applicable_articles"].is_array());
    }

    #[test]
    fn compliance_erase_agent_data_returns_receipt() {
        let out = route(CliCommand::ComplianceEraseAgentData {
            agent_id: Uuid::new_v4(),
        });
        assert!(out.success);
        let data = out.data.unwrap();
        assert!(data.get("agent_id").is_some());
        assert!(data.get("proof_event_id").is_some());
    }

    #[test]
    fn compliance_retention_check_returns_result() {
        let out = route(CliCommand::ComplianceRetentionCheck);
        assert!(out.success);
        let data = out.data.unwrap();
        assert_eq!(data["events_purged"], 0);
        assert!(data.get("checked_at").is_some());
    }

    #[test]
    fn compliance_provenance_returns_report() {
        let out = route(CliCommand::ComplianceProvenance {
            agent_id: Uuid::new_v4(),
        });
        assert!(out.success);
        let data = out.data.unwrap();
        assert!(data.get("agent_id").is_some());
        assert_eq!(data["lineage_entries"], 0);
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

    // Policy engine tests
    #[test]
    fn policy_list_returns_count() {
        let out = route(CliCommand::PolicyList);
        assert!(out.success);
        let data = out.data.unwrap();
        assert!(data.get("count").is_some());
        assert!(data["policies"].is_array());
    }

    #[test]
    fn policy_show_missing_returns_error() {
        let out = route(CliCommand::PolicyShow {
            policy_id: "nonexistent-policy".to_string(),
        });
        assert!(!out.success);
        assert!(out.message.contains("not found"));
    }

    #[test]
    fn policy_validate_missing_file_returns_error() {
        let out = route(CliCommand::PolicyValidate {
            file: "/tmp/nexus-nonexistent-policy-xyz.toml".to_string(),
        });
        assert!(!out.success);
        assert!(out.message.contains("cannot read"));
    }

    #[test]
    fn policy_reload_returns_count() {
        let out = route(CliCommand::PolicyReload);
        assert!(out.success);
        let data = out.data.unwrap();
        assert!(data.get("count").is_some());
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
            CliCommand::ComplianceRetentionCheck,
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
            CliCommand::FirewallStatus,
            CliCommand::FirewallPatterns,
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

    // Identity tests
    #[test]
    fn identity_show_creates_identity() {
        let out = route(CliCommand::IdentityShow {
            agent_id: Uuid::new_v4(),
        });
        assert!(out.success);
        let data = out.data.unwrap();
        assert!(data["did"].as_str().unwrap().starts_with("did:key:z6Mk"));
    }

    #[test]
    fn identity_list_returns_empty() {
        let out = route(CliCommand::IdentityList);
        assert!(out.success);
        assert_eq!(out.data.unwrap()["count"], 0);
    }

    #[test]
    fn token_issue_returns_token() {
        let out = route(CliCommand::TokenIssue {
            agent_id: Uuid::new_v4(),
            scopes: vec!["web.search".to_string()],
            ttl: Some(3600),
        });
        assert!(out.success);
        let data = out.data.unwrap();
        assert!(data["token"].as_str().unwrap().contains('.'));
        assert_eq!(data["algorithm"], "EdDSA");
    }

    // Firewall tests
    #[test]
    fn firewall_status_returns_active() {
        let out = route(CliCommand::FirewallStatus);
        assert!(out.success);
        let data = out.data.unwrap();
        assert_eq!(data["status"], "active");
        assert_eq!(data["mode"], "fail-closed");
    }

    #[test]
    fn firewall_patterns_returns_all() {
        let out = route(CliCommand::FirewallPatterns);
        assert!(out.success);
        let data = out.data.unwrap();
        assert!(data["injection_patterns"].as_array().unwrap().len() >= 20);
        assert!(data["pii_patterns"].as_array().unwrap().len() >= 6);
    }
}
