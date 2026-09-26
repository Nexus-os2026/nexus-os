// P0-002C4C3: the Builder runtime never creates processes. Node runs without
// child-process permission, which denies them natively; this guard makes every
// child_process entry point fail the same way without an unhandled exception.
// APIs with a callback receive the denial asynchronously; all others throw.
// Installed by the entry before any third-party module is imported.
//
// One question is answered instead of denied, still without any process: on
// Windows Vite runs `net use` once to map network-drive realpaths back to
// drive letters, and until it has an answer it resolves paths through ancestor
// directories this runtime may not read. The backend grants only local drive
// paths (UNC and network-drive locations are rejected before launch), so the
// answer is always "no mapped network drives".
import childProcess from 'node:child_process';
import { syncBuiltinESMExports } from 'node:module';

export const PROCESS_DENIED = 'ERR_NEXUS_PROCESS_DENIED';
export const NETWORK_DRIVES_QUERY = 'net use';

function denied(api) {
  const error = new Error(`process creation denied: ${api}`);
  error.code = PROCESS_DENIED;
  return error;
}

export function installProcessGuard() {
  for (const api of ['spawn', 'spawnSync', 'execSync', 'execFileSync', 'fork']) {
    childProcess[api] = () => {
      throw denied(api);
    };
  }
  for (const api of ['exec', 'execFile']) {
    childProcess[api] = (...args) => {
      const callback = args.findLast((arg) => typeof arg === 'function');
      if (callback === undefined) throw denied(api);
      if (api === 'exec' && args[0] === NETWORK_DRIVES_QUERY) {
        process.nextTick(callback, null, '', '');
      } else {
        process.nextTick(callback, denied(api), '', '');
      }
      return undefined;
    };
  }
  syncBuiltinESMExports();
}
