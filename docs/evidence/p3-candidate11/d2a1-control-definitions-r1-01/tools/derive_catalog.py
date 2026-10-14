#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-CONTROL-DEFINITIONS-R1-01, mission sections 5, 6, 7 and 8: derive the candidate catalog
(read-only with respect to the repository).

    derive_catalog.py ROOT [OUTDIR]

Reads only ROOT/inputs/ (written and hashed by authenticate.py), tools/architect_definitions.py and git objects of
Candidate C. Writes OUTDIR (default ROOT/out/catalog): catalog.json, CATALOG.md, PATCH-IDENTITIES.md and
patches/<id>.patch for all 35 kill-required controls. Deterministic: no clock, no host path, sorted keys; two runs
give identical bytes.

  - The 14 controls READY in the blocked catalog (d2a1-controls-v1, sha256 a0cdef79...) are carried forward: their
    patch is the blocked packet's patch file byte for byte (mission 5), re-authenticated; their anchor count, mutant
    blob and mutant sha256 are recomputed here and must equal the blocked record.
  - The 21 controls BLOCKED there take the Architect's section-6 definition, encoded in architect_definitions.py; their
    patch is generated from the resulting bytes (git diff --no-index, neutral configuration).
Every one of the 35 is CANDIDATE_READY, Architect review pending, execution NOT_ATTEMPTED. A4 is the accepted
comparison, separate and not a mutation. Nothing is applied to any worktree; Candidate C is read, never written.
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
from architect_definitions import DEFS, ORDER  # noqa: E402

REPO = "/home/nexus/NEXUS/nexus-os"
MISSION = "P3-C11-DR01-D2A1-CONTROL-DEFINITIONS-R1-01"
VERSION = "d2a1-controls-v1-r1-candidate"
C = "d5964ee5326c2733177307db6f507328023d361a"
C_TREE = "51bac5fd335999cc995c0ab9f582a30244973ef0"
PROTOCOL = "crates/nexus-governed-control/src/ledger/protocol.rs"
MODEL = "crates/nexus-governed-control/src/ledger/protocol/model_tests.rs"
UNIT = "crates/nexus-governed-control/src/ledger/protocol/unit_tests.rs"
FILES = {PROTOCOL: "C-protocol.rs", MODEL: "C-protocol-model_tests.rs", UNIT: "C-protocol-unit_tests.rs"}
BLOBS = {PROTOCOL: "a2df2841b99cad5d452008a6d4150e9369d07e66", MODEL: "28085ce232827301104b5d757af042ec933976f8",
         UNIT: "55dc66409eaccdaa93e8f0a56f2ccb14be81e114"}
SHA = {"contract": "cac6535115fb333e68e6c3c045e444342c950ab7a735b7b7deea21cb2cb53b3c",
       "ad1r4": "00ced6d535c78f34f9e92233b742126a7b19192bf3ac8348056be2665192b693",
       "ad1r5": "4e1ed8c92dbb2f3706eb524fa8b4fae766f027e7027f54049331ab312bb91473",
       "ad1r5_adoption": "9990d2565a5ec96e0db03be832e96f64f774e662f6131cb69240c9a435757aa4",
       "controls_defs": "ef0420a1a69a756d4e71b3a746e39f7e29933643c121645f44dce32c7e4a108f",
       "blocked_catalog": "a0cdef7976bbb1715a7da25ad2900c978a32c6b3d9029b46f16872eda66b4747"}
BLOCKED_COMMIT = "59df21036f7c0183fa17d76b6e0886f8f5784685"
CARRIED = ["C1", "C2", "C4", "C9", "C10", "C12", "C14", "C19", "C21", "C22", "C24", "C25", "C27", "C28"]
C22_COMMAND = "cargo test -p nexus-governed-control --locked --release --lib --no-run -j 2"
DROP = ("executor_status", "blocked_reason", "executor_observations", "tranche1", "tranche1_order",
        "execution_status", "anchor", "replacement", "patch", "patch_file", "patch_sha256", "mutant_blob",
        "mutant_sha256", "patch_index_line", "patch_index_matches", "patch_lines_removed_added",
        "actual_anchor_count_at_C", "anchor_first_line_at_C", "expected_anchor_count", "mutation_source_file",
        "baseline_blob", "patch_origin", "definition_origin", "documentary_patch_applies_to_C",
        "documentary_patch_result_equals_anchor_replacement", "expected_changed_blob_count")


