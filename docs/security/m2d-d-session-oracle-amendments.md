# M2d-D session oracle amendments

Authority: frozen session spec §5 and amendment B A1–A12; Decision 12 amendment A.

The following exact old/new assertions are committed before the production behavior they judge. Baseline historical reports remain in Git.

## mother/host/tests/faults.rs

```diff
--- 03e07b1a/mother/host/tests/faults.rs
+++ session/mother/host/tests/faults.rs
@@ -87,7 +87,7 @@
 }

 #[test]
-fn recovery_ordering_b_publication_then_separate_lift_and_undo() {
+fn emergency_ordering_b_preserves_publication_and_refuses_restoration() {
     let f = Fixture::new();
     let fault = Fault::new(&f, "after-final-check");
     let mut command = f.command("one handed-off publication\n");
@@ -113,23 +113,19 @@
     p.line("status");
     p.expect("STATUS END\n");
     let before = p.output.matches("LANE invoked").count();
-    p.line("lift-for-undo");
-    p.yes();
-    p.expect("LIFTED recovery only; missions remain stopped\n");
-    p.line("undo");
-    p.yes();
-    p.expect("UNDO REMOVED interrupted=none\n");
-    f.empty();
-    for command in ["run", "approve"] {
+    for command in ["lift-for-undo", "undo", "cleanup", "run", "approve", "detail"] {
+        let dialogues = p.output.matches("Answer code:").count();
         p.line(command);
-        p.expect("REFUSED recovery_only\n");
+        p.expect("REFUSED");
+        assert_eq!(p.output.matches("Answer code:").count(), dialogues);
     }
     assert_eq!(p.output.matches("LANE invoked").count(), before);
+    assert_eq!(std::fs::read(f.workspace.join("hello.txt")).unwrap(), b"one handed-off publication\n");
     p.line("status");
-    p.expect("host_recovery_only=true");
-    p.expect("kernel_stopped=false broker_stopped=false");
+    p.expect("kernel_stopped=true broker_stopped=true");
     p.expect("STATUS END\n");
-    p.quit();
+    p.line("quit");
+    p.finish();
 }

 #[test]
@@ -151,7 +147,7 @@
     }
     p.expect("STOP COALESCED");
     p.line("lift-for-undo");
-    p.expect("REFUSED stop_workers_outstanding\n");
+    p.expect("REFUSED unavailable\n");
     receipt.release();
     p.expect("STOP WORKERS outstanding=0\n");
     p.line("evidence");
@@ -257,7 +253,7 @@
     p.line("stop");
     p.expect("STOP REQUEST RECEIVED epoch=2");
     p.line("lift-for-undo");
-    p.expect("REFUSED stop_workers_outstanding\n");
+    p.expect("REFUSED unavailable\n");
     receipt.release();
     p.expect("STOP WORKERS outstanding=0\n");
     p.line("evidence");
@@ -267,66 +263,29 @@
     p.quit();
 }

-fn activity(p: &mut Pty) -> String {
-    p.line("status");
-    p.expect("STATUS END\n")
-        .lines()
-        .find(|line| line.starts_with("ADMITTED "))
-        .expect("all seven host admission counters")
-        .to_string()
-}
-
-#[test]
-fn recovery_composed_counts_include_queued_callbacks_after_successful_undo() {
+#[test]
+fn emergency_composition_preserves_file_and_rejects_queued_commands() {
     let f = Fixture::new();
     let fault = Fault::new(&f, "after-final-check");
     let mut command = f.command("one publication\n");
     fault.attach(&f, &mut command);
     let mut p = Pty::start(command);
-    p.ready();
-    p.mission();
-    fault.arm();
-    p.line("approve");
-    p.yes();
+    p.ready(); p.mission(); fault.arm();
+    p.line("approve"); p.yes();
     let receipt = fault.reached();
-    let before = activity(&mut p);
-    for route in ["mission", "door", "run", "files", "lane"] {
-        assert!(
-            !before.contains(&format!("{route}=0")),
-            "positive {route}: {before}"
-        );
-    }
-    assert!(before.contains("inference=0 model=0"));
-    p.line("run");
-    p.line("approve");
-    p.line("stop");
+    let before = p.output.matches("LANE invoked").count();
+    p.line("run"); p.line("approve"); p.line("stop");
     p.expect("STOP RECORDING COMPLETED");
-    receipt.release();
-    p.expect("ACTION FINISHED");
-    assert_eq!(
-        std::fs::read(f.workspace.join("hello.txt")).unwrap(),
-        b"one publication\n"
-    );
-    p.line("lift-for-undo");
-    p.yes();
-    p.expect("LIFTED recovery only; missions remain stopped\n");
-    p.line("undo");
-    p.yes();
-    p.expect("UNDO REMOVED interrupted=none\n");
-    f.empty();
-    assert_eq!(
-        activity(&mut p),
-        before,
-        "no post-emergency admission, including queued callbacks"
-    );
-    p.line("stop");
-    p.expect("STOP RECORDING COMPLETED");
-    assert_eq!(
-        activity(&mut p),
-        before,
-        "restriction survives repeated Stop"
-    );
-    p.quit();
+    receipt.release(); p.expect("ACTION FINISHED");
+    for _ in 0..2 {
+        p.line("stop"); p.expect("STOP RECORDING COMPLETED");
+        for command in ["lift-for-undo", "undo", "cleanup", "run", "approve"] {
+            p.line(command); p.expect("REFUSED");
+        }
+        assert_eq!(p.output.matches("LANE invoked").count(), before);
+        assert_eq!(std::fs::read(f.workspace.join("hello.txt")).unwrap(), b"one publication\n");
+    }
+    p.line("quit"); p.finish();
 }

 #[test]
@@ -338,26 +297,20 @@
     let mut p = Pty::start(command);
     p.ready();
     p.mission();
-    fault.arm();
-    p.line("stop");
-    p.expect("STOP RECORDING COMPLETED");
-    p.line("lift-for-undo");
-    p.yes();
-    p.expect("LIFTED recovery only; missions remain stopped\n");
-    // No undo slot exists, but this Owner call first drains the candidate
-    // transferred by Stop. Hold actual unlink of that candidate.
-    p.line("undo");
+    // Denial retires the preparation without removing its owned staging.
+    p.line("approve"); p.code(); p.line("no"); p.expect("ACTION DENIED\n");
+    fault.arm();
+    p.line("cleanup");
     let receipt = fault.reached();
     assert!(receipt.path.contains("/coding-runs/"));
-    p.line("stop");
-    p.expect("kernel_stopped=true broker_stopped=true");
-    p.expect("STOP RECORDING COMPLETED");
-    p.line("status");
-    p.expect("STATUS END\n");
-    receipt.release();
-    p.expect("UNDO REFUSED");
-    f.empty();
-    p.quit();
+    for _ in 0..2 {
+        p.line("stop"); p.expect("STOP RECORDING COMPLETED");
+    }
+    p.line("status"); p.expect("STATUS END\n");
+    receipt.release(); p.expect("CLEANUP");
+    p.line("cleanup"); p.expect("REFUSED");
+    f.empty();
+    p.line("quit"); p.finish();
 }

 #[test]
@@ -369,18 +322,11 @@
     let mut p = Pty::start(command);
     p.ready();
     p.mission();
-    fault.arm();
-    p.line("stop");
-    p.expect("STOP RECORDING COMPLETED");
-    p.line("lift-for-undo");
-    p.yes();
-    p.expect("LIFTED recovery only; missions remain stopped\n");
-    p.line("undo");
-    p.expect("UNDO REFUSED");
-    p.line("cleanup");
-    p.expect("CLEANUP settled=1 still_owed=0\n");
-    f.empty();
-    p.quit();
+    p.line("approve"); p.code(); p.line("no"); p.expect("ACTION DENIED\n");
+    fault.arm();
+    p.line("cleanup"); p.expect("CLEANUP");
+    p.line("cleanup"); p.expect("CLEANUP settled=1 still_owed=0\n");
+    f.empty(); p.quit();
 }

 #[test]
```

