#!/usr/bin/env python3
"""Builds the AD1R3 supplemental review packet in the mission worktree (P3-C11-DR01-D2A1-AD1R3-READINESS-01).
Copies only the files listed below from the mission evidence directory into one new prefix, writes INDEX.md and a
manifest PACKET-SHA256SUMS covering every packet file but itself, then verifies: every copy equals its source; the
prefix holds exactly the listed files plus INDEX.md and the manifest; total size <= 1 MiB; no NUL byte; no credential
pattern. Refuses if the prefix exists. Touches nothing outside the prefix. Runs nothing of Nexus.
    build-packet.py <evidence-dir> <worktree>"""
import hashlib, os, re, shutil, sys

EV, WT = sys.argv[1:3]
PREFIX = "docs/evidence/p3-candidate11/ad1r3-review-01"
DST = os.path.join(WT, PREFIX)
LIMIT = 1024 * 1024
FILES = [  # (packet path, source path relative to the evidence dir, description)
    ("ad1r3/P3-C11-D2A1-CONTRACT-02-R1-ADDENDUM-AD1-R3-DRAFT.md", "drafts/P3-C11-D2A1-CONTRACT-02-R1-ADDENDUM-AD1-R3-DRAFT.md",
     "AD-1-R3 DRAFT. NOT ACCEPTED, NOT ADOPTED, NOT APPLIED."),
    ("ad1r3/AD1-R2-to-AD1-R3.diff", "drafts/AD1-R2-to-AD1-R3.diff", "unified diff, AD-1-R2 (53cdb3d6…) to AD-1-R3"),
    ("preflight/C9-C10-C24-C4-ATTRIBUTION-PREFLIGHT.md", "preflight/C9-C10-C24-C4-ATTRIBUTION-PREFLIGHT.md",
     "read-only attribution preflight; predictions from source, nothing executed"),
    ("inert/controls_defs.py.txt", "inert/controls_defs.py.txt",
     "INERT byte copy of CAMPAIGN-01's sealed controls_defs.py; never imported or executed"),
    ("checks/01-ad1r3-mechanical-checks.txt", "checks/01-ad1r3-mechanical-checks.txt", "final gate G3: AD-1-R3 checker output"),
    ("checks/02-r2-to-r3-diff-review.txt", "checks/02-r2-to-r3-diff-review.txt", "executor's R2->R3 diff review; 30 scan items accounted for"),
    ("checks/03-defs-anchor-check.txt", "checks/03-defs-anchor-check.txt", "final gate G6: sealed definitions C9/C10/C24/C4 vs protocol.rs"),
    ("checks/04-preflight-checks.txt", "checks/04-preflight-checks.txt", "final gate G7: every preflight source citation checked"),
    ("checks/05-classification-cases.txt", "checks/05-classification-cases.txt", "final gate G5: K.2 classification cases (synthetic)"),
    ("checks/06-checker-negative-controls.txt", "checks/06-checker-negative-controls.txt", "final gate G4: checker negative controls"),
    ("checks/07-r2-r3-obligation-scan.txt", "checks/07-r2-r3-obligation-scan.txt", "final gate G8: R2 sentences not verbatim in R3"),
    ("receipts/00-STAGE1-AUTHENTICATION.txt", "receipts/00-STAGE1-AUTHENTICATION.txt", "provenance and sealed-input authentication"),
    ("receipts/01-worktree-authentication.txt", "receipts/01-worktree-authentication.txt", "mission worktree authentication"),
    ("receipts/02-development-log.txt", "receipts/02-development-log.txt", "every development run, failure and checker revision"),
    ("receipts/03-freeze.txt", "receipts/03-freeze.txt", "freeze record: frozen inputs and the declared gate"),
    ("receipts/04-final-gate.txt", "receipts/04-final-gate.txt", "the final mechanical gate: commands and exit codes"),
    ("receipts/github-refs-before.txt", "receipts/github-refs-before.txt", "git ls-remote github before any change"),
    ("tools/ad1r3-template.md", "tools/ad1r3-template.md", "the draft's template (placeholders for verbatim lines)"),
    ("tools/fill-ad1r3.py", "tools/fill-ad1r3.py", "fills verbatim blocks from the authenticated bytes"),
    ("tools/check-ad1r3.py", "tools/check-ad1r3.py", "independent mechanical checker (v3, frozen)"),
    ("tools/checker-negative-controls.py", "tools/checker-negative-controls.py", "seeded corruptions the checker must catch (v2, frozen)"),
    ("tools/classify-cases.py", "tools/classify-cases.py", "K.2 rules on synthetic receipt descriptors"),
    ("tools/defs-anchor-check.py", "tools/defs-anchor-check.py", "reads controls_defs.py with ast only; counts anchors"),
    ("tools/check-preflight.py", "tools/check-preflight.py", "preflight citation checker (v2, frozen)"),
    ("tools/r2-r3-obligation-scan.py", "tools/r2-r3-obligation-scan.py", "obligation-sentence scan (AD1R2's logic, unchanged)"),
    ("tools/final-gate.sh", "tools/final-gate.sh", "the declared final gate"),
    ("tools/stage1-auth.sh", "tools/stage1-auth.sh", "stage 1 authentication script"),
    ("tools/build-packet.py", "tools/build-packet.py", "this builder"),
    ("tools/revisions/check-ad1r3.v1.py", "tools/revisions/check-ad1r3.v1.py", "checker revision history: v1"),
    ("tools/revisions/check-ad1r3.v2.py", "tools/revisions/check-ad1r3.v2.py", "checker revision history: v2 (reconstructed; hash equals the logged one)"),
    ("tools/revisions/checker-negative-controls.v1.py", "tools/revisions/checker-negative-controls.v1.py", "negative controls v1"),
    ("tools/revisions/check-preflight.v1.py", "tools/revisions/check-preflight.v1.py", "preflight checker v1"),
]


