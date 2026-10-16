#!/usr/bin/env python3
"""Build 04-FAST-LOCAL-MATRIX.tsv: every step of .github/workflows/ci-fast-local.yml at Candidate C (read from the Git
object, so each command is quoted exactly), with its evidence disposition. Read-only. argv[1] = mission root."""
import hashlib
import json
import os
import subprocess
import sys

import yaml

ROOT = sys.argv[1]
C = "d5964ee5326c2733177307db6f507328023d361a"
wf_bytes = subprocess.run(["git", "-C", "/home/nexus/NEXUS/nexus-os", "show", f"{C}:.github/workflows/ci-fast-local.yml"],
                          capture_output=True, check=True).stdout
wf = yaml.safe_load(wf_bytes)
V02 = "VALIDATION-02 receipts/30-stage-B-workspace-test (raw ce11ea91...9533; 32/32; MANIFEST 36f10bf9...b822)"
FMT = "CLOSURE-PACKET-01 FAST-LOCAL-3.1-RECEIPT.txt + receipts/50-55 (MANIFEST 372d1bc1...7f3f)"
CL = "this mission receipts/43b-clippy-all-features-r2 (raw 00495e2b...b241; 18/18)"
WT = "this mission receipts/42-worktree (24/24)"
NET = "requires network access (not authorised by this mission)"
# disposition per (job, step name): (status, evidence, discrepancy / missing requirement)
D = {
    ("fast-linux", "Checkout"): ("EQUIVALENT_LOCAL_ACTION", WT, "actions/checkout replaced by `git worktree add --detach` of C from the local object store (no network); not the action itself"),
    ("fast-linux", "Verify the tested commit and a clean checkout"): ("DISCHARGED_LOCAL_EXACT_COMMANDS", WT, "GITHUB_SHA taken as C; same git commands; status empty"),
    ("fast-linux", "Use the runner's installed Rust 1.94.0 (no rustup, nothing installed)"): ("DISCHARGED_LOCAL_EXACT_CHECK", WT, "the same seven executables checked in the local user's toolchain directory (not the runner user's)"),
    ("fast-linux", "Verify the toolchain and the Tauri system libraries"): ("DISCHARGED_LOCAL_EXACT_COMMANDS", WT, "same commands; same host as the self-hosted runner, different user"),
    ("fast-linux", "Use the persistent Cargo target cache (outside the checkout)"): ("NOT_APPLICABLE_LOCALLY", "-", "runner cache layout; this mission used a fresh mission-owned target (worktree/target)"),
    ("fast-linux", "Provide ripgrep 14.1.1 and verify search backends"): ("NOT_DISCHARGED", "-", f"ripgrep 14.1.1 absent locally (/usr/bin/rg is 14.1.0); the step installs it with `cargo install` from crates.io: {NET}; the runner's tool cache is another user's and unreadable"),
    ("fast-linux", "Check formatting"): ("DISCHARGED_BY_EXACT_SOURCE_RECEIPT", FMT, "exact command at exact C with the pinned 1.94.0 rustfmt; env -i with CARGO_TERM_COLOR=never, CARGO_NET_OFFLINE=true (workflow: always, unset); not run inside a memory-limited scope (time -v + timeout only)"),
    ("fast-linux", "Run clippy"): ("DISCHARGED_BY_THIS_MISSION", CL, "exact command; env -i with CARGO_BUILD_JOBS=2 (workflow: .cargo/config.toml jobs=8), CARGO_TERM_COLOR=never (always), CARGO_NET_OFFLINE=true (unset); fresh target instead of the runner cache; no rg on PATH needed by clippy"),
    ("fast-linux", "Run tests"): ("DISCHARGED_BY_EXACT_SOURCE_RECEIPT_WITH_ENVIRONMENT_DEVIATION", V02, "exact command at exact C (Rust 1.94.0 via the rustup proxy and rust-toolchain.toml, not the toolchain dir directly); PATH held /usr/bin/rg 14.1.0, NOT the workflow's ripgrep 14.1.1 (privacy regressions exercise rg); CARGO_BUILD_JOBS=2, CARGO_TERM_COLOR=never, CARGO_NET_OFFLINE=true; run in a separate target, not after this step order; memory.peak 16,384 B above memory.max (observation)"),
    ("fast-linux", "Live webview boundary harness (item D)"): ("NOT_DISCHARGED", "-", "needs a dev-profile test build of nexus-desktop-backend (closure: 983 of 1,026 resolved packages; VALIDATION-02's whole-workspace test target was 31,998,849,024 B) — cannot fit above the 16 GiB disk floor (headroom ~6.5 GiB after Clippy); not attempted; reusing another mission's target is not permitted"),
    ("fast-linux", "Setup Node.js 24.21.0 (Builder toolchain assembly)"): ("NOT_DISCHARGED", "-", f"Node 24.21.0 absent locally (node v20.20.2); actions/setup-node downloads it: {NET}"),
    ("fast-linux", "Assemble Nexus Builder toolchain"): ("NOT_DISCHARGED", "-", "needs Node 24.21.0 and the pinned Node archive (assemble.mjs checks its digest); toolchain unavailable; not run"),
    ("fast-linux", "Trusted Builder entry tests (packaged Node)"): ("NOT_DISCHARGED", "-", "depends on the assembled Builder toolchain; not run"),
    ("fast-linux", "Linux package inspection negative controls"): ("NOT_DISCHARGED", "-", "workflow runs it under Node 24.21.0 (set up earlier in the job); only Node 20.20.2 is available locally; a run under another Node version would not be equivalent; not run"),
    ("fast-linux", "Packaged Builder toolchain verification and governed launch"): ("NOT_DISCHARGED", "-", "depends on the assembled Builder toolchain and a nexus-desktop-backend lib test build (disk, as above); not run"),
    ("fast-linux", "Live webview boundary harness, release profile (item D)"): ("NOT_DISCHARGED", "-", f"`npm ci` ({NET}) and `npm run build`, then a custom-protocol nexus-desktop-backend test build (disk); not run"),
    ("fast-linux", "Security gate (cargo-audit 0.22.1 and cargo-deny 0.19.6)"): ("NOT_DISCHARGED", "-", f"pinned scanners exist locally (~/.cargo/bin: cargo-audit 0.22.1, cargo-deny 0.19.6) but the step fetches both advisory databases fresh: {NET}; a stale local database is not equivalent; not run"),
    ("fast-frontend", "Checkout"): ("NOT_DISCHARGED", "-", "job not run locally (its toolchain is unavailable)"),
    ("fast-frontend", "Verify the tested commit and a clean checkout"): ("DISCHARGED_LOCAL_EXACT_COMMANDS", WT, "the same check on the same commit (job-independent)"),
    ("fast-frontend", "Setup Node.js"): ("NOT_DISCHARGED", "-", f"Node \"22\" absent locally (v20.20.2); {NET}"),
    ("fast-frontend", "npm advisory gate tests"): ("NOT_DISCHARGED", "-", "requires Node 22; not run"),
    ("fast-frontend", "npm advisory gate (audited npm roots)"): ("NOT_DISCHARGED", "-", f"runs `npm audit` against the registry: {NET}"),
    ("fast-frontend", "Install frontend dependencies"): ("NOT_DISCHARGED", "-", f"`npm ci`: {NET} (a ~2.0 GiB local npm cache exists; `--offline` would change the command)"),
    ("fast-frontend", "TypeScript check"): ("NOT_DISCHARGED", "-", "depends on npm ci and Node 22; not run"),
    ("fast-frontend", "Run frontend tests"): ("NOT_DISCHARGED", "-", "depends on npm ci and Node 22; not run"),
    ("fast-frontend", "Build frontend"): ("NOT_DISCHARGED", "-", "depends on npm ci and Node 22; not run"),
    ("fast-python", "Checkout"): ("NOT_DISCHARGED", "-", "job not run locally (its toolchain is unavailable)"),
    ("fast-python", "Verify the tested commit and a clean checkout"): ("DISCHARGED_LOCAL_EXACT_COMMANDS", WT, "the same check on the same commit (job-independent)"),
    ("fast-python", "Setup Python 3.11"): ("NOT_DISCHARGED", "-", f"python3.11 absent locally (python3 is 3.12.3); actions/setup-python downloads it: {NET}"),
    ("fast-python", "Install voice dependencies"): ("NOT_DISCHARGED", "-", f"hash-pinned `pip install` from voice/*.lock: {NET}; Python 3.11 required"),
    ("fast-python", "Run Python voice tests"): ("NOT_DISCHARGED", "-", "depends on the 3.11 virtual environment; not run"),
}
rows = [["#", "job", "step", "exact command (from the workflow at C)", "status", "evidence", "discrepancy or missing requirement"]]
n = 0
seen = set()
for job, spec in wf["jobs"].items():
    for st in spec["steps"]:
        n += 1
        name = st.get("name") or st.get("uses")
        cmd = st.get("run") or ("uses: " + st.get("uses", "") + (" with " + json.dumps(st.get("with")) if st.get("with") else ""))
        if st.get("env"):
            cmd = "env " + json.dumps(st["env"]) + " :: " + cmd
        status, ev, disc = D[(job, name)]
        seen.add((job, name))
        rows.append([str(n), job, name, cmd.strip().replace("\n", " ⏎ ").replace("\t", " "), status, ev, disc])
missing = set(D) - seen
assert not missing, missing
out = "\n".join("\t".join(r) for r in rows) + "\n"
open(os.path.join(ROOT, "04-FAST-LOCAL-MATRIX.tsv"), "x").write(out)
summary = {}
for r in rows[1:]:
    summary[r[4]] = summary.get(r[4], 0) + 1
print("workflow sha256", hashlib.sha256(wf_bytes).hexdigest(), "steps", n, json.dumps(summary))
