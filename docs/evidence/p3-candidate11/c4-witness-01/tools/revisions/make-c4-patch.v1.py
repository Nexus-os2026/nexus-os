#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-C4-WITNESS-01: derive the one C4 patch from the authenticated baseline protocol.rs (read from git,
not from any worktree), check its cardinality, and write it with its identities. It changes no worktree.

C4 (contract 7.5 row C4, "acknowledgement test 1 removed"; Architect's experimental definition): within
Publish::acknowledgement, preserve Test 0 and `let core = &mut *self.core;`, remove ONLY the five checks of Test 1
(session open; latch; Healthy phase; C/X occupancy; U occupancy invariant/latch), and preserve everything beginning
with `let set = snapshot.set;`. The comment line `// Test 1.` is kept: it is not a check.
"""
import difflib, hashlib, subprocess, sys
REPO = "/home/nexus/NEXUS/nexus-os"
SRC = "42dd1bd1cf21ffdea5968b256d2983f6575fab5b"
PATH = "crates/nexus-governed-control/src/ledger/protocol.rs"
BASE_BLOB = "a2df2841b99cad5d452008a6d4150e9369d07e66"
EV = "/home/nexus/nexus-audits/claude-architect/p3-c11-dr01-d2a1-c4-witness-01"
KEEP_HEAD = ("        // Test 1.\n"
             "        let core = &mut *self.core;\n")
CHECKS = ("        if core.session != Session::Open {\n"
          "            return not_now(Entry::NotOpen);\n"
          "        }\n"
          "        if core.latch.is_some() {\n"
          "            return not_now(Entry::Latched);\n"
          "        }\n"
          "        if core.phase != Phase::Healthy {\n"
          "            return not_now(Entry::NotHealthy);\n"
          "        }\n"
          "        if core.c.is_some() || core.x.is_some() {\n"
          "            return not_now(Entry::CellsBusy);\n"
          "        }\n"
          "        if core.u.is_some() {\n"
          "            core.latch(LatchCause::CellInvariant);\n"
          "            return not_now(Entry::Latched);\n"
          "        }\n")
KEEP_TAIL = "        let set = snapshot.set;\n"
ANCHOR = KEEP_HEAD + CHECKS + KEEP_TAIL
REPLACEMENT = KEEP_HEAD + KEEP_TAIL


def git(*args, data=None):
    return subprocess.run(["git", "-C", REPO, *args], input=data, capture_output=True, check=True).stdout


def main():
    assert git("rev-parse", f"{SRC}:{PATH}").decode().strip() == BASE_BLOB
    base = git("cat-file", "blob", BASE_BLOB)
    text = base.decode()
    fn_at = text.index("    fn acknowledgement(&mut self, snapshot: &Snapshot, io: &mut impl AckTxn) -> AckOutcome {\n")
    fn_end = text.index("\n    fn worker_belongs(", fn_at)
    out = []
    note = out.append
    note(f"baseline {SRC}:{PATH} blob {BASE_BLOB} sha256 {hashlib.sha256(base).hexdigest()} bytes {len(base)}")
    note(f"anchor occurrences in the file: {text.count(ANCHOR)}; inside Publish::acknowledgement: "
         f"{text[fn_at:fn_end].count(ANCHOR)}; replacement occurrences before: {text.count(REPLACEMENT)}")
    assert text.count(ANCHOR) == 1 and text[fn_at:fn_end].count(ANCHOR) == 1 and text.count(REPLACEMENT) == 0
    at = text.index(ANCHOR)
    line = text[:at].count("\n") + 1
    note(f"anchor starts at line {line} (`// Test 1.`); the removed checks are lines {line + 2}-{line + 17}")
    # Test 0 precedes the anchor, unchanged.
    test0 = text[fn_at:at]
    assert "// Test 0." in test0 and "if !Arc::ptr_eq(&snapshot.ledger, &self.core.shared) {" in test0
    mutant = text.replace(ANCHOR, REPLACEMENT).encode()
    a = base.decode().splitlines(keepends=True)
    b = mutant.decode().splitlines(keepends=True)
    mutant_blob = git("hash-object", "--stdin", data=mutant).decode().strip()
    diff = list(difflib.unified_diff(a, b, f"a/{PATH}", f"b/{PATH}", n=3))
    patch = (f"diff --git a/{PATH} b/{PATH}\nindex {BASE_BLOB[:12]}..{mutant_blob[:12]} 100644\n" + "".join(diff)).encode()
    removed = [l[1:] for l in patch.decode().splitlines() if l.startswith("-") and not l.startswith("---")]
    added = [l[1:] for l in patch.decode().splitlines() if l.startswith("+") and not l.startswith("+++")]
    hunks = [l for l in patch.decode().splitlines() if l.startswith("@@")]
    note(f"patch: files 1; hunks {len(hunks)} {hunks}; lines removed {len(removed)}; lines added {len(added)}")
    assert len(hunks) == 1 and added == [] and "".join(l + "\n" for l in removed) == CHECKS
    # Everything outside the removed checks is byte-identical.
    assert mutant == (text[:at] + KEEP_HEAD + KEEP_TAIL + text[at + len(ANCHOR):]).encode()
    assert len(base) - len(mutant) == len(CHECKS.encode())
    open(f"{EV}/patches/C4-test1-removed.patch", "wb").write(patch)
    open(f"{EV}/patches/C4-mutant-protocol.rs.txt", "wb").write(mutant)
    note(f"mutant protocol.rs blob {mutant_blob} sha256 {hashlib.sha256(mutant).hexdigest()} bytes {len(mutant)}")
    note(f"patch {EV}/patches/C4-test1-removed.patch sha256 {hashlib.sha256(patch).hexdigest()} bytes {len(patch)}")
    note("removed lines, exactly:")
    for l in removed:
        note(f"  - {l}")
    note("the mutated function head, as it will be built:")
    m = mutant.decode()
    for l in m[m.index("    fn acknowledgement(&mut self"):].splitlines()[:16]:
        note(f"  | {l}")
    print("\n".join(out))


if __name__ == "__main__":
    main()
