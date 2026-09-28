# Final-Gate findings, workstreams and file ownership

Base of every component branch: scaffold `63eb0d07` (parent `71c47acb`), which
adds one test-only guard module per workstream and labelled closure sections.
Component branches are local; the coordinator composes them into
`implement/p0-final-gate-closure` by ordinary merges.

Ownership rule: one owner per file. In the few files several workstreams must
change (`lib.rs`, `chat_llm.rs`, `apps.rs`, `crate_bridges.rs`, `agents.rs`,
`cognitive.rs`, `phase0_surface/tests.rs`), one owner per function region, as
listed. The coordinator owns the closure registry, workflows, `docs/security/*`,
composition, pushes and CI.

## S1 — item D: privileged webview, navigation, frames, IPC origin

Findings (read-only reconnaissance, pinned tauri 2.10.3 / wry 0.54.4):
- No app ACL manifest (`build.rs` calls bare `tauri_build::build()`), so Tauri
  checks no origin for application commands; the invoke key is the only gate.
- The key arrives through init scripts that run on every main-frame load,
  including after the main frame navigates to a remote page; on Windows
  (WebView2) they are also injected into every subframe.
- No navigation guard; note links navigate the top frame; no explicit
  new-window handling (WebKitGTK default create on Linux).
- Research view embeds arbitrary remote pages with
  `allow-scripts allow-same-origin allow-forms allow-popups`; a React-mode
  Builder iframe embeds a loopback dev server with `allow-same-origin`
  (currently unreachable from the UI); generated previews run `allow-scripts`.
- Monaco loads its script from a CDN into the privileged document (and embeds
  DOMPurify 3.2.7); Settings sends provider keys with `fetch` from the webview;
  CSP is `null`.

Disposition being implemented: Tauri app ACL restricting app commands to the
`main` window at the local origin; deny non-app navigation and all new windows;
remove remote/loopback frames; scriptless generated previews unless native
tests prove isolation on all three platforms; Monaco unavailable (no remote
script); restrictive CSP as defence in depth; native tests on Linux (Xvfb),
Windows and macOS.

Files: frontend `app/src/**`, `app/index.html`, `tauri.conf.json`,
`capabilities/*`, `build.rs`, `app/src-tauri/Cargo.toml`, a boundary module,
native tests; `lib.rs` `run()` main-window creation; the frontend-sink guard.

## S2 — items A, E, H: configuration key, vault key source, stored secrets

Findings:
- Any interface save can write new or changed credentials under a key derived
  from HOME/USER/USERNAME/HOSTNAME; an empty `NEXUS_CONFIG_KEY` yields a
  constant key; loads silently re-encrypt plaintext files.
- `NEXUS_ENCRYPTION_KEY` may be empty; `security.key_env` is ignored; the
  crypto module claims Argon2id but uses SHA-256 or raw bytes.
- The vault key file is read with no symlink, type, size, owner or permission
  check and no integrity check against existing rows.
- Deploy credentials are XORed with a host/user hash (recoverable from known
  plaintext); OAuth, messaging and API-client tokens are written in plaintext;
  backups copy them.

Disposition being implemented: `NEXUS_CONFIG_KEY` (non-empty) required for new
or changed credential writes; explicit tested legacy reads; no load-time
rewrites; validated `key_file` bound to the opened descriptor (Unix), refused
elsewhere; vault must open existing rows; OAuth start flows closed
(`SecretStorage`); messaging uses only the stored config token; deploy store
refuses; backups exclude credential stores; claims corrected.

Files: `kernel/src/{config,crypto,backup}.rs`, `kernel/src/startup/mod.rs`,
web-builder credential stores, `cli/src/setup.rs`,
`kernel/examples/dump_config.rs`; regions in `chat_llm.rs`, `apps.rs`, `lib.rs`.

## S3 — items B, F, C (argv), I: destinations, Nexus Link, credentials in argv, helpers

