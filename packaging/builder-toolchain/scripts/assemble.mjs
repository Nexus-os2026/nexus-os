// P0-002C4D2: release/CI assembly of the Nexus-owned Builder toolchain.
// Build tooling only; it is never packaged and never run by the application.
//
//   node assemble.mjs --target <triple> --out <new dir> [--node-archive <file>]
//   node assemble.mjs --compare <expected dir> <actual dir>
//
// Produces exactly node/<node|node.exe>, node/LICENSE, entry/** and
// node_modules/**. It writes no manifest: the desktop backend build script
// (NEXUS_BUILDER_TOOLCHAIN=packaged) generates the embedded manifest from the
// produced bytes and re-validates the tree, targets and native binaries.
import { spawnSync } from 'node:child_process';
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import zlib from 'node:zlib';
import { fileURLToPath } from 'node:url';

const packageDir = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const pins = JSON.parse(fs.readFileSync(path.join(packageDir, 'node-release.json'), 'utf8'));
const ENTRY_FILES = [
  'module-guard.mjs',
  'nexus-builder.mjs',
  'tailwind-theme.mjs',
  'trusted-config.mjs',
  'tsconfig.json',
];
// The C4D1A manifest grammar and limits (the Rust verifier is authoritative).
const COMPONENT = /^[A-Za-z0-9._@+-]+$/;
const DEVICES = new Set(['CON', 'PRN', 'AUX', 'NUL', ...[1, 2, 3, 4, 5, 6, 7, 8, 9].flatMap((n) => [`COM${n}`, `LPT${n}`])]);
const MAX_FILES = 100_000;
const MAX_FILE_BYTES = 1 << 30;
const MAX_PATH_BYTES = 200;

class AssemblyError extends Error {}
const fail = (message) => {
  throw new AssemblyError(message);
};
const sha256 = (bytes) => crypto.createHash('sha256').update(bytes).digest('hex');

function validPath(relative) {
  return (
    relative.length > 0 &&
    Buffer.byteLength(relative) <= MAX_PATH_BYTES &&
    relative.split('/').every(
      (c) =>
        COMPONENT.test(c) &&
        c.length <= 255 &&
        c !== '.' &&
        c !== '..' &&
        !c.endsWith('.') &&
        !DEVICES.has(c.split('.')[0].toUpperCase()),
    )
  );
}

// Every regular file under `root` (sorted), rejecting links, special files and
// empty directories; directories are implied by files exactly as in C4D1A.
function listTree(root) {
  const files = [];
  const walk = (dir, prefix) => {
    const names = fs.readdirSync(dir).sort();
    if (names.length === 0) fail(`empty directory: ${prefix || '.'}`);
    for (const name of names) {
      const relative = prefix ? `${prefix}/${name}` : name;
      const stat = fs.lstatSync(path.join(dir, name));
      if (stat.isSymbolicLink()) fail(`link in toolchain tree: ${relative}`);
      if (stat.isDirectory()) walk(path.join(dir, name), relative);
      else if (stat.isFile()) files.push({ relative, size: stat.size });
      else fail(`unsupported entry in toolchain tree: ${relative}`);
    }
  };
  walk(root, '');
  return files;
}

// --- Archive extraction (a single pinned member; nothing else is written). ---

function tarMember(archive, member) {
  const tar = zlib.gunzipSync(archive);
  let found = null;
  for (let offset = 0; offset + 512 <= tar.length; ) {
    const header = tar.subarray(offset, offset + 512);
    if (header.every((b) => b === 0)) break;
    const field = (start, length) => header.subarray(start, start + length).toString('utf8').replace(/\0.*$/s, '');
    const size = Number.parseInt(field(124, 12).trim() || '0', 8);
    const prefix = field(345, 155);
    const name = prefix ? `${prefix}/${field(0, 100)}` : field(0, 100);
    const type = field(156, 1);
    const data = offset + 512;
    if (name === member) {
      if (found !== null || (type !== '0' && type !== '')) fail(`archive member not a unique file: ${member}`);
      found = Buffer.from(tar.subarray(data, data + size));
    }
    offset = data + Math.ceil(size / 512) * 512;
  }
  return found ?? fail(`archive member missing: ${member}`);
}