## mother/host/tests/pty.rs

```diff
--- 03e07b1a/mother/host/tests/pty.rs
+++ session/mother/host/tests/pty.rs
@@ -206,9 +206,8 @@
     p.line("run");
     p.expect("REFUSED recovery_only\n");
     p.line("lift-for-undo");
-    p.yes();
-    p.expect("LIFTED recovery only; missions remain stopped\n");
-    for command in ["run", "approve"] {
+    p.expect("REFUSED unavailable\n");
+    for command in ["run", "approve", "undo", "cleanup", "detail"] {
         p.line(command);
         p.expect("REFUSED recovery_only\n");
     }
@@ -378,39 +377,30 @@
 }

 #[test]
-fn recovery_repeated_stop_invalidates_lift_and_undo_consents() {
-    let f = Fixture::new();
-    let mut p = Pty::start(f.command("recover once\n"));
+fn first_emergency_invalidates_normal_undo_and_repeated_stop_never_rearms() {
+    let f = Fixture::new();
+    let mut p = Pty::start(f.command("normal undo only\n"));
     p.ready();
     p.create();
-    p.line("stop");
-    p.expect("STOP RECORDING COMPLETED");
-    p.line("lift-for-undo");
-    let old = p.code();
-    p.line("stop");
-    p.expect("LIFT REFUSED");
-    p.expect("STOP RECORDING COMPLETED");
-    p.line("lift-for-undo");
-    let fresh = p.code();
-    assert_ne!(fresh, old);
-    p.line(&fresh);
-    p.expect("LIFTED recovery only; missions remain stopped\n");
     p.line("undo");
     let old = p.code();
     p.line("stop");
     p.expect("UNDO REFUSED");
     p.expect("STOP RECORDING COMPLETED");
     assert!(f.workspace.join("hello.txt").exists());
-    p.line("lift-for-undo");
-    p.yes();
-    p.expect("LIFTED recovery only; missions remain stopped\n");
-    p.line("undo");
-    let fresh = p.code();
-    assert_ne!(fresh, old);
-    p.line(&fresh);
-    p.expect("UNDO REMOVED interrupted=none\n");
-    f.empty();
-    p.quit();
+    p.line(&old);
+    p.expect("REFUSED");
+    p.line("stop");
+    p.expect("STOP RECORDING COMPLETED");
+    for command in ["lift-for-undo", "undo", "cleanup"] {
+        let before = p.output.matches("Answer code:").count();
+        p.line(command);
+        p.expect("REFUSED");
+        assert_eq!(p.output.matches("Answer code:").count(), before);
+    }
+    assert_eq!(std::fs::read(f.workspace.join("hello.txt")).unwrap(), b"normal undo only\n");
+    p.line("quit");
+    p.finish();
 }

 #[test]
```

