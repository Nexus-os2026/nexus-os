#!/usr/bin/env python3
"""Stage 1 — 35/35 mutation decision-to-evidence ledger. Read-only.

Inputs: Git objects at the evidence head (cca0d773, an evidence-only descendant of Candidate C) and, read-only, the
pinned mutant executables still on disk (hashed, never executed). Patches are applied only to throw-away copies under
the session scratch directory given as argv[1]; nothing is written to any worktree or to the object store.

Outputs (argv[2] = output directory): 01-MUTATION-DECISIONS.tsv, s1-ledger.json, 01-CAMPAIGN-CHECKS.txt.
"""
import json
import os
import re
import shutil
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from common import *  # noqa

SCRATCH, OUT = sys.argv[1], sys.argv[2]
CAT = f"{EVB}/d2a1-control-definitions-r2-01/catalog/catalog.json"
K1 = ("1918374263740d2f98640da00fc466737945975e", "d6c05a7cdc71b3ca1812b5199b8529cbe6db0363ca11bacc26d1f21c2cd11a83")

RULINGS = [  # (record path, reviewed evidence commit named in the record) — the nine by-ID acceptance records
    f"{EVB}/d2a1-mutation-tranche1-resume-01/ARCHITECT-RULING-C22-ACCEPTANCE.md",
    f"{EVB}/d2a1-mutation-tranche1-continue-02/ARCHITECT-RULING-TRANCHE1-PARTIAL-ACCEPTANCE.md",
    f"{EVB}/d2a1-mutation-tranche2-exec-01/documents/ARCHITECT-RULING-TRANCHE1-FINAL-ACCEPTANCE.md",
    f"{EVB}/d2a1-mutation-tranche2-continue-02/documents/ARCHITECT-RULING-TRANCHE2-PARTIAL-ACCEPTANCE.md",
    f"{EVB}/d2a1-mutation-tranche3-exec-01/documents/ARCHITECT-RULING-TRANCHE2-CONTINUATION-ACCEPTANCE.md",
    f"{EVB}/d2a1-mutation-tranche3-continue-02/documents/ARCHITECT-RULING-TRANCHE3-C2-ACCEPTANCE.md",
    f"{EVB}/d2a1-mutation-tranche3-tail-01/documents/ARCHITECT-RULING-TRANCHE3-CONTINUATION-ACCEPTANCE.md",
    f"{EVB}/d2a1-mutation-witness-c1-c25-01/documents/ARCHITECT-RULING-TRANCHE3-TAIL-ACCEPTANCE.md",
    f"{EVB}/d2a1-mutation-witness-c4-01/documents/ARCHITECT-RULING-C1-C25-WITNESS-ACCEPTANCE.md",
]
C4_DIR = f"{EVB}/d2a1-mutation-witness-c4-01/receipts/81-C4"
C7_DIR = f"{EVB}/d2a1-c7-compliant-exec-02"
C7_HIST = f"{EVB}/d2a1-mutation-tranche3-continue-02/receipts/85-C7"
UNIT = {"C14", "C19", "C21", "C27", "C28"}
# accepted runs made on a preserved, Architect-admitted earlier build: apply/build/restore receipts are in the earlier packet
BUILD_FROM = {"C28": f"{EVB}/d2a1-mutation-tranche1-resume-01/receipts/36-C28",
              "C5": f"{EVB}/d2a1-mutation-tranche2-exec-01/receipts/56-C5",
              "C24": f"{EVB}/d2a1-mutation-tranche3-exec-01/receipts/67-C24"}
ADMISSION_RULINGS = {"C28": f"{EVB}/d2a1-mutation-tranche1-continue-02/ARCHITECT-RULING-C28-BUILD-ADMISSIBILITY.md",
                     "C5": f"{EVB}/d2a1-mutation-tranche2-continue-02/documents/ARCHITECT-RULING-C5-BUILD-ADMISSIBILITY.md",
                     "C24": f"{EVB}/d2a1-mutation-tranche3-continue-02/documents/ARCHITECT-RULING-C24-C3B-C3C-BUILD-ADMISSIBILITY.md"}
WITNESS = {"C1", "C25", "C4", "C7"}

checks = []


def check(label, ok, detail=""):
    checks.append((label, bool(ok), str(detail)))
    return ok


def first_line(b):
    return b.decode(errors="replace").splitlines()[0] if b else ""


# ------------------------------------------------------------------------------------------------------ catalog
cat_bytes = show(HEAD_EV, CAT)
check("K1 catalog blob at evidence head", blob_id(HEAD_EV, CAT) == K1[0], blob_id(HEAD_EV, CAT))
check("K1 catalog sha256", sha(cat_bytes) == K1[1], sha(cat_bytes))
catalog = json.loads(cat_bytes)
ctl = {c["id"]: c for c in catalog["controls"]}
ids35 = [i for i in catalog["ids_in_contract_order"] if i != "A4"] if "ids_in_contract_order" in catalog else \
    [i for i in ctl if i != "A4"]
