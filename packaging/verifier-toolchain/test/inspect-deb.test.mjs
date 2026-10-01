// P2-R1 negative controls of the Debian package inspection
// (scripts/inspect-deb.mjs): a synthetic package with the release layout is
// accepted, and every deviation the release must never ship is refused.
// Needs `dpkg-deb` (Debian and Ubuntu hosts):
//   node --test packaging/verifier-toolchain/test/
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import crypto from 'node:crypto';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { test } from 'node:test';
import { fileURLToPath } from 'node:url';

import { inspect, Rejected } from '../scripts/inspect-deb.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));
const SIDECAR = 'nexus-verifier-sandbox-x86_64-unknown-linux-gnu';
const elf = (text) => Buffer.concat([Buffer.from('\x7fELF', 'latin1'), Buffer.from(text)]);
const sha256 = (bytes) => crypto.createHash('sha256').update(bytes).digest();
const md5 = (bytes) => crypto.createHash('md5').update(bytes).digest('hex');

function write(file, bytes, mode) {
  fs.mkdirSync(path.dirname(file), { recursive: true, mode: 0o755 });
  fs.writeFileSync(file, bytes);
  fs.chmodSync(file, mode);
}

/// The assembled trees, the staged helper and a package root with the
/// release layout, in a fresh directory.
function fixture() {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'nexus-p2r1-deb-'));
  const builder = {
    'node/node': [elf('node'), 0o755],
    'node/LICENSE': [Buffer.from('license\n'), 0o644],
    'entry/nexus-builder.mjs': [Buffer.from('export {};\n'), 0o644],
  };
  const verifier = {
    'bin/cargo': [elf('cargo'), 0o755],
    'bin/rustc': [elf('rustc'), 0o755],
    'lib/rustlib/x86_64-unknown-linux-musl/lib/libstd.rlib': [Buffer.from('rlib'), 0o644],
  };
  const helper = elf('helper');
  // The application embeds the verifier manifest: each file's digest.
  const application = Buffer.concat([
    elf('application'),
    ...Object.values(verifier).map(([bytes]) => sha256(bytes)),
  ]);
  const files = {
    'usr/bin/nexus-desktop-backend': [application, 0o755],
    'usr/bin/nexus-verifier-sandbox': [helper, 0o755],
    'usr/share/applications/NexusOS.desktop': [Buffer.from('[Desktop Entry]\n'), 0o644],
  };
  for (const [name, entry] of Object.entries(builder)) {
    write(path.join(dir, 'builder', name), ...entry);
    files[`usr/lib/NexusOS/toolchain/${name}`] = entry;
  }
  for (const [name, entry] of Object.entries(verifier)) {
    write(path.join(dir, 'verifier', name), ...entry);
    files[`usr/lib/NexusOS/verifier-toolchain/${name}`] = entry;
  }
  write(path.join(dir, 'binaries', SIDECAR), helper, 0o755);
  return {
    dir,
    files,
    control: ['Package: nexus-os', 'Version: 9.0.0', 'Architecture: amd64', 'Maintainer: nexus-os', 'Description: test', ''].join('\n'),
    scripts: {},
    rootOwned: true,
    md5Overrides: {},
    links: {},
    options: {
      application: 'nexus-desktop-backend',
      builderToolchain: path.join(dir, 'builder'),
      verifierToolchain: path.join(dir, 'verifier'),
      helper: path.join(dir, 'binaries', SIDECAR),
      helperSha256: sha256(helper).toString('hex'),
    },
  };
}

/// Build the fixture's package with dpkg-deb and inspect it.
function inspected(package_) {
  const fixture = package_;
  const root = path.join(fixture.dir, 'root');
  fs.rmSync(root, { recursive: true, force: true });
  fs.mkdirSync(path.join(root, 'DEBIAN'), { recursive: true, mode: 0o755 });
  const sums = [];
  for (const [name, [bytes, mode]] of Object.entries(fixture.files)) {
    write(path.join(root, name), bytes, mode);
    sums.push(`${fixture.md5Overrides[name] ?? md5(bytes)}  ${name}`);
  }
  for (const [name, target] of Object.entries(fixture.links)) {
    fs.mkdirSync(path.dirname(path.join(root, name)), { recursive: true, mode: 0o755 });
    fs.symlinkSync(target, path.join(root, name));
  }
  write(path.join(root, 'DEBIAN', 'control'), Buffer.from(fixture.control), 0o644);
  write(path.join(root, 'DEBIAN', 'md5sums'), Buffer.from(`${sums.join('\n')}\n`), 0o644);
  for (const [name, text] of Object.entries(fixture.scripts)) {
    write(path.join(root, 'DEBIAN', name), Buffer.from(text), 0o755);
  }
  for (const dir of [root, ...walkDirs(root)]) fs.chmodSync(dir, 0o755);
  const deb = path.join(fixture.dir, 'package.deb');
  const args = ['-Zgzip', '--build', root, deb];
  if (fixture.rootOwned) args.unshift('--root-owner-group');
  const built = spawnSync('dpkg-deb', args, { encoding: 'utf8' });
  assert.equal(built.status, 0, built.stderr);
  return inspect({ deb, ...fixture.options });
}

