#!/usr/bin/env python3
"""RS-01 mutation audit, revision R4: one fixed script, one fixed campaign.

Reference : P3-C11-RS01-MUTATION-AUDIT-01 / R4 (answers REVIEW-R3: R3-01 to R3-04; R3 answered REVIEW-R2)
Candidate : e71b982b420a51fd131aeb50367aca7b7477a3e6
Tree      : 3e023d583914ee37b3bfa0089eb7ddf341e5db3c
Repository: Nexus-os2026/nexus-os, branch review/p3-c11-rs01-candidate

What it does
    It checks that the exported source is the candidate's tree, file by file
    and mode by mode. It checks the place it was started in: an ordinary user
    with no other group, no network but loopback, no other process in sight,
    no socket anywhere in the filesystem it can see, and nothing writable
    there but the audit's own folders. It shuts itself in (Landlock): it may
    write only its receipts, the scratch folder and the four source files it
    mutates. The commands it starts are shut in further: they can create and
    write files only in the scratch folder. (They run as the audit's own
    user, and can still change the mode or times of its files: a changed mode
    is seen, changed times are not.) It lets Cargo unpack the checked crate files once,
    then makes the dependency cache read-only for itself and for every later
    command. It runs the unchanged candidate five times.
    Then, for each planned mutant, in the fixed order: it makes the exact text
    change, builds, runs the RS-01 tests, reads the result by fixed rules,
    puts the file back and checks the whole source again. It writes each
    result as it goes. At the end it checks the evidence itself: the logs,
    the dependency cache and the source.

What it never does
    It never chooses: there is no option to run a part of the plan. It never
    counts a test failure as a kill unless the named test failed at the named
    assertion itself, in that test's own failure output. It never calls a missing or
    unclear result "survived". It never runs the ignored timing test (T10).
    It never credits a control row unless the evidence was checked at the end
    and found whole. It never touches a Git repository, the network, or
    anything outside the folders it is given. It awards nothing: see the end
    of summary.txt.

Modes
    plan       print the fixed plan. Needs nothing.
    selftest   check the runner's own rules on made-up output and made-up
               folders. Builds nothing.
    verify     --src S: S is the candidate's tree, and every mutant and every
               expected assertion is found in it. Builds nothing.
    run        --src S --out O --work W --cargo-home C --toolchain-bin B
               --launcher L: the campaign.

Use
    The launcher is the proposed workflow, rs01-mutation-audit.yml: this script
    refuses a place that was not prepared as that file prepares it.
    S  an export of the candidate (git archive <candidate> | tar -x -C S)
    O  a new or empty folder: the receipts. On the same filesystem as S.
    W  a new or empty folder: build output and scratch. On another filesystem
       than S and O, of a fixed size: what a build writes cannot use up the
       space the receipts need.
    C  the dependency cache, on W's filesystem or another of a fixed size,
       holding only registry/cache and registry/index as "cargo fetch --locked
       --target x86_64-unknown-linux-gnu" downloaded them. This script checks
       each crate file against Cargo.lock; Cargo unpacks them inside the
       campaign, once, before anything is built.
    B  the folder of the real cargo and rustc, release 1.94.0
    L  the launcher's own file. Its bytes are copied into the receipts.
    Keep this script outside /usr and outside S, O, W, C and the toolchain.
    The evidence is O/receipt. W is build output and scratch.

Words used for a result
    KILLED                    every failure named in advance happened, each at its assertion
    KILLED, PART NOT CONFIRMED
                              the same, but a named part of the test was not confirmed
    KILLED IN PART            some of them did
    EXPECTED TEST FAILED, ORACLE NOT MET
                              a named test failed, but not at its named assertion
    FAILED, AWAITING ATTRIBUTION
                              tests failed that were not named (or none was named)
    SURVIVED (15 rs01_ tests run, T10 not)
                              the whole inventory was reported, nothing failed, exit status 0
    BUILD ERROR               the compiler refused the crate. Never a kill.
    TIMEOUT, CAMPAIGN DEADLINE, LOG LIMIT, OUTPUT HELD OPEN, EXECUTION FAILURE,
    STORAGE EXHAUSTED, INCOMPLETE OUTPUT, CONFLICTING OUTPUT, STALE BUILD,
    INPUT CHANGED, NOT APPLIED, RUNNER ERROR, INTERRUPTED, QUARANTINED,
    BASELINE NOT CLEAN, BASELINE LEFT A PROCESS BEHIND, NOT_RUN
                              no result about the mutant. Never a kill, never a survivor.
    A verdict marked "cleanup" was read from complete output of a command that
    ended by itself, but a process it had started was still alive and was ended
    by the runner. It is a qualified observation, not a clean execution.

Exit status (chosen so that none but 0 is one a shell, sudo, unshare or timeout
gives; a wrong command line alone ends with Python's own 2)
    0   every planned case gave a result, and the evidence is VALID
    10  the evidence is VALID, but not every planned case gave a result
    20  refused to start
    30  the evidence is INVALID or NOT VERIFIED, or the final receipts could not
        be written: nothing is credited
    40  the runner itself failed, or one of its own checks did: nothing is credited

Standard library only. Python 3.8 or later. Linux with Landlock (ABI 3 or later).
"""

import argparse
import ctypes
import difflib
import errno
import hashlib
import json
import os
import platform
import re
import resource
import select
import shutil
import signal
import socket
import stat
import subprocess
import sys
import tempfile
import time

REFERENCE = "P3-C11-RS01-MUTATION-AUDIT-01 / R4"
CANDIDATE = "e71b982b420a51fd131aeb50367aca7b7477a3e6"
TREE = "3e023d583914ee37b3bfa0089eb7ddf341e5db3c"
RUSTC_RELEASE = "1.94.0"  # rust-toolchain.toml of the candidate, and hosted CI run 38168538703
HOST_TRIPLE = "x86_64-unknown-linux-gnu"

PACKAGE = "nexus-governed-control"
CRATE = "crates/nexus-governed-control/"
AUTH = CRATE + "src/authority/"
RUN = AUTH + "run.rs"
MOD = AUTH + "mod.rs"
APPROVAL = AUTH + "approval.rs"
TESTS = AUTH + "tests.rs"

# sha256 of the candidate's bytes. Verified by the First Architect in
# P3-C11-RS01-MSR01-REVIEW-01 (tests.rs) and P3-C11-RS01-MOBILE-SOURCE-REVIEW-01.
CLEAN = {
    RUN: "394d725b5386fdcd29ba4306de0810a92d73be3c011caeb2a7c905981d7d5ae6",
    MOD: "ae99b2f59794470c4899396861c4942ba5db2e2e338fb3aeab8712be6c217013",
    APPROVAL: "34f849b502782f0c6a046c9fd2d5ceb50bf497a54e52e591339630670db49202",
    TESTS: "6f1bbc815ba286ac0e005c323a22cb7e1777002db8fb7f89279be3870334f2fc",
}

# The two commands, and nothing else. The first only builds, so that a build
# failure is known as one. "--lib" names the one test target that holds every
# RS-01 test; "rs01_" selects them; T10 stays ignored.
BUILD = ["cargo", "test", "-p", PACKAGE, "--lib", "--offline", "--locked", "--no-run"]
TEST = ["cargo", "test", "-p", PACKAGE, "--lib", "--offline", "--locked", "rs01_"]
TARGET_LABEL = "unittests src/lib.rs"
# The preparation, run once before the first build: Cargo unpacks the crate
# files this script has checked against Cargo.lock. It compiles nothing and
# runs no build script. After it the dependency cache is read-only (B2).
PREPARE = ["cargo", "fetch", "--offline", "--locked", "--target", HOST_TRIPLE]

# Limits, fixed before any run.
BASELINE_REPEATS = 5
OVERALL_BUDGET_S = 18900  # 5 h 15 min for the whole campaign, the last checks included
FINAL_RESERVE_S = 600  # of that budget, kept for the last checks and the final receipts (B4)
CASE_CLOSE_S = 120  # in every case, kept for ending its processes, the source checks and the restore
FIRST_BUILD_TIMEOUT_S = 2700  # the first build also builds the dependencies
BUILD_TIMEOUT_S = 900
TEST_TIMEOUT_S = 600
PREPARE_TIMEOUT_S = 600
TOOL_TIMEOUT_S = 60
QUIESCE_S = 30  # how long a case may take, once its command has ended, to leave no process behind
CASE_RESERVE_S = BUILD_TIMEOUT_S + TEST_TIMEOUT_S + CASE_CLOSE_S
LOG_LIMIT_BYTES = 32 * 1024 * 1024  # each log
# Storage (B4). What a build writes (W and the dependency cache) is on a
# filesystem of a fixed size, apart from the source and the receipts.
BUILD_FS_MAX_BYTES = 32 * 1024 ** 3  # such a filesystem may be no larger than this
MIN_FREE_START_BYTES = 8 * 1024 ** 3  # free for build output at the start
MIN_FREE_CASE_BYTES = 2 * 1024 ** 3  # free for build output before each case
EXHAUSTED_BYTES = 256 * 1024 ** 2  # less than this left after a step: no result, and the campaign stops
RECEIPT_MARGIN_BYTES = 512 * 1024 ** 2  # for the receipts, beyond the largest logs every case could write

# What the launcher prepares, and this script requires (B3). The first three
# are on the host, outside the view this script is started in: from inside,
# each must be out of reach. The fourth is inside the view, belongs to the
# audit's own user, and lies outside every audit folder: its mode and times
# must be beyond that user's reach all the same.
HOST_CANARY_SOCKET = "/run/rs01-audit-canary/socket"
HOST_CANARY_ABSTRACT = "rs01-audit-canary"
HOST_CANARY_FILE = "/var/tmp/rs01-audit-canary/file"
VIEW_CANARY_FILE = "/rs01-audit-view-canary"

# The dependency cache (B2). Before the preparation it may hold these and
# nothing else. Cargo's own lock and bookkeeping files may be written by the
# preparation only; after it nothing in the cache may change at all.
CARGO_BOOKKEEPING = (".package-cache", ".package-cache-mutate", ".global-cache")
CARGO_ROOT = ("registry",) + CARGO_BOOKKEEPING
CARGO_REGISTRY = ("CACHEDIR.TAG", "cache", "index", "src")

# The inventory: every test the filter "rs01_" selects in the lib target of
# the unchanged candidate, by its full name. Fifteen run; T10 is ignored.
_T = "authority::tests::"
_S = _T + "stop_measurement::"
INVENTORY = {
    "T1": _T + "rs01_t1_a_stop_during_the_resume_dialog_is_not_lifted",
    "T2": _T + "rs01_t2_a_stop_while_the_resumed_record_is_written_is_not_lifted",
    "T3": _T + "rs01_t3_quitting_while_the_resumed_record_is_written_lifts_nothing",
    "T3b": _T + "rs01_t3b_quitting_during_the_resume_dialog_writes_no_record",
    "T4": _T + "rs01_t4_a_resume_after_quitting_asks_nobody",
    "T4b": _T + "rs01_t4b_a_resume_after_quitting_is_refused_also_when_not_stopped",
    "T5": _T + "rs01_t5_an_exhausted_stop_number_never_wraps_and_stays_stopped",
    "T5b": _T + "rs01_t5b_exhaustion_during_the_dialog_refuses_the_resume",
    "T5c": _T + "rs01_t5c_exhaustion_while_the_record_is_written_refuses_the_resume",
    "T6": _T + "rs01_t6_each_stop_says_whether_the_one_before_it_was_lifted",
    "T7": _T + "rs01_t7_a_second_resume_of_the_same_stop_is_told_the_truth",
    "T8": _T + "rs01_t8_the_stop_and_the_compare_and_clear_each_hold_the_runs_lock_once",
    "T10": _S + "rs01_t10_the_registry_stop_seen_from_outside_its_lock",
    "T10a": _S + "rs01_t10a_the_receipt_claims_only_what_the_controller_received",
    "T10b": _S + "rs01_t10b_the_controller_accepts_nothing_after_its_deadline",
    "T11": _T + "rs01_t11_a_poisoned_runs_lock_lifts_nothing",
}
IGNORED = "T10"
SHORT = {full: short for short, full in INVENTORY.items()}

# ---------------------------------------------------------------------------
# Text of the candidate that the mutants refer to. Exact bytes.
# ---------------------------------------------------------------------------
A_STOP = "    pub(crate) fn stop_all(&self) -> Stop {\n"
A_READ = "    pub(crate) fn stop_to_lift(&self) -> Result<Option<StopToLift>, Refusal> {\n"
A_CHECK = "    pub(crate) fn check_lift(&self, number: u64) -> Result<Lift, Refusal> {\n"
A_LIFT = "    pub(crate) fn lift(&self, number: u64) -> Result<Lift, Refusal> {\n"
A_CLOSE = "    pub(crate) fn close(&self) {\n"
A_PROBE = "    pub(crate) fn probe_locks(&self) {\n"
A_RESUME = (
    "    pub(crate) fn resume(&self, confirmer: &dyn ControlConfirmer)"
    " -> Result<(), AuthorityError> {\n"
)
A_ESTOP = "    pub fn emergency_stop(&self) -> usize {\n"
A_REFUSED = "fn refused(refusal: Refusal) -> AuthorityError {\n"
A_CONTROL = "    fn control(\n"
A_PERCENTILE = "    fn percentile(sorted: &[Nanos], p: usize) -> Nanos {\n"
A_SUMMARY = "    fn summary(values: impl Iterator<Item = Value>) -> Summary {\n"
A_STATISTICS = "    fn statistics(samples: &[Sample]) -> Option<Statistics> {\n"
A_NUMBERS = "    fn numbers(r: &Received) -> Sample {\n"
A_EXPLANATION = "    fn explanation(outcome: &Outcome, sample: &Sample) -> Option<String> {\n"
A_UNTOUCHED = "fn untouched(f: &Scripted, stopped: bool, count: usize, next: Option<&str>) {\n"

END_METHOD = "\n    }\n"  # the closing brace of a function indented by four spaces
END_FREE = "\n}\n"

LOCK = (
    "        let Ok(_runs) = self.runs.lock() else {\n"
    "            return Err(Refusal::Unavailable);\n"
    "        };\n"
)
LOCK_THROUGH_POISON = (
    "        let _runs = self.runs.lock().unwrap_or_else(PoisonError::into_inner);\n"
)
CLOSED = (
    "        if self.closed.load(Ordering::SeqCst) {\n"
    "            return Err(Refusal::TooLate);\n"
    "        }\n"
)
EXHAUSTED = (
    "        if self.exhausted.load(Ordering::SeqCst) {\n"
    "            return Err(Refusal::Exhausted);\n"
    "        }\n"
)
NUMBER = (
    "        if self.stop_number.load(Ordering::SeqCst) != number {\n"
    "            return Err(Refusal::Superseded);\n"
    "        }\n"
)
ALREADY = (
    "        if !self.stopped.load(Ordering::SeqCst) {\n"
    "            return Ok(Lift::AlreadyLifted);\n"
    "        }\n"
)
STOP_LOCK = (
    "        let mut runs = self.runs.lock().unwrap_or_else(PoisonError::into_inner);\n"
)
SET_STOPPED = "        self.stopped.store(true, Ordering::SeqCst);\n"
CLEAR_STOPPED = "        self.stopped.store(false, Ordering::SeqCst);\n"
UNAVAILABLE = "            return Err(Refusal::Unavailable);\n"
ZERO_COUNT = "            self.cancelled_by_stop.store(0, Ordering::SeqCst);\n"
CHANGE_LAST = (
    "            self.last_lifted.store(self.stop_number.load(Ordering::SeqCst), Ordering::SeqCst);\n"
)
CLEAR_POISON = "            self.runs.clear_poison();\n"
SWEEP = "        for entry in runs.values_mut() {\n"
SWEEP_TEMPORARY = (
    "        for entry in self.runs.lock().unwrap_or_else(PoisonError::into_inner).values_mut() {\n"
)
EARLY_CHECK = (
    "        if self.runs.check_lift(request.stop_number).map_err(refused)?"
    " == Lift::AlreadyLifted {\n"
    "            // Another resume lifted this stop, and recorded it.\n"
    "            return Ok(());\n"
    "        }\n"
)
RECORD_THEN_LIFT = (
    "        // Evidence first: an unrecorded resume does not happen.\n"
    "        self.record(&record)?;\n"
    "        self.runs.lift(request.stop_number).map_err(refused)?;\n"
)
LATE_TURN = (
    "            if now > deadline {\n"
    "                // Nothing first seen in this turn is on time: the sample\n"
    "                // ends as it stood before it, and what was seen is late.\n"
    "                give_up(world, &mut received, now, seen);\n"
    "                return received;\n"
    "            }\n"
)
OVERLAP_TEST = "                    if call.end >= start && call.start <= ret {\n"
SUMMARY_GUARD = "        if !available.is_empty() {\n"
SEEN_FAULT = (
    "        if matches!((start, seen), (Some(start), Some(seen)) if seen < start) {\n"
    "            sample\n"
    "                .results\n"
    "                .push(Outcome::HarnessFault(\"seen before start\"));\n"
    "        }\n"
)
RETURN_FAULT = (
    "        if matches!((start, ret), (Some(start), Some(ret)) if ret < start) {\n"
    "            sample\n"
    "                .results\n"
    "                .push(Outcome::HarnessFault(\"return before start\"));\n"
    "        }\n"
)

# ---------------------------------------------------------------------------
# The assertions a kill must come from. Each is the first lines of one
# statement of tests.rs, whole lines, exact bytes. A panic is at "the named
# assertion" when its place is in tests.rs, on those lines, as the file was
# compiled (so a mutant of tests.rs moves them, and the script follows).
# ---------------------------------------------------------------------------
NEWER_STOP_LIFTED = (
    "    assert!(\n"
    "        f.auth.runs().is_stopped(),\n"
    "        \"the newer stop was lifted: {resumed:?}\"\n"
    "    );"
)
NO_RESUMED_RECORD = (
    "    assert!(records_of(f.evidence.records(), EvidencePhase::Resumed).is_empty());"
)
T1_SHOWN = "    assert_eq!(dialog.shown()[0].stop_number, 1);"
T1_DIALOG_TEXT = (
    "    assert!(\n"
    "        dialog.shown()[0]\n"
    "            .text()\n"
    "            .header\n"
    "            .contains(&(\"Lifts\", \"emergency stop 1\".to_string())),"
)
COUNT_STAYS = "    assert_eq!(f.auth.runs().cancelled_by_stop(), 1);"
T2_RECORD_NUMBER = "    assert_eq!(detail(&resumed_records[0], \"stop_number\"), Some(\"1\"));"
T3_STOPPED = (
    "    assert!(\n"
    "        f.auth.runs().is_stopped(),\n"
    "        \"a resume lifted the stop after quitting began: {resumed:?}\"\n"
    "    );"
)
T4_ASKED = (
    "    assert_eq!(\n"
    "        dialog.asked(),\n"
    "        0,\n"
    "        \"the owner was asked after quitting began: {resumed:?}\"\n"
    "    );"
)
T4B_REFUSED = (
    "    assert!(\n"
    "        resumed.is_err(),\n"
    "        \"a resume succeeded after quitting began: {resumed:?}\"\n"
    "    );"
)
T5_FIRST_RECORD = (
    "    assert_eq!(\n"
    "        stops_of(f.evidence.records()),\n"
    "        [format!(\"{max}/yes/exhausted yes\")]\n"
    "    );"
)
RESUME_REFUSED_EXHAUSTED = (
    "    assert_eq!(\n"
    "        f.auth.resume(&dialog),\n"
    "        Err(AuthorityError::EmergencyStopped)\n"
    "    );"
)
T5_ASKED = "    assert_eq!(dialog.asked(), 0);"
T5C_REFUSED = (
    "    assert_eq!(\n"
    "        resuming.join().unwrap(),\n"
    "        Err(AuthorityError::EmergencyStopped)\n"
    "    );"
)
T6_COUNT_ZERO = "    assert_eq!(f.auth.runs().cancelled_by_stop(), 0);"
T6_STOPS = "    assert_eq!(stops_of(f.evidence.records()), [\"1/none\", \"2/no\", \"3/yes\"]);"
T7_A = "    assert_eq!(b.join().unwrap(), Ok(()));"
T7_B = "    assert_eq!(registry.lift(1), Ok(Lift::AlreadyLifted));"
T7_C_OK = "    assert_eq!(f.auth.resume(&b), Ok(()));"
T7_C_ONE_RECORD = (
    "    assert_eq!(\n"
    "        records_of(f.evidence.records(), EvidencePhase::Resumed).len(),\n"
    "        1\n"
    "    );"
)
T8_FINAL = "    assert!(failures.is_empty(), \"{failures:#?}\");"
T11_A = "        assert_eq!(f.auth.resume(&dialog), Err(POISONED), \"(a) {before:?}\");"
T11_A_ASKED = "        assert_eq!(dialog.asked(), 0);"
T11_B = "        assert_eq!(f.auth.resume(&dialog), Err(POISONED), \"(b) {before:?}\");"
T11_B_NO_RECORD = (
    "        assert!(records_of(f.sink.memory.records(), EvidencePhase::Resumed).is_empty());"
)
T11_C = "        assert_eq!(resuming.join().unwrap(), Err(POISONED), \"(c) {before:?}\");"
T11_E = "    assert_eq!(f.auth.resume(&dialog), Err(POISONED));"
T11_STOP_AFTER_POISON = "        let cancelled = f.auth.emergency_stop();"
T11_UNTOUCHED = "        untouched(&f, true, 3, next);"  # three times: parts (a), (b), (c)
UNTOUCHED_COUNT = "    assert_eq!(f.auth.runs().cancelled_by_stop(), count);"
UNTOUCHED_AGAIN = "    assert_eq!(f.auth.resume(&again), Err(POISONED));"
UNTOUCHED_NEXT = (
    "        assert_eq!(stops.last().map(String::as_str), Some(next), \"{stops:?}\");"
)
T10A_FIRST = "        assert_eq!(four(&s), (Of(400), Of(800), Of(400), Of(400)));"
T10A_LATE_OBSERVER = "        assert_eq!(four(&s), (Of(1_000), Of(800), Of(800), Unavailable));"
T10A_SEEN_FAULT = "        assert_eq!(s.results, [HarnessFault(\"seen before start\")]);"
T10A_BOTH_FAULTS = (
    "        assert_eq!(\n"
    "            s.results,\n"
    "            [\n"
    "                HarnessFault(\"return before start\"),"
)
T10A_READINESS = "        assert_eq!(s.results, [ReadinessNotReported(vec![\"observer\"])]);"
T10A_OVERLAPS = "        assert_eq!(\n            overlaps,"
T10A_MISSING_TEXT = "        assert_eq!(sample_text(&missing, &s), PARTICIPANT_MISSING);"
T10A_LATE_CHANGES_NOTHING = "        assert_eq!(numbers(&late), s);"  # twice: caller, stop
T10A_STOP_RESULTS = (
    "        assert_eq!(\n"
    "            s.results,\n"
    "            [\n"
    "                StopReturnNotObserved,"
)
T10A_NO_NUMBER = (
    "        assert_eq!(\n"
    "            four(&s),\n"
    "            (Unavailable, Unavailable, Unavailable, Unavailable)\n"
    "        );"
)  # three times; the third is the stop's report missing
T10A_STOP_TEXT = "        assert_eq!(sample_text(&stop_missing, &s), STOP_MISSING);"
T10A_NO_STATISTICS = (
    "        assert!(statistics(&[complete.clone(), numbers(&stop_missing)]).is_none());"
)
T10A_99 = "        assert_eq!(\n            statistics(&totals(99))"
T10A_EMPTY_GROUPS = "            assert_eq!(one.of(group, Number::Total), Summary::default());"
T10A_MIXED = "        assert_eq!(\n            statistics(&mixed)"
T10B_FINAL = (
    "        assert!(\n"
    "            failed.is_empty(),\n"
    "            \"T10b: these schedules failed: {}\",\n"
    "            failed.join(\", \")\n"
    "        );"
)

W_UNTOUCHED = (TESTS, A_UNTOUCHED, END_FREE)
W_PERCENTILE = (TESTS, A_PERCENTILE, END_METHOD)
W_STOP = (RUN, A_STOP, END_METHOD)


def K(test, snippet, text=(), nth=1, where=None, how="assertion", via=None, says=""):
    """One expected failure: the named test fails at this place.

    snippet  whole lines of the statement the test's own thread must panic at:
             the panic that ended the test must be at the first line and at the
             column where the statement begins, not inside it (a fault: any
             column of its line)
    nth      which occurrence of those lines in the region (from 1)
    where    None: the named test's own function; else (file, anchor, end)
    text     every one of these must be in that panic's own message
    how      assertion | source-guard (T8's text check) | fault (a panic the
             contract's row itself names; not an assertion of the test)
    via      (lines, nth) in the named test: the statement the failure must have
             come through, read from the panic's own backtrace. Where it is
             named and not confirmed, the verdict is KILLED, PART NOT CONFIRMED.
    says     what the failure means, in words
    """
    if isinstance(text, str):
        text = (text,)
    return {"test": test, "snippet": snippet, "nth": nth, "where": where, "text": list(text),
            "how": how, "via": via, "says": says}


def T8(*criteria):
    return K("T8", T8_FINAL, text=criteria, how="source-guard",
             says="T8 reports " + ", ".join(criteria))


def T10B(schedules):
    return K("T10b", T10B_FINAL, text="T10b: these schedules failed: %s\n" % schedules,
             says="exactly the schedules %s fail" % schedules)


def M(ident, kind, row, what, file, anchor, edits, expect=(), end=END_METHOD, note="",
      hist="", origin="R1", build_error="", uncredited=None):
    return {
        "id": ident,
        # sanity | control (a contract row) | historical-form | probe | harness
        # | exploratory | reviewer
        "kind": kind,
        "row": row,  # the contract's control row (1 to 40), or 0
        "what": what,
        "file": file,
        "anchor": anchor,  # None: the edit's old text is unique in the whole file
        "end": end,
        "edits": [list(edit) for edit in edits],
        "expect": list(expect),  # no entry: no result is claimed in advance
        "build_error": build_error,  # text of the compiler error this form is expected to give
        "uncredited": dict(uncredited or {}),  # test -> why its failure earns nothing here
        "hist": hist,  # the same row's entry in the implementer's earlier campaign
        "origin": origin,  # R1: the draft of record | reviewer: R01-R08 | R2: added here
        "note": note,
    }


WAIT_NOT_EVIDENCE = (
    "T2 is expected to fail here on its sink's bounded wait (scripted.rs): ruled by the First "
    "Architect not to be stop-number evidence. It earns nothing."
)

