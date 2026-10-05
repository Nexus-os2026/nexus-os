//! Phase One P1-04 tests: backend-owned project registration. The picker is
//! a test stand-in for the desktop's native dialog; what it returns is what
//! a native dialog would hand the backend. `p1_p_nc_*` are negative controls.

use super::*;
use std::cell::Cell;

/// A native picker stand-in that returns one fixed choice.
struct Picker(Option<PathBuf>, Cell<usize>);

impl Picker {
    fn choose(path: &Path) -> Self {
        Self(Some(path.to_path_buf()), Cell::new(0))
    }
}

impl FolderPicker for Picker {
    fn pick_folder(&self) -> Option<PathBuf> {
        self.1.set(self.1.get() + 1);
        self.0.clone()
    }
}

struct Projects {
    f: Fixture,
    state: PathBuf,
    registry: ProjectRegistry,
}

fn projects() -> Projects {
    let f = fixture();
    let state = f.staging_parent.parent().unwrap().join("nexus-state");
    std::fs::create_dir(&state).unwrap();
    let registry = ProjectRegistry::new(Arc::clone(&f.registry), &state);
    Projects { f, state, registry }
}

fn binding() -> WorkspaceBinding {
    WorkspaceBinding {
        agent_id: Uuid::new_v4(),
        run_id: Uuid::new_v4(),
    }
}

fn run_for(p: &Projects, grant: ProjectGrant) -> CodingRun {
    CodingRun::create_for_project(
        Arc::clone(&p.f.ledger) as Arc<dyn LedgerStore>,
        grant,
        default_scopes(),
    )
    .unwrap()
}

// ── Positive behaviour ──────────────────────────────────────────────────────

#[test]
fn p1_p_01_native_selection_registers_and_each_run_gets_a_fresh_read_grant() {
    let p = projects();
    let picker = Picker::choose(&p.f.project);
    let info = p.registry.select(&picker).unwrap();
    assert_eq!(picker.1.get(), 1, "the backend invoked the picker");
    assert_eq!(info.name, "project");
    assert_eq!(p.registry.info(info.id), Some(info.clone()));
    assert_eq!(p.registry.list(), vec![info.clone()]);
    // Selecting the same folder again yields the same registration.
    assert_eq!(p.registry.select(&picker).unwrap().id, info.id);

    let (a, b) = (binding(), binding());
    let grant_a = p.registry.grant_for_run(info.id, a).unwrap();
    let grant_b = p.registry.grant_for_run(info.id, b).unwrap();
    assert_ne!(grant_a.grant_id(), grant_b.grant_id());
    let resolved = p.f.registry.resolve(grant_a.grant_id(), a).unwrap();
    assert_eq!(resolved.source(), WorkspaceAuthoritySource::UserSelected);
    assert_eq!(
        *resolved.permission(),
        FsPermissionLevel::ReadOnly,
        "registration implies no write permission"
    );
    assert!(resolved.expires_at().is_some());

    let before = digest(&p.f.project);
    let mut run = run_for(&p, grant_a);
    assert_eq!(run.project_id(), Some(info.id));
    run.grant(&parent(&p.f)).unwrap();
    run.snapshot().unwrap();
    run.edit(replace("src/lib.rs", "pub fn answer() -> u32 { 42 }\n"))
        .unwrap();
    assert!(run.verify_structural().unwrap().passed());
    assert_eq!(digest(&p.f.project), before);
    let created = &p.f.ledger.verify_run(run.id().ledger_key()).unwrap()[0];
    assert!(created.payload.contains(&info.id.to_string()));

    // Revoking one run's grant leaves the other run's grant intact.
    p.f.registry.revoke(grant_b.grant_id(), b).unwrap();
    assert!(p.f.registry.resolve(grant_b.grant_id(), b).is_err());
    let grant_c = p.registry.grant_for_run(info.id, binding()).unwrap();
    assert!(p
        .f
        .registry
        .resolve(grant_c.grant_id(), grant_c.binding())
        .is_ok());
}

#[test]
fn p1_p_02_apply_grants_are_separate_short_lived_and_writable() {
    let p = projects();
    let info = p.registry.select(&Picker::choose(&p.f.project)).unwrap();
    let b = binding();
    let apply = p.registry.grant_for_apply(info.id, b).unwrap();
    let resolved = p.f.registry.resolve(apply.grant_id(), b).unwrap();
    assert_eq!(*resolved.permission(), FsPermissionLevel::ReadWrite);
    let lifetime = resolved
        .expires_at()
        .unwrap()
        .duration_since(SystemTime::now())
        .unwrap();
    assert!(lifetime <= APPLY_GRANT_LIFETIME);
}

