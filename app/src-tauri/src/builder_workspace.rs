//! Private authority for fresh Builder planning, registered React file writes and
//! fail-closed dev-server selection (no process launch is authorized).
//! Metadata and paths are never accepted as credentials. No authority lock is
//! held by this adapter across audit, provider calls, filesystem I/O or delivery.
use nexus_kernel::manifest::FsPermissionLevel;
use nexus_kernel::workspace::resolve_existing_relative;
use nexus_kernel::workspace_authority::{
    WorkspaceAuthorityRegistry, WorkspaceAuthoritySource, WorkspaceBinding, WorkspaceGrantId,
};
use serde_json::{json, Value};
use std::collections::{hash_map::Entry, HashMap};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime};
use uuid::Uuid;
use web_builder_agent::model_router::ModelSelection;
use web_builder_agent::plan::PlanResult;
use web_builder_agent::project::{create_project, transition, ProjectState, ProjectStatus};

mod directory_identity;
use directory_identity::{DirectoryIdentity, IdentityError};
// P0-002C4B private lifecycle primitive. P0-002C4C1: constructed only by
// BuilderWorkspaceAuthority::provision; production uses stop/status/shutdown.
mod process_lifecycle;
use process_lifecycle::{Finalized, LifecycleError, LifecycleRegistry};
// P0-002C4D1A private trusted-toolchain verifier. Staged: production has no
// trusted toolchain or root, and nothing outside this adapter can reach it.
mod trusted_toolchain;

// Audit payloads contain descriptive project IDs, never grants or private principals.
type Audit = Arc<dyn Fn(Value) + Send + Sync>;

#[derive(Debug)]
enum PlanningError {
    Authority(String),
    Persistence(String),
    Provider(String),
}
impl std::fmt::Display for PlanningError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let (kind, detail) = match self {
            Self::Authority(s) => ("authority", s),
            Self::Persistence(s) => ("persistence", s),
            Self::Provider(s) => ("provider", s),
        };
        write!(f, "Builder planning {kind}: {detail}")
    }
}
type Result<T> = std::result::Result<T, PlanningError>;

pub(super) struct BuilderWorkspaceAuthority {
    registry: Arc<WorkspaceAuthorityRegistry>,
    root: PathBuf,
    allocator: Uuid,
    planner: Uuid,
    writer: Uuid,
    storage_identity: Arc<DirectoryIdentity>,
    catalog: Arc<ProjectCatalog>,
    // The only production lifecycle registry; it shares `catalog` exactly.
    lifecycle: LifecycleRegistry,
}
impl BuilderWorkspaceAuthority {
    /// Trusted startup only. The existing identity-home policy rejects malformed
    /// HOME and uses the native Windows profile only when HOME is absent.
    pub(super) fn setup(
        registry: Arc<WorkspaceAuthorityRegistry>,
    ) -> std::result::Result<Arc<Self>, String> {
        let root = crate::oracle_runtime::default_identity_path_for("builds")
            .map_err(|e| format!("Builder storage setup: {e}"))?;
        Self::provision(registry, &root)
            .map(Arc::new)
            .map_err(|e| e.to_string())
    }

    fn provision(registry: Arc<WorkspaceAuthorityRegistry>, root: &Path) -> Result<Self> {
        if !root.is_absolute() {
            return Err(PlanningError::Authority("storage must be absolute".into()));
        }
        // The only recursive storage creation in this adapter is trusted setup.
        std::fs::create_dir_all(root)
            .map_err(|_| PlanningError::Persistence("storage provisioning failed".into()))?;
        let root = root
            .canonicalize()
            .map_err(|_| PlanningError::Authority("storage unavailable".into()))?;
        validate_root(&root)?;
        let storage_identity = DirectoryIdentity::capture(&root)
            .map_err(|_| PlanningError::Authority("storage identity unavailable".into()))?;
        let catalog = Arc::new(ProjectCatalog::default());
        let lifecycle = LifecycleRegistry::new(Arc::clone(&catalog));
        Ok(Self {
            registry,
            root,
            allocator: Uuid::new_v4(),
            planner: Uuid::new_v4(),
            writer: Uuid::new_v4(),
            storage_identity: Arc::new(storage_identity),
            catalog,
            lifecycle,
        })
    }

    fn begin(&self, audit: Audit) -> Result<PlanningExecution> {
        validate_root(&self.root)?;
        self.allocate(Uuid::new_v4(), Uuid::new_v4(), None, audit)
    }

