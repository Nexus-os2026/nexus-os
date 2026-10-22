#!/usr/bin/env python3
"""Builds the AD1R4 consolidation review packet (P3-C11-DR01-D2A1-AD1R4-CONSOLIDATION-01). Derived from AD-1-R3's
build-packet.py. Copies only the files listed below from the mission evidence directory into one new prefix of the
destination tree, writes INDEX.md and a manifest PACKET-SHA256SUMS covering every packet file but itself, then verifies:
every copy equals its source; the prefix holds exactly the listed files plus INDEX.md and the manifest; total size at
most 1 MiB; every file is UTF-8 text with no NUL byte; no file is executable; no .rs or .patch file and no unified diff
of Rust source is carried; no credential pattern. Refuses if the prefix exists. Touches nothing outside the prefix.
Runs nothing of Nexus.
    build-packet.py <evidence-dir> <destination-root>"""
import hashlib, os, re, shutil, stat, sys

EV, WT = sys.argv[1:3]
PREFIX = "docs/evidence/p3-candidate11/ad1r4-consolidation-01"
DST = os.path.join(WT, PREFIX)
LIMIT = 1024 * 1024
R4 = "ad1r4/P3-C11-D2A1-CONTRACT-02-R1-ADDENDUM-AD1-R4-DRAFT.md"
FILES = [  # (packet path, source path relative to the evidence dir, description)
    (R4, "drafts/P3-C11-D2A1-CONTRACT-02-R1-ADDENDUM-AD1-R4-DRAFT.md", "AD-1-R4 DRAFT — NOT ACCEPTED — NOT ADOPTED — NOT APPLIED"),
    ("ad1r4/AD1-R3-to-AD1-R4.diff", "drafts/AD1-R3-to-AD1-R4.diff", "unified diff, AD-1-R3 (485d9256…) to AD-1-R4"),
    ("review/R3-TO-R4-SEMANTIC-REVIEW.md", "review/R3-TO-R4-SEMANTIC-REVIEW.md", "the executor's semantic review of every touched section; attention items"),
    ("matrix/CONSOLIDATION-MATRIX.tsv", "matrix/CONSOLIDATION-MATRIX.tsv", "the consolidation matrix (checked source): 242 rows"),
    ("matrix/CONSOLIDATION-MATRIX.md", "matrix/CONSOLIDATION-MATRIX.md", "the same matrix rendered with each source sentence"),
    ("matrix/README.md", "matrix/README.md", "how to read the matrix; kinds; checks"),
    ("checks/01-check-ad1r4.txt", "checks/01-check-ad1r4.txt", "final gate G3: mechanical checks of AD-1-R4 (quotes, supersession, taxonomy, C4, C9-b, C10/C24, ...)"),
    ("checks/02-contradiction-scan.txt", "checks/02-contradiction-scan.txt", "final gate G4: contradiction scan, twelve categories"),
    ("checks/03-check-matrix.txt", "checks/03-check-matrix.txt", "final gate G5: consolidation matrix checks"),
    ("checks/04-classification-cases.txt", "checks/04-classification-cases.txt", "final gate G6: K.2 cases on synthetic descriptors, matched to the draft's table"),
    ("checks/05-negative-controls.txt", "checks/05-negative-controls.txt", "final gate G7: seeded corruptions each tool must catch"),
    ("checks/06-obligation-scan-r3.txt", "checks/06-obligation-scan-r3.txt", "final gate G8: AD-1-R3 obligation sentences, verbatim or not in AD-1-R4"),
    ("checks/07-obligation-scan-disposition.txt", "checks/07-obligation-scan-disposition.txt", "final gate G8b: every sentence of the disposition sheet"),
    ("checks/08-review-coverage.txt", "checks/08-review-coverage.txt", "final gate G9: the semantic review covers exactly the touched sections"),
    ("checks/09-defs-anchor-check.txt", "checks/09-defs-anchor-check.txt", "final gate G10: sealed C10/C24 anchors at 42dd1bd1 (AD-1-R3's tool, unchanged)"),
    ("receipts/00-STAGE1-AUTHENTICATION.txt", "receipts/00-STAGE1-AUTHENTICATION.txt", "provenance and input authentication"),
    ("receipts/02-development-log.txt", "receipts/02-development-log.txt", "every development run, failure and revision before the freeze"),
    ("receipts/03-freeze.txt", "receipts/03-freeze.txt", "freeze record: frozen tools, template, matrix, review and inputs"),
    ("receipts/04-final-gate.txt", "receipts/04-final-gate.txt", "the final gate: commands and exit codes"),
    ("receipts/github-refs-before.txt", "receipts/github-refs-before.txt", "git ls-remote github before any change"),
    ("receipts/revisions/FAILURE-EXTRACTS.txt", "receipts/revisions/FAILURE-EXTRACTS.txt", "the failing lines of every development run that failed"),
    ("tools/ad1r4-template.md", "tools/ad1r4-template.md", "the draft's template (placeholders for verbatim lines)"),
    ("tools/fill-ad1r4.py", "tools/fill-ad1r4.py", "fills verbatim blocks from the authenticated bytes"),
    ("tools/check-ad1r4.py", "tools/check-ad1r4.py", "mechanical checker (final version)"),
    ("tools/contradiction-scan.py", "tools/contradiction-scan.py", "contradiction scan (final version)"),
    ("tools/check-matrix.py", "tools/check-matrix.py", "matrix checker (final version)"),
    ("tools/obligation-scan.py", "tools/obligation-scan.py", "obligation-sentence scan (AD-1-R3's splitter and keywords)"),
    ("tools/render-matrix.py", "tools/render-matrix.py", "renders the matrix for reading"),
    ("tools/classify-cases-r4.py", "tools/classify-cases-r4.py", "K.2 rules on synthetic descriptors"),
    ("tools/negative-controls-r4.py", "tools/negative-controls-r4.py", "negative controls (final version)"),
    ("tools/check-review.py", "tools/check-review.py", "review coverage check"),
    ("tools/defs-anchor-check.py", "tools/defs-anchor-check.py", "AD-1-R3's sealed-definition anchor check, byte-identical"),
    ("tools/stage1-auth.sh", "tools/stage1-auth.sh", "stage 1 authentication"),
    ("tools/final-gate.sh", "tools/final-gate.sh", "the declared final gate"),
    ("tools/build-packet.py", "tools/build-packet.py", "this builder"),
    ("tools/revisions/check-ad1r4.v1-to-check-ad1r4.v2.diff", "tools/revisions/check-ad1r4.v1-to-check-ad1r4.v2.diff", "checker revision v1 -> v2"),
    ("tools/revisions/check-ad1r4.v2-to-check-ad1r4.v3.diff", "tools/revisions/check-ad1r4.v2-to-check-ad1r4.v3.diff", "checker revision v2 -> v3"),
    ("tools/revisions/check-ad1r4.v3-to-check-ad1r4.v4.diff", "tools/revisions/check-ad1r4.v3-to-check-ad1r4.v4.diff", "checker revision v3 -> v4"),
    ("tools/revisions/check-ad1r4.v4-to-final.diff", "tools/revisions/check-ad1r4.v4-to-final.diff", "checker revision v4 -> final"),
    ("tools/revisions/contradiction-scan.v1-to-contradiction-scan.v2.diff", "tools/revisions/contradiction-scan.v1-to-contradiction-scan.v2.diff", "scan revision v1 -> v2"),
    ("tools/revisions/contradiction-scan.v2-to-final.diff", "tools/revisions/contradiction-scan.v2-to-final.diff", "scan revision v2 -> final"),
    ("tools/revisions/negative-controls-r4.v1-to-final.diff", "tools/revisions/negative-controls-r4.v1-to-final.diff", "negative-controls revision v1 -> final"),
    ("tools/revisions/check-matrix.v1-to-final.diff", "tools/revisions/check-matrix.v1-to-final.diff", "matrix-checker revision v1 -> final"),
]


