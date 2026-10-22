// P0-002C4D2/C4C3 trusted-entry tests. Run with the packaged Node against an
// assembled toolchain:
//   <toolchain>/node/node --test packaging/builder-toolchain/test/
// NEXUS_BUILDER_TOOLCHAIN_ROOT (test harness only) names the toolchain root;
// the default is the release staging location app/src-tauri/builder-toolchain.
import assert from 'node:assert/strict';
import { spawn, spawnSync } from 'node:child_process';
import fs from 'node:fs';
import http from 'node:http';
import os from 'node:os';
import path from 'node:path';
import { test } from 'node:test';
import { fileURLToPath, pathToFileURL } from 'node:url';

const here = path.dirname(fileURLToPath(import.meta.url));
const toolchainRoot = path.resolve(
  process.env.NEXUS_BUILDER_TOOLCHAIN_ROOT ??
    path.join(here, '..', '..', '..', 'app', 'src-tauri', 'builder-toolchain'),
);
const fromToolchain = (...parts) => pathToFileURL(path.join(toolchainRoot, ...parts)).href;

// Everything outside the toolchain is loaded above; from here on this process
// runs under the same module confinement the production entry installs.
const { installModuleGuard, MODULE_DENIED } = await import(fromToolchain('entry', 'module-guard.mjs'));
installModuleGuard(toolchainRoot);
const { build } = await import(fromToolchain('node_modules', 'vite', 'dist', 'node', 'index.js'));
const trusted = await import(fromToolchain('entry', 'trusted-config.mjs'));
const preview = await import(fromToolchain('entry', 'preview-server.mjs'));
const TOKEN = '0123456789abcdef0123456789abcdef';

const MARKERS = [
  'vite-config',
  'postcss-config',
  'postcssrc',
  'tailwind-config',
  'babel-config',
  'babelrc',
  'at-config',
  'at-plugin',
  'ancestor-package',
  'sass',
  'less',
  'browserslist',
];

function write(file, content) {
  fs.mkdirSync(path.dirname(file), { recursive: true });
  fs.writeFileSync(file, content);
}

// A throwing, marker-writing module: executing it anywhere is a failure.
function hostile(markerDir, name) {
  return `require('node:fs').writeFileSync(${JSON.stringify(path.join(markerDir, name))}, 'executed');\nmodule.exports = {};\n`;
}

const SINGLE_PAGE = {
  'index.html':
    '<!DOCTYPE html>\n<html lang="en">\n  <head>\n    <meta charset="UTF-8" />\n    <link rel="icon" type="image/svg+xml" href="/favicon.svg" />\n    <title>fixture</title>\n  </head>\n  <body>\n    <div id="root"></div>\n    <script type="module" src="/src/main.tsx"></script>\n  </body>\n</html>\n',
  'public/favicon.svg': '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 32 32"></svg>',
  'src/main.tsx':
    "import React from 'react'\nimport ReactDOM from 'react-dom/client'\nimport App from './App'\nimport './index.css'\n\nReactDOM.createRoot(document.getElementById('root')!).render(\n  <React.StrictMode>\n    <App />\n  </React.StrictMode>,\n)\n",
  'src/App.tsx': "import Home from './pages/Home'\n\nexport default function App() {\n  return <Home />\n}\n",
  'src/pages/Home.tsx':
    "import { cn } from '../lib/utils'\n\ninterface Props { title?: string }\n\nexport default function Home({ title = 'NEXUS_FIXTURE_TITLE' }: Props) {\n  return <main className={cn('min-h-screen bg-bg text-text-primary py-md', 'font-heading')}>{title}</main>\n}\n",
  'src/lib/utils.ts':
    "export function cn(...classes: (string | undefined | false)[]): string {\n  return classes.filter(Boolean).join(' ')\n}\n",
  'src/index.css':
    '@tailwind base;\n@tailwind components;\n@tailwind utilities;\n\n:root {\n  --color-bg: #0a0a0f;\n  --color-text: #f0f0f5;\n  --space-md: 1rem;\n  --font-heading: system-ui;\n}\n\nbody { background-color: var(--color-bg); }\n@media (min-width: 640px) { body { line-height: 1.6; } }\n@keyframes fade { from { opacity: 0 } to { opacity: 1 } }\n',
};

const MULTI_PAGE = {
  ...SINGLE_PAGE,
  'src/main.tsx':
    "import React from 'react'\nimport ReactDOM from 'react-dom/client'\nimport { BrowserRouter } from 'react-router-dom'\nimport App from './App'\nimport './index.css'\n\nReactDOM.createRoot(document.getElementById('root')!).render(\n  <React.StrictMode>\n    <BrowserRouter>\n      <App />\n    </BrowserRouter>\n  </React.StrictMode>,\n)\n",
  'src/App.tsx':
    "import { Routes, Route } from 'react-router-dom'\nimport Home from './pages/Home'\n\nexport default function App() {\n  return (\n    <Routes>\n      <Route path=\"/\" element={<Home />} />\n    </Routes>\n  )\n}\n",
};

