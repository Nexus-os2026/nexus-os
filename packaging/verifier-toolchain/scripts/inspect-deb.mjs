#!/usr/bin/env node
// Phase Two (P2-R1): inspect the Linux Debian package without installing or
// extracting it. Release/CI only; never packaged, never run by the app.
//
//   node packaging/verifier-toolchain/scripts/inspect-deb.mjs --deb <file.deb> \
//     --application nexus-desktop-backend \
//     --builder-toolchain app/src-tauri/builder-toolchain \
//     --verifier-toolchain app/src-tauri/verifier-toolchain \
//     --helper app/src-tauri/binaries/nexus-verifier-sandbox-x86_64-unknown-linux-gnu \
//     --helper-sha256 <the digest staging printed>
//
// The archive is read as it ships (the ar container, then the control and
// data tarballs). It proves:
// - no maintainer script: nothing runs when the package is installed;
// - every entry is owned by uid 0 and gid 0, is a directory or a regular
//   file (no link, device or special file), is writable by no one else and
//   carries no set-id or sticky bit; directories and executables are 0755,
//   other files 0644; `md5sums` lists every file with its digest;
// - `usr/bin` holds exactly the application and the helper,
//   `usr/bin/nexus-verifier-sandbox`; the helper is exactly the staged
//   sidecar and the build's helper (the digest staging printed), and no
//   other entry is, or is named like, the helper;
// - `usr/lib/NexusOS/verifier-toolchain` is exactly the assembled verifier
//   tree and `usr/lib/NexusOS/toolchain` exactly the assembled Builder tree
//   (the same paths, bytes and executable bits, and no other entry);
// - no ELF file exists outside those trees and the two executables;
// - the application embeds the verifier toolchain manifest: the SHA-256 of
//   every file of the assembled verifier tree occurs in its bytes.
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import zlib from 'node:zlib';

const HELPER = 'usr/bin/nexus-verifier-sandbox';
const VERIFIER_ROOT = 'usr/lib/NexusOS/verifier-toolchain';
const BUILDER_ROOT = 'usr/lib/NexusOS/toolchain';
const CONTROL_FILES = ['control', 'md5sums'];
const ELF = Buffer.from('\x7fELF', 'latin1');

export class Rejected extends Error {}

function reject(message) {
  throw new Rejected(message);
}

const sha256 = (bytes) => crypto.createHash('sha256').update(bytes).digest('hex');
const md5 = (bytes) => crypto.createHash('md5').update(bytes).digest('hex');

/// The members of an `ar` archive, by name.
function arMembers(archive) {
  if (!archive.subarray(0, 8).equals(Buffer.from('!<arch>\n', 'latin1'))) reject('not an ar archive');
  const members = [];
  let offset = 8;
  while (offset < archive.length) {
    if (offset + 60 > archive.length) reject('truncated ar header');
    const header = archive.subarray(offset, offset + 60);
    if (header.subarray(58, 60).toString('latin1') !== '`\n') reject('malformed ar header');
    const name = header.subarray(0, 16).toString('latin1').trim().replace(/\/$/, '');
    const size = Number.parseInt(header.subarray(48, 58).toString('latin1').trim(), 10);
    if (!Number.isSafeInteger(size) || size < 0) reject(`malformed ar member size for ${name}`);
    const start = offset + 60;
    if (start + size > archive.length) reject(`truncated ar member ${name}`);
    members.push({ name, data: archive.subarray(start, start + size) });
    offset = start + size + (size % 2);
  }
  return members;
}

function decompress(name, data) {
  if (name.endsWith('.tar.gz')) return zlib.gunzipSync(data);
  if (name.endsWith('.tar.zst') && typeof zlib.zstdDecompressSync === 'function') {
    return zlib.zstdDecompressSync(data);
  }
  if (name.endsWith('.tar')) return data;
  return reject(`unsupported member compression: ${name}`);
}

