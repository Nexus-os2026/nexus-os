//! Agent loop runtime — drives the multi-turn LLM <-> Tool cycle.

use std::sync::Arc;
use tokio::sync::mpsc;

use super::tool_protocol::{self, ToolDefinition, ToolProtocol, ToolResultMessage};
use super::AgentEvent;
use crate::error::NxError;
use crate::governance::{AuditAction, FuelCost};
use crate::llm::types::{LlmRequest, Message, Role};

/// Configuration for the agent loop.
#[derive(Debug, Clone)]
pub struct AgentConfig {
    /// Maximum number of agentic turns (default: 10).
    pub max_turns: u32,
    /// System prompt (tool descriptions are appended automatically).
    pub system_prompt: String,
    /// Model slot to use (default: Execution).
    pub model_slot: crate::llm::router::ModelSlot,
    /// Whether to auto-approve Tier2 tools (for headless mode).
    pub auto_approve_tier2: bool,
    /// Whether to auto-approve Tier3 tools (DANGEROUS — headless mode only).
    pub auto_approve_tier3: bool,
    /// Whether computer use mode is active (appends autonomous developer prompt).
    pub computer_use_active: bool,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            max_turns: 10,
            system_prompt: String::new(),
            model_slot: crate::llm::router::ModelSlot::Execution,
            auto_approve_tier2: false,
            auto_approve_tier3: false,
            computer_use_active: false,
        }
    }
}

/// Estimate fuel cost for an LLM call based on message history size.
fn estimate_llm_fuel(messages: &[Message]) -> u64 {
    // Rough estimate: ~4 chars per token, minimum 500 fuel units
    let total_chars: usize = messages.iter().map(|m| m.content.len()).sum();
    let estimated_tokens = (total_chars / 4) as u64;
    estimated_tokens.max(500)
}

/// Build the tool definitions list from the registry.
fn build_tool_definitions(tool_registry: &crate::tools::ToolRegistry) -> Vec<ToolDefinition> {
    tool_registry
        .all()
        .iter()
        .map(|t| ToolDefinition::from_tool(t.as_ref()))
        .collect()
}

