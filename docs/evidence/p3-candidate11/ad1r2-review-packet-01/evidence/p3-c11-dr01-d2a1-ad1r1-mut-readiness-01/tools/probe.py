#!/usr/bin/env python3
"""One exploratory mutant probe of P3-C11-DR01-D2A1-AD1R1-MUT-READINESS-01.

    probe.py C1|C25

At most one probe per control is authorised. The control's text is loaded from CAMPAIGN-01's sealed
tools/stage5-prepared-NOT-EXECUTED/controls_defs.py, checked against its sealed sha256 first. Steps:
  1. a disposable worktree, detached at the base commit, checked for commit, tree and cleanliness;
  2. the witness source applied from patches/witness-source.patch and checked against the blob that was
     built and run healthy (the authenticated baseline);
  3. the mutation applied: its anchor must occur exactly once and its replacement not at all; the patch is
     saved and hashed; only protocol.rs may differ from the authenticated baseline;
  4. a fresh optimised build in the probe's own target directory; the executable is hashed;
  5. only the control's witness test is run (run-witness.sh, --exact);
  6. the receipt is classified, and its per-step record compared with the healthy receipt;
  7. protocol.rs is restored, checked against the base blob, rebuilt and the witness run again (restored);
  8. the summary is written. The worktree is left for removal by the caller after the evidence is saved.
Nothing here runs an exploration.
"""
import hashlib, json, os, re, subprocess, sys

BASE = "2d83b74b3e9f0bfb6ca4763df56d57f3c044bcda"
TREE = "6f6a54f5c85d6a3102cd034c67e5791cb5a0286a"
EV = "/home/nexus/nexus-audits/claude-architect/p3-c11-dr01-d2a1-ad1r1-mut-readiness-01"
REPO = "/home/nexus/NEXUS/nexus-os"
DEFS_DIR = "/home/nexus/nexus-audits/claude-architect/p3-c11-dr01-d2a1-campaign-01/tools/stage5-prepared-NOT-EXECUTED"
DEFS_SHA = "ef0420a1a69a756d4e71b3a746e39f7e29933643c121645f44dce32c7e4a108f"
WITNESS_PATCH = os.path.join(EV, "patches/witness-source.patch")
WITNESS_BLOB = "5ad26ddc727d8be30f4fb8d7478bc69e1c974933"
PROTOCOL_BLOB = "a2df2841b99cad5d452008a6d4150e9369d07e66"
CORE = "crates/nexus-governed-control/src/ledger/protocol.rs"
MODEL = "crates/nexus-governed-control/src/ledger/protocol/model_tests.rs"
EXE_NAME = "target/release/deps/nexus_governed_control-2faa0b9718d3e6a3"
RUN = os.path.join(EV, "tools/run-witness.sh")
PROBES = {
    "C1": dict(test="c1_witness_p1_at_a_successful_admission", named="P1", first="E3",
               expect="NAMED PROPERTY P1: OBSERVED at step 11 (admission: admitted)",
               healthy=os.path.join(EV, "receipts/stage4/healthy/02-c1-witness-healthy.txt")),
    "C25": dict(test="c25_witness_p7_on_the_resulting_world", named="P7", first="E4",
                expect="NAMED PROPERTY P7: OBSERVED on the world after step 1 (append_gating: Committed (inserted))",
                healthy=os.path.join(EV, "receipts/stage4/healthy/03-c25-witness-healthy.txt")),
}
ENV = dict(os.environ, CARGO_INCREMENTAL="0", CARGO_BUILD_JOBS="8")


def sh(args, cwd, check=True, env=None):
    r = subprocess.run(args, cwd=cwd, capture_output=True, text=True, env=env)
    if check and r.returncode != 0:
        raise SystemExit(f"FAILED {args}: {r.returncode}\n{r.stdout}\n{r.stderr}")
    return r


def sha256(path):
    return hashlib.sha256(open(path, "rb").read()).hexdigest()


