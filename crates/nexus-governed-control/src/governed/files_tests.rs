//! M2d-B1 oracles for the broker's Files security core (design v2 §6.1-6.2,
//! §6.10 D-O12, D-O15, D-O18, D-O21, D-O23; §11 H1′ binding), pre-registered
//! before it exists: the door's consent table and its lifetime (a yes is
//! cached with the stop epoch, the door's halt state and closure; Stop and
//! halt invalidate every entry; a late confirmation cannot repopulate it;
//! the 65th entry is refused predictably), the run's sticky Files admission
//! (validated in one step with live membership, an active run, the halt,
//! Stop and closure), workspace selection, and prepare's refusals.
//!
//! Every race is a forced order: a named broker hook runs the interfering
//! call at its place, or the Owner's dialog runs it while it is open. Each
//! refusal has a positive control on the same fixture.

use super::{AgentDoor, AgentOutcome, GovernedControl, GrantRequest, Intent};
use crate::authority::approval::{
    AcknowledgementConfirmation, ActionConfirmation, AgentDoorConfirmation, ControlConfirmer,
    GrantConfirmation, MissionConfirmation, MissionResumeConfirmation, ResumeConfirmation,
};
use crate::authority::clock::SystemClock;
use crate::authority::evidence::{EvidencePhase, MemoryEvidence};
use crate::authority::ids::{AgentId, RunId};
use crate::authority::run::{ConsentKind, FilesAdmission};
use crate::authority::AuthorityError;
use crate::broker::Vault;
use crate::files::{Exclusions, FileCreate, WorkspaceInfo};
use crate::governed::HaltReason;
use crate::harness_tests::{temp_root, TempRoot};
use crate::test_hooks::at;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

const HASH: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
const HASH2: &str = "fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210";

fn label(n: u32) -> String {
    format!("msn-000001/1/{n}")
}

fn mother() -> AgentId {
    AgentId::new("mother").unwrap()
}

// ---------------------------------------------------------------- owner

type During = Box<dyn FnOnce() + Send>;

/// The Owner: yes to every dialog; counts the mission and held-mission
/// dialogs; runs `during` once inside the next mission or held dialog.
#[derive(Default)]
struct Owner {
    missions: AtomicUsize,
    helds: AtomicUsize,
    during: Mutex<Option<During>>,
}

impl Owner {
    fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    fn during(&self, f: impl FnOnce() + Send + 'static) {
        *self.during.lock().unwrap() = Some(Box::new(f));
    }

    fn fire(&self) {
        let f = self.during.lock().unwrap().take();
        if let Some(f) = f {
            f();
        }
    }

    fn missions(&self) -> usize {
        self.missions.load(Ordering::SeqCst)
    }
}

impl ControlConfirmer for Owner {
    fn confirm_action(&self, _: &ActionConfirmation) -> bool {
        true
    }
    fn confirm_grant(&self, _: &GrantConfirmation) -> bool {
        true
    }
    fn confirm_resume(&self, _: &ResumeConfirmation) -> bool {
        true
    }
    fn confirm_acknowledgement(&self, _: &AcknowledgementConfirmation) -> bool {
        true
    }
    fn confirm_mission(&self, _: &MissionConfirmation) -> bool {
        self.missions.fetch_add(1, Ordering::SeqCst);
        self.fire();
        true
    }
    fn confirm_agent_door(&self, _: &AgentDoorConfirmation) -> bool {
        true
    }
    fn confirm_held_mission(&self, _: &MissionResumeConfirmation) -> bool {
        self.helds.fetch_add(1, Ordering::SeqCst);
        self.fire();
        true
    }
}

// ---------------------------------------------------------------- dirs

static NEXT_DIR: AtomicUsize = AtomicUsize::new(0);

/// A private temporary tree, canonical, removed when dropped:
/// `home/` (the test's `$HOME`), `ident/` (its identity home), `reserved/`
/// (a reserved tree) and `home/ws/` (an empty workspace in `$HOME`).
struct Dirs {
    base: PathBuf,
}

impl Dirs {
    fn new() -> Self {
        let base = std::env::temp_dir().join(format!(
            "nexus-m2d-b1-{}-{}",
            std::process::id(),
            NEXT_DIR.fetch_add(1, Ordering::SeqCst)
        ));
        for sub in ["home/ws", "ident", "reserved"] {
            std::fs::create_dir_all(base.join(sub)).unwrap();
        }
        let base = std::fs::canonicalize(&base).unwrap();
        Self { base }
    }

    fn path(&self, sub: &str) -> PathBuf {
        self.base.join(sub)
    }

    fn ws(&self) -> PathBuf {
        self.path("home/ws")
    }

    fn exclusions(&self) -> Exclusions {
        Exclusions {
            home: self.path("home"),
            identity_home: self.path("ident"),
            reserved: vec![self.path("reserved"), self.path("ident/.nexus")],
            uid: crate::launcher::effective_uid(),
        }
    }

    fn entries(&self, dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }
}

impl Drop for Dirs {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.base);
    }
}

// ---------------------------------------------------------------- fixture

struct Fx {
    gov: Arc<GovernedControl>,
    evidence: Arc<MemoryEvidence>,
    owner: Arc<Owner>,
    door: AgentDoor,
    dirs: Dirs,
    _root: TempRoot,
}

fn control(root: &TempRoot, evidence: &Arc<MemoryEvidence>) -> Arc<GovernedControl> {
    Arc::new(
        GovernedControl::with_secrets(
            root.0.path(),
            Vault::Disabled.source(),
            evidence.clone(),
            Arc::new(SystemClock::default()),
        )
        .unwrap(),
    )
}

impl Fx {
    /// A front door with in-memory evidence, the Owner's door for
    /// `mother`, and (if `workspace`) the empty workspace `home/ws`.
    fn new(workspace: bool) -> Self {
        let root = temp_root("m2d-b1");
        let evidence = Arc::new(MemoryEvidence::new(100_000));
        let gov = control(&root, &evidence);
        let dirs = Dirs::new();
        if workspace {
            gov.register_workspace_in(&dirs.ws(), &dirs.exclusions())
                .expect("an empty workspace in $HOME");
        }
        let owner = Owner::new();
        let door = gov.issue_agent_door(mother(), owner.clone()).unwrap();
        Self {
            gov,
            evidence,
            owner,
            door,
            dirs,
            _root: root,
        }
    }

    fn workspace(&self) -> WorkspaceInfo {
        self.gov.workspace().expect("a registered workspace")
    }

    /// The Owner's Files grant on the registered workspace.
    fn grant(&self) {
        self.gov
            .request_grant(
                &GrantRequest::Files {
                    workspace: self.workspace().id,
                },
                Duration::from_secs(600),
                &*self.owner,
            )
            .expect("the Owner grants Files on the workspace");
    }

    fn consent(&self, door: &AgentDoor, hash: &str, request: &str) -> u64 {
        door.confirm_mission(&MissionConfirmation {
            contract_hash: hash.into(),
            request: request.into(),
            summary: vec!["A one-page site for Rosa's bakery".into()],
            page_facts: None,
        })
        .expect("the Owner consents")
        .evidence_seq()
    }

    fn admission(&self, run: RunId) -> Option<FilesAdmission> {
        self.gov.authority().runs().files_admission(run)
    }

    /// A run of the fixture's door, admitted to (HASH, label(1)).
    fn admitted_run(&self) -> RunId {
        self.consent(&self.door, HASH, &label(1));
        let run = self.door.open_run().unwrap();
        self.door.admit_files(run, HASH, &label(1)).unwrap();
        run
    }

