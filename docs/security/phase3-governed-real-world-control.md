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
4. **Authorization.** Every commitment rests on at least one live owner
   grant of its own kind. R2 additionally needs the owner's native approval
   of exactly this commitment (its id and binding digest are shown in the
   dialog); the approval is a crate-private, non-`Clone`,
   non-deserializable value consumed by authorization, which is recorded
   before it takes effect. The confirmation is a window Nexus owns, not a
   system message box: monospace, never re-wrapping a line, scrolling both
   ways; Cancel is the default answer, and Allow is armed only after a
   one-second delay and once the end and the right edge of the text have
   been reached. It shows everything the approval allows, never shortened:
   a line is at most 96 columns and a longer value continues on `↳` lines,
   every request header is shown with its value, content (a body, filled or
   typed text) is quoted line by line behind `│` with backslashes, hidden
   characters and stacked combining marks shown escaped, so that none of it
   can pass for the window's own text, and an action that would need more
   than 192 lines is refused rather than cut.
5. **Execution, once.** Immediately before the effect, the actuator resolves
   the target again; the commitment is consumed exactly once (`begin`, under
   the registry lock) only if the target and parameter digests still match,
   the run is active, the deadline has not passed, the policy generation has
   not moved and every grant is live. The effect runs under a one-shot guard
   that observes the run's cancellation between bounded steps: a browser
   session checks it before every DevTools command it sends and at every
   step, a tool is not spawned once its run is cancelled and is polled
   every 10 ms after, and an input action checks it before every step.
6. **Finalization.** The commitment ends succeeded, failed or cancelled
   (a dropped guard ends it as abandoned), evidenced, and its credential
   leases end with it.

There is no other way to an actuator: outside the crate the only entry is
`GovernedControl`'s intent-level API; the pipeline, the domain
preparations, the pending effects and the commitment lifecycle are
crate-private, and pending effects are keyed by commitment and taken out
exactly once.

### Effect classes

| Class | Meaning | Needs |
|---|---|---|
| R0 | local observation or computation | a live grant |
| R1 | bounded external or reversible effect | a live grant, commitment, revalidation, bounds, evidence |
| R2 | sensitive, irreversible or privileged effect | all of R1, plus the owner's exact, one-shot native approval |

### Grants, generation, cancellation

- Grants exist only after the owner's native confirmation of their exact,
  plain-text scope (at most 24 hours); the dialog says that any agent may
  use the grant until it expires or is revoked. Revoking a grant or an
  emergency stop moves the policy generation, which ends every unconsumed
  commitment (recorded as it happens). Grants and commitments expire by the
  wall clock as well as the monotonic one, so a suspended machine does not
  stretch them. A browser session asks before its start page, before
  each step and after the last whether its grants are still live and the
  policy generation unchanged: a grant revoked or expired, or any policy
  change (another grant revoked, an emergency stop), ends it there, and its
  evidence says which. Input actions are bound to their display and target
  instead and are checked at every press.
- A run holds at most 32 effects waiting to execute (an R2 commitment
  waits for its approval), and agents together can never take the last 64
  places, which are kept for the owner's own commands.
