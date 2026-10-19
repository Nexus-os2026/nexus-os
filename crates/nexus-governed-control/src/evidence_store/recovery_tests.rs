//! Sessions, reconciliation, Recovery required and the acknowledgement
//! (EL-1 §7), on real files; the crash is a real child process killed
//! (SIGKILL) between an effect's start record and its end record.

use super::tests::Scratch;
use super::*;
use crate::authority::approval::{
    AcknowledgementConfirmation, ActionConfirmation, ControlConfirmer, GrantConfirmation,
    ResumeConfirmation,
};
use crate::authority::clock::ManualClock;
use crate::authority::commitment::{ExecutionGuard, FailureClass, PreparedAction, TargetIdentity};
use crate::authority::effect::{CapabilityKind, EffectClass};
use crate::authority::evidence::{EvidencePhase, MemoryEvidence};
use crate::authority::ids::{AgentId, Digest};
use crate::authority::policy::GrantScope;
use crate::authority::run::RunOrigin;
use crate::authority::{Authority, AuthorityError};
use crate::control::{Control, EffectOutput, PendingEffect, Preparation};
use std::io::{BufRead, BufReader, Write as _};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// The owner's scripted answers, and what was shown.
#[derive(Default)]
struct Owner {
    acknowledge: bool,
    resume: bool,
    asked_acknowledgement: Mutex<Vec<AcknowledgementConfirmation>>,
    asked_resume: AtomicU32,
    /// Run while the acknowledgement dialog is open.
    while_open: Option<Box<dyn Fn() + Send + Sync>>,
}

impl ControlConfirmer for Owner {
    fn confirm_action(&self, _: &ActionConfirmation) -> bool {
        true
    }
    fn confirm_grant(&self, _: &GrantConfirmation) -> bool {
        true
    }
    fn confirm_resume(&self, _: &ResumeConfirmation) -> bool {
        self.asked_resume.fetch_add(1, Ordering::SeqCst);
        self.resume
    }
    fn confirm_acknowledgement(&self, request: &AcknowledgementConfirmation) -> bool {
        self.asked_acknowledgement
            .lock()
            .unwrap()
            .push(request.clone());
        if let Some(while_open) = &self.while_open {
            while_open();
        }
        self.acknowledge
    }
}

fn owner(acknowledge: bool, resume: bool) -> Owner {
    Owner {
        acknowledge,
        resume,
        ..Owner::default()
    }
}

fn rows_of(path: &Path) -> Vec<Row> {
    // Read through a store of its own: it qualifies (and reconciles) first.
    let store = EvidenceStore::open(path).expect("the store qualifies");
    store.rows()
}

fn phases(rows: &[Row]) -> Vec<&str> {
    rows.iter().map(|row| row.phase.as_str()).collect()
}

/// An effect that reports `READY` once it runs (its start record committed)
/// and then waits to be killed.
struct Hang;

impl PendingEffect for Hang {
    fn revalidate(&self) -> Result<Digest, AuthorityError> {
        Ok(Digest::of("fake.target", &[b"here"]))
    }
    fn parameters(&self) -> Digest {
        Digest::of("fake.params", &[b"p"])
    }
    fn execute(
        self: Box<Self>,
        guard: &ExecutionGuard,
    ) -> Result<EffectOutput, (FailureClass, String)> {
        let mut out = std::io::stdout().lock();
        writeln!(out, "READY {}", guard.commitment()).unwrap();
        out.flush().unwrap();
        for _ in 0..1200 {
            std::thread::sleep(Duration::from_millis(100));
        }
        Err((FailureClass::Timeout, "not killed".into()))
    }
}

const CHILD: &str = "NEXUS_P3_EVIDENCE_RECOVERY_CHILD";