def sha(b):
    return hashlib.sha256(b).hexdigest()


def blob_id(data):
    return subprocess.run(["git", "hash-object", "--stdin"], input=data, capture_output=True,
                          check=True).stdout.decode().strip()


def lines_of(text, needle):
    out, start = [], 0
    while True:
        i = text.find(needle, start)
        if i < 0:
            return out
        out.append(text.count("\n", 0, i) + 1)
        start = i + 1


def make_patch(path, base, mutant):
    """A git-format unified diff of one file (git diff --no-index in a scratch directory, every configuration
    source neutralised, so the bytes depend only on the two inputs)."""
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


def main(root, outdir):
    inp = lambda n: open(os.path.join(root, "inputs", n), "rb").read()  # noqa: E731
    blocked_raw = inp("blocked/catalog.json")
    if sha(blocked_raw) != SHA["blocked_catalog"]:
        raise SystemExit("the blocked catalog is not the authenticated one")
    for key, name in (("contract", "CONTRACT-02-R1.md"), ("ad1r4", "AD1-R4.md"), ("ad1r5", "AD1-R5.md"),
                      ("controls_defs", "controls_defs.py.txt"), ("ad1r5_adoption", "R5-OWNER-ADOPTION.md")):
        if sha(inp(name)) != SHA[key]:
            raise SystemExit(f"input {name} is not the authenticated one")
    src = {p: inp(n) for p, n in FILES.items()}
    for p, data in src.items():
        if blob_id(data) != BLOBS[p]:
            raise SystemExit(f"{p} is not C's blob")
    blocked = json.loads(blocked_raw)
    bby = {e["id"]: e for e in blocked["controls"]}
    if sorted(blocked["ready"]) != sorted(CARRIED) or sorted(blocked["blocked"]) != sorted(ORDER):
        raise SystemExit("the blocked catalog's READY/BLOCKED sets are not the mission's 14 and 21")
    os.makedirs(os.path.join(outdir, "patches"), exist_ok=True)
    controls = []
    for b in blocked["controls"]:
        cid = b["id"]
        e = {k: copy.deepcopy(v) for k, v in b.items() if k not in DROP}
        e["architect_review_status"] = "pending"
        if cid == "A4":
            e["status"] = "SATISFIED — ARCHITECT ACCEPTED"
            e["execution_status"] = "NOT_RERUN (the accepted Stage-1 comparison receipts/50-A4)"
            e["not_a_mutation"] = True
            e["counted_as_kill_required"] = False
            controls.append(e)
            continue
        e["definition_status"] = "CANDIDATE_READY"
        e["execution_status"] = "NOT_ATTEMPTED"
        st = e["restored_receipt"].get("status", "")
        if st.endswith("NOT PRODUCED: the catalog is blocked"):
            # The blocked catalog's wording, true there, is stale here: restate it without the blocked mission's
            # section numbers. Only this sentence changes; the requirement is the same.
            what = st.split(" on the pinned baseline executable")[0]
            e["restored_receipt"]["status"] = (what + " on the pinned baseline executable; NOT PRODUCED: no "
                                               "execution is authorised in " + MISSION)
        if cid in CARRIED:
            d = carried(cid, b, src, root)
        else:
            d = architect(cid, b, src)
        e.update(d)
        with open(os.path.join(outdir, "patches", f"{cid}.patch"), "wb") as fh:
            fh.write(e["patch"].encode())
        e["restoration"] = {
            "method": "reverse application of patches/" + cid + ".patch to the mutant bytes, in a disposable copy",
            "restores_blob": e["baseline_blob"], "restores_file": e["mutation_source_file"],
            "restored_side_receipt_type": e["restored_receipt"].get("type"),
            "restored_side": restored_side(cid, e)}
        if cid == "C7":
            e["additional_comparison"] = {
                "required_to_kill": False,
                "rule": "AD-1-R4 F (line 748): C7 without A4 — C7's mutant build run by full_f3 (the original "
                        "explorer, configuration F-3, A4 off), under 30,000,000; expected to fail E11; added to the "
                        "receipts; not required to kill; its unchanged-core side may cite the accepted exact-final "
                        "F-3 composed-quotient PASS under AD-1-R4's prerequisites",
                "patch": "the same patches/C7.patch in both executions (mission 6.6)",
                "test": "ledger::protocol::model_tests::full_f3", "configuration": "F-3"}
        if cid == "C22":
            rr = e["restored_receipt"]
            e["restored_receipt"] = {
                "type": "e4",
                "restored_receipt_status": "PENDING_EXACT_COMMAND_HEALTHY_BUILD",
                "ruling": "mission section 7: the Stage-1 healthy build does NOT satisfy C22's final type (e4) "
                          "restored receipt because its command omitted -j 2",
                "required_future_healthy_command": C22_COMMAND,
                "required_future_mutant_command": C22_COMMAND,
                "identical_between_healthy_and_mutant": ["command", "toolchain", "environment", "target policy",
                                                         "features/cfg/RUSTFLAGS",
                                                         "supervision/resource limits"],
                "intended_difference": "source identity only (Candidate C's protocol.rs versus the C22 mutant)",
                "not_run_in_this_mission": True,
                "historical_stage1_build_not_the_e4_receipt": {
                    k: rr[k] for k in ("commit", "directory", "receipt_blob", "pin_blob", "pin_executable_sha256",
                                       "receipt_first_line") if k in rr},
            }
            e["restoration"]["restored_side"] = ("PENDING_EXACT_COMMAND_HEALTHY_BUILD: a future healthy build of "
                                                 "Candidate C by exactly `" + C22_COMMAND + "`")
            e["restoration"]["restored_side_receipt_type"] = "e4"
        controls.append(e)
    kill = [c for c in controls if c["id"] != "A4"]
    ids = [c["id"] for c in controls]
    catalog = {
        "name": "D2A-1 control catalog (contract 7.5) — candidate",
        "version": VERSION,
        "mission": MISSION,
        "catalog_status": "CANDIDATE — INDEPENDENT ARCHITECT REVIEW PENDING",
        "acceptance": "belongs to the Sole Architect after review; this file claims none",
        "supersedes_as_candidate": {"version": blocked["version"], "sha256": SHA["blocked_catalog"],
                                    "evidence_commit": BLOCKED_COMMIT,
                                    "status": "immutable review history (CONTROL_CATALOG_BLOCKED); neither "
                                              "authoritative nor edited"},
        "namespace": blocked["namespace"],
        "baseline": {"commit": C, "tree": C_TREE, "blobs": BLOBS},
        "sources": {
            "contract": blocked["sources"]["contract"],
            "ad1r4": {"sha256": SHA["ad1r4"], "status": "Owner-adopted, governing"},
            "ad1r5": {"sha256": SHA["ad1r5"], "status": "Owner-adopted in full; supplements AD-1-R4 for F-R and "
                      "C15's restored side only", "adoption_record_sha256": SHA["ad1r5_adoption"]},
            "controls_defs": {"sha256": SHA["controls_defs"], "status": "historical partial definitions; the "
                              "source of 10 of the 14 carried-forward patches"},
            "documentary_patches": blocked["sources"]["documentary_patches"],
            "blocked_catalog": {"sha256": SHA["blocked_catalog"], "use": "the 14 carried-forward patches, byte for "
                                "byte, and every non-definition field of all 36 entries"},
            "architect_definitions": {"mission": MISSION, "section": "6 (6.1-6.21)", "status": "ARCHITECT-SPECIFIED "
                                      "— EXECUTOR-ENCODED — INDEPENDENT ARCHITECT REVIEW PENDING"},
        },
        "counts": {"kill_required": len(kill), "comparison": len(controls) - len(kill),
                   "candidate_ready": sum(1 for c in kill if c["definition_status"] == "CANDIDATE_READY"),
                   "blocked": sum(1 for c in kill if c["definition_status"] != "CANDIDATE_READY"),
                   "carried_forward": len(CARRIED), "architect_defined": len(ORDER)},
        "ids_in_contract_order": ids,
        "carried_forward": CARRIED,
        "architect_defined": ORDER,
        "controls": controls,
    }
    with open(os.path.join(outdir, "catalog.json"), "w") as fh:
        json.dump(catalog, fh, indent=1, sort_keys=True, ensure_ascii=False)
        fh.write("\n")
    with open(os.path.join(outdir, "CATALOG.md"), "w") as fh:
        fh.write(render(catalog))
    with open(os.path.join(outdir, "PATCH-IDENTITIES.md"), "w") as fh:
        fh.write(identities(catalog))
    print(f"catalog {VERSION}: {len(controls)} entries ({len(kill)} kill-required + {len(controls) - len(kill)} "
          f"comparison); CANDIDATE_READY {catalog['counts']['candidate_ready']}; blocked {catalog['counts']['blocked']}")
    print(f"catalog.json sha256 {sha(open(os.path.join(outdir, 'catalog.json'), 'rb').read())}")
    return 0


