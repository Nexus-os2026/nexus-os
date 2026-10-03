# Phase Two — Governed Verification Execution (Linux support profile)

Status: **implementation in progress** under the Architect-approved mission
P2-IMPLEMENT. Nothing in this record declares Phase Two complete; completion
needs the dedicated supported-host sandbox run, normal hosted CI, the
Security Audit and Architect review.

Base: `main` = `e47bf65788247946eb8401138f57435e7c1680fc` (tree
`1ea52b426ae859aaa1dadea0a0c8f1726e513376`). The four closure refs
(`rebuild/phase0-trust-boundary`, `evidence/phase0-closure`,
`rebuild/phase1-governed-coding`, `evidence/phase1-closure`) are immutable.

## 1. Goal

An exact Phase One candidate (a run in `StructurallyVerified`) may execute
project-owned verification code inside a backend-owned Linux sandbox without
granting the frontend, the model or the candidate ambient host authority.

```text
StructurallyVerified candidate
→ backend verification request
→ native owner confirmation (single-use, bound)
→ candidate materialization (exact manifest re-checked)
→ verified packaged verifier toolchain
→ backend-owned verifier profile
→ cgroup-owned trusted sandbox helper
→ mandatory Linux isolation layers (all or nothing)
→ bounded execution
→ truthful cleanup
→ candidate/profile/toolchain/policy-bound result
→ backend review (result bound into the review binding)
→ owner approval / apply (verification is advisory)
```

This is not a shell, a terminal or arbitrary process execution. Calling code
a "test" grants nothing: tests, proc macros and anything they spawn are
untrusted code.

## 2. A string is never authority

No authority is derived from frontend paths, project paths, executable text,
caller argv, model output, environment strings, PIDs, systemd unit names,
cgroup path strings, UUID strings, serialized JSON, candidate content, profile
names, toolchain names or a test result. Identifiers may locate
backend-owned objects only after the backend proves their retained authority.

## 3. Mandatory isolation (no degraded mode)

A production launch succeeds only if every layer below is established and
independently verified; otherwise verification is **unavailable**. There is
no production fallback to process groups, rlimits, Landlock only, seccomp
only, systemd sandbox properties, Bubblewrap, Docker, a shell or an
unsandboxed subprocess.

1. Linux only (other platforms: unavailable).
2. Dedicated trusted helper process (`nexus-verifier-sandbox`).
3. New user namespace (identity uid/gid map written by the parent for the
   retained, unreaped helper child).
4. New PID namespace; its PID 1 is trusted Nexus code.
5. New network namespace (loopback left down).
6. New IPC namespace.
7. New UTS namespace.
8. New cgroup namespace (entered after the helper is in its scope).
9. Landlock, strict (`HardRequirement`), host ABI ≥ 6, fully enforced.
10. seccomp-BPF production allow-list.
11. `PR_SET_NO_NEW_PRIVS`.
12. Backend-owned cgroup v2 limits (transient systemd-user scope).
13. Explicit inherited-fd closure (`close_range`).
14. Sealed environment (exact backend-constructed variables only).
15. Verified candidate workspace.
16. Verified packaged verifier toolchain.
17. Backend execution generation and lifecycle ownership.
18. Native owner launch approval.

## 4. Explicit non-claims

On the audited host (Ubuntu 24.04, kernel 6.17, AppArmor
`apparmor_restrict_unprivileged_userns=1`) an unprivileged mount namespace
cannot be used, so Phase Two v1 does **not** claim:

- a private mount view or private `/proc`;
- metadata confidentiality for host path names (`stat`, existence, symlink
  targets are not mediated by Landlock);
- protection against kernel vulnerabilities or root compromise;
- protection against malicious same-UID host processes modifying user-owned
  state outside Nexus's retained identities.

It **does** require that project code cannot read, write or execute host
content outside the explicit allowed roots, cannot reach any network or
local socket, cannot see or signal host processes, cannot create
namespaces, and cannot outlive finalization. Verifier access to `/proc` is
denied.

## 5. Process model

```text
desktop verification driver (long-lived thread; owns the execution)
  └─ nexus-verifier-sandbox helper (single-threaded, trusted)
       └─ PID-namespace init (PID 1, trusted, reaps, exits last)
            └─ verifier (profile executable; untrusted tree)
```

- The driver spawns the helper with a cleared environment, no arguments,
  cwd `/`, stdin = one end of a `SOCK_SEQPACKET` socketpair (the private
  control channel), stdout/stderr = the output pipes. No `pre_exec`.
- The helper checks it is single-threaded, closes every descriptor above 2,
  sets `PR_SET_PDEATHSIG(SIGKILL)` and confirms its parent is alive. The
  spawning thread is the driver thread that lives until finalization (the
  death signal follows the parent *thread*).
- The driver places the helper in a new transient scope (§9) and proves the
  membership from the kernel's view of the retained, unreaped child, and
  the manager's unit bound to exactly that cgroup, before sending the
  launch message. From before its request is issued, the possible scope is
  owned as a pending scope operation, beside the helper, until it is proven
  or confirmed gone (P2-V1-R3B-I4, -R1, §9).
- The helper validates the typed launch message (bounded sizes, at most 32
  descriptors each of the kind its role requires, compiled policy hash must
  match), `unshare`s the six namespaces in one call, reports, and waits
  while the driver writes `setgroups=deny` and identity `uid_map`/`gid_map`
  for that exact child. It then verifies the map, and that its user,
  network, IPC, UTS and cgroup namespaces changed, by reading (never
  following) the `/proc/self/ns/*` links: on this host AppArmor's
  `unprivileged_userns` profile denies access to the namespace files
  themselves. The init verifies it is PID 1 in a different PID namespace and
  that its network namespace has only a down loopback.
- The helper forks the PID-namespace init. Init forks the verifier child,
  which: gives itself `/dev/null` as stdin, applies `no_new_privs`, strict
  Landlock, then seccomp, closes every descriptor except 0–2 and the verified
  executable descriptor, verifies each layer, and `execveat`s the verified
  executable by descriptor. A failure before exec is reported through a
  close-on-exec pipe; nothing untrusted has run.
- Init reaps every child, reports the verifier's exit status, and exits;
  the kernel then kills anything left in the PID namespace. The parent-death
  chain (driver → helper → init) and the scope's runtime backstop cover
  crashes. `cgroup.kill` is the finalization backstop.
- A panic never abandons a live execution (P2-R1, section 9): the helper,
  the scope (pending or proven) and the output threads are owned outside
  every closure that can panic, and one finalizer runs whether the launch
  and the wait return or panic.

## 6. Landlock policy (roles → rights; host ABI ≥ 6, handled rights = all ABI 1–5 filesystem rights, TCP bind/connect, both ABI 6 scopes)

| Role | Rights |
|---|---|
| Toolchain root | read file, read dir, execute |
| Runtime file (exact root-owned files, see §10) | read file (the dynamic loader also execute) |
| Candidate `input/` | read file, read dir |
| `target/` | read, write, create/remove dir and regular file, refer, truncate, execute |
| `home/`, `tmp/`, `cargo-home/` | the same without execute |
| `/dev/null` | read, write |
| `/dev/urandom` | read |

No other path is granted: not `/home/<user>`, `~/.nexus`, `.ssh`, credential
files, the user D-Bus, the SSH agent, the Docker socket, arbitrary `/tmp`,
arbitrary `/run/user/<uid>` or `/proc`. No TCP bind/connect rule exists, and
abstract Unix sockets and signals are scoped to the sandbox. On ABI 7 there
is no pathname-Unix-socket or UDP restriction, so seccomp's socket policy is
mandatory.

## 7. seccomp policy

A compiled allow-list (seccompiler, `x86_64`), installed after
`no_new_privs` and Landlock:

- architecture pinned (other architectures killed); x32-ABI syscall numbers
  killed by an explicit prefix;