/// The child of the crash test: one effect started, then killed. Without
/// its task it does nothing.
#[test]
#[ignore = "run by el1_n4_a_crash_after_a_start_requires_recovery as a child process"]
fn el1_n4_recovery_child() {
    let Ok(path) = std::env::var(CHILD) else {
        return;
    };
    let evidence = PrimaryEvidence::open(Path::new(&path), vec![]).unwrap();
    let control = Control::new(Authority::new(evidence, Arc::new(ManualClock::default())));
    let agent = AgentId::new("agent-crash").unwrap();
    let run = control
        .authority()
        .open_run(agent.clone(), RunOrigin::AgentGoal)
        .unwrap();
    let grant = control
        .authority()
        .grants()
        .request(
            GrantScope::Perception {
                display: "agent".into(),
            },
            Duration::from_secs(600),
            &owner(true, true),
        )
        .unwrap();
    let view = control
        .propose(
            &agent,
            run,
            Preparation {
                action: PreparedAction {
                    kind: CapabilityKind::Perception,
                    class: EffectClass::R1,
                    operation: "fake.observe",
                    target: TargetIdentity {
                        display: "fake target".into(),
                        digest: Digest::of("fake.target", &[b"here"]),
                    },
                    parameters: Digest::of("fake.params", &[b"p"]),
                    grants: vec![grant],
                    leases: vec![],
                    summary: vec!["fake".into()],
                },
                effect: Box::new(Hang),
                ttl: Duration::from_secs(60),
            },
        )
        .unwrap();
    control
        .authorize(view.id, &agent, run, &owner(true, true))
        .unwrap();
    let _ = control.execute(view.id, &agent, run);
    panic!("the child was not killed");
}

/// Run the child, kill it (SIGKILL, through its own handle) once its effect
/// has started, reap it; returns the commitment that started.
fn crash_after_a_start(path: &Path) -> String {
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "evidence_store::recovery_tests::el1_n4_recovery_child",
            "--ignored",
            "--nocapture",
            "--test-threads=1",
        ])
        .env(CHILD, path)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap();
    let stdout = child.stdout.take().unwrap();
    let (tx, rx) = std::sync::mpsc::channel();
    let reader = std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            let Ok(line) = line else { break };
            if let Some(at) = line.find("READY ") {
                let _ = tx.send(line[at + "READY ".len()..].trim().to_string());
                break;
            }
        }
    });
    let started = rx
        .recv_timeout(Duration::from_secs(120))
        .expect("the child's effect started");
    child.kill().unwrap();
    let status = child.wait().unwrap();
    let _ = reader.join();
    assert!(!status.success(), "the child was killed");
    started
}

/// A store whose last session crashed after an effect's start record.
fn crashed() -> (Scratch, PathBuf, String) {
    let scratch = Scratch::new();
    let path = scratch.store();
    let started = crash_after_a_start(&path);
    (scratch, path, started)
}

/// EL-1 §7 (mutant 10, 14): a process killed after an effect's start record
/// and before its end leaves, at the next start, exactly one `unsettled`
/// statement for that commitment and one `unclean_session` for its session;
/// recovery is required, so every gating record is refused; the governed
/// control starts emergency-stopped, its stop record saying why.
#[test]
fn el1_n4_a_crash_after_a_start_requires_recovery() {
    let (_scratch, path, started) = crashed();
    let recent = Arc::new(MemoryEvidence::new(100));
    let evidence = PrimaryEvidence::open(&path, vec![recent.clone()]).unwrap();
    let rows = evidence.store().rows();
    let unsettled: Vec<&Row> = rows.iter().filter(|r| r.phase == "unsettled").collect();
    assert_eq!(unsettled.len(), 1);
    assert_eq!(unsettled[0].commitment.as_deref(), Some(started.as_str()));
    let body: serde_json::Value = serde_json::from_str(&unsettled[0].body).unwrap();
    assert_eq!(body["text"], UNSETTLED_TEXT);
    let started_row = rows
        .iter()
        .find(|r| r.phase == "started" && r.commitment.as_deref() == Some(started.as_str()))
        .unwrap();
    assert_eq!(body["started_seq"], started_row.seq);
    assert_eq!(
        phases(&rows)
            .iter()
            .filter(|p| **p == "unclean_session")
            .count(),
        1
    );
    let health = evidence.health();
    assert!(health.recovery_required);
    assert_eq!((health.unsettled, health.unclean_sessions), (1, 1));
    // Every gating record is refused (here without the start-time stop).
    let authority = Authority::new(evidence.clone(), Arc::new(ManualClock::default()));
    assert_eq!(
        authority
            .open_run(AgentId::new("agent-after").unwrap(), RunOrigin::AgentGoal)
            .unwrap_err(),
        AuthorityError::EvidenceUnavailable
    );
    assert_eq!(
        authority
            .grants()
            .request(
                GrantScope::Perception {
                    display: "agent".into()
                },
                Duration::from_secs(60),
                &owner(true, true)
            )
            .unwrap_err(),
        AuthorityError::EvidenceUnavailable
    );
    drop(authority);
    drop(evidence);
    // The governed control starts emergency-stopped, through the stop.
    let evidence = PrimaryEvidence::open(&path, vec![recent.clone()]).unwrap();
    let root = path.parent().unwrap().parent().unwrap().join("runtime");
    let governed = crate::governed::GovernedControl::with_secrets(
        &root,
        crate::broker::Vault::Disabled.source(),
        evidence.clone(),
        Arc::new(ManualClock::default()),
    )
    .unwrap();
    assert!(governed.status().emergency_stopped);
    let stop = recent
        .records()
        .into_iter()
        .find(|r| r.phase == EvidencePhase::EmergencyStop)
        .expect("the stop is recorded");
    assert!(stop
        .detail
        .contains(&("reason".to_string(), "recovery_required".to_string())));
    assert!(governed
        .open_run(AgentId::new("agent-after").unwrap(), RunOrigin::AgentGoal)
        .is_err());
}

