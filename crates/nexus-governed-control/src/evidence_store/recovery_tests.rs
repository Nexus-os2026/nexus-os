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

// ── P3-C11-M6: corrections to EL-1 (C-1, C-3) ──────────────────────────────

/// EL-1 §7 as amended by C-1 (P3-C11-M6), (b): a started commitment with
/// no end row (finished, revoked or expired) still gets exactly one
/// `unsettled` statement, and a later start adds none; one with an end row
/// gets none.
#[test]
fn el1_m6_c1_b_a_start_with_no_end_row_is_unsettled_once() {
    let scratch = Scratch::new();
    let path = scratch.store();
    {
        let store = EvidenceStore::open(&path).unwrap();
        for (phase, commitment) in [
            ("prepared", "cmt-open"),
            ("authorized", "cmt-open"),
            ("started", "cmt-open"),
            ("prepared", "cmt-ended"),
            ("authorized", "cmt-ended"),
            ("started", "cmt-ended"),
            ("finished", "cmt-ended"),
        ] {
            assert!(matches!(
                store.append(phase, Some(commitment), &format!("{{\"{phase}\":1}}")),
                Append::Committed { .. }
            ));
        }
    }
    for _ in 0..2 {
        let store = EvidenceStore::open(&path).unwrap();
        let unsettled: Vec<Option<String>> = store
            .rows()
            .into_iter()
            .filter(|row| row.phase == "unsettled")
            .map(|row| row.commitment)
            .collect();
        assert_eq!(unsettled, [Some("cmt-open".to_string())]);
        assert!(store.health().recovery_required);
    }
}

/// A store whose chain holds a start with no end: its next open writes one
/// `unsettled` statement, and recovery is required.
fn unsettled_store() -> (Scratch, PathBuf) {
    let scratch = Scratch::new();
    let path = scratch.store();
    let store = EvidenceStore::open(&path).unwrap();
    assert!(matches!(
        store.append("started", Some("cmt-unsettled"), "{\"started\":1}"),
        Append::Committed { .. }
    ));
    drop(store);
    (scratch, path)
}

/// EL-1 §6 as amended by C-3 (P3-C11-M6): a record the acknowledgement pays
/// (written first, before the `acknowledged` row) is projected, as every
/// record the store commits is.
#[test]
fn el1_m6_c3_records_paid_by_the_acknowledgement_are_projected() {
    let (_scratch, path) = unsettled_store();
    let recent = Arc::new(MemoryEvidence::new(100));
    let evidence = PrimaryEvidence::open(&path, vec![recent.clone()]).unwrap();
    assert!(evidence.health().recovery_required);
    // One end record owed (end records are written while recovery is
    // required; this one is refused).
    evidence.store().refuse_inserts(true);
    assert_eq!(
        evidence.record_end(&EvidenceRecord::new(EvidencePhase::RunCancelled, 1, 1)),
        EndStatus::Owed
    );
    evidence.store().refuse_inserts(false);
    assert!(recent.records().is_empty());
    let authority = Authority::new(evidence.clone(), Arc::new(ManualClock::default()));
    authority.resume(&owner(true, true)).unwrap();
    assert_eq!(evidence.health().owed, 0);
    let projected: Vec<EvidencePhase> = recent.records().iter().map(|r| r.phase).collect();
    assert_eq!(
        projected,
        [EvidencePhase::RunCancelled],
        "the record the acknowledgement paid is projected"
    );
    let rows = evidence.store().rows();
    let seq_of = |phase: &str| rows.iter().find(|r| r.phase == phase).unwrap().seq;
    assert!(seq_of("run_cancelled") < seq_of("acknowledged"));
}

// ── P3-C11-M6: corrections to EL-1 (C-4, C-7) ──────────────────────────────

/// EL-1 §7 as amended by C-4 (P3-C11-M6): an acknowledgement beyond the
/// highest committed sequence number is refused, and nothing is written (not
/// even an owed record); one within it is written.
#[test]
fn el1_m6_c4_an_acknowledgement_beyond_the_store_is_refused() {
    let (_scratch, path) = unsettled_store();
    let evidence = PrimaryEvidence::open(&path, vec![]).unwrap();
    evidence.store().refuse_inserts(true);
    assert_eq!(
        evidence.record_end(&EvidenceRecord::new(EvidencePhase::RunCancelled, 1, 1)),
        EndStatus::Owed
    );
    evidence.store().refuse_inserts(false);
    let rows = evidence.store().rows();
    let last = rows.last().unwrap().seq;
    assert!(evidence.acknowledge(last + 1).is_err());
    assert_eq!(evidence.store().rows(), rows, "nothing is written");
    assert_eq!(evidence.health().owed, 1);
    assert!(evidence.health().recovery_required);
    let seq = evidence.acknowledge(last).expect("within the store");
    assert_eq!(evidence.health().owed, 0);
    assert!(!evidence.health().recovery_required);
    assert_eq!(evidence.store().rows().last().unwrap().seq, seq);
}

