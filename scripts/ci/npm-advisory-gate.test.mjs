// XA-R4C and XA-R4C-R1: tests of the npm advisory gate, without npm or the
// network: the gate's evaluation runs over fixture policies, tracked-file
// lists, lockfiles and audit results shaped like npm 10's `npm audit --json`
// (report v2).
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
// npm's remediation signal for braces that the policy records as reviewed.
const REVIEWED_FIX = { name: 'tailwindcss', version: '4.3.3', isSemVerMajor: true };
const ABSENT = Symbol('absent');

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
        npmFixAvailable: [
          { root: 'app', value: { ...REVIEWED_FIX } },
          { root: 'packaging/builder-toolchain', value: { ...REVIEWED_FIX } },
        ],
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

// The live braces chain: the `vulnerabilities` of `npm audit --package-lock-only
// --json` for app and packaging/builder-toolchain alike, npm's remediation
// signal (`fixAvailable`) of every entry included (XA-R4C-R1, 2026-10-05:
// npm 10.8.2 and 10.9.9 returned byte-identical reports).
const LIVE_BRACES_CHAIN = {
  "braces": {
    "name": "braces",
    "severity": "high",
    "isDirect": false,
    "via": [
      {
        "source": 1240992,
        "name": "braces",
        "dependency": "braces",
        "title": "braces vulnerable to stack-exhaustion denial of service through deeply nested patterns",
        "url": "https://github.com/advisories/GHSA-vfj7-8cjw-p6xm",
        "severity": "high",
        "cwe": [
          "CWE-674"
        ],
        "cvss": {
          "score": 7.5,
          "vectorString": "CVSS:3.1/AV:N/AC:L/PR:N/UI:N/S:U/C:N/I:N/A:H"
        },
        "range": "<=3.0.3"
      }
    ],
    "effects": [
      "chokidar",
      "micromatch"
    ],
    "range": "*",
    "nodes": [
      "node_modules/braces"
    ],
    "fixAvailable": {
      "name": "tailwindcss",
      "version": "4.3.3",
      "isSemVerMajor": true
    }
  },
  "chokidar": {
    "name": "chokidar",
    "severity": "high",
    "isDirect": false,
    "via": [
      "braces"
    ],
    "effects": [
      "tailwindcss"
    ],
    "range": "2.0.0 - 3.6.0",
    "nodes": [
      "node_modules/chokidar"
    ],
    "fixAvailable": {
      "name": "tailwindcss",
      "version": "4.3.3",
      "isSemVerMajor": true
    }
  },
  "fast-glob": {
    "name": "fast-glob",
    "severity": "high",
    "isDirect": false,
    "via": [
      "micromatch"
    ],
    "effects": [],
    "range": "*",
    "nodes": [
      "node_modules/fast-glob"
    ],
    "fixAvailable": true
  },
  "micromatch": {
    "name": "micromatch",
    "severity": "high",
    "isDirect": false,
    "via": [
      "braces"
    ],
    "effects": [
      "fast-glob",
      "tailwindcss"
    ],
    "range": ">=0.2.0",
    "nodes": [
      "node_modules/micromatch"
    ],
    "fixAvailable": {
      "name": "tailwindcss",
      "version": "4.3.3",
      "isSemVerMajor": true
    }
  },
  "tailwindcss": {
    "name": "tailwindcss",
    "severity": "high",
    "isDirect": true,
    "via": [
      "chokidar",
      "fast-glob",
      "micromatch"
    ],
    "effects": [],
    "range": "<=0.0.0-oxide-insiders.ff2c25f || 2.1.0-canary.1 - 3.4.19",
    "nodes": [
      "node_modules/tailwindcss"
    ],
    "fixAvailable": {
      "name": "tailwindcss",
      "version": "4.3.3",
      "isSemVerMajor": true
    }
  }
};

