#!/usr/bin/env python3
"""Stage 3 — independent AD-1-R4 H.1-H.5 check of the current-tree manifest ct-1 against Candidate C. Read-only.
It does not reuse any earlier H.5 tool. argv[1] = output directory.
"""
import json
import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from common import *  # noqa

OUT = sys.argv[1]
BASE = "9d5fdf95920bdeddb7453911f86390417b6a4604"
D2A1_SET = {"M32", "M78", "M79", "M80", "M89", "M112", "M113", "M141", "M143", "M153", "M177", "M178", "M179", "M180", "M87"}
EARLY_SET = {"M18", "M19", "M39", "M84", "M05", "M45", "M129"}
checks = []


def check(label, ok, detail=""):
    checks.append((label, bool(ok), str(detail)))
    return ok


ctb, hb = show(C, CT1[0]), show(C, HIST183[0])
check("H.2: ct-1 path at C, git blob 5d31cb45", blob_id(C, CT1[0]) == CT1[1], blob_id(C, CT1[0]))
check("H.2: ct-1 sha256 20bcc6fe...", sha(ctb) == CT1[2], sha(ctb))
ct, hist = json.loads(ctb), json.loads(hb)
check("H.2: ct-1 version 'ct-1'", ct.get("version") == "ct-1", ct.get("version"))
check("H.1: historical manifest at C: blob 65fa69dc, sha256 2c4adc84..., version 1-r1, 183 controls",
      blob_id(C, HIST183[0]) == HIST183[1] and sha(hb) == HIST183[2] and hist.get("version") == "1-r1"
      and len(hist["controls"]) == 183)
check("H.1: historical manifest unchanged at the evidence head", blob_id(HEAD_EV, HIST183[0]) == HIST183[1])
check("ct-1 unchanged at the evidence head", blob_id(HEAD_EV, CT1[0]) == CT1[1])
df = ct.get("derived_from", {})
check("H.3(1): derived_from names the original's path, sha256 and version", df.get("path") == HIST183[0] and
      df.get("sha256") == HIST183[2] and df.get("version") == "1-r1", df)
def keys(o):
    if isinstance(o, dict):
        for k, v in o.items():
            yield k
            yield from keys(v)
    elif isinstance(o, list):
        for v in o:
            yield from keys(v)


check("H.3(6): no self-reference: ct-1 has no 'final_tree' key and contains neither C's commit nor C's tree hash",
      "final_tree" not in set(keys(ct)) and C not in ctb.decode() and C_TREE not in ctb.decode())
check("historical base 9d5fdf95 is an ancestor of C", git("merge-base", "--is-ancestor", BASE, C, check=False) == "" and
      subprocess.run(["git", "-C", REPO, "merge-base", "--is-ancestor", BASE, C]).returncode == 0)

hc, cc = hist["controls"], ct["controls"]
check("H.3(2): 183 controls in each, same ids in the same order", [c["id"] for c in hc] == [c["id"] for c in cc]
      and len(cc) == 183)
DEF = ["id", "category", "historical_meaning", "candidate10_meaning", "historical", "edits", "command", "cwd",
       "expected_failure_tests"]
HISTF = ["review_status", "lineage", "review", "application_review", "source_sha256"]
file_cache = {}


def file_at(rev, path):
    k = (rev, path)
    if k not in file_cache:
        file_cache[k] = show(rev, path) if exists(rev, path) else None
    return file_cache[k]


def lines_of(text, anchor):
    out, start = [], 0
    while True:
        i = text.find(anchor, start)
        if i < 0:
            return out
        out.append(text.count("\n", 0, i) + 1)
        start = i + 1


