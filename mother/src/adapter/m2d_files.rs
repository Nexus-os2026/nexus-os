//! M2d-C oracles against the real broker (design v2 §6.1-§6.3, §6.8 M2d-C,
//! §6.10 D-O1, D-O5, D-O6, D-O12, D-O15, D-O21, D-O24 at Mother; §11 M3′
//! and M4′; the Architect's decisions of 2026-11-03), pre-registered before
//! the code. One broker per process: they run inside
//! `real_broker_m1_oracles`, before its emergency-stop finale, on its home,
//! where they register the process's one workspace. The Owner approves and
//! executes each File create at the front door, as at the terminal.
//!
//! A cause after the final check lands at a reading of the broker's clock
//! (`TestClock`), the one step of the broker's an outside test reaches. The
//! candidate is an anonymous file (no name exists before the publication),
//! so the final check is found through the Files ledger: the first reading
//! after the ledger changes past the execution's start record (the
//! kernel's `apply.prepared`, then nothing but the kernel until the final
//! check) is the final check's deadline test, after its cancellation test.
//! The first reading once the name is there is the check after the
//! publication. The kernel write grant's lapse is not reachable from here:
//! the broker oracle and `oracles::m2d_c` cover it.

use super::m2c_support::{consent, fence_port, port_for, resume, Home};
use super::p0_support::{canaries, files_under, hits_in};
use super::{create_private, mother_agent, FileJournal, GovernedPort};
use crate::coordinator::{Coordinator, CoordinatorError, Lanes};
use crate::effect::{Admission, EffectPort};
use crate::evidence::BrokerClass;
use crate::journal::{Event, MissionId};
use crate::lane::LaneRole;
use crate::oracles::m2c_confirmers::answering;
use crate::oracles::m2d_c::{content_of, file, files_lanes, FilesOwner};
use crate::oracles::{contract_with, Recording, Yes};
use crate::state::{MissionState, Trigger};
use nexus_governed_control::authority::approval::MissionConfirmation;
use nexus_governed_control::authority::clock::SystemClock;
use nexus_governed_control::authority::commitment::CommitmentState;
use nexus_governed_control::authority::ids::{CommitmentId, GrantId, RunId};
use nexus_governed_control::governed::GrantRequest;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// A Files commitment's lifetime (it also bounds the publication).
const FILE_TTL: Duration = Duration::from_secs(10 * 60);
const LONG: Duration = Duration::from_secs(24 * 3600);
const SHORT: Duration = Duration::from_secs(60);
const SECOND: Duration = Duration::from_secs(1);

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    hex::encode(Sha256::digest(bytes))
}

/// The Files ledger's files, as (length, modification time) each.
type LedgerState = Vec<Option<(u64, std::time::SystemTime)>>;

fn ledger_state(files: &Path) -> LedgerState {
    ["ledger.db", "ledger.db-wal"]
        .iter()
        .map(|name| {
            let meta = std::fs::metadata(files.join(name)).ok()?;
            Some((meta.len(), meta.modified().ok()?))
        })
        .collect()
}

/// True at the first reading after the Files ledger changed since the first
/// reading that followed `run`'s execution start record (the reading
/// `begin` takes after it, before the commitment executes).
pub(super) fn past_the_start(home: &Home, run: RunId) -> impl Fn() -> bool + Send + 'static {
    let (gate, run) = (home.gate.clone(), run.to_string());
    let files = home.dir.join("broker").join("files");
    let since: Mutex<Option<LedgerState>> = Mutex::new(None);
    move || {
        let started = gate
            .seen()
            .iter()
            .any(|s| s.phase == "started" && s.run.as_deref() == Some(run.as_str()));
        if !started {
            return false;
        }
        let now = ledger_state(&files);
        let mut since = since.lock().unwrap();
        match since.as_ref() {
            None => {
                *since = Some(now);
                false
            }
            Some(before) => *before != now,
        }
    }
}

