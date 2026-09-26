# Nexus Builder toolchain (P0-002C4D2, P0-002C4C3)

The Nexus-owned frontend runtime the desktop app packages for Builder
projects. It is the Builder runtime contract; `app/package.json` is not.

## Contents

| File | Role |
|---|---|
| `package.json`, `package-lock.json` | Exact runtime closure: Vite 8.3.1, @vitejs/plugin-react 6.1.1, React/ReactDOM 19.3.0, react-router-dom 7.18.4, Tailwind CSS 3.4.19 (`v3-lts`), PostCSS 8.5.28. No dev, test or type-checking packages. |
| `node-release.json` | Node.js 24.21.0 (LTS "Krypton") and npm 11.19.0 pins: per-target official archive, SHA-256 (from the release-key-signed `SHASUMS256.txt`) and member paths. |
| `entry/` | Nexus-owned entry and programmatic Vite configuration, packaged as `toolchain/entry/`. |
| `scripts/assemble.mjs` | Release/CI assembly (never packaged, never run by the app). |
| `test/entry.test.mjs` | Trusted-entry tests, run with the packaged Node against an assembled toolchain. |

## Packaged tree

`node/node` (`node/node.exe` on Windows), `node/LICENSE`, `entry/**` and the
target's `node_modules/**`, exactly as `npm ci --ignore-scripts
--no-bin-links --omit=dev --os --cpu [--libc]` installs them. Supported
targets are the release targets: `x86_64-unknown-linux-gnu` (Debian package),
`x86_64-pc-windows-msvc` (NSIS/MSI) and `aarch64-apple-darwin` (DMG).

The desktop backend build script (`NEXUS_BUILDER_TOOLCHAIN=packaged`) hashes
the assembled `app/src-tauri/builder-toolchain/` tree, rejects anything the
runtime verifier would reject or any native binary not built for the target,
and embeds the exact manifest. Tauri bundles the same directory as the
`toolchain` resource (`app/src-tauri/tauri.builder-toolchain.conf.json`). At
runtime the root is derived only from the installed executable path and the
tree must match the embedded manifest exactly. Builds without an assembled
toolchain embed no manifest and stay unavailable.

## Trusted entry

Before any third-party module is imported, `entry/nexus-builder.mjs` confines
Node module loading to the toolchain (`entry/module-guard.mjs`), makes every
`child_process` API fail without creating a process (`entry/process-guard.mjs`)
and makes existence probes answer `false` for locations the permission model
denies (`entry/fs-probe.mjs`). It then validates the backend request and builds
the trusted configuration (`entry/trusted-config.mjs`). The configuration never
loads project Vite, PostCSS, Tailwind, Babel or TypeScript configuration,
`.env` files or package metadata; resolves bare imports only from the
toolchain; rejects CSS preprocessors, local CSS `@import` and the
JavaScript-loading `@config` and `@plugin` directives; and owns the Tailwind
theme mapping the generator's `tailwind.config.ts` expresses.

## Governed dev-server preview (P0-002C4C3)

The desktop backend freshly verifies the installed toolchain for every launch,
then runs exactly `node/node` against `entry/nexus-builder.mjs` through its
sealed, resource-limited spawn, with an empty environment (runtime home and
temp only), the project's governed `runtime/` as working directory and the
Node permission model: read the toolchain, the project's `react/` and
`runtime/{home,tmp,vite-cache}`; write only those three runtime children;
native addons (loaded only from the toolchain); no child-process, worker,
WASI, inspector or OpenSSL-store permission.

`entry/preview-server.mjs` runs Vite in middleware mode (no HMR, no WebSocket,
no CORS, no browser, a watcher that never follows links) behind the Nexus gate
on a Nexus-owned `http.Server` bound to `127.0.0.1` on an OS-assigned port. The
gate admits only `GET`/`HEAD` whose `Host` is exactly `127.0.0.1:<port>`, whose
`Origin` (if any) is that exact origin, and whose path, in any case or
percent-decoded spelling, is not the editor-launch route `/__open-in-editor`.
The entry writes exactly one readiness line,
`{"event":"ready","host":"127.0.0.1","port":<port>}`; the backend then probes
`GET /` (no redirects) for this launch's token before publishing the preview.
On Windows, Vite's `net use` question is answered "no mapped network drives"
without a process: the backend grants only local drive paths.

Not claimed: filesystem or network sandboxing, safe execution of arbitrary
project JavaScript, protection against a hostile same-user process racing
filesystem state, atomic toolchain verification, protection from every Node
permission-model limitation (native addons are outside it), or production
web-server suitability.

## Local use

```sh
node packaging/builder-toolchain/scripts/assemble.mjs \
  --target x86_64-unknown-linux-gnu --out app/src-tauri/builder-toolchain
app/src-tauri/builder-toolchain/node/node --test packaging/builder-toolchain/test/entry.test.mjs
NEXUS_BUILDER_TOOLCHAIN=packaged cargo test -p nexus-desktop-backend --lib -- \
  p0_002c4d2_assembled_ p0_002c4c3_packaged_
```

Assembly requires Node.js 24.21.0 with its bundled npm 11.19.0.