- unknown and unlisted syscalls fail closed (`ENOSYS`, which also makes
  `clone3` fall back to the inspectable `clone`);
- `clone` only without any namespace flag; `socketpair` only for `AF_UNIX`;
  `prctl`, `ioctl`, `prlimit64` and `fcntl` restricted by argument;
- never allowed: `socket`, `unshare`, `setns`, namespace-bearing `clone`,
  the mount API family, `chroot`, `pivot_root`, `ptrace`,
  `process_vm_readv/writev`, `process_madvise`, `process_mrelease`,
  `pidfd_getfd`, `bpf`, `perf_event_open`, keyring calls, `userfaultfd`,
  every `io_uring` call, module loading, `kexec`, `reboot`, swap control,
  `open_by_handle_at`, `name_to_handle_at`;
- path-based metadata mutation (`chmod`, `fchmodat`, `chown` family,
  `setxattr` family, `utime`, `utimes`, `futimesat`, and `utimensat` with a
  path) is not allowed, because Landlock does not mediate it and there is
  no mount namespace.

The final BPF program is hashed into the sandbox-policy identity. A
development-only derivation mode may exist only in test builds.

## 8. Network policy: none

Private network namespace (loopback down), Landlock TCP restriction,
abstract-socket scope, seccomp denial of `socket`, and no inherited socket
descriptors. No internet, LAN, host loopback, Ollama, D-Bus, Docker, SSH
agent, pathname sockets, UDP, telemetry or registries. Verification never
downloads dependencies.

## 9. cgroup v2 policy (profile-owned constants)

The driver obtains a transient scope from the systemd user manager over its
fixed D-Bus interface (destination, path and interface are constants; the
bus address is derived from the process's real uid, never from the
environment). systemd is used only to obtain and control the scope; none of
its sandbox properties is part of the boundary.

| Limit | `rust.cargo-test.offline.v1` |
|---|---|
| `memory.max` | 4 GiB (tmpfs workspace writes are charged here) |
| `memory.swap.max` | 0 |
| `pids.max` | 256 |
| CPU quota | 400 % (4 CPUs) |
| Wall deadline | 600 s |
| Scope runtime backstop | 660 s |
| Output ceiling | 8 MiB per stream; excerpt 16 KiB per stream |