/// EL-1 §8 as amended by C-7 (P3-C11-M6): the status's `unsettled` counts
/// only the statements no acknowledgement covers: after one, it is 0,
/// recovery is not required and the unclean sessions are unchanged; a
/// restart after it does not start stopped.
#[test]
fn el1_m6_c7_after_an_acknowledgement_nothing_is_unsettled() {
    let (scratch, path) = unsettled_store();
    let (evidence, authority) = authority_over(&path);
    let before = evidence.health();
    assert_eq!(
        (
            before.unsettled,
            before.unclean_sessions,
            before.recovery_required
        ),
        (1, 1, true)
    );
    authority.resume(&owner(true, true)).unwrap();
    let after = evidence.health();
    assert_eq!(after.unsettled, 0, "an acknowledged statement is settled");
    assert!(!after.recovery_required);
    assert_eq!(after.unclean_sessions, before.unclean_sessions);
    drop(authority);
    drop(evidence);
    let evidence = PrimaryEvidence::open(&path, vec![]).unwrap();
    assert_eq!(evidence.health().unsettled, 0);
    assert!(!evidence.health().recovery_required);
    let governed = crate::governed::GovernedControl::with_secrets(
        &scratch.path("runtime"),
        crate::broker::Vault::Disabled.source(),
        evidence.clone(),
        Arc::new(ManualClock::default()),
    )
    .unwrap();
    assert!(!governed.status().emergency_stopped);
}

// ── P3-C11-M6 C-9: reconciliation, the close and the acknowledgement ────────

/// An effect that completes at once.
struct Done;

impl PendingEffect for Done {
    fn revalidate(&self) -> Result<Digest, AuthorityError> {
        Ok(Digest::of("fake.target", &[b"here"]))
    }
    fn parameters(&self) -> Digest {
        Digest::of("fake.params", &[b"p"])
    }
    fn execute(
        self: Box<Self>,
        _guard: &ExecutionGuard,
    ) -> Result<EffectOutput, (FailureClass, String)> {
        Ok(EffectOutput::default())
    }
}

/// One effect, proposed, authorized and executed to its end through a
/// control over `evidence`; its commitment.
fn one_finished_effect(evidence: &Arc<PrimaryEvidence>) -> String {
    let owner = owner(true, true);
    let control = Control::new(Authority::new(
        evidence.clone(),
        Arc::new(ManualClock::default()),
    ));
    let agent = AgentId::new("agent-done").unwrap();
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
            &owner,
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
                effect: Box::new(Done),
                ttl: Duration::from_secs(60),
            },
        )
        .unwrap();
    control.authorize(view.id, &agent, run, &owner).unwrap();
    let output = control.execute(view.id, &agent, run).unwrap();
    assert!(matches!(
        output.evidence.map(|e| e.end),
        Some(EndStatus::Recorded { .. })
    ));
    view.id.to_string()
}

/// EL-1 §7 (C-9 M-A5): an effect that finished is not unsettled: after a
/// restart, whether the session closed cleanly or not, there is no
/// `unsettled` statement, no recovery, and governed control does not start
/// stopped.
#[test]
fn el1_m6_ma5_a_finished_effect_is_settled_after_a_clean_or_unclean_restart() {
    for clean in [true, false] {
        let scratch = Scratch::new();
        let path = scratch.store();
        let commitment = {
            let evidence = PrimaryEvidence::open(&path, vec![]).unwrap();
            let commitment = one_finished_effect(&evidence);
            if clean {
                assert!(matches!(
                    evidence.close_session(),
                    EndStatus::Recorded { .. }
                ));
            }
            commitment
        };
        let evidence = PrimaryEvidence::open(&path, vec![]).unwrap();
        let rows = evidence.store().rows();
        assert!(rows
            .iter()
            .any(|r| r.phase == "finished" && r.commitment.as_deref() == Some(&commitment)));
        assert!(
            !rows.iter().any(|r| r.phase == "unsettled"),
            "clean={clean}"
        );
        assert_eq!(
            rows.iter().filter(|r| r.phase == "unclean_session").count(),
            usize::from(!clean)
        );
        assert!(!evidence.health().recovery_required);
        let governed = crate::governed::GovernedControl::with_secrets(
            &scratch.path("runtime"),
            crate::broker::Vault::Disabled.source(),
            evidence.clone(),
            Arc::new(ManualClock::default()),
        )
        .unwrap();
        assert!(!governed.status().emergency_stopped, "clean={clean}");
    }
}

