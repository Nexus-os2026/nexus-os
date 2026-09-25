//! Owned OS containment for governed subprocesses.
//!
//! Linux/macOS create a process group before exec; Linux additionally applies
//! four hard rlimits. Windows assigns a private kill-on-close Job Object during
//! process creation. Always explicitly finalize, even after natural root exit.
//! Unix groups contain descendants that remain in the group; this is not a
//! sandbox against a workload deliberately calling setsid/setpgid.
//!
//! P0-002C4C2 adds a separate, opt-in sealed long-lived spawn: the child starts
//! from an empty environment (never the parent's), runs an explicit absolute
//! executable from a canonical directory, and receives one fixed kernel-owned
//! long-lived resource policy. It is not a filesystem, network or credential
//! sandbox. Legacy `spawn`/`spawn_actuator` behaviour is unchanged.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::ffi::OsString;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::ExitStatus;
#[cfg(any(target_os = "linux", target_os = "macos", windows))]
use std::time::Duration;
use std::time::Instant;

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[path = "resource_limiter/unix.rs"]
mod platform;
#[cfg(windows)]
#[path = "resource_limiter/windows.rs"]
mod platform;

/// Linux hard resource limits; other supported platforms provide tree containment.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ResourceLimits {
    /// Maximum virtual memory in bytes (RLIMIT_AS). Default: 512 MB.
    pub max_memory_bytes: u64,
    /// Maximum CPU time in seconds (RLIMIT_CPU). Default: 60.
    pub max_cpu_seconds: u64,
    /// Maximum number of processes for the user (RLIMIT_NPROC). Default: 4096.
    ///
    /// Note: RLIMIT_NPROC counts **all** processes for the UID, not just
    /// descendants of the child.  The default of 4096 stops fork bombs
    /// (which create thousands within milliseconds) while leaving ample
    /// headroom for normal operation.
    pub max_processes: u32,
    /// Maximum file size in bytes (RLIMIT_FSIZE). Default: 100 MB.
    pub max_file_size_bytes: u64,
    /// Wall-clock timeout in seconds. Default: 60.
    pub timeout_seconds: u64,
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            max_memory_bytes: 512 * 1024 * 1024,    // 512 MB
            max_cpu_seconds: 60,                    // 60 seconds
            max_processes: 4096,                    // 4096 processes
            max_file_size_bytes: 100 * 1024 * 1024, // 100 MB
            timeout_seconds: 60,                    // 60 seconds
        }
    }
}

