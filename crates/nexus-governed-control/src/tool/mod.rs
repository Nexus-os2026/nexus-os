//! P3-A: governed tools.
//!
//! There is no free-form shell, interpreter or command line here. A tool is
//! code: a key, one executable at a canonical absolute path, an effect
//! class, a builder that turns a typed input into an argument vector and
//! the input files it needs (rejecting anything else), a static environment
//! allowlist, a deadline and an output bound. The owner grants a tool after
//! seeing its executable; the grant pins the executable's identity
//! ([`crate::executable`]), which is checked again when an action is
//! prepared and immediately before the launch. The launch goes through the
//! kernel's sealed spawn: absolute program, cleared environment (only a
//! private home and temporary directory and the tool's allowlist), a fresh
//! private working directory, its own process group with resource limits,
//! no standard input. The run's cancellation and the deadline end the whole
//! group; every exit reaps it; the working directory is removed.

pub mod catalog;

use crate::authority::commitment::{ExecutionGuard, FailureClass, PreparedAction, TargetIdentity};
use crate::authority::effect::{CapabilityKind, EffectClass};
use crate::authority::evidence::escaped;
use crate::authority::ids::{Digest, GrantId};
use crate::authority::policy::GrantScope;
use crate::authority::{Authority, AuthorityError};
use crate::control::{EffectOutput, PendingEffect, Preparation};
use crate::executable::{inspect, Trust};
use crate::runtime_root::RuntimeRoot;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// What a tool's builder produced from its typed input.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolInvocation {
    pub args: Vec<String>,
    /// Files written into the working directory before the launch, by
    /// names the definition fixes.
    pub files: Vec<(&'static str, Vec<u8>)>,
    pub summary: Vec<String>,
}

/// What a tool returns.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolOutput {
    Text,
    Bytes,
}

/// One tool.
pub struct ToolDefinition {
    pub key: &'static str,
    pub executable: &'static str,
    pub class: EffectClass,
    pub build: fn(&Value) -> Result<ToolInvocation, AuthorityError>,
    pub env: &'static [(&'static str, &'static str)],
    pub timeout: Duration,
    pub max_output: usize,
    pub output: ToolOutput,
    pub(crate) trust: Trust,
}

/// A tool request, as data.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ToolIntent {
    pub tool: String,
    #[serde(default)]
    pub input: Value,
}

const MAX_ARGS: usize = 64;
const MAX_ARG: usize = 4096;
const MAX_FILES_BYTES: usize = 4 * 1024 * 1024;
const MAX_STDERR: usize = 64 * 1024;
/// How long a prepared tool run may wait for authorization.
const TOOL_TTL: Duration = Duration::from_secs(10 * 60);

/// The tool actuator.
pub struct Tools {
    definitions: Vec<ToolDefinition>,
    root: RuntimeRoot,
}

impl Tools {
    pub(crate) fn new(definitions: Vec<ToolDefinition>, root: RuntimeRoot) -> Self {
        Self { definitions, root }
    }

    /// The production catalog.
    pub fn production(root: RuntimeRoot) -> Self {
        Self::new(catalog::production(), root)
    }

    /// Every tool key with its class and executable (display).
    pub fn tools(&self) -> Vec<(&'static str, EffectClass, &'static str)> {
        self.definitions
            .iter()
            .map(|d| (d.key, d.class, d.executable))
            .collect()
    }

    fn find(&self, key: &str) -> Result<&ToolDefinition, AuthorityError> {
        self.definitions
            .iter()
            .find(|d| d.key == key)
            .ok_or(AuthorityError::Closed("no such tool"))
    }

    /// Pin the tool's executable as it is now, for the owner to grant.
    pub fn grant_scope(&self, key: &str) -> Result<GrantScope, AuthorityError> {
        let definition = self.find(key)?;
        let identity = inspect(Path::new(definition.executable), definition.trust)?;
        Ok(GrantScope::Tool {
            tool: definition.key.to_string(),
            executable: identity.path.display().to_string(),
            identity: identity.digest,
        })
    }

