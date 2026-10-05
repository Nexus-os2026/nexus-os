// XA-R4C: tests of the npm advisory gate, without npm or the network: the
// gate's evaluation runs over fixture policies, tracked-file lists, lockfiles
// and audit results shaped like npm 10's `npm audit --json` (report v2).
//
//   node --test scripts/ci/npm-advisory-gate.test.mjs
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { test } from 'node:test';
import { fileURLToPath } from 'node:url';
import { evaluate, redact, SUCCESS, validatePolicy } from './npm-advisory-gate.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));
const TODAY = '2026-10-05';
const BRACES = 'GHSA-vfj7-8cjw-p6xm';
const REASON = 'A reviewed reason long enough for the schema.';

function policy(edit = (p) => p) {
  return edit({
    schemaVersion: 1,
    auditedRoots: ['app', 'packaging/builder-toolchain'],
    excludedRoots: [
      { path: 'nexus-website', rationale: 'Dormant website sources, named only by documentation.' },
      { path: 'sdk/typescript', rationale: 'Dormant SDK sources without a lockfile, named only by documentation.' },
    ],
    exceptions: [
      {
        id: BRACES,
        package: 'braces',
        version: '3.0.3',
        severity: 'high',
        range: '<=3.0.3',
        roots: ['app', 'packaging/builder-toolchain'],
        reason: REASON,
        threatModel: 'A reviewed threat model long enough for the schema.',
        reviewBy: '2026-11-30',
      },
    ],
  });
}

// A lockfile v3 with the braces chain, plus `extra` entries.
function lockfile(extra = {}, bracesVersion = '3.0.3') {
  return {
    name: 'fixture',
    lockfileVersion: 3,
    requires: true,
    packages: {
      '': { name: 'fixture', devDependencies: { tailwindcss: '^3.4.19' } },
      'node_modules/braces': { version: bracesVersion },
      'node_modules/chokidar': { version: '3.6.0' },
      'node_modules/fast-glob': { version: '3.3.3' },
      'node_modules/micromatch': { version: '4.0.8' },
      'node_modules/tailwindcss': { version: '3.4.19' },
      'node_modules/vite': { version: '6.4.3' },
      ...extra,
    },
  };
}

// The real shape of the braces report (npm 10.8.2, 2026-10-05), chain included.
function bracesVulns(severity = 'high', url = `https://github.com/advisories/${BRACES}`, range = '<=3.0.3', name = 'braces') {
  const via = { source: 1240992, name, dependency: name, title: 'braces vulnerable to stack-exhaustion denial of service through deeply nested patterns', url, severity, cwe: ['CWE-674'], cvss: { score: 7.5, vectorString: 'CVSS:3.1/AV:N/AC:L/PR:N/UI:N/S:U/C:N/I:N/A:H' }, range };
  return {
    [name]: { name, severity, isDirect: false, via: [via], effects: ['chokidar', 'micromatch'], range: '*', nodes: [`node_modules/${name}`], fixAvailable: { name: 'tailwindcss', version: '4.3.3', isSemVerMajor: true } },
    chokidar: { name: 'chokidar', severity, isDirect: false, via: [name], effects: ['tailwindcss'], range: '2.0.0 - 3.6.0', nodes: ['node_modules/chokidar'], fixAvailable: true },
    micromatch: { name: 'micromatch', severity, isDirect: false, via: [name], effects: ['fast-glob', 'tailwindcss'], range: '>=0.2.0', nodes: ['node_modules/micromatch'], fixAvailable: true },
    'fast-glob': { name: 'fast-glob', severity, isDirect: false, via: ['micromatch'], effects: ['tailwindcss'], range: '*', nodes: ['node_modules/fast-glob'], fixAvailable: true },
    tailwindcss: { name: 'tailwindcss', severity, isDirect: true, via: ['chokidar', 'fast-glob', 'micromatch'], effects: [], range: '2.1.0-canary.1 - 3.4.19', nodes: ['node_modules/tailwindcss'], fixAvailable: true },
  };
}