Findings:
- Caller-chosen destinations: `api_client_request`, four `a2a_*` and three
  `a2a_crate_*` commands, `mcp_host_connect`/`call_tool`,
  `builder_theme_extract_from_url`, `tools_execute` (`rest_api`, `webhook`,
  `file_storage`), agent web fetch (allowlists from the interface or stored
  rows), and a persisted Ollama URL that redirects all later LLM traffic.
- Nexus Link: an empty peer list admits every peer; no authentication.
- Credentials in curl argv: four hosted providers, perception, external tools
  (GitHub, Slack, Jira), MCP bearer tokens.
- `ensure_ollama` spawns an unowned, never-reaped `ollama serve`.

Disposition being implemented: close caller-destination routes
(`NetworkDestination`, `PeerTransfer`); Ollama authority only from
`OLLAMA_URL` or the fixed default; agent web fetch refused; in-process
transport (existing reqwest dependency) for credentialed provider calls with
no redirects; perception closed (`CredentialTransport`); credential-bearing
external tools refused; `ensure_ollama` never spawns (`HelperLaunch`).

Files: `connectors/llm/src/{providers/*,nexus_link.rs}`,
`crates/nexus-external-tools/**`, `protocols/src/mcp_client.rs`,
`kernel/src/actuators/web.rs`, desktop `governance.rs`, `tools_infra.rs`,
`model_hub.rs`; regions in `crate_bridges.rs`, `agents.rs`, `cognitive.rs`,
`chat_llm.rs`, `apps.rs`, `lib.rs`.

## S4 — items J2-J5: standalone surfaces

Findings: `nexus-protocols-server` and its identical `nexus-os` alias bind
`0.0.0.0:8080` with the full actuator registry behind JWT and an
unauthenticated frontend fallback; `nexus-cli` runs manifest shell commands,
live social posting and writes the desktop configuration; `nx` spawns helpers
at startup and reads working-directory `.nxrc` auto-approval; recipes and
packaging (root Dockerfile, Compose, Helm, Makefile, install scripts, systemd,
launchd, Homebrew, WiX, GitLab release job) install or run them. Alternate
entry points: `coding-agent`, `social-poster-agent`, the `nx-*` computer-use
harness.

Disposition being implemented: J1-pattern fail-closed entry points (status 69,
fixed message, reads nothing), alias binaries withdrawn, recipes and packaging
fail at their first step, active install/deploy documentation replaced;
libraries preserved and not claimed governed; developer/benchmark binaries
kept behind a binary-target inventory guard (Architect decision).

## S5 — items G and C5: approval channels, residual capability-measurement route

Findings: `cm_run_ab_validation` is open and builds a Groq-endpoint client
from `GROQ_API_KEY`, else the NVIDIA or OpenRouter key; approvals over IPC
create L6 agents; an unapproved L6 request is registered after restart; the
caller's approver string becomes the audit principal; `create_agent` accepts
any autonomy up to L5, which gates credential-bearing external tools.

Disposition being implemented: close `cm_run_ab_validation` and remove the
fall-through; L6 creation, activation, approval and restore unavailable (rows
untouched); inert nx approval commands and `self_rewrite_apply_patch` closed;
fixed backend label instead of caller approver names; truthful self-improve
records; EOF -> Abort pinned by the final guard.

## S6 — item K, resource bounds, DEP

Findings: unbounded caller counts in persona generation, parallel simulations
and dilated sessions; an every-second scheduler spawning overlapping loops;
unbounded frontend error log; budget file overflow that destroys history;
19 cargo-audit vulnerabilities on the candidate lockfile (identical to main);
14 npm findings, all dev tooling; voice STT probing CUDA through torch.

Disposition being implemented: refuse out-of-range values before work; at most
one scheduled tick per minute and none while a loop runs; bounded appends with
no deletion; minimal lockfile updates (crossbeam-epoch, h2 0.4, quinn-proto,
tar, rustls stack, plist) and removal of the unused `rmcp`; targeted npm
dev-tool updates; CTranslate2 device probing. Residual advisories go to the
Architect.
