# Audit: Models
URL: http://localhost:1420/models
Audited at: 2026-04-09T19:19:00+01:00
Gate detected: false
Gate type: none

## Console (captured at 1920x1080, ALL messages)
### Errors
1. `[ModelHub] failed to load installed models Error: desktop runtime unavailable` — ModelHub.tsx:122:39 (x2, React StrictMode double-invoke)
   - Stack: invokeDesktop (backend.ts:17) -> listLocalModels (backend.ts:604) -> ModelHub.tsx:111 -> ModelHub.tsx:136
2. `[Nexus OS] Unhandled promise rejection: TypeError: Cannot read properties of undefined (reading 'transformCallback')` — main.tsx:41:10, origin ModelHub.tsx:238 (x2)
   - Stack: transformCallback (chunk-G7S6KQDI.js:22) -> Module.listen (@tauri-apps_api_event.js:38) -> ModelHub.tsx:238
3. `[Nexus OS] Unhandled promise rejection: TypeError: Cannot read properties of undefined (reading 'transformCallback')` — main.tsx:41:10, origin ModelHub.tsx:262 (x2)
   - Stack: transformCallback (chunk-G7S6KQDI.js:22) -> Module.listen (@tauri-apps_api_event.js:38) -> ModelHub.tsx:262

### Warnings
none

### Logs
none

### Info
1. `Download the React DevTools for a better development experience: https://reactjs.org/link/react-devtools` — chunk-NUMECXU6.js:21550:24

### Debug
1. `[vite] connecting...` — @vite/client:494:8
2. `[vite] connected.` — @vite/client:617:14

## Overflow

### 1920x1080
(actual viewport: 1888x895)
- documentElement: scrollWidth=1888 clientWidth=1888 [OK]
- body: scrollWidth=1888 clientWidth=1888 [OK]
- main `main.nexus-shell-content`: scrollWidth=1627 clientWidth=1627 [OK]
- other overflowing:
  - `div.living-background`: scrollWidth=2036 clientWidth=1888 [OVERFLOW +148px, position:fixed decorative]
  - `section.holo-panel.holo-panel-mid`: scrollWidth=1604 clientWidth=1577 [OVERFLOW +27px, overflow-x:hidden clips content]
  - 3x `span.nexus-sidebar-item-text`: scrollWidth=157 clientWidth=153 [OVERFLOW +4px, sidebar text truncation]

### 1280x800
(actual viewport: 1248x615)
- documentElement: scrollWidth=1248 clientWidth=1248 [OK]
- body: scrollWidth=1248 clientWidth=1248 [OK]
- main `main.nexus-shell-content`: scrollWidth=987 clientWidth=987 [OK]
- other overflowing:
  - `div.living-background`: scrollWidth=1344 clientWidth=1248 [OVERFLOW +96px, position:fixed decorative]
  - 3x `span.nexus-sidebar-item-text`: scrollWidth=157 clientWidth=153 [OVERFLOW +4px]

### 1024x768
(actual viewport: 992x583)
- documentElement: scrollWidth=992 clientWidth=992 [OK]
- body: scrollWidth=992 clientWidth=992 [OK]
- main `main.nexus-shell-content`: scrollWidth=731 clientWidth=731 [OK]
- other overflowing:
  - `div.living-background`: scrollWidth=1068 clientWidth=992 [OVERFLOW +76px, position:fixed decorative]
  - `section.holo-panel.holo-panel-mid`: scrollWidth=944 clientWidth=681 [OVERFLOW +263px, overflow-x:hidden clips content]
  - 3x `span.nexus-sidebar-item-text`: scrollWidth=157 clientWidth=153 [OVERFLOW +4px]

## Interactive elements (main content only)
| # | Label | Type | Href | Enabled |
|---|-------|------|------|---------|
| 1 | Search GGUF models on HuggingFace... | input[text] | — | yes |
| 2 | LLaMA | button | — | yes |
| 3 | Mistral | button | — | yes |
| 4 | Phi | button | — | yes |
| 5 | Gemma | button | — | yes |
| 6 | CodeLlama | button | — | yes |
| 7 | All | button | — | yes |
| 8 | Enable Sharing | button | — | yes |
| 9 | Disable Sharing | button | — | yes |
| 10 | Peer address (e.g. 192.168.1.100:9090) | input[text] | — | yes |
| 11 | Peer name (e.g. Office Desktop) | input[text] | — | yes |
| 12 | Add Peer | button | — | yes |
| 13 | Peer address | input[text] | — | yes |
| 14 | Model ID (e.g. TheBloke/Llama-2-7B-GGUF) | input[text] | — | yes |
| 15 | Filename (e.g. llama-2-7b.Q4_K_M.gguf) | input[text] | — | yes |
| 16 | Send Model | button | — | yes |

## Click sequence
### Click 1: "Search GGUF models on HuggingFace..." (input)
- Pathname before: /models
- New console: clean
- Network failures: none
- Visible change: input receives focus
- Pathname after: /models
- Reverted: n/a

### Click 2: "LLaMA"
- Pathname before: /models
- New console: clean
- Network failures: none
- Visible change: none — no active/selected state change, no search triggered
- Pathname after: /models
- Reverted: n/a

### Click 3: "Mistral"
- Pathname before: /models
- New console: clean
- Network failures: none
- Visible change: none — no active/selected state change
- Pathname after: /models
- Reverted: n/a

### Click 4: "Phi"
- Pathname before: /models
- New console: clean
- Network failures: none
- Visible change: none — no active/selected state change
- Pathname after: /models
- Reverted: n/a

### Click 5: "Gemma"
- Pathname before: /models
- New console: clean
- Network failures: none
- Visible change: none — no active/selected state change
- Pathname after: /models
- Reverted: n/a

