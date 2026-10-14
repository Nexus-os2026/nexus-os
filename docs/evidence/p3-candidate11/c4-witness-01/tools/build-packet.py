#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-C4-WITNESS-01: build and verify the review packet in the engineering worktree.

  build-packet.py <A-commit>

Writes docs/evidence/p3-candidate11/c4-witness-01/ (which must not exist) from the mission evidence directory:
text only; no binary, no executable, no applied mutant source (only the patch), no credential. INDEX.md is
generated from the receipts; PACKET-SHA256SUMS lists every other file (it excludes itself). The packet must be at
most 2 MiB. Exit 0 only if every check holds. It does not commit.
"""
import hashlib, json, os, re, subprocess, sys

EV = "/home/nexus/nexus-audits/claude-architect/p3-c11-dr01-d2a1-c4-witness-01"
WT = "/home/nexus/NEXUS/nexus-os-p3-c11-c4-witness-01"
REL = "docs/evidence/p3-candidate11/c4-witness-01"
OUT = os.path.join(WT, REL)
LIMIT = 2 * 1024 * 1024
EXCLUDE = {"patches/C4-mutant-protocol.rs.txt"}          # an applied mutant source: never committed
SECRET_NAMES = ("TWENTYFIRST_API_KEY", "CLAUDE_CODE_MESSAGING_TOKEN")
PATTERNS = [r"-----BEGIN [A-Z ]*PRIVATE KEY-----", r"\bghp_[A-Za-z0-9]{20,}", r"\bgithub_pat_[A-Za-z0-9_]{20,}",
            r"\bsk-[A-Za-z0-9_-]{20,}", r"\bAKIA[0-9A-Z]{16}\b", r"\bxox[abprs]-[A-Za-z0-9-]{10,}",
            r"(?i)\b(api[_-]?key|secret|password|token)\s*[=:]\s*['\"]?[A-Za-z0-9_\-]{16,}"]
fails = []


def chk(label, ok, detail=""):
    print(f"  {'OK  ' if ok else 'FAIL'} {label}{(': ' + detail) if detail else ''}")
    if not ok:
        fails.append(label)


def sha(path):
    return hashlib.sha256(open(path, "rb").read()).hexdigest()


def sources():
    picked = []
    for top in ("disposition", "frozen", "patches", "tools", "receipts"):
        for root, _, files in os.walk(os.path.join(EV, top)):
            for f in sorted(files):
                rel = os.path.relpath(os.path.join(root, f), EV)
                if rel not in EXCLUDE and "__pycache__" not in rel:
                    picked.append(rel)
    return sorted(picked)


def read_json(rel):
    p = os.path.join(EV, rel)
    return json.load(open(p)) if os.path.exists(p) else {}


def first_line(rel):
    p = os.path.join(EV, rel)
    return open(p).readline().strip() if os.path.exists(p) else "(absent)"


def index(a, files):
    g = lambda *x: subprocess.run(["git", "-C", WT, *x], capture_output=True, text=True, check=True).stdout.strip()
    hb, hr = read_json("receipts/08-build-healthy/descriptor.json"), read_json("receipts/09-run-healthy/descriptor.json")
    mb, mr = read_json("receipts/11-build-mutant/descriptor.json"), read_json("receipts/12-run-mutant/descriptor.json")
    gate = open(os.path.join(EV, "receipts/20-GATE-SUMMARY.txt")).read().strip().splitlines()[-1]
    sup = lambda d: d.get("supervisor", {})
    exe = lambda d: (d.get("recorded_executables") or [{}])[0]
    lines = [
        "# P3-C11-DR01-D2A1-C4-WITNESS-01 — review packet",
        "",
        "**Exploratory witness work. No mutation kill, contract adoption, component acceptance, integration or phase "
        "change.** Prepared by the executor (Claude Code, Claude Opus 5.5) for the Sole Architect's review.",
        "",
        "## Identities",
        "",
        "| Item | Identity |",
        "|---|---|",
        f"| Source commit A (unaccepted witness prototype) | `{a}` tree `{g('rev-parse', a + '^{tree}')}` parent "
        f"`{g('log', '-1', '--format=%P', a)}` |",
        f"| `model_tests.rs` at A | blob `{g('rev-parse', a + ':crates/nexus-governed-control/src/ledger/protocol/model_tests.rs')}` "
        "(append-only over `5ad26ddc`) |",
        "| `protocol.rs` at A | blob `a2df2841b99cad5d452008a6d4150e9369d07e66` (unchanged) |",
        "| Engineering base | `42dd1bd1cf21ffdea5968b256d2983f6575fab5b` |",
        "| Supplemented draft | AD-1-R3 blob `7f2463f5…`, sha256 `485d9256932220644d7368d64b7b9b4d8df8a3a5c65aba4a1c5fcdf390ae286e` |",
        f"| Disposition sheet | `disposition/AD1R3-DISPOSITION-01-DRAFT.md` sha256 "
        f"`{sha(os.path.join(EV, 'disposition/AD1R3-DISPOSITION-01-DRAFT.md'))}` |",
        f"| C4 patch | `patches/C4-test1-removed.patch` sha256 `{sha(os.path.join(EV, 'patches/C4-test1-removed.patch'))}`; "
        "mutant `protocol.rs` blob `f3793c2ff61f8adbf43fe17d6b5237b49bc507b0` |",
        f"| Healthy executable | sha256 `{exe(hb).get('sha256')}` ({exe(hb).get('size')} bytes) |",
        f"| Mutant executable | sha256 `{exe(mb).get('sha256')}` ({exe(mb).get('size')} bytes); never committed |",
        "",
        "## Results",
        "",
        f"- Gate: {gate.split(' ', 1)[1] if ' ' in gate else gate}",
        f"- Healthy build: {first_line('receipts/08-build-healthy/CLASSIFICATION.txt')}; wall "
        f"{sup(hb).get('wall_seconds')} s; memory.peak {sup(hb).get('memory_peak_bytes')} B; swap peak "
        f"{sup(hb).get('swap_peak_bytes')} B",
        f"- Healthy witness: {first_line('receipts/09-run-healthy/CLASSIFICATION.txt')}; exit "
        f"{hr.get('process', {}).get('exit_status')}; wall {sup(hr).get('wall_seconds')} s",
        f"- Mutant build: {first_line('receipts/11-build-mutant/CLASSIFICATION.txt')}; wall {sup(mb).get('wall_seconds')} s; "
        f"memory.peak {sup(mb).get('memory_peak_bytes')} B",
        f"- Mutant witness: {first_line('receipts/12-run-mutant/CLASSIFICATION.txt')}; exit "
        f"{mr.get('process', {}).get('exit_status')}; wall {sup(mr).get('wall_seconds')} s",
        f"- Restoration: {open(os.path.join(EV, 'receipts/13-mutant-restore.txt')).read().strip().splitlines()[-1].strip()}",
        "",
        "## Where to look",
        "",
        "- `disposition/`: the short disposition sheet (R3-01, R3-02, the C4 route, C9-b, C10/C24).",
        "- `frozen/`: the frozen schedules and the freeze record. `patches/`: the C4 patch only.",
        "- `receipts/09-run-healthy/raw-output.txt` and `receipts/12-run-mutant/raw-output.txt`: both full replay outputs.",
        "- `receipts/*/RECEIPT.txt`, `scope-record.txt`, `time-v.txt`, `descriptor.json`, `CLASSIFICATION.txt`: commands, "
        "identities, exit or signal, resource limits and peaks, and the classification of each receipt.",
        "- `tools/`: the complete runner, classifier, checker, supervisor and packet code; `tools/revisions/` and "
        "`receipts/revisions/`: development failures and their revisions, kept.",
        "",
        "Excluded by design: the applied mutant source (only its patch travels), every executable and build artefact.",
        "The post-publication read-back is kept outside this commit.",
        "",
        f"## Files ({len(files)} besides this index and the manifest)",
        "",
    ] + [f"- `{f}`" for f in files]
    return "\n".join(lines) + "\n"


def main():
    a = sys.argv[1]
    if os.path.exists(OUT):
        raise SystemExit(f"REFUSED: {OUT} exists")
    files = sources()
    secrets = [os.environ[n] for n in SECRET_NAMES if os.environ.get(n)]
    print(f"PACKET BUILD — {REL} from {EV}; A {a}")
    print("[1] content checks")
    for rel in files:
        data = open(os.path.join(EV, rel), "rb").read()
        try:
            text = data.decode("utf-8")
            binary = "\x00" in text
        except UnicodeDecodeError:
            binary = True
        if binary:
            chk(f"text only: {rel}", False)
            continue
        hits = [p for p in PATTERNS if re.search(p, text)]
        chk(f"no credential pattern: {rel}", not hits, f"{hits}" if hits else "")
        chk(f"no session secret value: {rel}", not any(s and s in text for s in secrets))
        chk(f"not executable-format: {rel}", not data.startswith(b"\x7fELF"))
    for rel in files:
        os.makedirs(os.path.dirname(os.path.join(OUT, rel)), exist_ok=True)
        open(os.path.join(OUT, rel), "wb").write(open(os.path.join(EV, rel), "rb").read())
    open(os.path.join(OUT, "INDEX.md"), "w").write(index(a, files))
    listed = sorted(files + ["INDEX.md"])
    manifest = "".join(f"{sha(os.path.join(OUT, f))}  {f}\n" for f in listed)
    open(os.path.join(OUT, "PACKET-SHA256SUMS"), "w").write(manifest)
    print("[2] packet checks")
    total = sum(os.path.getsize(os.path.join(r, f)) for r, _, fs in os.walk(OUT) for f in fs)
    chk(f"size {total} bytes, at most {LIMIT}", total <= LIMIT)
    r = subprocess.run(["sha256sum", "--quiet", "-c", "PACKET-SHA256SUMS"], cwd=OUT, capture_output=True, text=True)
    chk("PACKET-SHA256SUMS verifies", r.returncode == 0, r.stdout + r.stderr)
    on_disk = sorted(os.path.relpath(os.path.join(r_, f), OUT) for r_, _, fs in os.walk(OUT) for f in fs)
    chk("the manifest lists every file but itself", sorted(listed + ["PACKET-SHA256SUMS"]) == on_disk)
    chk("PACKET-SHA256SUMS excludes itself", "PACKET-SHA256SUMS" not in manifest)
    chk("no applied mutant source travels", not any("mutant-protocol.rs" in f for f in on_disk))
    por = subprocess.run(["git", "-C", WT, "status", "--porcelain", "--untracked-files=all"], capture_output=True,
                         text=True).stdout.splitlines()
    outside = [l for l in por if not l[3:].startswith(REL + "/")]
    chk("the worktree changes only inside the packet prefix", not outside and all(l.startswith("?? ") for l in por),
        f"{outside}")
    chk("HEAD is A", subprocess.run(["git", "-C", WT, "rev-parse", "HEAD"], capture_output=True, text=True).stdout.strip() == a)
    print(f"  files {len(on_disk)}; bytes {total}; manifest sha256 {sha(os.path.join(OUT, 'PACKET-SHA256SUMS'))}")
    print("PACKET RESULT:", "ALL CHECKS OK" if not fails else f"{len(fails)} FAILED: {fails[:10]}")
    sys.exit(1 if fails else 0)


if __name__ == "__main__":
    main()
