#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-MUTATION-TRANCHE1-RESUME-01, section 6: the exact old-to-new tooling diff and the mechanical check
that only authorized differences exist. Read-only but for OUTDIR.

    tool_diff.py OUTDIR      writes OUTDIR/TOOL-REPAIR.diff, OUTDIR/AUTHORIZATION.md and OUTDIR/TOOL-DIFF-CHECK.txt
    tool_diff.py --check     prints the checks only (the pre-execution gate runs this against the frozen tools)

Old: the tools of the EXEC-01 evidence commit (original-tools/, each byte-checked against the previous packet's
SHA256SUMS at the commit). New: tools/. Checks:
  D1  original-tools/ is exactly the previous packet's tools/ (names and sha256)
  D2  every tool is UNCHANGED, CHANGED or ADDED exactly as the authorization map below declares; none removed
  D3  the tools the mission forbids changing are byte-identical: classify.py (semantic classifications), supervise.py
      (resource enforcement), worktrees.py, expectations.py (catalog expectations), configs.py, normalize.py,
      report.py, verify_tree.py, scope-wrap.sh, healthy.py, rehearsal.py, tool_controls.py
  D4  inside changed files, every function the repair does not need is unchanged (AST identity), and the byte
      comparison of verify_mutant.main and sections [1]-[5] of authenticate.main are unchanged
  D5  common.py: every name keeps its value except the declared mission/path/publication names; only declared names
      are added; build/test commands, environments, limits, order, identities unchanged
  D6  the carried-forward expectations equal the previous packet's (commands, selections, properties, ceilings)