def steps_of(text):
    """The per-step record of a witness receipt: header (actor, choices, note), branches, violations."""
    out, cur = [], None
    for line in text.splitlines():
        m = re.search(r"WITNESS F-1 step +(\d+)/(\d+) (.*)$", line)
        if m:
            cur = {"step": int(m.group(1)), "header": m.group(3), "branches": None, "violations": []}
            out.append(cur)
        elif cur is not None and line.startswith("    branches "):
            cur["branches"] = line.split(";")[0][len("    branches "):]
            cur["word"] = line.split("; word after ")[1]
        elif cur is not None and line.startswith("    ledger after "):
            cur["ledger"] = line[len("    ledger after "):]
        elif cur is not None and line.startswith("    VIOLATION "):
            cur["violations"].append(line.strip())
    return out


def main():
    control = sys.argv[1]
    p = PROBES[control]
    out_dir = os.path.join(EV, f"receipts/stage4/probe-{control}")
    if os.path.exists(out_dir):
        raise SystemExit(f"REFUSED: {out_dir} exists (one probe per control)")
    os.makedirs(out_dir)
    log = []

    def note(text):
        print(text)
        log.append(text)

    # The control's text, from the sealed file.
    assert sha256(os.path.join(DEFS_DIR, "controls_defs.py")) == DEFS_SHA, "controls_defs.py changed"
    sys.path.insert(0, DEFS_DIR)
    from controls_defs import CONTROLS  # noqa: E402
    meaning, path, pairs, kind, target, expected = CONTROLS[control]
    assert path == CORE and len(pairs) == 1, (path, len(pairs))
    anchor, replacement = pairs[0]
    note(f"PROBE {control}: {meaning}")
    note(f"  definition source: {DEFS_DIR}/controls_defs.py sha256 {DEFS_SHA} (CAMPAIGN-01, sealed; never executed before)")
    note(f"  contract 7.5 row: named property {p['named']} on F-1; this probe selects only {p['test']}")

    # 1. The disposable worktree.
    wt = f"/home/nexus/NEXUS/nexus-os-p3-c11-ad1r1-probe-{control.lower()}"
    assert not os.path.exists(wt), f"{wt} exists"
    sh(["git", "worktree", "add", "--detach", wt, BASE], REPO)
    head = sh(["git", "rev-parse", "HEAD"], wt).stdout.strip()
    tree = sh(["git", "rev-parse", "HEAD^{tree}"], wt).stdout.strip()
    porcelain = sh(["git", "status", "--porcelain"], wt).stdout
    note(f"  worktree {wt}: HEAD {head} tree {tree} porcelain {len(porcelain.splitlines())}")
    assert head == BASE and tree == TREE and porcelain == ""

    # 2. The authenticated baseline: base + the witness source that ran healthy.
    sh(["git", "apply", "--check", WITNESS_PATCH], wt)
    sh(["git", "apply", WITNESS_PATCH], wt)
    blob = sh(["git", "hash-object", MODEL], wt).stdout.strip()
    note(f"  witness source applied: {MODEL} blob {blob} (healthy-run blob {WITNESS_BLOB}) "
         f"{'MATCH' if blob == WITNESS_BLOB else 'MISMATCH'}; patch sha256 {sha256(WITNESS_PATCH)}")
    assert blob == WITNESS_BLOB
    assert sh(["git", "hash-object", CORE], wt).stdout.strip() == PROTOCOL_BLOB

    # 3. The mutation.
    core_path = os.path.join(wt, CORE)
    src = open(core_path).read()
    n_anchor, n_repl = src.count(anchor), src.count(replacement)
    note(f"  anchor occurrences {n_anchor}, replacement occurrences before {n_repl}")
    assert n_anchor == 1 and n_repl == 0
    open(core_path, "w").write(src.replace(anchor, replacement))
    mutant_blob = sh(["git", "hash-object", CORE], wt).stdout.strip()
    diff = sh(["git", "diff", "--", CORE], wt).stdout
    patch_path = os.path.join(EV, f"patches/{control}-mutation.patch")
    open(patch_path, "w").write(diff)
    changed = sorted(l[3:] for l in sh(["git", "status", "--porcelain"], wt).stdout.splitlines())
    note(f"  mutation patch {patch_path} sha256 {sha256(patch_path)}")
    note(f"  protocol.rs mutant blob {mutant_blob} sha256 {sha256(core_path)}")
    note(f"  files differing from the base: {changed}")
    assert changed == sorted([CORE, MODEL])
    removed = [l for l in diff.splitlines() if l.startswith("-") and not l.startswith("---")]
    added = [l for l in diff.splitlines() if l.startswith("+") and not l.startswith("+++")]
    note(f"  mutation diff: {len(removed)} line(s) removed, {len(added)} added; no test file touched by it")
    for l in diff.splitlines():
        note(f"    | {l}")
    assert not any(k in l for l in removed for k in ("assert", "panic!", "fail(")), "a removed line asserts"

    # 4. The build.
    env = dict(ENV, CARGO_TARGET_DIR=os.path.join(wt, "target"))
    build_json = os.path.join(out_dir, "01-build.json")
    cmd = ["nice", "-n", "10", "cargo", "test", "-p", "nexus-governed-control", "--locked", "--release",
           "--lib", "--no-run", "-j", "8", "--message-format=json-render-diagnostics"]
    note(f"  build: {' '.join(cmd)} (cwd {wt}; CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS=8 CARGO_TARGET_DIR={wt}/target)")
    r = subprocess.run(cmd, cwd=wt, env=env, capture_output=True, text=True)
    open(build_json, "w").write(r.stdout)
    open(os.path.join(out_dir, "01-build.stderr"), "w").write(r.stderr)
    msgs = sum(1 for l in r.stdout.splitlines() if '"reason":"compiler-message"' in l)
    note(f"  build exit {r.returncode}; compiler messages {msgs}; {r.stderr.strip().splitlines()[-1]}")
    assert r.returncode == 0
    exe = os.path.join(wt, EXE_NAME)
    exe_sha = sha256(exe)
    note(f"  mutant test executable {exe} sha256 {exe_sha} ({os.path.getsize(exe)} bytes)")

    # 5. Only the witness.
    receipt = os.path.join(out_dir, "02-mutant-witness.txt")
    st = subprocess.run([RUN, receipt, wt, exe, p["test"]]).returncode
    text = open(receipt).read()
    note(f"  mutant witness receipt {receipt} sha256 {sha256(receipt)}; exit {st}")

    # 6. Classification and comparison.
    fixture = "WITNESS FIXTURE FAILURE" in text
    named = p["expect"] in text
    others = re.search(r"OTHER PROPERTIES VIOLATED, each reported separately and not counted as \w+: (\[.*\])", text)
    first_listed = bool(others) and f'"{p["first"]}"' in others.group(1)
    verdict = ("KILLED BY THE WITNESS (named property observed)" if st != 0 and named and not fixture
               else "FIXTURE FAILURE (kills nothing)" if fixture
               else "NOT KILLED AS NAMED" if st != 0 else "SURVIVED")
    note(f"  test failed: {st != 0}; fixture failure: {fixture}; '{p['expect']}': {named}")
    note(f"  other properties listed separately: {others.group(1) if others else None}; "
         f"{p['first']} among them: {first_listed}")
    mutant_steps = steps_of(text)
    healthy_steps = steps_of(open(p["healthy"]).read())
    note(f"  per-step comparison with the healthy receipt {p['healthy']} (sha256 {sha256(p['healthy'])}):")
    assert len(mutant_steps) == len(healthy_steps)
    enable_diff = []
    for h, m in zip(healthy_steps, mutant_steps):
        hdr_h = re.sub(r":.*$", "", h["header"])
        hdr_m = re.sub(r":.*$", "", m["header"])
        if hdr_h != hdr_m:
            enable_diff.append(h["step"])
        same = (h["header"] == m["header"], h["branches"] == m["branches"], h["ledger"] == m["ledger"],
                h["word"] == m["word"])
        note(f"    step {h['step']:>2}: actor+choices {'same' if hdr_h == hdr_m else 'DIFFER'}; "
             f"note {'same' if same[0] else 'differs'}; branches {'same' if same[1] else 'DIFFER'}; "
             f"ledger {'same' if same[2] else 'DIFFERS'}; word {'same' if same[3] else 'differs'}"
             f"; healthy violations {len(h['violations'])}, mutant {len(m['violations'])}")
        if not all(same):
            note(f"        healthy: {h['header']} | {h['branches']} | word {h['word']} | ledger {h['ledger']}")
            note(f"        mutant:  {m['header']} | {m['branches']} | word {m['word']} | ledger {m['ledger']}")
        for v in m["violations"]:
            note(f"        mutant {v}")
    note(f"  step-enablement or choice differences: {enable_diff or 'none'} "
         f"(every step was asserted enabled and taking its scheduled choices in both runs)")
    note(f"  VERDICT {control}: {verdict}")

    # 7. Restored.
    sh(["git", "checkout", "--", CORE], wt)
    rblob = sh(["git", "hash-object", CORE], wt).stdout.strip()
    rchanged = sorted(l[3:] for l in sh(["git", "status", "--porcelain"], wt).stdout.splitlines())
    note(f"  restored protocol.rs blob {rblob} ({'MATCH' if rblob == PROTOCOL_BLOB else 'MISMATCH'} base); "
         f"model_tests.rs blob {sh(['git', 'hash-object', MODEL], wt).stdout.strip()}; differing files {rchanged}")
    assert rblob == PROTOCOL_BLOB and rchanged == [MODEL]
    r = subprocess.run(cmd, cwd=wt, env=env, capture_output=True, text=True)
    open(os.path.join(out_dir, "03-restored-build.json"), "w").write(r.stdout)
    open(os.path.join(out_dir, "03-restored-build.stderr"), "w").write(r.stderr)
    note(f"  restored build exit {r.returncode}; {r.stderr.strip().splitlines()[-1]}")
    assert r.returncode == 0
    rexe_sha = sha256(exe)
    note(f"  restored test executable sha256 {rexe_sha}")
    rreceipt = os.path.join(out_dir, "04-restored-witness.txt")
    rst = subprocess.run([RUN, rreceipt, wt, exe, p["test"]]).returncode
    rtext = open(rreceipt).read()
    rpass = rst == 0 and "no violation; step" in rtext
    note(f"  restored witness receipt {rreceipt} sha256 {sha256(rreceipt)}; exit {rst}; "
         f"{'PASS (no violation, healthy branch)' if rpass else 'NOT PASS'}")
    rsteps = steps_of(rtext)
    same_as_healthy = [(a["header"], a["branches"], a["word"], a["ledger"], a["violations"]) for a in rsteps] == \
                      [(a["header"], a["branches"], a["word"], a["ledger"], a["violations"]) for a in healthy_steps]
    note(f"  restored per-step record identical to the healthy receipt's: {same_as_healthy}")

    summary = os.path.join(out_dir, "00-PROBE-SUMMARY.txt")
    open(summary, "w").write("\n".join(log) + "\n")
    json.dump(dict(control=control, verdict=verdict, mutant_exe_sha256=exe_sha, restored_exe_sha256=rexe_sha,
                   mutant_blob=mutant_blob, worktree=wt, restored_pass=rpass), open(os.path.join(out_dir, "05-result.json"), "w"), indent=1)
    print(f"SUMMARY {summary}")


if __name__ == "__main__":
    main()
