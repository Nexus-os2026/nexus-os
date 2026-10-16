#!/usr/bin/env python3
"""Stage 6B — the final mechanical consistency check of the closure packet. Read-only on the packet it checks.

  final_check.py check <root> <out-file>        one pass over <root> (the evidence root, or a fixture copy)
  final_check.py negatives <scratch> <out-file> builds 8 isolated fixtures under <scratch>, each one defect, and
                                                requires every one to be rejected
Identities are always re-derived from the Git object store, never from the packet itself.
"""
import csv
import hashlib
import json
import os
import re
import shutil
import subprocess
import sys

TOOLS = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, TOOLS)
from common import (C, C_TREE, C_PARENT, HEAD_EV, EVB, SRC_BLOBS, CT1, HIST183, CONTRACT_SHA, AD1R4, AD1R5,  # noqa
                    ADC7W, E_C, REPO, AUDITS, show, blob_id, exists, sha, git)

csv.field_size_limit(1 << 30)
REQUIRED = ["00-PROVENANCE.md", "00-SOURCE-IDENTITIES.json", "00-CONTRACT-CHAIN.md", "00-RESOURCE-PREFLIGHT.txt",
            "01-MUTATION-DECISIONS.tsv", "01-MUTATION-EVIDENCE-MATRIX.md", "01-ACCEPTANCE-PROVENANCE.md",
            "01-CAMPAIGN-CHECKS.txt", "02-CONFIGURATIONS.tsv", "02-BRANCH-VOCABULARY.tsv",
            "02-IGNORED-TEST-DISPOSITIONS.md", "02-RECEIPT-PROVENANCE.md", "02-ASSURANCE-LIMITATIONS.md",
            "02-SECTION84-CLOSURE.md", "03-CURRENT-TREE-ALIGNMENT.json", "03-RECONCILIATION-22.md",
            "03-M183-DECISION-PROVENANCE.tsv", "03-M18-TECHNICAL-DISPOSITION.md", "03-REMAINING-183-DECISIONS.md",
            "04-FAST-LOCAL-MATRIX.tsv", "04-RUST-VALIDATION.md", "04-NATIVE-FRONTEND-PYTHON.md",
            "04-SECURITY-DEPENDENCY-GATES.md", "04-RESOURCE-RECEIPTS.md", "04-FAST-LOCAL-VERDICT.md",
            "05-H6-ATTESTATION-CANDIDATE.json", "05-H6-ATTESTATION.md", "05-H6-INPUT-HASHES.tsv",
            "05-H6-DECISION-PROVENANCE.md", "05-H6-UNRESOLVED.md", "06-SECTION11-DECISION-DOCKET.md",
            "06-TECHNICAL-CLOSURE-CRITERIA.md", "06-UNRESOLVED-CONTRACT-OBLIGATIONS.md", "06-NEXT-AUTHORITY-BOUNDARY.md",
            "REPORT.md", "CLOSURE-MATRIX.tsv", "SOURCE-PROVENANCE.md", "MUTATION-35-ACCEPTANCE.md",
            "CONFIGURATION-20-CLOSURE.md", "BRANCH-193-CLOSURE.md", "CURRENT-TREE-183-STATUS.md", "FAST-LOCAL-FINAL.md",
            "H6-ATTESTATION-STATUS.md", "OWNER-DECISION-DOCKET.md", "ARCHITECT-REVIEW-QUESTIONS.md", "FINAL-STATE.txt"]
FR_RECORDING = "F-R — UNCHANGED-CORE OBLIGATIONS DISCHARGED BY ACCEPTED STUTTERING-REFINEMENT PROOF"
PROHIBITED_FR = ["F-R " + "PASS", "F-R ORIGINAL GRAPH " + "PASSED", "F-R ORIGINAL GRAPH " + "EXPLORED"]
LABEL = "IMPLEMENTED — INTERIM REVIEW PASSED — AUDIT PENDING"
CLAIMS = [r"D2A-?1 ACCEPTED", r"CANDIDATE 11 INTEGRATED", r"PHASE THREE COMPLETE", r"Phase Three (is )?complete",
          r"D2A-1 (is )?accepted", r"FAST_LOCAL_FULLY_DISCHARGED"]