    fn create(
        &self,
        run: RunId,
        name: &str,
        content: &str,
    ) -> Result<AgentOutcome, AuthorityError> {
        self.door.agent_action(
            run,
            &Intent::File(FileCreate {
                name: name.into(),
                content: content.into(),
            }),
        )
    }
}

fn mission(hash: &str, request: &str) -> MissionConfirmation {
    MissionConfirmation {
        contract_hash: hash.into(),
        request: request.into(),
        summary: vec!["A one-page site for Rosa's bakery".into()],
        page_facts: None,
    }
}

fn held(request: &str) -> MissionResumeConfirmation {
    MissionResumeConfirmation {
        mission: "msn-000001".into(),
        contract_hash: HASH.into(),
        request: request.into(),
        held_reason: "unclean_shutdown".into(),
        summary: vec!["A one-page site for Rosa's bakery".into()],
    }
}

fn class<T: std::fmt::Debug>(result: Result<T, AuthorityError>) -> &'static str {
    result.expect_err("refused").class()
}

// ---------------------------------------------------------------- H1′ and D-O15

/// H3 / D-O15 (b), MD-7: admission needs the Owner's cached yes to exactly
/// this contract hash and request at this door. An unconsented first tag
/// is refused (first use pins nothing), as is a consented hash with
/// another request and the reverse; the consented pair is admitted and
/// bound to its record's sequence number.
#[test]
fn m2d_b1_h1_a_an_unconsented_first_tag_is_refused() {
    let f = Fx::new(true);
    let run = f.door.open_run().unwrap();
    assert_eq!(
        class(f.door.admit_files(run, HASH, &label(1))),
        "no_consent"
    );
    assert_eq!(f.admission(run), None);

    let seq = f.consent(&f.door, HASH2, &label(2));
    assert_eq!(
        class(f.door.admit_files(run, HASH, &label(2))),
        "no_consent"
    );
    assert_eq!(
        class(f.door.admit_files(run, HASH2, &label(1))),
        "no_consent"
    );
    assert_eq!(f.admission(run), None);

    f.door.admit_files(run, HASH2, &label(2)).unwrap();
    let admission = f.admission(run).expect("admitted");
    assert_eq!(admission.contract_hash, HASH2);
    assert_eq!(admission.request, label(2));
    assert_eq!(admission.evidence_seq, seq);
    assert_eq!(admission.kind, ConsentKind::Mission);
    assert_eq!(admission.workspace.id, f.workspace().id);
}

/// H1′: a yes cached before a Stop never admits a context after it, even
/// after the Owner's resume; a run admitted before the Stop is cancelled
/// and stays unadmittable. A fresh yes after the resume admits.
#[test]
fn m2d_b1_h1_b_stop_resume_and_a_replayed_old_yes() {
    let f = Fx::new(true);
    f.consent(&f.door, HASH, &label(1));
    let before = f.door.open_run().unwrap();
    f.door.admit_files(before, HASH, &label(1)).unwrap();

    f.gov.emergency_stop();
    f.gov.resume(&*f.owner).unwrap();
    assert_eq!(f.door.consents_cached(), 0, "Stop invalidates every entry");

    let after = f.door.open_run().unwrap();
    assert_eq!(
        class(f.door.admit_files(after, HASH, &label(1))),
        "no_consent"
    );
    assert_eq!(f.admission(after), None);
    assert_eq!(
        class(f.door.admit_files(before, HASH, &label(1))),
        "run_cancelled"
    );

    let seq = f.consent(&f.door, HASH, &label(1));
    f.door.admit_files(after, HASH, &label(1)).unwrap();
    assert_eq!(f.admission(after).unwrap().evidence_seq, seq);
}

/// H1′: a halt during the Owner's mission dialog leaves no consent, and a
/// late yes cannot repopulate the table; the same for a held resume.
#[test]
fn m2d_b1_h1_c_a_halt_during_confirmation() {
    let f = Fx::new(true);
    let door = Arc::new(f.gov.issue_agent_door(mother(), f.owner.clone()).unwrap());
    let halting = door.clone();
    f.owner.during(move || {
        let _ = halting.halt(HaltReason::JournalFailed).write();
    });
    assert_eq!(
        class(door.confirm_mission(&mission(HASH, &label(1)))),
        "halted"
    );
    assert_eq!(door.consents_cached(), 0);

    let door = Arc::new(f.gov.issue_agent_door(mother(), f.owner.clone()).unwrap());
    let halting = door.clone();
    f.owner.during(move || {
        let _ = halting.halt(HaltReason::JournalFailed).write();
    });
    assert_eq!(class(door.confirm_held_mission(&held(&label(1)))), "halted");
    assert_eq!(door.consents_cached(), 0);

    // Control: the fixture's own door keeps a yes.
    f.consent(&f.door, HASH, &label(1));
    assert_eq!(f.door.consents_cached(), 1);
}

/// H1′: a halt, a Stop with its resume, or quitting that lands after the
/// record and the door's own checks, just before the yes is inserted,
/// leaves no consent: the insertion validates the lifetime it was asked
/// in. After the resume the old yes admits nothing.
#[test]
fn m2d_b1_h1_d_interference_during_insertion() {
    let f = Fx::new(true);
    let door = Arc::new(f.gov.issue_agent_door(mother(), f.owner.clone()).unwrap());
    let halting = door.clone();
    f.gov.hooks().arm(at::CONSENT_BEFORE_FILL, move || {
        let _ = halting.halt(HaltReason::JournalFailed).write();
    });
    assert_eq!(
        class(door.confirm_mission(&mission(HASH, &label(1)))),
        "halted"
    );
    assert_eq!(door.consents_cached(), 0);

    let gov = f.gov.clone();
    let owner = f.owner.clone();
    f.gov.hooks().arm(at::CONSENT_BEFORE_FILL, move || {
        gov.emergency_stop();
        gov.resume(&*owner).unwrap();
    });
    assert_eq!(
        class(f.door.confirm_mission(&mission(HASH, &label(2)))),
        "emergency_stopped"
    );
    assert_eq!(f.door.consents_cached(), 0);
    let run = f.door.open_run().unwrap();
    assert_eq!(
        class(f.door.admit_files(run, HASH, &label(2))),
        "no_consent"
    );

    let gov = f.gov.clone();
    f.gov.hooks().arm(at::CONSENT_BEFORE_FILL, move || {
        gov.shut_down();
    });
    assert_eq!(
        class(f.door.confirm_held_mission(&held(&label(3)))),
        "closed"
    );
    assert_eq!(f.door.consents_cached(), 0);
}

/// H1′: a late confirmation, whose dialog outlived a Stop and the Owner's
/// resume, cannot repopulate usable consent.
#[test]
fn m2d_b1_h1_e_a_late_confirmation_cannot_repopulate() {
    let f = Fx::new(true);
    let gov = f.gov.clone();
    let owner = f.owner.clone();
    f.owner.during(move || {
        gov.emergency_stop();
        gov.resume(&*owner).unwrap();
    });
    assert!(f.door.confirm_mission(&mission(HASH, &label(1))).is_err());
    assert_eq!(f.door.consents_cached(), 0);
    let run = f.door.open_run().unwrap();
    assert_eq!(
        class(f.door.admit_files(run, HASH, &label(1))),
        "no_consent"
    );
    // Control: asked again after the resume, the yes admits.
    f.consent(&f.door, HASH, &label(1));
    f.door.admit_files(run, HASH, &label(1)).unwrap();
}