- A run is the scope of one command or agent loop. Cancelling or finishing
  it sets its token (every executing effect observes it, and so does a
  browser session's proxy), ends its unconsumed commitments with their
  credential leases, and releases what it owns. A run whose agent loop or owner command has ended finishes as
  soon as nothing in it can start any more.
- The owner's emergency stop (the Phase Three control or the global
  emergency key, Ctrl+Alt+Shift+K) cancels every run, ends every
  unconsumed commitment, moves the policy generation, stops the agent
  display, and refuses new runs until the owner resumes through a native
  dialog. Every way the owner stops agents (Stop, the Admin "Stop all"
  and bulk Stop) works on the interface's IPC thread, before anything
  waits: everything every named agent runs or left waiting in Phase Three
  is cancelled first (a running effect is interrupted at its next step),
  then for each agent its schedule ends (under any spelling of its id), its
  loop's cancel flag is set, and the supervisor records the stop. A thread
  of its own then removes the agents' loops (only the loops they had then,
  never one started after the stop), which may wait while any agent's cycle
  holds the loop lock. A scheduled tick already under way when the owner
  stops the agent does not bring it back, and starts no loop for it.
  Ending an agent's goal cancels it in Phase Three at once and removes its
  loop the same way, under every spelling; it leaves the agent and its
  schedule, so a scheduled goal may start it again. A
  stopped or paused agent acts no more in Phase Three: the loop's bridge
  asks the supervisor whether the agent is running before every action,
  and refuses any action after a stop that came after the loop began.
  Pausing does not stop the agent's loop or schedule (they keep planning,
  and may use routes outside Phase Three) and does not cancel: an effect
  already executing runs to its end within its own bounds, and a paused
  agent's waiting R2 actions stay approvable until the owner denies them
  or cancels their run. The page's stop controls (the emergency stop,
  each run's Cancel, the display's Stop) stay usable while an approved
  effect runs: every command that reaches the agent loops' lock (consent
  decisions, goal assignment, review mode, HiveMind sessions) runs off the
  interface thread (`p3_g10_01` follows every chain of calls to it), and
  consent decisions are still made one at a time. Quitting the desktop
  refuses every new run and display start, cancels every open run, waits
  up to five seconds for executing effects and a display start under way
  to end, and stops the agent display. A display stop always counts: a
  start in progress, or one waiting for another to finish, ends unused.
- With the owner's Warden review enabled, an agent's governed action that
  the review covers is refused, as the kernel registry refuses it.
- Nothing is ever killed by process name: every process Phase Three starts
  runs in its own process group, which is what ends.

### Evidence

Every phase (run opened or cancelled, grant issued, declined or revoked,
commitment prepared, approved, declined, authorized, denied, started,
finished, expired, revoked, lease issued, credential released, emergency
stop, resumed, display started and stopped) is appended to the existing
hash-chained audit trail as a `p3.action.evidence` event before the step
proceeds; a display start that does not complete is recorded as ended, and
so is the display an emergency stop ends. Records carry
identities, classes, digests and bounded, plain text only: never a secret,
a request or response body, typed text, pixels or audio. A commitment's
parameters digest is salted with a nonce kept only in memory, so a record
cannot be used to guess low-entropy content (a PIN, a short message); a
request's path and query are not recorded at all (a URL may carry a token),
and a redirect records only its origin. A grant's scope is recorded line by
line, each line its own bounded field.

Everything a native dialog shows must be plain text: control,
bidirectional-override and zero-width characters are refused, so a target
or summary cannot read differently from what it is. Content shown in full
(a body, typed or filled text) has its hidden characters escaped, and a
character (grapheme cluster) shows at most three code points, the rest
escaped, so stacked marks cannot cover text (a value that begins with
marks may show them on the character before it, and some scripts' joined
letters show escapes); an escaped quote is written `\u{22}`, never `\"`.
Each line break of the content starts a new marked line (a trailing
Return shows as an empty marked line), and a window title is shown with
its own quotes escaped, exactly once. The owner's confirmation window lays every
line out left to right (right-to-left content cannot move where a line
starts), only one is open at a time, and its answer arms only after a
second, once the end and the right edge of its text have been reached;
each of the two is latched on its own, and line bounds count characters,
not rendered width (wide text may need more scrolling).

## 2. Domains

### P3-A tools (processes)

The runtime root holding every scratch directory is refused when a
directory above it could be changed by another user (owned by someone other
than root or the user, or writable by others without the sticky bit), so
no one else can swap a scratch directory for one of theirs. Directories
Nexus creates on the way are private (0700). A `~/.nexus` that already
exists group-writable (made under umask 002) leaves Phase Three unavailable
until it is made private.

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
it. Session processes (the agent display, the browser) are started by the
crate's own launcher, and only by the display and the browser, each with
the executable it pinned: absolute program, an explicit environment, their
own process group; every descriptor above the ones they inherit is
close-on-exec, the X server is given a second to remove its lock and socket
before the group is killed, and the browser's group is killed at once.
Session processes have no resource limits of their own (see §4); at most
four browser sessions run at once. Production tools: `text.sha256` (R0)
and `speech.synthesize` (the local `espeak-ng`, R1; the audio is held in
memory and reported to the agent by its size, and no file is written).

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
(the peer is verified, and a peer the client cannot name is refused). The transport ignores proxy variables, follows no
redirect, keeps no cookies, sends no referer and only allow-listed plain
headers (each shown with its value for approval), and bounds the body and
the time; cancellation drops the exchange.
Redirects are followed only for safe requests without a credential, within
the origin, each hop checked again. Every address of an answer is
classified before any is set aside.