/// One `run_cancelled` end record owed (the store refuses it), then the
/// store accepts again.
fn owe_one(evidence: &PrimaryEvidence) {
    evidence.store().refuse_inserts(true);
    assert_eq!(
        evidence.record_end(&EvidenceRecord::new(EvidencePhase::RunCancelled, 1, 1)),
        EndStatus::Owed
    );
    evidence.store().refuse_inserts(false);
    assert_eq!(evidence.health().owed, 1);
}

/// EL-1 §6, §7 (C-9 M-A6): a clean shutdown writes the owed records first:
/// owed is then 0, and the owed row comes before `session_closed`.
#[test]
fn el1_m6_ma6_the_close_pays_what_is_owed_first() {
    let scratch = Scratch::new();
    let evidence = PrimaryEvidence::open(&scratch.store(), vec![]).unwrap();
    owe_one(&evidence);
    assert!(matches!(
        evidence.close_session(),
        EndStatus::Recorded { .. }
    ));
    assert_eq!(evidence.health().owed, 0);
    let rows = evidence.store().rows();
    assert_eq!(
        phases(&rows[rows.len() - 2..]),
        ["run_cancelled", "session_closed"]
    );
}

/// EL-1 §6, §7 (C-9 M-A7): with recovery required and one end record owed,
/// the acknowledgement writes the owed record first: owed is then 0, and the
/// owed row comes before the `acknowledged` row.
#[test]
fn el1_m6_ma7_the_acknowledgement_pays_what_is_owed_first() {
    let (_scratch, path) = unsettled_store();
    let (evidence, authority) = authority_over(&path);
    assert!(evidence.health().recovery_required);
    owe_one(&evidence);
    authority.resume(&owner(true, true)).unwrap();
    assert_eq!(evidence.health().owed, 0);
    let rows = evidence.store().rows();
    assert_eq!(
        phases(&rows[rows.len() - 2..]),
        ["run_cancelled", "acknowledged"]
    );
}

/// EL-1 §7 (C-9 M-A8): what was acknowledged is not shown again: after a
/// first acknowledgement whose highest shown statement is an
/// `unclean_session`, a second dialog (forced by a newer `unsettled`
/// statement) shows only the statements not yet acknowledged.
#[test]
fn el1_m6_ma8_an_acknowledged_unclean_session_is_not_shown_again() {
    let (_scratch, path) = unsettled_store();
    let (evidence, authority) = authority_over(&path);
    let unclean_seq = evidence
        .store()
        .rows()
        .iter()
        .find(|r| r.phase == "unclean_session")
        .unwrap()
        .seq;
    let first = owner(true, true);
    authority.resume(&first).unwrap();
    let shown = first.asked_acknowledgement.lock().unwrap()[0].clone();
    assert_eq!(
        shown.covers_through, unclean_seq,
        "the highest shown is the unclean session"
    );
    assert_eq!(shown.unclean_sessions.len(), 1);
    assert!(!evidence.health().recovery_required);
    let body = statement(
        "unsettled",
        serde_json::json!({ "commitment": "cmt-later", "started_seq": 1, "text": UNSETTLED_TEXT }),
    );
    assert!(matches!(
        evidence
            .store()
            .append("unsettled", Some("cmt-later"), &body),
        Append::Committed { .. }
    ));
    assert!(evidence.health().recovery_required);
    let second = owner(true, true);
    authority.resume(&second).unwrap();
    let shown = second.asked_acknowledgement.lock().unwrap()[0].clone();
    assert_eq!(shown.unsettled, ["cmt-later".to_string()]);
    assert!(
        shown.unclean_sessions.is_empty(),
        "shown again: {:?}",
        shown.unclean_sessions
    );
}
