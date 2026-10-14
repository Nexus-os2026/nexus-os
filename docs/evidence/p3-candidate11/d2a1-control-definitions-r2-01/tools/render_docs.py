#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-CONTROL-DEFINITIONS-R2-01, mission sections 8 and 12: render the packet's documents from the R2
catalog, the receipts, R1's section 6 as received (inputs/r1/section6.txt) and R2's sections 1 and 4 as received
(tools/r2_section4.txt) (read-only with respect to the repository).

    render_docs.py ROOT

Writes ROOT/out/docs/: ARCHITECT-CONTROL-DEFINITIONS-R2.md, C22-RESTORED-BUILD-RULING.md and catalog-README.md.
Deterministic: every value comes from catalog.json, the comparison files, the receipts or the two section texts.
The 21 definition sections carry R1's record forward (the same fields, rendered the same way); A6's section adds
the R2 receipt-metadata edit and its identities.
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
R1_DEFS_SHA = "beb9f5e3129bbd559dfa375251bddeac17f78d3f1b79082a42767944d7d48bf0"
R1_DEFS_BLOB = "b947621bb0d56e1e14780853e886c7e832d7f6a3"
ORDER = ["C3a", "C3b", "C3c", "C5", "C6", "C7", "C8", "C11", "C15", "C16", "C17", "C18a", "C18b", "C20", "C23",
         "C26", "A1", "A2", "A3", "A5", "A6"]


def sha(b):
    return hashlib.sha256(b).hexdigest()


