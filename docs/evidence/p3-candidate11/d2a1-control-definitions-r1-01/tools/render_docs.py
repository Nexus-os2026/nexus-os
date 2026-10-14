#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-CONTROL-DEFINITIONS-R1-01, mission sections 7, 11 and 13: render the packet's documents from
the catalog, the receipts and section 6 as received (read-only with respect to the repository).

    render_docs.py ROOT

Writes ROOT/out/docs/: ARCHITECT-CONTROL-DEFINITIONS-R1.md, C22-RESTORED-BUILD-RULING.md and catalog-README.md.
Deterministic: every value comes from catalog.json, the receipts or tools/section6.txt.
"""
import hashlib
import json
import os
import re
import subprocess
import sys

REPO = "/home/nexus/NEXUS/nexus-os"
STAGE1 = "f023cc3d031a2a89e5ecbd4086e15f2e50869acd"
LABEL = "ARCHITECT-SPECIFIED — EXECUTOR-ENCODED — INDEPENDENT ARCHITECT REVIEW PENDING"
ORDER = ["C3a", "C3b", "C3c", "C5", "C6", "C7", "C8", "C11", "C15", "C16", "C17", "C18a", "C18b", "C20", "C23",
         "C26", "A1", "A2", "A3", "A5", "A6"]


def sha(b):
    return hashlib.sha256(b).hexdigest()


def section6(root):
    text = open(os.path.join(root, "tools", "section6.txt")).read()
    parts = re.split(r"\n-{60}\n(6\.\d+ (?:C\d+[a-c]?|A\d) — [^\n]+)\n-{60}\n", text)
    out = {}
    for i in range(1, len(parts), 2):
        head, body = parts[i], parts[i + 1]
        cid = head.split()[1]
        out[cid] = (head, body.strip("\n"))
    return out


def verdict(path, cid):
    for l in open(path).read().splitlines():
        if l.startswith("VERDICTS "):
            return json.loads(l[9:]).get(cid)
    return None


def fence(text, lang=""):
    n = 3
    while "`" * n in text:
        n += 1
    return f"{'`' * n}{lang}\n{text}{'' if text.endswith(chr(10)) else chr(10)}{'`' * n}"


def main(root):
    cat = json.load(open(os.path.join(root, "out", "catalog", "catalog.json")))
    catsha = sha(open(os.path.join(root, "out", "catalog", "catalog.json"), "rb").read())
    by = {e["id"]: e for e in cat["controls"]}
    s6 = section6(root)
    rec = os.path.join(root, "receipts")
    syn = open(os.path.join(rec, "30-syntax-check.txt")).read()
    outd = os.path.join(root, "out", "docs")
    os.makedirs(outd, exist_ok=True)
    o = [f"# Architect control definitions R1 — the 21 controls of mission section 6", "",
         f"**{LABEL}**", "",
         "Mission P3-C11-DR01-D2A1-CONTROL-DEFINITIONS-R1-01. Executor: Claude Code, Claude Opus 5.5 "
         "(`claude-opus-5-5`), effort High. Static definition work only: no Cargo, no rustc, no build, no test, no "
         "mutant executed, no explorer run. Candidate C was read and never written.", "",
         f"These 21 definitions are the Sole Architect's (mission section 6), encoded by the executor as exact edits "
         f"of Candidate C `{cat['baseline']['commit']}`. Each is one control of the candidate catalog "
         f"`{cat['version']}` (`catalog/catalog.json`, sha256 `{catsha}`). None is accepted: each definition's "
         "Architect review is pending, and no claim is made that any control will be killed.", "",
         "Each section gives: the adopted contract description (contract 7.5's row, verbatim); the Architect's "
         "encoding (section 6, verbatim as received in the mission text); the exact source item; the exact baseline "
         "anchor(s) and replacement(s); the patch and mutant identities; the named property, test and "
         "configuration; the receipt type; the behaviour preserved; why the encoding is one named change; and the "
         "uncertainty that only execution can remove.", "",
         "Static evidence for every one of the 21 (receipts): checker A and checker B both PASS "
         "(`receipts/20-checker-a.txt`, `receipts/21-checker-b.txt`; identical per-id verdicts); `rustfmt --check` "
         "on the patched disposable file passes: **SYNTAX_CHECKED_NOT_TYPECHECKED** (`receipts/30-syntax-check.txt`). "
         "Nothing was type-checked or compiled.", "",
         "Common to all 21: the mutant failing receipt is type (a), the original explorer (`explore`) through the "
         "configuration's `full_*` test with `--exact --ignored`, on the pinned mutant executable, under the "
         "configuration's ceiling (F-1: 60,000,000; every other: 30,000,000); a quotient run is never a failing "
         "receipt (AD-1-R4 F, F.1). The restored side is type (e1) on the unchanged core, except C15 (AD-1-R5 F).",
         "", "## Summary", "",
         "| Id | § | File | Item | Property | Configuration | Edits | Patch sha256 | Mutant blob |",
         "|---|---|---|---|---|---|---|---|---|"]
    for cid in ORDER:
        e = by[cid]
        o.append(f"| {cid} | {e['architect_section']} | `{e['mutation_source_file'].rsplit('/', 1)[1]}` | "
                 f"{e['architect_item']} | {e['named_property_text']} | {e['configuration']} | {len(e['edits'])} | "
                 f"`{e['patch_sha256'][:16]}…` | `{e['mutant_blob'][:12]}…` |")
    o.append("")
    for cid in ORDER:
        e = by[cid]
        head, body = s6[cid]
        a = verdict(os.path.join(rec, "20-checker-a.txt"), cid)
        b = verdict(os.path.join(rec, "21-checker-b.txt"), cid)
        s = re.search(rf"^(OK  |FAIL) {re.escape(cid)}: .* -> (\S+)$", syn, re.M)
        rr = e["restored_receipt"]
        o += [f"## {cid} — {e['description']} (section {e['architect_section']})", "", f"*{LABEL}*", "",
              f"**Adopted contract description** (contract 7.5, line {e['contract_line']}):", "",
              fence(e["contract_row"] + "\n"), "",
              f"**Named property / test / configuration.** {e['named_property_text']} on **{e['configuration']}**: "
              f"`{e['test']}` (ignored at C: {e['ignored_at_C']}), the original explorer, ceiling "
              f"{e['ceiling_states']:,} states. Printed property must be: {e['expected_primary_failure']['printed_property_must_be']}.",
              "",
              f"**Receipt type.** {e['receipt_type']}. Restored side: {e['restoration']['restored_side']}.", ""]
        if e.get("special_attribution_rules"):
            o += ["**Special rules.** " + " ".join(f"({i + 1}) {r}." for i, r in
                                                    enumerate(e["special_attribution_rules"])), ""]
        if cid == "C7":
            ac = e["additional_comparison"]
            o += [f"**Additional comparison (not required to kill).** {ac['rule']}. Patch: {ac['patch']}.", ""]
        o += [f"**Exact source item.** `{e['mutation_source_file']}` (blob `{e['baseline_blob']}`): "
              f"{e['architect_item']}; anchor line(s) at C: "
              f"{', '.join(str(x['anchor_first_line_at_C']) for x in e['edits'])}.", "",
              f"**Architect encoding — mission section {head}** (verbatim as received):", "", fence(body + "\n"), ""]
        for k, x in enumerate(e["edits"]):
            tag = f" {k + 1} of {len(e['edits'])}" if len(e["edits"]) > 1 else ""
            o += [f"**Baseline anchor{tag}** (exactly once in C's blob, at line {x['anchor_first_line_at_C']}):", "",
                  fence(x["anchor"], "rust"), "", f"**Replacement{tag}:**", "",
                  fence(x["replacement"], "rust") if x["replacement"] else "*(empty: the anchor is removed)*", ""]
        o += ["**Identities.**", "",
              f"- patch `catalog/{e['patch_file']}`: sha256 `{e['patch_sha256']}`; lines removed/added "
              f"{e['patch_lines_removed_added'][0]}/{e['patch_lines_removed_added'][1]}",
              f"- mutant `{e['mutation_source_file'].rsplit('/', 1)[1]}`: git blob `{e['mutant_blob']}`, sha256 "
              f"`{e['mutant_sha256']}`",
              f"- restoration: reverse application returns blob `{e['restoration']['restores_blob']}` (verified by "
              "both checkers)", "",
              "**Preserved behaviour.**", ""] + [f"- {p}" for p in e["preserved_behaviour"]] + [
              "", f"**Why this is one named change.** {e['one_named_change']}", "",
              f"**Static validation.** checker A: {a}; checker B: {b}; syntax: "
              f"{s.group(2) if s else 'NOT FOUND'}.", "",
              "**Remaining execution-dependent uncertainty.**", ""] + \
            [f"- {u}" for u in e["execution_dependent_uncertainty"]] + [""]
    open(os.path.join(outd, "ARCHITECT-CONTROL-DEFINITIONS-R1.md"), "w").write("\n".join(o))
    c22 = by["C22"]
    rr = c22["restored_receipt"]
    rec1 = subprocess.run(["git", "-C", REPO, "show", f"{STAGE1}:" + rr["historical_stage1_build_not_the_e4_receipt"]
                           ["directory"] + "/RECEIPT.txt"], capture_output=True, check=True).stdout.decode()
    cmd = [l.strip() for l in rec1.splitlines() if "cargo test -p nexus-governed-control" in l]
    h = rr["historical_stage1_build_not_the_e4_receipt"]
    c = ["# C22 — restored-build ruling record (mission section 7)", "",
         "Mission P3-C11-DR01-D2A1-CONTROL-DEFINITIONS-R1-01. Executor: Claude Code, Claude Opus 5.5. A record of the "
         "Architect's ruling as applied to the candidate catalog. Nothing was built.", "",
         "## The ruling (mission section 7, as received)", "",
         "> The Stage-1 healthy build does NOT satisfy C22's final type-(e4) restored receipt because its command "
         "omitted `-j 2`.", "",
         "## The Stage-1 healthy build, as recorded", "",
         f"- receipt: `{h['directory']}/RECEIPT.txt` at Stage-1 evidence commit `{h['commit']}` (blob "
         f"`{h.get('receipt_blob')}`); pin `PIN.json` blob `{h.get('pin_blob')}`, pinned executable sha256 "
         f"`{h.get('pin_executable_sha256')}`",
         "- its recorded command line(s), read from that receipt now:", ""] + [f"  - `{x}`" for x in cmd] + [
         "", "No `-j 2` appears in it (both static checkers re-read the receipt and check this).", "",
         "## The candidate catalog's record", "",
         fence(json.dumps({k: v for k, v in rr.items() if k != "historical_stage1_build_not_the_e4_receipt"},
                          indent=1, sort_keys=True, ensure_ascii=False) + "\n", "json"), "",
         "The Stage-1 build stays in the catalog only under `historical_stage1_build_not_the_e4_receipt`; it is not "
         "C22's restored receipt.", "",
         "## What a future mission must produce (not done here)", "",
         f"- a healthy build of Candidate C by exactly `{rr['required_future_healthy_command']}`;",
         f"- C22's mutant build by exactly `{rr['required_future_mutant_command']}`;",
         "- identical between the two: " + "; ".join(rr["identical_between_healthy_and_mutant"]) + ";",
         f"- the only intended difference: {rr['intended_difference']}.", "",
         "C22's mutant failing receipt stays type (d): the intentional compile failure, `error[E0283]` at "
         f"{c22['expected_primary_failure']['location']} (contract 7.5 line 731; AD-1-R4 F.1(d)).", "",
         "Status: **PENDING_EXACT_COMMAND_HEALTHY_BUILD**. Neither build ran in this mission.", ""]
    open(os.path.join(outd, "C22-RESTORED-BUILD-RULING.md"), "w").write("\n".join(c))
    r = ["# catalog/ — candidate catalog `" + cat["version"] + "`", "",
         f"Status: {cat['catalog_status']}. Acceptance {cat['acceptance']}.", "",
         "- `catalog.json` — the machine-readable catalog: 36 entries in contract 7.5's order (35 kill-required "
         "controls and A4, the separate accepted comparison). Each control carries its edits (anchor, replacement, "
         "expected count 1, line at C), patch text and sha256, mutant blob and sha256, named property, test, "
         "configuration, ceiling, receipt type, restored-receipt link and restoration mapping, and its definition, "
         "review and execution status.",
         "- `CATALOG.md` — one line per entry.",
         "- `PATCH-IDENTITIES.md` — every patch and mutant identity.",
         "- `patches/<id>.patch` — 35 documentary patches against Candidate C. 14 are the blocked catalog's "
         "patches, byte for byte; 21 encode the Architect's section-6 definitions. None has been applied to any "
         "worktree, built or run.", "",
         "Every control: definition CANDIDATE_READY, Architect review pending, execution NOT_ATTEMPTED.", ""]
    open(os.path.join(outd, "catalog-README.md"), "w").write("\n".join(r))
    for n in sorted(os.listdir(outd)):
        print(f"{n} sha256 {sha(open(os.path.join(outd, n), 'rb').read())}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1]))
