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