    fn allocate(
        &self,
        project_id: Uuid,
        run_id: Uuid,
        expiry: Option<SystemTime>,
        audit: Audit,
    ) -> Result<PlanningExecution> {
        validate_root(&self.root)?;
        let allocator = WorkspaceBinding {
            agent_id: self.allocator,
            run_id,
        };
        let planner = WorkspaceBinding {
            agent_id: self.planner,
            run_id,
        };
        let allocation = self
            .registry
            .issue_trusted_root(
                &self.root,
                allocator,
                WorkspaceAuthoritySource::BackendAllocated,
                FsPermissionLevel::ReadWrite,
                expiry,
            )
            .map_err(|e| PlanningError::Authority(e.to_string()))?;
        // Arm before audit/allocation/narrowing so all subsequent exits revoke.
        let mut execution = PlanningExecution {
            registry: Arc::clone(&self.registry),
            project_id,
            allocator,
            planner,
            allocation,
            project: None,
            root: self.root.join(project_id.to_string()),
            audit,
            revoked: false,
            registration: None,
            catalog: Arc::clone(&self.catalog),
        };
        execution.event("issue", "authorized");
        let allocated = (|| {
            let target = execution.authorize(
                Some(allocation),
                allocator,
                &self.root,
                Path::new(&project_id.to_string()),
                "allocate",
            )?;
            std::fs::create_dir(&target).map_err(|e| {
                PlanningError::Persistence(format!(
                    "exclusive project allocation failed: {}",
                    e.kind()
                ))
            })?;
            validate_root(&target)?;
            let canonical = target
                .canonicalize()
                .map_err(|_| PlanningError::Authority("project unavailable".into()))?;
            let identity = DirectoryIdentity::capture(&canonical)
                .map_err(|_| PlanningError::Authority("project identity unavailable".into()))?;
            execution.registration = Some(Arc::new(RegisteredBuilderProject {
                project_id,
                root: canonical.clone(),
                storage_root: self.root.clone(),
                storage_identity: Arc::clone(&self.storage_identity),
                identity,
            }));
            let child = self
                .registry
                .narrow(
                    allocation,
                    allocator,
                    self.planner,
                    &canonical,
                    FsPermissionLevel::ReadWrite,
                )
                .map_err(|e| PlanningError::Authority(e.to_string()))?;
            execution.root = canonical;
            execution.project = Some(child);
            execution.event("narrow", "authorized");
            Ok(())
        })();
        if let Err(error) = allocated {
            execution.revoke()?;
            return Err(error);
        }
        Ok(execution)
    }
}

type WriteResult<T> = std::result::Result<T, &'static str>;

// No grant or binding survives in this catalog. None is a permanent tombstone.
// Snapshots share the retained handle; invalidation releases the catalog's
// ownership. Any in-flight snapshot releases its last reference on exit.
#[derive(Default)]
struct ProjectCatalog {
    projects: Mutex<HashMap<Uuid, Option<Arc<RegisteredBuilderProject>>>>,
}

struct RegisteredBuilderProject {
    project_id: Uuid,
    root: PathBuf,
    storage_root: PathBuf,
    storage_identity: Arc<DirectoryIdentity>,
    identity: DirectoryIdentity,
}

impl RegisteredBuilderProject {
    fn validate_identity(&self) -> std::result::Result<(), IdentityError> {
        self.storage_identity.validate(&self.storage_root)?;
        if self.root.parent() != Some(self.storage_root.as_path()) {
            return Err(IdentityError::Changed);
        }
        self.identity.validate(&self.root)
    }
}

impl ProjectCatalog {
    // Called only by finish_plan after successful persistence AND revocation.
    fn publish(&self, project: Arc<RegisteredBuilderProject>) -> WriteResult<()> {
        project
            .validate_identity()
            .map_err(|_| "registration identity denied")?;
        let mut projects = self.projects.lock().map_err(|_| "catalog unavailable")?;
        match projects.entry(project.project_id) {
            Entry::Vacant(entry) => {
                entry.insert(Some(project));
                Ok(())
            }
            Entry::Occupied(_) => Err("registration already exists"),
        }
    }

    fn lookup(&self, id: Uuid) -> WriteResult<Arc<RegisteredBuilderProject>> {
        self.projects
            .lock()
            .map_err(|_| "catalog unavailable")?
            .get(&id)
            .and_then(Option::as_ref)
            .cloned()
            .ok_or("project not registered")
    }

    fn active(&self, project: &Arc<RegisteredBuilderProject>) -> WriteResult<()> {
        let current = self.lookup(project.project_id)?;
        if !Arc::ptr_eq(&current, project) {
            return Err("registration changed");
        }
        Ok(())
    }

    fn invalidate(&self, project: &Arc<RegisteredBuilderProject>) -> WriteResult<()> {
        let released = {
            let mut projects = self.projects.lock().map_err(|_| "catalog unavailable")?;
            let entry = projects
                .get_mut(&project.project_id)
                .ok_or("project not registered")?;
            if entry
                .as_ref()
                .is_some_and(|current| Arc::ptr_eq(current, project))
            {
                entry.take()
            } else {
                None
            }
        };
        // Native handle destruction is outside the catalog lock as well.
        drop(released);
        Ok(())
    }

    fn validate(&self, project: &Arc<RegisteredBuilderProject>, audit: &Audit) -> WriteResult<()> {
        self.active(project)?;
        if let Err(error) = project.validate_identity() {
            if error == IdentityError::Changed {
                self.invalidate(project)?;
                event(
                    audit,
                    Some(project.project_id),
                    "registration.invalidate",
                    "invalidated",
                );
            }
            return Err("registration identity denied");
        }
        self.active(project)
    }
}

