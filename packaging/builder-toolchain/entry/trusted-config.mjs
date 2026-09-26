// P0-002C4D2: Nexus-owned programmatic Vite configuration for Builder
// projects. Every location comes from the backend request; the project
// contributes only content (index.html, src/**, public/**). No project Vite,
// PostCSS, Tailwind or Babel configuration, package metadata, script,
// environment file, tsconfig or dependency is ever loaded or trusted.
import fs from 'node:fs';
import path from 'node:path';
import react from '@vitejs/plugin-react';
import postcss from 'postcss';
import tailwindcss from 'tailwindcss';
import { nexusTailwindConfig } from './tailwind-theme.mjs';

// The only bare imports project code may use; all resolve inside the toolchain.
export const RUNTIME_IMPORTS = Object.freeze([
  'react',
  'react/jsx-runtime',
  'react/jsx-dev-runtime',
  'react-dom',
  'react-dom/client',
  'react-router-dom',
]);

// Bare ids Vite itself injects into the HTML entry; they resolve to Vite's own
// virtual modules and are confined like any other resolution.
const VITE_INJECTED = Object.freeze(['vite/modulepreload-polyfill']);

// Only variables with this prefix reach client code. The sealed launch
// environment never defines it, and no .env file is ever read.
export const ENV_PREFIX = 'NEXUS_BUILDER_PUBLIC_';

