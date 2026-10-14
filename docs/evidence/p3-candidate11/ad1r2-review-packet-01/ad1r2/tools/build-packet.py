#!/usr/bin/env python3
"""Builds the AD-1-R2 review packet (P3-C11-DR01-D2A1-AD1R2-CONTRACT-EVIDENCE-01).

Copies a declared selection of existing sealed evidence byte for byte, plus this mission's new files, into the packet
directory; verifies every copy against its source file and against its origin directory's sealed SHA256SUMS; refuses
binaries, files over the size limit, and anything a credential pattern matches; writes INDEX.md and, last,
TRANSPORT-SHA256SUMS (which covers every packet file except itself). Neither file records its own hash or any commit
that will contain the packet.
    build-packet.py --dry-run | --write <packet-dir>"""
import hashlib, os, re, shutil, sys

ROOT = "/home/nexus/nexus-audits/claude-architect"
MISSION = "p3-c11-dr01-d2a1-ad1r2-contract-evidence-01"
LIMIT = 10 * 1024 * 1024
# The two origin hash lists whose hashes the mission states; the others' are recorded in this mission's stage-1 receipt.
EXPECTED_LISTS = {
    "p3-c11-dr01-d2a1-ad1r1-witness-audit-01/SHA256SUMS": "87062975919ac988905d28794abb74381b49de8a5579d31d808239c740c77dce",
    "p3-c11-dr01-d2a1-ad1r1-mut-readiness-01/SHA256SUMS": "106243539f31aae6cc0f6026b40c280af954f51af4a9907a2f7f0d063b7bbff3",
}

