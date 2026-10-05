// XA-R4C: the npm advisory gate for the npm roots Nexus OS builds, tests and
// ships, against security/npm-advisory-policy.json. CI tooling only; it is
// never packaged and never run by the application.
//
//   node scripts/ci/npm-advisory-gate.mjs
//
// It takes no arguments. It fails (exit 1) when:
// - the policy is not exactly the strict schema below (no wildcard, no
//   severity-wide or package-wide entry, no unknown key);
// - a tracked npm root is neither audited nor excluded, or a policy root no
//   longer exists;
// - an audited root lacks its package.json or package-lock.json (v3), or
//   carries another package manager's lockfile;
// - an excluded root is referenced by a tracked file outside it other than
//   documentation and this gate (it became part of CI, build, release or
//   runtime);
// - `npm audit --package-lock-only --json` fails, times out, reports an npm,
//   registry or network error, prints anything but a consistent v2 report, or
//   changes package.json or the lockfile;
// - any advisory, at any severity, is reported that the one exact exception
//   (id, package, installed version, severity, affected range, root) does
//   not match;
// - npm's remediation signal (`fixAvailable`) for an excepted vulnerability
//   is not exactly the one reviewed for that root: a changed remediation
//   (a patched release, another upgrade, another version or major flag, or
//   none at all) needs security review and is never accepted silently;
// - an exception is past its review date, or is stale (a root it names no
//   longer reports it).
// The audit's exit status is checked against its report but never trusted
// alone. Output is a bounded summary; npm's own output is never echoed, and
// credentials are redacted from anything derived from it.
import { spawnSync } from 'node:child_process';
import crypto from 'node:crypto';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

export const POLICY_PATH = 'security/npm-advisory-policy.json';
export const SUCCESS = 'No known unexcepted npm advisories were reported for the audited roots at this run.';
export const REGISTRY = 'https://registry.npmjs.org/';
export const GATE_FILES = Object.freeze([
  POLICY_PATH,
  'scripts/ci/npm-advisory-gate.mjs',
  'scripts/ci/npm-advisory-gate.test.mjs',
]);

const SEVERITIES = ['info', 'low', 'moderate', 'high', 'critical'];
const EXCEPTION_SEVERITIES = ['low', 'moderate', 'high', 'critical'];
const MANIFESTS = new Set(['package.json', 'package-lock.json', 'npm-shrinkwrap.json', 'yarn.lock', 'pnpm-lock.yaml', 'bun.lock', 'bun.lockb']);
const OTHER_LOCKFILES = ['npm-shrinkwrap.json', 'yarn.lock', 'pnpm-lock.yaml', 'bun.lock', 'bun.lockb'];
const GHSA_ID = /^GHSA(?:-[23456789cfghjmpqrvwx]{4}){3}$/;
const ADVISORY_URL = /^https:\/\/github\.com\/advisories\/(GHSA(?:-[23456789cfghjmpqrvwx]{4}){3})$/;
const PACKAGE_NAME = /^(?:@[a-z0-9][a-z0-9._-]*\/)?[a-z0-9][a-z0-9._-]*$/;
const EXACT_VERSION = /^(?:0|[1-9]\d*)\.(?:0|[1-9]\d*)\.(?:0|[1-9]\d*)(?:-[0-9A-Za-z.-]+)?$/;
const ROOT_PATH = /^[A-Za-z0-9._-]+(?:\/[A-Za-z0-9._-]+)*$/;
const ISO_DATE = /^\d{4}-\d{2}-\d{2}$/;
const NPM_ERROR_CODE = /^[A-Z][A-Z0-9_]{0,39}$/;
const MAX_LINES = 200;
const MAX_LINE = 240;
const MAX_ADVISORY_LINES = 40;
const AUDIT_TIMEOUT_MS = 300_000;

const POLICY_KEYS = ['auditedRoots', 'exceptions', 'excludedRoots', 'schemaVersion'];
const EXCLUDED_KEYS = ['path', 'rationale'];
const EXCEPTION_KEYS = ['id', 'npmFixAvailable', 'package', 'range', 'reason', 'reviewBy', 'roots', 'severity', 'threatModel', 'version'];
const FIX_RECORD_KEYS = ['root', 'value'];
const FIX_VALUE_KEYS = ['isSemVerMajor', 'name', 'version'];

