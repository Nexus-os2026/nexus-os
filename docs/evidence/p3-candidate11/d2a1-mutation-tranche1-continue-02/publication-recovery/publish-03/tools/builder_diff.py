#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-MUTATION-TRANCHE1-PUBLISH-03, mission section 7: the exact old-to-new packet-builder diff, and a
mechanical check that only the duplicate-content policy changed.

    builder_diff.py OUTDIR     OUTDIR must not exist; writes OUTDIR/BUILDER.diff (unified, frozen CONTINUE-02
                               tools/build_packet.py -> PUBLISH-03 tools/build_packet.py) and prints the checks

Every frozen line is kept, in order, except exactly the frozen duplicate-content policy: the `prev` table (built from
the previous packets' SHA256SUMS) and the "1 KiB or more" check with its informational line. Every added line is the
docstring revision paragraph, the two imports the policy uses (json, stat), the ruling constants, dup_policy(), or the
check that calls it. common.py is byte-identical to the frozen copy (its EV is the read-only CONTINUE-02 root).
"""
import difflib
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import pubcommon as Q  # noqa: E402

fails = []


def check(label, ok, detail=""):
    print(f"{'OK  ' if ok else 'FAIL'} {label}{(': ' + str(detail)) if detail != '' else ''}")
    if not ok:
        fails.append(label)


def main(out):
    if os.path.exists(out):
        raise SystemExit("REFUSED: output exists")
    os.makedirs(out)
    old_p, new_p = os.path.join(Q.SRC, "tools/build_packet.py"), os.path.join(Q.P, "tools/build_packet.py")
    old, new = open(old_p).read().splitlines(), open(new_p).read().splitlines()
    d = list(difflib.unified_diff(old, new, "continue-02/tools/build_packet.py (frozen)",
                                  "publish-03/tools/build_packet.py (repaired)", lineterm=""))
    open(os.path.join(out, "BUILDER.diff"), "w").write("\n".join(d) + "\n")
    print(f"frozen  {old_p} sha256 {Q.sha256_file(old_p)} ({len(old)} lines)")
    print(f"repaired {new_p} sha256 {Q.sha256_file(new_p)} ({len(new)} lines)")
    print(f"diff {os.path.join(out, 'BUILDER.diff')} sha256 {Q.sha256_file(os.path.join(out, 'BUILDER.diff'))}; "
          f"{sum(l.startswith('-') and not l.startswith('---') for l in d)} removed, "
          f"{sum(l.startswith('+') and not l.startswith('+++') for l in d)} added")
    check("the frozen builder is the CONTINUE-02 frozen file (05-FREEZE.json)",
          Q.sha256_file(old_p) == "a42cd96a9d996d2063adc916a606bf7e0cb9933ff18707afac7e2c2aacfe2e45")
    check("common.py beside the repaired builder is byte-identical to the frozen CONTINUE-02 common.py",
          Q.sha256_file(os.path.join(Q.P, "tools/common.py")) == Q.sha256_file(os.path.join(Q.SRC, "tools/common.py")))
    removed, added = [], []
    for tag, i1, i2, j1, j2 in difflib.SequenceMatcher(None, old, new, autojunk=False).get_opcodes():
        if tag in ("delete", "replace"):
            removed += list(range(i1 + 1, i2 + 1))
        if tag in ("insert", "replace"):
            added.append((j1 + 1, j2, new[j1:j2]))
    policy_old = [i + 1 for i, l in enumerate(old) if l.strip().startswith(("prev = {}", "prev[h] = "))]
    a = old.index("    prev = {}")
    b = old.index("    same = [f for f in allf if K.sha256_file(os.path.join(dest, f)) in prev and "
                  "os.path.getsize(os.path.join(dest, f)) > 0]")
    allowed_removed = list(range(a + 1, a + 6)) + list(range(b + 1, b + 8))
    print(f"     removed frozen lines: {removed}")
    print(f"     the frozen duplicate-content policy lines: prev table {a + 1}-{a + 5}, check {b + 1}-{b + 7} "
          f"({policy_old})")
    check("every removed frozen line is a line of the frozen duplicate-content policy (the prev table and the 1 KiB "
          "check with its info line), and all of them are removed", sorted(removed) == allowed_removed,
          f"{len(removed)} lines")
    seg_ok = []
    for j1, j2, lines in added:
        text = "\n".join(lines)
        kind = ("docstring revision" if lines[0].startswith("Revised by P3-C11-DR01-D2A1-MUTATION-TRANCHE1-PUBLISH-03")
                else "imports json/stat" if set(l for l in lines) <= {"import json", "import stat"}
                else "ruling constants (RULED, STAGE2_RECEIPT, MAP, MAP_COLUMNS)" if lines[1].startswith(
                    "# PUBLISH-03 (mission sections 1 and 6)") and "RULED = {" in text and "def " not in text
                else "dup_policy()" if lines[2].startswith("def dup_policy(") and text.count("\ndef ") == 1
                else "the check that calls dup_policy()" if lines[0].strip().startswith("accepted, problems = dup_policy(")
                else "OTHER")
        seg_ok.append(kind != "OTHER")
        print(f"     added new lines {j1}-{j2} ({len(lines)}): {kind}")
    check("every added segment is the docstring revision, the json/stat imports, the ruling constants, dup_policy(), "
          "or the check calling it", all(seg_ok) and len(added) == 6, f"{len(added)} segments")
    print("BUILDER DIFF: " + ("ONLY THE DUPLICATE-CONTENT POLICY CHANGED" if not fails else "FAILED: " + "; ".join(fails)))
    return 0 if not fails else 1


if __name__ == "__main__":
    sys.exit(main(os.path.abspath(sys.argv[1])))