check("catalog: exactly 35 kill-required controls (A4 excluded)", len(ids35) == 35 and len(set(ids35)) == 35,
      f"{len(ids35)}: {' '.join(ids35)}")
check("catalog: A4 present and not kill-required", "A4" in ctl, ctl.get("A4", {}).get("named_property_text"))

# ------------------------------------------------------------------------------------------------------ rulings
ROW = re.compile(r"^\|\s*(C\d+[abc]?|A\d)\s*\|.*?`(docs/evidence/[^`]+/RESULT\.txt)`\s*\|\s*`([0-9a-f]{40})`\s*\|\s*`([0-9a-f]{64})`")
ACC = re.compile(r"(C\d+[abc]?|A\d)\b[^|\n]*?(?:—|-)?\s*(?:ACCEPTED MUTATION CONTROL KILL|MUTATION CONTROL KILL ACCEPTED)")
rulings = []
cited = {}       # control -> list of (record, path, blob, sha)
accepted_in = {}  # control -> list of records that state its acceptance
for rp in RULINGS:
    b = show(HEAD_EV, rp)
    t = b.decode()
    rec = {"path": rp, "blob": blob_id(HEAD_EV, rp), "sha256": sha(b), "title": t.splitlines()[0]}
    # the acceptance list stated by the record: the quoted ruling block plus the record's title
    quoted = "\n".join(l[1:].strip() for l in t.splitlines() if l.startswith(">"))
    names = set()
    for m in re.finditer(r"(?m)^(C\d+[abc]?|A\d)\s*(?:—|-|:)\s*(?:ACCEPTED MUTATION CONTROL KILL|MUTATION CONTROL KILL ACCEPTED)",
                         quoted):
        names.add(m.group(1))
    m = re.search(r"(?m)^Accepted(?: controls)?:\s*(.+)$", quoted)
    if m:
        names |= set(re.findall(r"\b(?:C\d+[abc]?|A\d)\b", m.group(1)))
    m = re.search(r"ACCEPTS these mutation-control kills:\s*(.+)", quoted)
    if m:
        names |= set(re.findall(r"\b(?:C\d+[abc]?|A\d)\b", m.group(1)))
    m = re.search(r"(?m)^accepts:\s*$\n+^(C\d+[abc]?|A\d)", quoted)
    if m:
        names.add(m.group(1))
    # C1 / C25 record: "C1:\nACCEPTED MUTATION CONTROL KILL"
    for mm in re.finditer(r"(?m)^(C\d+[abc]?|A\d):\s*\n+ACCEPTED MUTATION CONTROL KILL", quoted):
        names.add(mm.group(1))
    title_ids = set()
    if "KILL ACCEPTED" in rec["title"]:
        title_ids = set(re.findall(r"\b(?:C\d+[abc]?|A\d)\b", rec["title"].split("KILL ACCEPTED")[0]))
    rec["title_accepted"] = sorted(title_ids)
    earlier = set(accepted_in)
    check(f"ruling {rec['blob'][:8]}: title list contains the quoted ruling's list; any extra id was accepted by an earlier record",
          names <= title_ids and (title_ids - names) <= earlier if names else bool(title_ids),
          f"title {sorted(title_ids)} quoted {sorted(names)} earlier {sorted(earlier)}")
    names |= title_ids
    rec["accepted_stated"] = sorted(names)
    rec["recordings"] = re.findall(r"(?m)^((?:C\d+[abc]?|A\d): P\d+ \(first failure [A-Z]\d+; witness\))$", quoted)
    m = re.search(r"(\d+) (?:OF|of) 35", rec["title"]) or re.search(r"(\d+)/35", quoted)
    rec["count_stated"] = m.group(1) if m else None
    for l in t.splitlines():
        mm = ROW.match(l)
        if mm:
            cited.setdefault(mm.group(1), []).append((rp, mm.group(2), mm.group(3), mm.group(4)))
    for n in names:
        accepted_in.setdefault(n, []).append(rp)
    rulings.append(rec)

# C22's RESULT.txt is cited in a different table form in the partial record; the final Tranche-1 record carries it
union = sorted(set(accepted_in))
check("ruling records: the union of controls stated accepted is exactly 33",
      len(union) == 33, f"{len(union)}: {' '.join(union)}")
check("ruling records: C4 never stated accepted by ID", "C4" not in accepted_in)
check("ruling records: C7 never stated accepted by ID", "C7" not in accepted_in)
check("ruling records: A4 never in a kill acceptance list", "A4" not in accepted_in)
check("ruling records: 33 + {C4, C7} = the catalog's 35", sorted(union + ["C4", "C7"]) == sorted(ids35))

# ------------------------------------------------------------------------------------------------------ helpers