R = "p3-c11-dr01-d2a1-ad1r1-mut-readiness-01"
W = "p3-c11-dr01-d2a1-ad1r1-witness-audit-01"
# (origin dir, path in it, purpose, category)
SEL = [
    (R, "contract/P3-C11-D2A1-CONTRACT-02-R1.md", "the accepted contract (sha256 cac65351…3b3c)", "CONTRACT"),
    (R, "contract/P3-C11-R3-AMENDMENT-01-RESPONSE-02-REVIEW-02.md", "binding review (fourth review)", "CONTRACT"),
    (R, "contract/P3-C11-R3-AMENDMENT-01-RESPONSE-03-REVIEW-01.md", "binding review (controlled baseline)", "CONTRACT"),
    (R, "contract/RECOVERY-NOTE.txt", "recovery note of the three contract documents", "CONTRACT"),
    (R, "contract/SHA256SUMS", "original hash list of the contract documents", "ORIGINAL-HASH-LIST"),
    ("p3-c11-dr01-d2a1-closure-prep-01", "drafts/P3-C11-D2A1-CONTRACT-02-R1-ADDENDUM-AD1-DRAFT.md", "AD-1 (history)", "DRAFT-HISTORY"),
    (R, "drafts/P3-C11-D2A1-CONTRACT-02-R1-ADDENDUM-AD1-R1-DRAFT.md", "AD-1-R1 (history; repaired by AD-1-R2)", "DRAFT-HISTORY"),
    (R, "drafts/AD1-to-AD1-R1.diff", "diff AD-1 -> AD-1-R1", "DRAFT-HISTORY"),
    # latest witness audit (narrow C25 probe and publication of 42dd1bd1)
    (W, "REPORT.txt", "latest witness-audit report", "REPORT"),
    (W, "SHA256SUMS", "original hash list of the witness audit", "ORIGINAL-HASH-LIST"),
    (W, "patches/C25-narrow-mutation.patch", "narrow C25 mutation (proposed definition), inert", "INERT-PATCH"),
    (W, "patches/C25-historical-vs-narrow.diff", "historical compound vs narrow C25 mutant, inert", "INERT-PATCH"),
    (W, "receipts/stage1/00-STAGE1-AUTHENTICATION.txt", "authentication of 42dd1bd1 before the probe", "BUILD-IDENTITY"),
    (W, "receipts/stage1/github-refs-before.txt", "remote refs before publishing 42dd1bd1", "PUBLICATION-RECEIPT"),
    (W, "receipts/stage2/01-c25-witness-healthy-42dd1bd1.txt", "healthy C25 witness at 42dd1bd1, raw", "RAW-RECEIPT"),
    (W, "receipts/stage2/02-probe-worktree-disposal.txt", "disposal of the narrow probe worktree", "RAW-RECEIPT"),
    (W, "receipts/stage2/probe-C25-narrow/00-PROBE-SUMMARY.txt", "narrow C25 probe: identities, A-J checks", "SUMMARY"),
    (W, "receipts/stage2/probe-C25-narrow/01-build.json", "narrow C25 mutant build messages (profile, executable)", "BUILD-IDENTITY"),
    (W, "receipts/stage2/probe-C25-narrow/01-build.stderr", "narrow C25 mutant build stderr", "BUILD-IDENTITY"),
    (W, "receipts/stage2/probe-C25-narrow/02-mutant-c25-witness.txt", "narrow C25 mutant witness, raw (exit 101)", "RAW-RECEIPT"),
    (W, "receipts/stage2/probe-C25-narrow/03-result.json", "narrow C25 probe result record", "SUMMARY"),
    (W, "receipts/stage2/probe-C25-narrow/PROBE-SHA256SUMS", "original hash list of the narrow probe", "ORIGINAL-HASH-LIST"),
    (W, "receipts/stage3/01-pre-push.txt", "pre-push checks for 42dd1bd1", "PUBLICATION-RECEIPT"),
    (W, "receipts/stage3/02-push.txt", "push of 42dd1bd1 to its review ref", "PUBLICATION-RECEIPT"),
    (W, "receipts/stage3/03-remote-verification.txt", "remote read-back of 42dd1bd1", "PUBLICATION-RECEIPT"),
    (W, "receipts/stage3/github-refs-after-push.txt", "remote refs after publishing 42dd1bd1", "PUBLICATION-RECEIPT"),
    (W, "receipts/final/01-final-state.txt", "witness audit final state", "SUMMARY"),
    (W, "receipts/final/02-ad1r1-handoff.txt", "AD-1-R1 handoff record", "SUMMARY"),
    (W, "receipts/final/github-refs-final.txt", "remote refs at the end of the witness audit", "PUBLICATION-RECEIPT"),
    (W, "tools/probe-c25-narrow.py", "narrow C25 probe script (inert copy)", "SCRIPT"),
    (W, "tools/run-witness.sh", "witness runner (inert copy)", "SCRIPT"),
    # readiness: C1 probe, historical C25 probe, healthy witnesses, gates, plan
    (R, "REPORT.txt", "readiness report (C1 probe, historical C25 probe, AD-1-R1)", "REPORT"),
    (R, "SHA256SUMS", "original hash list of the readiness mission", "ORIGINAL-HASH-LIST"),
    (R, "handoff/42dd1bd1-witnesses.patch", "format-patch of the witness commit, inert", "INERT-PATCH"),
    (R, "patches/C1-mutation.patch", "C1 mutation (provisionally valid encoding), inert", "INERT-PATCH"),
    (R, "patches/C25-mutation.patch", "historical compound C25 mutation, inert; history only", "INERT-PATCH"),
    (R, "patches/witness-source.patch", "witness source diff, inert", "INERT-PATCH"),
    (R, "receipts/stage1/00-STAGE1-SUMMARY.txt", "readiness provenance", "SUMMARY"),
    (R, "receipts/stage1/github-refs-before.txt", "remote refs before the readiness mission", "PUBLICATION-RECEIPT"),
    (R, "receipts/stage3/00-STAGE3-WITNESS-DESIGN.txt", "derivation of the C1/C25 schedules", "SUMMARY"),
    (R, "receipts/stage4/00-STAGE4-SUMMARY.txt", "probe stage summary", "SUMMARY"),
    (R, "receipts/stage4/03-probe-worktree-disposal.txt", "disposal of the readiness probe worktrees", "RAW-RECEIPT"),
    (R, "receipts/stage4/healthy/01-build.json", "healthy build messages (executable 0983a19f…)", "BUILD-IDENTITY"),
    (R, "receipts/stage4/healthy/01-build.stderr", "healthy build stderr", "BUILD-IDENTITY"),
    (R, "receipts/stage4/healthy/01-build.txt", "healthy build record", "BUILD-IDENTITY"),
    (R, "receipts/stage4/healthy/02-c1-witness-healthy.txt", "healthy C1 witness, raw", "RAW-RECEIPT"),
    (R, "receipts/stage4/healthy/03-c25-witness-healthy.txt", "healthy C25 witness, raw", "RAW-RECEIPT"),
]
for probe, what in (("probe-C1", "C1"), ("probe-C25", "historical compound C25")):
    SEL += [
        (R, f"receipts/stage4/{probe}/00-PROBE-SUMMARY.txt", f"{what} probe: identities and comparison", "SUMMARY"),
        (R, f"receipts/stage4/{probe}/01-build.json", f"{what} mutant build messages", "BUILD-IDENTITY"),
        (R, f"receipts/stage4/{probe}/01-build.stderr", f"{what} mutant build stderr", "BUILD-IDENTITY"),
        (R, f"receipts/stage4/{probe}/02-mutant-witness.txt", f"{what} mutant witness, raw (exit 101)", "RAW-RECEIPT"),
        (R, f"receipts/stage4/{probe}/03-restored-build.json", f"{what} restored build messages", "BUILD-IDENTITY"),
        (R, f"receipts/stage4/{probe}/03-restored-build.stderr", f"{what} restored build stderr", "BUILD-IDENTITY"),
        (R, f"receipts/stage4/{probe}/04-restored-witness.txt", f"{what} restored witness, raw (exit 0)", "RAW-RECEIPT"),
        (R, f"receipts/stage4/{probe}/05-result.json", f"{what} probe result record", "SUMMARY"),
    ]