function walkDirs(dir) {
  return fs
    .readdirSync(dir, { withFileTypes: true })
    .filter((entry) => entry.isDirectory() && !entry.isSymbolicLink())
    .flatMap((entry) => [path.join(dir, entry.name), ...walkDirs(path.join(dir, entry.name))]);
}

/// The package, changed by `change`, is refused with `message`.
function refused(change, message) {
  const changed = fixture();
  try {
    change(changed);
    assert.throws(
      () => inspected(changed),
      (error) => error instanceof Rejected && message.test(error.message),
    );
  } finally {
    fs.rmSync(changed.dir, { recursive: true, force: true });
  }
}

test('dpkg-deb is available', () => {
  assert.equal(spawnSync('dpkg-deb', ['--version']).status, 0);
});

test('a package with the release layout is accepted', () => {
  const accepted = fixture();
  try {
    const report = inspected(accepted);
    assert.equal(report.helper.path, 'usr/bin/nexus-verifier-sandbox');
    assert.equal(report.verifierToolchainFiles, 3);
  } finally {
    fs.rmSync(accepted.dir, { recursive: true, force: true });
  }
});

test('the helper must be exactly the staged build, at exactly its path', () => {
  refused((f) => { f.files['usr/bin/nexus-verifier-sandbox'][1] = 0o775; }, /nexus-verifier-sandbox: mode 775/);
  refused((f) => { f.files['usr/bin/nexus-verifier-sandbox'][1] = 0o4755; }, /set-id/);
  refused((f) => { f.files['usr/bin/nexus-verifier-sandbox'][0] = elf('other helper'); }, /not the staged sidecar/);
  refused((f) => { f.options.helperSha256 = '0'.repeat(64); }, /not the helper staging built/);
  refused((f) => { delete f.files['usr/bin/nexus-verifier-sandbox']; }, /usr\/bin must hold exactly/);
  refused((f) => { f.files['usr/bin/extra'] = [elf('extra'), 0o755]; }, /usr\/bin must hold exactly/);
  refused((f) => {
    f.files['usr/lib/NexusOS/helper-copy'] = [f.files['usr/bin/nexus-verifier-sandbox'][0], 0o755];
  }, /a second copy of the helper/);
  refused((f) => {
    f.files['usr/lib/nexus-verifier-sandbox'] = [Buffer.from('#!/bin/sh\n'), 0o755];
  }, /a second copy of the helper/);
  refused((f) => {
    fs.chmodSync(f.options.helper, 0o775);
  }, /staged sidecar is not a 0755 regular file/);
});

test('both toolchains must be exactly the assembled trees', () => {
  const verifier = 'usr/lib/NexusOS/verifier-toolchain';
  refused((f) => { delete f.files[`${verifier}/bin/rustc`]; }, /verifier toolchain: missing file bin\/rustc/);
  refused((f) => { f.files[`${verifier}/bin/extra`] = [Buffer.from('x'), 0o644]; }, /verifier toolchain: unexpected file bin\/extra/);
  refused((f) => { f.files[`${verifier}/bin/cargo`][0] = elf('changed'); }, /bin\/cargo differs/);
  refused((f) => { f.files[`${verifier}/bin/cargo`][1] = 0o644; }, /bin\/cargo has the wrong executable bit/);
  refused((f) => { delete f.files['usr/lib/NexusOS/toolchain/node/LICENSE']; }, /Builder toolchain: missing file node\/LICENSE/);
});

test('the application must embed the verifier manifest', () => {
  refused((f) => { f.files['usr/bin/nexus-desktop-backend'][0] = elf('application without a manifest'); }, /does not embed the verifier manifest/);
});

test('nothing runs at install, and everything is root-owned and plain', () => {
  refused((f) => { f.scripts.postinst = '#!/bin/sh\nexit 0\n'; }, /control members must be exactly control, md5sums/);
  refused((f) => { f.rootOwned = false; }, /not root/);
  refused((f) => { f.links['usr/lib/NexusOS/link'] = '/etc/passwd'; }, /not a regular file or directory/);
  refused((f) => { f.files['usr/share/applications/NexusOS.desktop'][1] = 0o664; }, /mode 664/);
  refused((f) => { f.files['usr/share/nexus/plugin.so'] = [elf('plugin'), 0o644]; }, /an unexpected ELF file/);
  refused((f) => { f.md5Overrides['usr/bin/nexus-desktop-backend'] = '0'.repeat(32); }, /md5sums does not record/);
  refused((f) => { f.control = f.control.replace('amd64', 'arm64'); }, /not for amd64/);
});

test('the inspection script is the one the release runs', () => {
  const workflow = fs.readFileSync(path.join(here, '..', '..', '..', '.github', 'workflows', 'release.yml'), 'utf8');
  assert.ok(workflow.includes('node packaging/verifier-toolchain/scripts/inspect-deb.mjs'));
  assert.ok(workflow.includes('node --test packaging/verifier-toolchain/test/'));
});