## Baseline evidence and limits

No production code changed for this checkpoint. The baseline suite passed five
model/manifest tests and failed the Hands source boundary. Its first PTY attempt
was blocked by group-writable ancestors under the workspace `target`; that is
fixture environment evidence, not behavioral red evidence. With private fixture
roots under the authorized session scratch directory, both real PTY oracles
failed: emergency lift opened a new dialogue, and emergency cleanup returned
`CLEANUP settled=0 still_owed=0` while pending staging remained.

The manifest currently names 37 catalogue rows and 130 sinks (75 W/R). Its
153,000 generated keys specify required schedules; they are not yet 153,000
executed production schedules. Model-only enumeration covers 369,600 traces
with three actors and three interfering Stop events and 4,160 literal
mission/cause/publication/ack/record cases. These are bounded claims.

The frozen 23 desktop live names are in
`mother/host/tests/session/desktop-live.txt`; no desktop behavior changed.

Remaining oracle work before production: bind every generated key to an actual
route or checked structural exclusion; complete the call/ownership ledgers,
capacity/poison/death fixtures and per-transfer plants; migrate the listed
Decision 10/11 host-only assertions and synthetic gate claims. Legacy controls
and frozen C-lite tests remain unchanged. This checkpoint is not completion of
the oracle-first exit gate.

## Decision 11 outcome notices and synthetic Gate authority claims

Session spec §5 and B A12 replace unconditional confirmation language. Gate-only
tests retain their presentation checks and establish no backend authority.
The new lift test first had an unavailable integration-binary fixture in the
unit-test target. It now uses the existing self-reexecuting unit-test pattern;
expected answers did not change. On the real route it fails at support/mod.rs:182:
expected `REFUSED unavailable`, observed `REFUSED recovery_only_lift`.
This is distinct from the emergency-dialogue behavioral red above.

### mother/host/src/stop_tests.rs