/// EL-1 §7: the reconciliation is idempotent: after the second start
/// (closed cleanly), a third appends nothing but its own session's opening.
#[test]
fn el1_n4_a_third_start_appends_nothing_new() {
    let (_scratch, path, _) = crashed();
    let second = {
        let evidence = PrimaryEvidence::open(&path, vec![]).unwrap();
        assert!(matches!(
            evidence.close_session(),
            EndStatus::Recorded { .. }
        ));
        evidence.store().rows()
    };
    assert_eq!(
        phases(&second)
            .iter()
            .filter(|p| **p == "unsettled")
            .count(),
        1
    );
    let third = rows_of(&path);
    assert_eq!(phases(&third[second.len()..]), ["session_opened"]);
}

/// EL-1 §7: a clean shutdown writes `session_closed`; the next start
/// appends no `unclean_session` for it.
#[test]
fn el1_n4_a_clean_shutdown_is_not_an_unclean_session() {
    let scratch = Scratch::new();
    let path = scratch.store();
    {
        let evidence = PrimaryEvidence::open(&path, vec![]).unwrap();
        assert!(matches!(
            evidence.close_session(),
            EndStatus::Recorded { .. }
        ));
    }
    let rows = rows_of(&path);
    assert_eq!(
        phases(&rows),
        ["session_opened", "session_closed", "session_opened"]
    );
    assert!(
        !EvidenceStore::open(&path)
            .unwrap()
            .health()
            .recovery_required
    );
}

/// EL-1 §7: an `unclean_session` statement alone is recorded and shown but
/// does not hold recovery.
#[test]
fn el1_n4_an_unclean_session_alone_does_not_require_recovery() {
    let scratch = Scratch::new();
    let path = scratch.store();
    drop(EvidenceStore::open(&path).unwrap());
    let store = EvidenceStore::open(&path).unwrap();
    let health = store.health();
    assert_eq!(health.unclean_sessions, 1);
    assert!(!health.recovery_required);
    assert!(store.recovery().is_none());
    assert!(store
        .gate(&EvidenceRecord::new(EvidencePhase::RunOpened, 1, 1))
        .0
        .is_ok());
}

/// EL-1 §7 (mutant 11): reconciliation appends nothing before the whole
/// chain verifies: a tampered row leaves the store unqualified and the
/// file without a statement.
#[test]
fn el1_n4_nothing_is_reconciled_before_the_chain_verifies() {
    let (_scratch, path, _) = crashed();
    let raw = || Connection::open_with_flags(&path, OpenFlags::SQLITE_OPEN_READ_WRITE).unwrap();
    let delete = SCHEMA
        .iter()
        .find(|(_, name, _, _)| *name == "record_no_update")
        .unwrap()
        .3;
    raw()
        .execute_batch(&format!(
            "BEGIN; DROP TRIGGER record_no_update; \
             UPDATE record SET body = '{{}}' WHERE seq = 1; {delete}; COMMIT;"
        ))
        .unwrap();
    let count = || -> i64 {
        raw()
            .query_row("SELECT count(*) FROM record", [], |row| row.get(0))
            .unwrap()
    };
    let before = count();
    assert_eq!(EvidenceStore::open(&path).err(), Some(Unqualified::Chain));
    assert_eq!(count(), before, "a statement was written");
    assert_eq!(
        raw()
            .query_row(
                "SELECT count(*) FROM record WHERE phase IN ('unsettled', 'unclean_session')",
                [],
                |row| row.get::<_, i64>(0)
            )
            .unwrap(),
        0
    );
}