fn event(audit: &Audit, project: Option<Uuid>, operation: &str, outcome: &str) {
    audit(json!({"operation": format!("builder.{operation}"),
        "project_id": project.map(|id| id.to_string()), "outcome": outcome}));
}

/// Portable lexical contract, before the existing P0-002A pathname policy.
fn validate_relative_file(relative: &str) -> WriteResult<()> {
    if relative.is_empty()
        || relative
            .chars()
            .any(|c| c.is_control() || "\\:<>\"|?*".contains(c))
    {
        return Err("relative file path denied");
    }
    for component in relative.split('/') {
        if component.is_empty()
            || component == "."
            || component == ".."
            || component.ends_with(['.', ' '])
        {
            return Err("relative file path denied");
        }
        let stem = component
            .split('.')
            .next()
            .unwrap_or("")
            .trim_end_matches(' ')
            .to_uppercase();
        if matches!(
            stem.as_str(),
            "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
        ) || ["COM", "LPT"].iter().any(|prefix| {
            stem.strip_prefix(prefix).is_some_and(|suffix| {
                matches!(
                    suffix,
                    "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
                )
            })
        }) {
            return Err("relative file path denied");
        }
    }
    Ok(())
}

impl BuilderWorkspaceAuthority {
    fn begin_write(
        &self,
        selector: &str,
        relative: &str,
        audit: Audit,
    ) -> WriteResult<WriteExecution<'_>> {
        let id = Uuid::parse_str(selector).map_err(|_| {
            event(&audit, None, "registration.lookup", "denied");
            "project not registered"
        })?;
        let project = self.catalog.lookup(id).inspect_err(|_| {
            event(&audit, Some(id), "registration.lookup", "denied");
        })?;
        self.catalog.validate(&project, &audit)?;
        let root = project.root.join("react");
        // capture requires the exact canonical child, a directory and no reparse
        // redirection. Nothing in this path provisions storage or directories.
        let identity = DirectoryIdentity::capture(&root).map_err(|_| "React identity denied")?;
        validate_relative_file(relative)?;
        let binding = WorkspaceBinding {
            agent_id: self.writer,
            run_id: Uuid::new_v4(),
        };
        // A synchronous single-write execution has a bounded authority lifetime
        // even if finalization fails. Explicit revoke is still required.
        let expiry = SystemTime::now()
            .checked_add(Duration::from_secs(60))
            .ok_or("expiry unavailable")?;
        let grant = self
            .registry
            .issue_trusted_root(
                &root,
                binding,
                WorkspaceAuthoritySource::BackendAllocated,
                FsPermissionLevel::ReadWrite,
                Some(expiry),
            )
            .map_err(|_| {
                event(&audit, Some(id), "write.issue", "denied");
                "write issuance denied"
            })?;
        let execution = WriteExecution {
            authority: self,
            project,
            root,
            identity,
            binding,
            grant,
            audit,
            revoked: false,
        };
        execution.event("issue", "authorized");
        Ok(execution)
    }

    fn write_file(
        &self,
        selector: &str,
        relative: &str,
        content: &[u8],
        audit: Audit,
    ) -> WriteResult<()> {
        let execution = self
            .begin_write(selector, relative, Arc::clone(&audit))
            .inspect_err(|_| {
                event(&audit, Uuid::parse_str(selector).ok(), "write", "denied");
            })?;
        execution.finish(relative, content)
    }
}

// P0-002C4A: no process-launch design is approved. Start validates the private
// registration and retained identities, then fails closed. P0-002C4C1: stop and
// status first consult the authority-owned lifecycle registry by project key;
// only without an owned execution do they fall back to C4A validation. Nothing
// here spawns a process, creates a directory or holds a lock across audit.

/// One bounded wait for a single stop (C4B cleanup budget plus monitor slack).
const DEV_SERVER_STOP_WAIT: Duration = Duration::from_secs(8);
/// One overall bound for all Builder dev servers at normal application exit.
const DEV_SERVER_SHUTDOWN_WAIT: Duration = Duration::from_secs(10);
#[derive(Clone, Copy)]
enum DevServerOperation {
    Start,
    Stop,
    Status,
    Shutdown,
}

impl DevServerOperation {
    fn name(self) -> &'static str {
        match self {
            Self::Start => "start",
            Self::Stop => "stop",
            Self::Status => "status",
            Self::Shutdown => "shutdown",
        }
    }
}

// Bounded reason categories; never paths, handles, principals or processes.
fn dev_server_event(
    audit: &Audit,
    project: Option<Uuid>,
    operation: DevServerOperation,
    outcome: &str,
    reason: &str,
) {
    audit(
        json!({"operation": format!("builder.devserver.{}", operation.name()),
        "project_id": project.map(|id| id.to_string()), "outcome": outcome, "reason": reason}),
    );
}

type DevServerDenial = (&'static str, &'static str); // (audit reason, client error)

