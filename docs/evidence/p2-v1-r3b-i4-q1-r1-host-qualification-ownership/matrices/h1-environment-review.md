# P2-V1-R3B-I4-Q1-R1: review of H1's process-wide environment change

## What Q1's H1 did

To show that `ScopeManager::connect()` ignores an ambient session bus, H1
(3d46b4d, `tests/phase2_live_sandbox.rs` 1574-1586) set
`DBUS_SESSION_BUS_ADDRESS` to `unix:path=/nonexistent/nexus-q1/bus`
process-wide with `std::env::set_var`, called `connect()`, removed the
variable with `std::env::remove_var`, and required `connect()` to succeed.

## Conclusion: not appropriate in the live harness; replaced

1. **It is a documented data race.** Rust 1.94.0's own contract
   (`primary/excerpts.md`, `library/std/src/env.rs` 303-325): in a
   multi-threaded program on Linux "the only safe option is to not use
   `set_var` or `remove_var` at all", because other threads may read the
   environment without going through `std::env` (libc itself, and functions
   that do not advertise it); from the 2024 edition both functions are
   `unsafe`. The live harness is multi-threaded when H1 runs: before the
   scoped cases, the host check and the earlier cases start `Listeners`
   (four accept/receive threads each, never joined and outliving it:
   `tests/phase2_live_sandbox.rs` `Listeners::start`), `missing_scope`
   leaves a listener thread blocked in `accept`, and output drains run on
   their own threads. No one can establish that none of them, or a library
   under them, reads the environment at that moment.
2. **It changes state it does not own.** `remove_var` deletes the variable
   rather than restoring it: an ambient `DBUS_SESSION_BUS_ADDRESS` (present,
   for example, when the suite runs in a desktop session) is gone for every
   later case and every process the harness starts afterwards.
3. **It proves less than a source pin.** Success with one bogus variable
   shows only that `connect()` did not use that variable; it says nothing of
   `XDG_RUNTIME_DIR`, `DBUS_SYSTEM_BUS_ADDRESS` or zbus's session and system
   builders.

The hazard is avoidable: the fact H1 needs is a property of `connect()`'s
code, and the live part of H1 never needed the mutation.

## What replaces it

- **A source pin**, run with the sandbox crate's unit tests
  (`scope::tests::i4q1r1_connect_derives_the_bus_from_the_real_uid_alone`):
  `connect()`'s body is exactly `let uid = unsafe { libc::getuid() };
  Self::connect_to(&format!("/run/user/{uid}/bus"))`; `connect_to` is
  reached only from `connect()` and the harness-only `connect_at`, and builds
  the one `ZbusManager::connect_at(path)`; that connects with
  `zbus::connection::Builder::address(format!("unix:path={path}"))`, the only
  zbus connection builder in the module (zbus 4.4.0's `address` builder
  reads no environment; its `session`/`system` builders do); and no source
  of the scope module names `std::env`, `env::var`, `var_os`, `getenv`,
  `set_var`, a session or system builder or address, `DBUS_` or
  `XDG_RUNTIME_DIR`. The accepted `p2_g_02` already forbids `std::env::var`,
  `env::var_os` and `XDG_RUNTIME_DIR` across the crate's production sources.
  Control `Q1-R1-H1-CONNECT-ENV` makes `connect()` follow
  `DBUS_SESSION_BUS_ADDRESS`; the pin fails.
- **The live H1, without the mutation**: the real uid's runtime directory
  is a private tmpfs of the uid, its `bus` is a socket of the uid, it is the
  very socket the checked cleanup observation reaches (same device and
  inode), `ScopeManager::connect()` succeeds, and `connect_at` refuses any
  other path. H3 then shows, live, that the unit the connected production
  manager created is the one the checked socket's manager reports, with the
  exact `Id` and `ControlGroup`.
- **A guard** (`p2_g_11`): the qualification cases (`p2q`) never call
  `set_var`, `remove_var` or anything of `std::env`. Control
  `Q1-R1-H1-ENV-MUTATION` puts Q1's `set_var` back; the guard fails.

Together these establish what Q1's H1 meant to: a normal build's manager is
the real uid's user bus, derived from the real uid alone, with no ambient
session-bus authority, now without changing the process environment.

## Observation (not changed: outside this mission)

Two accepted cases also change the environment of the live harness, for a
different invariant (the sandboxed verifier never inherits the backend's
environment): `p2c` `escapes_denied` sets and removes
`NEXUS_P2C_PARENT_SECRET`, and `p2g` `cargo_test` sets
`NEXUS_P2G_PARENT_SECRET`. They carry the same documented hazard. Changing
how those cases plant a parent secret is outside the H1 review this mission
asks for; recorded for the Architect.
