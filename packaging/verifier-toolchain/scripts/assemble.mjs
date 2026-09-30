// Phase Two P2F: release/CI assembly of the packaged verifier toolchain.
// Build tooling only; it is never packaged and never run by the application.
//
//   node assemble.mjs --out <new dir> [--archives <dir>]
//   node assemble.mjs --compare <expected dir> <actual dir>
//
// Produces exactly the pinned members of the pinned official Rust release
// components (../rust-release.json): the host rustc (with its driver, LLVM
// and rust-lld), cargo, the x86_64-unknown-linux-musl standard library with
// its self-contained CRT objects, and the upstream license files. Every
// archive, fetched or supplied, must match its pinned SHA-256, and the pins
// must agree with the pinned channel manifest. It writes no manifest: the
// verifier sandbox build script (NEXUS_VERIFIER_TOOLCHAIN=packaged) renders
// the embedded manifest from the produced bytes, and the application
// re-verifies the installed tree against it before every launch.
import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { pipeline } from 'node:stream/promises';
import { Readable } from 'node:stream';
import zlib from 'node:zlib';
import { fileURLToPath } from 'node:url';

const packageDir = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const pins = JSON.parse(fs.readFileSync(path.join(packageDir, 'rust-release.json'), 'utf8'));
// The manifest grammar (crates/nexus-verifier-sandbox/src/toolchain/contract.rs is authoritative).
const COMPONENT = /^[A-Za-z0-9._+-]+$/;
const MAX_PATH_BYTES = 200;

class AssemblyError extends Error {}
const fail = (message) => {
  throw new AssemblyError(message);
};

function validPath(relative) {
  return (
    relative.length > 0 &&
    Buffer.byteLength(relative) <= MAX_PATH_BYTES &&
    relative.split('/').every((c) => COMPONENT.test(c) && c.length <= 255 && c !== '.' && c !== '..')
  );
}

async function sha256File(file) {
  const hash = crypto.createHash('sha256');
  await pipeline(fs.createReadStream(file), hash);
  return hash.digest('hex');
}

// A pinned file: from --archives if given, otherwise fetched from the pinned
// source. Either way it must match its pinned digest.
async function obtain(name, digest, archives, work) {
  let file;
  if (archives) {
    file = path.join(archives, name);
  } else {
    file = path.join(work, name);
    const response = await fetch(new URL(name, pins.source));
    if (!response.ok) fail(`download failed: ${name} (${response.status})`);
    await pipeline(Readable.fromWeb(response.body), fs.createWriteStream(file, { flags: 'wx' }));
  }
  if ((await sha256File(file)) !== digest) fail(`digest mismatch: ${name}`);
  return file;
}

// The component pins must be exactly the pinned channel manifest's.
function checkChannel(channelText) {
  for (const component of pins.components) {
    const url = `${pins.source}${component.archive}`;
    const entry = channelText.indexOf(`url = "${url}"\nhash = "${component.sha256}"\n`);
    if (entry < 0) fail(`pin not in the channel manifest: ${component.archive}`);
  }
}

// --- Tar extraction: exactly the pinned members; nothing else is written. ---

function readBlock(fd, offset, length) {
  const buffer = Buffer.alloc(length);
  let done = 0;
  while (done < length) {
    const read = fs.readSync(fd, buffer, done, length - done, offset + done);
    if (read === 0) fail('truncated archive');
    done += read;
  }
  return buffer;
}

function paxPath(data) {
  let found = null;
  for (let at = 0; at < data.length; ) {
    const space = data.indexOf(0x20, at);
    if (space < 0) fail('malformed pax header');
    const length = Number.parseInt(data.subarray(at, space).toString('ascii'), 10);
    if (!Number.isInteger(length) || length <= 0) fail('malformed pax header');
    const record = data.subarray(space + 1, at + length - 1).toString('utf8');
    const equals = record.indexOf('=');
    if (record.slice(0, equals) === 'path') found = record.slice(equals + 1);
    at += length;
  }
  return found;
}

function* tarEntries(fd, size) {
  let longName = null;
  let paxName = null;
  for (let offset = 0; offset + 512 <= size; ) {
    const header = readBlock(fd, offset, 512);
    if (header.every((b) => b === 0)) return;
    const field = (start, length) => header.subarray(start, start + length).toString('utf8').replace(/\0.*$/s, '');
    const entrySize = Number.parseInt(field(124, 12).trim() || '0', 8);
    const type = field(156, 1) || '0';
    const data = offset + 512;
    offset = data + Math.ceil(entrySize / 512) * 512;
    if (type === 'L') {
      longName = readBlock(fd, data, entrySize).toString('utf8').replace(/\0.*$/s, '');
      continue;
    }
    if (type === 'x') {
      paxName = paxPath(readBlock(fd, data, entrySize));
      continue;
    }
    if (type === 'g') continue;
    const prefix = field(345, 155);
    const name = paxName ?? longName ?? (prefix ? `${prefix}/${field(0, 100)}` : field(0, 100));
    longName = null;
    paxName = null;
    const mode = Number.parseInt(field(100, 8).trim() || '0', 8);
    yield { name: name.replace(/\/$/, ''), type, mode, data, size: entrySize };
  }
}

function copyOut(fd, entry, destination) {
  fs.mkdirSync(path.dirname(destination), { recursive: true, mode: 0o755 });
  const out = fs.openSync(destination, 'wx', 0o644);
  try {
    for (let done = 0; done < entry.size; ) {
      const chunk = Math.min(1 << 20, entry.size - done);
      fs.writeSync(out, readBlock(fd, entry.data + done, chunk));
      done += chunk;
    }
  } finally {
    fs.closeSync(out);
  }
  // Umask-independent: executables 0755, everything else 0644.
  fs.chmodSync(destination, entry.mode & 0o111 ? 0o755 : 0o644);
}

