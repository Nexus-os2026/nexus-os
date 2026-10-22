#!/usr/bin/env python3
"""P2-V1-R3B-I3-I1-R1 negative controls for the custody store.

Adapted from docs/evidence/p2-v1-r3b-i3-i1/scripts/store_controls.py (which
stays unchanged); the adaptation diff is store_controls.adaptation.diff. The
90 I3-I1 controls keep their intentions: three anchors moved to the
equivalent defect point of the repaired sources (I-VALIDATOR-INEXACT,
I-SEAL-WHILE-LATCHED, A-WORKER-LOCK), and one control (A-ADMISSION-UNBOUND)
has no remaining behavioural call site and is listed in REMAPPED, with the
API/type guards that now carry its intention (api_probes.py); it is not run
and not counted here. The R1 boundaries add their own behavioural controls
(R1_BINDINGS, category authority-binding).

Each counted control restores one wrong behaviour in the store's own test
infrastructure (support/custody/store/) and must:

- apply: every edit's anchor occurs exactly once in its file, in order;
- compile: the store target builds (no "error[E", no "could not compile");
- run its one intended test alone (--exact) and fail it ("0 passed; 1
  failed", "<test> ... FAILED");
- fail the intended assertion: the panic message carries the control's
  marker (and every `require` string, and no `forbid` string);
- restore: the mutated files are written back byte for byte in `finally`;
  every file of FILES is verified by SHA-256 before each control and after
  each restoration, and the worktree status must equal the expected status.

Categories, counted separately and never added into one figure:
implementation-safety (the store's own logic, including the Linux
primitives that the N- controls exercise beneath CARGO_TARGET_TMPDIR),
simulator-source-conformance (the simulator's fidelity to the cited kernel
paths), and authority-api-surface (an authority the store must not have,
caught by a source or type guard rather than by behaviour). Design controls
of the R5 Python model that no Rust mutation restores are listed in the
coverage matrix as historical Python-model controls; they are not counted
here.

The protected custody core (core.rs, model.rs, codec.rs) and its tests are
never mutated by this script; they are hashed to prove it.

Usage:
  store_controls.py <checkout> <log directory> [--expected-status FILE]
  store_controls.py <checkout> --check-anchors

<checkout> must hold exactly the candidate's sources (CANDIDATE_SOURCES).
Without --expected-status the checkout must be clean (the committed
candidate). It mutates and restores files in the checkout, so run it alone,
with nothing else building, testing or editing there.
"""
import hashlib
import json
import pathlib
import subprocess
import sys

TESTS = "crates/nexus-verifier-sandbox/tests"
CUSTODY = f"{TESTS}/support/custody"
STORE = f"{CUSTODY}/store"
TARGET = "phase2_custody_store"
FILES = [
    f"{STORE}/classify.rs",
    f"{STORE}/disposition.rs",
    f"{STORE}/exchange.rs",
    f"{STORE}/faults.rs",
    f"{STORE}/format.rs",
    f"{STORE}/io.rs",
    f"{STORE}/maintenance.rs",
    f"{STORE}/mod.rs",
    f"{STORE}/open.rs",
    f"{STORE}/owner.rs",
    f"{STORE}/recorder.rs",
    f"{STORE}/sim.rs",
    f"{TESTS}/{TARGET}.rs",
    f"{CUSTODY}/mod.rs",
    f"{CUSTODY}/core.rs",
    f"{CUSTODY}/model.rs",
    f"{CUSTODY}/codec.rs",
    f"{TESTS}/phase2_custody_core.rs",
    f"{TESTS}/phase2_custody_codec.rs",
]
# The frozen candidate's sources (store_controls.py <checkout> --print-sources).
CANDIDATE_SOURCES = {}  # R1-CANDIDATE-SOURCES

SIM = f"{STORE}/sim.rs"
FMT = f"{STORE}/format.rs"
CLS = f"{STORE}/classify.rs"
OPN = f"{STORE}/open.rs"
EXC = f"{STORE}/exchange.rs"
REC = f"{STORE}/recorder.rs"
MNT = f"{STORE}/maintenance.rs"
IO = f"{STORE}/io.rs"
DSP = f"{STORE}/disposition.rs"
OWN = f"{STORE}/owner.rs"
FLT = f"{STORE}/faults.rs"


def control(cid, category, design, what, edits, test, marker, require=(), forbid=()):
    return dict(
        id=cid, category=category, design=list(design), what=what,
        edits=[dict(file=f, anchor=a, replacement=r) for (f, a, r) in edits],
        test=test, marker=marker, require=list(require), forbid=list(forbid),
    )

# ---------------------------------------------------------------------------
# Simulator conformance: the simulator's fidelity to the cited kernel paths
# ---------------------------------------------------------------------------

SIMULATOR = [
    control(
        "S-F1-DURABLE", "simulator-source-conformance", ["NC01"],
        "process death makes kernel-visible bytes durable",
        [(SIM, r'''        if let Some(proc) = self.procs.get_mut(&pid) {
            proc.alive = false;
        }
    }

    fn power_loss(
''', r'''        if let Some(proc) = self.procs.get_mut(&pid) {
            proc.alive = false;
        }
        for inode in self.inodes.values_mut() {
            if inode.kind != FileType::Directory {
                inode.d = inode.k.clone();
                inode.pending.clear();
            }
        }
    }

    fn power_loss(
''')],
        "s01_process_death_and_power_loss_keep_states_apart", "[F1-volatile]",
        require=["process death made the write durable"],
    ),
    control(
        "S-REOPEN-CERTIFIES", "simulator-source-conformance", ["NC03"],
        "a sync that returns 0 (as a new description's does after an error was seen) makes the visible bytes durable",
        [(SIM, r'''            if error != 0 && result.is_ok() {
                result = Err(Errno::Io);
            }
        }
        result
    }
''', r'''            if error != 0 && result.is_ok() {
                result = Err(Errno::Io);
            }
        }
        if result.is_ok() {
            if let Some(inode) = state.inodes.get_mut(&ino) {
                inode.d = inode.k.clone();
            }
        }
        result
    }
''')],
        "s02_writeback_errors_are_not_repaired_by_reopening", "[errseq-reopen]",
        require=["the durable bytes lack the write"],
    ),
    control(
        "S-TEAR-SPANS", "simulator-source-conformance", [],
        "a torn write reaches past its own 4096-byte block",
        [(SIM, r'''fn tear(inode: &mut Inode, block: u64, outcome: Tear) {
    let lo = block as usize * BLOCK;
    let hi = ((block as usize + 1) * BLOCK).min(inode.k.len());
''', r'''fn tear(inode: &mut Inode, block: u64, outcome: Tear) {
    let lo = block as usize * BLOCK;
    let hi = ((block as usize + 2) * BLOCK).min(inode.k.len());
''')],
        "s05_tears_stay_within_their_block", "[tear-containment]",
    ),
    control(
        "S-SCHEDULES-GUARANTEED-ONLY", "simulator-source-conformance", ["NC22a"],
        "directory entries become durable only through a directory sync (the R1 model): the ordered family is the guaranteed minimum alone",
        [(SIM, r'''        if ordered {
            let last = self.log.last().map_or(self.op_count, |half| half.op + 1);
            return (self.min_cut()..=last).map(Schedule::Ordered).collect();
        }
''', r'''        if ordered {
            return vec![Schedule::Ordered(self.min_cut())];
        }
''')],
        "s04_metadata_schedules_and_split_renames", "[metadata-schedules]",
        require=["ordered prefixes"],
    ),
    control(
        "S-NO-RECLAIM-WHILE-OPEN", "simulator-source-conformance", ["NC22b"],
        "no page reclaim while a descriptor is open (the R1 model)",
        [(SIM, r'''    pub fn reclaim_pages(&self, ino: Ino) {
        let mut state = self.lock();
''', r'''    pub fn reclaim_pages(&self, ino: Ino) {
        let mut state = self.lock();
        if state.ofds.values().any(|ofd| ofd.ino == ino) {
            return;
        }
''')],
        "s03_page_reclaim_with_a_descriptor_open", "[metadata-schedules]",
    ),
    control(
        "S-ERRSEQ-NO-WRAP", "simulator-source-conformance", [],
        "an invented detection: the error-sequence counter saturates instead of wrapping, so a 2^19-error collision would be detected",
        [(SIM, r'''            new = new.wrapping_add(Self::CTR_INC);
''', r'''            new = new.saturating_add(Self::CTR_INC);
''')],
        "j06_the_errseq_counter_limitation_is_documented_not_detected", "[errseq-limit]",
        require=["no detection"],
    ),
    control(
        "S-COMPLETED-TESTS-ABORT", "simulator-source-conformance", ["NC-FSYNC-COMPLETED"],
        "R3's fsync model: the completed-transaction branch tests the abort flag",
        [(SIM, r'''        self.trace.push(JournalTrace::CompletedBranch {
            aborted: self.aborted,
        });
''', r'''        self.trace.push(JournalTrace::CompletedBranch {
            aborted: self.aborted,
        });
        if self.aborted {
            return Err(Errno::Io);
        }
''')],
        "j01_the_completed_transaction_return", "[journal-conformance]",
        require=["the completed branch returns 0 untested"],
    ),
    control(
        "S-INVENTED-ABORT", "simulator-source-conformance", ["NC-ERR-DETECTED"],
        "a discarded flush status treated as detected: an abort invented at an unchecked flush site",
        [(SIM, r'''        if ok {
            Flush::Ok
        } else {
            Flush::Failed
        }
''', r'''        if ok {
            Flush::Ok
        } else {
            if !checked {
                self.abort("an invented abort at an unchecked flush site");
            }
            Flush::Failed
        }
''')],
        "j02_discarded_flushes_lose_history_on_a_volatile_cache", "[journal-conformance]",
        require=["the discarded status changes nothing"],
    ),
    control(
        "S-INFLIGHT-COMPLETE", "simulator-source-conformance", ["NC-STORAGE-COMPLETION"],
        "durability before completion: a home write still in flight counts as done and releases the tail",
        [(SIM, r'''            HomeState::Written | HomeState::Durable | HomeState::Failed
''', r'''            HomeState::Written | HomeState::Durable | HomeState::Failed | HomeState::InFlight
''')],
        "j03_the_stable_completion_checkpoint", "[stable-completion]",
        require=["in flight"],
    ),
    control(
        "S-HOME-ERROR-SUCCESS", "simulator-source-conformance", ["NC-STORAGE-ERROR"],
        "a reported home-write error converted into success (journal model)",
        [(SIM, r'''        if outcome == Home::Fail {
            self.dev_err = true;
        }
        if let Some(txn) = self.txns.get_mut(&tid) {
            txn.home = match outcome {
                Home::InFlight => HomeState::InFlight,
                Home::Fail => HomeState::Failed,
''', r'''        if let Some(txn) = self.txns.get_mut(&tid) {
            txn.home = match outcome {
                Home::InFlight => HomeState::InFlight,
                Home::Fail => HomeState::Durable,
''')],
        "j03_the_stable_completion_checkpoint", "[stable-completion]",
        require=["a failed home write is reported"],
    ),
    control(
        "S-TAIL-WITHOUT-HOME", "simulator-source-conformance", ["NC-STORAGE-TAIL"],
        "required history discarded: the log tail moves past a failed home write",
        [(SIM, r'''        if self.dev_err {
            self.abort("a home write failed (journal.c:1861-1864)");
            return Err(Errno::Io);
        }
''', "")],
        "j03_the_stable_completion_checkpoint", "[stable-completion]",
        require=["a failed home write"],
    ),
    control(
        "S-WRITEBACK-ERROR-SWALLOWED", "simulator-source-conformance", ["NC-STORAGE-ERROR"],
        "a writeback error the storage reported is not recorded in the error sequence (the sync returns 0)",
        [(SIM, r'''                if fail {
                    inode.errseq.set(EIO);
                    continue;
                }
''', r'''                if fail {
                    continue;
                }
''')],
        "r01_acknowledged_implies_durable_under_write_and_sync_faults", "[ack-durable]",
        require=["a reported write error is never an acknowledgement"],
    ),
]

