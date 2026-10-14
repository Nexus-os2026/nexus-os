#!/usr/bin/env python3
"""The one narrow C25 probe of P3-C11-DR01-D2A1-AD1R1-WITNESS-AUDIT-01. Run once; refuses to run twice.

The mutation: in Publish::gating, after Insert::Inserted(seq), on Commit::Unknown, the arm keeps
`self.failed(); self.rolled(io.rollback()); self.keep = true;` and replaces ONLY its final
`return refused(Persistence::Uncertain, Reason::Storage);` with `return GatingOutcome::Committed(seq);`.
The anchor is the historical sealed C25 anchor (CAMPAIGN-01 controls_defs.py, sha256 ef0420a1...). The historical
replacement is NOT used; it is only compared. Steps:
  1. a disposable worktree detached at the authenticated witness commit, checked for commit, tree, cleanliness and
     blobs;
  2. the mutation: anchor exactly once, replacement absent; original and mutant blobs; patch saved and hashed; the
     patch must remove exactly the one refusal line and add exactly the one Committed line;
  3. the comparison with the historical sealed C25 text, saved;
  4. a fresh optimised build in the probe's own target directory; the executable hashed;
  5. only the existing C25 witness test, through run-witness.sh;
  6. requirements A-J checked against the raw receipt and the healthy receipt;
  7. protocol.rs restored and checked against the base blob; the worktree left clean at the witness commit for the
     caller to remove after the evidence is sealed.
"""
import hashlib, json, os, re, subprocess, sys

WITNESS = "42dd1bd1cf21ffdea5968b256d2983f6575fab5b"
WITNESS_TREE = "7ac6bb21430d15b17e3723b217b20a49dccf8282"
EV = "/home/nexus/nexus-audits/claude-architect/p3-c11-dr01-d2a1-ad1r1-witness-audit-01"
REPO = "/home/nexus/NEXUS/nexus-os"
WT = "/home/nexus/NEXUS/nexus-os-p3-c11-ad1r1-audit-probe-c25"
DEFS_DIR = "/home/nexus/nexus-audits/claude-architect/p3-c11-dr01-d2a1-campaign-01/tools/stage5-prepared-NOT-EXECUTED"
DEFS_SHA = "ef0420a1a69a756d4e71b3a746e39f7e29933643c121645f44dce32c7e4a108f"
OLD_PATCH = "/home/nexus/nexus-audits/claude-architect/p3-c11-dr01-d2a1-ad1r1-mut-readiness-01/patches/C25-mutation.patch"
OLD_RECEIPT = "/home/nexus/nexus-audits/claude-architect/p3-c11-dr01-d2a1-ad1r1-mut-readiness-01/receipts/stage4/probe-C25/02-mutant-witness.txt"
HEALTHY = os.path.join(EV, "receipts/stage2/01-c25-witness-healthy-42dd1bd1.txt")
CORE = "crates/nexus-governed-control/src/ledger/protocol.rs"
MODEL = "crates/nexus-governed-control/src/ledger/protocol/model_tests.rs"
PROTOCOL_BLOB = "a2df2841b99cad5d452008a6d4150e9369d07e66"
MODEL_BLOB = "5ad26ddc727d8be30f4fb8d7478bc69e1c974933"
REFUSAL = "                    return refused(Persistence::Uncertain, Reason::Storage);"
COMMITTED = "                    return GatingOutcome::Committed(seq);"
KEEP = "                    self.keep = true;"
EXE_NAME = "target/release/deps/nexus_governed_control-2faa0b9718d3e6a3"
TEST = "c25_witness_p7_on_the_resulting_world"
RUN = os.path.join(EV, "tools/run-witness.sh")
OUT = os.path.join(EV, "receipts/stage2/probe-C25-narrow")


def sh(args, cwd, env=None):
    r = subprocess.run(args, cwd=cwd, capture_output=True, text=True, env=env)
    if r.returncode != 0:
        raise SystemExit(f"FAILED {args}: {r.returncode}\n{r.stdout}\n{r.stderr}")
    return r.stdout