/// Failures preserve their OS error and distinguish cleanup from command failure.
#[derive(Debug, thiserror::Error)]
pub enum ResourceLimitError {
    #[error("process setup/spawn failed: {0}")]
    SpawnFailed(#[source] io::Error),
    #[error("process group/job setup failed: {0}")]
    ContainmentSetupFailed(#[source] io::Error),
    #[error("setrlimit failed: {0}")]
    SetLimitFailed(#[source] io::Error),
    #[error("exit observation failed: {0}")]
    ObservationFailed(#[source] io::Error),
    #[error("tree termination/reap failed: {0}")]
    TerminationFailed(#[source] io::Error),
    #[error("process cleanup deadline exceeded")]
    CleanupDeadlineExceeded,
    #[error("resource containment is unsupported on this platform")]
    UnsupportedPlatform,
    /// Sealed environment rejected. Never carries variable values or secrets.
    #[error("sealed environment rejected: {0}")]
    InvalidSealedEnvironment(SealedEnvironmentError),
    /// Sealed spawn specification rejected before any process was created.
    #[error("sealed spawn rejected: {0}")]
    InvalidSealedSpawn(&'static str),
}

/// Bounded, value-free sealed environment validation failures.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum SealedEnvironmentError {
    #[error("invalid environment variable name")]
    InvalidName,
    #[error("reserved environment variable name")]
    ReservedName,
    #[error("duplicate environment variable name")]
    DuplicateName,
    #[error("invalid environment variable value")]
    InvalidValue,
    #[error("runtime directory must be absolute, existing, canonical and a directory")]
    InvalidDirectory,
    #[error("runtime home and temporary directories are required")]
    MissingRuntimeDirectory,
}

// Names the sealed environment never accepts from a caller (compared
// case-insensitively). Runtime directory names are generated only from the
// typed home/temp setters; SystemRoot/windir only from Windows APIs.
const RESERVED_NAMES: &[&str] = &[
    "PATH",
    "HOME",
    "TMPDIR",
    "USERPROFILE",
    "TEMP",
    "TMP",
    "HOMEDRIVE",
    "HOMEPATH",
    "SYSTEMROOT",
    "WINDIR",
    "NODE_OPTIONS",
    "NODE_PATH",
    "NODE_EXTRA_CA_CERTS",
    "NODE_V8_COVERAGE",
    "NODE_REPL_EXTERNAL_MODULE",
    "ESBUILD_BINARY_PATH",
    "HTTP_PROXY",
    "HTTPS_PROXY",
    "ALL_PROXY",
    "NO_PROXY",
    "FTP_PROXY",
    "SSH_AUTH_SOCK",
    "GIT_ASKPASS",
    "LD_PRELOAD",
    "LD_LIBRARY_PATH",
];
const RESERVED_PREFIXES: &[&str] = &["NPM_CONFIG_", "DYLD_", "NEXUS_"];
const RESERVED_SUFFIXES: &[&str] = &["_API_KEY", "_TOKEN", "_SECRET"];

fn valid_name(name: &str) -> bool {
    let mut bytes = name.bytes();
    bytes
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic() || first == b'_')
        && bytes.all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
}

fn reserved_name(canonical: &str) -> bool {
    RESERVED_NAMES.contains(&canonical)
        || RESERVED_PREFIXES
            .iter()
            .any(|prefix| canonical.starts_with(prefix))
        || RESERVED_SUFFIXES
            .iter()
            .any(|suffix| canonical.ends_with(suffix))
}

fn contains_nul(value: &OsStr) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        value.as_bytes().contains(&0)
    }
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        value.encode_wide().any(|unit| unit == 0)
    }
    #[cfg(not(any(unix, windows)))]
    {
        value.to_string_lossy().contains('\0')
    }
}

/// Absolute, existing, canonical directory. Canonicality is not ownership:
/// proving a directory is Nexus-owned is the caller's responsibility.
fn canonical_directory(dir: &Path) -> bool {
    dir.is_absolute() && dir.is_dir() && dir.canonicalize().ok().as_deref() == Some(dir)
}

/// Explicit child environment for the sealed spawn. It begins empty; there is
/// no `Default`, and nothing is ever copied from the parent process.
pub struct SealedEnvironment {
    home: PathBuf,
    temp: PathBuf,
    // Keyed by the ASCII-uppercase name: deterministic order and
    // case-insensitive uniqueness on every platform.
    variables: BTreeMap<String, (String, OsString)>,
}

impl SealedEnvironment {
    pub fn builder() -> SealedEnvironmentBuilder {
        SealedEnvironmentBuilder {
            home: None,
            temp: None,
            variables: BTreeMap::new(),
        }
    }

    pub(crate) fn home(&self) -> &Path {
        &self.home
    }

    pub(crate) fn temp(&self) -> &Path {
        &self.temp
    }

    /// Caller variables in deterministic (uppercase-name) order.
    pub(crate) fn variables(&self) -> impl Iterator<Item = (&str, &OsStr)> {
        self.variables
            .values()
            .map(|(name, value)| (name.as_str(), value.as_os_str()))
    }
}

// Names only: values (possibly sensitive) are never formatted.
impl std::fmt::Debug for SealedEnvironment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SealedEnvironment")
            .field("variables", &self.variables.keys().collect::<Vec<_>>())
            .finish_non_exhaustive()
    }
}

pub struct SealedEnvironmentBuilder {
    home: Option<PathBuf>,
    temp: Option<PathBuf>,
    variables: BTreeMap<String, (String, OsString)>,
}

