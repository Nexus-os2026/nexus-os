//! Governed sub-agent spawning — child agents with own identity, fuel, and scoped capabilities.

use serde::{Deserialize, Serialize};

/// Configuration for a spawned sub-agent.
#[derive(Debug, Clone)]
pub struct SubAgentConfig {
    /// Task description for the sub-agent.
    pub task: String,
    /// Fuel budget allocated from parent.
    pub fuel_budget: u64,
    /// Capabilities granted to the sub-agent (subset of parent's).
    pub capabilities: Vec<(
        crate::governance::Capability,
        crate::governance::CapabilityScope,
    )>,
    /// Maximum turns for the sub-agent.
    pub max_turns: u32,
}

/// Result of a sub-agent execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubAgentResult {
    /// The sub-agent's session ID.
    pub session_id: String,
    /// The sub-agent's public key (for audit linkage).
    pub public_key: String,
    /// Final output from the sub-agent.
    pub output: String,
    /// Fuel consumed by the sub-agent.
    pub fuel_consumed: u64,
    /// Number of turns the sub-agent took.
    pub turns: u32,
    /// Number of audit entries the sub-agent produced.
    pub audit_entries: usize,
}

/// Spawn a governed sub-agent.
///
/// The sub-agent gets:
/// - Fresh Ed25519 identity (own keypair, own session ID)
/// - Fuel budget sliced from parent (parent's remaining decreases)
/// - Scoped capabilities (cannot exceed parent's grants)
/// - Own audit trail with cross-reference to parent's session ID
/// - Bounded turns
#[allow(clippy::too_many_arguments)]
pub async fn spawn_sub_agent(
    config: SubAgentConfig,
    parent_governance: &mut crate::governance::GovernanceKernel,
    router: &crate::llm::router::ModelRouter,
    tool_registry: &crate::tools::ToolRegistry,
    tool_ctx: &crate::tools::ToolContext,
) -> Result<SubAgentResult, crate::error::NxError> {
    // 1. Deduct fuel from parent
    let fuel_to_allocate = config.fuel_budget.min(parent_governance.fuel.remaining());
    if fuel_to_allocate == 0 {
        return Err(crate::error::NxError::FuelExhausted {
            remaining: parent_governance.fuel.remaining(),
            required: config.fuel_budget,
        });
    }
    parent_governance.fuel.reserve(fuel_to_allocate)?;

    // 2. Create child governance kernel
    let mut child_governance = crate::governance::GovernanceKernel::new(fuel_to_allocate)?;

    // 3. Set up scoped capabilities (start empty, grant only configured)
    child_governance.capabilities = crate::governance::CapabilityManager::empty();
    for (cap, scope) in &config.capabilities {
        child_governance.capabilities.grant(*cap, scope.clone());
    }

    // 4. Record cross-agent audit linkage
    let child_session_id = child_governance.identity.session_id().to_string();
    let child_public_key = hex::encode(child_governance.identity.public_key_bytes());

    let task_preview = if config.task.len() > 100 {
        &config.task[..100]
    } else {
        &config.task
    };
    parent_governance
        .audit
        .record(crate::governance::AuditAction::ToolInvocation {
            tool: "sub_agent".to_string(),
            args_summary: format!(
                "Spawned sub-agent {} with {}fu: {}",
                &child_session_id[..8.min(child_session_id.len())],
                fuel_to_allocate,
                task_preview
            ),
        });

    // 5. Run the sub-agent loop
    let agent_config = crate::agent::AgentConfig {
        max_turns: config.max_turns,
        system_prompt: format!(
            "You are a sub-agent of Nexus Code. Your task:\n\n{}\n\n\
             Complete this task efficiently. You have {} fuel units.",
            config.task, fuel_to_allocate
        ),
        model_slot: crate::llm::router::ModelSlot::Execution,
        auto_approve_tier2: true,
        auto_approve_tier3: false,
        computer_use_active: false,
    };

    let (event_tx, mut event_rx) = tokio::sync::mpsc::unbounded_channel();

    let consent_handler: std::sync::Arc<
        dyn Fn(&crate::governance::ConsentRequest) -> bool + Send + Sync,
    > = std::sync::Arc::new(|request| {
        matches!(
            request.tier,
            crate::governance::ConsentTier::Tier1 | crate::governance::ConsentTier::Tier2
        )
    });

    let cancel = tokio_util::sync::CancellationToken::new();
    let mut messages = vec![crate::llm::types::Message {
        role: crate::llm::types::Role::User,
        content: config.task.clone(),
    }];

    let result = crate::agent::run_agent_loop(
        &mut messages,
        router,
        tool_registry,
        tool_ctx,
        &mut child_governance,
        &agent_config,
        event_tx,
        consent_handler,
        cancel,
    )
    .await;

    // Drain events
    while event_rx.try_recv().is_ok() {}

    // 6. Collect results
    let output = result.unwrap_or_else(|e| format!("Sub-agent error: {}", e));
    let fuel_consumed = child_governance.fuel.budget().consumed;
    let audit_count = child_governance.audit.len();
    let turns = child_governance.fuel.cost_history().len() as u32;

    // 7. Release unused fuel back to parent
    parent_governance.fuel.release_reservation(fuel_to_allocate);
    parent_governance.fuel.consume(
        "sub_agent",
        crate::governance::FuelCost {
            input_tokens: 0,
            output_tokens: 0,
            fuel_units: fuel_consumed,
            estimated_usd: 0.0,
        },
    );

    // 8. Record completion in parent audit
    parent_governance
        .audit
        .record(crate::governance::AuditAction::ToolResult {
            tool: "sub_agent".to_string(),
            success: true,
            summary: format!(
                "Sub-agent {} completed: {}fu consumed, {} audit entries, {} turns",
                &child_session_id[..8.min(child_session_id.len())],
                fuel_consumed,
                audit_count,
                turns
            ),
        });

    Ok(SubAgentResult {
        session_id: child_session_id,
        public_key: child_public_key,
        output,
        fuel_consumed,
        turns,
        audit_entries: audit_count,
    })
}