// Project content plus every project-controlled configuration and ancestor
// package a naive toolchain would load. Returns the backend request.
function fixture(files, extra = {}) {
  // The canonical spelling, as the backend passes it (no 8.3 or symlinked alias).
  const base = fs.realpathSync.native(fs.mkdtempSync(path.join(os.tmpdir(), 'nexus-c4d2-')));
  const markers = path.join(base, 'markers');
  fs.mkdirSync(markers);
  const ancestor = path.join(base, 'ancestor');
  const projectRoot = path.join(ancestor, 'builds', 'project', 'react');
  const cacheDir = path.join(ancestor, 'builds', 'project', 'runtime', 'vite-cache');
  fs.mkdirSync(cacheDir, { recursive: true });
  for (const [name, content] of Object.entries({ ...files, ...extra })) {
    write(path.join(projectRoot, name), content.replaceAll('__MARKERS__', markers));
  }
  // Project-controlled configuration; none may ever be executed.
  write(path.join(projectRoot, 'vite.config.js'), hostile(markers, 'vite-config'));
  write(path.join(projectRoot, 'vite.config.ts'), hostile(markers, 'vite-config'));
  write(path.join(projectRoot, 'postcss.config.js'), hostile(markers, 'postcss-config'));
  write(path.join(projectRoot, '.postcssrc.js'), hostile(markers, 'postcssrc'));
  write(path.join(projectRoot, 'tailwind.config.js'), hostile(markers, 'tailwind-config'));
  write(path.join(projectRoot, 'babel.config.js'), hostile(markers, 'babel-config'));
  write(path.join(projectRoot, '.babelrc.js'), hostile(markers, 'babelrc'));
  write(path.join(projectRoot, 'evil-config.js'), hostile(markers, 'at-config'));
  write(path.join(projectRoot, 'evil-plugin.js'), hostile(markers, 'at-plugin'));
  write(
    path.join(projectRoot, 'package.json'),
    JSON.stringify({
      name: 'hostile',
      type: 'commonjs',
      scripts: { dev: 'node -e "process.exit(9)"', prepare: 'node evil.js' },
      browserslist: ['extends browserslist-config-hostile'],
      imports: { '#x': '../../../outside.js' },
    }),
  );
  write(path.join(projectRoot, '.browserslistrc'), 'extends browserslist-config-hostile\n');
  write(path.join(projectRoot, '.env'), 'VITE_SECRET=project-env-secret\nNEXUS_BUILDER_PUBLIC_SECRET=project-env-secret\nBROWSER=/bin/false\n');
  write(path.join(projectRoot, '.env.development'), 'VITE_SECRET=project-env-secret\n');
  write(path.join(projectRoot, '.env.local'), 'VITE_SECRET=project-env-secret\n');
  write(
    path.join(projectRoot, 'tsconfig.json'),
    JSON.stringify({ compilerOptions: { jsx: 'react-jsx', jsxImportSource: 'hostile-jsx' } }),
  );
  write(path.join(projectRoot, 'src', 'tsconfig.json'), JSON.stringify({ compilerOptions: { jsxImportSource: 'hostile-jsx' } }));
  // Packages only an ancestor directory provides.
  for (const pkg of ['firebase', 'sass', 'sass-embedded', 'less', 'hostile-jsx', 'browserslist-config-hostile']) {
    const dir = path.join(ancestor, 'node_modules', pkg);
    write(path.join(dir, 'package.json'), JSON.stringify({ name: pkg, main: 'index.js' }));
    write(path.join(dir, 'index.js'), hostile(markers, pkg === 'sass' || pkg === 'sass-embedded' ? 'sass' : pkg === 'less' ? 'less' : pkg.startsWith('browserslist') ? 'browserslist' : 'ancestor-package'));
    write(path.join(dir, 'jsx-runtime.js'), hostile(markers, 'ancestor-package'));
    write(path.join(dir, 'jsx-dev-runtime.js'), hostile(markers, 'ancestor-package'));
  }
  write(path.join(ancestor, 'outside.js'), 'export const secret = "OUTSIDE_SECRET";\n');
  return { base, markers, projectRoot, cacheDir, request: { projectRoot, cacheDir } };
}

function executedMarkers(f) {
  return MARKERS.filter((name) => fs.existsSync(path.join(f.markers, name)));
}

async function trustedBuild(f) {
  const config = trusted.createTrustedViteConfig(f.request, toolchainRoot);
  const result = await build({
    ...config,
    logLevel: 'silent',
    build: { write: false, outDir: path.join(f.base, 'out'), minify: false },
  });
  const outputs = (Array.isArray(result) ? result : [result]).flatMap((r) => r.output);
  const text = (predicate) =>
    outputs
      .filter((o) => predicate(o.fileName))
      .map((o) => (o.type === 'chunk' ? o.code : String(o.source)))
      .join('\n');
  return { js: text((n) => n.endsWith('.js')), css: text((n) => n.endsWith('.css')), outputs };
}

async function rejectedBuild(f, pattern) {
  const config = trusted.createTrustedViteConfig(f.request, toolchainRoot);
  await assert.rejects(
    build({ ...config, logLevel: 'silent', build: { write: false, outDir: path.join(f.base, 'out') } }),
    (error) => {
      assert.match(String(error?.message ?? error), pattern);
      return true;
    },
  );
}

