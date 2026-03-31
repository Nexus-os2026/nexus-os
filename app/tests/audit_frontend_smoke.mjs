#!/usr/bin/env node

import fs from "node:fs/promises";
import path from "node:path";
import { pathToFileURL } from "node:url";
import * as esbuild from "esbuild";
import React from "react";
import { renderToString } from "react-dom/server";

const appRoot = process.cwd();
const srcPagesDir = path.join(appRoot, "src", "pages");
const appTsxPath = path.join(appRoot, "src", "App.tsx");
const outDir = path.join(appRoot, ".audit_tmp", "page-smoke");
const outReport = path.join(appRoot, "tests", "audit_frontend_smoke_results.json");

function shortError(error) {
  if (!error) return "unknown error";
  const text = error instanceof Error ? error.stack || error.message : String(error);
  return text.split("\n").slice(0, 6).join("\n");
}

function extractBalancedBlock(source, anchor, openChar, closeChar) {
  const anchorIndex = source.indexOf(anchor);
  if (anchorIndex === -1) return null;
  const openIndex = source.indexOf(openChar, anchorIndex);
  if (openIndex === -1) return null;
  let depth = 0;
  for (let i = openIndex; i < source.length; i += 1) {
    const ch = source[i];
    if (ch === openChar) depth += 1;
    if (ch === closeChar) {
      depth -= 1;
      if (depth === 0) {
        return source.slice(openIndex, i + 1);
      }
    }
  }
  return null;
}

function parsePageUnion(appSource) {
  const match = appSource.match(/type\s+Page\s*=\s*([^;]+);/s);
  if (!match) return [];
  return [...match[1].matchAll(/"([^"]+)"/g)].map((m) => m[1]);
}

function parseRouteOverrides(appSource) {
  const block = extractBalancedBlock(appSource, "const PAGE_ROUTE_OVERRIDES", "{", "}");
  if (!block) return {};
  const entries = {};
  for (const m of block.matchAll(/["']?([A-Za-z0-9_-]+)["']?\s*:\s*"([^"]+)"/g)) {
    entries[m[1]] = m[2];
  }
  return entries;
}

function parseNavItems(appSource) {
  const start = appSource.indexOf("const NAV_ITEMS");
  if (start === -1) return [];
  const end = appSource.indexOf("const PAGE_ROUTE_OVERRIDES", start);
  const block = end === -1 ? appSource.slice(start) : appSource.slice(start, end);
  const items = [];
  for (const objMatch of block.matchAll(/\{[\s\S]*?id:\s*"([^"]+)"[\s\S]*?\}/g)) {
    const text = objMatch[0];
    const idMatch = text.match(/id:\s*"([^"]+)"/);
    const labelMatch = text.match(/label:\s*"([^"]+)"/);
    const sectionMatch = text.match(/section:\s*"([^"]+)"/);
    if (!idMatch || !labelMatch) continue;
    items.push({
      id: idMatch[1],
      label: labelMatch[1],
      section: sectionMatch ? sectionMatch[1] : "CORE",
    });
  }
  return items;
}

function parseComponentToPageFile(appSource) {
  const map = new Map();

  for (const m of appSource.matchAll(/import\s+([A-Za-z0-9_]+)\s+from\s+"\.\/pages\/([^"]+)";/g)) {
    map.set(m[1], `${m[2]}.tsx`);
  }

  for (const m of appSource.matchAll(/import\s+\{([^}]+)\}\s+from\s+"\.\/pages\/([^"]+)";/g)) {
    const symbols = m[1]
      .split(",")
      .map((x) => x.trim())
      .filter(Boolean);
    for (const symbol of symbols) {
      const cleaned = symbol.replace(/^type\s+/, "").split(/\s+as\s+/)[0].trim();
      if (cleaned.length > 0) {
        map.set(cleaned, `${m[2]}.tsx`);
      }
    }
  }

  for (const m of appSource.matchAll(/const\s+([A-Za-z0-9_]+)\s*=\s*React\.lazy\(\(\)\s*=>\s*import\("\.\/pages\/([^"]+)"\)/g)) {
    map.set(m[1], `${m[2]}.tsx`);
  }

  return map;
}

