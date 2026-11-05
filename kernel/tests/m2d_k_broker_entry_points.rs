//! M2d-K: the kernel entry points the broker's Files staging (design v2
//! §6.1 step 4, built in M2d-B2) and its startup recovery report (§6.7)
//! need, used exactly as the broker can use them: through `nexus_kernel`
//! paths only. The broker depends on neither the persistence crate nor the
//! crate behind the kernel's identifiers, and this file names neither (the
//! last test checks its own text). The temporary directories (`tempfile`)
//! are fixture scaffolding only.

use nexus_kernel::workspace_authority::{
    WorkspaceAuthorityError, WorkspaceAuthorityRegistry, WorkspaceAuthoritySource, WorkspaceBinding,
};
use nexus_kernel::workspace_permissions::FsPermissionLevel;

/// The broker's own agent identity bytes, and a broker run's.
const AGENT: [u8; 16] = [0x4d; 16];
const RUN: [u8; 16] = [0x52; 16];
const OTHER: [u8; 16] = [0x6f; 16];

#[test]
fn m2d_k_a_a_binding_from_bytes_names_exactly_those_identities() {
    let binding = WorkspaceBinding::from_bytes(AGENT, RUN);
    assert_eq!(binding.agent_id.into_bytes(), AGENT);
    assert_eq!(binding.run_id.into_bytes(), RUN);
    assert_eq!(binding, WorkspaceBinding::from_bytes(AGENT, RUN));
    assert_ne!(binding, WorkspaceBinding::from_bytes(RUN, AGENT));
}

#[test]
fn m2d_k_a_a_grant_to_a_bytes_binding_resolves_only_for_that_binding() {
    let dir = tempfile::tempdir().unwrap();
    let registry = WorkspaceAuthorityRegistry::new();
    let issue = |binding| {
        registry.issue_trusted_root(
            dir.path(),
            binding,
            WorkspaceAuthoritySource::BackendAllocated,
            FsPermissionLevel::ReadOnly,
            None,
        )
    };
    let grant = issue(WorkspaceBinding::from_bytes(AGENT, RUN)).unwrap();
    assert!(registry
        .resolve(grant, WorkspaceBinding::from_bytes(AGENT, RUN))
        .is_ok());
    assert_eq!(
        registry
            .resolve(grant, WorkspaceBinding::from_bytes(OTHER, RUN))
            .unwrap_err(),
        WorkspaceAuthorityError::WrongOwner
    );
    assert_eq!(
        registry
            .resolve(grant, WorkspaceBinding::from_bytes(AGENT, OTHER))
            .unwrap_err(),
        WorkspaceAuthorityError::WrongContext
    );
    // Bytes are only a name: a nil identity is still refused at issuance.
    for nil in [
        WorkspaceBinding::from_bytes([0; 16], RUN),
        WorkspaceBinding::from_bytes(AGENT, [0; 16]),
    ] {
        assert_eq!(
            issue(nil).unwrap_err(),
            WorkspaceAuthorityError::InvalidBinding
        );
    }
}

#[cfg(target_os = "linux")]
mod linux {
    use super::{AGENT, RUN};
    use nexus_kernel::coding_run::{
        analyze_apply, make_non_dumpable, ApplyFinding, ApplyInterrupt, CandidateEdit, ChangeKind,
        CodingRun, ConfirmationKind, ConfirmationRequest, FolderPicker, InterruptCause,
        LedgerFailure, LedgerFile, OwnerConfirmer, ProjectInfo, ProjectRegistry, RelPath,
        RunScopes, ScopeEntry, ScopeSet, StagingParent,
    };
    use nexus_kernel::emergency_stop::StopAuthority;
    use nexus_kernel::workspace_authority::{WorkspaceAuthorityRegistry, WorkspaceBinding};
    use std::path::{Path, PathBuf};
    use std::sync::Arc;

    const NAME: &str = "notes.txt";
    const CONTENT: &str = "first line\nsecond line\n";

    struct Pick(PathBuf);

    impl FolderPicker for Pick {
        fn pick_folder(&self) -> Option<PathBuf> {
            Some(self.0.clone())
        }
    }

    /// The Owner's yes, for an apply only.
    struct Yes;

    impl OwnerConfirmer for Yes {
        fn confirm(&self, request: &ConfirmationRequest) -> bool {
            request.kind == ConfirmationKind::Apply
        }
    }

    /// An execution guard with nothing interrupting.
    struct Clear;

    impl ApplyInterrupt for Clear {
        fn interrupted(&self) -> Option<InterruptCause> {
            None
        }
    }

    /// The broker's backend state (ledger, staging) and the Owner's empty
    /// workspace, registered with the kernel with the state reserved.
    struct Broker {
        _tmp: tempfile::TempDir,
        root: PathBuf,
        state: PathBuf,
        ledger_path: PathBuf,
        projects: ProjectRegistry,
        info: ProjectInfo,
        stop: Arc<StopAuthority>,
    }

    fn broker() -> Broker {
        let tmp = tempfile::tempdir().unwrap();
        let base = tmp.path().canonicalize().unwrap();
        let (root, state) = (base.join("workspace"), base.join("state"));
        std::fs::create_dir(&root).unwrap();
        std::fs::create_dir(&state).unwrap();
        let projects =
            ProjectRegistry::reserving(Arc::new(WorkspaceAuthorityRegistry::new()), &[&state]);
        let info = projects.select(&Pick(root.clone())).unwrap();
        Broker {
            _tmp: tmp,
            ledger_path: state.join("files-ledger.db"),
            root,
            state,
            projects,
            info,
            stop: StopAuthority::new(),
        }
    }

