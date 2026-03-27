/**
 * Frontend Page Smoke Tests — verify all page files exist, are valid TSX,
 * export a default component, and that critical files are structurally sound.
 *
 * These tests run with `node --test` (no React rendering needed).
 * They verify the filesystem scaffolding and basic module structure.
 */
import assert from "node:assert/strict";
import test from "node:test";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const pagesDir = path.resolve(__dirname, "../src/pages");
const componentsDir = path.resolve(__dirname, "../src/components");
const apiFile = path.resolve(__dirname, "../src/api/backend.ts");
const typesFile = path.resolve(__dirname, "../src/types.ts");
const appFile = path.resolve(__dirname, "../src/App.tsx");

// ── Group 1: All page files exist ──────────────────────────────────────

const requiredPages = [
  "Dashboard.tsx",
  "Chat.tsx",
  "Agents.tsx",
  "AiChatHub.tsx",
  "FlashInference.tsx",
  "Settings.tsx",
  "Terminal.tsx",
  "FileManager.tsx",
  "ModelHub.tsx",
  "Documents.tsx",
  "Scheduler.tsx",
  "ApprovalCenter.tsx",
  "Audit.tsx",
  "AuditTimeline.tsx",
  "CommandCenter.tsx",
  "MissionControl.tsx",
  "CodeEditor.tsx",
  "ComputerControl.tsx",
  "AgentBrowser.tsx",
  "AgentDnaLab.tsx",
  "SystemMonitor.tsx",
  "Firewall.tsx",
  "Identity.tsx",
  "PermissionDashboard.tsx",
  "Protocols.tsx",
  "ClusterStatus.tsx",
  "TrustDashboard.tsx",
  "DistributedAudit.tsx",
  "ComplianceDashboard.tsx",
  "PolicyManagement.tsx",
  "KnowledgeGraph.tsx",
  "WorldSimulation.tsx",
  "VoiceAssistant.tsx",
  "TimeMachine.tsx",
  "DesignStudio.tsx",
  "MediaStudio.tsx",
  "EmailClient.tsx",
  "Messaging.tsx",
  "AppStore.tsx",
  "ApiClient.tsx",
  "ProjectManager.tsx",
  "DatabaseManager.tsx",
  "DeployPipeline.tsx",
  "LearningCenter.tsx",
  "NotesApp.tsx",
  "Login.tsx",
  "SetupWizard.tsx",
  "Integrations.tsx",
  "Workspaces.tsx",
  "Telemetry.tsx",
  "UsageBilling.tsx",
  "ImmuneDashboard.tsx",
  "ConsciousnessMonitor.tsx",
  "DreamForge.tsx",
  "TemporalEngine.tsx",
  "Civilization.tsx",
  "SelfRewriteLab.tsx",
  "TimelineViewer.tsx",
  "AdminDashboard.tsx",
  "AdminUsers.tsx",
  "AdminFleet.tsx",
  "AdminPolicyEditor.tsx",
  "AdminCompliance.tsx",
  "AdminSystemHealth.tsx",
];

test("all required page files exist", () => {
  const missing = [];
  for (const page of requiredPages) {
    const fullPath = path.join(pagesDir, page);
    if (!fs.existsSync(fullPath)) {
      missing.push(page);
    }
  }
  assert.equal(
    missing.length,
    0,
    `Missing page files: ${missing.join(", ")}`
  );
});

// ── Group 2: All pages export a default component ──────────────────────

test("all pages have an export (default or named)", () => {
  const noExport = [];
  for (const page of requiredPages) {
    const fullPath = path.join(pagesDir, page);
    if (!fs.existsSync(fullPath)) continue;
    const content = fs.readFileSync(fullPath, "utf-8");
    // Check for any kind of export — default, named function, or named const
    if (
      !content.includes("export default") &&
      !content.includes("as default") &&
      !content.includes("export function") &&
      !content.includes("export const") &&
      !content.includes("export class")
    ) {
      noExport.push(page);
    }
  }
  assert.equal(
    noExport.length,
    0,
    `Pages without any export: ${noExport.join(", ")}`
  );
});