def sha256(path):
    return hashlib.sha256(open(path, "rb").read()).hexdigest()


def step_record(text):
    m = re.search(r"WITNESS F-1 step +1/1 (\S+) choices (\[[^\]]*\]): (.*?)(   \[fault\])?$", text, re.M)
    b = re.search(r"^    branches (\[.*?\]); word after (0x[0-9a-f]+) \(live (\d+), unapplied (\d+)\)$", text, re.M)
    l = re.search(r"^    ledger after (.*)$", text, re.M)
    log = re.search(r"\[(Open:[^\]]*)\]", text)
    return dict(actor=m.group(1), choices=m.group(2), note=m.group(3), fault=bool(m.group(4)),
                branches=b.group(1), word=b.group(2), live=b.group(3), unapplied=b.group(4), ledger=l.group(1),
                verbs=log.group(1), violations=re.findall(r"^    VIOLATION (step|after): (.*)$", text, re.M))


def main():
    if os.path.exists(OUT) or os.path.exists(WT):
        raise SystemExit("REFUSED: the one narrow probe has already been started")
    os.makedirs(OUT)
    log = []

    def note(text):
        print(text)
        log.append(text)

    note("NARROW C25 PROBE (the one authorised): Commit::Unknown answered Committed, self.keep = true preserved")
    # 1. The worktree at the authenticated witness commit.
    sh(["git", "worktree", "add", "--detach", WT, WITNESS], REPO)
    head, tree = sh(["git", "rev-parse", "HEAD", "HEAD^{tree}"], WT).split()
    porcelain = sh(["git", "status", "--porcelain"], WT)
    pblob, mblob = sh(["git", "hash-object", CORE, MODEL], WT).split()
    note(f"  worktree {WT}: HEAD {head} tree {tree} porcelain {len(porcelain.splitlines())}; "
         f"protocol.rs {pblob}; model_tests.rs {mblob}")
    assert (head, tree, porcelain, pblob, mblob) == (WITNESS, WITNESS_TREE, "", PROTOCOL_BLOB, MODEL_BLOB)

    # 2. The mutation.
    assert sha256(os.path.join(DEFS_DIR, "controls_defs.py")) == DEFS_SHA, "controls_defs.py changed"
    sys.path.insert(0, DEFS_DIR)
    from controls_defs import CONTROLS  # noqa: E402
    _, _, pairs, _, _, _ = CONTROLS["C25"]
    anchor, historical = pairs[0]
    assert anchor.count(REFUSAL) == 1 and anchor.count(KEEP) == 1, "the anchor is not the expected arm"
    narrow = anchor.replace(REFUSAL, COMMITTED)
    core_path = os.path.join(WT, CORE)
    src = open(core_path).read()
    note(f"  anchor (historical sealed C25 anchor, {len(anchor.splitlines())} lines): occurs {src.count(anchor)}; "
         f"narrow replacement occurs {src.count(narrow)}; the refusal line alone occurs {src.count(REFUSAL)}")
    assert src.count(anchor) == 1 and src.count(narrow) == 0
    for l in anchor.splitlines():
        note(f"    anchor | {l}")
    for l in narrow.splitlines():
        note(f"    narrow | {l}")
    open(core_path, "w").write(src.replace(anchor, narrow))
    mutant_blob = sh(["git", "hash-object", CORE], WT).strip()
    patch = sh(["git", "diff", "--", CORE], WT)
    patch_path = os.path.join(EV, "patches/C25-narrow-mutation.patch")
    open(patch_path, "w").write(patch)
    removed = [l[1:] for l in patch.splitlines() if l.startswith("-") and not l.startswith("---")]
    added = [l[1:] for l in patch.splitlines() if l.startswith("+") and not l.startswith("+++")]
    changed = sh(["git", "status", "--porcelain"], WT).splitlines()
    note(f"  original protocol.rs blob {PROTOCOL_BLOB} sha256 a5454944368df70fa4ce1c7cc4191d283e0a54b371137338fb999e59960aa296")
    note(f"  mutant   protocol.rs blob {mutant_blob} sha256 {sha256(core_path)}")
    note(f"  patch {patch_path} sha256 {sha256(patch_path)}: removed {removed}; added {added}; files {changed}")
    assert removed == [REFUSAL] and added == [COMMITTED] and changed == [f" M {CORE}"]
    mutant_src = open(core_path).read()
    arm = mutant_src[mutant_src.index("Insert::Inserted(seq) => match io.commit() {"):][:520]
    note("  the mutated arm, as built:")
    for l in arm.splitlines()[:12]:
        note(f"    | {l}")
    assert KEEP in arm and COMMITTED in arm and "self.failed();" in arm and "self.rolled(io.rollback());" in arm

    # 3. The historical sealed C25 text, compared.
    hist_src = src.replace(anchor, historical)
    hist_path = os.path.join(OUT, "historical-C25-mutant-protocol.rs")
    narrow_path = os.path.join(OUT, "narrow-C25-mutant-protocol.rs")
    open(hist_path, "w").write(hist_src)
    open(narrow_path, "w").write(mutant_src)
    cmp = subprocess.run(["diff", "-u", "--label", "historical C25 mutant (CAMPAIGN-01 sealed text)", "--label",
                          "narrow C25 mutant (this probe)", hist_path, narrow_path], capture_output=True, text=True).stdout
    cmp_path = os.path.join(EV, "patches/C25-historical-vs-narrow.diff")
    open(cmp_path, "w").write(cmp)
    note(f"  historical sealed mutant vs narrow mutant ({cmp_path}, sha256 {sha256(cmp_path)}):")
    for l in cmp.splitlines():
        note(f"    {l}")
    old_patch_sha = sha256(OLD_PATCH)
    note(f"  historical probe patch (previous mission) {OLD_PATCH} sha256 {old_patch_sha} "
         f"(recorded a6de4ed9fdbc0c351790df899f8e92863bbb4e6950320ecddf07078368ac656b)")
    note("  The historical text is not redefined as accepted. Neither text is approved; this probe measures the narrow one.")

    # 4. The build.
    env = dict(os.environ, CARGO_INCREMENTAL="0", CARGO_BUILD_JOBS="8", CARGO_TARGET_DIR=os.path.join(WT, "target"))
    cmd = ["nice", "-n", "10", "cargo", "test", "-p", "nexus-governed-control", "--locked", "--release", "--lib",
           "--no-run", "-j", "8", "--message-format=json-render-diagnostics"]
    note(f"  build: {' '.join(cmd)} (cwd {WT}; CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=8 CARGO_TARGET_DIR={WT}/target)")
    r = subprocess.run(cmd, cwd=WT, env=env, capture_output=True, text=True)
    open(os.path.join(OUT, "01-build.json"), "w").write(r.stdout)
    open(os.path.join(OUT, "01-build.stderr"), "w").write(r.stderr)
    msgs = sum(1 for l in r.stdout.splitlines() if '"reason":"compiler-message"' in l)
    note(f"  build exit {r.returncode}; compiler messages {msgs}; {r.stderr.strip().splitlines()[-1]}")
    assert r.returncode == 0
    exe = os.path.join(WT, EXE_NAME)
    exe_sha = sha256(exe)
    note(f"  mutant test executable {exe} sha256 {exe_sha} ({os.path.getsize(exe)} bytes)")
    assert sh(["git", "hash-object", CORE], WT).strip() == mutant_blob

    # 5. Only the C25 witness.
    receipt = os.path.join(OUT, "02-mutant-c25-witness.txt")
    st = subprocess.run([RUN, receipt, WT, exe, TEST]).returncode
    text = open(receipt).read()
    note(f"  raw receipt {receipt} sha256 {sha256(receipt)}; exit {st}")

    # 6. Requirements A-J.
    healthy = open(HEALTHY).read()
    h, m = step_record(healthy), step_record(text)
    fixture = "WITNESS FIXTURE FAILURE" in text
    req = {
        "A same initial configuration (F-1)": "C25 WITNESS F-1 step  1/1" in text and "C25 WITNESS F-1 step  1/1" in healthy,
        "B same deterministic actor schedule": m["actor"] == h["actor"] == "Gc(0)",
        "C every step enabled": not fixture and "1 steps, each enabled and taking its choices" in text,
        "D same oracle choice sequence": m["choices"] == h["choices"] == "[0, 0, 0, 1, 0, 0]" and m["verbs"] == h["verbs"],
        "E mutant returns Committed": m["branches"] == '["append_gating: Committed (inserted)"]'
            and "append_gating G1 -> Committed (inserted)" in m["note"],
        "F gating record absent in the modelled outcome": m["ledger"] == "[Prepared(0), Prepared(1)]" and "Gating(0)" not in m["ledger"],
        "G E4 on the mutant's operation": ("step", "E4: append_gating said Committed but the record is absent") in m["violations"],
        "H P7 on the resulting state": ("after", "P7: G1 was told Committed and is absent") in m["violations"]
            and "NAMED PROPERTY P7: OBSERVED on the world after step 1 (append_gating: Committed (inserted))" in text,
        "I self.keep remains true (source, and U kept: word bit 0x20)": KEEP in arm and int(m["word"], 16) & 0x20 != 0,
        "J healthy witness passes the same schedule": "exit status: 0" in healthy and h["violations"] == []
            and "no violation; step 1 shows the healthy core's 'append_gating: Refused Uncertain Storage'" in healthy,
    }
    note(f"  healthy step: {h}")
    note(f"  mutant  step: {m}")
    for k, v in req.items():
        note(f"  {'PASS' if v else 'FAIL'}  {k}")
    others = re.search(r"OTHER PROPERTIES VIOLATED.*: (\[.*\])", text)
    note(f"  other properties listed separately: {others.group(1) if others else None}; test exit {st} (failure = the named "
         f"property observed; a fixture failure would kill nothing: {fixture})")
    old = step_record(open(OLD_RECEIPT).read())
    note(f"  previous (historical-text) probe step: {old}")
    note(f"  difference from the previous probe: word {old['word']} -> {m['word']} (U emptied there, kept here); "
         f"branches, choices, verbs, ledger and violations "
         f"{'identical' if (old['branches'], old['choices'], old['verbs'], old['ledger'], old['violations']) == (m['branches'], m['choices'], m['verbs'], m['ledger'], m['violations']) else 'DIFFER'}")
    verdict = "NARROW C25 CONFIRMED (P7 observed; E4 separate)" if all(req.values()) and st != 0 else "C25_DESIGN_BLOCKED"
    note(f"  VERDICT: {verdict}")

    # 7. Restored.
    sh(["git", "checkout", "--", CORE], WT)
    rblob = sh(["git", "hash-object", CORE], WT).strip()
    rpor = sh(["git", "status", "--porcelain"], WT)
    note(f"  restored protocol.rs blob {rblob} ({'MATCH' if rblob == PROTOCOL_BLOB else 'MISMATCH'}); porcelain "
         f"{len(rpor.splitlines())}; HEAD {sh(['git', 'rev-parse', 'HEAD'], WT).strip()}")
    assert rblob == PROTOCOL_BLOB and rpor == ""
    open(os.path.join(OUT, "00-PROBE-SUMMARY.txt"), "w").write("\n".join(log) + "\n")
    json.dump(dict(verdict=verdict, requirements=req, mutant_blob=mutant_blob, mutant_exe_sha256=exe_sha,
                   patch_sha256=sha256(patch_path), comparison_sha256=sha256(cmp_path), exit=st, healthy=h, mutant=m,
                   previous=old), open(os.path.join(OUT, "03-result.json"), "w"), indent=1)


if __name__ == "__main__":
    main()