/// A tar numeric field: octal text, or base-256 when the high bit is set.
function number(field) {
  if (field[0] & 0x80) {
    let value = BigInt(field[0] & 0x7f);
    for (const byte of field.subarray(1)) value = (value << 8n) | BigInt(byte);
    if (value > BigInt(Number.MAX_SAFE_INTEGER)) reject('tar number out of range');
    return Number(value);
  }
  const text = field.toString('latin1').replace(/\0.*$/s, '').trim();
  if (text === '') return 0;
  if (!/^[0-7]+$/.test(text)) reject(`malformed tar number ${JSON.stringify(text)}`);
  return Number.parseInt(text, 8);
}

const cString = (bytes) => bytes.toString('utf8').replace(/\0.*$/s, '');

/// Every entry of a tar archive (GNU long names and pax paths applied),
/// with a normalized relative path.
function tarEntries(tar) {
  const entries = [];
  let offset = 0;
  let longName = null;
  let paxPath = null;
  while (offset + 512 <= tar.length) {
    const header = tar.subarray(offset, offset + 512);
    if (header.every((byte) => byte === 0)) break;
    const stored = number(header.subarray(148, 156));
    let sum = 0;
    for (let i = 0; i < 512; i += 1) sum += i >= 148 && i < 156 ? 0x20 : header[i];
    if (sum !== stored) reject('tar header checksum mismatch');
    const size = number(header.subarray(124, 136));
    const type = header[156] === 0 ? '0' : String.fromCharCode(header[156]);
    const start = offset + 512;
    if (start + size > tar.length) reject('truncated tar entry');
    const data = tar.subarray(start, start + size);
    offset = start + Math.ceil(size / 512) * 512;
    if (type === 'L') {
      longName = cString(data);
      continue;
    }
    if (type === 'x') {
      for (const record of data.toString('utf8').split('\n')) {
        const match = /^\d+ path=(.*)$/s.exec(record);
        if (match) paxPath = match[1];
      }
      continue;
    }
    if (type === 'g' || type === 'K') reject(`unsupported tar entry type ${type}`);
    const ustar = header.subarray(257, 262).toString('latin1') === 'ustar';
    const prefix = ustar ? cString(header.subarray(345, 500)) : '';
    const name = cString(header.subarray(0, 100));
    let entryPath = longName ?? paxPath ?? (prefix ? `${prefix}/${name}` : name);
    longName = null;
    paxPath = null;
    entryPath = entryPath.replace(/^\.\//, '').replace(/\/+$/, '');
    if (entryPath === '.' || entryPath === '') continue;
    if (entryPath.startsWith('/') || entryPath.split('/').some((part) => part === '..' || part === '.' || part === '')) {
      reject(`unsafe path ${JSON.stringify(entryPath)}`);
    }
    entries.push({
      path: entryPath,
      type,
      mode: number(header.subarray(100, 108)),
      uid: number(header.subarray(108, 116)),
      gid: number(header.subarray(116, 124)),
      data,
    });
  }
  return entries;
}

/// The files (with digest and executable bit) and directories of an
/// assembled tree.
function assembled(root) {
  const files = new Map();
  const dirs = new Set();
  const walk = (dir, prefix) => {
    for (const name of fs.readdirSync(dir).sort()) {
      const full = path.join(dir, name);
      const relative = prefix ? `${prefix}/${name}` : name;
      const metadata = fs.lstatSync(full);
      if (metadata.isDirectory()) {
        dirs.add(relative);
        walk(full, relative);
      } else if (metadata.isFile()) {
        files.set(relative, { sha256: sha256(fs.readFileSync(full)), executable: (metadata.mode & 0o100) !== 0 });
      } else {
        reject(`assembled tree entry is neither a file nor a directory: ${relative}`);
      }
    }
  };
  walk(root, '');
  if (files.size === 0) reject(`empty assembled tree ${root}`);
  return { files, dirs };
}

/// The package's `prefix` subtree is exactly the assembled tree at `root`.
function exactTree(entries, prefix, root, what) {
  const expected = assembled(root);
  const files = new Map();
  const dirs = new Set();
  for (const entry of entries) {
    if (!entry.path.startsWith(`${prefix}/`)) continue;
    const relative = entry.path.slice(prefix.length + 1);
    if (entry.type === '5') dirs.add(relative);
    else files.set(relative, entry);
  }
  for (const [relative, entry] of files) {
    const want = expected.files.get(relative);
    if (!want) reject(`${what}: unexpected file ${relative}`);
    if (sha256(entry.data) !== want.sha256) reject(`${what}: ${relative} differs from the assembled tree`);
    if (((entry.mode & 0o100) !== 0) !== want.executable) reject(`${what}: ${relative} has the wrong executable bit`);
  }
  for (const relative of expected.files.keys()) {
    if (!files.has(relative)) reject(`${what}: missing file ${relative}`);
  }
  for (const relative of dirs) {
    if (!expected.dirs.has(relative)) reject(`${what}: unexpected directory ${relative}`);
  }
  for (const relative of expected.dirs) {
    if (!dirs.has(relative)) reject(`${what}: missing directory ${relative}`);
  }
  if (!entries.some((entry) => entry.path === prefix && entry.type === '5')) reject(`${what}: no ${prefix} directory`);
  return expected;
}

/// Inspect the package; throws `Rejected` with the first violation found.
export function inspect({ deb, application, builderToolchain, verifierToolchain, helper, helperSha256 }) {
  if (!/^[0-9a-f]{64}$/.test(helperSha256 ?? '')) reject('the staged helper digest must be 64 lowercase hex digits');
  if (!/^[A-Za-z0-9._-]+$/.test(application ?? '')) reject('the application name must be a plain file name');
  const members = arMembers(fs.readFileSync(deb));
  if (members.length !== 3 || members[0].name !== 'debian-binary' || members[0].data.toString('latin1') !== '2.0\n') {
    reject('not a Debian binary package of format 2.0 (debian-binary, control, data)');
  }
  const control = members[1];
  const data = members[2];
  if (!control.name.startsWith('control.tar') || !data.name.startsWith('data.tar')) reject('unexpected package members');

  // No maintainer script: nothing runs at install.
  const controlEntries = tarEntries(decompress(control.name, control.data));
  const controlFiles = controlEntries.filter((entry) => entry.type !== '5').map((entry) => entry.path).sort();
  if (JSON.stringify(controlFiles) !== JSON.stringify(CONTROL_FILES)) {
    reject(`control members must be exactly ${CONTROL_FILES.join(', ')}: ${controlFiles.join(', ')}`);
  }
  const controlText = controlEntries.find((entry) => entry.path === 'control').data.toString('utf8');
  if (!/^Architecture: amd64$/m.test(controlText)) reject('the package is not for amd64');

  const entries = tarEntries(decompress(data.name, data.data));
  const byPath = new Map();
  for (const entry of entries) {
    if (byPath.has(entry.path)) reject(`duplicate entry ${entry.path}`);
    byPath.set(entry.path, entry);
  }
  // Ownership and modes, as installed: root-owned, writable by no one else.
  for (const entry of entries) {
    if (entry.uid !== 0 || entry.gid !== 0) reject(`${entry.path}: owned by ${entry.uid}:${entry.gid}, not root`);
    if (entry.type !== '0' && entry.type !== '5') reject(`${entry.path}: not a regular file or directory (type ${entry.type})`);
    if (entry.mode & 0o7000) reject(`${entry.path}: set-id or sticky bit`);
    const executable = entry.type === '5' || (entry.mode & 0o111) !== 0;
    const expected = executable ? 0o755 : 0o644;
    if ((entry.mode & 0o777) !== expected) {
      reject(`${entry.path}: mode ${(entry.mode & 0o777).toString(8)}, not ${expected.toString(8)}`);
    }
  }
  // dpkg's own record of every file.
  const sums = new Map();
  for (const line of controlEntries.find((entry) => entry.path === 'md5sums').data.toString('utf8').split('\n')) {
    if (!line) continue;
    const match = /^([0-9a-f]{32}) {2}(.+)$/.exec(line);
    if (!match) reject('malformed md5sums line');
    sums.set(match[2].replace(/^\.\//, ''), match[1]);
  }
  const files = entries.filter((entry) => entry.type === '0');
  for (const entry of files) {
    if (sums.get(entry.path) !== md5(entry.data)) reject(`${entry.path}: md5sums does not record its content`);
  }
  if (sums.size !== files.length) reject('md5sums names files the package does not hold');

  // usr/bin: exactly the application and the helper.
  const appPath = `usr/bin/${application}`;
  const bin = files.filter((entry) => path.posix.dirname(entry.path) === 'usr/bin').map((entry) => entry.path).sort();
  if (JSON.stringify(bin) !== JSON.stringify([appPath, HELPER].sort())) {
    reject(`usr/bin must hold exactly ${application} and nexus-verifier-sandbox: ${bin.join(', ')}`);
  }
  const app = byPath.get(appPath);
  const packagedHelper = byPath.get(HELPER);
  for (const executable of [app, packagedHelper]) {
    if ((executable.mode & 0o777) !== 0o755 || !executable.data.subarray(0, 4).equals(ELF)) {
      reject(`${executable.path}: not a 0755 ELF executable`);
    }
  }
  // The helper is exactly the staged sidecar, which is the build's helper.
  const staged = fs.lstatSync(helper);
  if (!staged.isFile() || (staged.mode & 0o7777) !== 0o755) reject('the staged sidecar is not a 0755 regular file');
  if (!packagedHelper.data.equals(fs.readFileSync(helper))) reject('the packaged helper is not the staged sidecar');
  if (sha256(packagedHelper.data) !== helperSha256) reject('the packaged helper is not the helper staging built');
  for (const entry of files) {
    if (entry === packagedHelper) continue;
    if (path.posix.basename(entry.path).includes('nexus-verifier-sandbox') || sha256(entry.data) === helperSha256) {
      reject(`${entry.path}: a second copy of the helper`);
    }
  }

  // The two toolchains, exactly.
  const verifier = exactTree(entries, VERIFIER_ROOT, verifierToolchain, 'verifier toolchain');
  exactTree(entries, BUILDER_ROOT, builderToolchain, 'Builder toolchain');
  // No ELF anywhere else.
  for (const entry of files) {
    const allowed =
      entry === app ||
      entry === packagedHelper ||
      entry.path.startsWith(`${VERIFIER_ROOT}/`) ||
      entry.path.startsWith(`${BUILDER_ROOT}/`);
    if (!allowed && entry.data.subarray(0, 4).equals(ELF)) reject(`${entry.path}: an unexpected ELF file`);
  }
  // The application embeds the verifier toolchain manifest.
  for (const [relative, file] of verifier.files) {
    if (app.data.indexOf(Buffer.from(file.sha256, 'hex')) < 0) {
      reject(`the application does not embed the verifier manifest (${relative})`);
    }
  }
  return {
    entries: entries.length,
    files: files.length,
    application: { path: appPath, sha256: sha256(app.data) },
    helper: { path: HELPER, sha256: helperSha256, bytes: packagedHelper.data.length },
    verifierToolchainFiles: verifier.files.size,
  };
}

function cli() {
  const options = {};
  const names = {
    '--deb': 'deb',
    '--application': 'application',
    '--builder-toolchain': 'builderToolchain',
    '--verifier-toolchain': 'verifierToolchain',
    '--helper': 'helper',
    '--helper-sha256': 'helperSha256',
  };
  const argv = process.argv.slice(2);
  for (let i = 0; i < argv.length; i += 2) {
    const key = names[argv[i]];
    if (!key || argv[i + 1] === undefined || key in options) {
      console.error(`usage: inspect-deb.mjs ${Object.keys(names).map((flag) => `${flag} <value>`).join(' ')}`);
      process.exit(2);
    }
    options[key] = argv[i + 1];
  }
  if (Object.keys(options).length !== Object.keys(names).length) {
    console.error(`inspect-deb: every option is required: ${Object.keys(names).join(' ')}`);
    process.exit(2);
  }
  try {
    console.log(JSON.stringify(inspect(options)));
  } catch (error) {
    if (!(error instanceof Rejected)) throw error;
    console.error(`package rejected: ${error.message}`);
    process.exit(1);
  }
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) cli();