for f in sorted(os.listdir(f"{ROOT}/{R}/receipts/stage5")):
    cat = "SUMMARY" if f.startswith("00-") else "BUILD-IDENTITY" if f.startswith("01-") else "RAW-RECEIPT"
    SEL.append((R, f"receipts/stage5/{f}", "regression gate at 42dd1bd1 (listings, diagnostics, witnesses)", cat))
for f in sorted(os.listdir(f"{ROOT}/{R}/receipts/stage6")):
    SEL.append((R, f"receipts/stage6/{f}", "final-validation plan, manifest dry runs, control matrix", "SUMMARY"))
SEL += [
    (R, "receipts/final/01-final-state.txt", "readiness final state", "SUMMARY"),
    (R, "receipts/final/github-refs-after.txt", "remote refs after the readiness mission", "PUBLICATION-RECEIPT"),
]
for f in sorted(os.listdir(f"{ROOT}/{R}/tools")):
    SEL.append((R, f"tools/{f}", "readiness tool (inert copy)", "SCRIPT"))
SEL += [
    # proofs relied on by B, E and N
    ("p3-c11-dr01-d2a1-adm-quotient-01", "REPORT.txt", "admission quotient report", "REPORT"),
    ("p3-c11-dr01-d2a1-adm-quotient-01", "SHA256SUMS", "original hash list", "ORIGINAL-HASH-LIST"),
    ("p3-c11-dr01-d2a1-adm-quotient-01", "receipts/stage2/01-formal-argument.txt", "AQ proof: Theorem 1, Q1-Q15, AQ1-AQ6 (not accepted)", "PROOF"),
    ("p3-c11-dr01-d2a1-adm-quotient-01", "receipts/stage3/01-experimental-commit.txt", "source identity of 83816e66", "BUILD-IDENTITY"),
    ("p3-c11-dr01-d2a1-adm-quotient-01", "receipts/stage5/01-c1-c25-contract-diagnosis.txt", "C1/C25 first-failure diagnosis", "SUMMARY"),
    ("p3-c11-dr01-d2a1-gc-symmetry-01", "REPORT.txt", "gating-caller symmetry report", "REPORT"),
    ("p3-c11-dr01-d2a1-gc-symmetry-01", "SHA256SUMS", "original hash list", "ORIGINAL-HASH-LIST"),
    ("p3-c11-dr01-d2a1-gc-symmetry-01", "receipts/stage2/00-FORMAL-SYMMETRY-ANALYSIS.txt", "SQ proof: Lemma E, S1-S15, ASYM, AS1-AS4 (not accepted)", "PROOF"),
    ("p3-c11-dr01-d2a1-gc-symmetry-01", "receipts/stage2/01-identity-grep.txt", "SQ proof support: identity occurrences", "PROOF"),
    ("p3-c11-dr01-d2a1-gc-symmetry-01", "diff/00-diff-identity.txt", "source identity of 77a146ca", "BUILD-IDENTITY"),
    ("p3-c11-dr01-d2a1-gc-symmetry-01", "receipts/stage4/40-f3-prefix-diagnostic.txt", "F-3 prefix, INCOMPLETE by construction, raw", "RAW-RECEIPT"),
    ("p3-c11-dr01-d2a1-closure-prep-01", "REPORT.txt", "F-PT/F-B extension report", "REPORT"),
    ("p3-c11-dr01-d2a1-closure-prep-01", "SHA256SUMS", "original hash list", "ORIGINAL-HASH-LIST"),
    ("p3-c11-dr01-d2a1-closure-prep-01", "receipts/stage2/00-FPT-FB-SYMMETRY-PROOF.txt", "E-PT/E-B proof, items 1-17 (not accepted)", "PROOF"),
    ("p3-c11-dr01-d2a1-closure-prep-01", "receipts/stage2/01-grep.txt", "E-PT/E-B proof support", "PROOF"),
    ("p3-c11-dr01-d2a1-closure-prep-01", "receipts/stage3/01-experimental-commit.txt", "source identity of 2d83b74b", "BUILD-IDENTITY"),
    ("p3-c11-dr01-d2a1-closure-prep-01", "receipts/stage5/00-CONFIGURATION-MATRIX.txt", "configuration matrix", "SUMMARY"),
    # complete and incomplete runs cited by B, C, M
    ("p3-c11-dr01-d2a1-gc-sym-f3-01", "REPORT.txt", "F-3 composed quotient report", "REPORT"),
    ("p3-c11-dr01-d2a1-gc-sym-f3-01", "SHA256SUMS", "original hash list", "ORIGINAL-HASH-LIST"),
    ("p3-c11-dr01-d2a1-gc-sym-f3-01", "receipts/stage3/gc-symmetry-full-F-3.txt", "F-3 composed quotient PASS, raw", "RAW-RECEIPT"),
    ("p3-c11-dr01-d2a1-gc-sym-f3-01", "receipts/stage3/00-preflight.txt", "F-3 composed preflight", "SUMMARY"),
    ("p3-c11-dr01-d2a1-gc-sym-f3-01", "receipts/stage3/01-independent-checks.txt", "F-3 composed checks", "SUMMARY"),
    ("p3-c11-dr01-d2a1-gc-sym-f3-01", "receipts/stage2/02-executable.txt", "F-3 composed executable identity", "BUILD-IDENTITY"),
    ("p3-c11-dr01-d2a1-gc-sym-f3-01", "tools/run-gc-symmetry-f3.sh", "F-3 composed launcher (inert copy)", "SCRIPT"),
    ("p3-c11-dr01-d2a1-pt-fb-experiment-01", "REPORT.txt", "F-PT/F-B composed report", "REPORT"),
    ("p3-c11-dr01-d2a1-pt-fb-experiment-01", "SHA256SUMS", "original hash list", "ORIGINAL-HASH-LIST"),
    ("p3-c11-dr01-d2a1-pt-fb-experiment-01", "receipts/stage3/gc-symmetry-full-F-PT.txt", "F-PT composed quotient PASS, raw", "RAW-RECEIPT"),
    ("p3-c11-dr01-d2a1-pt-fb-experiment-01", "receipts/stage4/gc-symmetry-full-F-B.txt", "F-B composed quotient PASS, raw", "RAW-RECEIPT"),
    ("p3-c11-dr01-d2a1-pt-fb-experiment-01", "receipts/stage3/01-independent-checks.txt", "F-PT checks", "SUMMARY"),
    ("p3-c11-dr01-d2a1-pt-fb-experiment-01", "receipts/stage3/02-extracted-figures.txt", "F-PT figures", "SUMMARY"),
    ("p3-c11-dr01-d2a1-pt-fb-experiment-01", "receipts/stage4/01-independent-checks.txt", "F-B checks", "SUMMARY"),
    ("p3-c11-dr01-d2a1-pt-fb-experiment-01", "receipts/stage4/02-extracted-figures.txt", "F-B figures", "SUMMARY"),
    ("p3-c11-dr01-d2a1-pt-fb-experiment-01", "receipts/stage2/02-executable.txt", "F-PT/F-B executable identity", "BUILD-IDENTITY"),
    ("p3-c11-dr01-d2a1-pt-fb-experiment-01", "tools/run-gc-symmetry-full.sh", "F-PT/F-B launcher (inert copy)", "SCRIPT"),
    ("p3-c11-dr01-d2a1-pt-fb-experiment-01", "tools/extract-run.py", "figure extractor (inert copy)", "SCRIPT"),
    ("p3-c11-dr01-d2a1-f3-quotient-01", "REPORT.txt", "F-3 admission-only quotient report", "REPORT"),
    ("p3-c11-dr01-d2a1-f3-quotient-01", "SHA256SUMS", "original hash list", "ORIGINAL-HASH-LIST"),
    ("p3-c11-dr01-d2a1-f3-quotient-01", "receipts/stage3/quotient-full-F-3.txt", "F-3 admission-only quotient INCOMPLETE, raw", "RAW-RECEIPT"),
    ("p3-c11-dr01-d2a1-f3-quotient-01", "receipts/stage3/00-STAGE3-SUMMARY.txt", "its stage summary", "SUMMARY"),
    ("p3-c11-dr01-d2a1-f3-quotient-01", "tools/run-quotient-f3.sh", "its launcher (inert copy)", "SCRIPT"),
    ("p3-c11-dr01-d2a1-campaign-01", "REPORT.txt", "CAMPAIGN-01 report", "REPORT"),
    ("p3-c11-dr01-d2a1-campaign-01", "SHA256SUMS", "original hash list", "ORIGINAL-HASH-LIST"),
    ("p3-c11-dr01-d2a1-campaign-01", "receipts/explorations/full-F-1.txt", "F-1 PASS (60M one-run exception), raw", "RAW-RECEIPT"),
    ("p3-c11-dr01-d2a1-campaign-01", "receipts/explorations/full-F-2.txt", "F-2 PASS, raw", "RAW-RECEIPT"),
    ("p3-c11-dr01-d2a1-campaign-01", "receipts/explorations/full-F-3.txt", "F-3 original INCOMPLETE at 30M, raw", "RAW-RECEIPT"),
    ("p3-c11-dr01-d2a1-campaign-01", "receipts/explorations/full-F-1.vs-cap60-01.txt", "F-1 comparison with F1-CAP60-01", "SUMMARY"),
    ("p3-c11-dr01-d2a1-campaign-01", "receipts/final/01-mutation-manifest-stale-hashes.txt", "the 22 stale manifest pairs", "SUMMARY"),
    ("p3-c11-dr01-d2a1-campaign-01", "tools/run-exploration.sh", "exploration launcher (inert copy)", "SCRIPT"),
    ("p3-c11-dr01-d2a1-f3-cap60-01", "REPORT.txt", "F3-CAP60-01 report", "REPORT"),
    ("p3-c11-dr01-d2a1-f3-cap60-01", "SHA256SUMS", "original hash list", "ORIGINAL-HASH-LIST"),
    ("p3-c11-dr01-d2a1-f3-cap60-01", "receipts/stage4/full-F-3-cap60.txt", "F-3 original INCOMPLETE at 60M, raw", "RAW-RECEIPT"),
    ("p3-c11-dr01-d2a1-f3-cap60-01", "receipts/stage4/00-STAGE4-SUMMARY.txt", "its stage summary", "SUMMARY"),
    ("p3-c11-dr01-d2a1-f3-cap60-01", "receipts/stage5/01-source-excerpts.txt", "source excerpts for attribution", "SUMMARY"),
    ("p3-c11-dr01-d2a1-f3-cap60-01", "receipts/stage5/02-c1-c25-attribution.txt", "C1/C25 attribution analysis", "SUMMARY"),
    ("p3-c11-dr01-d2a1-f1-cap60-01", "REPORT.txt", "F1-CAP60-01 report", "REPORT"),
    ("p3-c11-dr01-d2a1-f1-cap60-01", "SHA256SUMS", "original hash list", "ORIGINAL-HASH-LIST"),
    ("p3-c11-dr01-d2a1-f1-cap60-01", "receipts/explorations/full-F-1-cap60.txt", "F-1 PASS at 243501f0, raw", "RAW-RECEIPT"),
]
# Files of the transported origins deliberately not transported, with the reason (all others: "not needed").
OMIT_REASON = {
    f"{W}/handoff/P3-C11-D2A1-CONTRACT-02-R1-ADDENDUM-AD1-R1-DRAFT.md": "byte-identical duplicate of AD-1-R1 (transported from the readiness drafts/)",
    f"{R}/handoff/P3-C11-D2A1-CONTRACT-02-R1-ADDENDUM-AD1-R1-DRAFT.md": "byte-identical duplicate of AD-1-R1 (transported from drafts/)",
    f"{R}/drafts/AD1-SOURCE-COPY-P3-C11-D2A1-CONTRACT-02-R1-ADDENDUM-AD1-DRAFT.md": "byte-identical duplicate of AD-1 (transported from closure-prep-01)",
    f"{W}/receipts/stage2/probe-C25-narrow/historical-C25-mutant-protocol.rs": "an applied mutant protocol file; never committed (mission rule)",
    f"{W}/receipts/stage2/probe-C25-narrow/narrow-C25-mutant-protocol.rs": "an applied mutant protocol file; never committed (mission rule)",
    "p3-c11-dr01-d2a1-campaign-01/tools/stage5-prepared-NOT-EXECUTED/controls_defs.py": "control definitions for a script kept outside the repository (contract line 750)",
    "p3-c11-dr01-d2a1-campaign-01/tools/stage5-prepared-NOT-EXECUTED/controls.py": "control-applying script kept outside the repository (contract line 750)",
}
NEW = [  # (path under the mission evidence dir, packet path, purpose, category)
    ("drafts/P3-C11-D2A1-CONTRACT-02-R1-ADDENDUM-AD1-R2-DRAFT.md", "ad1r2/P3-C11-D2A1-CONTRACT-02-R1-ADDENDUM-AD1-R2-DRAFT.md", "AD-1-R2 DRAFT — NOT ACCEPTED — NOT ADOPTED — NOT APPLIED", "NEW-DRAFT"),
    ("drafts/AD1-R1-to-AD1-R2.diff", "ad1r2/AD1-R1-to-AD1-R2.diff", "diff AD-1-R1 -> AD-1-R2", "NEW-DRAFT"),
    ("checks/01-ad1r2-mechanical-checks.txt", "ad1r2/checks/01-ad1r2-mechanical-checks.txt", "clause verification, supersession table, ignored-test inventory, identities, numbers", "NEW-CHECK"),
    ("checks/02-r1-to-r2-diff-review.txt", "ad1r2/checks/02-r1-to-r2-diff-review.txt", "AD-1-R1 -> AD-1-R2 review, every dropped sentence accounted for", "NEW-CHECK"),
    ("receipts/00-STAGE1-AUTHENTICATION.txt", "ad1r2/receipts/00-STAGE1-AUTHENTICATION.txt", "source baseline and sealed-input authentication", "NEW-RECEIPT"),
    ("receipts/01-worktree-authentication.txt", "ad1r2/receipts/01-worktree-authentication.txt", "packet worktree identity", "NEW-RECEIPT"),
    ("receipts/02-checker-revision.txt", "ad1r2/receipts/02-checker-revision.txt", "checker revisions before its receipted run", "NEW-RECEIPT"),
    ("receipts/github-refs-before.txt", "ad1r2/receipts/github-refs-before.txt", "remote refs before this mission", "NEW-RECEIPT"),
    ("tools/ad1r2-template.md", "ad1r2/tools/ad1r2-template.md", "AD-1-R2 source with contract-line placeholders", "NEW-TOOL"),
    ("tools/fill-ad1r2.py", "ad1r2/tools/fill-ad1r2.py", "fills the placeholders from the contract's bytes", "NEW-TOOL"),
    ("tools/check-ad1r2.py", "ad1r2/tools/check-ad1r2.py", "independent mechanical checker", "NEW-TOOL"),
    ("tools/r1-r2-obligation-scan.py", "ad1r2/tools/r1-r2-obligation-scan.py", "AD-1-R1 obligation-sentence scan", "NEW-TOOL"),
    ("tools/build-packet.py", "ad1r2/tools/build-packet.py", "this builder", "NEW-TOOL"),
]
CREDENTIAL_PATTERNS = re.compile(rb"(gh[pousr]_[A-Za-z0-9]{20,}|github_pat_[A-Za-z0-9_]{20,}|AKIA[0-9A-Z]{16}|-----BEGIN [A-Z ]*PRIVATE KEY"
                    rb"|xox[baprs]-[A-Za-z0-9-]{10,}|sk-ant-[A-Za-z0-9_-]{10,}|sk-[A-Za-z0-9]{32,}|Bearer [A-Za-z0-9._-]{20,}"
                    rb"|(?i:password|passwd|secret|api[_-]?key|access[_-]?token)\s*[:=]\s*['\"]?[A-Za-z0-9/+_.-]{8,})")


