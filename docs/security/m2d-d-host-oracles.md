# M2d-D host protocol and oracle registration

Base: `d20bc1f6ebe5e61561b85544633355f5db9891ce`. The Architect's
2026-11-04 decision 4 and its complete conflict inventory govern this slice.
This registration precedes implementation. Earlier tests remain frozen except
the explicitly quoted G1–G6 scanner amendments. No kernel change is authorized.

## Protocol

`mother-hands --home ABSOLUTE_HOME --workspace ABSOLUTE_EMPTY_FOLDER
--name NAME.txt --text TEXT [--hold-before-publish SECONDS]` is a deterministic
demonstration, with no model, shell, network, preview or content-file reader.
All four selections are required. Text is the exact UTF-8 argument, including
any final newline; the existing broker validates its name, size and language.
The hold is an integer from 0 through 30, configured before registration.
Both stdin and stdout must be terminals. Non-Linux startup refuses explicitly.
Hands establishes non-dumpable process state before constructing Home.

Startup shows recovery before admission, reserves Home's entire directory,
and registers one canonical workspace. It never restores workspace authority,
undo or an unresolved File mission from a previous process.

Commands are exact, lowercase, newline terminated, at most 64 bytes:

| Command | Operation |
|---|---|
| `run` | Once per process: ask for a 300-second Files grant, a Mother door, and mission consent; consume the original coordinator into its typed File session and advance deterministic lanes to the action wait. |
| `approve` | Obtain this mission's exact pending action from its original port; ask, authorize, execute and recognize the result. Continue only while host admission remains open. |
| `status` | Separate host restriction, actual kernel/broker Stop, worker ingress/completion, Mother settlement and uncertainty. |
| `evidence` | Bounded recent projection and primary health; explicitly not a complete verified durable chain. |
| `detail` | Take the Owner-only in-memory working detail. |
| `stop` | Memory-only emergency ingress; permanently enter recovery-only mode. |
| `lift-for-undo` | Only in recovery mode, with no outstanding Stop worker: ask for native lift for Owner undo only. The host restriction and lite latch remain set. |
| `undo` | Separate fresh confirmation for the last eligible file, once; report interruption even when removal succeeded. |
| `cleanup` | One bounded attempt through `files_retry_cleanup`; never lift Stop. |
| `quit` | Close admission, cancel dialogue, settle/join owned work, typed File close, broker shutdown, evidence close once. |

Unknown commands deny. Queued ordinary commands cannot survive emergency
ingress or revive eligibility. Recovery never calls `advance`, creates a new
door/run, admits Files/inference, invokes a lane or attempts a model call.
There is no model entry point in the product; gate tests use counters only.
The exact uncertainty notice is:

> A file may exist in your folder. A write already handed off may still finish. Mother will not retry or continue this mission.

## Input and output boundary

One reader owns stdin. It uses safe Linux terminal APIs, disables canonical
input, echo and output newline translation, preserves signal processing,
and restores the previous modes on orderly exit. Ctrl-C is not intercepted.
Ctrl-D is explicit EOF, including in the middle of a partial line.

Only one dialogue is active. Its deadline starts when its request arrives:
door/grant/undo/lift 120 seconds; consent/held resume 300; Files action the
minimum of 300 seconds and the backend's remaining commitment lifetime.
Rendering and freshness acknowledgement consume this same deadline.

Every header value is escaped. Details are plain, complete, fenced and
prefixed as data. Screen rejects raw C0/C1 other than LF and every character
rejected by `shown`; escaping also bounds combining clusters. Every output
route uses Screen, including lane text, failures, recovery, status and details.
No raw Debug or truncated approval facts are displayed.

A fresh UUID-v4-derived eight-lowercase-hex code is generated after the
display is fixed. Candidates colliding with any displayed untrusted text
are rejected; at most 16 attempts. Randomness failure denies the dialogue.
After complete output and successful flush, the reader drains already
available bytes, invalidates queued answers and marks a partial line stale.
Stop, Ctrl-D and errors still perform ingress during this drain. Only after
reader acknowledgement does the dialogue announce `INPUT READY` and accept
a fresh complete line. A stale partial line remains stale through its newline;
its tail cannot become another answer. No whitespace trimming occurs.

Line accumulation and the ordinary queue are bounded. Overlong lines are
discarded through their newline; invalid UTF-8, exhausted sequence numbers,
queue overflow, terminal-control failure, failed boundary acknowledgement,
write/flush failure and reader failure all deny. Fatal errors close admission
and cause bounded shutdown. Ordinary backpressure never blocks Stop ingress.

## Stop and shutdown ownership

Ingress first closes a monotone host gate, invalidates answers and advances
the receipt epoch. It calls neither broker nor coordinator and waits on no
output, filesystem, evidence or cleanup operation. Each Stop reserves one
owned worker, up to four; subsequent presses still perform ingress and
coalesce. Completed handles are reaped. Spawn failure, panic, poison and
counter exhaustion are visible failures, never evidence of backend engagement.

An off-reader snapshot corroborates actual kernel Stop/number and broker
Stop/generation. Worker return establishes only completion of its synchronous
recording phase. Durable/owed/uncertain status is reported separately.
Resume is refused while Stop workers remain outstanding; intervening Stop
invalidates a pending lift and undo confirmation.

One shutdown deadline covers all workers, including rendering, startup,
coordinator, execution, undo, cleanup and close. Timeout is nonzero and unclean.
No worker is declared joined until its handle finishes and is joined. Successful
typed File close precedes broker shutdown and confirmed `Recorded` evidence
close. EOF/read error performs emergency ingress before shutdown. A process
that exits with blocked work promises no eventual post-exit write.

