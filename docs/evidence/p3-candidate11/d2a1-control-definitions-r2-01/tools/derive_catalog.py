#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-CONTROL-DEFINITIONS-R2-01, mission sections 4, 5, 6 and 7: derive the R2 candidate catalog
(read-only with respect to the repository).

    derive_catalog.py ROOT [OUTDIR]

Reads only ROOT/inputs/ (written and hashed by authenticate.py) and tools/a6_correction.py. Writes OUTDIR (default
ROOT/out/catalog): catalog.json, CATALOG.md, PATCH-IDENTITIES.md, patches/<id>.patch for all 35 kill-required
controls, and OUTDIR/../comparison (R1-TO-R2-PATCH-COMPARISON.md, A6-R1-TO-R2-PATCH.diff, A6-R1-TO-R2-MUTANT.diff).
Deterministic: no clock, no host path, sorted keys; two runs give identical bytes.

  - 34 controls (every one but A6) and A4: the R1 catalog entry, unchanged as a whole object, and the R1 patch file
    byte for byte (mission 6).
  - A6: the R1 A6 edits, unchanged, plus the one KEY_FIELDS receipt-metadata edit appended (mission 4). The R1 edits
    alone must regenerate the R1 patch byte for byte; the R2 patch is generated from the nine edits (git diff
    --no-index, the R1 neutral configuration) and must contain every R1 hunk byte for byte plus exactly one more.
Nothing is applied to any worktree; Candidate C is read, never written.
"""
import copy
import hashlib
import json
import os
import re
import subprocess
import sys
import tempfile

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import a6_correction as A6C  # noqa: E402

MISSION = "P3-C11-DR01-D2A1-CONTROL-DEFINITIONS-R2-01"
VERSION = "d2a1-controls-v1-r2-candidate"
R1_MISSION = "P3-C11-DR01-D2A1-CONTROL-DEFINITIONS-R1-01"
R1_VERSION = "d2a1-controls-v1-r1-candidate"
R1_COMMIT = "f81159046bf0b62666851bc1185992318302d31f"
R1_REF = "refs/heads/review/p3-c11-d2a1-control-definitions-r1-01"
R1_CATALOG_SHA = "a73d5721ec59a5d14abf5c31db72d1da1d34962d1db7294347797dd677ae6479"
R1_CATALOG_BLOB = "56d4424397be7740cefd8d89b3530aab4b25d3ed"
MODEL = "crates/nexus-governed-control/src/ledger/protocol/model_tests.rs"
FILES = {"crates/nexus-governed-control/src/ledger/protocol.rs": "C-protocol.rs", MODEL: "C-protocol-model_tests.rs"}
BLOBS = {"crates/nexus-governed-control/src/ledger/protocol.rs": "a2df2841b99cad5d452008a6d4150e9369d07e66",
         MODEL: "28085ce232827301104b5d757af042ec933976f8"}
A6_CHANGED_KEYS = ["definition_origin", "edits", "execution_dependent_uncertainty", "mutant_blob", "mutant_sha256",
                   "one_named_change", "patch", "patch_lines_removed_added", "patch_origin", "patch_sha256",
                   "preserved_behaviour", "r2_correction", "receipt_metadata_item"]


def sha(b):
    return hashlib.sha256(b).hexdigest()


def blob_id(data):
    return subprocess.run(["git", "hash-object", "--stdin"], input=data, capture_output=True,
                          check=True).stdout.decode().strip()


def make_patch(path, base, mutant, label_a=None, label_b=None):
    """A git-format unified diff of one file (git diff --no-index in a scratch directory, every configuration
    source neutralised, so the bytes depend only on the two inputs). The R1 tool's function, unchanged."""
    env = dict(os.environ, GIT_CONFIG_GLOBAL="/dev/null", GIT_CONFIG_NOSYSTEM="1", LC_ALL="C")
    with tempfile.TemporaryDirectory() as d:
        for side, data in (("a", base), ("b", mutant)):
            p = os.path.join(d, side, path)
            os.makedirs(os.path.dirname(p), exist_ok=True)
            with open(p, "wb") as fh:
                fh.write(data)
            os.chmod(p, 0o644)
        r = subprocess.run(["git", "diff", "--no-index", "--no-color", "--no-ext-diff", "--full-index", "-U3",
                            "--diff-algorithm=myers", "--src-prefix=", "--dst-prefix=", "a/" + path, "b/" + path],
                           cwd=d, capture_output=True, env=env)
        if r.returncode != 1:
            raise SystemExit(f"git diff --no-index: rc {r.returncode} {r.stderr.decode()}")
        return r.stdout