fn authority_over(path: &Path) -> (Arc<PrimaryEvidence>, Authority) {
    let evidence = PrimaryEvidence::open(path, vec![]).unwrap();
    let authority = Authority::new(evidence.clone(), Arc::new(ManualClock::default()));
    (evidence, authority)
}

fn acknowledged(rows: &[Row]) -> Vec<u64> {
    rows.iter()
        .filter(|r| r.phase == "acknowledged")
        .map(|r| {
            serde_json::from_str::<serde_json::Value>(&r.body).unwrap()["covers_through"]
                .as_u64()
                .unwrap()
        })
        .collect()
}

/// EL-1 §7 (mutant 13): the acknowledgement is asked through the native
/// confirmation, with the backend's text; on a yes the store writes
/// `acknowledged{covers_through: N}` (N the highest statement shown) and
/// only then is the resume confirmation asked. A declined acknowledgement
/// changes nothing, and the resume is never asked.
#[test]
fn el1_n4_the_acknowledgement_is_the_owners_and_covers_what_was_shown() {
    let (_scratch, path, started) = crashed();
    let (evidence, authority) = authority_over(&path);
    authority.emergency_stop();
    let rows_before = evidence.store().rows();
    let shown_through = rows_before
        .iter()
        .filter(|r| r.phase == "unsettled" || r.phase == "unclean_session")
        .map(|r| r.seq)
        .max()
        .unwrap();
    // Declined: nothing changes.
    let no = owner(false, true);
    assert_eq!(authority.resume(&no), Err(AuthorityError::Declined));
    assert_eq!(no.asked_resume.load(Ordering::SeqCst), 0);
    assert!(evidence.health().recovery_required);
    assert!(authority.runs().is_stopped());
    assert!(acknowledged(&evidence.store().rows()).is_empty());
    // Confirmed: acknowledged through N, then the resume confirmation.
    let yes = owner(true, true);
    authority.resume(&yes).unwrap();
    let asked = yes.asked_acknowledgement.lock().unwrap().clone();
    assert_eq!(asked.len(), 1);
    assert_eq!(asked[0].covers_through, shown_through);
    assert_eq!(
        asked[0].unsettled,
        [started.chars().take(12).collect::<String>()]
    );
    assert_eq!(asked[0].unclean_sessions.len(), 1);
    let text = asked[0].message();
    assert!(text.contains(
        "These actions may or may not have happened.\nAcknowledging records that you were \
         shown this; it grants nothing and changes no outcome."
    ));
    assert_eq!(yes.asked_resume.load(Ordering::SeqCst), 1);
    assert_eq!(acknowledged(&evidence.store().rows()), [shown_through]);
    assert!(!evidence.health().recovery_required);
    assert!(
        !authority.runs().is_stopped(),
        "the existing resume lifted the stop"
    );
}

/// EL-1 §7 (mutant 12): the acknowledgement covers exactly its snapshot:
/// a statement written while the dialog is open (seq > N) stays
/// unacknowledged and keeps recovery required.
#[test]
fn el1_n4_a_statement_after_the_snapshot_stays_unacknowledged() {
    let (_scratch, path, _) = crashed();
    let (evidence, authority) = authority_over(&path);
    authority.emergency_stop();
    let late = evidence.clone();
    let mut yes = owner(true, true);
    yes.while_open = Some(Box::new(move || {
        let body = statement(
            "unsettled",
            serde_json::json!({ "commitment": "cmt-late", "started_seq": 1, "text": UNSETTLED_TEXT }),
        );
        assert!(matches!(
            late.store().append("unsettled", Some("cmt-late"), &body),
            Append::Committed { .. }
        ));
    }));
    assert!(authority.resume(&yes).is_err());
    let rows = evidence.store().rows();
    let late_seq = rows
        .iter()
        .find(|r| r.commitment.as_deref() == Some("cmt-late"))
        .unwrap()
        .seq;
    let covered = acknowledged(&rows);
    assert_eq!(covered.len(), 1);
    assert!(covered[0] < late_seq);
    assert!(evidence.health().recovery_required);
    assert_eq!(yes.asked_resume.load(Ordering::SeqCst), 0);
    assert!(authority.runs().is_stopped());
    // Shown and acknowledged in its turn, it settles.
    let again = owner(true, true);
    authority.resume(&again).unwrap();
    assert_eq!(
        again.asked_acknowledgement.lock().unwrap()[0].unsettled,
        ["cmt-late".to_string()]
    );
    assert!(!evidence.health().recovery_required);
}