# ---------------------------------------------------------------------------
# Implementation safety: formats, classification, bindings, dispositions
# ---------------------------------------------------------------------------

FORMATS = [
    control(
        "I-HEADER-RESERVED", "implementation-safety", ["NC10a"],
        "header reserved bytes unchecked",
        [(FMT, r'''    if !zero(&block[60..64]) || !zero(&block[89..92]) {
''', r'''    if false {
''')],
        "f01_header_bytes_are_assigned_and_checked", "[exact-bytes]",
        require=["reserved 60..64"],
    ),
    control(
        "I-SEAL-TAIL", "implementation-safety", ["NC10b"],
        "seal tail bytes unchecked (the first candidate's unassigned bytes)",
        [(FMT, r'''    if !zero(&block[5..8]) || !zero(&block[126..128]) || !zero(&block[160..]) {
''', r'''    if !zero(&block[5..8]) || !zero(&block[126..128]) {
''')],
        "f02_seal_bytes_are_assigned_and_checked", "[exact-bytes]",
        require=["seal tail bytes"],
    ),
    control(
        "I-RECORD-PADDING", "implementation-safety", ["NC10c"],
        "record-block padding unchecked",
        [(FMT, r'''        Ok(_) if !zero(&block[end..]) => RecordBlock::Invalid("padding".into()),
''', "")],
        "f03_record_blocks_take_exactly_one_frame", "[exact-bytes]",
        require=["padding"],
    ),
    control(
        "I-INVALID-HEADER-ABANDONED", "implementation-safety", ["NC10d"],
        "a checksum-valid invalid header taken as an abandoned claim (the first candidate)",
        [(CLS, r'''            Some(HeaderParse::Invalid(why)) => {
                report.class = FileClass::MalformedPoolFile;
                report.reason = format!("header invalid ({why})");
                return finish_report(report);
            }
''', r'''            Some(HeaderParse::Invalid(_)) if self.rest_zero => {
                report.class = FileClass::AbandonedClaim;
                return finish_report(report);
            }
            Some(HeaderParse::Invalid(why)) => {
                report.class = FileClass::MalformedPoolFile;
                report.reason = format!("header invalid ({why})");
                return finish_report(report);
            }
''')],
        "f01_header_bytes_are_assigned_and_checked", "[exact-bytes]",
        require=["taken as AbandonedClaim"],
    ),
    control(
        "I-DECIMAL-LEADING-ZERO", "implementation-safety", ["NC10e"],
        "non-canonical decimals (leading zeros) accepted in text",
        [(FMT, r'''        && (bytes.len() == 1 || bytes[0] != b'0');
''', r'''        && !bytes.is_empty();
''')],
        "f04_provision_grammar_is_exact", "[exact-bytes]",
        require=["leading zero"],
    ),
    control(
        "I-BINDING-LOCATION", "implementation-safety", ["NC11a"],
        "a journal's binding depends on where its bytes lie (the pool index), so the archive copy loses it",
        [(CLS, r'''            let binding = bind_journal(
                &root_id,
                header.claim,
                &header.generation,
                &report.content,
                class,
            );
''', r'''            let binding = bind_journal(
                &root_id,
                header.claim,
                &header.generation,
                &super::format::sha256(&[&report.content[..], &index.to_be_bytes()[..]].concat()),
                class,
            );
''')],
        "m04_archival_recycling_retirement_and_succession", "[archive-binding]",
    ),
    control(
        "I-BINDING-NO-CONTENT", "implementation-safety", ["NC11b"],
        "a journal's binding ignores its content",
        [(FMT, r'''    data.extend_from_slice(generation);
    data.extend_from_slice(content);
    data.push(class.byte());
''', r'''    data.extend_from_slice(generation);
    let _ = content;
    data.push(class.byte());
''')],
        "f07_bindings_are_content_addressed", "[archive-binding]",
        require=["journal binding bytes"],
    ),
    control(
        "I-GRAMMAR-G16", "implementation-safety", ["NC13"],
        "grammar rule G16 (a Failed verdict if and only if admission closed) removed",
        [(CLS, r'''                if (verdict == Verdict::Failed) != self.closed {
                    return Err(Rule::G16);
                }
''', "")],
        "g01_grammar_fixtures_prefixes_and_mutations", "[grammar-conformance]",
    ),
    control(
        "I-REFUSE-ONLY-RECORDED", "implementation-safety", ["NC09"],
        "refuse an unsealed generation only when it recorded outstanding work",
        [(CLS, r'''        if matches!(
            report.class,
            FileClass::UnsealedAction | FileClass::MalformedJournal
        ) {
''', r'''        if report.class == FileClass::MalformedJournal
            || (report.class == FileClass::UnsealedAction && !report.recorded_unsettled.is_empty())
        {
'''), (CLS, r'''    report.refused = refused(class) || class == FileClass::SizeInvalid;
''', r'''    report.refused = (refused(class)
        && (class != FileClass::UnsealedAction || !report.recorded_unsettled.is_empty()))
        || class == FileClass::SizeInvalid;
''')],
        "k04_a_late_owner_without_an_acknowledged_record_still_blocks", "[no-false-resolution]",
    ),
    control(
        "I-VISIBLE-RUNENDED-RESOLVES", "implementation-safety", ["NC16"],
        "a visible RunEnded taken as resolution",
        [(CLS, r'''            SealParse::Unsealed | SealParse::Unreadable if summary.action_started => {
                FileClass::UnsealedAction
            }
''', r'''            SealParse::Unsealed | SealParse::Unreadable
                if summary.action_started && summary.run_ended.is_none() =>
            {
                FileClass::UnsealedAction
            }
''')],
        "c01_classes_and_the_conservative_refusal", "[no-false-resolution]",
        require=["a visible RunEnded is not resolution"],
    ),
    control(
        "I-LATE-NOTE-MISSING", "implementation-safety", [],
        "an unsealed generation's report omits that late owners may exist with no durable record",
        [(CLS, r'''        report.notes.push(NOTE_LATE);
''', "")],
        "c01_classes_and_the_conservative_refusal", "[late-uncertain]",
    ),
    control(
        "I-DECIDE-FROM-REPORT", "implementation-safety", ["NC24b"],
        "a startup decided from the truncated report",
        [(CLS, r'''    let current = current_incidents(level);
    if current.len() > incident_limit {
''', r'''    let current = bounded_report(level).detail;
    if current.len() > incident_limit {
''')],
        "c03_pool_level_checks_and_bounded_reports", "[aggregate-bounds]",
        require=["never from the report"],
    ),
    control(
        "I-NONDETERMINISTIC-REPORT", "implementation-safety", [],
        "a file's report depends on something other than its bytes (a call counter)",
        [(CLS, r'''fn finish_report(mut report: FileReport) -> FileReport {
    let class = report.class;
''', r'''fn finish_report(mut report: FileReport) -> FileReport {
    static CALLS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let call = CALLS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    report.reason.push_str(&format!(" (call {call})"));
    let class = report.class;
''')],
        "c02_classification_is_deterministic", "[archive-binding]",
        require=["determinism"],
    ),
    control(
        "I-VALIDATOR-INEXACT", "implementation-safety", [],
        "the disposition validator accepts a disposition that does not restate the incident's facts",
        [(DSP, r'''        && disposition.facts == incident.facts
''', r'''        && {
            let _ = incident;
            true
        }
''')],
        "f09_the_validator_accepts_only_an_exact_restatement", "[disposition-exact]",
        require=["another count"],
    ),
]