/// Retained trusted snapshot: the private registration and the identity of its
/// existing React directory. React is always derived from the registration,
/// never from caller data. Not serialized; not a credential or containment.
struct DevServerTarget {
    project: Arc<RegisteredBuilderProject>,
    react_identity: DirectoryIdentity,
}

impl DevServerTarget {
    fn validate_react(&self) -> std::result::Result<(), IdentityError> {
        self.react_identity
            .validate(&self.project.root.join("react"))
    }
}

impl BuilderWorkspaceAuthority {
    /// Selector → private registration → storage, project and existing React
    /// identity. React must be the exact, non-redirected child of the
    /// registered project; it is observed, never created.
    fn validate_dev_server(
        &self,
        selector: &str,
        audit: &Audit,
    ) -> std::result::Result<Uuid, DevServerDenial> {
        self.dev_server_target(selector, audit)
            .map(|target| target.project.project_id)
    }

    fn dev_server_target(
        &self,
        selector: &str,
        audit: &Audit,
    ) -> std::result::Result<DevServerTarget, DevServerDenial> {
        let id =
            Uuid::parse_str(selector).map_err(|_| ("registration", "project not registered"))?;
        let project = self
            .catalog
            .lookup(id)
            .map_err(|error| ("registration", error))?;
        self.catalog
            .validate(&project, audit)
            .map_err(|error| ("identity", error))?;
        let react_identity = DirectoryIdentity::capture(&project.root.join("react"))
            .map_err(|_| ("react", "React identity denied"))?;
        let target = DevServerTarget {
            project,
            react_identity,
        };
        // Final checks after capture: the registration is still current and
        // React is still the same directory beneath the registered project.
        self.catalog
            .validate(&target.project, audit)
            .map_err(|error| ("identity", error))?;
        target
            .validate_react()
            .map_err(|_| ("react", "React identity denied"))?;
        Ok(target)
    }

    fn dev_server(
        &self,
        selector: &str,
        operation: DevServerOperation,
        audit: &Audit,
    ) -> WriteResult<Uuid> {
        self.validate_dev_server(selector, audit)
            .map_err(|(reason, error)| {
                let project = Uuid::parse_str(selector).ok();
                dev_server_event(audit, project, operation, "denied", reason);
                error
            })
    }

    /// Returns only a denial: launching npm, npx, Vite or any other process is
    /// not authorized until a separately approved launch design exists.
    fn dev_server_start(&self, selector: &str, audit: Audit) -> &'static str {
        match self.dev_server(selector, DevServerOperation::Start, &audit) {
            Ok(id) => {
                dev_server_event(
                    &audit,
                    Some(id),
                    DevServerOperation::Start,
                    "denied",
                    "launch_unavailable",
                );
                "launch unavailable"
            }
            Err(error) => error,
        }
    }

    /// Stop by backend-owned execution first. An execution this registry
    /// already owns is terminated through its retained owner even if the
    /// registration was since invalidated or tombstoned: termination authority
    /// is the owner, never a PID, port, process name or OS rediscovery.
    /// Without an owned execution, the C4A selector validation applies.
    fn dev_server_stop(&self, selector: &str, audit: Audit) -> WriteResult<()> {
        self.dev_server_stop_until(selector, Instant::now() + DEV_SERVER_STOP_WAIT, audit)
    }

    fn dev_server_stop_until(
        &self,
        selector: &str,
        deadline: Instant,
        audit: Audit,
    ) -> WriteResult<()> {
        if let Some(id) = self.owned_execution(selector) {
            let (result, outcome, reason) = match self.lifecycle.stop(id, deadline, &audit) {
                Ok(Finalized::NoOwnedServer) => (Ok(()), "succeeded", "no_owned_server"),
                Ok(_) => (Ok(()), "succeeded", "owned_server_stopped"),
                Err(error) => {
                    let (reason, client) = stop_failure(error);
                    (Err(client), "failed", reason)
                }
            };
            dev_server_event(&audit, Some(id), DevServerOperation::Stop, outcome, reason);
            return result;
        }
        let id = self.dev_server(selector, DevServerOperation::Stop, &audit)?;
        dev_server_event(
            &audit,
            Some(id),
            DevServerOperation::Stop,
            "succeeded",
            "no_owned_server",
        );
        Ok(())
    }

    /// Owned lifecycle state first (reported even for a since-invalidated
    /// registration); otherwise C4A validation and `stopped`. Launch remains
    /// unavailable and no URL, identifier or path is ever returned.
    fn dev_server_status(&self, selector: &str, audit: Audit) -> WriteResult<Value> {
        if let Some(id) = self.owned_execution(selector) {
            if let Some(status) = self.lifecycle.owned_status(id) {
                dev_server_event(
                    &audit,
                    Some(id),
                    DevServerOperation::Status,
                    "succeeded",
                    "owned_server",
                );
                return Ok(json!({"status": status.label(), "launch_available": false}));
            }
        }
        let id = self.dev_server(selector, DevServerOperation::Status, &audit)?;
        dev_server_event(
            &audit,
            Some(id),
            DevServerOperation::Status,
            "succeeded",
            "no_owned_server",
        );
        Ok(json!({"status": "stopped", "launch_available": false}))
    }

    /// The project key of an execution this registry currently owns. The
    /// selector is only parsed as a key; it confers no other authority.
    fn owned_execution(&self, selector: &str) -> Option<Uuid> {
        let id = Uuid::parse_str(selector).ok()?;
        self.lifecycle.owned_status(id).map(|_| id)
    }

    /// Bounded `shutdown_all` over the authority-owned registry (one overall
    /// deadline). Refuses new executions permanently; never reports success
    /// unless every owned execution was finalized.
    fn shutdown_dev_servers(&self, deadline: Instant, audit: &Audit) -> WriteResult<()> {
        let result = self.lifecycle.shutdown_all(deadline, audit);
        let (outcome, reason) = match result {
            Ok(()) => ("succeeded", "app_exit"),
            Err(_) => ("failed", "not_confirmed"),
        };
        dev_server_event(audit, None, DevServerOperation::Shutdown, outcome, reason);
        result.map_err(|_| "shutdown not confirmed")
    }
}

