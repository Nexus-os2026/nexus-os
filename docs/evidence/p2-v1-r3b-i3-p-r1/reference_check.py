#!/usr/bin/env python3
"""Reference check for the P2-V1-R3B-I3-P-R1 durable recorder design.

Verifies, against the Git objects of the source baseline (never the working
tree's source files):

1. the baseline commit exists and has the expected tree;
2. every `file:line` reference in the design document has an expectation
   below, every expectation is referenced, and each cited line (and, for a
   range, its last line) contains the expected text; a range ending on a bare
   closing brace must close at the indentation of its first line;
3. the golden record vectors embedded in design_checks.py are exactly the
   RECORD_VECTORS literals of phase2_custody_codec.rs at the baseline, at the
   cited lines.

Existence and text matches are not semantic conformance proof. Standard
library only; reads Git objects with `git show`; writes nothing.

Usage: python3 reference_check.py [--repo DIR] [--doc FILE] [--model FILE] [--self-test]

--self-test also runs the checker on two in-memory corruptions (a reference
moved by one line, and one golden digest altered); each must be reported.
"""

import argparse
import ast
import pathlib
import re
import subprocess
import sys

BASE = "45898e05178a56efaadeb1f8f9ee7a0a521c72c9"
BASE_TREE = "cd197e047cb5a57e458b2af5d32f9c5406371301"
TESTS = "crates/nexus-verifier-sandbox/tests/"
FILES = {
    "core.rs": TESTS + "support/custody/core.rs",
    "model.rs": TESTS + "support/custody/model.rs",
    "codec.rs": TESTS + "support/custody/codec.rs",
    "mod.rs": TESTS + "support/custody/mod.rs",
    "core-tests": TESTS + "phase2_custody_core.rs",
    "codec-tests": TESTS + "phase2_custody_codec.rs",
}
REF = re.compile(r"(?:core|model|codec|mod)\.rs:\d+(?:-\d+)?|(?:core|codec)-tests:\d+(?:-\d+)?")