def removed_added(patch):
    t = patch.decode().split("\n")
    return [sum(1 for l in t if l.startswith("-") and not l.startswith("---")),
            sum(1 for l in t if l.startswith("+") and not l.startswith("+++"))]


def apply_edits(base_t, edits):
    """Each anchor once in the baseline, at a line start, spans disjoint; applied in order."""
    spans = []
    for e in edits:
        n = base_t.count(e["anchor"])
        i = base_t.find(e["anchor"])
        if n != 1 or (i > 0 and base_t[i - 1] != "\n"):
            raise SystemExit(f"anchor not once at a line start: count {n}")
        spans.append((i, i + len(e["anchor"])))
    s = sorted(spans)
    if any(a[1] > b[0] for a, b in zip(s, s[1:])):
        raise SystemExit("overlapping anchors")
    out = base_t
    for e in edits:
        if out.count(e["anchor"]) != 1:
            raise SystemExit("anchor no longer once after an earlier edit")
        out = out.replace(e["anchor"], e["replacement"], 1)
    return out, spans


def split_patch(patch):
    """(header lines, [hunk text]) of a one-file git patch; a hunk's text runs from its @@ line to the next."""
    lines = patch.decode().split("\n")
    assert lines[-1] == ""
    lines = lines[:-1]
    first = next(i for i, l in enumerate(lines) if l.startswith("@@ "))
    hunks, cur = [], None
    for l in lines[first:]:
        if l.startswith("@@ "):
            cur = [l]
            hunks.append(cur)
        else:
            cur.append(l)
    return lines[:first], ["\n".join(h) + "\n" for h in hunks]


def a6_entry(r1e, base_t, r1_patch):
    """The R2 A6 entry: R1's, with only the fields that must change because A6 changed."""
    r1_edits = [{"anchor": x["anchor"], "replacement": x["replacement"]} for x in r1e["edits"]]
    r1_mut_t, _ = apply_edits(base_t, r1_edits)
    regenerated = make_patch(MODEL, base_t.encode(), r1_mut_t.encode())
    if regenerated != r1_patch or sha(r1_mut_t.encode()) != r1e["mutant_sha256"]:
        raise SystemExit("A6: the R1 edits do not regenerate the R1 patch and mutant byte for byte")
    edits = r1_edits + [{"anchor": A6C.EDIT["anchor"], "replacement": A6C.EDIT["replacement"]}]
    mut_t, spans = apply_edits(base_t, edits)
    mutant = mut_t.encode()
    patch = make_patch(MODEL, base_t.encode(), mutant)
    h1, k1 = split_patch(r1_patch)
    h2, k2 = split_patch(patch)
    mblob = blob_id(mutant)
    extra = [h for h in k2 if h not in k1]
    if not (len(k2) == len(k1) + 1 and [h for h in k2 if h in k1] == k1 and len(extra) == 1):
        raise SystemExit("A6: the R2 patch is not the R1 hunks byte for byte plus exactly one")
    want_h2 = [h1[0], f"index {BLOBS[MODEL]}..{mblob} 100644", h1[2], h1[3]]
    if h2 != want_h2 or h1[1] != f"index {BLOBS[MODEL]}..{r1e['mutant_blob']} 100644":
        raise SystemExit("A6: the patch header differs from R1's other than in the mutant blob id")
    body = [l for l in extra[0].split("\n")[1:] if l]
    minus = [l[1:] for l in body if l.startswith("-")]
    plus = [l[1:] for l in body if l.startswith("+")]
    if minus != [A6C.KEY_FIELDS_ANCHOR[:-1]] or plus != [A6C.KEY_FIELDS_REPLACEMENT[:-1]]:
        raise SystemExit("A6: the added hunk is not exactly the KEY_FIELDS line")
    e = copy.deepcopy(r1e)
    x = {"anchor": A6C.EDIT["anchor"], "replacement": A6C.EDIT["replacement"], "expected_count": 1,
         "anchor_first_line_at_C": base_t.count("\n", 0, spans[-1][0]) + 1, "role": A6C.EDIT["role"]}
    e["edits"] = copy.deepcopy(r1e["edits"]) + [x]
    e["patch"] = patch.decode()
    e["patch_sha256"] = sha(patch)
    e["mutant_blob"] = mblob
    e["mutant_sha256"] = sha(mutant)
    e["patch_lines_removed_added"] = removed_added(patch)
    e["definition_origin"] = r1e["definition_origin"] + [A6C.DEFINITION_ORIGIN_NOTE]
    e["patch_origin"] = r1e["patch_origin"] + A6C.PATCH_ORIGIN_NOTE
    e["one_named_change"] = r1e["one_named_change"] + A6C.ONE_CHANGE_NOTE
    e["preserved_behaviour"] = r1e["preserved_behaviour"] + [A6C.PRESERVED_NOTE]
    unc = r1e["execution_dependent_uncertainty"]
    if unc.count(A6C.R1_KEY_FIELDS_UNCERTAINTY) != 1:
        raise SystemExit("A6: the R1 KEY_FIELDS uncertainty item is not there once")
    e["execution_dependent_uncertainty"] = [A6C.R2_KEY_FIELDS_UNCERTAINTY if u == A6C.R1_KEY_FIELDS_UNCERTAINTY
                                            else u for u in unc]
    e["receipt_metadata_item"] = A6C.ITEM
    rc = dict(A6C.R2_CORRECTION)
    rc.update({"r1_patch_sha256": r1e["patch_sha256"], "r1_mutant_blob": r1e["mutant_blob"],
               "r1_mutant_sha256": r1e["mutant_sha256"], "r1_patch_lines_removed_added":
               r1e["patch_lines_removed_added"], "added_hunk": extra[0],
               "added_hunk_position": f"hunk {k2.index(extra[0]) + 1} of {len(k2)} (between the R1 hunks for "
                                      "initial and encode, in file order)",
               "anchor_first_line_at_C": x["anchor_first_line_at_C"]})
    e["r2_correction"] = rc
    changed = sorted(k for k in set(e) | set(r1e) if e.get(k) != r1e.get(k))
    if changed != A6_CHANGED_KEYS:
        raise SystemExit(f"A6: changed keys {changed} are not the expected set")
    return e, mut_t, r1_mut_t


