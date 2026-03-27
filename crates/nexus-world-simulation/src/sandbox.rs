use std::collections::HashMap;

use crate::engine::SimulationConfig;
use crate::outcome::{SideEffect, StepResult, StepRisk};
use crate::scenario::{Condition, ConditionCheck, SimActionType, SimulatedAction};

/// Simulation sandbox — executes actions in isolation.
/// File operations go to a virtual filesystem.
/// Network calls are blocked (unless explicitly allowed).
/// Terminal commands are dry-run only (risk analysis without execution).
pub struct SimulationSandbox {
    virtual_fs: HashMap<String, String>,
    allow_network: bool,
    max_file_size_bytes: u64,
}

impl SimulationSandbox {
    pub fn new(config: &SimulationConfig) -> Self {
        Self {
            virtual_fs: HashMap::new(),
            allow_network: config.allow_network,
            max_file_size_bytes: config.max_file_size_bytes,
        }
    }

    /// Check a precondition.
    pub fn check_condition(&self, condition: &Condition) -> bool {
        match &condition.check_type {
            ConditionCheck::FileExists(path) => {
                self.virtual_fs.contains_key(path) || std::path::Path::new(path).exists()
            }
            ConditionCheck::FileNotExists(path) => {
                !self.virtual_fs.contains_key(path) && !std::path::Path::new(path).exists()
            }
            ConditionCheck::EnvVarSet(var) => std::env::var(var).is_ok(),
            ConditionCheck::SufficientBudget { .. } => true,
            ConditionCheck::HasCapability(_) => true,
            ConditionCheck::ServiceReachable { .. } => true,
            ConditionCheck::Custom(_) => true,
        }
    }

    /// Simulate an action — no real side effects.
    pub fn simulate_action(&mut self, action: &SimulatedAction) -> StepResult {
        match &action.action_type {
            SimActionType::TerminalCommand {
                command,
                working_dir,
            } => self.simulate_terminal(action.step, command, working_dir.as_deref()),
            SimActionType::FileWrite { path, content } => {
                self.simulate_file_write(action.step, path, content)
            }
            SimActionType::FileDelete { path } => self.simulate_file_delete(action.step, path),
            SimActionType::HttpRequest { method, url, .. } => {
                self.simulate_http(action.step, method, url)
            }
            SimActionType::Deploy { target, artifact } => {
                self.simulate_deploy(action.step, target, artifact)
            }
            SimActionType::AgentMessage {
                target_agent,
                message,
            } => StepResult {
                step: action.step,
                success: true,
                output: format!(
                    "Would send to {}: {}",
                    target_agent,
                    &message[..message.len().min(100)]
                ),
                side_effects: Vec::new(),
                risk: StepRisk::Low,
            },
            SimActionType::LlmCall { model, prompt } => StepResult {
                step: action.step,
                success: true,
                output: format!("Would call {} with {} chars", model, prompt.len()),
                side_effects: Vec::new(),
                risk: StepRisk::Low,
            },
            SimActionType::Custom { action_name, .. } => StepResult {
                step: action.step,
                success: true,
                output: format!(
                    "Custom action '{}' — cannot simulate, assumed success",
                    action_name
                ),
                side_effects: Vec::new(),
                risk: StepRisk::Medium,
            },
        }
    }

    /// Get the virtual filesystem state (for snapshots).
    pub fn virtual_fs(&self) -> &HashMap<String, String> {
        &self.virtual_fs
    }

    /// Restore virtual filesystem from a snapshot.
    pub fn restore_fs(&mut self, fs: HashMap<String, String>) {
        self.virtual_fs = fs;
    }