# Expected text on the first line (and the last line of a range) of every reference.
EXPECT = {
    'mod.rs:1-21': ("//! The live harness's custody core", '//! while anything native or any required evidence is unresolved or failed.'),
    'mod.rs:139-143': ('//! - Destruction: this module does not preserve owners', '//!   refuses closure for good.'),
    'mod.rs:144-151': ('//! - Durability: records are made durable (write-ahead) by the recorder', '//!   native cleanup happened. Neither exists here.'),
    'mod.rs:164-167': ('//! - Returned owners: an owner refused by [`Custody::complete`] (anot', '//!   failure); the integration must keep and dispose of it.'),
    'mod.rs:168-171': ('//! - Unresolvable states: lost authority, an exhausted recovery budget, an', '//!   administrative bypass or forced destruction for them.'),
    'model.rs:395-449': ('pub enum RecordKind {', '}'),
    'model.rs:451-455': ('/// One record the recorder must make durable', '/// covered ([`super::codec::encode_record`] writes the frame).'),
    'model.rs:457': ('pub struct RecordIntent {', None),
    'model.rs:466-479': ('pub struct RecordAck {', '}'),
    'model.rs:683-702': ('pub struct IncidentBinding([u8; 32]);', '}'),
    'model.rs:705-711': ('pub enum DispositionReason {', '}'),
    'model.rs:713-718': ("/// An external validator's verdict that a prior incident was dispositioned.", '/// cleanup.'),
    'model.rs:763-775': ('pub const LIVE: Config = Config {', '};'),
    'model.rs:837-845': ('pub struct EvidenceView {', '}'),
    'model.rs:860-865': ('#[derive(Debug, Clone, PartialEq, Eq)]', '}'),
    'core.rs:89-95': ('/// Where issued evidence records go to be made durable. Submitting a ', '}'),
    'core.rs:117-126': ('/// The permit for one admitted native operation, and the binding its ', '}'),
    'core.rs:246-251': ('/// A late owner custody cannot hold: as many late incidents as it may', 'CustodyFull { owner: R },'),
    'core.rs:431-442': ('struct Shared {', '}'),
    'core.rs:448-465': ('fn close(&self, reason: ClosureReason, at: Tick) -> CancelReceipt {', '}'),
    'core.rs:522-548': ('pub fn status(&self) -> StatusView {', '    }'),
    'core.rs:616-623': ('pub fn submit(&self, request: Request) -> Result<(), Busy> {', '    }'),
    'core.rs:741-747': ('fn settlement(&self) -> Settlement {', '}'),
    'core.rs:967-968': ('general_capacity: config.record_capacity - config.control_reserve,', 'control_capacity: config.control_reserve,'),
    'core.rs:980-989': ('fn reserve(&mut self, count: u32) -> bool {', '    }'),
    'core.rs:1004-1011': ('fn issue_control(&mut self, at: Tick, kind: RecordKind) -> Option<RecordId> {', '    }'),
    'core.rs:1042-1069': ('fn acknowledge(&mut self, ack: &RecordAck) -> AckOutcome {', '    }'),
    'core.rs:1042-1090': ('fn acknowledge(&mut self, ack: &RecordAck) -> AckOutcome {', '}'),
    'core.rs:1072-1090': ('fn fail(&mut self, id: RecordId, now: Tick) -> AckOutcome {', '    }'),
    'core.rs:1105-1137': ('/// Failures: kept in detail up to a bound', '}'),
    'core.rs:1207-1209': ('fn blocks(&self) -> bool {', '    }'),
    'core.rs:1289-1297': ('/// A new custody for one generation, and its control handle. Prior', ') -> Result<(Custody<R>, Control), Refusal> {'),
    'core.rs:1298-1304': ('if prior.len() > config.incident_limit', '        }'),
    'core.rs:1410-1447': ('pub fn apply_disposition<V: DispositionValidator>(', '    }'),
    'core.rs:1459-1488': ('fn try_start(&mut self, now: Tick) -> Result<LeaseCap, Refusal> {', 'self.phase = RunPhase::Running;'),
    'core.rs:1463-1465': ('if self.prior.iter().any(Prior::blocks) {', '}'),
    'core.rs:1469-1471': ('if let Some(closure) = self.observe_closure(now) {', '}'),
    'core.rs:1476-1482': ('let dispositioned = self', '.issue_reserved(now, RecordKind::RunStarted { dispositioned });'),
    'core.rs:1519-1566': ('fn try_begin(', '    }'),
    'core.rs:1525-1529': ('// A halted run never begins another case, however clean its last one', '}'),
    'core.rs:1533-1535': ('if self.case.is_some() {', '}'),
    'core.rs:1542-1544': ('if self.ledger.unacknowledged() > 0 {', '}'),
    'core.rs:1579-1620': ('fn try_reserve(&mut self, kind: SlotKind, now: Tick) -> Result<Reservation, Refusal> {', '    }'),
    'core.rs:1580-1582': ('if let Some(closure) = self.observe_closure(now) {', '}'),
    'core.rs:1602-1606': ('let action = ActionId {', 'self.next_action += 1;'),
    'core.rs:1626-1697': ('pub fn admit(&mut self, reservation: Reservation, now: Tick) -> Result', '}'),
    'core.rs:1658-1664': ('let start_acked = self.ledger.is_acknowledged(start);', '}'),
    'core.rs:1658-1675': ('let start_acked = self.ledger.is_acknowledged(start);', '};'),
    'core.rs:1665-1675': ('// The admission linearization point.', '};'),
    'core.rs:1700-1704': ('fn settle_unadmitted(&mut self, now: Tick) {', '}'),
    'core.rs:1716-1726': ('if ticket.action.generation != self.generation {', 'self.clock = now;'),
    'core.rs:1744-1752': ('if !open {', '}'),
    'core.rs:1783-1808': ('fn resolve_without_owner(&mut self, how: Settlement, now: Tick) -> Completion {', '    }'),
    'core.rs:1813-1849': ('fn resolve_ended(&mut self, facts: EndFacts, now: Tick) -> Completion ', '}'),
    'core.rs:1855-1922': ('fn deposit(', '}'),
    'core.rs:1924-1930': ('/// An owner delivered for an operation already retired', "/// leaves the run's outcome as recorded."),
    'core.rs:1931-1987': ('fn retain_late(', '    }'),
    'core.rs:1952-1956': ('let incident = IncidentId {', 'self.next_incident += 1;'),
    'core.rs:1979-1982': ('if self.phase == RunPhase::Candidate {', '}'),
    'core.rs:1989-2016': ("/// Issue an action's failure record from its reservation, once.", '    }'),
    'core.rs:2157-2212': ('fn try_end_case<C: Cleanup<R>>(', '    }'),
    'core.rs:2216-2233': ('fn note_unknown(&mut self, now: Tick) {', '}'),
    'core.rs:2238-2255': ('fn note_unresolved(&mut self, id: EntryId, now: Tick) {', '}'),
    'core.rs:2259-2278': ('fn note_output_lost(&mut self, case: Option<CaseId>, now: Tick) {', '}'),
    'core.rs:2346-2353': ('pub fn observe_control(&mut self, now: Tick) -> Result<Option<Closure>', '}'),
    'core.rs:2374-2387': ('fn record_closure(&mut self, closure: Closure, now: Tick) {', '    }'),
    'core.rs:2395-2415': ('fn note_cancellation(&mut self, closure: Option<Closure>, now: Tick) {', '}'),
    'core.rs:2487-2565': ('fn attempt<C: Cleanup<R>>(', '}'),
    'core.rs:2570-2607': ('fn finish(&mut self, slot: SlotRef, now: Tick) {', '    }'),
    'core.rs:2617-2619': ('fn unresolved(&self) -> bool {', '    }'),
    'core.rs:2624-2644': ('fn stop(&mut self, now: Tick) {', '    }'),
    'core.rs:2637-2644': ('fn enter_recovery(&mut self, now: Tick) {', '}'),
    'core.rs:2648-2652': ('fn become_candidate(&mut self, now: Tick, resolved_by: Option<u32>) {', '}'),
    'core.rs:2662-2687': ('fn try_finalize(&mut self, now: Tick) {', '}'),
    'core.rs:2663-2669': ('if self.phase != RunPhase::Candidate', '}'),
    'core.rs:2678-2686': ('let record = self.ledger.issue_reserved(', 'self.phase = RunPhase::Finalizing;'),
    'core.rs:2690-2697': ('fn note_finalized(&mut self) {', '    }'),
    'core.rs:2700-2706': ('fn run_verdict(&self) -> Verdict {', '}'),
    'core.rs:2710-2718': ('fn verdict(&self) -> Verdict {', '    }'),
    'core.rs:2722-2737': ('pub fn serve<C: Cleanup<R>>(', '}'),
    'core.rs:2722-2785': ('pub fn serve<C: Cleanup<R>>(', '    }'),
    'core.rs:2745-2747': ('if request.generation != self.generation {', '}'),
    'core.rs:2801-2837': ('fn retry<C: Cleanup<R>>(&mut self, epoch: u64, cleanup: &mut C, now: Tick) -> Response {', '    }'),
    'core.rs:2846-2866': ('fn shutdown_requested(&mut self, now: Tick) -> ShutdownDecision {', '    }'),
    'core.rs:2856-2864': ('if matches!(decision, ShutdownDecision::Refused(_)) && !self.shutdown_refusal_recorded {', '}'),
    'core.rs:2872-2924': ('pub fn shutdown_decision(&self) -> ShutdownDecision {', '    }'),
    'core.rs:2881-2924': ('fn unresolved_now(&self) -> Vec<Unresolved> {', '}'),
    'core.rs:2914-2916': ('match self.ledger.failed {', 'None => {'),
    'core.rs:2929-2954': ('pub fn close(mut self, now: Tick) -> Result<Closed<R>, Box<Custody<R>>> {', '    }'),
    'core.rs:2958-2991': ('pub fn flush_records<S: RecordSink>(', '    }'),
    'core.rs:2974-2985': ('let submitted = contained(|| sink.submit(&intent));', '}'),
    'core.rs:2996-3010': ('pub fn acknowledge(&mut self, ack: &RecordAck, now: Tick) -> AckOutcome {', '    }'),
    'core.rs:3012-3015': ("/// The recorder's report that the next record could not be made durable:", '/// the custody never closes.'),
    'core.rs:3016-3031': ('pub fn record_failed(&mut self, id: RecordId, now: Tick) -> AckOutcome {', '    }'),
    'core.rs:3033-3039': ('fn advance(&mut self, now: Tick) -> Result<(), Refusal> {', '}'),
    'core.rs:3046-3069': ('fn fail(&mut self, class: FailureClass, detail: &str, now: Tick) {', '}'),
    'codec.rs:22-31': ('//! | Offset | Size | Field |', '//! A frame is exactly'),
    'codec.rs:33-38': ('//! Fields: `u32` and `u64` big-endian', '//! value not listed are invalid.'),
    'codec.rs:40-73': ('//! Record payload: record generation', '//! | failure class |'),
    'codec.rs:75': ('//! Record payloads are 33 to 83 bytes', None),
    'codec.rs:75-79': ('//! Record payloads are 33 to 83 bytes', '//! codec checks syntax, never lifecycle.'),
    'codec.rs:89-93': ('//! - [`decode_record`] and [`decode_request`] accept exactly one complete,', '//!   admit, acknowledge, disposition, clean up or reconstruct anything.'),
    'codec.rs:110': ('pub const MAX_FRAME: usize = 4096;', None),
    'codec.rs:115': ('const LARGEST_PAYLOAD: usize = 83;', None),
    'codec.rs:230-236': ('pub fn encode_record(', '}'),
    'codec.rs:238-324': ('/// Exactly one complete, consistent record frame', '}'),
    'core-tests:5237': ('fn h18_adversarial_transition_sequences_keep_every_invariant()', None),
    'codec-tests:121': ('const RECORD_VECTORS: &[RecordVector] = &[', None),
}