def sha(b):
    return hashlib.sha256(b).hexdigest()


if os.path.exists(DST):
    sys.exit(f"REFUSED: {DST} exists")
for rel, src, _ in FILES:
    p = os.path.join(DST, rel)
    os.makedirs(os.path.dirname(p), exist_ok=True)
    shutil.copyfile(os.path.join(EV, src), p)
    os.chmod(p, 0o644)

rows = []
for rel, src, desc in FILES:
    b = open(os.path.join(DST, rel), "rb").read()
    rows.append((rel, len(b), sha(b), desc))
draft = next(r for r in rows if r[0] == R4)
gate = open(os.path.join(EV, "receipts/04-final-gate.txt"), encoding="utf-8").read()
gate_line = [l for l in gate.split("\n") if l.startswith("FINAL GATE:")]
index = f"""# AD1R4 consolidation packet — P3-C11-DR01-D2A1-AD1R4-CONSOLIDATION-01

**Status: review materials only.** AD-1-R4 is a DRAFT — NOT ACCEPTED — NOT ADOPTED — NOT APPLIED. It is the one
canonical consolidated contract-amendment candidate, for the Sole Architect's independent final contract review.
Nothing here is Owner adoption, a mutation kill, D2A-1 acceptance or Phase Three completion. Nothing was built, tested,
mutated, explored or run against Nexus by this mission.

Prepared by Claude Code (Claude Opus 5.5, `claude-opus-5-5`; mission effort High).

## Identities

| Item | Identity |
|---|---|
| AD-1-R4 (this packet) | `{draft[0]}`, sha256 `{draft[2]}`, {draft[1]:,} bytes |
| Parent of this packet's commit | C4 evidence commit B `a464a36621523f539d2554a6b176d678c7f885ea` (parent: C4 prototype A `04e42a7ddf0d0a2af17b6506c51997006e7a08dc`); not an engineering baseline |
| Contract | `P3-C11-D2A1-CONTRACT-02-R1.md`, sha256 `cac6535115fb333e68e6c3c045e444342c950ab7a735b7b7deea21cb2cb53b3c` (in the AD1R2 packet, commit 8693e3a7 / c2c69e3a) |
| Binding reviews | `6dfb14560a8b2d22fe4e6d517891491b926423941179d0965eb38bc52aeecdce`, `1f964d7515d94f3d21e55d53f23b67fb99a67cb6307b6aacef2c585dcd39fb35` (same packet) |
| AD-1-R3 (history) | sha256 `485d9256932220644d7368d64b7b9b4d8df8a3a5c65aba4a1c5fcdf390ae286e`, blob `7f2463f5421502cc6ee74c070fc3abc41d467770`, commit c2c69e3a |
| AD1R3-DISPOSITION-01 (history) | sha256 `a8a6d29376cf6f393a7eaa9047923719a339377caa554527a8878760cbdcf11c`, at B |
| Historical source baseline | 42dd1bd1 (`protocol.rs` blob `a2df2841b99cad5d452008a6d4150e9369d07e66`) |
| Sealed C10/C24 definitions | `controls_defs.py` sha256 `ef0420a1a69a756d4e71b3a746e39f7e29933643c121645f44dce32c7e4a108f` (inert copy in the AD1R3 packet) |

## Where to start

1. `{R4}`: header, 0A.1 (the governing C4-WITNESS-01 ruling), 0A.5, section 1 (S-22 to S-25), F.1 and F.2, G.2, G.5,
   G.6, K.2, and the open items.
2. `review/R3-TO-R4-SEMANTIC-REVIEW.md`: every touched section, and seven items for the Architect's attention.
3. `matrix/CONSOLIDATION-MATRIX.md`: where every ruling item, every disposition-sheet sentence and every non-verbatim
   AD-1-R3 obligation landed.
4. `receipts/04-final-gate.txt` and `checks/`. Final gate: {gate_line[0] if gate_line else 'NOT FOUND'}

## Reproduce

The tools read the authenticated inputs from git objects or the sealed local originals named in
`receipts/03-freeze.txt`; none is copied here (the contract, reviews, AD-1-R3, the disposition sheet, `protocol.rs` at
42dd1bd1 and the inert `controls_defs.py` copy are each in an earlier commit). `tools/final-gate.sh` is the exact
sequence; each step's command is printed in `receipts/04-final-gate.txt`.

From this directory: `sha256sum -c PACKET-SHA256SUMS` (covers every file but itself; INDEX.md included).

Excluded by design: every executable, build artefact and build log; mutation source (no patch, no `.rs` file); the
earlier packets; full copies of superseded tool versions (their diffs are under `tools/revisions/`; the copies stay in
the local evidence directory). The post-publication read-back is kept outside this commit.

## Files

| Path | Bytes | sha256 | What |
|---|---|---|---|
""" + "\n".join(f"| `{r}` | {n:,} | `{h}` | {d} |" for r, n, h, d in rows) + "\n"
open(os.path.join(DST, "INDEX.md"), "x", encoding="utf-8").write(index)
os.chmod(os.path.join(DST, "INDEX.md"), 0o644)

