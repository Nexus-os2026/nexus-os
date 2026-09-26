// P0-002C4D2: Nexus-owned Builder frontend runtime entry, packaged at
// `entry/nexus-builder.mjs` inside the verified toolchain. A future launch
// (C4C3) runs the packaged Node against this file, never a project script.
//
// This checkpoint only confines module loading to the toolchain, validates
// the backend request and builds the trusted Vite configuration. It never
// starts a server, opens a port or launches any process.
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { installModuleGuard } from './module-guard.mjs';

const toolchainRoot = path.dirname(path.dirname(fileURLToPath(import.meta.url)));
installModuleGuard(toolchainRoot);

const { createTrustedViteConfig } = await import('./trusted-config.mjs');

export const USAGE = 64;
export const LAUNCH_UNAVAILABLE = 78;

let request;
try {
  request = JSON.parse(process.argv[2] ?? '');
  createTrustedViteConfig(request, toolchainRoot);
} catch {
  process.stderr.write('Nexus Builder runtime: invalid request\n');
  process.exit(USAGE);
}
process.stderr.write('Nexus Builder runtime: launch unavailable\n');
process.exit(LAUNCH_UNAVAILABLE);