async function extract(archive, component, out, work) {
  const tar = path.join(work, `${path.basename(archive)}.tar`);
  await pipeline(fs.createReadStream(archive), zlib.createGunzip(), fs.createWriteStream(tar, { flags: 'wx' }));
  const wanted = new Map(Object.entries(component.files));
  const trees = Object.entries(component.trees);
  const found = new Set();
  const fd = fs.openSync(tar, 'r');
  try {
    for (const entry of tarEntries(fd, fs.fstatSync(fd).size)) {
      let destination = wanted.get(entry.name) ?? null;
      if (destination === null) {
        const tree = trees.find(([prefix]) => `${entry.name}/`.startsWith(prefix) && `${entry.name}/` !== prefix);
        if (!tree) continue;
        destination = tree[1] + entry.name.slice(tree[0].length);
      }
      if (!validPath(destination)) fail(`path outside the manifest grammar: ${destination}`);
      if (entry.type === '5') continue;
      if (entry.type !== '0') fail(`archive member is not a regular file: ${entry.name}`);
      if (found.has(destination)) fail(`archive member repeated: ${entry.name}`);
      found.add(destination);
      copyOut(fd, entry, path.join(out, ...destination.split('/')));
    }
  } finally {
    fs.closeSync(fd);
    fs.rmSync(tar);
  }
  for (const [member, destination] of wanted) {
    if (!found.has(destination)) fail(`archive member missing: ${member}`);
  }
}

// Every regular file under `root` (sorted), rejecting links, special files and
// empty directories; directories are implied by files.
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
      else if (stat.isFile()) files.push({ relative, size: stat.size, executable: (stat.mode & 0o100) !== 0 });
      else fail(`unsupported entry in toolchain tree: ${relative}`);
    }
  };
  walk(root, '');
  return files;
}

async function assemble({ out, archives }) {
  if (fs.existsSync(out)) fail('output directory already exists');
  const work = fs.mkdtempSync(`${out}.assembly-`);
  const staged = path.join(work, 'tree');
  fs.mkdirSync(staged, { mode: 0o755 });
  try {
    const channel = await obtain(pins.channel.file, pins.channel.sha256, archives, work);
    checkChannel(fs.readFileSync(channel, 'utf8'));
    for (const component of pins.components) {
      const archive = await obtain(component.archive, component.sha256, archives, work);
      await extract(archive, component, staged, work);
    }
    const files = listTree(staged);
    if (files.length !== pins.files) fail(`expected ${pins.files} files, assembled ${files.length}`);
    for (const file of files) if (!validPath(file.relative)) fail(`path outside the manifest grammar: ${file.relative}`);
    for (const dir of [staged, ...listDirs(staged)]) fs.chmodSync(dir, 0o755);
    fs.renameSync(staged, out);
    const digest = crypto.createHash('sha256');
    for (const file of files) {
      digest.update(`${file.relative}\0${file.size}\0${file.executable ? 1 : 0}\0`);
      digest.update(await sha256File(path.join(out, ...file.relative.split('/'))));
      digest.update('\n');
    }
    process.stdout.write(
      `${JSON.stringify({ rust: pins.version, host: pins.host, target: pins.target, files: files.length, bytes: files.reduce((n, f) => n + f.size, 0), listing: digest.digest('hex') })}\n`,
    );
  } catch (error) {
    fs.rmSync(out, { recursive: true, force: true });
    throw error;
  } finally {
    fs.rmSync(work, { recursive: true, force: true });
  }
}

function listDirs(root) {
  const dirs = [];
  for (const name of fs.readdirSync(root)) {
    const full = path.join(root, name);
    if (fs.lstatSync(full).isDirectory()) dirs.push(full, ...listDirs(full));
  }
  return dirs;
}

// Post-bundle check: a bundled copy must equal the assembled tree byte for byte.
async function compare(expected, actual) {
  const left = listTree(expected);
  const right = listTree(actual);
  if (left.length !== right.length) fail(`file count differs: ${left.length} != ${right.length}`);
  for (let i = 0; i < left.length; i += 1) {
    const a = left[i];
    const b = right[i];
    if (a.relative !== b.relative || a.size !== b.size || a.executable !== b.executable) fail(`tree differs at ${a.relative}`);
    const [x, y] = await Promise.all([
      sha256File(path.join(expected, ...a.relative.split('/'))),
      sha256File(path.join(actual, ...b.relative.split('/'))),
    ]);
    if (x !== y) fail(`bytes differ: ${a.relative}`);
  }
  process.stdout.write(`identical: ${left.length} files\n`);
}

function parse(argv) {
  if (argv[0] === '--compare' && argv.length === 3) return { compare: [path.resolve(argv[1]), path.resolve(argv[2])] };
  const options = {};
  for (let i = 0; i < argv.length; i += 2) {
    const [flag, value] = [argv[i], argv[i + 1]];
    if (value === undefined) fail(`missing value for ${flag}`);
    if (flag === '--out') options.out = path.resolve(value);
    else if (flag === '--archives') options.archives = path.resolve(value);
    else fail(`unknown argument: ${flag}`);
  }
  if (!options.out) fail('--out is required');
  return options;
}

try {
  const options = parse(process.argv.slice(2));
  if (options.compare) await compare(...options.compare);
  else await assemble(options);
} catch (error) {
  process.stderr.write(`verifier toolchain assembly failed: ${error.message}\n`);
  process.exitCode = 1;
}
