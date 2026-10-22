#!/usr/bin/env python3
"""Reference check for the P2-V1-R3B-I3-P-R3 durable recorder design.

Verifies, against the Git objects of the source baseline (never the working
tree's source files) and against the R2 design model:

1. the baseline commit exists and has the expected tree;
2. every `file:line` reference in the design document has an expectation
   below, every expectation is referenced, and each cited line (and, for a
   range, its last line) contains the expected text; a range ending on a bare
   closing brace must close at the indentation of its first line;
3. the golden record vectors embedded in design_checks.py are exactly the
   RECORD_VECTORS literals of phase2_custody_codec.rs at the baseline, at the
   cited lines;
4. the crash matrix of section 15.2 says exactly what the model's
   DOC_CRASH_MATRIX says (which check C22 compares with what the model
   computes);
5. the operation table of section 15.3 lists exactly the model's PROTOCOL and
   RECOVERY_PROTOCOL operations, in order, with their step labels;
6. the coverage table of section 15.2 states the operation, crash-point,
   schedule, outcome and recovery counts the model computes;
7. the check table of section 16.1 names exactly the model's checks and
   markers, and the stated negative-control count is the model's;
8. the activation operations of section 10.6 are exactly the model's
   ACTIVATION_PROTOCOL;
9. the composed dimensions of section 15.4 are the counts the model's
   composed check computes;
10. with --kernel DIR (local copies of the Linux v6.17 files the design cites,
   checked against their SHA-256), every kernel citation of section 1.4
   contains the cited text at the cited line.

Existence and text matches are not semantic conformance proof. Standard
library only; reads Git objects with `git show`; imports the model by path
(its import defines functions and constants only); writes nothing.

Usage: python3 -B reference_check.py [--repo DIR] [--doc FILE] [--model FILE]
                                     [--kernel DIR] [--self-test]

--self-test also runs the checker on eight in-memory corruptions (a reference
moved by one line, a golden digest altered, a crash-matrix cell, an operation,
a coverage count, a check marker, an activation operation and a composed count
changed in the document); each must be reported.
"""

import argparse
import ast
import importlib.util
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
PROCEDURES = ["P-PROV", "P-DISP", "P-REVOKE", "P-ARCH", "P-RECYCLE", "P-RETIRE", "P-REQUALIFY",
              "P-SUCCESSOR"]

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
    'core.rs:367-371': ('fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<\'_, T> {', '}'),
    'core.rs:473-482': ('fn commit(&self, at: Tick) -> Option<Closure> {', '    }'),
    'core.rs:554-556': ('pub fn cancel(&self, reason: CancelReason, now: Tick) -> CancelReceipt {', '    }'),
    'core.rs:3012-3031': ("/// The recorder's report that the next record could not be made durable:", '    }'),
    'core.rs:3072-3077': ('fn publish(&mut self) {', '    }'),
    'model.rs:310-311': ('/// The recorder adapter panicked while a record was submitted.', 'RecorderFault,'),
    'model.rs:377-384': ('/// Why execution admission closed: a control-side cancellation, or the', '}'),
}