def derive_mutant(cid):
    """Apply the catalog patch to a throw-away copy of Candidate C's changed file(s); return {path: (blob, sha)}."""
    c = ctl[cid]
    patch = show(HEAD_EV, f"{EVB}/d2a1-control-definitions-r2-01/catalog/patches/{cid}.patch")
    d = os.path.join(SCRATCH, f"derive-{cid}")
    if os.path.exists(d):
        shutil.rmtree(d)
    os.makedirs(d)
    files = re.findall(r"(?m)^\+\+\+ b/(\S+)$", patch.decode())
    for f in files:
        os.makedirs(os.path.join(d, os.path.dirname(f)), exist_ok=True)
        open(os.path.join(d, f), "wb").write(show(C, f))
    open(os.path.join(d, "p.patch"), "wb").write(patch)
    r = subprocess.run(["git", "apply", "--whitespace=nowarn", "p.patch"], cwd=d, capture_output=True)
    res = {}
    if r.returncode == 0:
        for f in files:
            b = open(os.path.join(d, f), "rb").read()
            res[f] = (git_blob_of_bytes(b), sha(b))
    shutil.rmtree(d)
    return sha(patch), res, r.returncode


def disk_hash(path):
    try:
        st = os.stat(path)
        return file_sha(path), st.st_size
    except OSError as e:
        return f"ABSENT ({e.__class__.__name__})", None


def rel(p):
    return p.replace(f"{EVB}/", "")