### P3-C browser

A browser session is one commitment: a start URL and at most 32 typed steps,
under the owner's browser grant naming the reachable origins and pinning the
browser executable's identity; every file of its installation directory
must belong to root and be writable by no one else, not only the
executable. The session is a fresh headless Chrome with a new profile in a
private directory deleted when it ends, driven over its DevTools pipe (no
debugging port), in its own process group, with a `PATH` of an empty
session directory (a page cannot reach the system's opener through it).
Its only way out is a session-owned loopback proxy that admits connections
to the granted origins only, applying the egress address policy with the
same pinning, so subresources, redirects and page-initiated navigations
cannot reach anything else; the proxy holds at most 64 connections and
ends them with the session. Local schemes are refused before launch, every
other page is listed and closed before each step, downloads are refused or
kept inside the session, steps run fixed page scripts with JSON arguments
in an isolated world of the page (no model-written script runs, and nothing
the page redefines changes what they see), a step's selector must match
exactly one element and may hold no comment, and password and file inputs
are never filled (checked after the field takes focus). A read the page
interrupts by changing under it is asked again; an action never is.
Chrome's own temporary files stay in a private directory of the session,
QUIC is disabled, and only the DevTools events the session uses are kept,
bounded. A stop is checked before every DevTools command, so nothing a
session would do after it is sent. The proxy goes no further on any
connection once the run is cancelled or the session's grant is revoked or
expires (checked after a request is read, after it is admitted, between
connection attempts and before every write upstream; data already handed
to the kernel, or a write already under way, may still complete), and then
carries nothing more, ending every tunnel (a connection already being
opened may complete its handshake, and carries nothing); it keeps its port,
closing every new connection unserved, until the session ends, so no other
process can take the port while the browser uses it. Once a session has
reached its start page, a failure or a cancellation is recorded with where
it stopped: at the start page, at a step, or after the last. Reading is R1; clicking, filling and
pressing are R2.

### P3-D perception and P3-E input

Both act only on the agent display: a backend-owned Xvfb on a random
display number that is never the owner's, with no TCP or abstract socket,
admitting only clients that present its private MIT-MAGIC-COOKIE; Nexus
connects with explicit authorization, only to a socket this user owns in
a socket directory no other user can replace entries of and only while its
own server runs, and never reads `DISPLAY` or `XAUTHORITY` (that one
connection constructor is pinned, `p3_g9_02`). Starting the
display needs a live perception or input grant and no emergency stop; it is
recorded before the server is launched and is serialized against a stop,
so a stop always wins. Perception (R0, its own grant) binds the display instance and,
for a window, its id and geometry (a title only selects); the PNG is data
held in memory and only its digest and size are evidence. Input (its own
grant with a step budget) needs an observation of the run first and binds
the display instance, the window under the point and the run's latest
observation, all checked again immediately before the effect and before
every press, with the X server held so that nothing can intervene between
the check and the event (a window appearing over the point fails the
action); a press is checked where the pointer actually is; keys must reach
the bound window (the keyboard focus follows the pointer or is that window,
and keys aimed at the display background need the focus on no other
window); an active keyboard or pointer grab by anyone fails the action; a
drag is bound to the window it is dropped on as well, and its release is
checked at the drop point under the same hold; whatever an action pressed
is released on every way out of it, and one action runs at a time. A
button an interrupted action still holds is let go, with the server held,
on the window it was pressed on (at the press point, or another point
where that window is still on top); if that window is entirely covered, on
the bare display; only if neither exists is it let go at the press point,
after an Escape (which cancels a drag in most toolkits) if the keyboard
focus is on the window it was pressed on (never a key that follows the
pointer onto another window). The grant's step budget is spent at the effect.
Moves and scrolls are R1, and the input grant says so; clicks, drags and
keys are R2 unless the owner granted R1 input. Typed text is shown for approval and recorded only as a
digest. `ComputerAction` is an orchestrator: each step is its own
commitment.

### P3-F credentials and connectors

Secrets stay in the kernel vault (`SecretsFacade`, the verified `http`
connector scope); the desktop can only choose `Vault::Kernel` or
`Vault::Disabled` (no other secret reader can be passed in, and the raw
transport is crate-private); with no vault configured, every credentialed
operation fails closed, and a secret the facade would resolve from the
process environment is refused. An operation gets a lease bound to the agent, the run, the
service, the one origin and the header it may go to, and an expiry; the
commitment lists the lease (never the secret). Only while that commitment is
executing, for its own origin, once, does the broker read the secret and
hand it to the transport, which sends it marked sensitive and redacts it,
as the header value, the bare token, and the token JSON-escaped or
percent-encoded, from the body, the location and the content type of
what comes back; the forms it searches for and the replaced text are
zeroized, and so is every buffer Nexus collects a response body in (the
transport's chunks and the HTTP and TLS stack's own buffers are not; once
returned, the body is a plain vector, zeroized only where redaction
replaced it). A lease no commitment will list (its preparation failed or was
refused) ends at once, and the broker checks its capacity before recording
a lease. Connectors are code: one fixed origin and typed
operations whose requests must parse to that origin; an operation runs only
under the owner's connector grant naming it. The production catalog
migrates the Phase Zero email and messaging endpoints: Gmail and Outlook
list, search and send; Slack and Discord connection checks, reads and
posts. Reads are R1; sends are R2 with one bare recipient address (no
display name, quotes, brackets or lists), the subject and the whole text
shown natively. The account in a connector grant is only the owner's
label: the connector uses its one stored credential. Telegram is not migrated: its Bot API carries
the token in the URL path.

### P3-G front door

`GovernedControl` is the one object the desktop holds. Owner commands come
through a strict grammar (`fetch`, `browse`, `observe`, `hash`, `say`,
`connector`); a voice transcript is only text someone spoke; attachments
arrive only through the native picker, as data, and are used once. A
submission runs off the interface thread. The crate items the desktop may
name are pinned. Agents' `PlannedAction`s are
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
source (and the production modules named like test files) with the
structural resolver introduced in XA-R4 (aliases and module indirection
resolve to the real path) and pins each file's mechanisms by kind: process
launch and termination, raw system calls, sockets and HTTP and WebSocket
clients, the X server, the desktop bus (D-Bus, AT-SPI), loading a shared
library, the credential vault (its global facade and the OS keyring),
sealed spawns, launching the OS opener, name resolution (std's
`ToSocketAddrs`, a `to_socket_addrs` or `socket_addrs` call written as a
method or through any path, libc's resolver functions and the DNS
crates), network-capable crates in the production dependencies (the
Prometheus exporter, with the builder methods that start its listener or
push gateway counted as calls, and the Hugging Face hub client), and direct screen,
input, clipboard, audio and browser-driver crates, the kernel's unsealed spawn
(`ResourceSpawnSpec`, `ResourceProgram`), nix and rustix sockets and
process calls, sysinfo signals (and any kill in a file that lists
processes with sysinfo), inline assembly and input device nodes. It fails
closed on shadowing: a path is also measured as written, so a module or
binding named like a mechanism crate, even one in a test module, hides
nothing. Every "latent" classification names its entries
(`LATENT_ENTRIES`: the module, type or function a caller must name, and a
few method names), and a reference to one from any production source in
the workspace is itself a pinned `latent` site: a new caller anywhere fails
until it is classified. At this candidate: 180 rows over 149 files, classed
8 Phase Three (exactly `crates/nexus-governed-control`), 35 existing
governed, 97 closed and 9 non-production. A new mechanism anywhere in the
workspace fails the guard until it is classified.

The file set is the module tree (`p3_g6_13`): every production `mod`
declaration, `#[path]` included, is followed from the crate roots and must
load a source the guards scan, every target a manifest names is below its
crate's `src`, and the only production `include!`s are the two generated
toolchain manifests, pinned. `p3_g6_14` pins the production `macro_rules!`
and refuses a macro fragment in a path, as a trait, as a method or as a
macro name. `p3_g6_10` pins the files that declare foreign functions
(name-skipped modules included, with or without an ABI string), refuses
`#[link_name]` and any foreign declaration of a process, network (name
resolution and the resolver's query functions included), signal,
raw-syscall or loader function (a local `extern` would escape the
resolver). `p3_g6_09` pins the desktop's reach into the embedded Nexus Code
application (it lists tools and configures a router slot; its router, tool
execution, MCP manager and self-improvement run only behind the closed
`nx_*` commands). `p3_g6_11` pins that the global emergency key, every stop
route and quitting reach Phase Three, and that a paused or stopped agent
acts no more. `p3_g6_15` pins the non-forgeability doctests with their
error codes, `p3_g6_16` that test-only features are enabled only by tests
(so the production scanners may leave their items out), `p3_g6_17` that
only the browser and the display launch session processes, `p3_g9_01` that
a response body lives only in zeroizing buffers (and an unknown peer is
unpinned), and `p3_g9_02` that the agent display's X connection is made
only on its checked socket, with its cookie, `p3_g9_03` that a browser
session goes out only while it may, and `p3_g10_01` that no command the
interface thread runs, in any desktop file, reaches the agent loops' lock
through any chain of desktop functions.

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

- **Linux only.** Windows and macOS compile and fail closed: on Windows
  the runtime root cannot be made private, so no control is built; on
  macOS no program can be pinned; and off Linux the owner's grant and
  approval dialogs refuse. The desktop's end-to-end Phase Three tests run
  on Linux only. No security claim is made for either platform.
- **No owner-desktop observation or input.** Perception and input act only
  on the agent display; capturing or driving the owner's own session stays
  closed.
- **No microphone capture.** Voice enters only as a transcript (data);
  microphone capture is not a Phase Three route.
- **Same-user attackers are out of scope.** Another process of the same
  user can reach what that user can (the agent display's socket, the
  loopback proxy, the session directories).
- **Chrome is trusted to honour its flags.** The proxy, QUIC, WebRTC and
  DNS-prefetch settings bound what the browser reaches; they were not
  verified in a separate network namespace.
- **A plain-HTTP proxied connection is checked once.** The browser proxy
  checks the first request on a plain-HTTP connection and asks the
  upstream to close after it; a request a non-compliant upstream keeps
  alive for reaches only that already checked, granted address.
- **Speech text is parsed by `espeak-ng`.** It runs without SSML; its
  parser was not fuzzed.
- **Zeroization is Nexus's own.** Nexus's copies of a released secret are
  zeroized; the HTTP and TLS stack's copies are not under its control.
- **Tool processes rely on close-on-exec.** The kernel's sealed spawn
  starts tools without closing descriptors Nexus might hold without
  close-on-exec; the session launcher does close them.
- **No sandbox beyond the process group.** Tools and session processes run
  in their own process group with resource limits; a workload that escapes
  its group (`setsid`) is not contained by Phase Three. Chrome keeps its own
  sandbox.
- **Root is trusted.** Executable identity relies on root-owned, non-writable
  locations; an attacker with root is out of scope. A host where anyone
  else could replace a pinned program cannot use it: the GitHub-hosted
  runner image makes `/opt` world-writable, so CI restores Chrome's
  packaged modes, and checks them, before the live browser tests.
- **The loopback proxy is unauthenticated.** It admits only the session's
  granted public origins, which any local process could reach directly.
- **Telegram is not migrated** and stays closed.
- **Drags whose toolkit maps a drag image under the pointer fail
  closed.** The release is checked against the windows at the drop point,
  and a drag image there reads as a changed target.
- **Confirmation windows queue.** One is open at a time; a later one
  (Resume included) waits until the open one is answered, and an emergency
  stop does not close an open window: the action it would approve was
  revoked by the stop and is refused, but a grant or Resume window still
  takes effect if answered.
- **Other slow interface commands can still hold the interface thread**
  (a model call or a download, not the agent loops' lock). The global
  emergency key works meanwhile on X11; the page needs the interface
  thread.
- **A display Stop that comes before a pending start begins is overtaken.**
  A start waits for the runtime to pick it up; a Stop pressed before then
  finds nothing to stop, and the display then starts (and is recorded):
  pressing Stop again stops it.
- **Quitting during a display start longer than its wait** leaves that
  start's record without an end (the display ends with the desktop).
- **Window titles may contain characters that look like quotes.** The
  window's id at the end of its line is what identifies it.
- **The global emergency key is X11-only.** It is registered with an X11
  key grab: under Wayland it fires only while an XWayland window has the
  focus. The Phase Three page's emergency stop works under both.
- **Another local user can keep the agent display from starting.**
  Creating the display's lock, socket or keymap names in `/tmp` for every
  number it tries makes each start fail (availability only: Nexus uses only
  a socket this user owns, in a directory no one else can replace entries
  of).
- **A page can steer approved steps within its granted origins.** It can
  move the focus, take trusted key presses into a frame of a granted
  origin, or navigate between granted origins; it reaches nothing else.
- **Chrome's crash handlers leave the session's process group.** Chrome
  starts two crashpad handlers per session, which daemonize out of it.
  They end with the browser (a live test checks that no process whose
  command line names the session's directory outlives a stopped session),
  and their uploads need a consent the fresh profile never gives. No switch of the pinned Chrome removes them without
  breaking page loads: `--disable-crash-reporter` has no effect and
  `--disable-crashpad-for-testing` aborts navigations (checked live with
  Chrome 149).
- **A reading browser session can still write.** A session is R1 by its
  steps, not by its traffic: the page's own scripts can send any request,
  writes included, to a granted origin through the proxy. The fresh profile
  has no cookies or stored credentials.
- **Session processes have no resource limits.** The X server and Chrome
  run without per-process limits; concurrent browser sessions are capped at
  four and there is one agent display.
- **A crash leaves debris.** Quitting stops Phase Three cleanly; a crash
  leaves the display's lock and socket, session profiles and scratch
  directories (all private to the user) until removed.
- **"Observe before input" is procedural.** Input needs an observation of
  the same run and display generation, not one that shows the target; a
  window observation captures what is on screen in its rectangle.
- **Passive grabs and RECORD are not detected.** On a display other clients
  could join, a passive key grab or the RECORD extension could see or take
  input; today only Nexus can connect to the agent display.
- **Governed actions spend no kernel fuel.** The agent loop's capability
  and approval gates still apply.
- **The emergency key stops Phase Three and the legacy engines.** Agent
  loops' model and WebSearch calls continue until the loops are stopped.
- **Destinations are resolved before authorization.** DNS lookups at
  preparation (before an R2 approval) and again before the request are not
  evidenced.
- **An expired detached run settles lazily.** Its expiry evidence is
  recorded when the run is next touched.
- **The guards read Rust.** The llama-bridge build script and its C stubs,
  and build scripts generally, are outside them (no process, network or
  loader call in the C code today); the two generated manifests that
  production includes are pinned. The latent guard sees paths and a few
  named methods, not every method call on a value.
- **CI compiles Linux only.** Windows is type-checked and the CRLF checkout
  simulated locally for every candidate. Under CI, the live display,
  browser and speech tests fail rather than skip if their program is
  missing or refused. Rustdoc checks the doctests' error
  codes only on nightly; on the pinned stable toolchain their text is pinned
  and the codes were verified once with rustc.
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
  (Phase Zero class A; §15 keeps them unweakened). The kernel WebSearch
  follows https redirects from those hosts to any host (`curl -L
  --proto-redir =https`), so a search engine's redirect would carry the
  query there; DuckDuckGo answered a bang query without one when checked.
  This is Phase Zero's design (P0-002C5B), recorded for the Architect.
- **The operator's Ollama:** `delete_model` deletes a model from the
  authorized local Ollama address without a native confirmation
  (destination policy only, decision F), and the swarm's Ollama client
  follows redirects from that address (it carries no credential).
- **Credentialed provider calls** that predate Phase Three (LLM providers,
  swarm providers, Builder deploy reads with the stored deploy token) keep
  their Phase Zero bounds (fixed hosts, no redirect, size and time limits)
  and do not use the Phase Three broker. A swarm health refresh spends one
  small provider request. LLM keys are placed in the process environment
  (Phase Zero), so unsealed kernel children inherit them; Phase Three's
  children start with cleared environments.
- **The GitLab marketplace search** honours proxy variables; it carries no
  credential.