MUTANTS = [
    # ---- sanity: the audit must be able to see and attribute a failure ------
    M("S1", "sanity", 0, "stop_all never sets the stopped flag",
      RUN, A_STOP, [(SET_STOPPED, "")],
      [K("T1", NEWER_STOP_LIFTED, "the newer stop was lifted:")],
      note="The gate: unless this is KILLED the campaign stops and every other case is NOT_RUN."),

    # ---- rows 1 to 15 ---------------------------------------------------------
    M("C01", "control", 1, "lift: the clear made unconditional",
      RUN, A_LIFT, [(CLOSED + EXHAUSTED + NUMBER + ALREADY, "")],
      [K("T2", NEWER_STOP_LIFTED, "the newer stop was lifted:"),
       K("T3", T3_STOPPED, "a resume lifted the stop after quitting began:"),
       K("T5c", T5C_REFUSED)], hist="C01"),
    M("C02", "control", 2, "lift: the test of closed removed",
      RUN, A_LIFT, [(CLOSED, "")],
      [K("T3", T3_STOPPED, "a resume lifted the stop after quitting began:")], hist="C02"),
    M("C03", "control", 3, "lift: the test of exhausted removed",
      RUN, A_LIFT, [(EXHAUSTED, "")], [K("T5c", T5C_REFUSED)], hist="C03"),
    M("C04", "control", 4, "lift: the test of the stop number removed",
      RUN, A_LIFT, [(NUMBER, "")],
      [K("T2", NEWER_STOP_LIFTED, "the newer stop was lifted:")], hist="C04"),
    M("C05", "control", 5, "stop_to_lift: the test of closed removed",
      RUN, A_READ, [(CLOSED, "")],
      [K("T4", T4_ASKED, "the owner was asked after quitting began:"),
       K("T4b", T4B_REFUSED, "a resume succeeded after quitting began:")], hist="C05"),
    M("C06", "control", 6, "stop_to_lift: the test of exhausted removed",
      RUN, A_READ, [(EXHAUSTED, "")],
      [K("T5", T5_ASKED, says="the confirmer is asked")], hist="C06"),
    M("C07", "control", 7, "resume: the early check removed",
      MOD, A_RESUME, [(EARLY_CHECK, "")],
      [K("T1", NO_RESUMED_RECORD, says="a Resumed record appears"),
       K("T3b", NO_RESUMED_RECORD, says="a Resumed record appears"),
       K("T5b", NO_RESUMED_RECORD, says="a Resumed record appears")], hist="C07"),
    M("C08a", "control", 8, "lift: the count set to zero on a superseded refusal",
      RUN, A_LIFT,
      [("            return Err(Refusal::Superseded);\n",
        ZERO_COUNT + "            return Err(Refusal::Superseded);\n")],
      [K("T2", COUNT_STAYS)], hist="C08"),
    M("C08b", "control", 8, "lift: the count set to zero on a too-late refusal",
      RUN, A_LIFT,
      [("            return Err(Refusal::TooLate);\n",
        ZERO_COUNT + "            return Err(Refusal::TooLate);\n")],
      [K("T3", COUNT_STAYS)], hist="C08"),
    M("C09", "control", 9, "stop_all: the stop number not advanced",
      RUN, A_STOP, [("                self.stop_number.store(next, Ordering::SeqCst);\n", "")],
      [K("T1", NEWER_STOP_LIFTED, "the newer stop was lifted:")], hist="C09"),
    M("C10", "control", 10, "stop_all: the stop number wraps at its maximum",
      RUN, A_STOP,
      [("match current.checked_add(1) {", "match Some(current.wrapping_add(1)) {")],
      [K("T5", T5_FIRST_RECORD)], hist="C10"),
    M("C11", "control", 11, "stop_all: previous-stop-lifted never says yes",
      RUN, A_STOP,
      [("            PreviousStop::Lifted\n        };\n", "            PreviousStop::NotLifted\n        };\n")],
      [K("T6", T6_STOPS)], hist="C11"),
    M("C12a", "control", 12, "lift: a stop already lifted answered as lifted now",
      RUN, A_LIFT,
      [("            return Ok(Lift::AlreadyLifted);\n", "            return Ok(Lift::Lifted);\n")],
      [K("T7", T7_B, says="part (b)")], hist="C12-a"),
    M("C12b", "control", 12, "lift: a stop already lifted answered as a refusal",
      RUN, A_LIFT,
      [("            return Ok(Lift::AlreadyLifted);\n",
        "            return Err(Refusal::Superseded);\n")],
      [K("T7", T7_A, says="part (a)")], hist="C12-b"),
    M("C13a", "control", 13, "resume: the confirmation built with a wrong stop number (one more)",
      MOD, A_RESUME,
      [("            stop_number: stop.number,\n", "            stop_number: stop.number.wrapping_add(1),\n")],
      [K("T1", NEWER_STOP_LIFTED, "the newer stop was lifted:")],
      uncredited={"T2": WAIT_NOT_EVIDENCE},
      note="Row 13, paired reading ruled by the First Architect: the confirmation is judged by T1."),
    M("C13b", "control", 13, "resume: the Resumed record written with a wrong stop number (one more)",
      MOD, A_RESUME,
      [('            .push(("stop_number".into(), request.stop_number.to_string()));\n',
        '            .push(("stop_number".into(), request.stop_number.wrapping_add(1).to_string()));\n')],
      [K("T2", T2_RECORD_NUMBER)],
      note="Row 13, paired reading ruled by the First Architect: the record is judged by T2."),
    M("H13a", "control", 13, "resume: the confirmation built with a wrong stop number (one less)",
      MOD, A_RESUME,
      [("            stop_number: stop.number,\n", "            stop_number: stop.number - 1,\n")],
      [K("T1", T1_SHOWN, says="the confirmer received a number that is not 1")],
      uncredited={"T2": WAIT_NOT_EVIDENCE}, hist="C13-a", origin="R2",
      note="The earlier campaign's own text of C13-a. Row 13, paired reading: judged by T1."),
    M("H13b", "control", 13, "resume: the Resumed record written with a wrong stop number (one less)",
      MOD, A_RESUME,
      [('            .push(("stop_number".into(), request.stop_number.to_string()));\n',
        '            .push(("stop_number".into(), (request.stop_number - 1).to_string()));\n')],
      [K("T2", T2_RECORD_NUMBER)], hist="C13-b", origin="R2",
      note="The earlier campaign's own text of C13-b. Row 13, paired reading: judged by T2."),
    M("C14a", "control", 14, "check_lift: already lifted treated as go on",
      RUN, A_CHECK,
      [("            return Ok(Lift::AlreadyLifted);\n", "            return Ok(Lift::Lifted);\n")],
      [K("T7", T7_C_ONE_RECORD, says="part (c): a second Resumed record")], hist="C14-a"),
    M("C14b", "control", 14, "check_lift: already lifted treated as a refusal",
      RUN, A_CHECK,
      [("            return Ok(Lift::AlreadyLifted);\n",
        "            return Err(Refusal::Superseded);\n")],
      [K("T7", T7_C_OK, says="part (c): B is refused")], hist="C14-b"),
    M("C15", "control", 15, "lift: the count not set to zero on a lift",
      RUN, A_LIFT, [("        self.cancelled_by_stop.store(0, Ordering::SeqCst);\n", "")],
      [K("T6", T6_COUNT_ZERO)], hist="C15"),

    # ---- row 16: one hold split into two, in each of its four ways, twice ----
    M("C16a", "control", 16, "lift: a second acquisition (the lock dropped and taken again before the clear)",
      RUN, A_LIFT,
      [(CLEAR_STOPPED, "        drop(_runs);\n" + LOCK + CLEAR_STOPPED)],
      [T8("lift: (1)", "lift: (3)")], hist="C16-a"),
    M("C16b", "control", 16, "lift: its tests in an inner block under one hold, the clear under a second",
      RUN, A_LIFT,
      [(LOCK + CLOSED + EXHAUSTED + NUMBER + ALREADY,
        "        {\n" + LOCK + CLOSED + EXHAUSTED + NUMBER + ALREADY + "        }\n" + LOCK)],
      [T8("lift: (1)", "lift: (2)")], hist="C16-b", origin="R2"),
    M("C16c", "control", 16, "lift: a temporary guard for the poison test, a let for the clear",
      RUN, A_LIFT,
      [(LOCK + CLOSED + EXHAUSTED + NUMBER + ALREADY,
        "        if self.runs.lock().is_err() {\n"
        "            return Err(Refusal::Unavailable);\n"
        "        }\n" + CLOSED + EXHAUSTED + NUMBER + ALREADY + LOCK)],
      [T8("lift: (1)", "lift: (2)")], hist="C16-c", origin="R2"),
    M("C16d", "control", 16, "lift: its tests handed to self.check_lift, then its own lock for the clear",
      RUN, A_LIFT,
      [(LOCK + CLOSED + EXHAUSTED + NUMBER + ALREADY,
        "        if self.check_lift(number)? == Lift::AlreadyLifted {\n"
        "            return Ok(Lift::AlreadyLifted);\n"
        "        }\n" + LOCK)],
      [T8("lift: (4)", "lift: (5)")], hist="C16-d", origin="R2"),
    M("C16e", "control", 16, "stop_all: a second acquisition (the lock dropped and taken again before the sweep)",
      RUN, A_STOP, [(SET_STOPPED, SET_STOPPED + "        drop(runs);\n" + STOP_LOCK)],
      [T8("stop_all: (1)", "stop_all: (3)")], hist="C16-e"),
    M("C16f", "control", 16, "stop_all: the flag set in an inner block under one hold, the rest under a second",
      RUN, A_STOP,
      [(STOP_LOCK + "        let current",
        "        {\n"
        "            let _first = self.runs.lock().unwrap_or_else(PoisonError::into_inner);\n"
        "            self.stopped.store(true, Ordering::SeqCst);\n"
        "        }\n" + STOP_LOCK + "        let current"),
       ("        };\n" + SET_STOPPED + "        let mut cancelled = 0usize;\n",
        "        };\n        let mut cancelled = 0usize;\n")],
      [T8("stop_all: (1)", "stop_all: (2)")], hist="C16-f", origin="R2"),
    M("C16g", "historical-form", 16, "stop_all: a temporary guard through 'let _ =' (the earlier campaign's first form)",
      RUN, A_STOP,
      [(STOP_LOCK, "        let _ = self.runs.lock().unwrap_or_else(PoisonError::into_inner);\n"),
       (SWEEP, SWEEP_TEMPORARY)],
      [], build_error="non-binding let on a synchronization lock", hist="C16-g", origin="R2",
      note="The earlier campaign's exact text. There the compiler refused it. A build error is not "
           "a kill and is not counted as one; the defect is judged in its second form, C16g2."),
    M("C16g2", "control", 16, "stop_all: a temporary guard in an expression statement (the second form)",
      RUN, A_STOP,
      [(STOP_LOCK, "        self.runs.lock().unwrap_or_else(PoisonError::into_inner);\n"),
       (SWEEP, SWEEP_TEMPORARY)],
      [T8("stop_all: (1)", "stop_all: (2)")], hist="C16-g2", origin="R2",
      note="The earlier campaign's exact text."),
    M("C16h", "control", 16, "stop_all: a call to another locking method (self.views()) before its hold",
      RUN, A_STOP,
      [(STOP_LOCK,
        "        let active = self.views().iter().filter(|v| !v.cancelled && !v.finished).count();\n"
        "        let _ = active;\n" + STOP_LOCK)],
      [T8("stop_all: (4)")], hist="C16-h", origin="R2"),

    # ---- row 17 ---------------------------------------------------------------
    M("C17", "control", 17, "stop_all: a poisoned lock makes the stop panic",
      RUN, A_STOP,
      [(STOP_LOCK, '        let mut runs = self.runs.lock().expect("runs");\n')],
      [K("T11", '        let mut runs = self.runs.lock().expect("runs");', "runs: PoisonError",
         where=W_STOP, how="fault", via=(T11_STOP_AFTER_POISON, 1),
         says="the stop itself panics, which the row names: T11 (a)")], hist="C17"),

    # ---- row 18: closed, or exhausted, tested before poison, at each site ------
    M("C18a", "control", 18, "stop_to_lift: closed tested before poison",
      RUN, A_READ, [(LOCK + CLOSED, CLOSED + LOCK)], [K("T11", T11_A, "(a) Closed")], hist="C18-a"),
    M("C18b", "control", 18, "stop_to_lift: exhausted tested before poison",
      RUN, A_READ, [(EXHAUSTED, ""), (LOCK + CLOSED, EXHAUSTED + LOCK + CLOSED)],
      [K("T11", T11_A, "(a) Exhausted")], hist="C18-b", origin="R2"),
    M("C18c", "control", 18, "check_lift: closed tested before poison",
      RUN, A_CHECK, [(LOCK + CLOSED, CLOSED + LOCK)], [K("T11", T11_B, "(b) Closed")],
      hist="C18-c", origin="R2"),
    M("C18d", "control", 18, "check_lift: exhausted tested before poison",
      RUN, A_CHECK, [(LOCK + CLOSED + EXHAUSTED, EXHAUSTED + LOCK + CLOSED)],
      [K("T11", T11_B, "(b) Exhausted")], hist="C18-d", origin="R2"),
    M("C18e", "control", 18, "lift: closed tested before poison",
      RUN, A_LIFT, [(LOCK + CLOSED, CLOSED + LOCK)], [K("T11", T11_C, "(c) Closed")],
      hist="C18-e", origin="R2", note="The same edit as C40, judged against row 18 by T11."),
    M("C18f", "control", 18, "lift: exhausted tested before poison",
      RUN, A_LIFT, [(LOCK + CLOSED + EXHAUSTED, EXHAUSTED + LOCK + CLOSED)],
      [K("T11", T11_C, "(c) Exhausted")], hist="C18-f"),

    # ---- row 19: a refusal on a poisoned lock changes the count or last lifted -
    M("C19a", "control", 19, "stop_to_lift: a refusal on a poisoned lock sets the count to zero",
      RUN, A_READ, [(UNAVAILABLE, ZERO_COUNT + UNAVAILABLE)],
      [K("T11", UNTOUCHED_COUNT, where=W_UNTOUCHED, via=(T11_UNTOUCHED, 1), says="part (a)")],
      hist="C19-a"),
    M("C19b", "control", 19, "stop_to_lift: a refusal on a poisoned lock changes last lifted",
      RUN, A_READ, [(UNAVAILABLE, CHANGE_LAST + UNAVAILABLE)],
      [K("T11", UNTOUCHED_NEXT, 'Some("2/yes")', where=W_UNTOUCHED, via=(T11_UNTOUCHED, 1),
         says="part (a)")], hist="C19-b", origin="R2"),
    M("C19c", "control", 19, "check_lift: a refusal on a poisoned lock sets the count to zero",
      RUN, A_CHECK, [(UNAVAILABLE, ZERO_COUNT + UNAVAILABLE)],
      [K("T11", UNTOUCHED_COUNT, where=W_UNTOUCHED, via=(T11_UNTOUCHED, 2), says="part (b)")],
      hist="C19-c", origin="R2"),
    M("C19d", "control", 19, "check_lift: a refusal on a poisoned lock changes last lifted",
      RUN, A_CHECK, [(UNAVAILABLE, CHANGE_LAST + UNAVAILABLE)],
      [K("T11", UNTOUCHED_NEXT, 'Some("2/yes")', where=W_UNTOUCHED, via=(T11_UNTOUCHED, 2),
         says="part (b)")], hist="C19-d", origin="R2"),
    M("C19e", "control", 19, "lift: a refusal on a poisoned lock sets the count to zero",
      RUN, A_LIFT, [(UNAVAILABLE, ZERO_COUNT + UNAVAILABLE)],
      [K("T11", UNTOUCHED_COUNT, where=W_UNTOUCHED, via=(T11_UNTOUCHED, 3), says="part (c)")],
      hist="C19-e", note="In the R1 draft this case was called C19c. The edit is unchanged."),
    M("C19f", "control", 19, "lift: a refusal on a poisoned lock changes last lifted",
      RUN, A_LIFT, [(UNAVAILABLE, CHANGE_LAST + UNAVAILABLE)],
      [K("T11", UNTOUCHED_NEXT, 'Some("2/yes")', where=W_UNTOUCHED, via=(T11_UNTOUCHED, 3),
         says="part (c)")], hist="C19-f", origin="R2"),

    # ---- rows 20 to 32: the timing harness's arithmetic and wording (T10a) -----
    M("C20", "control", 20, "numbers: a late observer's interval set to zero",
      TESTS, A_NUMBERS,
      [("            sample.observation_to_return = Value::span(seen, ret);\n        }\n",
        "            sample.observation_to_return = Value::span(seen, ret);\n"
        "        } else {\n"
        "            sample.observation_to_return = Value::Of(0);\n"
        "        }\n")],
      [K("T10a", T10A_LATE_OBSERVER)], hist="C20", origin="R2"),
    M("C21", "control", 21, "statistics given for a run with an incomplete sample",
      TESTS, A_STATISTICS,
      [("        if !samples.iter().all(Sample::complete) {\n            return None;\n        }\n", "")],
      [K("T10a", T10A_NO_STATISTICS)], hist="C21"),
    M("C22a", "control", 22, "numbers: a call whose report did not arrive is given a latency",
      TESTS, A_NUMBERS,
      [("                let Some(report) = &caller.report else {\n"
        "                    continue;\n"
        "                };\n",
        "                let Some(report) = &caller.report else {\n"
        "                    if let Some(begun) = caller.progress {\n"
        "                        sample.overlaps.push(Overlap {\n"
        "                            caller: caller.name,\n"
        "                            record: 0,\n"
        "                            latency: Value::span(Some(begun), r.readings.gave_up),\n"
        "                        });\n"
        "                    }\n"
        "                    continue;\n"
        "                };\n")],
      [K("T10a", T10A_MISSING_TEXT)], hist="C22-a", origin="R2"),
    M("C22b", "control", 22, "numbers: a stop whose report did not arrive is given a total",
      TESTS, A_NUMBERS,
      [("        sample.total = Value::span(start, ret);\n",
        "        sample.total = match r.stop {\n"
        "            Some(_) => Value::span(start, ret),\n"
        "            None => Value::span(r.readings.released, r.readings.gave_up),\n"
        "        };\n")],
      [K("T10a", T10A_NO_NUMBER, nth=3)], hist="C22-b", origin="R2"),
    M("C22c", "control", 22, "receipt text: a missing participant report gains an 'at least' figure",
      TESTS, A_EXPLANATION,
      [("                 published (progress marker) {}; asked to end {}; no duration is given for \\\n"
        "                 that call\",\n"
        "                ns(*last_start),\n"
        "                ns(sample.readings.asked_to_end)\n",
        "                 published (progress marker) {}; asked to end {}; that call took at least {}\",\n"
        "                ns(*last_start),\n"
        "                ns(sample.readings.asked_to_end),\n"
        "                ns(sample.waiting_time)\n")],
      [K("T10a", T10A_MISSING_TEXT)], hist="C22-c", origin="R2"),
    M("C22d", "control", 22, "receipt text: a missing stop report gains an 'at least' figure",
      TESTS, A_EXPLANATION,
      [("                 released {} is the controller's own reading, not the stop's start\",\n"
        "                ns(sample.readings.released)\n",
        "                 released {} is the controller's own reading, not the stop's start; the stop \\\n"
        "                 took at least {}\",\n"
        "                ns(sample.readings.released),\n"
        "                ns(sample.waiting_time)\n")],
      [K("T10a", T10A_STOP_TEXT)], hist="C22-d", origin="R2"),
    M("C23", "control", 23, "numbers: callers recorded as missing when the stop's report is missing",
      TESTS, A_NUMBERS,
      [("            if !r.callers.is_empty() {\n"
        "                sample.results.push(Outcome::ParticipantReportNotRequested(\n"
        "                    r.callers.iter().map(|caller| caller.name).collect(),\n"
        "                ));\n"
        "            }\n",
        "            for caller in &r.callers {\n"
        "                sample.results.push(Outcome::ParticipantReportMissing {\n"
        "                    participant: caller.name,\n"
        "                    last_start: caller.progress,\n"
        "                });\n"
        "            }\n")],
      [K("T10a", T10A_STOP_RESULTS)], hist="C23", origin="R2"),
    M("C24a", "control", 24, "numbers: a late participant report removes 'participant report missing'",
      TESTS, A_NUMBERS,
      [("            for caller in r.callers.iter().filter(|caller| caller.report.is_none()) {\n",
        "            for caller in r.callers.iter().filter(|caller| {\n"
        "                caller.report.is_none()\n"
        "                    && !r.late.iter().any(\n"
        "                        |late| matches!(late, Late::Caller(name, _) if *name == caller.name),\n"
        "                    )\n"
        "            }) {\n")],
      [K("T10a", T10A_LATE_CHANGES_NOTHING, nth=1)], hist="C24-a", origin="R2"),
    M("C24b", "control", 24, "numbers: a late stop report removes 'stop return not observed'",
      TESTS, A_NUMBERS,
      [("        if r.stop.is_none() {\n"
        "            sample.results.push(Outcome::StopReturnNotObserved);\n",
        "        if r.stop.is_none() {\n"
        "            if !r.late.iter().any(|late| matches!(late, Late::Stop(_))) {\n"
        "                sample.results.push(Outcome::StopReturnNotObserved);\n"
        "            }\n")],
      [K("T10a", T10A_LATE_CHANGES_NOTHING, nth=2)], hist="C24-b", origin="R2"),
    M("C25a", "control", 25, "summary: an empty group is given statistics of zero",
      TESTS, A_SUMMARY,
      [("            }\n        }\n        summary\n",
        "            }\n"
        "        } else {\n"
        "            summary.min = Some(0);\n"
        "            summary.median = Some(0);\n"
        "            summary.max = Some(0);\n"
        "        }\n"
        "        summary\n")],
      [K("T10a", T10A_EMPTY_GROUPS)], hist="C25-a", origin="R2"),
    M("C25b", "control", 25, "summary: an empty group faults (its guard removed)",
      TESTS, A_SUMMARY, [(SUMMARY_GUARD, "        {\n")],
      [K("T10a", "        sorted[rank - 1]", "attempt to subtract with overflow",
         where=W_PERCENTILE, how="fault", via=(T10A_99, 1),
         says="the fault the row names, reached from T10a; not an assertion of T10a")],
      hist="C25-b", origin="R2"),
    M("C26a", "control", 26, "summary: an unavailable value is counted in n",
      TESTS, A_SUMMARY,
      [("        summary.n = available.len();\n",
        "        summary.n = available.len() + summary.unavailable;\n")],
      [K("T10a", T10A_MIXED)], hist="C26-a", origin="R2"),
    M("C26b", "control", 26, "summary: an unavailable value enters the statistics as zero",
      TESTS, A_SUMMARY,
      [("                Value::Unavailable => summary.unavailable += 1,\n",
        "                Value::Unavailable => {\n"
        "                    summary.unavailable += 1;\n"
        "                    available.push(0);\n"
        "                }\n"),
       ("        summary.n = available.len();\n",
        "        summary.n = available.len() - summary.unavailable;\n")],
      [K("T10a", T10A_MIXED)], hist="C26-b", origin="R2"),
    M("C27", "control", 27, "numbers: flag latency bound made the larger of its two numbers",
      TESTS, A_NUMBERS, [("Value::Of(observed.min(total))", "Value::Of(observed.max(total))")],
      [K("T10a", T10A_FIRST)], hist="C27"),
    M("C28", "control", 28, "numbers: a missing readiness signal goes on as a sample of the stop",
      TESTS, A_NUMBERS,
      [("            sample.waiting_time = waited(r.readings.gave_up, Some(r.readings.threads_started));\n"
        "            return sample;\n",
        "            sample.waiting_time = waited(r.readings.gave_up, Some(r.readings.threads_started));\n")],
      [K("T10a", T10A_READINESS)], hist="C28", origin="R2"),
    M("C29a", "control", 29, "numbers: seen before start is accepted",
      TESTS, A_NUMBERS, [(SEEN_FAULT, "")], [K("T10a", T10A_SEEN_FAULT)], hist="C29-a", origin="R2"),
    M("C29b", "control", 29, "numbers: return before start is accepted",
      TESTS, A_NUMBERS, [(RETURN_FAULT, "")], [K("T10a", T10A_BOTH_FAULTS)],
      hist="C29-b", origin="R2"),
    M("C30", "control", 30, "percentile: the position rounded down",
      TESTS, A_PERCENTILE,
      [("        let rank = (p * sorted.len()).div_ceil(100);\n",
        "        let rank = ((p * sorted.len()) / 100).max(1);\n")],
      [K("T10a", T10A_99)], hist="C30-a"),
    M("C30b", "control", 30, "percentile: the position counted from zero",
      TESTS, A_PERCENTILE, [("        sorted[rank - 1]\n", "        sorted[rank]\n")],
      [K("T10a", T10A_99)], hist="C30-b", origin="R2"),
    M("C31", "control", 31, "summary: a 99th percentile given for 99 values",
      TESTS, A_SUMMARY,
      [("            if available.len() >= 100 {\n", "            if available.len() >= 99 {\n")],
      [K("T10a", T10A_99)], hist="C31-a"),
    M("C31b", "control", 31, "summary: the median withheld from a group of fewer than 100",
      TESTS, A_SUMMARY,
      [("            summary.median = Some(percentile(&available, 50));\n",
        "            if available.len() >= 100 {\n"
        "                summary.median = Some(percentile(&available, 50));\n"
        "            }\n")],
      [K("T10a", T10A_99)], hist="C31-b", origin="R2"),
    M("C32a", "control", 32, "numbers: overlap taken from the log filter, not from the timestamps",
      TESTS, A_NUMBERS, [(OVERLAP_TEST, "                    if true {\n")],
      [K("T10a", T10A_OVERLAPS)], hist="C32-a"),
    M("C32b", "control", 32, "numbers: a call that ends exactly at the stop's start is not counted",
      TESTS, A_NUMBERS,
      [(OVERLAP_TEST, "                    if call.end > start && call.start <= ret {\n")],
      [K("T10a", T10A_OVERLAPS)], hist="C32-b"),
    M("C32c", "control", 32, "numbers: a call that starts exactly at the stop's return is not counted",
      TESTS, A_NUMBERS,
      [(OVERLAP_TEST, "                    if call.end >= start && call.start < ret {\n")],
      [K("T10a", T10A_OVERLAPS)], hist="C32-c", origin="R2"),

    # ---- rows 33 to 40 ----------------------------------------------------------
    M("C33", "control", 33, "stop_all: the stop clears the lock's poison",
      RUN, A_STOP, [(STOP_LOCK, "        self.runs.clear_poison();\n" + STOP_LOCK)],
      [K("T11", T11_A, "(a) Plain", says="the resume gets past its first lock site")],
      hist="C33", note="T8 is expected to fail too, on (6). The row names T11."),
    M("C34a", "control", 34, "stop_to_lift: a refusing resume clears the lock's poison",
      RUN, A_READ, [(UNAVAILABLE, CLEAR_POISON + UNAVAILABLE)],
      [K("T11", UNTOUCHED_AGAIN, where=W_UNTOUCHED, via=(T11_UNTOUCHED, 1),
         says="the second resume is not refused as unavailable; part (a)")],
      hist="C34-a", note="T8 is expected to fail too, on (6). The row names T11."),
    M("C34b", "control", 34, "check_lift: a refusing resume clears the lock's poison",
      RUN, A_CHECK, [(UNAVAILABLE, CLEAR_POISON + UNAVAILABLE)],
      [K("T11", UNTOUCHED_AGAIN, where=W_UNTOUCHED, via=(T11_UNTOUCHED, 2),
         says="the second resume is not refused as unavailable; part (b)")],
      hist="C34-b", origin="R2", note="T8 is expected to fail too, on (6). The row names T11."),
    M("C34c", "control", 34, "lift: a refusing resume clears the lock's poison",
      RUN, A_LIFT, [(UNAVAILABLE, CLEAR_POISON + UNAVAILABLE)],
      [K("T11", UNTOUCHED_AGAIN, where=W_UNTOUCHED, via=(T11_UNTOUCHED, 3),
         says="the second resume is not refused as unavailable; part (c)")],
      hist="C34-c", origin="R2", note="T8 is expected to fail too, on (6). The row names T11."),
    M("C35", "control", 35, "stop_to_lift: not stopped and poisoned returns nothing-to-lift",
      RUN, A_READ,
      [(UNAVAILABLE,
        "            if !self.stopped.load(Ordering::SeqCst) {\n"
        "                return Ok(None);\n"
        "            }\n" + UNAVAILABLE)],
      [K("T11", T11_E, says="part (e)")], hist="C35"),
    M("C36", "control", 36, "run.rs: clear_poison appears in code outside the stop and the resume",
      RUN, A_PROBE,
      [('        drop(self.runs.lock().expect("runs"));\n',
        '        self.runs.clear_poison();\n        drop(self.runs.lock().expect("runs"));\n')],
      [T8("run.rs: (6)")], hist="C36",
      note="A source-shape control: it shows T8's text check, not a fault a user could meet."),
    M("C37", "control", 37, "stop_to_lift: a poisoned lock no longer refuses",
      RUN, A_READ, [(LOCK, LOCK_THROUGH_POISON)],
      [K("T11", T11_A_ASKED, nth=1, says="part (a): the confirmer is asked")], hist="C37"),
    M("C38", "control", 38, "check_lift: a poisoned lock no longer refuses",
      RUN, A_CHECK, [(LOCK, LOCK_THROUGH_POISON)],
      [K("T11", T11_B_NO_RECORD, says="part (b): a Resumed record appears")], hist="C38"),
    M("C39", "control", 39, "lift: a poisoned lock no longer refuses",
      RUN, A_LIFT, [(LOCK, LOCK_THROUGH_POISON)], [K("T11", T11_C, "(c) Plain")], hist="C39"),
    M("C40", "control", 40, "lift: the test of closed moved before the lock",
      RUN, A_LIFT, [(LOCK + CLOSED, CLOSED + LOCK)], [T8("lift: (5)")], hist="C40",
      note="A source-shape control. The same edit as C18e, judged here against row 40 by T8."),

    # ---- not contract rows, each with a named expected failure -----------------
    M("A1", "probe", 0, "approval.rs: the dialog text no longer names the stop number",
      APPROVAL, None,
      [('                ("Lifts", format!("emergency stop {}", self.stop_number)),\n',
        '                ("Lifts", "the emergency stop".to_string()),\n')],
      [K("T1", T1_DIALOG_TEXT, "the dialog does not name stop 1:")], hist="A1"),
    M("X1", "probe", 0, "lift: its tests made through 'if self.check_lift(number)?' before its own lock",
      RUN, A_LIFT,
      [(LOCK + CLOSED + EXHAUSTED + NUMBER + ALREADY,
        "        if self.check_lift(number)? == Lift::AlreadyLifted {\n"
        "            return Ok(Lift::AlreadyLifted);\n"
        "        }\n" + LOCK + CLOSED)],
      [T8("lift: (4)")], hist="X1", note="A source-shape probe of T8's rule against a call on self."),
    M("X2", "probe", 0, "stop_all: 'for v in self.views()' before its hold",
      RUN, A_STOP,
      [(STOP_LOCK,
        "        let mut active = 0usize;\n"
        "        for v in self.views() {\n"
        "            if !v.cancelled && !v.finished {\n"
        "                active = active.saturating_add(1);\n"
        "            }\n"
        "        }\n"
        "        let _ = active;\n" + STOP_LOCK)],
      [T8("stop_all: (4)")], hist="X2", note="A source-shape probe."),
    M("X3", "probe", 0, "lift: 'match self.check_lift(number)?' before its own lock",
      RUN, A_LIFT,
      [(LOCK,
        "        match self.check_lift(number)? {\n"
        "            Lift::AlreadyLifted => return Ok(Lift::AlreadyLifted),\n"
        "            Lift::Lifted => {}\n"
        "        }\n" + LOCK)],
      [T8("lift: (4)")], hist="X3", note="A source-shape probe."),
    M("X4", "probe", 0, "stop_all: 'while self.views()...' before its hold",
      RUN, A_STOP,
      [(STOP_LOCK,
        "        while self.views().len() > RUN_CAPACITY {\n"
        "            break;\n"
        "        }\n" + STOP_LOCK)],
      [T8("stop_all: (4)")], hist="X4", origin="R2",
      note="A source-shape probe. The R1 draft left it out; the earlier campaign had one after 'while'. "
           "This text is written here: the earlier text was not available to the author."),
    M("X5", "probe", 0, "lift: 'return self.check_lift(number);' on a branch before its own lock",
      RUN, A_LIFT,
      [(LOCK,
        "        if !self.stopped.load(Ordering::SeqCst) {\n"
        "            return self.check_lift(number);\n"
        "        }\n" + LOCK)],
      [T8("lift: (4)")], hist="X5", note="A source-shape probe."),
    M("D1", "harness", 0, "controller: a turn whose clock reading is past the deadline is accepted",
      TESTS, A_CONTROL, [(LATE_TURN, "")], [T10B("S1, S3")], hist="D1"),
    M("D2", "harness", 0, "controller: readiness accepted without regard to its clock reading",
      TESTS, A_CONTROL,
      [("            if now <= readiness_deadline {\n                missing = not_ready;\n",
        "            {\n                missing = not_ready;\n")],
      [T10B("S2")], hist="D2"),
    M("D3", "harness", 0, "controller: the clock read before the reports are taken",
      TESTS, A_CONTROL,
      [("            let mut seen = Turn::default();\n            if received.stop.is_none() {\n",
        "            let now = world.now();\n"
        "            let mut seen = Turn::default();\n            if received.stop.is_none() {\n"),
       ("            let now = world.now();\n            if now > deadline {\n",
        "            if now > deadline {\n")],
      [T10B("S3")], hist="D3"),
    M("D4", "harness", 0, "controller: a reading equal to the deadline counted as late",
      TESTS, A_CONTROL,
      [("            if now > deadline {\n", "            if now >= deadline {\n")],
      [T10B("S5")], hist="D4",
      note="A contract-correctness control of the harness: the conservative side of the boundary."),
    M("D5", "harness", 0, "controller: the reports taken in a late turn are thrown away",
      TESTS, A_CONTROL,
      [("                give_up(world, &mut received, now, seen);\n",
        "                give_up(world, &mut received, now, Turn::default());\n")],
      [T10B("S1, S3")], hist="D5", origin="R2",
      note="Left out of the R1 draft by the author's oversight. It is planned here; no result is claimed."),
    M("R03", "reviewer", 0, "resume: refresh the stop number after recording instead of lifting the confirmed number",
      MOD, A_RESUME,
      [("        self.runs.lift(request.stop_number).map_err(refused)?;\n",
        "        let number_to_lift = self.runs.stop_to_lift().map_err(refused)?\n"
        "            .map_or(request.stop_number, |latest| latest.number);\n"
        "        self.runs.lift(number_to_lift).map_err(refused)?;\n")],
      [K("T2", NEWER_STOP_LIFTED, "the newer stop was lifted:")], origin="reviewer",
      note="Independent confirmation-authority probe. The registry comparison itself "
           "remains intact, but the caller substitutes an unconfirmed newer number. "
           "Credit requires T2's actual newer-stop assertion, not a wait failure or "
           "the same message printed by a different test."),
    M("R04", "reviewer", 0, "check_lift: clear stopped while returning Superseded",
      RUN, A_CHECK,
      [("            return Err(Refusal::Superseded);\n",
        "            self.stopped.store(false, Ordering::SeqCst);\n"
        "            return Err(Refusal::Superseded);\n")],
      [K("T1", NEWER_STOP_LIFTED, "the newer stop was lifted:")], origin="reviewer",
      note="Independent refusal-state probe. Returning the correct error is not enough "
           "when the supposedly read-only check changes authority state. Credit requires "
           "T1's actual newer-stop assertion in T1's failure block."),
    M("R07", "reviewer", 0, "stop_all: report exhaustion but fail to persist the exhausted flag",
      RUN, A_STOP,
      [("                self.exhausted.store(true, Ordering::SeqCst);\n", "")],
      [K("T5", RESUME_REFUSED_EXHAUSTED, says="the resume is not refused as emergency-stopped"),
       K("T5b", RESUME_REFUSED_EXHAUSTED, says="the resume is not refused as emergency-stopped"),
       K("T5c", T5C_REFUSED, says="the resume is not refused as emergency-stopped")],
      origin="reviewer",
      note="Independent exhaustion-state probe, distinct from wrapping the number. "
           "The returned Stop still reports exhaustion and the number stays at MAX, "
           "but resume no longer observes the permanent refusal state. Each expected "
           "failure needs its own causal assertion, not a fixture wait failure. "
           "The three assertions were chosen by the Lead Architect from the source; "
           "the reviewer named the tests."),

    # ---- no result is claimed in advance ---------------------------------------
    M("O1", "exploratory", 0, "stop_all: the stopped flag set BEFORE the lock is taken (the order before RS-01)",
      RUN, A_STOP,
      [(STOP_LOCK + "        let current", SET_STOPPED + STOP_LOCK + "        let current"),
       ("        };\n" + SET_STOPPED + "        let mut cancelled = 0usize;\n",
        "        };\n        let mut cancelled = 0usize;\n")],
      note="Mandatory stop-order regression probe (REVIEW-R1). The gap in T8 is already established "
           "from the source: for the stop, T8 does not require the flag to be set after the "
           "acquisition. The run-time outcome is unobserved. The mutant is a real defect: a lift "
           "holding the lock can clear the flag that a newer stop has set but not yet numbered."),
    M("O2", "exploratory", 0, "lift: the tests of the stop number and of already-lifted swapped",
      RUN, A_LIFT, [(NUMBER + ALREADY, ALREADY + NUMBER)]),
    M("O3", "exploratory", 0, "lift: the last lifted stop not written",
      RUN, A_LIFT, [("        self.last_lifted.store(number, Ordering::SeqCst);\n", "")]),
    M("O4", "exploratory", 0, "resume: the Resumed record written after the lift, not before",
      MOD, A_RESUME,
      [(RECORD_THEN_LIFT,
        "        self.runs.lift(request.stop_number).map_err(refused)?;\n"
        "        self.record(&record)?;\n")]),
    M("O5", "exploratory", 0, "check_lift: the test of the stop number removed",
      RUN, A_CHECK, [(NUMBER, "")]),
    M("O6", "exploratory", 0, "emergency_stop: its record carries a wrong stop number",
      MOD, A_ESTOP,
      [('            .push(("stop_number".into(), stop.number.to_string()));\n',
        '            .push(("stop_number".into(), stop.number.wrapping_add(1).to_string()));\n')]),
    M("O7", "exploratory", 0, "stop_all: the count keeps only this stop's runs, not those since the last lift",
      RUN, A_STOP,
      [("            .store(since_the_last_lift, Ordering::SeqCst);\n",
        "            .store(cancelled, Ordering::SeqCst);\n")]),
    M("O8", "exploratory", 0, "refused: a superseded resume reported as emergency-stopped",
      MOD, A_REFUSED,
      [("        Refusal::Superseded => AuthorityError::ResumeSuperseded,\n",
        "        Refusal::Superseded => AuthorityError::EmergencyStopped,\n")],
      end=END_FREE, note="A reporting-correctness probe, not a stop-authority one."),
    M("R01", "reviewer", 0, "lift: move the guard into a closure that releases it before the checks",
      RUN, A_LIFT,
      [(LOCK,
        LOCK +
        "        let release_guard = |guard| { let _guard = guard; };\n"
        "        release_guard(_runs);\n")],
      origin="reviewer",
      note="Independent guard-lifetime probe. The direct acquisition remains, but its guard "
           "is moved and dropped when the closure returns. No explicit drop( and no extra "
           "self call are introduced. Source inspection predicts that the current T8 "
           "shape checks miss this; compilation and runtime outcome remain untested."),
    M("R02", "reviewer", 0, "stop_all: read the stop generation before acquiring its mutex",
      RUN, A_STOP,
      [(STOP_LOCK + "        let current = self.stop_number.load(Ordering::SeqCst);\n",
        "        let current = self.stop_number.load(Ordering::SeqCst);\n" + STOP_LOCK)],
      origin="reviewer",
      note="Independent generation-serialization probe. Two concurrent stops can read the "
           "same current number before either acquisition and subsequently publish the "
           "same next number. The flag write and cancellation stay under the guard. "
           "A pass of sequential tests would not establish generation uniqueness."),
    M("R05", "reviewer", 0, "resume: discard failure to record Resumed and continue to lift",
      MOD, A_RESUME,
      [("        self.record(&record)?;\n", "        let _ = self.record(&record);\n")],
      origin="reviewer",
      note="Independent evidence-first probe. No test failure is claimed in advance for "
           "the filtered rs01_ suite. A survivor is scoped to that suite, not the entire "
           "repository. A valid kill must exercise a failed Resumed record and establish "
           "that the stop must remain in force."),
    M("R06", "reviewer", 0, "resume: ask the Owner but ignore a declined confirmation",
      MOD, A_RESUME,
      [("        if !confirmer.confirm_resume(&request) {\n"
        "            return Err(AuthorityError::Declined);\n"
        "        }\n",
        "        let _ = confirmer.confirm_resume(&request);\n")],
      origin="reviewer",
      note="Independent consent-authority probe. The dialog still runs, so tests using "
           "only affirmative replies may miss the change. A valid kill must establish "
           "that a declined confirmation cannot lift the stop. No whole-repository "
           "coverage claim follows from this filtered campaign."),
    M("R08", "reviewer", 0, "close: release the registry guard before publishing closed",
      RUN, A_CLOSE,
      [("        let _runs = self.runs.lock().expect(\"runs\");\n",
        "        let _runs = self.runs.lock().expect(\"runs\");\n"
        "        let release_guard = |guard| { let _guard = guard; };\n"
        "        release_guard(_runs);\n")],
      origin="reviewer",
      note="Independent shutdown-serialization probe. The closed write no longer shares "
           "the retained registry critical section with admission and compare-and-clear. "
           "T8 inspects stop_all and lift, not close; no current test kill is predicted. "
           "An unrelated panic is not a shutdown-ordering assertion."),
]