// The live chain at `severity`, with the advisory's URL and affected range,
// its package renamed to `name`, and braces' `fixAvailable` replaced by `fix`
// (ABSENT removes it).
function bracesVulns(severity = 'high', url = `https://github.com/advisories/${BRACES}`, range = '<=3.0.3', name = 'braces', fix = REVIEWED_FIX) {
  const vulns = structuredClone(LIVE_BRACES_CHAIN);
  for (const vuln of Object.values(vulns)) {
    vuln.severity = severity;
    vuln.via = vuln.via.map((via) => (via === 'braces' ? name : via));
  }
  const braces = vulns.braces;
  Object.assign(braces.via[0], { name, dependency: name, url, severity, range });
  if (fix === ABSENT) delete braces.fixAvailable;
  else braces.fixAvailable = structuredClone(fix);
  if (name !== 'braces') {
    delete vulns.braces;
    Object.assign(braces, { name, nodes: [`node_modules/${name}`] });
    vulns[name] = braces;
  }
  return vulns;
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
  const builderOnly = (records) => records.filter((record) => record.root === 'packaging/builder-toolchain');
  const pol = policy((p) => ({ ...p, exceptions: [{ ...p.exceptions[0], roots: ['packaging/builder-toolchain'], npmFixAvailable: builderOnly(p.exceptions[0].npmFixAvailable) }] }));
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
  assert.deepEqual(committed.exceptions[0].npmFixAvailable, [
    { root: 'app', value: REVIEWED_FIX },
    { root: 'packaging/builder-toolchain', value: REVIEWED_FIX },
  ]);
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

// XA-R4C-R1: npm's remediation signal for the excepted braces vulnerability
// must be exactly the one reviewed for each root. Any change fails and asks
// for security review; nothing is accepted silently.
const DRIFT = "npm's remediation signal for excepted GHSA-vfj7-8cjw-p6xm \\(braces\\) changed and requires security review";
const withFix = (fix) => ok(report(bracesVulns('high', undefined, undefined, 'braces', fix)));
const driftIn = (root, fix) => gate({ audits: { [root]: withFix(fix) } });
const accepted = (root) => `${root}: npm remediation signal for ${BRACES} matches the reviewed value: fixAvailable {"name":"tailwindcss","version":"4.3.3","isSemVerMajor":true}`;

test('R1-T1 the exact reviewed braces remediation signal passes in both roots', () => {
  const r = gate();
  passes(r);
  for (const root of ['app', 'packaging/builder-toolchain']) assert.ok(r.lines.includes(accepted(root)), r.text);
});

test('R1-T2 fixAvailable true fails and requires review', () => {
  fails(driftIn('app', true), new RegExp(`^FAIL app: ${DRIFT}: fixAvailable is now true, reviewed tailwindcss@4\\.3\\.3 \\(major\\)$`, 'm'));
});

test('R1-T3 fixAvailable false fails and requires review', () => {
  fails(driftIn('app', false), new RegExp(`^FAIL app: ${DRIFT}: fixAvailable is now false, reviewed tailwindcss@4\\.3\\.3 \\(major\\)$`, 'm'));
});

test('R1-T4 a missing or null fixAvailable fails', () => {
  fails(driftIn('app', ABSENT), new RegExp(`${DRIFT}: fixAvailable is now absent, reviewed tailwindcss@4\\.3\\.3 \\(major\\)$`, 'm'));
  fails(driftIn('app', null), new RegExp(`${DRIFT}: fixAvailable is now null, reviewed`));
});

test('R1-T5 a direct patched braces remediation fails and requires review', () => {
  fails(driftIn('app', { name: 'braces', version: '3.0.4', isSemVerMajor: false }), new RegExp(`${DRIFT}: fixAvailable is now braces@3\\.0\\.4 \\(not major\\), reviewed tailwindcss@4\\.3\\.3 \\(major\\)`));
});

test('R1-T6 a Tailwind remediation version drift fails', () => {
  const r = driftIn('packaging/builder-toolchain', { ...REVIEWED_FIX, version: '4.3.4' });
  fails(r, new RegExp(`^FAIL packaging/builder-toolchain: ${DRIFT}: fixAvailable is now tailwindcss@4\\.3\\.4 \\(major\\), reviewed tailwindcss@4\\.3\\.3 \\(major\\)$`, 'm'));
  assert.doesNotMatch(r.text, /stale exception/, 'a drifted root is not misreported as stale');
});

test('R1-T7 a Tailwind isSemVerMajor drift fails', () => {
  fails(driftIn('app', { ...REVIEWED_FIX, isSemVerMajor: false }), new RegExp(`${DRIFT}: fixAvailable is now tailwindcss@4\\.3\\.3 \\(not major\\), reviewed tailwindcss@4\\.3\\.3 \\(major\\)`));
});

test('R1-T8 a remediation package-name or shape drift fails', () => {
  fails(driftIn('app', { ...REVIEWED_FIX, name: '@tailwindcss/postcss' }), new RegExp(`${DRIFT}: fixAvailable is now @tailwindcss/postcss@4\\.3\\.3 \\(major\\)`));
  fails(driftIn('app', { ...REVIEWED_FIX, isDirect: true }), new RegExp(`${DRIFT}: fixAvailable is now an object with keys isDirect,isSemVerMajor,name,version, reviewed`));
  fails(driftIn('app', { name: 'tailwindcss', version: '4.3.3' }), new RegExp(`${DRIFT}: fixAvailable is now an object with keys name,version, reviewed`));
  fails(driftIn('app', 'tailwindcss@4.3.3'), new RegExp(`${DRIFT}: fixAvailable is now a string, reviewed`));
  fails(driftIn('app', [REVIEWED_FIX]), new RegExp(`${DRIFT}: fixAvailable is now a list, reviewed`));
});

test("R1-T9 a policy without one root's remediation expectation fails schema validation", () => {
  const pol = policy((p) => ({ ...p, exceptions: [{ ...p.exceptions[0], npmFixAvailable: [p.exceptions[0].npmFixAvailable[0]] }] }));
  assert.match(validatePolicy(pol).join('\n'), /npmFixAvailable must hold exactly one reviewed remediation signal for each of the exception's roots/);
  const r = gate({ pol });
  fails(r, /npmFixAvailable must hold exactly one reviewed remediation signal/);
  assert.deepEqual(r.calls, [], 'no audit runs under an invalid policy');
  const { npmFixAvailable, ...without } = policy().exceptions[0];
  fails(gate({ pol: policy((p) => ({ ...p, exceptions: [without] })) }), /must have exactly the keys/);
});

test('R1-T10 a remediation expectation for a root the exception does not name fails schema validation', () => {
  const records = policy().exceptions[0].npmFixAvailable;
  for (const [extra, pattern] of [
    [[...records, { root: 'nexus-website', value: { ...REVIEWED_FIX } }], /npmFixAvailable must hold exactly one reviewed remediation signal for each of the exception's roots/],
    [[records[0], { root: 'scripts/page-audit', value: { ...REVIEWED_FIX } }], /npmFixAvailable must hold exactly one reviewed remediation signal/],
    [[records[0], records[0]], /npmFixAvailable names a root twice/],
  ]) {
    const pol = policy((p) => ({ ...p, exceptions: [{ ...p.exceptions[0], npmFixAvailable: extra }] }));
    assert.match(validatePolicy(pol).join('\n'), pattern);
    const r = gate({ pol });
    fails(r, pattern);
    assert.deepEqual(r.calls, [], 'no audit runs under an invalid policy');
  }
});

test('R1-T11 roots whose remediation differs from the policy fail independently', () => {
  const drifted = withFix({ ...REVIEWED_FIX, version: '4.3.4' });
  const onlyBuilder = gate({ audits: { 'packaging/builder-toolchain': drifted } });
  fails(onlyBuilder, new RegExp(`^FAIL packaging/builder-toolchain: ${DRIFT}`, 'm'));
  assert.ok(onlyBuilder.lines.includes(accepted('app')), 'app is accepted on its own');
  assert.equal(onlyBuilder.lines.at(-1), 'npm advisory gate: FAIL (1 finding)');
  const onlyApp = gate({ audits: { app: drifted } });
  fails(onlyApp, new RegExp(`^FAIL app: ${DRIFT}`, 'm'));
  assert.ok(onlyApp.lines.includes(accepted('packaging/builder-toolchain')), 'the Builder root is accepted on its own');
  assert.equal(onlyApp.lines.at(-1), 'npm advisory gate: FAIL (1 finding)');
  const both = gate({ audits: { app: drifted, 'packaging/builder-toolchain': withFix(false) } });
  fails(both, /fixAvailable is now false/);
  assert.equal(both.lines.at(-1), 'npm advisory gate: FAIL (2 findings)');
  // The expectation is per root: each root is held to its own reviewed value.
  const perRoot = policy((p) => ({ ...p, exceptions: [{ ...p.exceptions[0], npmFixAvailable: [{ root: 'app', value: true }, p.exceptions[0].npmFixAvailable[1]] }] }));
  const r = gate({ pol: perRoot });
  fails(r, new RegExp(`^FAIL app: ${DRIFT}: fixAvailable is now tailwindcss@4\\.3\\.3 \\(major\\), reviewed true$`, 'm'));
  assert.ok(!r.lines.some((line) => line.startsWith('FAIL packaging/builder-toolchain')), r.text);
  passes(gate({ pol: perRoot, audits: { app: withFix(true) } }));
});

test('R1 remediation expectations are exact: no wildcard and no "any fix"', () => {
  const records = policy().exceptions[0].npmFixAvailable;
  const pattern = /npmFixAvailable must be a list of exact \{root, value\} records/;
  const variants = [[], 'tailwindcss', true, [{ root: 'app' }, records[1]], [{ root: 'app', value: true, note: 'any fix' }, records[1]], [{ value: true }, records[1]], [{ root: 1, value: true }, records[1]]];
  for (const value of ['*', 'any', null, 1, [], {}, { ...REVIEWED_FIX, name: '*' }, { ...REVIEWED_FIX, version: '4.x' }, { ...REVIEWED_FIX, version: '^4.3.3' },
    { ...REVIEWED_FIX, isSemVerMajor: 'true' }, { ...REVIEWED_FIX, extra: 1 }, { name: 'tailwindcss', version: '4.3.3' }]) {
    variants.push([{ root: 'app', value }, records[1]]);
  }
  for (const npmFixAvailable of variants) {
    const pol = policy((p) => ({ ...p, exceptions: [{ ...p.exceptions[0], npmFixAvailable }] }));
    assert.match(validatePolicy(pol).join('\n'), pattern, JSON.stringify(npmFixAvailable));
    const r = gate({ pol });
    fails(r, pattern);
    assert.deepEqual(r.calls, [], 'no audit runs under an invalid policy');
  }
});