NEG = re.compile(r"\b(not|NOT|No|no|never|Never|nor|NOR|without|WITHHELD|Do not|do not|unless|until|cannot|Only|only|"
                 r"either|Never|neither|withheld|pending|PENDING)\b|\bnone\b|\bNONE\b")
SCAN_SKIP_DIRS = ("raw", "tools", "development", "fixtures")
CONSERVED = {"primary_head": "f727f5c39fab8d5c729a55eb28ad576d3d56ce47", "main": "4b36f60694148b60029733bc7b3d26e5a215d460",
             "rebuild/phase0-trust-boundary": "f727f5c39fab8d5c729a55eb28ad576d3d56ce47",
             "github/main": "6e15dee613ea35823d1adbf6d8917ed59eeda583",
             "github/rebuild/phase0-trust-boundary": "f727f5c39fab8d5c729a55eb28ad576d3d56ce47",
             "github/review/p3-c11-d2a1-c7-compliant-exec-02": HEAD_EV}
WT = "/home/nexus/NEXUS/nexus-os-p3-c11-d2a1-final-closure-01"


def tsv(path):
    with open(path, newline="") as f:
        return list(csv.DictReader(f, delimiter="\t", quoting=csv.QUOTE_NONE))


def fsha(path):
    return hashlib.sha256(open(path, "rb").read()).hexdigest()