// ── Negative controls ───────────────────────────────────────────────────────

#[test]
fn p1_p_nc_01_only_a_native_pick_registers_and_bad_picks_are_refused() {
    let p = projects();
    assert_eq!(
        p.registry.select(&Picker(None, Cell::new(0))),
        Err(ProjectError::Cancelled)
    );
    assert_eq!(
        p.registry
            .select(&Picker::choose(Path::new("relative/project"))),
        Err(ProjectError::NotAbsolute)
    );
    assert_eq!(
        p.registry
            .select(&Picker::choose(&p.f.project.join("src/lib.rs"))),
        Err(ProjectError::NotADirectory)
    );
    assert_eq!(
        p.registry
            .select(&Picker::choose(&p.f.project.join("missing"))),
        Err(ProjectError::Unavailable)
    );
    // A non-normalized spelling is not the canonical folder.
    assert_eq!(
        p.registry
            .select(&Picker::choose(&p.f.project.join("src/.."))),
        Err(ProjectError::NotCanonical)
    );
    assert!(p.registry.list().is_empty());
}

#[test]
fn p1_p_nc_02_symlinked_roots_are_refused() {
    let p = projects();
    let tmp = p.f.project.parent().unwrap();
    let link = tmp.join("link-to-project");
    std::os::unix::fs::symlink(&p.f.project, &link).unwrap();
    assert_eq!(
        p.registry.select(&Picker::choose(&link)),
        Err(ProjectError::NotCanonical)
    );
    // A symlinked ancestor is a redirect too.
    let linked_parent = tmp.join("linked-parent");
    std::os::unix::fs::symlink(tmp, &linked_parent).unwrap();
    assert_eq!(
        p.registry
            .select(&Picker::choose(&linked_parent.join("project"))),
        Err(ProjectError::NotCanonical)
    );
    assert!(p.registry.list().is_empty());
}

#[test]
fn p1_p_nc_03_a_replaced_root_is_denied() {
    let p = projects();
    let info = p.registry.select(&Picker::choose(&p.f.project)).unwrap();
    let moved = p.f.project.with_file_name("moved-away");

    // Replaced before a grant is issued.
    std::fs::rename(&p.f.project, &moved).unwrap();
    std::fs::create_dir(&p.f.project).unwrap();
    assert_eq!(
        p.registry.grant_for_run(info.id, binding()).unwrap_err(),
        ProjectError::IdentityChanged
    );
    std::fs::remove_dir(&p.f.project).unwrap();
    std::fs::rename(&moved, &p.f.project).unwrap();

    // Replaced after the grant, before the run opens the folder.
    let grant = p.registry.grant_for_run(info.id, binding()).unwrap();
    std::fs::rename(&p.f.project, &moved).unwrap();
    std::fs::create_dir(&p.f.project).unwrap();
    let mut run = run_for(&p, grant);
    assert_eq!(run.grant(&parent(&p.f)), Err(RunError::IdentityChanged));
    assert_eq!(
        run.state(),
        RunState::Failed(FailureReason::IdentityChanged)
    );

    // Replaced by a symlink to the original.
    std::fs::remove_dir(&p.f.project).unwrap();
    std::os::unix::fs::symlink(&moved, &p.f.project).unwrap();
    assert!(p.registry.grant_for_run(info.id, binding()).is_err());
}

#[test]
fn p1_p_nc_04_stale_and_foreign_project_ids_are_denied() {
    let p = projects();
    let info = p.registry.select(&Picker::choose(&p.f.project)).unwrap();
    let unknown = ProjectId::parse(&Uuid::new_v4().to_string()).unwrap();
    assert_eq!(
        p.registry.grant_for_run(unknown, binding()).unwrap_err(),
        ProjectError::UnknownProject
    );
    // An id registered in another registry (another process) is unknown here.
    let other = ProjectRegistry::new(Arc::clone(&p.f.registry), &p.state);
    assert_eq!(
        other.grant_for_run(info.id, binding()).unwrap_err(),
        ProjectError::UnknownProject
    );
    // A forgotten registration is stale.
    assert!(p.registry.forget(info.id));
    assert_eq!(
        p.registry.grant_for_run(info.id, binding()).unwrap_err(),
        ProjectError::UnknownProject
    );
    assert!(ProjectId::parse("not-a-uuid").is_none());
    assert!(ProjectId::parse(&Uuid::nil().to_string()).is_none());
}

