// P0-002C4D2 trusted-entry tests. Run with the packaged Node against an
// assembled toolchain:
//   <toolchain>/node/node --test packaging/builder-toolchain/test/
// NEXUS_BUILDER_TOOLCHAIN_ROOT (test harness only) names the toolchain root;
// the default is the release staging location app/src-tauri/builder-toolchain.
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
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
  const base = fs.realpathSync(fs.mkdtempSync(path.join(os.tmpdir(), 'nexus-c4d2-')));
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
    await rejectedBuild(f, /does not provide/);
    assert.deepEqual(executedMarkers(f), [], source);
  }
});

test('relative imports cannot escape the project', async () => {
  const f = fixture(SINGLE_PAGE, {
    'src/App.tsx':
      "import Home from './pages/Home'\nimport { secret } from '../../../../outside.js'\nexport default function App() {\n  return <Home title={secret} />\n}\n",
  });
  await rejectedBuild(f, /Nexus Builder runtime denied module/);
});

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

test('entry validates the request and never launches', () => {
  const f = fixture(SINGLE_PAGE);
  const node = process.execPath;
  const entry = path.join(toolchainRoot, 'entry', 'nexus-builder.mjs');
  const good = spawnSync(node, [entry, JSON.stringify(f.request)], { encoding: 'utf8', timeout: 60_000 });
  assert.equal(good.status, 78, good.stderr);
  assert.match(good.stderr, /launch unavailable/);
  const bad = spawnSync(node, [entry, '{"projectRoot":"relative"}'], { encoding: 'utf8', timeout: 60_000 });
  assert.equal(bad.status, 64, bad.stderr);
  assert.deepEqual(executedMarkers(f), []);
});