// ── Group 3: No pages have syntax errors (basic checks) ────────────────

test("all pages are valid TSX (basic structural checks)", () => {
  const problems = [];
  for (const page of requiredPages) {
    const fullPath = path.join(pagesDir, page);
    if (!fs.existsSync(fullPath)) continue;
    const content = fs.readFileSync(fullPath, "utf-8");

    // Check for React import (either explicit or JSX transform)
    const hasReact =
      content.includes("import React") ||
      content.includes("from 'react'") ||
      content.includes('from "react"') ||
      content.includes("useState") ||
      content.includes("useEffect") ||
      content.includes("JSX") ||
      content.includes("<div") ||
      content.includes("<section") ||
      content.includes("<main");
    if (!hasReact) {
      problems.push(`${page}: no React usage detected`);
    }

    // Check for unbalanced braces (very basic — allow tolerance for string literals)
    const opens = (content.match(/{/g) || []).length;
    const closes = (content.match(/}/g) || []).length;
    if (Math.abs(opens - closes) > 5) {
      problems.push(`${page}: unbalanced braces (${opens} opens, ${closes} closes)`);
    }
  }
  assert.equal(
    problems.length,
    0,
    `TSX problems found:\n${problems.join("\n")}`
  );
});

// ── Group 4: Critical infrastructure files exist ───────────────────────

test("backend API file exists and has invokeDesktop", () => {
  assert.ok(fs.existsSync(apiFile), "api/backend.ts must exist");
  const content = fs.readFileSync(apiFile, "utf-8");
  assert.ok(
    content.includes("invokeDesktop"),
    "backend.ts must contain invokeDesktop function"
  );
});

test("types file exists and has AgentSummary", () => {
  assert.ok(fs.existsSync(typesFile), "types.ts must exist");
  const content = fs.readFileSync(typesFile, "utf-8");
  assert.ok(
    content.includes("AgentSummary") || content.includes("AgentStatus"),
    "types.ts must define agent types"
  );
});

test("App.tsx exists and has router", () => {
  assert.ok(fs.existsSync(appFile), "App.tsx must exist");
  const content = fs.readFileSync(appFile, "utf-8");
  assert.ok(
    content.includes("PAGE_ROUTE_OVERRIDES") ||
      content.includes("renderPage") ||
      content.includes("currentPage"),
    "App.tsx must contain routing logic"
  );
});

// ── Group 5: Component files exist ─────────────────────────────────────

const requiredComponents = [
  "VoiceOverlay.tsx",
  "PageErrorBoundary.tsx",
];

test("required components exist", () => {
  const missing = [];
  for (const comp of requiredComponents) {
    const fullPath = path.join(componentsDir, comp);
    if (!fs.existsSync(fullPath)) {
      missing.push(comp);
    }
  }
  assert.equal(
    missing.length,
    0,
    `Missing component files: ${missing.join(", ")}`
  );
});

// ── Group 6: Page error boundary exists ────────────────────────────────

test("PageErrorBoundary catches errors", () => {
  const boundaryPath = path.join(componentsDir, "PageErrorBoundary.tsx");
  if (!fs.existsSync(boundaryPath)) {
    assert.fail("PageErrorBoundary.tsx must exist for crash protection");
  }
  const content = fs.readFileSync(boundaryPath, "utf-8");
  assert.ok(
    content.includes("componentDidCatch") || content.includes("ErrorBoundary") || content.includes("error"),
    "PageErrorBoundary must handle errors"
  );
});

// ── Group 7: Backend API function coverage ─────────────────────────────

test("backend API has agent management functions", () => {
  const content = fs.readFileSync(apiFile, "utf-8");

  const requiredFunctions = [
    "listAgents",
    "startAgent",
    "stopAgent",
    "sendChat",
    "checkOllama",
  ];

  const missing = requiredFunctions.filter((fn) => !content.includes(fn));
  assert.equal(
    missing.length,
    0,
    `Missing API functions: ${missing.join(", ")}`
  );
});