    fn scopes() -> RunScopes {
        RunScopes::new(
            ScopeSet::new([ScopeEntry::WholeProject]),
            ScopeSet::new([ScopeEntry::File(RelPath::parse(NAME).unwrap())]),
            ScopeSet::default(),
        )
        .unwrap()
    }

    /// §6.1 step 4 up to the review: a run observing the broker's stop,
    /// granted, snapshotted, with one created file, structurally verified.
    fn staged(b: &Broker, ledger: &LedgerFile) -> (CodingRun, WorkspaceBinding) {
        let binding = WorkspaceBinding::from_bytes(AGENT, RUN);
        let grant = b.projects.grant_for_run(b.info.id, binding).unwrap();
        let mut run =
            CodingRun::create_for_project_observing(ledger.store(), grant, scopes(), &b.stop)
                .unwrap();
        run.grant(&StagingParent::within(&b.state).unwrap())
            .unwrap();
        run.snapshot().unwrap();
        run.edit(CandidateEdit::Create {
            path: RelPath::parse(NAME).unwrap(),
            content: CONTENT.as_bytes().to_vec(),
        })
        .unwrap();
        assert!(run.verify_structural().unwrap().passed());
        (run, binding)
    }

    #[test]
    fn m2d_k_b_the_files_staging_step_and_its_report_use_kernel_paths_only() {
        assert!(make_non_dumpable(), "the host's startup step");
        let b = broker();
        let ledger = LedgerFile::open(&b.ledger_path).unwrap();
        let (mut run, binding) = staged(&b, &ledger);
        let review = run.review().unwrap();
        let only = review.single_complete().expect("one new file, shown whole");
        assert_eq!(only.path, RelPath::parse(NAME).unwrap());
        assert_eq!(only.kind, ChangeKind::Create);

        // The startup report's view: the run is listed, awaiting approval.
        let id = run.id();
        assert_eq!(ledger.run_ids().unwrap(), vec![id]);
        let report =
            |ledger: &LedgerFile| analyze_apply(ledger.store().as_ref(), id.ledger_key()).unwrap();
        assert_eq!(report(&ledger), vec![ApplyFinding::StagingAwaitingApproval]);

        // Execute's view: one publication of exactly the reviewed bytes.
        assert!(std::fs::read_dir(&b.root).unwrap().next().is_none());
        let approval = run.request_approval(&b.info.name, &Yes).unwrap();
        let write = b.projects.grant_for_apply(b.info.id, binding).unwrap();
        let staging = StagingParent::within(&b.state).unwrap();
        run.apply_observed(approval, write, &staging, &Clear)
            .unwrap();
        assert_eq!(
            std::fs::read(b.root.join(NAME)).unwrap(),
            CONTENT.as_bytes()
        );
        assert!(report(&ledger).is_empty(), "published and recorded");

        // A second handle on the same file (a restart) sees the same run.
        drop(run);
        drop(ledger);
        let reopened = LedgerFile::open(&b.ledger_path).unwrap();
        assert_eq!(reopened.run_ids().unwrap(), vec![id]);
    }

    #[test]
    fn m2d_k_c_a_ledger_file_is_only_an_absolute_file_backed_database() {
        let b = broker();
        for refused in [
            PathBuf::from("files-ledger.db"),
            PathBuf::from(":memory:"),
            PathBuf::from("file:files-ledger.db"),
            b.state.clone(),
        ] {
            assert_eq!(
                LedgerFile::open(&refused).unwrap_err(),
                LedgerFailure::Unavailable,
                "{}",
                refused.display()
            );
        }
        assert!(LedgerFile::open(&b.ledger_path)
            .unwrap()
            .run_ids()
            .unwrap()
            .is_empty());
    }

    /// Replace every occurrence of `from` with `to` (same length) in `path`.
    fn tamper(path: &Path, from: &str, to: &str) -> usize {
        assert_eq!(from.len(), to.len());
        let mut bytes = std::fs::read(path).unwrap();
        let mut hits = 0;
        let mut at = 0;
        while let Some(found) = bytes[at..]
            .windows(from.len())
            .position(|window| window == from.as_bytes())
        {
            let start = at + found;
            bytes[start..start + to.len()].copy_from_slice(to.as_bytes());
            hits += 1;
            at = start + to.len();
        }
        std::fs::write(path, bytes).unwrap();
        hits
    }

    #[test]
    fn m2d_k_c_a_stored_run_id_that_is_not_canonical_is_an_integrity_failure() {
        let b = broker();
        let ledger = LedgerFile::open(&b.ledger_path).unwrap();
        let (run, _) = staged(&b, &ledger);
        let id = run.id().to_string();
        drop(run);
        drop(ledger);
        // Fixture precondition: closing the last handle checkpointed the
        // log into the database file itself.
        let mut wal = b.ledger_path.clone().into_os_string();
        wal.push("-wal");
        assert!(!Path::new(&wal).exists(), "the log was checkpointed");
        // The same length, and never canonical: `g` is not a hex digit.
        let malformed = format!("g{}", &id[1..]);
        assert!(
            tamper(&b.ledger_path, &id, &malformed) > 0,
            "the id is stored"
        );
        assert_eq!(
            LedgerFile::open(&b.ledger_path)
                .unwrap()
                .run_ids()
                .unwrap_err(),
            LedgerFailure::Integrity,
            "never skipped, never merely unavailable"
        );
    }
}

#[test]
fn m2d_k_d_this_file_names_no_dependency_the_broker_lacks() {
    let source = include_str!("m2d_k_broker_entry_points.rs").to_ascii_lowercase();
    for needle in [concat!("uu", "id"), concat!("nexus_", "persistence")] {
        assert!(!source.contains(needle), "this file names {needle}");
    }
}