def git(repo, *args):
    return subprocess.run(["git", "-C", str(repo), *args], capture_output=True, text=True,
                          check=True).stdout


def check(repo, doc, model):
    """Return (lines, failures) for one document and model text."""
    failures = []
    out = []

    # 1. The baseline.
    kind = git(repo, "cat-file", "-t", BASE).strip()
    tree = git(repo, "rev-parse", BASE + "^{tree}").strip()
    if kind != "commit" or tree != BASE_TREE:
        failures.append(f"baseline {BASE}: type {kind}, tree {tree}")
    out.append(f"baseline {BASE} tree {tree}: {'OK' if not failures else 'MISMATCH'}")

    # 2. References.
    if BASE not in doc:
        failures.append("the document does not name the source baseline")
    found = set(REF.findall(doc))
    missing = sorted(found - set(EXPECT))
    unused = sorted(set(EXPECT) - found)
    if missing:
        failures.append(f"references without an expectation: {missing}")
    if unused:
        failures.append(f"expectations not referenced: {unused}")
    sources = {}
    checked = 0
    for ref, (first, last) in sorted(EXPECT.items()):
        key, span = ref.rsplit(":", 1)
        lo, _, hi = span.partition("-")
        lo = int(lo)
        hi = int(hi) if hi else lo
        if key not in sources:
            sources[key] = git(repo, "show", f"{BASE}:{FILES[key]}").splitlines()
        text = sources[key]
        if not 1 <= lo <= hi <= len(text):
            failures.append(f"{ref}: out of range ({len(text)} lines)")
            continue
        good = first in text[lo - 1] and (last is None or last in text[hi - 1])
        if good and last is not None and text[hi - 1].strip() in ("}", "};"):
            indent = len(text[lo - 1]) - len(text[lo - 1].lstrip(" "))
            closing = len(text[hi - 1]) - len(text[hi - 1].lstrip(" "))
            good = indent == closing
        checked += 1
        if not good:
            failures.append(f"{ref}: line {lo} {text[lo - 1].strip()[:90]!r}; "
                            f"line {hi} {text[hi - 1].strip()[:90]!r}")
    out.append(f"references in the document: {len(found)}; checked against {BASE[:12]}: {checked}")

    # 3. Golden vectors.
    vectors = None
    for node in ast.parse(model).body:
        if isinstance(node, ast.Assign) and any(getattr(t, "id", None) == "GOLDEN_RECORD_VECTORS"
                                                for t in node.targets):
            vectors = ast.literal_eval(node.value)
    if vectors is None:
        failures.append("GOLDEN_RECORD_VECTORS not found in the model")
        vectors = ()
    codec = git(repo, "show", f"{BASE}:{FILES['codec-tests']}").splitlines()
    start = next(i for i, line in enumerate(codec) if line.startswith("const RECORD_VECTORS"))
    end = next(i for i in range(start, len(codec)) if codec[i] == "];")
    pairs = {}
    for i in range(start, end):
        m = re.fullmatch(r'\s*frame: "([^"]+)",', codec[i])
        if m:
            d = re.fullmatch(r'\s*digest: "([0-9a-f]{64})",', codec[i + 1])
            pairs[i + 1] = (m.group(1), d.group(1) if d else None)
    for line, frame, digest, *_ in vectors:
        if pairs.get(line) != (frame, digest):
            failures.append(f"golden vector at codec-tests line {line} does not match the baseline")
    if len(vectors) != len(pairs) or {v[0] for v in vectors} != set(pairs):
        failures.append(f"golden vectors: model {len(vectors)}, baseline {len(pairs)}")
    out.append(f"golden record vectors: model {len(vectors)}, baseline RECORD_VECTORS {len(pairs)}")
    return out, failures


