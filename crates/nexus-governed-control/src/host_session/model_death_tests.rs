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

/// SB-3B2 X1-B: real private-call death around the actual ticket, the
/// dispatch (killed while the chat is held open), the reply, the record and
/// the slot completion. Only verified prefixes; every restart is
/// report-only: no request, registration, run, consent, binding or attempt.
#[test]
fn sb3_private_call_death_restarts_report_only() {
    const KEY: &str = "NEXUS_SB3B2_DEATH_CHILD";
    if let Ok(point) = std::env::var(KEY) {
        let f = super::model_call_tests::Call::at(&std::env::var("NEXUS_SB3B2_PEER").unwrap());
        let [root, selected] = f.b.roots();
        println!(
            "SB3B2_ROOT={}\nSB3B2_SELECTED={}",
            root.display(),
            selected.display()
        );
        use std::io::Write;
        std::io::stdout().flush().unwrap();
        let at = match point.as_str() {
            "ticket" => Some("model.call.gate"),
            "reply" => Some("model.attempt.return"),
            "record" => Some("model.settle.before"),
            _ => None, // "dispatch": the parent kills it; "completed": below.
        };
        if let Some(at) = at {
            f.b.session.core.cell.1.arm(at, || std::process::abort());
        }
        let outcome = f.call().expect("ticketed attempt");
        if point == "completed" && outcome.result().is_ok() {
            std::process::abort();
        }
        panic!("death boundary not reached: {point}");
    }
    let call = ["/api/version", "/api/status", "/api/tags", "/api/chat"];
    for point in ["ticket", "dispatch", "reply", "record", "completed"] {
        let peer = super::model_tests::Peer::chat((point == "dispatch").then_some(7), None);
        let mut child = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "host_session::model_death_tests::sb3_private_call_death_restarts_report_only",
                "--nocapture",
            ])
            .env(KEY, point)
            .env("NEXUS_SB3B2_PEER", &peer.endpoint)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        let limit = Instant::now() + Duration::from_secs(20);
        if point == "dispatch" {
            for _ in 0..8 {
                let (_, closed) = peer.events.recv_timeout(Duration::from_secs(10)).unwrap();
                assert!(!closed, "dispatch: the chat entered and is held");
            }
            child.kill().unwrap(); // Killed while its chat request is open.
        }
        while child.try_wait().unwrap().is_none() {
            if Instant::now() >= limit {
                child.kill().unwrap();
                child.wait().unwrap();
                panic!("private call death watchdog {point}");
            }
            std::thread::sleep(Duration::from_millis(5)); // Polls exit only.
        }
        let output = child.wait_with_output().unwrap();
        assert!(!output.status.success(), "{point}: the child died");
        let text = String::from_utf8_lossy(&output.stdout);
        let line = |key: &str| {
            text.lines()
                .find_map(|l| l.strip_prefix(key))
                .map(str::to_owned)
        };
        let (Some(root), Some(selected)) = (line("SB3B2_ROOT="), line("SB3B2_SELECTED=")) else {
            panic!(
                "{point}: unreached: {text}{}",
                String::from_utf8_lossy(&output.stderr)
            );
        };
        let sent = if point == "ticket" {
            &call[..0]
        } else {
            &call[..]
        };
        assert_eq!(
            peer.paths.lock().unwrap()[4..],
            *sent,
            "{point}: verified call prefix"
        );
        let established = matches!(point, "record" | "completed");
        for _ in 0..2 {
            let evidence = crate::evidence_store::PrimaryEvidence::open(
                &std::path::Path::new(&root).join("p3-evidence/evidence.sqlite3"),
                vec![],
            )
            .unwrap();
            let session = HostSession::open(
                SessionSeed::new().unwrap(),
                std::path::Path::new(&root),
                evidence.clone(),
                Arc::new(crate::authority::clock::SystemClock::default()),
                Arc::new(crate::harness_tests::Yes::new(true)),
            )
            .unwrap();
            let rows = evidence.store().rows();
            let ends: Vec<_> = rows.iter().filter(|r| r.phase == "model_call").collect();
            assert_eq!(
                ends.len(),
                usize::from(established),
                "{point}: verified ends only"
            );
            let status = model::historical_outcome(
                ends.first()
                    .map(|r| crate::authority::evidence::EndStatus::Recorded { seq: r.seq }),
            );
            let expected = if established { "recorded" } else { "unknown" };
            assert_eq!(status, expected, "{point}: a missing outcome stays unknown");
            assert!(
                !session.core.model.installed(),
                "{point}: no restored registration"
            );
            assert_eq!(
                session.core.model.snapshot().unwrap().operation,
                "none",
                "{point}"
            );
            assert!(
                !session.startup_report().ready(),
                "{point}: report-only restart"
            );
            assert_eq!(
                session.control.authority().runs().review2_counts(),
                [0; 4],
                "{point}: no restored run, consent, binding or attempt"
            );
            let after = peer.paths.lock().unwrap().len();
            assert_eq!(after, 4 + sent.len(), "{point}: restart metadata/chat 0/0");
        }
        std::fs::remove_dir_all(root).unwrap();
        std::fs::remove_dir_all(selected).unwrap();
    }
}