function zipMember(archive, member) {
  let eocd = -1;
  for (let i = archive.length - 22; i >= Math.max(0, archive.length - 65_557); i -= 1) {
    if (archive.readUInt32LE(i) === 0x06054b50) {
      eocd = i;
      break;
    }
  }
  if (eocd < 0) fail('zip end of central directory missing');
  const entries = archive.readUInt16LE(eocd + 10);
  let offset = archive.readUInt32LE(eocd + 16);
  let found = null;
  for (let n = 0; n < entries; n += 1) {
    if (archive.readUInt32LE(offset) !== 0x02014b50) fail('zip central directory corrupt');
    const method = archive.readUInt16LE(offset + 10);
    const crc = archive.readUInt32LE(offset + 16);
    const compressed = archive.readUInt32LE(offset + 20);
    const size = archive.readUInt32LE(offset + 24);
    const nameLength = archive.readUInt16LE(offset + 28);
    const extraLength = archive.readUInt16LE(offset + 30);
    const commentLength = archive.readUInt16LE(offset + 32);
    const local = archive.readUInt32LE(offset + 42);
    const name = archive.subarray(offset + 46, offset + 46 + nameLength).toString('utf8');
    if (name === member) {
      if (found !== null || compressed === 0xffffffff || size === 0xffffffff) {
        fail(`archive member not a unique supported file: ${member}`);
      }
      if (archive.readUInt32LE(local) !== 0x04034b50) fail('zip local header corrupt');
      const start = local + 30 + archive.readUInt16LE(local + 26) + archive.readUInt16LE(local + 28);
      const raw = archive.subarray(start, start + compressed);
      const bytes = method === 0 ? Buffer.from(raw) : method === 8 ? zlib.inflateRawSync(raw) : fail('zip method unsupported');
      if (bytes.length !== size || zlib.crc32(bytes) !== crc) fail(`archive member corrupt: ${member}`);
      found = bytes;
    }
    offset += 46 + nameLength + extraLength + commentLength;
  }
  return found ?? fail(`archive member missing: ${member}`);
}

function archiveMember(archive, name, member) {
  if (name.endsWith('.tar.gz')) return tarMember(archive, member);
  if (name.endsWith('.zip')) return zipMember(archive, member);
  return fail(`unsupported archive: ${name}`);
}

async function nodeArchive(pin, supplied) {
  const bytes = supplied
    ? fs.readFileSync(supplied)
    : Buffer.from(await (await fetch(new URL(pin.archive, pins.source))).arrayBuffer());
  if (sha256(bytes) !== pin.sha256) fail(`Node archive digest mismatch: ${pin.archive}`);
  return bytes;
}

// --- Dependency closure (release-time npm only; lifecycle scripts never run). ---

function npm(args, cwd, home) {
  const cli =
    process.platform === 'win32'
      ? path.join(path.dirname(process.execPath), 'node_modules', 'npm', 'bin', 'npm-cli.js')
      : path.join(path.dirname(process.execPath), '..', 'lib', 'node_modules', 'npm', 'bin', 'npm-cli.js');
  const userconfig = path.join(home, 'npmrc');
  fs.writeFileSync(userconfig, '');
  const env = {
    PATH: process.env.PATH ?? '',
    HOME: home,
    USERPROFILE: home,
    TMPDIR: home,
    TEMP: home,
    TMP: home,
    ...(process.platform === 'win32' ? { SystemRoot: process.env.SystemRoot, ComSpec: process.env.ComSpec } : {}),
    npm_config_userconfig: userconfig,
    npm_config_cache: path.join(home, 'cache'),
    npm_config_registry: 'https://registry.npmjs.org/',
    npm_config_ignore_scripts: 'true',
    npm_config_update_notifier: 'false',
  };
  const result = spawnSync(process.execPath, [cli, ...args], { cwd, env, encoding: 'utf8' });
  if (result.status !== 0) fail(`npm ${args[0]} failed:\n${result.stderr}`);
  return result.stdout.trim();
}