def sha(p):
    return hashlib.sha256(open(p, "rb").read()).hexdigest()


def sums(path):
    out = {}
    for line in open(path, encoding="utf-8"):
        h, _, name = line.rstrip("\n").partition("  ")
        out[name.lstrip("*").removeprefix("./")] = h
    return out


def main():
    mode = sys.argv[1]
    dest = sys.argv[2] if mode == "--write" else None
    problems, rows, total = [], [], 0
    seen = set()
    for origin, rel, purpose, cat in SEL:
        src = f"{ROOT}/{origin}/{rel}"
        key = f"{origin}/{rel}"
        assert key not in seen, f"duplicate selection {key}"
        seen.add(key)
        data = open(src, "rb").read()
        h = hashlib.sha256(data).hexdigest()
        if rel.endswith("SHA256SUMS") and "/" not in rel.replace("contract/", "").replace("receipts/stage2/probe-C25-narrow/PROBE-", ""):
            sealed = EXPECTED_LISTS.get(key, "recorded in ad1r2/receipts/00-STAGE1-AUTHENTICATION.txt")
            if key in EXPECTED_LISTS and h != EXPECTED_LISTS[key]:
                problems.append(f"{key}: hash list {h} != mission's {EXPECTED_LISTS[key]}")
        else:
            lst = f"{ROOT}/{origin}/SHA256SUMS"
            if rel.startswith("contract/"):
                lst = f"{ROOT}/{origin}/contract/SHA256SUMS"
                srel = rel.removeprefix("contract/")
            elif rel.startswith("receipts/stage2/probe-C25-narrow/") and rel.endswith("PROBE-SHA256SUMS"):
                srel = rel
            else:
                srel = rel
            sealed = sums(lst).get(srel)
            if sealed != h:
                problems.append(f"{key}: sha256 {h} != origin list entry {sealed}")
        if b"\x00" in data:
            problems.append(f"{key}: contains NUL bytes (not text)")
        try:
            data.decode("utf-8")
        except UnicodeDecodeError:
            problems.append(f"{key}: not UTF-8 text")
        for m in CREDENTIAL_PATTERNS.finditer(data):
            problems.append(f"{key}: credential-like pattern at byte {m.start()} (category {m.group(0)[:6]!r}…)")
        total += len(data)
        rows.append((key, f"evidence/{key}", h, len(data), purpose, cat))
    for src_rel, ppath, purpose, cat in NEW:
        data = open(f"{ROOT}/{MISSION}/{src_rel}", "rb").read()
        if b"\x00" in data:
            problems.append(f"{src_rel}: NUL")
        for m in CREDENTIAL_PATTERNS.finditer(data):
            problems.append(f"{src_rel}: credential-like pattern at byte {m.start()}")
        total += len(data)
        rows.append((f"{MISSION}/{src_rel} (new)", ppath, hashlib.sha256(data).hexdigest(), len(data), purpose, cat))
    # coverage of every origin's hash list
    coverage = []
    for origin in sorted({o for o, *_ in SEL}):
        listed = sums(f"{ROOT}/{origin}/SHA256SUMS")
        taken = [r for r in listed if f"{origin}/{r}" in seen]
        omitted = [r for r in listed if f"{origin}/{r}" not in seen]
        coverage.append((origin, len(listed), len(taken), omitted))
    print(f"selected {len(SEL)} originals + {len(NEW)} new files; {total} bytes before INDEX/TRANSPORT ({total / 1048576:.2f} MiB)")
    for o, n, t, om in coverage:
        print(f"  {o}: hash list {n} entries; transported {t}; omitted {len(om)}")
    if total > LIMIT * 0.95:
        problems.append(f"size {total} near or over the 10 MiB limit")
    if problems:
        print("PROBLEMS:")
        for p in problems:
            print("  " + p)
        sys.exit(1)
    if mode == "--dry-run":
        print("dry run: no problems")
        return
    assert not os.path.exists(dest), f"REFUSED: {dest} exists"
    os.makedirs(dest)
    for (key, ppath, h, n, purpose, cat), item in zip(rows, SEL + [None] * len(NEW)):
        target = f"{dest}/{ppath}"
        os.makedirs(os.path.dirname(target), exist_ok=True)
        src = f"{ROOT}/{key}" if item else f"{ROOT}/{key.removesuffix(' (new)')}"
        shutil.copyfile(src, target)
        assert sha(target) == h == sha(src), f"copy mismatch {key}"
    write_index(dest, rows, coverage)
    files = sorted(os.path.relpath(os.path.join(d, f), dest) for d, _, fs in os.walk(dest) for f in fs)
    with open(f"{dest}/TRANSPORT-SHA256SUMS", "x", encoding="utf-8") as out:
        for f in files:
            out.write(f"{sha(os.path.join(dest, f))}  {f}\n")
    size = sum(os.path.getsize(os.path.join(dest, f)) for f in files) + os.path.getsize(f"{dest}/TRANSPORT-SHA256SUMS")
    print(f"written {len(files) + 1} files, {size} bytes ({size / 1048576:.2f} MiB) to {dest}")
    assert size <= LIMIT


