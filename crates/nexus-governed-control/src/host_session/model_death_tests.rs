//! SB-2a real registration death prefixes. Attempt history is SB-2b.
use super::*;
#[test]
fn sb2_restart_has_no_model_authority_or_network() {
    const KEY: &str = "NEXUS_SB2A_DEATH_CHILD";
    if let Ok(point) = std::env::var(KEY) {
        let f = super::model_tests::fixture();
        let at = match point.as_str() {
            "before_record" => "model.record.before",
            "recorded" => "model.install.before",
            "installed" => "model.install.after",
            _ => panic!("unknown death boundary"),
        };
        let root = f.root.0.path().to_owned();
        f.session.core.cell.1.arm(at, move || {
            println!("SB2A_ROOT={}", root.display());
            use std::io::Write;
            std::io::stdout().flush().unwrap();
            std::process::abort();
        });
        f.session
            .register_model_private(
                &std::env::var("NEXUS_SB2A_PEER").unwrap(),
                &super::model_tests::pin(),
            )
            .unwrap();
        panic!("death boundary not reached");
    }
    for point in ["before_record", "recorded", "installed"] {
        let peer = super::model_tests::Peer::new(None, None);
        let mut child = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "host_session::model_death_tests::sb2_restart_has_no_model_authority_or_network",
                "--nocapture",
            ])
            .env(KEY, point)
            .env("NEXUS_SB2A_PEER", &peer.endpoint)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(10);
        while child.try_wait().unwrap().is_none() {
            if Instant::now() >= deadline {
                child.kill().unwrap();
                child.wait().unwrap();
                panic!("registration death child exceeded outer watchdog: {point}");
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        let output = child.wait_with_output().unwrap();
        assert!(!output.status.success());
        let text = String::from_utf8_lossy(&output.stdout);
        let root = text
            .lines()
            .find_map(|l| l.strip_prefix("SB2A_ROOT="))
            .unwrap_or_else(|| {
                panic!(
                    "death boundary {point} not reached: {text}{}",
                    String::from_utf8_lossy(&output.stderr)
                )
            });
        peer.prefix(4);
        for _ in 0..2 {
            let evidence = crate::evidence_store::PrimaryEvidence::open(
                &std::path::Path::new(root).join("p3-evidence/evidence.sqlite3"),
                vec![],
            )
            .unwrap();
            let session = HostSession::open(
                SessionSeed::new().unwrap(),
                std::path::Path::new(root),
                evidence.clone(),
                Arc::new(crate::authority::clock::SystemClock::default()),
                Arc::new(crate::harness_tests::Yes::new(true)),
            )
            .unwrap();
            assert!(!session.core.model.installed(), "replay_has_no_authority");
            assert_eq!(
                session.core.model.snapshot().unwrap().operation,
                "none",
                "replay_has_no_authority"
            );
            assert!(
                !session.startup_report().ready(),
                "report-only unresolved restart"
            );
            assert_eq!(
                evidence
                    .store()
                    .rows()
                    .iter()
                    .any(|r| r.phase == "model_registered"),
                point != "before_record"
            );
            peer.prefix(4); // Restart issued zero metadata/chat; no registration retry.
        }
        std::fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn sb2_attempt_death_prefixes_are_report_only() {
    const KEY: &str = "NEXUS_SB2B_DEATH_CHILD";
    if let Ok(point) = std::env::var(KEY) {
        let f = super::model_tests::fixture();
        let account = super::model_tests::Accounting::new(f.primary.clone());
        let mut attempt = account.acquire().unwrap();
        let die = || {
            println!("SB2B_ROOT={}", f.root.0.path().display());
            use std::io::Write;
            std::io::stdout().flush().unwrap();
            std::process::abort();
        };
        if point == "ticket" {
            die();
        }
        if point == "record_before" {
            let core = Arc::downgrade(&account.seed.core);
            account
                .seed
                .core
                .cell
                .1
                .arm("model.attempt.record.before", move || {
                    assert!(core.upgrade().is_some());
                    std::process::abort();
                });
            // Print before the real record hook so the parent can verify the prefix.
            println!("SB2B_ROOT={}", f.root.0.path().display());
            use std::io::Write;
            std::io::stdout().flush().unwrap();
        }
        attempt
            .record_return(
                &account.authority,
                Err(crate::inference::ModelError::Cancelled),
            )
            .unwrap();
        if point == "recorded" {
            die();
        }
        attempt.complete().unwrap();
        if point == "settled" {
            die();
        }
        panic!("unreached death boundary");
    }
    for point in ["ticket", "record_before", "recorded", "settled"] {
        let mut child = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "host_session::model_death_tests::sb2_attempt_death_prefixes_are_report_only",
                "--nocapture",
            ])
            .env(KEY, point)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        let limit = Instant::now() + Duration::from_secs(10);
        while child.try_wait().unwrap().is_none() {
            if Instant::now() >= limit {
                child.kill().unwrap();
                child.wait().unwrap();
                panic!("attempt death watchdog {point}");
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        let output = child.wait_with_output().unwrap();
        assert!(!output.status.success());
        let text = String::from_utf8_lossy(&output.stdout);
        let root = text
            .lines()
            .find_map(|l| l.strip_prefix("SB2B_ROOT="))
            .unwrap_or_else(|| {
                panic!(
                    "unreached death {point}: {text}{}",
                    String::from_utf8_lossy(&output.stderr)
                )
            });
        for _ in 0..2 {
            let evidence = crate::evidence_store::PrimaryEvidence::open(
                &std::path::Path::new(root).join("p3-evidence/evidence.sqlite3"),
                vec![],
            )
            .unwrap();
            let session = HostSession::open(
                SessionSeed::new().unwrap(),
                std::path::Path::new(root),
                evidence.clone(),
                Arc::new(crate::authority::clock::SystemClock::default()),
                Arc::new(crate::harness_tests::Yes::new(true)),
            )
            .unwrap();
            let rows = evidence.store().rows(); // Reopened, qualified existing store, not recent_evidence.
            let ends: Vec<_> = rows.iter().filter(|r| r.phase == "model_call").collect();
            let established = matches!(point, "recorded" | "settled");
            assert_eq!(ends.len(), usize::from(established));
            let status = model::historical_outcome(
                ends.first()
                    .map(|r| crate::authority::evidence::EndStatus::Recorded { seq: r.seq }),
            );
            assert_eq!(
                status,
                if established { "recorded" } else { "unknown" },
                "missing attempt outcome stays unknown"
            );
            assert_eq!(session.core.model.snapshot().unwrap().operation, "none");
            assert!(!session.core.model.installed());
            assert!(
                !session.startup_report().ready(),
                "unresolved restart is report-only"
            );
            assert_eq!(
                session.control.authority().runs().review2_counts(),
                [0; 4],
                "no recreated run, consent, admission or attempt"
            );
        }
        std::fs::remove_dir_all(root).unwrap();
    }
}
