import assert from "node:assert/strict";
import test from "node:test";
import fs from "node:fs";

const requiredFiles = [
  "src/pages/Chat.tsx",
  "src/pages/Dashboard.tsx",
  "src/pages/Audit.tsx",
  "src/voice/PushToTalk.ts",
  "src-tauri/src/main.rs"
];

test("desktop scaffold files exist", () => {
  for (const file of requiredFiles) {
    assert.equal(fs.existsSync(new URL(`../${file}`, import.meta.url)), true);
  }
});