// Bounded (audit reason, client error) for a lifecycle stop that could not be
// confirmed. Never native errors, identifiers or paths.
fn stop_failure(error: LifecycleError) -> (&'static str, &'static str) {
    match error {
        LifecycleError::CleanupFailed => ("cleanup_failed", "cleanup failed"),
        LifecycleError::NotConfirmed | LifecycleError::StopPending => {
            ("not_confirmed", "stop not confirmed")
        }
        LifecycleError::Unavailable => ("unavailable", "lifecycle unavailable"),
        LifecycleError::NotRegistered
        | LifecycleError::Busy
        | LifecycleError::ShuttingDown
        | LifecycleError::GenerationExhausted
        | LifecycleError::StopRequested
        | LifecycleError::OwnerUnavailable
        | LifecycleError::LaunchFailed
        | LifecycleError::IdentityDenied => ("not_confirmed", "stop not confirmed"),
    }
}

struct WriteExecution<'a> {
    authority: &'a BuilderWorkspaceAuthority,
    project: Arc<RegisteredBuilderProject>,
    root: PathBuf,
    identity: DirectoryIdentity,
    binding: WorkspaceBinding,
    grant: WorkspaceGrantId,
    audit: Audit,
    revoked: bool,
}

impl WriteExecution<'_> {
    fn event(&self, operation: &str, outcome: &str) {
        event(
            &self.audit,
            Some(self.project.project_id),
            &format!("write.{operation}"),
            outcome,
        );
    }

    fn resolve(&self) -> WriteResult<()> {
        let grant = self
            .authority
            .registry
            .resolve(self.grant, self.binding)
            .map_err(|_| "write authority denied")?;
        if grant.permission() != &FsPermissionLevel::ReadWrite || grant.root() != self.root {
            return Err("write authority scope denied");
        }
        Ok(())
    }

    fn target(&self, relative: &str) -> WriteResult<PathBuf> {
        self.resolve()?;
        self.authority
            .catalog
            .validate(&self.project, &self.audit)?;
        self.identity
            .validate(&self.root)
            .map_err(|_| "React identity denied")?;
        validate_relative_file(relative)?;
        let path = resolve_existing_relative(&self.root, Path::new(relative))
            .map_err(|_| "write containment denied")?;
        let parent = path.parent().ok_or("write parent denied")?;
        if !parent.is_dir() || parent.canonicalize().map_err(|_| "write parent denied")? != parent {
            return Err("write parent denied");
        }
        match std::fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.is_file() => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            _ => return Err("write target type denied"),
        }
        Ok(path)
    }

    fn mutate(&self, relative: &str, content: &[u8]) -> WriteResult<()> {
        let resolved = self.resolve();
        self.event(
            "resolve",
            if resolved.is_ok() {
                "authorized"
            } else {
                "denied"
            },
        );
        resolved?;
        let authorized = self.target(relative);
        self.event(
            "authorize",
            if authorized.is_ok() {
                "authorized"
            } else {
                "denied"
            },
        );
        authorized?;
        // Do not reuse the pre-audit path. All authority, lifecycle, identity,
        // parent and target checks run again immediately before the write.
        // Like P0-002A, this is pathname validation, not an atomic namespace or
        // hard-link isolation guarantee against concurrent hostile OS mutation.
        let path = self.target(relative)?;
        std::fs::write(path, content).map_err(|_| "file write failed")
    }

    fn revoke(&mut self) -> WriteResult<()> {
        if self.revoked {
            return Ok(());
        }
        let result = self
            .authority
            .registry
            .revoke(self.grant, self.binding)
            .map_err(|_| "write revocation failed");
        if result.is_ok() {
            self.revoked = true;
        }
        self.event("revoke", if result.is_ok() { "revoked" } else { "failed" });
        result
    }

    fn finish(self, relative: &str, content: &[u8]) -> WriteResult<()> {
        let result = self.mutate(relative, content);
        self.finalize(result)
    }

    fn finalize(mut self, result: WriteResult<()>) -> WriteResult<()> {
        let cleanup = self.revoke();
        let result = cleanup.and(result);
        self.event(
            "complete",
            if result.is_ok() {
                "succeeded"
            } else {
                "failed"
            },
        );
        result
    }
}