const isObject = (value) => value !== null && typeof value === 'object' && !Array.isArray(value);
const exactKeys = (value, keys) => isObject(value) && Object.keys(value).sort().join(',') === keys.join(',');
const longText = (value) => typeof value === 'string' && value.trim().length >= 20 && value.length <= 4000;
const rootPath = (value) =>
  typeof value === 'string' && value.length <= 200 && ROOT_PATH.test(value) && value.split('/').every((c) => c !== '.' && c !== '..');
const rank = (severity) => SEVERITIES.indexOf(severity);

function calendarDate(value) {
  if (typeof value !== 'string' || !ISO_DATE.test(value)) return false;
  const [y, m, d] = value.split('-').map(Number);
  const date = new Date(Date.UTC(y, m - 1, d));
  return date.getUTCFullYear() === y && date.getUTCMonth() === m - 1 && date.getUTCDate() === d;
}

// npm's remediation signal for a vulnerability (`fixAvailable`): true, false,
// or the upgrade npm proposes, {name, version, isSemVerMajor}.
function remediationValue(value) {
  return (
    typeof value === 'boolean' ||
    (exactKeys(value, FIX_VALUE_KEYS) &&
      typeof value.name === 'string' &&
      PACKAGE_NAME.test(value.name) &&
      typeof value.version === 'string' &&
      EXACT_VERSION.test(value.version) &&
      typeof value.isSemVerMajor === 'boolean')
  );
}

// Exactly the reviewed signal: the same boolean, or an object with exactly
// the reviewed name, version and major flag and no other key.
function sameRemediation(actual, reviewed) {
  if (typeof reviewed === 'boolean') return actual === reviewed;
  return (
    exactKeys(actual, FIX_VALUE_KEYS) &&
    actual.name === reviewed.name &&
    actual.version === reviewed.version &&
    actual.isSemVerMajor === reviewed.isSemVerMajor
  );
}

function remediation(value) {
  if (value === undefined) return 'absent';
  if (typeof value === 'boolean') return String(value);
  if (exactKeys(value, FIX_VALUE_KEYS) && typeof value.name === 'string' && typeof value.version === 'string' && typeof value.isSemVerMajor === 'boolean') {
    return `${value.name.slice(0, 80)}@${value.version.slice(0, 40)} (${value.isSemVerMajor ? 'major' : 'not major'})`;
  }
  if (isObject(value)) return `an object with keys ${Object.keys(value).sort().join(',').slice(0, 60)}`;
  return value === null ? 'null' : `a ${Array.isArray(value) ? 'list' : typeof value}`;
}