```diff
--- 03e07b1a/mother/host/src/stop_tests.rs
+++ session/mother/host/src/stop_tests.rs
@@ -1,10 +1,13 @@
 //! Registered before the Stop implementation. All waits use readiness.
+//! Session-spec §5 amendment: the two Gate tests below check presentation
+//! projections only. They establish no backend authority or sink coverage.
+//! Session-sink schedules and real-route controls are separately required.
 use crate::stop::{Gate, Route, StopRoute};
 use std::sync::{Arc, Condvar, Mutex};
 use std::time::{Duration, Instant};

 #[test]
-fn m2dd_recovery_gate_covers_each_route_and_reserved_callbacks() {
+fn m2dd_presentation_gate_projects_each_route_and_reserved_callbacks() {
     let gate = Gate::default();
     for route in Route::ALL {
         assert!(gate.admit(route));
@@ -65,7 +68,7 @@
 }

 #[test]
-fn m2dd_admission_counters_have_positive_controls_for_all_routes() {
+fn m2dd_presentation_counters_have_positive_controls_for_all_labels() {
     let gate = Gate::default();
     for route in Route::ALL {
         assert!(gate.admit(route));
```

### mother/host/src/hands_tests.rs

```diff
--- 03e07b1a/mother/host/src/hands_tests.rs
+++ session/mother/host/src/hands_tests.rs
@@ -509,16 +509,22 @@
         } else {
             "was refused"
         };
-        p.expect(&format!(
-            "Stop received while your confirmed undo was finishing; the undo {outcome}."
-        ));
+        p.expect(&if outcome == "completed" {
+            "Stop received after undo handoff. The restore completed; remaining obligations are reported separately.".to_owned()
+        } else {
+            "Stop received after undo handoff. The restore was refused.".to_owned()
+        });
         assert_eq!(
             f.workspace.join("hello.txt").exists(),
             scenario == "undo-refused"
         );
     } else {
         p.line("cleanup");
-        p.expect("Stop received while your confirmed cleanup was finishing; cleanup settled=0 still_owed=0.");
+        p.expect("CLEANUP");
+        assert!(
+            !p.output.contains("confirmed cleanup"),
+            "cleanup is requested, never natively confirmed"
+        );
     }
     p.expect("REVIEW4 NO LATER WORK");
     // The reporter may have emitted the receipt earlier; inspect full output.
@@ -546,6 +552,40 @@
     review4_outcome("cleanup", "decision11_host_cleanup_is_honest");
 }
 #[test]
-fn decision11_host_lift_reengages_before_next_action() {
-    review4_outcome("lift", "decision11_host_lift_reengages_before_next_action");
-}
+fn session_lift_is_unavailable_before_and_after_emergency_without_dialogue() {
+    const CHILD: &str = "M2DD_SESSION_LIFT_CHILD";
+    if let Ok(args) = std::env::var(CHILD) {
+        assert!(run(args.split('\n').map(str::to_owned)));
+        return;
+    }
+    let f = Fixture::new();
+    let mut command = std::process::Command::new(std::env::current_exe().unwrap());
+    command
+        .args([
+            "--exact",
+            "hands_tests::session_lift_is_unavailable_before_and_after_emergency_without_dialogue",
+            "--nocapture",
+        ])
+        .env(
+            CHILD,
+            format!(
+                "--home\n{}\n--workspace\n{}\n--name\nhello.txt\n--text\nno lift authority",
+                f.home.display(),
+                f.workspace.display()
+            ),
+        );
+    let mut p = Pty::start(command);
+    p.ready();
+    for emergency in [false, true] {
+        if emergency {
+            p.line("stop");
+            p.expect("STOP RECORDING COMPLETED");
+        }
+        let before = p.output.matches("Answer code:").count();
+        p.line("lift-for-undo");
+        p.expect("REFUSED unavailable\n");
+        assert_eq!(p.output.matches("Answer code:").count(), before);
+    }
+    f.empty();
+    p.quit();
+}
```

## Ruling R-1 and the remaining Decision 10/11 host amendments

The Architect accepted the disclosed `05239391` ordering deviation in Ruling
R-1. It is not a STOP. The next checkpoint must contain the remaining oracles
before another production commit. Every subsequent fix names that checkpoint.

The following replaces only the host-bound expectations expressly listed in
spec §5. The legacy cell-less recovery control and its `published_recovery`
setup are unchanged. Normal undo/cleanup now starts before the first emergency.
A later request refuses without dialogue or removal; native lift is unavailable
in every host lifecycle state. Entered-call completion remains permitted.

The current old-route fixtures are baseline adapters. They must migrate to the
mandatory facade; changing legacy semantics to satisfy these host expectations
is prohibited. Exact old/new assertions follow.

