# Phase Three: governed real-world control

Status: implementation candidate on `implement/p3-governed-real-world-control`
(base `forward/post-p2-hardening` `309f2c5a`), Linux support profile, under
Architect review. It is not integrated into `main` and is not a completed
phase. This document states what Phase Three governs, how, and what it does
not claim.

## 1. One authority architecture

Every real-world effect a Phase Three route can cause (a contained process,
a network request, a browser session, an observation of a display, mouse or
keyboard input, a connector operation with a credential) passes through one
pipeline in `crates/nexus-governed-control`:

1. **Data in.** A request is data: the owner's command text or voice
   transcript, an agent's `PlannedAction`, a structured intent from the
   interface. A string is never authority.
2. **Preparation.** The domain actuator resolves it into a canonical target
   identity (an origin, an executable identity, a window identity, a
   connector origin) and a digest of the exact parameters it will use.
3. **Commitment.** The authority records an `ActionCommitment` bound to the
   agent, the run, the capability kind, the effect class, the operation,
   the target digest, the parameter digest, the grants and credential leases
   it relies on, the policy generation and its deadline (the binding digest
   covers all of them). Evidence is recorded first: an unrecorded
   commitment is never created.
4. **Authorization.** R0 and R1 need a live owner grant that covers them. R2
   additionally needs the owner's native approval of exactly this commitment
   (its id and binding digest are shown in the dialog); the approval is a
   crate-private, non-`Clone`, non-deserializable value consumed by
   authorization.
5. **Execution, once.** Immediately before the effect, the actuator resolves
   the target again; the commitment is consumed exactly once (`begin`, under
   the registry lock) only if the target and parameter digests still match,
   the run is active, the deadline has not passed, the policy generation has
   not moved and every grant is live. The effect runs under a one-shot guard
   that observes the run's cancellation between bounded steps.
6. **Finalization.** The commitment ends succeeded, failed or cancelled
   (a dropped guard ends it as abandoned), evidenced, and its credential
   leases end with it.

There is no other way to an actuator: pending effects are private to the
pipeline, keyed by commitment, taken out exactly once; the commitment
lifecycle is crate-private.

### Effect classes

| Class | Meaning | Needs |
|---|---|---|
| R0 | local observation or computation | a live grant |
| R1 | bounded external or reversible effect | a live grant, commitment, revalidation, bounds, evidence |
| R2 | sensitive, irreversible or privileged effect | all of R1, plus the owner's exact, one-shot native approval |

### Grants, generation, cancellation

- Grants exist only after the owner's native confirmation of their exact,
  plain-text scope (at most 24 hours). Revoking a grant or an emergency stop
  moves the policy generation, which ends every unconsumed commitment.
- A run is the scope of one command or agent loop. Cancelling it sets its
  token (every executing effect observes it), ends its unconsumed
  commitments, and releases what it owns. Finishing a run does the same.
- The owner's emergency stop cancels every run, ends every unconsumed
  commitment, moves the policy generation, stops the agent display, and
  refuses new runs until the owner resumes through a native dialog.
- Nothing is ever killed by process name: every process Phase Three starts
  runs in its own process group, which is what ends.

### Evidence

Every phase (run opened or cancelled, grant issued, declined or revoked,
commitment prepared, approved, declined, authorized, denied, started,
finished, expired, revoked, lease issued, credential released, emergency
stop, resumed) is appended to the existing hash-chained audit trail as a
`p3.action.evidence` event before the step proceeds. Records carry
identities, classes, digests and bounded, plain text only: never a secret,
a request or response body, typed text, pixels or audio.

Everything a native dialog shows must be plain text: control,
bidirectional-override and zero-width characters are refused, so a target
or summary cannot read differently from what it is.

## 2. Domains

### P3-A tools (processes)

There is no shell, interpreter, container or code runner (`ShellCommand`,
`DockerCommand` and `CodeExecute` stay closed). A tool is code: a key, one
executable at a canonical absolute path, an effect class, a typed input
builder producing an argument vector and fixed-name input files, a static
environment allowlist, a deadline and an output bound. The owner's grant
pins the executable's identity (canonical path, device, inode, size, mtime,
mode, owner and SHA-256; production executables and every directory above
them must belong to root and be writable by no one else); preparation and
the launch both check it again. The launch uses the kernel's sealed spawn:
absolute program, cleared environment, a fresh private working directory,
its own process group with resource limits, no standard input; the deadline,
cancellation and output overflow end the whole group, and every exit reaps
it. Production tools: `text.sha256` (R0) and `speech.synthesize` (the local
`espeak-ng`, R1; audio returned as data, no file written).