rows = []
ledger = {}
for cid in ids35:
    c = ctl[cid]
    L = {"id": cid, "description": c.get("description"), "named_property": c.get("named_property_text"),
         "configuration": c.get("configuration"), "kind": c.get("kind"), "contract_row": c.get("contract_row"),
         "mutation_source_file": c.get("mutation_source_file"),
         "catalog_mutant_blob": c.get("mutant_blob"), "catalog_mutant_sha256": c.get("mutant_sha256")}
    psha, derived, rc = derive_mutant(cid)
    L["catalog_patch_sha256"] = psha
    L["derived_mutant"] = {k: list(v) for k, v in derived.items()}
    check(f"{cid}: catalog patch applies to Candidate C (scratch copy)", rc == 0, rc)
    dv = list(derived.values())[0] if derived else (None, None)
    check(f"{cid}: re-derived mutant blob/sha256 equal the catalog's", dv == (c.get("mutant_blob"), c.get("mutant_sha256")),
          f"{dv} vs {(c.get('mutant_blob'), c.get('mutant_sha256'))}")

    if cid == "C7":
        L.update(route="AD-C7-W witness route (exec-02)")
        a = jshow(HEAD_EV, f"{C7_DIR}/receipts/41-run-A-full_fpt/CLASSIFICATION.json")
        bw = jshow(HEAD_EV, f"{C7_DIR}/receipts/42-run-B-mutant-witness/CLASSIFICATION.json")
        cw = jshow(HEAD_EV, f"{C7_DIR}/receipts/40-run-C-healthy-witness/CLASSIFICATION.json")
        fin = show(HEAD_EV, f"{C7_DIR}/receipts/70-final/FINAL-CLASSIFICATION.txt").decode()
        link = show(HEAD_EV, f"{C7_DIR}/receipts/60-link/LINK.txt").decode()
        rst = show(HEAD_EV, f"{C7_DIR}/receipts/50-restoration/restoration.txt").decode() \
            if exists(HEAD_EV, f"{C7_DIR}/receipts/50-restoration/restoration.txt") else ""
        raws = {}
        for k, d in (("A", "41-run-A-full_fpt"), ("B", "42-run-B-mutant-witness"), ("C", "40-run-C-healthy-witness"),
                     ("F3", "43-run-F3-informational")):
            rb = show(HEAD_EV, f"{C7_DIR}/receipts/{d}/raw-output.txt")
            raws[k] = sha(rb)
        braw = show(HEAD_EV, f"{C7_DIR}/receipts/42-run-B-mutant-witness/raw-output.txt").decode()
        vl = [l.strip() for l in braw.splitlines() if l.strip().startswith("VIOLATION ")]
        distinct = list(dict.fromkeys(vl))
        v13 = [l for l in distinct if l.startswith("VIOLATION step 13 transition: E11")]
        v24 = [l for l in distinct if l.startswith("VIOLATION after 24: P6")]
        check("C7: the mutant witness prints exactly two distinct canonical VIOLATION records, E11 first, P6 second",
              distinct == v13 + v24 and len(distinct) == 2, distinct)
        araw = show(HEAD_EV, f"{C7_DIR}/receipts/41-run-A-full_fpt/raw-output.txt").decode()
        arep = parse_reports(araw)
        L.update(primary_receipt=f"{rel(C7_DIR)}/receipts/41-run-A-full_fpt/raw-output.txt",
                 primary_raw_sha256=raws["A"], primary_outcome=f"{a.get('outcome')} {a.get('fail', [''])[0]}",
                 primary_states=a.get("states"), primary_verdict=a.get("verdict"),
                 witness_receipt=f"{rel(C7_DIR)}/receipts/42-run-B-mutant-witness/raw-output.txt",
                 witness_raw_sha256=raws["B"], witness_verdict=bw.get("verdict"),
                 witness_lines=v13 + v24,
                 healthy_receipt=f"{rel(C7_DIR)}/receipts/40-run-C-healthy-witness/raw-output.txt",
                 healthy_raw_sha256=raws["C"], healthy_verdict=cw.get("verdict"),
                 informational_f3_raw_sha256=raws["F3"],
                 final_classification=[l for l in fin.splitlines() if "STAGE 12 RESULT" in l],
                 link=[l for l in link.splitlines() if "LINK USABLE" in l],
                 restoration=[l for l in rst.splitlines() if "EXACTLY_W" in l][:2],
                 first_failure=(a.get("fail") or [None])[0], receipt_type="(a) primary + (c) mutant witness + (e3) healthy witness (AD-C7-W G.7/G.9)",
                 mutant_exe_sha256="156cf0cc27ac1ed50d31e3a2a2adc924211da2a2be77bc6f617f774f27e04a38",
                 healthy_exe_sha256="e8b4e986b36d31c313d307991c51b3a7f8b3cc169a39afc459ee49974313b843")
        check("C7: run A is the original explorer on F-PT, A4 on, FAIL E11, admissible",
              a.get("verdict") == "FAIL_E11_ADMISSIBLE" and arep and arep[0]["config"] == "F-PT"
              and "A4 true" in arep[0].get("bounds", "") and arep[0].get("fail", "").startswith("E11:"),
              a.get("verdict"))
        check("C7: run A printed no P6 failure (no original F-PT P6 is claimed)",
              not any(r.get("fail", "").startswith("P6") for r in arep))
        check("C7: mutant witness: E11 at step-13 transition, then P6 after step 24; ADMISSIBLE_TWO_CANONICAL",
              len(v13) == 1 and len(v24) == 1 and bw.get("verdict") == "ADMISSIBLE_TWO_CANONICAL", bw.get("verdict"))
        check("C7: healthy witness PASS_ADMISSIBLE", cw.get("verdict") == "PASS_ADMISSIBLE", cw.get("verdict"))
        check("C7: Stage 12 KILL_EVIDENCE_CANDIDATE (executor maximum; not acceptance)",
              any("KILL_EVIDENCE_CANDIDATE" in l for l in L["final_classification"]), L["final_classification"])
        check("C7: restoration EXACTLY_W recorded (receipts/50-restoration/restoration.txt)", "EXACTLY_W" in rst, rst[:200])
        check("C7: link = Candidate C base PASS on E-C, not a W-SRC PASS (CR-B)",
              any("C-BASE PASS on E-C; NOT W-SRC PASS" in l for l in L["link"]), L["link"])
        for role, s in (("mutant E-W7", L["mutant_exe_sha256"]), ("healthy E-W", L["healthy_exe_sha256"])):
            p = ("/home/nexus/NEXUS/nexus-os-p3-c11-c7w02-mutant" if "W7" in role else
                 "/home/nexus/NEXUS/nexus-os-p3-c11-c7w02-healthy") + "/target/release/deps/nexus_governed_control-2faa0b9718d3e6a3"
            h, size = disk_hash(p)
            L[f"on_disk_{role.split()[0]}"] = h
            check(f"C7: {role} executable on disk hashes to its pin (hashed, not run)", h == s, h)
        L["restored_side"] = "(CR-B link) " + "; ".join(L["link"])
        L["restoration"] = "EXACTLY_W" if "RESTORATION COMPLETE: EXACTLY_W" in rst else "NOT ESTABLISHED"
        L["mutant_blob"], L["mutant_sha256"] = list(derived.values())[0] if derived else (None, None)
        L["primary_raw_blob"] = blob_id(HEAD_EV, L["primary_receipt"].replace("d2a1-c7", f"{EVB}/d2a1-c7", 1))
        L["decision"] = ("ACCEPTED — conveyed in mission P3-C11-DR01-D2A1-FINAL-CLOSURE-01 section 1 as ruling "
                         "P3-C11-DR01-D2A1-C7-ACCEPTANCE-01 (conversational; no Git ruling record exists)")
        L["recording"] = "C7: P6 (first failure E11; witness)"
        L["decision_provenance"] = "CONVERSATIONAL_ARCHITECT_RULING (mission text only); no by-ID Git record"
        L["amendment"] = "AD-C7-W (Owner adopted 2026-10-15; identity approval 2026-10-16), CR-B B-W/A"
        L["historical"] = f"{rel(C7_HIST)}/RESULT.txt (MUTATION_CONTROL_BLOCKED, E11 on E-C-based mutant; preserved, not the accepted route)"
        ledger[cid] = L
        continue

    # ---- tranche / witness controls
    if cid == "C4":
        rdir = C4_DIR
        rb = show(HEAD_EV, f"{rdir}/RESULT.txt")
        L.update(result_txt=f"{rel(rdir)}/RESULT.txt", result_blob=blob_id(HEAD_EV, f"{rdir}/RESULT.txt"),
                 result_sha256=sha(rb), ruling_cites=[])
    else:
        cs = cited.get(cid)
        if not check(f"{cid}: at least one ruling record cites its RESULT.txt with blob and sha256", bool(cs)):
            ledger[cid] = L
            continue
        last = cs[-1]
        rdir = os.path.dirname(last[1])
        rb = show(HEAD_EV, last[1])
        L.update(result_txt=rel(last[1]), result_blob=blob_id(HEAD_EV, last[1]), result_sha256=sha(rb),
                 ruling_cites=[(rel(r), b, s) for r, _, b, s in cs])
        for r, p, b, s in cs:
            check(f"{cid}: cited RESULT.txt blob/sha256 equal the Git object ({os.path.basename(r)})",
                  blob_id(HEAD_EV, p) == b and sha(show(HEAD_EV, p)) == s, f"{b} {s[:12]}")
        if len({p for _, p, _, _ in cs}) > 1:
            check(f"{cid}: every citing record names the same RESULT.txt", False, [p for _, p, _, _ in cs])
    L["result_first_line"] = first_line(rb)
    L["receipt_dir"] = rel(rdir)
    bdir = BUILD_FROM.get(cid, rdir)
    if cid in BUILD_FROM:
        adm = ADMISSION_RULINGS[cid]
        L["build_receipt_dir"] = rel(bdir)
        L["build_admission_ruling"] = (rel(adm), blob_id(HEAD_EV, adm), first_line(show(HEAD_EV, adm)))
        L["build_packet_result_first_line"] = first_line(show(HEAD_EV, f"{bdir}/RESULT.txt"))
        check(f"{cid}: preserved-build admission ruling record present", exists(HEAD_EV, adm), adm)
    ap = jshow(HEAD_EV, f"{bdir}/02-apply/APPLY.json")
    vm = ap.get("verify_mutant", {})
    mf = vm.get("mutant_file", {})
    L.update(apply_patch_sha256=ap.get("patch_sha256"), apply_catalog_patch_sha256=ap.get("catalog_patch_sha256"),
             apply_verdict=vm.get("verdict"), mutant_blob=mf.get("git_blob"), mutant_sha256=mf.get("sha256"),
             mutant_head=vm.get("head"), mutant_tree=vm.get("tree"))
    check(f"{cid}: applied patch sha256 = catalog patch sha256 = frozen patch blob", ap.get("patch_sha256") == psha
          and ap.get("catalog_patch_sha256") == psha, ap.get("patch_sha256"))
    check(f"{cid}: mutant built from C (head/tree) plus the patch: EXACTLY_C_PLUS_PATCH",
          vm.get("verdict") == "EXACTLY_C_PLUS_PATCH" and vm.get("head") == C and vm.get("tree") == C_TREE, vm.get("verdict"))
    check(f"{cid}: recorded mutant blob/sha256 = independently re-derived", (mf.get("git_blob"), mf.get("sha256")) == dv,
          f"{mf.get('git_blob')}")
    if cid == "C22":
        pr = jshow(HEAD_EV, f"{rdir}/03-build/PARSED-RECEIPT.json")
        stderr = show(HEAD_EV, f"{rdir}/03-build/build.stderr") if exists(HEAD_EV, f"{rdir}/03-build/build.stderr") else b""
        e0283 = [l for l in stderr.decode(errors="replace").splitlines() if l.startswith("error[")]
        loc = [l.strip() for l in stderr.decode(errors="replace").splitlines() if "unit_tests.rs:1089:5" in l]
        L.update(receipt_type="(d) intentional mutant compile failure", primary_receipt=f"{rel(rdir)}/03-build/build.stderr",
                 primary_raw_sha256=sha(stderr) if stderr else "NOT IN PACKET",
                 first_failure="E0283 at unit_tests.rs:1089:5" if e0283 == ["error[E0283]: type annotations needed"] and loc else str(e0283),
                 mutant_exe_sha256="NOT PRODUCED BY DESIGN", exit_status=pr.get("process", {}).get("exit_status"))
        check("C22: exactly one coded error, E0283, at unit_tests.rs:1089:5; exit 101 by a normal exit",
              e0283 == ["error[E0283]: type annotations needed"] and bool(loc)
              and pr.get("process", {}).get("exit_status") == 101 and pr.get("process", {}).get("signal") is None, e0283)
        rs = jshow(HEAD_EV, f"{rdir}/05-restore/RESTORE.json")
        L["restoration"] = (f"historical tool label {first_line(show(HEAD_EV, rdir + '/05-restore/RESTORE.txt'))!r}; "
                            f"protocol.rs after = {rs.get('after', {}).get('git_blob')}; Architect independently verified "
                            f"restoration to C (C22 ruling record)")
        check("C22: restored protocol.rs blob = Candidate C's", rs.get("after", {}).get("git_blob") == SRC_BLOBS[
            "crates/nexus-governed-control/src/ledger/protocol.rs"], rs.get("after"))
        hb = f"{EVB}/d2a1-mutation-tranche1-exec-01/receipts/20-healthy-build/CLASSIFICATION.txt"
        L["restored_side"] = f"(e4) {rel(hb)}: {first_line(show(HEAD_EV, hb)) if exists(HEAD_EV, hb) else 'MISSING'}"
        check("C22: linked healthy type-(e4) build classification PASS", exists(HEAD_EV, hb) and
              first_line(show(HEAD_EV, hb)).startswith("CLASSIFICATION: PASS"), L["restored_side"])
    else:
        pinp = [p for p in (f"{rdir}/03-build/PIN.json", f"{rdir}/02-pin/PIN.json", f"{rdir}/03-pin/PIN.json")
                if exists(HEAD_EV, p)]
        pin = jshow(HEAD_EV, pinp[0])
        if cid in BUILD_FROM:
            bpin = f"{bdir}/03-build/PIN.json"
            if exists(HEAD_EV, bpin):
                check(f"{cid}: the continuation's pin equals the earlier build's pin", jshow(HEAD_EV, bpin).get("sha256") == pin.get("sha256"))
        L.update(mutant_exe_path=pin.get("path"), mutant_exe_sha256=pin.get("sha256"), mutant_exe_size=pin.get("size"))
        check(f"{cid}: pinned mutant executable built from the recorded mutant blob",
              (pin.get("mutant_blob") in (mf.get("git_blob"), None)) and
              (pin.get("source_verdict") in ("EXACTLY_C_PLUS_PATCH", None)), {k: pin.get(k) for k in ("mutant_blob", "source_verdict")})
        check(f"{cid}: pinned mutant executable is not the baseline E-C", pin.get("sha256") != E_C)
        h, size = disk_hash(pin.get("path"))
        L["mutant_exe_on_disk"] = h
        check(f"{cid}: pinned mutant executable still on disk with its pinned sha256 (hashed, not run)",
              h == pin.get("sha256"), h)
        raw = show(HEAD_EV, f"{rdir}/04-run/raw-output.txt")
        pr = jshow(HEAD_EV, f"{rdir}/04-run/PARSED-RECEIPT.json")
        L.update(primary_receipt=f"{rel(rdir)}/04-run/raw-output.txt", primary_raw_sha256=sha(raw),
                 primary_raw_blob=blob_id(HEAD_EV, f"{rdir}/04-run/raw-output.txt"),
                 exit_status=pr.get("process", {}).get("exit_status"),
                 run_exe_before=pr.get("executable", {}).get("before", {}).get("sha256"),
                 run_exe_after=pr.get("executable", {}).get("after", {}).get("sha256"))
        check(f"{cid}: run used the pinned mutant executable, equal before and after",
              L["run_exe_before"] == L["run_exe_after"] == pin.get("sha256"))
        check(f"{cid}: primary run exited 101 by a normal exit (no signal), supervisor completed, no OOM/timeout",
              pr.get("process", {}).get("exit_status") == 101 and pr.get("process", {}).get("signal") is None
              and pr.get("supervisor", {}).get("completed") and not pr.get("supervisor", {}).get("timed_out")
              and pr.get("supervisor", {}).get("oom_kill") in (0, "0"), pr.get("process"))
        if cid in UNIT:
            text = raw.decode(errors="replace")
            m_f = re.search(r"(?ms)^failures:\n((?:    \S+\n)+)", text)
            failed = m_f.group(1).split() if m_f else []
            pan = re.findall(r"panicked at (crates/nexus-governed-control/src/ledger/protocol/unit_tests\.rs:\d+:\d+)", text)
            L.update(receipt_type="(b) named non-explorer mutant-test FAIL", first_failure=f"{failed} at {pan}")
            want = c.get("controls_defs_target", {}).get("target")
            check(f"{cid}: exactly the catalog's named unit test failed ({want}), panicking at a unit_tests.rs check",
                  failed == [want] and len(pan) == 1, f"{failed} {pan}")
            check(f"{cid}: harness '1 failed'", "test result: FAILED. 0 passed; 1 failed" in text)
        else:
            reps = parse_reports(raw.decode(errors="replace"))
            rep = reps[0] if reps else {}
            fp = rep.get("fail", "").split(":")[0]
            L.update(receipt_type="(a) original-explorer mutant FAIL", first_failure=fp, primary_config=rep.get("config"),
                     primary_states=rep.get("states"), primary_expanded=rep.get("expanded"), primary_fail=rep.get("fail"),
                     primary_header=rep.get("header"))
            ceil = c.get("ceiling_states")
            check(f"{cid}: primary is the original explorer (no quotient/symmetry line) on {c.get('configuration')}",
                  rep.get("config") == c.get("configuration") and rep.get("quotient") is None and rep.get("symmetry") is None,
                  rep.get("header"))
            check(f"{cid}: discovered count within the configuration's ceiling ({ceil})",
                  rep.get("states") is not None and ceil and rep["states"] <= ceil, rep.get("states"))
            named = c.get("named_property") or []
            if cid in ("C1", "C25", "C4"):
                exp = {"C1": "E3", "C25": "E4", "C4": "P4"}[cid]
                check(f"{cid}: primary first failure printed as {exp} (recorded as printed, never as the named property)",
                      fp == exp, fp)
            else:
                check(f"{cid}: printed first failure is a named property {named}", fp in named, fp)
        rsp = f"{rdir}/05-restore/RESTORE.json" if exists(HEAD_EV, f"{rdir}/05-restore/RESTORE.json") else f"{bdir}/05-restore/RESTORE.json"
        rs = jshow(HEAD_EV, rsp)
        L["restoration"] = rs.get("verify_tree", {}).get("verdict") or first_line(show(HEAD_EV, rsp.replace(".json", ".txt"))).replace("RESTORE: ", "")
        L["restoration_receipt"] = rel(rsp)
        if exists(HEAD_EV, f"{rdir}/05-reauth/REAUTH.txt"):
            L["reauth_after_run"] = first_line(show(HEAD_EV, f"{rdir}/05-reauth/REAUTH.txt"))
            check(f"{cid}: re-authentication after the run holds", "HOLDS" in L["reauth_after_run"], L["reauth_after_run"])
        after_blob = (rs.get("after") or rs.get("file") or {}).get("git_blob")
        check(f"{cid}: restoration EXACTLY_C (blob equality to C, clean)", L["restoration"] == "EXACTLY_C"
              and after_blob == SRC_BLOBS[c.get("mutation_source_file")], f"{L['restoration']} {after_blob}")
        lp = f"{rdir}/06-link/LINK.json" if exists(HEAD_EV, f"{rdir}/06-link/LINK.json") else f"{rdir}/06b-link/LINK.json"
        lk = jshow(HEAD_EV, lp)
        L["restored_side"] = f"({lk.get('type')}) {lk.get('reference')}"
        check(f"{cid}: restored-side link verified, every check holds", all(x[1] for x in lk.get("checks", [])),
              [x[0] for x in lk.get("checks", []) if not x[1]])
        if cid == "C15":
            ref = lk.get("reference", "")
            check("C15: restored side cites the AD-1-R5 F refinement discharge record (not a run, never PASS), bound to C, F-2 and the proof",
                  "refinement discharge record" in ref and "never PASS" in ref and C in ref and "41-full-F-2" in ref
                  and "c97332a33e68f3fecae12dc0ed863b24fbc1f8bffdbf67430765fd3b5281606b" in ref, ref[:120])
        if cid in ("C1", "C25", "C4"):
            wr = show(HEAD_EV, f"{rdir}/04c-witness/raw-output.txt")
            wp = jshow(HEAD_EV, f"{rdir}/04c-witness/PARSED-RECEIPT.json")
            hj = jshow(HEAD_EV, f"{rdir}/06a-healthy-witness/HEALTHY.json")
            gj = jshow(HEAD_EV, f"{rdir}/07-g3/G3.json" if cid != "C4" else f"{rdir}/07-g5/G5.json")
            pos = [l for l in wr.decode(errors="replace").splitlines() if "NAMED PROPERTY" in l and "OBSERVED" in l
                   and "NOT OBSERVED" not in l]
            L.update(witness_receipt=f"{rel(rdir)}/04c-witness/raw-output.txt", witness_raw_sha256=sha(wr),
                     witness_lines=pos, healthy_reference=hj.get("reference"),
                     g_rule_holds=gj.get("holds"), g_recording_proposed=gj.get("recording_proposed_for_architect_review"),
                     witness_exe_before=wp.get("executable", {}).get("before", {}).get("sha256"))
            check(f"{cid}: mutant witness on the same pinned mutant executable", L["witness_exe_before"] == pin.get("sha256"))
            check(f"{cid}: healthy witness link verified, every check holds", all(x[1] for x in hj.get("checks", [])))
            gh = gj.get("holds") if cid != "C4" else gj.get("holds_A_to_D_and_E_identity")
            L["g_rule_holds"] = gh
            if cid == "C4":
                L["g5_E_architect_review_at_execution"] = gj.get("E_architect_review")
                L["g5_first_printed_property"] = gj.get("first_printed_property")
            check(f"{cid}: {'G.3' if cid != 'C4' else 'G.5 (A)-(D) and (E) identity'} criterion record holds", gh is True, gh)
            if cid == "C4":
                check("C4: witness shows both P4 and P8 OBSERVED", any("P4" in l for l in pos) and any("P8" in l for l in pos), pos)
            else:
                want = {"C1": "P1", "C25": "P7"}[cid]
                check(f"{cid}: witness shows {want} OBSERVED", any(f"PROPERTY {want}:" in l for l in pos), pos)
    # ---- decision
    if cid == "C4":
        L["decision"] = ("ACCEPTED — conveyed in mission P3-C11-DR01-D2A1-FINAL-CLOSURE-01 section 1 ('Previously "
                         "Architect accepted'; conversational checkpoint: P4 and P8, first failure P4); no by-ID Git record")
        L["recording"] = "C4: P4, P8 (first failure P4; witness)"
        L["decision_provenance"] = ("CONVERSATIONAL_ARCHITECT_RULING; indirect Git corroboration only: executor-written "
                                    "AD-C7-W adoption record (2026-10-15) states 'Mutation campaign: 34/35 ARCHITECT "
                                    "ACCEPTED' with C7 blocked")
        L["amendment"] = "AD-1-R4 G.5, G.6, S-22 (both P4 and P8 required)"
    else:
        recs = accepted_in.get(cid, [])
        L["decision"] = "MUTATION CONTROL KILL ACCEPTED"
        L["decision_records"] = [(rel(r), blob_id(HEAD_EV, r)) for r in recs]
        L["recording"] = {"C1": "C1: P1 (first failure E3; witness)", "C25": "C25: P7 (first failure E4; witness)"}.get(cid, "")
        L["decision_provenance"] = ("EXECUTOR-WRITTEN GIT RECORD of the Architect ruling as conveyed in the named "
                                    "mission's section 1; unsigned; a string is not authority")
        am = ["AD-1-R4 F, F.1, I, K.2"]
        if cid in ("C1", "C25"):
            am.append("AD-1-R4 G.3 (C1/C25 only)")
        if cid == "C15":
            am.append("AD-1-R5 F (restored side only)")
        if cid == "C9":
            am.append("AD-1-R4 S-23 (C9-b)")
        if cid in ("C10", "C24"):
            am.append(f"AD-1-R4 {'S-24' if cid == 'C10' else 'S-25'} (sealed definition)")
        if cid == "C22":
            am.append("contract line 731 exception; AD-1-R4 F.1 (d)/(e4)")
        L["amendment"] = "; ".join(am)
        check(f"{cid}: stated accepted by at least one by-ID ruling record", bool(recs))
    ledger[cid] = L