/// H1′: a halt, a Stop or quitting between admission's workspace check and
/// its binding refuses it, binding nothing; the binding step validates
/// membership, the run, the halt, Stop and closure together.
#[test]
fn m2d_b1_h1_f_interference_during_admission() {
    let f = Fx::new(true);
    f.consent(&f.door, HASH, &label(1));

    let door = Arc::new(f.gov.issue_agent_door(mother(), f.owner.clone()).unwrap());
    f.consent(&door, HASH, &label(1));
    let run = door.open_run().unwrap();
    let halting = door.clone();
    f.gov.hooks().arm(at::ADMIT_BEFORE_BIND, move || {
        let _ = halting.halt(HaltReason::JournalFailed).write();
    });
    assert_eq!(class(door.admit_files(run, HASH, &label(1))), "halted");
    assert_eq!(f.admission(run), None);

    let run = f.door.open_run().unwrap();
    let gov = f.gov.clone();
    f.gov.hooks().arm(at::ADMIT_BEFORE_BIND, move || {
        gov.emergency_stop();
    });
    assert_eq!(
        class(f.door.admit_files(run, HASH, &label(1))),
        "emergency_stopped"
    );
    assert_eq!(f.admission(run), None);
    f.gov.resume(&*f.owner).unwrap();

    f.consent(&f.door, HASH, &label(2));
    let run = f.door.open_run().unwrap();
    let gov = f.gov.clone();
    f.gov.hooks().arm(at::ADMIT_BEFORE_BIND, move || {
        gov.shut_down();
    });
    assert_eq!(class(f.door.admit_files(run, HASH, &label(2))), "closed");
    assert_eq!(f.admission(run), None);
}

/// H1′: at capacity (64 cached yeses) the next consent is refused
/// predictably, before any dialog or record; the cached ones still admit,
/// and another door has its own table.
#[test]
fn m2d_b1_h1_g_the_65th_entry_is_refused_predictably() {
    let f = Fx::new(true);
    for n in 1..=64 {
        f.consent(&f.door, HASH, &label(n));
    }
    assert_eq!(f.door.consents_cached(), 64);
    let consented = f
        .evidence
        .records()
        .iter()
        .filter(|r| r.phase == EvidencePhase::MissionConsented)
        .count();
    assert_eq!(
        class(f.door.confirm_mission(&mission(HASH, &label(65)))),
        "capacity"
    );
    assert_eq!(
        class(f.door.confirm_held_mission(&held(&label(66)))),
        "capacity"
    );
    assert_eq!(f.owner.missions(), 64, "the Owner was not asked");
    assert_eq!(f.owner.helds.load(Ordering::SeqCst), 0);
    let after = f
        .evidence
        .records()
        .iter()
        .filter(|r| {
            matches!(
                r.phase,
                EvidencePhase::MissionConsented | EvidencePhase::MissionResumeConfirmed
            )
        })
        .count();
    assert_eq!(after, consented, "nothing recorded");

    let run = f.door.open_run().unwrap();
    f.door.admit_files(run, HASH, &label(64)).unwrap();
    let other = f.gov.issue_agent_door(mother(), f.owner.clone()).unwrap();
    f.consent(&other, HASH, &label(65));
}

/// D-O15 (c), (d), (f) and live membership: another door's yes admits
/// nothing here, and a door cannot admit another door's run; a second,
/// different admission is refused and changes nothing (the same one again
/// is the same binding); a held-mission resume yes admits.
#[test]
fn m2d_b1_d_o15_membership_stickiness_and_a_resume_yes() {
    let f = Fx::new(true);
    let other = f.gov.issue_agent_door(mother(), f.owner.clone()).unwrap();
    f.consent(&other, HASH, &label(1));
    let run = f.door.open_run().unwrap();
    assert_eq!(
        class(f.door.admit_files(run, HASH, &label(1))),
        "no_consent"
    );
    assert_eq!(class(other.admit_files(run, HASH, &label(1))), "not_member");
    assert_eq!(f.admission(run), None);

    f.consent(&f.door, HASH, &label(1));
    f.consent(&f.door, HASH2, &label(2));
    f.door.admit_files(run, HASH, &label(1)).unwrap();
    let first = f.admission(run).unwrap();
    assert_eq!(
        class(f.door.admit_files(run, HASH2, &label(2))),
        "already_admitted"
    );
    assert_eq!(f.admission(run), Some(first.clone()));
    f.door.admit_files(run, HASH, &label(1)).unwrap();
    assert_eq!(f.admission(run), Some(first));

    let resumed = f.door.open_run().unwrap();
    let approval = f.door.confirm_held_mission(&held(&label(3))).unwrap();
    f.door.admit_files(resumed, HASH, &label(3)).unwrap();
    let admission = f.admission(resumed).unwrap();
    assert_eq!(admission.kind, ConsentKind::Resume);
    assert_eq!(admission.evidence_seq, approval.evidence_seq());
}

/// D-O15 (e) and the run's life: a new broker (a clean restart) holds no
/// yes; a finished or cancelled run, or an owner run, is never admitted;
/// without a workspace nothing is admitted; a workspace replaced at its
/// path is refused (D-O15 i).
#[test]
fn m2d_b1_d_o15_restart_run_life_and_workspace_identity() {
    let f = Fx::new(true);
    let restarted = Fx::new(true);
    let run = restarted.door.open_run().unwrap();
    assert_eq!(
        class(restarted.door.admit_files(run, HASH, &label(1))),
        "no_consent"
    );

    f.consent(&f.door, HASH, &label(1));
    let finished = f.door.open_run().unwrap();
    f.door.finish_when_settled(finished).unwrap();
    assert!(f.door.admit_files(finished, HASH, &label(1)).is_err());
    assert_eq!(f.admission(finished), None);
    let cancelled = f.door.open_run().unwrap();
    let _ = f.door.cancel_run(cancelled).unwrap().write();
    assert!(f.door.admit_files(cancelled, HASH, &label(1)).is_err());
    assert_eq!(f.admission(cancelled), None);
    let (_, owner_run) = f.gov.open_owner_run(vec![]).unwrap();
    assert!(f.door.admit_files(owner_run, HASH, &label(1)).is_err());

    let bare = Fx::new(false);
    bare.consent(&bare.door, HASH, &label(1));
    let run = bare.door.open_run().unwrap();
    assert_eq!(
        class(bare.door.admit_files(run, HASH, &label(1))),
        "unavailable"
    );
    assert_eq!(bare.admission(run), None);

    let run = f.door.open_run().unwrap();
    std::fs::rename(f.dirs.ws(), f.dirs.path("home/moved")).unwrap();
    std::fs::create_dir(f.dirs.ws()).unwrap();
    assert_eq!(
        class(f.door.admit_files(run, HASH, &label(1))),
        "target_changed"
    );
    assert_eq!(f.admission(run), None);
}

/// P0 sticky privacy: a Files admission leaves the run's privacy class as
/// it was admitted (Restricted for a door's run), also once its door is
/// gone and its group pruned.
#[test]
fn m2d_b1_privacy_is_unchanged_by_admission() {
    use crate::authority::evidence::Privacy;
    let f = Fx::new(true);
    let door = f.gov.issue_agent_door(mother(), f.owner.clone()).unwrap();
    f.consent(&door, HASH, &label(1));
    let run = door.open_run().unwrap();
    door.admit_files(run, HASH, &label(1)).unwrap();
    assert_eq!(f.gov.authority().runs().privacy(run), Privacy::Restricted);
    drop(door);
    let _ = f.gov.issue_agent_door(mother(), f.owner.clone()).unwrap();
    assert_eq!(f.gov.authority().runs().privacy(run), Privacy::Restricted);
    assert!(f.admission(run).is_some(), "the binding is never dropped");
}