impl Drop for WriteExecution<'_> {
    fn drop(&mut self) {
        if !self.revoked {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| self.revoke()));
            if !matches!(result, Ok(Ok(()))) {
                eprintln!("Builder write abnormal-exit revocation failed");
            }
        }
    }
}

pub(super) fn write_file(
    state: &crate::AppState,
    selector: &str,
    relative: &str,
    content: &str,
) -> std::result::Result<(), String> {
    let audit = audit_for(state);
    let authority = state.builder_workspace.as_ref().map_err(|_| {
        event(&audit, None, "write", "denied");
        "Builder write authority unavailable".to_owned()
    })?;
    authority
        .write_file(selector, relative, content.as_bytes(), audit)
        .map_err(|error| format!("Builder write: {error}"))
}

fn dev_server_authority<'a>(
    state: &'a crate::AppState,
    audit: &Audit,
    operation: DevServerOperation,
) -> std::result::Result<&'a BuilderWorkspaceAuthority, String> {
    state.builder_workspace.as_deref().map_err(|_| {
        dev_server_event(audit, None, operation, "denied", "authority_unavailable");
        "Builder dev server: authority unavailable".to_owned()
    })
}

/// Always an error: no process-launch design is approved (P0-002C4A).
pub(super) fn dev_server_start(
    state: &crate::AppState,
    selector: &str,
) -> std::result::Result<String, String> {
    let audit = audit_for(state);
    let authority = dev_server_authority(state, &audit, DevServerOperation::Start)?;
    let denial = authority.dev_server_start(selector, audit);
    Err(format!("Builder dev server: {denial}"))
}

pub(super) fn dev_server_stop(
    state: &crate::AppState,
    selector: &str,
) -> std::result::Result<(), String> {
    let audit = audit_for(state);
    let authority = dev_server_authority(state, &audit, DevServerOperation::Stop)?;
    authority
        .dev_server_stop(selector, audit)
        .map_err(|error| format!("Builder dev server: {error}"))
}

pub(super) fn dev_server_status(
    state: &crate::AppState,
    selector: &str,
) -> std::result::Result<Value, String> {
    let audit = audit_for(state);
    let authority = dev_server_authority(state, &audit, DevServerOperation::Status)?;
    authority
        .dev_server_status(selector, audit)
        .map_err(|error| format!("Builder dev server: {error}"))
}

/// Normal application exit: bounded Builder dev-server shutdown with one
/// overall deadline. Never waits beyond it and never prevents exit; a failure
/// is reported only as bounded stderr and exit continues. Forced termination
/// (SIGKILL, abort, or an immediate process exit call) bypasses this hook.
///
/// Final exit prioritizes process-tree cleanup over audit persistence: the
/// lifecycle receives a quiet sink, so this path never takes the AppState
/// audit lock or writes the audit database (either could exceed the bound).
pub(super) fn shutdown_dev_servers(state: &crate::AppState) {
    let Ok(authority) = state.builder_workspace.as_deref() else {
        // No authority, so no authority-owned registry or execution exists.
        return;
    };
    let quiet: Audit = Arc::new(|_| {});
    let deadline = Instant::now() + DEV_SERVER_SHUTDOWN_WAIT;
    if authority.shutdown_dev_servers(deadline, &quiet).is_err() {
        eprintln!("[shutdown] Builder dev-server cleanup not confirmed");
    }
}

fn audit_for(state: &crate::AppState) -> Audit {
    let state = state.clone();
    Arc::new(move |payload| {
        state.log_event(
            Uuid::nil(),
            nexus_kernel::audit::EventType::UserAction,
            payload,
        );
    })
}

fn validate_root(root: &Path) -> Result<()> {
    if !root.is_absolute() || !root.is_dir() || root.canonicalize().ok().as_deref() != Some(root) {
        return Err(PlanningError::Authority(
            "root missing or canonical identity changed".into(),
        ));
    }
    Ok(())
}

struct PlanningExecution {
    registry: Arc<WorkspaceAuthorityRegistry>,
    project_id: Uuid,
    allocator: WorkspaceBinding,
    planner: WorkspaceBinding,
    allocation: WorkspaceGrantId,
    project: Option<WorkspaceGrantId>,
    root: PathBuf,
    audit: Audit,
    revoked: bool,
    registration: Option<Arc<RegisteredBuilderProject>>,
    catalog: Arc<ProjectCatalog>,
}
impl PlanningExecution {
    fn event(&self, operation: &str, outcome: &str) {
        (self.audit)(json!({"operation": format!("builder.planning.{operation}"),
            "project_id": self.project_id.to_string(), "outcome": outcome}));
    }