# The contract's control rows (P3-C11-RS01-CONTRACT-05, section 8), in its order.
ROWS = {
    1: ("step 6's clear made unconditional", "T2; T3; T5c"),
    2: ("the test of closed removed from step 6", "T3"),
    3: ("the test of exhausted removed from step 6", "T5c"),
    4: ("the test of the stop number removed from step 6", "T2"),
    5: ("the test of closed removed from step 1", "T4, T4b"),
    6: ("the test of exhausted removed from step 1", "T5"),
    7: ("the early check removed", "T1, T3b, T5b"),
    8: ("the count zeroed on a step-6 refusal", "T2, T3"),
    9: ("the stop number not advanced by a stop", "T1"),
    10: ("the stop number wrapping at its maximum", "T5"),
    11: ("previous stop lifted always no", "T6"),
    12: ("already lifted answered as lifted; or as a refusal", "T7 (b); T7 (a)"),
    13: ("the confirmation or the Resumed record carries a wrong stop number",
         "T1, T2 (paired reading: confirmation by T1, record by T2)"),
    14: ("the early check treats already lifted as go on, or as a refusal", "T7 (c)"),
    15: ("the count not zeroed on a lift", "T6"),
    16: ("step 6, or the stop, split into two lock holds: a second acquisition, an inner "
         "block, a temporary guard, or a call to another locking method", "T8"),
    17: ("the stop's lock site fails on a poisoned lock", "T11 (a)"),
    18: ("a resume tests closed, or exhausted, before poison, at any of its three lock sites",
         "T11 (d)"),
    19: ("a refused resume on a poisoned lock zeroes the count or changes last lifted",
         "T11 (a), (b), (c)"),
    20: ("the receipt's arithmetic clamps a late observer's interval to zero", "T10a"),
    21: ("the receipt gives any statistic for a run with a sample that is not complete", "T10a"),
    22: ("a number for a call or a stop whose completion was not received; or the receipt "
         "text for a missing report differs, for example by an 'at least' figure", "T10a"),
    23: ("competing threads recorded as missing when the stop's report is missing", "T10a"),
    24: ("a late report changes a result", "T10a"),
    25: ("an empty group is given a statistic, or faults", "T10a"),
    26: ("an unavailable value is counted in n, or enters a statistic as zero", "T10a"),
    27: ("flag latency bound is the larger of its two numbers", "T10a"),
    28: ("a missing readiness signal is recorded as a sample of the stop", "T10a"),
    29: ("seen or return before start is accepted as a sample", "T10a"),
    30: ("the percentile position is rounded down, or counted from zero", "T10a"),
    31: ("a 99th percentile for fewer than 100 values; or the median withheld", "T10a"),
    32: ("overlap from the filter and not the timestamps; or a single-instant touch not counted",
         "T10a"),
    33: ("the stop clears the lock's poison", "T11 (a)"),
    34: ("a refusing resume clears the lock's poison", "T11"),
    35: ("a resume on a poisoned lock that is not stopped returns success", "T11 (e)"),
    36: ("the text clear_poison appears in run.rs", "T8"),
    37: ("the resume's read goes on through a poisoned lock", "T11 (a)"),
    38: ("the resume's early check goes on through a poisoned lock", "T11 (b)"),
    39: ("the resume's compare-and-clear goes on through a poisoned lock", "T11 (c)"),
    40: ("the closed test of step 6 moved before the lock", "T8"),
}


# ===========================================================================
# Small tools
# ===========================================================================
class Refused(Exception):
    """The script will not start, or will not go on. Nothing is awarded."""


class Interrupted(Exception):
    """A signal asked the campaign to end."""


class Quarantine(Exception):
    """A case left a process behind that could not be ended: nothing is restored."""


SIGNALS = []


def on_signal(number, _frame):
    SIGNALS.append(number)


def check_signals():
    if SIGNALS:
        raise Interrupted("the runner's own alarm (the whole budget is used up)"
                          if SIGNALS[0] == signal.SIGALRM else "signal %d" % SIGNALS[0])


def say(text):
    """Progress for whoever watches. The receipts never depend on it."""
    try:
        sys.stdout.write(text + "\n")
        sys.stdout.flush()
    except OSError:
        pass


def now_utc():
    return time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())


def sha256(data):
    return hashlib.sha256(data).hexdigest()


def sha256_file(path):
    digest = hashlib.sha256()
    with open(path, "rb") as handle:
        for chunk in iter(lambda: handle.read(1 << 20), b""):
            digest.update(chunk)
    return digest.hexdigest()


def blob_id(data):
    return hashlib.sha1(b"blob %d\0" % len(data) + data).hexdigest()


def durable_write(path, data):
    """Write a file so that it is either the old one or the whole new one."""
    if isinstance(data, str):
        data = data.encode("utf-8")
    temporary = path + ".part"
    with open(temporary, "wb") as handle:
        handle.write(data)
        handle.flush()
        os.fsync(handle.fileno())
    os.replace(temporary, path)
    folder = os.open(os.path.dirname(path), os.O_RDONLY)
    try:
        os.fsync(folder)
    finally:
        os.close(folder)


def durable_append(path, line):
    with open(path, "ab") as handle:
        handle.write(line.encode("utf-8") + b"\n")
        handle.flush()
        os.fsync(handle.fileno())


def as_json(value):
    return json.dumps(value, indent=2, sort_keys=True, ensure_ascii=False) + "\n"


# ===========================================================================
# The text of the mutants and of the expected assertions (no build)
# ===========================================================================
def region(text, anchor, end, who):
    """The part of a file between an anchor line and the end of its function."""
    if anchor is None:
        return 0, len(text)
    if text.count(anchor) != 1:
        raise ValueError("%s: its anchor occurs %d times" % (who, text.count(anchor)))
    start = text.index(anchor)
    stop = text.find(end, start)
    if stop < 0:
        raise ValueError("%s: the end of its function was not found" % who)
    return start, stop + len(end)


def apply(text, mutant):
    """The mutated text. Every edit's old text must occur exactly once in the region."""
    for old, new in mutant["edits"]:
        start, stop = region(text, mutant["anchor"], mutant["end"], mutant["id"])
        body = text[start:stop]
        if body.count(old) != 1:
            raise ValueError(
                "%s: an edit's old text occurs %d times in its region" % (mutant["id"], body.count(old))
            )
        if old == new:
            raise ValueError("%s: an edit changes nothing" % mutant["id"])
        text = text[:start] + body.replace(old, new, 1) + text[stop:]
    return text


def unified(rel, before, after):
    return "".join(
        difflib.unified_diff(
            before.splitlines(keepends=True),
            after.splitlines(keepends=True),
            "a/" + rel,
            "b/" + rel,
            n=3,
        )
    )


def test_region(short):
    """Where a test's own function is: (file, anchor, end)."""
    name = INVENTORY[short].split("::")[-1]
    if INVENTORY[short].startswith(_S):
        return TESTS, "    fn %s() {\n" % name, END_METHOD
    return TESTS, "fn %s() {\n" % name, END_FREE


def lines_of(text, where, snippet, nth, who):
    """The first and last line (from 1) of the nth whole-line occurrence of snippet."""
    file, anchor, end = where
    start, stop = region(text, anchor, end, who)
    needle = "\n" + snippet + "\n"
    at = start
    for _ in range(nth):
        at = text.find(needle, at + 1, stop)
        if at < 0:
            raise ValueError("%s: fewer than %d occurrences of %r" % (who, nth, snippet[:60]))
    first = text.count("\n", 0, at + 1) + 1
    return first, first + snippet.count("\n")


def resolve(oracle, compiled, who):
    """An expected failure as file and lines, in the files as they are compiled."""
    where = oracle["where"] or test_region(oracle["test"])
    if where[0] not in compiled:
        raise ValueError("%s: an expected failure is outside the four files" % who)
    lo, hi = lines_of(compiled[where[0]], where, oracle["snippet"], oracle["nth"], who)
    first_line = oracle["snippet"].split("\n")[0]
    found = {"test": oracle["test"], "file": where[0], "lo": lo, "hi": hi,
             "column": len(first_line) - len(first_line.lstrip(" ")) + 1, "text": oracle["text"],
             "how": oracle["how"], "says": oracle["says"], "via": None}
    if oracle["via"]:
        snippet, nth = oracle["via"]
        home = test_region(oracle["test"])
        via_lo, via_hi = lines_of(compiled[home[0]], home, snippet, nth, who)
        found["via"] = {"file": home[0], "lo": via_lo, "hi": via_hi}
    return found


def check_plan():
    """The plan is well formed. No file is read."""
    ids = [mutant["id"] for mutant in MUTANTS]
    if len(ids) != len(set(ids)):
        raise ValueError("duplicate mutant ids")
    if not ids or ids[0] != "S1":
        raise ValueError("the sanity gate S1 must be the first mutant")
    if any(re.match(r"^B(\d+|F)$", ident) for ident in ids):
        raise ValueError("a mutant has a baseline's id")
    for mutant in MUTANTS:
        who = mutant["id"]
        if mutant["file"] not in CLEAN:
            raise ValueError("%s: a file outside the four" % who)
        if mutant["row"] not in ROWS and mutant["row"] != 0:
            raise ValueError("%s: unknown row" % who)
        if (mutant["kind"] == "control") != (mutant["row"] != 0 and mutant["kind"] != "historical-form"):
            raise ValueError("%s: a control has a row, and only a control or a historical form has one" % who)
        if mutant["kind"] == "control" and not mutant["expect"]:
            raise ValueError("%s: a control names its expected failure" % who)
        if not mutant["edits"]:
            raise ValueError("%s: no edit" % who)
        tests = [oracle["test"] for oracle in mutant["expect"]]
        if len(tests) != len(set(tests)):
            raise ValueError("%s: a test is expected twice" % who)
        for name in tests + list(mutant["uncredited"]):
            if name not in INVENTORY or name == IGNORED:
                raise ValueError("%s: unknown test %s" % (who, name))
    for row in ROWS:
        if not any(m["row"] == row and m["kind"] == "control" for m in MUTANTS):
            raise ValueError("row %d has no control" % row)
    if len(INVENTORY) != 16 or len(set(INVENTORY.values())) != 16:
        raise ValueError("the inventory is not sixteen distinct tests")


def check_inventory_text(tests_text):
    """tests.rs defines exactly the sixteen tests of the inventory."""
    names = re.findall(r"^\s*fn (rs01_\w+)\(\) \{$", tests_text, re.M)
    want = sorted(full.split("::")[-1] for full in INVENTORY.values())
    if sorted(names) != want:
        raise ValueError("tests.rs does not define exactly the sixteen RS-01 tests of the inventory")
    for short, full in INVENTORY.items():
        name = full.split("::")[-1]
        at = tests_text.index("fn %s() {" % name)
        head = tests_text[tests_text.rfind("\n\n", 0, at):at]
        if "#[test]" not in head:
            raise ValueError("%s is not marked #[test]" % name)
        if ("#[ignore" in head) != (short == IGNORED):
            raise ValueError("%s: only T10 is ignored" % name)
    if tests_text.count("mod stop_measurement {") != 1:
        raise ValueError("the module stop_measurement was not found once")


def prepare(pristine):
    """Every mutant applied as text, and every expected failure found. No build.

    pristine: {file: bytes} of the four files. Returns {id: {...}} or raises.
    """
    check_plan()
    texts = {rel: data.decode("utf-8") for rel, data in pristine.items()}
    check_inventory_text(texts[TESTS])
    ready = {}
    for mutant in MUTANTS:
        before = texts[mutant["file"]]
        after = apply(before, mutant)
        compiled = dict(texts)
        compiled[mutant["file"]] = after
        data = after.encode("utf-8")
        patch = unified(mutant["file"], before, after)
        ready[mutant["id"]] = {
            "bytes": data,
            "sha256": sha256(data),
            "blob": blob_id(data),
            "diff": patch,
            "diff_sha256": sha256(patch.encode("utf-8")),
            "changed_lines": sum(1 for line in patch.splitlines()
                                 if line[:1] in "+-" and line[:3] not in ("+++", "---")),
            "oracles": [resolve(oracle, compiled, mutant["id"]) for oracle in mutant["expect"]],
        }
    hashes = [entry["sha256"] for entry in ready.values()]
    twins = {}
    for mutant in MUTANTS:
        twins.setdefault((mutant["file"], ready[mutant["id"]]["sha256"]), []).append(mutant["id"])
    allowed = [["C18e", "C40"]]  # one edit, judged against two rows by two tests
    for same in twins.values():
        if len(same) > 1 and same not in allowed:
            raise ValueError("these mutants are the same text: %s" % ", ".join(same))
    if len(hashes) != len(MUTANTS):
        raise ValueError("a mutant was lost")
    return ready


# ===========================================================================
# Identity of the source: the Git tree, from the files themselves
# ===========================================================================
def scan(src, tick=None):
    """{path: (git mode, blob id)} of every file under src. Refuses what a tree cannot hold."""
    found = {}

    def walk(folder, prefix):
        if tick:
            tick()
        entries = sorted(os.scandir(folder), key=lambda entry: entry.name)
        if not entries:
            raise Refused("an empty folder is in the source: %s" % (prefix or "."))
        for entry in entries:
            rel = prefix + entry.name
            info = entry.stat(follow_symlinks=False)
            if stat.S_ISLNK(info.st_mode):
                raise Refused("a symbolic link is in the source: %s" % rel)
            if stat.S_ISDIR(info.st_mode):
                walk(entry.path, rel + "/")
            elif stat.S_ISREG(info.st_mode):
                if info.st_nlink != 1:
                    raise Refused("a hard-linked file is in the source: %s" % rel)
                with open(entry.path, "rb") as handle:
                    data = handle.read()
                found[rel] = ("100755" if info.st_mode & stat.S_IXUSR else "100644", blob_id(data))
            else:
                raise Refused("a special file is in the source: %s" % rel)

    walk(src, "")
    return found


def tree_id(manifest):
    """The Git tree id of a manifest: what `git rev-parse <commit>^{tree}` prints."""
    root = {}
    for rel, value in manifest.items():
        node = root
        parts = rel.split("/")
        for part in parts[:-1]:
            node = node.setdefault(part, {})
        node[parts[-1]] = value

    def write(node):
        items = []
        for name, value in node.items():
            raw = os.fsencode(name)
            if isinstance(value, dict):
                items.append((raw + b"/", b"40000", raw, write(value)))
            else:
                items.append((raw, value[0].encode("ascii"), raw, bytes.fromhex(value[1])))
        items.sort(key=lambda item: item[0])
        body = b"".join(mode + b" " + raw + b"\0" + object_id for _key, mode, raw, object_id in items)
        return hashlib.sha1(b"tree %d\0" % len(body) + body).digest()

    return write(root).hex()


def manifest_text(manifest):
    return "".join("%s blob %s\t%s\n" % (mode, blob, rel) for rel, (mode, blob) in sorted(manifest.items()))


def differences(reference, seen, allowed=None):
    """How the source differs from the candidate, beyond the one declared change."""
    allowed = allowed or {}
    out = []
    for rel in sorted(set(reference) | set(seen)):
        if rel not in seen:
            out.append("missing: " + rel)
        elif rel not in reference:
            out.append("added: " + rel)
        elif rel in allowed:
            if seen[rel] != (reference[rel][0], allowed[rel]):
                out.append("not the declared mutant: " + rel)
        elif seen[rel] != reference[rel]:
            out.append("changed: " + rel)
    for rel in allowed:
        if rel not in seen:
            out.append("the mutated file is missing: " + rel)
    return out


def identity(src):
    """The manifest of src, after checking it is the candidate's tree."""
    manifest = scan(src)
    got = tree_id(manifest)
    if got != TREE:
        raise Refused("NOT THE CANDIDATE: the files make tree %s, not %s" % (got, TREE))
    for rel, want in CLEAN.items():
        if sha256_file(os.path.join(src, rel)) != want:
            raise Refused("NOT THE CANDIDATE: %s" % rel)
    return manifest


# ===========================================================================
# Reading a log. Fixed rules; nothing here is a judgement.
# ===========================================================================
RE_TARGET = re.compile(r"^\s+Running (.+) \((.+)\)$")
RE_DOCTEST = re.compile(r"^\s+Doc-tests (.+)$")
RE_EXECUTABLE = re.compile(r"^\s+Executable (.+) \((.+)\)$")
RE_COMPILING = re.compile(r"^\s+Compiling nexus-governed-control v\S+ \(.+\)$", re.M)
RE_COULD_NOT_COMPILE = re.compile(r"^error: could not compile `nexus-governed-control`", re.M)
RE_RUNNING = re.compile(r"^running (\d+) tests?$")
RE_RESULT = re.compile(r"^test (\S+) \.\.\. (ok|FAILED|ignored)(?:, .*)?$")
RE_SUMMARY = re.compile(
    r"^test result: (ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored; (\d+) measured; "
    r"(\d+) filtered out; finished in .+$"
)
RE_BLOCK = re.compile(r"^---- (\S+) stdout ----$")
RE_PANIC = re.compile(r"^thread '(.*)' (?:\(\d+\) )?panicked at (.+):(\d+):(\d+):$")
RE_ERROR = re.compile(r"^error(\[E\d+\])?: (?!could not compile )", re.M)
RE_SPAN = re.compile(r"^\s*--> crates/nexus-governed-control/", re.M)
RE_QUOTED_SOURCE = re.compile(r"^\s*\d*\s*[|=]")
NOT_A_REFUSAL = ("process didn't exit successfully", "(signal:", "internal compiler error",
                 "unexpectedly panicked", "linking with", "No space left on device", "out of memory",
                 "failed to write", "could not write", "Input/output error", "Permission denied")
RE_FRAME = re.compile(r"^\s+at (.+?):(\d+)(?::\d+)?$")


def read_test_log(text):
    """What a test log says, line by line. Lines inside a failure block are only that block's."""
    log = {"targets": [], "running": [], "results": [], "summaries": [], "blocks": []}
    block = None
    for line in text.split("\n"):
        line = line.rstrip("\r")
        match = RE_BLOCK.match(line)
        if match:
            block = {"test": match.group(1), "lines": []}
            log["blocks"].append(block)
            continue
        if block is not None:
            if line == "failures:":
                block = None
            else:
                block["lines"].append(line)
            continue
        match = RE_TARGET.match(line)
        if match:
            log["targets"].append((match.group(1), match.group(2)))
            continue
        match = RE_DOCTEST.match(line)
        if match:
            log["targets"].append(("Doc-tests " + match.group(1), ""))
            continue
        match = RE_RUNNING.match(line)
        if match:
            log["running"].append(int(match.group(1)))
            continue
        match = RE_RESULT.match(line)
        if match:
            log["results"].append((match.group(1), match.group(2)))
            continue
        match = RE_SUMMARY.match(line)
        if match:
            log["summaries"].append({
                "result": match.group(1), "passed": int(match.group(2)), "failed": int(match.group(3)),
                "ignored": int(match.group(4)), "measured": int(match.group(5)),
                "filtered_out": int(match.group(6)), "line": line,
            })
    return log


def panics_of(lines):
    """The panics printed in one failure block, each with its own message and backtrace."""
    panics = []
    current = None
    part = None
    for line in lines:
        match = RE_PANIC.match(line)
        if match:
            current = {"thread": match.group(1), "file": match.group(2), "line": int(match.group(3)),
                       "column": int(match.group(4)), "message": [], "frames": []}
            panics.append(current)
            part = "message"
            continue
        if current is None:
            continue
        if line == "stack backtrace:":
            part = "frames"
        elif line.startswith("note: "):
            part = None
        elif part == "message":
            current["message"].append(line)
        elif part == "frames":
            match = RE_FRAME.match(line)
            if match:
                current["frames"].append((match.group(1), int(match.group(2))))
    for panic in panics:
        panic["message"] = "\n".join(panic["message"]).strip("\n") + "\n"
    return panics


def same_file(printed, rel):
    """A panic names its file from the workspace's root."""
    printed = printed.replace("\\", "/")
    while printed.startswith("./"):
        printed = printed[2:]
    return printed == rel or printed.endswith("/" + rel)


def same_file_in_a_frame(printed, rel):
    """A backtrace frame names it from the workspace's root or from the crate's."""
    from_crate = rel[len(CRATE):] if rel.startswith(CRATE) else rel
    return same_file(printed, rel) or same_file(printed, from_crate)