## Frozen oracle groups and plants

| Group | Required observations | Negative plant |
|---|---|---|
| Guards G1–G6/G8 | Exactly named/path-pinned terminal confirmer; narrow walk delegation; desktop unchanged; exact FD/ExitCode exceptions; test modules unreachable in production; unsafe denied at every crate root | Second implementation, wrong path/type, always-yes body, broadened item exemption, production test inclusion |
| D-O1/3 | Composed fresh success with full facts/content, exact file bytes, terminal Mother result and records; wrong code, `no`, empty, whitespace and embedded code deny with zero publication | Accept wrong answer |
| D-O4/M5′ | Queued correct code, early input during held rendering, partial line, 65 bytes, invalid UTF-8, queue/sequence exhaustion, timeout, simultaneous dialogues, collision and exhausted randomness, write/flush/read errors | MD-10 queued acceptance; MD-17 EOF yes; omit serialization, deadline, collision or overflow denial |
| D-O13 | CSI, OSC52, C1, CR, BEL, bidi, zero-width and combining stack; fake Binding stays fenced/prefixed; every emitted application byte checked | MD-11 escaping removed |
| D-O14 | Independently reject nonterminal stdin/stdout; real PTY succeeds; mode ownership/restoration and explicit Ctrl-D | MD-12 stdout gate removed |
| D-O11/18 | Recovery shown without sweep, entire Home reserved, unsupported/noncanonical/nonempty workspace refusal, restart grants/undo not restored | Sweep or omit Home reservation |
| D-O19 | Full undo facts, exact removal, denied/edited/repeated undo refuses; report interrupted; Stop during dialogue invalidates it and retains later undo | Ignore interrupted or reuse old confirmation |
| D-O20/H1 | Exact before-publication readiness; Stop, Ctrl-D, master close and read error each ingress before bounded exit and prevent publication | MD-13 omit Stop; synchronous broker call on reader |
| H1 storage | Actual durable append/mutex held before Stop, own Stop write held, four held workers and fifth coalesced; second ingress and dialogue cancellation before release; actual backend halt; outstanding-worker resume refusal | Late engagement; cleanup on Stop; synchronous reader; unbounded handles; lift with worker outstanding |
| H1 shutdown | Held storage produces bounded nonzero process exit; separate in-process timeout then release/join establishes eventual records; successful Quit confirms session close | Claim clean on timeout or close evidence first |
| Recovery B | Real post-final-check boundary, emergency, one truthful publication, separate native lift and undo; successful removal; no continuation, doors/runs, Files binding, lanes, inference bindings or model attempts | One gate-bypass plant for each route |
| Recovery schedules | Queued/reserved work, emergency before File selection, repeated Stop during recovery and lift; retain lite latch and legacy positive controls | Clear monotone gate, current-Stop-only admission, queued callback bypass |
| D-O21/24 | Raw/encoded/fragmented canaries absent from journal, ledger, evidence including WAL/SHM; working detail only through Owner path; failure codes only | Persist raw content/detail |
| FileJournal V8 | File and parent directory durability before success for new and existing file; sync error propagated; concurrent creator cannot bypass; frozen symlink refusal | Omit directory sync, cache creator flag, ignore error |
| Close S9 | Active/uncertain/owed File work refuses clean close; once-only primary close after typed lifecycle; restart reporting remains truthful | Close evidence while lifecycle unfinished |

Native interposition fixtures may hold exact syscall boundaries in isolated
test children. They must preserve production admission and identify the held
file descriptor/path and boundary with a receipt. A projection stall is not
durable-storage evidence. A pre-check hold is not ordering-B evidence.
Broker test support, if needed, stays test-only inside that crate. No ordinary
product flag enables test hooks. Children use owned PTYs, bounded reads/writes,
strict receipts and owned kill/reap; no sleeps establish readiness. Each test
Home lives in its own process. Mandatory tests never skip unsupported storage.

Each plant is run red, restored and never committed. Logs remain in the
Owner-designated `m2dd` scratch directory. Baseline failure evidence must say
whether the missing host prevented a behavioral test from starting; it must
not mislabel unavailable code as an observed runtime security violation.

Final local gates are fmt, five-crate all-target Clippy with warnings denied,
workspace tests with no fail-fast, all 23 desktop live named receipts, diff
check, and the exact host-dependencies-only lockfile diff. The Architect owns
push/CI/PR; installed Stop/Ctrl-D acceptance is Owner-performed and OBSERVED,
not CI. C-11, portable support and capability acceptance remain unclaimed.

## Review 2 — Decision 6 (registered on 6ba3d000)

The worker-unscheduled ingress schedules use the real terminal Stop route
before draft and between draft/submit. Both must refuse, with zero backend
runs or model calls. Broker schedules engage the original memory-only File
cell while workspace validation is held, between validation and acquiring the
binding hold, and immediately before File/inference binding, run insertion,
and model-attempt admission inside the hold. No recording worker runs. The
broker's own table counts must be unchanged; no Files preparation or
commitment is allowed. Positive controls increment every backend count.

Startup is held at Home opening and recovery reporting. Two ingress events
and two worker outcomes each get exactly one receipt, including two recording
panics before startup finishes. Receipts must appear before releasing startup.
The renderer oracle interprets wire CR/LF cursor movement and requires column
zero for every row, with an LF-only negative control.

Plants (one at a time, uncommitted): backend engagement delayed to the worker;
after-validation re-check removed; host-only gate; startup-dependent receipt
reporting/event absorption; LF-only renderer. Broker checks already present
before this repair remain covered by the validation schedules and legacy tests.