The scope is created for the backend's retained, unreaped helper child
(its PID is only that child's locator) and is proven before any launch:
the helper is in it, the retained cgroup directory is a populated cgroup v2
directory, it carries exactly the limits above and `memory.oom.group 0`,
the manager's unit for the name is bound to exactly that cgroup (below), and
the unit has the runtime backstop and `OOMPolicy=continue` (an
out-of-memory kill ends only the chosen process; the manager never stops the
scope for it).

Uncertain remote operations (P2-V1-R3B-I4, -R1). StartTransientUnit and
StopUnit are remote operations whose effect and reply are independent. A
timeout, a broken transport, an unexpected error reply or a reply that does
not decode is evidence of uncertainty, never of absence or of success. The
only definite answers relied on are a delivered reply, systemd's
`UnitExists` (StartTransientUnit refused the name before any effect: that
unit is not this request's and is never stopped or claimed) and
`NoSuchUnit` (GetUnit: no unit of the name is loaded), the latter only
after StartTransientUnit's own reply was delivered (below).

What the primary sources establish, and what they do not
(`docs/evidence/p2-v1-r3b-i4-r1-native-scope/baseline/`):

- D-Bus. A method call's reply is identified by the `REPLY_SERIAL` of the
  call it answers, and the protocol is designed for asynchronous operation
  (the D-Bus Specification). Messages between the same two peers keep their
  order, except that a response to a method call may overtake the response
  to an earlier call when the callee answers the later request first
  (NetworkManager's "Notes on D-Bus"). So messages to one recipient are
  delivered in the order they were sent, but the recipient need not process
  or answer concurrently issued calls in that order. Nothing here relies on
  calls being answered in order.
- systemd (`org.freedesktop.systemd1(5)`, systemd 255, the host's manual).
  StartTransientUnit creates and starts a transient unit, whose name must be
  unique, and returns its start job. GetUnit returns the object path of the
  unit loaded under a name, and fails if no unit has been loaded by that
  name. `Id` (unit interface) is the unit's primary name. A scope's
  properties correspond to a service's, and `ControlGroup` is the control
  group path the unit's processes are placed in. systemd unloads a unit only
  when it is not needed: not while it is starting, running or stopping, has
  a pending job or has running processes (`systemd.unit(5)`, "Unit garbage
  collection").
- I4-R1 relies on no systemd-specific fence ordering a later call after an
  earlier, unanswered one. Whether one exists is a G-HOST/G-LIVE
  qualification question.

- Ownership. From before StartTransientUnit is issued, the operation is
  owned as a pending scope, stored in the execution's owner (later its
  retained boundary) before the request can have an effect. It is bound to
  the retained helper (its PID as locator, and a backend-local identity
  that is never reused), the backend-random unit name (a locator, never
  authority), the expected limits, and the manager connection that issued
  the request, which it shares, so it outlives the `ScopeManager`. The
  helper's identity is allocated before the helper is spawned, is nonzero
  and increasing, and never wraps: once identities are exhausted, no helper
  is spawned (P2-V1-R3B-I4-R1). A scope is None, Pending or Proven; Pending
  becomes Proven only once every proof passed, in place, and nothing
  (handshake or launch) reaches the helper while it is Pending.
- An uncertain start (no reply decoded, an unexpected error, or a panic
  before the reply was recorded). The unit is discovered through the
  helper's membership and proven exactly like an accepted one, or settled
  through the candidate cgroup. Without a candidate nothing the manager
  answers confirms it (P2-V1-R3B-I4-R1): not a later `NoSuchUnit` with the
  helper outside, since the request may still be processed after GetUnit
  is answered (above), and not any `StopUnit` reply. It stays Pending, its
  helper killed but unreaped, its execution `CleanupFailed`: possibly for
  the life of the backend process. Nothing of it is ever stopped by the
  unit's name (P2-V1-R3B-I4-R2, name authority below).
- The candidate. The cgroup the kernel reports the helper in, at an
  absolute path in normal form whose last component is the unit's name, is
  retained by descriptor as soon as it is seen, before any later fallible
  check (populated, `cgroup.procs`, limits, GetUnit, `Id`, `ControlGroup`,
  `RuntimeMaxUSec`, `OOMPolicy`), so no failed or uncertain check can lose
  it. It grants cleanup ownership only, never scope authority. A kernel
  that shows the helper in a cgroup of the unit's name the manager does not
  have loaded is a contradiction: nothing is proven.
- The binding (P2-V1-R3B-I4-R1). A unit name is a locator, never a binding
  between the manager and the kernel. Pending becomes Proven only if, at
  the object path GetUnit returned for the name, the unit's `Id` is exactly
  the backend-generated name and the scope's `ControlGroup` is exactly the
  path the kernel reported for the helper, from which the retained
  candidate was opened. Paths are compared byte for byte and never
  canonicalized: a cgroup of the same name in another slice, a path that
  merely ends with the name, an empty, relative or parent-traversing value,
  a value that is not a string, an unavailable or uncertain property, or
  any disagreement proves nothing, and nothing is launched; the candidate
  remains cleanup ownership.
- Settling. The helper is killed but stays unreaped until the operation is
  confirmed gone: its PID stays reserved (a still-queued start job cannot
  attach a reused id) and its membership observable. Everything in the
  candidate is ended with `cgroup.kill`. The operation is confirmed gone
  only when (a) the retained candidate is empty or removed (the helper was
  seen in it, so the start job has run), exactly as for a proven scope; or
  (b) without a candidate, and only after StartTransientUnit's reply (its
  start job) was delivered and recorded: the manager answers on the issuing
  connection that no unit of the name is loaded and the kernel then reports
  the helper outside any cgroup of that name (GetUnit was sent after that
  reply arrived, so after the request was handled, and systemd unloads no
  unit that still has a job or running processes). Otherwise, for an
  accepted operation only, `StopUnit` is attempted: its reply is never a
  confirmation, and a failed or timed-out one never proves that nothing
  was stopped; after every attempt the state is observed again, within a
  bound. An uncertain operation is only observed, within the same bound:
  once the kernel reports the helper in a cgroup of the unit, that cgroup
  is retained, ended through its descriptor and confirmed only once empty
  or removed. A collided operation is confirmed once the helper is outside
  any cgroup of that name.
- Name authority (P2-V1-R3B-I4-R2). `StopUnit` acts on a unit by its name,
  so it is asked only for an operation whose StartTransientUnit success
  reply (its job path) was delivered and recorded (accepted): only that
  reply shows that the manager created the unit for this request. The
  generated name is a locator, never authority, however random: an
  uncertain start may as well have been refused as already loaded
  (`UnitExists`) with that answer lost, the name then a foreign unit's, and
  a collision's unit is foreign; neither is ever stopped by name, and
  neither GetUnit's presence or absence nor the helper's position grants
  or settles anything. A reply that arrived but was not recorded before a
  panic grants nothing. The availability cost is explicit: an uncertain
  operation without a candidate is never confirmed; its helper stays an
  unreaped zombie and its execution `CleanupFailed`, possibly for the life
  of the backend process.
- What cannot be confirmed is `CleanupFailed`. The pending operation stays
  in the `RetainedBoundary` with the unreaped helper, and a retry reconciles
  it again on the issuing connection, without the `ScopeManager` that
  started the execution; there is no detached or background cleanup. A
  scope failure is reported with confirmed cleanup (`SandboxUnavailable`)
  only when absence or cleanup was actually established. Dropping an
  unresolved operation ends what its candidate holds, without waiting and
  without the manager, and never reaps its helper: defense in depth, never
  a confirmation.
- Nothing is reconstructed across a backend restart: no unit is found by a
  name pattern, rebuilt from a saved unit string, or trusted from a PID, a
  cgroup path or a serialized record.
- The API (P2-V1-R3B-I4-R1). A normal build constructs a `ScopeManager`
  only with `ScopeManager::connect` (the bus derived from the real uid and
  verified; `connect_at` exists only for the crate's own tests and live
  harness) and starts a scope only within `execution::run`. A pending
  operation is crate-private: there is no public direct scope start, no
  public settling, and no failure that carries a pending operation without
  its helper. The live harness's direct owner (`execution::place`,
  `ScopedHelper`, `PlacementFailed`; harness builds only) consumes the
  helper and owns it with the operation: a failure, a panic included, ends
  both with the finalizer's steps or moves both into one
  `RetainedBoundary`, and it never unwinds.

After its final report the helper sends that report only once the PID
namespace init is reaped (the kernel has then ended every process of the
namespace) and keeps the scope in existence until the backend releases it,
so the backend reads the scope's counters (`memory.events oom_kill`,
`pids.events max`) for exactly the ended tree. Classification uses them:
without readable counters no run is reported as passed or failed.

Finalization: read the counters, terminate (`cgroup.kill`), reap the
retained helper child, and confirm through the retained directory
descriptor that `cgroup.events` reports `populated 0` — or that the cgroup
has been removed: systemd removes an emptied scope's cgroup, the kernel
removes a cgroup only while it is unpopulated, and a removed cgroup can
never hold a process again. Removal is recognised only through the retained
descriptor (the `cgroup.events` file every live cgroup has is gone and the
directory lists nothing), never by path or unit name. Only then is cleanup
reported clean. Otherwise the execution is `CleanupFailed`, which retains
the boundary and blocks another verifier for the run and blocks Apply until
cleanup is confirmed. No PID-only cleanup and no startup sweep by name.

Panic safety (P2-R1). `execution::run` never unwinds. Everything an
execution creates is stored, as it is created, in an owner held in `run`'s
own frame: the helper (before its output threads start), its scope
operation (pending before its request is issued, proven before any launch
message), the output threads. The spawn, placement, launch and
wait run behind `catch_unwind`; whether they return or panic, the same
finalizer then runs over whatever exists: read the counters, `cgroup.kill`,
kill the helper, a bounded reap, a bounded wait for the scope to be empty or
removed. A helper without a scope never received a launch, so reaping it
ends everything it started. A panic inside the finalizer, or any step it
cannot confirm, moves the scope and the unreaped helper into a
`RetainedBoundary`, which a retry ends with the same steps (a panic while
retrying keeps it retained). A panic while the scope is pending (before or
after its request, while proving it, while stopping or reconciling it)
leaves the pending operation with the execution, and the finalizer settles
it like any other part of the boundary (P2-V1-R3B-I4); a panic before a
StartTransientUnit reply was recorded leaves the operation uncertain
(P2-V1-R3B-I4-R1). A panicked execution is never a pass: before its launch
could reach the helper it is a setup failure
(nothing untrusted ran); after it, `SandboxFailed` (an unknown result); a
panicked output thread loses its record, which is `SandboxFailed` too.
`RetainedBoundary` and the execution's owner end what they hold on drop,
without waiting, only as defense in depth: never a confirmation. The
desktop holds the boundary an execution leaves unconfirmed, and the
workspace, outside its own panicking work; a panic there is settled from
them: the workspace is removed only once no verifier process can remain,
and whatever is unconfirmed is retained (`CleanupFailed`, Apply refused)
for an explicit retry. A retry claims the run before it takes the retained
cleanup, and a run whose verification cleanup is unconfirmed cannot be
discarded.

## 10. Toolchain authority

A neutral `VerifiedVerifierToolchain` (`nexus-verifier-sandbox::toolchain`),
separate from Builder's toolchain object (Builder is not modified). It is
opaque backend authority: private fields, constructed only at the successful
end of a verification, not `Clone`, `Copy`, `Default` or serializable, and
never created from a frontend path. User rustup installations are never
verifier authority.

Packaging (`packaging/verifier-toolchain`): the tree is assembled from three
official Rust 1.94.0 component archives pinned by SHA-256, each pin checked
against the pinned `channel-rust-1.94.0.toml`: `rustc`
(`x86_64-unknown-linux-gnu`, which ships `rust-lld`), `cargo`
(`x86_64-unknown-linux-gnu`) and `rust-std` for `x86_64-unknown-linux-musl`
(self-contained CRT objects, `libc.a`, `libunwind.a`). Exactly 84 files:
`cargo`, `rustc`, the compiler driver and LLVM libraries, `rust-lld`, the musl
standard library and the upstream license files. Test binaries are
static-pie musl executables linked by `rust-lld`; no host C toolchain is
used.

Manifest: rendered at build time (`NEXUS_VERIFIER_TOOLCHAIN=packaged`) from
the assembled tree and embedded in the library: schema, Rust release, host
and verifier targets, and per file its path, size, SHA-256 and executable
mode. It is never read from disk; a build without the assembled toolchain
embeds none, and the toolchain is then unavailable before any filesystem
access.

Production root: derived only from the installed executable, the Debian
package's `/usr/bin/<exe>` giving `/usr/lib/NexusOS/verifier-toolchain`
(Tauri resource `verifier-toolchain`, section 10a). `/usr`, `/usr/lib`,
`/usr/lib/NexusOS`, the root and every directory and file of the tree must
be root-owned and writable by no one else: the installed-package invariant.
Verification is descriptor-relative and never follows a link; missing,
extra, symlinked, special, resized, changed or mode-changed entries,
unexpected directories, another platform or another release are rejected.

Host runtime: the only host files the toolchain loads are
`ld-linux-x86-64.so.2`, `libc.so.6`, `libm.so.6`, `libdl.so.2`, `librt.so.1`,
`libpthread.so.0`, `libgcc_s.so.1` and `libz.so.1` in
`/usr/lib/x86_64-linux-gnu`: each a root-owned regular file writable by no
one else (or a root-owned symlink to one by a plain name in the same
directory) in root-owned directories, and the ELF interpreter path
`/lib64/ld-linux-x86-64.so.2` must resolve to exactly that loader. They are
the sandbox's `RuntimeLoader` (read and execute) and `RuntimeLibrary` (read)
rules.

Binding: the toolchain digest (domain `nexus.verifier.toolchain.v1`) covers
the embedded manifest and the host runtime files' SHA-256; each
verification also gets a backend-owned generation. Both are what a launch
approval and a result bind. The toolchain is re-verified immediately before
each launch (the root still at its path, the tree still exact, the same
runtime files), and `cargo` is launched by descriptor after checking it is
the verified file.

Development: the live suite verifies the assembled development tree against
the same embedded manifest only in builds with the crate's
`development-toolchain` feature; that constructor does not exist in
production builds, and the Phase Two supported-host run fails (it does not
skip) without it.

## 10a. Linux package (P2-R1)

The Linux release (`.github/workflows/release.yml`, `build-linux`) builds
the Debian package with the verifier runtime; Windows and macOS have no
verifier sandbox and their release jobs are unchanged.

- The job assembles the Builder toolchain and the verifier toolchain, and
  builds with `NEXUS_BUILDER_TOOLCHAIN=packaged` and
  `NEXUS_VERIFIER_TOOLCHAIN=packaged`, so the packaged backend embeds the
  manifest of exactly the tree the package installs.
- `packaging/verifier-toolchain/scripts/stage-helper.mjs` builds the helper
  from the checkout (`cargo build --release --locked`, the path cargo
  reports) and stages it, exclusively, as the Tauri sidecar
  `app/src-tauri/binaries/nexus-verifier-sandbox-x86_64-unknown-linux-gnu`,
  printing its digest.
- The bundle merges the Builder configuration and then
  `app/src-tauri/tauri.verifier-runtime.conf.json`: the
  `verifier-toolchain` resource and the `binaries/nexus-verifier-sandbox`
  sidecar, which the Debian package installs at
  `/usr/lib/NexusOS/verifier-toolchain` and exactly
  `/usr/bin/nexus-verifier-sandbox`, the paths production derives. No
  maintainer script.
- Evidence: both bundled toolchains are compared with the assembled trees;
  `packaging/verifier-toolchain/scripts/inspect-deb.mjs` reads the `.deb`
  itself, without installing it: root ownership and modes from the archive's
  own metadata, no maintainer script, `md5sums`, exactly the application and
  the helper in `usr/bin`, the helper byte-identical to the staged build, no
  other copy of it and no ELF file outside the expected places, both
  toolchains exact, and the application embedding the verifier manifest
  (every file's digest); its own negative controls
  (`packaging/verifier-toolchain/test/`) run first and in Fast Local; the
  production feature graph of the backend has neither the
  `development-toolchain` nor the `live-sandbox-harness` feature; and the
  extracted package (`dpkg-deb -x`) satisfies production's layout and tree
  checks with the extracting user as owner
  (`tests/phase2_package_layout.rs`; the root ownership is the inspection's).
- An installed package missing the helper, its protection, the toolchain or
  any file of it, or a backend built without the manifest, fails closed:
  verification is unavailable.

## 11. Workspace

The workspaces directory is derived from the real uid, never from
`$XDG_RUNTIME_DIR` or `$HOME`: `/run/user/<uid>/nexus-verifier`. It is
reached from `/` one component at a time without following symlinks; `/run`
and `/run/user` must be root-owned and writable by no one else,
`/run/user/<uid>` must be a tmpfs directory owned by the uid with mode 0700,
and `nexus-verifier` is opened or created owner-only and must be the same
(owner, mode 0700, same filesystem). It is retained by descriptor.

Per execution a fresh workspace `ws-<128-bit random>/` is created
exclusively (owner-only) with six areas, each created exclusively and
retained by descriptor:

| Area | Verifier role (Landlock) | Environment |
|---|---|---|
| `input/` | `CandidateInput`: read file, read directory | working directory |
| `scratch/` | `Scratch`: workspace write, no execute | — |
| `home/` | `Scratch` | `HOME` |
| `tmp/` | `Scratch` | `TMPDIR` |
| `cargo-home/` | `Scratch` | `CARGO_HOME` (empty) |
| `target/` | `Target`: workspace write and execute | `CARGO_TARGET_DIR` |

The verifier has no rights on the workspace directory itself, so it cannot
rename or replace an area, and Landlock grants no symlink, FIFO, socket or
device creation anywhere; seccomp denies every `chmod` variant. Environment
paths are derived only for the verifier and must resolve, without following
a symlink, to exactly the retained directories before a launch.

Materialization (`CodingRun::materialize_verification_input`) requires a
`StructurallyVerified` run whose candidate is not in the owner's project
(not applied, or rolled back) and an empty `input/`. The staged candidate is
re-read through the retained staging handle (the revoked staging grant is
never reopened) and must still hash to the verified candidate manifest;
otherwise the verification is withdrawn, as for a review. Only the
candidate's regular files are copied, each through retained handles and
checked against its manifest entry; the copy is then scanned like staging
(symlink, special file, hard link, unscoped name or redirect is a violation)
and its manifest hash must equal the exact Phase One candidate manifest hash.
Any other failure is `VerificationInput` and leaves the run unchanged: the
owner can still review and apply it. After execution
`CodingRun::check_verification_input` rescans `input/`; any difference makes
the result `CandidateChanged`.

Removal is identity-bound and explicit, through retained descriptors only,
never following a symlink or entering another filesystem. It removes
whatever the verifier left: names that are not UTF-8, directories created
without permissions (made accessible with `fchmodat2(AT_SYMLINK_NOFOLLOW)`),
hard links and any nesting depth (a subtree at depth 32 is moved up into the
workspace and removed from there, so open descriptors stay bounded). A
directory is removed by name only while the name still refers to the
retained directory, and removal is confirmed through the retained descriptor
(a removed directory has no links). An unconfirmed removal retains the
workspace for a retry and is `CleanupFailed`. Not claims: a same-uid process
racing a rename between the identity check and the removal; workspaces left
by a crashed backend stay (owner-only, on the session tmpfs) until the
session ends: there is no startup sweep by name.

## 12. First profile: `rust.cargo-test.offline.v1`

Applicability (`nexus-verifier-sandbox::applicability`) is decided by the
backend from the candidate's own files, never by a model, with a parser that
accepts exactly this grammar (a TOML subset: bare keys, basic and literal
strings, integers, booleans, arrays of those, `#` comments; anything else is
not applicable):

- `[package]` (required): `name` and `version` (required); only `edition`
  (2015, 2018, 2021, 2024), `rust-version`, `authors`, `description`,
  `license`, `readme`, `repository`, `homepage`, `documentation`,
  `keywords`, `categories` and `publish`;
- `[lib]` (optional): only `name`, `path` (a normalized relative `.rs` path
  of the candidate), `test`, `doctest`, `bench`, `doc`;
- `[dependencies]` (optional): no entry.

Refused, with a bounded reason: a workspace; any dependency (registry, git or
path; normal, dev, build or target-specific); `[patch]`/`[replace]`;
features, profiles, lints, explicit targets, package metadata and every
other table or key (`cargo-features`, `build`, `links`, `resolver`, ...);
`build.rs`; `.cargo/config` or `.cargo/config.toml` anywhere; a proc macro
or other crate type; a missing library source; a missing or stale
`Cargo.lock` (it must be lock format 3 or 4 with exactly the package itself:
no source, checksum or dependency). On a materialized workspace the
candidate is read through the retained `input/` descriptor.

Launch (`nexus-verifier-sandbox::profile_launch::launch_spec`): the profile
must require exactly the verified toolchain (release, host, target, entry
executable); the toolchain is re-verified and every workspace path
re-checked; then `bin/cargo` is launched by descriptor with
`cargo test --offline --locked --no-fail-fast --lib --tests`, working
directory `input/`, and only the profile's environment: `HOME`, `TMPDIR`,
`CARGO_HOME` (empty), `CARGO_TARGET_DIR` from the workspace; `RUSTC` and the
musl target linker (`rust-lld`) from the verified toolchain;
`CARGO_BUILD_TARGET=x86_64-unknown-linux-musl`,
`CARGO_ENCODED_RUSTFLAGS=-Clink-self-contained=yes -Clinker-flavor=ld.lld`,
`CARGO_NET_OFFLINE=true`, `CARGO_TERM_COLOR=never`, `CARGO_INCREMENTAL=0`,
`CARGO_BUILD_JOBS=4`, `RUST_TEST_THREADS=4`, `LC_ALL=C`, `TZ=UTC`; no
`PATH`. The rules are the toolchain's (tree, loader, libraries), the six
workspace areas and `/dev/null` and `/dev/urandom` (checked device
numbers). The passing exit status is 0.

Live evidence on the supported host: a passing crate passes (unit and
integration tests); a failing test and a compile error are `Failed` (101);
a candidate whose own tests attempt TCP, UDP, pathname and abstract Unix
sockets, host files, `~/.ssh`, `/etc/passwd`, `/proc`, writing the input, a
shell and reading a parent secret or `PATH` finds each denied (no host
listener contacted); a candidate with a registry dependency is not
applicable, and run anyway it cannot resolve offline and no network is
reachable.

## 13. Lifecycle and native launch approval

A run's sandboxed verification has its own lifecycle, separate from the
Phase One run state (`VerificationPhase`): Idle → Prepared → Materialized →
Approved → Starting → Running → Finalizing → Idle, or `CleanupFailed`. One
execution at a time; a backend-assigned execution generation that only
increases; reruns only once the previous execution is finalized. Every
transition that matters is recorded in the coding-run ledger first; one
that cannot be recorded does not happen.

1. **Prepared** (`CodingRun::prepare_verification`): the run must be
   `StructurallyVerified`, not applied, with no active or unconfirmed
   execution. The backend computes the inputs from its own verified objects
   (profile hash, toolchain digest and generation, sandbox and resource
   policy hashes) and the launch binding (run, candidate manifest hash,
   structural binding hash, inputs); `verify.prepared` is recorded.
2. **Materialized**: the candidate is materialized into a fresh workspace
   (section 11).
3. **Approved** (`request_verification_approval`): the backend's native
   dialog shows bounded facts only (profile, "on this machine", "no network
   access", time, memory, CPU and process limits, the run and a short
   candidate hash; no paths or secrets); the decision is recorded
   (`verify.approval_granted` / `verify.approval_declined`). Only a recorded
   confirmation yields the `VerifierLaunchApproval`: crate-private
   constructor, no deserializer, not `Clone`, consumed by the launch.
4. **Starting** (`begin_verification`): the backend recomputes the inputs
   immediately before the launch; the approval, the prepared binding and the
   current binding must all be equal (a changed candidate, toolchain
   verification or policy is `Stale` and nothing starts). The generation is
   assigned and `verify.launch` recorded.
5. **Running / Finalizing**: set by the desktop around the sandboxed
   execution, which runs on a thread of its own, off the IPC thread.
6. **Finalized** (`finish_verification`): the result is built from the
   approved binding (never from anything a caller names), recorded
   (`verify.result`: result hash, generation, class, duration, per-stream
   size, digest and truncation, cleanup) and only then becomes the latest
   result. An unconfirmed cleanup leaves `CleanupFailed` until
   `confirm_verification_cleanup` (after a successful retry, recorded as
   `verify.cleanup`).

The desktop (`coding_flow/verification.rs`, Linux x86_64) orders it: the
packaged toolchain and the installed helper must verify (a development
build has neither, so verification is unavailable there), the profile must
apply to the verified candidate, then prepare, materialize, native approval,
begin, and the execution thread: `launch_spec` (toolchain re-verified,
workspace paths re-checked), `ScopeManager::connect`, `execution::run`, input
rescan, workspace removal, finish. Anything failing before the launch ran
nothing and records nothing beyond what was already recorded. A panic in
the execution thread is settled from what the thread owns (section 9): an
unknown result (`SandboxFailed`) with confirmed cleanup, or `CleanupFailed`
with the boundary and workspace retained for a retry. A launch whose thread
never received it ran nothing; its workspace is removed or retained like
any other.

## 14. Result, review and apply

`VerificationResult` is bound under `nexus.verification.result.v1` to the
run, candidate, structural binding, profile, toolchain, sandbox and resource
policies, execution generation, exit class, duration, per-stream size,
digest and truncation, and cleanup status. Exit classes: Passed, Failed,
TimedOut, OutputLimitExceeded, OomKilled, ProcessLimit, Signalled,
SandboxUnavailable, SandboxSetupFailed (nothing untrusted ran),
SandboxFailed (the verifier ran but the sandbox lost it or its counters
before a final report: the result is unknown), ToolchainUnavailable,
CandidateChanged, CleanupFailed. The recorded class is decided in this
order: an unconfirmed cleanup (execution boundary or workspace) is
`CleanupFailed`; else a changed input is `CandidateChanged`; else the
sandbox's class. The result grants nothing.

Verification is **advisory**: a failed verification does not prohibit Apply.
The review binding (`ReviewBinding.verification`) binds either the latest
finalized result hash or an explicit "no verification result" marker, and
the review shows that result, so a rerun invalidates an earlier owner
approval (the apply's current binding no longer matches). Apply is refused
(`VerificationInProgress`) while a verification is starting, running or
finalizing, and (`VerificationCleanupFailed`) while its cleanup is
unconfirmed; a normal failure with confirmed cleanup can still be approved.

IPC: `coding_verification_profiles(run_id)`,
`coding_start_verification(run_id, profile)` and
`coding_retry_verification_cleanup(run_id)`. Only the opaque run id and a
compiled-in profile name cross IPC (the name is looked up exactly and grants
nothing); no command, argument, executable, working directory, environment,
sandbox or network setting, path, PID, cgroup or approval. Views are bounded
display data: the phase, the result class and exit status or signal,
duration, output sizes and truncation, cleanup, a short result hash, and
the output tails (at most 4096 characters per stream, escaped with the
review's `display_safe`, rendered as text).

## 15. Negative-control suite and CI

The automated controls (mission §26, items 1–45) are tests. "Live" tests
run the real helper on a supported host
(`crates/nexus-verifier-sandbox/tests/phase2_live_sandbox.rs`, required with
`NEXUS_PHASE2_REQUIRE_LIVE_SANDBOX=1`, `NEXUS_VERIFIER_TOOLCHAIN=packaged`
and `--features development-toolchain`, where a missing layer or toolchain
fails the suite instead of skipping it). Source-mutation controls, each
restored exactly, showed every layer and check below is load-bearing.

| # | Control | Test |
|---|---|---|
| 1–6 | host sentinel read and write, HOME, `~/.nexus`, synthetic Git/SSH/API credentials, `/etc/passwd` | live `p2c_live_escape_attempts_are_all_denied` (`read_sentinel`, `write_sentinel`, `write_outside`, `list_real_home`, `list_nexus_dir`, `read_git_credentials`, `read_ssh_key`, `read_api_key`, `read_etc_passwd`; sentinel unchanged); live `p2g_live_offline_rust_profile_runs_cargo_test` (a candidate's own tests) |
| 7 | candidate symlink escape | live `symlink_relative`, `symlink_absolute`; kernel `p2e_input_rescan_detects_every_change` (a symlink in the input invalidates it) |
| 8–12 | TCP, UDP, abstract and pathname Unix sockets, Ollama loopback | live `tcp_host`, `udp_socket`, `abstract_socket`, `pathname_socket`, `tcp_ollama`, `socketpair_inet`; P2G candidate tests; host listeners count zero contacts |
| 13, 14 | unexpected inherited descriptor, parent secret in the environment | live `no_inherited_fds` (a leaked non-close-on-exec descriptor), environment keys exactly the launch's; P2G `sealed_environment` (no parent secret, no `PATH`) |
| 15–19 | nested user namespace, `setns`/`unshare`, mount/`chroot`/`pivot_root`, `io_uring`, ptrace/`process_vm`/pidfd | live seccomp checks (`ENOSYS` from the filter itself) and ablation `p2c_live_each_layer_is_necessary` |
| 20–22 | `setsid`/`setpgid` descendant, grandchild after init exit, `cgroup.kill` reach | live `p2c_live_descendants_do_not_survive_init_exit`, `p2c_live_parent_death_ends_the_sandbox`, `p2d_live_deadline_kill_reaches_every_descendant` |
| 23–26 | pids, memory, wall timeout, output flood | live `p2d_live_pids_limit_is_enforced`, `p2d_live_memory_limit_is_enforced`, `p2d_live_descendant_oom_is_never_a_pass`, `p2d_live_deadline_kill_reaches_every_descendant`, `p2d_live_output_flood_is_bounded` |
| 27 | no sockets after the filter | live `socketpair_inet`, `socket_raw` and every socket check (`socket` is `ENOSYS`) |
| 28–30 | input not writable, input hash unchanged, a modified input invalidates the result | live `write_input_new`, `modify_input`, `p2e_live_workspace_confines_the_verifier`, P2G input unchanged after every run; kernel `p2e_input_rescan_detects_every_change`; desktop `p2h_finalization_never_reports_more_than_it_proved` |
| 31, 32 | toolchain tamper, missing packaged toolchain | `p2f_the_exact_tree_verifies_and_every_deviation_is_rejected`, `p2f_verification_binds_tree_and_runtime_and_reverify_sees_changes`, `p2f_the_launch_material_is_the_verified_files`, `p2f_production_is_unavailable_without_an_installed_package`, live `p2f_live_packaged_toolchain_verifies` |
| 33 | unsupported Landlock ABI | `p2i_nc_33_a_landlock_below_abi_6_is_never_accepted`; live `p2i_live_unavailable_landlock_fails_closed` |
| 34, 35 | missing namespace, missing cgroup delegation | live `p2c_live_missing_namespace_fails_closed`, `p2d_live_missing_scope_fails_closed`, `p2d_live_unmovable_process_fails_closed` |
| 36 | seccomp installation failure launches nothing | live `p2i_live_failed_seccomp_install_runs_nothing` |
| 37–42 | forged approval, stale approval, a result of another candidate, rerun invalidates the review, `CleanupFailed` blocks Apply, a failed but cleaned verification is advisory | kernel compile-fail doctests; `p2h_nc_37`–`p2h_nc_42`; `p2b_a_result_binds_only_its_own_launch` |
| 43 | no caller or model input of command, argv, executable, cwd, environment or network policy | `p1_g_01_coding_commands_take_only_opaque_ids_and_choices`, `p2_g_01`–`p2_g_05`, `p2b_the_profile_has_no_shell_path_or_network_escape`, frontend `p1_g_governed_coding_sends_no_path_grant_or_approval` and the page tests |
| 44 | `/proc` | live `read_proc_status`, `list_proc`; P2G `no_host_files` |
| 45 | no raw shell | `p2b_the_profile_has_no_shell_path_or_network_escape`; live `exec_shell`; P2G `no_shell` |

P2-R1 controls (the panic invariant, helper construction and the Linux
package):

| Control | Test |
|---|---|
| a panic after the helper spawned, while proving the scope, after the scope was proven, after the launch, while running (2 s, a detached marker tree), while draining output, in an output thread, immediately before finalization, and in finalization (panic or failed step) | live `p2r1_live_panic_after_helper_spawn_is_finalized`, `p2r1_live_panic_while_proving_the_scope_stops_it`, `p2r1_live_panic_after_the_scope_is_proven_is_finalized`, `p2r1_live_panic_after_launch_ends_the_tree`, `p2r1_live_panic_while_running_ends_every_descendant`, `p2r1_live_panic_while_draining_output_ends_the_tree`, `p2r1_live_output_thread_panic_is_never_a_result`, `p2r1_live_panic_before_finalization_is_never_a_pass`, `p2r1_live_panic_in_finalization_retains_the_live_boundary`, `p2r1_live_failed_finalization_is_retried_to_confirmation`: never a pass; confirmed cleanup only with the helper reaped, no process of the verifier's tree alive and the scope gone; otherwise a retained boundary that still owns the live tree, which a retry ends |
| the finalizer and a retained boundary without a scope | `p2r1_finalization_ends_and_reaps_a_helper_without_a_scope`, `p2r1_a_failed_or_panicking_finalization_retains_the_live_boundary`, `p2r1_a_dropped_boundary_still_ends_its_helper`, `p2r1_a_panicking_output_thread_loses_its_record`, `p2r1_an_interrupted_execution_is_never_passed` |
| the desktop's panic path, retry and discard | `p2r1_a_panic_after_the_execution_retains_its_unconfirmed_boundary`, `p2r1_a_panic_with_nothing_unconfirmed_is_an_unknown_result`, `p2_g_07_a_retained_verification_cleanup_is_never_dropped`; kernel `p2r1_a_panicked_verification_keeps_apply_refused_until_its_cleanup_is_confirmed` |
| no arbitrary helper path in production | `p2_g_06_production_cannot_construct_a_helper_from_an_arbitrary_path`; a normal build of the desktop cannot name `HelperProgram::at` (it does not exist without the harness feature) |
| the Linux package | `p2_g_08_the_linux_package_installs_the_verifier_runtime`; `packaging/verifier-toolchain/test/inspect-deb.test.mjs`; `tests/phase2_package_layout.rs`; the release job's inspection |

P2-V1-R3B-I4 controls (uncertain scope operations). They run on every
host, over a deterministic simulation of the user manager and the kernel
that exists only in test builds (`scope/tests.rs`); nothing in them
contacts a bus or creates a cgroup. Behavioural source-mutation controls
(`NC-I4-*`), each restored exactly, showed each mechanism is load-bearing
(`docs/evidence/p2-v1-r3b-i4-native-scope/`).

| Control | Test |
|---|---|
| a confirmed start; a start without effect, its reply lost (never proven absent: P2-V1-R3B-I4-R1); with effect, its reply lost (discovered and proven); a lost connection | `i4_01`–`i4_03` (`scope::tests`) |
| a property or a manager query uncertain after the candidate is retained; mismatched limits, runtime backstop, out-of-memory policy, membership; a name never proves | `i4_04`, `i4_10`, `i4_12`–`i4_16` |
| a unit the helper never entered; `StopUnit` failed, delivered with the scope still populated, timed out with the target gone or remaining | `i4_05`–`i4_09` |
| the manager's absence with the helper in a cgroup of the unit's name; a name collision | `i4_11`, `i4_17` |
| no launch message before the proof | `i4_18` (a stand-in helper that shows any launch reaching it) |
| finalization of a proven and of a pending scope; `CleanupFailed`; retry, also without the `ScopeManager`; drop | `i4_19`–`i4_24` |
| a panic after the start request, after the candidate is retained, while proving, while stopping or reconciling; ordering; classification | `i4_25`–`i4_30` |

P2-V1-R3B-I4-R1 controls (scope ownership closure). Over the same
simulation; nothing in them contacts a bus or creates a cgroup.
Behavioural source-mutation controls (`NC-I4R1-*`) and the normal-build API
guards (an external crate compiled against the crate without features) are
in `docs/evidence/p2-v1-r3b-i4-r1-native-scope/`.

| Control | Test |
|---|---|
| a panic after an uncertain start without a candidate; a panic at every ownership point, through an execution and the harness owner, and while retrying | `i4r1_01`, `i4r1_x_a_panic_at_any_ownership_point_leaves_confirmation_or_one_owner` |
| the harness owner: one owner of the operation and its helper; drop is defense only; the scope-hold sequence | `i4r1_02`, `i4r1_03`, `i4r1_23` |
| a normal build: a manager only by `connect`; no public pending operation, direct start, settling or split failure | `i4r1_04`, `i4r1_05`; the normal-build API guards; desktop `p2_g_01`, `p2_g_06` |
| the binding: a cgroup of the unit's name elsewhere; a control group not exactly the kernel's; one unavailable or uncertain; a unit id not exactly the name; the exact binding; no launch before it | `i4r1_06`–`i4r1_10`, `i4r1_17` |
| an uncertain start without a candidate: not released by `NoSuchUnit` or a timed-out stop, and (P2-V1-R3B-I4-R2) never stopped by name; acquired once the helper is placed | `i4r1_11`–`i4r1_14` |
| collisions; the proven lifecycle; a retry without the `ScopeManager` | `i4r1_15`, `i4r1_16`, `i4r1_18`, `i4r1_19` |
| the helper's identity: refused instead of wrapping; no spawn once exhausted | `i4r1_20`, `i4r1_21` |
| no real manager or kernel in unit tests | `i4r1_24`; the strace proof in the evidence |

P2-V1-R3B-I4-R2 name authority. A collision whose `UnitExists` reply was
lost made settling stop the foreign unit by its name (reproduced on the
model against the I4-R1 code, and by the tests below against it); settling
now asks a stop by name only for an accepted operation. Tests over the
deterministic model, controls and evidence in
`docs/evidence/p2-v1-r3b-i4-r2-uncertain-stop-authority/`.

| Control | Test |
|---|---|
| a lost collision reply: the foreign unit never stopped, killed or claimed; the operation owned, `CleanupFailed`, its helper unreaped, through retries and through the harness owner | `i4r2_01` |
| an uncertain start without effect or candidate: never stopped by name, never resolved by the manager's absence, its helper unreaped after bounded retries | `i4r2_02` |
| an uncertain start whose helper is placed later: its cgroup retained by descriptor through the helper's membership, ended and observed empty, the helper reaped only then; no stop by name | `i4r2_03` |
| an uncertain start's candidate whose proof fails: ended by descriptor, never stopped by name, nothing launched, retained while populated | `i4r2_04` |
| a recorded start reply alone authorizes a stop by name, whose reply confirms nothing | `i4r2_05` |
| a delivered `UnitExists`: never stopped or claimed (unchanged) | `i4r2_06` |
| a panic before the start reply is recorded: no stop by name, no owner escape | `i4r2_07` |
| a retained uncertain operation retried without its `ScopeManager`: no stop by name | `i4r2_08` |
| the only stop by name is behind the recorded start reply, which is set in that reply's arm only | `i4r2_x_only_a_recorded_start_reply_authorizes_a_stop_by_name` |

P2-V1-R3B-I4-Q1 host qualification (live). The live suite qualifies, on
the supported host and the same live helper the production path uses, the
facts the accepted scope mechanism depends on (G-HOST H1 to H7), beside the
production proof, which stays the only authority; the real values are
printed as bounded evidence. Its probe (`tests/support/host_qualification.rs`)
is the live harness's own: a bounded connection to the checked user bus, in
no library and no normal build (`p2_g_10`). It creates nothing and holds
nothing by a unit name: besides its reads (`GetUnit`, `Properties.Get`), its
one request is a start that carries no process and no property, sent only
for the unit of a scope the production owner has proven and still holds; it
has no request that stops, kills or changes a unit (P2-V1-R3B-I4-Q1-R1,
`p2_g_11`).

| Fact | Live case |
|---|---|
| H1: the manager is the real uid's `/run/user/<uid>/bus`, the socket the cleanup observation checks; `ScopeManager::connect` derives it from the real uid alone and reads no environment (pinned at the source, `i4q1r1_connect_derives_the_bus_from_the_real_uid_alone`; no case changes the environment of the multi-threaded live harness) | `p2q_live_h1_the_manager_is_the_real_uid_s_user_bus` |
| H2: the live helper's `/proc/<pid>/cgroup` is one unified `0::` line, absolute and in normal form, on a cgroup2 hierarchy (hybrid or v1 fails closed) | `p2q_live_h2_the_helper_s_membership_is_one_unified_cgroup_v2_line` |
| H3, H4: `GetUnit`'s object reports `Id` equal to the generated name and `ControlGroup` byte-equal to the kernel's membership; `RuntimeMaxUSec` (`t`) and `OOMPolicy` (`s`) exact | `p2q_live_h3_h4_the_manager_s_unit_binds_to_the_kernel_s_cgroup`, `p2r1_live_panic_during_the_binding_proof_settles_it` |
| H5: a fresh name is refused with exactly `org.freedesktop.systemd1.NoSuchUnit` | `p2q_live_h5_a_fresh_name_is_exactly_no_such_unit` |
| H6: a start for the name of a loaded transient scope is refused with exactly `org.freedesktop.systemd1.UnitExists`; the scope is the production owner's (`execution::place`, proven and held with its helper), the request carries no process and no property, and the owner releases the scope only after the answer, the helper seen where the proof put it (P2-V1-R3B-I4-Q1-R1) | `p2q_live_h6_an_existing_name_is_exactly_unit_exists` |
| H7: a helper killed in its proven scope and kept unreaped keeps a readable `/proc/<pid>/cgroup` naming that cgroup (marked ` (deleted)` once removed); the finalizer settles a retained candidate on the real host | `p2q_live_h7_a_killed_unreaped_helper_keeps_its_membership`, `p2r1_live_panic_after_the_candidate_is_retained_settles_it`, `p2r1_live_panic_while_proving_the_scope_stops_it` |

The error identities the cases assert are the production manager's own
constants (`i4q1_the_manager_s_definite_answers_are_exactly_systemd_s_error_names`,
`p2_g_10`); the gate's passed-case count (39) is exactly the suite's cases
(`p2_g_10`). The qualification establishes facts of this host and systemd
version; it adds no generic D-Bus or systemd ordering guarantee, and the
no-candidate rule for an uncertain start is unchanged.

P2-V1-R3B-I4-Q1-R1 (collision qualification ownership). The Q1 collision
case owned its first start itself: an owner built before the request ended
its helper (killed and reaped) and stopped the unit by its name whatever
the request's outcome, so a first start refused as already loaded
(`UnitExists`, a foreign unit) was stopped by name, and an uncertain or
panicking first start had its helper reaped while the request could still
attach it. The case now makes no start of its own: its first start is the
production owner's (`execution::place`), with the accepted I4-R1 semantics
for every outcome (proven; a collision never stopped, killed or claimed,
its helper ended only once outside; an uncertain start or a panic settled,
or retained with its helper unreaped). Its one request names only the unit
of the proven scope and carries no process and no property: systemd refuses
a loaded transient unit's name before it reads any property, and a request
handled after that unit is gone would create a scope without processes,
which systemd refuses when it loads it (systemd v255 `dbus-manager.c`,
`unit.c`, `scope.c`, `transaction.c`); whatever it is answered and whenever
it is handled, it cannot place a process or change the loaded unit. The
targeted tests `i4q1r1_nc1` to `i4q1r1_nc4` (the first start's outcomes
through `execution::place`) and `p2_g_11` (the harness) carry it; the
evidence is in `docs/evidence/p2-v1-r3b-i4-q1-r1-host-qualification-ownership/`.

CI: a dedicated exact-SHA workflow on the self-hosted runner
(`.github/workflows/ci-phase2-linux-sandbox.yml`) runs the live suite and
fails if a layer is missing. Hosted CI runs the portable tests and asserts
fail-closed unavailability where the host lacks the layers; no required test
is ignored.

Known runner prerequisite: the runner user (`github-runner`) currently has
no systemd user manager (not lingering) and so no `/run/user/<uid>` or user
bus: the cgroup and workspace layers are unavailable there until an
administrator provides them; the dedicated workflow fails until then, by
design.

## 16. Feasibility evidence (P2A-001 and this mission's spike)

Measured on the audited host: unprivileged `unshare` of user, PID, net,
IPC, UTS and cgroup namespaces in one call succeeds (mount does not);
Landlock ABI 7; seccomp filter attach under `no_new_privs`; delegated
cgroup v2 limits through the user manager; the exact first-profile invocation
compiles, links (`rust-lld`, self-contained musl) and passes with an empty
environment under strict Landlock (no `/proc`, `/sys`, `/etc`, `/usr` beyond
the eight runtime files) and with path-based metadata mutation denied.

## 17. Phase Two authority inventory

| Authority | Where | Governed by |
|---|---|---|
| Spawn a process | `launcher::Helper::spawn` (the only `Command::new` in the sandbox crate) | only `execution::run`, only from the desktop's verification module after the owner's recorded native approval; the program is `HelperProgram::installed()` (root-owned, `/usr/bin`, beside the installed application; the arbitrary-path `HelperProgram::at` and the other live-harness seams exist only for the sandbox crate's own tests, through its `live-sandbox-harness` feature, which only its own dev-dependency enables); cleared environment, no arguments, `/` as working directory, no `pre_exec`; each helper's backend-local identity is allocated before it is spawned and never wraps, and none is spawned once they are exhausted (P2-V1-R3B-I4-R1) |
| Execute project code | the helper's verifier child (`execveat` of the verified `cargo` by descriptor) | every mandatory layer established and re-checked first; any failure reports a setup stage and executes nothing |
| Namespaces | the helper (`unshare` once) | uid/gid identity maps written only for the backend's own unreaped child; identities verified by `readlink` of `/proc/self/ns/*` |
| cgroup scope | `scope::ScopeManager` over the user manager's fixed D-Bus interface | constructed in a normal build only by `ScopeManager::connect`: bus from the real uid, owner-checked; a scope started only within `execution::run` (P2-V1-R3B-I4-R1); backend-random unit names; limits verified from the cgroup files; the manager's unit bound to the kernel's cgroup by its exact `Id` and `ControlGroup` (P2-V1-R3B-I4-R1); the operation owned as a pending scope, beside its helper, from before its request until proven or confirmed gone (P2-V1-R3B-I4); `StopUnit` only for an operation whose StartTransientUnit success reply was delivered and recorded, never for an uncertain or collided one, never merely because the backend generated the unit name, and never taken as confirmation (P2-V1-R3B-I4-R2); kill, counters and emptiness through the retained descriptor |
| Workspace | `workspace::WorkspaceRoot`/`Workspace` under `/run/user/<uid>/nexus-verifier` | uid-derived, walked without symlinks, owner-only, exclusive creation, identity-bound removal |
| Toolchain | `toolchain::VerifiedVerifierToolchain::installed()` | embedded manifest, root-owned installed tree, host runtime checks, re-verified before each launch |
| Candidate copy | `CodingRun::materialize_verification_input` | the retained staging handle, the verified manifest, an empty backend-created directory |
| Launch approval | `CodingRun::request_verification_approval` with the desktop's native dialog | bounded facts; recorded; single-use approval bound to the exact binding |
| IPC | `coding_verification_profiles`, `coding_start_verification`, `coding_retry_verification_cleanup` | run id and profile name only; capabilities grant them to the main window at the local origin |
| Ledger | `verify.prepared`, `verify.approval_granted`, `verify.approval_declined`, `verify.launch`, `verify.result`, `verify.cleanup` | hashes, sizes, classes and generations only; fail closed |
| Release package | the Linux `.deb`: `/usr/bin/nexus-verifier-sandbox` and `/usr/lib/NexusOS/verifier-toolchain` | built from the release checkout (section 10a); no maintainer script; inspected as it ships |

Unchanged: Builder's trusted toolchain and every Phase Zero and Phase One
surface; the Phase Zero and Phase One guards still pass, with reviewed pin
updates for the new commands, the new Tauri config merge and the new ignored
build-output directories.

## 18. Residuals

- The installed-package path (installed helper, installed toolchain) cannot
  run in a development build by design; the desktop glue is covered by unit
  tests of its decisions and by source pins, the layers below it by the live
  suite, and the package by its inspection and the extracted-layout tests
  (an installed package was not exercised as root: installing needs root).
- A workspace left by a crashed backend stays (owner-only, on the session
  tmpfs) until the session ends; there is no startup sweep by name.
- Retained boundaries live in the backend's memory: if the backend process
  itself ends, the parent-death chain and the scope's runtime backstop end
  what remained, and nothing is left to confirm.
- A panic inside the kernel's own result recording, after the desktop has
  stored the retained boundary, leaves the run's phase unfinished (Apply
  refused); the boundary itself stays owned.
- Timing: a verification's generation binds the toolchain verification it
  was prepared with; a re-verification from scratch is a new binding.
- Uncertain scope operations (P2-V1-R3B-I4, -R1) are validated against a
  deterministic simulation; the facts they depend on (that systemd names
  `UnitExists` and `NoSuchUnit` as relied on, reports a scope's `Id` and
  `ControlGroup` exactly as `/proc/<pid>/cgroup` names the same cgroup for
  a backend in the manager's cgroup namespace, and that `/proc/<pid>/cgroup`
  of an unreaped, killed helper still names its cgroup, with ` (deleted)`
  once it is removed) are qualified on the supported host only by the
  exact-SHA live gate's host-qualification cases (section 15,
  P2-V1-R3B-I4-Q1); a backend that sees other paths proves nothing (fail
  closed). Until that gate has passed on the runner, G-HOST and G-LIVE
  remain open.
- An uncertain StartTransientUnit whose helper is never seen in a cgroup of
  its unit is never confirmed (P2-V1-R3B-I4-R1): it stays `CleanupFailed`,
  its helper an unreaped zombie, for the life of the backend process. No
  systemd-specific fence is relied on to release it; whether one exists is
  for G-HOST/G-LIVE to qualify.
- A collision whose `UnitExists` reply is lost (a timeout, a broken
  connection) is, to the backend, an uncertain start without a candidate.
  Since P2-V1-R3B-I4-R2 nothing of it is ever stopped by the unit's name, so
  the foreign unit is never touched; the operation stays `CleanupFailed`,
  its helper an unreaped zombie, for the life of the backend process. That
  availability cost applies to every uncertain start without a candidate
  and is accepted: no unit this backend did not create is ever stopped.
- An accepted operation without a candidate may still be stopped by name.
  Its recorded reply shows that the manager created the unit for this
  request; it does not bind a unit loaded under the name later. A party of
  the same uid that learned the name could load another unit under it
  after this backend's was unloaded, and a later settling stop could reach
  that unit (its reply still confirms nothing). Binding the stop to the
  unit's own identity is not attempted.
- A pending operation whose manager connection broke cannot be confirmed
  through that connection: unless its retained candidate empties, it stays
  `CleanupFailed` for the life of the backend process. No other connection
  is trusted to answer for it.
- The helper of an unresolved operation whose retained boundary is dropped
  (defense in depth only) stays an unreaped zombie until the backend process
  exits: its PID stays reserved.
- On a host whose `/proc/<pid>/cgroup` is not the single unified line
  (cgroup v1 or hybrid, unsupported), an operation that cannot be proven
  absent is `CleanupFailed`, never `SandboxUnavailable`.