### P3-B egress

URLs are parsed with the WHATWG rules (numeric host tricks become the
address they are), stripped of fragments, and refused with user
information, whitespace, control characters or any scheme other than https
(http only where a grant names it). Every address a destination resolves to
is classified; loopback, private, shared, link-local, unique-local and
site-local addresses need a grant that names private destinations; the
unspecified, multicast, broadcast, reserved, documentation and benchmarking
ranges and the deprecated IPv6 transition forms are never reachable;
IPv4-mapped, NAT64 and 6to4 addresses are classified by the IPv4 address
they embed; a mixed DNS answer is refused. Grants name exactly one origin
and its methods; GET, HEAD and OPTIONS are R1, the others R2. The
destination is resolved at preparation and again immediately before the
request, and the connection is pinned to exactly the addresses checked then
(the peer is verified). The transport ignores proxy variables, follows no
redirect, keeps no cookies, sends no referer and only allow-listed plain
headers, and bounds the body and the time; cancellation drops the exchange.
Redirects are followed only for safe requests without a credential, within
the origin, each hop checked again.

### P3-C browser

A browser session is one commitment: a start URL and at most 32 typed steps,
under the owner's browser grant naming the reachable origins and pinning the
browser executable's identity. The session is a fresh headless Chrome with a
new profile in a private directory deleted when it ends, driven over its
DevTools pipe (no debugging port), in its own process group. Its only way
out is a session-owned loopback proxy that admits connections to the granted
origins only, applying the egress address policy with the same pinning, so
subresources, redirects and page-initiated navigations cannot reach
anything else. Local schemes are refused before launch, new windows are
closed, downloads are refused or kept inside the session, steps run fixed
page scripts with JSON arguments (no model-written script runs), and
password and file inputs are never filled. Reading is R1; clicking,
filling and pressing are R2.

### P3-D perception and P3-E input

Both act only on the agent display: a backend-owned Xvfb on a random
display number that is never the owner's, with no TCP or abstract socket,
admitting only clients that present its private MIT-MAGIC-COOKIE; Nexus
connects with explicit authorization and never reads `DISPLAY` or
`XAUTHORITY`. Perception (R0, its own grant) binds the display instance and,
for a window, its id and geometry (a title only selects); the PNG is data
held in memory and only its digest and size are evidence. Input (its own
grant with a step budget) needs an observation of the run first and binds
the display instance, the window under the point and the run's latest
observation, all checked again immediately before the effect and before
every press (a window appearing over the point fails the action). Moves and
scrolls are R1; clicks, drags and keys are R2 unless the owner granted an R1
input session. Typed text is shown for approval and recorded only as a
digest. `ComputerAction` is an orchestrator: each step is its own
commitment.

### P3-F credentials and connectors

Secrets stay in the kernel vault (`SecretsFacade`, the verified `http`
connector scope); with no vault configured, every credentialed operation
fails closed. An operation gets a lease bound to the agent, the run, the
service, the one origin and the header it may go to, and an expiry; the
commitment lists the lease (never the secret). Only while that commitment is
executing, for its own origin, once, does the broker read the secret and
hand it to the transport, which sends it marked sensitive and redacts it
from anything echoed back. Connectors are code: one fixed origin and typed
operations whose requests must parse to that origin; an operation runs only
under the owner's connector grant naming it. The production catalog
migrates the Phase Zero email and messaging endpoints: Gmail and Outlook
list, search and send; Slack and Discord connection checks, reads and
posts. Reads are R1; sends are R2 with the exact recipient, subject and
body digest shown natively. Telegram is not migrated: its Bot API carries
the token in the URL path.

### P3-G front door