def carried(cid, b, src, root):
    """A carried-forward definition: the blocked patch file's bytes, unchanged; identities recomputed."""
    patch = open(os.path.join(root, "inputs", "blocked", "patches", f"{cid}.patch"), "rb").read()
    if sha(patch) != b["patch_sha256"] or patch != b["patch"].encode():
        raise SystemExit(f"{cid}: carried patch is not the blocked one")
    path = b["mutation_source_file"]
    base_t = src[path].decode()
    mutant_t, spans = apply_edits(base_t, [{"anchor": b["anchor"], "replacement": b["replacement"]}])
    mutant = mutant_t.encode()
    mblob = blob_id(mutant)
    if mblob != b["mutant_blob"] or sha(mutant) != b["mutant_sha256"]:
        raise SystemExit(f"{cid}: recomputed mutant differs from the blocked record")
    return {"definition_source": "CARRIED_FORWARD",
            "definition_origin": b["definition_origin"] + [
                f"carried forward byte for byte from the blocked catalog d2a1-controls-v1 (sha256 "
                f"{SHA['blocked_catalog']}, evidence commit {BLOCKED_COMMIT}); re-authenticated (mission 5); "
                f"READY there; its Architect review is pending here"],
            "patch_origin": b["patch_origin"] + "; carried forward unchanged",
            "mutation_source_file": path, "baseline_blob": BLOBS[path], "expected_changed_blob_count": 1,
            "edits": [{"anchor": b["anchor"], "replacement": b["replacement"], "expected_count": 1,
                       "anchor_first_line_at_C": lines_of(base_t, b["anchor"])[0]}],
            "anchor": b["anchor"], "replacement": b["replacement"],
            "patch": patch.decode(), "patch_file": f"patches/{cid}.patch", "patch_sha256": sha(patch),
            "mutant_blob": mblob, "mutant_sha256": sha(mutant),
            "patch_lines_removed_added": removed_added(patch),
            "architect_item": None, "syntax_check": "not required by the mission for carried-forward controls"}