// ---------------------------------------------------------------- D-O18

/// D-O18 (MD-16): `/`, `$HOME` and the identity home exactly are refused,
/// a subdirectory of `$HOME` is allowed; inside or containing a reserved
/// tree, non-empty, a link, or another owner's is refused; one workspace
/// per process, and none after a restart.
#[test]
fn m2d_b1_d_o18_workspace_selection() {
    let root = temp_root("m2d-b1-ws");
    let evidence = Arc::new(MemoryEvidence::new(1_000));
    let dirs = Dirs::new();
    let ex = dirs.exclusions();
    let refused = |path: &Path, ex: &Exclusions| {
        let gov = control(&root, &evidence);
        let result = gov.register_workspace_in(path, ex);
        assert!(result.is_err(), "{path:?} is refused");
        assert!(gov.workspace().is_none());
    };

    refused(Path::new("/"), &ex);
    // Each exact selection on its own: an empty, owned directory that no
    // other rule refuses, named as `$HOME` or as the identity home.
    std::fs::create_dir(dirs.path("bare-home")).unwrap();
    std::fs::create_dir(dirs.path("bare-ident")).unwrap();
    let only = |home: &str, ident: &str| Exclusions {
        home: dirs.path(home),
        identity_home: dirs.path(ident),
        reserved: Vec::new(),
        uid: crate::launcher::effective_uid(),
    };
    refused(&dirs.path("bare-home"), &only("bare-home", "ident"));
    refused(&dirs.path("bare-ident"), &only("home", "bare-ident"));
    let gov = control(&root, &evidence);
    gov.register_workspace_in(&dirs.path("bare-home"), &only("home", "ident"))
        .expect("the same directory, not named as a home, is allowed");

    std::fs::create_dir(dirs.path("reserved/inner")).unwrap();
    refused(&dirs.path("reserved/inner"), &ex);
    let mut containing = dirs.exclusions();
    std::fs::create_dir(dirs.path("home/outer")).unwrap();
    containing.reserved.push(dirs.path("home/outer/staging"));
    refused(&dirs.path("home/outer"), &containing);

    std::fs::create_dir(dirs.path("home/full")).unwrap();
    std::fs::write(dirs.path("home/full/a.txt"), b"x").unwrap();
    refused(&dirs.path("home/full"), &ex);
    std::os::unix::fs::symlink(dirs.ws(), dirs.path("home/link")).unwrap();
    refused(&dirs.path("home/link"), &ex);
    let mut stranger = dirs.exclusions();
    stranger.uid = stranger.uid.wrapping_add(1);
    refused(&dirs.ws(), &stranger);
    refused(&dirs.path("home/missing"), &ex);

    let gov = control(&root, &evidence);
    let info = gov.register_workspace_in(&dirs.ws(), &ex).unwrap();
    assert_eq!(info.name, "ws");
    assert_eq!(gov.workspace(), Some(info.clone()));
    std::fs::create_dir(dirs.path("home/second")).unwrap();
    assert!(gov
        .register_workspace_in(&dirs.path("home/second"), &ex)
        .is_err());
    assert_eq!(gov.workspace(), Some(info));
    assert_eq!(dirs.entries(&dirs.ws()), Vec::<String>::new(), "untouched");

    let restarted = control(&root, &evidence);
    assert!(restarted.workspace().is_none());
}

/// The Files grant names the registered workspace only.
#[test]
fn m2d_b1_files_grant_names_the_registered_workspace() {
    let f = Fx::new(true);
    assert_eq!(
        class(f.gov.grant_scope(&GrantRequest::Files {
            workspace: "2f1c3e4d-5a6b-4c7d-8e9f-0a1b2c3d4e5f".into(),
        })),
        "invalid_action"
    );
    let scope = f
        .gov
        .grant_scope(&GrantRequest::Files {
            workspace: f.workspace().id,
        })
        .unwrap();
    assert_eq!(scope.kind().as_str(), "files");
    assert!(scope
        .describe()
        .iter()
        .any(|line| line.contains(&f.workspace().id)));
    let bare = Fx::new(false);
    assert!(bare
        .gov
        .grant_scope(&GrantRequest::Files {
            workspace: f.workspace().id,
        })
        .is_err());
}

// ---------------------------------------------------------------- D-O12, D-O15 a

/// D-O15 (a), MD-8: a Files intent on a run that is not admitted is
/// refused, the binding is never taken from the intent; once admitted the
/// same run reaches the content checks.
#[test]
fn m2d_b1_d_o15_a_files_on_an_unadmitted_run() {
    let f = Fx::new(true);
    f.grant();
    f.consent(&f.door, HASH, &label(1));
    let run = f.door.open_run().unwrap();
    assert_eq!(
        class(f.create(run, "index.html", "<h1>hi</h1>\n")),
        "not_admitted"
    );
    assert_eq!(class(f.create(run, ".x", "<h1>hi</h1>\n")), "not_admitted");
    f.door.admit_files(run, HASH, &label(1)).unwrap();
    assert_eq!(
        class(f.create(run, ".x", "<h1>hi</h1>\n")),
        "invalid_action"
    );
    let (owner, owner_run) = f.gov.open_owner_run(vec![]).unwrap();
    let intent = Intent::File(FileCreate {
        name: "index.html".into(),
        content: "x".into(),
    });
    assert_eq!(
        class(f.gov.propose(&owner, owner_run, &intent)),
        "not_admitted"
    );
}

/// D-O12 at the broker: too big, too many lines, not text, a bad name, an
/// existing name (a file, a directory or a dangling link), more than the
/// dialog shows, and no live Files grant are each refused at prepare: no
/// commitment, the workspace untouched, nothing recorded.
#[test]
fn m2d_b1_d_o12_prepare_refusals_leave_nothing() {
    let f = Fx::new(true);
    let run = f.admitted_run();
    assert_eq!(class(f.create(run, "index.html", "x")), "no_covering_grant");
    f.grant();
    std::fs::write(f.dirs.ws().join("taken.html"), b"the Owner's").unwrap();
    std::fs::create_dir(f.dirs.ws().join("dir")).unwrap();
    std::os::unix::fs::symlink("/nonexistent-target", f.dirs.ws().join("dangling")).unwrap();
    let before = f.dirs.entries(&f.dirs.ws());
    let records = f.evidence.records().len();

    let long = "n".repeat(65);
    let cases: Vec<(&str, String, &str)> = vec![
        ("big.txt", "x".repeat(8 * 1024 + 1), "invalid_action"),
        ("long.txt", "l\n".repeat(151), "invalid_action"),
        ("nul.txt", "a\0b".into(), "invalid_action"),
        (".x", "x".into(), "invalid_action"),
        (".nexus-coding-run-tmp-1", "x".into(), "invalid_action"),
        ("a/b", "x".into(), "invalid_action"),
        ("..", "x".into(), "invalid_action"),
        ("é.txt", "x".into(), "invalid_action"),
        (&long, "x".into(), "invalid_action"),
        ("taken.html", "x".into(), "file_exists"),
        ("dir", "x".into(), "file_exists"),
        ("dangling", "x".into(), "file_exists"),
        (
            "hidden.txt",
            "\u{7f}".repeat(8_000),
            "change_too_large_to_show",
        ),
    ];
    for (name, content, expected) in &cases {
        assert_eq!(class(f.create(run, name, content)), *expected, "{name:?}");
    }
    assert!(f.gov.authority().commitments().views_of_run(run).is_empty());
    assert_eq!(f.dirs.entries(&f.dirs.ws()), before);
    assert_eq!(
        std::fs::read(f.dirs.ws().join("taken.html")).unwrap(),
        b"the Owner's"
    );
    assert_eq!(f.evidence.records().len(), records, "nothing recorded");

    for grant in f.gov.grants() {
        f.gov.revoke_grant(grant.id).unwrap();
    }
    assert_eq!(class(f.create(run, "index.html", "x")), "no_covering_grant");
}

