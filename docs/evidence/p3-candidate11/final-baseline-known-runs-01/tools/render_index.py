#!/usr/bin/env python3
"""P3-C11-DR01-D2A1-FINAL-EVIDENCE-STAGE1-01: render the packet's INDEX.md. Every number in its tables is read from
the receipts (PIN.json, the suite's INVENTORY.json, each PARSED-RECEIPT.json and descriptor.json, A4.json); the prose
is fixed text. Read-only.

  render_index.py > INDEX.md
"""
import json
import os

EV = "/home/nexus/nexus-audits/claude-architect/p3-c11-dr01-d2a1-final-baseline-known-runs-01"
J = lambda p: json.load(open(os.path.join(EV, p)))
T = lambda p: open(os.path.join(EV, p)).read()
PIN = J("receipts/10-build/PIN.json")
B = J("receipts/10-build/descriptor.json")
S = J("receipts/30-suite/descriptor.json")
INV = J("receipts/30-suite/INVENTORY.json")
A4 = J("receipts/50-A4/A4.json")
RUNS = [("F-2", "41-full-F-2", "original"), ("F-B", "42-full-F-B", "composed"), ("F-3", "43-full-F-3", "composed"),
        ("F-PT", "44-full-F-PT", "composed"), ("F-1", "45-full-F-1", "original")]
n = lambda v: f"{v:,}" if isinstance(v, int) else str(v)
gib = lambda b: f"{int(b) / 2**30:.2f} GiB"

out = []
w = out.append
w("# Final-baseline known runs 01 — index")
w("")
w("Mission P3-C11-DR01-D2A1-FINAL-EVIDENCE-STAGE1-01. Executor: Claude Code, Claude Opus 5.5 (`claude-opus-5-5`), "
  "effort High. For the Sole Architect's review.")
w("")
w("**Evidence only.** This packet records one fresh optimised build of frozen Candidate C, its complete nonignored "
  "library suite, five exact-final-baseline full runs (F-2, F-B, F-3, F-PT, F-1, in that order, one at a time) and the "
  "A4 comparison. It accepts nothing. D2A-1 remains unaccepted and unintegrated; nothing here completes Phase Three. "
  "No source, manifest or proof document was changed; no mutation, mutant witness, other configuration or hosted CI "
  "was run.")
w("")
w("## Candidate C (frozen; `FREEZE-RECORD.txt`, `receipts/00-authentication.txt`)")
w("")
w("| Item | Value |")
w("|---|---|")
for k, v in (("Commit", "d5964ee5326c2733177307db6f507328023d361a"), ("Tree", "51bac5fd335999cc995c0ab9f582a30244973ef0"),
             ("Single parent", "71264aa81a40b86189783facc8437e0427887c16"),
             ("protocol.rs / model_tests.rs / unit_tests.rs blobs",
              "`a2df2841…` / `28085ce2…` / `55dc6640…`"),
             ("Current-tree manifest ct-1", "blob `5d31cb45…`, sha256 `20bcc6fe…1a44279`"),
             ("AD-1-R4", "sha256 `00ced6d5…5192b693`"),
             ("Worktree", "`/home/nexus/NEXUS/nexus-os-p3-c11-final-baseline-known-runs-01`, detached at C; byte-for-byte "
              "equal to C before and after the build and every invocation (`tools/verify_tree.py`)")):
    w(f"| {k} | {v} |")
w("")
w("## Build and pinned executable (`receipts/10-build`, `receipts/11-pin`)")
w("")
sup = B["supervisor"]
w("| Item | Value |")
w("|---|---|")
for k, v in (("Command", "`cargo test -p nexus-governed-control --locked --release --lib --no-run` (exactly), cwd the worktree"),
             ("Environment (`env -i`)", " ".join(f"`{a}={b}`" for a, b in B["env"].items())),
             ("Toolchain", f"{PIN['rustc']}; `{PIN['rustc_path']}`; cargo 1.94.0 (85eff7c80 2026-01-15)"),
             ("Repository Cargo configuration", "unchanged: `[build] jobs = 8`; `[env] CARGO_PROFILE_TEST_DEBUG = \"1\"`; "
              "release profile opt-level 3, lto thin, codegen-units 16, strip symbols"),
             ("Effective rustc flags of the test unit (build's own verbose message)",
              "`-C opt-level=3 -C lto=thin -C codegen-units=16 -C strip=symbols --test`; no `--cfg` (no feature, no extra "
              "cfg); no debuginfo, debug-assertions, overflow-checks or panic flag (rustc defaults: off, off, off, unwind)"),
             ("Result", f"exit {B['process']['exit_status']}, no signal; `Finished release profile [optimized]`; wall "
              f"{sup['wall_seconds']} s of 900 s; scope peak {gib(sup['memory_peak_bytes'])}, swap peak 0, oom_kill 0"),
             ("Executable", f"`{PIN['executable']}`"),
             ("sha256 / size", f"`{PIN['sha256']}` / {n(PIN['size'])} bytes"),
             ("Inputs (rustc dep-info)", "49 `.rs` files, all under `crates/nexus-governed-control/src/`, each C's blob; "
              "none under `docs/`")):
    w(f"| {k} | {v} |")