const CSS_REQUEST = /\.(?:css|less|sass|scss|styl|stylus|pcss|postcss|sss)(?:$|\?)/;
const PREPROCESSED = /\.(?:less|sass|scss|styl|stylus|sss)(?:$|\?)/;
const REMOTE_IMPORT = /^(?:url\(\s*)?["']?https:\/\//i;

function invalidRequest() {
  return new Error('invalid Builder runtime request');
}

// Canonical existing locations only: the one spelling every containment
// check compares (no symlinked, 8.3 short-name or other alias spellings).
function absolutePath(value) {
  if (
    typeof value !== 'string' ||
    value.includes('\0') ||
    !path.isAbsolute(value) ||
    path.resolve(value) !== value ||
    // Verbatim and UNC spellings are rejected; the backend passes plain paths.
    (process.platform === 'win32' && value.startsWith('\\\\')) ||
    canonical(value) !== value
  ) {
    throw invalidRequest();
  }
  return value;
}

function canonical(value) {
  try {
    return fs.realpathSync.native(value);
  } catch {
    return null;
  }
}

export function isInside(file, root) {
  const relative = path.relative(root, file);
  return (
    relative === '' ||
    (relative !== '..' && !relative.startsWith(`..${path.sep}`) && !path.isAbsolute(relative))
  );
}

// The backend request: exactly two canonical, disjoint locations.
export function builderPaths(request) {
  if (request === null || typeof request !== 'object' || Array.isArray(request)) {
    throw invalidRequest();
  }
  if (Object.keys(request).sort().join(',') !== 'cacheDir,projectRoot') throw invalidRequest();
  const projectRoot = absolutePath(request.projectRoot);
  const cacheDir = absolutePath(request.cacheDir);
  if (isInside(projectRoot, cacheDir) || isInside(cacheDir, projectRoot)) throw invalidRequest();
  return { projectRoot, cacheDir };
}

function cleanId(id) {
  const query = id.indexOf('?');
  return query === -1 ? id : id.slice(0, query);
}

function isBare(source) {
  return !/^(?:[./\\\0]|[A-Za-z][A-Za-z0-9+.-]*:)/.test(source);
}

function filesystemId(id) {
  if (!id.startsWith('/@fs/')) return id;
  const rest = id.slice('/@fs/'.length);
  return /^[A-Za-z]:/.test(rest) ? rest : `/${rest}`;
}

// Resolution guard. Bare imports from project code must be Nexus runtime
// packages and resolve from the toolchain, never from ancestor node_modules;
// every resolved file must lie in the project, the toolchain's node_modules or
// the Nexus cache. Preprocessed CSS and project CSS that could make trusted
// tooling import files or load JavaScript are rejected before Vite sees them.
function nexusBuilderGuard({ toolchainRoot, projectRoot, cacheDir }) {
  const toolchainImporter = path.join(toolchainRoot, 'entry', 'trusted-config.mjs');
  const roots = [projectRoot, path.join(toolchainRoot, 'node_modules'), cacheDir];
  const confine = (context, resolved, source) => {
    if (!resolved) return resolved;
    const id = cleanId(resolved.id);
    if (id.startsWith('\0') || (id.startsWith('/@') && !id.startsWith('/@fs/'))) return resolved;
    const file = filesystemId(id);
    if (!resolved.external && path.isAbsolute(file) && roots.some((root) => isInside(file, root))) {
      return resolved;
    }
    return context.error(`Nexus Builder runtime denied module "${source}"`);
  };
  return {
    name: 'nexus:builder-guard',
    enforce: 'pre',
    async resolveId(source, importer, options) {
      if (source.startsWith('\0')) return null;
      const fromProject = importer === undefined || isInside(cleanId(importer), projectRoot);
      if (fromProject && isBare(source) && !VITE_INJECTED.includes(source)) {
        if (!RUNTIME_IMPORTS.includes(source)) {
          return this.error(`Nexus Builder runtime does not provide "${source}"`);
        }
        const resolved = await this.resolve(source, toolchainImporter, { ...options, skipSelf: true });
        return confine(this, resolved, source);
      }
      const resolved = await this.resolve(source, importer, { ...options, skipSelf: true });
      return confine(this, resolved, source);
    },
    transform(code, id) {
      if (!CSS_REQUEST.test(id)) return null;
      if (PREPROCESSED.test(id)) {
        return this.error('CSS preprocessors are not provided by the Nexus Builder runtime');
      }
      postcss.parse(code, { from: cleanId(id) }).walkAtRules((rule) => {
        const name = rule.name.toLowerCase();
        if (name === 'config' || name === 'plugin') {
          throw rule.error(`@${name} is not permitted in Nexus Builder CSS`);
        }
        if (name === 'import' && !REMOTE_IMPORT.test(rule.params.trim())) {
          throw rule.error('only remote https @import is permitted in Nexus Builder CSS');
        }
      });
      return null;
    },
  };
}

// Defense in depth inside the PostCSS pipeline, ahead of Tailwind: no
// directive can make Tailwind load a configuration or plugin module.
function nexusCssGuard() {
  return {
    postcssPlugin: 'nexus-css-guard',
    Once(root) {
      root.walkAtRules((rule) => {
        const name = rule.name.toLowerCase();
        if (name === 'config' || name === 'plugin') {
          throw rule.error(`@${name} is not permitted in Nexus Builder CSS`);
        }
      });
    },
  };
}
nexusCssGuard.postcss = true;

export function createTrustedViteConfig(request, toolchainRoot) {
  const { projectRoot, cacheDir } = builderPaths(request);
  const root = absolutePath(toolchainRoot);
  return {
    configFile: false,
    envDir: false,
    envPrefix: ENV_PREFIX,
    root: projectRoot,
    base: '/',
    mode: 'development',
    publicDir: path.join(projectRoot, 'public'),
    cacheDir,
    appType: 'spa',
    clearScreen: false,
    devtools: false,
    tsconfig: path.join(root, 'entry', 'tsconfig.json'),
    plugins: [
      nexusBuilderGuard({ toolchainRoot: root, projectRoot, cacheDir }),
      react({ jsxRuntime: 'automatic', jsxImportSource: 'react' }),
    ],
    css: {
      transformer: 'postcss',
      // An inline object: Vite never searches for a PostCSS config file.
      postcss: { plugins: [nexusCssGuard(), tailwindcss(nexusTailwindConfig(projectRoot))] },
      modules: false,
      devSourcemap: false,
    },
    optimizeDeps: { noDiscovery: true, entries: [], include: [...RUNTIME_IMPORTS] },
    server: { open: false, fs: { strict: true, allow: [projectRoot, cacheDir] } },
  };
}
