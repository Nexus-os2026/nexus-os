/**
 * Phase One guards for the governed coding surface (node --test).
 *
 * - The desktop registers the official Tauri dialog plugin for backend-owned
 *   native dialogs; the plugin replaces the webview's `window.alert` and
 *   `window.confirm` with asynchronous IPC the webview may not call. No
 *   production frontend code may rely on those globals (a Promise is truthy,
 *   so a synchronous `if (confirm(...))` would stop waiting for the user).
 * - The governed coding page and its API wrappers never send a path, grant
 *   or approval to the backend.
 */
import assert from "node:assert/strict";
import test from "node:test";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const srcDir = path.resolve(__dirname, "../src");

function productionSources(dir, out = []) {
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const full = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      if (entry.name !== "__tests__" && entry.name !== "test") productionSources(full, out);
    } else if (/\.(ts|tsx)$/.test(entry.name) && !/\.test\.(ts|tsx)$/.test(entry.name)) {
      out.push(full);
    }
  }
  return out;
}

test("p1_g_frontend_never_relies_on_native_alert_or_confirm", () => {
  const files = productionSources(srcDir);
  assert.ok(files.length > 100, `expected the frontend sources, found ${files.length}`);
  const offenders = [];
  const pattern = /(^|[^.\w])(window\.)?(alert|confirm)\s*\(/;
  for (const file of files) {
    if (file.endsWith(path.join("lib", "ownerConfirm.tsx"))) continue;
    const lines = fs.readFileSync(file, "utf-8").split("\n");
    lines.forEach((line, i) => {
      const code = line.replace(/\/\/.*$/, "");
      if (pattern.test(code)) offenders.push(`${path.relative(srcDir, file)}:${i + 1}`);
    });
  }
  assert.deepEqual(offenders, [], `native alert/confirm used: ${offenders.join(", ")}`);
});

test("p1_g_governed_coding_sends_no_path_grant_or_approval", () => {
  const backend = fs.readFileSync(path.join(srcDir, "api", "backend.ts"), "utf-8");
  const calls = [...backend.matchAll(/invokeDesktop[^(]*\(\s*"(coding_[a-z_]+)"\s*(,\s*\{([^}]*)\})?/g)];
  const names = calls.map((m) => m[1]).sort();
  assert.deepEqual(names, [
    "coding_approve_apply",
    "coding_discard_run",
    "coding_list_local_models",
    "coding_list_projects",
    "coding_list_runs",
    "coding_restore_run",
    // Phase Two: governed sandboxed verification (reviewed).
    "coding_retry_verification_cleanup",
    "coding_select_project",
    "coding_start_run",
    "coding_start_verification",
    "coding_status",
    "coding_verification_profiles",
  ]);
  for (const [, name, , args = ""] of calls) {
    assert.doesNotMatch(args, /path|approv|confirm|grant|allow/i, `${name} sends ${args}`);
  }
  // Verification sends only the run id and a compiled-in profile name: never
  // a command, argument, executable, environment, sandbox or network setting.
  for (const [, name, , args = ""] of calls) {
    if (!/verification/.test(name)) continue;
    const sent = args.replace(/\.\.\.codingRunArgs\(runId\)/, "").split(",").map((a) => a.trim()).filter(Boolean);
    assert.ok(sent.every((a) => a === "profile"), `${name} sends ${args}`);
    assert.doesNotMatch(args, /cmd|command|argv|exec|env|sandbox|network|path/i, `${name} sends ${args}`);
  }
  // Every argument object in the Governed Coding API section, including
  // helpers, carries no path, approval or grant key.
  const start = backend.indexOf("// ── Governed Coding");
  assert.ok(start >= 0, "Governed Coding API section not found");
  const section = backend
    .slice(start)
    .replace(/\/\*[\s\S]*?\*\//g, "")
    .replace(/\/\/.*$/gm, "");
  // Argument objects are built in the section's function bodies (response
  // interfaces are display data and may name a changed file's path).
  const bodies = [...section.matchAll(/function\s+coding\w*\s*\([^)]*\)[^{]*\{([\s\S]*?)\n\}/g)]
    .map((m) => m[1])
    .join("\n");
  assert.ok(bodies.includes("coding_approve_apply"), "Governed Coding wrappers not found");
  const keys = [...bodies.matchAll(/([A-Za-z_$][\w$]*)\s*:/g)].map((m) => m[1]);
  const forbidden = keys.filter((k) => /path|approv|confirm|grant|allow/i.test(k));
  assert.deepEqual(forbidden, [], `forbidden keys: ${forbidden.join(", ")}`);
});
