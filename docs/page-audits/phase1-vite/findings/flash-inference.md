# Audit: Flash Inference
URL: http://localhost:1420/flash-inference
Audited at: 2026-04-09T19:23:45+01:00
Gate detected: false
Gate type: none

## Console (captured at 1920x1080, ALL messages)
### Errors
1. `[Nexus OS] Unhandled promise rejection: TypeError: Cannot read properties of undefined (reading 'transformCallback')` — `src/main.tsx:41:10`
   - Stack: `transformCallback` at `chunk-G7S6KQDI.js:22:37` -> `listen` at `@tauri-apps_api_event.js:38:14` -> `setup` at `src/pages/FlashInference.tsx:216:24` -> `src/pages/FlashInference.tsx:311:5`
   - (Fired twice — React StrictMode double-invoke)

### Warnings
1. `[FlashInference] Error: desktop runtime unavailable` — `src/pages/FlashInference.tsx:144:39`
   - Stack: `invokeDesktop` at `src/api/backend.ts:17:11` -> `flashDetectHardware` at `src/api/backend.ts:2206:10` -> `src/pages/FlashInference.tsx:141:5`
   - (Fired twice — React StrictMode double-invoke)

### Logs
none

### Info
1. `Download the React DevTools for a better development experience` — `chunk-NUMECXU6.js:21550:24`

### Debug
1. `[vite] connecting...` — `@vite/client:494:8`
2. `[vite] connected.` — `@vite/client:617:14`

## Overflow

### 1920x1080
- documentElement: scrollWidth=1888 clientWidth=1888 [OK]
- body: scrollWidth=1888 clientWidth=1888 [OK]
- main `main`: scrollWidth=1627 clientWidth=1627 [OK]
- other overflowing:
  - `div.living-background`: scrollWidth=2039 clientWidth=1888 [OVERFLOW +151px]
  - `section.holo-panel.holo-panel-mid`: scrollWidth=1768 clientWidth=1625 [OVERFLOW +143px]
  - `span.nexus-sidebar-item-text` (x3): scrollWidth=157 clientWidth=153 [OVERFLOW +4px]

### 1280x800 (simulated)
- documentElement: scrollWidth=1888 clientWidth=1888 [OK]
- body: scrollWidth=1280 clientWidth=1280 [OK]
- main `main`: scrollWidth=1019 clientWidth=1019 [OK]
- other overflowing:
  - `div.living-background`: scrollWidth=2039 clientWidth=1888 [OVERFLOW +151px]
  - `section.holo-panel`: scrollWidth=1106 clientWidth=1017 [OVERFLOW +89px]

### 1024x768 (simulated)
- documentElement: scrollWidth=1888 clientWidth=1888 [OK]
- body: scrollWidth=1024 clientWidth=1024 [OK]
- main `main`: scrollWidth=763 clientWidth=763 [OK]
- other overflowing:
  - `div.living-background`: scrollWidth=2039 clientWidth=1888 [OVERFLOW +151px]
  - `section.holo-panel`: scrollWidth=828 clientWidth=761 [OVERFLOW +67px]

## Interactive elements (main content only)
| # | Label | Type | Href | Enabled |
|---|-------|------|------|---------|
| 1 | + Load Model | button[submit] | — | yes |
| 2 | Run Benchmark | button[submit] | — | no |
| 3 | Export Report | button[submit] | — | no |
| 4 | (placeholder: "Load a model first...") | input[text] | — | no |
| 5 | Send | button[submit] | — | no |

## Click sequence
### Click 1: "+ Load Model"
- Pathname before: /flash-inference
- New console: clean (no new messages)
- Network failures: none
- Visible change: none — button click produced no modal, dropdown, or state change
- Pathname after: /flash-inference
- Reverted: n/a

### Click 2–5: skipped (disabled)
- "Run Benchmark" — disabled, not clickable
- "Export Report" — disabled, not clickable
- input "Load a model first..." — disabled, not interactive
- "Send" — disabled, not clickable

### Skipped (destructive)
none

### Total interactive elements found: 5
### Elements clicked: 1 (others disabled)

## Accessibility
- Images without alt: 0
- Inputs without label: 1 (selectors: `input[placeholder="Load a model first..."]`)
- Buttons without accessible name: 0

## Findings

### flash-inference-01
- SEVERITY: P1
- DIMENSION: console
- VIEWPORT: all
- EVIDENCE: `TypeError: Cannot read properties of undefined (reading 'transformCallback')` at `src/pages/FlashInference.tsx:216:24` via `@tauri-apps_api_event.js:38:14`. Fires as unhandled promise rejection caught by global handler at `src/main.tsx:41:10`. Tauri `listen()` call in `setup()` function crashes because the Tauri IPC bridge is undefined.
- IMPACT: Event listener for backend model-loading events fails to register; any Tauri->frontend push events on this page are silently lost.

### flash-inference-02
- SEVERITY: P2
- DIMENSION: console
- VIEWPORT: all
- EVIDENCE: `[FlashInference] Error: desktop runtime unavailable` warning at `src/pages/FlashInference.tsx:144:39`. `flashDetectHardware()` at `src/api/backend.ts:2206` calls `invokeDesktop()` which throws because the Tauri runtime is absent.
- IMPACT: Hardware detection (GPU/RAM/cores) fails silently; UI shows "0.0 GB | 0 cores" with no error feedback to the user explaining why hardware info is unavailable.

### flash-inference-03
- SEVERITY: P2
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: "+ Load Model" button (the only enabled element) click produces no visible response — no modal, no dropdown, no console output, no state change. `window.location.pathname` unchanged.
- IMPACT: The primary CTA for the page is a silent no-op in demo mode. Users have no way to load a model or progress beyond the empty state.

### flash-inference-04
- SEVERITY: P2
- DIMENSION: overflow
- VIEWPORT: all
- EVIDENCE: `section.holo-panel.holo-panel-mid` has `overflow: hidden` but `scrollWidth` exceeds `clientWidth` at all viewports: +143px at 1920x1080, +89px at 1280x800, +67px at 1024x768. Content is clipped, not scrollable.
- IMPACT: Panel content may be cut off at the right edge; users cannot scroll to see clipped content.

### flash-inference-05
- SEVERITY: P2
- DIMENSION: overflow
- VIEWPORT: all
- EVIDENCE: `div.living-background` overflows at all viewports: scrollWidth=2039 vs clientWidth=1888 (+151px). Element has `position: fixed` and covers the full viewport as a decorative background layer.
- IMPACT: Cosmetic — fixed-position decorative element extends beyond viewport. No user-visible scroll bar due to parent clipping, but indicates the background canvas is wider than the viewport.

### flash-inference-06
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: `input[placeholder="Load a model first..."]` has no `aria-label`, no associated `<label>`, and no `title` attribute. The placeholder text serves as the only label.
- IMPACT: Screen readers cannot identify the purpose of this text input. Placeholder text is not a reliable accessible name per WCAG 2.1 SC 1.3.1.

### flash-inference-07
- SEVERITY: P2
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: All 4 buttons in main content have `type="submit"` instead of `type="button"`. These buttons ("+ Load Model", "Run Benchmark", "Export Report", "Send") are not inside a `<form>` element but use submit type.
- IMPACT: If these buttons were ever placed inside a form, they would trigger unexpected form submission. Semantic mismatch — action buttons should use `type="button"`.

## Summary
- Gate detected: no
- Total interactive elements: 5
- Elements clicked: 1 (4 disabled)
- P0: 0
- P1: 1
- P2: 6