test('trusted configuration is programmatic and discovers no project configuration', () => {
  const f = fixture(SINGLE_PAGE);
  const config = trusted.createTrustedViteConfig(f.request, toolchainRoot);
  assert.equal(config.configFile, false);
  assert.equal(config.envDir, false);
  assert.equal(config.envPrefix, 'NEXUS_BUILDER_PUBLIC_');
  assert.equal(config.server.open, false);
  assert.equal(config.devtools, false);
  assert.equal(config.root, f.projectRoot);
  assert.equal(config.cacheDir, f.cacheDir);
  assert.equal(config.tsconfig, path.join(toolchainRoot, 'entry', 'tsconfig.json'));
  assert.equal(typeof config.css.postcss, 'object', 'inline PostCSS config disables discovery');
  assert.equal(config.css.modules, false);
  assert.equal(config.optimizeDeps.noDiscovery, true);
  assert.deepEqual(config.optimizeDeps.entries, []);
  assert.deepEqual(config.server.fs.allow, [f.projectRoot, f.cacheDir]);
  // P0-002C4C3 dev-server form: middleware behind the Nexus gate; no HMR,
  // WebSocket, CORS or browser; a watcher that never follows links.
  assert.equal(config.server.middlewareMode, true);
  assert.equal(config.server.hmr, false);
  assert.equal(config.server.ws, false);
  assert.equal(config.server.cors, false);
  assert.equal(config.server.fs.strict, true);
  assert.deepEqual(config.server.watch, { followSymlinks: false });
  assert.equal(config.server.host, undefined);
  assert.equal(config.server.proxy, undefined);
  assert.equal(config.server.https, undefined);
  assert.equal(config.preview, undefined);
  assert.equal(config.logLevel, 'warn');
  // Runtime imports resolve only to the toolchain's exact packages.
  const aliases = config.resolve.alias.map(({ find, replacement }) => [String(find), replacement]);
  assert.deepEqual(
    aliases.map(([find]) => find),
    ['/^react$/', '/^react\\/jsx-runtime$/', '/^react\\/jsx-dev-runtime$/', '/^react-dom$/', '/^react-dom\\/client$/', '/^react-router-dom$/'],
  );
  for (const [, replacement] of aliases) {
    assert.ok(replacement.startsWith(path.join(toolchainRoot, 'node_modules') + path.sep), replacement);
  }
});

test('generated single-page project builds only from Nexus toolchain dependencies', async () => {
  const f = fixture(SINGLE_PAGE);
  const out = await trustedBuild(f);
  assert.match(out.js, /NEXUS_FIXTURE_TITLE/);
  assert.match(out.js, /createRoot/);
  assert.match(out.css, /\.bg-bg\s*\{[^}]*var\(--color-bg\)/, 'Nexus theme mapping applies');
  assert.match(out.css, /\.py-md\s*\{[^}]*var\(--space-md\)/);
  assert.match(out.css, /@keyframes fade/, 'ordinary CSS stays supported');
  assert.deepEqual(executedMarkers(f), []);
  assert.equal(fs.existsSync(path.join(f.projectRoot, 'node_modules')), false, 'no project node_modules');
  assert.equal(fs.existsSync(path.join(f.projectRoot, 'dist')), false);
});

// Candidate 10: exercise the parser override through the actual trusted build,
// including selectors supplied as project data, with hostile config fixtures.
test('generated project keeps utilities, variants, apply and complex nested CSS', async () => {
  const f = fixture(SINGLE_PAGE, {
    'src/pages/Home.tsx':
      'export default function Home() { return <main className="flex hover:bg-blue-500 md:grid group-hover:font-bold w-[13px] card primary"><span className="child:token">NEXUS_SELECTOR_COMPAT</span></main> }',
    'src/index.css': String.raw`
      @tailwind utilities;
      .card:is(.primary,.secondary):not([data-note="comma, and )"]) {
        @apply px-4 font-bold;
        & > .child\:token:hover { @apply text-red-500; }
      }
    `,
  });
  try {
    const out = await trustedBuild(f);
    assert.match(out.js, /NEXUS_SELECTOR_COMPAT/);
    assert.match(out.css, /display:\s*flex/);
    assert.match(out.css, /display:\s*grid/);
    assert.ok(out.css.includes(String.raw`.hover\:bg-blue-500:hover`));
    assert.match(out.css, /padding-left:\s*1rem/);
    assert.match(out.css, /font-weight:\s*700/);
    assert.match(out.css, /width:\s*13px/);
    assert.ok(out.css.includes(String.raw`.child\:token:hover`));
    assert.doesNotMatch(out.css, /@apply|@tailwind/);
    assert.deepEqual(executedMarkers(f), []);
    assert.equal(fs.existsSync(path.join(f.projectRoot, 'node_modules')), false);
  } finally {
    fs.rmSync(f.base, { recursive: true, force: true });
  }
});