impl SealedEnvironmentBuilder {
    /// Runtime home (Unix `HOME`; Windows `USERPROFILE`). Set exactly once.
    pub fn home_dir(mut self, dir: &Path) -> Result<Self, SealedEnvironmentError> {
        if self.home.is_some() {
            return Err(SealedEnvironmentError::DuplicateName);
        }
        if !canonical_directory(dir) {
            return Err(SealedEnvironmentError::InvalidDirectory);
        }
        self.home = Some(dir.to_path_buf());
        Ok(self)
    }

    /// Runtime temp (Unix `TMPDIR`; Windows `TEMP` and `TMP`). Set once.
    pub fn temp_dir(mut self, dir: &Path) -> Result<Self, SealedEnvironmentError> {
        if self.temp.is_some() {
            return Err(SealedEnvironmentError::DuplicateName);
        }
        if !canonical_directory(dir) {
            return Err(SealedEnvironmentError::InvalidDirectory);
        }
        self.temp = Some(dir.to_path_buf());
        Ok(self)
    }

    /// An explicit non-reserved runtime variable. ASCII `[A-Za-z_][A-Za-z0-9_]*`
    /// names; duplicates (case-insensitive) are rejected, never replaced.
    pub fn set(
        mut self,
        name: &str,
        value: impl Into<OsString>,
    ) -> Result<Self, SealedEnvironmentError> {
        if !valid_name(name) {
            return Err(SealedEnvironmentError::InvalidName);
        }
        let canonical = name.to_ascii_uppercase();
        if reserved_name(&canonical) {
            return Err(SealedEnvironmentError::ReservedName);
        }
        let value = value.into();
        if contains_nul(&value) {
            return Err(SealedEnvironmentError::InvalidValue);
        }
        if self.variables.contains_key(&canonical) {
            return Err(SealedEnvironmentError::DuplicateName);
        }
        self.variables.insert(canonical, (name.to_owned(), value));
        Ok(self)
    }

    pub fn build(self) -> Result<SealedEnvironment, SealedEnvironmentError> {
        let (Some(home), Some(temp)) = (self.home, self.temp) else {
            return Err(SealedEnvironmentError::MissingRuntimeDirectory);
        };
        Ok(SealedEnvironment {
            home,
            temp,
            variables: self.variables,
        })
    }
}

/// Opt-in sealed long-lived spawn. Deliberately has no `ResourceLimits`, no
/// profile selector, no shell form and no stdin choice (always null): the one
/// fixed long-lived policy is kernel-owned.
#[derive(Debug)]
pub struct SealedSpawnSpec {
    /// Absolute executable; never resolved through PATH.
    pub program: PathBuf,
    pub args: Vec<OsString>,
    /// Absolute, existing, canonical working directory.
    pub current_dir: PathBuf,
    pub environment: SealedEnvironment,
    pub stdout: ResourceOutput,
    pub stderr: ResourceOutput,
}

/// Only the two execution forms required by the terminal and native helpers.
#[derive(Debug, Clone)]
pub enum ResourceProgram {
    /// Use an explicit executable path for portability (Windows does not search PATH).
    Executable {
        program: OsString,
        args: Vec<OsString>,
    },
    /// `sh -lc` on Unix; system-directory `cmd.exe /C` on Windows.
    Shell(String),
}

#[derive(Debug, Clone, Copy)]
pub enum ResourceStdin {
    Inherit,
    Null,
}

#[derive(Debug, Clone, Copy)]
pub enum ResourceOutput {
    Inherit,
    Null,
    Piped,
}

/// Environment is inherited unchanged. No arbitrary pre-exec or handle hooks.
#[derive(Debug, Clone)]
pub struct ResourceSpawnSpec {
    pub program: ResourceProgram,
    pub current_dir: PathBuf,
    pub stdin: ResourceStdin,
    pub stdout: ResourceOutput,
    pub stderr: ResourceOutput,
}

/// Fixed backend-only environment policy for Windows actuators. No caller map.
#[cfg(windows)]
pub(crate) enum ActuatorEnvironment {
    Shell { path: OsString },
    InlineCode { path: OsString },
}

pub type ResourceReader = Box<dyn Read + Send>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminationReport {
    pub status: ExitStatus,
    pub already_finalized: bool,
}