# recordings stated in the C1/C25 record
c1c25 = [r for r in rulings if r["path"].endswith("ARCHITECT-RULING-C1-C25-WITNESS-ACCEPTANCE.md")][0]
check("C1/C25 record: recordings 'C1: P1 (first failure E3; witness)' and 'C25: P7 (first failure E4; witness)'",
      sorted(c1c25["recordings"]) == ["C1: P1 (first failure E3; witness)", "C25: P7 (first failure E4; witness)"],
      c1c25["recordings"])
check("ledger: exactly 35 distinct control ids", len(ledger) == 35 and set(ledger) == set(ids35))

# superseded (non-accepted) duplicate receipts — recorded, never relabelled
dups = {}
for cid in ids35:
    found = [p for p in git("ls-tree", "-r", "--name-only", HEAD_EV, "--", EVB).splitlines()
             if re.search(rf"/receipts/\d+-{re.escape(cid)}/RESULT\.txt$", p) and "/04b-render-rehearsal/" not in p]
    acc = ledger[cid].get("result_txt")
    others = [rel(p) for p in found if rel(p) != acc]
    if others:
        dups[cid] = [(o, first_line(show(HEAD_EV, EVB + "/" + o))) for o in others]
ledger["_superseded_receipts"] = dups
ledger["_rulings"] = rulings