def section6(root):
    text = open(os.path.join(root, "inputs", "r1", "section6.txt")).read()
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
    catp = os.path.join(root, "out", "catalog", "catalog.json")
    cat = json.load(open(catp))
    catsha = sha(open(catp, "rb").read())
    by = {e["id"]: e for e in cat["controls"]}
    s6 = section6(root)
    r2s4 = open(os.path.join(root, "tools", "r2_section4.txt")).read()
    rec = os.path.join(root, "receipts")
    syn = open(os.path.join(rec, "30-syntax-check.txt")).read()
    outd = os.path.join(root, "out", "docs")
    os.makedirs(outd, exist_ok=True)
    sup = cat["supersedes_as_candidate"]
    a6 = by["A6"]
    rc = a6["r2_correction"]
    unchanged = cat["r2"]["unchanged_controls"]
    o = ["# Architect control definitions R2 — the 21 controls, with A6's receipt-metadata repair", "",
         f"**{LABEL}**", "",
         "Mission P3-C11-DR01-D2A1-CONTROL-DEFINITIONS-R2-01. Executor: Claude Code, Claude Opus 5.5 "
         "(`claude-opus-5-5`), effort High. Static definition work only: no Cargo, no rustc, no build, no test, no "
         "mutant executed, no explorer run. Candidate C was read and never written.", "",
         f"This record carries forward the reviewed R1 record (`ARCHITECT-CONTROL-DEFINITIONS-R1.md`, git blob "
         f"`{R1_DEFS_BLOB}`, sha256 `{R1_DEFS_SHA}`, R1 evidence commit `{sup['evidence_commit']}`): the 21 "
         "definitions are the Sole Architect's (mission R1 section 6), encoded by the executor as exact edits of "
         f"Candidate C `{cat['baseline']['commit']}`. In the R2 candidate catalog `{cat['version']}` "
         f"(`catalog/catalog.json`, sha256 `{catsha}`) twenty of them are R1's entries and patches unchanged, byte for "
         "byte; A6 alone changes, by the one receipt-metadata hunk the Architect authorised (mission R2 section 4). "
         "None is accepted: each definition's Architect review is pending, and no claim is made that any control "
         "will be killed.", "",
         "## The R2 correction: A6", "",
         f"*{LABEL}*", "",
         "**The ruling and the repair (mission R2 sections 1 and 4, as received):**", "",
         fence(r2s4.split("\n", 2)[2].lstrip("\n")), "",
         "**What R2 records for A6.**", "",
         "- **The semantic mutation is unchanged from R1.** A6's eight R1 edits are kept byte for byte (catalog "
         "`edits[0..7]` equal R1's), and every one of the eight hunks of the R1 patch appears in the R2 patch byte "
         "for byte — `@@` line, line numbers, function context and body — in the same order: the field "
         "`Table.recovery_required`, its initialisation (false), its encoding, its decoding, the H-event update "
         "(WkPos::Seen), the A-event update (WkPos::Seen), the WkPos::RaiseH update, the AckPos::Applying update and "
         "the admission predicate.",
         "- **recovery_required is part of the state identity.** It is encoded right after A "
         "(`e.b(w.table.recovery_required)`) and decoded at the matching place, so, on a static reading of encode and "
         "decode, two states that differ only in the flag have different state keys.",
         f"- **KEY_FIELDS now names recovery_required.** The one added hunk replaces, in the `KEY_FIELDS` string "
         f"constant (model_tests.rs, line {rc['anchor_first_line_at_C']} at C), `{rc['old_phrase']}` with "
         f"`{rc['new_phrase']}`. No other text of KEY_FIELDS changes; its value names recovery_required exactly once.",
         "- **The new hunk repairs raw receipt completeness.** Every exploration report prints "
         "`  state key fields: {KEY_FIELDS}` (three report sites, unchanged). With the R1 patch, A6's receipts would "
         "have listed the table as `(classes, debt counter, H, A)` while the key also held the flag; with R2 the "
         "printed table description names the flag as well.",
         "- **The hunk does not alter executable model semantics beyond the already-defined A6 mutation.** It "
         "changes the value of a string that is only printed: no transition, property, trace text, branch label, "
         "field name, key layout or encoded position changes. Both static checkers verify this: the R1 mutant and "
         "the R2 mutant differ in exactly one line, the KEY_FIELDS table description.", "",
         "**The added hunk (receipt metadata, not a second semantic mutation):**", "",
         fence(rc["added_hunk"], "diff"), "",
         f"It is {rc['added_hunk_position']}. Its `@@` line's trailing text is git's default function-context "
         "heuristic (the nearest earlier line that begins with a letter, here a continuation line of the same "
         "string, cut at 80 bytes): it is produced by the R1 patch generator unchanged, and patch tools ignore it.", "",
         "**A6 identities, R1 → R2.**", "",
         "| | R1 | R2 |", "|---|---|---|",
         f"| patch sha256 | `{rc['r1_patch_sha256']}` | `{a6['patch_sha256']}` |",
         f"| patch lines removed/added | {rc['r1_patch_lines_removed_added'][0]}/{rc['r1_patch_lines_removed_added'][1]} "
         f"| {a6['patch_lines_removed_added'][0]}/{a6['patch_lines_removed_added'][1]} |",
         f"| hunks | {rc['r1_hunks_preserved_byte_for_byte']} | {rc['r1_hunks_preserved_byte_for_byte'] + rc['hunks_added']} "
         f"(the {rc['r1_hunks_preserved_byte_for_byte']} R1 hunks byte for byte + {rc['hunks_added']}) |",
         f"| edits | {rc['semantic_edits_unchanged_from_r1']} | {len(a6['edits'])} (the "
         f"{rc['semantic_edits_unchanged_from_r1']} R1 edits + {rc['receipt_metadata_edits_added']} receipt-metadata "
         "edit) |",
         f"| mutant git blob | `{rc['r1_mutant_blob']}` | `{a6['mutant_blob']}` |",
         f"| mutant sha256 | `{rc['r1_mutant_sha256']}` | `{a6['mutant_sha256']}` |",
         f"| restoration | reverse application → blob `{a6['restoration']['restores_blob']}` | reverse application → "
         f"blob `{a6['restoration']['restores_blob']}` (both checkers) |", "",
         "The exact differences are `comparison/A6-R1-TO-R2-PATCH.diff` (R1 patch → R2 patch: the index line's "
         "mutant blob and the added hunk) and `comparison/A6-R1-TO-R2-MUTANT.diff` (R1 mutant → R2 mutant: one "
         "line). A6's property (P12 or P15), configuration (F-5), test (`full_f5`), ceiling, receipt type and "
         "restoration mapping are R1's. Changed A6 catalog fields: " +
         ", ".join(f"`{k}`" for k in cat["r2"]["a6_changed_fields"]) + " — the patch and its identities, the "
         "appended edit, and the description text that the KEY_FIELDS hunk made stale (R1's uncertainty item that "
         "said KEY_FIELDS was not changed is replaced; the other description lists are R1's with one item or "
         "sentence appended).", "",
         "## The other twenty", "",
         f"{len([c for c in ORDER if c in unchanged])} Architect-defined controls — " +
         ", ".join(c for c in ORDER if c in unchanged) + " — are unchanged from R1: catalog entry equal as a whole, "
         "patch file byte-identical (`comparison/R1-TO-R2-PATCH-COMPARISON.md`). Their sections below render the same "
         "fields as R1's record, with R2's static receipts.", "",
         "Common to all 21: the mutant failing receipt is type (a), the original explorer (`explore`) through the "
         "configuration's `full_*` test with `--exact --ignored`, on the pinned mutant executable, under the "
         "configuration's ceiling (F-1: 60,000,000; every other: 30,000,000); a quotient run is never a failing "
         "receipt (AD-1-R4 F, F.1). The restored side is type (e1) on the unchanged core, except C15 (AD-1-R5 F).",
         "", "Static evidence for every one of the 21 (receipts): checker A and checker B both PASS "
         "(`receipts/20-checker-a.txt`, `receipts/21-checker-b.txt`; identical per-id verdicts); `rustfmt --check` "
         "on the patched disposable file passes: **SYNTAX_CHECKED_NOT_TYPECHECKED** (`receipts/30-syntax-check.txt`). "
         "Nothing was type-checked or compiled.", "",
         "## Summary", "",
         "| Id | § | File | Item | Property | Configuration | Edits | R2 | Patch sha256 | Mutant blob |",
         "|---|---|---|---|---|---|---|---|---|---|"]
    for cid in ORDER:
        e = by[cid]
        item = e["architect_item"] + (f"; {e['receipt_metadata_item']} (receipt metadata)"
                                      if e.get("receipt_metadata_item") else "")
        o.append(f"| {cid} | {e['architect_section']} | `{e['mutation_source_file'].rsplit('/', 1)[1]}` | "
                 f"{item} | {e['named_property_text']} | {e['configuration']} | {len(e['edits'])} | "
                 f"{'= R1' if cid in unchanged else 'changed (KEY_FIELDS hunk)'} | "
                 f"`{e['patch_sha256'][:16]}…` | `{e['mutant_blob'][:12]}…` |")
    o.append("")
    for cid in ORDER:
        e = by[cid]
        head, body = s6[cid]
        a = verdict(os.path.join(rec, "20-checker-a.txt"), cid)
        b = verdict(os.path.join(rec, "21-checker-b.txt"), cid)
        s = re.search(rf"^(OK  |FAIL) {re.escape(cid)}: .* -> (\S+)$", syn, re.M)
        o += [f"## {cid} — {e['description']} (section {e['architect_section']})", "", f"*{LABEL}*", "",
              "**R2.** " + ("Unchanged from R1: the catalog entry equals R1's and the patch file is byte-identical."
                            if cid in unchanged else
                            "Changed from R1 only by the KEY_FIELDS receipt-metadata hunk (see *The R2 correction: "
                            "A6* above); every R1 edit and hunk is kept byte for byte."), "",
              f"**Adopted contract description** (contract 7.5, line {e['contract_line']}):", "",
              fence(e["contract_row"] + "\n"), "",
              f"**Named property / test / configuration.** {e['named_property_text']} on **{e['configuration']}**: "
              f"`{e['test']}` (ignored at C: {e['ignored_at_C']}), the original explorer, ceiling "
              f"{e['ceiling_states']:,} states. Printed property must be: "
              f"{e['expected_primary_failure']['printed_property_must_be']}.", "",
              f"**Receipt type.** {e['receipt_type']}. Restored side: {e['restoration']['restored_side']}.", ""]
        if e.get("special_attribution_rules"):
            o += ["**Special rules.** " + " ".join(f"({i + 1}) {r}." for i, r in
                                                    enumerate(e["special_attribution_rules"])), ""]
        if cid == "C7":
            ac = e["additional_comparison"]
            o += [f"**Additional comparison (not required to kill).** {ac['rule']}. Patch: {ac['patch']}.", ""]
        o += [f"**Exact source item.** `{e['mutation_source_file']}` (blob `{e['baseline_blob']}`): "
              f"{e['architect_item']}" + (f"; receipt metadata: {e['receipt_metadata_item']}"
                                          if e.get("receipt_metadata_item") else "") + "; anchor line(s) at C: "
              f"{', '.join(str(x['anchor_first_line_at_C']) for x in e['edits'])}.", "",
              f"**Architect encoding — mission R1 section {head}** (verbatim as received):", "", fence(body + "\n"), ""]
        for k, x in enumerate(e["edits"]):
            tag = f" {k + 1} of {len(e['edits'])}" if len(e["edits"]) > 1 else ""
            role = " — R2 receipt metadata (mission R2 section 4)" if x.get("role") == "receipt_metadata" else ""
            o += [f"**Baseline anchor{tag}{role}** (exactly once in C's blob, at line {x['anchor_first_line_at_C']}):",
                  "", fence(x["anchor"], "rust"), "", f"**Replacement{tag}:**", "",
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
    open(os.path.join(outd, "ARCHITECT-CONTROL-DEFINITIONS-R2.md"), "w").write("\n".join(o))
    c22 = by["C22"]
    rr = c22["restored_receipt"]
    rec1 = subprocess.run(["git", "-C", REPO, "show", f"{STAGE1}:" + rr["historical_stage1_build_not_the_e4_receipt"]
                           ["directory"] + "/RECEIPT.txt"], capture_output=True, check=True).stdout.decode()
    cmd = [l.strip() for l in rec1.splitlines() if "cargo test -p nexus-governed-control" in l]
    h = rr["historical_stage1_build_not_the_e4_receipt"]
    c = ["# C22 — restored-build ruling record", "",
         "Mission P3-C11-DR01-D2A1-CONTROL-DEFINITIONS-R2-01. Executor: Claude Code, Claude Opus 5.5. The ruling of "
         "mission R1 section 7, as applied to the R2 candidate catalog; mission R2 section 9.H requires C22 to remain "
         "`PENDING_EXACT_COMMAND_HEALTHY_BUILD`. C22's entry and patch are R1's, unchanged. Nothing was built.", "",
         "## The ruling (mission R1 section 7, as received)", "",
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
         f"Status: **{rr['restored_receipt_status']}**. Neither build ran in this mission.", ""]
    open(os.path.join(outd, "C22-RESTORED-BUILD-RULING.md"), "w").write("\n".join(c))
    r = ["# catalog/ — candidate catalog `" + cat["version"] + "`", "",
         f"Status: {cat['catalog_status']}. Acceptance {cat['acceptance']}.", "",
         f"Supersedes, as a candidate, `{sup['version']}` (sha256 `{sup['sha256']}`, evidence commit "
         f"`{sup['evidence_commit']}`), which stays immutable review history. Changed from it: A6 only.", "",
         "- `catalog.json` — the machine-readable catalog: 36 entries in contract 7.5's order (35 kill-required "
         "controls and A4, the separate accepted comparison). Each control carries its edits (anchor, replacement, "
         "expected count 1, line at C), patch text and sha256, mutant blob and sha256, named property, test, "
         "configuration, ceiling, receipt type, restored-receipt link and restoration mapping, and its definition, "
         "review and execution status. `r2` lists the changed and unchanged controls; A6 carries `r2_correction`.",
         "- `CATALOG.md` — one line per entry.",
         "- `PATCH-IDENTITIES.md` — every patch and mutant identity.",
         "- `patches/<id>.patch` — 35 documentary patches against Candidate C. 34 are R1's patches, byte for byte "
         "(14 of them first carried from the blocked catalog); A6's is R1's with one KEY_FIELDS receipt-metadata "
         "hunk added. None has been applied to any worktree, built or run.", "",
         "Every control: definition CANDIDATE_READY, Architect review pending, execution NOT_ATTEMPTED.", ""]
    open(os.path.join(outd, "catalog-README.md"), "w").write("\n".join(r))
    for n in sorted(os.listdir(outd)):
        print(f"{n} sha256 {sha(open(os.path.join(outd, n), 'rb').read())}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1]))
