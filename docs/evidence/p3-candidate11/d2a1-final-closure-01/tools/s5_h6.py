#!/usr/bin/env python3
"""Stage 5 — build the AD-1-R4 H.6 detached attestation CANDIDATE. Read-only on every input; writes two new files in
the mission root: 05-H6-ATTESTATION-CANDIDATE.json and 05-H6-INPUT-HASHES.tsv. Every hash is recomputed now from the
bytes it names. The attestation lives outside the repository and outside every commit it attests (H.6)."""
import json
import os
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from common import *  # noqa

OUT = sys.argv[1]
L1 = json.load(open(os.path.join(ROOT, "receipts/12-S1-LEDGER.json")))
S2 = json.load(open(os.path.join(ROOT, "receipts/22-S2-DERIVATION.json")))
S3 = json.load(open(os.path.join(ROOT, "03-CURRENT-TREE-ALIGNMENT.json")))
inputs = []


def gitin(label, rev, path, nature, binds):
    b = show(rev, path)
    inputs.append({"label": label, "where": f"git {rev[:12]}:{path}", "git_blob": blob_id(rev, path), "sha256": sha(b),
                   "bytes": len(b), "nature": nature, "binds": binds})


def filein(label, path, nature, binds):
    b = open(path, "rb").read()
    inputs.append({"label": label, "where": path.replace(ROOT + "/", "mission:").replace(AUDITS + "/", "audits:"),
                   "git_blob": None, "sha256": sha(b), "bytes": len(b), "nature": nature, "binds": binds})


# A. source
for p, bl in SRC_BLOBS.items():
    assert blob_id(C, p) == bl
    gitin(f"A source {os.path.basename(p)}", C, p, "SOURCE", "A")
# B. ct-1 and the historical manifest
gitin("B ct-1 manifest", C, CT1[0], "SOURCE (candidate manifest; no string in it confers acceptance)", "B")
gitin("B historical 183 manifest", C, HIST183[0], "SOURCE (historical, preserved)", "B")
# contract chain
filein("contract P3-C11-D2A1-CONTRACT-02-R1 (authenticated copy)", os.path.join(ROOT, "raw/CONTRACT-02-R1.md"), "CONTRACT", "governing")
gitin("AD-1-R4", C, f"{EVB}/ad1r4-consolidation-01/ad1r4/P3-C11-D2A1-CONTRACT-02-R1-ADDENDUM-AD1-R4-DRAFT.md", "CONTRACT AMENDMENT (Owner adopted)", "governing")
gitin("AD-1-R4 Owner adoption record", C, f"{EVB}/ad1r4-adoption/OWNER-ADOPTION-2026-10-14.md", "OWNER DECISION RECORD (executor-written, unsigned)", "governing")
gitin("AD-1-R5", HEAD_EV, f"{EVB}/fr-stutter-r5-01/P3-C11-D2A1-CONTRACT-02-R1-ADDENDUM-AD1-R5-DRAFT.md", "CONTRACT AMENDMENT (Owner adopted)", "governing")
gitin("AD-1-R5 Owner adoption record", HEAD_EV, f"{EVB}/ad1r5-adoption/OWNER-ADOPTION-2026-10-14.md", "OWNER DECISION RECORD (executor-written, unsigned)", "governing; F")
gitin("AD-C7-W (K2)", HEAD_EV, f"{EVB}/d2a1-c7-adc7w-final-freeze-01/AD-C7-W-FROZEN-REVIEW.md", "CONTRACT AMENDMENT (Owner adopted, C7 only)", "governing; E")
gitin("AD-C7-W Owner adoption 2026-10-15", HEAD_EV, f"{EVB}/d2a1-c7-adc7w-owner-adoption-01/OWNER-ADOPTION-2026-10-15.md", "OWNER DECISION RECORD (executor-written, unsigned)", "E")
gitin("AD-C7-W Owner identity approval 2026-10-16", HEAD_EV, f"{EVB}/c7-owner-identity-reauthorization-2026-10-16/OWNER-DECISION-2026-10-16.md", "OWNER DECISION RECORD (executor-written, unsigned)", "E")
gitin("K1 control catalog", HEAD_EV, f"{EVB}/d2a1-control-definitions-r2-01/catalog/catalog.json", "ACCEPTED EVIDENCE (frozen catalog)", "E")
# C, D
filein("C/D all-183 alignment and 22 reconciliation (recomputed)", os.path.join(ROOT, "03-CURRENT-TREE-ALIGNMENT.json"), "EXECUTOR ANALYSIS (mechanical)", "C, D")
filein("C alignment checks", os.path.join(ROOT, "receipts/32-S3-CHECKS.txt"), "EXECUTOR ANALYSIS (mechanical)", "C")
filein("D reconciliation record", os.path.join(ROOT, "03-RECONCILIATION-22.md"), "EXECUTOR ANALYSIS", "D")
gitin("D/I relayed SAME PROPERTY ruling (FINAL-EVIDENCE-STAGE1 freeze record)", STAGE1, f"{EVB}/final-baseline-known-runs-01/FREEZE-RECORD.txt", "ARCHITECT DECISION RECORD (relayed; executor-written)", "D, I")
# E. mutations
for r in L1["_rulings"]:
    gitin(f"E ruling record {os.path.basename(r['path'])}", HEAD_EV, r["path"], "ARCHITECT DECISION RECORD (executor-written, unsigned)", "E")