// Anything derived from npm output is redacted before it is printed.
export function redact(text) {
  return String(text)
    .replace(/(\/\/)[^\s/@]+@/g, '$1<redacted>@')
    .replace(/((?:_authToken|_auth|_password|password|passwd|token|secret|authorization)["']?\s*[=:]\s*["']?)[^\s"',;]+/gi, '$1<redacted>')
    .replace(/\b(Bearer|Basic)\s+[A-Za-z0-9._~+/=-]+/gi, '$1 <redacted>')
    .replace(/\bnpm_[A-Za-z0-9]{16,}/g, '<redacted>')
    .replace(/\bgh[pousr]_[A-Za-z0-9]{16,}/g, '<redacted>')
    .replace(/\bgithub_pat_[A-Za-z0-9_]{16,}/g, '<redacted>');
}

function bounded(lines) {
  const out = lines.slice(0, MAX_LINES).map((line) => {
    const clean = redact(line).replace(/[\u0000-\u0008\u000b-\u001f\u007f]/g, '?');
    return clean.length > MAX_LINE ? `${clean.slice(0, MAX_LINE - 3)}...` : clean;
  });
  if (lines.length > MAX_LINES) out.push(`... ${lines.length - MAX_LINES} more lines omitted`);
  return out;
}

// The strict policy schema. Returns every violation.
export function validatePolicy(policy) {
  const errors = [];
  if (!exactKeys(policy, POLICY_KEYS)) {
    return [`the policy must have exactly the keys ${POLICY_KEYS.join(', ')}`];
  }
  if (policy.schemaVersion !== 1) errors.push('schemaVersion must be 1');
  const audited = policy.auditedRoots;
  if (!Array.isArray(audited) || audited.length === 0 || !audited.every(rootPath) || new Set(audited).size !== audited.length) {
    errors.push('auditedRoots must be a non-empty list of distinct relative root paths');
  }
  const auditedSet = new Set(Array.isArray(audited) ? audited : []);
  const excluded = policy.excludedRoots;
  if (!Array.isArray(excluded)) {
    errors.push('excludedRoots must be a list');
  } else {
    const seen = new Set();
    for (const [i, entry] of excluded.entries()) {
      if (!exactKeys(entry, EXCLUDED_KEYS) || !rootPath(entry.path) || !longText(entry.rationale)) {
        errors.push(`excludedRoots[${i}] must be exactly {path, rationale} with a relative root path and a rationale`);
        continue;
      }
      if (seen.has(entry.path) || auditedSet.has(entry.path)) errors.push(`excluded root ${entry.path} is listed twice`);
      seen.add(entry.path);
    }
  }
  const exceptions = policy.exceptions;
  if (!Array.isArray(exceptions)) {
    errors.push('exceptions must be a list');
  } else {
    const seen = new Set();
    for (const [i, entry] of exceptions.entries()) {
      const where = `exceptions[${i}]`;
      if (!exactKeys(entry, EXCEPTION_KEYS)) {
        errors.push(`${where} must have exactly the keys ${EXCEPTION_KEYS.join(', ')}`);
        continue;
      }
      if (typeof entry.id !== 'string' || !GHSA_ID.test(entry.id)) errors.push(`${where}.id must be one exact GHSA identifier`);
      if (typeof entry.package !== 'string' || !PACKAGE_NAME.test(entry.package)) errors.push(`${where}.package must be one exact npm package name`);
      if (typeof entry.version !== 'string' || !EXACT_VERSION.test(entry.version)) errors.push(`${where}.version must be one exact version`);
      if (!EXCEPTION_SEVERITIES.includes(entry.severity)) errors.push(`${where}.severity must be low, moderate, high or critical`);
      if (typeof entry.range !== 'string' || entry.range.trim() === '' || entry.range.includes('*') || entry.range.length > 100) {
        errors.push(`${where}.range must be the advisory's exact affected range`);
      }
      if (!Array.isArray(entry.roots) || entry.roots.length === 0 || new Set(entry.roots).size !== entry.roots.length || !entry.roots.every((r) => auditedSet.has(r))) {
        errors.push(`${where}.roots must be distinct audited roots`);
      }
      const records = entry.npmFixAvailable;
      if (!Array.isArray(records) || records.length === 0 || !records.every((r) => exactKeys(r, FIX_RECORD_KEYS) && typeof r.root === 'string' && remediationValue(r.value))) {
        errors.push(`${where}.npmFixAvailable must be a list of exact {root, value} records, each value true, false or {name, version, isSemVerMajor}`);
      } else {
        const named = records.map((r) => r.root);
        if (new Set(named).size !== named.length) errors.push(`${where}.npmFixAvailable names a root twice`);
        if (Array.isArray(entry.roots) && (named.length !== entry.roots.length || !entry.roots.every((r) => named.includes(r)) || !named.every((r) => entry.roots.includes(r)))) {
          errors.push(`${where}.npmFixAvailable must hold exactly one reviewed remediation signal for each of the exception's roots`);
        }
      }
      if (!longText(entry.reason)) errors.push(`${where}.reason is required`);
      if (!longText(entry.threatModel)) errors.push(`${where}.threatModel is required`);
      if (!calendarDate(entry.reviewBy)) errors.push(`${where}.reviewBy must be a YYYY-MM-DD date`);
      const key = `${entry.id} ${entry.package}`;
      if (seen.has(key)) errors.push(`${where} repeats ${key}`);
      seen.add(key);
    }
  }
  return errors;
}

// Every directory holding a tracked npm manifest or lockfile, with the
// manifests it holds ('.' is the repository root).
export function discoverRoots(trackedFiles) {
  const roots = new Map();
  for (const file of trackedFiles) {
    const base = file.split('/').pop();
    if (!MANIFESTS.has(base)) continue;
    const dir = file.includes('/') ? file.slice(0, file.lastIndexOf('/')) : '.';
    if (!roots.has(dir)) roots.set(dir, new Set());
    roots.get(dir).add(base);
  }
  return roots;
}

// For each excluded root ({ root, packageName }), the tracked files outside
// it that name it: its path, or its package name in quotes. Documentation
// (docs/**, *.md) and the gate's own files may. One pass over the files.
export function referencesTo(roots, trackedFiles, readText) {
  const searches = roots.map(({ root, packageName }) => ({
    root,
    needles: packageName ? [root, `"${packageName}"`, `'${packageName}'`] : [root],
  }));
  const hits = new Map(roots.map(({ root }) => [root, []]));
  for (const file of trackedFiles) {
    if (GATE_FILES.includes(file) || file.startsWith('docs/') || file.toLowerCase().endsWith('.md')) continue;
    const outside = searches.filter(({ root }) => file !== root && !file.startsWith(`${root}/`));
    if (outside.length === 0) continue;
    const text = readText(file);
    if (text === null) continue;
    for (const { root, needles } of outside) {
      if (needles.some((needle) => text.includes(needle))) hits.get(root).push(file);
    }
  }
  return hits;
}

function parseJson(text) {
  try {
    return { value: JSON.parse(text) };
  } catch {
    return { error: true };
  }
}

// One audited root's audit result against its lockfile and the policy.
function checkAudit(root, result, lock, policy, today, used, fail, note) {
  if (result.error) return fail(`${root}: the audit command failed (${NPM_ERROR_CODE.test(result.error) ? result.error : 'spawn error'})`);
  if (result.signal) return fail(`${root}: the audit command was terminated (${result.signal})`);
  if (result.mutated) return fail(`${root}: the audit changed package.json or package-lock.json`);
  if (result.status !== 0 && result.status !== 1) return fail(`${root}: the audit command exited ${result.status}`);
  const parsed = parseJson(result.stdout);
  if (parsed.error || !isObject(parsed.value)) {
    return fail(`${root}: the audit printed invalid JSON (${Buffer.byteLength(String(result.stdout))} bytes)`);
  }
  const report = parsed.value;
  if ('error' in report) {
    const code = isObject(report.error) && NPM_ERROR_CODE.test(String(report.error.code)) ? report.error.code : 'unknown';
    const summary = isObject(report.error) && typeof report.error.summary === 'string' ? report.error.summary.slice(0, 160) : '';
    return fail(`${root}: npm audit reported an error (${code}): ${summary}`);
  }
  if (report.auditReportVersion !== 2 || !isObject(report.vulnerabilities) || !isObject(report.metadata)) {
    return fail(`${root}: the audit report is not an npm audit v2 report`);
  }
  const counts = report.metadata.vulnerabilities;
  const deps = report.metadata.dependencies;
  if (!isObject(counts) || ![...SEVERITIES, 'total'].every((k) => Number.isInteger(counts[k]) && counts[k] >= 0) || !isObject(deps) || !Number.isInteger(deps.total)) {
    return fail(`${root}: the audit report metadata is malformed`);
  }
  const entries = Object.entries(report.vulnerabilities);
  const lockEntries = Object.keys(lock.packages).filter((key) => key !== '').length;
  if (deps.total !== lockEntries) {
    return fail(`${root}: the audit covered ${deps.total} packages but the lockfile has ${lockEntries}`);
  }
  if (counts.total !== entries.length || SEVERITIES.some((s) => counts[s] !== entries.filter(([, v]) => isObject(v) && v.severity === s).length)) {
    return fail(`${root}: the audit report counts disagree with its entries`);
  }
  if ((result.status === 0) !== (entries.length === 0)) {
    return fail(`${root}: the audit exit status ${result.status} disagrees with its report (${entries.length} vulnerable packages)`);
  }
  note(`${root}: ${lockEntries} locked packages audited; ${entries.length} vulnerable package entries`);
  let shown = 0;
  const show = (line) => {
    if (shown < MAX_ADVISORY_LINES) fail(line);
    else if (shown === MAX_ADVISORY_LINES) fail(`${root}: further advisories omitted`);
    shown += 1;
  };
  for (const [name, vuln] of entries) {
    if (!isObject(vuln) || vuln.name !== name || !SEVERITIES.includes(vuln.severity) || !Array.isArray(vuln.via) || vuln.via.length === 0 || !Array.isArray(vuln.nodes) || vuln.nodes.length === 0) {
      show(`${root}: malformed audit entry for ${name}`);
      continue;
    }
    const versions = vuln.nodes.map((node) => (typeof node === 'string' && isObject(lock.packages[node]) ? lock.packages[node].version : null));
    if (versions.some((v) => typeof v !== 'string')) {
      show(`${root}: ${name} is reported at a location the lockfile does not have`);
      continue;
    }
    let worst = -1;
    for (const via of vuln.via) {
      if (typeof via === 'string') {
        if (!isObject(report.vulnerabilities[via])) show(`${root}: ${name} is reported through ${via}, which the report does not list`);
        else worst = Math.max(worst, rank(report.vulnerabilities[via].severity));
        continue;
      }
      if (!isObject(via) || via.name !== name || !SEVERITIES.includes(via.severity) || typeof via.range !== 'string') {
        show(`${root}: malformed advisory on ${name}`);
        continue;
      }
      worst = Math.max(worst, rank(via.severity));
      const match = typeof via.url === 'string' ? ADVISORY_URL.exec(via.url) : null;
      const id = match ? match[1] : 'an advisory without a GHSA identifier';
      const label = `${id} on ${name}@${[...new Set(versions)].join(',')} (${via.severity}, affected ${via.range})`;
      const exception = match ? policy.exceptions.find((e) => e.id === id && e.package === name) : undefined;
      if (!exception) {
        show(`${root}: unexcepted advisory ${label}`);
        continue;
      }
      const problems = [];
      if (!exception.roots.includes(root)) problems.push(`the exception does not cover ${root}`);
      if (versions.some((v) => v !== exception.version)) problems.push(`installed ${[...new Set(versions)].join(',')}, excepted ${exception.version}`);
      if (via.severity !== exception.severity) problems.push(`severity is now ${via.severity}, excepted ${exception.severity}`);
      if (via.range !== exception.range) problems.push(`affected range is now ${via.range}, excepted ${exception.range}`);
      if (today > exception.reviewBy) problems.push(`the exception expired after ${exception.reviewBy}`);
      // npm's remediation advice for the excepted vulnerability must be
      // exactly the one reviewed for this root; any change needs review.
      const reviewed = exception.npmFixAvailable.find((record) => record.root === root);
      const drifted = reviewed === undefined ? problems.length === 0 : !sameRemediation(vuln.fixAvailable, reviewed.value);
      if (problems.length > 0) show(`${root}: exception ${exception.id} does not match ${label}: ${problems.join('; ')}`);
      if (drifted) {
        show(
          `${root}: npm's remediation signal for excepted ${exception.id} (${name}) changed and requires security review: ` +
            `fixAvailable is now ${remediation(vuln.fixAvailable)}, reviewed ${reviewed ? remediation(reviewed.value) : 'nothing'}`,
        );
      }
      if (problems.length > 0 || drifted) continue;
      used.add(`${exception.id} ${exception.package} ${root}`);
      note(`${root}: excepted ${label}; review by ${exception.reviewBy}`);
      note(`${root}: npm remediation signal for ${exception.id} matches the reviewed value: fixAvailable ${JSON.stringify(reviewed.value)}`);
    }
    if (worst !== rank(vuln.severity)) show(`${root}: ${name} is reported as ${vuln.severity}, inconsistent with its advisories`);
  }
}

// The whole gate over injected inputs: { policy, trackedFiles, readText,
// audit(root) -> result, today: 'YYYY-MM-DD' }. Returns { ok, lines }.
export function evaluate({ policy, trackedFiles, readText, audit, today, npmVersion = 'unknown' }) {
  const failures = [];
  const notes = [`npm ${npmVersion}; registry ${REGISTRY}; full dependency trees (prod, dev, optional, peer); date ${today} (UTC)`];
  const fail = (line) => failures.push(line);
  const note = (line) => notes.push(line);
  const finish = () => {
    const lines = [...notes, ...failures.map((line) => `FAIL ${line}`)];
    if (failures.length === 0) lines.push(SUCCESS);
    else lines.push(`npm advisory gate: FAIL (${failures.length} finding${failures.length === 1 ? '' : 's'})`);
    return { ok: failures.length === 0, lines: bounded(lines) };
  };
  const schema = validatePolicy(policy);
  if (schema.length > 0) {
    schema.forEach((error) => fail(`${POLICY_PATH}: ${error}`));
    return finish();
  }
  for (const exception of policy.exceptions) {
    if (today > exception.reviewBy) fail(`exception ${exception.id} (${exception.package}) expired after ${exception.reviewBy}`);
  }

  const discovered = discoverRoots(trackedFiles);
  const excludedPaths = policy.excludedRoots.map((entry) => entry.path);
  for (const root of [...discovered.keys()].sort()) {
    if (!policy.auditedRoots.includes(root) && !excludedPaths.includes(root)) {
      fail(`unknown npm root ${root} (${[...discovered.get(root)].sort().join(', ')}): audit it or exclude it in ${POLICY_PATH}`);
    }
  }
  for (const root of [...policy.auditedRoots, ...excludedPaths]) {
    if (!discovered.has(root)) fail(`${POLICY_PATH} names ${root}, which holds no tracked npm manifest`);
  }

  const dormant = policy.excludedRoots
    .filter(({ path: root }) => discovered.has(root))
    .map(({ path: root }) => {
      let packageName = null;
      if (discovered.get(root).has('package.json')) {
        const manifest = parseJson(readText(`${root}/package.json`) ?? '');
        packageName = !manifest.error && isObject(manifest.value) && typeof manifest.value.name === 'string' ? manifest.value.name : null;
      }
      return { root, packageName };
    });
  const references = referencesTo(dormant, trackedFiles, readText);
  for (const { root } of dormant) {
    const hits = references.get(root);
    if (hits.length > 0) {
      fail(`excluded root ${root} is referenced outside it, so it is no longer dormant: ${hits.slice(0, 5).join(', ')}${hits.length > 5 ? ', ...' : ''}`);
    } else {
      note(`${root}: excluded (dormant; named only by documentation)`);
    }
  }

  const used = new Set();
  const failedRoots = new Set();
  for (const root of policy.auditedRoots) {
    const manifests = discovered.get(root);
    if (!manifests) {
      failedRoots.add(root);
      continue;
    }
    const before = failures.length;
    if (!manifests.has('package.json') || !manifests.has('package-lock.json')) {
      fail(`${root}: an audited root needs a tracked package.json and package-lock.json`);
    }
    const others = OTHER_LOCKFILES.filter((name) => manifests.has(name));
    if (others.length > 0) fail(`${root}: an audited root may not carry ${others.join(', ')}`);
    const lock = failures.length === before ? parseJson(readText(`${root}/package-lock.json`) ?? '') : { error: true };
    if (failures.length === before && (lock.error || !isObject(lock.value) || lock.value.lockfileVersion !== 3 || !isObject(lock.value.packages) || !isObject(lock.value.packages['']))) {
      fail(`${root}: package-lock.json is not a lockfileVersion 3 npm lockfile`);
    }
    if (failures.length === before) checkAudit(root, audit(root), lock.value, policy, today, used, fail, note);
    if (failures.length !== before) failedRoots.add(root);
  }
  for (const exception of policy.exceptions) {
    for (const root of exception.roots) {
      if (!failedRoots.has(root) && !used.has(`${exception.id} ${exception.package} ${root}`)) {
        fail(`stale exception ${exception.id} (${exception.package}): ${root} no longer reports it; remove it from ${POLICY_PATH}`);
      }
    }
  }
  return finish();
}

// ---- the command --------------------------------------------------------

function digest(dir) {
  const hash = crypto.createHash('sha256');
  for (const name of ['package.json', 'package-lock.json']) {
    const file = path.join(dir, name);
    hash.update(`${name}\0`);
    hash.update(fs.existsSync(file) ? fs.readFileSync(file) : Buffer.from('<absent>'));
  }
  return hash.digest('hex');
}

// npm with an empty user and global configuration, no inherited npm_config_*
// variable, the public registry, every dependency type and the lowest audit
// level, so the report and the exit status do not depend on local settings.
function npmRunner(workDir) {
  // Two files: npm refuses to load one file as both configurations.
  const userConfig = path.join(workDir, 'user-npmrc');
  const globalConfig = path.join(workDir, 'global-npmrc');
  fs.writeFileSync(userConfig, '');
  fs.writeFileSync(globalConfig, '');
  const env = Object.fromEntries(Object.entries(process.env).filter(([key]) => !/^npm_config_/i.test(key)));
  const config = [
    `--userconfig=${userConfig}`,
    `--globalconfig=${globalConfig}`,
    `--cache=${path.join(workDir, 'cache')}`,
    `--registry=${REGISTRY}`,
    '--no-update-notifier',
    '--no-fund',
  ];
  const npm = (args, cwd, timeout) =>
    spawnSync('npm', [...args, ...config], { cwd, env, encoding: 'utf8', maxBuffer: 256 * 1024 * 1024, timeout, windowsHide: true });
  return {
    version(cwd) {
      const r = npm(['--version'], cwd, 60_000);
      return r.status === 0 && /^\d+\.\d+\.\d+/.test(r.stdout.trim()) ? r.stdout.trim() : null;
    },
    audit(dir) {
      const before = digest(dir);
      const r = npm(
        ['audit', '--package-lock-only', '--json', '--audit-level=info', '--include=prod', '--include=dev', '--include=optional', '--include=peer'],
        dir,
        AUDIT_TIMEOUT_MS,
      );
      return {
        status: r.status,
        signal: r.signal,
        error: r.error ? String(r.error.code ?? 'spawn error') : null,
        stdout: r.stdout ?? '',
        mutated: digest(dir) !== before,
      };
    },
  };
}

function trackedFiles(repoRoot) {
  const r = spawnSync('git', ['-C', repoRoot, 'ls-files', '-z'], { encoding: 'utf8', maxBuffer: 256 * 1024 * 1024 });
  if (r.status !== 0) return null;
  return r.stdout.split('\0').filter(Boolean);
}

// Text of a tracked file, or null for a binary file. An unreadable tracked
// file throws, which fails the gate.
function readTextIn(repoRoot) {
  return (file) => {
    const bytes = fs.readFileSync(path.join(repoRoot, file));
    return bytes.includes(0) ? null : bytes.toString('utf8');
  };
}

export function main(argv) {
  if (argv.length > 0) {
    console.log('npm advisory gate: takes no arguments');
    return 2;
  }
  const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', '..');
  const workDir = fs.mkdtempSync(path.join(os.tmpdir(), 'npm-advisory-gate-'));
  try {
    const files = trackedFiles(repoRoot);
    const policy = parseJson(fs.existsSync(path.join(repoRoot, POLICY_PATH)) ? fs.readFileSync(path.join(repoRoot, POLICY_PATH), 'utf8') : '');
    let result;
    if (files === null) {
      result = { ok: false, lines: ['FAIL git ls-files failed', 'npm advisory gate: FAIL (1 finding)'] };
    } else if (policy.error) {
      result = { ok: false, lines: [`FAIL ${POLICY_PATH} is missing or is not JSON`, 'npm advisory gate: FAIL (1 finding)'] };
    } else {
      const npm = npmRunner(workDir);
      const npmVersion = npm.version(repoRoot);
      if (npmVersion === null) {
        result = { ok: false, lines: ['FAIL npm is not available (npm --version failed)', 'npm advisory gate: FAIL (1 finding)'] };
      } else {
        result = evaluate({
          policy: policy.value,
          trackedFiles: files,
          readText: readTextIn(repoRoot),
          audit: (root) => npm.audit(path.join(repoRoot, root)),
          today: new Date().toISOString().slice(0, 10),
          npmVersion,
        });
      }
    }
    for (const line of result.lines) console.log(line);
    return result.ok ? 0 : 1;
  } catch (error) {
    console.log(`FAIL the gate could not complete (${error?.code ?? 'error'})`);
    console.log('npm advisory gate: FAIL (1 finding)');
    return 1;
  } finally {
    fs.rmSync(workDir, { recursive: true, force: true });
  }
}

if (process.argv[1] && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href) {
  process.exitCode = main(process.argv.slice(2));
}