# ---------------------------------------------------------------------------
# Implementation safety: opening, activation, profile, storage, the scan
# ---------------------------------------------------------------------------

PROBE = r'''    io.touch(lock)
        .map_err(|error| Refused::Unreliable(format!("activation probe: {:?}", error.errno)))?;
    hooks.activation(ActivationPoint::ProbeFsync);
    io.fsync(lock)
        .map_err(|error| Refused::Unreliable(format!("activation probe: {:?}", error.errno)))?;
'''

OWNER_ACTIVATION = r'''    let admission = activate(
        io,
        &Activation {
            path,
            selection: &selection,
            dirs: &dirs,
            lock: &lock,
            opening,
            observed: &observed,
            pinning: Pinning::Pinned,
        },
        hooks,
    )
    .map_err(|refused| fail(refused, revision))?;
    drop(observed);
'''

OPENING = [
    control(
        "I-PRESERVATION-SKIPPED", "implementation-safety", ["NC02"],
        "startup reports what it read without the evidence-preservation sync",
        [(OPN, r'''        if mode == ScanMode::Owner && io.fdatasync(file).is_err() {
            return Err(Refused::Unreliable(name));
        }
''', "")],
        "o13_the_preservation_sync_makes_what_the_report_saw_durable", "[F1-F2-sync]",
    ),
    control(
        "I-SYNC-BEFORE-LOCK", "implementation-safety", ["NC06"],
        "startup syncs and reads journals before taking the store lock (the first candidate)",
        [(OPN, r'''        let lock = open_checked(io, &dirs.root, "LOCK", &lock_stat)
            .map_err(|refused| fail(refused, revision))?;
        hooks.before_lock(attempt);
''', r'''        let lock = open_checked(io, &dirs.root, "LOCK", &lock_stat)
            .map_err(|refused| fail(refused, revision))?;
        if let Ok(store_dirs) = open_store_dirs(io, &dirs.root, &selection.provision) {
            for index in 0..selection.provision.pool() {
                if let Ok(file) = io.open_read(&store_dirs.journals, &pool_name(index)) {
                    let _ = io.fdatasync(&file);
                    let mut block = [0u8; 16];
                    let _ = io.pread(&file, 0, &mut block);
                }
            }
        }
        hooks.before_lock(attempt);
''')],
        "o03_busy_before_any_sync_or_read", "[busy-before-sync]",
        require=["synced or read the store"],
    ),
    control(
        "I-NO-REVALIDATION", "implementation-safety", ["NC21a"],
        "no post-lock revalidation of the PROVISION selection",
        [(OPN, r'''    fresh().unwrap_or(false)
}
''', r'''    let _ = fresh;
    true
}
''')],
        "o04_a_replaced_selection_never_regains_authority", "[provision-selection]",
        require=["the new revision"],
    ),
    control(
        "I-OPEN-BEFORE-TYPE", "implementation-safety", ["NC14a"],
        "an untrusted pool entry is opened before its type is checked",
        [(OPN, r'''    for (index, inode) in provision.pool_inodes.iter().enumerate() {
        let name = pool_name(index as u32);
''', r'''    for (index, inode) in provision.pool_inodes.iter().enumerate() {
        let name = pool_name(index as u32);
        let _ = io.open_read(&dirs.journals, &name);
''')],
        "o05_safe_open_refuses_before_opening", "[safe-open]",
        require=["the entry was opened"],
    ),
    control(
        "I-PROVISION-INODE-IGNORED", "implementation-safety", ["NC12b"],
        "a pool file whose inode PROVISION does not record is accepted",
        [(OPN, r'''        if stat.ino != *inode {
            return Err(lost(format!("{name} replaced")));
        }
''', r'''        let _ = inode;
''')],
        "o05_safe_open_refuses_before_opening", "[safe-open]",
        require=["a replaced pool file"],
    ),
    control(
        "I-EXT4-MAGIC", "implementation-safety", ["NC14b"],
        "ext4 decided by the shared superblock magic alone: the mount's filesystem type is not checked",
        [(OPN, r'''    if record.fstype != "ext4" {
        return Err(unsupported(format!("filesystem {}", record.fstype)));
    }
''', "")],
        "o06_the_mount_is_identified_through_the_descriptor", "[safe-open]",
        require=["ext3"],
    ),
    control(
        "I-NO-ACTIVATION", "implementation-safety", ["NC-ACT-VISIBLE"],
        "a freshly revalidated, visible selection treated as activated: no directory syncs, no PROVISION sync, no probe",
        [(OPN, r'''    for which in ACTIVATION_DIRECTORIES {
        hooks.activation(ActivationPoint::BeforeSync(which));
''', r'''    for which in ACTIVATION_DIRECTORIES.into_iter().take(0) {
        hooks.activation(ActivationPoint::BeforeSync(which));
'''), (OPN, r'''    sync_with_retries(io, &provision, true).map_err(|error| {
        Refused::Unreliable(format!("activation sync of PROVISION: {:?}", error.errno))
    })?;
''', ""), (OPN, PROBE, r'''    hooks.activation(ActivationPoint::ProbeFsync);
''')],
        "o02_activation_precedes_every_decision_and_the_claim", "[durable-activation]",
        require=["the probe"],
    ),
    control(
        "I-ACTIVATION-AFTER-SCAN", "implementation-safety", ["NC-ACT-ORDER"],
        "the scan, the decision (and so the claim) before the activation",
        [(OPN, OWNER_ACTIVATION + r'''    hooks.before_scan();
''', r'''    hooks.before_scan();
'''), (OPN, r'''    let StoreDirs { journals, .. } = store_dirs;
    Ok(Opened {
''', OWNER_ACTIVATION + r'''    let StoreDirs { journals, .. } = store_dirs;
    Ok(Opened {
''')],
        "o02_activation_precedes_every_decision_and_the_claim", "[durable-activation]",
        require=["order"],
    ),
    control(
        "I-R2-PROTOCOL", "implementation-safety", ["NC-ACT-COMPOSE"],
        "the R2 protocol: no activation, and a root holding history not found after Unprovisioned (so a fresh one replaces it), run through the composed tests",
        [(OPN, r'''    for which in ACTIVATION_DIRECTORIES {
        hooks.activation(ActivationPoint::BeforeSync(which));
''', r'''    for which in ACTIVATION_DIRECTORIES.into_iter().take(0) {
        hooks.activation(ActivationPoint::BeforeSync(which));
'''), (OPN, r'''    sync_with_retries(io, &provision, true).map_err(|error| {
        Refused::Unreliable(format!("activation sync of PROVISION: {:?}", error.errno))
    })?;
''', ""), (OPN, PROBE, r'''    hooks.activation(ActivationPoint::ProbeFsync);
'''), (MNT, r'''                if buf[..got].iter().any(|byte| *byte != 0) {
                    history = true;
                    break;
                }
''', r'''                if buf[..got].iter().any(|byte| *byte != 0) {
                    break;
                }
''')],
        "x01_composed_recovery_keeps_every_acknowledged_generation", "[composed-recovery]",
    ),
    control(
        "I-ACTIVATION-ERROR-IGNORED", "implementation-safety", ["NC-ACT-ERROR"],
        "a failed or uncertain activation directory sync treated as success",
        [(OPN, r'''        sync_with_retries(io, &handle, false).map_err(|error| {
            Refused::Unreliable(format!(
                "activation sync of {}: {:?}",
                which.label(),
                error.errno
            ))
        })?;
''', r'''        let _ = sync_with_retries(io, &handle, false);
''')],
        "o11_activation_refuses_on_failure_and_certifies_late_errors", "[durable-activation]",
        require=["sync 0 failing"],
    ),
    control(
        "I-NO-PROBE", "implementation-safety", ["NC-ACT-PROBE"],
        "no certification probe: syncs that returned 0 after a silent commit failure are trusted",
        [(OPN, PROBE, r'''    hooks.activation(ActivationPoint::ProbeFsync);
''')],
        "o11_activation_refuses_on_failure_and_certifies_late_errors", "[activation-proof]",
        require=["a silent abort"],
    ),
    control(
        "I-PROBE-FIRST", "implementation-safety", ["NC-PROOF-ORDER"],
        "the probe runs before the directory syncs",
        [(OPN, r'''    // A3: the probe, after the last A2 sync.
    hooks.activation(ActivationPoint::Probe);
''' + PROBE, r'''    // A3 moved before A2.
'''), (OPN, r'''    // A2: each dependency directory, resolved afresh, checked, synced.
''', r'''    hooks.activation(ActivationPoint::Probe);
''' + PROBE + r'''    // A2: each dependency directory, resolved afresh, checked, synced.
''')],
        "o02_activation_precedes_every_decision_and_the_claim", "[durable-activation]",
        require=["order"],
    ),
    control(
        "I-NO-A4", "implementation-safety", ["NC-ACT-REVALIDATE"],
        "the selection and directory revalidation after the activation syncs (A4) omitted",
        [(OPN, r'''    hooks.activation(ActivationPoint::Revalidate);
    for which in ACTIVATION_DIRECTORIES {
        let (base, name, expected) =
            resolve_activation(io, path, selection, which).map_err(|_| {
                Refused::SelectionChanged(format!("after activation: {}", which.label()))
            })?;
        match io.stat_at(&base, &name) {
            Ok(stat) if stat.ino == expected => {}
            _ => {
                return Err(Refused::SelectionChanged(format!(
                    "after activation: {}",
                    which.label()
                )))
            }
        }
    }
    if !revalidate(io, path, selection, lock) {
        return Err(Refused::SelectionChanged("after activation".into()));
    }
''', r'''    hooks.activation(ActivationPoint::Revalidate);
''')],
        "o11_activation_refuses_on_failure_and_certifies_late_errors", "[provision-selection]",
        require=["a replacement racing activation"],
    ),
    control(
        "I-A2-IDENTITY", "implementation-safety", [],
        "an activation directory is synced without checking it is the selected one",
        [(OPN, r'''        if stat.ino != expected || stat.file_type != FileType::Directory {
            return Err(invalid(format!(
                "activation: {} is not the selected directory",
                which.label()
            )));
        }
''', "")],
        "o11_activation_refuses_on_failure_and_certifies_late_errors", "[durable-activation]",
        require=["identity"],
    ),
    control(
        "I-KERNEL-PREFIX", "implementation-safety", ["NC-ACT-UNQUALIFIED"],
        "the kernel accepted by its release prefix, not its exact build identity",
        [(OPN, r'''        if format!("{release} {version}") != provision.kernel {
''', r'''        let _ = &version;
        if !provision.kernel.starts_with(release.as_str()) {
''')],
        "o07_the_effective_profile_is_read_not_inferred", "[supported-profile]",
        require=["another kernel"],
    ),
    control(
        "I-EXTERNAL-JOURNAL", "implementation-safety", ["NC-PROF-JOURNAL"],
        "an external journal accepted: the journal's location never checked",
        [(OPN, r'''    let journal = format!("{name}-{JOURNAL_INODE}");
    if !io
        .jbd2_entry_exists(&journal)
        .map_err(|error| unsupported(format!("journal location: {error}")))?
    {
        return Err(unsupported("journal not internal"));
    }
''', "")],
        "o07_the_effective_profile_is_read_not_inferred", "[supported-profile]",
        require=["an external journal"],
    ),
    control(
        "I-EXCLUDED-MODE", "implementation-safety", ["NC-PROF-MODE"],
        "an excluded mode in the effective listing accepted",
        [(OPN, r'''        || EXCLUDED_OPTIONS
            .iter()
            .any(|option| listed.contains(option))
''', "")],
        "o07_the_effective_profile_is_read_not_inferred", "[supported-profile]",
    ),
    control(
        "I-PROFILE-FROM-PINNED", "implementation-safety", ["NC-ACT-UNQUALIFIED"],
        "the profile taken from the pinned mountinfo strings: the effective listing, journal and kernel never read",
        [(OPN, r'''        .to_string();
    let listing = io
        .ext4_options(&name, OPTIONS_LIMIT)
''', r'''        .to_string();
    if !name.is_empty() {
        return Ok(name);
    }
    let listing = io
        .ext4_options(&name, OPTIONS_LIMIT)
''')],
        "o07_the_effective_profile_is_read_not_inferred", "[supported-profile]",
    ),
    control(
        "I-NO-A4-PROFILE", "implementation-safety", ["NC-PROF-GUARD"],
        "the effective profile not checked again after the activation syncs (A4)",
        [(OPN, r'''    let (device, _) = check_mount(
        io,
        &selection.provision,
        &fresh_root,
        &[&fresh_provdir, &fresh_parent],
        pinning,
    )?;
''', r'''    let _ = (&fresh_provdir, &fresh_parent);
    let device = major_minor(
        io.stat_dir(&fresh_root)
            .map_err(|error| io_refusal(error, "state root"))?
            .dev,
    );
''')],
        "o08_a_profile_changed_during_activation_refuses_at_a4", "[supported-profile]",
        require=["A4"],
    ),
    control(
        "I-WRITE-CACHE-ALONE", "implementation-safety", ["NC-STORAGE-VOLATILE"],
        "a volatile-completion profile admitted as stable: queue/write_cache alone decides",
        [(OPN, r'''    let cache = (write_cache.as_slice(), fua.as_slice());
    if cache == (&b"write back\n"[..], &b"1\n"[..]) {
        return Err(unsupported("volatile write cache"));
    }
    if cache == (&b"write through\n"[..], &b"1\n"[..]) {
        return Err(unsupported(
            "volatile write cache, flushes disabled in the kernel's view only",
        ));
    }
    if cache != (ADMITTED_CACHE.0.as_bytes(), ADMITTED_CACHE.1.as_bytes()) {
        return Err(unsupported("contradictory storage information"));
    }
''', r'''    let _ = &fua;
    if write_cache != ADMITTED_CACHE.0.as_bytes() {
        return Err(unsupported("volatile write cache"));
    }
''')],
        "o09_storage_that_is_not_admitted_refuses_before_any_claim", "[storage-qualification]",
    ),
    control(
        "I-IDENTITY-UNCHECKED", "implementation-safety", ["NC-STORAGE-CLAIM"],
        "the storage identity PROVISION records is not compared: another device of the same kind is admitted",
        [(OPN, r'''            || observation.partition != provision.storage_partition
            || observation.identity != provision.storage_identity)
''', r'''            || observation.partition != provision.storage_partition)
''')],
        "o09_storage_that_is_not_admitted_refuses_before_any_claim", "[storage-qualification]",
    ),
    control(
        "I-ATTESTATION-ADMITS", "implementation-safety", ["NC-STORAGE-COMPOSED"],
        "storage admitted on the Owner's attestation (A-S5, R4's domain), whatever the device shows",
        [(OPN, r''') -> Result<StorageObservation, Refused> {
    let link = io
        .block_device_link(device.0, device.1)
''', r''') -> Result<StorageObservation, Refused> {
    if !provision.storage_attestation.is_empty() {
        return Ok(StorageObservation {
            pci_function: provision.storage_pci_function.clone(),
            partition: provision.storage_partition,
            identity: provision.storage_identity,
            controller_dir: String::new(),
            disk_dir: String::new(),
        });
    }
    let link = io
        .block_device_link(device.0, device.1)
''')],
        "x03_storage_events_composed", "[storage-composed]",
    ),
    control(
        "I-CLAIM-UNBOUNDED", "implementation-safety", ["NC17"],
        "the claim counter is not bounded at 2^63",
        [(OPN, r'''        .filter(|claim| *claim <= MAX_CLAIM)
''', "")],
        "c04_the_claim_counter_never_wraps", "[arith-bounds]",
    ),
    control(
        "I-TMP-DISPOSITION", "implementation-safety", ["NC12a"],
        "a disposition under a temporary name is read as a disposition",
        [(OPN, r'''    name == "revoked" || format::parse_disposition_name(name).is_some()
''', r'''    name == "revoked"
        || name.starts_with(TMP_PREFIX)
        || format::parse_disposition_name(name).is_some()
'''), (OPN, r'''        if *name != format::disposition_name(&disposition.binding) {
''', r'''        if !name.starts_with(TMP_PREFIX) && *name != format::disposition_name(&disposition.binding) {
''')],
        "m09_the_crash_matrix_is_the_documented_one", "[admin-crash]",
    ),
    control(
        "I-BOUND-AFTER-EXAMINE", "implementation-safety", ["NC24a"],
        "enumeration bounds checked only after the entries were examined",
        [(OPN, r'''    match io.list_dir(dir, limit) {
        Ok(Listing::Names(names)) => Ok(names),
''', r'''    match io.list_dir(dir, usize::MAX) {
        Ok(Listing::Names(names)) if names.len() > limit => {
            for name in &names {
                let _ = io.open_read(dir, name);
            }
            Err(invalid(format!("enumeration bound: {what}")))
        }
        Ok(Listing::Names(names)) => Ok(names),
''')],
        "m07_aggregate_bounds_refuse_before_any_entry_is_examined", "[aggregate-bounds]",
        require=["an entry was examined"],
    ),
]