function parseRenderMap(appSource) {
  const start = appSource.indexOf("function renderPage()");
  if (start === -1) return {};
  const end = appSource.indexOf("\n\n  return (", start);
  const renderFn = end === -1 ? appSource.slice(start) : appSource.slice(start, end);
  const lines = renderFn.split("\n");
  const map = {};
  for (let i = 0; i < lines.length; i += 1) {
    const line = lines[i];
    if (!line.includes("if (page ===")) continue;
    const pageIds = [...line.matchAll(/page\s*===\s*"([^"]+)"/g)].map((m) => m[1]);
    if (pageIds.length === 0) continue;

    let component = null;
    for (let j = i; j < Math.min(lines.length, i + 50); j += 1) {
      const candidate = lines[j];
      const direct = candidate.match(/return\s+<([A-Za-z0-9_]+)/);
      if (direct) {
        component = direct[1];
        break;
      }
      if (candidate.trim() === "return (") {
        for (let k = j + 1; k < Math.min(lines.length, j + 15); k += 1) {
          const nested = lines[k].match(/<([A-Za-z0-9_]+)/);
          if (nested) {
            component = nested[1];
            break;
          }
        }
        if (component) break;
      }
      if (j > i && candidate.includes("if (page ===")) {
        break;
      }
    }

    if (component) {
      for (const id of pageIds) {
        map[id] = component;
      }
    }
  }
  // Default branch in renderPage returns <Settings ... />
  map.settings = "Settings";
  // permissions route has a conditional fallback <div> before it returns <PermissionDashboard />
  map.permissions = "PermissionDashboard";
  return map;
}

