#!/usr/bin/env python3
"""Renders CONTROL-CATALOG-BLOCKER.md and TRANCHE-SUMMARY.md from ROOT/out/catalog/catalog.json (deterministic).

    render_docs.py ROOT
"""
import hashlib
import json
import os
import sys

TRANCHE_GROUPS = [("Intentional compile failure", ["C22"]),
                  ("Named non-explorer unit tests", ["C14", "C19", "C21", "C27", "C28"]),
                  ("Original explorer", ["C12", "C9", "C11", "C17", "C18a", "C18b"])]


def main(root):
    cpath = os.path.join(root, "out", "catalog", "catalog.json")
    raw = open(cpath, "rb").read()
    cat = json.loads(raw)
    digest = hashlib.sha256(raw).hexdigest()
    by = {e["id"]: e for e in cat["controls"]}
    kill = [e for e in cat["controls"] if e["id"] != "A4"]
    ready = [e for e in kill if e["executor_status"] == "READY"]
    blocked = [e for e in kill if e["executor_status"] == "BLOCKED"]
    out = os.path.join(root, "out", "packet")
    os.makedirs(out, exist_ok=True)
    b = ["# D2A-1 control catalog — CONTROL_CATALOG_BLOCKED", "",
         "Mission P3-C11-DR01-D2A1-MUTATION-TRANCHE1-01. Executor: Claude Code, Claude Opus 5.5 (`claude-opus-5-5`), "
         "effort High. For the Sole Architect's review. **Evidence only.**", "",
         f"Catalog `catalog/catalog.json`, version `{cat['version']}`, sha256 `{digest}`: "
         f"**{cat['catalog_status']}**, **not frozen**. Kill-required controls: {cat['counts']['kill_required']}; "
         f"A4 recorded separately as the accepted comparison. Definitions READY: {cat['counts']['ready']}. "
         f"Definitions BLOCKED: {cat['counts']['blocked']}.", "",
         "## Why the catalog is blocked", "",
         "Mission section 7: \"Stop if any of the 35 definitions remains ambiguous. Return CONTROL_CATALOG_BLOCKED "
         "rather than inventing a definition.\" Section 8: no Rust build is authorised before the static catalog gate "
         "passes, and the gate freezes all 35 patches.", "",
         "The authorised definition sources were each authenticated (`receipts/00-authentication.txt`). Between them "
         "they give an exact change — an anchor and its replacement, or a patch — for 14 of the 35 controls only:", "",
         "| Source | Exact definitions it gives |", "|---|---|",
         "| Contract P3-C11-D2A1-CONTRACT-02-R1, 7.5 (`cac65351…3b3c`) | none: one line of prose per control, its "
         "named property and its configuration |",
         "| AD-1-R4 G.2 (`00ced6d5…b693`), with the documentary patches it cites | C1, C25 (narrow), C4, C9 (C9-b); "
         "C10 and C24 retained verbatim from the sealed file |",
         "| AD-1-R5 (`4e1ed8c9…1bb91473`) | none: C15's explorer, property, ceiling and restored side, not its text |",
         "| Sealed historical `controls_defs.py` (`ef0420a1…a108f`) | C1, C2, C10, C24, C12, C14, C19, C21, C22, "
         "C27, C28; its compound C25 is superseded by G.2's narrow C25. Its own sealed README says: \"controls_defs.py "
         "holds only a partial set of definitions\" |", "",
         f"The other {len(blocked)} controls — {', '.join(e['id'] for e in blocked)} — have no exact definition in "
         "any authorised source. Writing one would be the executor authoring a kill-required control's mutation, "
         "which the mission forbids. Four of them are tranche-1 controls: "
         f"{', '.join(c for c in cat['tranche1_execution_order'] if c in cat['blocked'])}.", "",
         "Because the gate cannot pass, nothing was built or run: no baseline exact receipt (mission 10), no mutant "
         "worktree, no mutant build, no mutant test. This includes the eight tranche-1 controls whose definitions "
         "are READY (C9, C12, C14, C19, C21, C22, C27, C28): mission 8 makes the gate a precondition of any build, "
         "and mission 19 says that if the catalog is blocked no baseline test and no mutant is executed.", "",
         "## The 21 blocked controls", "",
         "Each row: the contract's change, its named property or test and its configuration, and the executor's "
         "observations — where the change would land in Candidate C, and what an exact definition must settle. The "
         "observations are **not definitions**, carry no patch, and are not Architect-reviewed.", ""]
    for e in blocked:
        prop = " / ".join(e["named_property"]) if isinstance(e["named_property"], list) else e["named_property"]
        b += [f"### {e['id']} — {e['description']}", "",
              f"- Contract 7.5 line {e['contract_line']}; fails **{e.get('named_property_text', prop)}**; on "
              f"**{e['configuration']}** (`{e['test']}`, ceiling {e.get('ceiling_states'):,} states)."
              if e.get("ceiling_states") else f"- Contract 7.5 line {e['contract_line']}; {prop}.",
              "- Candidate sites at C (location only, not anchors):"]
        for s in e["executor_observations"]["candidate_sites"]:
            first = s["text"].split("\n")[0].strip()
            b.append(f"  - `{s['file'].rsplit('/', 1)[1]}` line(s) {', '.join(map(str, s['lines_at_C']))} "
                     f"({s['occurrences_at_C']} occurrence(s)): `{first}`")
        b.append("- Open:")
        for q in e["executor_observations"]["open_questions"]:
            b.append(f"  - {q}")
        b.append("")
    b += ["## The 14 controls whose definitions are READY (not frozen, not applied)", "",
          "Each passed every static check of `receipts/20-static-gate.txt`: anchor once in C's blob; the patch applies "
          "with GNU patch and with `git apply` to a disposable byte copy, gives the anchor replacement, changes only "
          "the anchor's span of `protocol.rs`, and reverses to blob `a2df2841…`; patches and mutant blobs pairwise "
          "distinct; named test present at C and in the pinned executable's listing; restored-evidence links resolve.",
          "", "| Id | Origin | Anchor line at C | Patch sha256 | Mutant blob | Tranche 1 |", "|---|---|---|---|---|---|"]
    for e in ready:
        origin = e["definition_origin"][0].split(":")[0].split(" (")[0]
        b.append(f"| {e['id']} | {origin} | {e['anchor_first_line_at_C'][0]} | `{e['patch_sha256'][:16]}…` | "
                 f"`{e['mutant_blob'][:12]}…` | {e['tranche1_order'] or '—'} |")
    b += ["", "These patches are documentary copies under `catalog/patches/`. None was applied to any worktree.", "",
          "## Observations for the Architect (no action taken)", "",
          "1. **The definition gap is structural.** CAMPAIGN-01's `controls_defs.py` was a partial preparation "
          "(its README); AD-1-R4 G.2 ruled on C1, C25, C4, C9, C10 and C24 only. No later document supplied the rest.",
          "2. **Dormant model scaffolding.** `model_tests.rs` at C already carries state for two actor controls — "
          "`World.adm_kept` (\"Actor change A2 only\") and `WkPos::RaiseH` (\"Under actor change A3 only\") — but "
          "not the code that would use them. A definition for A2 and A3 still has to be written and ruled on.",
          "3. **C22's healthy-build link.** Mission 10 links Stage-1's healthy build as C22's type (e4) evidence. That "
          "build ran `cargo test -p nexus-governed-control --locked --release --lib --no-run` (no `-j`; the "
          "repository's `[build] jobs = 8`). Mission 12's mutant command adds `-j 2`. AD-1-R4 F.1(d) asks for \"a "
          "type (e4) receipt of the same command succeeding on the unmutated baseline\". The job count enters no "
          "artifact or fingerprint, but the command lines differ; whether the link satisfies F.1(d) is the "
          "Architect's reading.",
          "4. **Baseline receipts of mission 10** (X-1 alone for C12; T-BIND, T-CELL, W6, T-PREC and T-RES alone) "
          "were not produced. They remain required before C12 and the unit-test controls can be attempted.",
          "5. **C1's two encodings agree.** The documentary `C1-mutation.patch` and `controls_defs.py`'s C1 anchor "
          "give byte-identical mutants (blob `7d95dd42…`).", ""]
    open(os.path.join(out, "CONTROL-CATALOG-BLOCKER.md"), "w").write("\n".join(b))
    t = ["# Mutation tranche 1 — summary", "",
         "Mission P3-C11-DR01-D2A1-MUTATION-TRANCHE1-01. **No control was attempted. Tranche kill count: 0.** The "
         "catalog is CONTROL_CATALOG_BLOCKED (`CONTROL-CATALOG-BLOCKER.md`), so mission 8 and 19 authorise no build "
         "and no test. Nothing below is a kill, a survival or a verdict.", "",
         "Baseline exact receipts (mission 10): **not run** — X-1 alone, and the five named unit tests alone. The "
         "pinned executable `cde9f9d2…` was hashed and sized only; it was never run.", "",
         "| Order | Control | Category | Catalog status | Patch sha256 | Mutant blob | Mutant executable | Selected "
         "test | Run | Restoration |", "|---|---|---|---|---|---|---|---|---|---|"]
    cats = {c: g for g, cs in TRANCHE_GROUPS for c in cs}
    for i, cid in enumerate(cat["tranche1_execution_order"], 1):
        e = by[cid]
        ok = e["executor_status"] == "READY"
        exe = "NOT PRODUCED BY DESIGN" if cid == "C22" else "not built"
        t.append(f"| {i} | {cid} | {cats[cid]} | {e['executor_status']} | "
                 f"{'`' + e['patch_sha256'][:16] + '…`' if ok else '— (no definition)'} | "
                 f"{'`' + e['mutant_blob'][:12] + '…`' if ok else '—'} | {exe} | `{e['test'].rsplit('::', 1)[1]}` | "
                 f"NOT ATTEMPTED | not applicable (no mutant worktree was created) |")
    others = [e["id"] for e in kill if not e["tranche1"]]
    t += ["", f"Unattempted outside the tranche (not authorised by mission 16): {', '.join(others)}. A4 not rerun. "
          "M01–M183 not executed.", "",
          "Named properties and configurations of the tranche, as the catalog derives them:", ""]
    for cid in cat["tranche1_execution_order"]:
        e = by[cid]
        prop = " / ".join(e["named_property"]) if isinstance(e["named_property"], list) else e["named_property"]
        t.append(f"- {cid}: {prop} — `{e['test']}`" + (f", ceiling {e['ceiling_states']:,} states"
                                                     if e.get("ceiling_states") else ""))
    t.append("")
    open(os.path.join(out, "TRANCHE-SUMMARY.md"), "w").write("\n".join(t))
    print(f"rendered CONTROL-CATALOG-BLOCKER.md and TRANCHE-SUMMARY.md from catalog.json {digest}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1]))
