# Retained verifier toolchain tooling

These scripts and pin data support the retained Linux x86_64 verifier library.
They do **not** provide a Mother release, installer or permission to restore the
removed desktop app. The former app packaging/release recipe is retired and
remains in Git history. Mother is native GTK4 only.

## Retained contract

`rust-release.json` pins the Rust components, channel manifest and exact archive
members. `scripts/assemble.mjs` verifies all input archives and their relationship
to that manifest, refuses missing/repeated/non-regular members, and writes only
the pinned files. Executables are 0755 and other files 0644. It can compare trees:

```sh
node packaging/verifier-toolchain/scripts/assemble.mjs --compare <expected> <actual>
node --test packaging/verifier-toolchain/test/inspect-deb.test.mjs
```

These are build/test tools, never application authority. Assembly with `--out`
can fetch pinned archives unless `--archives` supplies them; this is not a setup
step to run without an authorized packaging mission. The package-inspection
negative controls use synthetic archives and need `dpkg-deb`, not an installed app.

The sandbox build script embeds a manifest only when
`NEXUS_VERIFIER_TOOLCHAIN=packaged` validates the assembled tree. It still expects
`app/src-tauri/verifier-toolchain/`, a **retained compatibility path**, not a
present application. A normal build has no embedded manifest and verification
stays unavailable. Do not recreate the removed app to satisfy it.

The library derives the installed root `/usr/lib/NexusOS/verifier-toolchain`
from an executable under `/usr/bin`. Those names are fixed compatibility
identities in source, not Mother branding or an installed-build claim. The root,
parents, files and host runtime libraries must be root-owned and protected;
the embedded path/size/digest/mode manifest must match exactly. No disk manifest
or user's rustup tree becomes authority. Reverify immediately before launch and
execute by descriptor. A missing helper or manifest fails closed.

`scripts/stage-helper.mjs` builds and exclusively stages the helper with exact
bytes/mode and emits its digest and size. Its historic sidecar name is retained
in code; there is no active app bundling command here. `scripts/inspect-deb.mjs`
checks archive ownership/modes, file inventory, checksums, helper identity,
toolchain trees and embedded manifest. Its synthetic tests and
`phase2_package_layout` exercise that contract, not a released Mother package.
Development-toolchain paths are test-only and never production authority.

A new Mother packaging route needs a bounded approved mission, its own current
recipe and installed-host evidence. Nothing here authorizes installs, privilege
changes, release tags, network downloads or a weaker verifier mode. See the
[verifier boundary](../../docs/security/phase2-governed-verification.md).
