// P0-002C4C3: the Nexus-owned loopback preview server. Vite runs in
// middleware mode (HMR and its WebSocket off) behind the Nexus gate, on a
// Nexus-owned http.Server bound only to 127.0.0.1 on an OS-assigned port.
import http from 'node:http';
import { createServer as createVite } from 'vite';
import { builderPaths, createTrustedViteConfig } from './trusted-config.mjs';

export const LOOPBACK = '127.0.0.1';
export const TOKEN_HEADER = 'x-nexus-preview-token';
const TOKEN = /^[0-9a-f]{32}$/;
const EDITOR = '__open-in-editor';

function invalidRequest() {
  return new Error('invalid Builder runtime request');
}

// The backend launch request: the canonical project root and Vite cache, and
// the per-launch token the backend's readiness probe expects back.
export function parseLaunchRequest(text) {
  let request;
  try {
    request = JSON.parse(text);
  } catch {
    throw invalidRequest();
  }
  if (request === null || typeof request !== 'object' || Array.isArray(request)) {
    throw invalidRequest();
  }
  if (Object.keys(request).sort().join(',') !== 'cacheDir,probeToken,projectRoot') {
    throw invalidRequest();
  }
  if (typeof request.probeToken !== 'string' || !TOKEN.test(request.probeToken)) {
    throw invalidRequest();
  }
  const { projectRoot, cacheDir } = builderPaths({
    projectRoot: request.projectRoot,
    cacheDir: request.cacheDir,
  });
  return { projectRoot, cacheDir, probeToken: request.probeToken };
}

// Exactly one readiness line, written only after the server is listening.
export function readinessRecord(port) {
  return `${JSON.stringify({ event: 'ready', host: LOOPBACK, port })}\n`;
}

function headerValues(req, name) {
  const values = [];
  for (let i = 0; i + 1 < req.rawHeaders.length; i += 2) {
    if (req.rawHeaders[i].toLowerCase() === name) values.push(req.rawHeaders[i + 1]);
  }
  return values;
}

// Raw and percent-decoded spellings of the request path (lower case), so an
// encoded or case-varied editor route is recognized; undecodable paths fail.
function pathSpellings(url) {
  let path = url.split(/[?#]/, 1)[0];
  const spellings = [path.toLowerCase()];
  for (let round = 0; round < 3; round += 1) {
    const decoded = decodeURIComponent(path);
    if (decoded === path) break;
    path = decoded;
    spellings.push(path.toLowerCase());
  }
  return spellings;
}

function deny(res, status) {
  res.writeHead(status, { 'Content-Type': 'text/plain', Connection: 'close' });
  res.end('denied by the Nexus preview gate\n');
  return false;
}

// Every request passes here before Vite. Only same-origin loopback GET/HEAD
// requests addressed to this exact server reach Vite; the editor-launch route
// never does.
export function admit(req, res, { port, token }) {
  const host = `${LOOPBACK}:${port}`;
  if (port === 0) return deny(res, 503);
  if (req.method !== 'GET' && req.method !== 'HEAD') return deny(res, 405);
  const hosts = headerValues(req, 'host');
  if (hosts.length !== 1 || hosts[0] !== host) return deny(res, 403);
  const origins = headerValues(req, 'origin');
  if (origins.length > 1 || (origins.length === 1 && origins[0] !== `http://${host}`)) {
    return deny(res, 403);
  }
  if (typeof req.url !== 'string' || !req.url.startsWith('/') || req.url.startsWith('//')) {
    return deny(res, 400);
  }
  let spellings;
  try {
    spellings = pathSpellings(req.url);
  } catch {
    return deny(res, 400);
  }
  if (spellings.some((spelling) => spelling.includes(EDITOR))) return deny(res, 403);
  res.setHeader(TOKEN_HEADER, token);
  return true;
}

export async function startPreview(request, toolchainRoot) {
  const vite = await createVite(
    createTrustedViteConfig(
      { projectRoot: request.projectRoot, cacheDir: request.cacheDir },
      toolchainRoot,
    ),
  );
  const state = { port: 0, token: request.probeToken };
  const server = http.createServer((req, res) => {
    if (admit(req, res, state)) vite.middlewares(req, res);
  });
  server.on('upgrade', (_req, socket) => socket.destroy());
  server.on('clientError', (_error, socket) => socket.destroy());
  await new Promise((resolve, reject) => {
    server.once('error', reject);
    server.listen({ port: 0, host: LOOPBACK, exclusive: true }, resolve);
  });
  const address = server.address();
  if (
    address === null ||
    typeof address !== 'object' ||
    address.address !== LOOPBACK ||
    address.family !== 'IPv4' ||
    !Number.isInteger(address.port) ||
    address.port < 1 ||
    address.port > 65535
  ) {
    server.close();
    throw new Error('preview server is not bound to loopback');
  }
  state.port = address.port;
  return { server, vite, port: address.port };
}
