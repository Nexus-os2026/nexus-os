//! Executor agent — executes plan steps through the governance pipeline.

use super::AgentEvent;
use crate::tools::ToolContext;

/// Result of a single plan step execution.
#[derive(Debug, Clone)]
pub struct StepResult {
    pub step: u32,
    pub success: bool,
    pub output: String,
    pub duration_ms: u64,
}

/// Execute a Plan step by step, with governance enforcement on each step.
/// Returns a result for each step.
pub async fn execute_plan(
    plan: &super::planner::Plan,
    tool_ctx: &ToolContext,
    governance: &mut crate::governance::GovernanceKernel,
    event_tx: &tokio::sync::mpsc::UnboundedSender<AgentEvent>,
    consent_handler: &(dyn Fn(&crate::governance::ConsentRequest) -> bool + Send + Sync),
) -> Result<Vec<StepResult>, crate::error::NxError> {
    let mut results = Vec::new();

    for step in &plan.steps {
        let _ = event_tx.send(AgentEvent::ToolCallStart {
            name: step.tool.clone(),
            id: format!("plan-step-{}", step.step),
        });

        // Create a standalone tool instance (avoids borrow conflicts)
        let tool = match crate::tools::create_tool(&step.tool) {
            Some(t) => t,
            None => {
                let _ = event_tx.send(AgentEvent::ToolCallDenied {
                    name: step.tool.clone(),
                    reason: "Unknown tool".to_string(),
                });
                results.push(StepResult {
                    step: step.step,
                    success: false,
                    output: format!("Unknown tool: {}", step.tool),
                    duration_ms: 0,
                });
                continue;
            }
        };

        // Execute through governance pipeline
        match crate::tools::execute_governed(
            tool.as_ref(),
            step.input.clone(),
            tool_ctx,
            governance,
        )
        .await
        {
            Ok(tool_result) => {
                let success = tool_result.is_success();
                let _ = event_tx.send(AgentEvent::ToolCallComplete {
                    name: step.tool.clone(),
                    success,
                    duration_ms: tool_result.duration_ms,
                    summary: tool_result.summary(),
                });
                results.push(StepResult {
                    step: step.step,
                    success,
                    output: tool_result.output,
                    duration_ms: tool_result.duration_ms,
                });
            }
            Err(crate::error::NxError::ConsentRequired { request }) => {
                let granted = consent_handler(&request);
                if granted {
                    match crate::tools::execute_after_consent(
                        tool.as_ref(),
                        step.input.clone(),
                        tool_ctx,
                        governance,
                        &request,
                        true,
                    )
                    .await
                    {
                        Ok(tool_result) => {
                            let success = tool_result.is_success();
                            let _ = event_tx.send(AgentEvent::ToolCallComplete {
                                name: step.tool.clone(),
                                success,
                                duration_ms: tool_result.duration_ms,
                                summary: tool_result.summary(),
                            });
                            results.push(StepResult {
                                step: step.step,
                                success,
                                output: tool_result.output,
                                duration_ms: tool_result.duration_ms,
                            });
                        }
                        Err(e) => {
                            results.push(StepResult {
                                step: step.step,
                                success: false,
                                output: format!("Execution error: {}", e),
                                duration_ms: 0,
                            });
                        }
                    }
                } else {
                    let fuel_est = tool.estimated_fuel(&step.input);
                    let _ = governance.finalize_authorization(&request, false, fuel_est);
                    let _ = event_tx.send(AgentEvent::ToolCallDenied {
                        name: step.tool.clone(),
                        reason: "User denied consent".to_string(),
                    });
                    results.push(StepResult {
                        step: step.step,
                        success: false,
                        output: "Denied by user".to_string(),
                        duration_ms: 0,
                    });
                }
            }
            Err(e) => {
                let _ = event_tx.send(AgentEvent::ToolCallDenied {
                    name: step.tool.clone(),
                    reason: format!("{}", e),
                });
                results.push(StepResult {
                    step: step.step,
                    success: false,
                    output: format!("{}", e),
                    duration_ms: 0,
                });
            }
        }
    }

    Ok(results)
}