ids = [k for k in L1 if not k.startswith("_")]
for cid in sorted(ids):
    x = L1[cid]
    if x.get("result_txt"):
        gitin(f"E {cid} RESULT.txt (historical executor label)", HEAD_EV, f"{EVB}/{x['result_txt']}", "RAW EXECUTOR RECORD (label is not acceptance)", "E")
    pr = x.get("primary_receipt")
    if pr:
        gitin(f"E {cid} failing receipt ({x.get('receipt_type')})", HEAD_EV, pr if pr.startswith("docs/") else f"{EVB}/{pr}", "RAW MEASUREMENT (accepted evidence)", "E")
    if x.get("witness_receipt"):
        wp = x["witness_receipt"]
        gitin(f"E {cid} mutant witness receipt", HEAD_EV, wp if wp.startswith("docs/") else f"{EVB}/{wp}", "RAW MEASUREMENT (accepted evidence)", "E")
    if cid == "C7":
        gitin("E C7 healthy witness receipt", HEAD_EV, f"{EVB}/d2a1-c7-compliant-exec-02/receipts/40-run-C-healthy-witness/raw-output.txt", "RAW MEASUREMENT (accepted evidence)", "E")
        gitin("E C7 packet manifest", HEAD_EV, f"{EVB}/d2a1-c7-compliant-exec-02/MANIFEST.sha256", "ACCEPTED EVIDENCE (seal)", "E")
for cid, n in (("C1", "50-witness-C1"), ("C25", "51-witness-C25"), ("C4", "52-witness-C4")):
    gitin(f"E {cid} healthy witness receipt (e3)", STAGE2, f"{EVB}/final-baseline-remaining-configs-01/receipts/{n}/raw-output.txt", "RAW MEASUREMENT (accepted evidence)", "E")
gitin("E C22 healthy build (e4) classification", HEAD_EV, f"{EVB}/d2a1-mutation-tranche1-exec-01/receipts/20-healthy-build/CLASSIFICATION.txt", "RAW EXECUTOR RECORD (accepted as C22's restored side)", "E")
filein("E 35-control decision ledger", os.path.join(ROOT, "01-MUTATION-DECISIONS.tsv"), "EXECUTOR ANALYSIS (consolidation)", "E")
filein("E campaign checks", os.path.join(ROOT, "01-CAMPAIGN-CHECKS.txt"), "EXECUTOR ANALYSIS (mechanical)", "E")
# F. configurations
KR, RC = f"{EVB}/final-baseline-known-runs-01/receipts", f"{EVB}/final-baseline-remaining-configs-01/receipts"
DIRS = {"F-2": (STAGE1, KR, "41-full-F-2"), "F-B": (STAGE1, KR, "42-full-F-B"), "F-3": (STAGE1, KR, "43-full-F-3"),
        "F-PT": (STAGE1, KR, "44-full-F-PT"), "F-1": (STAGE1, KR, "45-full-F-1"), "F-G": (STAGE2, RC, "60-full-F-G"),
        "F-B2": (STAGE2, RC, "61-full-F-B2"), "F-4": (STAGE2, RC, "62-full-F-4"), "F-W": (STAGE2, RC, "63-full-F-W"),
        "F-8": (STAGE2, RC, "64-full-F-8"), "F-5": (STAGE2, RC, "65-full-F-5"), "F-6": (STAGE2, RC, "66-full-F-6"),
        "F-7": (STAGE2, RC, "67-full-F-7"), "F-D": (STAGE2, RC, "68-full-F-D"), "F-R": (STAGE2, RC, "69-full-F-R")}