struct Fx<'a> {
    home: &'a Home,
    ws: PathBuf,
    folder: String,
    workspace: String,
    grant: Mutex<GrantId>,
    owner: FilesOwner,
    port: Arc<GovernedPort>,
    mother: Arc<Coordinator<GovernedPort>>,
}

pub(super) fn on_the_real_broker(home: &Home, origin: &str) {
    let fx = Fx::new(home);
    fx.port_level_admission();
    let existing = fx.create_composed();
    fx.refusals(&existing);
    fx.stale_admissions();
    fx.restart_and_resume();
    fx.interrupted_publications();
    fx.expiry_before_start();
    fx.workspace_changed();
    // The clock moved forward: the World's finale gets its egress back.
    home.control
        .request_grant(
            &GrantRequest::Egress {
                origin: origin.into(),
                methods: vec!["GET".into(), "POST".into()],
                allow_private: true,
            },
            Duration::from_secs(600),
            &fx.owner,
        )
        .unwrap();
}

impl<'a> Fx<'a> {
    fn new(home: &'a Home) -> Self {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        let ws = std::env::temp_dir().join(format!("m2dc-ws-{nanos}"));
        create_private(&ws).unwrap();
        let ws = std::fs::canonicalize(&ws).unwrap();
        let info = home
            .control
            .register_workspace(&ws, &[home.dir.as_path()])
            .expect("the Owner's empty workspace");
        let owner = FilesOwner::default();
        let grant = home
            .control
            .request_grant(
                &GrantRequest::Files {
                    workspace: info.id.clone(),
                },
                LONG,
                &owner,
            )
            .unwrap();
        let port = Arc::new(port_for(&home.control, true).with_workspace(info.name.clone()));
        Self {
            home,
            ws,
            folder: info.name,
            workspace: info.id,
            grant: Mutex::new(grant),
            owner,
            port: port.clone(),
            mother: Arc::new(
                Coordinator::open(
                    FileJournal::open(&home.dir.join("m2d-c-journal.jsonl")).unwrap(),
                    port,
                    Lanes::new(files_lanes()).unwrap(),
                    Arc::new(SystemClock::default()),
                    home.private(),
                )
                .unwrap(),
            ),
        }
    }

    fn port(&self) -> Arc<GovernedPort> {
        Arc::new(port_for(&self.home.control, true).with_workspace(self.folder.clone()))
    }

    fn open(&self, port: &Arc<GovernedPort>, journal: &str) -> Coordinator<GovernedPort> {
        Coordinator::open(
            FileJournal::open(&self.home.dir.join(journal)).unwrap(),
            port.clone(),
            Lanes::new(files_lanes()).unwrap(),
            Arc::new(SystemClock::default()),
            self.home.private(),
        )
        .unwrap()
    }

    fn regrant(&self, ttl: Duration) {
        let grant = self
            .home
            .control
            .request_grant(
                &GrantRequest::Files {
                    workspace: self.workspace.clone(),
                },
                ttl,
                &self.owner,
            )
            .unwrap();
        *self.grant.lock().unwrap() = grant;
    }

    fn approved_in(&self, mother: &Coordinator<GovernedPort>, markers: &str) -> MissionId {
        let mut contract = contract_with(markers);
        contract.requested_folder = Some(self.folder.clone());
        let id = mother.draft(contract).unwrap();
        mother.submit(id).unwrap();
        assert_eq!(consent(mother, id, &Yes), Ok(MissionState::Approved));
        id
    }

    /// Approved, and advanced to the Builder's first File, waiting.
    fn waiting(&self, markers: &str) -> MissionId {
        let id = self.approved_in(&self.mother, markers);
        for _ in 0..3 {
            self.mother.advance(id).unwrap();
        }
        assert_eq!(self.mother.state(id), Some(MissionState::AwaitingApproval));
        id
    }

