// P0-002C4C3: under the Node permission model a denied location is one this
// runtime can never read. Existence probes (Vite walks the project's ancestors
// looking for workspace markers) therefore answer `false` for it instead of
// throwing. Nothing is granted: every read or write there still fails with the
// native permission denial.
//
// Windows drive letters are case-insensitive, but the permission model
// compares path spellings exactly. Tailwind derives the paths it stats through
// `url.parse`, which lower-cases the drive letter; those are stat-ed with the
// canonical upper-case letter. No other part of a path changes.
import fs from 'node:fs';
import { syncBuiltinESMExports } from 'node:module';

const LOWER_DRIVE = /^[a-z]:[\\/]/;

export function canonicalDrive(path) {
  return typeof path === 'string' && LOWER_DRIVE.test(path)
    ? path[0].toUpperCase() + path.slice(1)
    : path;
}

export function installExistenceProbes(platform = process.platform) {
  const spell = platform === 'win32' ? canonicalDrive : (path) => path;
  const existsSync = fs.existsSync;
  fs.existsSync = function existsSyncWithinPermissions(path) {
    try {
      return existsSync(spell(path));
    } catch (error) {
      if (error?.code === 'ERR_ACCESS_DENIED') return false;
      throw error;
    }
  };
  if (platform === 'win32') {
    const statSync = fs.statSync;
    fs.statSync = function statSyncWithCanonicalDrive(path, options) {
      return statSync(canonicalDrive(path), options);
    };
  }
  syncBuiltinESMExports();
}