# The Linux v6.17 files section 1.4 cites, their SHA-256, and the text each
# cited line must contain.
KERNEL_FILES = {
    "fs/ext4/fsync.c": "86fd5dc8e49491377f4f09e1f99b39dce0842083269ac28de44a71a08d12adaf",
    "fs/jbd2/journal.c": "dd5ee45c815c753a0eac6014371b3788cd7fca54f95a219e08446703b5226857",
    "fs/jbd2/commit.c": "736c3ef6d9795d5677b00b79b7b3092a054b4e77893456d47705b51d361a11da",
    "fs/ext4/ext4_jbd2.c": "bec1989a8e255e6b62c87f78a12c18e0a07854c71a8b391a34c1233b1ac66d2e",
    "fs/ext4/ext4.h": "ff09ac7308e85079b8f17d91834c2c83bff0d3c6b890c4401528c4b672fbfe4b",
    "fs/ext4/super.c": "3aa53b5ef5983ced5161a590c358bec779257dfce46e96459f41314e77e21a9b",
    "fs/utimes.c": "bf4ddd2748076206d32d55eb8981c2a39c8eeec9f0237510409f2d4c42864ff1",
    "fs/ext4/inode.c": "97a8325d4aa77171e57250cf3547e53b42c8d9f609a2318f0c89e70a3f43561a",
    "fs/fs-writeback.c": "9ed872cd85e6c54792e79241d31ad6781b88addaa05ab26626ad46f70da2ec28",
    "fs/ext4/fast_commit.c": "a09183712b8b8ab83123a68e4b3b6b91802b3daf33b66dab277527b1893ac892",
    "fs/sync.c": "4154c9dee43416c05a91a2845ac55e335f0ee65da7df33232bd2e515077af654",
    "Documentation/filesystems/ext4/journal.rst":
        "ae23e8ec62ae06fac5a14226a1bdaa4d1b7276aaeefd1c25a1ce1d98739bbbf2",
}
KERNEL_EXPECT = [
    ("fs/ext4/fsync.c", 97, "static int ext4_fsync_journal(struct inode *inode, bool datasync,"),
    ("fs/ext4/fsync.c", 109, "return ext4_force_commit(inode->i_sb);"),
    ("fs/ext4/fsync.c", 129, "int ext4_sync_file(struct file *file, loff_t start, loff_t end, int datasync)"),
    ("fs/ext4/fsync.c", 135, "ret = ext4_emergency_state(inode->i_sb);"),
    ("fs/ext4/fsync.c", 143, "if (sb_rdonly(inode->i_sb))"),
    ("fs/jbd2/journal.c", 499, "static int __jbd2_journal_force_commit(journal_t *journal)"),
    ("fs/jbd2/journal.c", 514, "/* Nothing to commit */"),
    ("fs/jbd2/journal.c", 652, "int jbd2_log_wait_commit(journal_t *journal, tid_t tid)"),
    ("fs/jbd2/journal.c", 690, "err = -EIO;"),
    ("fs/jbd2/journal.c", 2515, "This operation cannot be"),
    ("fs/jbd2/journal.c", 2516, "undone without closing and reopening the journal."),
    ("fs/jbd2/journal.c", 2549, "void jbd2_journal_abort(journal_t *journal, int errno)"),
    ("fs/jbd2/commit.c", 548, "jbd2_journal_abort(journal, err);"),
    ("fs/jbd2/commit.c", 889, "jbd2_journal_abort(journal, err);"),
    ("fs/ext4/ext4_jbd2.c", 65, "static int ext4_journal_check_start(struct super_block *sb)"),
    ("fs/ext4/ext4_jbd2.c", 82, "aborted behind our"),
    ("fs/ext4/ext4_jbd2.c", 87, "ext4_abort(sb, -journal->j_errno, \"Detected aborted journal\");"),
    ("fs/ext4/ext4.h", 2267, "static inline int ext4_emergency_state(struct super_block *sb)"),
    ("fs/ext4/ext4.h", 3197, "#define ext4_abort(sb, err, fmt, a...)"),
    ("fs/ext4/super.c", 727, "We don't set SB_RDONLY"),
    ("fs/ext4/super.c", 732, "set_bit(EXT4_FLAGS_EMERGENCY_RO, &EXT4_SB(sb)->s_ext4_flags);"),
    ("fs/utimes.c", 20, "int vfs_utimes(const struct path *path, struct timespec64 *times)"),
    ("fs/utimes.c", 66, "error = notify_change("),
    ("fs/ext4/inode.c", 5845, "int ext4_setattr(struct mnt_idmap *idmap, struct dentry *dentry,"),
    ("fs/ext4/inode.c", 6531, "void ext4_dirty_inode(struct inode *inode, int flags)"),
    ("fs/fs-writeback.c", 2495, "void __mark_inode_dirty(struct inode *inode, int flags)"),
    ("fs/fs-writeback.c", 2526, "if (sb->s_op->dirty_inode)"),
    ("fs/ext4/fast_commit.c", 1197, "int ext4_fc_commit(journal_t *journal, tid_t commit_tid)"),
    ("fs/ext4/fast_commit.c", 1208, "return jbd2_complete_transaction(journal, commit_tid);"),
    ("fs/sync.c", 205, "static int do_fsync(unsigned int fd, int datasync)"),
    ("Documentation/filesystems/ext4/journal.rst", 31, "Ext4 also supports fast commits"),
]