# ------------------------------------------------------------------------------------------------------ outputs
os.makedirs(OUT, exist_ok=True)
cols = ["id", "description", "named_property", "configuration", "receipt_type", "first_failure", "recording",
        "catalog_patch_sha256", "mutant_blob", "mutant_sha256", "mutant_exe_sha256", "primary_receipt",
        "primary_raw_sha256", "witness_raw_sha256", "restored_side", "restoration", "amendment", "decision",
        "decision_provenance", "result_txt", "result_blob", "result_sha256"]
tsv = ["\t".join(cols)]
for cid in ids35:
    L = ledger[cid]
    if cid == "C7":
        L.setdefault("mutant_blob", L["derived_mutant"] and list(L["derived_mutant"].values())[0][0])
    tsv.append("\t".join(str(L.get(k, "")).replace("\t", " ").replace("\n", " ") for k in cols))
open(os.path.join(OUT, "01-MUTATION-DECISIONS.tsv"), "w").write("\n".join(tsv) + "\n")
json.dump(ledger, open(os.path.join(OUT, "s1-ledger.json"), "w"), indent=1, sort_keys=True)
fails = [c for c in checks if not c[1]]
with open(os.path.join(OUT, "01-CAMPAIGN-CHECKS.txt"), "w") as f:
    for label, ok, d in checks:
        f.write(f"{'OK  ' if ok else 'FAIL'} {label}" + (f" :: {d}" if d and not ok else "") + "\n")
    f.write(f"\nRESULT: {len(checks) - len(fails)} of {len(checks)} checks hold; {len(fails)} fail\n")
print(f"{len(checks) - len(fails)} of {len(checks)} hold")
for label, ok, d in fails:
    print("FAIL", label, "::", d)
sys.exit(0 if not fails else 1)