def examine(log, exit_code, filtered_out=None):
    """Is this the whole, unambiguous result of the sixteen tests?

    Returns {"state": OK | EXECUTION FAILURE | INCOMPLETE OUTPUT | CONFLICTING OUTPUT,
             "problems": [...], "status": {short: ok|FAILED|ignored}, "filtered_out": n}
    """
    incomplete, conflict = [], []
    if not log["targets"]:
        incomplete.append("no test target was run")
    elif len(log["targets"]) != 1 or log["targets"][0][0] != TARGET_LABEL:
        conflict.append("test targets: %s" % ", ".join(label for label, _path in log["targets"]))
    if not log["running"]:
        incomplete.append("no 'running N tests' line")
    elif log["running"] != [len(INVENTORY)]:
        conflict.append("'running' lines: %s" % log["running"])
    status = {}
    names = [name for name, _state in log["results"]]
    for short, full in INVENTORY.items():
        states = [state for name, state in log["results"] if name == full]
        if not states:
            incomplete.append("no result for %s" % short)
        elif len(states) > 1:
            conflict.append("%d results for %s" % (len(states), short))
        else:
            status[short] = states[0]
    for name in sorted(set(names) - set(INVENTORY.values())):
        conflict.append("a result for a test outside the inventory: %s" % name)
    for short, state in status.items():
        if (state == "ignored") != (short == IGNORED):
            conflict.append("%s is %s" % (short, state))
    summary = None
    if not log["summaries"]:
        incomplete.append("no 'test result' line")
    elif len(log["summaries"]) > 1:
        conflict.append("%d 'test result' lines" % len(log["summaries"]))
    else:
        summary = log["summaries"][0]
    failed = sorted(short for short, state in status.items() if state == "FAILED")
    if summary and not incomplete:
        counted = (
            sum(1 for state in status.values() if state == "ok"),
            len(failed),
            sum(1 for state in status.values() if state == "ignored"),
        )
        if (summary["passed"], summary["failed"], summary["ignored"]) != counted or summary["measured"]:
            conflict.append("the summary's counts are not the results': %s" % summary["line"])
        if (summary["result"] == "FAILED") != bool(failed):
            conflict.append("the summary says %s" % summary["result"])
        if filtered_out is not None and summary["filtered_out"] != filtered_out:
            conflict.append("%d filtered out, not the baseline's %d" % (summary["filtered_out"], filtered_out))
    blocks = [block["test"] for block in log["blocks"]]
    for short in failed:
        count = blocks.count(INVENTORY[short])
        if count == 0:
            incomplete.append("no failure output for %s" % short)
        elif count > 1:
            conflict.append("%d failure outputs for %s" % (count, short))
    for name in sorted(set(blocks)):
        if SHORT.get(name) not in failed:
            conflict.append("failure output for a test that did not fail: %s" % name)
    if summary and not incomplete and not conflict:
        if exit_code == 0 and failed:
            conflict.append("exit status 0 with failed tests")
        elif exit_code == 101 and not failed:
            conflict.append("exit status 101 with no failed test")
        elif exit_code not in (0, 101):
            conflict.append("exit status %s" % exit_code)
    if exit_code != 0 and summary is None:
        state = "EXECUTION FAILURE"
    elif conflict:
        state = "CONFLICTING OUTPUT"
    elif incomplete:
        state = "INCOMPLETE OUTPUT"
    else:
        state = "OK"
    return {"state": state, "problems": incomplete + conflict, "status": status,
            "filtered_out": summary["filtered_out"] if summary else None}


def at_the_place(panic, oracle):
    """Is this panic the named statement itself, and not something inside it?

    An assertion reports the line and column where its macro begins. A panic
    raised by an expression inside the assertion (an unwrap, an index) reports
    another column, or another line, and is not the assertion. A fault (how =
    "fault") is a panic in a named line of code, at any column.
    """
    if not same_file(panic["file"], oracle["file"]):
        return False
    if oracle["how"] == "fault":
        return oracle["lo"] <= panic["line"] <= oracle["hi"]
    return panic["line"] == oracle["lo"] and panic["column"] == oracle["column"]


# Sixteen tests are selected and reported; fifteen run; T10 is ignored and does not.
SURVIVED = "SURVIVED (15 rs01_ tests run, T10 not)"


def judge(mutant, oracles, log, exam):
    """The verdict of a mutant whose test output is whole and unambiguous."""
    failed = sorted(short for short, state in exam["status"].items() if state == "FAILED")
    blocks = {block["test"]: panics_of(block["lines"]) for block in log["blocks"]}
    observed = {}
    for short in failed:
        full = INVENTORY[short]
        observed[short] = [
            {"thread": "the test's own" if panic["thread"] == full else panic["thread"],
             "at": "%s:%d:%d" % (panic["file"], panic["line"], panic["column"]),
             "message": panic["message"][:600]}
            for panic in blocks.get(full, [])
        ]
    expected = []
    for oracle in oracles:
        full = INVENTORY[oracle["test"]]
        entry = {"test": oracle["test"],
                 "at": "%s:%d" % (oracle["file"], oracle["lo"]) + (
                     "-%d" % oracle["hi"] if oracle["how"] == "fault" else ":%d" % oracle["column"]),
                 "text": oracle["text"], "how": oracle["how"], "says": oracle["says"],
                 "test_failed": oracle["test"] in failed, "met": False, "part": ""}
        # The panic that ended the test: the last one of the test's own thread.
        own = [panic for panic in blocks.get(full, []) if panic["thread"] == full]
        last = own[-1] if (own and entry["test_failed"]) else None
        if last and at_the_place(last, oracle) and all(text in last["message"] for text in oracle["text"]):
            entry["met"] = True
            entry["panic"] = "%s:%d:%d" % (last["file"], last["line"], last["column"])
            if oracle["via"]:
                through = any(same_file_in_a_frame(file, oracle["via"]["file"])
                              and oracle["via"]["lo"] <= line <= oracle["via"]["hi"]
                              for file, line in last["frames"])
                entry["part"] = ("confirmed by the backtrace: %s:%d-%d" % (
                    oracle["via"]["file"], oracle["via"]["lo"], oracle["via"]["hi"])
                    if through else "NOT CONFIRMED by the backtrace")
        expected.append(entry)
    met = [entry for entry in expected if entry["met"]]
    if expected and len(met) == len(expected):
        verdict = "KILLED" if all(not entry["part"].startswith("NOT") for entry in met) \
            else "KILLED, PART NOT CONFIRMED"
    elif met:
        verdict = "KILLED IN PART"
    elif any(entry["test_failed"] for entry in expected):
        verdict = "EXPECTED TEST FAILED, ORACLE NOT MET"
    elif failed:
        verdict = "FAILED, AWAITING ATTRIBUTION"
    else:
        verdict = SURVIVED
    return {
        "verdict": verdict,
        "failed": failed,
        "expected": expected,
        "other_failed": [short for short in failed if short not in [e["test"] for e in expected]],
        "uncredited": {short: why for short, why in mutant["uncredited"].items() if short in failed},
        "observed": observed,
    }


def examine_build(text, exit_code):
    """The build step: OK with its test executable, or what kind of failure.

    BUILD ERROR means the compiler read the crate and refused it: it printed a
    diagnostic of its own that points into the crate's source. A compiler that
    was killed, that crashed or could not write, or a link that failed, is an
    execution failure, not a refusal.
    """
    executables = []
    for line in text.split("\n"):
        match = RE_EXECUTABLE.match(line.rstrip("\r"))
        if match:
            executables.append((match.group(1), match.group(2)))
    if exit_code == 0:
        if len(executables) == 1 and executables[0][0] == TARGET_LABEL:
            return {"state": "OK", "executable": executables[0][1]}
        return {"state": "INCOMPLETE OUTPUT", "problems": ["executables: %s" % executables]}
    said = "\n".join(line for line in text.split("\n") if not RE_QUOTED_SOURCE.match(line))
    if (RE_COULD_NOT_COMPILE.search(text) and RE_ERROR.search(text) and RE_SPAN.search(text)
            and not any(mark in said for mark in NOT_A_REFUSAL)):
        return {"state": "BUILD ERROR"}
    return {"state": "EXECUTION FAILURE", "problems": ["the build ended with status %s and not with a "
                                                        "compiler diagnostic for the crate alone" % exit_code]}


# ===========================================================================
# The runner's own checks (MA-01; B1, B2, B4 and section 5 of REVIEW-R2).
# Made-up output and made-up folders; nothing is built.
# ===========================================================================
def fake_panic(thread, file, line, message, frames=(), column=5):
    lines = ["", "thread '%s' (4242) panicked at %s:%d:%d:" % (thread, file, line, column), message,
             "stack backtrace:", "   0: __rustc::rust_begin_unwind"]
    for index, (frame_file, frame_line) in enumerate(frames, 1):
        lines += ["   %d: frame" % index, "             at ./%s:%d:9" % (frame_file, frame_line)]
    lines.append("note: Some details are omitted, run with `RUST_BACKTRACE=full` for a verbose backtrace.")
    return "\n".join(lines)


def fake_log(states=None, blocks=(), drop=(), twice=(), summary="auto", target=TARGET_LABEL, running=16,
             filtered=197):
    given = states or {}
    states = {short: "ok" for short in INVENTORY}
    states[IGNORED] = "ignored"
    states.update(given)
    lines = ["    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.10s"]
    if target:
        lines.append("     Running %s (/w/target/debug/deps/nexus_governed_control-0123456789abcdef)" % target)
    lines += ["", "running %d tests" % running]
    for short, state in states.items():
        if short in drop:
            continue
        for _ in range(2 if short in twice else 1):
            lines.append("test %s ... %s" % (INVENTORY[short], state))
    failed = [short for short, state in states.items() if state == "FAILED"]
    if failed:
        lines += ["", "failures:", ""]
        for short, body in blocks:
            lines += ["---- %s stdout ----" % INVENTORY[short], body, ""]
        lines += ["", "failures:"] + ["    " + INVENTORY[short] for short in failed] + [""]
    if summary == "auto":
        summary = "test result: %s. %d passed; %d failed; %d ignored; 0 measured; %d filtered out; finished in 0.50s" % (
            "FAILED" if failed else "ok", sum(1 for s in states.values() if s == "ok"), len(failed),
            sum(1 for s in states.values() if s == "ignored"), filtered)
    if summary:
        lines += [summary, ""]
    return "\n".join(lines)


def fake_report(evidence, final=True, cleanup=(), not_run=()):
    """A made-up report of the whole plan: every control KILLED, every baseline clean."""
    cases = []
    for case in plan_of(None):
        entry = {key: case[key] for key in case if key != "edits"}
        if case["id"] in not_run:
            entry["verdict"] = "NOT_RUN"
        elif case["type"] == "baseline":
            entry["verdict"] = "BASELINE CLEAN"
        elif case["build_error"]:
            entry["verdict"] = "BUILD ERROR, AS EXPECTED (not a kill)"
        elif case["expect"]:
            entry["verdict"] = "KILLED"
        else:
            entry["verdict"] = SURVIVED
        if case["id"] in cleanup:
            entry["cleanup"] = [{"step": "test", "lifecycle": NORMAL_LEFT, "alive_before_cleanup": 1,
                                 "signals_sent": 1}]
        cases.append(entry)
    return {"reference": REFERENCE, "state": "FINAL" if final else "IN PROGRESS", "script_sha256": "0" * 64,
            "launcher": {"sha256": "0" * 64}, "candidate": CANDIDATE, "tree": TREE, "files_in_tree": 0,
            "commands": {"build": " ".join(BUILD), "test": " ".join(TEST)}, "started": "-", "finished": "-",
            "seconds_used": 0, "limits": {"overall_budget_s": OVERALL_BUDGET_S},
            "ended": EVERY_CASE if (final and not not_run) else "", "cases": cases, "evidence": evidence}


def made_up_campaign(folder):
    """A campaign whose every case has ended, over made-up folders.

    It is a real Campaign object, so that the last checks, the final receipts
    and the exit status are the runner's own code, run on made-up evidence.
    """
    campaign = Campaign.__new__(Campaign)
    campaign.receipt = os.path.join(folder, "receipt")
    campaign.src = os.path.join(folder, "src")
    campaign.cargo_home = os.path.join(folder, "cache")
    os.makedirs(os.path.join(campaign.receipt, "logs"))
    os.makedirs(os.path.join(campaign.src, "a"))
    os.makedirs(os.path.join(campaign.cargo_home, "registry", "cache"))
    with open(os.path.join(campaign.src, "a", "b.rs"), "wb") as handle:
        handle.write(b"fn main() {}\n")
    with open(os.path.join(campaign.cargo_home, "registry", "cache", "a-1.0.0.crate"), "wb") as handle:
        handle.write(b"a crate file")
    campaign.reference = scan(campaign.src)
    campaign.inputs_sealed = dependency_inputs(campaign.cargo_home)
    campaign.inputs_alarm = []
    campaign.containment_alarm = []
    campaign.core_pattern = None
    campaign.clock = time.monotonic
    campaign.tool_logs = []
    campaign.cases = plan_of(None)
    campaign.results = {}
    made_up = fake_report(None)
    for entry in made_up["cases"]:
        log = "%s.test.log" % entry["name"]
        with open(os.path.join(campaign.receipt, "logs", log), "wb") as handle:
            handle.write(b"as it was read\n")
        entry["steps"] = {"test": {"log": log, "log_sha256": sha256(b"as it was read\n"), "seconds": 0.0,
                                   "cleanup_required": False}}
        entry["restored"] = True
        campaign.results[entry["id"]] = entry
    campaign.header = {key: made_up[key] for key in made_up
                       if key not in ("cases", "evidence", "state", "ended", "finished", "seconds_used")}
    campaign.started = time.monotonic()
    campaign.ended = EVERY_CASE
    return campaign


def selftest(scratch=None):
    """[(name, passed, what was seen)]. Every one must pass before a campaign starts.

    scratch: a folder in which the checks may make their own folder of made-up
    files, which they remove. None: the system's place for temporary files.
    """
    oracle = {"test": "T1", "file": TESTS, "lo": 2131, "hi": 2134, "column": 5,
              "text": ["the newer stop was lifted:"], "how": "assertion", "says": "", "via": None}
    with_via = dict(oracle, via={"file": TESTS, "lo": 2130, "hi": 2130})
    fault = dict(oracle, file=RUN, lo=246, hi=246, column=9, text=["runs: PoisonError"], how="fault")
    with_claim = {"uncredited": {"T2": "ruled"}}
    no_claim = {"uncredited": {}}
    t1, t2 = INVENTORY["T1"], INVENTORY["T2"]
    right = fake_panic(t1, TESTS, 2131, "the newer stop was lifted: Ok(())", [(TESTS, 2130)])
    wait = fake_panic(t2, AUTH + "scripted.rs", 75, "the record was never held")
    checks = []

    def verdict(text, exit_code, mutant=with_claim, oracles=(oracle,), filtered=197):
        log = read_test_log(text)
        exam = examine(log, exit_code, filtered)
        if exam["state"] != "OK":
            return exam["state"], exam
        result = judge(mutant, list(oracles), log, exam)
        return result["verdict"], result

    def failing(body, short="T1"):
        return fake_log({short: "FAILED"}, [(short, body)])

    def check(name, got, want):
        checks.append((name, got == want, "%s" % (got,)))

    got, _ = verdict(fake_log(), 0)
    check("01 every test passes, exit 0: survived, and only then", got, SURVIVED)
    got, _ = verdict(failing(right), 101)
    check("02 the named test fails at the named assertion: killed", got, "KILLED")
    got, result = verdict(failing(right), 101, with_claim, (with_via,))
    check("03 the part is read from that panic's own backtrace",
          (got, result["expected"][0]["part"][:9]), ("KILLED", "confirmed"))
    got, _ = verdict(fake_log(drop=("T7",)), 0)
    check("04 a test's result is missing (incomplete output): not survived", got, "INCOMPLETE OUTPUT")
    got, _ = verdict(fake_log(summary=None), 0)
    check("05 no summary line, exit 0: not survived", got, "INCOMPLETE OUTPUT")
    got, _ = verdict(fake_log({"T2": "FAILED"}, [("T2", wait)]), 101)
    check("06 another test fails, the named one passes (wrong test): no kill", got, "FAILED, AWAITING ATTRIBUTION")
    wrong_line = fake_panic(t1, TESTS, 2139, "assertion `left == right` failed\n  left: 0\n right: 1")
    got, _ = verdict(failing(wrong_line), 101)
    check("07 the named test fails at another assertion (wrong cause): no kill", got,
          "EXPECTED TEST FAILED, ORACLE NOT MET")
    got, _ = verdict(fake_log(), 101)
    check("08 every test passes but exit 101 (conflicting status): not survived", got, "CONFLICTING OUTPUT")
    got, _ = verdict(failing(right), 0)
    check("09 a test failed but exit 0 (conflicting status): no kill", got, "CONFLICTING OUTPUT")
    got, _ = verdict(fake_log(twice=("T3",)), 0)
    check("10 a test's result appears twice (duplicate name): not survived", got, "CONFLICTING OUTPUT")
    no_text = fake_panic(t1, TESTS, 2131, "assertion failed")
    elsewhere = fake_panic(t2, TESTS, 2171, "the newer stop was lifted: Ok(())")
    got, _ = verdict(fake_log({"T1": "FAILED", "T2": "FAILED"}, [("T1", no_text), ("T2", elsewhere)]), 101)
    check("11 the required text is in another test's block: no kill", got, "EXPECTED TEST FAILED, ORACLE NOT MET")
    empty = "\n".join(["     Running %s (/w/x)" % TARGET_LABEL, "", "running 0 tests", "",
                       "test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 213 filtered out; finished in 0.00s"])
    got, _ = verdict(empty, 101)
    check("12 a summary of an empty suite, command failed: not survived", got, "CONFLICTING OUTPUT")
    got, _ = verdict(empty, 0)
    check("13 a summary of an empty suite, exit 0: not survived", got, "CONFLICTING OUTPUT")
    got, _ = verdict("error: test failed, to rerun pass `-p nexus-governed-control --lib`\n", 101)
    check("14 the command failed and printed no result: execution failure", got, "EXECUTION FAILURE")
    got, _ = verdict(fake_log({"T1": "FAILED"}, [("T1", right)], summary=None), 101)
    check("15 the test program died before its summary: execution failure, no kill", got, "EXECUTION FAILURE")
    other_thread = fake_panic("<unnamed>", TESTS, 2131, "the newer stop was lifted: Ok(())")
    got, _ = verdict(failing(other_thread), 101)
    check("16 the panic at the named line is another thread's: no kill", got, "EXPECTED TEST FAILED, ORACLE NOT MET")
    got, _ = verdict(fake_log({IGNORED: "ok"}), 0)
    check("17 the ignored timing test ran: not survived", got, "CONFLICTING OUTPUT")
    got, _ = verdict(fake_log(target="tests/egress_ignores_proxy_environment.rs"), 0)
    check("18 another test target (wrong target identity): not survived", got, "CONFLICTING OUTPUT")
    got, _ = verdict(failing(right), 101, no_claim, ())
    check("19 no failure named in advance, a test fails: awaiting attribution", got, "FAILED, AWAITING ATTRIBUTION")
    both = fake_log({"T1": "FAILED", "T2": "FAILED"}, [("T1", right), ("T2", wait)])
    got, result = verdict(both, 101)
    check("20 row 13: T1 kills, T2's wait failure is listed and earns nothing",
          (got, result["other_failed"], sorted(result["uncredited"])), ("KILLED", ["T2"], ["T2"]))
    second = dict(oracle, test="T2", lo=2171, hi=2174)
    got, _ = verdict(both, 101, with_claim, (oracle, second))
    check("21 two failures named, one met: killed in part", got, "KILLED IN PART")
    got, _ = verdict(fake_log(filtered=196), 0)
    check("22 a different number of tests filtered out than the baseline: not survived", got, "CONFLICTING OUTPUT")
    got, _ = verdict(fake_log({"T1": "FAILED"}, []), 101)
    check("23 a failed test with no failure output: no kill", got, "INCOMPLETE OUTPUT")
    inside = fake_panic(t1, TESTS, 2131, "called `Result::unwrap()` on an `Err` value: Any { .. }\n"
                                         "the newer stop was lifted: x", column=27)
    got, _ = verdict(failing(inside), 101)
    check("24 a panic inside the named statement (an unwrap, another column): no kill", got,
          "EXPECTED TEST FAILED, ORACLE NOT MET")
    later_line = fake_panic(t1, TESTS, 2132, "index out of bounds\nthe newer stop was lifted: x")
    got, _ = verdict(failing(later_line), 101)
    check("25 a panic on a later line of the named statement: no kill", got, "EXPECTED TEST FAILED, ORACLE NOT MET")
    got, _ = verdict(failing(right + "\n" + wrong_line), 101)
    check("26 the right panic is followed by another of the test's own: the last one counts, no kill", got,
          "EXPECTED TEST FAILED, ORACLE NOT MET")
    wrong_way = fake_panic(t1, TESTS, 2131, "the newer stop was lifted: Ok(())", [(TESTS, 2899)])
    got, _ = verdict(failing(wrong_way), 101, with_claim, (with_via,))
    check("27 the failure came through another statement than the one named: not a plain kill", got,
          "KILLED, PART NOT CONFIRMED")
    in_code = fake_panic(t1, RUN, 246, "runs: PoisonError { .. }", column=41)
    got, _ = verdict(failing(in_code), 101, with_claim, (fault,))
    check("28 a fault named by its line of code is met at any column", got, "KILLED")
    built = "   Compiling nexus-governed-control v0.1.0 (/s/crates/nexus-governed-control)\n" \
            "    Finished `test` profile\n  Executable unittests src/lib.rs (/w/t/nexus_governed_control-0f)\n"
    check("29 a build that succeeded names its one test executable", examine_build(built, 0)["state"], "OK")
    could_not = "error: could not compile `nexus-governed-control` (lib test) due to 1 previous error\n"
    refused = "error[E0308]: mismatched types\n  --> crates/nexus-governed-control/src/authority/run.rs:36:9\n" \
              "   |\n36 |         linking with\n" + could_not
    check("30 a compiler diagnostic pointing into the crate: build error",
          examine_build(refused, 101)["state"], "BUILD ERROR")
    check("30b the compiler could not write its output: execution failure, not build error",
          examine_build("error: failed to write /w/x.rmeta: Input/output error\n" + could_not, 101)["state"],
          "EXECUTION FAILURE")
    check("31 a failed build with no compiler diagnostic for the crate: execution failure",
          examine_build("error: no matching package named `x` found\n", 101)["state"], "EXECUTION FAILURE")
    check("32 a build that says it succeeded but names no executable: incomplete",
          examine_build("    Finished `test` profile\n", 0)["state"], "INCOMPLETE OUTPUT")
    killed = "error: could not compile `nexus-governed-control` (lib test)\n\nCaused by:\n" \
             "  process didn't exit successfully: `rustc ...` (signal: 9, SIGKILL: kill)\n"
    check("33 the compiler was killed: execution failure, not build error",
          examine_build(killed, 101)["state"], "EXECUTION FAILURE")
    crashed = "error: internal compiler error: unexpected\nthe compiler unexpectedly panicked. this is a bug.\n" + could_not
    check("34 the compiler crashed: execution failure, not build error",
          examine_build(crashed, 101)["state"], "EXECUTION FAILURE")
    check("35 the link failed: execution failure, not build error",
          examine_build("error: linking with `cc` failed: exit status: 1\n" + could_not, 101)["state"],
          "EXECUTION FAILURE")
    sample = {"a/b.txt": ("100644", blob_id(b"x\n")), "a/c": ("100755", blob_id(b"y\n")), "d": ("100644", blob_id(b""))}
    check("36 the tree id of a known small tree", tree_id(sample), "244d535ae1dd1b2b84e1390713a7d6ea2d9a9c27")
    changed = dict(sample, d=("100644", blob_id(b"z")))
    check("37 a change outside the declared mutation is seen",
          differences(sample, changed, {"a/b.txt": blob_id(b"x\n")}), ["changed: d"])

    # ---- how a command's life ended, and what may be read of it (section 5) ----
    one = [{"pid": 7}]

    def life(ended_by, own_status, alive, quiet=True):
        lifecycle = lifecycle_of(ended_by, own_status, alive, quiet)
        step = {"lifecycle": lifecycle, "ended_by": ended_by,
                "exit": own_status if lifecycle in (NORMAL, NORMAL_LEFT) else None}
        return lifecycle, step_no_result(step, "test")

    check("38 the command ends by itself, its output ends, nothing is left: its output is read",
          life("exit", 101, []), (NORMAL, None))
    check("39 the same, but a process it started is still alive: read, and marked for cleanup",
          life("exit", 0, one), (NORMAL_LEFT, None))
    check("40 the command ended but its output is held open: no result",
          life("output held open", 0, one), (HELD_OPEN, "OUTPUT HELD OPEN (test)"))
    check("41 the step's own time limit: no result", life("timeout", None, one), (TIMED_OUT, "TIMEOUT (test)"))
    check("42 the campaign's deadline cut the step: no result",
          life("campaign deadline", None, one), (TIMED_OUT, "CAMPAIGN DEADLINE (test)"))
    check("43 the log limit: the command is ended by the runner, no result",
          life("log limit", None, one), (FORCED, "LOG LIMIT (test)"))
    check("44 it closed its output and did not end: ended by the runner, no result",
          life("it closed its output and did not end", None, one), (FORCED, "EXECUTION FAILURE (test)"))
    check("45 the command had ended by itself when the log limit was reached: still no result",
          life("log limit", 0, []), (FORCED, "LOG LIMIT (test)"))
    check("46 the command died of a signal the runner did not send: no result",
          life("exit", -6, []), (OWN_SIGNAL, "EXECUTION FAILURE (test)"))
    check("47 a process could not be ended: no result (and the case is quarantined)",
          life("exit", 0, [], quiet=False), (UNRESOLVED, "EXECUTION FAILURE (test)"))
    check("48 a step gets the lesser of its own limit and the time the campaign has left",
          (allowed_time(600, 400 + CASE_CLOSE_S), allowed_time(600, 5000), allowed_time(600, CASE_CLOSE_S)),
          ((400, "campaign deadline"), (600, "timeout"), (0, "campaign deadline")))

    # ---- the place: mounts (B3) -----------------------------------------------
    def mount(at, options, kind="ext4", sup="rw"):
        return {"at": at, "options": options.split(","), "type": kind, "super": sup.split(",")}

    view = [mount("/", "ro,nosuid", "tmpfs", "ro"), mount("/usr", "ro,nosuid"), mount("/proc", "rw", "proc"),
            mount("/audit/evidence", "rw,nosuid"), mount("/audit/build", "rw,nosuid")]
    audit = ["/audit/build", "/audit/evidence"]
    check("49 the launcher's view: nothing is writable but the audit's filesystems and /proc",
          writable_mounts(view, audit), [])
    check("50 a writable mount outside the audit folders is seen",
          writable_mounts(view + [mount("/tmp", "rw,nosuid", "tmpfs")], audit), ["/tmp"])
    check("51 something writable mounted at /proc that is not proc is seen",
          writable_mounts([mount("/proc", "rw", "tmpfs")], audit), ["/proc"])
    check("52 an audit folder is found on its own mount, not on the root",
          (mount_of("/audit/build/work", view), mount_of("/home/x", view)), ("/audit/build", "/"))

    # ---- the evidence itself (B1) and the dependency cache (B2), on made-up folders
    whole = {name: {"result": UNCHANGED, "detail": []} for name in EVIDENCE_CHECKS}
    credited = "ALL %d ROWS KILLED AS SPECIFIED" % len(ROWS)
    control = next(mutant for mutant in MUTANTS if mutant["kind"] == "control")  # the plan's first control
    last = MUTANTS[-1]  # and its last case

    def outcome(changed=None, **more):
        results = dict(whole)
        results.update(changed or {})
        report = fake_report(evidence_of(results), **more)
        said = summary_text(report)
        return report["evidence"]["state"], credited in said, "NO CREDIT" in said, exit_status(report)

    check("53 a complete campaign whose evidence is whole: credited, exit status 0",
          outcome(), (VALID, True, False, EXIT_OK))
    check("54 the same evidence, but a case was not run: no credit line, exit status 10",
          outcome(not_run=(last["id"],)), (VALID, False, False, EXIT_NOT_EVERY_CASE))
    in_progress = fake_report({"state": NOT_YET, "reasons": [], "checks": {}}, final=False)
    check("55 a campaign in progress credits nothing",
          (credited in summary_text(in_progress), "REQUIRED CONTROLS: NOT DECIDED" in summary_text(in_progress)),
          (False, True))
    marked = summary_text(fake_report(evidence_of(whole), cleanup=(control["id"],)))
    check("56 a control that needed cleanup is marked: at its verdict, in its row, under the credit line",
          ("%s=KILLED [cleanup]" % control["id"] in marked, "row %2d  yes* " % control["row"] in marked,
           "* QUALIFIED: rows %d hold" % control["row"] in marked), (True, True, True))
    check("57 a survivor is said to have survived the fifteen tests that ran, T10 not among them",
          ("15" in SURVIVED and "T10 not" in SURVIVED, "the fifteen rs01_ tests that were run" in marked),
          (True, True))

    folder = tempfile.mkdtemp(prefix="rs01-audit-selftest-", dir=scratch)
    try:
        logs = os.path.join(folder, "logs")
        os.makedirs(logs)
        a_log = os.path.join(logs, "a.log")
        with open(a_log, "wb") as handle:
            handle.write(b"as it was read\n")
        recorded = [("a.log", sha256(b"as it was read\n"))]
        seen = checked(lambda: (None, changed_logs(logs, recorded)))
        check("58 a raw log that is still the bytes that were read: unchanged", seen["result"], UNCHANGED)
        with open(a_log, "ab") as handle:
            handle.write(b"added later\n")
        seen = checked(lambda: (None, changed_logs(logs, recorded)))
        check("59 a raw log changed after it was read: seen", seen["result"], CHANGED)
        check("60 an otherwise complete campaign with a changed raw log: INVALID, no credit, exit status 30",
              outcome({"raw logs": seen}), (INVALID, False, True, EXIT_EVIDENCE))
        os.unlink(a_log)
        check("61 a raw log that is missing: seen",
              checked(lambda: (None, changed_logs(logs, recorded)))["result"], CHANGED)

        home = os.path.join(folder, "cargo-home")
        crates = os.path.join(home, "registry", "cache", "index.crates.io-0000000000000000")
        os.makedirs(crates)
        os.makedirs(os.path.join(home, "registry", "index", "index.crates.io-0000000000000000", ".cache"))
        crate = os.path.join(crates, "a-1.0.0.crate")
        with open(crate, "wb") as handle:
            handle.write(b"a crate file")
        lock = '[[package]]\nname = "a"\nversion = "1.0.0"\nchecksum = "%s"\n' % sha256(b"a crate file")

        def refused_at_the_start():
            try:
                check_cache(home, lock)
            except Refused as problem:
                return str(problem)[:40]
            return "accepted"

        def at_the_end(sealed):
            return checked(lambda: (None, changed_inputs(sealed, dependency_inputs(home))))

        check("62 a cache of crate files that Cargo.lock names, and an index: accepted",
              refused_at_the_start(), "accepted")
        sealed, top = dependency_inputs(home), cache_top(home)
        check("63 the same cache, looked at again: unchanged",
              (at_the_end(sealed)["result"], cache_top(home) == top, cache_top_problems(top)),
              (UNCHANGED, True, []))
        config = os.path.join(home, "config.toml")
        with open(config, "wb") as handle:
            handle.write(b'[build]\nrustc-wrapper = "/x"\n')
        seen = at_the_end(sealed)
        check("64 a Cargo configuration appears in the cache: refused at the start, seen at its top "
              "before a case, seen at the end",
              (refused_at_the_start(), cache_top_problems(cache_top(home)), cache_top(home) != top, seen["result"]),
              ("THE DEPENDENCY CACHE HOLDS AN ENTRY THAT", ["config.toml (file)"], True, CHANGED))
        check("65 an otherwise complete campaign with a changed dependency input: INVALID, no credit, "
              "exit status 30", outcome({"dependency inputs": seen}), (INVALID, False, True, EXIT_EVIDENCE))
        os.unlink(config)
        check("66 with the configuration gone the cache is as it was sealed", at_the_end(sealed)["result"], UNCHANGED)
        with open(crate, "wb") as handle:
            handle.write(b"another file")
        check("67 a crate file with other bytes: refused at the start, seen at the end",
              (refused_at_the_start(), at_the_end(sealed)["result"]),
              ("A CRATE FILE WITHOUT ITS Cargo.lock CHEC", CHANGED))
        with open(crate, "wb") as handle:
            handle.write(b"a crate file")
        mode = stat.S_IMODE(os.lstat(crate).st_mode)
        os.chmod(crate, mode ^ 0o100)
        check("68 a changed mode in the cache: seen", at_the_end(sealed)["result"], CHANGED)
        os.chmod(crate, mode)
        os.unlink(crate)
        os.symlink("/dev/null", crate)
        check("69 a file of the cache replaced by a link: refused at the start, seen at the end",
              (refused_at_the_start(), at_the_end(sealed)["result"]),
              ("THE DEPENDENCY CACHE HOLDS A LINK OR A S", CHANGED))
        os.unlink(crate)
        with open(crate, "wb") as handle:
            handle.write(b"a crate file")
        os.chmod(crate, mode)
        stray = os.path.join(home, "registry", "src", "index.crates.io-0000000000000000", "a-1.0.0")
        os.makedirs(stray)
        with open(os.path.join(stray, "lib.rs"), "wb") as handle:
            handle.write(b"")
        check("70 sources already unpacked: refused at the start, seen at the end",
              (refused_at_the_start(), at_the_end(sealed)["result"]),
              ("THE DEPENDENCY CACHE HOLDS registry/src ", CHANGED))
        shutil.rmtree(os.path.join(home, "registry", "src"))
        check("71 the cache is again as it was sealed", (refused_at_the_start(), at_the_end(sealed)["result"]),
              ("accepted", UNCHANGED))

        def cannot():
            raise OSError(5, "Input/output error")

        def too_late():
            raise TimeoutError("the time kept for the last checks was used up")

        seen = checked(cannot)
        check("72 a check that could not be completed is not a check that passed", seen["result"], COULD_NOT)
        check("73 an otherwise complete campaign with a check that could not be completed: NOT VERIFIED, "
              "no credit, exit status 30", outcome({"source": seen}), (NOT_VERIFIED, False, True, EXIT_EVIDENCE))
        with open(a_log, "wb") as handle:
            handle.write(b"as it was read\n")
        seen = checked(lambda: (None, changed_logs(logs, recorded, too_late)))
        check("74 a check that ran out of its time: NOT VERIFIED, no credit, exit status 30",
              (seen["result"],) + outcome({"raw logs": seen}), (COULD_NOT, NOT_VERIFIED, False, True, EXIT_EVIDENCE))
        seen = checked(lambda: ("the campaign was ended by a signal", []))
        check("75 a check that was not made: NOT VERIFIED, no credit, exit status 30",
              (seen["result"],) + outcome({"dependency inputs": seen}), (NOT_MADE, NOT_VERIFIED, False, True, EXIT_EVIDENCE))
        check("76 a source that is not the candidate at the end: INVALID, no credit, exit status 30",
              outcome({"source": {"result": CHANGED, "detail": ["changed: Cargo.lock"]}}),
              (INVALID, False, True, EXIT_EVIDENCE))
        check("77 a check that is missing altogether: NOT VERIFIED",
              evidence_of({"raw logs": whole["raw logs"], "source": whole["source"]})["state"], NOT_VERIFIED)

        # ---- the runner's own last checks and final receipts, on a made-up campaign (B1) ----
        def final(spoil=None):
            campaign = made_up_campaign(tempfile.mkdtemp(prefix="campaign-", dir=folder))
            if spoil:
                spoil(campaign)
            status = campaign.close()
            with open(os.path.join(campaign.receipt, "summary.txt"), "r", encoding="utf-8") as handle:
                said = handle.read()
            with open(os.path.join(campaign.receipt, "SHA256SUMS"), "r", encoding="utf-8") as handle:
                listed = "  progress.jsonl\n" in handle.read()
            state = campaign.report["evidence"]["state"] if hasattr(campaign, "report") else "no report"
            return state, credited in said, "NO CREDIT" in said, status, listed

        def spoil_a_log(campaign):
            log = campaign.results["S1"]["steps"]["test"]["log"]
            with open(os.path.join(campaign.receipt, "logs", log), "ab") as handle:
                handle.write(b"added later\n")

        def spoil_the_cache(campaign):
            with open(os.path.join(campaign.cargo_home, "config.toml"), "wb") as handle:
                handle.write(b"[build]\n")

        def spoil_the_source(campaign):
            with open(os.path.join(campaign.src, "a", "b.rs"), "ab") as handle:
                handle.write(b"// later\n")

        check("78 the runner's last checks and final receipts on a whole made-up campaign: VALID, credited, "
              "exit status 0, progress.jsonl in the final list", final(), (VALID, True, False, EXIT_OK, True))
        check("79 the same campaign, a raw log changed after the last case: INVALID, no credit, exit status 30",
              final(spoil_a_log), (INVALID, False, True, EXIT_EVIDENCE, True))
        check("80 the same campaign, a Cargo configuration added to the cache after the last case: INVALID, "
              "no credit, exit status 30", final(spoil_the_cache), (INVALID, False, True, EXIT_EVIDENCE, True))
        check("81 the same campaign, a source file changed after the last case: INVALID, no credit, "
              "exit status 30", final(spoil_the_source), (INVALID, False, True, EXIT_EVIDENCE, True))
        check("82 the same campaign, the cache can no longer be read: NOT VERIFIED, no credit, exit status 30",
              final(lambda campaign: shutil.rmtree(campaign.cargo_home)),
              (NOT_VERIFIED, False, True, EXIT_EVIDENCE, True))
        check("83 the same campaign, a case whose source was not restored: INVALID, no credit, exit status 30",
              final(lambda campaign: campaign.results[control["id"]].update(restored=False)),
              (INVALID, False, True, EXIT_EVIDENCE, True))
        check("84 the same campaign, the top of the cache had changed before a case: INVALID, no credit, "
              "exit status 30", final(lambda campaign: campaign.inputs_alarm.append("config.toml appeared")),
              (INVALID, False, True, EXIT_EVIDENCE, True))
        check("85 the same campaign, the cache was never sealed: NOT VERIFIED, no credit, exit status 30",
              final(lambda campaign: setattr(campaign, "inputs_sealed", None)),
              (NOT_VERIFIED, False, True, EXIT_EVIDENCE, True))
        check("86 the same campaign, a quarantined case: NOT VERIFIED, no credit, exit status 30",
              final(lambda campaign: campaign.results[last["id"]].update(verdict="QUARANTINED", restored=False)),
              (NOT_VERIFIED, False, True, EXIT_EVIDENCE, True))

        def no_final_list(campaign):
            os.makedirs(os.path.join(campaign.receipt, "SHA256SUMS.part"))

        campaign = made_up_campaign(tempfile.mkdtemp(prefix="campaign-", dir=folder))
        no_final_list(campaign)
        status = campaign.close(announce=lambda _words: None)
        with open(os.path.join(campaign.receipt, "summary.txt"), "r", encoding="utf-8") as handle:
            said = handle.read()
        check("87 a whole campaign whose final receipts cannot be written: a marker, a summary that credits "
              "nothing, exit status 30",
              (status, credited in said, "NOTHING IS CREDITED" in said,
               os.path.isfile(os.path.join(campaign.receipt, "FINAL-RECEIPTS-INCOMPLETE.txt"))),
              (EXIT_EVIDENCE, False, True, True))
        # A made-up entry, and only it is taken out again: a real signal that arrives
        # meanwhile stays on record.
        SIGNALS.append("made up by the runner's own checks")
        try:
            seen = final()
        finally:
            SIGNALS.remove("made up by the runner's own checks")
        check("88 a whole campaign ended by a signal: the last checks are not made, NOT VERIFIED, no credit, "
              "exit status 30", seen, (NOT_VERIFIED, False, True, EXIT_EVIDENCE, True))
        check("89 a signal that had to be sent means a process was left behind, whatever the listings saw",
              (lifecycle_of("exit", 0, [] or [{"signal": "SIGKILL"}], True), lifecycle_of("exit", 0, [] or [], True)),
              (NORMAL_LEFT, NORMAL))

        # ---- R3-02: the decision is made after the checks, from the clock and signals as they are then
        made_up_signal = "a signal made up by the runner's own checks"

        def a_signal_during_the_checks(campaign):
            first = campaign.recorded_logs

            def recorded_and_signalled():
                SIGNALS.append(made_up_signal)
                return first()
            campaign.recorded_logs = recorded_and_signalled

        try:
            seen = final(a_signal_during_the_checks)
        finally:
            while made_up_signal in SIGNALS:
                SIGNALS.remove(made_up_signal)
        check("90 a signal arriving while the last checks run: NOT VERIFIED, no credit, exit status 30", seen,
              (NOT_VERIFIED, False, True, EXIT_EVIDENCE, True))

        def the_last_check_ends_late(campaign):
            late = {"by": 0.0}
            campaign.clock = lambda: time.monotonic() + late["by"]

            class LateReference(dict):
                # differences() reads the reference last, after the walk of the source:
                # reading it is the end of the last check.
                def __iter__(self):
                    late["by"] = 2.0 * (OVERALL_BUDGET_S + FINAL_RESERVE_S)
                    return dict.__iter__(self)
            campaign.reference = LateReference(campaign.reference)

        check("91 the last check returns after its deadline: NOT VERIFIED, no credit, exit status 30",
              final(the_last_check_ends_late), (NOT_VERIFIED, False, True, EXIT_EVIDENCE, True))
        check("92 a decision after the checks never makes INVALID evidence VALID, nor VALID evidence on time "
              "anything else",
              (final_eligibility({"state": INVALID, "reasons": ["x"]}, 2, 1, 0, 1)["state"],
               final_eligibility({"state": VALID, "reasons": []}, 1, 2, 3, 3)["state"],
               final_eligibility({"state": VALID, "reasons": []}, 1, 2, 0, 0, ["core_pattern"])["state"]),
              (INVALID, VALID, NOT_VERIFIED))
    finally:
        shutil.rmtree(folder, ignore_errors=True)

    # ---- R3-01: quiet only on the kernel's word that no child is left ------------
    class Ended:
        pid, returncode = 4242, 0

        def wait(self, timeout=None):
            return 0

    def cleanup(answers, signal_all=None):
        """end_everything() with made-up kernel answers and an empty listing; time is made up too."""
        now = {"t": 0.0}
        sent = []

        def wait(_pid, _flags):
            answer = answers.pop(0) if len(answers) > 1 else answers[0]
            if answer == "none":
                raise ChildProcessError(10, "No child processes")
            return answer, 0

        def tick(_seconds):
            now["t"] += 1.0
        quiet, actions = end_everything(Ended(), wait=wait, listing=lambda: [],
                                        signal_one=lambda *_: sent.append("one"),
                                        signal_group=lambda *_: sent.append("group"),
                                        signal_all=(lambda: sent.append("all")) if signal_all else None,
                                        clock=lambda: now["t"], pause=tick)
        return quiet, [action.get("to", action.get("what")) for action in actions], sent[:2]

    check("93 waitpid says a child remains while the listing is empty: not quiet, and every other process "
          "of the namespace is signalled", cleanup([0], signal_all=True)[0::2], (False, ["all", "all"]))
    check("94 the same where this process is not known to be alone: not quiet, and nothing is sent blindly",
          cleanup([0]), (False, [], []))
    check("95 the kernel says no child is left: quiet, no signal", cleanup(["none"], signal_all=True),
          (True, [], []))
    check("96 a zombie is collected, then the kernel says no child is left: quiet, no signal",
          cleanup([77, "none"], signal_all=True), (True, [], []))

    # ---- R3-04: no crash may start a program on the host -----------------------
    infinite = resource.RLIM_INFINITY
    check("97 crash configuration: the pattern \"core\" with a hard limit of 1 or 0 passes; a pipe, a "
          "socket, another pattern, an unreadable one, or a hard limit that can be raised does not",
          [core_dump_problem(pattern, limit) is None for pattern, limit in (
              ("core", (1, 1)), ("core", (0, 0)), ("|/usr/share/apport/apport %p", (1, 1)), (None, (1, 1)),
              ("core", (1, 2)), ("core", (0, infinite)), ("@/run/systemd/coredump.socket", (1, 1)),
              ("core.%p", (1, 1)))],
          [True, True, False, False, False, False, False, False])
    return checks


