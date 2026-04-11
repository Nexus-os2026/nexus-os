# Audit: Consciousness
URL: http://localhost:1420/consciousness
Audited at: 2026-04-09T23:21:00Z
Gate detected: true
Gate type: RequiresLlm

## Console (captured at 1248x671, ALL messages)
### Errors
none

### Warnings
none

### Logs
none

### Info
- `%cDownload the React DevTools for a better development experience: https://reactjs.org/link/react-devtools font-weight:bold` — chunk-NUMECXU6.js:21550

### Debug
- `[vite] connecting...` — @vite/client:494
- `[vite] connected.` — @vite/client:617

## Overflow

> **Note:** `resize_window` has no effect in Chrome extension tab context. Viewport locked at 1248x671 across all resize attempts. All three measurement sets below reflect the same locked viewport.

### 1920x1080 (requested — actual 1248x671)
- documentElement: scrollWidth=1248 clientWidth=1248 [OK]
- body: scrollWidth=1248 clientWidth=1248 [OK]
- main `main`: scrollWidth=987 clientWidth=987 [OK]
- other overflowing:
  - `div.living-background`: scrollWidth=1348 clientWidth=1248 [OVERFLOW +100px]
  - `section.holo-panel.holo-panel-mid`: scrollWidth=1019 clientWidth=937 [OVERFLOW +82px]

### 1280x800 (requested — actual 1248x671)
- documentElement: scrollWidth=1248 clientWidth=1248 [OK]
- body: scrollWidth=1248 clientWidth=1248 [OK]
- main `main`: scrollWidth=987 clientWidth=987 [OK]
- other overflowing:
  - `div.living-background`: scrollWidth=1348 clientWidth=1248 [OVERFLOW +100px]
  - `section.holo-panel.holo-panel-mid`: scrollWidth=1019 clientWidth=937 [OVERFLOW +82px]

### 1024x768 (requested — actual 1248x671)
- documentElement: scrollWidth=1248 clientWidth=1248 [OK]
- body: scrollWidth=1248 clientWidth=1248 [OK]
- main `main`: scrollWidth=987 clientWidth=987 [OK]
- other overflowing:
  - `div.living-background`: scrollWidth=1348 clientWidth=1248 [OVERFLOW +100px]
  - `section.holo-panel.holo-panel-mid`: scrollWidth=1019 clientWidth=937 [OVERFLOW +82px]

## Interactive elements (main content only)
| # | Label | Type | Href | Enabled |
|---|-------|------|------|---------|
| 1 | Refresh | button (no `type` attr) | — | yes |
| 2 | Start Jarvis | button (no `type` attr) | — | yes |
| 3 | Install Ollama (Free, Local, Private) | button (no `type` attr) | — | yes |
| 4 | I have an API key (OpenAI, Anthropic, etc.) | a | #/settings | yes |

## Click sequence
### Click 1: "Refresh"
- Pathname before: /consciousness
- New console: clean
- Network failures: none
- Visible change: none — silent no-op
- Pathname after: /consciousness
- Reverted: n/a

### Click 2: "Start Jarvis"
- Pathname before: /consciousness
- New console: clean
- Network failures: none
- Visible change: none — silent no-op
- Pathname after: /consciousness
- Reverted: n/a

### Click 3: "Install Ollama (Free, Local, Private)"
- Pathname before: /consciousness
- New console: clean
- Network failures: none
- Visible change: none — silent no-op
- Pathname after: /consciousness
- Reverted: n/a

### Click 4: "I have an API key (OpenAI, Anthropic, etc.)"
- Pathname before: /consciousness
- New console: clean
- Network failures: none
- Visible change: none — hash fragment appended to URL (`#/settings`), page did not navigate to /settings
- Pathname after: /consciousness (hash changed to #/settings)
- Reverted: n/a (no real navigation occurred)

### Skipped (destructive)
none

### Total interactive elements found: 4
### Elements clicked: 4

## Accessibility
- Images without alt: 0
- Inputs without label: 0
- Buttons without accessible name: 0

## Findings

### consciousness-01
- SEVERITY: P1
- DIMENSION: overflow
- VIEWPORT: all (locked at 1248x671)
- EVIDENCE: `div.living-background` scrollWidth=1348 > clientWidth=1248 (+100px). Background layer extends 100px beyond viewport.
- IMPACT: Horizontal scroll or clipped content; decorative layer bleeds outside viewport bounds.

### consciousness-02
- SEVERITY: P1
- DIMENSION: overflow
- VIEWPORT: all (locked at 1248x671)
- EVIDENCE: `section.holo-panel.holo-panel-mid` scrollWidth=1019 > clientWidth=937 (+82px). `overflow: hidden` on section masks the overflow but content is clipped.
- IMPACT: Gate card content may be clipped at narrower viewports; overflow is hidden but the layout bug persists underneath.

### consciousness-03
- SEVERITY: P1
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: All 3 buttons ("Refresh", "Start Jarvis", "Install Ollama") produce zero feedback on click — no console output, no visual change, no loading state, no toast. Silent no-ops.
- IMPACT: User has no indication whether the action was attempted and failed or is simply unimplemented. "Install Ollama" is the primary CTA on the gate screen and gives no feedback at all.

### consciousness-04
- SEVERITY: P1
- DIMENSION: gate
- VIEWPORT: all
- EVIDENCE: `a[href="#/settings"]` ("I have an API key") resolves to `http://localhost:1420/consciousness#/settings` — a hash fragment on the current page — instead of navigating to `/settings`. The app uses path-based routing, not hash routing, so this link is broken.
- IMPACT: Users clicking the gate's secondary CTA to configure an API key are stranded on the consciousness page. The settings page is never reached.

### consciousness-05
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: All 3 `<button>` elements in the main content area ("Refresh", "Start Jarvis", "Install Ollama") lack a `type` attribute. Default type is `submit`, which is semantically incorrect outside a `<form>`.
- IMPACT: Assistive technologies may announce these as submit buttons. If a form is ever added as an ancestor, these buttons would trigger form submission.

### consciousness-06
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: `section.holo-panel.holo-panel-mid` inside `<main>` has no `role` attribute and no `aria-label`. It is the sole content section containing the RequiresLlm gate.
- IMPACT: Screen readers cannot identify the purpose of this section landmark.

## Summary
- Gate detected: yes
- Total interactive elements: 4
- Elements clicked: 4
- P0: 0
- P1: 4
- P2: 2
