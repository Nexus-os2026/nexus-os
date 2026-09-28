# Decisions: coordinator applications of the contracts, and Architect requests

Coordinator decisions apply the approved contracts; each is open to Architect
review. "Architect decision" items are not accepted by the implementer.

## Coordinator decisions applying the contracts

- D (webview): enforce app-command origin with Tauri's own ACL (app manifest +
  capability limited to window `main`, local origin) in addition to
  navigation/new-window denial; new windows are denied rather than opened in
  the OS browser (that would add a helper launch, item I).
- A: `NEXUS_CONFIG_KEY` (valid UTF-8, not empty or whitespace) is the explicit
  key material for new or changed credential writes; a credential save with it
  re-keys an ambient file explicitly (reported and audited).
- H: OAuth start flows close (`SecretStorage`) rather than persist tokens;
  existing token files stay readable and untouched.
- E: key files are refused on non-Unix platforms.
- B/F: only `OLLAMA_URL` or the fixed default is Ollama authority; interface
  saves may not change the Ollama address; agent web fetch is refused;
  SearXNG requires `SEARXNG_URL`; Ollama pulls use the default registry only.
- C: perception closes rather than add a dependency; credential-bearing
  external tools are refused (also resolves the item-G finding on
  `tools_execute`).
- G: no autonomy-6 row is registered at restore (rows untouched).
- J: `coding-agent`, `social-poster-agent` and the `nx-*` harness are withdrawn
  as alternate entry points; the GitLab release job becomes a fail-closed stub.
- DEP: take the minimal compatible fixes listed in FINDINGS-MAP; do not take
  `ammonia` 4.1.4 (4 new crates, caller closed) or non-failing warning bumps.

## Requests for Architect decision (collected; final list at handoff)

1. I: treat the launch-environment `PATH` as operator configuration (as items
   E and I already describe), so fixed-argument helpers (curl, notifications,
   hardware probes, `open`) remain, while unowned launches are removed.
2. J: keep `nexus-ui-repair`, `scout`, `sg5_probe`, the swarm healthcheck and
   the 13 benchmark binaries as developer/benchmark-only, pinned by the
   binary-target inventory guard.
3. DEP residual advisories without a safe minimal fix: h2 0.3.27 (via
   readability -> reqwest 0.11), quick-xml 0.30.0 (developer-only
   nexus-ui-repair), rsa 0.9.10 (no upstream fix; signature verification
   only), wasmtime 43.0.2 (two advisories; fix is a major upgrade), and the
   unmaintained bitmaps/im-rc/sized-chunks via wasm-compose; `ammonia` 4.1.2
   (caller closed, vulnerable configurations absent).
4. DEP governance: `audit.yml` never scans the candidate, and four ignore lists
   (audit.yml, audit.toml, deny.toml, .gitlab-ci.yml) diverge.
5. npm residue: vite 5 / esbuild and vitest (dev-only majors).
6. `nexus-website` and `scripts/page-audit` findings: in or out of Phase Zero.
7. K: `executes_python_code` kept as monitored debt (not observed failing in
   43 hosted Windows logs); the host GPU driver mismatch is an Owner matter.
8. Python voice dependencies are unpinned and download models during tests.

## Coordinator decisions added during implementation

- G: prebuilt autonomy-6 manifests are not registered at startup either, and
  registered autonomy 6 is refused where a goal or tool call would run
  (internal review S4->S5, finding 1).
- G: Warden review enabled with no runnable Warden fails closed with a
  bounded reason instead of allowing (the setting defaults to off).
- I: in-flight model downloads are owned child processes, terminated and
  reaped at application exit with truthful failure reporting (I5); no
  cleanup by PID, name or port.
- Guards: every `fg_*` guard module keeps its guard code in
  `fg_<name>/tests.rs`, which the production-source scanners already skip;
  the scanners are not changed.
- K: the arena keeps S6's `ArenaRun` compatibility type (refusals surface as
  errors to its callers); the desktop uses the kernel's limits directly after
  composition. Presented to the Architect as a design note.