# ---------------------------------------------------------------------------
# Implementation safety: the recorder, the exchange and the admission gate
# ---------------------------------------------------------------------------

RECORDER = [
    control(
        "I-DURABLE-BEFORE-SYNC", "implementation-safety", ["NC15a"],
        "a record is published durable without its sync",
        [(REC, r'''        if let Err(error) = sync_with_retries(&self.io, &self.file, true) {
            return self.fail(Cause::Sync(seq, error.errno));
        }
''', "")],
        "r01_acknowledged_implies_durable_under_write_and_sync_faults", "[ack-durable]",
        require=["acknowledged before it was durable"],
    ),
    control(
        "I-SYNC-RETRIED-AFTER-EIO", "implementation-safety", ["NC15b"],
        "a failed sync (EIO) is retried into success",
        [(IO, r'''            Err(IoError {
                errno: Errno::Intr, ..
            }) if attempts < RETRIES => attempts += 1,
''', r'''            Err(IoError {
                errno: Errno::Intr | Errno::Io,
                ..
            }) if attempts < RETRIES => attempts += 1,
''')],
        "r01_acknowledged_implies_durable_under_write_and_sync_faults", "[ack-durable]",
    ),
    control(
        "I-WORKER-IDENTITY", "implementation-safety", [],
        "the worker's recheck (W5) is skipped: a write to a replaced journal is published",
        [(REC, r'''    fn identity_ok(&self) -> bool {
''', r'''    fn identity_ok(&self) -> bool {
        if !self.name.is_empty() {
            return true;
        }
''')],
        "r10_a_replaced_journal_latches_before_publication", "[ack-durable]",
    ),
    control(
        "I-NO-GATE", "implementation-safety", ["NC18a"],
        "no store admission gate: admission continues after a fatal condition",
        [(EXC, r'''        let held = self.exchange.acquire();
        if let Some((cause, _)) = held.state.fatal {
            return Err(GateRefused::RecorderFatal { reservation, cause });
        }
''', r'''        let held = self.exchange.acquire();
''')],
        "r02_invalid_and_conflicting_submissions_latch_and_land_at_an_issued_record", "[admission-fence]",
    ),
    control(
        "I-FAILURE-AT-CAUSE", "implementation-safety", ["NC18b"],
        "R1 failure targeting: the failure is delivered at the cause's own sequence",
        [(EXC, r'''        let target = evidence.acknowledged + 1;
''', r'''        let target = match self.exchange.acquire().state.fatal {
            Some((Cause::Conflict(seq) | Cause::InvalidSubmission(seq, _), _)) => seq,
            _ => evidence.acknowledged + 1,
        };
''')],
        "r02_invalid_and_conflicting_submissions_latch_and_land_at_an_issued_record", "[fatal-total]",
    ),
    control(
        "I-FUTURE-SILENT", "implementation-safety", ["NC18c"],
        "a submission for an unissued future sequence is ignored silently",
        [(EXC, r'''    if seq != state.submitted_through + 1 {
        state.latch(Cause::InvalidSubmission(seq, Invalid::Future));
        return false;
    }
''', r'''    if seq != state.submitted_through + 1 {
        return false;
    }
''')],
        "r02_invalid_and_conflicting_submissions_latch_and_land_at_an_issued_record", "[fatal-total]",
        require=["a future sequence"],
    ),
    control(
        "I-PUBLISH-AFTER-LATCH", "implementation-safety", ["NC18d"],
        "the worker publishes durability after the fatal latch",
        [(REC, r'''            if !held.poisoned && held.state.fatal.is_none() && held.state.durable_through + 1 == seq
            {
''', r'''            if !held.poisoned && held.state.durable_through + 1 == seq {
''')],
        "r03_every_interleaving_of_gate_worker_apply_and_latch", "[admission-fence]",
        require=["published after the latch"],
    ),
    control(
        "I-UNEXPECTED-ACK-IGNORED", "implementation-safety", ["NC18e"],
        "an unexpected acknowledgement outcome is ignored",
        [(EXC, r'''                if outcome == AckOutcome::Acknowledged {
                    self.applied_through = seq;
                } else {
                    self.exchange.latch(Cause::UnexpectedAck(seq, outcome));
                    self.stopped = true;
                    break;
                }
''', r'''                let _ = outcome;
                self.applied_through = seq;
''')],
        "r05_in_flight_appends_and_unexpected_acknowledgement_outcomes", "[fatal-total]",
        require=["an unexpected outcome latches"],
    ),
    control(
        "I-FAIL-UNISSUED", "implementation-safety", ["NC18f"],
        "record_failed for a record not issued yet, counted as delivered",
        [(EXC, r'''        if target > evidence.issued {
            self.delivery = Delivery::Waiting;
            return;
        }
''', ""), (EXC, r'''        self.delivery = if outcome == AckOutcome::FailureRecorded {
            Delivery::Delivered { target }
        } else {
            Delivery::IntegrationFault { target, outcome }
        };
''', r'''        let _ = outcome;
        self.delivery = Delivery::Delivered { target };
''')],
        "r04_worker_loss_vanishing_and_poisoning", "[fatal-total]",
        require=["no record to fail yet"],
    ),
    control(
        "I-DELIVERY-DROPPED", "implementation-safety", ["NC08"],
        "the failure is reported as a message the core never applies",
        [(EXC, r'''        let outcome = custody.record_failed(id, now);
''', r'''        let _ = (id, now);
        let outcome = AckOutcome::FailureRecorded;
''')],
        "r02_invalid_and_conflicting_submissions_latch_and_land_at_an_issued_record", "[fatal-total]",
        require=["the core recorded the failure"],
    ),
    control(
        "I-DUP-CONFLICT", "implementation-safety", ["NC07"],
        "an identical resubmission is a conflict (submission not idempotent)",
        [(EXC, r'''            .is_some_and(|stored| stored.digest == intent.digest);
        if !same {
''', r'''            .is_some_and(|stored| stored.digest == intent.digest);
        if !same || seq > 0 {
''')],
        "r09_resubmission_is_idempotent_and_bounded", "[dup-bounded]",
        require=["a duplicate is not a conflict"],
    ),
    control(
        "I-DUP-RETAINED", "implementation-safety", ["NC07"],
        "an identical resubmission is stored and retained again (a growing queue)",
        [(EXC, r'''        if !same {
            state.latch(Cause::Conflict(seq));
        }
        return false;
''', r'''        if !same {
            state.latch(Cause::Conflict(seq));
            return false;
        }
        return true;
'''), (EXC, r'''            if self.recorder.retained.len() as u64 == seq - 1 {
                self.recorder.retained.push(intent.clone());
            }
''', r'''            self.recorder.retained.push(intent.clone());
''')],
        "r09_resubmission_is_idempotent_and_bounded", "[dup-bounded]",
        require=["one retained copy each"],
    ),
    control(
        "I-TIMEOUT-STOPS", "implementation-safety", [],
        "a bounded wait that times out is taken as the worker having stopped",
        [(EXC, r'''            held = self.exchange.wait_owner(held);
        }
        held.state.durable_through
''', r'''            held = self.exchange.wait_owner(held);
        }
        if held.state.durable_through < seq && held.state.fatal.is_none() {
            held.state.latch(Cause::WorkerVanished);
        }
        held.state.durable_through
''')],
        "r06_a_stall_is_reported_never_failed_and_nothing_waits_on_storage", "[fatal-latched]",
        require=["a timeout is never a failure"],
    ),
    control(
        "I-POISON-IGNORED", "implementation-safety", [],
        "a poisoned exchange mutex is used as if healthy",
        [(EXC, r'''            Err(poisoned) => {
                let mut state = poisoned.into_inner();
                state.note_poison();
                Held {
                    state,
                    poisoned: true,
                }
            }
''', r'''            Err(poisoned) => Held {
                state: poisoned.into_inner(),
                poisoned: false,
            },
''')],
        "r04_worker_loss_vanishing_and_poisoning", "[fatal-total]",
        require=["a poisoned mutex"],
    ),
    control(
        "I-VANISHED-IGNORED", "implementation-safety", [],
        "a worker thread that ended without a recorded exit is not noticed",
        [(EXC, r'''        if vanished {
            self.exchange.latch(Cause::WorkerVanished);
        }
''', r'''        let _ = vanished;
''')],
        "r04_worker_loss_vanishing_and_poisoning", "[fatal-total]",
        require=["a vanished worker"],
    ),
    control(
        "I-NO-DROP-GUARD", "implementation-safety", [],
        "a worker that unwinds latches nothing (its drop guard disarmed)",
        [(REC, r'''        if self.armed {
            self.exchange.latch(Cause::WorkerLost);
        }
''', r'''        let _ = self.armed;
''')],
        "r04_worker_loss_vanishing_and_poisoning", "[fatal-total]",
        require=["worker loss latched by its drop guard"],
    ),
    control(
        "I-SEAL-WHILE-LATCHED", "implementation-safety", [],
        "a seal is requested although a fatal cause is latched",
        [(EXC, r'''            if let Some((cause, _)) = held.state.fatal {
                return Err(SealWithheld::Latched(cause));
            }
            if held.state.claim != ClaimState::Claimed {
''', r'''            if held.state.claim != ClaimState::Claimed {
'''), (EXC, r'''            if let Some((cause, _)) = held.state.fatal {
                return Err(SealWithheld::Latched(cause));
            }
            held.state.seal = SealState::Requested(seal);
''', r'''            held.state.seal = SealState::Requested(seal);
''')],
        "r08_claim_and_seal_boundaries", "[fatal-total]",
        require=["no seal for a failed claim", "Latched(ClaimWrite)"],
    ),
]