def sha(b):
    return hashlib.sha256(b).hexdigest()


if os.path.exists(DST):
    sys.exit(f"REFUSED: {DST} exists")
for rel, src, _ in FILES:
    p = os.path.join(DST, rel)
    os.makedirs(os.path.dirname(p), exist_ok=True)
    shutil.copyfile(os.path.join(EV, src), p)

rows = []
for rel, src, desc in FILES:
    b = open(os.path.join(DST, rel), "rb").read()
    rows.append((rel, len(b), sha(b), desc))
draft = next(r for r in rows if r[0].endswith("AD1-R3-DRAFT.md"))
index = f"""# AD1R3 review packet — P3-C11-DR01-D2A1-AD1R3-READINESS-01

**Status: review materials only.** AD-1-R3 is a DRAFT — NOT ACCEPTED — NOT ADOPTED — NOT APPLIED. Nothing here is a
mutation kill, D2A-1 acceptance, adoption readiness or Phase Three completion.

Prepared by Claude Code (Claude Opus 5.5, `claude-opus-5-5`, effort high) for the Sole Architect's review.

## Identities

| Item | Identity |
|---|---|
| Engineering source | `42dd1bd1cf21ffdea5968b256d2983f6575fab5b` (tree `7ac6bb21430d15b17e3723b217b20a49dccf8282`); `protocol.rs` blob `a2df2841b99cad5d452008a6d4150e9369d07e66`; `model_tests.rs` blob `5ad26ddc727d8be30f4fb8d7478bc69e1c974933` |
| Parent of this packet's commit | evidence-transport commit `8693e3a742ee4d3917cbfdb809b377bcd919d9f4` (tree `1e9568ddb5a40336820d266652643d6067cd33f6`); not an engineering baseline |
| AD-1-R2 (history) | blob `8505fb80867f787e29a20d8f4f3b121cee77213b`, sha256 `53cdb3d6ea8683a11ccaf90a6671a37b42dd5db178b599cfeb421b898a089ebc`, at `docs/evidence/p3-candidate11/ad1r2-review-packet-01/ad1r2/` (unchanged) |
| AD-1-R3 (this packet) | `{draft[0]}`, sha256 `{draft[2]}`, {draft[1]:,} bytes |
| Contract | sha256 `cac6535115fb333e68e6c3c045e444342c950ab7a735b7b7deea21cb2cb53b3c` (in the earlier packet; unchanged) |
| Binding reviews | `6dfb14560a8b2d22fe4e6d517891491b926423941179d0965eb38bc52aeecdce`, `1f964d7515d94f3d21e55d53f23b67fb99a67cb6307b6aacef2c585dcd39fb35` (in the earlier packet) |
| controls_defs.py | original `/home/nexus/nexus-audits/claude-architect/p3-c11-dr01-d2a1-campaign-01/tools/stage5-prepared-NOT-EXECUTED/controls_defs.py`, sha256 `ef0420a1a69a756d4e71b3a746e39f7e29933643c121645f44dce32c7e4a108f`, sealed in CAMPAIGN-01's SHA256SUMS; carried here as `inert/controls_defs.py.txt` (byte-identical) |

## Where to start

1. `ad1r3/P3-C11-D2A1-CONTRACT-02-R1-ADDENDUM-AD1-R3-DRAFT.md`, sections 0A.1–0A.2, 1 (S-1, S-2, S-6, S-14, S-19 to S-21),
   F.1, G.3, K.1, K.2, O.4, R and the open items; `ad1r3/AD1-R2-to-AD1-R3.diff` for every change.
2. `preflight/C9-C10-C24-C4-ATTRIBUTION-PREFLIGHT.md`: C10 and C24 have sealed, unapproved definitions; C9 and C4 have
   none. Predicted first failures: C10 E4, C24 P8, C9 E4 (named in each case); C4 P4 only, so "P4, P8" needs the
   Architect's reading.
3. `receipts/04-final-gate.txt` and `checks/`.

## Verify

From this directory: `sha256sum -c PACKET-SHA256SUMS` (covers every file but itself; INDEX.md included).
The tools reproduce the draft and the checks from the authenticated inputs named in `receipts/03-freeze.txt`; they
read the contract and reviews from the earlier packet's byte copies or the sealed originals.

## Files

| Path | Bytes | sha256 | What |
|---|---|---|---|
""" + "\n".join(f"| `{r}` | {n:,} | `{h}` | {d} |" for r, n, h, d in rows) + "\n"
open(os.path.join(DST, "INDEX.md"), "x", encoding="utf-8").write(index)