#[derive(Debug, Clone, Default)]
pub struct ResourceLimiter {
    limits: ResourceLimits,
}

impl ResourceLimiter {
    pub fn new(limits: ResourceLimits) -> Self {
        Self { limits }
    }

    pub fn limits(&self) -> &ResourceLimits {
        &self.limits
    }

    /// Private actuator adapter; public spawn retains inherited environment semantics.
    #[cfg(windows)]
    pub(crate) fn spawn_actuator(
        &self,
        spec: &ResourceSpawnSpec,
        environment: &ActuatorEnvironment,
    ) -> Result<ResourceLimitedChild, ResourceLimitError> {
        let child = platform::Child::spawn_actuator(spec, &self.limits, environment)?;
        Ok(ResourceLimitedChild {
            id: child.id(),
            child: Some(child),
            status: None,
        })
    }

    /// Sealed long-lived spawn (P0-002C4C2): empty environment plus only the
    /// sealed entries and mandatory OS-derived variables, explicit absolute
    /// executable, canonical cwd, and the fixed long-lived policy installed
    /// before the workload runs. Never uses this limiter's `ResourceLimits`.
    pub fn spawn_sealed(
        &self,
        spec: &SealedSpawnSpec,
    ) -> Result<ResourceLimitedChild, ResourceLimitError> {
        if !spec.program.is_absolute() {
            return Err(ResourceLimitError::InvalidSealedSpawn(
                "executable must be an absolute path",
            ));
        }
        if !canonical_directory(&spec.current_dir) {
            return Err(ResourceLimitError::InvalidSealedSpawn(
                "working directory must be absolute, existing and canonical",
            ));
        }
        if !canonical_directory(spec.environment.home())
            || !canonical_directory(spec.environment.temp())
        {
            return Err(ResourceLimitError::InvalidSealedEnvironment(
                SealedEnvironmentError::InvalidDirectory,
            ));
        }
        #[cfg(any(target_os = "linux", target_os = "macos", windows))]
        {
            let child = platform::Child::spawn_sealed(spec)?;
            Ok(ResourceLimitedChild {
                id: child.id(),
                child: Some(child),
                status: None,
            })
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
        {
            Err(ResourceLimitError::UnsupportedPlatform)
        }
    }

    /// Fail closed if containment cannot be established before execution.
    pub fn spawn(
        &self,
        spec: &ResourceSpawnSpec,
    ) -> Result<ResourceLimitedChild, ResourceLimitError> {
        #[cfg(any(target_os = "linux", target_os = "macos", windows))]
        {
            let child = platform::Child::spawn(spec, &self.limits)?;
            Ok(ResourceLimitedChild {
                id: child.id(),
                child: Some(child),
                status: None,
            })
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
        {
            let _ = spec;
            Err(ResourceLimitError::UnsupportedPlatform)
        }
    }
}

/// Sole owner of process identity and containment. Intentionally not Clone.
///
/// `poll_exit` never releases that identity. After successful finalization only
/// the historical ID/status remain; subsequent calls never signal that ID.
/// On Unix the application must not independently reap this owner's child
/// (including via a global SIGCHLD reaper or SA_NOCLDWAIT).
pub struct ResourceLimitedChild {
    id: u32,
    #[cfg(any(target_os = "linux", target_os = "macos", windows))]
    child: Option<platform::Child>,
    status: Option<ExitStatus>,
}

impl ResourceLimitedChild {
    pub fn id(&self) -> u32 {
        self.id
    }

    pub fn take_stdout(&mut self) -> Option<ResourceReader> {
        #[cfg(any(target_os = "linux", target_os = "macos", windows))]
        {
            self.child.as_mut()?.take_stdout()
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
        {
            None
        }
    }

    pub fn take_stderr(&mut self) -> Option<ResourceReader> {
        #[cfg(any(target_os = "linux", target_os = "macos", windows))]
        {
            self.child.as_mut()?.take_stderr()
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
        {
            None
        }
    }

    /// Observe root exit without reaping or closing the containment identity.
    pub fn poll_exit(&mut self) -> Result<Option<ExitStatus>, ResourceLimitError> {
        if let Some(status) = self.status {
            return Ok(Some(status));
        }
        #[cfg(any(target_os = "linux", target_os = "macos", windows))]
        {
            self.child.as_mut().expect("unfinalized owner").poll_exit()
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
        {
            Err(ResourceLimitError::UnsupportedPlatform)
        }
    }

    /// Request tree termination, then finalize the root within `deadline`.
    /// Failure retains ownership for retry/Drop unless the root was finalized
    /// just as the deadline expired. Never join pipe readers after failure.
    /// An expired deadline still requests termination, but cannot
    /// report successful cleanup. Repeated successful calls are deterministic.
    pub fn terminate_and_reap(
        &mut self,
        deadline: Instant,
    ) -> Result<TerminationReport, ResourceLimitError> {
        if let Some(status) = self.status {
            return Ok(TerminationReport {
                status,
                already_finalized: true,
            });
        }
        #[cfg(any(target_os = "linux", target_os = "macos", windows))]
        {
            let child = self.child.as_mut().expect("unfinalized owner");
            child.request_termination()?;
            loop {
                if Instant::now() >= deadline {
                    return Err(ResourceLimitError::CleanupDeadlineExceeded);
                }
                if let Some(status) = child.try_finalize()? {
                    self.status = Some(status);
                    // OS identity is released only after termination and root reap.
                    self.child.take();
                    if Instant::now() >= deadline {
                        return Err(ResourceLimitError::CleanupDeadlineExceeded);
                    }
                    return Ok(TerminationReport {
                        status,
                        already_finalized: false,
                    });
                }
                std::thread::sleep(
                    Duration::from_millis(5)
                        .min(deadline.saturating_duration_since(Instant::now())),
                );
            }
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
        {
            let _ = deadline;
            Err(ResourceLimitError::UnsupportedPlatform)
        }
    }
}

impl Drop for ResourceLimitedChild {
    fn drop(&mut self) {
        #[cfg(any(target_os = "linux", target_os = "macos", windows))]
        if let Some(child) = self.child.as_mut() {
            // Defense in depth only: no blocking wait, no reportable success.
            if child.request_termination().is_ok() {
                let _ = child.try_finalize();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_limits_are_sensible() {
        let limits = ResourceLimits::default();
        assert_eq!(limits.max_memory_bytes, 512 * 1024 * 1024);
        assert_eq!(limits.max_cpu_seconds, 60);
        assert_eq!(limits.max_processes, 4096);
        assert_eq!(limits.max_file_size_bytes, 100 * 1024 * 1024);
        assert_eq!(limits.timeout_seconds, 60);
    }

    #[test]
    fn limiter_custom_limits() {
        let limits = ResourceLimits {
            max_memory_bytes: 1024,
            max_cpu_seconds: 5,
            max_processes: 2,
            max_file_size_bytes: 2048,
            timeout_seconds: 10,
        };
        let limiter = ResourceLimiter::new(limits.clone());
        assert_eq!(limiter.limits().max_memory_bytes, 1024);
        assert_eq!(limiter.limits().max_cpu_seconds, 5);
        assert_eq!(limiter.limits().max_processes, 2);
        assert_eq!(limiter.limits().max_file_size_bytes, 2048);
        assert_eq!(limiter.limits().timeout_seconds, 10);
    }

    #[test]
    fn limiter_default_trait() {
        let limiter = ResourceLimiter::default();
        assert_eq!(limiter.limits().max_memory_bytes, 512 * 1024 * 1024);
    }

    #[test]
    fn error_display() {
        let e = ResourceLimitError::SetLimitFailed(std::io::Error::from_raw_os_error(12));
        assert!(std::error::Error::source(&e).is_some());

        let e = ResourceLimitError::ContainmentSetupFailed(std::io::Error::from_raw_os_error(3));
        assert!(std::error::Error::source(&e).is_some());
    }

    #[test]
    fn resource_limits_serialize_deserialize() {
        let limits = ResourceLimits {
            max_memory_bytes: 256 * 1024 * 1024,
            max_cpu_seconds: 30,
            max_processes: 100,
            max_file_size_bytes: 50 * 1024 * 1024,
            timeout_seconds: 45,
        };
        let json = serde_json::to_string(&limits).expect("serialize");
        let deserialized: ResourceLimits = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(limits, deserialized);
    }

    #[test]
    fn default_limits_serialize_round_trip() {
        let defaults = ResourceLimits::default();
        let json = serde_json::to_string_pretty(&defaults).expect("serialize");
        let round_tripped: ResourceLimits = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(defaults, round_tripped);
    }

    // ── P0-002C4C2 sealed environment / spec / fixed policy ──────────────

    fn canonical_dir() -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let canonical = dir.path().canonicalize().unwrap();
        (dir, canonical)
    }

    fn sealed(home: &Path, temp: &Path) -> SealedEnvironmentBuilder {
        SealedEnvironment::builder()
            .home_dir(home)
            .unwrap()
            .temp_dir(temp)
            .unwrap()
    }

    #[test]
    fn sealed_environment_starts_empty_and_requires_runtime_directories() {
        let (_dir, root) = canonical_dir();
        assert_eq!(
            SealedEnvironment::builder().build().unwrap_err(),
            SealedEnvironmentError::MissingRuntimeDirectory
        );
        assert_eq!(
            SealedEnvironment::builder()
                .home_dir(&root)
                .unwrap()
                .build()
                .unwrap_err(),
            SealedEnvironmentError::MissingRuntimeDirectory
        );
        let environment = sealed(&root, &root).build().unwrap();
        assert_eq!(environment.variables().count(), 0);
        assert_eq!(environment.home(), root);
        // Each runtime directory is typed and set exactly once.
        assert_eq!(
            sealed(&root, &root).home_dir(&root).err(),
            Some(SealedEnvironmentError::DuplicateName)
        );
    }

    #[test]
    fn sealed_runtime_directories_must_be_absolute_existing_canonical_directories() {
        let (_dir, root) = canonical_dir();
        let file = root.join("file");
        std::fs::write(&file, b"x").unwrap();
        std::fs::create_dir(root.join("sub")).unwrap();
        let mut non_canonical = root.as_os_str().to_owned();
        non_canonical.push(std::path::MAIN_SEPARATOR_STR);
        non_canonical.push("sub");
        non_canonical.push(std::path::MAIN_SEPARATOR_STR);
        non_canonical.push("..");
        for dir in [
            PathBuf::from("relative"),
            root.join("missing"),
            file,
            PathBuf::from(non_canonical),
        ] {
            let builder = SealedEnvironment::builder;
            assert_eq!(
                builder().home_dir(&dir).err(),
                Some(SealedEnvironmentError::InvalidDirectory),
                "{dir:?}"
            );
            assert_eq!(
                builder().temp_dir(&dir).err(),
                Some(SealedEnvironmentError::InvalidDirectory),
                "{dir:?}"
            );
        }
    }

    #[test]
    fn sealed_names_are_validated_reserved_and_case_insensitively_unique() {
        let (_dir, root) = canonical_dir();
        for name in ["", "1A", "A-B", "A B", "A=B", "\u{e9}T\u{c9}", "A.B", "A\0"] {
            assert_eq!(
                sealed(&root, &root).set(name, "v").err(),
                Some(SealedEnvironmentError::InvalidName),
                "{name:?}"
            );
        }
        for name in [
            "PATH",
            "Path",
            "home",
            "TMPDIR",
            "UserProfile",
            "temp",
            "TMP",
            "HOMEDRIVE",
            "HomePath",
            "SystemRoot",
            "WINDIR",
            "node_options",
            "NODE_PATH",
            "Node_Extra_CA_Certs",
            "NODE_V8_COVERAGE",
            "NODE_REPL_EXTERNAL_MODULE",
            "npm_config_registry",
            "NPM_CONFIG_FAKE",
            "esbuild_binary_path",
            "http_proxy",
            "HTTPS_PROXY",
            "all_proxy",
            "No_Proxy",
            "FTP_PROXY",
            "ssh_auth_sock",
            "GIT_ASKPASS",
            "ld_preload",
            "LD_LIBRARY_PATH",
            "DYLD_INSERT_LIBRARIES",
            "dyld_anything",
            "OPENAI_API_KEY",
            "fake_provider_api_key",
            "GITHUB_TOKEN",
            "my_token",
            "DB_SECRET",
            "client_secret",
            "NEXUS_FAKE_SECRET",
            "nexus_anything",
            "NEXUS_DEVSERVER_NONCE",
        ] {
            assert_eq!(
                sealed(&root, &root).set(name, "v").err(),
                Some(SealedEnvironmentError::ReservedName),
                "{name}"
            );
        }
        // Duplicates are rejected, never replaced, including case aliases.
        let first = sealed(&root, &root).set("Mode", "one").unwrap();
        assert_eq!(
            first.set("MODE", "two").err(),
            Some(SealedEnvironmentError::DuplicateName)
        );
        assert_eq!(
            sealed(&root, &root)
                .set("mode", "a")
                .unwrap()
                .set("mode", "b")
                .err(),
            Some(SealedEnvironmentError::DuplicateName)
        );
    }

    #[test]
    fn sealed_values_reject_nul_and_errors_never_carry_values() {
        let (_dir, root) = canonical_dir();
        let error = sealed(&root, &root)
            .set("VISIBLE", "sentinel-secret-value\0tail")
            .err()
            .unwrap();
        assert_eq!(error, SealedEnvironmentError::InvalidValue);
        for text in [
            error.to_string(),
            format!("{error:?}"),
            ResourceLimitError::InvalidSealedEnvironment(error).to_string(),
        ] {
            assert!(!text.contains("sentinel"), "{text}");
        }
        let environment = sealed(&root, &root)
            .set("VISIBLE", "sentinel-secret-value")
            .unwrap()
            .build()
            .unwrap();
        let debug = format!("{environment:?}");
        assert!(
            debug.contains("VISIBLE") && !debug.contains("sentinel"),
            "{debug}"
        );
    }

    #[test]
    fn sealed_variables_have_deterministic_order() {
        let (_dir, root) = canonical_dir();
        let orders = [
            ["zeta", "Alpha", "_under", "beta"],
            ["beta", "_under", "zeta", "Alpha"],
        ];
        for order in orders {
            let environment = order
                .iter()
                .fold(sealed(&root, &root), |builder, name| {
                    builder.set(name, "v").unwrap()
                })
                .build()
                .unwrap();
            let names: Vec<_> = environment.variables().map(|(name, _)| name).collect();
            assert_eq!(names, ["Alpha", "beta", "zeta", "_under"]);
        }
    }

    #[test]
    fn sealed_spawn_rejects_invalid_specification_before_any_process() {
        let (_dir, root) = canonical_dir();
        std::fs::create_dir(root.join("sub")).unwrap();
        let spec = |program: PathBuf, current_dir: PathBuf| SealedSpawnSpec {
            program,
            args: Vec::new(),
            current_dir,
            environment: sealed(&root, &root).build().unwrap(),
            stdout: ResourceOutput::Null,
            stderr: ResourceOutput::Null,
        };
        let absolute = root.join("missing-program");
        let mut non_canonical = root.as_os_str().to_owned();
        non_canonical.push(std::path::MAIN_SEPARATOR_STR);
        non_canonical.push("sub");
        non_canonical.push(std::path::MAIN_SEPARATOR_STR);
        non_canonical.push("..");
        for (program, cwd) in [
            (PathBuf::from("node"), root.clone()),
            (PathBuf::from("bin/node"), root.clone()),
            (absolute.clone(), PathBuf::from("relative")),
            (absolute.clone(), root.join("missing")),
            (absolute.clone(), PathBuf::from(non_canonical)),
        ] {
            assert!(
                matches!(
                    ResourceLimiter::default().spawn_sealed(&spec(program.clone(), cwd.clone())),
                    Err(ResourceLimitError::InvalidSealedSpawn(_))
                ),
                "{program:?} {cwd:?}"
            );
        }
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn sealed_linux_policy_is_exactly_data_and_file_size() {
        use platform::Rlimit;
        assert_eq!(
            platform::sealed_rlimits(),
            vec![
                (Rlimit::Data, 2 * 1024 * 1024 * 1024),
                (Rlimit::FileSize, 100 * 1024 * 1024),
            ]
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn sealed_macos_policy_is_exactly_file_size() {
        use platform::Rlimit;
        assert_eq!(
            platform::sealed_rlimits(),
            vec![(Rlimit::FileSize, 100 * 1024 * 1024)]
        );
    }

    // Focused guards on the sealed functions only (comments stripped).
    fn function(source: &str, signature: &str) -> String {
        let start = source.find(signature).unwrap();
        let body = &source[start..];
        // Methods (indented) end at their own closing brace, never the impl's.
        let close = if signature.starts_with(' ') {
            "\n    }\n"
        } else {
            "\n}\n"
        };
        let end = body.find(close).unwrap();
        body[..end]
            .lines()
            .map(|line| line.split("//").next().unwrap_or(""))
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn sealed_paths_cannot_inherit_resolve_or_take_caller_limits() {
        let common = include_str!("resource_limiter.rs");
        let unix = include_str!("resource_limiter/unix.rs");
        let windows = include_str!("resource_limiter/windows.rs");
        let sealed = [
            function(common, "    pub fn spawn_sealed("),
            function(unix, "    pub(super) fn spawn_sealed("),
            function(unix, "pub(super) fn sealed_rlimits("),
            function(windows, "    pub(super) fn spawn_sealed("),
            function(windows, "pub(super) fn sealed_environment_block("),
            function(windows, "pub(super) fn sealed_job_limits("),
            function(windows, "fn windows_directory("),
        ];
        for code in &sealed {
            for forbidden in [
                "vars_os",
                "env::vars",
                "env::var(",
                "ResourceProgram::Shell",
                "ResourceLimits",
                "self.limits",
                "Command::new(\"",
                "legacy_job_limits",
                "BREAKAWAY",
                "RLIMIT_AS",
                "RLIMIT_CPU",
                "RLIMIT_NPROC",
                "Rlimit::AddressSpace",
                "Rlimit::Cpu",
                "Rlimit::Processes",
            ] {
                assert!(!code.contains(forbidden), "{forbidden} in:\n{code}");
            }
        }
        // Unix clears the inherited environment before any sealed entry and
        // passes only the fixed sealed policy.
        let unix_spawn = &sealed[1];
        let clear = unix_spawn.find(".env_clear()").unwrap();
        assert!(clear < unix_spawn.find(".env(").unwrap());
        assert!(unix_spawn.contains("spawn_contained(command, sealed_rlimits())"));
        // Windows never passes a null (inheriting) block and uses the sealed
        // Job policy; the legacy path keeps the legacy policy.
        let windows_spawn = &sealed[3];
        assert!(windows_spawn.contains("sealed_environment_block(entries)"));
        assert!(windows_spawn
            .contains("Self::create(&process_spec, Some(&block), &sealed_job_limits())"));
        assert!(!windows_spawn.contains("None"));
        let legacy = function(windows, "    fn spawn_with_environment(");
        assert!(legacy.contains("Self::create(spec, environment, &legacy_job_limits())"));
    }

    #[test]
    fn limits_are_reasonable() {
        let limits = ResourceLimits::default();
        assert!(limits.max_memory_bytes >= 100 * 1024 * 1024);
        assert!(limits.max_memory_bytes <= 2 * 1024 * 1024 * 1024);
        assert!(limits.max_cpu_seconds >= 10);
        assert!(limits.max_cpu_seconds <= 300);
        assert!(limits.max_processes >= 100);
        assert!(limits.max_processes <= 10_000);
        assert!(limits.max_file_size_bytes >= 10 * 1024 * 1024);
        assert!(limits.max_file_size_bytes <= 1024 * 1024 * 1024);
        assert!(limits.timeout_seconds >= 10);
        assert!(limits.timeout_seconds <= 300);
    }
}