# ---------------------------------------------------------------------------
# Implementation safety: maintenance sessions and procedures
# ---------------------------------------------------------------------------

MAINTENANCE = [
    control(
        "I-SESSION-NEW-LOCK", "implementation-safety", ["NC20a"],
        "in-session verification requests a new shared lock on the store lock (the R1 workflow)",
        [(MNT, r'''        let OpenedRoot { dirs, .. } =
            open_root(&self.io, &self.path, &self.selection, self.opening, pinning)
                .map_err(fail)?;
''', r'''        let OpenedRoot { dirs, .. } =
            open_root(&self.io, &self.path, &self.selection, self.opening, pinning)
                .map_err(fail)?;
        let fresh = super::open::open_lock(&self.io, &dirs.root, &self.selection.provision)
            .map_err(fail)?;
        if self.io.flock(&fresh, LockRequest::Shared).is_err() {
            return Err(fail(Refused::Busy));
        }
''')],
        "m01_sessions_verify_through_their_retained_lock", "[session-verify]",
    ),
    control(
        "I-SESSION-SHARED", "implementation-safety", ["NC20b"],
        "in-session verification converts the session's exclusive lock to shared",
        [(MNT, r'''        let lock = self
            .locks
            .get(state)
            .ok_or_else(|| fail(Refused::NotAuthorized))?;
''', r'''        let lock = self
            .locks
            .get(state)
            .ok_or_else(|| fail(Refused::NotAuthorized))?;
        let _ = self.io.flock(lock, LockRequest::Shared);
''')],
        "m01_sessions_verify_through_their_retained_lock", "[session-verify]",
    ),
    control(
        "I-SESSION-RELOCK", "implementation-safety", ["NC20c"],
        "the session releases its lock around verification and takes it again",
        [(MNT, r'''        let store_dirs =
            open_store_dirs(&self.io, &dirs.root, &self.selection.provision).map_err(fail)?;
        let held: BTreeMap<u32, &P::File> = self
''', r'''        let store_dirs =
            open_store_dirs(&self.io, &dirs.root, &self.selection.provision).map_err(fail)?;
        if let Some(old) = self.locks.remove(state) {
            drop(old);
            let relocked = super::open::open_lock(&self.io, &dirs.root, &self.selection.provision)
                .map_err(fail)?;
            if self.io.flock(&relocked, LockRequest::Exclusive).is_err() {
                return Err(fail(Refused::Busy));
            }
            self.locks.insert(state.to_string(), relocked);
        }
        let held: BTreeMap<u32, &P::File> = self
''')],
        "m01_sessions_verify_through_their_retained_lock", "[session-verify]",
    ),
    control(
        "I-BUSY-SUCCESS", "implementation-safety", ["NC20d"],
        "a standalone verification that found the store busy proceeds as a success",
        [(OPN, r'''        if io.flock(&lock, LockRequest::Shared).is_err() {
            return Err(fail(Refused::Busy, revision));
        }
''', r'''        let _ = io.flock(&lock, LockRequest::Shared);
''')],
        "m01_sessions_verify_through_their_retained_lock", "[session-verify]",
        require=["the standalone verifier is Busy"],
    ),
    control(
        "I-AUTHORITY-FLAG", "implementation-safety", ["NC20e"],
        "maintenance authority asserted by a flag instead of the retained lock",
        [(MNT, r'''    pub fn authorized(&self, state: &str) -> bool {
        if !self.active {
            return false;
        }
''', r'''    pub fn authorized(&self, state: &str) -> bool {
        if self.active && !state.is_empty() {
            return true;
        }
        if !self.active {
            return false;
        }
''')],
        "m01_sessions_verify_through_their_retained_lock", "[session-verify]",
        require=["authority only from the retained lock"],
    ),
    control(
        "I-VERIFY-CACHED", "implementation-safety", ["NC20f"],
        "in-session verification skipped: an earlier report reused",
        [(MNT, r'''    pub fn verify(&mut self, config: &Config) -> Result<SessionReport, Refusal> {
''', r'''    pub fn verify(&mut self, config: &Config) -> Result<SessionReport, Refusal> {
        thread_local! {
            static LAST: std::cell::RefCell<Option<SessionReport>> =
                const { std::cell::RefCell::new(None) };
        }
        if let Some(last) = LAST.with(|last| last.borrow().clone()) {
            return Ok(last);
        }
'''), (MNT, r'''        let outcome = self.verify_inner(config, &state);
''', r'''        let outcome = self.verify_inner(config, &state);
        if let Ok(report) = &outcome {
            LAST.with(|last| *last.borrow_mut() = Some(report.clone()));
        }
''')],
        "m01_sessions_verify_through_their_retained_lock", "[session-verify]",
        require=["verify-after scans afresh"],
    ),
    control(
        "I-RETIRE-NO-PRECONDITIONS", "implementation-safety", ["NC12c"],
        "retirement without its preconditions hides pool claims",
        [(MNT, r'''pub fn retirement_allowed(report: &SessionReport, through: u64) -> bool {
''', r'''pub fn retirement_allowed(report: &SessionReport, through: u64) -> bool {
    if through > 0 {
        return true;
    }
''')],
        "m04_archival_recycling_retirement_and_succession", "[admin-crash]",
        require=["retirement without its preconditions"],
    ),
    control(
        "I-RETIRE-FROM-REPORT", "implementation-safety", ["NC24c"],
        "retirement preconditions evaluated on the truncated report",
        [(MNT, r'''    report
        .scan
        .level
        .incidents
        .iter()
        .all(|(binding, incident)| {
''', r'''    report
        .scan
        .report
        .detail
        .iter()
        .all(|(binding, incident)| {
''')],
        "m10_retirement_is_decided_from_the_complete_set", "[aggregate-bounds]",
    ),
    control(
        "I-HIDDEN-STEP", "implementation-safety", ["NC23"],
        "an undocumented directory sync in recycling (a step the protocol does not list)",
        [(MNT, r'''    {
        let (io, path, tmp) = (Rc::clone(&io), journals_path.clone(), tmp);
        proc.add("13.8/3", move || {
''', r'''    {
        let (io, path) = (Rc::clone(&io), journals_path.clone());
        proc.add("13.8/3", move || sync_dir(&*io, &dir_ref(&*io, &path)?));
    }
    {
        let (io, path, tmp) = (Rc::clone(&io), journals_path.clone(), tmp);
        proc.add("13.8/3", move || {
''')],
        "m08_procedures_perform_exactly_the_documented_steps", "[step-parity]",
        require=["P-RECYCLE"],
    ),
    control(
        "I-FRESH-ROOT", "implementation-safety", [],
        "after Unprovisioned, a root holding history is not found, so a fresh root replaces it (the R2 protocol)",
        [(MNT, r'''                if buf[..got].iter().any(|byte| *byte != 0) {
                    history = true;
                    break;
                }
''', r'''                if buf[..got].iter().any(|byte| *byte != 0) {
                    break;
                }
''')],
        "m06_an_unprovisioned_root_with_history_is_republished", "[composed-recovery]",
    ),
]

