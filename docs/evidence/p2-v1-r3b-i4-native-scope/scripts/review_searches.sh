#!/usr/bin/env bash
# P2-V1-R3B-I4 whole-module review (mission section 20): the specific
# searches, over the candidate's scope, execution, launcher and fault
# sources (tests included where they are callers), each with its command
# and its complete output (file:line). The findings are interpreted in
# matrices/module-self-review.md. Read-only.
# Usage: review_searches.sh <checkout>
set -u
cd "$1" || exit 2
S=crates/nexus-verifier-sandbox/src
PROD="$S/scope.rs $S/scope/manager.rs $S/scope/native.rs $S/scope/pending.rs $S/execution.rs $S/launcher.rs $S/fault.rs"
search() {
  local title="$1"
  shift
  echo "== $title"
  echo "\$ $*"
  "$@" || true
  echo
}
echo "P2-V1-R3B-I4 whole-module review searches; HEAD $(git rev-parse HEAD), tree $(git rev-parse 'HEAD^{tree}')"
echo
search "1. remote side effects followed by ? with no owner: every remote call site" \
  grep -n -E "start_scope\(|stop_unit\(|get_unit\(|runtime_max_usec\(|oom_policy\(|call_method\(" $PROD
search "1b. every ? in the scope module (each must be an observation, never a remote effect)" \
  grep -n -E "\?[;)]|\?$|\? *\{" $S/scope.rs $S/scope/pending.rs $S/scope/manager.rs $S/scope/native.rs
search "2. StopUnit results: where they go" \
  grep -n -E "stop_unit|last_stop" $PROD
search "3. timeout interpreted as absence: every uncertainty arm, and every absence" \
  grep -n -E "Uncertain|timed out|Presence::Absent|NO_SUCH_UNIT|UNIT_EXISTS|Collision" $PROD
search "4. string / PID / path authority: process ids, cgroup paths, unit names" \
  grep -n -E "\.pid\(\)|helper_pid|/proc/|/sys/fs/cgroup|names_unit|unit: |&self\.unit|\.unit\(\)" $PROD
search "5. cleanup confirmation: every place an operation or scope is confirmed" \
  grep -n -E "settled = |Cleanup::Confirmed|return true|wait_empty|fn gone|fn absent|fn outside" $PROD
search "6. native descriptors: where a candidate is retained, moved or dropped" \
  grep -n -E "candidate|Box<dyn CgroupDir>|\.open\(" $S/scope.rs $S/scope/pending.rs $S/scope/native.rs
search "7. scope ownership and panicking closures: every catch_unwind and what it borrows" \
  grep -n -E "catch_unwind|resume_unwind|AssertUnwindSafe" $PROD
search "8. drops: every Drop implementation and every reset of a scope boundary" \
  grep -n -E "impl Drop|fn drop|ScopeBoundary::None|mem::take|into_pending|end_now" $PROD
search "9. launch while pending: the handshake and launch call sites and their guard" \
  grep -n -E "handshake\(|\.launch\(|is_proven|launched = true" $S/execution.rs
search "10. retries and stack borrows: lifetimes and references held by owners" \
  grep -n -E "struct (PendingScope|RetainedBoundary|Owned|Scope|ScopeManager|Controller)|<'[a-z]+>|Arc<Controller>|Weak" $PROD
search "11. background cleanup: threads and tasks" \
  grep -n -E "thread::spawn|thread::Builder|tokio::spawn|spawn_blocking|task::spawn" $PROD
search "12. unit-name sweeps: enumeration, patterns, lookups by process or cgroup" \
  grep -n -E "ListUnits|nexus-verifier-|SubscribeUnits|GetUnitBy|read_dir" $PROD
search "13. manager reconnection: every connection construction and controller assignment" \
  grep -n -E "connect_at|connect\(\)|Builder::address|controller:|Controller \{|with_controller" $PROD
search "14. production construction of the test seams" \
  grep -n -E "cfg\(test\)|mod tests|FakeManager|FakeNative|World" $S/scope.rs $S/scope/pending.rs $S/scope/manager.rs $S/scope/native.rs $S/execution.rs
search "15. callers of the scope API outside the module" \
  grep -rn -E "ScopeManager|StartFailed|PendingScope|RetainedBoundary|\.start\(&" crates/nexus-verifier-sandbox/tests/phase2_live_sandbox.rs app/src-tauri/src/coding_flow/verification.rs app/src-tauri/src/coding_flow.rs