    /// The Owner approves, natively, the action `id` waits on.
    fn approve(&self, port: &GovernedPort, id: MissionId) -> (RunId, CommitmentId) {
        let pending = port.pending(id).expect("the action the mission waits on");
        let run = RunId::parse(&pending.run).unwrap();
        let commitment = CommitmentId::parse(&pending.commitment).unwrap();
        self.home
            .control
            .authorize(commitment, &mother_agent(), run, &self.owner)
            .expect("the Owner approves");
        (run, commitment)
    }

    fn execute(&self, (run, commitment): (RunId, CommitmentId)) -> bool {
        self.home
            .control
            .execute(commitment, &mother_agent(), run)
            .is_ok()
    }

    fn commitments(&self, run: RunId) -> usize {
        self.home
            .control
            .authority()
            .commitments()
            .views_of_run(run)
            .len()
    }

    fn events(&self, id: MissionId) -> Vec<Event> {
        let journal = self.mother.journal().unwrap();
        journal
            .into_iter()
            .filter(|e| e.mission == id)
            .map(|e| e.event)
            .collect()
    }

    fn content(&self, name: &str) -> Option<Vec<u8>> {
        std::fs::read(self.ws.join(name)).ok()
    }

    /// D-O15 (a, b, c, d, g) at the port: no File perform without an
    /// admission; an admission needs this door's yes to exactly that hash
    /// and request; a second, different admission is refused; a perform of
    /// another lifetime is `admission_stale` and cancels the run.
    fn port_level_admission(&self) {
        let port = self.port();
        let hash = "ab".repeat(32);
        let (one, two) = ("msn-000901/1/1", "msn-000901/2/2");
        for request in [one, two] {
            answering(&Yes, || {
                port.confirm_mission(&MissionConfirmation {
                    contract_hash: hash.clone(),
                    request: request.into(),
                    summary: vec!["M2d-C at the port".into()],
                    page_facts: None,
                })
            })
            .unwrap();
        }
        let admission = |hash: &str, request: &str| Admission {
            contract_hash: hash.into(),
            consent_request: request.into(),
            generation: 1,
        };
        let class = |result: Result<(), crate::effect::EffectRefusal>| result.unwrap_err().class;
        let m = MissionId(901);
        let create = file("port-level.txt", "p\n");
        let refused = port.perform(m, 1, &create).unwrap_err().class;
        assert_eq!(refused, BrokerClass::NotAdmitted, "(a)");
        let other_hash = "cd".repeat(32);
        assert_eq!(
            class(port.admit_files(m, &admission(&other_hash, one))),
            BrokerClass::NoConsent
        );
        let unasked = admission(&hash, "msn-000901/3/3");
        assert_eq!(
            class(port.admit_files(m, &unasked)),
            BrokerClass::NoConsent,
            "(b)"
        );
        let other = self.port();
        let theirs = other.admit_files(MissionId(902), &admission(&hash, one));
        assert_eq!(class(theirs), BrokerClass::NoConsent, "(c)");
        port.admit_files(m, &admission(&hash, one)).unwrap();
        port.admit_files(m, &admission(&hash, one))
            .expect("the same admission again");
        let second = port.admit_files(m, &admission(&hash, two));
        assert_eq!(class(second), BrokerClass::AlreadyAdmitted, "(d)");
        let run = port.run_of(m).unwrap();
        let stale = port.perform(m, 2, &create).unwrap_err().class;
        assert_eq!(stale, BrokerClass::AdmissionStale, "(g)");
        let runs = self.home.control.authority().runs();
        assert!(
            runs.view(run).unwrap().cancelled,
            "(g) the run is cancelled"
        );
        assert!(port.perform(m, 1, &create).is_err());
        assert_eq!(self.commitments(run), 0);
        assert!(self.content("port-level.txt").is_none());
    }