# ===========================================================================
# Shutting the runner in (MA-02, B2, B3): Landlock, by the kernel. One set of
# rules for this process, a narrower one for every process it starts, and a
# second one for this process once the dependency cache has been unpacked.
# Landlock does not control connections to path-named sockets, nor a change
# of mode or times of a file its user owns: those two walls are the place
# the launcher prepares, which check_view() and the probes verify.
# ===========================================================================
FS = {
    "EXECUTE": 1 << 0, "WRITE_FILE": 1 << 1, "READ_FILE": 1 << 2, "READ_DIR": 1 << 3,
    "REMOVE_DIR": 1 << 4, "REMOVE_FILE": 1 << 5, "MAKE_CHAR": 1 << 6, "MAKE_DIR": 1 << 7,
    "MAKE_REG": 1 << 8, "MAKE_SOCK": 1 << 9, "MAKE_FIFO": 1 << 10, "MAKE_BLOCK": 1 << 11,
    "MAKE_SYM": 1 << 12, "REFER": 1 << 13, "TRUNCATE": 1 << 14, "IOCTL_DEV": 1 << 15,
}
FS_OF_A_FILE = FS["EXECUTE"] | FS["WRITE_FILE"] | FS["READ_FILE"] | FS["TRUNCATE"] | FS["IOCTL_DEV"]
READ = FS["READ_FILE"] | FS["READ_DIR"]
READ_RUN = READ | FS["EXECUTE"]
REWRITE = FS["READ_FILE"] | FS["WRITE_FILE"] | FS["TRUNCATE"]
EVERYTHING = sum(FS.values())
SYSTEM_READ = ["/usr", "/lib", "/lib64", "/lib32", "/libx32", "/bin", "/sbin", "/etc/alternatives",
               "/etc/ld.so.cache", "/etc/ld.so.conf", "/etc/ld.so.conf.d", "/proc",
               "/sys/devices/system/cpu", "/dev/urandom", "/dev/random", "/dev/zero"]


class RulesetAttr(ctypes.Structure):
    _fields_ = [("handled_access_fs", ctypes.c_uint64), ("handled_access_net", ctypes.c_uint64),
                ("scoped", ctypes.c_uint64)]


class PathBeneathAttr(ctypes.Structure):
    _pack_ = 1
    _fields_ = [("allowed_access", ctypes.c_uint64), ("parent_fd", ctypes.c_int32)]


class Landlock:
    def __init__(self):
        self.libc = ctypes.CDLL(None, use_errno=True)
        self.libc.syscall.restype = ctypes.c_long
        self.abi = self.libc.syscall(ctypes.c_long(444), None, ctypes.c_size_t(0), ctypes.c_uint32(1))
        if self.abi < 3:
            raise Refused("CONTAINMENT UNAVAILABLE: Landlock ABI %d (3 or later is needed)" % self.abi)
        self.handled = sum(FS[name] for name in FS
                           if not (name == "IOCTL_DEV" and self.abi < 5))

    def call(self, number, *arguments):
        result = self.libc.syscall(ctypes.c_long(number), *arguments)
        if result < 0:
            raise OSError(ctypes.get_errno(), "Landlock call %d: %s" % (number, os.strerror(ctypes.get_errno())))
        return result

    def ruleset(self, rules):
        """A new set of rules: [(path, access, why, needed)]. Returns (fd, what was applied)."""
        attr = RulesetAttr(self.handled, 3 if self.abi >= 4 else 0, 3 if self.abi >= 6 else 0)
        size = 24 if self.abi >= 6 else 16 if self.abi >= 4 else 8
        fd = self.call(444, ctypes.byref(attr), ctypes.c_size_t(size), ctypes.c_uint32(0))
        applied = []
        for path, access, why, needed in rules:
            if not os.path.exists(path):
                if needed:
                    raise Refused("CONTAINMENT: a needed path is missing: %s" % path)
                continue
            access &= self.handled
            if not os.path.isdir(path):
                access &= FS_OF_A_FILE
            target = os.open(path, os.O_PATH | os.O_CLOEXEC)
            try:
                rule = PathBeneathAttr(access, target)
                self.call(445, ctypes.c_int(fd), ctypes.c_int(1), ctypes.byref(rule), ctypes.c_uint32(0))
            finally:
                os.close(target)
            applied.append({"path": path, "why": why,
                            "access": sorted(name for name, bit in FS.items() if access & bit)})
        return fd, applied

    def restrict(self, fd):
        self.call(446, ctypes.c_int(fd), ctypes.c_uint32(0))


def host_core_pattern():
    try:
        with open("/proc/sys/kernel/core_pattern", "r", encoding="utf-8", errors="replace") as handle:
            return handle.read().strip()
    except OSError:
        return None


HOST_CORE_PATTERN = "core"  # what the launcher sets: a plain file name


def core_dump_problem(pattern, limit):
    """None when no crash can start a program on the host or write a dump; else why not (R3-04).

    The pattern must be exactly the plain name the launcher sets: not a pipe
    to a program ("|"), not a socket of a host service ("@", "@@"), nothing
    else. A hard core-size limit of 1 or 0 cannot be raised without a
    capability this user does not have, and is below the smallest dump the
    kernel writes.
    """
    if pattern is None:
        return "the host's core_pattern cannot be read"
    if pattern != HOST_CORE_PATTERN:
        return "the host's core_pattern is %r, not %r" % (pattern, HOST_CORE_PATTERN)
    if limit[1] == resource.RLIM_INFINITY or limit[1] > 1:
        return "the hard core-size limit is %s: a process could raise its own limit" % (limit[1],)
    return None


def no_core_dumps():
    """A crash must start no helper on the host and write no dump (R3-04).

    The launcher sets the host's core_pattern to a plain name and this
    process's core-size limit to 1, soft and hard. This process tries the
    limit itself as well, then refuses to start unless core_dump_problem()
    finds nothing. Every process it starts inherits the hard limit, and none
    can raise it.
    """
    pattern = host_core_pattern()
    try:
        resource.setrlimit(resource.RLIMIT_CORE, (1, 1))
    except (ValueError, OSError):
        pass
    limit = resource.getrlimit(resource.RLIMIT_CORE)
    problem = core_dump_problem(pattern, limit)
    if problem:
        raise Refused("CONTAINMENT UNAVAILABLE: %s" % problem)
    return {"host_core_pattern": pattern, "core_size_limit": list(limit),
            "note": "The host's core_pattern is the plain name %r: the kernel starts no program and hands "
                    "no dump to a host service. The hard core-size limit is at most 1: no dump is written, "
                    "and no process can raise it. The core_pattern is read again before every case and at "
                    "the end; a change stops the campaign or withholds VALID. A change made and undone "
                    "between two readings would not be seen." % HOST_CORE_PATTERN}


def confine(src, out, work, cargo_home, toolchain_root):
    """Restrict this process, for good, and prepare the rules of what it starts.

    Returns (what was applied, the function the preparation runs before its
    command, the function every other child runs before its command, seal).
    seal() puts a second set of rules on this process: from then on neither
    it nor anything it starts can write the dependency cache.
    """
    try:
        landlock = Landlock()
        common = [(toolchain_root, READ_RUN, "the Rust toolchain: read and run", True)]
        common += [(path, READ_RUN, "system: read and run", False) for path in SYSTEM_READ]
        common += [(path, READ_RUN, "this Python: read and run", False)
                   for path in sorted({sys.prefix, sys.base_prefix, sys.exec_prefix})
                   if not (path == "/usr" or path.startswith("/usr/"))]
        common += [("/dev/null", FS["READ_FILE"] | FS["WRITE_FILE"] | FS["IOCTL_DEV"], "the null device", True)]
        source = [(src, READ, "the candidate's source: read only", True)]
        four = [(os.path.join(src, rel), REWRITE, "one of the four files a mutant rewrites (the runner only)", True)
                for rel in sorted(CLEAN)]
        receipts = [(out, EVERYTHING, "the receipts (the runner only)", True)]
        scratch = [(work, EVERYTHING, "build output and scratch", True)]
        cache_open = [(cargo_home, EVERYTHING,
                       "the dependency cache while Cargo unpacks it (the preparation only)", True)]
        cache_shut = [(cargo_home, READ_RUN, "the dependency cache: read and run, never write", True)]
        first_fd, first_rules = landlock.ruleset(source + four + receipts + scratch + cache_open + common)
        sealed_fd, sealed_rules = landlock.ruleset(source + four + receipts + scratch + cache_shut + common)
        prepare_fd, prepare_rules = landlock.ruleset(source + scratch + cache_open + common)
        child_fd, child_rules = landlock.ruleset(source + scratch + cache_shut + common)
        if landlock.libc.prctl(38, 1, 0, 0, 0) != 0:  # PR_SET_NO_NEW_PRIVS
            raise Refused("CONTAINMENT UNAVAILABLE: no_new_privs")
        landlock.restrict(first_fd)
        os.close(first_fd)
        if landlock.libc.prctl(36, 1, 0, 0, 0) != 0:  # PR_SET_CHILD_SUBREAPER: a case's orphans come to this process
            raise Refused("CONTAINMENT UNAVAILABLE: child subreaper")
        cores = no_core_dumps()
    except OSError as problem:
        raise Refused("CONTAINMENT UNAVAILABLE: %s" % problem)

    def before_the_preparation():
        landlock.restrict(prepare_fd)

    def before_each_child():
        landlock.restrict(child_fd)

    def seal():
        try:
            landlock.restrict(sealed_fd)
            os.close(sealed_fd)
        except OSError as problem:
            raise Refused("CONTAINMENT UNAVAILABLE: the dependency cache could not be sealed: %s" % problem)

    applied = {"mechanism": "Landlock", "abi": landlock.abi, "no_new_privs": True, "child_subreaper": True,
               "tcp_bind_and_connect_denied": landlock.abi >= 4,
               "abstract_unix_and_signals_scoped": landlock.abi >= 6,
               "path_named_unix_sockets": "not controlled by these rules: the wall is the filesystem view "
                                          "(no socket in it; see the view and the probes)",
               "mode_and_times_of_own_files": "not controlled by these rules: the wall is the filesystem view "
                                              "(read-only outside the audit folders; see the view and the probes)",
               "core_dumps": cores,
               "this_process_at_first": first_rules,
               "this_process_once_the_cache_is_unpacked": sealed_rules,
               "the_preparation": prepare_rules,
               "every_other_child": child_rules,
               "every_child_note": "A child is under this process's rules and its own. The preparation can "
                                   "write the scratch folder and the dependency cache. Every other child can "
                                   "create, remove and write files only in the scratch folder: not in the "
                                   "receipts, not in the source, not in the dependency cache.",
               "what_these_rules_do_not_stop": "A child runs as the user that owns the audit folders. It can "
                                               "change the mode or the times of a file there. A changed mode "
                                               "is seen: in the source as INPUT CHANGED, in the dependency "
                                               "cache at the end, in the receipts as a log that can no longer "
                                               "be read. Changed times are not looked at. Memory and the "
                                               "number of processes are not limited. Sockets of other "
                                               "families than TCP and UNIX are not controlled here."}
    return applied, before_the_preparation, before_each_child, seal


CHILD_PROBE = r"""
import errno, json, os, socket, sys
given = json.loads(sys.argv[1])
seen = {}
def attempt(name, action):
    try:
        action()
        seen[name] = "allowed"
    except FileNotFoundError:
        seen[name] = "absent"
    except ConnectionRefusedError:
        seen[name] = "refused"
    except PermissionError:
        seen[name] = "denied"
    except OSError as problem:
        seen[name] = "read-only" if problem.errno == errno.EROFS else "error: %s" % problem
def create(path):
    os.close(os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600))
def connect(address):
    with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as probe:
        probe.settimeout(5)
        probe.connect(address)
attempt("rewrite a source file", lambda: os.close(os.open(given["source_file"], os.O_WRONLY)))
attempt("create a file in the receipts", lambda: create(os.path.join(given["receipt"], ".rs01-audit-child-probe")))
attempt("create a file in the scratch folder", lambda: create(os.path.join(given["scratch"], ".rs01-audit-child-probe")))
attempt("create a Cargo configuration in the dependency cache",
        lambda: create(os.path.join(given["cargo_home"], "config.toml")))
attempt("rewrite a file of the dependency cache", lambda: os.close(os.open(given["cache_file"], os.O_WRONLY)))
attempt("connect to the runner's own socket in the scratch folder", lambda: connect(given["own_socket"]))
attempt("connect to the launcher's canary socket, a path on the host", lambda: connect(given["host_socket"]))
attempt("connect to the launcher's canary socket, an abstract name on the host",
        lambda: connect("\0" + given["host_abstract"]))
attempt("change the mode of the launcher's canary file on the host", lambda: os.chmod(given["host_file"], 0o600))
attempt("change the mode of this user's own file outside the audit folders",
        lambda: os.chmod(given["view_file"], 0o600))
attempt("change the times of that file", lambda: os.utime(given["view_file"]))
print("PROBE " + json.dumps(seen, sort_keys=True))
"""
# A child other than the preparation is under these rules from the first: it
# cannot write the dependency cache whether or not the cache is yet sealed.
CHILD_PROBE_WANT = {
    "rewrite a source file": "denied",
    "create a file in the receipts": "denied",
    "create a file in the scratch folder": "allowed",
    "create a Cargo configuration in the dependency cache": "denied",
    "rewrite a file of the dependency cache": "denied",
    # The control: Landlock does not stop a connection to a path-named socket.
    # That the next two are out of reach is therefore the view's doing.
    "connect to the runner's own socket in the scratch folder": "allowed",
    "connect to the launcher's canary socket, a path on the host": "absent",
    "connect to the launcher's canary socket, an abstract name on the host": "refused",
    "change the mode of the launcher's canary file on the host": "absent",
    "change the mode of this user's own file outside the audit folders": "read-only",
    "change the times of that file": "read-only",
}


