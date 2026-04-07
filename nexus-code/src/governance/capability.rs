use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::error::NxError;

/// The 13 permission types controlling what the agent can do.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Capability {
    /// Read files from the filesystem.
    FileRead,
    /// Write files to the filesystem.
    FileWrite,
    /// Delete files from the filesystem.
    FileDelete,
    /// Execute shell commands.
    ShellExecute,
    /// Execute dangerous shell commands (rm -rf, sudo, etc.).
    ShellExecuteDangerous,
    /// Access the network.
    NetworkAccess,
    /// Read from git repositories.
    GitRead,
    /// Write to git repositories (commit, push).
    GitWrite,
    /// Spawn new processes.
    ProcessSpawn,
    /// Read environment variables.
    EnvRead,
    /// Write/modify environment variables.
    EnvWrite,
    /// Make LLM API calls.
    LlmCall,
    /// Computer use — screen capture, interaction, and vision analysis.
    ComputerUse,
}

impl Capability {
    /// Map a tool name to the required capability.
    pub fn for_tool(tool_name: &str) -> Option<Self> {
        match tool_name {
            "file_read" => Some(Self::FileRead),
            "file_write" | "file_edit" => Some(Self::FileWrite),
            "file_delete" => Some(Self::FileDelete),
            "bash" | "shell" => Some(Self::ShellExecute),
            "search" | "glob" => Some(Self::FileRead),
            "git_status" | "git_log" | "git_diff" => Some(Self::GitRead),
            "git_commit" | "git_push" => Some(Self::GitWrite),
            "lsp_query" => Some(Self::FileRead),
            "llm_call" => Some(Self::LlmCall),
            "screen_capture" | "screen_interact" | "screen_analyze" => Some(Self::ComputerUse),
            _ => None,
        }
    }

    /// Human-readable name for display.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::FileRead => "file.read",
            Self::FileWrite => "file.write",
            Self::FileDelete => "file.delete",
            Self::ShellExecute => "shell.execute",
            Self::ShellExecuteDangerous => "shell.execute.dangerous",
            Self::NetworkAccess => "network.access",
            Self::GitRead => "git.read",
            Self::GitWrite => "git.write",
            Self::ProcessSpawn => "process.spawn",
            Self::EnvRead => "env.read",
            Self::EnvWrite => "env.write",
            Self::LlmCall => "llm.call",
            Self::ComputerUse => "computer.use",
        }
    }
}

impl std::fmt::Display for Capability {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// The scope of a capability grant.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CapabilityScope {
    /// Unrestricted within this capability type.
    Full,
    /// Restricted to paths matching these glob patterns.
    PathScoped(Vec<String>),
    /// Restricted to specific commands.
    CommandScoped(Vec<String>),
}

/// A granted capability with its scope and timestamp.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityGrant {
    /// The capability that was granted.
    pub capability: Capability,
    /// The scope of the grant.
    pub scope: CapabilityScope,
    /// When this grant was created.
    pub granted_at: DateTime<Utc>,
}

/// Manages capability grants and access control checks.
pub struct CapabilityManager {
    grants: HashMap<Capability, CapabilityGrant>,
    denied_log: Vec<(Capability, String, DateTime<Utc>)>,
}

impl CapabilityManager {
    /// Create with no capabilities granted.
    pub fn empty() -> Self {
        Self {
            grants: HashMap::new(),
            denied_log: Vec::new(),
        }
    }

    /// Create with default grants: FileRead, GitRead, EnvRead, LlmCall — all Full scope.
    pub fn with_defaults() -> Self {
        let mut grants = HashMap::new();
        let now = Utc::now();
        for cap in [
            Capability::FileRead,
            Capability::GitRead,
            Capability::EnvRead,
            Capability::LlmCall,
        ] {
            grants.insert(
                cap,
                CapabilityGrant {
                    capability: cap,
                    scope: CapabilityScope::Full,
                    granted_at: now,
                },
            );
        }
        Self {
            grants,
            denied_log: Vec::new(),
        }
    }

    /// Grant a capability with a specific scope.
    pub fn grant(&mut self, capability: Capability, scope: CapabilityScope) {
        self.grants.insert(
            capability,
            CapabilityGrant {
                capability,
                scope,
                granted_at: Utc::now(),
            },
        );
    }

    /// Revoke a capability.
    pub fn revoke(&mut self, capability: Capability) {
        self.grants.remove(&capability);
    }

    /// Check if a capability is granted.
    ///
    /// For PathScoped, `context` is the file path being accessed.
    /// For CommandScoped, `context` is the command being executed.
    pub fn check(&mut self, capability: Capability, context: &str) -> Result<(), NxError> {
        match self.grants.get(&capability) {
            None => {
                self.denied_log
                    .push((capability, context.to_string(), Utc::now()));
                Err(NxError::CapabilityDenied {
                    capability: capability.as_str().to_string(),
                    reason: format!("Capability {} not granted", capability.as_str()),
                })
            }
            Some(grant) => match &grant.scope {
                CapabilityScope::Full => Ok(()),
                CapabilityScope::PathScoped(patterns) => {
                    if Self::check_path_scope(patterns, context) {
                        Ok(())
                    } else {
                        self.denied_log
                            .push((capability, context.to_string(), Utc::now()));
                        Err(NxError::CapabilityDenied {
                            capability: capability.as_str().to_string(),
                            reason: format!("Path '{}' not in allowed scope", context),
                        })
                    }
                }
                CapabilityScope::CommandScoped(commands) => {
                    if commands.iter().any(|c| context.starts_with(c)) {
                        Ok(())
                    } else {
                        self.denied_log
                            .push((capability, context.to_string(), Utc::now()));
                        Err(NxError::CapabilityDenied {
                            capability: capability.as_str().to_string(),
                            reason: format!("Command '{}' not in allowed commands", context),
                        })
                    }
                }
            },
        }
    }

    /// Check path against glob patterns using `glob_match`.
    fn check_path_scope(patterns: &[String], path: &str) -> bool {
        patterns
            .iter()
            .any(|pattern| glob_match::glob_match(pattern, path))
    }

    /// Get all currently granted capabilities.
    pub fn granted(&self) -> Vec<&CapabilityGrant> {
        self.grants.values().collect()
    }

    /// Get the denial log.
    pub fn denial_log(&self) -> &[(Capability, String, DateTime<Utc>)] {
        &self.denied_log
    }
}