def main(argv=None):
    here = pathlib.Path(__file__).resolve().parent
    parser = argparse.ArgumentParser(description="Reference check for the durable recorder design")
    parser.add_argument("--repo", default=None, help="a Git repository holding the baseline")
    parser.add_argument("--doc", default=str(here.parent.parent / "architecture" /
                                             "p2-custody-durable-recorder-design.md"))
    parser.add_argument("--model", default=str(here / "design_checks.py"))
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args(argv)
    repo = args.repo or git(here, "rev-parse", "--show-toplevel").strip()
    doc = pathlib.Path(args.doc).read_text(encoding="utf-8")
    model = pathlib.Path(args.model).read_text(encoding="utf-8")
    out, failures = check(repo, doc, model)
    out.extend(f"FAIL {f}" for f in failures)
    if args.self_test:
        moved = doc.replace("`core.rs:2846-2866`", "`core.rs:2847-2866`", 1)
        _, f1 = check(repo, moved, model)
        digest = "1df88a9cfe3b6dd027815af0cfb6a92c412c6de2d94cb8e84f80d3cf0ee516e3"
        _, f2 = check(repo, doc, model.replace(digest, digest[:-1] + "4", 1))
        caught = (any("core.rs:2847-2866" in f for f in f1)
                  and any("line 123 does not match" in f for f in f2))
        out.append(f"self-test: moved reference reported {any('core.rs:2847-2866' in f for f in f1)}; "
                   f"altered golden digest reported {any('line 123 does not match' in f for f in f2)}")
        if not caught:
            failures.append("self-test: a corruption was not reported")
            out.append("FAIL self-test")
    out.append("RESULT: " + ("PASS" if not failures else "FAIL"))
    print("\n".join(out))
    return 0 if not failures else 1


if __name__ == "__main__":
    sys.exit(main())