for cfg in ["X-1", "X-2", "X-3", "X-4", "X-5"]:
    n = {"X-1": "41", "X-2": "42", "X-3": "43", "X-4": "44", "X-5": "45"}[cfg]
    filein(f"F {cfg} receipt (Stage C, optimised e1)", f"{AUDITS}/p3-c11-dr01-d2a1-exact-source-validation-02/receipts/{n}-stage-C-{cfg}/raw-output.txt", "RAW MEASUREMENT", "F")
gitin("F X-1..X-5 Stage-1 suite", STAGE1, f"{KR}/30-suite/raw-output.txt", "RAW MEASUREMENT", "F; H")
for cfg, (rev, d, n) in DIRS.items():
    nat = "RAW MEASUREMENT — INCOMPLETE (establishes no property)" if cfg == "F-R" else "RAW MEASUREMENT (accepted evidence)"
    gitin(f"F {cfg} receipt", rev, f"{d}/{n}/raw-output.txt", nat, "F")
gitin("F F-R stuttering-refinement proof", HEAD_EV, f"{EVB}/fr-stutter-r5-01/FR-STUTTERING-REFINEMENT-PROOF-DRAFT.md", "DERIVED PROOF OBLIGATION (accepted, as conveyed)", "F; G")
gitin("F/G F-R branch disposition", HEAD_EV, f"{EVB}/fr-stutter-r5-01/FR-BRANCH-DISPOSITION.tsv", "DERIVED PROOF OBLIGATION (accepted, as conveyed)", "G")
gitin("F replay occurrence matrix", HEAD_EV, f"{EVB}/fr-stutter-r5-01/REPLAY-OCCURRENCE-MATRIX.tsv", "EXECUTOR ANALYSIS (AD-1-R5 C(4))", "F")
filein("F configuration record", os.path.join(ROOT, "02-CONFIGURATIONS.tsv"), "EXECUTOR ANALYSIS (consolidation of raw measurements)", "F")
# G. branch accounting
gitin("G branch vocabulary (193 rows)", HEAD_EV, f"{EVB}/final-baseline-remaining-configs-01/vocabulary/VOCABULARY.tsv", "ACCEPTED EVIDENCE (vocabulary)", "G")
filein("G consolidated branch accounting", os.path.join(ROOT, "02-BRANCH-VOCABULARY.tsv"), "EXECUTOR ANALYSIS (consolidation)", "G")
# H. Fast Local
filein("H fmt receipt (reused)", f"{AUDITS}/p3-c11-dr01-d2a1-closure-packet-01/receipts/50-fmt-gate-and-run.json", "RAW MEASUREMENT", "H")
filein("H all-features clippy raw output (this mission)", os.path.join(ROOT, "receipts/43b-clippy-all-features-r2/raw-output.txt"), "RAW MEASUREMENT", "H")
filein("H all-features clippy descriptor", os.path.join(ROOT, "receipts/43b-clippy-all-features-r2/descriptor.json"), "RAW MEASUREMENT", "H")
filein("H workspace tests raw output (reused)", f"{AUDITS}/p3-c11-dr01-d2a1-exact-source-validation-02/receipts/30-stage-B-workspace-test/raw-output.txt", "RAW MEASUREMENT", "H")
filein("H Fast Local matrix", os.path.join(ROOT, "04-FAST-LOCAL-MATRIX.tsv"), "EXECUTOR ANALYSIS", "H")

configs = {c: {"raw_outcome": S2["configs"][c]["outcome"], "raw_sha256": S2["configs"][c]["raw_sha256"],
               "admissibility": S2["configs"][c]["admissibility"], "discharge": S2["configs"][c]["discharge"]}
           for c in S2["configs"]}
mut = {cid: {"recording": L1[cid].get("recording") or None, "first_failure_as_printed": L1[cid].get("first_failure"),
             "failing_raw_sha256": L1[cid].get("primary_raw_sha256"), "decision": "ACCEPTED",
             "decision_provenance": L1[cid].get("decision_provenance")} for cid in ids}
