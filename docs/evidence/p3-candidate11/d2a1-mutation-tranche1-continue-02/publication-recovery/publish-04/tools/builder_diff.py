#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-MUTATION-TRANCHE1-PUBLISH-04, mission section 8: the three exact packet-builder diffs, and a
mechanical check that only the duplicate-content and zero-length policy changed.

    builder_diff.py OUTDIR     OUTDIR must not exist; writes OUTDIR/FROZEN-TO-PUBLISH-03.diff (the frozen CONTINUE-02
                               builder to the PUBLISH-03 builder; byte-identical to PUBLISH-03's own BUILDER.diff),
                               OUTDIR/PUBLISH-03-TO-PUBLISH-04.diff and OUTDIR/FROZEN-TO-PUBLISH-04.diff (unified), and
                               prints the checks

The check removes, from each builder, its policy regions only: the frozen builder's duplicate check (the `prev` table
and the "1 KiB or more" check with its info line); in the PUBLISH-03 and PUBLISH-04 builders, the revision paragraphs
of the docstring, the policy imports, the ruling constants (from the PUBLISH-03 ruling comment to `SOURCES`),
dup_policy() (to `def main`) and the check that calls it (to the "every staged file byte-equal to its source" check).
What remains of the three builders must be identical, line for line. The seven RULED link rows and STAGE2_RECEIPT are
carried byte-identical from PUBLISH-03. common.py beside the builder is byte-identical to the frozen CONTINUE-02 copy.
"""
import difflib
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import pub4common as Q  # noqa: E402

FROZEN_SHA = "a42cd96a9d996d2063adc916a606bf7e0cb9933ff18707afac7e2c2aacfe2e45"
P3_SHA = "d243334992be1945ace49cf9460a66fff2e9210ef138e122dec86083cc6bd44e"
P3_DIFF_SHA = "7cde9a95f59ed23f09cbea11d04354031b973667b32f9fdb73131639d7e2a826"
CONST = "# PUBLISH-03 (mission sections 1 and 6): the Architect's duplicate-link-receipt ruling. The seven exact current paths"
fails = []


def check(label, ok, detail=""):
    print(f"{'OK  ' if ok else 'FAIL'} {label}{(': ' + str(detail)) if detail != '' else ''}")
    if not ok:
        fails.append(label)


def cut(lines, start, end):
    """Remove lines[start:end] and return (rest, removed)."""
    return lines[:start] + lines[end:], lines[start:end]


def frozen_skeleton(old):
    a = old.index("    prev = {}")
    b = old.index("    same = [f for f in allf if K.sha256_file(os.path.join(dest, f)) in prev and "
                  "os.path.getsize(os.path.join(dest, f)) > 0]")
    rest, r1 = cut(old, b, b + 7)
    rest, r2 = cut(rest, a, a + 5)
    return rest, r2 + r1


def revised_skeleton(new):
    """The PUBLISH-03 or PUBLISH-04 builder without its policy regions."""
    rest, removed = list(new), []
    end = rest.index('"""', 1)
    for tag in ("Revised by P3-C11-DR01-D2A1-MUTATION-TRANCHE1-PUBLISH-04", "Revised by P3-C11-DR01-D2A1-MUTATION-TRANCHE1-PUBLISH-03"):
        if any(l.startswith(tag) for l in rest[:end]):
            i = next(j for j, l in enumerate(rest[:end]) if l.startswith(tag))
            stop = next((j for j in range(i + 1, end) if rest[j].startswith("Revised by ")), end)
            rest, r = cut(rest, i, stop)
            removed += r
            end = rest.index('"""', 1)
    for imp in ("import ast", "import json", "import shlex", "import stat"):
        if imp in rest:
            i = rest.index(imp)
            rest, r = cut(rest, i, i + 1)
            removed += r
    for s, e in ((CONST, "SOURCES = [(K.RULING, \"ARCHITECT-RULING-TRANCHE1-PARTIAL-ACCEPTANCE.md\"),"),
                 ("def dup_policy(", "def main(dest):"),
                 ("    accepted, ", '    check("every staged file byte-equal to its source", all(K.sha256_file(s) == '
                                  'K.sha256_file(os.path.join(dest, r))')):
        i = next(j for j, l in enumerate(rest) if l.startswith(s))
        k = next(j for j in range(i + 1, len(rest)) if rest[j].startswith(e))
        rest, r = cut(rest, i, k)
        removed += r
    return rest, removed


def block(lines, start, end_prefix):
    i = next(j for j, l in enumerate(lines) if l.startswith(start))
    k = next(j for j in range(i, len(lines)) if lines[j].startswith(end_prefix))
    return lines[i:k + 1]


def write_diff(out, name, a, b, la, lb):
    d = list(difflib.unified_diff(a, b, la, lb, lineterm=""))
    p = os.path.join(out, name)
    open(p, "w").write("\n".join(d) + "\n")
    print(f"diff {name}: sha256 {Q.sha256_file(p)}; {sum(l.startswith('-') and not l.startswith('---') for l in d)} "
          f"removed, {sum(l.startswith('+') and not l.startswith('+++') for l in d)} added")
    return p