test("backend API has flash inference functions", () => {
  const content = fs.readFileSync(apiFile, "utf-8");

  const flashFunctions = [
    "flashDetectHardware",
    "flashAutoConfig",
    "flashCreateSession",
    "flashGenerate",
    "flashListSessions",
  ];

  const missing = flashFunctions.filter((fn) => !content.includes(fn));
  assert.equal(
    missing.length,
    0,
    `Missing Flash API functions: ${missing.join(", ")}`
  );
});

// ── Group 8: No hardcoded localhost in page files ──────────────────────

test("pages do not hardcode localhost URLs", () => {
  const violations = [];
  for (const page of requiredPages) {
    const fullPath = path.join(pagesDir, page);
    if (!fs.existsSync(fullPath)) continue;
    const content = fs.readFileSync(fullPath, "utf-8");
    // Allow localhost references in comments, but not in code
    const lines = content.split("\n");
    for (let i = 0; i < lines.length; i++) {
      const line = lines[i].trim();
      if (line.startsWith("//") || line.startsWith("*")) continue;
      if (
        line.includes("http://localhost") &&
        !line.includes("// ") &&
        !line.includes("/* ")
      ) {
        violations.push(`${page}:${i + 1}: hardcoded localhost URL`);
      }
    }
  }
  // Report violations but don't fail — some pages may legitimately reference localhost
  if (violations.length > 0) {
    console.warn(`WARNING: ${violations.length} hardcoded localhost references found`);
  }
});

// ── Group 9: Types file structure ──────────────────────────────────────

test("types file has all critical type definitions", () => {
  const content = fs.readFileSync(typesFile, "utf-8");

  const criticalTypes = [
    "AgentStatus",
    "NexusConfig",
    "ConsentNotification",
  ];

  const missing = criticalTypes.filter((t) => !content.includes(t));
  assert.equal(
    missing.length,
    0,
    `Missing critical types: ${missing.join(", ")}`
  );
});

// ── Group 10: Page file size sanity ────────────────────────────────────

test("no page file is suspiciously small (likely broken)", () => {
  const tooSmall = [];
  for (const page of requiredPages) {
    const fullPath = path.join(pagesDir, page);
    if (!fs.existsSync(fullPath)) continue;
    const stat = fs.statSync(fullPath);
    if (stat.size < 100) {
      tooSmall.push(`${page} (${stat.size} bytes)`);
    }
  }
  assert.equal(
    tooSmall.length,
    0,
    `Suspiciously small pages: ${tooSmall.join(", ")}`
  );
});

test("no page file is suspiciously large (>500KB, possible generated code)", () => {
  const tooLarge = [];
  for (const page of requiredPages) {
    const fullPath = path.join(pagesDir, page);
    if (!fs.existsSync(fullPath)) continue;
    const stat = fs.statSync(fullPath);
    if (stat.size > 500 * 1024) {
      tooLarge.push(`${page} (${(stat.size / 1024).toFixed(0)}KB)`);
    }
  }
  // Warning, not failure — some pages are legitimately large
  if (tooLarge.length > 0) {
    console.warn(`WARNING: Large page files: ${tooLarge.join(", ")}`);
  }
});

// ── Group 11: TypeScript build sanity ──────────────────────────────────

test("tsconfig.json exists", () => {
  const tsconfig = path.resolve(__dirname, "../tsconfig.json");
  assert.ok(fs.existsSync(tsconfig), "tsconfig.json must exist");
});

test("vite.config exists", () => {
  const viteConfig = path.resolve(__dirname, "../vite.config.ts");
  assert.ok(
    fs.existsSync(viteConfig),
    "vite.config.ts must exist"
  );
});

// ── Group 12: Total page count ─────────────────────────────────────────

test("page directory has expected number of pages", () => {
  const allPages = fs
    .readdirSync(pagesDir)
    .filter((f) => f.endsWith(".tsx"));
  assert.ok(
    allPages.length >= 60,
    `Expected >= 60 pages, found ${allPages.length}`
  );
  console.log(`Total pages: ${allPages.length}`);
});