    /// Prepare a tool run.
    pub fn prepare(
        &self,
        authority: &Authority,
        intent: &ToolIntent,
    ) -> Result<Preparation, AuthorityError> {
        let definition = self.find(&intent.tool)?;
        let invocation = (definition.build)(&intent.input)?;
        if invocation.args.len() > MAX_ARGS
            || invocation
                .args
                .iter()
                .any(|arg| arg.len() > MAX_ARG || arg.contains('\0'))
        {
            return Err(AuthorityError::InvalidAction(
                "tool arguments out of bounds",
            ));
        }
        if invocation
            .files
            .iter()
            .map(|(_, bytes)| bytes.len())
            .sum::<usize>()
            > MAX_FILES_BYTES
        {
            return Err(AuthorityError::InvalidAction("tool input too large"));
        }
        let identity = inspect(Path::new(definition.executable), definition.trust)?;
        let grant = covering_grant(authority, definition.key, &identity.digest)?;
        let target = Digest::of(
            "nexus.p3.tool.target.v1",
            &[definition.key.as_bytes(), identity.digest.as_bytes()],
        );
        let mut parts: Vec<Vec<u8>> = vec![definition.key.as_bytes().to_vec()];
        parts.extend(invocation.args.iter().map(|a| a.as_bytes().to_vec()));
        for (name, bytes) in &invocation.files {
            parts.push(name.as_bytes().to_vec());
            parts.push(
                Digest::of("nexus.p3.tool.file.v1", &[bytes])
                    .as_bytes()
                    .to_vec(),
            );
        }
        let refs: Vec<&[u8]> = parts.iter().map(Vec::as_slice).collect();
        let parameters = Digest::of("nexus.p3.tool.invocation.v1", &refs);
        let mut summary = vec![escaped(&format!(
            "Run the tool {} ({})",
            definition.key, definition.executable
        ))];
        summary.extend(invocation.summary.iter().map(|line| escaped(line)));
        let action = PreparedAction {
            kind: CapabilityKind::Tool,
            class: definition.class,
            operation: definition.key,
            target: TargetIdentity {
                display: escaped(&format!("{} at {}", definition.key, definition.executable)),
                digest: target,
            },
            parameters,
            grants: vec![grant],
            leases: vec![],
            summary,
        };
        let effect = ToolEffect {
            key: definition.key,
            executable: identity.path,
            identity: identity.digest,
            trust: definition.trust,
            args: invocation.args,
            files: invocation.files,
            env: definition.env,
            timeout: definition.timeout,
            max_output: definition.max_output,
            output: definition.output,
            root: self.root.clone(),
            target,
            parameters,
        };
        Ok(Preparation {
            action,
            effect: Box::new(effect),
            ttl: TOOL_TTL,
        })
    }
}

/// The live grant for this tool whose pinned identity is the executable's
/// identity now.
fn covering_grant(
    authority: &Authority,
    key: &str,
    identity: &Digest,
) -> Result<GrantId, AuthorityError> {
    let grants = authority.grants().live_of(CapabilityKind::Tool);
    let mut named = false;
    for grant in grants {
        if let GrantScope::Tool {
            tool,
            identity: pinned,
            ..
        } = &grant.scope
        {
            if tool == key {
                named = true;
                if pinned == identity {
                    return Ok(grant.id);
                }
            }
        }
    }
    Err(if named {
        AuthorityError::Closed("the tool changed since it was granted; grant it again")
    } else {
        AuthorityError::NoCoveringGrant
    })
}

struct ToolEffect {
    key: &'static str,
    executable: PathBuf,
    identity: Digest,
    trust: Trust,
    args: Vec<String>,
    files: Vec<(&'static str, Vec<u8>)>,
    env: &'static [(&'static str, &'static str)],
    timeout: Duration,
    max_output: usize,
    output: ToolOutput,
    root: RuntimeRoot,
    target: Digest,
    parameters: Digest,
}

impl PendingEffect for ToolEffect {
    fn revalidate(&self) -> Result<Digest, AuthorityError> {
        let now =
            inspect(&self.executable, self.trust).map_err(|_| AuthorityError::TargetChanged)?;
        if now.digest != self.identity {
            return Err(AuthorityError::TargetChanged);
        }
        Ok(self.target)
    }

    fn parameters(&self) -> Digest {
        self.parameters
    }

    fn execute(
        self: Box<Self>,
        guard: &ExecutionGuard,
    ) -> Result<EffectOutput, (FailureClass, String)> {
        launch::run(&self, guard)
    }
}

/// A reader that keeps at most `cap` bytes and reports whether more came.
struct Capped {
    handle: std::thread::JoinHandle<(Vec<u8>, bool)>,
    overflow: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl Capped {
    fn spawn(reader: Option<Box<dyn std::io::Read + Send>>, cap: usize) -> Self {
        use std::sync::atomic::{AtomicBool, Ordering};
        let overflow = std::sync::Arc::new(AtomicBool::new(false));
        let flag = overflow.clone();
        let handle = std::thread::spawn(move || {
            let mut kept = Vec::new();
            let Some(mut reader) = reader else {
                return (kept, false);
            };
            let mut chunk = [0u8; 8192];
            loop {
                match reader.read(&mut chunk) {
                    Ok(0) | Err(_) => return (kept, false),
                    Ok(n) if kept.len() + n > cap => {
                        flag.store(true, Ordering::SeqCst);
                        // Stop reading: the writer meets a closed pipe.
                        return (kept, true);
                    }
                    Ok(n) => kept.extend_from_slice(&chunk[..n]),
                }
            }
        });
        Self { handle, overflow }
    }

    fn overflowed(&self) -> bool {
        self.overflow.load(std::sync::atomic::Ordering::SeqCst)
    }

    fn join(self) -> (Vec<u8>, bool) {
        self.handle.join().unwrap_or((Vec::new(), true))
    }
}

#[cfg(target_os = "linux")]
mod launch {
    use super::{Capped, ToolEffect, ToolOutput, MAX_STDERR};
    use crate::authority::commitment::{ExecutionGuard, FailureClass};
    use crate::control::EffectOutput;
    use nexus_kernel::resource_limiter::{
        ResourceLimiter, ResourceLimits, ResourceOutput, SealedEnvironment, SealedSpawnSpec,
    };
    use std::ffi::OsString;
    use std::time::{Duration, Instant};