```diff
diff --git a/crates/nexus-governed-control/src/governed/files_host_decision10_tests.rs b/crates/nexus-governed-control/src/governed/files_host_decision10_tests.rs
index bdd1d534..cb686086 100644
--- a/crates/nexus-governed-control/src/governed/files_host_decision10_tests.rs
+++ b/crates/nexus-governed-control/src/governed/files_host_decision10_tests.rs
@@ -57,8 +57,15 @@ fn published_recovery() -> Fx {
     f
 }

+fn published_normal() -> Fx {
+    let f = Fx::new();
+    let a = f.ready(NAME, CONTENT);
+    f.execute(&a).unwrap();
+    f
+}
+
 fn undo_ingress(point: &'static str) {
-    let f = published_recovery();
+    let f = published_normal();
     let cell = f.gov.file_session_halt();
     let gate = Gate::new();
     let held = gate.clone();
@@ -66,7 +73,7 @@ fn undo_ingress(point: &'static str) {
     let (gov, owner, carried) = (f.gov.clone(), f.owner.clone(), cell.clone());
     let worker = std::thread::spawn(move || gov.files_undo_with_halt(&*owner, Some(&carried)));
     gate.reached();
-    cell.engage_emergency(); // repeated ingress, NO Stop worker
+    cell.engage_emergency(); // first emergency, NO Stop worker
     assert!(!f.gov.stop_snapshot().broker_stopped);
     assert!(!f.gov.stop_snapshot().kernel_stopped);
     gate.open();
@@ -82,10 +89,11 @@ fn undo_ingress(point: &'static str) {
         "typed stale ingress refusal"
     );
     assert!(!f.gov.files().write_grant_live_for_test());
-    // Same permanently engaged cell, but a NEW request/confirmation is valid.
-    f.gov.files_undo_with_halt(&*f.owner, Some(&cell)).unwrap();
-    assert!(!f.ws.join(NAME).exists());
-    assert_eq!(f.owner.undos().len(), 2);
+    // Host amendment: no fresh restore after the first emergency.
+    let asks = f.owner.undos().len();
+    assert!(matches!(f.gov.files_undo_with_halt(&*f.owner, Some(&cell)), Err(AuthorityError::Halted)));
+    assert!(f.ws.join(NAME).exists());
+    assert_eq!(f.owner.undos().len(), asks);
 }

 #[test]
@@ -99,8 +107,8 @@ fn decision10_undo_final_dispatch_then_ingress_no_worker() {
 }

 #[test]
-fn decision10_undo_dialogue_ingress_requires_new_request() {
-    let f = published_recovery();
+fn decision10_normal_undo_dialogue_emergency_forbids_new_request() {
+    let f = published_normal();
     let cell = f.gov.file_session_halt();
     let ingress = cell.clone();
     f.owner.during_undo(move || ingress.engage_emergency());
@@ -110,8 +118,10 @@ fn decision10_undo_dialogue_ingress_requires_new_request() {
         "dialogue ingress must not remove the file"
     );
     assert!(matches!(result, Err(AuthorityError::Halted)));
-    f.gov.files_undo_with_halt(&*f.owner, Some(&cell)).unwrap();
-    assert!(!f.ws.join(NAME).exists());
+    let asks = f.owner.undos().len();
+    assert!(matches!(f.gov.files_undo_with_halt(&*f.owner, Some(&cell)), Err(AuthorityError::Halted)));
+    assert!(f.ws.join(NAME).exists());
+    assert_eq!(f.owner.undos().len(), asks);
 }

 #[test]
@@ -126,7 +136,6 @@ fn decision10_cleanup_ingress_before_effect_preserves_obligation() {
     assert_eq!(f.gov.files().kept_for_test(), 1);
     std::fs::set_permissions(&staging, std::fs::Permissions::from_mode(0o700)).unwrap();
     let cell = f.gov.file_session_halt();
-    cell.engage_emergency();
     let ingress = cell.clone();
     f.gov
         .files()
@@ -141,32 +150,22 @@ fn decision10_cleanup_ingress_before_effect_preserves_obligation() {
     );
     assert!(matches!(result, Err(AuthorityError::Halted)));
     assert_eq!(f.gov.files().kept_for_test(), 1);
-    let retry = f.gov.files_retry_cleanup_with_halt(Some(&cell)).unwrap();
-    assert_eq!((retry.settled, retry.still_owed), (1, 0));
-    assert!(!staging.exists());
+    assert!(matches!(f.gov.files_retry_cleanup_with_halt(Some(&cell)), Err(AuthorityError::Halted)));
+    assert!(staging.exists());
+    assert_eq!(f.gov.files().kept_for_test(), 1);
 }

 #[test]
-fn decision10_recovery_lift_ingress_after_record() {
-    let f = Fx::new();
-    f.gov.emergency_stop();
-    let cell = f.gov.file_session_halt();
-    let ingress = cell.clone();
-    f.gov
-        .authority()
-        .hooks()
-        .arm("resume.before_lift", move || ingress.engage_emergency());
-    let result = f.gov.resume_with_halt(&*f.owner, Some(&cell));
-    assert!(
-        f.gov.stop_snapshot().broker_stopped,
-        "ingress after resume record must not lift broker Stop"
-    );
-    assert!(f.gov.stop_snapshot().kernel_stopped);
-    assert!(matches!(result, Err(AuthorityError::Halted)));
-    f.gov.resume_with_halt(&*f.owner, Some(&cell)).unwrap();
-    assert!(!f.gov.stop_snapshot().broker_stopped);
-    assert!(!f.gov.stop_snapshot().kernel_stopped);
-    assert!(cell.is_engaged(), "lift never reopens the host session");
+fn decision10_host_lift_is_unavailable_before_any_native_record() {
+    for emergency in [false, true] {
+        let f = Fx::new();
+        if emergency { f.gov.emergency_stop(); }
+        let cell = f.gov.file_session_halt();
+        let before = f.evidence.records().len();
+        let result = f.gov.resume_with_halt(&*f.owner, Some(&cell));
+        assert!(matches!(result, Err(AuthorityError::Unavailable(_))));
+        assert_eq!(f.evidence.records().len(), before, "unsupported lift records no native resume");
+    }
 }

 fn selected() -> Fx {
@@ -258,7 +257,7 @@ fn decision10_legacy_cell_less_recovery_control() {
 // Decision 11: hooks are at the broker/kernel handoff, not inside kernel U1.
 #[test]
 fn decision11_undo_ingress_after_last_broker_check_reports_actual_removal() {
-    let f = published_recovery();
+    let f = published_normal();
     let cell = f.gov.file_session_halt();
     let ingress = cell.clone();
     f.gov
@@ -299,7 +298,6 @@ fn decision11_cleanup_ingress_after_handoff_preserves_actual_outcome() {
     assert_eq!(f.gov.files().kept_for_test(), 1);
     std::fs::set_permissions(&staging, std::fs::Permissions::from_mode(0o700)).unwrap();
     let cell = f.gov.file_session_halt();
-    cell.engage_emergency();
     let ingress = cell.clone();
     f.gov
         .files()
@@ -318,33 +316,18 @@ fn decision11_cleanup_ingress_after_handoff_preserves_actual_outcome() {
 }

 #[test]
-fn decision11_lift_ingress_at_and_after_kernel_call_reengages_native() {
+fn decision11_host_lift_cannot_reach_a_kernel_handoff() {
     for point in ["resume.kernel_boundary", "resume.kernel_returned"] {
         let f = Fx::new();
         f.gov.emergency_stop();
         let cell = f.gov.file_session_halt();
-        let ingress = cell.clone();
-        f.gov
-            .authority()
-            .hooks()
-            .arm(point, move || ingress.engage_emergency());
-        let result = f.gov.resume_with_halt(&*f.owner, Some(&cell));
-        assert!(
-            f.gov.stop_snapshot().kernel_stopped,
-            "{point}: native halt re-engaged before return"
-        );
+        let reached = Arc::new(AtomicUsize::new(0));
+        let seen = reached.clone();
+        f.gov.authority().hooks().arm(point, move || { seen.fetch_add(1, Ordering::SeqCst); });
+        assert!(matches!(f.gov.resume_with_halt(&*f.owner, Some(&cell)), Err(AuthorityError::Unavailable(_))));
+        assert_eq!(reached.load(Ordering::SeqCst), 0);
+        assert!(f.gov.stop_snapshot().kernel_stopped);
         assert!(f.gov.stop_snapshot().broker_stopped);
-        assert!(matches!(result, Err(AuthorityError::Halted)));
-        assert!(cell.is_engaged());
-        assert!(f.door.open_run().is_err());
-        assert!(
-            f.evidence
-                .records()
-                .iter()
-                .filter(|r| r.phase == EvidencePhase::EmergencyStop)
-                .count()
-                >= 2
-        );
     }
 }

@@ -371,9 +354,8 @@ fn decision11_undo_deferred_cleanup_rechecks_ingress_and_retains_tail() {
     let f = Fx::new();
     f.propose(NAME, CONTENT).unwrap();
     let staging = f.state().join("coding-runs").join(&f.staged()[0]);
-    f.gov.emergency_stop();
+    let _ = f.door.cancel_run(f.run).unwrap().write();
     let cell = f.gov.file_session_halt();
-    f.gov.resume_with_halt(&*f.owner, Some(&cell)).unwrap();
     assert_eq!(f.gov.files().deferred_for_test(), 1);
     let ingress = cell.clone();
     f.gov
@@ -394,12 +376,11 @@ fn decision11_undo_deferred_cleanup_rechecks_ingress_and_retains_tail() {
         !f.gov.stop_snapshot().kernel_stopped,
         "no recording worker ran"
     );
-    // A fresh request still performs the inherited deferred drain, even
-    // though there is no published file eligible for undo.
+    // No new request can drain after emergency; the unattempted tail stays.
     assert!(matches!(
         f.gov.files_undo_with_halt(&*f.owner, Some(&cell)),
-        Err(AuthorityError::Unavailable(_))
+        Err(AuthorityError::Halted)
     ));
-    assert!(!staging.exists());
-    assert_eq!(f.gov.files().deferred_for_test(), 0);
+    assert!(staging.exists());
+    assert_eq!(f.gov.files().deferred_for_test(), 1);
 }
```