# ---------------------------------------------------------------------------
# Native: the Linux primitives (exercised beneath CARGO_TARGET_TMPDIR only)
# ---------------------------------------------------------------------------

OPEN_READ_FLAGS = r'''                libc::O_RDONLY
                    | libc::O_NONBLOCK
                    | libc::O_NOCTTY
                    | libc::O_NOFOLLOW
                    | libc::O_CLOEXEC,
                0,
                "openat2 read",
'''

NATIVE = [
    control(
        "N-DOT-NAME", "implementation-safety", [],
        "a component name of . or .. is accepted",
        [(IO, r'''        if value.is_empty() || value == "." || value == ".." || value.contains('/') {
''', r'''        if value.is_empty() || value.contains('/') {
''')],
        "n01_native_opening_is_descriptor_relative_and_no_follow", "[safe-open]",
    ),
    control(
        "N-FOLLOW", "implementation-safety", [],
        "symbolic links are followed (O_NOFOLLOW and RESOLVE_NO_SYMLINKS both removed)",
        [(IO, r'''    const RESOLVE: u64 = libc::RESOLVE_BENEATH
        | libc::RESOLVE_NO_SYMLINKS
''', r'''    const RESOLVE: u64 = libc::RESOLVE_BENEATH
'''), (IO, OPEN_READ_FLAGS, OPEN_READ_FLAGS.replace("                    | libc::O_NOFOLLOW\n", ""))],
        "n01_native_opening_is_descriptor_relative_and_no_follow", "[safe-open]",
        require=["a link refuses"],
    ),
    control(
        "N-BLOCKING", "implementation-safety", [],
        "an entry is opened for reading without O_NONBLOCK (a FIFO open blocks)",
        [(IO, OPEN_READ_FLAGS, OPEN_READ_FLAGS.replace("                    | libc::O_NONBLOCK\n", ""))],
        "n03_native_fifo_opens_without_blocking", "[safe-open]",
        require=["the FIFO open blocked or failed"],
    ),
]

# ---------------------------------------------------------------------------
# Authority surface: authority the store must not have, caught by a source
# or type guard (not behaviour)
# ---------------------------------------------------------------------------

ENTRY = r'''    let _ = request;
    Err(IntegrationUnavailable::DeploymentNotAuthorized)
}
'''