listing = []
for root, _, files in os.walk(DST):
    for f in files:
        listing.append(os.path.relpath(os.path.join(root, f), DST))
listing = sorted(listing)
man = "".join(f"{sha(open(os.path.join(DST, r), 'rb').read())}  {r}\n" for r in listing)
open(os.path.join(DST, "PACKET-SHA256SUMS"), "x", encoding="utf-8").write(man)

# ---- verification -----------------------------------------------------------------------------------------------
bad = []
expected = sorted([r for r, _, _ in FILES] + ["INDEX.md"])
if listing != expected:
    bad.append(f"prefix holds {len(listing)} files, expected {len(expected)}")
for rel, src, _ in FILES:
    if open(os.path.join(DST, rel), "rb").read() != open(os.path.join(EV, src), "rb").read():
        bad.append(f"copy differs: {rel}")
allf = listing + ["PACKET-SHA256SUMS"]
total = sum(os.path.getsize(os.path.join(DST, r)) for r in allf)
if total > LIMIT:
    bad.append(f"size {total} > {LIMIT}")
PAT = re.compile("|".join([
    "gh" + "p_[A-Za-z0-9]{30,}", "github" + "_pat_[A-Za-z0-9_]{20,}", "gh" + "o_[A-Za-z0-9]{30,}",
    "AK" + "IA[0-9A-Z]{16}", "-----BEGIN [A-Z ]*PRIV" + "ATE KEY-----", "s" + "k-[A-Za-z0-9]{32,}",
    "xox" + "[abprs]-[A-Za-z0-9-]{10,}", "(?i)pass" + "word\\s*[:=]\\s*\\S+"]))
hits = []
for r in allf:
    b = open(os.path.join(DST, r), "rb").read()
    if b"\x00" in b:
        bad.append(f"NUL byte: {r}")
    for m in PAT.finditer(b.decode("utf-8", "replace")):
        hits.append(f"{r}: {m.group(0)[:12]}…")
if hits:
    bad.append(f"credential pattern hits: {hits}")
print(f"packet {PREFIX}: {len(allf)} files, {total:,} bytes (limit {LIMIT:,})")
print(f"INDEX.md sha256 {sha(open(os.path.join(DST, 'INDEX.md'), 'rb').read())}")
print(f"PACKET-SHA256SUMS sha256 {sha(man.encode())} ({len(listing)} entries, excludes itself)")
print(f"credential-pattern hits: {len(hits)}; NUL files: {sum(1 for b in bad if b.startswith('NUL'))}")
for b in bad:
    print("FAIL", b)
print("BUILD:", "OK" if not bad else "FAILED")
sys.exit(1 if bad else 0)