// ---------------------------------------------------------------- D-O21

/// D-O21 at the broker: canaries in the name and the content of refused
/// creates reach no record and no returned error.
#[test]
fn m2d_b1_d_o21_names_and_content_leave_no_trace() {
    let f = Fx::new(true);
    f.grant();
    let run = f.admitted_run();
    let c = format!(
        "NXCANARY-{}",
        hex::encode(crate::authority::ids::random16())
    );
    let cl = format!(
        "nxsecret_{}",
        &hex::encode(crate::authority::ids::random16())[..16]
    );
    std::fs::write(f.dirs.ws().join(&cl), b"x").unwrap();
    let mut errors = Vec::new();
    // Every case is a refusal: a valid name carrying C with content that is
    // not text, C and Cl in content, C in content too large to show, an
    // existing name carrying Cl, and an invalid name carrying C.
    for (name, content) in [
        (c.clone(), "x\0".to_string()),
        ("a.txt".to_string(), format!("{c}\0{cl}")),
        (
            "a.txt".to_string(),
            format!("{c}{}", "\u{7f}".repeat(8_000)),
        ),
        (cl.clone(), c.clone()),
        (format!(".{c}"), c.clone()),
    ] {
        let error = f.create(run, &name, &content).expect_err("refused");
        errors.push(format!("{error:?} {error}"));
    }
    let mut haystack = errors.join("\n");
    for record in f.evidence.records() {
        haystack.push_str(&format!("{record:?}\n{}\n", record.to_json()));
    }
    for canary in [&c, &cl] {
        for form in [
            canary.to_string(),
            canary.to_lowercase(),
            canary.to_uppercase(),
            hex::encode(canary.as_bytes()),
            canary[..12].to_string(),
            canary[canary.len() - 12..].to_string(),
        ] {
            assert!(!haystack.contains(&form), "{form} leaked");
        }
    }
}

// ---------------------------------------------------------------- Codex review 1 of PR #57

/// Codex MEDIUM (D-O18): a reserved location is compared by where it
/// really is. A reserved path that does not exist yet is resolved through
/// its longest existing ancestor, links included, and its missing
/// components appended; when that cannot be established, the selection is
/// refused (fail closed). Each case on a fresh broker, against an empty,
/// owned, canonical workspace that no other rule refuses; the controls show
/// the same workspaces allowed when the reserved path lies elsewhere.
#[test]
fn m2d_b1_d_o18_b_reserved_paths_resolve_through_links() {
    let root = temp_root("m2d-b1-ws-links");
    let evidence = Arc::new(MemoryEvidence::new(1_000));
    let dirs = Dirs::new();
    let with_reserved = |reserved: PathBuf| Exclusions {
        home: dirs.path("home"),
        identity_home: dirs.path("ident"),
        reserved: vec![reserved],
        uid: crate::launcher::effective_uid(),
    };
    let select = |path: &Path, reserved: PathBuf| {
        let gov = control(&root, &evidence);
        let result = gov.register_workspace_in(path, &with_reserved(reserved));
        (result.is_ok(), gov.workspace().is_some())
    };
    for sub in ["home/wsa", "home/wsb", "home/wsc", "home/wsd", "home/wse"] {
        std::fs::create_dir(dirs.path(sub)).unwrap();
    }

    // Codex's case: `alias -> wsa`, reserved `alias/future` (absent).
    std::os::unix::fs::symlink(dirs.path("home/wsa"), dirs.path("alias")).unwrap();
    assert_eq!(
        select(&dirs.path("home/wsa"), dirs.path("alias/future")),
        (false, false),
        "the workspace contains the reserved location, through a link"
    );

    // An existing ancestor that is a link, with missing components below.
    std::os::unix::fs::symlink(dirs.path("home"), dirs.path("lnk")).unwrap();
    assert_eq!(
        select(&dirs.path("home/wsb"), dirs.path("lnk/wsb/staging/inner")),
        (false, false),
        "resolved through an ancestor link"
    );

    // Not established: a dangling link on the way, or `..` after a missing
    // component, refuses the selection.
    std::os::unix::fs::symlink(dirs.path("home/wsc/later"), dirs.path("dangle")).unwrap();
    assert_eq!(
        select(&dirs.path("home/wsc"), dirs.path("dangle/x")),
        (false, false),
        "a dangling link: containment cannot be established"
    );
    assert_eq!(
        select(&dirs.path("home/wsd"), dirs.path("home/absent/../wsd")),
        (false, false),
        "`..` after a missing component: containment cannot be established"
    );

    // Controls: a missing reserved path elsewhere, plain or through a link
    // to elsewhere, refuses nothing.
    assert_eq!(
        select(&dirs.path("home/wse"), dirs.path("reserved/absent/deep")),
        (true, true)
    );
    std::os::unix::fs::symlink(dirs.path("reserved"), dirs.path("elsewhere")).unwrap();
    assert_eq!(
        select(&dirs.path("home/wse"), dirs.path("elsewhere/future")),
        (true, true)
    );
    assert_eq!(dirs.entries(&dirs.path("home/wse")), Vec::<String>::new());
}

/// Codex LOW: the cached consent keeps its record's own time. The store
/// takes a while (here the clock jumps an hour while the consent or resume
/// record is written): the admission, and so the Owner's display, still
/// says when the recorded yes was given.
#[test]
fn m2d_b1_h1_h_the_cached_consent_keeps_its_record_s_time() {
    use crate::authority::clock::ManualClock;
    use crate::authority::evidence::{EvidenceRecord, EvidenceSink, EvidenceUnavailable};

    struct Slow {
        memory: Arc<MemoryEvidence>,
        clock: Arc<ManualClock>,
    }
    impl EvidenceSink for Slow {
        fn record(&self, record: &EvidenceRecord) -> Result<u64, EvidenceUnavailable> {
            let seq = self.memory.record(record)?;
            if matches!(
                record.phase,
                EvidencePhase::MissionConsented | EvidencePhase::MissionResumeConfirmed
            ) {
                self.clock.advance(Duration::from_secs(3600));
            }
            Ok(seq)
        }
    }

    let root = temp_root("m2d-b1-time");
    let memory = Arc::new(MemoryEvidence::new(1_000));
    let clock = Arc::new(ManualClock::default());
    let gov = Arc::new(
        GovernedControl::with_secrets(
            root.0.path(),
            Vault::Disabled.source(),
            Arc::new(Slow {
                memory: memory.clone(),
                clock: clock.clone(),
            }),
            clock.clone(),
        )
        .unwrap(),
    );
    let dirs = Dirs::new();
    gov.register_workspace_in(&dirs.ws(), &dirs.exclusions())
        .unwrap();
    let owner = Owner::new();
    let door = gov.issue_agent_door(mother(), owner.clone()).unwrap();
    let recorded = |phase: EvidencePhase| {
        memory
            .records()
            .iter()
            .rev()
            .find(|r| r.phase == phase)
            .map(|r| r.at_wall_ms)
            .expect("the yes is recorded")
    };

    door.confirm_mission(&mission(HASH, &label(1))).unwrap();
    let run = door.open_run().unwrap();
    door.admit_files(run, HASH, &label(1)).unwrap();
    let admission = gov.authority().runs().files_admission(run).unwrap();
    assert_eq!(
        admission.consented_wall_ms,
        recorded(EvidencePhase::MissionConsented)
    );

    door.confirm_held_mission(&held(&label(2))).unwrap();
    let resumed = door.open_run().unwrap();
    door.admit_files(resumed, HASH, &label(2)).unwrap();
    let admission = gov.authority().runs().files_admission(resumed).unwrap();
    assert_eq!(
        admission.consented_wall_ms,
        recorded(EvidencePhase::MissionResumeConfirmed)
    );
}