def main(root, outdir):
    inp = lambda n: open(os.path.join(root, "inputs", n), "rb").read()  # noqa: E731
    r1_raw = inp("r1/catalog.json")
    if sha(r1_raw) != R1_CATALOG_SHA or blob_id(r1_raw) != R1_CATALOG_BLOB:
        raise SystemExit("the R1 catalog is not the authenticated one")
    r1 = json.loads(r1_raw)
    if r1["version"] != R1_VERSION:
        raise SystemExit("unexpected R1 version")
    src = {p: inp(n) for p, n in FILES.items()}
    for p, data in src.items():
        if blob_id(data) != BLOBS[p]:
            raise SystemExit(f"{p} is not C's blob")
    os.makedirs(os.path.join(outdir, "patches"), exist_ok=True)
    cmpdir = os.path.join(os.path.dirname(os.path.abspath(outdir)), "comparison")
    os.makedirs(cmpdir, exist_ok=True)
    controls, rows = [], []
    for r1e in r1["controls"]:
        cid = r1e["id"]
        if cid == "A4":
            controls.append(copy.deepcopy(r1e))
            continue
        r1_patch = inp(f"r1/patches/{cid}.patch")
        if r1_patch != r1e["patch"].encode() or sha(r1_patch) != r1e["patch_sha256"]:
            raise SystemExit(f"{cid}: the R1 patch file is not the R1 catalog's")
        if cid == "A6":
            e, mut_t, r1_mut_t = a6_entry(r1e, src[MODEL].decode(), r1_patch)
            patch = e["patch"].encode()
        else:
            e, patch = copy.deepcopy(r1e), r1_patch
        with open(os.path.join(outdir, "patches", f"{cid}.patch"), "wb") as fh:
            fh.write(patch)
        controls.append(e)
        rows.append((cid, r1e, e))
    kill = [c for c in controls if c["id"] != "A4"]
    unchanged = [cid for cid, a, b in rows if a == b]
    changed = [cid for cid, a, b in rows if a != b]
    if changed != ["A6"] or len(unchanged) != 34:
        raise SystemExit(f"unexpected changed set {changed}")
    top = {k: copy.deepcopy(v) for k, v in r1.items() if k != "controls"}
    top["version"] = VERSION
    top["mission"] = MISSION
    top["supersedes_as_candidate"] = {
        "version": R1_VERSION, "sha256": R1_CATALOG_SHA, "git_blob": R1_CATALOG_BLOB, "evidence_commit": R1_COMMIT,
        "review_ref": R1_REF, "mission": R1_MISSION,
        "status": "immutable review history (a candidate in which the Architect's review found one blocking defect, "
                  "A6's receipt metadata: mission R2 section 1); neither authoritative nor edited",
        "previous": copy.deepcopy(r1["supersedes_as_candidate"])}
    top["sources"]["r1_candidate_catalog"] = {
        "sha256": R1_CATALOG_SHA, "git_blob": R1_CATALOG_BLOB, "evidence_commit": R1_COMMIT,
        "use": "every entry except A6 carried as a whole object, and every patch except A6's carried byte for byte; "
               "A6's eight R1 edits carried unchanged"}
    top["sources"]["a6_receipt_metadata_repair"] = {
        "mission": MISSION, "section": "4",
        "status": "ARCHITECT-SPECIFIED — EXECUTOR-ENCODED — INDEPENDENT ARCHITECT REVIEW PENDING"}
    top["r2"] = {"changed_controls": changed, "unchanged_controls": unchanged,
                 "rule": "mission R2 section 6: the 34 patches other than A6 are byte-identical to R1, and their "
                         "catalog entries are R1's unchanged; A6 changes only by one receipt-metadata hunk (KEY_FIELDS) "
                         "and the identities and description fields derived from it",
                 "a6_changed_fields": A6_CHANGED_KEYS}
    catalog = dict(top)
    catalog["controls"] = controls
    with open(os.path.join(outdir, "catalog.json"), "w") as fh:
        json.dump(catalog, fh, indent=1, sort_keys=True, ensure_ascii=False)
        fh.write("\n")
    with open(os.path.join(outdir, "CATALOG.md"), "w") as fh:
        fh.write(render(catalog))
    with open(os.path.join(outdir, "PATCH-IDENTITIES.md"), "w") as fh:
        fh.write(identities(catalog))
    with open(os.path.join(cmpdir, "R1-TO-R2-PATCH-COMPARISON.md"), "w") as fh:
        fh.write(comparison(rows))
    a6 = next(e for e in controls if e["id"] == "A6")
    r1a6 = next(e for e in r1["controls"] if e["id"] == "A6")
    with open(os.path.join(cmpdir, "A6-R1-TO-R2-PATCH.diff"), "wb") as fh:
        fh.write(make_patch("catalog/patches/A6.patch", r1a6["patch"].encode(), a6["patch"].encode()))
    with open(os.path.join(cmpdir, "A6-R1-TO-R2-MUTANT.diff"), "wb") as fh:
        fh.write(make_patch(MODEL, r1_mut_t.encode(), mut_t.encode()))
    print(f"catalog {VERSION}: {len(controls)} entries ({len(kill)} kill-required + {len(controls) - len(kill)} "
          f"comparison); CANDIDATE_READY {catalog['counts']['candidate_ready']}; blocked {catalog['counts']['blocked']}; "
          f"unchanged from R1 {len(unchanged)}; changed {changed}")
    print(f"catalog.json sha256 {sha(open(os.path.join(outdir, 'catalog.json'), 'rb').read())}")
    print(f"A6 patch sha256 {a6['patch_sha256']}; mutant blob {a6['mutant_blob']}; mutant sha256 {a6['mutant_sha256']}")
    return 0


