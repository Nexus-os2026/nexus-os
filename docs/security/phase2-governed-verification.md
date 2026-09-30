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
  membership from the kernel's view of the retained, unreaped child before
  sending the launch message.
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
directory, and it carries exactly the limits above, `memory.oom.group 0`,
the runtime backstop and `OOMPolicy=continue` (an out-of-memory kill ends
only the chosen process; the manager never stops the scope for it).

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
(Tauri resource `verifier-toolchain`). `/usr`, `/usr/lib`,
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

Applicable only to a single dependency-free Rust library package, parsed by
the backend: no workspace, no registry/git/path/dev/build/target-specific
dependencies, no `.cargo/config` or `.cargo/config.toml`, no `build.rs`, no
custom runner, no proc-macro, a lock file that exists and is current, and a
library target. The model never decides applicability. Fixed backend
invocation: `test --offline --locked --no-fail-fast --lib --tests`, target
`x86_64-unknown-linux-musl`, private empty `CARGO_HOME`, private
`CARGO_TARGET_DIR`, fixed `RUSTC`, linker `rust-lld` with self-contained
linking, `LC_ALL=C`, `TZ=UTC`, no `PATH`.

## 13. Native launch approval

Before any project code executes the backend invokes a native confirmation
and obtains a single-use, unforgeable approval bound to: run id, candidate
manifest hash, structural binding hash, verifier profile hash, toolchain
digest and generation, sandbox policy hash and resource policy hash. The
prompt shows bounded facts (profile, "local", "no network", time and
resource limits); no paths or secrets. If any bound input differs at launch,
the launch is refused. Frontend or model flags grant nothing.

## 14. Result, review and apply

`VerificationResult` is bound under `nexus.verification.result.v1` to the
run, candidate, structural binding, profile, toolchain, sandbox and resource
policies, execution generation, exit class, duration, per-stream size,
digest and truncation, and cleanup status. Exit classes: Passed, Failed,
TimedOut, OutputLimitExceeded, OomKilled, ProcessLimit, Signalled,
SandboxUnavailable, SandboxSetupFailed (nothing untrusted ran),
SandboxFailed (the verifier ran but the sandbox lost it or its counters
before a final report: the result is unknown), ToolchainUnavailable,
CandidateChanged, CleanupFailed. The result grants nothing.

Verification is **advisory**: a failed verification does not prohibit Apply.
The review binding binds either the latest finalized result hash or an
explicit "no verification result" marker, so rerunning or changing
verification invalidates an earlier approval. Apply is refused while a
verification is starting, running or finalizing, or its cleanup failed.

## 15. Negative-control suite and CI

The automated controls (mission §26, items 1–45) run live on a supported
host through a dedicated exact-SHA workflow on the self-hosted runner
(`.github/workflows/ci-phase2-linux-sandbox.yml`), which fails if a layer is
missing. Hosted CI runs the portable tests and asserts fail-closed
unavailability where the host lacks the layers; no required test is ignored.

Known runner prerequisite: the runner user (`github-runner`) currently has
no systemd user manager (not lingering), so the cgroup layer is unavailable
there until an administrator provides one; the dedicated workflow will fail
until then, by design.

## 16. Feasibility evidence (P2A-001 and this mission's spike)

Measured on the audited host: unprivileged `unshare` of user, PID, net,
IPC, UTS and cgroup namespaces in one call succeeds (mount does not);
Landlock ABI 7; seccomp filter attach under `no_new_privs`; delegated
cgroup v2 limits through the user manager; the exact first-profile invocation
compiles, links (`rust-lld`, self-contained musl) and passes with an empty
environment under strict Landlock (no `/proc`, `/sys`, `/etc`, `/usr` beyond
the eight runtime files) and with path-based metadata mutation denied.