// ---------------------------------------------------------------- Codex review 2 of PR #57

/// Codex review 2 (MEDIUM): overlap is decided by directory identity, not by
/// path strings. A bind mount or a casefolded name gives one directory a
/// second canonical spelling that no string check relates; an unprivileged
/// test makes one with the seam `mount_alias_for_test` (a symlink whose own
/// spelling is kept as canonical, while opening it reaches the directory).
/// Each alias is refused: the workspace inside a reserved tree through a
/// bind alias; a reserved tree named in a casefold-equivalent spelling
/// above the workspace; a reserved location below the workspace through a
/// bind alias; `$HOME` reached through a bind alias. Controls: distinct
/// directories with confusable names are accepted.
#[test]
fn m2d_b1_d_o18_c_aliases_are_decided_by_directory_identity() {
    use crate::files::mount_alias_for_test;
    let root = temp_root("m2d-b1-ws-identity");
    let evidence = Arc::new(MemoryEvidence::new(1_000));
    let dirs = Dirs::new();
    let with = |home: PathBuf, reserved: Vec<PathBuf>| Exclusions {
        home,
        identity_home: dirs.path("ident"),
        reserved,
        uid: crate::launcher::effective_uid(),
    };
    let select = |path: &Path, ex: Exclusions| {
        let gov = control(&root, &evidence);
        let result = gov.register_workspace_in(path, &ex);
        (result.is_ok(), gov.workspace().is_some())
    };
    for sub in [
        "real/reserved/ws",
        "home/wsf",
        "home/wsg",
        "conf/Reserved",
        "conf/reserved/ws",
        "conf/res",
        "conf/res-ws",
    ] {
        std::fs::create_dir_all(dirs.path(sub)).unwrap();
    }
    let home = || dirs.path("home");

    // Each alias: one directory under two canonical spellings.
    std::os::unix::fs::symlink(dirs.path("real/reserved"), dirs.path("bind")).unwrap();
    mount_alias_for_test(dirs.path("bind"));
    std::os::unix::fs::symlink(dirs.path("real/reserved"), dirs.path("real/Reserved")).unwrap();
    mount_alias_for_test(dirs.path("real/Reserved"));
    std::os::unix::fs::symlink(dirs.path("home/wsf"), dirs.path("bind2")).unwrap();
    mount_alias_for_test(dirs.path("bind2"));
    std::os::unix::fs::symlink(dirs.path("home/wsg"), dirs.path("homebind")).unwrap();
    mount_alias_for_test(dirs.path("homebind"));

    let outcomes = vec![
        (
            "the workspace inside a reserved tree, spelled through a bind alias",
            select(
                &dirs.path("bind/ws"),
                with(home(), vec![dirs.path("real/reserved")]),
            ),
        ),
        (
            "a reserved tree above the workspace, in a casefold-equivalent spelling",
            select(
                &dirs.path("real/reserved/ws"),
                with(home(), vec![dirs.path("real/Reserved")]),
            ),
        ),
        (
            "a reserved location below the workspace, through a bind alias of it",
            select(
                &dirs.path("home/wsf"),
                with(home(), vec![dirs.path("bind2/future")]),
            ),
        ),
        (
            "$HOME itself, reached through a bind alias",
            select(&dirs.path("home/wsg"), with(dirs.path("homebind"), vec![])),
        ),
        (
            "control: a distinct directory differing only in case",
            select(
                &dirs.path("conf/reserved/ws"),
                with(home(), vec![dirs.path("conf/Reserved")]),
            ),
        ),
        (
            "control: a sibling whose name begins with the reserved name",
            select(
                &dirs.path("conf/res-ws"),
                with(home(), vec![dirs.path("conf/res")]),
            ),
        ),
    ];
    let expected = vec![
        (outcomes[0].0, (false, false)),
        (outcomes[1].0, (false, false)),
        (outcomes[2].0, (false, false)),
        (outcomes[3].0, (false, false)),
        (outcomes[4].0, (true, true)),
        (outcomes[5].0, (true, true)),
    ];
    assert_eq!(outcomes, expected);
}

// ---------------------------------------------------------------- Codex review 3 of PR #57

/// `dev` as `major:minor` (the glibc encoding of `st_dev`).
fn major_minor(dev: u64) -> String {
    let major = ((dev >> 8) & 0xfff) | ((dev >> 32) & !0xfff);
    let minor = (dev & 0xff) | ((dev >> 12) & !0xff);
    format!("{major}:{minor}")
}

fn dev_of(path: &Path) -> String {
    use std::os::unix::fs::MetadataExt;
    major_minor(std::fs::metadata(path).unwrap().dev())
}

/// Codex review 3 (MEDIUM): a location reached through a subtree bind mount
/// (a mount whose root, field 4 of `/proc/self/mountinfo`, is not `/`) has
/// ancestors no `..` walk visits, so the selection is refused
/// (`unsupported_mount_topology`), as it is when the mount topology cannot
/// be read, parsed or matched. A pure mountinfo seam feeds each topology;
/// the directories, and their devices, are real.
#[test]
fn m2d_b1_d_o18_d_subtree_bind_mounts_are_refused() {
    use crate::files::mountinfo_for_test;
    let root = temp_root("m2d-b1-ws-mounts");
    let evidence = Arc::new(MemoryEvidence::new(1_000));
    let dirs = Dirs::new();
    for sub in ["data/reserved", "mnt/ws", "mnt/res", "plain/ws"] {
        std::fs::create_dir_all(dirs.path(sub)).unwrap();
    }
    let tmp = std::fs::canonicalize(std::env::temp_dir()).unwrap();
    // Each device once: the temporary directory's own entry only when it is
    // a filesystem of its own (on a host whose /tmp is on the root
    // filesystem, a second entry would list the root device twice).
    let mut ordinary = format!(
        "1 0 {} / / rw,relatime shared:1 - ext4 /dev/root rw\n",
        dev_of(Path::new("/"))
    );
    if dev_of(&tmp) != dev_of(Path::new("/")) {
        ordinary.push_str(&format!(
            "2 1 {} / {} rw,relatime shared:2 - tmpfs tmpfs rw\n",
            dev_of(&tmp),
            tmp.display()
        ));
    }
    let bind = |id: u32, source: &str, point: &str| {
        format!(
            "{id} 2 {} {source} {} rw,relatime shared:3 - tmpfs tmpfs rw\n",
            dev_of(&dirs.path(point)),
            dirs.path(point).display()
        )
    };
    let with = |reserved: Vec<PathBuf>| Exclusions {
        home: dirs.path("home"),
        identity_home: dirs.path("ident"),
        reserved,
        uid: crate::launcher::effective_uid(),
    };
    let select = |mountinfo: Option<Result<String, ()>>, path: &Path, reserved: Vec<PathBuf>| {
        mountinfo_for_test(mountinfo);
        let gov = control(&root, &evidence);
        let result = gov
            .register_workspace_in(path, &with(reserved))
            .map(|_| ())
            .map_err(|error| error.class());
        mountinfo_for_test(None);
        result
    };
    let refused = Err("unsupported_mount_topology");

    let outcomes = vec![
        (
            "Codex's topology: the workspace is a bind of a reserved tree's subdirectory",
            select(
                Some(Ok(format!(
                    "{ordinary}{}",
                    bind(50, "/reserved/ws", "mnt/ws")
                ))),
                &dirs.path("mnt/ws"),
                vec![dirs.path("data/reserved")],
            ),
        ),
        (
            "the reverse: a reserved location under a subtree bind",
            select(
                Some(Ok(format!(
                    "{ordinary}{}",
                    bind(51, "/reserved/x", "mnt/res")
                ))),
                &dirs.path("plain/ws"),
                vec![dirs.path("mnt/res/future")],
            ),
        ),
        (
            "mountinfo cannot be read",
            select(Some(Err(())), &dirs.path("plain/ws"), vec![]),
        ),
        (
            "mountinfo is malformed",
            select(
                Some(Ok("33 2 259:6 / / rw shared:1 ext4\n".to_string())),
                &dirs.path("plain/ws"),
                vec![],
            ),
        ),
        (
            "no mount matches the workspace's device",
            select(
                Some(Ok(
                    "1 0 4095:1048575 / / rw shared:1 - ext4 /dev/x rw\n".to_string()
                )),
                &dirs.path("plain/ws"),
                vec![],
            ),
        ),
        (
            "control: ordinary mounts, root /",
            select(Some(Ok(ordinary.clone())), &dirs.path("plain/ws"), vec![]),
        ),
    ];
    let expected = vec![
        (outcomes[0].0, refused),
        (outcomes[1].0, refused),
        (outcomes[2].0, refused),
        (outcomes[3].0, refused),
        (outcomes[4].0, refused),
        (outcomes[5].0, Ok(())),
    ];
    assert_eq!(outcomes, expected);
}

