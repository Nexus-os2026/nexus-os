/**
 * Page module integrity tests — verify all page files are non-empty,
 * contain exports, and the page directory has expected count.
 *
 * Runs with `node --test` (no React rendering needed).
 */
import assert from "node:assert/strict";
import test from "node:test";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const pagesDir = path.resolve(__dirname, "../src/pages");
const componentsDir = path.resolve(__dirname, "../src/components");

const pages = fs
  .readdirSync(pagesDir)
  .filter((f) => f.endsWith(".tsx") && f !== "commandCenterUi.tsx");

test("All page files exist and are non-empty", () => {
  assert.ok(pages.length > 75, `Expected > 75 pages, found ${pages.length}`);

  const problems = [];
  for (const file of pages) {
    const content = fs.readFileSync(path.join(pagesDir, file), "utf-8");
    if (content.length <= 100) {
      problems.push(`${file}: only ${content.length} bytes (suspiciously small)`);
    }
    if (
      !content.includes("export default") &&
      !content.includes("export function") &&
      !content.includes("export const") &&
      !content.includes("as default")
    ) {
      problems.push(`${file}: no export found`);
    }
  }

  assert.equal(
    problems.length,
    0,
    `Page module problems:\n${problems.join("\n")}`
  );
});

test("No orphan component files (all imported somewhere)", () => {
  const allComponents = [];
  function walk(dir) {
    for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
      const full = path.join(dir, entry.name);
      if (entry.isDirectory()) {
        walk(full);
      } else if (entry.name.endsWith(".tsx") || entry.name.endsWith(".ts")) {
        allComponents.push(full);
      }
    }
  }
  walk(componentsDir);

  const srcDir = path.resolve(__dirname, "../src");
  const orphans = [];

  for (const comp of allComponents) {
    const name = path.basename(comp).replace(/\.(tsx|ts)$/, "");
    // Skip barrel index files
    if (name === "index") continue;

    // Search for references in all .tsx/.ts files under src/
    let found = false;
    function searchDir(dir) {
      if (found) return;
      for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
        if (found) return;
        const full = path.join(dir, entry.name);
        if (entry.isDirectory()) {
          searchDir(full);
        } else if (
          (entry.name.endsWith(".tsx") || entry.name.endsWith(".ts")) &&
          full !== comp
        ) {
          const content = fs.readFileSync(full, "utf-8");
          if (content.includes(name)) {
            found = true;
          }
        }
      }
    }
    searchDir(srcDir);

    if (!found) {
      orphans.push(path.relative(componentsDir, comp));
    }
  }

  assert.equal(
    orphans.length,
    0,
    `Orphan components found:\n${orphans.join("\n")}`
  );
});

test("Backend API file exists and has expected export count", () => {
  const backendPath = path.resolve(__dirname, "../src/api/backend.ts");
  assert.ok(fs.existsSync(backendPath), "backend.ts must exist");
  const content = fs.readFileSync(backendPath, "utf-8");

  const exportCount = (content.match(/^export /gm) || []).length;
  assert.ok(
    exportCount > 600,
    `Expected > 600 exports, found ${exportCount}`
  );
});