"""
import ast
import difflib
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import common as K  # noqa: E402

NEW = os.path.join(K.EV, "tools")
OLD = os.path.join(K.EV, "original-tools")
I1 = "item 1 (verify_mutant.py: the path-aware status predicate of section 7)"
I2 = "item 2 (control.py: the restoration status predicate, the same path-aware policy)"
I3 = "item 3 (control gate/order: start after the Architect-accepted C22 record)"
I4 = ("item 4 (tool controls, freeze/final-gate logic, packet/index rendering and publication paths required to "
      "validate and document this repair)")
CHANGED = {
    "verify_mutant.py": [I1 + ": status_records/target_facts/target_entry_problem/mount_points/status_policy added; "
                         "main's three status lines and two status terms of `ok` replaced by status_policy; read-only "
                         "git (GIT_OPTIONAL_LOCKS=0); docstring"],
    "control.py": [I2 + ": restored_check added; restore's status listing and its predicate replaced by it; the "
                   "RESTORE.txt status line", I3 + ": gate() over RESUME_ORDER, C14's prior steps = the frozen C22 "
                   "ruling record, the carried-forward baselines and healthy build, the pre-execution gate; main() "
                   "refuses C22 and any id outside RESUME_ORDER; docstring"],
    "common.py": [I3 + ": ACCEPTED, RESUME_ORDER, C22_RULING, C22_RULING_FIRST",
                  I4 + ": MISSION, EV, EVIDENCE_WT, DEV_WT, PREV_*, CARRIED, CARRIED_DOCS, PACKET, BRANCH, LOCAL_BRANCH "
                  "(DEST follows BRANCH)"],
    "authenticate.py": [I4 + ": section [6] states the resume paths; section [7] authenticates the complete previous "
                        "packet, the C22 ruling's facts and the carried-forward copies (section 2; pre-execution and "
                        "final gates)"],
    "record_freeze.py": [I4 + ": freeze logic (section 10): covers original-tools/, the carried-forward receipts and "
                         "the C22 ruling record; this mission's pre-freeze receipts; policy text"],
    "final_gate.py": [I4 + ": final-gate logic: carried-forward receipts, C22 accepted, RESUME_ORDER, path-aware "
                      "restored checks"],
    "render.py": [I4 + ": packet/index rendering: combined Tranche-1 summary, CARRIED-FORWARD.md, INDEX"],
    "render_rehearsal.py": [I4 + ": the rendering's tool control, for the revised render.py"],
    "build_packet.py": [I4 + ": packet: the ruling record and CARRIED-FORWARD.md staged; carried-forward copies "
                        "excluded and checked not republished"],
    "publish_checks.py": [I4 + ": publication paths: parent 19eda8cc, resume packet and ref, previous packet unchanged, "
                          "key receipts"],
}
ADDED = {
    "verifier_controls.py": [I4 + ": the tool controls of section 8 (plus replay, sensitivity, randomized trials)"],
    "pre_exec_gate.py": [I4 + ": the static final pre-execution gate of section 10"],
    "tool_diff.py": [I4 + ": this diff and its check (section 6)"],
}
FORBIDDEN = ["classify.py", "supervise.py", "worktrees.py", "expectations.py", "configs.py", "normalize.py", "report.py",
             "verify_tree.py", "scope-wrap.sh", "healthy.py", "rehearsal.py", "tool_controls.py"]
SAME_FUNCS = {"control.py": ["git", "apply_patch", "link", "finish"],
              "verify_mutant.py": ["blob_id"],
              "authenticate.py": ["git", "gtext", "chk", "copy_in", "packet_sums", "patch_result"],
              "render.py": ["lines", "jload", "outer", "res", "control_row"],
              "build_packet.py": ["files_under"],
              "final_gate.py": ["check"],
              "publish_checks.py": ["git", "check", "tree_map", "glob_re", "push_matches", "gh", "mutant_blobs",
                                    "remote_blob", "actions"],
              "common.py": ["control_wt", "build_env", "git", "blob", "source", "sha256_bytes", "sha256_file",
                            "git_blob_id", "inputs"]}
COMMON_CHANGED = {"MISSION", "EV", "EVIDENCE_WT", "PACKET", "BRANCH", "DEST", "LOCAL_BRANCH"}
COMMON_ADDED = {"DEV_WT", "ACCEPTED", "RESUME_ORDER", "C22_RULING", "C22_RULING_FIRST", "PREV_MISSION", "PREV",
                "PREV_TREE", "PREV_PARENT", "PREV_REF", "PREV_PACKET", "PREV_EVIDENCE_WT", "PREV_DEV_WT", "CARRIED",
                "CARRIED_DOCS"}
results = []


def check(label, ok, detail=""):
    results.append((label, bool(ok), str(detail)))


def funcs(path):
    tree = ast.parse(open(path).read())
    return {n.name: ast.dump(n) for n in tree.body if isinstance(n, ast.FunctionDef)}, tree


def fn(tree, name):
    return next(n for n in tree.body if isinstance(n, ast.FunctionDef) and n.name == name)


def namespace(path):
    ns = {}
    exec(compile(open(path).read(), path, "exec"), ns)
    return {k: v for k, v in ns.items() if not k.startswith("__") and not callable(v) and k not in ("hashlib", "os",
                                                                                                    "subprocess")}


def run():
    prev = {}
    for line in K.git("show", f"{K.PREV}:{K.PREV_PACKET}/SHA256SUMS").decode().splitlines():
        h, rel = line.split("  ", 1)
        if rel.startswith("tools/"):
            prev[rel[6:]] = h
    old = {f: K.sha256_file(os.path.join(OLD, f)) for f in sorted(os.listdir(OLD))}
    new = {f: K.sha256_file(os.path.join(NEW, f)) for f in sorted(os.listdir(NEW)) if f.endswith((".py", ".sh"))}
    check("D1 original-tools/ is exactly the EXEC-01 packet's tools/ (names and sha256 from its SHA256SUMS at "
          f"{K.PREV[:8]})", old == prev, f"{len(old)} files")
    status = {f: ("UNCHANGED" if f in new and new[f] == old[f] else "CHANGED" if f in new else "REMOVED") for f in old}
    status.update({f: "ADDED" for f in new if f not in old})
    check("D2 no tool removed", not [f for f, s in status.items() if s == "REMOVED"])
    check("D2 the changed tools are exactly the declared ones", sorted(f for f, s in status.items() if s == "CHANGED")
          == sorted(CHANGED), sorted(f for f, s in status.items() if s == "CHANGED"))
    check("D2 the added tools are exactly the declared ones", sorted(f for f, s in status.items() if s == "ADDED")
          == sorted(ADDED), sorted(f for f, s in status.items() if s == "ADDED"))
    for f in FORBIDDEN:
        check(f"D3 {f} byte-identical (sha256 {old.get(f, '')[:16]}…)", status.get(f) == "UNCHANGED", status.get(f))
    for f, names in SAME_FUNCS.items():
        a, ta = funcs(os.path.join(OLD, f))
        b, tb = funcs(os.path.join(NEW, f))
        names = [n for n in names if n in a]
        diff = [n for n in names if a.get(n) != b.get(n)]
        check(f"D4 {f}: unchanged functions {names} (AST identity)", not diff, diff)
    # verify_mutant.main: the byte comparison is unchanged; `ok` lost exactly the two status terms and gained pol['ok'].
    src_old, src_new = open(os.path.join(OLD, "verify_mutant.py")).read(), open(os.path.join(NEW, "verify_mutant.py")).read()
    seg = lambda s: s[s.index("    entries = []\n"):s.index("    proc.wait()\n")]
    check("D4 verify_mutant.main: the byte comparison (ls-tree of C, every blob, mode, missing, mutant identity) is "
          "unchanged text", seg(src_old) == seg(src_new))
    terms = lambda t: [ast.unparse(v) for v in next(s for s in fn(t, "main").body if isinstance(s, ast.Assign) and
                                                    s.targets[0].id == "ok").value.values]
    to, tn = terms(ast.parse(src_old)), terms(ast.parse(src_new))
    check("D4 verify_mutant.main `ok`: the original terms minus the two exact-string status terms, plus pol['ok']",
          [t for t in to if t not in ("not unexpected_status", "' M ' + path in status")] == [t for t in tn if t !=
                                                                                            "pol['ok']"]
          and tn.count("pol['ok']") == 1 and len(tn) == len(to) - 1, f"old {to}; new {tn}")
    # control.restore: only the status predicate changed.
    ro, rn = (ast.unparse(fn(ast.parse(open(os.path.join(d, "control.py")).read()), "restore")) for d in (OLD, NEW))
    keep = lambda s: [l for l in s.splitlines() if "status" not in l and "rv" not in l and "v = " not in l
                      and "chk" not in l and "pol" not in l and "verify_tree" not in l]
    check("D4 control.restore: every line not about the status predicate or its record is unchanged",
          keep(ro) == keep(rn), [l for l in difflib.unified_diff(keep(ro), keep(rn), lineterm="")][:6])
    mo, mn = (fn(ast.parse(open(os.path.join(d, "control.py")).read()), "main") for d in (OLD, NEW))
    check("D4 control.main: unchanged but for the leading refusal of C22 and any id outside RESUME_ORDER",
          [ast.dump(s) for s in mo.body] == [ast.dump(s) for s in mn.body[1:]]
          and "RESUME_ORDER" in ast.unparse(mn.body[0]))
    ao, an = (fn(ast.parse(open(os.path.join(d, "authenticate.py")).read()), "main") for d in (OLD, NEW))
    cut = next(i for i, s in enumerate(ao.body) if isinstance(s, ast.Assign) and ast.unparse(s).startswith("paths ="))
    check("D4 authenticate.main: sections [1]-[5] and the [6] ref checks unchanged (statement by statement)",
          [ast.dump(s) for s in ao.body[:cut]] == [ast.dump(s) for s in an.body[:cut]], f"{cut} statements")
    co, cn = namespace(os.path.join(OLD, "common.py")), namespace(os.path.join(NEW, "common.py"))
    changed = {k for k in co if k in cn and co[k] != cn[k]}
    check("D5 common.py: no name removed", not [k for k in co if k not in cn], [k for k in co if k not in cn])
    check("D5 common.py: changed values only for the declared mission/path/publication names", changed <= COMMON_CHANGED,
          sorted(changed))
    check("D5 common.py: only declared names added", set(cn) - set(co) <= COMMON_ADDED, sorted(set(cn) - set(co)))
    check("D5 common.py: commands, environments, limits, order and identities unchanged",
          all(co[k] == cn[k] for k in ("C", "C_TREE", "BLOBS", "CATALOG_BLOB", "CATALOG_SHA", "BUILD_ARGV",
                                        "BUILD_LIMIT", "RUN_ENV", "MEMORY_MAX", "NEED_KIB", "ORDER", "NOT_AUTHORIZED",
                                        "NAMED", "EXPLORE_CFG", "EXE", "EXE_SHA256", "EXE_SIZE", "LISTED",
                                        "UNIT_PREFIX", "HEALTHY_WT")))
    e = os.path.join(K.EV, "receipts/02-EXPECTATIONS.json")
    want = [l.split("  ")[0] for l in K.git("show", f"{K.PREV}:{K.PREV_PACKET}/SHA256SUMS").decode().splitlines()
            if l.endswith("  receipts/02-EXPECTATIONS.json")]
    check("D6 the carried-forward expectations equal the previous packet's", [K.sha256_file(e)] == want, want)
    return old, new, status


def main(argv):
    old, new, status = run()
    bad = [r for r in results if not r[1]]
    lines = [f"{'OK  ' if o else 'FAIL'} {l}{(': ' + d) if d else ''}" for l, o, d in results]
    lines.append("TOOL DIFF: " + ("ALL CHECKS HOLD" if not bad else "FAILED"))
    if argv[0] == "--check":
        print("\n".join(lines))
        return 0 if not bad else 1
    out = os.path.abspath(argv[0])
    if os.path.exists(out):
        raise SystemExit("REFUSED: output exists")
    os.makedirs(out)
    diff = []
    for f in sorted(status):
        if status[f] in ("CHANGED", "ADDED"):
            a = open(os.path.join(OLD, f)).read().splitlines(keepends=True) if status[f] == "CHANGED" else []
            b = open(os.path.join(NEW, f)).read().splitlines(keepends=True)
            diff += difflib.unified_diff(a, b, f"EXEC-01/tools/{f}" if a else "/dev/null", f"RESUME-01/tools/{f}")
    open(os.path.join(out, "TOOL-REPAIR.diff"), "w").write("".join(diff))
    md = [f"# Tool repair authorization map — {K.MISSION}", "",
          f"Old: the tools of `{K.PREV_PACKET}/tools/` at `{K.PREV}`. New: this mission's `tools/`. The exact diff is "
          "`TOOL-REPAIR.diff`; the mechanical checks are `TOOL-DIFF-CHECK.txt`.", "",
          "| Tool | Status | Old sha256 | New sha256 | Authorization |", "|---|---|---|---|---|"]
    for f in sorted(status):
        why = "; ".join(CHANGED.get(f, ADDED.get(f, ["byte-identical (no change)"])))
        md.append(f"| `{f}` | {status[f]} | `{old.get(f, '—')}` | `{new.get(f, '—')}` | {why} |")
    md += ["", "Not changed (mission section 6): classify.py's semantic classifications, supervise.py's resource "
           "enforcement, the catalog expectations, any patch, any source, the named-property logic, cardinality logic, "
           "the build command and the test commands."]
    open(os.path.join(out, "AUTHORIZATION.md"), "w").write("\n".join(md) + "\n")
    open(os.path.join(out, "TOOL-DIFF-CHECK.txt"), "w").write("\n".join(lines) + "\n")
    print("\n".join(lines))
    return 0 if not bad else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
