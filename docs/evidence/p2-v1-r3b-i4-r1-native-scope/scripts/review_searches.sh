#!/usr/bin/env bash
# P2-V1-R3B-I4-R1 whole-module review (mission section 21): one search per
# listed hazard, over the candidate's scope, execution, launcher and fault
# sources, the live harness's callers and the desktop's callers and guards,
# each with its command and its complete output (file:line). The findings
# are interpreted, and every value classified, in
# matrices/module-self-review.md. Read-only.
# Usage: review_searches.sh <checkout>
set -u
cd "$1" || exit 2
S=crates/nexus-verifier-sandbox/src
PROD="$S/scope.rs $S/scope/manager.rs $S/scope/native.rs $S/scope/pending.rs $S/execution.rs $S/launcher.rs $S/fault.rs"
HARNESS=crates/nexus-verifier-sandbox/tests/phase2_live_sandbox.rs
DESKTOP="app/src-tauri/src/coding_flow/verification.rs app/src-tauri/src/coding_flow.rs app/src-tauri/src/phase2_tests.rs"
search() {
  local title="$1"
  shift
  echo "== $title"
  echo "\$ $*"
  "$@" || true
  echo
}
echo "P2-V1-R3B-I4-R1 whole-module review searches; HEAD $(git rev-parse HEAD), tree $(git rev-parse 'HEAD^{tree}')"
echo
search "R1. public pending ownership without the helper: every declaration and re-export of the pending operation, its failure types and settling" \
  grep -n -E "struct PendingScope|use pending::|StartFailed|fn settle|fn reconcile|fn prepare|into_pending|pub struct PlacementFailed|pub struct ScopedHelper|pub fn place" $PROD
search "R2. public alternate manager socket: every constructor of a manager or a connection" \
  grep -n -E "fn connect|connect_at|connect_to|Builder::address|fn with_controller|ZbusManager::" $PROD
search "R3. public direct start weaker than execution::run: every start of a scope operation" \
  grep -n -E "fn start\(|\.establish\(|ScopeBoundary::Pending\(|fn place|fn run\(|fn run_with_fault|fn execute" $PROD
search "R4. resume_unwind while pending, and every catch_unwind with what it covers" \
  grep -n -E "resume_unwind|catch_unwind|AssertUnwindSafe|panic::" $PROD
search "R5. unit-name-only authority: every use of the unit name" \
  grep -n -E "self\.unit|&self\.unit|unit: |\.unit\(\)|names_unit|nexus-verifier-" $PROD
search "R6. basename-only binding: every comparison of a path, an id or a name" \
  grep -n -E "rsplit|ends_with|starts_with|strip_prefix|== path|== self\.unit|id ==|group ==|canonical" $S/scope.rs $S/scope/pending.rs $S/scope/native.rs
search "R7. GetUnit absence after an uncertain start: every absence, acceptance and uncertainty" \
  grep -n -E "accepted|absent\(|Presence::Absent|Started::Uncertain|Started::Accepted|fn gone" $S/scope/pending.rs $S/scope/manager.rs
search "R8. StopUnit reply as proof: where every StopUnit outcome goes" \
  grep -n -E "stop_unit|last_stop|stops" $PROD
search "R9. helper reaped before its operation is confirmed: every reap" \
  grep -n -E "reap\(|try_reap|reap_within|reap_helper|waitpid|unresolved\(\)" $PROD
search "R10. serial wrap and spawn before identity: the identity allocation and the spawn" \
  grep -n -E "NEXT_SERIAL|allocate_serial|fetch_add|fetch_update|checked_add|wrapping|IdentitiesExhausted|\.spawn\(\)|serial" $S/launcher.rs
search "R11. candidate dropped on a later proof error: every assignment and take of the candidate" \
  grep -n -E "candidate = |candidate\.take|candidate\.insert|self\.candidate|Option<Box<dyn CgroupDir>>" $S/scope/pending.rs
search "R12. launch before the ControlGroup proof: the proof's steps and the launch's guard" \
  grep -n -E "control_group\(|unit_id\(|get_unit\(|runtime_max_usec\(|oom_policy\(|is_proven|handshake\(|\.launch\(|launched = true" $S/scope/pending.rs $S/execution.rs
search "R13. test manager available in production: every test seam" \
  grep -n -E "cfg\(test\)|cfg\(any\(test|mod tests|FakeManager|FakeNative|World|with_controller|SPAWNED" $PROD
search "R14. unit-name sweeps: enumeration, patterns, lookups by process or cgroup" \
  grep -n -E "ListUnits|SubscribeUnits|GetUnitBy|read_dir|nexus-verifier-\*" $PROD
search "R15. PID-only cleanup: every signal, wait or open of a process by id" \
  grep -n -E "libc::kill|waitpid|waitid|pidfd|/proc/\{|\.pid\(\)|helper_pid" $PROD
search "R16. manager reconnect changing authority: every controller and its sharing" \
  grep -n -E "Arc<Controller>|Arc::clone\(&self\.controller\)|Arc::new\(Controller|controller:|Weak" $PROD
search "R17. the live harness's callers of the direct scope API" \
  grep -n -E "execution::place|ScopedHelper|PlacementFailed|reap_helper|placed\.|connect_at|scopes\.start|StartFailed|PendingScope|unplaced" $HARNESS
search "R18. the desktop's route to the sandbox, and its guards of the closed surface" \
  grep -n -E "ScopeManager::connect|execution::run\(|execution::place|connect_at|ScopedHelper|PlacementFailed|StartFailed|pub fn start|RetainedBoundary::retry" $DESKTOP