function advisory(name, id, severity, range = '<9.9.9') {
  return { [name]: { name, severity, isDirect: true, via: [{ source: 1, name, dependency: name, title: 't', url: `https://github.com/advisories/${id}`, severity, range }], effects: [], range, nodes: [`node_modules/${name}`], fixAvailable: true } };
}

// A consistent npm audit v2 report over `vulnerabilities` for `lock`.
function report(vulnerabilities, lock = lockfile()) {
  const counts = { info: 0, low: 0, moderate: 0, high: 0, critical: 0 };
  for (const v of Object.values(vulnerabilities)) counts[v.severity] += 1;
  const total = Object.keys(lock.packages).length - 1;
  return {
    auditReportVersion: 2,
    vulnerabilities,
    metadata: { vulnerabilities: { ...counts, total: Object.keys(vulnerabilities).length }, dependencies: { prod: 0, dev: total, optional: 0, peer: 0, peerOptional: 0, total } },
  };
}

const ok = (rep) => ({ status: Object.keys(rep.vulnerabilities ?? {}).length === 0 ? 0 : 1, signal: null, error: null, stdout: JSON.stringify(rep), mutated: false });

const TRACKED = [
  '.github/workflows/ci.yml',
  'app/package.json',
  'app/package-lock.json',
  'app/src/main.tsx',
  'docs/notes.md',
  'nexus-website/package.json',
  'nexus-website/package-lock.json',
  'packaging/builder-toolchain/package.json',
  'packaging/builder-toolchain/package-lock.json',
  'sdk/typescript/package.json',
  'security/npm-advisory-policy.json',
];

// Runs the gate over a fixture repository. `audits` maps root -> result;
// `files` maps extra or replaced file contents; `tracked` edits the list.
function gate({ pol = policy(), audits = {}, locks = {}, files = {}, tracked = (t) => t, today = TODAY } = {}) {
  const contents = {
    '.github/workflows/ci.yml': 'jobs: {}\n',
    'app/src/main.tsx': 'export {}\n',
    'docs/notes.md': 'nexus-website and sdk/typescript are dormant.\n',
    'nexus-website/package.json': '{"name": "nexus-website"}',
    'sdk/typescript/package.json': '{"name": "@nexus-os/sdk"}',
    'app/package-lock.json': JSON.stringify(locks.app ?? lockfile()),
    'packaging/builder-toolchain/package-lock.json': JSON.stringify(locks['packaging/builder-toolchain'] ?? lockfile()),
    ...files,
  };
  const calls = [];
  const result = evaluate({
    policy: pol,
    trackedFiles: tracked([...TRACKED]),
    readText: (file) => (file in contents ? contents[file] : '{}'),
    audit: (root) => {
      calls.push(root);
      return audits[root] ?? ok(report(bracesVulns()));
    },
    today,
    npmVersion: '10.8.2',
  });
  return { ...result, calls, text: result.lines.join('\n') };
}

function fails(result, pattern) {
  assert.equal(result.ok, false, result.text);
  assert.match(result.text, pattern);
  assert.ok(!result.lines.includes(SUCCESS), 'no success message on failure');
  assert.match(result.lines.at(-1), /^npm advisory gate: FAIL \(\d+ findings?\)$/);
}

function passes(result) {
  assert.equal(result.ok, true, result.text);
  assert.equal(result.lines.at(-1), SUCCESS);
  assert.ok(!/FAIL/.test(result.text), result.text);
}

test('no advisory and no exception passes', () => {
  passes(gate({ pol: policy((p) => ({ ...p, exceptions: [] })), audits: { app: ok(report({})), 'packaging/builder-toolchain': ok(report({})) } }));
});