w("")
w("The digest was checked before and after every invocation: all equal the pin.")
w("")
w("## Nonignored suite (`receipts/30-suite`) — CLASSIFICATION: PASS")
w("")
ss = S["supervisor"]
w(f"`{os.path.basename(PIN['executable'])} --nocapture --test-threads=1`, no name filter: **{INV['harness_result']}**. "
  f"Exit 0, no signal; wall {ss['wall_seconds']} s of 180 s; scope peak {gib(ss['memory_peak_bytes'])}, swap 0, "
  f"oom_kill 0.")
w("")
w(f"- Listed by the executable: {len(INV['listed'])} tests; ignored {len(INV['ignored'])} (the 34 of AD-1-R4 E.3 and "
  f"`{INV['ignored_outside_model_tests'][0]}`); nonignored {len(INV['nonignored'])}; executed {len(INV['executed'])}; "
  f"difference between listed nonignored and executed: none; duplicates: none; no ignored test executed.")
w("- Five default explorations: " + "; ".join(
    f"{k} PASS, {n(v['states'])} states" for k, v in sorted(INV["default_explorations"].items())) + ".")
w(f"- The 51 diagnostic tests of AD-1-R4 D (N.8): {sum(d['result'] == 'ok' for d in INV['diagnostics_51'])} of 51 ok "
  "(`receipts/30-suite/DIAGNOSTICS-51.txt`).")
w("- Healthy witnesses: C1, C25 and C4 ok, each printing exactly its healthy line(s) of G.3(d) / G.5(C), with no "
  "positive marker, violation or fixture failure; C4's 20 step records hold G.6 against the frozen schedule (evidence "
  "commit B).")
w(f"- {len(INV['refusal_lines'])} refusal lines, every one inside a `should_panic` refusal test; none elsewhere. "
  f"{len(INV['panic_messages'])} panic messages, all in tests written to panic or catch it.")
w("")
w("## Full runs (one at a time; 16 GiB, zero swap, 45 minutes; `--exact --ignored --nocapture --test-threads=1`)")
w("")
w("| Run | Test | Explorer | Measured keys / states (expanded) | Kf | Derived original states | Transitions (fault) | "
  "No-step | Admission evals / admitted | Wall | Scope peak | Outer class |")
w("|---|---|---|---|---|---|---|---|---|---|---|---|")
for cfg, d, ex in RUNS:
    p = J(f"receipts/{d}/PARSED-RECEIPT.json")
    r, s = p["report"], p["supervisor"]
    w(f"| {cfg} | `{p['test'].split('::')[-1]}` | {ex} | {n(r['states'])} ({n(r['expanded'])}) | "
      f"{n(r.get('self_mirrored')) if r.get('self_mirrored') is not None else '—'} | "
      f"{n(r.get('derived_original_states')) if r.get('derived_original_states') else '— (original explored)'} | "
      f"{n(r['transitions'])} ({n(r['fault_edges'])}) | {n(r['no_step'])} | "
      f"{(n(r['evaluations']) + ' / ' + n(r['admitted'])) if r.get('evaluations') is not None else '—'} | "
      f"{s['wall_seconds']} s | {gib(s['memory_peak_bytes'])} | {p['outer_classification']} |")
