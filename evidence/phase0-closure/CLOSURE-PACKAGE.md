# P0-FINAL-GATE-CLOSURE — Linux technical closure candidate

Internal consolidation for Architect review. Nothing here approves or completes
the Final Gate, FG1 or Phase Zero. Owner scope decision: Linux is the Phase
Zero active target; Windows/macOS are deferred (PLATFORM-SCOPE.md).

## Refs

| Ref | Commit |
|---|---|
| Authoritative `rebuild/phase0-trust-boundary` (unchanged by this stage) | `71c47acbf3f8ee8210109587b3229f8d89067b6b` |
| Linux candidate `implement/p0-final-gate-closure` | `9ed607f8ab7f54ef0e00b510db5703f5f5d78207`, tree `f3778691bda389487640c003e64c9b60bb7a5424` |
| Scaffold (common base of the components) | `63eb0d07` (parent `71c47acb`) |
| PR #15 | open, draft, unmerged, auto-merge off (untouched) |

225 commits over `71c47acb`; 198 files changed. The candidate composes, by
ordinary merges, seven local component branches (unpushed) and coordinator
commits:

| Component | Items | Head |
|---|---|---|
| S1 webview | D | `2a92a910` |
| S2 secrets | A, E, H | `2ef4ae6b` |
| S3 egress | B, C (argv), F, I | `353d5c68` |
| S4 standalone | J2-J5 | `d6bd03a2` |
| S5 approval | G, C5 | `93dbdbaf` |
| S6 reliability | K, DEP | `691456c1` |
| docs | docs/security | `fee7ec47` |

Coordinator commits on the candidate: closure registry entries, latent-API
needles and final trust-surface rows with a completeness check (every
fg_* guard pinned); consent-module use of the kernel bounds (`910063d0`);
L6-first scheduled tick (`636356fd`); composed sub-minute schedule test
(`55ae7959`); autonomous-loop interval bound (`4c310f77`); messaging clients
with no redirect and no Referer (`614fca08`); guard pin update (`fcc60e8a`);
CI steps for the live webview harness (`83712a86`).

## What changed (summary; details in docs/security and REVIEWS.md)

- **D** Application-command Tauri ACL (804 commands, main window, local
  origin); exact-origin navigation guard, new windows denied; CSP without
  third-party origins; frontend untrusted vectors removed; live native harness
  (dev and release profiles). Linux finding: wry attributes a postMessage to
  the webview's URL at handling time, so neither the ACL nor the guard is
  sufficient alone; the guard is load-bearing.
- **A/E/H** New or changed credentials only under `NEXUS_CONFIG_KEY`; no
  silent rewrite or overwrite; vault key file validated on the opened file
  (Linux/macOS), stored rows verified; deploy/Supabase/OAuth token storage
  closed; messaging and API Client secrets kept out of plaintext files;
  backups exclude credential stores; every protection change audited.
- **B/F/C/I** Caller-chosen destinations and peer transfer closed; Ollama
  authority only `OLLAMA_URL` or the default; credentials off process command
  lines; credentialed clients follow no redirect and are bounded; Nexus Link
  requires an explicit authenticated policy; no Ollama spawn or `which`;
  model downloads owned and ended at a normal exit.
- **G/C5** A/B validation route closed; caller approvals are not human
  approval; L6 agents unavailable on every route; Warden review fails closed.
- **J2-J5** 18 standalone binaries withdrawn on the J1 pattern (including six
  benchmarks that put provider keys on curl's command line); recipes and CI
  guarded.
- **K** Resource bounds before any work; test isolation from the real home.
- **DEP** cargo-audit 19 -> 7 vulnerabilities; cargo-deny advisory errors
  16 -> 10; npm (app) 14 -> 6; rsa removed with the unused openidconnect.

Closure registry: 804 registered commands, 154 closed (132 before), 650 open.

## Linux validation

| Check | Result |
|---|---|
| Local (coordinator, isolated HOME), `d1735577` + pin fix | fmt, clippy `-D warnings` clean; workspace 7,674 passed, 1 failed (pin fixed in `fcc60e8a`; desktop 442/0 after), 43 ignored; live harness dev/release ok; frontend ok |
| Fast-local #7 `36557040915` on `9ed607f8` | **success**: 7,675 passed / 0 failed / 43 ignored; 241/241 Final Gate guards; live harness dev/release ok; voice 27 OK; frontend 461/461 |
| Hosted #109 `36559976709` on `9ed607f8`, Linux jobs | **success** (same totals); Windows/macOS failed (deferred) |

## Internal reviews

Every component was adversarially reviewed by another workstream, repaired and
re-reviewed; an integration review of the composition found no blocker. See
REVIEWS.md.

## Remaining Linux blockers

None known. Items below need Architect disposition; none is a failing test.

## Architect decision requests (collected)

1. PATH as operator configuration for fixed-argument helpers (curl, probes,
   notifications; nexus-code `tools/search.rs` `which rg`).
2. In-process credential transport: the desktop build uses reqwest with
   native-tls (OS trust store) and system proxy settings (feature unification).
3. D3: 4 developer and 7 benchmark binaries, 5 bench targets, 2 examples kept.
4. Residual advisories: ammonia 4.1.2 x2, quick-xml 0.30.0 x2, wasmtime
   43.0.2 x2, h2 0.3.27; 13 unmaintained, 12 unsound; npm (app) 6; stale
   advisory ignores (rsa) in deny.toml, audit.toml, .gitlab-ci.yml; audit
   governance across four ignore lists; nexus-website and page-audit scope;
   unused `open` crate and unused frontend packages (Monaco, yjs).
5. `NEXUS_CONFIG_KEY` as the configuration key source; plaintext legacy
   configuration handling; key minimums; first-run creation on IPC-reachable
   loads; reopening token flows through an approved store; all-scope vault
   verification.
6. Item D: wry/tauri postMessage URL attribution (possible upstream issue);
   tauri channel-fetch ACL exemption (unused).
7. Item G: stored L6 records and pending transcendent requests (untouched);
   the "desktop-ui (unverified)" resolver label; the interface-owned Warden
   toggle; residual IPC consent channel for HITL-gated steps.
8. Item K: a stored agent whose schedule is now refused cannot be started;
   connectors-llm per-phase timeouts without response caps; default-redirect
   clients carrying Authorization (email, deploy, Slack/Discord) whose bodies a
   307/308 would re-send; voice real-transcription tests skip without a cached
   model; unpinned Python voice dependencies; host GPU driver mismatch (Owner).
9. ArenaRun compatibility API; one-token minimum budget.
10. FG1 post-integration evidence (#108) awaits Architect acceptance.
11. Windows/macOS portability stage (PLATFORM-SCOPE.md).