att = {
    "document": "AD-1-R4 H.6 DETACHED ATTESTATION — CANDIDATE",
    "status": "CANDIDATE — PENDING_DECISIONS. Not accepted. Not an approval, signature or publication.",
    "authority": "A string is never authority. A SHA256 authenticates bytes, not the truth of statements inside them. "
                 "Only the Sole Architect may accept this attestation; the Owner keeps every Owner decision.",
    "detachment": "Lives only in the local mission evidence root, outside the repository; not written into Candidate C "
                  "or into any commit whose identity it attests; no commit or push was made.",
    "generated_utc": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
    "mission": "P3-C11-DR01-D2A1-FINAL-CLOSURE-01",
    "A_source": {"commit": C, "tree": C_TREE, "parent": C_PARENT, "blobs": SRC_BLOBS, "pinned_executable_sha256": E_C},
    "B_current_tree_manifest": {"path": CT1[0], "git_blob": CT1[1], "sha256": CT1[2], "version": CT1[3]},
    "C_alignment": {"status": "ALIGNED (mechanical; H.1-H.5)", "counts": S3["counts"],
                    "current_review_status_all": "pending"},
    "D_reconciliation_22": [{"control": r["control"], "file": r["file"], "set": r["set"],
                             "first_stale_commit": r["first_stale_commit"], "definition_unchanged": r["definition_unchanged"],
                             "class": ("MATERIALLY_CHANGED_OBSERVABLE_MECHANISM — REQUIRES_EXPLICIT_ARCHITECT_REVIEW"
                                       if r["control"] == "M18" else
                                       "ESTABLISHED_UNDER_EXISTING_ARCHITECT_DECISION + MECHANICALLY_ALIGNED")}
                            for r in S3["changed_22"]],
    "E_mutations_35": {"count": len(ids), "by_id_git_records": 33, "conversational": ["C4", "C7"], "controls": mut},
    "F_configurations_20": configs,
    "F_R_recording": "F-R — UNCHANGED-CORE OBLIGATIONS DISCHARGED BY ACCEPTED STUTTERING-REFINEMENT PROOF",
    "G_branch_accounting": {"rows": 193, "counts": S2["branch_counts"], "proof_taken_rows": S2["derived_taken_rows"],
                            "rule": "no derived row is recorded as measured; no unobserved row is recorded as covered"},
    "H_fast_local": {"verdict": "FAST_LOCAL_NOT_FULLY_DISCHARGED",
                     "evidenced": ["checkout verification", "toolchain and system libraries", "cargo fmt --all -- --check",
                                   "cargo clippy --workspace --all-targets --all-features --locked -- -D warnings",
                                   "cargo test --workspace --locked (ripgrep 14.1.0, not 14.1.1)"],
                     "not_discharged_steps": 21},
    "I_current_tree_review_decisions": [
        {"decision": "All 22 stale M-control mappings: SAME PROPERTY", "source": "relayed FINAL-EVIDENCE-STAGE1; FREEZE-RECORD.txt blob 4321cb34", "nature": "ARCHITECT DECISION (relayed)"},
        {"decision": "M18: SAME INTENDED AUTHORITY PROPERTY. CHANGED OBSERVABLE FAILURE MECHANISM.", "source": "mission text (CLOSURE-PACKET-01; FINAL-CLOSURE-01 section 8)", "nature": "ARCHITECT SOURCE-LEVEL DISPOSITION (conveyed); not an acceptance"},
        {"decision": "current-tree acceptance of any of the 183", "source": "none", "nature": "PENDING"}],
    "J_assumptions_and_limitations": ["A-1 and A-2 undischarged", "AQ6 binding model-scope limitation for F-3/F-PT/F-B",
                                      "quotient original counts DERIVED only", "F-R safety/D1/L1 DERIVED through the proof",
                                      "105 branch rows unobserved", "D2A-1 core dormant, production-unactivated",
                                      "decision records executor-written and unsigned"],
    "K_outstanding_decisions": "see 05-H6-UNRESOLVED.md",
    "section11_label": "WITHHELD",
    "inputs": inputs,
}
open(os.path.join(OUT, "05-H6-ATTESTATION-CANDIDATE.json"), "x").write(json.dumps(att, indent=1, ensure_ascii=False) + "\n")
cols = ["label", "where", "git_blob", "sha256", "bytes", "nature", "binds"]
open(os.path.join(OUT, "05-H6-INPUT-HASHES.tsv"), "x").write(
    "\n".join(["\t".join(cols)] + ["\t".join(str(i[k]) for k in cols) for i in inputs]) + "\n")
print(len(inputs), "inputs bound")