listing = sorted(os.path.relpath(os.path.join(r, f), DST) for r, _, fs in os.walk(DST) for f in fs)
man = "".join(f"{sha(open(os.path.join(DST, r), 'rb').read())}  {r}\n" for r in listing)
open(os.path.join(DST, "PACKET-SHA256SUMS"), "x", encoding="utf-8").write(man)
os.chmod(os.path.join(DST, "PACKET-SHA256SUMS"), 0o644)

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
    "xox" + "[abprs]-[A-Za-z0-9-]{10,}", "(?i:pass" + "word)\\s*[:=]\\s*\\S+"]))
hits = []
for r in allf:
    p = os.path.join(DST, r)
    b = open(p, "rb").read()
    if b"\x00" in b:
        bad.append(f"NUL byte: {r}")
    try:
        t = b.decode("utf-8")
    except UnicodeDecodeError:
        bad.append(f"not UTF-8 text: {r}")
        t = b.decode("utf-8", "replace")
    if os.stat(p).st_mode & (stat.S_IXUSR | stat.S_IXGRP | stat.S_IXOTH):
        bad.append(f"executable mode: {r}")
    if r.endswith((".rs", ".patch")) or re.search(r"^\+\+\+ .*\.rs\b", t, flags=re.M):
        bad.append(f"mutation or Rust source: {r}")
    for m in PAT.finditer(t):
        hits.append(f"{r}: {m.group(0)[:12]}…")
if hits:
    bad.append(f"credential pattern hits: {hits}")
print(f"packet {PREFIX}: {len(allf)} files, {total:,} bytes (limit {LIMIT:,})")
print(f"AD-1-R4 {draft[0]} sha256 {draft[2]} bytes {draft[1]}")
print(f"INDEX.md sha256 {sha(open(os.path.join(DST, 'INDEX.md'), 'rb').read())}")
print(f"PACKET-SHA256SUMS sha256 {sha(man.encode())} ({len(listing)} entries, excludes itself)")
print(f"credential-pattern hits: {len(hits)}")
for b in bad:
    print("FAIL", b)
print("BUILD:", "OK" if not bad else "FAILED")
sys.exit(1 if bad else 0)