def architect(cid, b, src):
    d = DEFS[cid]
    for k, v in (("configuration", d["configuration"]), ("named_property", d["named_property"]),
                 ("named_property_text", d["named_property_text"])):
        if b[k] != v:
            raise SystemExit(f"{cid}: section 6 {k} {v!r} differs from the contract row's {b[k]!r}")
    path = d["file"]
    base = src[path]
    base_t = base.decode()
    mutant_t, spans = apply_edits(base_t, d["edits"])
    mutant = mutant_t.encode()
    patch = make_patch(path, base, mutant)
    m = re.search(rb"^index ([0-9a-f]+)\.\.([0-9a-f]+)", patch, re.M)
    mblob = blob_id(mutant)
    if not (m and m.group(1).decode() == BLOBS[path] and m.group(2).decode() == mblob):
        raise SystemExit(f"{cid}: patch index line does not name C's blob and the mutant's")
    return {"definition_source": "ARCHITECT_DEFINED",
            "definition_origin": [f"mission {MISSION} section {d['mission_section']} ({d['title']}): "
                                  "ARCHITECT-SPECIFIED — EXECUTOR-ENCODED — INDEPENDENT ARCHITECT REVIEW PENDING"],
            "patch_origin": "generated by derive_catalog.py from the encoded edits (git diff --no-index, neutral "
                            "configuration)",
            "mutation_source_file": path, "baseline_blob": BLOBS[path], "expected_changed_blob_count": 1,
            "edits": [{"anchor": e["anchor"], "replacement": e["replacement"], "expected_count": 1,
                       "anchor_first_line_at_C": base_t.count("\n", 0, s[0]) + 1}
                      for e, s in zip(d["edits"], spans)],
            "anchor": d["edits"][0]["anchor"] if len(d["edits"]) == 1 else None,
            "replacement": d["edits"][0]["replacement"] if len(d["edits"]) == 1 else None,
            "patch": patch.decode(), "patch_file": f"patches/{cid}.patch", "patch_sha256": sha(patch),
            "mutant_blob": mblob, "mutant_sha256": sha(mutant),
            "patch_lines_removed_added": removed_added(patch),
            "architect_item": d["item"], "architect_section": d["mission_section"],
            "preserved_behaviour": d["preserved"], "one_named_change": d["one_change"],
            "execution_dependent_uncertainty": d["uncertain"],
            "syntax_check": "SYNTAX_CHECKED_NOT_TYPECHECKED (rustfmt --check on the patched disposable file; "
                            "receipts/30-syntax-check.txt)"}