## Independent model correction, amendment B A6

Old `Model::quit`: after `SealedFinalizing`, `ClosedClean` or `ClosedUnclean`,
it could install `ClosingNormal` when no close identity had been populated.
New assertion: Quit preserves all three final lifecycle states and their
restriction facts. The explicit nine-state literal table in
`session_model_lifecycle.rs` comes from A6; it imports no production state table.
This correction was specified before that new reference-machine test ran.
It changes no product expectation.
## A9: fifth coalesced event remains a recording obligation

Affected oracle: `mother/host/src/stop_tests.rs::m2dd_stop_cap_timeout_release_join_and_reaping`.

Old assertion:

```rust
assert_eq!(route.receipt().finished, 4);
```

Replacement assertions:

```rust
assert_eq!(route.receipt().finished, 5);
assert_eq!(lock.lock().unwrap().0, 5);
```

The four-slot concurrent-worker limit is preserved. Amendment B A9 requires
the fifth accepted event to remain pending and be drained after a slot is
available; coalescing cannot discharge it. The distinct real-store oracle
`session_stop_accounting` independently verifies the primary evidence chain
and requires one durable Stop record for each of one, four and five accepted
events. The body-count check above alone makes no durability claim.


## R-1 continuation checkpoint: executed evidence

The following supersedes the earlier checkpoint's outstanding-oracle inventory.
The checked-in `mother/host/tests/session/executed-baseline.json` contains 583
unique executed schedule observations on production baseline `e7a67505`, with
287 conforming and 296 violating the specification. Its two source logs are
identified by SHA-256. These are observations, not the 153,000 planned keys.
The baseline adapters still use the old host-bound routes: their migration to
the mandatory facade preserves these expected answers and native observations.

