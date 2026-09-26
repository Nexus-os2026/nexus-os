// P0-002C4D2/C4C3: Nexus-owned Builder frontend runtime entry, packaged at
// `entry/nexus-builder.mjs` inside the verified toolchain. The backend runs the
// verified packaged Node against this file under the Node permission model
// (never a project script) with one argument: the launch request.
//
// Before any third-party module is imported it confines module loading to the
// toolchain, denies process creation and makes existence probes respect the
// permission model. It then validates the request, serves the project on a
// loopback-only preview server and writes exactly one readiness line.
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { installExistenceProbes } from './fs-probe.mjs';
import { installModuleGuard } from './module-guard.mjs';
import { installProcessGuard } from './process-guard.mjs';

const toolchainRoot = path.dirname(path.dirname(fileURLToPath(import.meta.url)));
installModuleGuard(toolchainRoot);
installProcessGuard();
installExistenceProbes();

const { parseLaunchRequest, readinessRecord, startPreview } = await import('./preview-server.mjs');

export const USAGE = 64;
export const PREVIEW_FAILED = 70;

let request;
try {
  request = parseLaunchRequest(process.argv[2] ?? '');
} catch {
  process.stderr.write('Nexus Builder runtime: invalid request\n');
  process.exit(USAGE);
}
try {
  const { port } = await startPreview(request, toolchainRoot);
  process.stdout.write(readinessRecord(port));
} catch (error) {
  process.stderr.write(`Nexus Builder runtime: preview failed (${error?.code ?? 'error'})\n`);
  process.exit(PREVIEW_FAILED);
}