    /// D-O1, D-O6, D-O21 and M3′ composed: the Owner's dialog shows the
    /// complete content and the facts; the code (here, the front door's
    /// approval) puts exactly the bytes at the name; Mother continues with
    /// File B and a File of a later phase without another consent, to Done;
    /// a replayed approval or execution is refused; the journal, the
    /// broker's evidence and the Files ledger are present and hold neither
    /// canary of the names and contents. Returns a name now taken.
    fn create_composed(&self) -> String {
        let cs = canaries();
        let (a, b, c) = (
            format!("{}.txt", cs.c),
            format!("{}.txt", cs.cl),
            format!("v-{}.txt", &cs.cl[9..]),
        );
        let id = self.waiting(&format!("MKFILE:{a} MKFILE:{b} VFILE:{c}"));
        let first = self.approve(&self.port, id);
        let shown = self.owner.0.lock().unwrap().last().cloned().unwrap();
        let hash = self.mother.consented_hash(id).unwrap().to_hex();
        let content = content_of("MKFILE", &a).unwrap();
        for line in [
            format!("Mission contract: {hash}"),
            format!("Workspace: {} ({})", self.folder, self.workspace),
            format!("File: {a} (new, at the workspace's top level)"),
            format!("SHA-256: {}", sha256_hex(content.as_bytes())),
            "New file, complete:".to_string(),
            format!("│ {}", content.trim_end()),
        ] {
            assert!(
                shown.summary.contains(&line),
                "{line} in {:?}",
                shown.summary
            );
        }
        assert!(self.execute(first));
        assert_eq!(self.content(&a), Some(content.into_bytes()));
        assert!(!self.execute(first), "D-O6: executed once");
        assert!(self
            .home
            .control
            .authorize(first.1, &mother_agent(), first.0, &self.owner)
            .is_err());

        let to = |state| assert_eq!(self.mother.advance(id).unwrap().to, state);
        to(MissionState::AwaitingApproval);
        let second = self.approve(&self.port, id);
        assert_eq!(second.0, first.0, "one run, one admission");
        assert!(self.execute(second));
        to(MissionState::Verifying);
        to(MissionState::AwaitingApproval);
        assert!(self.execute(self.approve(&self.port, id)));
        to(MissionState::Explaining);
        to(MissionState::Done);
        for name in [&a, &b, &c] {
            assert!(self.content(name).is_some(), "{name}");
        }
        assert_eq!(self.commitments(first.0), 3);

        let events = self.events(id);
        let count = |pick: fn(&Event) -> bool| events.iter().filter(|e| pick(e)).count();
        assert_eq!(count(|e| matches!(e, Event::EffectDispatched { .. })), 3);
        assert_eq!(count(|e| matches!(e, Event::ApprovalClosed { .. })), 3);
        assert_eq!(
            count(|e| matches!(
                e,
                Event::Transition {
                    trigger: Trigger::ActionApproved { .. }
                }
            )),
            3
        );
        let run = first.0.to_string();
        let seen = self.home.gate.seen();
        let phases = |phase: &str| {
            seen.iter()
                .filter(|s| s.phase == phase && s.run.as_deref() == Some(run.as_str()))
                .count()
        };
        assert_eq!((phases("prepared"), phases("finished")), (3, 3));
        let files = self.home.dir.join("broker").join("files");
        let ledger = std::fs::metadata(files.join("ledger.db")).expect("the Files ledger");
        assert!(ledger.len() > 0);
        assert!(self
            .home
            .control
            .files_recovery_report()
            .unwrap()
            .is_empty());

        let mut hits = hits_in(
            &std::fs::read(self.home.dir.join("m2d-c-journal.jsonl")).unwrap(),
            "journal",
            &cs,
        );
        let staging = files.join("coding-runs");
        let evidence = self.home.dir.join("broker").join("p3-evidence");
        for (path, bytes) in files_under(&files, Some(&staging))
            .into_iter()
            .chain(files_under(&evidence, None))
        {
            hits.extend(hits_in(&bytes, &path.display().to_string(), &cs));
        }
        assert!(hits.is_empty(), "D-O21: {hits:?}");
        a
    }