The 144 coordinator schedules reach 18 actual append/mutation/invocation
boundaries, before/after and zero through three original-cell ingresses.
Each child proves it reached the named boundary; failed fixture setup is not
reported as a behavioral violation. Every durable journal prefix is replayed.
The 112 private-store schedules include error-path removal as its own sink.
The native broker schedules use actual verified ledger events, independent
resource identities/content, and explicitly unscheduled native Stop workers.

Additional executed observations include 256 allocations plus overflow,
32 pending actions plus overflow, 64 consent entries plus overflow, exact
one/four/five Stop records, and two native recovery-required publications
(with and without interruption). The baseline has 257 native allocations at
the inventory limit, 33 allocations at the pending limit, and four durable
Stop records for five events. These are red evidence, not accepted behavior.
Nine poisoned ownership holds and ordinary/emergency last-owner destruction
remove staging on this baseline. Exact ack without a durable Mother reference
incorrectly succeeds for declined and published commitments.

The process-death suite reaches and kills/reaps 30 producer children: 11 native
preparation/apply/restore boundaries, 11 Mother settlement/terminal boundaries,
and eight host pending/close/Stop/restore/cleanup boundaries. Twenty-five of
these points establish two fresh successful native reports each (22 broker/
Mother plus three restore/cleanup). Host restart protocol assertions remain
red where required reporting/readiness is missing. Empty native summaries are
explicitly not all-clear claims. Torn Mother validation fails separately.