def probe_containment(script_path, src, work):
    """Show, from inside this process, that the walls are there. Any surprise refuses the run."""
    probes = []

    def expect_denied(name, action, or_read_only=False):
        try:
            action()
        except PermissionError:
            probes.append({"probe": name, "result": "denied, as it must be"})
            return
        except OSError as problem:
            if or_read_only and problem.errno == errno.EROFS:
                # The view answers before Landlock is asked: that place is read-only.
                probes.append({"probe": name, "result": "refused: a read-only filesystem, as the view makes it"})
                return
            raise Refused("CONTAINMENT PROBE: %s: unexpected %s" % (name, problem))
        raise Refused("CONTAINMENT PROBE FAILED: %s was allowed" % name)

    def write_beside_script():
        path = os.path.join(os.path.dirname(script_path), ".rs01-audit-write-probe")
        try:
            fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
        except FileExistsError:
            raise Refused("CONTAINMENT PROBE: %s exists" % path)
        os.close(fd)
        os.unlink(path)

    def write_in_source():
        fd = os.open(os.path.join(src, ".rs01-audit-write-probe"), os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
        os.close(fd)

    def read_script():
        with open(script_path, "rb") as handle:
            handle.read(1)

    def bind_tcp():
        with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as probe:
            probe.bind(("127.0.0.1", 0))

    expect_denied("read a file outside the allowed folders (this script)", read_script)
    expect_denied("create a file outside the allowed folders (beside this script)", write_beside_script,
                  or_read_only=True)
    # The source belongs to this user and is on a writable filesystem: only Landlock stands here.
    expect_denied("create a file in the source", write_in_source)
    try:
        bind_tcp()
        probes.append({"probe": "bind a TCP socket on loopback", "result": "allowed (Landlock ABI below 4); "
                       "the only interface is loopback"})
    except PermissionError:
        probes.append({"probe": "bind a TCP socket on loopback", "result": "denied"})
    except OSError as problem:
        probes.append({"probe": "bind a TCP socket on loopback", "result": "failed: %s" % problem})
    inside = os.path.join(work, "tmp", ".rs01-audit-write-probe")
    with open(inside, "wb") as handle:
        handle.write(b"x")
    os.unlink(inside)
    probes.append({"probe": "create and remove a file in the scratch folder", "result": "allowed, as it must be"})
    return probes


def probe_the_seal(cargo_home):
    """Once the dependency cache is sealed, this process itself cannot change it (B2)."""
    probes = []
    for name, action in (
            ("create a Cargo configuration in the dependency cache",
             lambda: os.close(os.open(os.path.join(cargo_home, "config.toml"),
                                      os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600))),
            ("create a folder in the dependency cache",
             lambda: os.mkdir(os.path.join(cargo_home, ".rs01-audit-seal-probe")))):
        try:
            action()
        except PermissionError:
            probes.append({"probe": name, "result": "denied, as it must be"})
            continue
        except OSError as problem:
            raise Refused("CONTAINMENT PROBE: %s: unexpected %s" % (name, problem))
        raise Refused("CONTAINMENT PROBE FAILED: %s was allowed after the seal" % name)
    return probes


# ===========================================================================
# The place this script is started in (B3). The launcher makes it; this
# script checks it, before it shuts itself in, and refuses anything else.
# ===========================================================================
def require_audit_identity():
    """An ordinary user that is in no group but its own."""
    if os.geteuid() == 0 or os.getuid() == 0:
        raise Refused("RUN AS AN ORDINARY USER: this process is root")
    if os.getuid() != os.geteuid() or os.getgid() != os.getegid():
        raise Refused("RUN AS ONE USER: the real and effective ids differ")
    if len(set(os.getresuid())) != 1 or len(set(os.getresgid())) != 1:
        raise Refused("RUN AS ONE USER: the saved ids differ (%s, %s)" % (os.getresuid(), os.getresgid()))
    others = sorted(set(os.getgroups()) - {os.getegid()})
    if others:
        raise Refused("RUN WITHOUT SUPPLEMENTARY GROUPS: this process is also in groups %s"
                      % ", ".join(str(group) for group in others))
    capabilities = {}
    with open("/proc/self/status", "r", encoding="ascii", errors="replace") as handle:
        for line in handle:
            if line.startswith(("CapPrm:", "CapEff:", "CapAmb:")):
                capabilities[line[:6]] = int(line.split()[1], 16)
    if len(capabilities) != 3 or any(capabilities.values()):
        raise Refused("RUN WITHOUT CAPABILITIES: %s" % capabilities)
    return {"uid": os.geteuid(), "gid": os.getegid(), "saved_ids_equal": True, "supplementary_groups": [],
            "capabilities_permitted_effective_ambient": 0}


def mounts_in_view():
    """Every mount this process can see: [{at, options, type, super}]."""
    found = []
    with open("/proc/self/mountinfo", "r", encoding="utf-8", errors="replace") as handle:
        for line in handle:
            left, dash, right = line.rstrip("\n").partition(" - ")
            fields = left.split(" ")
            tail = right.split(" ")
            if not dash or len(fields) < 6 or len(tail) < 3:
                raise Refused("CONTAINMENT: a line of /proc/self/mountinfo could not be read: %r" % line)
            point = re.sub(r"\\([0-7]{3})", lambda match: chr(int(match.group(1), 8)), fields[4])
            found.append({"at": point, "options": fields[5].split(","), "type": tail[0],
                          "super": tail[2].split(",")})
    return found


def mount_of(path, mounts):
    best = "/"
    for mount in mounts:
        if within(path, mount["at"]) and len(mount["at"]) > len(best):
            best = mount["at"]
    return best


def writable_mounts(mounts, audit):
    """The mounts that are not read-only, other than the audit's own and /proc."""
    return sorted({mount["at"] for mount in mounts
                   if "ro" not in mount["options"] and "ro" not in mount["super"]
                   and mount["at"] not in audit and not (mount["at"] == "/proc" and mount["type"] == "proc")})


def sockets_in_view(skip=("/proc", "/sys")):
    """Every socket in the filesystem this process can reach.

    Returns (the sockets, how many folders were listed, the folders closed to
    this user, the folders it can enter but not list). A folder this user can
    neither list nor enter is closed to it: nothing in it can be reached. A
    folder it can enter but not list could hold a socket this walk cannot
    see: check_view() refuses a place that has one.
    """
    found, folders, closed, unseen = [], 0, [], []
    stack = ["/"]
    while stack:
        folder = stack.pop()
        try:
            entries = list(os.scandir(folder))
        except OSError:
            (unseen if os.access(folder, os.X_OK) else closed).append(folder)
            continue
        folders += 1
        for entry in entries:
            try:
                if entry.is_symlink():
                    continue
                if entry.is_dir(follow_symlinks=False):
                    if entry.path not in skip:
                        stack.append(entry.path)
                elif (not entry.is_file(follow_symlinks=False)
                      and stat.S_ISSOCK(entry.stat(follow_symlinks=False).st_mode)):
                    found.append(entry.path)
            except OSError:
                # Listed, but its kind cannot be read: the folder can be listed and not entered.
                (unseen if os.access(folder, os.X_OK) else closed).append(entry.path)
    return sorted(found), folders, sorted(closed), sorted(unseen)


def check_view(src, out, work, cargo_home):
    """The place is the one the launcher makes. Returns what was seen; refuses anything else.

    Nothing here contacts a service: the checks read the mount table, walk the
    filesystem, and read two of the kernel's own lists.
    """
    mounts = mounts_in_view()
    audit = sorted({mount_of(path, mounts) for path in (src, out, work, cargo_home)})
    if "/" in audit:
        raise Refused("CONTAINMENT: an audit folder is on the root filesystem of this view")
    writable = writable_mounts(mounts, audit)
    if writable:
        raise Refused("CONTAINMENT: writable outside the audit folders: %s" % ", ".join(writable[:10]))
    sockets, folders, closed, unseen = sockets_in_view()
    if sockets:
        raise Refused("CONTAINMENT: a socket is in the filesystem this process can see: %s"
                      % ", ".join(sockets[:10]))
    if unseen:
        raise Refused("CONTAINMENT: this user can enter but not list %s: a socket there could not be seen"
                      % ", ".join(unseen[:10]))
    bound = []
    try:
        with open("/proc/net/unix", "r", encoding="utf-8", errors="replace") as handle:
            for line in handle.read().split("\n")[1:]:
                fields = line.split(None, 7)
                if len(fields) == 8:
                    bound.append(fields[7])
    except FileNotFoundError:
        bound = None
    if bound:
        raise Refused("CONTAINMENT: named UNIX sockets exist in this network namespace: %s" % ", ".join(bound[:10]))
    others = sorted(pid for pid in process_table() if pid != os.getpid())
    if others or os.getpid() != 1:
        raise Refused("CONTAINMENT: other processes are in sight (%d), or this is not process 1: run alone "
                      "in a process namespace" % len(others))
    ALONE_IN_ITS_PROCESS_NAMESPACE[:] = [True]
    try:
        canary = os.lstat(VIEW_CANARY_FILE)
    except OSError:
        raise Refused("CONTAINMENT: the launcher's file %s is missing: this place was not prepared by the "
                      "launcher" % VIEW_CANARY_FILE)
    if not stat.S_ISREG(canary.st_mode) or canary.st_uid != os.geteuid():
        raise Refused("CONTAINMENT: %s is not a file of this user" % VIEW_CANARY_FILE)
    for path in (HOST_CANARY_SOCKET, HOST_CANARY_FILE):
        if os.path.lexists(path):
            raise Refused("CONTAINMENT: the host's %s can be seen from here" % path)
    return {
        "mounts": [{"at": mount["at"], "type": mount["type"],
                    "read_only": "ro" in mount["options"] or "ro" in mount["super"]} for mount in mounts],
        "writable_mounts": audit + (["/proc"] if any(m["at"] == "/proc" for m in mounts) else []),
        "writable_mounts_note": "The audit's own filesystems, and /proc. Every other mount is read-only.",
        "sockets_in_the_filesystem": 0, "folders_listed": folders,
        "folders_this_user_can_enter_but_not_list": 0,
        "folders_closed_to_this_user": {"count": len(closed), "first": closed[:20],
                                        "note": "This user can neither list nor enter them: nothing in them "
                                                "can be reached. Every other folder was listed."},
        "named_unix_sockets_in_this_network_namespace": 0 if bound is not None else "not listed by this kernel",
        "other_processes_in_sight": 0,
        "this_process_is_pid_1_of_its_process_namespace": True,
        "launcher_file_in_the_view": VIEW_CANARY_FILE,
        "host_canaries_out_of_sight": [HOST_CANARY_SOCKET, HOST_CANARY_FILE],
    }


# ===========================================================================
# Owning a child process (MA-03, section 5 of REVIEW-R2)
# ===========================================================================
NORMAL = "NORMAL EXIT, OUTPUT ENDED"
NORMAL_LEFT = "NORMAL EXIT, A PROCESS IT STARTED WAS STILL ALIVE"
HELD_OPEN = "OUTPUT HELD OPEN AFTER THE COMMAND EXITED"
OWN_SIGNAL = "COMMAND ENDED BY A SIGNAL THE RUNNER DID NOT SEND"
FORCED = "COMMAND FORCIBLY ENDED BY THE RUNNER"
TIMED_OUT = "TIMEOUT"
UNRESOLVED = "QUIESCENCE NOT ESTABLISHED"
SNAPSHOT_NOTE = ("The processes below the runner that were alive, as listed before any signal was sent. "
                 "A process can start or end between the listing and the signals: this is what was "
                 "seen, not an exact count of what was ended.")


def process_table():
    """Every process this one can see: {pid: (parent, group, state, name)}."""
    table = {}
    for name in os.listdir("/proc"):
        if not name.isdigit():
            continue
        try:
            with open("/proc/%s/stat" % name, "rb") as handle:
                raw = handle.read()
        except OSError:
            continue
        head, _mark, tail = raw.rpartition(b")")
        fields = tail.split()
        if len(fields) < 3:
            continue
        table[int(name)] = (int(fields[1]), int(fields[2]), fields[0].decode("ascii", "replace"),
                            head.split(b"(", 1)[-1].decode("utf-8", "replace"))
    return table


def below_this_process(alive_only=True):
    """The processes below this one: its children, theirs, and every orphan that came to it.

    This process is a subreaper: a process whose parent has ended becomes its
    child, so nothing a case starts can leave this list while it lives.
    """
    table = process_table()
    mine = os.getpid()
    found = []
    for pid in sorted(table):
        up, seen = pid, set()
        while up in table and up != mine and up not in seen:
            seen.add(up)
            up = table[up][0]
        parent, group, state, name = table[pid]
        if up == mine and pid != mine and not (alive_only and state in ("Z", "X")):
            found.append({"pid": pid, "parent": parent, "group": group, "state": state, "name": name})
    return found


# Set by check_view() once it has seen that this process is alone in its own
# process namespace (it is PID 1 there). Only then may a cleanup signal every
# other process at once (kill -1): every one of them is the campaign's own.
ALONE_IN_ITS_PROCESS_NAMESPACE = []


def signal_every_other_process():
    os.kill(-1, signal.SIGKILL)


def end_everything(process, wait=os.waitpid, listing=None, signal_one=os.kill, signal_group=os.killpg,
                   signal_all=None, clock=time.monotonic, pause=time.sleep):
    """End every process below this one, and collect them (R3-01).

    Returns (True only when the kernel says this process has no child left,
    the signals that were sent). The listings of /proc find what to signal and
    what is recorded; they are not the proof. The proof is waitpid's answer
    that there is no child at all (ECHILD). An answer that a child remains
    (0) is never taken as quiet: the listing is taken again, and if it finds
    nothing, every other process of this process namespace is signalled, where
    that is known to be only the campaign's own. If the proof does not come
    within QUIESCE_S, the answer is False: the case is quarantined, nothing is
    restored or reused. Signals are recorded as they are sent; nothing is
    counted afterwards. The parameters other than process are for the
    runner's own checks.
    """
    listing = listing or below_this_process
    deadline = clock() + QUIESCE_S
    actions = []
    told = set()
    while True:
        alive = listing()
        for entry in alive:
            if entry["pid"] not in told:
                told.add(entry["pid"])
                actions.append({"signal": "SIGKILL", "pid": entry["pid"], "name": entry["name"],
                                "state_seen": entry["state"],
                                "what": "the command itself" if entry["pid"] == process.pid else
                                        "a process the command started"})
            try:
                signal_one(entry["pid"], signal.SIGKILL)
            except (ProcessLookupError, PermissionError):
                pass
        if not alive and process.returncode is None:
            # The command is not collected, yet no listing shows it alive (a leader
            # that has ended while its other threads run): it is signalled by its pid.
            try:
                signal_one(process.pid, signal.SIGKILL)
                if "command" not in told:
                    told.add("command")
                    actions.append({"signal": "SIGKILL", "pid": process.pid,
                                    "what": "the command itself, not collected and not listed as alive"})
            except (ProcessLookupError, PermissionError):
                pass
        if alive:
            # A sweep of the command's own group, in one call: whatever it started
            # between the listing and the signals above.
            try:
                signal_group(process.pid, signal.SIGKILL)
                if "group" not in told:
                    told.add("group")
                    actions.append({"signal": "SIGKILL", "group": process.pid,
                                    "what": "the command's process group, in one call, as a sweep"})
            except (ProcessLookupError, PermissionError):
                pass
        if process.returncode is None:
            try:
                process.wait(timeout=0.2)
            except subprocess.TimeoutExpired:
                pass
        if process.returncode is not None:
            # Only now may others be collected: collecting the command itself by
            # another call would lose its exit status.
            while True:
                try:
                    pid, _status = wait(-1, os.WNOHANG)
                except ChildProcessError:
                    return True, actions  # the kernel: no child is left
                if pid == 0:
                    break  # a child remains, alive or not yet collectable: not quiet
            if not alive and signal_all is not None:
                # The kernel has a child that no listing found.
                try:
                    signal_all()
                    if "all" not in told:
                        told.add("all")
                        actions.append({"signal": "SIGKILL", "to": "every other process of this process "
                                                                   "namespace, in one call (kill -1)",
                                        "what": "the kernel still had a child that no listing found"})
                except (ProcessLookupError, PermissionError):
                    pass
        if clock() > deadline:
            return False, actions
        pause(0.1)


def lifecycle_of(ended_by, own_status, alive_before, quiet):
    """How a command's life ended, as one of seven fixed words.

    ended_by      why the runner stopped reading its output
    own_status    the status the command ended with by itself, before any
                  signal was sent by the runner; None if it had not ended
    alive_before  the processes below the runner that were still alive then,
                  or, if none was listed, the signals that had to be sent
    quiet         no process of it is left
    """
    if not quiet:
        return UNRESOLVED
    if ended_by in ("timeout", "campaign deadline"):
        return TIMED_OUT
    if ended_by == "output held open":
        return HELD_OPEN
    if ended_by != "exit" or own_status is None:
        return FORCED
    if own_status < 0:
        return OWN_SIGNAL
    return NORMAL_LEFT if alive_before else NORMAL


def step_no_result(step, which):
    """The no-result verdict that a step's own ending gives; None if its output may be read.

    Only a command that ended by itself, with an exit status, and whose
    output ended, has output that is read at all (section 5, E).
    """
    if step["lifecycle"] == TIMED_OUT:
        return ("CAMPAIGN DEADLINE (%s)" if step["ended_by"] == "campaign deadline" else "TIMEOUT (%s)") % which
    if step["ended_by"] == "log limit":
        return "LOG LIMIT (%s)" % which
    if step["lifecycle"] == HELD_OPEN:
        return "OUTPUT HELD OPEN (%s)" % which
    if step["lifecycle"] not in (NORMAL, NORMAL_LEFT) or step["exit"] is None:
        return "EXECUTION FAILURE (%s)" % which
    return None


def allowed_time(step_limit_s, work_left_s):
    """(seconds a step may run, what cuts it): the campaign's deadline binds every step (B4)."""
    room = work_left_s - CASE_CLOSE_S
    if room < step_limit_s:
        return max(0, int(room)), "campaign deadline"
    return step_limit_s, "timeout"


def run_child(argv, cwd, env, log_path, timeout_s, before=None, cut="timeout"):
    """Run one command to its end, owning it throughout.

    Its output (stdout and stderr, one combined stream) goes to log_path as
    the bytes that were read; the hash is of those bytes. Returns what
    happened: how the command's life ended, what was alive before any
    signal was sent, and every signal that was sent. Whatever else goes
    wrong, a process that could not be ended raises Quarantine: nothing may
    be restored or reused while it lives. A signal to the runner raises
    Interrupted, after the child is ended.
    """
    started = time.monotonic()
    deadline = started + timeout_s
    digest = hashlib.sha256()
    size = 0
    ended_by = "exit"
    interrupted = None
    failure = None
    quiet, actions, alive_before, own_status, gone_by_itself = False, [], [], None, 0
    with open(log_path, "wb") as log:
        try:
            process = subprocess.Popen(argv, cwd=cwd, env=env, stdin=subprocess.DEVNULL,
                                       stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                                       start_new_session=True, close_fds=True, preexec_fn=before)
        except (OSError, subprocess.SubprocessError) as problem:
            raise Refused("COULD NOT START %s: %s" % (argv[0], problem))
        fd = process.stdout.fileno()
        exited_at = None
        try:
            while True:
                check_signals()
                if time.monotonic() >= deadline:
                    ended_by = cut
                    break
                ready, _w, _x = select.select([fd], [], [], 0.5)
                if ready:
                    chunk = os.read(fd, 65536)
                    if not chunk:
                        break
                    if size + len(chunk) > LOG_LIMIT_BYTES:
                        chunk = chunk[:LOG_LIMIT_BYTES - size]
                        ended_by = "log limit"
                    log.write(chunk)
                    digest.update(chunk)
                    size += len(chunk)
                    if ended_by == "log limit":
                        break
                elif process.poll() is not None:
                    # The command has ended but something still holds its output open.
                    exited_at = exited_at or time.monotonic()
                    if time.monotonic() - exited_at > 5:
                        ended_by = "output held open"
                        break
        except Interrupted as signalled:
            interrupted = signalled
            ended_by = "signal to the runner"
        except Exception as problem:  # the runner's own fault: still end the child first
            failure = problem
            ended_by = "runner error"
        try:
            if ended_by == "exit":
                try:
                    process.wait(timeout=10)
                except subprocess.TimeoutExpired:
                    ended_by = "it closed its output and did not end"
            # No signal has been sent yet. What the command ended with, if it has
            # ended, is its own; what is alive below the runner is listed now.
            if process.poll() is not None:
                own_status = process.returncode
            alive_before = below_this_process()
            if ended_by == "exit":
                # Look twice, a moment apart. Whatever is alive at the second look was
                # left behind, whether or not it was there at the first. What was alive
                # only at the first look ended by itself: it is counted apart.
                time.sleep(0.1)
                second = below_this_process()
                gone_by_itself = len({entry["pid"] for entry in alive_before}
                                     - {entry["pid"] for entry in second})
                alive_before = second
            quiet, actions = end_everything(
                process, signal_all=signal_every_other_process if ALONE_IN_ITS_PROCESS_NAMESPACE else None)
        except Exception as problem:
            quiet = False
            failure = failure or problem
        try:
            process.stdout.close()
            log.flush()
            os.fsync(log.fileno())
        except Exception as problem:
            failure = failure or problem
    code = process.returncode
    # A signal that had to be sent is a process that was left behind, even if no
    # listing caught it: a command is NORMAL only if nothing was alive and nothing was sent.
    lifecycle = lifecycle_of(ended_by, own_status, alive_before or actions, quiet)
    result = {
        "command": " ".join(argv),
        "ended_by": ended_by,
        "lifecycle": lifecycle,
        "exit": own_status if lifecycle in (NORMAL, NORMAL_LEFT) else None,
        "signal": -code if (code is not None and code < 0) else None,
        "signal_sent_by_the_runner": bool(code is not None and code < 0 and own_status is None),
        "seconds": round(time.monotonic() - started, 1),
        "log": os.path.basename(log_path),
        "log_bytes": size,
        "log_sha256": digest.hexdigest(),
        "stream": "stdout and stderr, combined, as read",
        "output": {"exit": "ended (end of file)", "output held open": "held open after the command exited; cut",
                   }.get(ended_by, "cut by the runner: %s" % ended_by),
        "alive_before_cleanup": alive_before[:40],
        "alive_before_cleanup_count": len(alive_before),
        "alive_before_cleanup_note": SNAPSHOT_NOTE,
        "alive_when_the_command_ended_and_gone_by_itself_a_moment_later": gone_by_itself,
        "cleanup_signals": actions[:40],
        "cleanup_signals_count": len(actions),
        "cleanup_required": bool(actions) or lifecycle not in (NORMAL, OWN_SIGNAL),
        "no_process_left": quiet,
    }
    if not quiet:
        raise Quarantine(as_json(result))
    if interrupted:
        interrupted.step = result
        raise interrupted
    if failure:
        raise failure
    return result


def read_log(path):
    with open(path, "rb") as handle:
        return handle.read().decode("utf-8", "replace")


# ===========================================================================
# Paths and the environment (MA-02)
# ===========================================================================
def canonical(path):
    return os.path.realpath(os.path.abspath(path))


def within(path, folder):
    return path == folder or path.startswith(folder.rstrip(os.sep) + os.sep)


def parents(path):
    while True:
        yield path
        up = os.path.dirname(path)
        if up == path:
            return
        path = up


def check_paths(script_path, launcher, src, out, work, cargo_home, toolchain_bin):
    toolchain_root = os.path.dirname(toolchain_bin)
    if not os.path.isdir(src):
        raise Refused("--src is not a folder: %s" % src)
    for name, path in (("--out", out), ("--work", work)):
        if os.path.lexists(path) and (not os.path.isdir(path) or os.listdir(path)):
            raise Refused("%s exists and is not an empty folder: %s" % (name, path))
        if not os.path.isdir(path) and not os.path.isdir(os.path.dirname(path)):
            raise Refused("%s cannot be made: its parent is not a folder: %s" % (name, path))
    if not os.path.isdir(cargo_home):
        raise Refused("--cargo-home is not a folder: %s" % cargo_home)
    if not os.path.isfile(launcher):
        raise Refused("--launcher is not a file: %s" % launcher)
    for tool in ("cargo", "rustc"):
        if not os.path.isfile(os.path.join(toolchain_bin, tool)):
            raise Refused("--toolchain-bin has no %s: %s" % (tool, toolchain_bin))
    named = {"--src": src, "--out": out, "--work": work, "--cargo-home": cargo_home,
             "the toolchain": toolchain_root}
    for name, path in named.items():
        if path in ("/", os.path.expanduser("~")):
            raise Refused("%s is too wide: %s" % (name, path))
    for name, path in list(named.items())[:4]:
        for above in parents(path):
            if os.path.lexists(os.path.join(above, ".git")):
                raise Refused("%s is inside a Git repository (%s)" % (name, above))
    names = list(named)
    for index, first in enumerate(names):
        for second in names[index + 1:]:
            if within(named[first], named[second]) or within(named[second], named[first]):
                raise Refused("%s and %s overlap" % (first, second))
    for name, path in named.items():
        for what, file in (("this script", script_path), ("the launcher's file", launcher)):
            if within(file, path):
                raise Refused("%s is inside %s" % (what, name))
    for above in list(parents(src))[1:]:
        for name in (".cargo/config", ".cargo/config.toml"):
            if os.path.lexists(os.path.join(above, name)):
                raise Refused("a Cargo configuration above the source would apply: %s" % os.path.join(above, name))
    return toolchain_root


def child_environment(toolchain_bin, cargo_home, work):
    """The whole environment of every child. Nothing is taken from the caller's."""
    return {
        "PATH": os.pathsep.join([toolchain_bin, "/usr/bin", "/bin"]),
        "HOME": os.path.join(work, "home"),
        "TMPDIR": os.path.join(work, "tmp"),
        "CARGO_HOME": cargo_home,
        "CARGO_TARGET_DIR": os.path.join(work, "target"),
        "CARGO_TERM_COLOR": "never",
        "CARGO_NET_OFFLINE": "true",
        "CARGO_INCREMENTAL": "0",
        "RUST_BACKTRACE": "1",
        "LANG": "C.UTF-8",
        "LC_ALL": "C.UTF-8",
    }


def free_bytes(path):
    state = os.statvfs(path)
    return state.f_bavail * state.f_frsize


def check_storage(src, out, work, cargo_home, planned_cases):
    """Storage is bounded, and what a build writes cannot take the receipts' space (B4).

    Children can write only W. W and the dependency cache must be on a
    filesystem of a fixed size, and not on the filesystem of the source and
    the receipts, which no child can write. So a build that fills its disk
    fails by itself: the source can still be put back and the receipts closed.
    """
    device = {name: os.stat(path).st_dev for name, path in
              (("src", src), ("out", out), ("work", work), ("cargo_home", cargo_home))}
    if {device["work"], device["cargo_home"]} & {device["src"], device["out"]}:
        raise Refused("STORAGE: --work and --cargo-home must be on another filesystem than --src and --out")
    sizes = {}
    for name, path in (("work", work), ("cargo_home", cargo_home)):
        state = os.statvfs(path)
        sizes[name] = state.f_blocks * state.f_frsize
        if sizes[name] > BUILD_FS_MAX_BYTES:
            raise Refused("STORAGE NOT BOUNDED: the filesystem of --%s holds %d bytes; at most %d is accepted"
                          % (name.replace("_", "-"), sizes[name], BUILD_FS_MAX_BYTES))
    if free_bytes(work) < MIN_FREE_START_BYTES:
        raise Refused("LESS THAN %d BYTES FREE for build output" % MIN_FREE_START_BYTES)
    reserve = (2 * planned_cases + 4) * LOG_LIMIT_BYTES + RECEIPT_MARGIN_BYTES
    if free_bytes(out) < reserve:
        raise Refused("LESS THAN %d BYTES FREE for the receipts: every log at its limit would not fit" % reserve)
    return {"filesystem_of": device, "build_filesystem_bytes": sizes,
            "build_filesystem_limit_bytes": BUILD_FS_MAX_BYTES,
            "free_for_build_output_at_start": free_bytes(work),
            "free_for_receipts_at_start": free_bytes(out), "receipt_reserve_bytes": reserve,
            "note": "No child can write the filesystem of the source and the receipts. Every log at its "
                    "limit fits in the reserve that was free there at the start."}


def kind_of(mode):
    if stat.S_ISLNK(mode):
        return "link"
    if stat.S_ISDIR(mode):
        return "folder"
    if stat.S_ISREG(mode):
        return "file"
    return "special"


def check_cache(cargo_home, lock_text):
    """Before anything is unpacked, the dependency cache holds only what may be there (B2).

    Allowed: registry/cache with crate files that Cargo.lock names, each with
    its checksum there; registry/index; Cargo's own lock and bookkeeping
    files. Refused: any other entry in its root or in registry/ (a Cargo
    configuration, credentials, a folder of binaries, Git checkouts), any link
    or special file anywhere, and sources already unpacked: nothing ties
    those to the lock file. Cargo unpacks the checked crate files itself, in
    the preparation.
    """
    want = {}
    for block in lock_text.split("[[package]]")[1:]:
        name = re.search(r'^name = "(.+)"$', block, re.M)
        version = re.search(r'^version = "(.+)"$', block, re.M)
        checksum = re.search(r'^checksum = "([0-9a-f]{64})"$', block, re.M)
        if name and version and checksum:
            want["%s-%s.crate" % (name.group(1), version.group(1))] = checksum.group(1)
    for folder, allowed in ((cargo_home, CARGO_ROOT), (os.path.join(cargo_home, "registry"), CARGO_REGISTRY)):
        if not os.path.isdir(folder):
            raise Refused("THE DEPENDENCY CACHE HAS NO %s" % os.path.relpath(folder, os.path.dirname(cargo_home)))
        for name in sorted(os.listdir(folder)):
            if name not in allowed:
                raise Refused("THE DEPENDENCY CACHE HOLDS AN ENTRY THAT IS NOT ALLOWED THERE: %s"
                              % os.path.relpath(os.path.join(folder, name), cargo_home))
    for name in CARGO_BOOKKEEPING + ("registry/CACHEDIR.TAG",):
        path = os.path.join(cargo_home, name)
        if os.path.lexists(path) and kind_of(os.lstat(path).st_mode) != "file":
            raise Refused("THE DEPENDENCY CACHE: %s is not a plain file" % name)
    for root, dirs, files in os.walk(cargo_home):
        for name in dirs + files:
            path = os.path.join(root, name)
            if kind_of(os.lstat(path).st_mode) not in ("file", "folder"):
                raise Refused("THE DEPENDENCY CACHE HOLDS A LINK OR A SPECIAL FILE: %s"
                              % os.path.relpath(path, cargo_home))
    for _root, _dirs, files in os.walk(os.path.join(cargo_home, "registry", "src")):
        if files:
            raise Refused("THE DEPENDENCY CACHE HOLDS registry/src ALREADY: remove it; only downloaded crate "
                          "files are accepted")
    checked = 0
    for root, dirs, files in os.walk(os.path.join(cargo_home, "registry", "cache")):
        dirs.sort()
        for file in sorted(files):
            path = os.path.join(root, file)
            if not file.endswith(".crate") or file not in want:
                raise Refused("A FILE AMONG THE CRATE FILES THAT Cargo.lock DOES NOT NAME: %s" % file)
            if sha256_file(path) != want[file]:
                raise Refused("A CRATE FILE WITHOUT ITS Cargo.lock CHECKSUM: %s" % file)
            checked += 1
    return {"crate_files_checked_against_Cargo_lock": checked, "packages_with_a_checksum_in_Cargo_lock": len(want)}


CACHE_AREAS = ("registry/cache/", "registry/index/", "registry/src/")
CACHE_REST = "the rest: the root, registry/ itself, Cargo's own lock and bookkeeping files"


def dependency_inputs(cargo_home, tick=None):
    """Everything the dependency cache holds: every entry's kind, mode, and content.

    {area: {"entries": n, "sha256_of_the_list": h}}. A file is listed with its
    mode, size and SHA-256; a folder with its mode; a link with its target.
    So a new entry, a missing one, a changed file, a changed mode, and a file
    turned into a link or a folder, each change the result.
    """
    lists = {area: [] for area in CACHE_AREAS + (CACHE_REST,)}

    def walk(folder, prefix):
        for entry in sorted(os.scandir(folder), key=lambda item: item.name):
            if tick:
                tick()
            rel = prefix + entry.name
            info = entry.stat(follow_symlinks=False)
            kind = kind_of(info.st_mode)
            if kind == "link":
                line = "link -> %s" % os.readlink(entry.path)
            elif kind == "folder":
                line = "folder %o" % stat.S_IMODE(info.st_mode)
            elif kind == "file":
                line = "file %o %d %s" % (stat.S_IMODE(info.st_mode), info.st_size, sha256_file(entry.path))
            else:
                line = "special %o" % info.st_mode
            area = next((area for area in CACHE_AREAS if (rel + "/").startswith(area)), CACHE_REST)
            lists[area].append("%s\t%s\n" % (line, rel))
            if kind == "folder":
                walk(entry.path, rel + "/")

    walk(cargo_home, "")
    return {area: {"entries": len(lines), "sha256_of_the_list": sha256("".join(lines).encode("utf-8", "replace"))}
            for area, lines in lists.items()}


def cache_top(cargo_home):
    """The names and kinds at the top of the dependency cache: where a configuration would appear."""
    found = []
    for folder, prefix in ((cargo_home, ""), (os.path.join(cargo_home, "registry"), "registry/")):
        for name in sorted(os.listdir(folder)):
            found.append("%s%s (%s)" % (prefix, name, kind_of(os.lstat(os.path.join(folder, name)).st_mode)))
    return found


def cache_top_problems(top):
    """What at the top of the cache is not allowed there, at any time."""
    allowed = set(CARGO_ROOT) | {"registry/" + name for name in CARGO_REGISTRY}
    return [entry for entry in top if entry.rsplit(" (", 1)[0] not in allowed]


# ===========================================================================
# The evidence itself (B1). Three checks, made once, at the end. A control is
# credited only if all three were made and found nothing changed.
# ===========================================================================
EXIT_OK, EXIT_NOT_EVERY_CASE, EXIT_REFUSED, EXIT_EVIDENCE, EXIT_RUNNER = 0, 10, 20, 30, 40
EXIT_WORDS = ("0: every case gave a result and evidence VALID; 10: evidence VALID, not every case gave a "
              "result; 20: refused to start; 30: evidence INVALID or NOT VERIFIED, or the final receipts "
              "could not be written; 40: the runner itself failed")
UNCHANGED, CHANGED = "unchanged", "CHANGED"
COULD_NOT, NOT_MADE = "COULD NOT BE COMPLETED", "NOT MADE"
EVIDENCE_CHECKS = ("raw logs", "dependency inputs", "source")
VALID, INVALID, NOT_VERIFIED, NOT_YET = "VALID", "INVALID", "NOT VERIFIED", "NOT YET CHECKED"


def checked(action):
    """One check: {"result": unchanged | CHANGED | COULD NOT BE COMPLETED | NOT MADE, "detail": [...]}.

    action returns (None or a reason it was not made, the list of what changed).
    A check that fails for any reason of its own is not a check that passed.
    """
    try:
        not_made, changed = action()
    except Exception as problem:
        return {"result": COULD_NOT, "detail": [repr(problem)]}
    if not_made:
        return {"result": NOT_MADE, "detail": [not_made]}
    return {"result": CHANGED if changed else UNCHANGED, "detail": list(changed)[:40]}


def evidence_of(checks):
    """The outcome of the evidence: VALID only if every required check was made and found no change."""
    reasons = []
    for name in EVIDENCE_CHECKS:
        result = checks.get(name, {"result": NOT_MADE, "detail": ["no such check was recorded"]})
        if result["result"] != UNCHANGED:
            reasons.append("%s: %s%s" % (name, result["result"],
                                          (" (%s)" % "; ".join(str(item) for item in result["detail"][:3]))
                                          if result["detail"] else ""))
    if any(checks.get(name, {}).get("result") == CHANGED for name in EVIDENCE_CHECKS):
        state = INVALID
    elif reasons:
        state = NOT_VERIFIED
    else:
        state = VALID
    return {"state": state, "reasons": reasons, "checks": {name: checks.get(name) for name in EVIDENCE_CHECKS}}


def final_eligibility(evidence, decided_at, until, signals_before, signals_now, alarms=()):
    """Whether the evidence may be VALID, decided after the last checks (R3-02).

    Checks that ended after their deadline, a signal that arrived while they
    ran, or a containment alarm: the evidence is not VALID (INVALID stays
    INVALID). What the checks found is kept as it was.
    """
    late = decided_at > until
    signalled = signals_now > signals_before
    reasons = (["the last checks ended after their deadline"] if late else []) \
        + (["a signal arrived while the last checks ran"] if signalled else []) + list(alarms)
    decided = dict(evidence)
    decided["eligibility"] = {"decided_after_the_checks": True, "within_the_deadline": not late,
                              "a_signal_during_the_checks": signalled, "containment_alarms": list(alarms),
                              "after_the_decision": "A signal after this decision does not change it: the "
                                                    "final files are written after it, and are whole only "
                                                    "if SHA256SUMS lists progress.jsonl and every hash "
                                                    "holds. Its last line records the signals seen by "
                                                    "then."}
    if reasons:
        decided["reasons"] = list(evidence["reasons"]) + reasons
        if evidence["state"] == VALID:
            decided["state"] = NOT_VERIFIED
    return decided


def changed_logs(folder, recorded, tick=None):
    """The logs that are no longer the bytes that were hashed as they were read: [(name, sha256)] -> names."""
    wrong = []
    for name, digest in recorded:
        if tick:
            tick()
        path = os.path.join(folder, name)
        if not os.path.isfile(path) or os.path.islink(path):
            wrong.append("%s: missing" % name)
        elif sha256_file(path) != digest:
            wrong.append("%s: not the bytes that were read" % name)
    return wrong


def changed_inputs(sealed, now):
    """The areas of the dependency cache that are not as they were sealed."""
    return ["%s: %d entries, list %s; sealed with %d entries, list %s" % (
        area, now[area]["entries"], now[area]["sha256_of_the_list"][:16],
        sealed[area]["entries"], sealed[area]["sha256_of_the_list"][:16])
        for area in sorted(sealed) if now.get(area) != sealed[area]]


def exit_status(report):
    """0 every case gave a result and the evidence is valid; 30 the evidence is not valid; 10 otherwise."""
    if report["evidence"]["state"] != VALID:
        return EXIT_EVIDENCE
    return EXIT_OK if campaign_state(report)[0] == "COMPLETE" else EXIT_NOT_EVERY_CASE


def tool_identity(name, path_list):
    for folder in path_list.split(os.pathsep):
        candidate = os.path.join(folder, name)
        if os.path.isfile(candidate) and os.access(candidate, os.X_OK):
            real = canonical(candidate)
            return {"found_as": candidate, "path": real, "sha256": sha256_file(real)}
    return {"found_as": None}


# ===========================================================================
# The campaign
# ===========================================================================
def plan_of(ready):
    """Every case of the campaign, in its fixed order."""
    cases = []
    for attempt in range(1, BASELINE_REPEATS + 1):
        cases.append({"id": "B%d" % attempt, "type": "baseline", "what": "the unchanged candidate"})
    for mutant in MUTANTS:
        case = {"type": "mutant"}
        case.update(mutant)
        if ready:
            entry = ready[mutant["id"]]
            case.update({"mutant_sha256": entry["sha256"], "mutant_blob": entry["blob"],
                         "diff_sha256": entry["diff_sha256"], "changed_lines": entry["changed_lines"],
                         "expected_at": [
                             {key: oracle[key]
                              for key in ("test", "file", "lo", "hi", "column", "text", "how", "says", "via")}
                             for oracle in entry["oracles"]]})
        cases.append(case)
    cases.append({"id": "BF", "type": "baseline", "what": "the unchanged candidate again, after the last restore"})
    for number, case in enumerate(cases, 1):
        case["no"] = number
        case["name"] = "%03d-%s" % (number, case["id"])
    return cases


STARTED = "STARTED, NO RESULT YET"
EVERY_CASE = "EVERY PLANNED CASE WAS RUN"
NOT_RUN = ("NOT_RUN", STARTED)
# Run to an end, but saying nothing about the mutant. Never a kill, never a survivor.
NO_RESULT = ("INTERRUPTED", "QUARANTINED", "RUNNER ERROR", "TIMEOUT", "CAMPAIGN DEADLINE", "LOG LIMIT",
             "OUTPUT HELD OPEN", "EXECUTION FAILURE", "STORAGE EXHAUSTED", "INCOMPLETE OUTPUT",
             "CONFLICTING OUTPUT", "STALE BUILD", "INPUT CHANGED", "NOT APPLIED", "BASELINE NOT CLEAN",
             "BASELINE LEFT A PROCESS BEHIND")
# After one of these the campaign stops: going on could not give a credited result.
STOPS_THE_CAMPAIGN = ("CAMPAIGN DEADLINE", "STORAGE EXHAUSTED")


class Campaign:
    def __init__(self, args):
        self.script_path = canonical(__file__)
        with open(self.script_path, "rb") as handle:
            self.script_bytes = handle.read()
        self.script_sha256 = sha256(self.script_bytes)
        self.launcher = canonical(args.launcher)
        self.src = canonical(args.src)
        self.out = canonical(args.out)
        self.work = canonical(args.work)
        self.cargo_home = canonical(args.cargo_home)
        self.toolchain_bin = canonical(args.toolchain_bin)
        self.receipt = os.path.join(self.out, "receipt")
        self.results = {}
        self.ended = ""
        self.filtered_out = None
        self.baseline_executables = set()
        self.started_utc = now_utc()
        self.started = time.monotonic()
        self.before = None
        self.tool_logs = []  # (name, sha256) of each log written before the first case
        self.inputs_given = None  # the dependency cache as it was handed over
        self.inputs_sealed = None  # the dependency cache once unpacked, as it was sealed
        self.top_sealed = None
        self.inputs_alarm = []
        self.containment_alarm = []
        self.core_pattern = None
        self.clock = time.monotonic

    def work_left(self):
        """Seconds left for cases: the whole budget, less what is kept for the last checks (B4)."""
        return OVERALL_BUDGET_S - FINAL_RESERVE_S - (time.monotonic() - self.started)

    def progress(self, event, **more):
        record = {"at": now_utc(), "event": event}
        record.update(more)
        durable_append(os.path.join(self.receipt, "progress.jsonl"), json.dumps(record, sort_keys=True))

    def tool(self, argv, log_path, timeout_s, before=None):
        """One command before the first case. It must end by itself and leave nothing behind."""
        seconds, cut = allowed_time(timeout_s, self.work_left())
        if seconds < 1:
            raise Refused("NO TIME LEFT TO RUN %s" % argv[0])
        step = run_child(argv, self.src, self.env, log_path, seconds, before or self.before, cut)
        self.tool_logs.append((step["log"], step["log_sha256"]))
        if step["lifecycle"] != NORMAL or step["exit"] != 0:
            raise Refused("%s DID NOT END CLEANLY (%s, exit %s): see %s" % (
                " ".join(argv[:2]), step["lifecycle"], step["exit"], step["log"]))
        return step

    # -- before anything is built ------------------------------------------
    def start(self):
        user = require_audit_identity()
        toolchain_root = check_paths(self.script_path, self.launcher, self.src, self.out, self.work,
                                     self.cargo_home, self.toolchain_bin)
        with open(self.launcher, "rb") as handle:
            launcher_bytes = handle.read()
        interfaces = sorted(name for _index, name in socket.if_nameindex())
        if interfaces != ["lo"]:
            raise Refused("NETWORK PRESENT: interfaces %s. Run inside a network namespace that has only "
                          "loopback." % ", ".join(interfaces))
        for folder in (self.out, self.work):
            os.makedirs(folder, exist_ok=True)
        checks = selftest(self.work)
        failed = [name for name, passed, _seen in checks if not passed]
        if failed:
            raise Refused("THE RUNNER'S OWN CHECKS FAILED: %s" % "; ".join(failed))
        if os.listdir(self.work):
            raise Refused("THE RUNNER'S OWN CHECKS LEFT SOMETHING IN --work")
        view = check_view(self.src, self.out, self.work, self.cargo_home)
        self.reference = identity(self.src)
        self.pristine = {}
        for rel in CLEAN:
            with open(os.path.join(self.src, rel), "rb") as handle:
                self.pristine[rel] = handle.read()
        self.ready = prepare(self.pristine)
        self.cases = plan_of(self.ready)
        storage = check_storage(self.src, self.out, self.work, self.cargo_home, len(self.cases))
        for folder in ("cases", "logs", "diffs", "provenance"):
            os.makedirs(os.path.join(self.receipt, folder))
        for folder in ("home", "tmp", "target"):
            os.makedirs(os.path.join(self.work, folder))
        self.env = child_environment(self.toolchain_bin, self.cargo_home, self.work)
        cargo = canonical(os.path.join(self.toolchain_bin, "cargo"))
        rustc = canonical(os.path.join(self.toolchain_bin, "rustc"))
        tools = {"cargo": {"path": cargo, "sha256": sha256_file(cargo)},
                 "rustc": {"path": rustc, "sha256": sha256_file(rustc)},
                 "cc": tool_identity("cc", self.env["PATH"]), "ld": tool_identity("ld", self.env["PATH"])}
        with open(os.path.join(self.src, "Cargo.lock"), "rb") as handle:
            cache_check = check_cache(self.cargo_home, handle.read().decode("utf-8"))
        self.inputs_given = dependency_inputs(self.cargo_home)
        containment, before_the_preparation, self.before, seal = confine(
            self.src, self.out, self.work, self.cargo_home, toolchain_root)
        self.core_pattern = containment["core_dumps"]["host_core_pattern"]
        containment["probes_in_this_process"] = probe_containment(self.script_path, self.src, self.work)
        durable_write(os.path.join(self.receipt, "selftest.txt"),
                      "".join("%s  %s  [%s]\n" % ("PASS" if passed else "FAIL", name, seen)
                              for name, passed, seen in checks))
        durable_write(os.path.join(self.receipt, "input-manifest.txt"), manifest_text(self.reference))
        # The exact bytes of this script and of the launcher's file, kept with the receipts.
        durable_write(os.path.join(self.receipt, "provenance", "rs01_mutation_audit.py"), self.script_bytes)
        durable_write(os.path.join(self.receipt, "provenance", "launcher-file"), launcher_bytes)
        for mutant in MUTANTS:
            durable_write(os.path.join(self.receipt, "diffs", mutant["id"] + ".diff"),
                          self.ready[mutant["id"]]["diff"])
        self.progress("started", script_sha256=self.script_sha256, launcher_sha256=sha256(launcher_bytes))
        # The same walls, and the launcher's, seen from inside a child (B3). This is the
        # first command the runner starts: nothing is unpacked, built or run before it.
        top_given = cache_top(self.cargo_home)
        log_path = os.path.join(self.receipt, "logs", "000-child-probe.log")
        scratch = os.path.join(self.work, "tmp")
        crate_files = sorted(os.path.join(root, name)
                             for root, _dirs, files in os.walk(os.path.join(self.cargo_home, "registry", "cache"))
                             for name in files)
        if not crate_files:
            raise Refused("THE DEPENDENCY CACHE HOLDS NO CRATE FILE")
        own_socket = os.path.join(scratch, ".rs01-audit-probe.sock")
        listener = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        try:
            listener.bind(own_socket)
            listener.listen(1)
            self.tool([sys.executable, "-I", "-c", CHILD_PROBE, json.dumps({
                "source_file": os.path.join(self.src, RUN), "receipt": self.receipt, "scratch": scratch,
                "cargo_home": self.cargo_home, "cache_file": crate_files[0], "own_socket": own_socket,
                "host_socket": HOST_CANARY_SOCKET, "host_abstract": HOST_CANARY_ABSTRACT,
                "host_file": HOST_CANARY_FILE, "view_file": VIEW_CANARY_FILE})], log_path, TOOL_TIMEOUT_S)
        finally:
            listener.close()
            if os.path.lexists(own_socket):
                os.unlink(own_socket)
        seen = [line[6:] for line in read_log(log_path).split("\n") if line.startswith("PROBE ")]
        if len(seen) != 1 or json.loads(seen[0]) != CHILD_PROBE_WANT:
            raise Refused("CONTAINMENT PROBE FAILED IN A CHILD: see %s" % log_path)
        if os.path.lexists(os.path.join(self.receipt, ".rs01-audit-child-probe")):
            raise Refused("CONTAINMENT PROBE FAILED: a child wrote into the receipts")
        if cache_top(self.cargo_home) != top_given:
            raise Refused("CONTAINMENT PROBE FAILED: a child changed the top of the dependency cache")
        os.unlink(os.path.join(scratch, ".rs01-audit-child-probe"))
        containment["probes_in_a_child"] = json.loads(seen[0])
        for tool, argv in (("cargo", ["cargo", "-V"]), ("rustc", ["rustc", "-Vv"])):
            log_path = os.path.join(self.receipt, "logs", "000-%s-version.log" % tool)
            tools[tool]["ran"] = self.tool(argv, log_path, TOOL_TIMEOUT_S)
            tools[tool]["version"] = read_log(log_path).strip()
        release = re.search(r"^release: (\S+)$", tools["rustc"]["version"], re.M)
        if not release or release.group(1) != RUSTC_RELEASE:
            raise Refused("NOT THE TOOLCHAIN: rustc release %s, not %s" % (
                release.group(1) if release else "unknown", RUSTC_RELEASE))
        if not tools["cargo"]["version"].startswith("cargo %s " % RUSTC_RELEASE):
            raise Refused("NOT THE TOOLCHAIN: %s, not cargo %s" % (tools["cargo"]["version"], RUSTC_RELEASE))
        host = re.search(r"^host: (\S+)$", tools["rustc"]["version"], re.M)
        if not host or host.group(1) != HOST_TRIPLE:
            raise Refused("NOT THE HOST: %s, not %s" % (host.group(1) if host else "unknown", HOST_TRIPLE))
        # The preparation (B2): Cargo unpacks the checked crate files. Nothing is
        # compiled and no build script runs. Then the cache is sealed: read-only
        # for this process and for everything it starts from here on.
        prepared = self.tool(PREPARE, os.path.join(self.receipt, "logs", "000-prepare.log"),
                             PREPARE_TIMEOUT_S, before_the_preparation)
        top = cache_top(self.cargo_home)
        if cache_top_problems(top):
            raise Refused("THE PREPARATION LEFT AN ENTRY THAT IS NOT ALLOWED IN THE DEPENDENCY CACHE: %s"
                          % ", ".join(cache_top_problems(top)))
        self.inputs_sealed = dependency_inputs(self.cargo_home)
        for area in ("registry/cache/", "registry/index/"):
            if self.inputs_sealed[area] != self.inputs_given[area]:
                raise Refused("THE PREPARATION CHANGED %s, WHICH IT MAY ONLY READ" % area)
        if not self.inputs_sealed["registry/src/"]["entries"]:
            raise Refused("THE PREPARATION UNPACKED NOTHING")
        seal()
        self.top_sealed = top
        self.progress("dependency cache sealed", top=top)
        containment["probes_in_this_process_after_the_seal"] = probe_the_seal(self.cargo_home)
        self.header = {
            "reference": REFERENCE,
            "script_sha256": self.script_sha256,
            "script_copy": "provenance/rs01_mutation_audit.py",
            "launcher": {"path": self.launcher, "bytes": len(launcher_bytes), "sha256": sha256(launcher_bytes),
                         "copy": "provenance/launcher-file",
                         "note": "The exact bytes of the file named as the launcher. That this file is the "
                                 "workflow that ran is established by the hosting service, not by this script."},
            "candidate": CANDIDATE,
            "candidate_note": "A commit id cannot be computed from an export. The script verifies the tree; "
                              "that the commit has this tree is read from Git by whoever makes the export.",
            "tree": TREE,
            "tree_verified_from_the_files": True,
            "files_in_tree": len(self.reference),
            "input_manifest_sha256": sha256(manifest_text(self.reference).encode("utf-8")),
            "clean_sha256": CLEAN,
            "commands": {"prepare": " ".join(PREPARE), "build": " ".join(BUILD), "test": " ".join(TEST),
                         "working_directory": self.src},
            "test_target": TARGET_LABEL,
            "inventory": INVENTORY,
            "inventory_note": "Sixteen tests are selected. Fifteen are run. T10 is ignored and is not run.",
            "limits": {
                "overall_budget_s": OVERALL_BUDGET_S, "kept_for_the_last_checks_s": FINAL_RESERVE_S,
                "kept_in_each_case_for_its_close_s": CASE_CLOSE_S,
                "first_build_timeout_s": FIRST_BUILD_TIMEOUT_S, "build_timeout_s": BUILD_TIMEOUT_S,
                "test_timeout_s": TEST_TIMEOUT_S, "prepare_timeout_s": PREPARE_TIMEOUT_S,
                "quiesce_s": QUIESCE_S, "log_limit_bytes": LOG_LIMIT_BYTES,
                "build_filesystem_limit_bytes": BUILD_FS_MAX_BYTES,
                "min_free_start_bytes": MIN_FREE_START_BYTES, "min_free_case_bytes": MIN_FREE_CASE_BYTES,
                "exhausted_below_bytes": EXHAUSTED_BYTES, "baseline_repeats": BASELINE_REPEATS,
                "note": "No step may run past the campaign's deadline: each is given the lesser of its own "
                        "limit and the time left. The last checks stop, as not completed, at the whole "
                        "budget. An alarm at the whole budget ends a step or a wait between cases that "
                        "passed it. The checks before the first case and the writing of the final files "
                        "are not timed: they read and write a known, bounded set of files.",
            },
            "exit_status": EXIT_WORDS,
            "cleanup": "The script deletes nothing but its own probe files and its own checks' folder. Build "
                       "output and scratch stay in --work; only receipt/ is evidence.",
            "manifest": "SHA256SUMS lists every file of the receipt but itself. progress.jsonl grows during "
                        "the run and is listed in the final SHA256SUMS only. Final receipts are whole only "
                        "if SHA256SUMS lists progress.jsonl, every hash in it holds, and there is no file "
                        "FINAL-RECEIPTS-INCOMPLETE.txt.",
            "started": self.started_utc,
        }
        environment = {
            "paths": {"src": self.src, "out": self.out, "work": self.work, "cargo_home": self.cargo_home,
                      "toolchain_bin": self.toolchain_bin, "script": self.script_path, "launcher": self.launcher},
            "child_environment": self.env,
            "child_environment_note": "This is the whole environment of every child. The caller's "
                                      "environment is not passed on and is not recorded.",
            "toolchain": tools,
            "cargo_configuration": {
                "in_the_tree": ".cargo/config.toml, blob %s (part of the verified tree)" % (
                    self.reference.get(".cargo/config.toml", ("", "absent"))[1]),
                "above_the_source": "none (checked)",
                "in_the_dependency_cache": "none (checked at the start and after the preparation; the cache "
                                           "is then read-only, and its top is looked at before every case)"},
            "dependency_cache_checked": cache_check,
            "dependency_cache_as_given": self.inputs_given,
            "dependency_cache_as_sealed": self.inputs_sealed,
            "dependency_cache_top_as_sealed": self.top_sealed,
            "dependency_cache_note": "Given: the downloaded crate files and the index. The preparation, and "
                                     "only it, may add the unpacked sources and Cargo's own lock and "
                                     "bookkeeping files; it left the crate files and the index as given. "
                                     "After the seal nothing in the cache may change: Cargo is given a "
                                     "read-only cache and writes no bookkeeping.",
            "preparation": prepared,
            "network_interfaces": interfaces,
            "view": view,
            "storage": storage,
            "containment": containment,
            "user": user,
            "kernel": platform.release(),
            "machine": platform.machine(),
            "python": platform.python_version(),
            "cpus": os.cpu_count(),
            "pid": os.getpid(),
        }
        durable_write(os.path.join(self.receipt, "environment.json"), as_json(environment))
        durable_write(os.path.join(self.receipt, "plan.json"), as_json({"header": self.header, "cases": self.cases}))
        self.progress("planned", cases=len(self.cases))
        self.publish()

    # -- one case -----------------------------------------------------------
    def state(self, allowed=None):
        return differences(self.reference, scan(self.src), allowed)

    def write_source(self, rel, data):
        fd = os.open(os.path.join(self.src, rel), os.O_WRONLY | os.O_TRUNC)
        try:
            view = memoryview(data)
            while view:
                view = view[os.write(fd, view):]
            os.fsync(fd)
        finally:
            os.close(fd)

    def run_case(self, case):
        """Returns the case's result. Raises Refused to end the campaign after it."""
        name = case["name"]
        mutant = case if case["type"] == "mutant" else None
        entry = {key: case[key] for key in case if key not in ("edits", "bytes")}
        entry.update({"started": now_utc(), "verdict": STARTED, "steps": {}})
        self.results[case["id"]] = entry
        stop = None
        mutated = False
        allowed = {}
        self.progress("case started", case=name)
        self.publish()  # so that a runner killed outright leaves the case it was in on record
        try:
            before = self.state()
            if before:
                entry["verdict"] = "INPUT CHANGED"
                entry["problems"] = before[:20]
                raise Refused("THE SOURCE IS NOT THE CANDIDATE BEFORE %s" % name)
            if mutant:
                ready = self.ready[case["id"]]
                allowed = {case["file"]: ready["blob"]}
                self.progress("source mutated", case=name, file=case["file"])
                mutated = True
                self.write_source(case["file"], ready["bytes"])
                wrong = self.state(allowed)
                if wrong:
                    entry["verdict"] = "NOT APPLIED"
                    entry["problems"] = wrong[:20]
                    raise Refused("THE MUTANT %s WAS NOT WRITTEN AS DECLARED" % name)
            entry["verdict"] = self.build_and_test(case, entry)
        except Interrupted as signalled:
            entry["verdict"] = "INTERRUPTED"
            entry["problems"] = [str(signalled)]
            entry["steps"]["interrupted"] = getattr(signalled, "step", None)
            stop = Refused("INTERRUPTED BY %s DURING %s" % (signalled, name))
        except Quarantine as left:
            entry["verdict"] = "QUARANTINED"
            entry["problems"] = ["a process of this case could not be ended; the source was NOT restored"]
            entry["restored"] = False
            try:
                entry["quarantine"] = json.loads(str(left))
                durable_write(os.path.join(self.receipt, "QUARANTINE.txt"),
                              "Case %s left a process that could not be ended within %d s.\n"
                              "The source folder was not restored and must not be used again.\n%s"
                              % (name, QUIESCE_S, left))
            finally:
                self.close_case(entry, name)
            raise Refused("QUARANTINED AT %s" % name)
        except Refused as problem:
            stop = problem
            if entry["verdict"] == STARTED:
                entry["verdict"] = "RUNNER ERROR"
                entry["problems"] = [str(problem)]
        except Exception as problem:  # a fault of the runner itself: restore, record, stop
            entry["verdict"] = "RUNNER ERROR"
            entry["problems"] = [repr(problem)]
            stop = Refused("RUNNER ERROR AT %s: %r" % (name, problem))
        # No child is alive here. Look at the source as the case left it; then put
        # the file back whatever that look said or did; then check the whole source.
        after = []
        failed_restore = []
        if mutated:
            try:
                after = self.state(allowed)
            except Exception as problem:
                after = ["the source could not be read after the case: %r" % (problem,)]
            entry["source_during_case"] = "only the declared mutation" if not after else after[:20]
            for attempt in (1, 2):
                try:
                    self.write_source(case["file"], self.pristine[case["file"]])
                    failed_restore = []
                    break
                except Exception as problem:
                    failed_restore = ["writing the file back failed (attempt %d): %r" % (attempt, problem)]
                    entry.setdefault("notes", []).append(failed_restore[0])
        try:
            failed_restore += ["after the restore: " + line for line in self.state()[:20]]
        except Exception as problem:
            failed_restore.append("the source could not be read after the restore: %r" % (problem,))
        entry["restored"] = not failed_restore
        if after and entry["verdict"] not in ("NOT APPLIED", "INPUT CHANGED"):
            entry["verdict_before_input_check"] = entry["verdict"]
            entry["verdict"] = "INPUT CHANGED"
            entry["problems"] = after[:20]
            stop = stop or Refused("THE SOURCE CHANGED DURING %s BEYOND ITS MUTATION" % name)
        if failed_restore:
            entry["problems"] = entry.get("problems", []) + failed_restore
            stop = stop or Refused("%s %s" % (
                "RESTORE FAILED AFTER" if mutated else "THE SOURCE IS NOT THE CANDIDATE AT", name))
            try:
                durable_write(os.path.join(self.receipt, "SOURCE-NOT-THE-CANDIDATE.txt"),
                              "After case %s the source is not the candidate (%s). It must not be used again.\n%s\n"
                              % (name, "this case's mutation was not undone" if mutated
                                 else "the runner had not mutated it in this case", "\n".join(failed_restore)))
            except Exception:
                pass
        self.close_case(entry, name)
        if stop:
            raise stop
        return entry

    def close_case(self, entry, name):
        entry["finished"] = now_utc()
        durable_write(os.path.join(self.receipt, "cases", name + ".json"), as_json(entry))
        self.progress("case finished", case=name, verdict=entry["verdict"],
                      cleanup=bool(entry.get("cleanup")))
        self.publish()

    def step(self, entry, which, argv, log_path, limit_s):
        """Run one step of a case. Returns (the step, a no-result verdict or None).

        A step is given the lesser of its own limit and the time the campaign
        has left (B4). Its output is read only if the command ended by itself
        and its output ended (section 5).
        """
        seconds, cut = allowed_time(limit_s, self.work_left())
        if seconds < 1:
            entry["problems"] = ["the campaign's deadline left no time to start the %s step" % which]
            return None, "CAMPAIGN DEADLINE (%s, not started)" % which
        step = run_child(argv, self.src, self.env, log_path, seconds, self.before, cut)
        entry["steps"][which] = step
        verdict = step_no_result(step, which)
        if step["cleanup_required"]:
            entry.setdefault("notes", []).append(
                "%s: %s (ended by: %s); alive before any signal: %d; signals sent: %d" % (
                    step["log"], step["lifecycle"], step["ended_by"], step["alive_before_cleanup_count"],
                    step["cleanup_signals_count"]))
        if verdict:
            entry["problems"] = ["the %s step: %s (ended by: %s, signal %s)" % (
                which, step["lifecycle"], step["ended_by"], step["signal"])]
        elif step["lifecycle"] == NORMAL_LEFT:
            entry.setdefault("cleanup", []).append({
                "step": which, "lifecycle": step["lifecycle"],
                "alive_before_cleanup": step["alive_before_cleanup_count"],
                "signals_sent": step["cleanup_signals_count"],
                "what_it_means": "The command ended by itself and its output ended, so its output is read. "
                                 "A process it had started was still alive and was ended by the runner. "
                                 "This is a qualified observation: it is not a clean execution, and it "
                                 "says nothing about the correctness of the processes' own lifecycle."})
        free = free_bytes(self.work)
        if free < EXHAUSTED_BYTES:
            entry.setdefault("problems", []).append(
                "only %d bytes are left for build output after the %s step" % (free, which))
            verdict = "STORAGE EXHAUSTED (%s)" % which
        return step, verdict

    def build_and_test(self, case, entry):
        name = case["name"]
        first = not self.baseline_executables
        log_path = os.path.join(self.receipt, "logs", name + ".build.log")
        build, stopped = self.step(entry, "build", BUILD, log_path,
                                   FIRST_BUILD_TIMEOUT_S if first else BUILD_TIMEOUT_S)
        if stopped:
            return stopped
        text = read_log(log_path)
        if "No space left on device" in text:
            entry["problems"] = ["the build's output says: No space left on device"]
            return "STORAGE EXHAUSTED (build)"
        built = examine_build(text, build["exit"])
        compiled = bool(RE_COMPILING.search(text))
        entry["crate_recompiled"] = compiled
        wanted = case.get("build_error", "")
        if built["state"] == "BUILD ERROR":
            entry["compiler_errors"] = [line for line in text.split("\n") if line.startswith("error")][:10]
            if wanted and wanted in text:
                return "BUILD ERROR, AS EXPECTED (not a kill)"
            return "BUILD ERROR"
        if built["state"] != "OK":
            entry["problems"] = built.get("problems", [])
            return "%s (build)" % built["state"]
        executable = built["executable"]
        if (not os.path.isabs(executable) or not os.path.isfile(executable)
                or not within(canonical(executable), os.path.join(self.work, "target"))):
            entry["problems"] = ["the test executable is not in the build folder: %s" % executable]
            return "CONFLICTING OUTPUT (build)"
        entry["test_executable"] = {"path": executable, "sha256": sha256_file(executable)}
        entry["test_executable"]["byte_for_byte_a_baseline"] = (
            entry["test_executable"]["sha256"] in self.baseline_executables)
        if case["type"] == "mutant":
            if not compiled:
                entry["problems"] = ["the crate was not recompiled after the mutation"]
                return "STALE BUILD"
            if entry["test_executable"]["sha256"] in self.baseline_executables:
                entry["problems"] = ["the test executable is byte for byte a baseline's"]
                return "STALE BUILD"
        elif (first or case["id"] == "BF") and not compiled:
            entry["problems"] = ["the crate was not compiled for this baseline"]
            return "STALE BUILD"
        log_path = os.path.join(self.receipt, "logs", name + ".test.log")
        test, stopped = self.step(entry, "test", TEST, log_path, TEST_TIMEOUT_S)
        if stopped:
            return stopped
        text = read_log(log_path)
        if "No space left on device" in text:
            entry["problems"] = ["the tests' output says: No space left on device"]
            return "STORAGE EXHAUSTED (test)"
        log = read_test_log(text)
        exam = examine(log, test["exit"], self.filtered_out)
        entry["test_status"] = exam["status"]
        entry["filtered_out"] = exam["filtered_out"]
        problems = list(exam["problems"])
        if exam["state"] == "OK" and (len(log["targets"]) != 1 or canonical(log["targets"][0][1]) != canonical(executable)):
            exam["state"] = "CONFLICTING OUTPUT"
            problems.append("the tests ran another executable than the one built")
        if exam["state"] == "OK" and RE_COMPILING.search(text):
            exam["state"] = "CONFLICTING OUTPUT"
            problems.append("the test step compiled the crate again")
        if exam["state"] != "OK":
            entry["problems"] = problems
            return "%s (test)" % exam["state"]
        if case["type"] == "baseline":
            failed = sorted(short for short, state in exam["status"].items() if state == "FAILED")
            if failed:
                entry["failed"] = failed
                return "BASELINE NOT CLEAN"
            if entry.get("cleanup"):
                # The unchanged candidate left a process behind, and nothing explains
                # it: this is not a clean baseline, and the campaign stops (section 5, F).
                entry["problems"] = ["the unchanged candidate left a process behind: see cleanup"]
                return "BASELINE LEFT A PROCESS BEHIND"
            if self.filtered_out is None:
                self.filtered_out = exam["filtered_out"]
            self.baseline_executables.add(entry["test_executable"]["sha256"])
            return "BASELINE CLEAN"
        judged = judge(case, self.ready[case["id"]]["oracles"], log, exam)
        entry.update({key: judged[key] for key in ("failed", "expected", "other_failed", "uncredited", "observed")})
        if wanted:
            entry["problems"] = ["this form was expected not to compile; it compiled"]
            return "EXPECTED BUILD ERROR DID NOT OCCUR: " + judged["verdict"]
        return judged["verdict"]

    # -- the whole campaign --------------------------------------------------
    def run(self):
        """Run every case. Returns the exit status."""
        try:
            for case in self.cases:
                check_signals()
                name = case["name"]
                if self.work_left() < CASE_RESERVE_S:
                    raise Refused("THE TIME BUDGET LEAVES NO ROOM FOR %s" % name)
                if free_bytes(self.work) < MIN_FREE_CASE_BYTES:
                    raise Refused("LESS THAN %d BYTES FREE FOR BUILD OUTPUT BEFORE %s" % (MIN_FREE_CASE_BYTES, name))
                if free_bytes(self.receipt) < 2 * LOG_LIMIT_BYTES + RECEIPT_MARGIN_BYTES:
                    raise Refused("TOO LITTLE FREE FOR THE RECEIPTS BEFORE %s" % name)
                if host_core_pattern() != self.core_pattern:
                    self.containment_alarm = ["before %s the host's core_pattern was %r; at the start %r" % (
                        name, host_core_pattern(), self.core_pattern)]
                    raise Refused("THE HOST'S CORE PATTERN CHANGED BEFORE %s" % name)
                top = cache_top(self.cargo_home)
                if top != self.top_sealed:
                    self.inputs_alarm = ["before %s the top of the dependency cache was %s; sealed as %s" % (
                        name, top, self.top_sealed)]
                    raise Refused("THE DEPENDENCY CACHE CHANGED BEFORE %s" % name)
                entry = self.run_case(case)
                say("%-10s %s%s" % (name, entry["verdict"], "  [cleanup]" if entry.get("cleanup") else ""))
                if case["type"] == "baseline" and entry["verdict"] != "BASELINE CLEAN":
                    raise Refused("BASELINE NOT CLEAN AT %s: %s" % (name, entry["verdict"]))
                if case["id"] == "S1" and (entry["verdict"] != "KILLED" or entry.get("cleanup")):
                    raise Refused("THE SANITY GATE S1 WAS NOT CLEANLY KILLED (%s%s): the audit cannot attribute "
                                  "a failure, and stops" % (entry["verdict"],
                                                            ", cleanup needed" if entry.get("cleanup") else ""))
                if entry["verdict"].startswith(STOPS_THE_CAMPAIGN):
                    raise Refused("%s AT %s" % (entry["verdict"], name))
            self.ended = EVERY_CASE
        except Interrupted as signalled:
            self.ended = "INTERRUPTED BY %s BETWEEN CASES" % signalled
        except Refused as problem:
            self.ended = str(problem)
        except Exception as problem:  # the receipts are still closed, as far as that is possible
            self.ended = "RUNNER ERROR: %r" % (problem,)
        return self.close()

    def close(self, announce=say):
        """The last checks and the final receipts. Returns the exit status.

        If the final receipts cannot be written whole, nothing is credited: a
        marker says so, summary.txt is replaced by a text that credits
        nothing, and the status is that of evidence that is not verified.
        """
        try:
            self.publish(final=True)
        except Exception as problem:
            announce("THE FINAL RECEIPTS COULD NOT BE WRITTEN: %r" % (problem,))
            words = ("RS-01 MUTATION AUDIT   %s\n"
                     "THE FINAL RECEIPTS COULD NOT BE WRITTEN: %r\n"
                     "NOTHING IS CREDITED. results.json, summary.txt and SHA256SUMS in this folder may be\n"
                     "those of an earlier moment, or part of the final ones. They are not final receipts.\n"
                     % (REFERENCE, problem))
            for name in ("FINAL-RECEIPTS-INCOMPLETE.txt", "summary.txt"):
                path = os.path.join(self.receipt, name)
                try:
                    durable_write(path, words)
                except Exception:
                    # Not even that could be written (a full disk). A summary that might
                    # credit something must not be left: empty it in place, or remove it.
                    if name == "summary.txt":
                        try:
                            with open(path, "wb"):
                                pass
                        except Exception:
                            try:
                                os.unlink(path)
                            except Exception:
                                pass
            try:
                self.progress("final receipts incomplete", exit_status=EXIT_EVIDENCE, problem=repr(problem))
            except Exception:
                pass
            return EXIT_EVIDENCE
        return exit_status(self.report)

    # -- receipts -------------------------------------------------------------
    def recorded_logs(self):
        """(name, sha256) of every log, the hash being of the bytes as they were read."""
        logs = list(self.tool_logs)
        for entry in self.results.values():
            for step in entry.get("steps", {}).values():
                if step and step.get("log"):
                    logs.append((step["log"], step["log_sha256"]))
        return logs

    def final_checks(self):
        """The three checks of the evidence (B1). None may cost the final receipts.

        Each stops, as not completed, when the time kept for the last checks
        is used up or the whole budget is: past the budget nothing is VALID.
        """
        until = min(self.clock() + FINAL_RESERVE_S, self.started + OVERALL_BUDGET_S)
        signals_before = len(SIGNALS)
        signalled = "the campaign was ended by a signal: the last checks were not made" if SIGNALS else None

        def tick():
            if self.clock() > until:
                raise TimeoutError("the time kept for the last checks was used up")

        def logs():
            if signalled:
                return signalled, []
            return None, changed_logs(os.path.join(self.receipt, "logs"), self.recorded_logs(), tick)

        def inputs():
            if self.inputs_alarm:
                return None, self.inputs_alarm
            if signalled:
                return signalled, []
            if self.inputs_sealed is None:
                return "the dependency cache was never unpacked and sealed", []
            return None, changed_inputs(self.inputs_sealed, dependency_inputs(self.cargo_home, tick))

        def source():
            changed = []
            for entry in self.results.values():
                if entry["verdict"] == "QUARANTINED":
                    raise RuntimeError("case %s is quarantined: the source was not restored, on purpose, "
                                       "and is unresolved" % entry["id"])
                if entry.get("restored") is False:
                    changed.append("case %s: the source was not restored" % entry["id"])
                elif entry["verdict"] in ("INPUT CHANGED", "NOT APPLIED"):
                    changed.append("case %s: %s" % (entry["id"], entry["verdict"]))
            if changed:
                return None, changed
            if signalled:
                return signalled, []
            tick()
            try:
                return None, differences(self.reference, scan(self.src, tick))
            except Refused as problem:
                return None, [str(problem)]

        checks = {"raw logs": checked(logs), "dependency inputs": checked(inputs), "source": checked(source)}
        return checks, {"until": until, "signals_before": signals_before}

    def publish(self, final=False):
        """results.json, summary.txt and SHA256SUMS, rewritten whole at every case's start and end."""
        cases = []
        for case in self.cases:
            entry = self.results.get(case["id"])
            if entry is None:
                entry = {key: case[key] for key in case if key not in ("edits", "bytes")}
                entry["verdict"] = "NOT_RUN"
                if final:
                    entry["reason"] = self.ended
            cases.append(entry)
        report = dict(self.header)
        report.update({
            "state": "FINAL" if final else "IN PROGRESS: this file is rewritten at every case's start and end",
            "ended": self.ended if final else "",
            "finished": now_utc() if final else "",
            "seconds_used": round(time.monotonic() - self.started),
            "cases": cases,
            "evidence": {"state": NOT_YET, "reasons": ["the checks of the evidence are made once, at the end"],
                         "checks": {}},
        })
        if final:
            if self.core_pattern is not None and host_core_pattern() != self.core_pattern \
                    and not self.containment_alarm:
                self.containment_alarm = ["at the end the host's core_pattern was %r; at the start %r" % (
                    host_core_pattern(), self.core_pattern)]
            checks, frame = self.final_checks()
            # The decision, made after the checks, from the clock and the signals as they are now.
            report["evidence"] = final_eligibility(evidence_of(checks), self.clock(), frame["until"],
                                                   frame["signals_before"], len(SIGNALS), self.containment_alarm)
            report["finished"] = now_utc()
            report["seconds_used"] = round(time.monotonic() - self.started)
            report["exit_status_of_this_run"] = exit_status(report)
            # The last line of progress.jsonl, written before any final file: if it
            # cannot be written, no final file is. Nothing is appended after it, so
            # the final SHA256SUMS can list the file.
            self.progress("ended", ended=self.ended, campaign=campaign_state(report)[0],
                          evidence=report["evidence"]["state"], exit_status=report["exit_status_of_this_run"],
                          signals_seen=len(SIGNALS))
        self.report = report
        durable_write(os.path.join(self.receipt, "results.json"), as_json(report))
        durable_write(os.path.join(self.receipt, "summary.txt"), summary_text(report))
        sums = []
        for root, dirs, files in os.walk(self.receipt):
            dirs.sort()
            for name in sorted(files):
                if name == "SHA256SUMS" or name.endswith(".part") or (name == "progress.jsonl" and not final):
                    continue
                path = os.path.join(root, name)
                sums.append("%s  %s" % (sha256_file(path), os.path.relpath(path, self.receipt)))
        sums.sort(key=lambda line: line.split("  ", 1)[1])
        durable_write(os.path.join(self.receipt, "SHA256SUMS"), "\n".join(sums) + "\n")


def campaign_state(report):
    """(COMPLETE | RUN TO THE END, SOME CASES GAVE NO RESULT | NOT COMPLETE, not run, no result).

    This is about execution only: whether the cases were run and gave
    results. Whether the evidence is whole is evidence_of(), apart.
    """
    not_run = [case for case in report["cases"] if case["verdict"].startswith(NOT_RUN)]
    no_result = [case for case in report["cases"] if case["verdict"].startswith(NO_RESULT)]
    if report["ended"] != EVERY_CASE or not_run:
        return "NOT COMPLETE", not_run, no_result
    if no_result:
        return "RUN TO THE END, BUT SOME CASES GAVE NO RESULT", not_run, no_result
    return "COMPLETE", not_run, no_result


def summary_text(report):
    cases = report["cases"]
    mutants = [case for case in cases if case["type"] == "mutant"]
    baselines = [case for case in cases if case["type"] == "baseline"]
    state, not_run, no_result = campaign_state(report)
    final = report["state"] == "FINAL"
    evidence = report["evidence"]
    credit = final and evidence["state"] == VALID

    def some(listed):
        return ", ".join("%s" % case["id"] for case in listed[:14]) + (" ..." if len(listed) > 14 else "")

    lines = [
        "RS-01 MUTATION AUDIT   %s" % report["reference"],
        "state      %s" % report["state"],
        "script     sha256 %s" % report["script_sha256"],
        "launcher   sha256 %s" % report.get("launcher", {}).get("sha256", "-"),
        "candidate  %s" % report["candidate"],
        "tree       %s  (computed from the %d files at the start: equal)" % (
            report["tree"], report["files_in_tree"]),
        "build      %s" % report["commands"]["build"],
        "test       %s" % report["commands"]["test"],
        "started %s   finished %s   %d s used of %d" % (
            report["started"], report["finished"] or "-", report["seconds_used"], report["limits"]["overall_budget_s"]),
        "ended      %s" % (report["ended"] or "-"),
        "",
        "1. CAMPAIGN: was every planned case run, and did each give a result?",
        "   planned %d (baselines %d, mutants %d); gave a result %d; run but gave no result %d; not run %d" % (
            len(cases), len(baselines), len(mutants), len(cases) - len(not_run) - len(no_result),
            len(no_result), len(not_run)),
    ]
    if no_result:
        lines.append("   gave no result: " + ", ".join(
            "%s=%s" % (case["id"], case["verdict"]) for case in no_result[:14]) + (" ..." if len(no_result) > 14 else ""))
    if not_run:
        lines.append("   not run: " + some(not_run))
    lines += [
        "   CAMPAIGN %s" % state,
        "   This line says only whether the cases were run and gave results. It is not a verdict on the repair.",
        "",
        "2. EVIDENCE: are the logs, the dependency cache and the source what they were?",
    ]
    if final:
        for name in EVIDENCE_CHECKS:
            result = evidence["checks"].get(name) or {"result": NOT_MADE, "detail": []}
            lines.append("   %-18s %s%s" % (name, result["result"], (": " + "; ".join(
                str(item) for item in result["detail"][:3])) if result["detail"] else ""))
        lines.append("   EVIDENCE %s" % evidence["state"])
        if not credit:
            lines.append("   NOTHING BELOW IS CREDITED. The verdicts are kept as they were observed; "
                         "they are not rewritten.")
    else:
        lines.append("   EVIDENCE %s: these checks are made once, at the end. Nothing is credited before." % NOT_YET)
    lines += [
        "   This line says only whether the evidence is whole. It is not a verdict on the repair.",
        "",
        "3. BASELINES: the unchanged candidate",
    ]
    for case in baselines:
        test = case.get("steps", {}).get("test") or {}
        lines.append("   %-3s %-30s %s" % (case["id"], case["verdict"],
                                             ("%.1f s" % test["seconds"]) if test else ""))
    lines += ["", "4. EVERY MUTANT, in the order run   (after a test: + its failure was at the named assertion;",
              "   +? the same, but the part it came through was not confirmed; - it was not;",
              "   [cleanup] after a verdict: a qualified observation, see section 9)",
              "   no  id     kind             row verdict                                          expected -> met         others failed"]
    for case in mutants:
        expected = ",".join("%s%s" % (e["test"], ("+?" if e["part"].startswith("NOT") else "+") if e.get("met") else "-")
                            for e in case.get("expected", [])) \
            or ",".join(o["test"] for o in case.get("expected_at", [])) or "-"
        lines.append("   %3d %-6s %-16s %3s %-52s %-23s %s" % (
            case["no"], case["id"], case["kind"], case["row"] or "-",
            case["verdict"] + (" [cleanup]" if case.get("cleanup") else ""), expected,
            ",".join(case.get("other_failed", [])) or "-"))
    lines += ["", "5. THE CONTRACT'S %d CONTROL ROWS: is every control of the row KILLED as specified?" % len(ROWS),
              "   (yes: every control of the row reads KILLED; yes*: the same, and one of them needed cleanup;",
              "   n/c: reads KILLED but is not credited, see the line below the rows; NO: it does not)"]
    unsatisfied, qualified = [], []
    for row in sorted(ROWS):
        of_row = [case for case in mutants if case["row"] == row and case["kind"] == "control"]
        good = bool(of_row) and all(case["verdict"] == "KILLED" for case in of_row)
        if not good:
            unsatisfied.append(row)
        elif any(case.get("cleanup") for case in of_row):
            qualified.append(row)
        mark = "NO" if not good else "n/c" if not credit else "yes*" if row in qualified else "yes"
        lines.append("   row %2d  %-4s %s" % (row, mark, "  ".join(
            "%s=%s%s" % (case["id"], case["verdict"], " [cleanup]" if case.get("cleanup") else "")
            for case in of_row)))
    if not final:
        lines.append("   REQUIRED CONTROLS: NOT DECIDED. The campaign is in progress and the evidence is not yet checked.")
    elif not credit:
        lines.append("   REQUIRED CONTROLS: NO CREDIT. THE EVIDENCE IS %s (section 2)." % evidence["state"])
        lines.append("   That is not a finding that the controls are satisfied, and not one that they are not.")
    elif unsatisfied:
        lines.append("   REQUIRED CONTROLS: NOT ALL. Rows not satisfied in this campaign: %s" % ", ".join(
            str(row) for row in unsatisfied))
    elif state == "NOT COMPLETE":
        lines.append("   REQUIRED CONTROLS: every row's controls read KILLED, BUT THE CAMPAIGN IS NOT COMPLETE (section 1).")
        lines.append("   That is not a finding that the controls are satisfied.")
    else:
        lines.append("   REQUIRED CONTROLS: ALL %d ROWS KILLED AS SPECIFIED%s" % (
            len(ROWS), "" if state == "COMPLETE" else "  (other cases gave no result: section 1)"))
    if credit and qualified:
        lines.append("   * QUALIFIED: rows %s hold a control that needed cleanup (section 9). That is an "
                     "observation of the tests' output, not a clean execution." % ", ".join(
                         str(row) for row in qualified))
    lines.append("   This line is about the tests' power to detect the listed mutations. It is not acceptance.")
    lines += ["", "6. OTHER CASES WITH A FAILURE NAMED IN ADVANCE (not contract rows)"]
    for case in mutants:
        if case["kind"] != "control" and (case.get("expected_at") or case.get("expect") or case.get("build_error")):
            lines.append("   %-6s %-16s %s%s" % (case["id"], case["kind"], case["verdict"],
                                                 " [cleanup]" if case.get("cleanup") else ""))
    lines += ["", "7. CASES WITH NO RESULT CLAIMED IN ADVANCE",
              "   A survivor here survived the fifteen rs01_ tests that were run, in one test target.",
              "   The sixteenth, T10, is ignored and was not run. That is not evidence that it",
              "   survives T10 or any other test in the repository. A failure here awaits",
              "   attribution by the reviewer: the script credits nothing."]
    for case in mutants:
        if case["kind"] != "control" and not (case.get("expected_at") or case.get("expect")) \
                and not case.get("build_error"):
            lines.append("   %-6s %-16s %-52s failed: %s" % (
                case["id"], case["kind"], case["verdict"] + (" [cleanup]" if case.get("cleanup") else ""),
                ",".join(case.get("failed", [])) or "-"))
    counts = {}
    for case in mutants:
        counts[case["verdict"]] = counts.get(case["verdict"], 0) + 1
    lines += ["", "8. COUNTS (mutants)"] + ["   %-45s %d" % (name, counts[name]) for name in sorted(counts)]
    lines += ["", "9. CLEANUP: steps after which the runner had to end a process",
              "   A step whose command was ended by the runner, or whose output did not end, gives no",
              "   result (section 1). A step marked \"read\" ended by itself with its output complete, and",
              "   its output was read; a process it had started was still alive and was ended. That is a",
              "   qualified observation. It is not a clean execution, and it is no evidence about the",
              "   lifecycle of the processes themselves. The count of processes is what was listed before",
              "   any signal was sent; it is not an exact count of what was ended."]
    needed = 0
    for case in cases:
        for which, step in sorted((case.get("steps") or {}).items()):
            if step and step.get("cleanup_required"):
                needed += 1
                lines.append("   %-6s %-11s %-4s %s; alive before: %d; signals sent: %d" % (
                    case["id"], which, "read" if any(item["step"] == which for item in case.get("cleanup", []))
                    else "no", step["lifecycle"], step["alive_before_cleanup_count"],
                    step["cleanup_signals_count"]))
    for case in cases:
        if case.get("cleanup") and not case.get("steps"):
            needed += 1
            lines.append("   %-6s %s" % (case["id"], "cleanup needed"))
    if not needed:
        lines.append("   none")
    lines += [
        "",
        "10. WHAT THIS IS, AND IS NOT",
        "   Each verdict is a fixed rule applied to one test log. KILLED means: the named test failed;",
        "   the panic that ended it, on its own thread, is at the named assertion (file, line and column",
        "   as compiled), not inside it; the required words are in that panic's own message; and, where a",
        "   part is named, the backtrace shows the failure came through it. Nothing else is a kill: not a",
        "   build error, not a timeout, not another test's failure, not the right test failing elsewhere.",
        "   The four lines CAMPAIGN, EVIDENCE, REQUIRED CONTROLS and acceptance are separate. This script",
        "   decides the first three and never the fourth. It awards no pass to the repair, and no",
        "   independent-reproduction label: it was run by an operator, and its script and receipts are",
        "   for review. The timing test T10 was not run. The corrected T10 has still never been run.",
    ]
    return "\n".join(lines) + "\n"


# ===========================================================================
# Modes
# ===========================================================================
def command_plan(_args):
    check_plan()
    say("%s   candidate %s" % (REFERENCE, CANDIDATE))
    say("build: %s" % " ".join(BUILD))
    say("test : %s" % " ".join(TEST))
    for case in plan_of(None):
        if case["type"] == "baseline":
            say("%3d %-6s baseline         %s" % (case["no"], case["id"], case["what"]))
        else:
            expect = ",".join(oracle["test"] for oracle in case["expect"]) or (
                "(build error expected)" if case["build_error"] else "(no claim)")
            say("%3d %-6s %-16s row %-2s %-9s %-8s %-12s %s" % (
                case["no"], case["id"], case["kind"], case["row"] or "-", case["origin"],
                os.path.basename(case["file"]), expect, case["what"]))
    say("%d mutants; %d baselines." % (len(MUTANTS), BASELINE_REPEATS + 1))


def command_selftest(_args):
    checks = selftest()
    for name, passed, seen in checks:
        say("%s  %s  [%s]" % ("PASS" if passed else "FAIL", name, seen))
    failed = sum(1 for _name, passed, _seen in checks if not passed)
    say("%d checks, %d failed. These check the runner's rules on made-up output and made-up folders. "
        "Nothing was built." % (len(checks), failed))
    return EXIT_RUNNER if failed else 0


def command_verify(args):
    src = canonical(args.src)
    manifest = identity(src)
    pristine = {}
    for rel in CLEAN:
        with open(os.path.join(src, rel), "rb") as handle:
            pristine[rel] = handle.read()
    ready = prepare(pristine)
    say("tree %s: computed from %d files, equal to the candidate's" % (TREE, len(manifest)))
    for mutant in MUTANTS:
        entry = ready[mutant["id"]]
        places = " ".join("%s@%s:%d" % (o["test"], os.path.basename(o["file"]), o["lo"]) for o in entry["oracles"])
        say("%-6s %-16s applies; %2d changed lines; %s  %s" % (
            mutant["id"], mutant["kind"], entry["changed_lines"], entry["sha256"][:16], places or "-"))
    say("%d mutants. Text check only: nothing was built or run." % len(MUTANTS))
    return 0


def command_run(args):
    campaign = Campaign(args)
    for number in (signal.SIGINT, signal.SIGTERM, signal.SIGHUP, signal.SIGQUIT, signal.SIGALRM):
        signal.signal(number, on_signal)
    # The last resort of the time budget (B4). Every step is already bound by the
    # campaign's deadline; this alarm ends a campaign that passed it all the same.
    signal.setitimer(signal.ITIMER_REAL, OVERALL_BUDGET_S)
    try:
        campaign.start()
    except Interrupted as signalled:
        raise Refused("INTERRUPTED BY %s BEFORE THE CAMPAIGN STARTED" % signalled)
    except Quarantine as left:
        raise Refused("A PROCESS COULD NOT BE ENDED BEFORE THE CAMPAIGN STARTED: %s" % left)
    status = campaign.run()
    try:
        say(read_log(os.path.join(campaign.receipt, "summary.txt")).rstrip("\n"))
    except Exception as problem:
        say("summary.txt could not be read back: %r" % (problem,))
    say("exit status %d (%s)" % (status, EXIT_WORDS))
    return status


def main(argv=None):
    parser = argparse.ArgumentParser(description="RS-01 mutation audit, R4 (one fixed campaign).")
    sub = parser.add_subparsers(dest="mode", required=True)
    sub.add_parser("plan", help="print the fixed plan")
    sub.add_parser("selftest", help="the runner's own checks; nothing is built")
    verify = sub.add_parser("verify", help="text check only; nothing is built")
    verify.add_argument("--src", required=True)
    run = sub.add_parser("run", help="the whole campaign; there is no way to run a part of it")
    run.add_argument("--src", required=True, help="an exported copy of the candidate, outside any repository")
    run.add_argument("--out", required=True, help="a new or empty folder: the receipts")
    run.add_argument("--work", required=True, help="a new or empty folder on another, bounded filesystem: "
                                                   "build output and scratch")
    run.add_argument("--cargo-home", required=True, help="the dependency cache: registry/cache and "
                                                         "registry/index, as fetched")
    run.add_argument("--toolchain-bin", required=True, help="the folder holding the real cargo and rustc")
    run.add_argument("--launcher", required=True, help="the launcher's own file; its bytes go into the receipts")
    args = parser.parse_args(argv)
    try:
        return {"plan": command_plan, "selftest": command_selftest, "verify": command_verify,
                "run": command_run}[args.mode](args) or 0
    except (Refused, ValueError) as problem:
        say("REFUSED: %s" % problem)
        return EXIT_REFUSED
    except Exception as problem:  # never a status that says anything good
        say("THE RUNNER ITSELF FAILED: %r" % (problem,))
        try:
            import traceback
            say(traceback.format_exc())
        except Exception:
            pass
        return EXIT_RUNNER


if __name__ == "__main__":
    sys.exit(main())
