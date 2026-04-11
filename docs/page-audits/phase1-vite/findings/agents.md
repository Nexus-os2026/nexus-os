# Audit: Agents
URL: http://localhost:1420/agents
Audited at: 2026-04-09T18:50:20+01:00
Gate detected: true
Gate type: RequiresLlm

## Console (captured at 1248x671 — viewport locked, see note under Overflow)
### Errors
1. `[TEST] standalone listen failed: TypeError: Cannot read properties of undefined (reading 'transformCallback')` — src/pages/Agents.tsx:204:95 (via @tauri-apps_api_event.js → chunk-G7S6KQDI.js:22:37). Fires twice (React StrictMode double-invoke).
### Warnings
none
### Logs
1. `[TEST] attaching standalone test listener` — src/pages/Agents.tsx:200:12 (fires twice, StrictMode)
### Info
1. `Download the React DevTools for a better development experience: https://reactjs.org/link/react-devtools` — chunk-NUMECXU6.js:21550:24
### Debug
1. `[vite] connecting...` — @vite/client:494:8
2. `[vite] connected.` — @vite/client:617:14

## Overflow

> **Note:** MCP `resize_window` does not change the JS-visible viewport (confirmed in prior audits — observations #106, #107). The viewport is locked at 1248x671 in the browser extension context. All three requested sizes measured identically at this locked viewport. Values below are the actual measurements.

### 1920x1080 (actual: 1248x671)
- documentElement: scrollWidth=1248 clientWidth=1248 [OK]
- body: scrollWidth=1248 clientWidth=1248 [OK]
- main `main.nexus-shell-content`: scrollWidth=987 clientWidth=987 [OK]
- other overflowing:
  - `div.living-background`: scrollWidth=1348 clientWidth=1248 [OVERFLOW +100px]
  - `section.holo-panel.holo-panel-mid.nexus-page-panel`: scrollWidth=1019 clientWidth=937 [OVERFLOW +82px]
  - `span.nexus-sidebar-item-text` (×3): scrollWidth=157 clientWidth=153 [OVERFLOW +4px each]

### 1280x800 (actual: 1248x671)
- (identical to above — viewport locked)

### 1024x768 (actual: 1248x671)
- (identical to above — viewport locked)

## Interactive elements (main content only)
| # | Label | Type | Href | Enabled |
|---|-------|------|------|---------|
| 1 | Refresh | button | — | yes |
| 2 | Start Jarvis | button | — | yes |
| 3 | Install Ollama (Free, Local, Private) | button | — | yes |
| 4 | I have an API key (OpenAI, Anthropic, etc.) | anchor | #/settings | yes |

## Click sequence
### Click 1: "Refresh"
- Pathname before: /agents
- New console: clean (no new messages)
- Network failures: none
- Visible change: none — silent no-op in demo mode
- Pathname after: /agents
- Reverted: n/a

### Click 2: "Start Jarvis"
- Pathname before: /agents
- New console: clean
- Network failures: none
- Visible change: none — silent no-op in demo mode
- Pathname after: /agents
- Reverted: n/a

### Click 3: "Install Ollama (Free, Local, Private)"
- Pathname before: /agents
- New console: clean
- Network failures: none
- Visible change: none — button has no onclick attribute, only `cursor-pointer` class; handler is presumably a React synthetic event that calls a Tauri command which is unavailable in demo mode
- Pathname after: /agents
- Reverted: n/a

### Click 4: "I have an API key (OpenAI, Anthropic, etc.)"
- Pathname before: /agents
- New console: clean
- Network failures: none
- Visible change: none — link href is `#/settings` which resolves to `http://localhost:1420/agents#/settings`. App uses path-based routing (react-router), not hash routing; the hash fragment is ignored by the router. No navigation occurs.
- Pathname after: /agents
- Reverted: n/a

### Skipped (destructive)
none

### Total interactive elements found: 4
### Elements clicked: 4

## Accessibility
- Images without alt: 0
- Inputs without label: 0
- Buttons without accessible name: 0

## Findings

### agents-01
- SEVERITY: P1
- DIMENSION: console
- VIEWPORT: all
- EVIDENCE: `[TEST] standalone listen failed: TypeError: Cannot read properties of undefined (reading 'transformCallback')` at src/pages/Agents.tsx:204:95. The error fires twice on page load (StrictMode double-invoke). The call chain is: Agents.tsx:202 → @tauri-apps_api_event.js `listen()` → chunk-G7S6KQDI.js `transformCallback()` which reads `window.__TAURI_IPC__` (undefined in demo mode).
- IMPACT: Tauri event listener setup crashes on every page load in demo/browser mode; the `[TEST]` prefix and hardcoded line suggest test-only instrumentation leaked into production code.

### agents-02
- SEVERITY: P1
- DIMENSION: gate
- VIEWPORT: all
- EVIDENCE: Link "I have an API key (OpenAI, Anthropic, etc.)" has `href="#/settings"` which resolves to `http://localhost:1420/agents#/settings`. The app uses `react-router` path-based routing. Clicking produces no navigation and no visible feedback. Same bug as ai-chat page (observation #126–#128).
- IMPACT: Users who have an API key cannot navigate to the settings page from the gate screen; the only CTA for existing API key holders is completely broken.

### agents-03
- SEVERITY: P2
- DIMENSION: action
- VIEWPORT: all
- EVIDENCE: "Refresh" button (header banner) produces no console output, no network request, and no visible change when clicked. No feedback of any kind.
- IMPACT: Button appears clickable but is a silent no-op in demo mode; users get no indication that the action is unavailable.

### agents-04
- SEVERITY: P2
- DIMENSION: action
- VIEWPORT: all
- EVIDENCE: "Start Jarvis" button (header banner) produces no console output, no network request, and no visible change when clicked.
- IMPACT: Primary CTA in the header is a silent no-op in demo mode with no user feedback.

### agents-05
- SEVERITY: P2
- DIMENSION: gate
- VIEWPORT: all
- EVIDENCE: "Install Ollama (Free, Local, Private)" button has no `onclick` attribute, only the CSS class `cursor-pointer`. The React synthetic event handler presumably calls a Tauri IPC command. Clicking produces no console output, no network request, no visible change, and no error.
- IMPACT: Gate CTA is non-functional in demo mode with no user feedback.

### agents-06
- SEVERITY: P2
- DIMENSION: overflow
- VIEWPORT: all (measured at 1248x671)
- EVIDENCE: `div.living-background` has scrollWidth=1348 vs clientWidth=1248 (+100px overflow). Element uses `position: fixed` which causes it to exceed the viewport boundary. Same root cause as dashboard and ai-chat pages.
- IMPACT: Phantom horizontal scroll potential; mitigated by parent overflow settings but indicates a layout sizing issue in the living-background component.

### agents-07
- SEVERITY: P2
- DIMENSION: overflow
- VIEWPORT: all (measured at 1248x671)
- EVIDENCE: `section.holo-panel.holo-panel-mid.nexus-page-panel` has scrollWidth=1019 vs clientWidth=937 (+82px overflow). The element is inside `main` and has `overflow: hidden` which clips the excess. The holo-panel is the gate screen container.
- IMPACT: Content inside the gate card exceeds its container width; clipped by `overflow: hidden` but may truncate content on narrower viewports.

### agents-08
- SEVERITY: P2
- DIMENSION: console
- VIEWPORT: all
- EVIDENCE: `[TEST] attaching standalone test listener` logged at src/pages/Agents.tsx:200:12, fires twice on page load. The `[TEST]` prefix indicates debug/test instrumentation.
- IMPACT: Test-only console.log statements are present in production code; pollutes browser console.

## Summary
- Gate detected: yes
- Total interactive elements: 4
- Elements clicked: 4
- P0: 0
- P1: 2
- P2: 6