test('generated multi-page project resolves react-router-dom from the toolchain', async () => {
  const f = fixture(MULTI_PAGE);
  const out = await trustedBuild(f);
  assert.match(out.js, /NEXUS_FIXTURE_TITLE/);
  assert.match(out.js, /BrowserRouter|useRoutes|Routes/);
  assert.deepEqual(executedMarkers(f), []);
  assert.equal(fs.existsSync(path.join(f.projectRoot, 'node_modules')), false);
});

test('project and process environment never reach client code', async () => {
  const f = fixture(SINGLE_PAGE, {
    'src/App.tsx':
      "import Home from './pages/Home'\nexport const env = JSON.stringify(import.meta.env)\nexport const leaks = [import.meta.env.VITE_SECRET, import.meta.env.NEXUS_BUILDER_PUBLIC_SECRET, import.meta.env.VITE_PROCESS_LEAK]\nexport default function App() {\n  return <Home title={env} />\n}\n",
  });
  process.env.VITE_PROCESS_LEAK = 'process-env-secret';
  try {
    const out = await trustedBuild(f);
    assert.doesNotMatch(out.js, /project-env-secret/);
    assert.doesNotMatch(out.js, /process-env-secret/);
  } finally {
    delete process.env.VITE_PROCESS_LEAK;
  }
  assert.deepEqual(executedMarkers(f), []);
});

test('project tsconfig and Babel configuration are never consulted', async () => {
  const f = fixture(SINGLE_PAGE);
  const out = await trustedBuild(f);
  assert.doesNotMatch(out.js, /hostile-jsx/);
  assert.deepEqual(executedMarkers(f), []);
});

test('CSS @config and @plugin cannot make trusted tooling load JavaScript', async () => {
  for (const [directive, marker] of [
    ['@config "../evil-config.js";', 'at-config'],
    ['@plugin "../evil-plugin.js";', 'at-plugin'],
    ['@CONFIG "../evil-config.js";', 'at-config'],
  ]) {
    const f = fixture(SINGLE_PAGE, { 'src/index.css': `${directive}\n${SINGLE_PAGE['src/index.css']}` });
    await rejectedBuild(f, /is not permitted in Nexus Builder CSS/);
    assert.equal(fs.existsSync(path.join(f.markers, marker)), false, directive);
    assert.deepEqual(executedMarkers(f), [], directive);
  }
});

test('CSS imports of local files are rejected; remote https imports pass through', async () => {
  const local = fixture(SINGLE_PAGE, {
    'src/index.css': `@import "./other.css";\n${SINGLE_PAGE['src/index.css']}`,
    'src/other.css': '@config "../evil-config.js";\n',
  });
  await rejectedBuild(local, /only remote https @import is permitted/);
  assert.deepEqual(executedMarkers(local), []);
  const remote = fixture(SINGLE_PAGE, {
    'src/index.css': `@import url('https://fonts.googleapis.com/css2?family=Inter');\n${SINGLE_PAGE['src/index.css']}`,
  });
  const out = await trustedBuild(remote);
  assert.match(out.css, /fonts\.googleapis\.com/);
});

test('CSS preprocessors are rejected before any preprocessor package is loaded', async () => {
  for (const [file, marker] of [
    ['src/style.scss', 'sass'],
    ['src/style.sass', 'sass'],
    ['src/style.less', 'less'],
    ['src/style.styl', 'less'],
  ]) {
    const f = fixture(SINGLE_PAGE, {
      [file]: 'body { color: red; }\n',
      'src/App.tsx': `import Home from './pages/Home'\nimport './${path.basename(file)}'\nexport default function App() {\n  return <Home />\n}\n`,
    });
    await rejectedBuild(f, /CSS preprocessors are not provided by the Nexus Builder runtime/);
    assert.equal(fs.existsSync(path.join(f.markers, marker)), false, file);
    assert.deepEqual(executedMarkers(f), [], file);
  }
});

test('unprovided bare imports never resolve from ancestor node_modules', async () => {
  for (const source of ['firebase', 'sass', '#x', 'react-dom/server']) {
    const f = fixture(SINGLE_PAGE, {
      'src/App.tsx': `import Home from './pages/Home'\nimport '${source}'\nexport default function App() {\n  return <Home />\n}\n`,
    });
    await rejectedBuild(f, new RegExp(`does not provide "${source.replace(/[.*+?^${}()|[\]\\/]/g, '\\$&')}"`));
    assert.deepEqual(executedMarkers(f), [], source);
  }
});

test('relative imports cannot escape the project', async () => {
  const f = fixture(SINGLE_PAGE, {
    'src/App.tsx':
      "import Home from './pages/Home'\nimport { secret } from '../../../../outside.js'\nexport default function App() {\n  return <Home title={secret} />\n}\n",
  });
  await rejectedBuild(f, /Nexus Builder runtime denied module "\.\.\/\.\.\/\.\.\/\.\.\/outside\.js"/);
});