### Click 6: "CodeLlama"
- Pathname before: /models
- New console: clean
- Network failures: none
- Visible change: none — no active/selected state change
- Pathname after: /models
- Reverted: n/a

### Click 7: "All"
- Pathname before: /models
- New console: clean
- Network failures: none
- Visible change: none — no active/selected state change
- Pathname after: /models
- Reverted: n/a

### Click 8: "Enable Sharing"
- Pathname before: /models
- New console: clean
- Network failures: none
- Visible change: none — silent no-op in demo mode
- Pathname after: /models
- Reverted: n/a

### Click 9: "Disable Sharing"
- Pathname before: /models
- New console: clean
- Network failures: none
- Visible change: none — silent no-op in demo mode
- Pathname after: /models
- Reverted: n/a

### Click 10: "Add Peer"
- Pathname before: /models
- New console: clean
- Network failures: none
- Visible change: none — silent no-op in demo mode
- Pathname after: /models
- Reverted: n/a

### Skipped (destructive)
none

### Total interactive elements found: 16
### Elements clicked: 10 (capped at 10)

## Accessibility
- Images without alt: 0
- Inputs without label: 6 (selectors: input[placeholder="Search GGUF models on HuggingFace..."], input[placeholder="Peer address (e.g. 192.168.1.100:9090)"], input[placeholder="Peer name (e.g. Office Desktop)"], input[placeholder="Peer address"], input[placeholder="Model ID (e.g. TheBloke/Llama-2-7B-GGUF)"], input[placeholder="Filename (e.g. llama-2-7b.Q4_K_M.gguf)"])
- Buttons without accessible name: 0

## Findings

### models-01
- SEVERITY: P1
- DIMENSION: console
- VIEWPORT: all
- EVIDENCE: `[ModelHub] failed to load installed models Error: desktop runtime unavailable` at ModelHub.tsx:122:39 (fired twice due to StrictMode). The component calls `listLocalModels()` via `invokeDesktop()` (backend.ts:17) without checking runtime availability first. Error is caught and logged but no user-facing fallback or empty-state is shown in the Installed Models panel.
- IMPACT: Console errors on every page load; Installed Models panel shows stale "No local models detected yet" text instead of a clear demo-mode message.

### models-02
- SEVERITY: P1
- DIMENSION: console
- VIEWPORT: all
- EVIDENCE: Four unhandled promise rejections: `TypeError: Cannot read properties of undefined (reading 'transformCallback')` at main.tsx:41:10, originating from ModelHub.tsx:238 (x2) and ModelHub.tsx:262 (x2). The component calls `@tauri-apps/api/event.listen()` without guarding for Tauri runtime availability.
- IMPACT: Four unhandled promise rejections flood the console on every page load. Event listeners for model download progress and sharing state are never registered.

### models-03
- SEVERITY: P2
- DIMENSION: overflow
- VIEWPORT: all
- EVIDENCE: `section.holo-panel.holo-panel-mid` has `overflow-x:hidden` which clips content. At 1920x1080: scrollWidth=1604 vs clientWidth=1577 (+27px clipped). At 1024x768: scrollWidth=944 vs clientWidth=681 (+263px clipped). Content is silently truncated.
- IMPACT: Right-side content of the holo-panel is invisible to users, worsening significantly at smaller viewports.

### models-04
- SEVERITY: P2
- DIMENSION: overflow
- VIEWPORT: all
- EVIDENCE: `div.living-background` (position:fixed decorative layer) exceeds viewport at all sizes: 1920 (+148px), 1280 (+96px), 1024 (+76px). This is a known app-level layout pattern.
- IMPACT: Cosmetic — fixed-position background extends beyond viewport but does not cause user-visible scrollbars due to body overflow containment.

### models-05
- SEVERITY: P2
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: Filter buttons (LLaMA, Mistral, Phi, Gemma, CodeLlama, All) produce zero visible change on click. No CSS class toggle, no `aria-pressed` attribute, no active/selected state. All buttons have empty `className`. No console output or network requests on click.
- IMPACT: Users clicking filter buttons get no feedback — no indication which filter is active or that the click was registered. Search filtering appears completely non-functional.

### models-06
- SEVERITY: P2
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: "Enable Sharing" and "Disable Sharing" buttons are both displayed simultaneously and both produce no visible change, console output, or network activity on click. Silent no-ops in demo mode.
- IMPACT: Users see contradictory toggle buttons with no state indication and no response to interaction.

### models-07
- SEVERITY: P2
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: "Add Peer" button click produces no visible change, console output, or network activity. Silent no-op in demo mode.
- IMPACT: Peer management workflow is completely non-functional with no user feedback.

### models-08
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: All 6 text inputs in main content lack `aria-label` or associated `<label>` elements. They rely solely on `placeholder` text for identification: "Search GGUF models on HuggingFace...", "Peer address (e.g. 192.168.1.100:9090)", "Peer name (e.g. Office Desktop)", "Peer address", "Model ID (e.g. TheBloke/Llama-2-7B-GGUF)", "Filename (e.g. llama-2-7b.Q4_K_M.gguf)".
- IMPACT: Screen readers cannot identify input purpose. Placeholder text disappears on focus, leaving no visible label. WCAG 2.1 Level A violation (1.3.1, 4.1.2).

### models-09
- SEVERITY: P2
- DIMENSION: copy
- VIEWPORT: all
- EVIDENCE: Nexus Link section displays raw error text "Error: desktop runtime unavailable" inline in the UI, visible to users alongside the Enable/Disable Sharing buttons.
- IMPACT: Technical error message exposed to end users instead of a graceful demo-mode fallback.

## Summary
- Gate detected: no
- Total interactive elements: 16
- Elements clicked: 10
- P0: 0
- P1: 2
- P2: 7
