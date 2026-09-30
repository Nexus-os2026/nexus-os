# Nexus verifier toolchain (Phase Two, P2F)

The Nexus-owned Rust toolchain the desktop app packages for governed
verification (`rust.cargo-test.offline.v1`). It is verifier authority only
as the installed package; a user's rustup installation never is.

## Contents

| File | Role |
|---|---|
| `rust-release.json` | Rust 1.94.0 (2026-03-05) pins: the channel manifest's SHA-256, and for each official component archive (`rustc` and `cargo` for `x86_64-unknown-linux-gnu`, `rust-std` for `x86_64-unknown-linux-musl`) its SHA-256 and the exact members packaged. |
| `scripts/assemble.mjs` | Release/CI assembly (never packaged, never run by the app). |

## Packaged tree (84 files)

`bin/cargo`, `bin/rustc`, `lib/librustc_driver-*.so`,
`lib/libLLVM.so.21.1-rust-1.94.0-stable`,
`lib/rustlib/x86_64-unknown-linux-gnu/bin/rust-lld`, the complete
`lib/rustlib/x86_64-unknown-linux-musl/lib/` (standard library and its
self-contained CRT objects, `libc.a` and `libunwind.a`), and the upstream
license files under `share/doc/{rustc,cargo,rust-std}/`. Test binaries are
static-pie musl executables linked by `rust-lld`; no host C toolchain is
used. Executables are mode 0755, everything else 0644.

```
node packaging/verifier-toolchain/scripts/assemble.mjs --out app/src-tauri/verifier-toolchain [--archives <dir>]
node packaging/verifier-toolchain/scripts/assemble.mjs --compare <expected> <actual>
```

Every archive, fetched from the pinned source or supplied with
`--archives`, must match its pin, and each component pin must be the one the
pinned channel manifest lists. Only the pinned members are written; any
missing, repeated or non-regular member fails the assembly and leaves no
output.

## Embedding and installation

The verifier sandbox build script (`NEXUS_VERIFIER_TOOLCHAIN=packaged`)
hashes the assembled `app/src-tauri/verifier-toolchain/`, rejects anything
the runtime verifier would reject, and embeds the exact manifest (path,
size, SHA-256, executable mode). Tauri bundles the same directory as the
`verifier-toolchain` resource (`app/src-tauri/tauri.verifier-toolchain.conf.json`),
installed by the Debian package at `/usr/lib/NexusOS/verifier-toolchain`.

At runtime the root is derived only from the installed executable
(`/usr/bin/<exe>`); the tree, its directories and `/usr`, `/usr/lib` and
`/usr/lib/NexusOS` must be root-owned and writable by no one else, and the
tree must match the embedded manifest exactly. The host runtime it loads
(the loader and seven glibc/libgcc/zlib libraries in
`/usr/lib/x86_64-linux-gnu`) must be root-owned regular files. It is
re-verified immediately before every launch and `cargo` is launched by
descriptor. Builds without an assembled toolchain embed no manifest and stay
unavailable.

The live sandbox suite verifies a development tree against the same
embedded manifest only in builds with the `development-toolchain` feature;
that path is never production authority.
