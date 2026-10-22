#!/usr/bin/env node
// Phase Two (P2-R1): build the verifier sandbox helper from this checkout and
// stage it as the Tauri sidecar that the Linux package installs at exactly
// /usr/bin/nexus-verifier-sandbox (the path `HelperProgram::installed`
// derives). Release/CI only; never packaged, never run by the app.
//
//   node packaging/verifier-toolchain/scripts/stage-helper.mjs --out app/src-tauri/binaries
//
// cargo builds the helper (`--release --locked`, this package's binary only)
// and reports where it is: the path is never guessed from a target
// directory. The staged sidecar is created exclusively (an existing one is
// never reused), with the helper's exact bytes and mode 0755, and read back.
// Prints `sha256=<hex>` and `bytes=<n>` (GitHub step-output lines) so the
// package inspection can bind the packaged helper to this build.
import { spawnSync } from 'node:child_process';
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';

const PACKAGE = 'nexus-verifier-sandbox';
const BINARY = 'nexus-verifier-sandbox';
// Tauri's sidecar convention: `<externalBin>-<target triple>`, installed by
// the Debian package as `/usr/bin/<externalBin name>`.
const TRIPLE = 'x86_64-unknown-linux-gnu';
const SIDECAR = `${BINARY}-${TRIPLE}`;

function fail(message) {
  console.error(`verifier helper staging failed: ${message}`);
  process.exit(1);
}

function args() {
  const argv = process.argv.slice(2);
  if (argv.length !== 2 || argv[0] !== '--out' || !argv[1]) {
    fail('usage: stage-helper.mjs --out <directory>');
  }
  return argv[1];
}

/// The helper cargo builds from this checkout: its reported executable.
function build() {
  const result = spawnSync(
    'cargo',
    [
      'build',
      '--release',
      '--locked',
      '-p',
      PACKAGE,
      '--bin',
      BINARY,
      '--message-format=json-render-diagnostics',
    ],
    { encoding: 'utf8', stdio: ['ignore', 'pipe', 'inherit'], maxBuffer: 1 << 28 },
  );
  if (result.status !== 0) fail('cargo build failed');
  const executables = result.stdout
    .split('\n')
    .filter((line) => line.startsWith('{'))
    .map((line) => JSON.parse(line))
    .filter(
      (message) =>
        message.reason === 'compiler-artifact' &&
        message.target?.name === BINARY &&
        message.target?.kind?.includes('bin') &&
        typeof message.executable === 'string',
    )
    .map((message) => message.executable);
  if (executables.length !== 1) {
    fail(`cargo reported ${executables.length} ${BINARY} executables, not exactly one`);
  }
  return executables[0];
}

function main() {
  const out = args();
  if (process.platform !== 'linux' || process.arch !== 'x64') {
    fail('the verifier helper is packaged for x86_64 Linux only');
  }
  const helper = build();
  const metadata = fs.lstatSync(helper);
  if (!metadata.isFile()) fail('the built helper is not a regular file');
  const bytes = fs.readFileSync(helper);
  if (bytes.length !== metadata.size || !bytes.subarray(0, 4).equals(Buffer.from('\x7fELF', 'latin1'))) {
    fail('the built helper is not an ELF executable');
  }
  fs.mkdirSync(out, { recursive: true });
  const sidecar = path.join(out, SIDECAR);
  // Exclusive: a sidecar left by anything else is never packaged.
  fs.writeFileSync(sidecar, bytes, { flag: 'wx', mode: 0o755 });
  fs.chmodSync(sidecar, 0o755);
  const staged = fs.lstatSync(sidecar);
  if (!staged.isFile() || (staged.mode & 0o7777) !== 0o755 || !fs.readFileSync(sidecar).equals(bytes)) {
    fail('the staged sidecar is not exactly the built helper');
  }
  const sha256 = crypto.createHash('sha256').update(bytes).digest('hex');
  console.error(`staged ${helper} as ${sidecar} (${bytes.length} bytes, sha256 ${sha256})`);
  process.stdout.write(`sha256=${sha256}\nbytes=${bytes.length}\n`);
}

main();