    fn simulate_terminal(&self, step: u32, command: &str, working_dir: Option<&str>) -> StepResult {
        let parts: Vec<&str> = command.split_whitespace().collect();
        let cmd = parts.first().copied().unwrap_or("");

        let (risk, side_effects) = match cmd {
            "ls" | "cat" | "echo" | "pwd" | "whoami" | "date" | "uname" | "which" | "head"
            | "tail" | "wc" | "grep" | "find" => (StepRisk::Low, Vec::new()),
            "mkdir" | "touch" | "cp" | "mv" => {
                let target = parts.get(1).unwrap_or(&"").to_string();
                (
                    StepRisk::Medium,
                    vec![SideEffect::FileCreated { path: target }],
                )
            }
            "rm" => {
                let target = parts.last().unwrap_or(&"").to_string();
                let risk = if command.contains("-rf") || command.contains("-r") {
                    StepRisk::Critical
                } else {
                    StepRisk::High
                };
                (
                    risk,
                    vec![
                        SideEffect::FileDeleted {
                            path: target.clone(),
                        },
                        SideEffect::DataLoss {
                            description: format!("rm {target}"),
                        },
                    ],
                )
            }
            "git" => {
                let subcmd = parts.get(1).copied().unwrap_or("");
                match subcmd {
                    "status" | "log" | "diff" | "branch" => (StepRisk::Low, Vec::new()),
                    "push" | "commit" | "merge" => (
                        StepRisk::Medium,
                        vec![SideEffect::StateChange {
                            component: "git".into(),
                            from: "current".into(),
                            to: "modified".into(),
                        }],
                    ),
                    _ => (StepRisk::Medium, Vec::new()),
                }
            }
            "docker" | "kubectl" | "systemctl" => (
                StepRisk::High,
                vec![SideEffect::ServiceDisruption {
                    service: cmd.into(),
                    duration_estimate: "unknown".into(),
                }],
            ),
            "sudo" | "shutdown" | "reboot" | "kill" | "killall" => (
                StepRisk::Critical,
                vec![SideEffect::ServiceDisruption {
                    service: "system".into(),
                    duration_estimate: "indefinite".into(),
                }],
            ),
            _ => (StepRisk::Medium, Vec::new()),
        };

        StepResult {
            step,
            success: true,
            output: format!("Dry-run: {} (in {:?})", command, working_dir),
            side_effects,
            risk,
        }
    }

    fn simulate_file_write(&mut self, step: u32, path: &str, content: &str) -> StepResult {
        if content.len() as u64 > self.max_file_size_bytes {
            return StepResult {
                step,
                success: false,
                output: format!(
                    "File too large: {} bytes (max {})",
                    content.len(),
                    self.max_file_size_bytes
                ),
                side_effects: Vec::new(),
                risk: StepRisk::Low,
            };
        }

        let existed = self.virtual_fs.contains_key(path) || std::path::Path::new(path).exists();
        self.virtual_fs.insert(path.into(), content.into());

        let side_effect = if existed {
            SideEffect::FileModified { path: path.into() }
        } else {
            SideEffect::FileCreated { path: path.into() }
        };

        StepResult {
            step,
            success: true,
            output: format!("Would write {} bytes to {}", content.len(), path),
            side_effects: vec![side_effect],
            risk: if existed {
                StepRisk::Medium
            } else {
                StepRisk::Low
            },
        }
    }

    fn simulate_file_delete(&mut self, step: u32, path: &str) -> StepResult {
        let exists = self.virtual_fs.contains_key(path) || std::path::Path::new(path).exists();
        self.virtual_fs.remove(path);

        if exists {
            StepResult {
                step,
                success: true,
                output: format!("Would delete {}", path),
                side_effects: vec![
                    SideEffect::FileDeleted { path: path.into() },
                    SideEffect::DataLoss {
                        description: format!("Delete {}", path),
                    },
                ],
                risk: StepRisk::High,
            }
        } else {
            StepResult {
                step,
                success: false,
                output: format!("File not found: {}", path),
                side_effects: Vec::new(),
                risk: StepRisk::Low,
            }
        }
    }

    fn simulate_http(&self, step: u32, method: &str, url: &str) -> StepResult {
        let risk = match method {
            "GET" | "HEAD" | "OPTIONS" => StepRisk::Low,
            "POST" | "PUT" | "PATCH" => StepRisk::Medium,
            "DELETE" => StepRisk::High,
            _ => StepRisk::Medium,
        };

        StepResult {
            step,
            success: true,
            output: if self.allow_network {
                format!("Would {} {}", method, url)
            } else {
                format!("Would {} {} (network blocked in simulation)", method, url)
            },
            side_effects: vec![SideEffect::NetworkCall { url: url.into() }],
            risk,
        }
    }

    fn simulate_deploy(&self, step: u32, target: &str, artifact: &str) -> StepResult {
        StepResult {
            step,
            success: true,
            output: format!("Would deploy {} to {}", artifact, target),
            side_effects: vec![
                SideEffect::ServiceDisruption {
                    service: target.into(),
                    duration_estimate: "deployment window".into(),
                },
                SideEffect::StateChange {
                    component: target.into(),
                    from: "current".into(),
                    to: artifact.into(),
                },
            ],
            risk: StepRisk::High,
        }
    }
}