// Other spellings of the canonical project root: a symlink (a junction on
// Windows) and, where the platform provides one, the 8.3 short-name form.
function aliases(f) {
  const link = path.join(f.base, 'alias');
  fs.symlinkSync(f.projectRoot, link, process.platform === 'win32' ? 'junction' : 'dir');
  const found = [link];
  const tmp = os.tmpdir();
  const short = path.join(tmp, path.relative(fs.realpathSync.native(tmp), f.projectRoot));
  if (short !== f.projectRoot) found.push(short);
  return found;
}

test('backend requests are strictly validated', () => {
  const f = fixture(SINGLE_PAGE);
  const good = f.request;
  for (const request of [
    null,
    [],
    'x',
    {},
    { projectRoot: good.projectRoot },
    { ...good, extra: 1 },
    { ...good, projectRoot: 'relative/react' },
    { ...good, projectRoot: `${good.projectRoot}${path.sep}..${path.sep}react` },
    { ...good, projectRoot: `${good.projectRoot}${path.sep}` },
    { ...good, cacheDir: path.join(good.projectRoot, 'cache') },
    { ...good, projectRoot: path.join(good.cacheDir, 'react') },
    { ...good, cacheDir: good.projectRoot },
    { ...good, projectRoot: `${good.projectRoot}\0` },
    { ...good, projectRoot: path.join(f.base, 'missing') },
    ...aliases(f).map((alias) => ({ ...good, projectRoot: alias })),
  ]) {
    assert.throws(() => trusted.createTrustedViteConfig(request, toolchainRoot), /invalid Builder runtime request/);
  }
});

test('module guard denies code outside the toolchain in this process', async () => {
  const f = fixture(SINGLE_PAGE);
  const outside = path.join(f.base, 'ancestor', 'node_modules', 'firebase', 'index.js');
  await assert.rejects(import(pathToFileURL(outside).href), (error) => error.code === MODULE_DENIED);
  const { createRequire } = await import('node:module');
  const require = createRequire(path.join(toolchainRoot, 'entry', 'trusted-config.mjs'));
  assert.throws(() => require(outside), (error) => error.code === MODULE_DENIED);
  assert.deepEqual(executedMarkers(f), []);
});

// ── P0-002C4C3: the served preview ───────────────────────────────────────

// The production Node arguments for a fixture (the backend builds the real
// ones): the permission model with reads of the toolchain, React and the
// runtime home/tmp/cache, writes of those three, and nothing else.
function permissionArguments(f, runtime) {
  return [
    '--permission',
    '--allow-addons',
    `--allow-fs-read=${toolchainRoot}`,
    `--allow-fs-read=${f.projectRoot}`,
    ...['home', 'tmp', 'vite-cache'].map((child) => `--allow-fs-read=${path.join(runtime, child)}`),
    ...['home', 'tmp', 'vite-cache'].map((child) => `--allow-fs-write=${path.join(runtime, child)}`),
  ];
}

function sealedEnvironment(runtime) {
  const home = path.join(runtime, 'home');
  const tmp = path.join(runtime, 'tmp');
  const env = process.platform === 'win32'
    ? { USERPROFILE: home, TEMP: tmp, TMP: tmp, SystemRoot: process.env.SystemRoot, windir: process.env.windir }
    : { HOME: home, TMPDIR: tmp };
  return env;
}

function runtimeFor(f) {
  const runtime = path.dirname(f.cacheDir);
  for (const child of ['home', 'tmp']) fs.mkdirSync(path.join(runtime, child), { recursive: true });
  return runtime;
}

function firstLine(stream) {
  return new Promise((resolve, reject) => {
    let text = '';
    const timer = setTimeout(() => reject(new Error(`no readiness line: ${text}`)), 60_000);
    stream.on('data', (chunk) => {
      text += chunk;
      const end = text.indexOf('\n');
      if (end >= 0) {
        clearTimeout(timer);
        resolve(text.slice(0, end));
      }
    });
    stream.on('end', () => {
      clearTimeout(timer);
      reject(new Error(`stdout ended before readiness: ${text}`));
    });
  });
}

function request(port, target, headers = {}, method = 'GET') {
  return new Promise((resolve, reject) => {
    const req = http.request(
      { host: '127.0.0.1', port, path: target, method, headers: { connection: 'close', ...headers }, setHost: false },
      (res) => {
        let body = '';
        res.setEncoding('utf8');
        res.on('data', (chunk) => (body += chunk));
        res.on('end', () => resolve({ status: res.statusCode, headers: res.headers, body }));
      },
    );
    req.on('error', reject);
    req.end();
  });
}

test('entry rejects an invalid request before serving', () => {
  const f = fixture(SINGLE_PAGE);
  const node = process.execPath;
  const entry = path.join(toolchainRoot, 'entry', 'nexus-builder.mjs');
  for (const bad of [
    '{"projectRoot":"relative"}',
    JSON.stringify(f.request),
    JSON.stringify({ ...f.request, probeToken: 'short' }),
    JSON.stringify({ ...f.request, probeToken: TOKEN.toUpperCase() }),
    JSON.stringify({ ...f.request, probeToken: TOKEN, extra: 1 }),
    '',
  ]) {
    const run = spawnSync(node, [entry, bad], { encoding: 'utf8', timeout: 60_000 });
    assert.equal(run.status, 64, `${bad}: ${run.stderr}`);
    assert.equal(run.stdout, '');
    assert.match(run.stderr, /invalid request/);
  }
  assert.deepEqual(executedMarkers(f), []);
});

