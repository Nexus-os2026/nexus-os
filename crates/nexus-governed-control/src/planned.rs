//! The final classification of every agent `PlannedAction`.
//!
//! The match is exhaustive and has no wildcard: a new `PlannedAction`
//! variant does not compile until it is classified here. Inert variants
//! stay on their existing routes; governed variants become a typed
//! [`Intent`] for the Phase Three pipeline (data, prepared and committed
//! like any other); `ComputerAction` is an orchestrator whose every step is
//! its own commitment; everything else stays closed.

use crate::browser::{BrowserIntent, BrowserStep};
use crate::display::{Button, InputIntent, PerceptionIntent, Rect, ScrollDirection};
use crate::egress::EgressIntent;
use crate::governed::Intent;
use crate::tool::ToolIntent;
use nexus_kernel::cognitive::types::BrowserAction;
use nexus_kernel::cognitive::PlannedAction;
use serde_json::json;

/// What happens to a planned action.
#[derive(Clone, Debug, PartialEq)]
pub enum Disposition {
    /// No real-world effect: it keeps its existing route.
    Inert,
    /// A governed effect: prepared, committed, authorized, executed.
    Governed(Intent),
    /// A goal for the agent's own planning: each step it takes on the
    /// agent display is a separate governed action (observe, then input).
    Orchestrated { max_steps: u32 },
    /// Not available: the reason is bounded and names no input.
    Closed(&'static str),
}

fn closed(reason: &'static str) -> Disposition {
    Disposition::Closed(reason)
}

fn coordinate(value: u32) -> Option<u16> {
    u16::try_from(value).ok()
}

fn button(name: &str) -> Option<Button> {
    match name.to_ascii_lowercase().as_str() {
        "" | "left" => Some(Button::Left),
        "middle" => Some(Button::Middle),
        "right" => Some(Button::Right),
        _ => None,
    }
}

fn scroll(direction: &str) -> Option<ScrollDirection> {
    match direction.to_ascii_lowercase().as_str() {
        "up" => Some(ScrollDirection::Up),
        "down" => Some(ScrollDirection::Down),
        "left" => Some(ScrollDirection::Left),
        "right" => Some(ScrollDirection::Right),
        _ => None,
    }
}

fn step(action: &BrowserAction) -> BrowserStep {
    match action {
        BrowserAction::Navigate { url } => BrowserStep::Navigate { url: url.clone() },
        BrowserAction::Click { selector } => BrowserStep::Click {
            selector: selector.clone(),
        },
        BrowserAction::Fill { selector, text } => BrowserStep::Fill {
            selector: selector.clone(),
            text: text.clone(),
        },
        BrowserAction::Press { selector, key } => BrowserStep::Press {
            selector: selector.clone(),
            key: key.clone(),
        },
        BrowserAction::WaitFor {
            selector,
            timeout_ms,
        } => BrowserStep::WaitFor {
            selector: selector.clone(),
            timeout_ms: *timeout_ms,
        },
        BrowserAction::ExtractText { selector } => BrowserStep::ExtractText {
            selector: selector.clone(),
        },
    }
}

fn input(intent: Option<InputIntent>) -> Disposition {
    match intent {
        Some(intent) => Disposition::Governed(Intent::Input(intent)),
        None => closed("the input is outside the agent display"),
    }
}

/// Classify one planned action.
pub fn classify(action: &PlannedAction) -> Disposition {
    match action {
        // Inert: no real-world effect beyond existing governed routes.
        PlannedAction::LlmQuery { .. }
        | PlannedAction::Noop
        | PlannedAction::MemoryStore { .. }
        | PlannedAction::MemoryRecall { .. }
        | PlannedAction::SendNotification { .. }
        | PlannedAction::AgentMessage { .. }
        | PlannedAction::HitlRequest { .. }
        | PlannedAction::WebSearch { .. }
        | PlannedAction::KnowledgeGraphUpdate { .. }
        | PlannedAction::KnowledgeGraphQuery { .. } => Disposition::Inert,

        // P3-B egress.
        PlannedAction::WebFetch { url } => Disposition::Governed(Intent::Request(EgressIntent {
            method: "GET".into(),
            url: url.clone(),
            headers: vec![],
            body: None,
        })),
        PlannedAction::ApiCall {
            method,
            url,
            body,
            headers,
        } => {
            let mut headers: Vec<(String, String)> = headers
                .iter()
                .flatten()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();
            headers.sort();
            Disposition::Governed(Intent::Request(EgressIntent {
                method: method.trim().to_ascii_uppercase(),
                url: url.clone(),
                headers,
                body: body.clone(),
            }))
        }

        // P3-C browser.
        PlannedAction::BrowserAutomate {
            start_url,
            actions,
            screenshot_dir,
        } => {
            if screenshot_dir
                .as_deref()
                .is_some_and(|dir| !dir.trim().is_empty())
            {
                return closed(
                    "browser screenshots to files stay closed; observe the agent display instead",
                );
            }
            Disposition::Governed(Intent::Browse(BrowserIntent {
                start_url: start_url.clone(),
                steps: actions.iter().map(step).collect(),
            }))
        }

        // P3-D perception.
        PlannedAction::CaptureScreen { region } => match region {
            None => {
                Disposition::Governed(Intent::Observe(PerceptionIntent::Screen { region: None }))
            }
            Some(region) => match (
                coordinate(region.x),
                coordinate(region.y),
                coordinate(region.width),
                coordinate(region.height),
            ) {
                (Some(x), Some(y), Some(width), Some(height)) => {
                    Disposition::Governed(Intent::Observe(PerceptionIntent::Screen {
                        region: Some(Rect {
                            x,
                            y,
                            width,
                            height,
                        }),
                    }))
                }
                _ => closed("the region is outside the agent display"),
            },
        },
        PlannedAction::CaptureWindow { window_title } => {
            Disposition::Governed(Intent::Observe(PerceptionIntent::Window {
                title: window_title.clone(),
            }))
        }
        // The observation is governed here; its interpretation is the
        // agent's own model reading the returned image as data.
        PlannedAction::AnalyzeScreen { .. } => {
            Disposition::Governed(Intent::Observe(PerceptionIntent::Screen { region: None }))
        }

        // P3-E input.
        PlannedAction::MouseMove { x, y } => input(
            coordinate(*x)
                .zip(coordinate(*y))
                .map(|(x, y)| InputIntent::Move { x, y }),
        ),
        PlannedAction::MouseClick { x, y, button: name } => match button(name) {
            Some(button) => {
                input(
                    coordinate(*x)
                        .zip(coordinate(*y))
                        .map(|(x, y)| InputIntent::Click {
                            x,
                            y,
                            button: Some(button),
                        }),
                )
            }
            None => closed("unknown mouse button"),
        },
        PlannedAction::MouseDoubleClick { x, y } => input(
            coordinate(*x)
                .zip(coordinate(*y))
                .map(|(x, y)| InputIntent::DoubleClick { x, y }),
        ),
        PlannedAction::MouseDrag {
            from_x,
            from_y,
            to_x,
            to_y,
        } => input(
            match (
                coordinate(*from_x),
                coordinate(*from_y),
                coordinate(*to_x),
                coordinate(*to_y),
            ) {
                (Some(from_x), Some(from_y), Some(to_x), Some(to_y)) => Some(InputIntent::Drag {
                    from_x,
                    from_y,
                    to_x,
                    to_y,
                }),
                _ => None,
            },
        ),
        PlannedAction::KeyboardType { text } => {
            input(Some(InputIntent::Type { text: text.clone() }))
        }
        PlannedAction::KeyboardPress { key } => {
            input(Some(InputIntent::Press { key: key.clone() }))
        }
        PlannedAction::KeyboardShortcut { keys } => {
            input(Some(InputIntent::Shortcut { keys: keys.clone() }))
        }
        PlannedAction::ScrollWheel { direction, amount } => {
            match (scroll(direction), u8::try_from(*amount)) {
                (Some(direction), Ok(amount)) => {
                    input(Some(InputIntent::Scroll { direction, amount }))
                }
                _ => closed("unknown scroll"),
            }
        }
        PlannedAction::ComputerAction { max_steps, .. } => Disposition::Orchestrated {
            max_steps: (*max_steps).clamp(1, crate::display::MAX_INPUT_STEPS),
        },

        // P3-A: speech is the local engine, returned as audio data.
        PlannedAction::TextToSpeech {
            text,
            output_path,
            provider,
            voice,
            ..
        } => {
            if !output_path.trim().is_empty() {
                return closed("speech is returned as audio data; writing files stays closed");
            }
            if provider
                .as_deref()
                .is_some_and(|p| !matches!(p.trim(), "" | "local" | "espeak" | "espeak-ng"))
            {
                return closed("only the local speech engine is governed");
            }
            let mut input = json!({ "text": text });
            if let Some(voice) = voice.as_deref().filter(|v| !v.trim().is_empty()) {
                input["voice"] = json!(voice);
            }
            Disposition::Governed(Intent::Tool(ToolIntent {
                tool: "speech.synthesize".into(),
                input,
            }))
        }

        // Still closed in Phase Three.
        PlannedAction::FileRead { .. } => closed("agent file reads stay closed"),
        PlannedAction::FileWrite { .. } => closed("agent file writes stay closed"),
        PlannedAction::ShellCommand { .. } => closed("shell commands stay closed"),
        PlannedAction::DockerCommand { .. } => closed("container commands stay closed"),
        PlannedAction::CodeExecute { .. } => closed("code execution stays closed"),
        PlannedAction::ImageGenerate { .. } => closed("image generation stays closed"),
        PlannedAction::A2aDelegation { .. } => closed("agent-to-agent delegation stays closed"),
        PlannedAction::SelfModifyDescription { .. } => closed("self-modification stays closed"),
        PlannedAction::SelfModifyStrategy { .. } => closed("self-modification stays closed"),
        PlannedAction::CreateSubAgent { .. } => closed("creating agents stays closed"),
        PlannedAction::DestroySubAgent { .. } => closed("destroying agents stays closed"),
        PlannedAction::RunEvolutionTournament { .. } => closed("evolution stays closed"),
        PlannedAction::ModifyGovernancePolicy { .. } => closed("agents never change governance"),
        PlannedAction::AllocateEcosystemFuel { .. } => closed("fuel allocation stays closed"),
        PlannedAction::ModifyCognitiveParams { .. } => closed("self-modification stays closed"),
        PlannedAction::SelectLlmProvider { .. } => closed("provider selection stays closed"),
        PlannedAction::SelectAlgorithm { .. } => closed("self-modification stays closed"),
        PlannedAction::DesignAgentEcosystem { .. } => closed("creating agents stays closed"),
        PlannedAction::RunCounterfactual { .. } => closed("counterfactual runs stay closed"),
        PlannedAction::TemporalPlan { .. } => closed("long-horizon planning stays closed"),
    }
}