def render(cat):
    out = [f"# {cat['name']} — `{cat['version']}`", "",
           f"Status: **{cat['catalog_status']}**. Acceptance {cat['acceptance']}.", "",
           f"Baseline: Candidate C `{cat['baseline']['commit']}` (tree `{cat['baseline']['tree']}`).", "",
           f"Supersedes as a candidate: `{cat['supersedes_as_candidate']['version']}` (sha256 "
           f"`{cat['supersedes_as_candidate']['sha256']}`, evidence commit "
           f"`{cat['supersedes_as_candidate']['evidence_commit']}`), immutable review history. Changed from it: "
           f"{', '.join(cat['r2']['changed_controls'])} only (one KEY_FIELDS receipt-metadata hunk); the other "
           f"{len(cat['r2']['unchanged_controls'])} kill-required controls and A4 are unchanged.", "",
           f"Kill-required: {cat['counts']['kill_required']} (definition CANDIDATE_READY: "
           f"{cat['counts']['candidate_ready']}; carried forward {cat['counts']['carried_forward']}, Architect-defined "
           f"{cat['counts']['architect_defined']}). Separate comparison: {cat['counts']['comparison']} (A4).", "",
           "| Id | Change (contract 7.5) | Named | On / test | Receipt type | Definition | Source | R2 | Review | "
           "Execution |", "|---|---|---|---|---|---|---|---|---|---|"]
    for c in cat["controls"]:
        named = c.get("named_property")
        named = " / ".join(named) if isinstance(named, list) else named
        on = c.get("test") or c.get("configuration")
        r2 = "changed (KEY_FIELDS hunk)" if c["id"] in cat["r2"]["changed_controls"] else "= R1"
        out.append(f"| {c['id']} | {c['description']} | {named} | `{on}` | {c['receipt_type'].split(';')[0]} | "
                   f"{c.get('definition_status', c.get('status'))} | {c.get('definition_source', 'comparison')} | "
                   f"{r2} | {c['architect_review_status']} | {c['execution_status'].split(' (')[0]} |")
    out += ["", "The machine-readable record — edits, patches, identities, receipt and restoration mapping — is "
            "`catalog.json`; `PATCH-IDENTITIES.md` lists every patch and mutant identity; "
            "`../comparison/R1-TO-R2-PATCH-COMPARISON.md` compares every patch with R1's.", ""]
    return "\n".join(out)