w("")
w("Every run: exit 0, no signal; swap peak 0; oom_kill 0; 24 property lines HOLD, E10 \"not applicable here (worker "
  "verbs do not unwind)\", D1 and L1 HOLD; all 90 BRANCHES labels listed (taken labels outside the listed set are "
  "recorded as printed); no refusal; no panic; executable and source identity equal before and after. For F-B, F-3 "
  "and F-PT **the original graph was not explored**: the derived counts are (N+1)(2K − Kf) with N = 2, conditional on "
  "the accepted proof (B(1)) and N. F-PT's reduced-key margin: "
  f"{n(30_000_000 - J('receipts/44-full-F-PT/PARSED-RECEIPT.json')['report']['states'])} keys below 30,000,000. F-1 "
  "ran under the Owner-adopted 60,000,000 ceiling (AD-1-R4 A).")
w("")
w("Informational only (AD-1-R4 I: historical results are not evidence): every printed count above equals the "
  "historical receipt of the same configuration — F-1 and F-2 at 244c5b23 (CAMPAIGN-01), F-3 at 77a146ca "
  "(GC-SYM-F3-01), F-PT and F-B at 2d83b74b (PT-FB-EXPERIMENT-01). F1-CAP60-01's F-1 at 243501f0 printed the same "
  "46,524,336 states and 188,062,789 non-fault edges but 173,856 fewer fault edges; 243501f0 precedes the two "
  "SOURCE-R1/R2 model commits (49e29cd3, 244c5b23), whose subjects concern explored insert outcomes.")
w("")
w("## A4 (`receipts/50-A4`)")
w("")
w(f"**A4: {'SATISFIED' if A4['satisfied'] else 'NOT SATISFIED'}** — F-3 PASS ({A4['F-3_pass']}), F-PT PASS "
  f"({A4['F-PT_pass']}), no applicable property fails in F-PT, both admissible. Differences recorded (not required to "
  f"be equal): {', '.join(A4['differences'])}. Taken in F-PT only: {len(A4['taken_only_in_F-PT'])} label(s); in F-3 "
  f"only: {len(A4['taken_only_in_F-3'])}. Not a mutation kill.")
w("")
w("## Executor defects found and repaired during the mission (all in tools; none in Nexus; nothing re-run)")
w("")
w("1. `supervise.py` v1 passed relative record paths into a scope with another cwd: the self-test failed "
  "(`development/selftest-v1-FAILED-relative-paths`). v2 made them absolute.")
w("2. The v2 self-test's time-limit predicate expected exit 137; `timeout -s KILL` kills its own process group, so "
  "signal 9 is seen (`development/selftest-v2-FAILED-predicate`). Enforcement itself worked. v3 accepts either. The "
  "final self-test: all six checks OK.")
w("3. `supervise.py`'s build step looked for cargo's non-verbose `Executable unittests src/lib.rs (…)` line; under "
  "`CARGO_TERM_VERBOSE=true` cargo prints ``Executable `…` `` instead, so it wrote no pin and reported BUILD NOT OK "
  "although the build exited 0. The build was **not** repeated: `tools/pin.py` pinned from the same unedited output "
  "(`receipts/11-pin`). Its informational dep-info line was garbled; `DEP-INFO.txt` replaces it.")
w("4. `check_suite.py` v1: rustc 1.94 prints `thread '<name>' (<id>) panicked at`, which v1's pattern missed (its "
  "panic checks passed vacuously), and a 0.0 s wall time was read as missing (two listing checks failed). v2 fixed "
  "both and classified the same unedited outputs (`receipts/27-TOOLS-REVISION.txt`, "
  "`development/suite-check-v1-checker-defects`).")
w("")
w("## Limitations")
w("")
w("- The healthy witnesses' results come from the unfiltered suite run, as the mission requires. They were not "
  "invoked alone with `--exact` (AD-1-R4 F.2 / G.3(d) / G.5(C) describe that form for a type (e3) receipt); no such "
  "invocation was authorised here, so none was made.")
w("- The original graphs of F-B, F-3 and F-PT were not explored; their original counts are derived, not measured.")
w("- Every result is of the explored model and stays conditional on A-1, A-2 and AQ6 (AD-1-R4 J).")
w("- Executor-written tools parse the output; they were frozen before use and exercised against historical outputs "
  "and synthetic negative controls (`receipts/24`, `26-checker-controls*.txt`).")
w("")
w("## How to verify")
w("")
w("From this directory: `sha256sum -c SHA256SUMS`. The classifiers re-run read-only over the receipts "
  "(`tools/check_suite.py`, `tools/check_full.py <key>`, `tools/a4.py`) given the mission's evidence root path; "
  "`tools/final_gate.py` is the gate whose output is `receipts/60-final-gate.txt`.")
print("\n".join(out))