test('entry serves the project on loopback under the permission model', async () => {
  const f = fixture(SINGLE_PAGE);
  const runtime = runtimeFor(f);
  const entry = path.join(toolchainRoot, 'entry', 'nexus-builder.mjs');
  const child = spawn(
    process.execPath,
    [...permissionArguments(f, runtime), entry, JSON.stringify({ ...f.request, probeToken: TOKEN })],
    { cwd: runtime, env: sealedEnvironment(runtime), stdio: ['ignore', 'pipe', 'pipe'] },
  );
  let stderr = '';
  child.stderr.on('data', (chunk) => (stderr += chunk));
  try {
    const line = await firstLine(child.stdout.setEncoding('utf8'));
    const record = JSON.parse(line);
    assert.equal(line, preview.readinessRecord(record.port).trimEnd());
    assert.deepEqual(Object.keys(record), ['event', 'host', 'port']);
    assert.equal(record.host, '127.0.0.1');
    const port = record.port;
    const host = `127.0.0.1:${port}`;
    const page = await request(port, '/', { host, accept: 'text/html' });
    assert.equal(page.status, 200, stderr);
    assert.equal(page.headers[preview.TOKEN_HEADER], TOKEN);
    assert.match(page.body, /<div id="root"><\/div>/);
    assert.equal(page.headers['access-control-allow-origin'], undefined);
    // Project modules and pre-bundled dependencies (Windows: after Vite's
    // network-drive question was answered without a process).
    const main = await request(port, '/src/main.tsx', { host });
    assert.equal(main.status, 200, stderr);
    assert.match(main.body, /createRoot/);
    const app = await request(port, '/src/App.tsx', { host });
    assert.equal(app.status, 200, stderr);
    // Project CSS through the Nexus-owned PostCSS/Tailwind (preflight ran).
    const css = await request(port, '/src/index.css', { host });
    assert.equal(css.status, 200, stderr);
    assert.match(css.body, /box-sizing/);
    // The gate on the real server.
    assert.equal((await request(port, '/', { host: 'evil.example' })).status, 403);
    assert.equal((await request(port, '/', { host, origin: 'http://evil.example' })).status, 403);
    assert.equal((await request(port, '/__open-in-editor?file=src/main.tsx', { host })).status, 403);
    assert.equal((await request(port, '/%5f%5fopen-in-editor?file=src/main.tsx', { host })).status, 403);
    assert.equal((await request(port, '/', { host }, 'POST')).status, 405);
    // Still serving: nothing above disturbed the preview.
    assert.equal((await request(port, '/', { host, accept: 'text/html' })).status, 200);
    assert.equal(child.exitCode, null, stderr);
    // No write reached the project or its ancestors; the cache was used.
    assert.equal(fs.existsSync(path.join(f.projectRoot, 'node_modules')), false);
    assert.ok(fs.readdirSync(f.cacheDir).length > 0);
    assert.deepEqual(executedMarkers(f), []);
  } finally {
    child.kill();
  }
});

test('readiness records are exactly one canonical line', () => {
  assert.equal(preview.readinessRecord(5173), '{"event":"ready","host":"127.0.0.1","port":5173}\n');
});

test('launch requests are strict', () => {
  const f = fixture(SINGLE_PAGE);
  const good = { ...f.request, probeToken: TOKEN };
  assert.deepEqual(preview.parseLaunchRequest(JSON.stringify(good)), {
    projectRoot: f.projectRoot,
    cacheDir: f.cacheDir,
    probeToken: TOKEN,
  });
  for (const bad of [
    '',
    'null',
    '[]',
    JSON.stringify(f.request),
    JSON.stringify({ ...good, extra: 1 }),
    JSON.stringify({ ...good, probeToken: 1 }),
    JSON.stringify({ ...good, probeToken: TOKEN.slice(1) }),
    JSON.stringify({ ...good, probeToken: `${TOKEN}0` }),
    JSON.stringify({ ...good, probeToken: TOKEN.replace('a', 'g') }),
    JSON.stringify({ ...good, projectRoot: 'relative' }),
  ]) {
    assert.throws(() => preview.parseLaunchRequest(bad), /invalid Builder runtime request/, bad);
  }
});

// A request object as Node's http server presents it, and a recording response.
function exchange(url, rawHeaders, method = 'GET') {
  const req = { url, method, rawHeaders };
  const res = {
    status: null,
    headers: {},
    body: '',
    writeHead(status, headers) {
      this.status = status;
      Object.assign(this.headers, headers);
    },
    setHeader(name, value) {
      this.headers[name] = value;
    },
    end(body) {
      this.body = body ?? '';
    },
  };
  const admitted = preview.admit(req, res, { port: 5173, token: TOKEN });
  return { admitted, res };
}