    fn unavailable(what: &str) -> (FailureClass, String) {
        (FailureClass::Unavailable, what.to_string())
    }

    pub(super) fn run(
        effect: &ToolEffect,
        guard: &ExecutionGuard,
    ) -> Result<EffectOutput, (FailureClass, String)> {
        let scratch = effect
            .root
            .scratch("tool")
            .map_err(|_| unavailable("no working directory"))?;
        let home = scratch
            .subdir("home")
            .map_err(|_| unavailable("no home directory"))?;
        let temp = scratch
            .subdir("tmp")
            .map_err(|_| unavailable("no temporary directory"))?;
        for (name, bytes) in &effect.files {
            scratch
                .write_new(name, bytes)
                .map_err(|_| unavailable("an input file cannot be written"))?;
        }
        let mut environment = SealedEnvironment::builder()
            .home_dir(&home)
            .and_then(|b| b.temp_dir(&temp))
            .map_err(|_| unavailable("the sealed environment was refused"))?;
        for (name, value) in effect.env {
            environment = environment
                .set(name, *value)
                .map_err(|_| unavailable("the sealed environment was refused"))?;
        }
        let environment = environment
            .build()
            .map_err(|_| unavailable("the sealed environment was refused"))?;
        let spec = SealedSpawnSpec {
            program: effect.executable.clone(),
            args: effect.args.iter().map(OsString::from).collect(),
            current_dir: scratch.path().to_path_buf(),
            environment,
            stdout: ResourceOutput::Piped,
            stderr: ResourceOutput::Piped,
        };
        let limiter = ResourceLimiter::new(ResourceLimits::default());
        let mut child = limiter
            .spawn_sealed(&spec)
            .map_err(|_| unavailable("the tool could not start"))?;
        let stdout = Capped::spawn(child.take_stdout(), effect.max_output);
        let stderr = Capped::spawn(child.take_stderr(), MAX_STDERR);
        let deadline = Instant::now() + effect.timeout;
        let outcome = loop {
            match child.poll_exit() {
                Ok(Some(status)) => break Ok(status),
                Ok(None) => {}
                Err(_) => break Err((FailureClass::Unavailable, "exit observation failed".into())),
            }
            if guard.is_cancelled() {
                break Err((FailureClass::Actuator, "cancelled".into()));
            }
            if Instant::now() >= deadline {
                break Err((FailureClass::Timeout, "the tool ran out of time".into()));
            }
            if stdout.overflowed() || stderr.overflowed() {
                break Err((
                    FailureClass::Bounds,
                    "the tool's output was too large".into(),
                ));
            }
            std::thread::sleep(Duration::from_millis(10));
        };
        // Whatever happened, the whole process group ends and is reaped.
        let reaped = child.terminate_and_reap(Instant::now() + Duration::from_secs(5));
        let (out, out_overflow) = stdout.join();
        let (err, _) = stderr.join();
        let status = outcome?;
        if reaped.is_err() {
            return Err((
                FailureClass::Unavailable,
                "the tool's processes could not be reaped".into(),
            ));
        }
        if out_overflow {
            return Err((
                FailureClass::Bounds,
                "the tool's output was too large".into(),
            ));
        }
        if !status.success() {
            return Err((
                FailureClass::Actuator,
                format!(
                    "{} exited with status {}",
                    effect.key,
                    status
                        .code()
                        .map_or("signal".to_string(), |c| c.to_string())
                ),
            ));
        }
        let meta = vec![
            ("tool".to_string(), effect.key.to_string()),
            ("stdout_bytes".to_string(), out.len().to_string()),
            ("stderr_bytes".to_string(), err.len().to_string()),
            ("exit".to_string(), "0".to_string()),
        ];
        Ok(match effect.output {
            ToolOutput::Text => EffectOutput {
                text: Some(String::from_utf8_lossy(&out).into_owned()),
                bytes: None,
                meta,
            },
            ToolOutput::Bytes => EffectOutput {
                text: None,
                bytes: Some(out),
                meta,
            },
        })
    }
}

#[cfg(not(target_os = "linux"))]
mod launch {
    use super::ToolEffect;
    use crate::authority::commitment::{ExecutionGuard, FailureClass};
    use crate::control::EffectOutput;

    pub(super) fn run(
        _effect: &ToolEffect,
        _guard: &ExecutionGuard,
    ) -> Result<EffectOutput, (FailureClass, String)> {
        Err((
            FailureClass::Unavailable,
            "governed tools are available on Linux only".into(),
        ))
    }
}

#[cfg(test)]
mod tests;