    // Returned paths stay within the immediate mutation method, never a lease
    // passed to raw Builder persistence helpers or across provider execution.
    fn authorize(
        &self,
        id: Option<WorkspaceGrantId>,
        binding: WorkspaceBinding,
        root: &Path,
        relative: &Path,
        operation: &str,
    ) -> Result<PathBuf> {
        let checked = (|| {
            let id =
                id.ok_or_else(|| PlanningError::Authority("missing execution grant".into()))?;
            let grant = self
                .registry
                .resolve(id, binding)
                .map_err(|e| PlanningError::Authority(e.to_string()))?;
            if grant.permission() != &FsPermissionLevel::ReadWrite || grant.root() != root {
                return Err(PlanningError::Authority(
                    "mutation permission or root mismatch".into(),
                ));
            }
            validate_root(root)?;
            resolve_existing_relative(root, relative)
                .map_err(|_| PlanningError::Authority("relative target denied".into()))
        })();
        // Registry methods have returned; no registry/private state guard exists.
        self.event(
            operation,
            if checked.is_ok() {
                "authorized"
            } else {
                "denied"
            },
        );
        checked
    }

    fn project_target(&self, relative: &str, operation: &str) -> Result<PathBuf> {
        self.authorize(
            self.project,
            self.planner,
            &self.root,
            Path::new(relative),
            operation,
        )
    }

    fn provider_ready(&self) -> Result<()> {
        self.project_target("builder_state.json", "provider")?;
        Ok(())
    }

    fn create_artefacts(&self) -> Result<()> {
        let path = self.project_target("artefacts", "mkdir.artefacts")?;
        std::fs::create_dir(path).map_err(|e| {
            PlanningError::Persistence(format!("artefacts creation failed: {}", e.kind()))
        })
    }

    fn write(&self, relative: &str, bytes: &[u8]) -> Result<()> {
        let path = self.project_target(relative, relative)?;
        std::fs::write(path, bytes).map_err(|e| {
            PlanningError::Persistence(format!("{relative} write failed: {}", e.kind()))
        })
    }

    fn save_state(&self, state: &ProjectState) -> Result<()> {
        let bytes = serde_json::to_vec_pretty(state)
            .map_err(|e| PlanningError::Persistence(e.to_string()))?;
        self.write("builder_state.json", &bytes)
    }

    fn persist_plan(&self, generated: &PlanResult, state: &mut ProjectState) -> Result<()> {
        let brief = serde_json::to_vec_pretty(&generated.plan.product_brief)
            .map_err(|e| PlanningError::Persistence(e.to_string()))?;
        let criteria = serde_json::to_vec_pretty(&generated.plan.acceptance_criteria)
            .map_err(|e| PlanningError::Persistence(e.to_string()))?;
        self.create_artefacts()?;
        self.write("artefacts/product_brief.json", &brief)?;
        self.write("artefacts/acceptance_criteria.json", &criteria)?;
        state.project_name = Some(generated.plan.product_brief.project_name.clone());
        state.plan_cost = generated.cost_usd;
        state.total_cost += generated.cost_usd;
        transition(state, ProjectStatus::Planned).map_err(PlanningError::Persistence)?;
        self.save_state(state)
    }

    fn revoke(&mut self) -> Result<()> {
        if self.revoked {
            return Ok(());
        }
        let result = self
            .registry
            .revoke(self.allocation, self.allocator)
            .map_err(|e| PlanningError::Authority(format!("cleanup failed: {e}")));
        if result.is_ok() {
            self.revoked = true;
        }
        self.event("revoke", if result.is_ok() { "revoked" } else { "denied" });
        result
    }
}
impl Drop for PlanningExecution {
    fn drop(&mut self) {
        if !self.revoked {
            // Audit may be a callback; never allow it to double-panic on unwind.
            let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| self.revoke()));
            if !matches!(outcome, Ok(Ok(()))) {
                eprintln!("Builder planning abnormal-exit revocation failed");
            }
        }
    }
}

struct GeneratedPlan {
    result: PlanResult,
    selection: ModelSelection,
}
struct CompletedPlan {
    project_id: String,
    project_dir: String,
    generated: GeneratedPlan,
}
impl CompletedPlan {
    fn into_json(self) -> Value {
        let r = self.generated.result;
        let m = self.generated.selection;
        json!({"project_id": self.project_id, "project_dir": self.project_dir,
            "plan": r.plan, "input_tokens": r.input_tokens, "output_tokens": r.output_tokens,
            "cost_usd": r.cost_usd, "elapsed_seconds": r.elapsed_seconds,
            "model": m.display_name, "model_id": m.model_id, "provider": m.provider.to_string(), "is_local": m.is_local})
    }
}

fn run_plan(
    authority: &BuilderWorkspaceAuthority,
    audit: Audit,
    prompt: &str,
    attempt: impl FnMut(bool) -> std::result::Result<GeneratedPlan, String>,
) -> Result<CompletedPlan> {
    let execution = authority.begin(Arc::clone(&audit)).inspect_err(|_| {
        audit(json!({"operation": "builder.planning.begin", "outcome": "denied"}));
    })?;
    finish_plan(execution, prompt, attempt)
}

