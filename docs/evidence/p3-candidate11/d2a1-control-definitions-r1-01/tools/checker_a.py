#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-CONTROL-DEFINITIONS-R1-01, mission sections 9 and 10: static checker A (read-only; no build,
no test, no rustc, no Cargo).

    checker_a.py ROOT [CATDIR]

Checker A works at the level of bytes and text. It reads Candidate C's sources with `git show C:path`, applies
every patch with GNU patch and with `git apply` to disposable byte copies (dry run first, then a real application in
a three-file disposable tree), reverses it, extracts named source items by exact header text and brace matching, and
tests each of the 21 Architect definitions' section-6 postconditions and preserved behaviours on the patched bytes.
It never imports checker B or the deriver. Every check prints one line tagged with its control id and check class;
the last line is the per-id verdict map as JSON. Exit 0 only if every check holds.
"""
import hashlib
import json
import os
import re
import subprocess
import sys
import tempfile

REPO = "/home/nexus/NEXUS/nexus-os"
C = "d5964ee5326c2733177307db6f507328023d361a"
STAGE1 = "f023cc3d031a2a89e5ecbd4086e15f2e50869acd"
PARENT = "59df21036f7c0183fa17d76b6e0886f8f5784685"
EXE_SHA = "cde9f9d20f32f2b8a17b7dcc2f521462ab6c7a6292d5e2cbd1dd2ba2c3d0d30c"
PROTOCOL = "crates/nexus-governed-control/src/ledger/protocol.rs"
MODEL = "crates/nexus-governed-control/src/ledger/protocol/model_tests.rs"
UNIT = "crates/nexus-governed-control/src/ledger/protocol/unit_tests.rs"
BLOB = {PROTOCOL: "a2df2841b99cad5d452008a6d4150e9369d07e66", MODEL: "28085ce232827301104b5d757af042ec933976f8",
        UNIT: "55dc66409eaccdaa93e8f0a56f2ccb14be81e114"}
INPUT_SHA = {"CONTRACT-02-R1.md": "cac6535115fb333e68e6c3c045e444342c950ab7a735b7b7deea21cb2cb53b3c",
             "AD1-R4.md": "00ced6d535c78f34f9e92233b742126a7b19192bf3ac8348056be2665192b693",
             "AD1-R5.md": "4e1ed8c92dbb2f3706eb524fa8b4fae766f027e7027f54049331ab312bb91473",
             "R5-OWNER-ADOPTION.md": "9990d2565a5ec96e0db03be832e96f64f774e662f6131cb69240c9a435757aa4",
             "blocked/catalog.json": "a0cdef7976bbb1715a7da25ad2900c978a32c6b3d9029b46f16872eda66b4747"}
CARRIED = ["C1", "C2", "C4", "C9", "C10", "C12", "C14", "C19", "C21", "C22", "C24", "C25", "C27", "C28"]
# Mission section 6, as stated: id -> (file, named properties, configuration).
SECTION6 = {"C3a": (PROTOCOL, ["P13", "P15"], "F-7"), "C3b": (PROTOCOL, ["P13", "P12"], "F-6"),
            "C3c": (PROTOCOL, ["P12"], "F-6"), "C5": (PROTOCOL, ["P13"], "F-5"), "C6": (PROTOCOL, ["P6"], "F-PT"),
            "C7": (PROTOCOL, ["P6"], "F-PT"), "C8": (PROTOCOL, ["E5"], "F-B"), "C11": (PROTOCOL, ["E4"], "F-B2"),
            "C15": (PROTOCOL, ["E8"], "F-R"), "C16": (PROTOCOL, ["E3"], "F-D"), "C17": (PROTOCOL, ["E4"], "F-B2"),
            "C18a": (PROTOCOL, ["E7"], "F-G"), "C18b": (PROTOCOL, ["E7"], "F-G"), "C20": (PROTOCOL, ["L1"], "F-2"),
            "C23": (PROTOCOL, ["E1"], "F-2"), "C26": (PROTOCOL, ["E9"], "F-4"), "A1": (MODEL, ["P1"], "F-1"),
            "A2": (MODEL, ["P1"], "F-1"), "A3": (MODEL, ["P12"], "F-5"), "A5": (MODEL, ["L1"], "F-2"),
            "A6": (MODEL, ["P12", "P15"], "F-5")}
HEAD = {  # exact item headers (start of line) used to extract a named item
    "acknowledgement": "    fn acknowledgement(&mut self, snapshot: &Snapshot, io: &mut impl AckTxn) -> AckOutcome {",
    "resolve_ack": "    fn resolve_ack(&mut self, set: &SetFacts, nonce: &[u8; 16], io: &mut impl Reads) -> Resolve {",
    "resolve": "    fn resolve(&mut self, worker: &Worker, token: &CellToken, io: &mut impl Reads) -> Resolve {",
    "classify": "    fn classify<'m>(\n",
    "known": "    fn known(&self, key: &RecordKey, explained: Option<Seq>) -> Option<Known> {",
    "lookup": "    fn lookup(&mut self, key: &RecordKey, io: &mut impl Reads) -> Lookup {",
    "explain": "    fn explain(&mut self, key: &RecordKey, io: &mut impl Reads) -> Explain {",
    "settle": "    fn settle(&mut self, cell: Cell, seq: Seq) {",
    "facts": "    fn facts(\n        &mut self,\n",
    "gating": "    fn gating(&mut self, record: &GatingRecord, io: &mut impl Txn) -> GatingOutcome {",
    "cell": "    fn cell(&mut self, worker: &Worker, token: &CellToken, io: &mut impl CellTxn) -> CellAppend {",
    "names": "    fn names(&self, token: &CellToken, cell: Cell) -> bool {",
    "flags": "    fn flags(&self) -> u64 {",
    "barrier": "    fn barrier(&mut self, worker: &Worker, io: &mut impl BarrierRun) -> BarrierOutcome {",
    "drop_ticket": "impl Drop for Ticket {",
    "drop_publish": "impl Drop for Publish<'_> {",
    "confirm": "    pub(super) fn confirm(mut self) {",
    "Table": "struct Table {",
    "initial": "fn initial(config: &Config) -> World {",
    "encode": "fn encode(config: &Config, w: &World) -> Vec<u8> {",
    "decode": "fn decode(config: &Config, key: &[u8]) -> World {",
    "step_fw": "fn step_fw(",
    "step_se": "fn step_se(",
    "step_wk": "fn step_wk(",
    "step_ack": "fn step_ack(",
    "step_adm": "fn step_adm(",
    "claimable": "fn claimable(",
    "admission_properties": "fn admission_properties(",
    "admit_by": "fn admit_by(",
    "World": "struct World {",
    "FwPos": "enum FwPos {",
    "WkPos": "enum WkPos {",
}
NAMED = {"C3a": ["acknowledgement"], "C3b": ["acknowledgement"], "C3c": ["acknowledgement"], "C5": ["resolve_ack"],
         "C6": ["classify"], "C7": ["explain"], "C8": ["facts"], "C11": ["facts"], "C15": ["names"],
         "C16": ["drop_ticket"], "C17": ["drop_publish"], "C18a": ["gating"], "C18b": ["classify"],
         "C20": ["cell"], "C23": ["flags"], "C26": ["barrier"], "A1": ["step_fw"], "A2": ["step_adm"],
         "A3": ["step_wk"], "A5": ["claimable"],
         "A6": ["Table", "initial", "encode", "decode", "step_wk", "step_ack", "step_adm"]}
QUOTIENT = re.compile(r"(gc_symmetry|quotient|symmetric|explore_experimental)")
LIST_ALL = "docs/evidence/p3-candidate11/final-baseline-known-runs-01/receipts/20-list-all/raw-output.txt"
LIST_IGN = "docs/evidence/p3-candidate11/final-baseline-known-runs-01/receipts/21-list-ignored/raw-output.txt"
C22_CMD = "cargo test -p nexus-governed-control --locked --release --lib --no-run -j 2"
results = []


def check(cid, cls, label, ok, detail=""):
    results.append((cid, cls, bool(ok)))
    print(f"{'OK  ' if ok else 'FAIL'} [{cls}] {cid}: {label}{(' — ' + str(detail)) if detail != '' else ''}")


def git(*args):
    return subprocess.run(["git", "-C", REPO, *args], capture_output=True, check=True).stdout


def blob_id(data):
    return subprocess.run(["git", "hash-object", "--stdin"], input=data, capture_output=True,
                          check=True).stdout.decode().strip()


def sha(b):
    return hashlib.sha256(b).hexdigest()


def tree(files, fn):
    with tempfile.TemporaryDirectory() as d:
        for path, data in files.items():
            p = os.path.join(d, path)
            os.makedirs(os.path.dirname(p), exist_ok=True)
            with open(p, "wb") as fh:
                fh.write(data)
        return fn(d)


def run_patch(files, patch, tool, reverse=False, dry=False):
    """Returns (rc, {path: bytes after}) for GNU patch (fuzz 0) or git apply, on a disposable tree."""
    def go(d):
        if tool == "gnu":
            cmd = ["patch", "-s", "-p1", "-F", "0", "--no-backup-if-mismatch", "-r", "-", "-N"]
            cmd += (["-R"] if reverse else []) + (["--dry-run"] if dry else [])
            env = None
        else:
            cmd = ["git", "apply", "--whitespace=nowarn"] + (["-R"] if reverse else []) + (["--check"] if dry else [])
            env = dict(os.environ, GIT_CONFIG_GLOBAL="/dev/null", GIT_CONFIG_NOSYSTEM="1",
                       GIT_DIR=os.path.join(d, ".nogit"))
        r = subprocess.run(cmd, cwd=d, input=patch, capture_output=True, env=env)
        after = {}
        for root_, _, names in os.walk(d):
            for n in names:
                full = os.path.join(root_, n)
                after[os.path.relpath(full, d)] = open(full, "rb").read()
        return r.returncode, after
    return tree(files, go)


def item(text, key):
    """The named item: from its exact header at a line start to its matching closing brace and newline."""
    head = HEAD[key]
    n = sum(1 for m in re.finditer(re.escape(head), text) if m.start() == 0 or text[m.start() - 1] == "\n")
    if n != 1:
        return None
    start = text.index(head) if text.startswith(head) else text.index("\n" + head) + 1
    i = text.index("{", start + len(head.rstrip("\n").rstrip("{")) - 1 if head.endswith("{") else start)
    depth, j, in_str = 0, i, False
    while j < len(text):
        ch = text[j]
        if in_str:
            if ch == "\\":
                j += 2
                continue
            if ch == '"':
                in_str = False
        elif text.startswith("//", j):
            j = text.index("\n", j)
            continue
        elif ch == '"':
            in_str = True
        elif ch == "'" and re.match(r"'(\\.|[^\\'])'", text[j:j + 4]):
            j += len(re.match(r"'(\\.|[^\\'])'", text[j:j + 4]).group(0))
            continue
        elif ch == "{":
            depth += 1
        elif ch == "}":
            depth -= 1
            if depth == 0:
                end = text.index("\n", j) + 1
                return start, end, text[start:end]
        j += 1
    return None


def has(t, *subs):
    return all(s in t for s in subs)


def order(t, *subs):
    idx = [t.find(s) for s in subs]
    return all(i >= 0 for i in idx) and idx == sorted(idx)


def contract_rows(text):
    sec = text[text.index("### 7.5 Controls"):text.index("### 7.6")]
    rows = {}
    for l in sec.splitlines():
        cells = [c.strip() for c in l.split("|")]
        if len(cells) > 2 and re.fullmatch(r"(C\d+[a-c]?|A\d)", cells[1]):
            rows.setdefault(cells[1], []).append(cells[2:-1])
    return rows, [l.split("|")[1].strip() for l in sec.splitlines()
                  if l.startswith("| ") and re.fullmatch(r"(C\d+[a-c]?|A\d)", l.split("|")[1].strip())]


def semantic(cid, B, M, Mfile, Bfile):
    """Section-6 postconditions and preserved behaviours of one Architect definition. B and M map item key ->
    healthy and patched item text; Bfile and Mfile are the whole files."""
    out = []
    t = lambda label, ok: out.append((label, ok))  # noqa: E731
    if cid in ("C3a", "C3b", "C3c"):
        m, b = M["acknowledgement"], B["acknowledgement"]
        t2 = "        let refusal = if set.frontier != now.frontier {\n            Some(AckRefusal::Stale)\n        } else"
        t3 = "if set.top <= set.frontier || set.count < 1 {\n            Some(AckRefusal::Empty)\n"
        t4 = "        } else if set.top > set.head || set.head > now.head {\n            Some(AckRefusal::Future)\n"
        t5 = ("        } else if now.count != set.count || now.digest != set.digest {\n"
              "            Some(AckRefusal::Set)\n")
        common = ["        let now = match io.recheck(set.frontier, set.top) {",
                  "        if let Some(refusal) = refusal {\n            self.rolled(io.rollback());\n"
                  "            return AckOutcome::Refused(refusal);", "        match io.insert(snapshot) {",
                  "        match io.commit() {", "        } else {\n            None\n        };"]
        t("recheck verb, refusal rollback, insertion, commit handling and the final None preserved", has(m, *common))
        if cid == "C3a":
            t("Test 2 removed whole: no frontier-equality comparison, no Stale", "set.frontier != now.frontier" not in m
              and "AckRefusal::Stale" not in m)
            t("the chain begins directly with Test 3", "        let refusal = " + t3 in m)
            t("Tests 4 and 5 preserved", has(m, t4, t5))
            t("item = healthy item with exactly the Test-2 clause removed",
              m == b.replace(t2 + " " + t3, "        let refusal = " + t3, 1))
        if cid == "C3b":
            t("Test 4 removed whole: neither comparison remains, no Future", "set.top > set.head" not in m and
              "set.head > now.head" not in m and "AckRefusal::Future" not in m)
            t("Test 3 connects directly to Test 5", "            Some(AckRefusal::Empty)\n" + t5 in m)
            t("Tests 2, 3 and 5 preserved", has(m, t2, t3, t5))
            t("item = healthy item with exactly the Test-4 clause removed", m == b.replace(t4, "", 1))
        if cid == "C3c":
            t("Test 5 removed whole: neither comparison remains, no Set", "now.count != set.count" not in m and
              "now.digest != set.digest" not in m and "AckRefusal::Set" not in m)
            t("after Test 4 the chain falls to None", "            Some(AckRefusal::Future)\n        } else {\n"
              "            None\n        };" in m)
            t("Tests 2, 3 and 4 preserved", has(m, t2, t3, t4))
            t("item = healthy item with exactly the Test-5 clause removed", m == b.replace(t5, "", 1))
    elif cid == "C5":
        m, b = M["resolve_ack"], B["resolve_ack"]
        t("signature unchanged byte for byte", m.split("\n")[0] == b.split("\n")[0])
        t("no acknowledgement read, no frontier read, no storage verb at all",
          "io.acknowledgement(" not in m and "io.frontier(" not in m and "io." not in m)
        t("no branch of the healthy match preserved", "match" not in m and "Read::" not in m)
        t("raise, empty U, Healthy, Acknowledged(set.top), in that order",
          order(m, "        if set.top > self.core.frontier {\n            self.core.frontier = set.top;\n        }",
                "        self.core.u = None;", "        self.core.phase = Phase::Healthy;",
                "        Resolve::Acknowledged(set.top)\n    }"))
        t("the caller resolve unchanged", M["resolve"] == B["resolve"])
    elif cid == "C6":
        m, b = M["classify"], B["classify"]
        t("row b no longer consults the refusal table or Core::known", "self.known(" not in m and
          "self.refusals.get" not in m)
        t("X's explanation of this original, then facts.explained, in that precedence",
          order(m, "Some((_, Occupant::Explanation { original, cause }, stage))", "if *original == member.key",
                "Stage::Built => Known::Cause(*cause)", "Stage::Committed(seq) => Known::Explained(*seq)",
                "_ => facts.explained.map(Known::Explained),"))
        t("row b stays between row a and row c", order(m, "return Verdict::Present(row.seq);",
                                                       "_ => facts.explained.map(Known::Explained),",
                                                       "return Verdict::Refused(Reason::Conflict { member: at, "
                                                       "known }, Some(at));",
                                                       "if self.session == Session::Sealed {"))
        t("rows d and e unchanged (one table insertion, one table-full check)",
          m.count("self.refusals.insert(member.key, cause);") == 1 and m.count("self.table_full()") == 1)
        t("Core::known unchanged", M["known"] == B["known"])
        t("Publish::lookup unchanged", M["lookup"] == B["lookup"])
    elif cid == "C7":
        m = M["explain"]
        t("Explained arm clears the whole table and returns Explained(seq)",
          "            Read::Got(Some(seq)) => {\n                self.core.refusals.clear();\n"
          "                return Explain::Explained(seq);\n            }" in m)
        t("the one-key removal is gone from explain", "refusals.remove(" not in m and m.count("refusals.clear()") == 1)
        t("Publish::settle unchanged (X's committed path keeps its one-key removal)",
          M["settle"] == B["settle"] and "self.core.refusals.remove(&original);" in M["settle"])
    elif cid == "C8":
        m, b = M["facts"], B["facts"]
        t("pending array and the separate insertion loop removed",
          "pending" not in m and "BATCH_MAX" not in m and m.count("for (") == 1 and m.count("io.insert(") == 1)
        t("Steps 0 to 2 unchanged", m[:m.index("        // Step 2.")] == b[:b.index("        // Step 2.")] and
          "        // Step 2.\n        if io.open() != Read::Got(()) {\n            self.failed();\n"
          "            return self.not_applied(Persistence::NotWritten, Reason::Storage);\n        }" in m)
        t("one member-order loop: facts, classify, then insert at once on Insert",
          order(m, "        let mut any_inserted = false;", "        for (index, member) in members().enumerate() {",
                "io.facts(at, member)", ".classify(at, member, &facts, members().take(index), false)",
                "                Verdict::Insert => {\n                    self.issued = true;\n"
                "                    any_inserted = true;\n                    let reason = match io.insert(at, member) {"))
        t("insert outcomes as healthy Step 5: Constraint latches; one rollback; rollback-derived persistence",
          "                        Insert::Inserted(_) => continue,\n                        Insert::Constraint => {\n"
          "                            self.core.latch(LatchCause::Constraint);\n"
          "                            Reason::Constraint\n                        }\n"
          "                        Insert::Failed => Reason::Storage,\n                    };\n"
          "                    self.failed();\n                    let persistence = self.rolled(io.rollback());\n"
          "                    return self.not_applied(persistence, reason);" in m)
        t("a refusal: conflict recorded, one rollback, Not written with the classification reason",
          "                Verdict::Refused(reason, conflict) => {\n                    self.conflict = conflict;\n"
          "                    self.rolled(io.rollback());\n"
          "                    return self.not_applied(Persistence::NotWritten, reason);" in m)
        t("no insert issued: the empty-transaction rollback and Committed, before Step 6",
          order(m, "        if !any_inserted {\n            self.rolled(io.rollback());\n"
                "            return FactOutcome::Committed;\n        }", "        // Step 6.\n        match io.commit() {"))
        t("Step 6 unchanged", m[m.index("        // Step 6."):] == b[b.index("        // Step 6."):])
        t("rollback count unchanged (5)", m.count("io.rollback()") == 5 == b.count("io.rollback()"))
    elif cid == "C11":
        m = M["facts"]
        t("fact Step-5 Constraint arm: latch, failed, one rollback (answer ignored), Not written + Constraint",
          "            let reason = match io.insert(member_index(index), member) {\n"
          "                Insert::Inserted(_) => continue,\n                Insert::Constraint => {\n"
          "                    self.core.latch(LatchCause::Constraint);\n                    self.failed();\n"
          "                    self.rolled(io.rollback());\n"
          "                    return self.not_applied(Persistence::NotWritten, Reason::Constraint);\n"
          "                }\n                Insert::Failed => Reason::Storage,\n            };" in m)
        t("Insert::Failed keeps the healthy rollback-derived persistence",
          "            self.failed();\n            let persistence = self.rolled(io.rollback());\n"
          "            return self.not_applied(persistence, reason);" in m)
        t("gating, acknowledgement and cell insert constraints unchanged",
          all(M[k] == B[k] for k in ("gating", "acknowledgement", "cell")))
    elif cid == "C15":
        m = M["names"]
        t("same ledger and same cell kept; occupancy equality removed; no is_some()",
          "Arc::ptr_eq(&token.ledger, &self.shared)" in m and "token.cell == cell" in m and "occupancy" not in m
          and "is_some" not in m)
        t("exactly two conjuncts", m.count("&&") == 1)
    elif cid == "C16":
        m, b = M["drop_ticket"], B["drop_ticket"]
        arm = m[m.index("TicketState::Counted(_) => {"):m.index("TicketState::Settled")]
        t("Counted arm: count_step(word | HANDOVER_FAULT, UNAPPLIED_SHIFT, false), one step",
          "count_step(word | HANDOVER_FAULT, UNAPPLIED_SHIFT, false)" in arm and arm.count("count_step(") == 1)
        t("live not lowered; handover fault kept; no confirm path", "LIVE_SHIFT" not in arm and
          "HANDOVER_FAULT" in arm and ".confirm(" not in m)
        t("Armed and Settled arms unchanged", "TicketState::Armed => {\n                exchange(&self.shared, 0, "
          "|word| count_step(word, LIVE_SHIFT, false));\n            }" in m and
          "            TicketState::Settled => {}" in m)
        t("Ticket::confirm unchanged", M["confirm"] == B["confirm"])
    elif cid == "C17":
        m, b = M["drop_publish"], B["drop_publish"]
        t("unwind persistence is always Not written", "let persistence = Persistence::NotWritten;" in m and
          "Persistence::Uncertain" not in m)
        t("phase, conflict, issued, keep, U retention, exit marking, publication and ticket state preserved",
          has(m, "self.core.phase = Phase::NeedBarrier;", "conflict_member: self.conflict,",
              "            self.keep = self.issued;\n", "if !self.keep {\n                self.core.u = None;",
              "Exit::ReturnedUncertain", "Exit::Unwound", "exchange(&self.core.shared, self.guess, |word| {",
              "ticket.state = TicketState::Counted(counted);") and m.count("self.keep") == b.count("self.keep"))
    elif cid == "C18a":
        m, b = M["gating"], B["gating"]
        t("classification refusal sets keep before its rollback and NotWritten return",
          "            Verdict::Refused(reason, _) => {\n                self.keep = true;\n"
          "                self.rolled(io.rollback());\n"
          "                return refused(Persistence::NotWritten, reason);" in m)
        t("keep added there only (count +1)", m.count("self.keep = true;") == b.count("self.keep = true;") + 1)
        t("open, facts-read, insert and commit failure paths unchanged",
          has(m, "        if io.open() != Read::Got(()) {\n            self.failed();\n"
                 "            return refused(Persistence::NotWritten, Reason::Storage);\n        }",
              "            failed => {\n                self.read_failed(failed == Read::Corrupt);\n"
              "                self.rolled(io.rollback());\n"
              "                return refused(Persistence::NotWritten, Reason::Storage);",
              "        self.keep = persistence == Persistence::Uncertain;",
              "                Commit::Unknown => {\n                    self.failed();\n"
              "                    self.rolled(io.rollback());\n                    self.keep = true;"))
        t("entry refusals before U is placed unchanged", m[:m.index("        self.hold = Hold::Gating;")] ==
          b[:b.index("        self.hold = Hold::Gating;")])
        t("the guard's exit marking unchanged", M["drop_publish"] == B["drop_publish"])
    elif cid == "C18b":
        m = M["classify"]
        t("gating new cause: X explanation kept, table entry added, same conflict returned",
          "        if gating {\n            if self.x.is_none() {\n                self.explanation(member.key, cause);\n"
          "            }\n            self.refusals.insert(member.key, cause);\n"
          "            return Verdict::Refused(conflict, Some(at));\n        }" in m)
        t("no fall-through and no table-full check in the gating branch",
          m.count("self.table_full()") == 1 and order(m, "            return Verdict::Refused(conflict, Some(at));",
                                                       "        if self.table_full() {"))
        t("already-known handling and the gating correction test unchanged",
          has(m, "        if let Some(known) = self.known(&member.key, facts.explained) {",
              "        if gating {\n            if let Some(seq) = facts.corrected {"))
    elif cid == "C20":
        m = M["cell"]
        t("present occupant: rollback; settle only for C; Committed(seq) for both",
          "            Read::Got(Some(seq)) => {\n                self.rolled(io.rollback());\n"
          "                if cell == Cell::C {\n                    self.settle(cell, seq);\n                }\n"
          "                return CellAppend::Committed(seq);\n            }" in m)
        t("inserted-and-Durable path unchanged", "            Commit::Durable => {\n"
          "                self.settle(cell, seq);\n                CellAppend::Committed(seq)\n            }" in m and
          m.count("self.settle(cell, seq);") == 2)
        t("Publish::settle unchanged", M["settle"] == B["settle"])
    elif cid == "C23":
        m = M["flags"]
        t("X bit removed; phase, session, latch, U and C bits kept", "X_OCCUPIED" not in m and
          has(m, "bit(self.latch.is_some(), LATCHED)", "bit(self.u.is_some(), U_OCCUPIED)",
              "bit(self.c.is_some(), C_OCCUPIED)", "        phase\n            | session\n"))
        t("word layout constants unchanged", "const X_OCCUPIED: u64 = 0x80;" in Mfile and
          Mfile.split("fn field(")[0] == Bfile.split("fn field(")[0])
    elif cid == "C26":
        m = M["barrier"]
        t("latch cleared after io.run() returns true and before the phase assignment",
          "        if !io.run() {\n            return BarrierOutcome::Failed;\n        }\n"
          "        self.core.latch = None;\n        self.core.phase = if self.core.u.is_some() {" in m)
        t("not cleared on OtherLedger, NotNeeded or a failed barrier", m.count("self.core.latch = None;") == 1 and
          order(m, "return BarrierOutcome::OtherLedger;", "return BarrierOutcome::NotNeeded;",
                "return BarrierOutcome::Failed;", "self.core.latch = None;", "BarrierOutcome::Passed"))
    elif cid == "A1":
        m = M["step_fw"]
        g = m[m.index("        FwPos::Got(held) if w.fw[i].ticket.is_none() => {"):m.index("        FwPos::Got(held) => {")]
        u = m[m.index("        FwPos::Got(held) => {"):m.index("        FwPos::Recorded => {")]
        o0 = u[u.index("                0 => {"):u.index("                1 => {")]
        t("post-confirm step (Got, no ticket) before the Got arm", bool(g) and order(
            m, "FwPos::Got(held) if w.fw[i].ticket.is_none() => {", "        FwPos::Got(held) => {"))
        t("post-confirm step records as healthy option 0, moves to Done, no Oracle choice, no ticket operation",
          has(g, "set_class(&mut w.table, f, Class::Paid);", "Held::Counted(counted) => record_counted(w, &members, "
              "counted),", "set_class(&mut w.table, f, Class::Uncertain);", "w.ghost.unrecorded &= !bit(f);",
              "w.fw[i].pos = FwPos::Done;") and "o." not in g.replace("obs.", "") and "confirm" not in g and
          ".take()" not in g)
        t("option 0 takes and confirms the ticket, records nothing, leaves the position",
          has(o0, "obs.ticket_steps += 1;", ".take()", ".confirm();") and "set_class" not in o0 and
          "record_counted" not in o0 and "pos =" not in o0)
        t("Oracle option structure, option 1 and DROP unchanged",
          has(u, "                &[false, true, false]\n", "                &[false, true]\n",
              "                1 => {\n                    w.fw[i].pos = FwPos::Dropping;",
              "// DROP: the Counted ticket is dropped unrecorded and"))
        t("FwPos::Recorded's healthy branch remains", "        FwPos::Recorded => {\n            if o.choose(&[false, "
          "true]) == 0 {" in m)
        t("settlement unchanged", M["step_se"] == B["step_se"])
        t("no new FwPos variant", M["FwPos"] == B["FwPos"])
    elif cid == "A2":
        m = M["step_adm"]
        one = m[m.index("    let Some(kept) = w.adm_kept else {"):m.index("    };\n") + 7]
        two = m[m.index("    };\n") + 7:]
        t("step one: disabled at adm == 0; one view; keeps view & FLAGS; no decrement, branch or property; enabled",
          order(one, "        if w.adm == 0 {\n            return false;\n        }", "        obs.views += 1;",
                "w.health.view()", "w.adm_kept = Some(first.word & FLAGS);", "        return true;") and
          one.count("w.health.view()") == 1 and "w.adm -=" not in one and "obs.branch" not in one and
          "admission_properties" not in one)
        t("step two: one view; only its unapplied field; low bits from step one; other fields zero",
          order(two, "    obs.views += 1;", "    let second = w.health.view();",
                "word: kept | with_field(0, UNAPPLIED_SHIFT, field(second.word, UNAPPLIED_SHIFT)),") and
          "second.word & FLAGS" not in two and two.count("w.health.view()") == 1)
        t("decision: split view admits, debt 0, H <= A; then adm_kept cleared and adm decremented once",
          order(two, "    let admitted = view.admits() && w.table.debt == 0 && w.table.h <= w.table.a;",
                "    w.adm_kept = None;", "    w.adm -= 1;", "    obs.branch(format!(",
                "        admission_properties(config, w, obs);") and m.count("w.adm -= 1;") == 1)
        t("no new world field", M["World"] == B["World"])
    elif cid == "A3":
        m = M["step_wk"]
        seen = m[m.index("        WkPos::Seen(frontier, action) => {"):m.index("        WkPos::Act(action) => {")]
        act = m[m.index("        WkPos::Act(action) => {"):m.index("        WkPos::RaiseH(seq) => {")]
        t("Seen keeps the A update, raises no H, proceeds to Act for EmptyX",
          "w.table.a = w.table.a.max(frontier);" in seen and "w.table.h" not in seen and
          "WkPos::Act(action)" in seen)
        t("Act: worker_op first, replay save kept, EmptyX -> RaiseH(seq), otherwise Idle",
          order(act, "worker_op(config, w, &action, o, obs);", "Action::EmptyX(_, seq) => WkPos::RaiseH(*seq),",
                "_ => WkPos::Idle,", "w.wk.last = Some(action);", "w.wk.pos = next;") and "w.table.h" not in act)
        t("RaiseH unchanged; H raised only there", "        WkPos::RaiseH(seq) => {\n"
          "            w.table.h = w.table.h.max(seq);\n            obs.note(|| \"[T] H raised\".into());\n"
          "            w.wk.pos = WkPos::Idle;" in m and m.count("w.table.h =") == 1)
        t("no new WkPos variant", M["WkPos"] == B["WkPos"])
    elif cid == "A5":
        m, b = M["claimable"], B["claimable"]
        t("Conflict arm: Healthy conjunct removed, X-empty kept, claim Explain",
          "        Class::Conflict if !view.x_occupied() => Some(Claim::Explain),\n" in m and "Phase::Healthy && "
          "!view.x_occupied()" not in m)
        t("handover/latch rejection, Refused and Uncertain arms unchanged",
          has(m, "    if view.handover_fault() || view.latched() {\n        return None;\n    }",
              "        Class::Refused if view.phase() == Phase::Healthy => Some(Claim::Append),",
              "        Class::Uncertain if view.phase() != Phase::NeedBarrier => Some(Claim::Lookup),"))
    elif cid == "A6":
        t("Table gains recovery_required: bool after A",
          "    a: Seq,\n    recovery_required: bool,\n}" in M["Table"])
        t("initial Table: false", "            a: 0,\n            recovery_required: false,\n        }," in M["initial"])
        t("encoded right after A", "    e.u(w.table.a);\n    e.b(w.table.recovery_required);\n" in M["encode"])
        t("decoded at the matching place", "        a: d.u(),\n        recovery_required: d.b(),\n    };" in M["decode"])
        wk = M["step_wk"]
        t("Seen: A increase clears, H increase sets (no A-H comparison)",
          "            if frontier > w.table.a {\n                w.table.a = frontier;\n"
          "                w.table.recovery_required = false;\n            }\n" in wk and
          "                if *seq > w.table.h {\n                    w.table.h = *seq;\n"
          "                    w.table.recovery_required = true;\n                }\n" in wk)
        t("RaiseH: H increase sets", "            if seq > w.table.h {\n                w.table.h = seq;\n"
          "                w.table.recovery_required = true;\n            }\n" in wk)
        t("Applying: A increase clears", "            if n > w.table.a {\n                w.table.a = n;\n"
          "                w.table.recovery_required = false;\n            }\n" in M["step_ack"])
        t("admission reads the flag instead of H <= A; other conditions unchanged",
          "    let admitted = view.admits() && w.table.debt == 0 && !w.table.recovery_required;\n" in M["step_adm"]
          and "w.table.h <= w.table.a" not in M["step_adm"])
        t("flag set only on an H increase and cleared only on an A increase; never from an A-H comparison",
          Mfile.count("recovery_required = true;") == 2 and Mfile.count("recovery_required = false;") == 2 and
          not re.search(r"recovery_required\s*=\s*[^;]*table\.(h|a)", Mfile) and
          Mfile.count("recovery_required") == 9)
        t("admission_properties (P12, P15) unchanged and flag-free",
          M["admission_properties"] == B["admission_properties"] and
          "recovery_required" not in M["admission_properties"])
        t("numeric H and A updates only on an actual increase (same values as max)",
          ".max(" not in wk[wk.index("WkPos::Seen(frontier, action) => {"):wk.index("WkPos::Act(action) => {")] and
          "w.table.h = w.table.h.max" not in wk and "w.table.a = w.table.a.max(n)" not in M["step_ack"])
        t("experimental admissions keep h <= a", M["admit_by"] == B["admit_by"] and
          Mfile.count("w.table.h <= w.table.a") == Bfile.count("w.table.h <= w.table.a") - 1)
    return out


def main(root, catdir):
    print("P3-C11-DR01-D2A1-CONTROL-DEFINITIONS-R1-01 — STATIC CHECKER A (bytes/text; no build, no test)")
    for name, digest in INPUT_SHA.items():
        check("inputs", "inputs", f"{name} authenticated",
              sha(open(os.path.join(root, "inputs", name), "rb").read()) == digest)
    src = {p: git("show", f"{C}:{p}") for p in BLOB}
    for p, b in src.items():
        check("inputs", "inputs", f"C:{p} is blob {BLOB[p][:8]}", blob_id(b) == BLOB[p])
    raw = open(os.path.join(catdir, "catalog.json"), "rb").read()
    cat = json.loads(raw)
    entries = cat["controls"]
    ids = [e["id"] for e in entries]
    contract = open(os.path.join(root, "inputs", "CONTRACT-02-R1.md")).read()
    rows, cids = contract_rows(contract)
    print("[catalog identity and counts]")
    check("catalog", "ids", "contract 7.5: 36 ids, each once", len(cids) == 36 and len(set(cids)) == 36)
    check("catalog", "ids", "catalog ids equal the contract's, in order", ids == cids, [i for i in cids if
                                                                                      ids.count(i) != 1])
    check("catalog", "ids", "every id exactly once", len(ids) == len(set(ids)))
    kill = [e for e in entries if e["id"] != "A4"]
    check("catalog", "ids", "exactly 35 kill-required controls", len(kill) == 35 and
          sorted(e["id"] for e in kill) == sorted(i for i in cids if i != "A4"), len(kill))
    a4 = [e for e in entries if e["id"] == "A4"]
    check("A4", "a4", "A4 once, separate, not a mutation, SATISFIED — ARCHITECT ACCEPTED, no definition or patch",
          len(a4) == 1 and a4[0].get("not_a_mutation") is True and a4[0].get("counted_as_kill_required") is False
          and a4[0].get("status") == "SATISFIED — ARCHITECT ACCEPTED" and "definition_status" not in a4[0] and
          not any(k in a4[0] for k in ("patch", "edits", "mutant_blob", "anchor")) and
          not os.path.exists(os.path.join(catdir, "patches", "A4.patch")))
    check("catalog", "counts", "version and counts", cat["version"] == "d2a1-controls-v1-r1-candidate" and
          cat["counts"] == {"kill_required": 35, "comparison": 1, "candidate_ready": 35, "blocked": 0,
                            "carried_forward": 14, "architect_defined": 21})
    check("catalog", "counts", "35 patch files and no other", sorted(os.listdir(os.path.join(catdir, "patches"))) ==
          sorted(f"{e['id']}.patch" for e in kill))
    text = raw.decode()
    check("catalog", "wording", "no FROZEN / READY FOR EXECUTION; 'ARCHITECT ACCEPTED' only as A4's status",
          "FROZEN" not in text.upper() and "READY FOR EXECUTION" not in text.upper() and
          text.upper().count("ARCHITECT ACCEPTED") == 1 and '"status": "SATISFIED — ARCHITECT ACCEPTED"' in text and
          "accepted" not in cat["catalog_status"].lower())
    print("[status, property and configuration]")
    for e in kill:
        cid = e["id"]
        check(cid, "status", "CANDIDATE_READY / review pending / NOT_ATTEMPTED",
              e.get("definition_status") == "CANDIDATE_READY" and e.get("architect_review_status") == "pending" and
              e.get("execution_status") == "NOT_ATTEMPTED")
        row = rows[cid][0]
        if e["kind"] == "explore":
            props = re.findall(r"\b(P\d+|E\d+|L1|D1)\b", row[1])
            check(cid, "property", f"named property {e['named_property']} and configuration {e['configuration']} "
                  "equal the contract row", e["named_property"] == props and e["configuration"] == row[2] and
                  e["named_property_text"] == row[1])
        if cid in SECTION6:
            f, props, conf = SECTION6[cid]
            check(cid, "property", "property, configuration and file equal mission section 6",
                  e["named_property"] == props and e["configuration"] == conf and e["mutation_source_file"] == f)
            check(cid, "status", "definition source ARCHITECT_DEFINED", e.get("definition_source") ==
                  "ARCHITECT_DEFINED")
        elif cid in CARRIED:
            check(cid, "status", "definition source CARRIED_FORWARD", e.get("definition_source") == "CARRIED_FORWARD")
        else:
            check(cid, "status", "id belongs to section 5 or section 6", False)
    print("[anchors, patches, application, restoration]")
    blocked = json.load(open(os.path.join(root, "inputs", "blocked", "catalog.json")))
    bby = {e["id"]: e for e in blocked["controls"]}
    seen_patch, seen_mut = {}, {}
    mutants = {}
    for e in kill:
        cid = e["id"]
        path = e["mutation_source_file"]
        if path not in BLOB:
            check(cid, "source", "source file is one of C's three", False, path)
            continue
        base = src[path]
        check(cid, "source", "baseline blob recorded is C's", e["baseline_blob"] == BLOB[path])
        bt = base.decode()
        spans, ok = [], True
        for k, ed in enumerate(e["edits"]):
            a = ed["anchor"]
            n = bt.count(a)
            i = bt.find(a)
            ok = ok and n == ed["expected_count"] == 1 and (i == 0 or bt[i - 1] == "\n") and \
                ed["anchor_first_line_at_C"] == bt.count("\n", 0, i) + 1
            spans.append((i, i + len(a)))
        s = sorted(spans)
        check(cid, "anchor", f"{len(e['edits'])} anchor(s): each exactly once in C's blob, at a line start, at the "
              "recorded line; spans disjoint", ok and all(x[1] <= y[0] for x, y in zip(s, s[1:])))
        expected = bt
        for ed in e["edits"]:
            expected = expected.replace(ed["anchor"], ed["replacement"], 1) if expected.count(ed["anchor"]) == 1 \
                else None
            if expected is None:
                break
        exp_b = expected.encode() if expected is not None else None
        pfile = open(os.path.join(catdir, e["patch_file"]), "rb").read()
        check(cid, "patch", "patch file = catalog patch text; sha256 as recorded",
              pfile == e["patch"].encode() and sha(pfile) == e["patch_sha256"])
        if cid in CARRIED:
            check(cid, "carried", "patch byte for byte the blocked catalog's (mission 5)",
                  pfile == bby[cid]["patch"].encode() and sha(pfile) == bby[cid]["patch_sha256"] and
                  e["edits"][0]["anchor"] == bby[cid]["anchor"] and e["edits"][0]["replacement"] ==
                  bby[cid]["replacement"] and e["mutant_blob"] == bby[cid]["mutant_blob"])
        headers = re.findall(rb"^diff --git a/(\S+) b/(\S+)$", pfile, re.M)
        minus = re.findall(rb"^--- (\S+)$", pfile, re.M)
        plus = re.findall(rb"^\+\+\+ (\S+)$", pfile, re.M)
        check(cid, "single-file", "patch names exactly one file, the named source file",
              headers == [(path.encode(), path.encode())] and minus == [b"a/" + path.encode()] and
              plus == [b"b/" + path.encode()] and e["expected_changed_blob_count"] == 1)
        three = dict(src)
        g_dry, _ = run_patch(three, pfile, "gnu", dry=True)
        a_dry, _ = run_patch(three, pfile, "git", dry=True)
        check(cid, "apply", "GNU patch --dry-run (fuzz 0) and git apply --check succeed on a disposable copy",
              g_dry == 0 and a_dry == 0)
        g_rc, g_after = run_patch(three, pfile, "gnu")
        a_rc, a_after = run_patch(three, pfile, "git")
        gm, am = g_after.get(path), a_after.get(path)
        check(cid, "apply", "both applications give the same bytes, equal to the edits' result",
              g_rc == 0 and a_rc == 0 and gm == am == exp_b and exp_b is not None)
        others = [p for p in BLOB if p != path]
        check(cid, "confine", "application changes only the named source file (the other two blobs unchanged; no "
              "file added)", g_rc == 0 and all(blob_id(g_after[p]) == BLOB[p] for p in others) and
              sorted(g_after) == sorted(BLOB) and all(blob_id(a_after[p]) == BLOB[p] for p in others))
        check(cid, "identity", "mutant git blob and sha256 as recorded",
              gm is not None and blob_id(gm) == e["mutant_blob"] and sha(gm) == e["mutant_sha256"])
        idx = re.search(rb"^index ([0-9a-f]+)\.\.([0-9a-f]+) ", pfile, re.M)
        check(cid, "identity", "patch index line names C's blob and the mutant blob (full, or abbreviated to at "
              "least 7 hex digits as in the documentary patches)",
              idx is not None and len(idx.group(1)) >= 7 and len(idx.group(2)) >= 7 and
              BLOB[path].startswith(idx.group(1).decode()) and e["mutant_blob"].startswith(idx.group(2).decode()))
        rest = dict(src)
        rest[path] = exp_b if exp_b is not None else b""
        gr_rc, gr_after = run_patch(rest, pfile, "gnu", reverse=True)
        ar_rc, ar_after = run_patch(rest, pfile, "git", reverse=True)
        check(cid, "restoration", "reverse application (patch -R, git apply -R) of the patch to the recorded mutant "
              "returns byte for byte to C", gr_rc == 0 and ar_rc == 0 and gr_after.get(path) == base ==
              ar_after.get(path) and e["restoration"]["restores_blob"] == BLOB[path] and
              e["restoration"]["restores_file"] == path)
        if e["patch_sha256"] in seen_patch or (path, e["mutant_blob"]) in seen_mut:
            check(cid, "distinct", "patch and mutant identity distinct from every other control", False,
                  seen_patch.get(e["patch_sha256"]) or seen_mut.get((path, e["mutant_blob"])))
        seen_patch[e["patch_sha256"]] = cid
        seen_mut[(path, e["mutant_blob"])] = cid
        mutants[cid] = (path, gm if gm is not None else b"")
    check("catalog", "distinct", "35 distinct patch sha256 and 35 distinct mutant blobs (no equivalence expected)",
          len(seen_patch) == 35 and len(seen_mut) == 35)
    print("[section-6 postconditions and preserved behaviour (the 21)]")
    for cid in SECTION6:
        if cid not in mutants:
            check(cid, "semantic", "mutant available", False)
            continue
        path, mb = mutants[cid]
        bt, mt = src[path].decode(), mb.decode()
        keys = [k for k in HEAD if (k in ("acknowledgement", "resolve_ack", "resolve", "classify", "known", "lookup",
                                          "explain", "settle", "facts", "gating", "cell", "names", "flags", "barrier",
                                          "drop_ticket", "drop_publish", "confirm")) == (path == PROTOCOL)]
        B = {k: (item(bt, k) or (0, 0, None))[2] for k in keys}
        M = {k: (item(mt, k) or (0, 0, None))[2] for k in keys}
        miss = [k for k in keys if B[k] is None or M[k] is None]
        check(cid, "semantic", "every reference item found once in C and in the mutant", not miss, miss)
        if miss:
            continue
        named = NAMED[cid]
        rb, rm = bt, mt
        for k in sorted(named, key=lambda k: -item(bt, k)[0]):
            s0, e0, _ = item(rb, k)
            rb = rb[:s0] + f"<<{k}>>\n" + rb[e0:]
            s1, e1, _ = item(rm, k)
            rm = rm[:s1] + f"<<{k}>>\n" + rm[e1:]
        check(cid, "confine-item", f"outside the named item(s) {', '.join(named)} the file is byte-identical",
              rb == rm)
        check(cid, "confine-item", "every named item actually changes", all(B[k] != M[k] for k in named))
        for label, ok in semantic(cid, B, M, mt, bt):
            check(cid, "semantic", label, ok)
    print("[named tests, receipt types, configurations]")
    model = src[MODEL].decode()
    unit = src[UNIT].decode()
    listing = git("show", f"{PARENT}:{LIST_ALL}").decode()
    ignored = git("show", f"{PARENT}:{LIST_IGN}").decode()
    consts = dict(re.findall(r'^const (\w+): Config = (?:default_run\()?Config \{\n    name: "([^"]+)"', model, re.M))
    r4 = open(os.path.join(root, "inputs", "AD1-R4.md")).read()
    for e in kill:
        cid, test = e["id"], e.get("test")
        name = test.rsplit("::", 1)[1] if test else None
        srcf = model if e["kind"] == "explore" else unit
        m = re.search(rf"^#\[test\]\n(#\[ignore[^\n]*\n)?fn {re.escape(name)}\(\) \{{\n(.*?)\n\}}", srcf,
                      re.M | re.S) if name else None
        ok = bool(m) and f"{test}: test" in listing
        if e["kind"] == "explore" and m:
            body = m.group(2)
            const = re.search(r"\.\.(\w+)\n|_exploration\(&(\w+)\)", body)
            cname = const and (const.group(1) or const.group(2))
            ok = ok and consts.get(cname) == e["configuration"] and (m.group(1) is not None) == e["ignored_at_C"] \
                and ((f"{test}: test" in ignored) == e["ignored_at_C"])
        check(cid, "test", f"named test {name} exists at C, in the pinned executable's listing"
              + (", runs the configuration's constant" if e["kind"] == "explore" else ""), ok)
        want = {"explore": "(a) original-explorer mutant FAIL; restored (e1)",
                "unit": "(b) named non-explorer mutant-test FAIL; restored (e2) PASS",
                "build": "(d) intentional mutant compile failure; restored (e4) healthy build"}[e["kind"]]
        check(cid, "receipt", f"receipt type {want.split(';')[0]} per AD-1-R4 F/F.1",
              e["receipt_type"] == want and want.split(" ")[0] in r4)
        check(cid, "quotient", "failing receipt is never a quotient run; original explorer",
              not QUOTIENT.search(test or "x") and (e["kind"] != "explore" or e.get("explorer") ==
                                                    "original (`explore`)"))
    by = {e["id"]: e for e in entries}
    print("[special rules: C7, C15, C22]")
    c7 = by.get("C7", {}).get("additional_comparison", {})
    check("C7", "receipt", "the separate original-explorer F-3 comparison without A4 recorded, not required to kill, "
          "same patch", c7.get("required_to_kill") is False and c7.get("configuration") == "F-3" and
          c7.get("test") == "ledger::protocol::model_tests::full_f3" and "same" in c7.get("patch", "") and
          "C7 without A4 (line 748)" in r4)
    c15 = by.get("C15", {})
    check("C15", "receipt", "failing receipt original full_fr at 30,000,000; restored side the AD-1-R5 F discharge "
          "only after verified restoration", c15.get("test") == "ledger::protocol::model_tests::full_fr" and
          c15.get("ceiling_states") == 30000000 and "after verified restoration" in
          c15.get("restoration", {}).get("restored_side", "") and
          c15.get("restored_receipt", {}).get("ad1r5_sha256") == INPUT_SHA["AD1-R5.md"])
    c22 = by.get("C22", {}).get("restored_receipt", {})
    s1 = git("show", f"{STAGE1}:docs/evidence/p3-candidate11/final-baseline-known-runs-01/receipts/10-build/"
             "RECEIPT.txt").decode()
    s1cmd = [l for l in s1.splitlines() if "cargo test -p nexus-governed-control" in l]
    check("C22", "c22", "Stage-1 healthy build's recorded command omits -j 2", bool(s1cmd) and
          all("-j" not in l for l in s1cmd), s1cmd[:1])
    check("C22", "c22", "restored_receipt_status PENDING_EXACT_COMMAND_HEALTHY_BUILD; required command exact; "
          "Stage-1 build recorded only as historical, not as the (e4) receipt",
          c22.get("restored_receipt_status") == "PENDING_EXACT_COMMAND_HEALTHY_BUILD" and
          c22.get("required_future_healthy_command") == C22_CMD == c22.get("required_future_mutant_command") and
          "directory" not in c22 and c22.get("not_run_in_this_mission") is True and
          "historical_stage1_build_not_the_e4_receipt" in c22 and
          sorted(c22.get("identical_between_healthy_and_mutant", [])) ==
          sorted(["command", "toolchain", "environment", "target policy", "features/cfg/RUSTFLAGS",
                  "supervision/resource limits"]))
    print("[restored-evidence links]")
    for e in kill:
        rr = e["restored_receipt"]
        if e["id"] in ("C15", "C22") or "directory" not in rr:
            check(e["id"], "restored", "restored side recorded (to be produced or by special rule)",
                  bool(e["restoration"].get("restored_side")))
            continue
        files = {os.path.basename(l.split("\t")[1]): l.split()[2] for l in
                 git("ls-tree", PARENT, rr["directory"] + "/").decode().splitlines()}
        cls = git("cat-file", "blob", files["CLASSIFICATION.txt"]).decode().splitlines()[0]
        desc = json.loads(git("cat-file", "blob", files["descriptor.json"]))
        rawo = git("cat-file", "blob", files["raw-output.txt"])
        test = desc.get("test", (desc.get("argv") or [None, None])[1])
        want = e["test"] if e["configuration"] not in ("F-PT", "F-B") else \
            "ledger::protocol::model_tests::" + {"F-PT": "gc_symmetry_fpt", "F-B": "gc_symmetry_fb"}[e["configuration"]]
        check(e["id"], "restored", f"e1 link {rr['directory'].rsplit('/', 1)[1]}: PASS, its test, raw-output digest",
              cls == "CLASSIFICATION: PASS" and test == want and sha(rawo) == desc.get("raw_output_sha256") ==
              rr["raw_output_sha256"])
    verdict = {}
    for cid, _, ok in results:
        verdict[cid] = verdict.get(cid, True) and ok
    fails = [r for r in results if not r[2]]
    print()
    print(f"checks {len(results)}; failing {len(fails)}")
    print("CHECKER A: " + ("PASS" if not fails else "FAIL"))
    print("VERDICTS " + json.dumps({k: ("PASS" if v else "FAIL") for k, v in sorted(verdict.items())},
                                   sort_keys=True))
    return 0 if not fails else 1


if __name__ == "__main__":
    r = sys.argv[1]
    try:
        sys.exit(main(r, sys.argv[2] if len(sys.argv) > 2 else os.path.join(r, "out", "catalog")))
    except Exception as ex:  # noqa: BLE001 — a crash is reported as a failing check, never as a pass
        check("catalog", "crash", f"checker A raised {type(ex).__name__}: {ex}", False)
        v = {}
        for row in results:
            v[row[0]] = v.get(row[0], True) and row[-1]
        print("CHECKER: FAIL (crash)")
        print("VERDICTS " + json.dumps({k: ("PASS" if x else "FAIL") for k, x in sorted(v.items())}, sort_keys=True))
        sys.exit(1)