function platformMatches(entry, platform) {
  const allowed = (list, value) => !list || list.includes(value);
  return allowed(entry.os, platform.os) && allowed(entry.cpu, platform.cpu) && allowed(entry.libc, platform.libc);
}

function verifyClosure(nodeModules, lock, platform) {
  const expected = new Map();
  for (const [key, entry] of Object.entries(lock.packages)) {
    if (key === '') continue;
    if (entry.dev || entry.peer || entry.link) fail(`unsupported lock entry: ${key}`);
    if (!entry.integrity?.startsWith('sha512-') || !entry.resolved?.startsWith('https://registry.npmjs.org/')) {
      fail(`lock entry without registry sha512 integrity: ${key}`);
    }
    const matches = platformMatches(entry, platform);
    if (!matches && !entry.optional) fail(`required package excluded by platform: ${key}`);
    if (matches) expected.set(key, entry);
  }
  const installed = new Map();
  const scan = (dir, prefix) => {
    for (const name of fs.readdirSync(dir).sort()) {
      if (name === '.package-lock.json' && prefix === 'node_modules') continue;
      if (name.startsWith('@')) {
        for (const scoped of fs.readdirSync(path.join(dir, name)).sort()) {
          visit(path.join(dir, name, scoped), `${prefix}/${name}/${scoped}`);
        }
      } else visit(path.join(dir, name), `${prefix}/${name}`);
    }
  };
  const visit = (dir, key) => {
    const manifest = JSON.parse(fs.readFileSync(path.join(dir, 'package.json'), 'utf8'));
    installed.set(key, manifest);
    if (fs.existsSync(path.join(dir, 'node_modules'))) scan(path.join(dir, 'node_modules'), `${key}/node_modules`);
  };
  scan(nodeModules, 'node_modules');
  for (const [key, manifest] of installed) {
    const entry = expected.get(key) ?? fail(`package not expected for target: ${key}`);
    if (manifest.version !== entry.version) fail(`installed version differs from lock: ${key}`);
    if (!platformMatches(manifest, platform)) fail(`installed package declares another platform: ${key}`);
  }
  for (const key of expected.keys()) if (!installed.has(key)) fail(`locked package missing: ${key}`);
  return installed.size;
}