def write_index(dest, rows, coverage):
    cats = {
        "CONTRACT": "the accepted contract, its binding reviews and recovery note",
        "DRAFT-HISTORY": "AD-1, AD-1-R1 and the AD-1 -> AD-1-R1 diff (sealed; unchanged)",
        "NEW-DRAFT": "AD-1-R2 and its diff from AD-1-R1, authored by this mission",
        "NEW-CHECK": "mechanical checks and review records of this mission",
        "NEW-RECEIPT": "this mission's receipts made before publication",
        "NEW-TOOL": "this mission's tools",
        "REPORT": "an earlier mission's executor report (EXECUTOR-REPORTED)",
        "ORIGINAL-HASH-LIST": "an origin directory's sealed hash list, byte for byte",
        "RAW-RECEIPT": "raw executor test or exploration output (EXECUTOR-REPORTED; not re-run)",
        "BUILD-IDENTITY": "build messages, executable and source identity records, exit codes",
        "PUBLICATION-RECEIPT": "remote refs, push and read-back records",
        "INERT-PATCH": "a documentary patch or diff; never applied from the repository",
        "SCRIPT": "an earlier mission's probe or receipt-generation script (inert copy)",
        "PROOF": "an executor proof document (not accepted)",
        "SUMMARY": "an executor stage summary, plan or analysis (EXECUTOR-REPORTED)",
    }
    L = []
    w = L.append
    w("# AD-1-R2 review packet 01 — index")
    w("")
    w("**Mission:** P3-C11-DR01-D2A1-AD1R2-CONTRACT-EVIDENCE-01. **Executor:** Claude Code, Claude Opus 5.5.")
    w("")
    w("**What this is.** One evidence-transport packet for the independent audit of the AD-1-R2 draft and of the raw C1/C25")
    w("witness evidence. It is documentation only. Its commit is an evidence-transport commit, not an engineering baseline,")
    w("and not the final commit of any mission. AD-1-R2 is a DRAFT — NOT ACCEPTED — NOT ADOPTED — NOT APPLIED. Nothing here")
    w("is a run, a mutation kill, an acceptance or an adoption.")
    w("")
    w("**Source baseline.** Parent commit `42dd1bd1cf21ffdea5968b256d2983f6575fab5b` (tree")
    w("`7ac6bb21430d15b17e3723b217b20a49dccf8282`), the witness commit published on")
    w("`refs/heads/review/p3-c11-d2a1-ad1r1-witness-01`. Every file outside this directory is unchanged from it.")
    w("")
    w("**Originals.** Every file under `evidence/` is a byte copy of the file at the same relative path under the evidence")
    w("root `/home/nexus/nexus-audits/claude-architect/` (outside the repository), which stays authoritative and sealed.")
    w("Its sha256 below was checked against the source file and against the origin directory's sealed `SHA256SUMS` before")
    w("copying. Each origin's `SHA256SUMS` is transported byte for byte; it lists files not transported (coverage below), so")
    w("a transported subset does not verify an entire origin directory.")
    w("")
    w("**Verify.** From this directory: `sha256sum -c TRANSPORT-SHA256SUMS` checks every packet file (it covers every file")
    w("but itself). For one origin, from `evidence/<origin>/`: `sha256sum -c --ignore-missing SHA256SUMS` checks the")
    w("transported subset against the sealed list. AD-1-R2's checks re-run with `ad1r2/tools/check-ad1r2.py`.")
    w("")
    w("**Not in the packet:** binaries, executables, build directories, applied mutant source files, credentials, archives.")
    w("")
    w("## Provenance categories")
    w("")
    w("| Category | Meaning |")
    w("|---|---|")
    for k, v in cats.items():
        w(f"| {k} | {v} |")
    w("")
    w("## Items")
    w("")
    w("| # | Original (evidence-relative path) | Packet path | SHA256 | Bytes | Purpose | Category |")
    w("|---|---|---|---|---|---|---|")
    for i, (key, ppath, h, n, purpose, cat) in enumerate(rows, 1):
        w(f"| {i} | `{key}` | `{ppath}` | `{h}` | {n} | {purpose} | {cat} |")
    w("")
    w("## Coverage of each origin's sealed hash list")
    w("")
    w("| Origin | Entries | Transported | Omitted |")
    w("|---|---|---|---|")
    for o, n, t, om in coverage:
        w(f"| `{o}` | {n} | {t} | {len(om)} |")
    w("")
    w("Omitted files that matter, with the reason (every other omitted file is not needed for this audit and stays")
    w("verifiable in its origin by the transported hash list):")
    w("")
    for k, v in OMIT_REASON.items():
        w(f"- `{k}`: {v}.")
    w("")
    w("A hash list itself is not listed in its own entries; its sha256 is in the table above and in")
    w("`ad1r2/receipts/00-STAGE1-AUTHENTICATION.txt`. The readiness `contract/SHA256SUMS` covers the four contract")
    w("documents, all transported.")
    w("")
    open(f"{dest}/INDEX.md", "x", encoding="utf-8").write("\n".join(L))


main()