function parseBackendImports(source) {
  const imports = [];
  for (const m of source.matchAll(/import\s+\{([^}]+)\}\s+from\s+["']\.\.\/api\/backend["']/g)) {
    const names = m[1]
      .split(",")
      .map((n) => n.trim())
      .filter(Boolean)
      .map((n) => n.replace(/^type\s+/, "").split(/\s+as\s+/)[0].trim())
      .filter(Boolean);
    imports.push(...names);
  }
  return [...new Set(imports)];
}

function seedPropsByFile(fileName) {
  const noop = () => {};
  const commonConfig = {
    llm: {
      default_model: "mock",
      anthropic_api_key: "",
      openai_api_key: "",
      deepseek_api_key: "",
      gemini_api_key: "",
      nvidia_api_key: "",
      ollama_url: "http://localhost:11434",
    },
    search: { brave_api_key: "" },
    social: {
      x_api_key: "",
      x_api_secret: "",
      x_access_token: "",
      x_access_secret: "",
      facebook_page_token: "",
      instagram_access_token: "",
    },
    messaging: {
      telegram_bot_token: "",
      whatsapp_business_id: "",
      whatsapp_api_token: "",
      discord_bot_token: "",
      slack_bot_token: "",
      matrix_access_token: "",
      matrix_homeserver: "",
      webhook_outbound_url: "",
      webhook_signing_secret: "",
    },
    voice: { whisper_model: "auto", wake_word: "hey nexus", tts_voice: "default" },
    privacy: { telemetry: false, audit_retention_days: 365 },
    governance: { enable_warden_review: false },
  };

  const map = {
    "Chat.tsx": {
      messages: [],
      draft: "",
      isRecording: false,
      isSending: false,
      agents: [],
      selectedAgent: "",
      selectedModel: "",
      onAgentChange: noop,
      onModelChange: noop,
      onDraftChange: noop,
      onSend: noop,
      onToggleMic: noop,
      onClearMessages: noop,
      onNavigate: noop,
    },
    "Agents.tsx": {
      agents: [],
      auditEvents: [],
      factoryTrigger: 0,
      onStart: noop,
      onPause: noop,
      onStop: noop,
      onCreate: noop,
      onDelete: noop,
      onClearAll: noop,
      onPermissions: noop,
      onNavigate: noop,
    },
    "Audit.tsx": {
      events: [],
      onRefresh: noop,
    },
    "AuditTimeline.tsx": {
      events: [],
    },
    "Identity.tsx": {
      agents: [],
    },
    "PermissionDashboard.tsx": {
      agentId: "",
      agentName: "Agent",
      fuelRemaining: 0,
      fuelBudget: 10000,
      memoryUsageBytes: 0,
      onBack: noop,
    },
    "MissionControl.tsx": {
      onNavigate: noop,
    },
    "Settings.tsx": {
      config: commonConfig,
      saving: false,
      onChange: noop,
      uiSoundEnabled: false,
      uiSoundVolume: 0.5,
      onUiSoundEnabledChange: noop,
      onUiSoundVolumeChange: noop,
      onSave: noop,
      ollamaConnected: false,
      ollamaModels: [],
      onDeleteModel: noop,
      onRerunSetup: noop,
      onRefreshOllama: noop,
    },
    "SetupWizard.tsx": {
      onDetectHardware: async () => ({
        gpu: "Mock GPU",
        vram_mb: 8192,
        ram_mb: 16384,
        detected_at: new Date().toISOString(),
        tier: "Medium (8-24GB VRAM)",
        recommended_primary: "qwen3.5:9b",
        recommended_fast: "qwen3.5:4b",
      }),
      onCheckOllama: async () => ({ connected: false, base_url: "http://localhost:11434", models: [] }),
      onEnsureOllama: async () => false,
      onIsOllamaInstalled: async () => false,
      onPullModel: async () => "success",
      onListAvailableModels: async () => [],
      onSetAgentModel: async () => undefined,
      onComplete: noop,
      onSkip: noop,
    },
  };
  return map[fileName] || {};
}

function resolveComponentExport(mod) {
  const pools = [];
  if (mod) pools.push(mod);
  if (mod && mod.default && typeof mod.default === "object") pools.push(mod.default);

  for (const pool of pools) {
    if (typeof pool === "function") {
      return { component: pool, exportName: "default" };
    }
    if (pool && typeof pool.default === "function") {
      return { component: pool.default, exportName: "default.default" };
    }
    for (const [key, value] of Object.entries(pool)) {
      if (typeof value === "function" && /^[A-Z]/.test(key)) {
        return { component: value, exportName: key };
      }
    }
  }
  return null;
}

function setupBrowserStubs() {
  const storage = {
    getItem: () => null,
    setItem: () => {},
    removeItem: () => {},
    clear: () => {},
  };
  const notification = class MockNotification {
    static permission = "denied";
    static requestPermission = async () => "denied";
  };
  const windowStub = {
    location: { pathname: "/", search: "", hash: "" },
    history: { replaceState: () => {}, pushState: () => {} },
    addEventListener: () => {},
    removeEventListener: () => {},
    localStorage: storage,
    sessionStorage: storage,
    matchMedia: () => ({ matches: false, addEventListener: () => {}, removeEventListener: () => {} }),
    requestAnimationFrame: (cb) => setTimeout(cb, 0),
    cancelAnimationFrame: (id) => clearTimeout(id),
    setTimeout,
    clearTimeout,
    setInterval,
    clearInterval,
    Notification: notification,
  };

  globalThis.window = windowStub;
  globalThis.localStorage = storage;
  globalThis.sessionStorage = storage;
  globalThis.navigator = { userAgent: "audit-node" };
  globalThis.document = {
    body: {},
    addEventListener: () => {},
    removeEventListener: () => {},
    createElement: () => ({ style: {} }),
    querySelector: () => null,
    getElementById: () => null,
  };
  globalThis.Notification = notification;
  globalThis.HTMLElement = class {};
  globalThis.ResizeObserver = class {
    observe() {}
    unobserve() {}
    disconnect() {}
  };
}

async function main() {
  await fs.mkdir(outDir, { recursive: true });
  setupBrowserStubs();

  const appSource = await fs.readFile(appTsxPath, "utf8");
  const routePageIds = parsePageUnion(appSource);
  const routeOverrides = parseRouteOverrides(appSource);
  const navItems = parseNavItems(appSource);
  const renderMap = parseRenderMap(appSource);
  const componentToPageFile = parseComponentToPageFile(appSource);

  const pageFiles = (await fs.readdir(srcPagesDir))
    .filter((name) => name.endsWith(".tsx"))
    .sort();

  const pageIdToRoute = Object.fromEntries(
    routePageIds.map((id) => [id, routeOverrides[id] || `/${id}`]),
  );

  const routeToPageIds = {};
  for (const [id, route] of Object.entries(pageIdToRoute)) {
    routeToPageIds[route] = routeToPageIds[route] || [];
    routeToPageIds[route].push(id);
  }

  const pageIdToComponent = {};
  const pageIdToFile = {};
  for (const pageId of routePageIds) {
    const component = renderMap[pageId] || "Settings";
    pageIdToComponent[pageId] = component;
    pageIdToFile[pageId] = componentToPageFile.get(component) || null;
  }

  const routedFiles = new Set(Object.values(pageIdToFile).filter(Boolean));

  const pageAnalyses = [];
  for (const fileName of pageFiles) {
    const absPath = path.join(srcPagesDir, fileName);
    const source = await fs.readFile(absPath, "utf8");
    const backendImports = parseBackendImports(source);
    const usesDirectTauriApi = /@tauri-apps\/api\//.test(source);
    const usesHasDesktopRuntime = /\bhasDesktopRuntime\s*\(/.test(source);
    const usesFetch = /\bfetch\s*\(/.test(source);
    const usesEnv = /import\.meta\.env|process\.env/.test(source);
    const usesLocalStorage = /\blocalStorage\b/.test(source);
    const usesSessionStorage = /\bsessionStorage\b/.test(source);
    const hasTodoFixme = /\bTODO\b|\bFIXME\b/.test(source);
    const hasDemoModeHints = /\bdemo\b|preview mode|mock mode|fallback/i.test(source);
    const hasPlaceholderHints = /coming soon|not implemented|placeholder/i.test(source);

    const outFile = path.join(outDir, `${fileName.replace(/\.tsx$/, "")}.cjs`);
    const smoke = {
      bundled: false,
      imported: false,
      rendered: false,
      exportResolvedAs: null,
      htmlLength: 0,
      buildWarnings: [],
      buildError: null,
      importError: null,
      renderError: null,
    };

    try {
      const result = await esbuild.build({
        entryPoints: [absPath],
        outfile: outFile,
        bundle: true,
        platform: "node",
        format: "cjs",
        logLevel: "silent",
        write: true,
        loader: {
          ".css": "text",
          ".svg": "text",
          ".png": "dataurl",
          ".jpg": "dataurl",
          ".jpeg": "dataurl",
          ".webp": "dataurl",
        },
        define: {
          "import.meta.env.DEV": "false",
          "import.meta.env.PROD": "true",
        },
        external: [
          "react",
          "react-dom",
          "react-dom/server",
          "react/jsx-runtime",
        ],
      });
      smoke.bundled = true;
      smoke.buildWarnings = result.warnings.map((w) => w.text).slice(0, 8);
    } catch (error) {
      smoke.buildError = shortError(error);
    }

    if (smoke.bundled) {
      try {
        const mod = await import(pathToFileURL(outFile).href);
        smoke.imported = true;
        const resolved = resolveComponentExport(mod);
        if (!resolved) {
          smoke.renderError = "No React component export could be resolved";
        } else {
          smoke.exportResolvedAs = resolved.exportName;
          try {
            const html = renderToString(React.createElement(resolved.component, seedPropsByFile(fileName)));
            smoke.rendered = true;
            smoke.htmlLength = html.length;
          } catch (error) {
            smoke.renderError = shortError(error);
          }
        }
      } catch (error) {
        smoke.importError = shortError(error);
      }
    }

    pageAnalyses.push({
      fileName,
      path: `src/pages/${fileName}`,
      routedByApp: routedFiles.has(fileName),
      backendImports,
      dependencyFlags: {
        usesBackendWrapper: backendImports.length > 0,
        usesDirectTauriApi,
        usesHasDesktopRuntime,
        usesFetch,
        usesEnv,
        usesLocalStorage,
        usesSessionStorage,
        hasTodoFixme,
        hasDemoModeHints,
        hasPlaceholderHints,
      },
      smoke,
    });
  }

  const report = {
    generatedAt: new Date().toISOString(),
    appRoot,
    summary: {
      totalPageFiles: pageFiles.length,
      totalRoutePageIds: routePageIds.length,
      totalNavItems: navItems.length,
      totalUniqueRoutes: Object.keys(routeToPageIds).length,
      routedFilesCount: routedFiles.size,
      unroutedFiles: pageFiles.filter((f) => !routedFiles.has(f)),
    },
    routing: {
      pageIds: routePageIds,
      navItems,
      pageIdToRoute,
      routeToPageIds,
      pageIdToComponent,
      pageIdToFile,
      hiddenRoutePageIds: routePageIds.filter((id) => !navItems.some((n) => n.id === id)),
      navItemsWithoutRoute: navItems.filter((n) => !routePageIds.includes(n.id)).map((n) => n.id),
      duplicateRoutes: Object.entries(routeToPageIds).filter(([, ids]) => ids.length > 1),
      rootRouteAlias: { "/": "mission-control", "/index.html": "mission-control" },
    },
    pages: pageAnalyses,
  };

  await fs.writeFile(outReport, JSON.stringify(report, null, 2));
  // eslint-disable-next-line no-console
  console.log(`Wrote ${outReport}`);
}

main().catch((error) => {
  // eslint-disable-next-line no-console
  console.error(error);
  process.exit(1);
});