rows, pairs, files, anchors = [], [], set(), 0
for h, c in zip(hc, cc):
    cid = c["id"]
    defeq = all(json.dumps(h.get(k), sort_keys=True) == json.dumps(c.get(k), sort_keys=True) for k in DEF)
    check(f"{cid}: H.3(2) definition fields equal the original's", defeq,
          [k for k in DEF if json.dumps(h.get(k), sort_keys=True) != json.dumps(c.get(k), sort_keys=True)])
    hr = c.get("historical_review", {})
    check(f"{cid}: H.3(3) historical review fields copied unchanged and marked historical",
          all(json.dumps(hr.get("fields", {}).get(k), sort_keys=True) == json.dumps(h.get(k), sort_keys=True) for k in HISTF)
          and "HISTORICAL" in hr.get("marker", "") and "Not current-tree approval" in hr.get("marker", ""))
    st = c.get("current_tree", {}).get("review_status")
    check(f"{cid}: H.3(4) current review status is 'pending' (no string confers acceptance)", st == "pending", st)
    cfiles = sorted({e["file"] for e in c["edits"]})
    check(f"{cid}: source_sha256 lists exactly the files the control edits", sorted(c["source_sha256"]) == cfiles,
          (sorted(c["source_sha256"]), cfiles))
    for f in cfiles:
        b = file_at(C, f)
        cur = sha(b) if b is not None else None
        files.add(f)
        recorded_hist = h["source_sha256"].get(f)
        eq_hist = cur == recorded_hist
        pairs.append((cid, f, eq_hist))
        check(f"{cid} {f}: H.3(5)/H.5 recorded source_sha256 equals the file at C", c["source_sha256"].get(f) == cur,
              (c["source_sha256"].get(f), cur))
        ctf = [x for x in c["current_tree"]["files"] if x["file"] == f]
        check(f"{cid} {f}: ct-1 current_tree record (sha, blob, equality flag) equals the recomputation",
              len(ctf) == 1 and ctf[0]["sha256"] == cur and ctf[0]["git_blob"] == blob_id(C, f)
              and ctf[0]["historical_recorded_sha256"] == recorded_hist and ctf[0]["sha256_equals_historical_record"] == eq_hist)
    # anchors: unedited counts and lines, and counts at application in order
    work = {f: file_at(C, f).decode() for f in cfiles}
    arows = []
    for i, e in enumerate(c["edits"], 1):
        anchors += 1
        un = file_at(C, e["file"]).decode()
        n_un, lines = un.count(e["anchor"]), lines_of(un, e["anchor"])
        n_app = work[e["file"]].count(e["anchor"])
        if n_app == e["count"]:
            work[e["file"]] = work[e["file"]].replace(e["anchor"], e["replacement"], e["count"])
        rec = [a for a in c["current_tree"]["anchors"] if a["edit"] == i]
        check(f"{cid} edit {i}: H.5 anchor occurs exactly count={e['count']} times at application", n_app == e["count"],
              (n_un, n_app))
        check(f"{cid} edit {i}: ct-1 anchor record equals the recomputation (unedited count, count at application, lines)",
              len(rec) == 1 and rec[0]["count_in_unedited_file"] == n_un and rec[0]["count_at_application"] == n_app
              and rec[0]["lines_in_unedited_file"] == lines and rec[0]["expected_count"] == e["count"], rec)
        arows.append({"edit": i, "file": e["file"], "count": e["count"], "unedited": n_un, "at_application": n_app,
                      "lines_at_C": lines})
    rows.append({"id": cid, "category": c["category"], "files": cfiles, "definition_equal": defeq,
                 "current_review_status": st, "historical_review_status": h.get("review_status"),
                 "historical_lineage": h.get("lineage"), "anchors": arows,
                 "pairs_equal_historical": {f: e for x, f, e in pairs if x == cid}})

changed = sorted({(x, f) for x, f, e in pairs if not e}, key=lambda t: int(t[0][1:]))
check("counts: 183 controls, 184 control-file pairs, 39 source files, 191 anchors",
      len(cc) == 183 and len(pairs) == 184 and len(files) == 39 and anchors == 191, (len(cc), len(pairs), len(files), anchors))
check("counts: 162 pairs equal their historical hash, 22 differ", sum(e for _, _, e in pairs) == 162 and len(changed) == 22,
      (sum(e for _, _, e in pairs), len(changed)))
check("the 22 changed pairs are exactly AD-1-R4 H.4's two sets (15 D2A-1, 7 earlier)",
      {x for x, _ in changed} == D2A1_SET | EARLY_SET, sorted({x for x, _ in changed}))
check("ct-1 summary's differing_pairs equals the recomputation",
      sorted(ct["current_tree_summary"]["differing_pairs"]) == sorted(f"{x} {f}" for x, f in changed))
check("ct-1 summary counts equal the recomputation", ct["current_tree_summary"]["recorded_control_file_pairs"] == 184 and
      ct["current_tree_summary"]["source_files"] == 39 and ct["current_tree_summary"]["anchor_edits"] == 191 and
      ct["current_tree_summary"]["controls_with_review_status_pending"] == 183)
check("H.5: the original manifest's sha256 at C is still 2c4adc84...", sha(show(C, HIST183[0])) == HIST183[2])

# ---- the 22: source-level reconciliation
fp = git("rev-list", "--first-parent", "--reverse", f"{BASE}..{C}").split()


def first_stale(path, hist_sha):
    prev_ok = sha(file_at(BASE, path)) == hist_sha if file_at(BASE, path) else False
    touched = set(git("log", "--first-parent", "--format=%H", f"{BASE}..{C}", "--", path).split())
    for cm in fp:
        if cm not in touched:
            continue
        b = file_at(cm, path)
        now_ok = b is not None and sha(b) == hist_sha
        if prev_ok and not now_ok:
            return cm, git("log", "-1", "--format=%s", cm).strip()
        prev_ok = now_ok
    return None, None


def numstat(path):
    o = git("diff", "--numstat", BASE, C, "--", path).split()
    return f"+{o[0]} -{o[1]}" if o else "+0 -0"


crate_files = {}


