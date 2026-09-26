// P0-002C4D2: confines every module this Node process loads to the verified
// Nexus toolchain tree. Installed by the entry before any third-party module
// is imported, so ancestor `node_modules` directories, global folders,
// project files and packages installed elsewhere on the machine can never be
// resolved as code. Only `node:` builtins and files under the toolchain root
// load; everything else fails before evaluation.
import module from 'node:module';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

export const MODULE_DENIED = 'ERR_NEXUS_MODULE_DENIED';

let installedRoot = null;

export function installModuleGuard(toolchainRoot) {
  if (typeof toolchainRoot !== 'string' || !path.isAbsolute(toolchainRoot)) {
    throw new Error('module guard requires an absolute toolchain root');
  }
  const root = path.resolve(toolchainRoot);
  if (installedRoot !== null) {
    if (installedRoot !== root) throw new Error('module guard already installed');
    return;
  }
  const prefix = root.endsWith(path.sep) ? root : root + path.sep;
  module.registerHooks({
    resolve(specifier, context, nextResolve) {
      const resolved = nextResolve(specifier, context);
      if (resolved.url.startsWith('node:')) return resolved;
      if (resolved.url.startsWith('file:') && fileURLToPath(resolved.url).startsWith(prefix)) {
        return resolved;
      }
      const error = new Error(`module outside the Nexus toolchain denied: ${specifier}`);
      error.code = MODULE_DENIED;
      throw error;
    },
  });
  installedRoot = root;
}