AUTHORITY = [
    control(
        "A-ENTRY-CALLS", "authority-api-surface", [],
        "the real configured-store entry evaluates its request before refusing",
        [(OPN, ENTRY, r'''    let _ = request.provision_path.len();
    Err(IntegrationUnavailable::DeploymentNotAuthorized)
}
''')],
        "o01_the_real_store_entry_is_closed", "[real-entry-closed]",
        require=["it calls nothing"],
    ),
    control(
        "A-ENTRY-SWITCH", "authority-api-surface", [],
        "the real configured-store entry reads an environment switch",
        [(OPN, ENTRY, r'''    let _ = request;
    if std::env::var_os("NEXUS_P2_REAL_STORE").is_some() {
        return Err(IntegrationUnavailable::DeploymentNotAuthorized);
    }
    Err(IntegrationUnavailable::DeploymentNotAuthorized)
}
''')],
        "o01_the_real_store_entry_is_closed", "[real-entry-closed]",
        require=["a switch"],
    ),
    control(
        "A-ADMISSION-CLONE", "authority-api-surface", [],
        "a storage admission can be cloned (and so kept beyond its opening)",
        [(OPN, r'''#[derive(Debug)]
pub struct StorageAdmission {
''', r'''#[derive(Debug, Clone)]
pub struct StorageAdmission {
''')],
        "o10_a_storage_admission_is_bound_to_its_opening", "[storage-qualification]",
        require=["no Clone or Default"],
    ),
    control(
        "A-ADMISSION-FORGED", "authority-api-surface", [],
        "a storage admission constructed outside the storage check (a second constructor)",
        [(OPN, r'''impl StorageAdmission {
    pub fn opening(&self) -> OpeningId {
''', r'''pub fn forged_admission(opening: OpeningId, observation: StorageObservation) -> StorageAdmission {
    StorageAdmission {
        opening,
        provision_digest: [0; 32],
        observation,
    }
}

impl StorageAdmission {
    pub fn opening(&self) -> OpeningId {
''')],
        "o10_a_storage_admission_is_bound_to_its_opening", "[storage-qualification]",
        require=["one constructor"],
    ),
    control(
        "A-LOCK-UNLOCK", "authority-api-surface", ["NC05b"],
        "the store's I/O trait gains a lock release (a duplicate's LOCK_UN releases the original's lock)",
        [(IO, r'''    /// `LOCK_SH | LOCK_NB`.
    Shared,
}
''', r'''    /// `LOCK_SH | LOCK_NB`.
    Shared,
    /// `LOCK_UN`.
    Unlock,
}
'''), (IO, r'''                LockRequest::Shared => libc::LOCK_SH | libc::LOCK_NB,
''', r'''                LockRequest::Shared => libc::LOCK_SH | libc::LOCK_NB,
                LockRequest::Unlock => libc::LOCK_UN,
'''), (SIM, r'''                LockRequest::Shared => "flock-sh",
''', r'''                LockRequest::Shared => "flock-sh",
                LockRequest::Unlock => "flock-un",
'''), (SIM, r'''                LockRequest::Shared => LockMode::Shared,
''', r'''                LockRequest::Shared => LockMode::Shared,
                LockRequest::Unlock => LockMode::Shared,
''')],
        "a01_the_authority_surface_is_fixed_in_the_source", "[lock-retained]",
        require=["a lock request only takes a lock"],
    ),
    control(
        "A-WORKER-LOCK", "authority-api-surface", ["NC05a"],
        "the recorder worker holds a lock description (the first candidate's recorder thread)",
        [(REC, r'''pub(super) struct Worker<P: StoreIo> {
    io: P,
''', r'''pub(super) struct Worker<P: StoreIo> {
    io: P,
    lock: Option<P::File>,
'''), (REC, r'''        Worker {
            io,
''', r'''        Worker {
            io,
            lock: None,
''')],
        "a01_the_authority_surface_is_fixed_in_the_source", "[lock-retained]",
        require=["the worker holds a lock description"],
    ),
    control(
        "A-GATE-SPLIT", "authority-api-surface", ["NC19"],
        "the admission gate's health check is followed by an unprotected admission call",
        [(EXC, r'''        let admitted = custody.admit(reservation, now);
        drop(held);
''', r'''        drop(held);
        let admitted = custody.admit(reservation, now);
''')],
        "a01_the_authority_surface_is_fixed_in_the_source", "[admission-fence]",
    ),
    control(
        "A-SELECTION-KEPT", "authority-api-surface", ["NC21b"],
        "a selection keeps a descriptor from its read (through which revalidation could look)",
        [(OPN, r'''pub struct Selection {
    pub provision: Provision,
''', r'''pub struct Selection {
    pub kept: Option<std::os::fd::RawFd>,
    pub provision: Provision,
'''), (OPN, r'''    Ok(Selection {
        provision,
''', r'''    Ok(Selection {
        kept: None,
        provision,
''')],
        "a01_the_authority_surface_is_fixed_in_the_source", "[provision-selection]",
    ),
]

# ---------------------------------------------------------------------------
# R1: behavioural controls of the authority bindings P2-V1-R3B-I3-I1-R1 added
# (closure to seal, the opening's complete set, disposition provenance, the
# exchange's containment, the session's verification, the exclusion's life)
# ---------------------------------------------------------------------------

R1_BINDINGS = [
    control(
        "R1-REFUSED-CLOSE-SEALS", "authority-binding", [],
        "(B1) a refused closure still requests the seal, from the recorder's durable count",
        [(OWN, r'''            Err(custody) => Err(Box::new(StoreOwner {
                custody: *custody,
''', r'''            Err(custody) => Err(Box::new(StoreOwner {
                custody: {
                    let durable = recorder.status().durable_through;
                    let _ = recorder.request_seal(&header, durable, now.0);
                    *custody
                },
''')],
        "b01_a_refused_closure_requests_no_seal_and_keeps_its_owner", "[closure-seal]",
        require=["no seal requested"],
    ),
    control(
        "R1-START-FROM-REPORT", "authority-binding", [],
        "(B3) the start decides over the incidents the bounded report lists, not the complete set",
        [(OWN, r'''    let current = opened.current();
''', r'''    let listed: Vec<[u8; 32]> = opened
        .report()
        .report
        .detail
        .iter()
        .map(|(binding, _)| *binding)
        .collect();
    let current: Vec<([u8; 32], super::classify::Incident)> = opened
        .current()
        .into_iter()
        .filter(|(binding, _)| listed.contains(binding))
        .collect();
''')],
        "b04_a_truncated_report_hides_no_blocking_incident", "[complete-set]",
    ),
    control(
        "R1-VALIDATOR-UNCHECKED", "authority-binding", [],
        "(B5) the store's validator returns a validation for any disposition file, exact or not",
        [(DSP, r'''        exact_restatement(&self.root_id, bytes, incident, disposition)
            .then(|| ValidatedDisposition::new(*binding, disposition.reason))
''', r'''        let _ = (incident, exact_restatement);
        Some(ValidatedDisposition::new(*binding, disposition.reason))
''')],
        "b06_a_disposition_takes_effect_only_through_its_opening", "[disposition-provenance]",
        require=["another count"],
    ),
    control(
        "R1-APPLIED-REASON-INVENTED", "authority-binding", [],
        "(B5) the claim's applied reasons are not the verified disposition files' reasons",
        [(OPN, r'''                .map(|disposition| (*binding, disposition.reason))
''', r'''                .map(|_| (*binding, DispositionReason::Other))
''')],
        "b06_a_disposition_takes_effect_only_through_its_opening", "[disposition-provenance]",
        require=["the dispositioned store refused"],
    ),
    control(
        "R1-FAULT-ACKS-UNDURABLE", "authority-binding", [],
        "(B6) the fault interface acknowledges a record that is not durable",
        [(FLT, r'''    if seq == 0 || seq > durable {
''', r'''    if seq == 0 || durable == u64::MAX {
''')],
        "b07_status_copies_and_fault_operations_clear_and_manufacture_nothing",
        "[exchange-contained]",
        require=["no acknowledgement of a record that is not durable"],
    ),
    control(
        "R1-FAULT-INJECTS-NEXT", "authority-binding", [],
        "(B6) the fault interface submits a record the sink stores as the next one",
        [(FLT, r'''        if stores_as_new(&held, intent) {
''', r'''        if stores_as_new(&held, intent) && intent.id.seq() == 0 {
''')],
        "r02_invalid_and_conflicting_submissions_latch_and_land_at_an_issued_record",
        "[exchange-contained]",
        require=["the next valid record is not a fault"],
    ),
    control(
        "R1-LATCH-REPLACED", "authority-binding", [],
        "(B6) a later latch replaces the first cause (a latch is no longer monotonic)",
        [(EXC, r'''    pub(super) fn latch(&mut self, cause: Cause) {
        if self.fatal.is_none() {
''', r'''    pub(super) fn latch(&mut self, cause: Cause) {
        if self.fatal.is_none() || self.later_causes == 0 {
''')],
        "b07_status_copies_and_fault_operations_clear_and_manufacture_nothing",
        "[exchange-contained]",
        require=["the first cause stays"],
    ),
    control(
        "R1-VERIFICATION-REUSED", "authority-binding", [],
        "(session) one session verification authorizes more than one procedure",
        [(MNT, r'''        match self.verified.take() {
''', r'''        match self.verified.clone() {
''')],
        "b08_a_session_decides_only_from_its_own_current_verification", "[session-verify]",
        require=["one verification, one procedure"],
    ),
    control(
        "R1-VERIFICATION-NEVER-LAPSES", "authority-binding", [],
        "(session) a procedure step does not lapse the session's earlier verification",
        [(MNT, r'''            if let Some(mutations) = &step.session {
                mutations.set(mutations.get() + 1);
            }
''', r'''            let _ = &step.session;
''')],
        "b08_a_session_decides_only_from_its_own_current_verification", "[session-verify]",
        require=["older than the revocation"],
    ),
    control(
        "R1-GUARD-NOT-KEPT", "authority-binding", [],
        "(exclusion) the worker's thread does not keep the owner's guard: the exclusion ends with the owner, I/O in flight",
        [(OWN, r'''        let handle = spawn_worker(*worker, hooks, Arc::clone(&self.guard));
''', r'''        let handle = spawn_worker(*worker, hooks, ());
''')],
        "b09_exclusion_outlasts_the_owner_while_its_io_is_in_flight", "[closure-owner]",
        require=["exclusion while I/O is in flight"],
    ),
]