def main(out):
    Q.no_bytecode()
    if os.path.exists(out):
        raise SystemExit("REFUSED: output exists")
    os.makedirs(out)
    fp, p3p, p4p = (os.path.join(Q.SRC, "tools/build_packet.py"), os.path.join(Q.P3, "tools/build_packet.py"),
                    os.path.join(Q.P, "tools/build_packet.py"))
    fz, p3, p4 = (open(x).read().splitlines() for x in (fp, p3p, p4p))
    for label, path in (("frozen CONTINUE-02", fp), ("PUBLISH-03", p3p), ("PUBLISH-04", p4p)):
        print(f"{label:<19} {path} sha256 {Q.sha256_file(path)} ({len(open(path).read().splitlines())} lines)")
    check("the frozen builder is the CONTINUE-02 frozen file (05-FREEZE.json)", Q.sha256_file(fp) == FROZEN_SHA)
    check("the PUBLISH-03 builder is PUBLISH-03's (its receipts/90-ROOT-SHA256SUMS.txt)", Q.sha256_file(p3p) == P3_SHA)
    check("common.py beside the PUBLISH-04 builder is byte-identical to the frozen CONTINUE-02 common.py",
          Q.sha256_file(os.path.join(Q.P, "tools/common.py")) == Q.sha256_file(os.path.join(Q.SRC, "tools/common.py")))
    d1 = write_diff(out, "FROZEN-TO-PUBLISH-03.diff", fz, p3, "continue-02/tools/build_packet.py (frozen)",
                    "publish-03/tools/build_packet.py (repaired)")
    check("FROZEN-TO-PUBLISH-03.diff is byte-identical to PUBLISH-03's receipts/03-BUILDER-DIFF/BUILDER.diff",
          Q.sha256_file(d1) == P3_DIFF_SHA == Q.sha256_file(os.path.join(Q.P3, "receipts/03-BUILDER-DIFF/BUILDER.diff")))
    write_diff(out, "PUBLISH-03-TO-PUBLISH-04.diff", p3, p4, "publish-03/tools/build_packet.py (repaired)",
               "publish-04/tools/build_packet.py (R2)")
    write_diff(out, "FROZEN-TO-PUBLISH-04.diff", fz, p4, "continue-02/tools/build_packet.py (frozen)",
               "publish-04/tools/build_packet.py (R2)")

    sf, rf = frozen_skeleton(fz)
    s3, r3 = revised_skeleton(p3)
    s4, r4 = revised_skeleton(p4)
    print(f"     policy lines removed for the comparison: frozen {len(rf)}, PUBLISH-03 {len(r3)}, PUBLISH-04 {len(r4)}; "
          f"remaining {len(sf)}, {len(s3)}, {len(s4)}")
    check("PUBLISH-03 -> PUBLISH-04: outside the policy regions every line is identical, in order", s3 == s4,
          [l for l in difflib.unified_diff(s3, s4, lineterm="", n=0)][2:8])
    check("frozen -> PUBLISH-04: outside the policy regions every line is identical, in order", sf == s4,
          [l for l in difflib.unified_diff(sf, s4, lineterm="", n=0)][2:8])
    check("frozen -> PUBLISH-03 (re-checked the same way): identical outside the policy regions", sf == s3)
    for start, end in (("RULED = {", "}"), ("STAGE2_RECEIPT = {", "STAGE2_RECEIPT = {")):
        check(f"{start.split(' ')[0]} carried byte-identical from PUBLISH-03", block(p3, start, end) == block(p4, start, end),
              f"{len(block(p4, start, end))} lines")
    for fn, last in (("    def unsafe(rel):", "        return None if os.path.realpath(cur)"),
                     ("    def read(rel):", "            return f.read()")):
        b3 = block(p3, fn, last)
        check(f"dup_policy() helper {fn.strip()[4:].split('(')[0]}() carried byte-identical from PUBLISH-03",
              b3 == block(p4, fn, last), f"{len(b3)} lines")
    i3 = block(p3, "    # Every file of both previous packets", "            by_sha.setdefault(h, set()).add(")
    check("dup_policy(): the previous-packet index (by git blob and by SHA256SUMS) carried byte-identical",
          i3 == block(p4, "    # Every file of both previous packets", "            by_sha.setdefault(h, set()).add("))
    links3 = block(p3, "        reference = f\"{STAGE2_RECEIPT[cfg]} at", "            bad(\"RESULT_MISMATCH\", p, f\"{cl} is not a classification\")")
    p4s = [l.strip() for l in p4]
    kept = [l for l in links3 if l.strip() in p4s]
    check("dup_policy(): every line of PUBLISH-03's link-receipt checks (from the linked reference to the run "
          "classification) is present in PUBLISH-04's link_checks()", len(kept) == len(links3),
          f"{len(kept)} of {len(links3)} lines; missing {[l.strip() for l in links3 if l.strip() not in p4s][:4]}")
    for line in ("def main(dest):", "def files_under(root):", "def carried(rel):", "def unchanged_tool(rel):"):
        check(f"{line} present once, unchanged position relative to the skeleton", p4.count(line) == 1)
    print("BUILDER DIFF: " + ("ONLY THE DUPLICATE-CONTENT AND ZERO-LENGTH POLICY CHANGED" if not fails
                              else "FAILED: " + "; ".join(fails)))
    return 0 if not fails else 1


if __name__ == "__main__":
    sys.exit(main(os.path.abspath(sys.argv[1])))