test('the Nexus gate admits only same-origin loopback reads', () => {
  const host = ['Host', '127.0.0.1:5173'];
  const ok = exchange('/src/main.tsx?import', host);
  assert.equal(ok.admitted, true);
  assert.equal(ok.res.headers[preview.TOKEN_HEADER], TOKEN);
  assert.equal(exchange('/', [...host, 'Origin', 'http://127.0.0.1:5173']).admitted, true);
  assert.equal(exchange('/', host, 'HEAD').admitted, true);
  for (const [url, headers, method, status] of [
    ['/', host, 'POST', 405],
    ['/', host, 'OPTIONS', 405],
    ['/', [], 'GET', 403],
    ['/', ['Host', 'localhost:5173'], 'GET', 403],
    ['/', ['Host', '127.0.0.1:5174'], 'GET', 403],
    ['/', ['Host', '127.0.0.1'], 'GET', 403],
    ['/', ['Host', 'evil.example'], 'GET', 403],
    ['/', [...host, ...host], 'GET', 403],
    ['/', [...host, 'Origin', 'http://evil.example'], 'GET', 403],
    ['/', [...host, 'Origin', 'null'], 'GET', 403],
    ['/', [...host, 'Origin', 'https://127.0.0.1:5173'], 'GET', 403],
    ['/', [...host, 'Origin', 'http://127.0.0.1:5173', 'Origin', 'http://127.0.0.1:5173'], 'GET', 403],
    ['http://evil.example/', host, 'GET', 400],
    ['//evil.example/', host, 'GET', 400],
    ['*', host, 'GET', 400],
    ['/%E0%A4%A', host, 'GET', 400],
    ['/__open-in-editor', host, 'GET', 403],
    ['/__open-in-editor?file=src/main.tsx', host, 'GET', 403],
    ['/__OPEN-IN-EDITOR?file=x', host, 'GET', 403],
    ['/%5f%5fopen-in-editor?file=x', host, 'GET', 403],
    ['/%255f%255fopen-in-editor?file=x', host, 'GET', 403],
    ['/__open%2Din%2Deditor?file=x', host, 'GET', 403],
    ['/src/../__open-in-editor?file=x', host, 'GET', 403],
    ['/x/%2e%2e/__open-in-editor', host, 'GET', 403],
  ]) {
    const { admitted, res } = exchange(url, headers, method);
    assert.equal(admitted, false, `${method} ${url} ${headers}`);
    assert.equal(res.status, status, `${method} ${url} ${headers}`);
    assert.equal(res.headers[preview.TOKEN_HEADER], undefined);
  }
  // Nothing is served before the server knows its own port.
  const early = preview.admit({ url: '/', method: 'GET', rawHeaders: ['Host', '127.0.0.1:0'] }, exchange('/', host).res, { port: 0, token: TOKEN });
  assert.equal(early, false);
});

// Runs `source` (an ES module) in a fresh Node under the permission model
// with read access to the toolchain only (no child-process permission).
function confined(source) {
  const run = spawnSync(
    process.execPath,
    ['--permission', `--allow-fs-read=${toolchainRoot}`, '--input-type=module', '-e', source],
    { encoding: 'utf8', timeout: 60_000, cwd: os.tmpdir() },
  );
  assert.equal(run.status, 0, run.stderr);
  return JSON.parse(run.stdout);
}

test('the process guard creates no process and answers only the network-drive question', () => {
  const guard = JSON.stringify(fromToolchain('entry', 'process-guard.mjs'));
  const results = confined(`
    const { installProcessGuard, PROCESS_DENIED } = await import(${guard});
    installProcessGuard();
    const childProcess = await import('node:child_process');
    const { spawn, execSync } = childProcess;
    const out = {};
    const code = (fn) => { try { fn(); return 'returned'; } catch (error) { return error.code; } };
    out.spawn = code(() => spawn('node'));
    out.namedSpawn = code(() => childProcess.default.spawn('node'));
    out.spawnSync = code(() => childProcess.spawnSync('node'));
    out.execSync = code(() => execSync('echo'));
    out.execFileSync = code(() => childProcess.execFileSync('node'));
    out.fork = code(() => childProcess.fork('x'));
    out.execNoCallback = code(() => childProcess.exec('net use'));
    out.netUse = await new Promise((resolve) => childProcess.exec('net use', { windowsHide: true }, (error, stdout, stderr) => resolve({ error: error?.code ?? null, stdout, stderr })));
    out.other = await new Promise((resolve) => childProcess.exec('whoami', (error) => resolve(error?.code ?? null)));
    out.execFile = await new Promise((resolve) => childProcess.execFile('net', ['use'], (error) => resolve(error?.code ?? null)));
    out.denied = PROCESS_DENIED;
    process.stdout.write(JSON.stringify(out));
  `);
  const denied = results.denied;
  assert.equal(denied, 'ERR_NEXUS_PROCESS_DENIED');
  for (const api of ['spawn', 'namedSpawn', 'spawnSync', 'execSync', 'execFileSync', 'fork', 'execNoCallback', 'other', 'execFile']) {
    assert.equal(results[api], denied, api);
  }
  // Vite's Windows probe: "no mapped network drives", without a process
  // (the native permission denial would be ERR_ACCESS_DENIED).
  assert.deepEqual(results.netUse, { error: null, stdout: '', stderr: '' });
});