def check(root):
    out = []
    add = lambda label, ok, detail="": out.append((label, bool(ok), str(detail)[:400]))
    j = lambda n: json.load(open(os.path.join(root, n)))
    # 0. every required output present and non-empty
    missing = [n for n in REQUIRED if not os.path.isfile(os.path.join(root, n)) or os.path.getsize(os.path.join(root, n)) == 0]
    add("required outputs: all present and non-empty", not missing, missing)
    if missing:
        return out
    # 1. exact source and contract identities, from Git
    ids = j("00-SOURCE-IDENTITIES.json")
    cc = ids["candidate_C"]
    add("source: commit/tree/parent of Candidate C equal Git", cc["commit"] == C and cc["tree"] == C_TREE ==
        git("rev-parse", f"{C}^{{tree}}").strip() and cc["parent"] == C_PARENT == git("rev-parse", f"{C}^").strip())
    bad = [p for p, v in cc["files"].items() if v["blob"] != blob_id(C, p) or v["sha256"] != sha(show(C, p))]
    add("source: every recorded source file blob and sha256 equal Git at C", not bad, bad)
    add("source: the three ledger blobs are the governing ones",
        all(cc["files"][p]["blob"] == b for p, b in SRC_BLOBS.items()))
    add("contract: sha256 of the authenticated copy", ids["contract"]["sha256"] == CONTRACT_SHA ==
        fsha(os.path.join(root, "raw/CONTRACT-02-R1.md")))
    add("contract: AD-1-R4, AD-1-R5, AD-C7-W identities equal Git",
        (ids["AD-1-R4"]["blob"], ids["AD-1-R4"]["sha256"]) == AD1R4 == (AD1R4[0], sha(git("cat-file", "-p", AD1R4[0], binary=True)))
        and (ids["AD-1-R5"]["blob"], ids["AD-1-R5"]["sha256"]) == AD1R5 == (AD1R5[0], sha(git("cat-file", "-p", AD1R5[0], binary=True)))
        and (ids["AD-C7-W"]["K2_blob"], ids["AD-C7-W"]["K2_sha256"]) == ADC7W == (ADC7W[0], sha(git("cat-file", "-p", ADC7W[0], binary=True))))
    add("manifests: ct-1 and the historical 183 manifest identities equal Git",
        (ids["current_tree_manifest_ct1"]["blob"], ids["current_tree_manifest_ct1"]["sha256"]) == (CT1[1], CT1[2])
        == (blob_id(C, CT1[0]), sha(show(C, CT1[0]))) and ids["historical_183_manifest"]["sha256"] == HIST183[2] ==
        sha(show(C, HIST183[0])))
    # 2. valid manifests of the prior roots and the packet custody receipt
    for name, exp in (("p3-c11-dr01-d2a1-exact-source-validation-02", "36f10bf966cdd0709dca61885004d02873e47e214e13af57b4b7901bb0fcb822"),
                      ("p3-c11-dr01-d2a1-closure-evidence-audit-01", "aa38221f0e4d53ab99e5c49aad104fae952637f8b3984d6250c27732f4c01732"),
                      ("p3-c11-dr01-d2a1-closure-packet-01", "372d1bc16e5a6f4a04a1e9f8b9dd5b15c7ebea45ca2000ee9ef7f83aa55a7f3f")):
        d = os.path.join(AUDITS, name)
        r = subprocess.run(["sha256sum", "-c", "--quiet", "MANIFEST.sha256"], cwd=d, capture_output=True)
        add(f"prior root {name}: manifest sha256 as stated and every entry verifies", fsha(os.path.join(d, "MANIFEST.sha256")) == exp
            and r.returncode == 0, r.stdout[-200:])
    pc = open(os.path.join(root, "receipts/25-PACKET-CUSTODY.txt")).read()
    add("packet custody receipt: 19 of 19 Git evidence packets verify", "RESULT: 19 of 19 packets verify" in pc)
    # 3-4. the 35 acceptances, no relabel
    cat = json.loads(show(HEAD_EV, f"{EVB}/d2a1-control-definitions-r2-01/catalog/catalog.json"))
    ids35 = [i for i in cat["ids_in_contract_order"] if i != "A4"]
    rows = tsv(os.path.join(root, "01-MUTATION-DECISIONS.tsv"))
    add("mutations: exactly the catalog's 35 control ids, each once", sorted(r["id"] for r in rows) == sorted(ids35)
        and len(rows) == 35, [r["id"] for r in rows])
    add("mutations: every decision ACCEPTED with a stated provenance", all(r["decision"].startswith(("MUTATION CONTROL KILL ACCEPTED", "ACCEPTED"))
                                                                          and r["decision_provenance"] for r in rows))
    conv = sorted(r["id"] for r in rows if "CONVERSATIONAL" in r["decision_provenance"])
    add("mutations: exactly C4 and C7 are conversational rulings; the other 33 cite Git records", conv == ["C4", "C7"]
        and all("EXECUTOR-WRITTEN GIT RECORD" in r["decision_provenance"] for r in rows if r["id"] not in ("C4", "C7")), conv)
    relab, rawbad = [], []
    for r in rows:
        if r["result_txt"]:
            b = show(HEAD_EV, f"{EVB}/{r['result_txt']}")
            if sha(b) != r["result_sha256"] or blob_id(HEAD_EV, f"{EVB}/{r['result_txt']}") != r["result_blob"]:
                relab.append(r["id"])
        p = r["primary_receipt"]
        if not p or not exists(HEAD_EV, f"{EVB}/{p}") or sha(show(HEAD_EV, f"{EVB}/{p}")) != r["primary_raw_sha256"]:
            rawbad.append(r["id"])
    add("mutations: no executor RESULT record relabelled (bytes equal Git)", not relab, relab)
    add("mutations: every accepted failing receipt present in Git with the recorded sha256", not rawbad, rawbad)
    led = j("receipts/12-S1-LEDGER.json")
    recs = [(x[0], x[1]) for cid in ids35 for x in led[cid].get("decision_records", [])]
    add("mutations: every cited ruling record exists in Git with its blob",
        all(blob_id(HEAD_EV, f"{EVB}/{p}") == b for p, b in recs) and len({p for p, _ in recs}) == 9, len({p for p, _ in recs}))
    rec = {r["id"]: r["recording"] for r in rows}
    add("mutations: recordings exactly C1, C25, C4, C7", rec["C1"] == "C1: P1 (first failure E3; witness)" and
        rec["C25"] == "C25: P7 (first failure E4; witness)" and rec["C4"] == "C4: P4, P8 (first failure P4; witness)"
        and rec["C7"] == "C7: P6 (first failure E11; witness)", {k: rec[k] for k in ("C1", "C25", "C4", "C7")})
    c7 = [r for r in rows if r["id"] == "C7"][0]
    add("mutations: C7's original F-PT first failure is E11, never P6", c7["first_failure"] == "E11")
    # 5-7. configurations, F-R
    cfg = {r["config"]: r for r in tsv(os.path.join(root, "02-CONFIGURATIONS.tsv"))}
    order = ["X-1", "X-2", "X-3", "X-4", "X-5", "F-1", "F-2", "F-3", "F-4", "F-5", "F-6", "F-7", "F-8", "F-PT", "F-B",
             "F-B2", "F-G", "F-W", "F-D", "F-R"]
    add("configurations: all 20 accounted for", sorted(cfg) == sorted(order), sorted(cfg))
    if sorted(cfg) == sorted(order):
        add("configurations: 19 PASS and F-R INCOMPLETE", all(cfg[c]["raw_outcome"] == "PASS" for c in order[:-1])
            and cfg["F-R"]["raw_outcome"] == "INCOMPLETE" and "INCOMPLETE" in cfg["F-R"]["admissibility"],
            {c: cfg[c]["raw_outcome"] for c in order})
        add("configurations: F-R's discharge named exactly as AD-1-R5 D", cfg["F-R"]["discharge"].startswith(FR_RECORDING)
            and "NOT an explorer PASS" in cfg["F-R"]["discharge"])
        add("configurations: quotient runs only for F-3, F-PT, F-B", sorted(c for c in order if "quotient" in cfg[c]["explorer"])
            == ["F-3", "F-B", "F-PT"])
        hs = {i["label"]: i["sha256"] for i in tsv(os.path.join(root, "05-H6-INPUT-HASHES.tsv"))}
        add("configurations: every full-run raw sha256 equals the H.6-bound hash",
            all(cfg[c]["raw_sha256"] == hs.get(f"F {c} receipt") for c in order[5:]))
    # 8. branches
    br = tsv(os.path.join(root, "02-BRANCH-VOCABULARY.tsv"))
    cnt = {k: sum(r["classification"] == k for r in br) for k in ("MEASURED_COMPLETE", "MEASURED_IN_INCOMPLETE_PREFIX", "UNOBSERVED")}
    add("branches: 193 rows, 85 / 3 / 105", len(br) == 193 and cnt == {"MEASURED_COMPLETE": 85, "MEASURED_IN_INCOMPLETE_PREFIX": 3,
                                                                       "UNOBSERVED": 105}, cnt)
    wrong = [r["id"] for r in br if (r["complete_runs_taking"] == "-" and r["fr_prefix"] != "TAKEN") != (r["classification"] == "UNOBSERVED")
             or (r["classification"] == "MEASURED_COMPLETE") != (r["complete_runs_taking"] != "-")]
    add("branches: no unobserved row marked covered; classification follows the measured columns only", not wrong, wrong)
    # 9-10. 183
    al = j("03-CURRENT-TREE-ALIGNMENT.json")
    add("183: alignment identities equal Git (ct-1 blob/sha256, C)", al["identities"]["ct1"]["blob"] == blob_id(C, CT1[0])
        and al["identities"]["ct1"]["sha256"] == sha(show(C, CT1[0])) and al["identities"]["commit"] == C)
    add("183: counts 183/184/39/191, 162 equal, 22 changed", al["counts"] == {"controls": 183, "pairs": 184, "files": 39,
                                                                         "anchors": 191, "pairs_equal": 162, "pairs_changed": 22}, al["counts"])
    m183 = tsv(os.path.join(root, "03-M183-DECISION-PROVENANCE.tsv"))
    add("183: 183 rows; every current status 'pending'; no acceptance recorded",
        len(m183) == 183 and all(r["ct1_current_review_status"] == "pending" and
                                 r["current_tree_acceptance_decision"].startswith("NONE RECORDED") for r in m183))
    h6 = j("05-H6-ATTESTATION-CANDIDATE.json")
    add("183: the attestation keeps every current review status pending", h6["C_alignment"]["current_review_status_all"] == "pending"
        and any(d["nature"] == "PENDING" for d in h6["I_current_tree_review_decisions"]))
    # 11. Fast Local
    v = open(os.path.join(root, "04-FAST-LOCAL-VERDICT.md")).read()
    add("Fast Local: verdict recorded as FAST_LOCAL_NOT_FULLY_DISCHARGED", "## FAST_LOCAL_NOT_FULLY_DISCHARGED" in v
        and h6["H_fast_local"]["verdict"] == "FAST_LOCAL_NOT_FULLY_DISCHARGED")
    cl = os.path.join(root, "receipts/43b-clippy-all-features-r2")
    ok = os.path.isfile(os.path.join(cl, "CLASSIFICATION.txt")) and \
        open(os.path.join(cl, "CLASSIFICATION.txt")).readline().strip() == "CLASSIFICATION: PASS" and \
        os.path.isfile(os.path.join(cl, "raw-output.txt")) and \
        fsha(os.path.join(cl, "raw-output.txt")) == {i["label"]: i["sha256"] for i in tsv(os.path.join(root, "05-H6-INPUT-HASHES.tsv"))}.get(
            "H all-features clippy raw output (this mission)")
    add("Fast Local: the all-features Clippy receipt is present, PASS, and equal to its bound hash", ok)
    desc = json.load(open(os.path.join(cl, "descriptor.json"))) if os.path.isfile(os.path.join(cl, "descriptor.json")) else {}
    add("Fast Local: that receipt ran exactly the workflow's Clippy command",
        desc.get("argv") == ["cargo", "clippy", "--workspace", "--all-targets", "--all-features", "--locked", "--", "-D", "warnings"])
    fm = tsv(os.path.join(root, "04-FAST-LOCAL-MATRIX.tsv"))
    add("Fast Local: the matrix lists all 31 workflow steps; 21 not discharged", len(fm) == 31 and
        sum(r["status"] == "NOT_DISCHARGED" for r in fm) == 21)
    # 12. H.6 detached
    a6 = os.path.join(root, "05-H6-ATTESTATION-CANDIDATE.json")
    b = open(a6, "rb").read()
    hb = hashlib.sha1(b"blob %d\0" % len(b) + b).hexdigest()
    in_store = subprocess.run(["git", "-C", REPO, "cat-file", "-e", hb], capture_output=True).returncode == 0
    add("H.6: attestation binds Candidate C exactly", h6["A_source"]["commit"] == C and h6["A_source"]["tree"] == C_TREE
        and h6["B_current_tree_manifest"]["git_blob"] == CT1[1])
    add("H.6: detached — outside every worktree, and its blob is in no Git object store", not in_store and
        not os.path.realpath(root).startswith("/home/nexus/NEXUS/"), hb)
    add("H.6: status is CANDIDATE — PENDING_DECISIONS", h6["status"].startswith("CANDIDATE — PENDING_DECISIONS"))
    badin = [i["label"] for i in h6["inputs"] if i["git_blob"] and (not exists(HEAD_EV, i["where"].split(":", 1)[1]) and
                                                                    not exists(i["where"].split()[1].split(":")[0], i["where"].split(":", 1)[1]))]
    add("H.6: every Git-bound input path exists", not badin, badin)
    # 13. decision provenance not invented
    prov = open(os.path.join(root, "05-H6-DECISION-PROVENANCE.md")).read() + open(os.path.join(root, "01-ACCEPTANCE-PROVENANCE.md")).read()
    add("provenance: C4 and C7 stated as having no Git record", prov.count("no Git record") >= 2)
    add("provenance: no claimed signature", not re.search(r"\bsigned by\b|\bsignature of\b|\bdigitally signed\b", prov, re.I))
    claimed = set(re.findall(r"ARCHITECT-RULING-[A-Z0-9-]+\.md", prov))
    real = set(os.path.basename(p) for p in git("ls-tree", "-r", "--name-only", HEAD_EV, "--", EVB).split() if "ARCHITECT-RULING" in p)
    add("provenance: every ARCHITECT-RULING record named exists in Git", claimed <= real, sorted(claimed - real))
    # 14-15. no source change, no publication
    st = git("status", "--porcelain=v1").strip()
    refs = {k: git("rev-parse", k).strip() for k in CONSERVED if k != "primary_head"}
    add("no source change: primary checkout clean at its starting HEAD", st == "" and git("rev-parse", "HEAD").strip() == CONSERVED["primary_head"], st[:100])
    add("no publication: protected and evidence refs unchanged", all(refs[k] == CONSERVED[k] for k in refs), refs)
    add("no publication: no ref names this mission", not [l for l in git("for-each-ref", "--format=%(refname)").split() if "d2a1-final-closure" in l])
    r = subprocess.run([sys.executable, os.path.join(TOOLS, "verify_tree.py"), WT], capture_output=True, text=True)
    vt = json.loads(r.stdout) if r.stdout else {}
    add("no source change: the mission worktree's tracked bytes are exactly C", vt.get("head") == C and not vt.get("content_mismatches")
        and not vt.get("missing") and not vt.get("mode_mismatches") and not vt.get("index_differs_from_head")
        and set(vt.get("unexpected_status", [])) <= {"!! app/src-tauri/permissions/"})
    # 16-17. no label, no completion claim, no prohibited F-R wording
    hits_fr, hits_label, hits_claim, fs = [], [], [], open(os.path.join(root, "FINAL-STATE.txt")).read()
    for dp, dn, fn in os.walk(root):
        rel = os.path.relpath(dp, root)
        if rel.split(os.sep)[0] in SCAN_SKIP_DIRS:
            continue
        for n in fn:
            if not n.endswith((".md", ".txt", ".tsv", ".json")) or n == "MANIFEST.sha256":
                continue
            p = os.path.join(dp, n)
            for k, line in enumerate(open(p, errors="replace"), 1):
                where = f"{os.path.relpath(p, root)}:{k}"
                if any(x in line for x in PROHIBITED_FR):
                    hits_fr.append(where)
                if LABEL in line and not n.startswith("06-SECTION11-DECISION-DOCKET"):
                    hits_label.append(where)
                if any(re.search(c, line) for c in CLAIMS) and not NEG.search(line):
                    hits_claim.append(where)
    add("no prohibited F-R wording anywhere in the packet's documents", not hits_fr, hits_fr)
    add("no Section 11 label: the label text appears only as the docket's quotation; FINAL-STATE says WITHHELD",
        not hits_label and re.search(r"(?m)^\s*SECTION11_LABEL:\s+WITHHELD\s*$", fs) is not None
        and h6["section11_label"] == "WITHHELD", hits_label)
    add("no acceptance, integration or Phase Three completion claim (every such phrase is negated)", not hits_claim, hits_claim)
    add("FINAL-STATE: one allowed classification and the six status fields",
        sum(x in fs for x in ("D2A1_CLOSURE_EVIDENCE_CANDIDATE — ARCHITECT_REVIEW_PENDING", "D2A1_CLOSURE_BLOCKED — MISSING_TECHNICAL_EVIDENCE",
                              "D2A1_CLOSURE_BLOCKED — OWNER_DECISION_REQUIRED", "D2A1_CLOSURE_BLOCKED — FAST_LOCAL_INCOMPLETE",
                              "D2A1_CLOSURE_STOPPED — IDENTITY_OR_RESOURCE_FAILURE")) == 1
        and all(k in fs for k in ("SECTION11_LABEL:", "H6_ATTESTATION:", "SOURCE:", "MUTATIONS:", "CURRENT_TREE_183:", "FAST_LOCAL:")))
    return out