    /// D-O12, D-O15 (h) and D-O24 at Mother: what breaks Mother's rules is
    /// refused with her own code before the port is asked; what only the
    /// broker can see (the name taken, a display too long to show whole) is
    /// refused at its prepare with its code, and no commitment exists.
    fn refusals(&self, existing: &str) {
        let long = format!("MKFILE:{}", "n".repeat(65));
        let taken = format!("MKFILE:{existing}");
        for (marker, code, at_broker) in [
            ("MKFILE:.x", "file_name_invalid", false),
            ("MKFILE:.nexus-coding-run-tmp-x", "file_name_invalid", false),
            ("MKFILE:a/b", "file_name_invalid", false),
            ("MKFILE:..", "file_request_not_relative", false),
            (long.as_str(), "file_name_invalid", false),
            ("MKFILE:é.txt", "file_name_invalid", false),
            ("MKBIG:big.txt", "file_too_large", false),
            ("MKLINES:lines.txt", "file_too_many_lines", false),
            ("MKNUL:nul.txt", "file_not_text", false),
            (taken.as_str(), "broker_refused:file_exists", true),
            (
                "MKWIDE:wide.txt",
                "broker_refused:change_too_large_to_show",
                true,
            ),
        ] {
            let before = (
                self.port.performed(),
                std::fs::read_dir(&self.ws).unwrap().count(),
            );
            let id = self.approved_in(&self.mother, marker);
            self.mother.advance(id).unwrap();
            self.mother.advance(id).unwrap();
            let step = self.mother.advance(id).unwrap();
            assert_eq!(step.to, MissionState::Blocked, "{code}");
            assert_eq!(self.mother.reason(id).as_deref(), Some(code));
            let ran = self.port.performed() - before.0;
            assert_eq!(ran, usize::from(at_broker), "{code}");
            match self.port.run_of(id) {
                Some(run) => assert_eq!(self.commitments(run), 0, "{code}"),
                None => assert!(!at_broker, "{code}"),
            }
            let after = std::fs::read_dir(&self.ws).unwrap().count();
            assert_eq!(after, before.1, "{code}");
        }
        let mut contract = contract_with("MKFILE:elsewhere.txt");
        contract.requested_folder = Some("rosas-bakery".into());
        let id = self.mother.draft(contract).unwrap();
        self.mother.submit(id).unwrap();
        assert_eq!(consent(&self.mother, id, &Yes), Ok(MissionState::Approved));
        let before = self.port.performed();
        for _ in 0..3 {
            self.mother.advance(id).unwrap();
        }
        assert_eq!(
            self.mother.reason(id).as_deref(),
            Some("file_folder_not_workspace")
        );
        assert_eq!(self.port.performed(), before);
        assert_eq!(self.port.run_of(id), None);
    }

    /// M3′: an admission's answer that comes back after a Stop, or after
    /// the port's fence, revives nothing: no admission is kept, nothing is
    /// proposed, and the mission stays ended.
    fn stale_admissions(&self) {
        let id = self.approved_in(&self.mother, "MKFILE:stale-stop.txt");
        self.mother.advance(id).unwrap();
        self.mother.advance(id).unwrap();
        let mother = self.mother.clone();
        self.port.after_admit.arm(0, move || {
            assert_eq!(mother.stop(id), Ok(MissionState::Stopped));
        });
        assert_eq!(
            self.mother.advance(id).err(),
            Some(CoordinatorError::Stopped)
        );
        assert_eq!(self.mother.state(id), Some(MissionState::Stopped));
        let run = self.port.run_of(id).expect("the run admission opened");
        assert_eq!(self.commitments(run), 0);
        let create = file("stale-stop.txt", "s\n");
        assert!(self.port.perform(id, u64::MAX, &create).is_err());
        assert!(self.content("stale-stop.txt").is_none());

        let port = self.port();
        let mother = self.open(&port, "m2d-c-fence.jsonl");
        let id = self.approved_in(&mother, "MKFILE:stale-fence.txt");
        mother.advance(id).unwrap();
        mother.advance(id).unwrap();
        let fenced = port.clone();
        port.after_admit.arm(0, move || fence_port(&fenced));
        let _ = mother.advance(id);
        assert_eq!(mother.state(id), Some(MissionState::Blocked));
        assert_eq!(
            mother.reason(id).as_deref(),
            Some("broker_refused:port_fenced")
        );
        let run = port.run_of(id).expect("the run admission opened");
        assert_eq!(self.commitments(run), 0);
        assert!(self.content("stale-fence.txt").is_none());
    }