test('existence probes answer false for denied locations and grant nothing', () => {
  const probe = JSON.stringify(fromToolchain('entry', 'fs-probe.mjs'));
  const outside = fs.realpathSync.native(fs.mkdtempSync(path.join(os.tmpdir(), 'nexus-c4c3-outside-')));
  fs.writeFileSync(path.join(outside, 'secret'), 'outside');
  const results = confined(`
    import fs from 'node:fs';
    const { installExistenceProbes } = await import(${probe});
    const code = (fn) => { try { return fn(); } catch (error) { return error.code; } };
    const outside = ${JSON.stringify(path.join(outside, 'secret'))};
    const before = code(() => fs.existsSync(outside));
    installExistenceProbes();
    const { existsSync } = await import('node:fs');
    process.stdout.write(JSON.stringify({
      before,
      after: code(() => fs.existsSync(outside)),
      named: code(() => existsSync(outside)),
      toolchain: fs.existsSync(${JSON.stringify(path.join(toolchainRoot, 'entry', 'fs-probe.mjs'))}),
      read: code(() => fs.readFileSync(outside, 'utf8')),
      stat: code(() => fs.statSync(outside)),
    }));
  `);
  assert.equal(results.after, false);
  assert.equal(results.named, false);
  assert.equal(results.toolchain, true);
  assert.equal(results.read, 'ERR_ACCESS_DENIED');
  assert.equal(results.stat, 'ERR_ACCESS_DENIED');
  // Before installation Node itself already reports no access (never true).
  assert.notEqual(results.before, true);
});

test('Windows drive letters are restored to their canonical spelling, nothing else', async () => {
  const { canonicalDrive } = await import(fromToolchain('entry', 'fs-probe.mjs'));
  assert.equal(canonicalDrive('c:/Users/x/src/index.css'), 'C:/Users/x/src/index.css');
  assert.equal(canonicalDrive('d:\\Temp\\a b\\x.tsx'), 'D:\\Temp\\a b\\x.tsx');
  for (const unchanged of ['C:/Users/x', 'C:\\x', '/home/x/c:/y', 'cc:/x', 'c:x', '\\\\?\\c:\\x', '', undefined, 7]) {
    assert.equal(canonicalDrive(unchanged), unchanged);
  }
  if (process.platform !== 'win32') return;
  // Native Windows: the permission model compares spellings exactly, so a
  // granted file named with a lower-case drive letter is denied until the
  // probe restores the canonical letter. Nothing outside is granted.
  const probe = JSON.stringify(fromToolchain('entry', 'fs-probe.mjs'));
  const granted = path.join(toolchainRoot, 'entry', 'fs-probe.mjs');
  const lower = granted[0].toLowerCase() + granted.slice(1).replaceAll('\\', '/');
  const outside = fs.realpathSync.native(fs.mkdtempSync(path.join(os.tmpdir(), 'nexus-c4c3-drive-')));
  const outsideLower = outside[0].toLowerCase() + outside.slice(1);
  const results = confined(`
    import fs from 'node:fs';
    const code = (fn) => { try { fn(); return 'ok'; } catch (error) { return error.code; } };
    const before = code(() => fs.statSync(${JSON.stringify(lower)}));
    const { installExistenceProbes } = await import(${probe});
    installExistenceProbes();
    process.stdout.write(JSON.stringify({
      before,
      after: code(() => fs.statSync(${JSON.stringify(lower)})),
      outside: code(() => fs.statSync(${JSON.stringify(outsideLower)})),
    }));
  `);
  assert.deepEqual(results, { before: 'ERR_ACCESS_DENIED', after: 'ok', outside: 'ERR_ACCESS_DENIED' });
});

test('the entry installs every guard before any third-party module is imported', () => {
  const entry = fs.readFileSync(path.join(toolchainRoot, 'entry', 'nexus-builder.mjs'), 'utf8');
  const statics = [...entry.matchAll(/^import .* from '([^']+)';$/gm)].map((m) => m[1]);
  assert.deepEqual(statics, ['node:path', 'node:url', './fs-probe.mjs', './module-guard.mjs', './process-guard.mjs']);
  const order = ['installModuleGuard(toolchainRoot);', 'installProcessGuard();', 'installExistenceProbes();', "await import('./preview-server.mjs')"]
    .map((needle) => entry.indexOf(needle));
  assert.ok(order.every((at) => at > 0), String(order));
  assert.deepEqual([...order].sort((a, b) => a - b), order);
  // The guard modules themselves import only Node builtins.
  for (const name of ['fs-probe.mjs', 'module-guard.mjs', 'process-guard.mjs']) {
    const source = fs.readFileSync(path.join(toolchainRoot, 'entry', name), 'utf8');
    for (const [, specifier] of source.matchAll(/^import .* from '([^']+)';$/gm)) {
      assert.match(specifier, /^node:/, `${name}: ${specifier}`);
    }
    assert.doesNotMatch(source, /import\(/, name);
  }
});
