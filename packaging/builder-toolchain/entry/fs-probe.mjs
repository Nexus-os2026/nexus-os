// P0-002C4C3: under the Node permission model a denied location is one this
// runtime can never read. Existence probes (Vite walks the project's ancestors
// looking for workspace markers) therefore answer `false` for it instead of
// throwing. Nothing is granted: every read or write there still fails with the
// native permission denial.
import fs from 'node:fs';
import { syncBuiltinESMExports } from 'node:module';

export function installExistenceProbes() {
  const existsSync = fs.existsSync;
  fs.existsSync = function existsSyncWithinPermissions(path) {
    try {
      return existsSync(path);
    } catch (error) {
      if (error?.code === 'ERR_ACCESS_DENIED') return false;
      throw error;
    }
  };
  syncBuiltinESMExports();
}