#[test]
fn p1_p_nc_05_a_run_grant_cannot_serve_another_run() {
    let p = projects();
    let info = p.registry.select(&Picker::choose(&p.f.project)).unwrap();
    let (a, b) = (binding(), binding());
    let grant = p.registry.grant_for_run(info.id, a).unwrap();
    assert!(p.f.registry.resolve(grant.grant_id(), b).is_err());
    let mut stolen = CodingRun::create(
        Arc::clone(&p.f.ledger) as Arc<dyn LedgerStore>,
        Arc::clone(&p.f.registry),
        grant.grant_id(),
        b,
        default_scopes(),
    )
    .unwrap();
    assert_eq!(stolen.grant(&parent(&p.f)), Err(RunError::AuthorityDenied));
    assert_eq!(
        stolen.state(),
        RunState::Failed(FailureReason::AuthorityDenied)
    );
}

#[test]
fn p1_p_nc_06_a_serialized_project_id_restores_nothing() {
    let p = projects();
    let info = p.registry.select(&Picker::choose(&p.f.project)).unwrap();
    let text = info.id.to_string();
    // A "restarted" process: a fresh registry over the same authority.
    let restarted = ProjectRegistry::new(Arc::clone(&p.f.registry), &p.state);
    let parsed = ProjectId::parse(&text).unwrap();
    assert_eq!(parsed, info.id);
    assert!(restarted.info(parsed).is_none());
    assert_eq!(
        restarted.grant_for_run(parsed, binding()).unwrap_err(),
        ProjectError::UnknownProject
    );
    assert!(restarted.list().is_empty());
}

#[test]
fn p1_p_nc_07_system_and_nexus_state_roots_are_forbidden() {
    let p = projects();
    for forbidden in [
        PathBuf::from("/"),
        p.state.clone(),
        p.state.parent().unwrap().to_path_buf(),
    ] {
        assert_eq!(
            p.registry.select(&Picker::choose(&forbidden)),
            Err(ProjectError::Forbidden),
            "{}",
            forbidden.display()
        );
    }
    let inside = p.state.join("inner");
    std::fs::create_dir(&inside).unwrap();
    assert_eq!(
        p.registry.select(&Picker::choose(&inside)),
        Err(ProjectError::Forbidden)
    );
}

/// XA-L-01: a reserved backend location (such as the verifier's workspaces)
/// is refused like the state directory: the location itself, anything inside
/// it and anything containing it. It is compared by path components, not by
/// string prefix, and its canonical spelling is reserved as well. Without the
/// reservation the same directory registers, which is the gap the desktop
/// closes by reserving the verifier workspaces.
#[test]
fn p1_p_nc_08_reserved_backend_locations_are_forbidden() {
    let p = projects();
    let base = p.state.parent().unwrap().to_path_buf();
    let runtime = base.join("runtime");
    let workspaces = runtime.join("nexus-verifier");
    std::fs::create_dir_all(workspaces.join("ws-1").join("input")).unwrap();
    let registry = ProjectRegistry::reserving(Arc::clone(&p.f.registry), &[&p.state, &workspaces]);
    for forbidden in [
        workspaces.clone(),
        workspaces.join("ws-1"),
        workspaces.join("ws-1").join("input"),
        runtime.clone(),
        p.state.clone(),
    ] {
        assert_eq!(
            registry.select(&Picker::choose(&forbidden)),
            Err(ProjectError::Forbidden),
            "{}",
            forbidden.display()
        );
    }
    for allowed in [runtime.join("nexus-verifier-other"), runtime.join("other")] {
        std::fs::create_dir(&allowed).unwrap();
        assert!(
            registry.select(&Picker::choose(&allowed)).is_ok(),
            "{}",
            allowed.display()
        );
    }
    let link = base.join("runtime-link");
    std::os::unix::fs::symlink(&runtime, &link).unwrap();
    let by_link = ProjectRegistry::reserving(
        Arc::clone(&p.f.registry),
        &[&p.state, &link.join("nexus-verifier")],
    );
    assert_eq!(
        by_link.select(&Picker::choose(&workspaces.join("ws-1"))),
        Err(ProjectError::Forbidden)
    );
    assert!(p.registry.select(&Picker::choose(&workspaces)).is_ok());
}