# ---------------------------------------------------------------------------
# Remapped: I3-I1 controls whose defect point has no behavioural call site
# left. Not run and not counted; their intention is carried by the API/type
# guards named (api_probes.py), reported apart.
# ---------------------------------------------------------------------------

REMAPPED = [
    control(
        "A-ADMISSION-UNBOUND", "authority-api-surface", [],
        "the claim accepts an admission from another opening of the same selection",
        [(OPN, r'''        admission.opening == opening && admission.provision_digest == *provision_digest
''', r'''        let _ = opening;
        admission.provision_digest == *provision_digest
''')],
        "o10_a_storage_admission_is_bound_to_its_opening", "[storage-qualification]",
        require=["an earlier opening"],
    ),
]
REMAPPED_TO = {
    "A-ADMISSION-UNBOUND": [
        "P-B3-CLAIM-CALL (the claim is internal; it takes no caller admission)",
        "P-B4-IDENTITY-EDIT (an opening's identity and selection cannot be rebound)",
        "P-B4-OPENING-MINT (no opening identity is minted outside the store)",
        "P-B4-ADMISSION-FORGE (no admission is built from parts)",
    ],
}

COUNTED = SIMULATOR + FORMATS + OPENING + RECORDER + MAINTENANCE + NATIVE + AUTHORITY + R1_BINDINGS

# ---------------------------------------------------------------------------
# The runner
# ---------------------------------------------------------------------------

ROOT = pathlib.Path()
LOG = pathlib.Path()
EXPECTED_STATUS = set()


def sha256(path: pathlib.Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def state() -> dict:
    return {path: sha256(ROOT / path) for path in FILES}


def status() -> set:
    out = subprocess.run(
        ["git", "status", "--porcelain=v1", "--untracked-files=all"],
        cwd=ROOT, capture_output=True, text=True, check=True,
    ).stdout
    return {line for line in out.splitlines() if line}


def mutate(control_, originals):
    """The mutated text of every file the control edits (anchors applied in
    order, each exactly once in the text as it stands)."""
    texts = {}
    for edit in control_["edits"]:
        text = texts.get(edit["file"], originals[edit["file"]].decode())
        count = text.count(edit["anchor"])
        if count != 1:
            raise SystemExit(
                f"{control_['id']}: anchor occurs {count} times in {edit['file']}: "
                f"{edit['anchor'][:80]!r}")
        texts[edit["file"]] = text.replace(edit["anchor"], edit["replacement"], 1)
    return texts


def run_test(test: str) -> subprocess.CompletedProcess:
    return subprocess.run(
        ["cargo", "test", "-p", "nexus-verifier-sandbox", "--locked",
         "--test", TARGET, "--", test, "--exact"],
        cwd=ROOT, capture_output=True, text=True, timeout=1800,
    )


def message_of(output: str, test: str) -> str:
    """The message of the last panic on the test's own thread: the assertion
    that ended the test. Earlier panics (a fixture worker's, or a sink panic
    the core contained) are not the intended assertion."""
    lines = output.splitlines()
    starts = [
        i for i, line in enumerate(lines)
        if line.startswith(f"thread '{test}'") and "panicked at" in line
    ]
    if not starts:
        return ""
    tail = []
    for line in lines[starts[-1] + 1:]:
        if (line.startswith("note: run with") or line.startswith("failures:")
                or line.startswith("thread '")):
            break
        tail.append(line)
    return "\n".join(tail).strip()


def run_one(control_, originals, original_state):
    texts = mutate(control_, originals)
    if state() != original_state:
        raise SystemExit(f"{control_['id']}: files changed before the control")
    try:
        for path, text in texts.items():
            (ROOT / path).write_bytes(text.encode())
        proc = run_test(control_["test"])
    finally:
        for path in texts:
            (ROOT / path).write_bytes(originals[path])
    restored = state() == original_state
    worktree = status()
    if worktree != EXPECTED_STATUS:
        raise SystemExit(f"{control_['id']}: unexpected worktree state after restoring: {worktree}")
    output = proc.stdout + proc.stderr
    test = control_["test"]
    compiled = (
        "could not compile" not in output
        and "error[E" not in output
        and f"Running tests/{TARGET}.rs" in output
    )
    failed = (
        proc.returncode != 0
        and "test result: FAILED. 0 passed; 1 failed" in output
        and f"{test} ... FAILED" in output
    )
    return output, compiled, failed, restored, message_of(output, test), proc.returncode


def main() -> int:
    global ROOT, LOG, EXPECTED_STATUS
    args = sys.argv[1:]
    if len(args) == 2 and args[1] in ("--check-anchors", "--print-sources"):
        ROOT = pathlib.Path(args[0]).resolve()
        if args[1] == "--print-sources":
            print(json.dumps(state(), indent=4))
            return 0
        originals = {path: (ROOT / path).read_bytes() for path in FILES}
        ids = [c["id"] for c in COUNTED]
        if len(ids) != len(set(ids)):
            raise SystemExit("duplicate control ids")
        for c in COUNTED:
            mutate(c, originals)
            if not c["marker"].startswith("[") or not c["test"]:
                raise SystemExit(f"{c['id']}: marker or test missing")
            touched = {e["file"] for e in c["edits"]}
            protected = {f"{CUSTODY}/core.rs", f"{CUSTODY}/model.rs", f"{CUSTODY}/codec.rs",
                         f"{CUSTODY}/mod.rs", f"{TESTS}/{TARGET}.rs"}
            if touched & protected:
                raise SystemExit(f"{c['id']}: mutates a protected or test file")
        print(json.dumps({"controls": len(COUNTED), "anchors": "ok"}))
        return 0
    only = None
    if "--only" in args:
        at = args.index("--only")
        only = set(args[at + 1].split(","))
        del args[at:at + 2]
    expected_file = None
    if "--expected-status" in args:
        at = args.index("--expected-status")
        expected_file = args[at + 1]
        del args[at:at + 2]
    if len(args) != 2:
        raise SystemExit(__doc__)
    ROOT = pathlib.Path(args[0]).resolve()
    LOG = pathlib.Path(args[1]).resolve()
    LOG.mkdir(parents=True, exist_ok=True)
    if expected_file:
        EXPECTED_STATUS = {
            line for line in pathlib.Path(expected_file).read_text().splitlines() if line
        }
    original_state = state()
    if CANDIDATE_SOURCES and original_state != CANDIDATE_SOURCES:
        changed = sorted(p for p in FILES if original_state.get(p) != CANDIDATE_SOURCES.get(p))
        raise SystemExit(f"the checkout's sources are not the candidate's: {changed}")
    originals = {path: (ROOT / path).read_bytes() for path in FILES}
    if status() != EXPECTED_STATUS:
        raise SystemExit(f"unexpected worktree state before the controls: {status()}")
    results = []
    for c in COUNTED:
        if only is not None and c["id"] not in only:
            continue
        output, compiled, failed, restored, message, code = run_one(c, originals, original_state)
        (LOG / f"{c['id']}.log").write_text(output)
        marked = c["marker"] in message
        required = all(item in message for item in c["require"])
        forbidden = any(item in message for item in c["forbid"])
        ok = compiled and failed and marked and required and not forbidden and restored
        result = dict(
            id=c["id"], category=c["category"], design=c["design"], what=c["what"],
            files=sorted({e["file"] for e in c["edits"]}), edits=len(c["edits"]),
            test=c["test"], marker=c["marker"], require=c["require"],
            compiled=compiled, failed_intended_test=failed, marker_in_assertion=marked,
            required_present=required, forbidden_present=forbidden,
            files_restored=restored, exit=code, ok=ok, assertion=message[:1500],
        )
        results.append(result)
        print(json.dumps(result), flush=True)
    final_state = state()
    by_category = {}
    for r in results:
        by_category.setdefault(r["category"], []).append(r["id"])
    summary = dict(
        original=original_state,
        final=final_state,
        identical=final_state == original_state,
        status=sorted(status()),
        expected_status=sorted(EXPECTED_STATUS),
        counted=len(results),
        by_category={k: len(v) for k, v in sorted(by_category.items())},
        all_required=all(r["ok"] for r in results),
        failures=[r["id"] for r in results if not r["ok"]],
    )
    print(json.dumps(summary), flush=True)
    (LOG / "summary.json").write_text(json.dumps(
        dict(summary=summary, counted=results), indent=2) + "\n")
    return 0 if summary["identical"] and summary["all_required"] else 1


if __name__ == "__main__":
    sys.exit(main())