def test_body(rev, test):
    """Body of the test fn named by the last path segment; searched in every .rs of the crates that hold it."""
    name = test.split("::")[-1]
    key = rev
    if key not in crate_files:
        crate_files[key] = [p for p in git("ls-tree", "-r", "--name-only", rev, "--", "crates/nexus-governed-control",
                                         "app/src-tauri/src").split() if p.endswith(".rs")]
    hits = []
    for p in crate_files[key]:
        t = file_at(rev, p).decode(errors="replace")
        ls = t.splitlines(keepends=True)
        for k, line in enumerate(ls):
            m = re.match(rf"^([ \t]*)(?:pub(?:\([^)]*\))? )?(?:async )?fn {re.escape(name)}\s*\(", line)
            if not m:
                continue
            # rustfmt layout: the body ends at the first later line that is exactly the fn line's indentation + "}"
            end = next((j for j in range(k + 1, len(ls)) if ls[j].rstrip("\n") == m.group(1) + "}"), None)
            hits.append((p, "".join(ls[k:end + 1]) if end is not None else None))
    return hits


recon = []
for cid, f in changed:
    h = [x for x in hc if x["id"] == cid][0]
    c = [x for x in cc if x["id"] == cid][0]
    hs = h["source_sha256"][f]
    cm, subj = first_stale(f, hs)
    hb_base = file_at(BASE, f)
    tb = []
    for t in c["expected_failure_tests"]:
        a, b = test_body(BASE, t), test_body(C, t)
        same = len(a) == 1 and len(b) == 1 and a[0][1] is not None and a[0][1] == b[0][1]
        tb.append({"test": t, "base_hits": [p for p, _ in a], "c_hits": [p for p, _ in b], "body_byte_identical": same})
    lines_base = [lines_of(hb_base.decode(), e["anchor"]) for e in c["edits"] if e["file"] == f] if hb_base else None
    lines_c = [lines_of(file_at(C, f).decode(), e["anchor"]) for e in c["edits"] if e["file"] == f]
    r = {"control": cid, "set": "D2A-1 (15)" if cid in D2A1_SET else "earlier Candidate 11 (7)", "file": f,
         "historical_recorded_sha256": hs, "base_9d5fdf95_sha256_equals_record": hb_base is not None and sha(hb_base) == hs,
         "current_sha256": sha(file_at(C, f)), "current_blob": blob_id(C, f), "first_stale_commit": cm,
         "first_stale_subject": subj, "diff_base_to_C": numstat(f), "anchor_lines_base": lines_base,
         "anchor_lines_C": lines_c, "definition_unchanged": all(json.dumps(h.get(k), sort_keys=True) ==
                                                                json.dumps(c.get(k), sort_keys=True) for k in DEF),
         "expected_test_bodies": tb, "ct1_current_review_status": c["current_tree"]["review_status"]}
    check(f"{cid}: the historical base holds the recorded hash, so 'stale' is measured from it", r["base_9d5fdf95_sha256_equals_record"])
    check(f"{cid}: first-stale commit found on C's first-parent line", cm is not None, cm)
    check(f"{cid}: anchors, replacements, command and expected tests unchanged (definition equal)", r["definition_unchanged"])
    check(f"{cid}: every expected-failure test body is byte-identical between 9d5fdf95 and C (one definition each)",
          all(t["body_byte_identical"] for t in tb), [t["test"] for t in tb if not t["body_byte_identical"]])
    if cid in D2A1_SET:
        check(f"{cid}: D2A-1 set: first stale at the D2A-1 source commit 22ebf9a5",
              cm is not None and cm.startswith("22ebf9a5"), cm)
    recon.append(r)

json.dump({"controls": rows, "pairs": [{"control": x, "file": f, "equals_historical": e} for x, f, e in pairs],
           "changed_22": recon, "counts": {"controls": len(cc), "pairs": len(pairs), "files": len(files),
                                          "anchors": anchors, "pairs_equal": sum(e for _, _, e in pairs),
                                          "pairs_changed": len(changed)},
           "identities": {"ct1": {"path": CT1[0], "blob": CT1[1], "sha256": sha(ctb), "version": ct.get("version")},
                          "historical": {"path": HIST183[0], "blob": HIST183[1], "sha256": sha(hb)}, "commit": C,
                          "tree": C_TREE}},
          open(os.path.join(OUT, "s3-alignment.json"), "w"), indent=1)
fails = [x for x in checks if not x[1]]
with open(os.path.join(OUT, "s3-checks.txt"), "w") as fo:
    for label, ok, d in checks:
        fo.write(f"{'OK  ' if ok else 'FAIL'} {label}" + (f" :: {d}" if (d and not ok) else "") + "\n")
    fo.write(f"\nRESULT: {len(checks) - len(fails)} of {len(checks)} checks hold; {len(fails)} fail\n")
print(f"{len(checks) - len(fails)} of {len(checks)} hold")
for label, ok, d in fails[:40]:
    print("FAIL", label, "::", d[:300])
sys.exit(0 if not fails else 1)