async function assemble({ target, out, nodeArchivePath }) {
  const pin = pins.targets[target] ?? fail(`unsupported target: ${target}`);
  if (process.version !== `v${pins.version}`) fail(`assembly requires Node ${pins.version}`);
  if (fs.existsSync(out)) fail('output directory already exists');
  // Beside the output (same volume), so the installed closure moves without copying.
  const work = fs.mkdtempSync(`${out}.assembly-`);
  try {
    const npmVersion = npm(['--version'], work, work);
    if (npmVersion !== pins.npm) fail(`assembly requires npm ${pins.npm}`);

    const entryDir = path.join(packageDir, 'entry');
    const entryNames = fs.readdirSync(entryDir).sort();
    if (entryNames.join('\n') !== ENTRY_FILES.join('\n')) fail('entry directory differs from the packaged entry set');

    const lockBytes = fs.readFileSync(path.join(packageDir, 'package-lock.json'));
    const lock = JSON.parse(lockBytes);
    if (lock.lockfileVersion !== 3) fail('lockfileVersion 3 required');
    const install = path.join(work, 'install');
    fs.mkdirSync(install);
    fs.copyFileSync(path.join(packageDir, 'package.json'), path.join(install, 'package.json'));
    fs.writeFileSync(path.join(install, 'package-lock.json'), lockBytes);
    const platformArgs = Object.entries(pin.platform).map(([key, value]) => `--${key}=${value}`);
    npm(
      ['ci', '--ignore-scripts', '--omit=dev', '--no-bin-links', '--no-audit', '--no-fund', ...platformArgs],
      install,
      work,
    );
    if (!fs.readFileSync(path.join(install, 'package-lock.json')).equals(lockBytes)) fail('npm ci changed the lock');
    const packages = verifyClosure(path.join(install, 'node_modules'), lock, pin.platform);

    const archive = await nodeArchive(pin, nodeArchivePath);
    fs.mkdirSync(path.join(out, 'node'), { recursive: true });
    fs.writeFileSync(path.join(out, pin.packaged), archiveMember(archive, pin.archive, pin.executable), { mode: 0o755 });
    fs.writeFileSync(path.join(out, 'node', 'LICENSE'), archiveMember(archive, pin.archive, pin.license), { mode: 0o644 });
    fs.mkdirSync(path.join(out, 'entry'));
    for (const name of ENTRY_FILES) fs.copyFileSync(path.join(entryDir, name), path.join(out, 'entry', name));
    fs.renameSync(path.join(install, 'node_modules'), path.join(out, 'node_modules'));

    const files = listTree(out);
    if (files.length > MAX_FILES) fail('too many toolchain files');
    const folded = new Set();
    for (const file of files) {
      if (!validPath(file.relative)) fail(`path outside the manifest grammar: ${file.relative}`);
      if (file.size > MAX_FILE_BYTES) fail(`file too large: ${file.relative}`);
      const lower = file.relative.toLowerCase();
      if (folded.has(lower)) fail(`case-insensitive duplicate: ${file.relative}`);
      folded.add(lower);
    }
    const node = fs.readFileSync(path.join(out, pin.packaged));
    process.stdout.write(
      `${JSON.stringify({ target, node: pins.version, npm: npmVersion, packages, files: files.length, bytes: files.reduce((n, f) => n + f.size, 0), nodeSha256: sha256(node) })}\n`,
    );
  } catch (error) {
    fs.rmSync(out, { recursive: true, force: true });
    throw error;
  } finally {
    fs.rmSync(work, { recursive: true, force: true });
  }
}

// Post-bundle check: a bundled copy must equal the assembled tree byte for byte.
function compare(expected, actual) {
  const left = listTree(expected);
  const right = listTree(actual);
  if (left.length !== right.length) fail(`file count differs: ${left.length} != ${right.length}`);
  for (let i = 0; i < left.length; i += 1) {
    const a = left[i];
    const b = right[i];
    if (a.relative !== b.relative || a.size !== b.size) fail(`tree differs at ${a.relative}`);
    if (sha256(fs.readFileSync(path.join(expected, a.relative))) !== sha256(fs.readFileSync(path.join(actual, b.relative)))) {
      fail(`bytes differ: ${a.relative}`);
    }
  }
  process.stdout.write(`identical: ${left.length} files\n`);
}

function parse(argv) {
  if (argv[0] === '--compare' && argv.length === 3) return { compare: [path.resolve(argv[1]), path.resolve(argv[2])] };
  const options = {};
  for (let i = 0; i < argv.length; i += 2) {
    const [flag, value] = [argv[i], argv[i + 1]];
    if (value === undefined) fail(`missing value for ${flag}`);
    if (flag === '--target') options.target = value;
    else if (flag === '--out') options.out = path.resolve(value);
    else if (flag === '--node-archive') options.nodeArchivePath = path.resolve(value);
    else fail(`unknown argument: ${flag}`);
  }
  if (!options.target || !options.out) fail('--target and --out are required');
  return options;
}

try {
  const options = parse(process.argv.slice(2));
  if (options.compare) compare(...options.compare);
  else await assemble(options);
} catch (error) {
  process.stderr.write(`toolchain assembly failed: ${error.message}\n`);
  process.exitCode = 1;
}