/// Run the agent loop.
///
/// Drives the multi-turn LLM <-> Tool cycle. Sends AgentEvents to the
/// provided channel so the REPL (or headless runner) can display progress.
///
/// The `consent_handler` closure is called when Tier2/3 consent is needed.
/// In interactive mode, it presents a prompt to the user.
/// In headless mode, it returns based on auto_approve settings.
///
/// Returns the final assistant message content.
#[allow(clippy::too_many_arguments)]
pub async fn run_agent_loop(
    messages: &mut Vec<Message>,
    router: &crate::llm::router::ModelRouter,
    tool_registry: &crate::tools::ToolRegistry,
    tool_ctx: &crate::tools::ToolContext,
    governance: &mut crate::governance::GovernanceKernel,
    config: &AgentConfig,
    event_tx: mpsc::UnboundedSender<AgentEvent>,
    consent_handler: Arc<dyn Fn(&crate::governance::ConsentRequest) -> bool + Send + Sync>,
    cancel: tokio_util::sync::CancellationToken,
) -> Result<String, NxError> {
    let tool_defs = build_tool_definitions(tool_registry);

    // Determine provider protocol from the router's slot config
    let provider_name = router
        .get_slot(config.model_slot)
        .map(|s| s.provider.clone())
        .unwrap_or_else(|| "openai".to_string());
    let protocol = ToolProtocol::for_provider(&provider_name);

    // Build full system prompt with tool descriptions (and computer use section if active)
    let system_prompt = super::build_system_prompt_with_computer_use(
        &config.system_prompt,
        tool_registry,
        config.computer_use_active,
    );

    let mut last_text = String::new();

    for turn in 0..config.max_turns {
        if cancel.is_cancelled() {
            let _ = event_tx.send(AgentEvent::Done {
                reason: "cancelled".to_string(),
                total_turns: turn,
            });
            return Ok(last_text);
        }

        // ── 1. Build LLM request ──
        let tools_json = match protocol {
            ToolProtocol::Anthropic => Some(
                tool_protocol::format_tools_anthropic(&tool_defs)
                    .as_array()
                    .cloned()
                    .unwrap_or_default(),
            ),
            ToolProtocol::OpenAi => Some(
                tool_protocol::format_tools_openai(&tool_defs)
                    .as_array()
                    .cloned()
                    .unwrap_or_default(),
            ),
            ToolProtocol::Google => Some(
                tool_protocol::format_tools_google(&tool_defs)
                    .as_array()
                    .cloned()
                    .unwrap_or_default(),
            ),
        };

        let request = LlmRequest {
            messages: messages.clone(),
            model: String::new(), // Router will override
            max_tokens: 4096,
            temperature: Some(0.7),
            stream: false,
            system: Some(system_prompt.clone()),
            tools: tools_json,
        };

        // ── 2. Audit: LlmRequest ──
        let model_name = router
            .get_slot(config.model_slot)
            .map(|s| s.model.clone())
            .unwrap_or_default();
        let fuel_estimate = estimate_llm_fuel(messages);

        governance.audit.record(AuditAction::LlmRequest {
            provider: provider_name.clone(),
            model: model_name.clone(),
            token_count: fuel_estimate,
        });

        // ── 3. Reserve fuel ──
        if let Err(e) = governance.fuel.reserve(fuel_estimate) {
            let _ = event_tx.send(AgentEvent::Done {
                reason: "fuel_exhausted".to_string(),
                total_turns: turn,
            });
            return Err(e);
        }

        // ── 4. Call LLM (streaming with tool detection when available) ──
        let collected = match router.stream_raw(config.model_slot, &request).await {
            Ok(Some(raw_response)) => {
                // Provider supports raw streaming — use appropriate collector
                let (text_tx_inner, mut text_rx_inner) =
                    tokio::sync::mpsc::unbounded_channel::<String>();

                // Forward text deltas to the event channel in real-time
                let event_tx_clone = event_tx.clone();
                let forward_handle = tokio::spawn(async move {
                    while let Some(text) = text_rx_inner.recv().await {
                        let _ = event_tx_clone.send(AgentEvent::TextDelta(text));
                    }
                });

                let result = match protocol {
                    ToolProtocol::Anthropic => {
                        crate::llm::streaming::collect_anthropic_stream(raw_response, text_tx_inner)
                            .await
                    }
                    _ => {
                        // OpenAI-compatible (OpenAI, Ollama, OpenRouter, Groq, DeepSeek)
                        crate::llm::streaming::collect_openai_stream(raw_response, text_tx_inner)
                            .await
                    }
                };

                let _ = forward_handle.await;

                match result {
                    Ok(c) => c,
                    Err(e) => {
                        governance.fuel.release_reservation(fuel_estimate);
                        let _ = event_tx.send(AgentEvent::Error(format!("{}", e)));
                        return Err(e);
                    }
                }
            }
            Ok(None) | Err(_) => {
                // Provider doesn't support raw streaming — fall back to complete()
                let response = match router.complete(config.model_slot, &request).await {
                    Ok(resp) => resp,
                    Err(e) => {
                        governance.fuel.release_reservation(fuel_estimate);
                        let _ = event_tx.send(AgentEvent::Error(format!("{}", e)));
                        return Err(e);
                    }
                };

                // Send text as a single delta
                if !response.content.is_empty() {
                    let _ = event_tx.send(AgentEvent::TextDelta(response.content.clone()));
                }

                // Convert LlmResponse to CollectedResponse
                let mut tool_blocks = Vec::new();
                if let Some(blocks) = response.content_blocks {
                    tool_blocks = blocks;
                } else if let Some(calls) = response.tool_calls {
                    tool_blocks = calls;
                }

                crate::llm::streaming::CollectedResponse {
                    text: response.content,
                    tool_use_blocks: tool_blocks,
                    usage: response.usage,
                    stop_reason: response.stop_reason,
                }
            }
        };

        // ── 5. Audit: LlmResponse ──
        governance.audit.record(AuditAction::LlmResponse {
            provider: provider_name.clone(),
            model: model_name.clone(),
            token_count: collected.usage.total_tokens,
        });

        // ── 6. Consume fuel ──
        let actual_fuel = if collected.usage.total_tokens > 0 {
            collected.usage.total_tokens
        } else {
            fuel_estimate / 2
        };
        governance.record_fuel(
            &provider_name,
            FuelCost {
                input_tokens: collected.usage.input_tokens,
                output_tokens: collected.usage.output_tokens,
                fuel_units: actual_fuel,
                estimated_usd: actual_fuel as f64 * 0.000003,
            },
        );
        if fuel_estimate > actual_fuel {
            governance
                .fuel
                .release_reservation(fuel_estimate - actual_fuel);
        }

        // ── 7. Send token usage event ──
        let _ = event_tx.send(AgentEvent::TokenUsage {
            input_tokens: collected.usage.input_tokens,
            output_tokens: collected.usage.output_tokens,
        });

        // ── 8. Update last_text from collected response ──
        if !collected.text.is_empty() {
            last_text = collected.text.clone();
        }

        // ── 9. Parse tool calls from collected response ──
        let tool_calls = match protocol {
            ToolProtocol::Anthropic => {
                tool_protocol::parse_tool_calls_anthropic(&collected.tool_use_blocks)
            }
            ToolProtocol::OpenAi => {
                tool_protocol::parse_tool_calls_openai(&collected.tool_use_blocks)
            }
            ToolProtocol::Google => {
                tool_protocol::parse_tool_calls_google(&collected.tool_use_blocks)
            }
        };

        // ── 10. If no tool calls and stop reason isn't tool_use, this is the final response ──
        let is_tool_stop = match &collected.stop_reason {
            Some(reason) => matches!(
                (protocol, reason.as_str()),
                (ToolProtocol::Anthropic, "tool_use")
                    | (ToolProtocol::OpenAi, "tool_calls")
                    | (ToolProtocol::Google, "TOOL_CALL")
                    | (ToolProtocol::Google, "tool_use")
            ),
            None => !tool_calls.is_empty(),
        };
        if tool_calls.is_empty() && !is_tool_stop {
            let _ = event_tx.send(AgentEvent::Done {
                reason: "end_turn".to_string(),
                total_turns: turn + 1,
            });
            return Ok(last_text);
        }

        // ── 11. Execute each tool through governance pipeline ──
        let mut tool_results: Vec<ToolResultMessage> = Vec::new();

        for tool_call in &tool_calls {
            let _ = event_tx.send(AgentEvent::ToolCallStart {
                name: tool_call.name.clone(),
                id: tool_call.id.clone(),
            });

            // Create a standalone tool instance (avoids borrow conflicts)
            let tool = match crate::tools::create_tool(&tool_call.name) {
                Some(t) => t,
                None => {
                    // Unknown tool — send error result back to LLM
                    let _ = event_tx.send(AgentEvent::ToolCallDenied {
                        name: tool_call.name.clone(),
                        reason: "Unknown tool".to_string(),
                    });
                    tool_results.push(ToolResultMessage {
                        tool_call_id: tool_call.id.clone(),
                        tool_name: tool_call.name.clone(),
                        content: format!(
                            "Unknown tool '{}'. Available tools: {}",
                            tool_call.name,
                            tool_registry.list().join(", ")
                        ),
                        is_error: true,
                    });
                    continue;
                }
            };

            // Try governed execution
            match crate::tools::execute_governed(
                tool.as_ref(),
                tool_call.input.clone(),
                tool_ctx,
                governance,
            )
            .await
            {
                Ok(result) => {
                    let success = result.is_success();
                    let _ = event_tx.send(AgentEvent::ToolCallComplete {
                        name: tool_call.name.clone(),
                        success,
                        duration_ms: result.duration_ms,
                        summary: result.summary(),
                    });
                    tool_results.push(ToolResultMessage {
                        tool_call_id: tool_call.id.clone(),
                        tool_name: tool_call.name.clone(),
                        content: result.output,
                        is_error: !success,
                    });
                }
                Err(NxError::ConsentRequired { request }) => {
                    // Call the consent handler
                    let granted = consent_handler(&request);

                    if granted {
                        match crate::tools::execute_after_consent(
                            tool.as_ref(),
                            tool_call.input.clone(),
                            tool_ctx,
                            governance,
                            &request,
                            true,
                        )
                        .await
                        {
                            Ok(result) => {
                                let success = result.is_success();
                                let _ = event_tx.send(AgentEvent::ToolCallComplete {
                                    name: tool_call.name.clone(),
                                    success,
                                    duration_ms: result.duration_ms,
                                    summary: result.summary(),
                                });
                                tool_results.push(ToolResultMessage {
                                    tool_call_id: tool_call.id.clone(),
                                    tool_name: tool_call.name.clone(),
                                    content: result.output,
                                    is_error: !success,
                                });
                            }
                            Err(e) => {
                                let _ = event_tx.send(AgentEvent::ToolCallDenied {
                                    name: tool_call.name.clone(),
                                    reason: format!("{}", e),
                                });
                                tool_results.push(ToolResultMessage {
                                    tool_call_id: tool_call.id.clone(),
                                    tool_name: tool_call.name.clone(),
                                    content: format!("Tool execution failed: {}", e),
                                    is_error: true,
                                });
                            }
                        }
                    } else {
                        // User denied consent — finalize the denial
                        let fuel_est = tool.estimated_fuel(&tool_call.input);
                        // finalize_authorization with granted=false records the denial
                        // and returns Err(ConsentDenied), which we expect and handle.
                        let _ = governance.finalize_authorization(&request, false, fuel_est);

                        let _ = event_tx.send(AgentEvent::ToolCallDenied {
                            name: tool_call.name.clone(),
                            reason: "User denied consent".to_string(),
                        });
                        tool_results.push(ToolResultMessage {
                            tool_call_id: tool_call.id.clone(),
                            tool_name: tool_call.name.clone(),
                            content: "Tool denied by user. The user did not grant \
                                      consent for this operation. Please explain \
                                      what you were trying to do and ask how to proceed."
                                .to_string(),
                            is_error: true,
                        });
                    }
                }
                Err(NxError::CapabilityDenied {
                    ref capability,
                    ref reason,
                }) => {
                    let _ = event_tx.send(AgentEvent::ToolCallDenied {
                        name: tool_call.name.clone(),
                        reason: format!("{}: {}", capability, reason),
                    });
                    tool_results.push(ToolResultMessage {
                        tool_call_id: tool_call.id.clone(),
                        tool_name: tool_call.name.clone(),
                        content: format!(
                            "Tool '{}' denied: capability {} not granted. {}",
                            tool_call.name, capability, reason
                        ),
                        is_error: true,
                    });
                }
                Err(NxError::FuelExhausted {
                    remaining,
                    required,
                }) => {
                    let _ = event_tx.send(AgentEvent::Done {
                        reason: "fuel_exhausted".to_string(),
                        total_turns: turn + 1,
                    });
                    return Err(NxError::FuelExhausted {
                        remaining,
                        required,
                    });
                }
                Err(e) => {
                    let _ = event_tx.send(AgentEvent::ToolCallDenied {
                        name: tool_call.name.clone(),
                        reason: format!("{}", e),
                    });
                    tool_results.push(ToolResultMessage {
                        tool_call_id: tool_call.id.clone(),
                        tool_name: tool_call.name.clone(),
                        content: format!("Tool error: {}", e),
                        is_error: true,
                    });
                }
            }
        }

        // ── 12. Append assistant message + tool results to conversation ──
        // Add the assistant's response (including text + tool calls) to history
        messages.push(Message {
            role: Role::Assistant,
            content: collected.text.clone(),
        });

        // Add tool results formatted as a user message
        // (the next LLM call will see these results)
        let tool_result_text = tool_results
            .iter()
            .map(|r| {
                let status = if r.is_error { "ERROR" } else { "OK" };
                format!(
                    "[Tool: {} | Status: {}]\n{}",
                    r.tool_name, status, r.content
                )
            })
            .collect::<Vec<_>>()
            .join("\n\n");

        if !tool_result_text.is_empty() {
            messages.push(Message {
                role: Role::User,
                content: tool_result_text,
            });
        }

        // ── 13. Send turn complete event ──
        let _ = event_tx.send(AgentEvent::TurnComplete {
            turn: turn + 1,
            has_more: true,
        });
    }

    // Max turns reached
    let _ = event_tx.send(AgentEvent::Done {
        reason: "max_turns_reached".to_string(),
        total_turns: config.max_turns,
    });
    Ok(last_text)
}