    /// D-O15 (e, c, f): after a clean restart the new door holds no yes of
    /// the old one: Blocked `files_consent_unverifiable`, nothing asked; the
    /// old yes does not admit there. After an unclean one, the Owner's
    /// resume yes admits the held mission.
    fn restart_and_resume(&self) {
        let port = self.port();
        let before = self.open(&port, "m2d-c-restart.jsonl");
        let asked = Recording::default();
        let mut contract = contract_with("MKFILE:restart.txt");
        contract.requested_folder = Some(self.folder.clone());
        let id = before.draft(contract).unwrap();
        before.submit(id).unwrap();
        assert_eq!(consent(&before, id, &asked), Ok(MissionState::Approved));
        let request = asked.0.lock().unwrap()[0].request.clone();
        let hash = before.consented_hash(id).unwrap().to_hex();
        before.close_ref().unwrap();
        let port = self.port();
        let after = self.open(&port, "m2d-c-restart.jsonl");
        for _ in 0..3 {
            after.advance(id).unwrap();
        }
        assert_eq!(
            after.reason(id).as_deref(),
            Some("files_consent_unverifiable")
        );
        assert_eq!((port.performed(), port.run_of(id)), (0, None));
        let old = Admission {
            contract_hash: hash,
            consent_request: request,
            generation: 1,
        };
        let refused = port.admit_files(MissionId(903), &old).unwrap_err().class;
        assert_eq!(refused, BrokerClass::NoConsent);

        let port = self.port();
        let crashed = self.open(&port, "m2d-c-held.jsonl");
        let id = self.approved_in(&crashed, "MKFILE:resumed.txt");
        drop(crashed);
        let port = self.port();
        let mother = self.open(&port, "m2d-c-held.jsonl");
        assert!(mother.held(id).is_some());
        assert_eq!(resume(&mother, id, true), Ok(MissionState::Approved));
        for _ in 0..3 {
            mother.advance(id).unwrap();
        }
        assert_eq!(mother.state(id), Some(MissionState::AwaitingApproval));
        let run = port.run_of(id).unwrap();
        let views = self
            .home
            .control
            .authority()
            .commitments()
            .views_of_run(run);
        assert!(views.iter().any(|v| v.state == CommitmentState::Prepared));
        assert_eq!(mother.stop(id), Ok(MissionState::Stopped));
        assert!(self.content("resumed.txt").is_none());
    }