`GovernedControl` is the one object the desktop holds. Owner commands come
through a strict grammar (`fetch`, `browse`, `observe`, `hash`, `say`,
`connector`); a voice transcript is only text someone spoke; attachments
arrive only through the native picker, as data. Agents' `PlannedAction`s are
classified by an exhaustive, wildcard-free table: 10 inert actions keep
their routes, 16 are governed (R0 and R1 run under the owner's standing
grants; R2 waits for the owner's native approval in the interface; an
agent's path never raises the dialog itself), 20 stay closed. Without Phase
Three (unavailable) the executor is exactly the Phase Zero executor.

## 3. Direct-bypass closure (§22)

Every production route able to launch a process, reach the network, drive a
browser, capture the screen, move the mouse or press keys, use a credential
or run a connector effect has exactly one of four classes: **Phase Three
governed**, **existing stronger governed** (Phase Zero, One or Two
governance or an Architect decision, named per file), **intentionally
closed**, or **non-production**. There is no fifth.

`p3_g6_08_every_real_world_mechanism_in_the_workspace_is_classified`
(`app/src-tauri/src/phase3_tests.rs`) resolves every workspace production
source with the structural resolver introduced in XA-R4 (aliases and module
indirection resolve to the real path) and pins each file's mechanisms by
kind: process launch and termination, sockets and HTTP and WebSocket
clients, the X server, the credential vault (its global facade and the OS
keyring), sealed spawns, launching the OS opener, and direct screen, input,
clipboard, audio and browser-driver crates. At this candidate: 153 sites in
128 files, classed 7 Phase Three (exactly `crates/nexus-governed-control`),
33 existing governed, 82 closed and 6 non-production. A new mechanism
anywhere in the workspace fails the guard until it is classified.
`p3_g6_09` pins the desktop's reach into the embedded Nexus Code
application (it lists tools and configures a router slot; its router, tool
execution, MCP manager and self-improvement run only behind the closed
`nx_*` commands).

Routes closed or repaired by Phase Three (`Closure::GovernedRoute` unless
stated):

- **G-INV-1 and G-INV-4:** the six email and messaging commands (open sends
  and credential-bearing reads); their transport is removed and their
  endpoints run as governed connector operations.
- **G-INV-2:** `nx_computer_use_status`, a readiness probe that ran programs
  found on `PATH`; the agent display's status replaces it.
- **G-INV-3:** the seven legacy direct browser commands.
- **G-INV-5:** `builder_image_gen_status` (`Closure::HelperLaunch`): it sent
  a request to an unowned local service, following its redirects, and ran
  `which` from `PATH`, only to report on image generators that are stubs
  behind closed commands.
- **G-INV-6:** `marketplace_search_gitlab`, the fixed-host GitLab read,
  now follows no redirect, sends no Referer, ends within 30 seconds and
  reads at most 4 MiB.

## 4. Non-claims

- **Linux only.** Windows and macOS compile and fail closed; no security
  claim is made for them.
- **No owner-desktop observation or input.** Perception and input act only
  on the agent display; capturing or driving the owner's own session stays
  closed.
- **No microphone capture.** Voice enters only as a transcript (data);
  microphone capture is not a Phase Three route.
- **No sandbox beyond the process group.** Tools and session processes run
  in their own process group with resource limits; a workload that escapes
  its group (`setsid`) is not contained by Phase Three. Chrome keeps its own
  sandbox.
- **Root is trusted.** Executable identity relies on root-owned, non-writable
  locations; an attacker with root is out of scope.
- **The loopback proxy is unauthenticated.** It admits only the session's
  granted public origins, which any local process could reach directly.
- **Telegram is not migrated** and stays closed.
- **No economic autonomy**, payments, trading, self-modification,
  governance changes by agents, or Phase Four powers.

### Residuals in the existing-governed class

These routes are governed by earlier decisions, not by Phase Three
commitments. They are recorded rather than changed:

- **Hardware and disk probes** (`nvidia-smi`, `rocm-smi`, `lspci`, `dmesg`,
  `dmidecode`, `df`; the flash engine runs `dmesg` and `dmidecode` at
  startup): fixed program names and arguments found through `PATH`, under
  Architect decision A. Open IPC can run them, read-only, without a
  commitment.
- **Fixed-host reads:** the kernel WebSearch and the external-tools web
  search send the model's query text to their fixed search hosts by design
  (Phase Zero class A; §15 keeps them unweakened).
- **The operator's Ollama:** `delete_model` deletes a model from the
  authorized local Ollama address without a native confirmation
  (destination policy only, decision F), and the swarm's Ollama client
  follows redirects from that address (it carries no credential).
- **Credentialed provider calls** that predate Phase Three (LLM providers,
  swarm providers, Builder deploy reads with the stored deploy token) keep
  their Phase Zero bounds (fixed hosts, no redirect, size and time limits)
  and do not use the Phase Three broker. A swarm health refresh spends one
  small provider request.