Model-only counts remain separate: 369,600 bounded three-actor/three-event
traces; 4,160 state/cause/publication/ack/record combinations; 7,547,904 literal
outcome products; 18 lifecycle cases; 1,280 resource-fact products. The literal
model fingerprints are frozen independently of production classifiers.

Full workspace baseline: twelve failed targets, including the new behavioral
oracles and the amended host expectations. Kernel: 1,464 passed, six existing
ignored, no failures; frozen desktop live suite: 23/23. Five-crate all-target,
all-feature warning-denied Clippy passed before the final receipt-only test.
No kernel, workflow or lockfile change. These are baseline diagnostics, not
final-HEAD gates or mission completion.

Five model plants have actual red/restored-control receipts: absence treated
as durability, unknown ack treated as duplicate, coalesced ingress treated as
covered, omitted terminal state, and blanket admission denial. Production
route/ownership/bypass plants remain a required implementation exit gate;
this oracle checkpoint does not claim they have run.

The full route and plant obligations remain those of frozen spec §§4/6 and
amendment B. Each W/R sink's successful native control and denial comparison
must follow its fixture when the call graph moves. Test-only hook relocation
must retain immediate native entry observations; reported admission counters
are never substitute primitive observations. Ownership-transfer plants are
applied to each final transfer location, then byte-exactly reverted. No plant
is part of a commit.


### Native discharge and execution unwind addendum (2026-11-05)

- `decision11_cleanup_ingress_after_handoff_preserves_actual_outcome`: old
  `cleanup().unwrap(); (settled, still_owed) == (1, 0)`. New: the entered
  discard removes the real staging directory, but the command returns
  `Halted`, the native entry is not discharged and the original session has
  unresolved resource debt. §5 explicitly amends this host-bound family;
  A8 requires a separately admitted parent synchronization and independent
  recording/authority proof. The physical post-handoff assertion is retained.
- `structure_the_effect_port_is_sealed_to_the_adapter`: old sealed production
  implementer list contains only `GovernedPort`; new list contains that legacy
  adapter and the closed `SessionPort`, plus the unchanged test-only
  `RecordingPort`. §4 requires the session-bound Mother port without exposing
  raw authority. The trait and sealing module remain private at the same
  boundary. This structural expectation was missed when the port was added;
  this process deviation is disclosed, not rewritten.
- The shared test fixture's memory-only confirmer now resides in the existing
  test-only oracle tree. Its filesystem operations remain in the adapter,
  whose test module uses the scanner's recognized `cfg(test)` declaration.
  No privacy scanner exemption or production confirmer is added.
- `session_execution_guard_unwind_retains_the_unrecorded_outcome` reads actual
  primary evidence rows and actual native resources. After a controlled
  execution unwind it expects one started row, zero finished rows, a pinned
  Executing commitment, and identical staging after all owners drop. Restoring
  the old destructor recording body is red: `finished` is 1, expected 0.
  Restored control passes. The first probe used an independent SQLite
  connection and failed on the store's exclusive lock in both variants; that
  harness failure is not counted as a detected plant. The final probe reads
  the existing primary connection's rows, not a production classifier.
- Additional executed schedules before the inventory implementation checkpoint:
  four close/final-sync races and two undo-grant handoffs (422553e1), the same
  112 private-store schedules with conserved-obligation expectations
  (dabfe107), and one real native execution unwind. These are additional
  receipts, not the planned Cartesian schedule count.

## Fix round 2 — held storage close notice (R2 local gate)

The frozen A12 text requires “UNCLEAN EXIT: this process did not complete the
full clean-close protocol” and says to “state which end record is established
Recorded, Failed or Unknown.” A6 says “All startup, input, work, Stop,
settlement, cleanup, recording, rendering and join helpers use” the original
three-second deadline. The old `UNCLEAN outstanding=` assertion predates the
F07 correction that separates producer count from actual resource debt.

`h1_shutdown_with_storage_held_is_bounded_and_unclean` now requires the entire
A12 notice, all three not-attempted/Unknown final obligations, explicit unknown
resource accounting, and exactly one outstanding producer. Its existing
bounded PTY deadline, held storage, nonzero exit and absence of a successful
close remain required. This strengthens the oracle; no production notice is
changed to recover the obsolete subtotal wording.