def identities(cat):
    out = ["# Patch identity table — candidate catalog `" + cat["version"] + "`", "",
           "Every patch applies to Candidate C (`" + cat["baseline"]["commit"] + "`) and changes one file. Mutant "
           "blob: `git hash-object` of the patched bytes. None of these patches has been applied to any worktree; no "
           "mutant has been built or run. Only A6 differs from R1 (`" + cat["supersedes_as_candidate"]["version"] +
           "`).", "",
           "| Id | Source | File | Item | Edits | -/+ lines | Patch sha256 | Mutant blob | Mutant sha256 |",
           "|---|---|---|---|---|---|---|---|---|"]
    for c in cat["controls"]:
        if c["id"] == "A4":
            continue
        f = c["mutation_source_file"].rsplit("/", 1)[1]
        item = c.get("architect_item") or "—"
        if c.get("receipt_metadata_item"):
            item += f"; {c['receipt_metadata_item']} (receipt metadata)"
        out.append(f"| {c['id']} | {c['definition_source']} | `{f}` | {item} | "
                   f"{len(c['edits'])} | {c['patch_lines_removed_added'][0]}/{c['patch_lines_removed_added'][1]} | "
                   f"`{c['patch_sha256']}` | `{c['mutant_blob']}` | `{c['mutant_sha256']}` |")
    out.append("")
    return "\n".join(out)


def comparison(rows):
    keys = ["patch bytes", "patch sha256", "mutant blob", "mutant sha256", "source file", "property/test",
            "configuration", "receipt type", "restoration mapping", "special attribution rules", "whole entry"]

    def facts(e):
        return {"patch bytes": e["patch"], "patch sha256": e["patch_sha256"], "mutant blob": e["mutant_blob"],
                "mutant sha256": e["mutant_sha256"], "source file": e["mutation_source_file"],
                "property/test": (e.get("named_property"), e.get("named_property_text"), e.get("test")),
                "configuration": e.get("configuration"), "receipt type": e["receipt_type"],
                "restoration mapping": (e["restoration"], e["restored_receipt"]),
                "special attribution rules": (e.get("special_attribution_rules"), e.get("additional_comparison")),
                "whole entry": e}
    out = ["# R1 → R2 patch identity comparison", "",
           f"R1: `{R1_VERSION}` (sha256 `{R1_CATALOG_SHA}`, evidence commit `{R1_COMMIT}`). R2: `{VERSION}`.", "",
           "Each row compares the R2 entry with the R1 entry field by field (`=` equal, `≠` different). The 34 "
           "controls other than A6 must be equal in every column, the whole entry included; A6 may differ only in "
           "its patch, its mutant identities and the description fields that the KEY_FIELDS hunk makes stale (the "
           "catalog's `r2.a6_changed_fields`).", "",
           "| Id | " + " | ".join(keys) + " | R1 patch sha256 | R2 patch sha256 | R1 mutant blob | R2 mutant blob |",
           "|---|" + "---|" * (len(keys) + 4)]
    for cid, a, b in rows:
        fa, fb = facts(a), facts(b)
        out.append(f"| {cid} | " + " | ".join("=" if fa[k] == fb[k] else "≠" for k in keys) +
                   f" | `{a['patch_sha256']}` | `{b['patch_sha256']}` | `{a['mutant_blob']}` | `{b['mutant_blob']}` |")
    same = [cid for cid, a, b in rows if a == b]
    out += ["", f"Unchanged, whole entry and patch bytes: {len(same)} — {', '.join(same)}.", "",
            "Changed: " + ", ".join(cid for cid, a, b in rows if a != b) + ". The exact A6 difference is "
            "`A6-R1-TO-R2-PATCH.diff` (R1 patch → R2 patch: the index line's mutant blob and one added hunk) and "
            "`A6-R1-TO-R2-MUTANT.diff` (R1 mutant → R2 mutant: the one KEY_FIELDS line).", ""]
    return "\n".join(out)


if __name__ == "__main__":
    r = sys.argv[1]
    sys.exit(main(r, sys.argv[2] if len(sys.argv) > 2 else os.path.join(r, "out", "catalog")))