def restored_side(cid, e):
    rr = e["restored_receipt"]
    if cid == "C15":
        return ("AD-1-R5 F: the adopted F-R proof discharge record, citable only after verified restoration to "
                "Candidate C; the mutant failing receipt stays the original full_fr, 30,000,000")
    if "directory" in rr:
        return f"{rr['type']}: {rr['directory']} at {rr['commit']}"
    return f"{rr.get('type')}: {rr.get('status', '')}".strip()


def render(cat):
    out = [f"# {cat['name']} — `{cat['version']}`", "",
           f"Status: **{cat['catalog_status']}**. Acceptance {cat['acceptance']}.", "",
           f"Baseline: Candidate C `{cat['baseline']['commit']}` (tree `{cat['baseline']['tree']}`).", "",
           f"Kill-required: {cat['counts']['kill_required']} (definition CANDIDATE_READY: "
           f"{cat['counts']['candidate_ready']}; carried forward {cat['counts']['carried_forward']}, Architect-defined "
           f"{cat['counts']['architect_defined']}). Separate comparison: {cat['counts']['comparison']} (A4).", "",
           "| Id | Change (contract 7.5) | Named | On / test | Receipt type | Definition | Source | Review | "
           "Execution |", "|---|---|---|---|---|---|---|---|---|"]
    for c in cat["controls"]:
        named = c.get("named_property")
        named = " / ".join(named) if isinstance(named, list) else named
        on = c.get("test") or c.get("configuration")
        out.append(f"| {c['id']} | {c['description']} | {named} | `{on}` | {c['receipt_type'].split(';')[0]} | "
                   f"{c.get('definition_status', c.get('status'))} | {c.get('definition_source', 'comparison')} | "
                   f"{c['architect_review_status']} | {c['execution_status'].split(' (')[0]} |")
    out += ["", "The machine-readable record — edits, patches, identities, receipt and restoration mapping — is "
            "`catalog.json`; `PATCH-IDENTITIES.md` lists every patch and mutant identity.", ""]
    return "\n".join(out)


def identities(cat):
    out = ["# Patch identity table — candidate catalog `" + cat["version"] + "`", "",
           "Every patch applies to Candidate C (`" + cat["baseline"]["commit"] + "`) and changes one file. Mutant "
           "blob: `git hash-object` of the patched bytes. None of these patches has been applied to any worktree; no "
           "mutant has been built or run.", "",
           "| Id | Source | File | Item | Edits | -/+ lines | Patch sha256 | Mutant blob | Mutant sha256 |",
           "|---|---|---|---|---|---|---|---|---|"]
    for c in cat["controls"]:
        if c["id"] == "A4":
            continue
        f = c["mutation_source_file"].rsplit("/", 1)[1]
        out.append(f"| {c['id']} | {c['definition_source']} | `{f}` | {c.get('architect_item') or '—'} | "
                   f"{len(c['edits'])} | {c['patch_lines_removed_added'][0]}/{c['patch_lines_removed_added'][1]} | "
                   f"`{c['patch_sha256']}` | `{c['mutant_blob']}` | `{c['mutant_sha256']}` |")
    out.append("")
    return "\n".join(out)


if __name__ == "__main__":
    r = sys.argv[1]
    sys.exit(main(r, sys.argv[2] if len(sys.argv) > 2 else os.path.join(r, "out", "catalog")))
