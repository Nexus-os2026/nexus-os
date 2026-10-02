#!/usr/bin/env python3
"""Reference check for the P2-V1-R3B-I3-P-R5 durable recorder design.

Verifies, against the Git objects of the source baseline (never the working
tree's source files), against the R5 design model and, with --kernel, against
local copies of the cited Linux v6.17 files:

1. the baseline commit exists and has the expected tree;
2. every Rust `file:line` reference in the design document has an expectation
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
10. (R4) every Linux `file:line` citation in the document has an expectation
   in KERNEL_EXPECT and every expectation is cited; with --kernel DIR, each
   cited file is checked against its SHA-256 and each citation's first and
   last line against the expected text, with the closing-brace rule of 2;
11. (R4) the enumeration counts of section 10.7 are the counts the model's
   activation-proof check computes;
12. (R4) the required and excluded options of section 6.4 step 9, and the
   journal inode of step 10, are the model's;
13. (R4) the PROVISION keys of section 7.5 are the model's, in order;
14. (R5) the storage rule of section 6.4 step 12 names the model's storage
   class, admitted cache values, identity prefix and identity attributes;
15. (R5) the storage-event dimensions of section 15.4 are the counts the
   model's storage-composed check computes.

Existence and text matches are not semantic conformance proof; a successful
line check is not a semantic proof. Standard library only; reads Git objects
with `git show`; imports the model by path (its import defines functions and
constants only); writes nothing.

Usage: python3 -B reference_check.py [--repo DIR] [--doc FILE] [--model FILE]
                                     [--kernel DIR] [--self-test]

--self-test also runs the checker on in-memory corruptions: R3's eight (a
reference moved by one line, a golden digest altered, a crash-matrix cell, an
operation, a coverage count, a check marker, an activation operation and a
composed count changed in the document), R4's four (a kernel citation moved,
an enumeration count, an excluded option dropped, a PROVISION key renamed) and
R5's four (an R5 kernel citation moved, the admitted fua value, the identity
attributes' order, a storage-event count), and, with --kernel, an R4 and an
R5 kernel expectation altered; each must be reported.
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


# The Linux v6.17 files the design cites (short name -> path, SHA-256), and the
# text the first and last line of every citation must contain. Every Linux
# `file:line` citation in the document must have an entry, and every entry
# must be cited.
KERNEL_FILES = {
    'journal.c': ('fs/jbd2/journal.c',
        'dd5ee45c815c753a0eac6014371b3788cd7fca54f95a219e08446703b5226857'),
    'commit.c': ('fs/jbd2/commit.c',
        '736c3ef6d9795d5677b00b79b7b3092a054b4e77893456d47705b51d361a11da'),
    'checkpoint.c': ('fs/jbd2/checkpoint.c',
        'e984f455acd823c46947d59fdde6883449b7c7200ee7c6a05b8e87161ad2d388'),
    'recovery.c': ('fs/jbd2/recovery.c',
        'ccb2b0572719fc07ea851cb91c3c89440bc0fd296b3a5a94f11f69d64f72cd45'),
    'transaction.c': ('fs/jbd2/transaction.c',
        'ffbe13a83744de683daa35bea1b72d08693163b65c3efa76feb216888004205d'),
    'fsync.c': ('fs/ext4/fsync.c',
        '86fd5dc8e49491377f4f09e1f99b39dce0842083269ac28de44a71a08d12adaf'),
    'super.c': ('fs/ext4/super.c',
        '3aa53b5ef5983ced5161a590c358bec779257dfce46e96459f41314e77e21a9b'),
    'inode.c': ('fs/ext4/inode.c',
        '97a8325d4aa77171e57250cf3547e53b42c8d9f609a2318f0c89e70a3f43561a'),
    'ext4_jbd2.c': ('fs/ext4/ext4_jbd2.c',
        'bec1989a8e255e6b62c87f78a12c18e0a07854c71a8b391a34c1233b1ac66d2e'),
    'ext4_jbd2.h': ('fs/ext4/ext4_jbd2.h',
        '91482cf3a796ed01c705c3131bba9533a14769ac21df9ca766a9a0eb85e87c4c'),
    'ext4.h': ('fs/ext4/ext4.h',
        'ff09ac7308e85079b8f17d91834c2c83bff0d3c6b890c4401528c4b672fbfe4b'),
    'fast_commit.c': ('fs/ext4/fast_commit.c',
        'a09183712b8b8ab83123a68e4b3b6b91802b3daf33b66dab277527b1893ac892'),
    'sysfs.c': ('fs/ext4/sysfs.c',
        '6bf675eded83565fa48e50240284ff92cdc5ecfa98ba7a2c2cc86beaf18b1260'),
    'fs-writeback.c': ('fs/fs-writeback.c',
        '9ed872cd85e6c54792e79241d31ad6781b88addaa05ab26626ad46f70da2ec28'),
    'utimes.c': ('fs/utimes.c',
        'bf4ddd2748076206d32d55eb8981c2a39c8eeec9f0237510409f2d4c42864ff1'),
    'sync.c': ('fs/sync.c',
        '4154c9dee43416c05a91a2845ac55e335f0ee65da7df33232bd2e515077af654'),
    'buffer.c': ('fs/buffer.c',
        '1ecea2c74781410b847be3e131818bbcf591da7bde0ee78e7df152dcc754aa67'),
    'blk-flush.c': ('block/blk-flush.c',
        '6b26d0c13ab486a85afb543c6dd16506a5d03e9b9092f342df6fa4b105550714'),
    'blk-core.c': ('block/blk-core.c',
        '2809b4d1b174ddb392cf1358d9173e1b272b6151646749cf5f95d98e532055fb'),
    'jbd2.h': ('include/linux/jbd2.h',
        'b695252b8160cbb195ea0aea39e20f84e56d3f1e707e4b783fde64e0808c5408'),
    'journal.rst': ('Documentation/filesystems/ext4/journal.rst',
        'ae23e8ec62ae06fac5a14226a1bdaa4d1b7276aaeefd1c25a1ce1d98739bbbf2'),
    # R5
    'blk-sysfs.c': ('block/blk-sysfs.c',
        '97e0861b75a2c1ebba1a9fded21ff4b9386ef8e21a6c88f70cbc948b74a8763d'),
    'blk-settings.c': ('block/blk-settings.c',
        '54a1f8350b0c9be21332343533ca47ccf63bf7d362c6dce33000f1e80d23b795'),
    'blkdev.h': ('include/linux/blkdev.h',
        '984c6db19eabc6ac9f2dc99e76659f7f03887bb7732d7ca68da8e1d50d65ee55'),
    'pagemap.h': ('include/linux/pagemap.h',
        'f4cbe9d56c4249bca2aab25fc7b396bb279d96a413d848434566f4a8889444d8'),
    'nvme.h': ('include/linux/nvme.h',
        '0ad73c691e20c670fec8bf188f9ae4f92fa87c51629b3199dc31255cdac70d97'),
    'nvme/core.c': ('drivers/nvme/host/core.c',
        '205c7bd990a6d6ed9dbe8ced834d5f64cb54d813440f6d29d8ca8fffaac02aac'),
    'nvme/sysfs.c': ('drivers/nvme/host/sysfs.c',
        '762b68c9e3a75128de0c0b5c59e7e18070882118e2caf7583dfe4f787bfdbfcd'),
    'nvme/pci.c': ('drivers/nvme/host/pci.c',
        '1693dae2b398b44e75e3608769ee0a7e57b8de5d73f649089b89156418018b80'),
    'nvme/multipath.c': ('drivers/nvme/host/multipath.c',
        '81d1d6c7c4e7cbf1725d8113d1c5c9927178c49c30eeabeaa3d385c8e18c0933'),
    'base/core.c': ('drivers/base/core.c',
        'e2571ce207f4206bd716a55af14df8074dac8d49f328374caae926fc2adf75e7'),
    'errseq.c': ('lib/errseq.c',
        '24902b343264ec09a57e47b4817f6ff34b723491807b20fc7f42b0b89d794c57'),
    'filemap.c': ('mm/filemap.c',
        '3420a8870d67e5dd3156139a07ae8ce696a7332259dca8afcde205357898948b'),
    'page-io.c': ('fs/ext4/page-io.c',
        '83aa2af3f58c5200037ac54735d1b033cd4caab2cdd160cbec62b2c8c55cae04'),
    'writeback_cache_control.rst': ('Documentation/block/writeback_cache_control.rst',
        'f126397f886b4ae18c9935565f5ff942ad15ab0e51731fff7e7664b8ce8e1ce3'),
}
KERNEL_EXPECT = {
    'blk-core.c:809-820': ('if (op_is_flush(bio->bi_opf)) {', '}'),
    'blk-flush.c:468-474': ('int blkdev_issue_flush(struct block_device *bdev)', '}'),
    'buffer.c:165-176': ('void end_buffer_write_sync(struct buffer_head *bh, int uptodate)', '}'),
    'buffer.c:1214-1222': ('void mark_buffer_write_io_error(struct buffer_head *bh)', '}'),
    'checkpoint.c:49-124': ('void __jbd2_log_wait_for_space(journal_t *journal)', '}'),
    'checkpoint.c:154-298': ('int jbd2_log_do_checkpoint(journal_t *journal)', '}'),
    'checkpoint.c:318-342': ('int jbd2_cleanup_journal_tail(journal_t *journal)', '}'),
    'checkpoint.c:338-339': ('if (journal->j_flags & JBD2_BARRIER)', 'blkdev_issue_flush(journal->j_fs_dev);'),
    'commit.c:114-159': ('static int journal_submit_commit_record(journal_t *journal,', '}'),
    'commit.c:126-127': ('if (is_journal_aborted(journal))', 'return 0;'),
    'commit.c:152-154': ('if (journal->j_flags & JBD2_BARRIER &&', 'write_flags |= REQ_PREFLUSH | REQ_FUA;'),
    'commit.c:165-178': ('static int journal_wait_on_commit_record(journal_t *journal,', '}'),
    'commit.c:240-247': ('int jbd2_journal_finish_inode_data_buffers(struct jbd2_inode *jinode)', '}'),
    'commit.c:548': ('jbd2_journal_abort(journal, err);', None),
    'commit.c:582-600': ("/* If we're in abort mode, we just un-journal the buffer and", '}'),
    'commit.c:614': ('jbd2_journal_abort(journal, -EIO);', None),
    'commit.c:642': ('jbd2_journal_abort(journal, err);', None),
    'commit.c:738-744': ('err = journal_finish_inode_data_buffers(journal, commit_transaction);', '}'),
    'commit.c:746-765': ('/*', '}'),
    'commit.c:775-778': ('if ((commit_transaction->t_need_data_flush || update_tail) &&', 'blkdev_issue_flush(journal->j_fs_dev);'),
    'commit.c:781-786': ('if (jbd2_has_feature_async_commit(journal)) {', '}'),
    'commit.c:785': ('jbd2_journal_abort(journal, err);', None),
    'commit.c:866': ('jbd2_journal_abort(journal, err);', None),
    'commit.c:874-881': ('if (!jbd2_has_feature_async_commit(journal)) {', 'err = journal_wait_on_commit_record(journal, cbh);'),
    'commit.c:878': ('jbd2_journal_abort(journal, err);', None),
    'commit.c:883-886': ('if (jbd2_has_feature_async_commit(journal) &&', '}'),
    'commit.c:888-889': ('if (err)', 'jbd2_journal_abort(journal, err);'),
    'commit.c:889': ('jbd2_journal_abort(journal, err);', None),
    'commit.c:899-900': ('if (update_tail)', 'jbd2_update_log_tail(journal, first_tid, first_block);'),
    'commit.c:1014-1018': ('if (buffer_jbddirty(bh)) {', 'clear_buffer_jbddirty(bh);'),
    'commit.c:1102-1105': ('commit_transaction->t_state = T_COMMIT_CALLBACK;', 'journal->j_committing_transaction = NULL;'),
    'ext4.h:2267-2274': ('static inline int ext4_emergency_state(struct super_block *sb)', '}'),
    'ext4.h:3197-3198': ('#define ext4_abort(sb, err, fmt, a...)\t\t\t\t\t\\', '__ext4_error((sb), __func__, __LINE__, true, (err), 0, (fmt), ## a)'),
    'ext4_jbd2.c:65-91': ('static int ext4_journal_check_start(struct super_block *sb)', '}'),
    'ext4_jbd2.c:72-74': ('ret = ext4_emergency_state(sb);', 'return ret;'),
    'ext4_jbd2.c:81-89': ('/*', '}'),
    'ext4_jbd2.c:93-117': ('handle_t *__ext4_journal_start_sb(struct inode *inode,', '}'),
    'ext4_jbd2.h:354-365': ('static inline void ext4_update_inode_fsync_trans(handle_t *handle,', '}'),
    'fast_commit.c:1207-1208': ('if (!test_opt2(sb, JOURNAL_FAST_COMMIT))', 'return jbd2_complete_transaction(journal, commit_tid);'),
    'fs-writeback.c:2495-2526': ('void __mark_inode_dirty(struct inode *inode, int flags)', 'if (sb->s_op->dirty_inode)'),
    'fsync.c:97-116': ('static int ext4_fsync_journal(struct inode *inode, bool datasync,', '}'),
    'fsync.c:108-109': ('if (!S_ISREG(inode->i_mode))', 'return ext4_force_commit(inode->i_sb);'),
    'fsync.c:111-113': ('if (journal->j_flags & JBD2_BARRIER &&', '*needs_barrier = true;'),
    'fsync.c:111-115': ('if (journal->j_flags & JBD2_BARRIER &&', 'return ext4_fc_commit(journal, commit_tid);'),
    'fsync.c:115': ('return ext4_fc_commit(journal, commit_tid);', None),
    'fsync.c:129-177': ('int ext4_sync_file(struct file *file, loff_t start, loff_t end, int data', '}'),
    'fsync.c:135-137': ('ret = ext4_emergency_state(inode->i_sb);', 'return ret;'),
    'fsync.c:143-144': ('if (sb_rdonly(inode->i_sb))', 'goto out;'),
    'fsync.c:166-170': ('if (needs_barrier) {', '}'),
    'inode.c:5400-5416': ('if (journal) {', '}'),
    'inode.c:5845': ('int ext4_setattr(struct mnt_idmap *idmap, struct dentry *dentry,', None),
    'inode.c:5854-5856': ('error = ext4_emergency_state(inode->i_sb);', 'return error;'),
    'inode.c:6052-6056': ('if (!error) {', 'mark_inode_dirty(inode);'),
    'inode.c:6531-6540': ('void ext4_dirty_inode(struct inode *inode, int flags)', '}'),
    'jbd2.h:1701-1707': ('static inline int jbd2_check_fs_dev_write_error(journal_t *journal)', '}'),
    'journal.c:197-204': ('if (journal->j_commit_sequence != journal->j_commit_request) {', '}'),
    'journal.c:241-245': ('transaction = journal->j_running_transaction;', '}'),
    'journal.c:499-527': ('static int __jbd2_journal_force_commit(journal_t *journal)', '}'),
    'journal.c:513-517': ('if (!transaction) {', '}'),
    'journal.c:603-645': ('int jbd2_trans_will_send_data_barrier(journal_t *journal, tid_t tid)', '}'),
    'journal.c:611-613': ('/* Transaction already committed? */', 'goto out;'),
    'journal.c:633-636': ('if (journal->j_fs_dev != journal->j_dev) {', 'goto out;'),
    'journal.c:652-692': ('int jbd2_log_wait_commit(journal_t *journal, tid_t tid)', '}'),
    'journal.c:678-686': ('while (tid_gt(tid, journal->j_commit_sequence)) {', '}'),
    'journal.c:678-690': ('while (tid_gt(tid, journal->j_commit_sequence)) {', 'err = -EIO;'),
    'journal.c:689-690': ('if (unlikely(is_journal_aborted(journal)))', 'err = -EIO;'),
    'journal.c:787-808': ('int jbd2_complete_transaction(journal_t *journal, tid_t tid)', '}'),
    'journal.c:792-798': ('if (journal->j_running_transaction &&', 'goto wait_commit;'),
    'journal.c:792-807': ('if (journal->j_running_transaction &&', 'return jbd2_log_wait_commit(journal, tid);'),
    'journal.c:800-805': ('} else if (!(journal->j_committing_transaction &&', 'return 0;'),
    'journal.c:1056-1091': ('int __jbd2_update_log_tail(journal_t *journal, tid_t tid, unsigned long ', '}'),
    'journal.c:1069': ('ret = jbd2_journal_update_sb_log_tail(journal, tid, block, REQ_FUA);', None),
    'journal.c:1222-1229': ('static void jbd2_stats_proc_init(journal_t *journal)', '}'),
    'journal.c:1641-1657': ('journal_t *jbd2_journal_init_dev(struct block_device *bdev,', '}'),
    'journal.c:1651-1653': ('snprintf(journal->j_devname, sizeof(journal->j_devname),', "strreplace(journal->j_devname, '/', '!');"),
    'journal.c:1667-1697': ('journal_t *jbd2_journal_init_inode(struct inode *inode)', '}'),
    'journal.c:1684': ('journal = journal_init_common(inode->i_sb->s_bdev, inode->i_sb->s_bdev,', None),
    'journal.c:1691-1693': ('snprintf(journal->j_devname, sizeof(journal->j_devname),', "strreplace(journal->j_devname, '/', '!');"),
    'journal.c:1827-1837': ('if (buffer_write_io_error(bh)) {', '}'),
    'journal.c:1852-1885': ('int jbd2_journal_update_sb_log_tail(journal_t *journal, tid_t tail_tid,', '}'),
    'journal.c:1859-1860': ('if (is_journal_aborted(journal))', 'return -EIO;'),
    'journal.c:1861-1864': ('if (jbd2_check_fs_dev_write_error(journal)) {', '}'),
    'journal.c:2116-2193': ('int jbd2_journal_destroy(journal_t *journal)', '}'),
    'journal.c:2402-2470': ('int jbd2_journal_flush(journal_t *journal, unsigned int flags)', '}'),
    'journal.c:2515-2516': ('* journal (not of a single transaction).  This operation cannot be', '* undone without closing and reopening the journal.'),
    'journal.c:2549-2597': ('void jbd2_journal_abort(journal_t *journal, int errno)', '}'),
    'journal.c:2565-2589': ('write_lock(&journal->j_state_lock);', 'write_unlock(&journal->j_state_lock);'),
    'journal.rst:31-40': ('In case of ``data=ordered`` mode, Ext4 also supports fast commits which', 'commits. This feature needs to be enabled at mkfs time.'),
    'recovery.c:282-345': ('int jbd2_journal_recover(journal_t *journal)', '}'),
    'recovery.c:339-343': ('if (journal->j_flags & JBD2_BARRIER) {', '}'),
    'recovery.c:611': ('next_log_block = be32_to_cpu(sb->s_start);', None),
    'super.c:680-733': ('static void ext4_handle_error(struct super_block *sb, bool force_ro, int', '}'),
    'super.c:717-720': ('if (test_opt(sb, ERRORS_PANIC) && !system_going_down()) {', '}'),
    'super.c:722-732': ('if (ext4_emergency_ro(sb) || continue_fs)', 'set_bit(EXT4_FLAGS_EMERGENCY_RO, &EXT4_SB(sb)->s_ext4_flags);'),
    'super.c:727': ("* We don't set SB_RDONLY because that requires sb->s_umount", None),
    'super.c:732': ('set_bit(EXT4_FLAGS_EMERGENCY_RO, &EXT4_SB(sb)->s_ext4_flags);', None),
    'super.c:1726-1727': ('fsparam_flag\t("norecovery",\t\tOpt_noload),', 'fsparam_flag\t("noload",\t\tOpt_noload),'),
    'super.c:1854-1860': ('{Opt_journal_async_commit, (EXT4_MOUNT_JOURNAL_ASYNC_COMMIT |', '{Opt_nobarrier, EXT4_MOUNT_BARRIER, MOPT_CLEAR},'),
    'super.c:1893-1896': ('#ifdef CONFIG_EXT4_DEBUG', '#endif'),
    'super.c:2903-2911': ('static const char *token2str(int token)', '}'),
    'super.c:2918-3042': ('static int _ext4_show_options(struct seq_file *seq, struct super_block *', '}'),
    'super.c:2949-2951': ('/* skip if same as the default */', 'continue;'),
    'super.c:2985-2993': ('if (nodefs || EXT4_MOUNT_DATA_FLAGS &', '}'),
    'super.c:3034-3038': ('if (ext4_emergency_ro(sb))', 'SEQ_OPTS_PUTS("shutdown");'),
    'super.c:3049-3058': ('int ext4_seq_options_show(struct seq_file *seq, void *offset)', '}'),
    'super.c:4058-4095': ('static int set_journal_csum_feature_set(struct super_block *sb)', '}'),
    'super.c:4330-4386': ('static void ext4_set_def_opts(struct super_block *sb,', '}'),
    'super.c:4349-4350': ('if (ext4_has_feature_fast_commit(sb))', 'set_opt2(sb, JOURNAL_FAST_COMMIT);'),
    'super.c:4963-4968': ('if (test_opt(sb, DATA_FLAGS) == EXT4_MOUNT_ORDERED_DATA &&', '}'),
    'super.c:5298-5303': ('err = parse_apply_sb_mount_options(sb, ctx);', 'sbi->s_def_mount_opt2 = sbi->s_mount_opt2;'),
    'super.c:5415-5444': ('if (!test_opt(sb, NOLOAD) && ext4_has_feature_journal(sb)) {', '}'),
    'super.c:5768-5788': ('static void ext4_init_journal_params(struct super_block *sb, journal_t *', '}'),
    'super.c:5984': ('static int ext4_load_journal(struct super_block *sb,', None),
    'super.c:5998-6020': ('if (journal_devnum &&', '}'),
    'super.c:6006-6010': ('if (journal_inum && journal_dev) {', '}'),
    'sync.c:205-213': ('static int do_fsync(unsigned int fd, int datasync)', '}'),
    'sysfs.c:571-583': ('err = kobject_init_and_add(&sbi->s_kobj, &ext4_sb_ktype, ext4_root,', 'ext4_seq_options_show, sb);'),
    'transaction.c:272-279': ('if (jbd2_log_space_left(journal) < journal->j_max_transaction_buffers) {', '__jbd2_log_wait_for_space(journal);'),
    'transaction.c:312': ('static int start_this_handle(journal_t *journal, handle_t *handle,', None),
    'transaction.c:366-371': ('if (is_journal_aborted(journal) ||', '}'),
    'utimes.c:20-66': ('int vfs_utimes(const struct path *path, struct timespec64 *times)', 'error = notify_change(mnt_idmap(path->mnt), path->dentry, &newattrs,'),
    # R5
    'base/core.c:3257-3264': ('if (parent == NULL)', '}'),
    'base/core.c:3257-3258': ('if (parent == NULL)', 'parent_kobj = virtual_device_parent();'),
    'blk-flush.c:160-163': ('if (likely(!error))', 'seq = REQ_FSEQ_DONE;'),
    'blk-flush.c:182-192': ('case REQ_FSEQ_DONE:', 'break;'),
    'blk-flush.c:22-23': ('* If the device has writeback cache and supports FUA, REQ_PREFLUSH is', '* translated to PREFLUSH but REQ_FUA is passed down directly with DATA.'),
    'blk-flush.c:25-26': ("* If the device has writeback cache and doesn't support FUA, REQ_PREFLUSH", '* is translated to PREFLUSH and REQ_FUA to POSTFLUSH.'),
    'blk-flush.c:398-403': ('if (blk_queue_write_cache(q)) {', '}'),
    'blk-flush.c:437-446': ('case REQ_FSEQ_DATA | REQ_FSEQ_POSTFLUSH:', 'spin_unlock_irq(&fq->mq_flush_lock);'),
    'blk-flush.c:8-26': ('* REQ_{PREFLUSH|FUA} requests are decomposed to sequences consisted of three', '* is translated to PREFLUSH and REQ_FUA to POSTFLUSH.'),
    'blk-settings.c:468-469': ('if (!(lim->features & BLK_FEAT_WRITE_CACHE))', 'lim->features &= ~BLK_FEAT_FUA;'),
    'blk-sysfs.c:284': ('QUEUE_SYSFS_FEATURE_SHOW(fua, BLK_FEAT_FUA);', None),
    'blk-sysfs.c:452-457': ('static ssize_t queue_wc_show(struct gendisk *disk, char *page)', '}'),
    'blk-sysfs.c:459-478': ('static int queue_wc_store(struct gendisk *disk, const char *page,', '}'),
    'blk-sysfs.c:493-497': ('#define QUEUE_LIM_RO_ENTRY(_prefix, _name)\t\t\t\\', '}'),
    'blk-sysfs.c:499-504': ('#define QUEUE_LIM_RW_ENTRY(_prefix, _name)\t\t\t\\', '}'),
    'blk-sysfs.c:554': ('QUEUE_LIM_RW_ENTRY(queue_wc, "write_cache");', None),
    'blk-sysfs.c:555': ('QUEUE_LIM_RO_ENTRY(queue_fua, "fua");', None),
    'blkdev.h:1464-1468': ('static inline bool blk_queue_write_cache(struct request_queue *q)', '}'),
    'blkdev.h:302-306': ('/* supports a volatile write cache */', '#define BLK_FEAT_FUA\t\t\t((__force blk_features_t)(1u << 1))'),
    'blkdev.h:360-361': ('/* do not send FLUSH/FUA commands despite advertising a write cache */', '#define BLK_FLAG_WRITE_CACHE_DISABLED\t((__force blk_flags_t)(1u << 0))'),
    'buffer.c:2833-2843': ('void write_dirty_buffer(struct buffer_head *bh, blk_opf_t op_flags)', '}'),
    'buffer.c:387-424': ('static void end_buffer_async_write(struct buffer_head *bh, int uptodate)', '}'),
    'checkpoint.c:127-144': ('__flush_batch(journal_t *journal, int *batch_count)', '}'),
    'checkpoint.c:235-248': ('if (!trylock_buffer(bh)) {', 'goto retry;'),
    'checkpoint.c:235-258': ('if (!trylock_buffer(bh)) {', 'goto out;'),
    'checkpoint.c:249-258': ('} else if (!buffer_dirty(bh)) {', 'goto out;'),
    'checkpoint.c:310-315': ('* This is the only part of the journaling code which really needs to be', '* buffers which should be written-back to the filesystem.'),
    'checkpoint.c:323-324': ('if (is_journal_aborted(journal))', 'return -EIO;'),
    'checkpoint.c:566-618': ('int __jbd2_journal_remove_checkpoint(struct journal_head *jh)', '}'),
    'checkpoint.c:627-648': ('int jbd2_journal_try_remove_checkpoint(struct journal_head *jh)', '}'),
    'commit.c:803-840': ('while (!list_empty(&io_bufs)) {', '}'),
    'commit.c:847-863': ('while (!list_empty(&log_bufs)) {', '}'),
    'errseq.c:146-153': ('int errseq_check(errseq_t *eseq, errseq_t since)', '}'),
    'errseq.c:22-23': ('* Note that there is a risk of collisions if new errors are being recorded', '* frequently, since we have so few bits to use as a counter.'),
    'errseq.c:36-46': ('/* The low bits are designated for error code (max of MAX_ERRNO) */', '#define ERRSEQ_CTR_INC\t\t(1 << (ERRSEQ_SHIFT + 1))'),
    'errseq.c:62-109': ('errseq_t errseq_set(errseq_t *eseq, int err)', '}'),
    'errseq.c:75-77': ('if (WARN(unlikely(err == 0 || (unsigned int)-err > MAX_ERRNO),', 'return old;'),
    'errseq.c:83': ('new = (old & ~(ERRNO_MASK | ERRSEQ_SEEN)) | -err;', None),
    'filemap.c:709-714': ('void __filemap_set_wb_err(struct address_space *mapping, int err)', '}'),
    'filemap.c:785-804': ('int file_write_and_wait_range(struct file *file, loff_t lstart, loff_t lend)', '}'),
    'fsync.c:154': ('ret = file_write_and_wait_range(file, start, end);', None),
    'fsync.c:172-174': ('err = file_check_and_advance_wb_err(file);', 'ret = err;'),
    'jbd2.h:1690-1699': ('static inline void jbd2_init_fs_dev_write_error(journal_t *journal)', '}'),
    'journal.c:1017-1044': ('int jbd2_journal_get_log_tail(journal_t *journal, tid_t *tid,', '}'),
    'journal.c:1069-1071': ('ret = jbd2_journal_update_sb_log_tail(journal, tid, block, REQ_FUA);', 'goto out;'),
    'journal.c:1084-1086': ('journal->j_free += freed;', 'journal->j_tail = block;'),
    'journal.c:1098': ('void jbd2_update_log_tail(journal_t *journal, tid_t tid, unsigned long block)', None),
    'journal.c:1538': ('jbd2_init_fs_dev_write_error(journal);', None),
    'journal.c:1784-1840': ('static int jbd2_write_superblock(journal_t *journal, blk_opf_t write_flags)', '}'),
    'journal.c:1871-1872': ('sb->s_sequence = cpu_to_be32(tail_tid);', 'sb->s_start    = cpu_to_be32(tail_block);'),
    'journal.c:2039-2052': ('void jbd2_journal_update_sb_errno(journal_t *journal)', '}'),
    'journal.c:2592-2595': ('* Record errno to the journal super block, so that fsck and jbd2', 'jbd2_journal_update_sb_errno(journal);'),
    'nvme.h:407': ('NVME_CTRL_VWC_PRESENT\t\t\t= 1 << 0,', None),
    'nvme.h:601': ('NVME_NS_VWC_NOT_PRESENT = 1 << 5,', None),
    'nvme/core.c:1676': ('info->no_vwc = id->nsfeat & NVME_NS_VWC_NOT_PRESENT;', None),
    'nvme/core.c:2395-2398': ('if ((ns->ctrl->vwc & NVME_CTRL_VWC_PRESENT) && !info->no_vwc)', 'lim.features &= ~(BLK_FEAT_WRITE_CACHE | BLK_FEAT_FUA);'),
    'nvme/core.c:3238-3242': ('subsys->dev.class = &nvme_subsys_class;', 'device_initialize(&subsys->dev);'),
    'nvme/core.c:3578': ('ctrl->vwc = id->vwc;', None),
    'nvme/core.c:4146-4149': ('if (nvme_ns_head_multipath(ns->head)) {', 'disk->flags |= GENHD_FL_HIDDEN;'),
    'nvme/core.c:4150-4152': ('} else if (multipath) {', 'ns->head->instance);'),
    'nvme/core.c:4175': ('if (device_add_disk(ctrl->device, ns->disk, nvme_ns_attr_groups))', None),
    'nvme/core.c:5121-5126': ('device_initialize(&ctrl->ctrl_device);', 'ctrl->device->parent = ctrl->dev;'),
    'nvme/core.c:5154': ('ret = dev_set_name(ctrl->device, "nvme%d", ctrl->instance);', None),
    'nvme/multipath.c:736-740': ('if (!multipath_always_on) {', '}'),
    'nvme/multipath.c:787-788': ('rc = device_add_disk(&head->subsys->dev, head->disk,', 'nvme_ns_attr_groups);'),
    'nvme/pci.c:3212-3217': ('static int nvme_pci_get_address(struct nvme_ctrl *ctrl, char *buf, int size)', '}'),
    'nvme/pci.c:3240-3241': ('static const struct nvme_ctrl_ops nvme_pci_ctrl_ops = {', '.name\t\t\t= "pcie",'),
    'nvme/sysfs.c:103-132': ('static ssize_t wwid_show(struct device *dev, struct device_attribute *attr,', 'static DEVICE_ATTR_RO(wwid);'),
    'nvme/sysfs.c:362-374': ('#define nvme_show_str_function(field)\t\t\t\t\t\t\\', 'nvme_show_str_function(firmware_rev);'),
    'nvme/sysfs.c:406-414': ('static ssize_t nvme_sysfs_show_transport(struct device *dev,', 'static DEVICE_ATTR(transport, S_IRUGO, nvme_sysfs_show_transport, NULL);'),
    'nvme/sysfs.c:470-478': ('static ssize_t nvme_sysfs_show_address(struct device *dev,', 'static DEVICE_ATTR(address, S_IRUGO, nvme_sysfs_show_address, NULL);'),
    'page-io.c:100-147': ('static void ext4_finish_bio(struct bio *bio)', '}'),
    'page-io.c:118-121': ('if (bio->bi_status) {', '}'),
    'page-io.c:142-145': ('if (!under_io) {', '}'),
    'page-io.c:349-395': ('static void ext4_end_bio(struct bio *bio)', '}'),
    'page-io.c:365-376': ('if (bio->bi_status) {', '}'),
    'pagemap.h:239-256': ('static inline void mapping_set_error(struct address_space *mapping, int error)', '}'),
    'transaction.c:1222-1232': ('if (jbd2_check_fs_dev_write_error(journal)) {', '}'),
    'writeback_cache_control.rst:23-27': ('The REQ_PREFLUSH flag can be OR ed into the r/w flags of a bio submitted from', 'storage before the flagged bio starts. In addition the REQ_PREFLUSH flag can be'),
    'writeback_cache_control.rst:52-55': ('For devices that do not support volatile write caches there is no driver', 'requests that have a payload.'),
    'writeback_cache_control.rst:57-68': ('For devices with volatile write caches the driver needs to tell the block layer', 'flag in the features field of the queue_limits structure.'),
    'writeback_cache_control.rst:8-13': ('Many storage devices, especially in the consumer market, come with volatile', 'a data integrity operation like fsync, sync or an unmount.'),
    'writeback_cache_control.rst:92-95': ('When the BLK_FEAT_FUA flags is set, the REQ_FUA bit is simply passed on for the', 'bit set.'),
}

KREF = re.compile(r"(?<![\w./-])(" + "|".join(
    re.escape(s) for s in sorted(KERNEL_FILES, key=len, reverse=True)) + r"):(\d+)(?:-(\d+))?")


def git(repo, *args):
    return subprocess.run(["git", "-C", str(repo), *args], capture_output=True, text=True,
                          check=True).stdout


def load_model(path):
    sys.dont_write_bytecode = True
    spec = importlib.util.spec_from_file_location("design_checks_r5", path)
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


def computed_journal(model):
    """The enumeration counts of section 10.7, as the model's activation-proof
    check computes them."""
    model.check_c28_activation_proof()
    stats = dict(model.JOURNAL_STATS)
    completed = stats["certified through the completed branch with the journal aborted"]
    late = stats["certified, the journal aborted by then"]
    return {
        "Initial journal states: the two dependencies running, committing, committed or failed, "
        "with unrelated work": stats["initial states"],
        "Activation runs: any background event or none before each step, an abort inside the "
        "probe's handle start or not, forced commits succeeding or failing": stats["runs"],
        "Certified": stats["certified"],
        "Refused": stats["refused"],
        "Refused although the dependencies had committed (conservative)":
            stats["refused, the dependencies committed (a conservative refusal)"],
        "Certified with the journal aborted after the probe's test, each through the "
        "completed-transaction return": late if late == completed else ("differs", late, completed),
        "Later continuations of certified runs on admitted storage, failures the storage reports "
        "included, each followed by a power loss, every certified dependency kept (R5)":
            stats["continuations, then power loss"],
    }


def computed_storage(model):
    """The storage-event dimensions of section 15.4 (R5), as the model's
    storage-composed check computes them."""
    model.check_c32_storage_composed()
    stats = dict(model.STORAGE_COMPOSED_STATS)
    unsupported = sum(v for k, v in stats.items() if k.startswith("no dependent work")
                      and not k.startswith("no dependent work: admitted") and k.endswith(": Unsupported"))
    unprovisioned = sum(v for k, v in stats.items() if k.startswith("no dependent work")
                        and not k.startswith("no dependent work: admitted") and k.endswith(": Unprovisioned"))
    return {
        "Administrative prefixes: every operation of first provisioning and of succession, each "
        "followed by process death": stats["prefixes"],
        "Openings on the four hosts": stats["openings"],
        "Runs with dependent work on admitted storage": stats["runs with dependent work: admitted storage"],
        "Second crashes after the storage events: F1, or F2 under every distinct schedule with data "
        "old and new": stats["second crashes"],
        "Second crashes after a loss of qualification, each refused with the history kept":
            stats["loss of qualification: second crashes refused with the history kept"],
        "Openings on storage that is not admitted, refused as Unsupported before any dependent work":
            unsupported,
        "Openings on storage that is not admitted that found no `PROVISION` yet (Unprovisioned)":
            unprovisioned,
    }


def kernel_citations(doc):
    """Every Linux `file:line` citation in the document, against KERNEL_EXPECT."""
    found = set(m.group(0) for m in KREF.finditer(doc))
    failures = []
    missing = sorted(found - set(KERNEL_EXPECT))
    unused = sorted(set(KERNEL_EXPECT) - found)
    if missing:
        failures.append(f"kernel citations without an expectation: {missing}")
    if unused:
        failures.append(f"kernel expectations not cited: {unused}")
    return found, failures


def check_kernel(kernel_dir, expect=None):
    """Every kernel citation against local copies of the cited v6.17 files."""
    import hashlib
    expect = KERNEL_EXPECT if expect is None else expect
    failures = []
    texts = {}
    for short, (path, digest) in KERNEL_FILES.items():
        local = pathlib.Path(kernel_dir) / path
        if not local.is_file():
            local = pathlib.Path(kernel_dir) / path.replace("/", "_")
        if not local.is_file():
            failures.append(f"kernel file {path} not found under {kernel_dir}")
            continue
        if hashlib.sha256(local.read_bytes()).hexdigest() != digest:
            failures.append(f"kernel file {path}: SHA-256 differs from the cited v6.17 file")
            continue
        texts[short] = local.read_text(encoding="utf-8").splitlines()
    for ref, (first, last) in sorted(expect.items()):
        short, span = ref.rsplit(":", 1)
        lo, _, hi = span.partition("-")
        lo = int(lo)
        hi = int(hi) if hi else lo
        lines = texts.get(short)
        if lines is None:
            continue
        if not 1 <= lo <= hi <= len(lines):
            failures.append(f"kernel citation {ref}: out of range ({len(lines)} lines)")
            continue
        good = first in lines[lo - 1] and (last is None or last in lines[hi - 1])
        if good and last is not None and lines[hi - 1].strip() in ("}", "};"):
            indent = lines[lo - 1][:len(lines[lo - 1]) - len(lines[lo - 1].lstrip())]
            closing = lines[hi - 1][:len(lines[hi - 1]) - len(lines[hi - 1].lstrip())]
            good = indent == closing
        if not good:
            failures.append(f"kernel citation {ref}: line {lo} {lines[lo - 1].strip()[:80]!r}; "
                            f"line {hi} {lines[hi - 1].strip()[:80]!r}")
    return failures


def check(repo, doc, model_text, model, coverage, composed=None, journal=None, storage=None):
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
    m = re.search(r"There are (\d+)\. R1's 27", checks_text)
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

    # 10. Every Linux citation has an expectation, and every expectation is cited.
    kfound, kfail = kernel_citations(doc)
    failures.extend(kfail)
    out.append(f"kernel citations in the document: {len(kfound)}; expectations {len(KERNEL_EXPECT)}")

    # 11. The enumeration counts of section 10.7.
    if journal is not None:
        proof_text = section(doc, "### 10.7 ", "## 11. ")
        rows = dict((k.strip(), int(v)) for k, v in
                    re.findall(r"^\| ([^|]+?) \| (\d+) \|$", proof_text, re.M))
        for label, value in journal.items():
            if rows.get(label) != value:
                failures.append(f"enumeration count {label!r}: document {rows.get(label)}, model {value}")
        out.append(f"enumeration counts: document {len(rows)}, model {len(journal)}")

    # 12. The required and excluded options of section 6.4 step 9; the journal inode.
    profile_text = section(doc, "### 6.4 ", "### 6.5 ")
    m_req = re.search(r"it contains ((?:`[^`]+`(?:, | and )?)+);", profile_text)
    m_exc = re.search(r"it contains none of ((?:`[^`]+`(?:, | or )?)+);", profile_text)
    required = tuple(re.findall(r"`([^`]+)`", m_req.group(1))) if m_req else None
    excluded = tuple(re.findall(r"`([^`]+)`", m_exc.group(1))) if m_exc else None
    if required != tuple(model.REQUIRED_OPTIONS):
        failures.append(f"required options: document {required}, model {model.REQUIRED_OPTIONS}")
    if excluded != tuple(model.EXCLUDED_OPTIONS):
        failures.append(f"excluded options: document {excluded}, model {model.EXCLUDED_OPTIONS}")
    m_jnl = re.search(r"`/proc/fs/jbd2/<name>-(\d+)` must exist", profile_text)
    if m_jnl is None or int(m_jnl.group(1)) != model.JOURNAL_INODE:
        failures.append(f"journal inode: document {m_jnl.group(1) if m_jnl else None}, "
                        f"model {model.JOURNAL_INODE}")
    out.append(f"profile options: required {len(required or ())}, excluded {len(excluded or ())}; "
               f"model {len(model.REQUIRED_OPTIONS)}, {len(model.EXCLUDED_OPTIONS)}")

    # 13. The PROVISION keys of section 7.5, in order.
    grammar = section(doc, "### 7.5 ", "### 7.6 ")
    block = grammar[grammar.index("nexus-phase2-custody-provision 1"):]
    block = block[:block.index("```")]
    keys = [m.group(1) for m in re.finditer(r"^([a-z0-9-]+)=", block, re.M)]
    expected_keys = list(model.PROV_HEAD) + ["pool-00000-inode"] + list(model.PROV_TAIL) + ["digest"]
    if keys != expected_keys:
        failures.append(f"PROVISION keys: document {keys}, model {expected_keys}")
    out.append(f"PROVISION keys: document {len(keys)}, model {len(expected_keys)}")

    # 14. (R5) The storage rule of section 6.4 step 12.
    step = profile_text[profile_text.index("12. **The storage (R5).**"):]
    found = {
        "class": re.findall(r"must name the class `([^`]+)`", step),
        "cache": re.findall(r"`write_cache` is exactly `([^`]+)` and `fua` exactly `([^`]+)`", step),
        "prefix": re.findall(r"The ASCII prefix `([^`]+)`, a zero byte and the version byte `01`", step),
        "attributes": re.findall(r"for each of `(\w+)`, `(\w+)`, `(\w+)` and `(\w+)` in that order", step),
    }
    wanted = {
        "class": [model.STORAGE_CLASS],
        "cache": [tuple(v.rstrip("\n") for v in model.STORAGE_CACHE_ADMITTED)],
        "prefix": [model.STORAGE_IDENTITY_PREFIX[:-2].decode("ascii")]
        if model.STORAGE_IDENTITY_PREFIX.endswith(b"\x00\x01") else ["(version differs)"],
        "attributes": [tuple(model.STORAGE_IDENTITY_ATTRS)],
    }
    for key in wanted:
        if found[key] != wanted[key]:
            failures.append(f"storage {key}: document {found[key]}, model {wanted[key]}")
    if f"storage-class={model.STORAGE_CLASS}" not in grammar:
        failures.append("storage class: section 7.5 does not fix it")
    out.append(f"storage rule: class, cache values, identity prefix and {len(model.STORAGE_IDENTITY_ATTRS)} "
               f"identity attributes compared with the model")

    # 15. (R5) The storage-event dimensions of section 15.4.
    if storage is not None:
        comp_text = section(doc, "### 15.4 ", "## 16. ")
        rows = dict((k.strip(), int(v)) for k, v in
                    re.findall(r"^\| ([^|]+?) \| (\d+) \|$", comp_text, re.M))
        for label, value in storage.items():
            if rows.get(label) != value:
                failures.append(f"storage dimension {label!r}: document {rows.get(label)}, model {value}")
        out.append(f"storage-event dimensions: model {len(storage)}")
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
    journal = computed_journal(model)
    storage = computed_storage(model)
    out, failures = check(repo, doc, model_text, model, coverage, composed, journal, storage)
    if args.kernel:
        kfail = check_kernel(args.kernel)
        failures.extend(kfail)
        out.append(f"kernel citations against the files: {len(KERNEL_EXPECT)} citations in "
                   f"{len(KERNEL_FILES)} files, SHA-256 verified, "
                   f"{'all match' if not kfail else str(len(kfail)) + ' failures'}")
    else:
        out.append("kernel citations against the files: not checked (no --kernel)")
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
            ("kernel citation moved", doc.replace("`checkpoint.c:338-339`", "`checkpoint.c:337-339`", 1),
             model_text, "checkpoint.c:337-339"),
            ("enumeration count", doc.replace("| Certified | ", "| Certified | 9", 1),
             model_text, "enumeration count 'Certified'"),
            ("excluded option dropped", doc.replace("`data=writeback`, `journal_async_commit`, ",
                                                    "`data=writeback`, ", 1),
             model_text, "excluded options"),
            ("PROVISION key", doc.replace("\nkernel=<statement", "\nkernel-identity=<statement", 1),
             model_text, "PROVISION keys"),
            ("R5 kernel citation moved", doc.replace("`blk-sysfs.c:459-478`", "`blk-sysfs.c:458-478`", 1),
             model_text, "blk-sysfs.c:458-478"),
            ("admitted fua value", doc.replace("and `fua` exactly `0`", "and `fua` exactly `1`", 1),
             model_text, "storage cache"),
            ("identity attributes' order", doc.replace(
                "for each of `model`, `serial`, `firmware_rev` and `wwid` in that order",
                "for each of `serial`, `model`, `firmware_rev` and `wwid` in that order", 1),
             model_text, "storage attributes"),
            ("storage-event count", doc.replace("| Runs with dependent work on admitted storage | ",
                                                "| Runs with dependent work on admitted storage | 9", 1),
             model_text, "storage dimension"),
        ]
        results = []
        for label, d, mt, needle in corruptions:
            if d == doc and mt == model_text:
                results.append((label, False))
                continue
            _, f = check(repo, d, mt, model, coverage, composed, journal, storage)
            results.append((label, any(needle in x for x in f)))
        if args.kernel:
            altered = dict(KERNEL_EXPECT)
            altered["journal.c:800-805"] = ("} else if (!(journal->j_running_transaction &&", "return 0;")
            f = check_kernel(args.kernel, altered)
            results.append(("kernel expectation altered", any("journal.c:800-805" in x for x in f)))
            altered = dict(KERNEL_EXPECT)
            altered["blk-sysfs.c:459-478"] = ("static ssize_t queue_wc_show(struct gendisk *disk, char *page)", "}")
            f = check_kernel(args.kernel, altered)
            results.append(("R5 kernel expectation altered", any("blk-sysfs.c:459-478" in x for x in f)))
        out.append("self-test: " + "; ".join(f"{label} reported {ok}" for label, ok in results))
        if not all(ok for _, ok in results):
            failures.append("self-test: a corruption was not reported")
            out.append("FAIL self-test")
    out.append("RESULT: " + ("PASS" if not failures else "FAIL"))
    print("\n".join(out))
    return 0 if not failures else 1


if __name__ == "__main__":
    sys.exit(main())