fn finish_plan(
    mut execution: PlanningExecution,
    prompt: &str,
    mut attempt: impl FnMut(bool) -> std::result::Result<GeneratedPlan, String>,
) -> Result<CompletedPlan> {
    let result = (|| {
        let mut state = create_project(&execution.project_id.to_string(), prompt);
        execution.provider_ready()?;
        // Only these closures can produce provider errors. Authority and I/O
        // errors never enter this fallback branch.
        let generated = match attempt(false) {
            Ok(value) => Ok(value),
            Err(_) => {
                execution.provider_ready()?;
                attempt(true)
            }
        };
        match generated {
            Ok(generated) => {
                execution.persist_plan(&generated.result, &mut state)?;
                Ok(CompletedPlan {
                    project_id: execution.project_id.to_string(),
                    project_dir: execution.root.to_string_lossy().into_owned(),
                    generated,
                })
            }
            Err(error) => {
                state.error_message = Some(error.clone());
                transition(&mut state, ProjectStatus::PlanFailed)
                    .map_err(PlanningError::Persistence)?;
                execution.save_state(&state)?;
                Err(PlanningError::Provider(error))
            }
        }
    })();
    // Finalization precedes success, error delivery, and cost recording.
    let cleanup = execution.revoke();
    if cleanup.is_err() || result.is_err() {
        event(
            &execution.audit,
            Some(execution.project_id),
            "registration",
            "denied",
        );
    }
    cleanup?;
    if result.is_ok() {
        let registration = execution
            .registration
            .as_ref()
            .ok_or_else(|| PlanningError::Authority("missing allocation identity".into()))?;
        let published = execution.catalog.publish(Arc::clone(registration));
        event(
            &execution.audit,
            Some(execution.project_id),
            "registration",
            if published.is_ok() {
                "registered"
            } else {
                "denied"
            },
        );
        published.map_err(|error| PlanningError::Authority(error.into()))?;
    }
    execution.event(
        "complete",
        if result.is_ok() {
            "succeeded"
        } else {
            "failed"
        },
    );
    result
}

pub(super) fn generate_plan(
    state: &crate::AppState,
    prompt: &str,
) -> std::result::Result<Value, String> {
    let authority = state.builder_workspace.as_ref().map_err(Clone::clone)?;
    let audit = audit_for(state);
    let completed = run_plan(authority, audit, prompt, |fallback| {
        generate_with_provider(prompt, fallback)
    })
    .map_err(|e| e.to_string())?;
    // Existing global budget store is separate from project authority.
    web_builder_agent::plan::record_plan_cost(
        &completed.generated.result,
        &completed.generated.result.plan.product_brief.project_name,
    );
    Ok(completed.into_json())
}

fn generate_with_provider(
    prompt: &str,
    fallback: bool,
) -> std::result::Result<GeneratedPlan, String> {
    use nexus_connectors_llm::providers::{
        claude_code::ClaudeCodeProvider, codex_cli::CodexCliProvider, LlmProvider,
    };
    use web_builder_agent::model_router::*;
    let config = crate::load_config().map_err(|e| format!("config error: {e}"))?;
    let provider_config = crate::build_provider_config(&config);
    let (provider, selection): (Box<dyn LlmProvider>, ModelSelection) = if fallback {
        let selected = select_model(
            &BuilderTask::PlanGeneration,
            &RoutingBudget::from_budget_tracker(),
        );
        let provider: Box<dyn LlmProvider> = match selected.provider {
            ProviderType::Ollama => Box::new(crate::OllamaProvider::from_env()),
            ProviderType::Anthropic => {
                let (provider, _) = crate::provider_from_prefixed_model(
                    &format!("anthropic/{}", selected.model_id),
                    &provider_config,
                )?;
                provider
            }
            ProviderType::OpenAI => Box::new(crate::OpenAiProvider::new(
                provider_config.openai_api_key.clone(),
            )),
            ProviderType::CodexCli => Box::new(CodexCliProvider::new()),
            ProviderType::ClaudeCode => Box::new(ClaudeCodeProvider::new()),
        };
        (provider, selected)
    } else {
        let model = web_builder_agent::model_config::load_config().planning;
        let prefixed = web_builder_agent::model_config::to_prefixed_model(&model);
        let (provider, _) = crate::provider_from_prefixed_model(&prefixed, &provider_config)?;
        let selection = ModelSelection {
            provider: match model.provider.as_str() {
                "anthropic_api" | "anthropic" => ProviderType::Anthropic,
                "openai_api" | "openai" => ProviderType::OpenAI,
                "codex_cli" => ProviderType::CodexCli,
                "claude_cli" => ProviderType::ClaudeCode,
                _ => ProviderType::Ollama,
            },
            model_id: model.model_id,
            display_name: model.display_name,
            estimated_cost: 0.0,
            is_local: model.provider == "ollama",
        };
        (provider, selection)
    };
    let result = web_builder_agent::plan::generate_plan_with_model(
        provider.as_ref(),
        prompt,
        &selection.model_id,
    )?;
    Ok(GeneratedPlan { result, selection })
}

#[cfg(test)]
mod tests;