    /// M4′ composed, for each cause a test can drive at the broker: the one
    /// publication completes; Mother journals it as the success it is
    /// (`ActionApproved`, the action closed), then ends the mission with
    /// `file_publication_interrupted:<cause>`; the rest of the Builder's
    /// answer (File B) and the later Verifier never run.
    fn interrupted_publications(&self) {
        for code in ["cancelled", "revoked", "expired", "commitment_expired"] {
            if code == "expired" {
                let long = *self.grant.lock().unwrap();
                self.home.control.revoke_grant(long).unwrap();
                self.regrant(SHORT);
            }
            let (a, b) = (format!("{code}-a.txt"), format!("{code}-b.txt"));
            let id = self.waiting(&format!("MKFILE:{a} MKFILE:{b} VFILE:{code}-c.txt"));
            let (run, commitment) = self.approve(&self.port, id);
            let (control, clock) = (self.home.control.clone(), self.home.clock.clone());
            let name = self.ws.join(&a);
            let grant = *self.grant.lock().unwrap();
            let published = move || name.exists();
            match code {
                "cancelled" => self
                    .home
                    .clock
                    .arm(past_the_start(self.home, run), move || {
                        control.cancel_run(run).unwrap()
                    }),
                "revoked" => self
                    .home
                    .clock
                    .arm(published, move || control.revoke_grant(grant).unwrap()),
                "expired" => self
                    .home
                    .clock
                    .arm(published, move || clock.advance(SHORT + SECOND)),
                _ => self
                    .home
                    .clock
                    .arm(published, move || clock.advance(FILE_TTL + SECOND)),
            }
            let done = self.execute((run, commitment));
            assert!(!self.home.clock.disarm(), "{code}: the cause landed");
            assert!(done, "{code}: the one publication completes");
            let content = content_of("MKFILE", &format!("{code}-a.txt")).unwrap();
            assert_eq!(
                self.content(&format!("{code}-a.txt")),
                Some(content.into_bytes())
            );
            let step = self.mother.advance(id).unwrap();
            assert_eq!(step.to, MissionState::Blocked, "{code}");
            let reason = format!("file_publication_interrupted:{code}");
            assert_eq!(self.mother.reason(id), Some(reason));
            let tail: Vec<Event> = self
                .events(id)
                .into_iter()
                .skip_while(|e| {
                    !matches!(
                        e,
                        Event::Transition {
                            trigger: Trigger::ActionApproved { .. }
                        }
                    )
                })
                .collect();
            assert!(
                matches!(
                    tail.as_slice(),
                    [
                        Event::Transition {
                            trigger: Trigger::ActionApproved { .. }
                        },
                        Event::ApprovalClosed { .. },
                        Event::Transition {
                            trigger: Trigger::Block { .. }
                        },
                    ]
                ),
                "{code}: {tail:?}"
            );
            assert_eq!(self.commitments(run), 1, "{code}: File B never asked");
            assert!(self.content(&b).is_none(), "{code}");
            let verifier = self.events(id).iter().any(|e| {
                matches!(
                    e,
                    Event::LaneInvoked {
                        role: LaneRole::Verifier,
                        ..
                    }
                )
            });
            assert!(!verifier, "{code}: no later lane");
            if matches!(code, "revoked" | "expired") {
                self.regrant(LONG);
            }
        }
    }

    /// D-O5 at Mother: a commitment that expired before the Owner started
    /// it ends the mission `approval_expired` (Mother's frozen M2a code),
    /// and nothing is written.
    fn expiry_before_start(&self) {
        let id = self.waiting("MKFILE:late.txt");
        let pending = self.port.pending(id).unwrap();
        self.home.clock.advance(FILE_TTL + SECOND);
        assert_eq!(self.mother.advance(id).unwrap().to, MissionState::Blocked);
        assert_eq!(self.mother.reason(id).as_deref(), Some("approval_expired"));
        let run = RunId::parse(&pending.run).unwrap();
        let commitment = CommitmentId::parse(&pending.commitment).unwrap();
        assert!(self
            .home
            .control
            .authorize(commitment, &mother_agent(), run, &self.owner)
            .is_err());
        assert!(self.content("late.txt").is_none());
    }

    /// D-O15 (i): the workspace replaced by another directory at its path
    /// is not the admitted one: refused, nothing written. Put back after.
    fn workspace_changed(&self) {
        let moved = self.ws.with_extension("moved");
        std::fs::rename(&self.ws, &moved).unwrap();
        create_private(&self.ws).unwrap();
        let id = self.approved_in(&self.mother, "MKFILE:changed.txt");
        for _ in 0..3 {
            let _ = self.mother.advance(id);
        }
        assert_eq!(
            self.mother.reason(id).as_deref(),
            Some("broker_refused:target_changed")
        );
        assert!(self.content("changed.txt").is_none());
        std::fs::remove_dir(&self.ws).unwrap();
        std::fs::rename(&moved, &self.ws).unwrap();
    }
}