test('the braces advisory alone, exactly as excepted, passes in both roots', () => {
  const r = gate();
  passes(r);
  assert.deepEqual(r.calls, ['app', 'packaging/builder-toolchain']);
  assert.equal(r.lines.filter((l) => l.includes(`excepted ${BRACES} on braces@3.0.3 (high, affected <=3.0.3); review by 2026-11-30`)).length, 2);
});

test('a new low advisory fails', () => {
  fails(gate({ audits: { app: ok(report({ ...bracesVulns(), ...advisory('vite', 'GHSA-c2f4-gq7p-m8vr', 'low') })) } }), /unexcepted advisory GHSA-c2f4-gq7p-m8vr on vite@6\.4\.3 \(low/);
});

test('a new moderate, high or critical advisory fails', () => {
  for (const severity of ['moderate', 'high', 'critical']) {
    fails(gate({ audits: { 'packaging/builder-toolchain': ok(report({ ...bracesVulns(), ...advisory('vite', 'GHSA-wq9x-h3m5-r6fj', severity) })) } }), new RegExp(`unexcepted advisory GHSA-wq9x-h3m5-r6fj on vite@6\\.4\\.3 \\(${severity}`));
  }
});

test('an informational advisory fails too: every severity counts', () => {
  fails(gate({ audits: { app: ok(report({ ...bracesVulns(), ...advisory('vite', 'GHSA-c2f4-gq7p-m8vr', 'info') })) } }), /unexcepted advisory GHSA-c2f4-gq7p-m8vr/);
});

test('a different advisory on braces (wrong id) fails and the exception goes stale', () => {
  const r = gate({ audits: { app: ok(report(bracesVulns('high', 'https://github.com/advisories/GHSA-2222-3333-4444'))) } });
  fails(r, /unexcepted advisory GHSA-2222-3333-4444 on braces@3\.0\.3/);
});

test('the excepted advisory on another package (wrong package) fails', () => {
  const lock = lockfile({ 'node_modules/notbraces': { version: '3.0.3' } });
  const vulns = bracesVulns('high', undefined, undefined, 'notbraces');
  fails(gate({ locks: { app: lock }, audits: { app: ok(report(vulns, lock)) } }), new RegExp(`unexcepted advisory ${BRACES} on notbraces@3\\.0\\.3`));
});

test('another installed braces version (wrong version) fails', () => {
  const lock = lockfile({}, '3.0.2');
  fails(gate({ locks: { app: lock }, audits: { app: ok(report(bracesVulns(), lock)) } }), /installed 3\.0\.2, excepted 3\.0\.3/);
});

test('a severity change of the excepted advisory (drift) fails', () => {
  fails(gate({ audits: { app: ok(report(bracesVulns('critical'))) } }), /severity is now critical, excepted high/);
});

test('a change of the advisory affected range fails', () => {
  fails(gate({ audits: { app: ok(report(bracesVulns('high', undefined, '<3.0.4'))) } }), /affected range is now <3\.0\.4, excepted <=3\.0\.3/);
});

test('an exception past its review date (expiry) fails', () => {
  const r = gate({ today: '2026-12-01' });
  fails(r, /exception GHSA-vfj7-8cjw-p6xm \(braces\) expired after 2026-11-30/);
  passes(gate({ today: '2026-11-30' }));
});

test('an exception no root reports any more (stale) fails', () => {
  fails(gate({ audits: { app: ok(report({})) } }), /stale exception GHSA-vfj7-8cjw-p6xm \(braces\): app no longer reports it/);
  fails(gate({ audits: { app: ok(report({})), 'packaging/builder-toolchain': ok(report({})) } }), /packaging\/builder-toolchain no longer reports it/);
});

test('an exception reported in a root it does not name fails', () => {
  const pol = policy((p) => ({ ...p, exceptions: [{ ...p.exceptions[0], roots: ['packaging/builder-toolchain'] }] }));
  fails(gate({ pol }), /the exception does not cover app/);
});

test('an unknown npm root fails', () => {
  fails(gate({ tracked: (t) => [...t, 'tools/new-thing/package.json'] }), /unknown npm root tools\/new-thing \(package\.json\)/);
  fails(gate({ tracked: (t) => [...t, 'package-lock.json'] }), /unknown npm root \. \(package-lock\.json\)/);
  fails(gate({ tracked: (t) => [...t, 'web/yarn.lock'] }), /unknown npm root web \(yarn\.lock\)/);
});

test('a policy root that no longer exists fails', () => {
  fails(gate({ tracked: (t) => t.filter((f) => !f.startsWith('nexus-website/')) }), /names nexus-website, which holds no tracked npm manifest/);
});

test('an audited root without its lockfile, or with another lockfile, fails', () => {
  fails(gate({ tracked: (t) => t.filter((f) => f !== 'app/package-lock.json') }), /app: an audited root needs a tracked package\.json and package-lock\.json/);
  fails(gate({ tracked: (t) => [...t, 'app/yarn.lock'] }), /app: an audited root may not carry yarn\.lock/);
  fails(gate({ locks: { app: { ...lockfile(), lockfileVersion: 2 } } }), /app: package-lock\.json is not a lockfileVersion 3 npm lockfile/);
});

test('an audit command failure fails', () => {
  fails(gate({ audits: { app: { status: null, signal: null, error: 'ENOENT', stdout: '', mutated: false } } }), /app: the audit command failed \(ENOENT\)/);
  fails(gate({ audits: { app: { status: null, signal: 'SIGTERM', error: null, stdout: '', mutated: false } } }), /app: the audit command was terminated \(SIGTERM\)/);
  fails(gate({ audits: { app: { ...ok(report(bracesVulns())), status: 2 } } }), /app: the audit command exited 2/);
  fails(gate({ audits: { app: { ...ok(report(bracesVulns())), mutated: true } } }), /app: the audit changed package\.json or package-lock\.json/);
});

test('invalid audit JSON fails', () => {
  fails(gate({ audits: { app: { status: 1, signal: null, error: null, stdout: '{"auditReportVersion": 2, "vulnerab', mutated: false } } }), /app: the audit printed invalid JSON/);
  fails(gate({ audits: { app: { status: 0, signal: null, error: null, stdout: '', mutated: false } } }), /app: the audit printed invalid JSON \(0 bytes\)/);
  fails(gate({ audits: { app: { status: 0, signal: null, error: null, stdout: '[]', mutated: false } } }), /app: the audit printed invalid JSON/);
  fails(gate({ audits: { app: ok({ ...report({}), auditReportVersion: 1 }) } }), /app: the audit report is not an npm audit v2 report/);
});

test('a registry or network failure fails', () => {
  for (const code of ['ENOTFOUND', 'EAI_AGAIN', 'ECONNRESET', 'E503', 'ENOAUDIT']) {
    const stdout = JSON.stringify({ error: { code, summary: `request to ${'https://registry.npmjs.org/-/npm/v1/security/advisories/bulk'} failed`, detail: '' } });
    fails(gate({ audits: { 'packaging/builder-toolchain': { status: 1, signal: null, error: null, stdout, mutated: false } } }), new RegExp(`packaging/builder-toolchain: npm audit reported an error \\(${code}\\)`));
  }
});

test('an excluded root that becomes active fails', () => {
  const r = gate({ files: { '.github/workflows/ci.yml': 'run: cd nexus-website && npm ci\n' } });
  fails(r, /excluded root nexus-website is referenced outside it, so it is no longer dormant: \.github\/workflows\/ci\.yml/);
  fails(gate({ files: { 'app/src/main.tsx': "import { Client } from '@nexus-os/sdk'\n" } }), /excluded root sdk\/typescript is referenced outside it.*app\/src\/main\.tsx/);
  // Documentation and the gate's own files may name it.
  passes(gate({ files: { 'docs/notes.md': 'cd nexus-website && npm ci' } }));
});

test('an exit status that disagrees with the report fails', () => {
  fails(gate({ audits: { app: { ...ok(report(bracesVulns())), status: 0 } } }), /app: the audit exit status 0 disagrees with its report \(5 vulnerable packages\)/);
  fails(gate({ pol: policy((p) => ({ ...p, exceptions: [] })), audits: { app: { ...ok(report({})), status: 1 }, 'packaging/builder-toolchain': ok(report({})) } }), /exit status 1 disagrees/);
});

test('inconsistent report metadata or chains fail', () => {
  const rep = report(bracesVulns());
  fails(gate({ audits: { app: ok({ ...rep, metadata: { ...rep.metadata, vulnerabilities: { ...rep.metadata.vulnerabilities, total: 1 } } }) } }), /counts disagree with its entries/);
  fails(gate({ audits: { app: ok({ ...rep, metadata: { ...rep.metadata, dependencies: { ...rep.metadata.dependencies, total: 2 } } }) } }), /the audit covered 2 packages but the lockfile has 6/);
  const dangling = bracesVulns();
  dangling.tailwindcss.via = ['postcss'];
  fails(gate({ audits: { app: ok(report(dangling)) } }), /tailwindcss is reported through postcss, which the report does not list/);
  const hidden = bracesVulns();
  hidden.tailwindcss.severity = 'critical';
  fails(gate({ audits: { app: ok(report(hidden)) } }), /tailwindcss is reported as critical, inconsistent with its advisories/);
  const elsewhere = bracesVulns();
  elsewhere.braces.nodes = ['node_modules/x/node_modules/braces'];
  fails(gate({ audits: { app: ok(report(elsewhere)) } }), /braces is reported at a location the lockfile does not have/);
  const nonGhsa = bracesVulns('high', 'https://www.npmjs.com/advisories/1');
  fails(gate({ audits: { app: ok(report(nonGhsa)) } }), /unexcepted advisory an advisory without a GHSA identifier on braces/);
});

test('the policy schema is strict: no wildcard, severity-wide or package-wide exception', () => {
  const base = policy();
  const variants = [
    [{ ...base, extra: true }, /exactly the keys/],
    [{ ...base, schemaVersion: 2 }, /schemaVersion must be 1/],
    [{ ...base, auditedRoots: [] }, /auditedRoots/],
    [{ ...base, auditedRoots: ['../app'] }, /auditedRoots/],
    [{ ...base, excludedRoots: [{ path: 'nexus-website' }] }, /excludedRoots\[0\]/],
    [{ ...base, excludedRoots: [{ path: 'app', rationale: REASON }] }, /listed twice/],
  ];
  const exception = base.exceptions[0];
  for (const [edit, pattern] of [
    [{ id: 'GHSA-*' }, /id must be one exact GHSA identifier/],
    [{ id: '*' }, /id must be one exact GHSA identifier/],
    [{ package: '*' }, /package must be one exact npm package name/],
    [{ package: 'brace*' }, /package must be one exact npm package name/],
    [{ version: '3.x' }, /version must be one exact version/],
    [{ version: '*' }, /version must be one exact version/],
    [{ version: '^3.0.3' }, /version must be one exact version/],
    [{ severity: '*' }, /severity must be low, moderate, high or critical/],
    [{ severity: 'info' }, /severity must be low, moderate, high or critical/],
    [{ range: '*' }, /range must be the advisory's exact affected range/],
    [{ roots: ['nexus-website'] }, /roots must be distinct audited roots/],
    [{ roots: [] }, /roots must be distinct audited roots/],
    [{ reason: '' }, /reason is required/],
    [{ threatModel: 'short' }, /threatModel is required/],
    [{ reviewBy: '2026-02-30' }, /reviewBy must be a YYYY-MM-DD date/],
    [{ reviewBy: 'never' }, /reviewBy must be a YYYY-MM-DD date/],
  ]) {
    variants.push([{ ...base, exceptions: [{ ...exception, ...edit }] }, pattern]);
  }
  const { id, ...noId } = exception;
  variants.push([{ ...base, exceptions: [noId] }, /must have exactly the keys/]);
  variants.push([{ ...base, exceptions: [{ ...exception, severityAtOrBelow: 'high' }] }, /must have exactly the keys/]);
  variants.push([{ ...base, exceptions: [exception, exception] }, /repeats GHSA-vfj7-8cjw-p6xm braces/]);
  for (const [pol, pattern] of variants) {
    assert.match(validatePolicy(pol).join('\n'), pattern, JSON.stringify(pol).slice(0, 200));
    const r = gate({ pol });
    fails(r, pattern);
    assert.deepEqual(r.calls, [], 'no audit runs under an invalid policy');
  }
});

test('the committed policy is valid and excepts exactly the braces advisory', () => {
  const committed = JSON.parse(fs.readFileSync(path.join(here, '..', '..', 'security', 'npm-advisory-policy.json'), 'utf8'));
  assert.deepEqual(validatePolicy(committed), []);
  assert.deepEqual(committed.auditedRoots, ['app', 'packaging/builder-toolchain']);
  assert.deepEqual(committed.excludedRoots.map((e) => e.path), ['nexus-website', 'scripts/page-audit', 'sdk/typescript']);
  assert.deepEqual(
    committed.exceptions.map(({ id, package: name, version, severity, range, roots, reviewBy }) => ({ id, name, version, severity, range, roots, reviewBy })),
    [{ id: BRACES, name: 'braces', version: '3.0.3', severity: 'high', range: '<=3.0.3', roots: ['app', 'packaging/builder-toolchain'], reviewBy: '2026-11-30' }],
  );
});

test('credentials never reach the output', () => {
  const secret = 'npm_0123456789abcdefABCDEF0123456789abcd';
  const stdout = JSON.stringify({ error: { code: 'E401', summary: `Unable to authenticate: //registry.npmjs.org/:_authToken=${secret} https://user:hunter2@registry.example/ Bearer abc.def`, detail: '' } });
  const r = gate({ audits: { app: { status: 1, signal: null, error: null, stdout, mutated: false } } });
  fails(r, /npm audit reported an error \(E401\)/);
  for (const leaked of [secret, 'hunter2', 'abc.def']) assert.ok(!r.text.includes(leaked), `leaked ${leaked}`);
  assert.equal(redact('token=ghp_0123456789abcdefghijABCDEFGHIJ012345'), 'token=<redacted>');
  assert.equal(redact('//user:pass@host/'), '//<redacted>@host/');
});

test('the output is bounded', () => {
  const vulns = { ...bracesVulns() };
  const lock = lockfile(Object.fromEntries(Array.from({ length: 120 }, (_, i) => [`node_modules/p${i}`, { version: '1.0.0' }])));
  for (let i = 0; i < 120; i += 1) Object.assign(vulns, advisory(`p${i}`, 'GHSA-c2f4-gq7p-m8vr', 'low'));
  const r = gate({ locks: { app: lock }, audits: { app: ok(report(vulns, lock)) } });
  fails(r, /further advisories omitted/);
  assert.ok(r.lines.length <= 201, `${r.lines.length} lines`);
  assert.ok(r.lines.every((line) => line.length <= 240));
});

test('the gate claims no more than its finding and takes no arguments', () => {
  assert.equal(SUCCESS, 'No known unexcepted npm advisories were reported for the audited roots at this run.');
  const source = fs.readFileSync(path.join(here, 'npm-advisory-gate.mjs'), 'utf8');
  for (const claim of [/dependencies are secure/i, /zero vulnerabilities/i, /no vulnerabilities/i, /\bis secure\b/i]) {
    assert.doesNotMatch(source, claim);
  }
  const run = spawnSync(process.execPath, [path.join(here, 'npm-advisory-gate.mjs'), '--skip'], { encoding: 'utf8', timeout: 30_000 });
  assert.equal(run.status, 2);
  assert.match(run.stdout, /takes no arguments/);
});