def write(results, path, header):
    fails = [r for r in results if not r[1]]
    with open(path, "x") as f:
        f.write(header + "\n")
        for label, ok, d in results:
            f.write(f"{'OK  ' if ok else 'FAIL'} {label}" + (f" :: {d}" if (d and not ok) else "") + "\n")
        f.write(f"\nRESULT: {len(results) - len(fails)} of {len(results)} checks hold; {len(fails)} fail\n")
    return fails


def negatives(scratch, outp):
    root = os.path.dirname(TOOLS)
    os.makedirs(scratch, exist_ok=True)
    cases = []

    def fixture(n):
        d = os.path.join(scratch, f"fixture-{n}")
        if os.path.exists(d):
            shutil.rmtree(d)
        shutil.copytree(root, d, ignore=shutil.ignore_patterns("development", "tools", "MANIFEST.sha256", "06B-*"))
        return d

    def edit(path, fn):
        s = open(path).read()
        t = fn(s)
        assert t != s, path
        open(path, "w").write(t)

    d = fixture(1)
    edit(f"{d}/00-SOURCE-IDENTITIES.json", lambda s: s.replace(SRC_BLOBS["crates/nexus-governed-control/src/ledger/protocol.rs"],
                                                                  "0" * 40, 1))
    cases.append(("1 altered source identity", d, "source: every recorded source file blob and sha256 equal Git at C"))
    d = fixture(2)
    edit(f"{d}/01-MUTATION-DECISIONS.tsv", lambda s: "\n".join(l for l in s.split("\n") if not l.startswith("C10\t")))
    cases.append(("2 missing accepted mutation receipt (C10 row removed)", d, "mutations: exactly the catalog's 35 control ids, each once"))
    d = fixture(3)
    edit(f"{d}/02-CONFIGURATIONS.tsv", lambda s: "\n".join(l.replace("\tINCOMPLETE\t", "\tPASS\t", 1) if l.startswith("F-R\t") else l
                                                           for l in s.split("\n")))
    cases.append(("3 F-R relabelled PASS", d, "configurations: 19 PASS and F-R INCOMPLETE"))
    d = fixture(4)
    edit(f"{d}/02-BRANCH-VOCABULARY.tsv", lambda s: s.replace("\tUNOBSERVED\t", "\tMEASURED_COMPLETE\t", 1))
    cases.append(("4 unobserved branch relabelled covered", d, "branches: no unobserved row marked covered; classification follows the measured columns only"))
    d = fixture(5)
    edit(f"{d}/03-M183-DECISION-PROVENANCE.tsv", lambda s: s.replace("\tpending\t", "\taccepted\t", 1))
    cases.append(("5 pending 183 decision relabelled accepted", d, "183: 183 rows; every current status 'pending'; no acceptance recorded"))
    d = fixture(6)
    edit(f"{d}/05-H6-ATTESTATION-CANDIDATE.json", lambda s: s.replace(f'"commit": "{C}"', '"commit": "71264aa81a40b86189783facc8437e0427887c16"', 1))
    cases.append(("6 wrong source commit in H.6", d, "H.6: attestation binds Candidate C exactly"))
    d = fixture(7)
    shutil.rmtree(f"{d}/receipts/43b-clippy-all-features-r2")
    cases.append(("7 missing required Fast Local receipt (all-features Clippy)", d,
                  "Fast Local: the all-features Clippy receipt is present, PASS, and equal to its bound hash"))
    d = fixture(8)
    edit(f"{d}/01-ACCEPTANCE-PROVENANCE.md", lambda s: s + "\nC7 acceptance record: `ARCHITECT-RULING-C7-ACCEPTANCE.md`.\n")
    cases.append(("8 invented Architect decision record (a C7 ruling file that does not exist)", d,
                  "provenance: every ARCHITECT-RULING record named exists in Git"))
    lines, allok = [], True
    for name, d, must_fail in cases:
        res = check(d)
        failed = [l for l, ok, _ in res if not ok]
        rejected = must_fail in failed
        allok &= rejected
        lines.append(f"{'REJECTED' if rejected else 'NOT REJECTED'}  {name}: targeted check failed = {rejected}; "
                     f"failed checks {len(failed)} of {len(res)}: {failed}")
    lines.append(f"\nRESULT: {sum(l.startswith('REJECTED') for l in lines)} of {len(cases)} negative controls rejected")
    open(outp, "x").write("\n".join(lines) + "\n")
    print("\n".join(lines))
    return 0 if allok else 1


if __name__ == "__main__":
    if sys.argv[1] == "check":
        res = check(sys.argv[2])
        fails = write(res, sys.argv[3], f"FINAL CONSISTENCY CHECK of {sys.argv[2]}")
        print(f"{len(res) - len(fails)} of {len(res)} hold")
        for l, ok, d in fails:
            print("FAIL", l, "::", d)
        sys.exit(0 if not fails else 1)
    sys.exit(negatives(sys.argv[2], sys.argv[3]))