/// A mountinfo path field (root or mount point) as the kernel escapes it,
/// decoded for the independent evaluator, separately from the production
/// parser's decoder: exactly the four escapes the kernel writes (`\040`
/// space, `\011` tab, `\012` newline, `\134` backslash), in one pass, so
/// a decoded backslash never starts another escape (Codex review 6).
fn decoded(field: &str) -> PathBuf {
    let mut out = String::new();
    let mut rest = field;
    while let Some(at) = rest.find('\\') {
        out.push_str(&rest[..at]);
        let escape = rest.get(at + 1..at + 4);
        let replacement = match escape {
            Some("040") => Some(' '),
            Some("011") => Some('\t'),
            Some("012") => Some('\n'),
            Some("134") => Some('\\'),
            _ => None,
        };
        match replacement {
            Some(c) => {
                out.push(c);
                rest = &rest[at + 4..];
            }
            None => {
                out.push('\\');
                rest = &rest[at + 1..];
            }
        }
    }
    out.push_str(rest);
    PathBuf::from(out)
}

/// Codex review 6: the evaluator's decoder, seam-free: a mount point with a
/// space (`/mnt/ci\040tmp`) is `/mnt/ci tmp`, and a path below it is
/// below it component-wise; tab, newline and backslash decode; a decoded
/// backslash does not start another escape.
#[test]
fn m2d_b1_d_o18_e_the_evaluator_decodes_kernel_escapes() {
    let point = decoded("/mnt/ci\\040tmp");
    assert_eq!(point, PathBuf::from("/mnt/ci tmp"));
    assert!(Path::new("/mnt/ci tmp/nexus/ws").starts_with(&point));
    assert!(!Path::new("/mnt/ci/tmp").starts_with(&point));
    assert_eq!(decoded("/a\\011b\\012c"), PathBuf::from("/a\tb\nc"));
    assert_eq!(decoded("/a\\134b"), PathBuf::from("/a\\b"));
    assert_eq!(decoded("/a\\134040"), PathBuf::from("/a\\040"));
    assert_eq!(decoded("/"), PathBuf::from("/"));
}

/// The parser on real data (no seam), whatever this host's topology (Codex
/// review 5, LOW 2): `/proc/self/mountinfo` is read and every line of it
/// parsed; and the decision for the workspace and for `/` agrees with the
/// rule evaluated independently here from the raw table: accepted, with
/// that entry's mount point, device and root `/`, iff the path's device is
/// listed exactly once, with root `/`, a supported type, a mount point that
/// is the path or its ancestor, and no other mount point between them or at
/// the path; otherwise refused, `unsupported_mount_topology`. On a supported
/// host every earlier assertion holds (and `/`'s mount point is `/`).
#[test]
fn m2d_b1_d_o18_e_the_real_mountinfo_is_parsed() {
    use std::os::unix::fs::MetadataExt;
    let raw = std::fs::read("/proc/self/mountinfo").expect("the mount table is readable");
    let parsed = crate::files::parse_mountinfo(&raw).expect("the real table parses in full");
    let text = String::from_utf8_lossy(&raw).into_owned();
    let lines: Vec<Vec<&str>> = text
        .lines()
        .filter(|line| !line.is_empty())
        .map(|line| line.split(' ').collect())
        .collect();
    assert_eq!(parsed.len(), lines.len(), "every line is a mount");
    let fstype = |fields: &[&str]| {
        let separator = 6 + fields[6..].iter().position(|f| *f == "-").unwrap();
        fields[separator + 1].to_string()
    };
    let dirs = Dirs::new();
    for path in [dirs.ws(), PathBuf::from("/")] {
        let device = major_minor(std::fs::metadata(&path).unwrap().dev());
        let listed: Vec<&Vec<&str>> = lines.iter().filter(|f| f[2] == device).collect();
        let supported = match listed.as_slice() {
            [only] => {
                let point = decoded(only[4]);
                let between = lines.iter().any(|other| {
                    let at = decoded(other[4]);
                    other[0] != only[0]
                        && (at == path
                            || (at != point && at.starts_with(&point) && path.starts_with(&at)))
                });
                decoded(only[3]) == Path::new("/")
                    && ["ext4", "xfs", "tmpfs"].contains(&fstype(only).as_str())
                    && path.starts_with(&point)
                    && !between
            }
            _ => false,
        };
        eprintln!(
            "{}: device {device} listed {} time(s); supported {supported}",
            path.display(),
            listed.len()
        );
        match crate::files::mount_of(&path) {
            Ok(mount) => {
                assert!(supported, "accepted although the rule refuses: {mount:?}");
                assert_eq!(mount.mount_point, decoded(listed[0][4]));
                assert!(path.starts_with(&mount.mount_point), "{mount:?}");
                assert_eq!(format!("{}:{}", mount.major, mount.minor), device);
                assert_eq!(mount.root, std::ffi::OsString::from("/"), "{mount:?}");
                if path == Path::new("/") {
                    assert_eq!(mount.mount_point, PathBuf::from("/"));
                }
            }
            Err(error) => {
                assert!(!supported, "refused although the rule accepts: {error:?}");
                assert_eq!(error, AuthorityError::UnsupportedMountTopology);
            }
        }
    }
}

// ---------------------------------------------------------------- Codex review 4 of PR #57

