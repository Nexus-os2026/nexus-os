# Nexus verifier toolchain (Phase Two, P2F)

The Nexus-owned Rust toolchain the desktop app packages for governed
verification (`rust.cargo-test.offline.v1`). It is verifier authority only
as the installed package; a user's rustup installation never is.

## Contents

| File | Role |
|---|---|
| `rust-release.json` | Rust 1.94.0 (2026-03-05) pins: the channel manifest's SHA-256, and for each official component archive (`rustc` and `cargo` for `x86_64-unknown-linux-gnu`, `rust-std` for `x86_64-unknown-linux-musl`) its SHA-256 and the exact members packaged. |
| `scripts/assemble.mjs` | Release/CI assembly (never packaged, never run by the app). |
| `scripts/stage-helper.mjs` | Release: builds the verifier sandbox helper and stages it as the Linux package's Tauri sidecar (P2-R1). |
| `scripts/inspect-deb.mjs` | Release: inspects the Debian package as it ships (P2-R1). |
| `test/inspect-deb.test.mjs` | The inspection's negative controls (`node --test packaging/verifier-toolchain/test/`; needs `dpkg-deb`). |

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
`verifier-toolchain` resource (`app/src-tauri/tauri.verifier-runtime.conf.json`),
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

## Linux package (P2-R1)

The Linux release job (`.github/workflows/release.yml`, `build-linux`)
assembles both toolchains, builds with `NEXUS_BUILDER_TOOLCHAIN=packaged`
and `NEXUS_VERIFIER_TOOLCHAIN=packaged`, and then:

```
node packaging/verifier-toolchain/scripts/stage-helper.mjs --out app/src-tauri/binaries
npm run tauri build -- --bundles deb --config src-tauri/tauri.builder-toolchain.conf.json --config src-tauri/tauri.verifier-runtime.conf.json
node --test packaging/verifier-toolchain/test/
node packaging/verifier-toolchain/scripts/inspect-deb.mjs --deb <.deb> --application nexus-desktop-backend \
  --builder-toolchain app/src-tauri/builder-toolchain --verifier-toolchain app/src-tauri/verifier-toolchain \
  --helper app/src-tauri/binaries/nexus-verifier-sandbox-x86_64-unknown-linux-gnu --helper-sha256 <staged digest>
```

`stage-helper.mjs` builds the helper with cargo (`--release --locked`; the
path is the one cargo reports) and creates the sidecar
`nexus-verifier-sandbox-x86_64-unknown-linux-gnu` exclusively, mode 0755,
printing `sha256=` and `bytes=`. Tauri installs it at exactly
`/usr/bin/nexus-verifier-sandbox`, the path `HelperProgram::installed()`
derives, beside the application. The package has no maintainer script.

`inspect-deb.mjs` reads the archive itself (nothing is installed or
extracted) and refuses it unless: every entry is root-owned (uid and gid 0),
a directory or a regular file, writable by no one else and without set-id
or sticky bits (directories and executables 0755, other files 0644);
`md5sums` records every file; `usr/bin` holds exactly the application and
the helper; the helper is byte-identical to the staged sidecar and its
digest, and nothing else is, or is named like, the helper; both toolchains
are exactly their assembled trees; no other ELF file exists; and the
application embeds the digest of every verifier toolchain file (the
manifest it was built with). The job then checks that the backend's
production feature graph has no development or harness verifier feature,
and evaluates the extracted package (`dpkg-deb -x`) with production's own
layout and tree checks, the extracting user as owner
(`crates/nexus-verifier-sandbox/tests/phase2_package_layout.rs`).

An installed package that lacks the helper or the toolchain, or whose
files are not root-owned and protected, or a backend built without the
manifest, leaves verification unavailable: it fails closed.