def git(repo, *args):
    return subprocess.run(["git", "-C", str(repo), *args], capture_output=True, text=True,
                          check=True).stdout


def load_model(path):
    sys.dont_write_bytecode = True
    spec = importlib.util.spec_from_file_location("design_checks_r2", path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    module.MUT.active = frozenset()
    return module


def section(doc, heading, following):
    start = doc.index(heading)
    return doc[start:doc.index(following, start)]


def labels(cell):
    cell = cell.strip()
    return () if cell == "—" else tuple(part.strip() for part in cell.split(","))


def computed_coverage(model):
    """Operation, crash-point, schedule, outcome and recovery counts, computed
    by the model exactly as its crash series enumerates them."""
    scenarios = model.admin_scenarios()
    counts = {}
    for name in PROCEDURES:
        world, factory, _, use_session = scenarios[name]
        probe = model._clone_world(world)
        ops = len(model._steps_for(probe, factory, use_session))
        ordered = perdir = 0
        for k in range(ops + 1):
            base = model._clone_world(world)
            steps = model._steps_for(base, factory, use_session)
            for step in steps[:k]:
                step()
            base.fs.kill(base.admin)
            ordered += len(base.fs.schedules("ordered"))
            perdir += len(base.fs.schedules("perdir"))
        counts[name] = [ops, ops + 1, ordered, perdir, (ops + 1) + 2 * (ordered + perdir), None]
    summary = model.check_c12_admin_crash()
    for name in PROCEDURES:
        m = re.search(re.escape(name) + r" (\d+)(?:,|$)", summary)
        if m is None or int(m.group(1)) != counts[name][4]:
            counts[name][4] = ("C12 reported", m.group(1) if m else None)
        r = re.search(re.escape(name) + r" recovered (\d+)", summary)
        counts[name][5] = int(r.group(1)) if r else None
    return counts


def computed_composed(model):
    """The composed dimensions, as the model's composed check computes them."""
    model.check_c26_composed_recovery()
    stats = dict(model.COMPOSED_STATS)
    return {
        "Administrative prefixes: every operation of every procedure": stats["prefixes"],
        "First crashes: F1, or F2 under every distinct schedule of both families with data old and new":
            stats["first crashes"],
        "Runs in which an owner then activated, claimed and acknowledged an `ActionStarted`":
            stats["runs with dependent work"],
        "Second crashes after that work: F1, or F2 under every distinct schedule with data old and new":
            stats["second crashes"],
        "Chained administration: an archival at every point, then a recycling and a retirement, "
        "then F1 or F2": stats["chained administration: crashes"],
        "Positive path: a generation dispositioned in a session, then F1 or F2":
            stats["dispositioned generation: Ready"],
    }


def check_kernel(kernel_dir):
    """Every kernel citation against local copies of the cited v6.17 files."""
    failures = []
    for path, digest in KERNEL_FILES.items():
        local = pathlib.Path(kernel_dir) / path
        if not local.is_file():
            local = pathlib.Path(kernel_dir) / path.replace("/", "_")
        if not local.is_file():
            failures.append(f"kernel file {path} not found under {kernel_dir}")
            continue
        import hashlib
        if hashlib.sha256(local.read_bytes()).hexdigest() != digest:
            failures.append(f"kernel file {path}: SHA-256 differs from the cited v6.17 file")
            continue
        lines = local.read_text(encoding="utf-8").splitlines()
        for cpath, line, text in KERNEL_EXPECT:
            if cpath == path and (line > len(lines) or text not in lines[line - 1]):
                failures.append(f"kernel citation {path}:{line}: expected {text!r}")
    return failures


def check(repo, doc, model_text, model, coverage, composed=None):
    """Return (lines, failures) for one document and model."""
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
    for node in ast.parse(model_text).body:
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

    # 4. The crash matrix of section 15.2.
    matrix_text = section(doc, "### 15.2 ", "### 15.3 ")
    row = re.compile(r"^\| (P-[A-Z]+) \| (\d+)(?:–(\d+))? \| ([^|]+) \| ([^|]+) \| ([^|]+) \|$")
    documented = {}
    for line in matrix_text.splitlines():
        m = row.match(line)
        if m:
            a = int(m.group(2))
            b = int(m.group(3)) if m.group(3) else a
            documented.setdefault(m.group(1), []).append(
                (a, b, labels(m.group(4)), labels(m.group(5)), labels(m.group(6))))
    transcribed = {name: [tuple(r) for r in rows] for name, rows in model.DOC_CRASH_MATRIX.items()}
    for name in sorted(set(documented) | set(transcribed)):
        if documented.get(name) != transcribed.get(name):
            failures.append(f"crash matrix {name}: document {documented.get(name)}, "
                            f"model {transcribed.get(name)}")
    out.append(f"crash matrix rows: document {sum(map(len, documented.values()))}, "
               f"model {sum(map(len, transcribed.values()))}")

    # 5. The operation table of section 15.3.
    ops_text = section(doc, "### 15.3 ", "## 16. ")
    oprow = re.compile(r"^\| ([PR]-[A-Z]+) \| (\d+) \| (.+) \|$")
    item = re.compile(r"(\d+) (\S+) `([^`]+)` \(([^)]+)\)")
    expected = dict(model.PROTOCOL)
    expected.update(model.RECOVERY_PROTOCOL)
    listed = {}
    for line in ops_text.splitlines():
        m = oprow.match(line)
        if not m:
            continue
        entries = []
        for k, part in enumerate(m.group(3).split("; "), 1):
            p = item.fullmatch(part)
            if p is None or int(p.group(1)) != k:
                entries.append(("unparsed", part))
                continue
            entries.append((p.group(4), p.group(2), p.group(3)))
        if int(m.group(2)) != len(entries):
            failures.append(f"operations {m.group(1)}: stated count {m.group(2)}, listed {len(entries)}")
        listed[m.group(1)] = entries
    for name in sorted(set(listed) | set(expected)):
        if listed.get(name) != [tuple(e) for e in expected.get(name, [])]:
            failures.append(f"operations {name}: document {listed.get(name)}, model {expected.get(name)}")
    out.append(f"operation rows: document {len(listed)}, model {len(expected)}")

    # 6. The coverage table of section 15.2.
    cov = re.compile(r"^\| (P-[A-Z]+) \| (\d+) \| (\d+) \| (\d+) \| (\d+) \| (\d+) \| (\d+|—) \|$")
    stated = {}
    for line in matrix_text.splitlines():
        m = cov.match(line)
        if m:
            stated[m.group(1)] = [int(m.group(i)) for i in range(2, 7)] + \
                [None if m.group(7) == "—" else int(m.group(7))]
    for name in PROCEDURES:
        if stated.get(name) != coverage[name]:
            failures.append(f"coverage {name}: document {stated.get(name)}, model {coverage[name]}")
    out.append(f"coverage rows: document {len(stated)}, model {len(coverage)}; "
               f"outcomes {sum(c[4] for c in coverage.values() if isinstance(c[4], int))}")

    # 7. The check table of section 16.1 and the control count.
    checks_text = section(doc, "### 16.1 ", "### 16.2 ")
    named = re.findall(r"^\| (C\d\d) \| `(\[[^\]]+\])` \|", checks_text, re.M)
    model_checks = [(c[0], c[1]) for c in model.CHECKS]
    if named != model_checks:
        failures.append(f"check table: document {named}, model {model_checks}")
    m = re.search(r"There are (\d+): R1's 27", checks_text)
    if m is None or int(m.group(1)) != len(model.NEGATIVE_CONTROLS):
        failures.append(f"negative controls: document {m.group(1) if m else None}, "
                        f"model {len(model.NEGATIVE_CONTROLS)}")
    out.append(f"checks: document {len(named)}, model {len(model_checks)}; "
               f"negative controls: model {len(model.NEGATIVE_CONTROLS)}")

    # 8. The activation operations of section 10.6.
    act_text = section(doc, "### 10.6 ", "## 11. ")
    act = re.findall(r"^\| (10\.6/A\d) \| (\w+) \| `([^`]+)` \|$", act_text, re.M)
    if act != [tuple(op) for op in model.ACTIVATION_PROTOCOL]:
        failures.append(f"activation operations: document {act}, model {model.ACTIVATION_PROTOCOL}")
    out.append(f"activation operations: document {len(act)}, model {len(model.ACTIVATION_PROTOCOL)}")

    # 9. The composed dimensions of section 15.4.
    if composed is not None:
        comp_text = section(doc, "### 15.4 ", "## 16. ")
        rows = dict((k.strip(), int(v)) for k, v in
                    re.findall(r"^\| ([^|]+?) \| (\d+) \|$", comp_text, re.M))
        for label, value in composed.items():
            if rows.get(label) != value:
                failures.append(f"composed dimension {label!r}: document {rows.get(label)}, model {value}")
        out.append(f"composed dimensions: document {len(rows)}, model {len(composed)}")
    return out, failures


def main(argv=None):
    here = pathlib.Path(__file__).resolve().parent
    parser = argparse.ArgumentParser(description="Reference check for the durable recorder design")
    parser.add_argument("--repo", default=None, help="a Git repository holding the baseline")
    parser.add_argument("--doc", default=str(here.parent.parent / "architecture" /
                                             "p2-custody-durable-recorder-design.md"))
    parser.add_argument("--model", default=str(here / "design_checks.py"))
    parser.add_argument("--kernel", default=None,
                        help="a directory holding the cited Linux v6.17 files")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args(argv)
    repo = args.repo or git(here, "rev-parse", "--show-toplevel").strip()
    doc = pathlib.Path(args.doc).read_text(encoding="utf-8")
    model_text = pathlib.Path(args.model).read_text(encoding="utf-8")
    model = load_model(args.model)
    coverage = computed_coverage(model)
    composed = computed_composed(model)
    out, failures = check(repo, doc, model_text, model, coverage, composed)
    if args.kernel:
        kfail = check_kernel(args.kernel)
        failures.extend(kfail)
        out.append(f"kernel citations: {len(KERNEL_EXPECT)} lines in {len(KERNEL_FILES)} files, "
                   f"{'all match' if not kfail else str(len(kfail)) + ' failures'}")
    else:
        out.append("kernel citations: not checked (no --kernel)")
    out.extend(f"FAIL {f}" for f in failures)
    if args.self_test:
        digest = "1df88a9cfe3b6dd027815af0cfb6a92c412c6de2d94cb8e84f80d3cf0ee516e3"
        corruptions = [
            ("moved reference", doc.replace("`core.rs:2846-2866`", "`core.rs:2847-2866`", 1),
             model_text, "core.rs:2847-2866"),
            ("altered golden digest", doc, model_text.replace(digest, digest[:-1] + "4", 1),
             "line 123 does not match"),
            ("crash-matrix cell", doc.replace("| P-REVOKE | 1 | blocks | blocks, published | invalid |",
                                              "| P-REVOKE | 1 | blocks | blocks, published | — |", 1),
             model_text, "crash matrix P-REVOKE"),
            ("operation", doc.replace("4 link `dispositions/final` (13.4/4)",
                                      "4 rename `dispositions/final` (13.4/4)", 1),
             model_text, "operations P-DISP"),
            ("coverage count", doc.replace("| P-REVOKE | 3 | 4 | 5 | 8 | 30 | — |",
                                           "| P-REVOKE | 3 | 4 | 5 | 8 | 31 | — |", 1),
             model_text, "coverage P-REVOKE"),
            ("check marker", doc.replace("| C19 | `[admission-fence]` |", "| C19 | `[admission-gate]` |", 1),
             model_text, "check table"),
            ("activation operation", doc.replace("| 10.6/A3 | futimens | `LOCK` |",
                                                 "| 10.6/A3 | fsync | `LOCK` |", 1),
             model_text, "activation operations"),
            ("composed count", doc.replace("| Administrative prefixes: every operation of every procedure | ",
                                           "| Administrative prefixes: every operation of every procedure | 9", 1),
             model_text, "composed dimension"),
        ]
        results = []
        for label, d, mt, needle in corruptions:
            if d == doc and mt == model_text:
                results.append((label, False))
                continue
            _, f = check(repo, d, mt, model, coverage, composed)
            results.append((label, any(needle in x for x in f)))
        out.append("self-test: " + "; ".join(f"{label} reported {ok}" for label, ok in results))
        if not all(ok for _, ok in results):
            failures.append("self-test: a corruption was not reported")
            out.append("FAIL self-test")
    out.append("RESULT: " + ("PASS" if not failures else "FAIL"))
    print("\n".join(out))
    return 0 if not failures else 1


if __name__ == "__main__":
    sys.exit(main())