/// Codex review 4 (MEDIUM ×2, LOW): only the supported topology is
/// accepted, a plain local filesystem (ext4, xfs or tmpfs) mounted once,
/// whole: the location's device is listed exactly once in
/// `/proc/self/mountinfo`, with root `/`, an allowed type, a mount point
/// that is the location's ancestor or itself, and no other mount point
/// between them or at the location. Everything else is refused
/// (`unsupported_mount_topology`): Codex's hidden mount (the device listed
/// at `/`, at a hidden deeper mount with root `/`, and under the covering
/// subtree bind); an overlay (root `/`); a btrfs subvolume; a device listed
/// twice with root `/`; a truncated record; a mount point strictly between.
/// Controls: a single ext4 and a single tmpfs are accepted. A pure
/// mountinfo seam over real directories; each table lists every real
/// device the selection meets exactly once unless the case says otherwise.
#[test]
fn m2d_b1_d_o18_f_only_a_whole_local_filesystem_mounted_once_is_supported() {
    use crate::files::mountinfo_for_test;
    let root = temp_root("m2d-b1-ws-allowlist");
    let evidence = Arc::new(MemoryEvidence::new(1_000));
    let dirs = Dirs::new();
    for sub in [
        "data/reserved",
        "mnt/ws/deep",
        "plain/ws",
        "mid/ws",
        "other",
    ] {
        std::fs::create_dir_all(dirs.path(sub)).unwrap();
    }
    let tmp = std::fs::canonicalize(std::env::temp_dir()).unwrap();
    let rootfs = dev_of(Path::new("/"));
    let tmpfs = dev_of(&tmp);
    // The ordinary table: `/` (ext4), and the temporary directory's own
    // filesystem if it has one, of type `fstype` (when the temporary
    // directory is on the root filesystem, `/` itself takes `fstype`);
    // `truncated` drops that entry's superblock options.
    let table = |fstype: &str, truncated: bool| {
        let end = if truncated { "" } else { " rw" };
        if tmpfs == rootfs {
            format!("1 0 {rootfs} / / rw,relatime shared:1 - {fstype} /dev/root{end}\n")
        } else {
            format!(
                "1 0 {rootfs} / / rw,relatime shared:1 - ext4 /dev/root rw\n\
                 2 1 {tmpfs} / {} rw,relatime shared:2 - {fstype} {fstype}{end}\n",
                tmp.display()
            )
        }
    };
    let entry = |id: u32, dev: &str, source: &str, point: &str, fstype: &str| {
        format!(
            "{id} 2 {dev} {source} {} rw,relatime shared:9 - {fstype} {fstype} rw\n",
            dirs.path(point).display()
        )
    };
    let with = |reserved: Vec<PathBuf>| Exclusions {
        home: dirs.path("home"),
        identity_home: dirs.path("ident"),
        reserved,
        uid: crate::launcher::effective_uid(),
    };
    let select = |mountinfo: String, path: &Path, reserved: Vec<PathBuf>| {
        mountinfo_for_test(Some(Ok(mountinfo)));
        let gov = control(&root, &evidence);
        let result = gov
            .register_workspace_in(path, &with(reserved))
            .map(|_| ())
            .map_err(|error| error.class());
        mountinfo_for_test(None);
        result
    };
    let plain = dirs.path("plain/ws");
    let refused = Err("unsupported_mount_topology");

    let outcomes = vec![
        (
            "Codex's hidden mount: the device at /, hidden at deep (root /), under a subtree bind",
            select(
                format!(
                    "{}{}{}",
                    table("tmpfs", false),
                    entry(40, &tmpfs, "/", "mnt/ws/deep", "tmpfs"),
                    entry(41, &tmpfs, "/data/reserved/ws", "mnt/ws", "tmpfs"),
                ),
                &dirs.path("mnt/ws/deep"),
                vec![dirs.path("data/reserved")],
            ),
        ),
        (
            "an overlay, root /",
            select(table("overlay", false), &plain, vec![]),
        ),
        (
            "a btrfs subvolume",
            select(table("btrfs", false), &plain, vec![]),
        ),
        (
            "the device listed twice, root / both times",
            select(
                format!(
                    "{}{}",
                    table("tmpfs", false),
                    entry(42, &tmpfs, "/", "other", "tmpfs")
                ),
                &plain,
                vec![],
            ),
        ),
        (
            "a truncated record (no superblock options)",
            select(table("tmpfs", true), &plain, vec![]),
        ),
        (
            "a mount point strictly between the mount and the workspace",
            select(
                format!(
                    "{}{}",
                    table("tmpfs", false),
                    entry(43, "4095:1048575", "/", "mid", "tmpfs")
                ),
                &dirs.path("mid/ws"),
                vec![],
            ),
        ),
        (
            "control: a single ext4",
            select(table("ext4", false), &plain, vec![]),
        ),
        (
            "control: a single tmpfs",
            select(table("tmpfs", false), &plain, vec![]),
        ),
    ];
    let expected = vec![
        (outcomes[0].0, refused),
        (outcomes[1].0, refused),
        (outcomes[2].0, refused),
        (outcomes[3].0, refused),
        (outcomes[4].0, refused),
        (outcomes[5].0, refused),
        (outcomes[6].0, Ok(())),
        (outcomes[7].0, Ok(())),
    ];
    assert_eq!(outcomes, expected);
}

// ---------------------------------------------------------------- Codex review 5 of PR #57

/// Codex review 5 (LOW 1): a record is well formed only when every
/// required field is present and non-empty. Codex's exact record, ending
/// `- ext4 /dev/root ` (the superblock options an empty final field once
/// the line is split on spaces), and one with an empty source, each refuse
/// the whole table (`unsupported_mount_topology`), although the record is
/// another device's; the control, the same record with its options,
/// is accepted. Through the mountinfo seam over real directories.
#[test]
fn m2d_b1_d_o18_g_empty_required_fields_refuse_the_table() {
    use crate::files::mountinfo_for_test;
    let root = temp_root("m2d-b1-ws-fields");
    let evidence = Arc::new(MemoryEvidence::new(1_000));
    let dirs = Dirs::new();
    std::fs::create_dir_all(dirs.path("plain/ws")).unwrap();
    let tmp = std::fs::canonicalize(std::env::temp_dir()).unwrap();
    let rootfs = dev_of(Path::new("/"));
    let tmpfs = dev_of(&tmp);
    // Each device the selection meets, listed once and well formed.
    let mut ordinary = format!("1 0 {rootfs} / / rw,relatime shared:1 - ext4 /dev/root rw\n");
    if tmpfs != rootfs {
        ordinary.push_str(&format!(
            "2 1 {tmpfs} / {} rw,relatime shared:2 - tmpfs tmpfs rw\n",
            tmp.display()
        ));
    }
    // Another device's record, at a mount point the selection never meets.
    let other = |tail: &str| {
        format!(
            "{ordinary}9 1 4095:1048575 / /nexus-m2d-b1-elsewhere rw,relatime shared:9 - {tail}\n"
        )
    };
    let select = |mountinfo: String| {
        mountinfo_for_test(Some(Ok(mountinfo)));
        let gov = control(&root, &evidence);
        let result = gov
            .register_workspace_in(
                &dirs.path("plain/ws"),
                &Exclusions {
                    home: dirs.path("home"),
                    identity_home: dirs.path("ident"),
                    reserved: vec![],
                    uid: crate::launcher::effective_uid(),
                },
            )
            .map(|_| ())
            .map_err(|error| error.class());
        mountinfo_for_test(None);
        result
    };
    let outcomes = vec![
        (
            "Codex's record: empty superblock options",
            select(other("ext4 /dev/root ")),
        ),
        ("an empty source", select(other("ext4  rw"))),
        (
            "control: the same record, well formed",
            select(other("ext4 /dev/root rw")),
        ),
    ];
    let expected = vec![
        (outcomes[0].0, Err("unsupported_mount_topology")),
        (outcomes[1].0, Err("unsupported_mount_topology")),
        (outcomes[2].0, Ok(())),
    ];
    assert_eq!(outcomes, expected);
}
