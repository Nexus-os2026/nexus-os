# Audit: Measurement Batteries
URL: http://localhost:1420/measurement-batteries
Audited at: 2026-04-09T22:14:00+01:00
Gate detected: false
Gate type: none

## Console (captured at 1920x1080, ALL messages)
### Errors
1. `Error: desktop runtime unavailable` at `src/api/backend.ts:17:11` via `cmGetBatteries` at `src/api/backend.ts:2353:10` called from `src/pages/MeasurementBatteries.tsx:47:5` (React commitHookEffectListMount)
2. `Error: desktop runtime unavailable` at `src/api/backend.ts:17:11` via `cmGetBatteries` at `src/api/backend.ts:2353:10` called from `src/pages/MeasurementBatteries.tsx:47:5` (React StrictMode double-invoke)

### Warnings
none

### Logs
none

### Info
none

### Debug
none

## Overflow

### 1920x1080
- documentElement: scrollWidth=1248 clientWidth=1248 OK
- body: scrollWidth=1248 clientWidth=1248 OK
- main `main.nexus-shell-content`: scrollWidth=987 clientWidth=987 OK
- other overflowing:
  - `div.living-background`: scrollWidth=1348 clientWidth=1248 OVERFLOW (+100px)
  - `section.holo-panel.holo-panel-mid`: scrollWidth=1019 clientWidth=937 OVERFLOW (+82px)
  - `span.nexus-sidebar-item-text` (x3): scrollWidth=157 clientWidth=153 OVERFLOW (+4px each)

### 1280x800
- documentElement: scrollWidth=1248 clientWidth=1248 OK
- body: scrollWidth=1248 clientWidth=1248 OK
- main `main.nexus-shell-content`: scrollWidth=987 clientWidth=987 OK
- other overflowing:
  - `div.living-background`: scrollWidth=1348 clientWidth=1248 OVERFLOW (+100px)
  - `section.holo-panel.holo-panel-mid`: scrollWidth=1019 clientWidth=937 OVERFLOW (+82px)
  - `span.nexus-sidebar-item-text` (x3): scrollWidth=157 clientWidth=153 OVERFLOW (+4px each)

NOTE: resize_window MCP tool does not change CSS viewport metrics (known limitation from obs 715). Values identical across all three viewports.

### 1024x768
- documentElement: scrollWidth=1248 clientWidth=1248 OK
- body: scrollWidth=1248 clientWidth=1248 OK
- main `main.nexus-shell-content`: scrollWidth=987 clientWidth=987 OK
- other overflowing:
  - `div.living-background`: scrollWidth=1348 clientWidth=1248 OVERFLOW (+100px)
  - `section.holo-panel.holo-panel-mid`: scrollWidth=1019 clientWidth=937 OVERFLOW (+82px)
  - `span.nexus-sidebar-item-text` (x3): scrollWidth=157 clientWidth=153 OVERFLOW (+4px each)

## Interactive elements (main content only)
| # | Label | Type | Href | Enabled |
|---|-------|------|------|---------|
| (none) | — | — | — | — |

Zero interactive elements found inside `<main>`. The page renders 20 L1-L5 level indicator divs (4 battery cards x 5 levels) that appear visually as pill/badge elements but are plain `<div>` elements with no button semantics, role, tabindex, or click handler.

### Header toolbar (outside main, outside sidebar)
| # | Label | Type | Href | Enabled |
|---|-------|------|------|---------|
| 1 | Refresh | button (no type attr) | — | true |
| 2 | Start Jarvis | button (no type attr) | — | true |

### Jarvis overlay (outside main, outside sidebar)
| # | Label | Type | Href | Enabled |
|---|-------|------|------|---------|
| 3 | Open chat | button (no type attr) | — | true |
| 4 | Dismiss | button (no type attr) | — | true |

## Click sequence

### Click 1: "Refresh"
- Pathname before: /measurement-batteries
- New console: clean
- Network failures: none
- Visible change: none (silent no-op in demo mode)
- Pathname after: /measurement-batteries
- Reverted: n/a

### Click 2: "Start Jarvis"
- Pathname before: /measurement-batteries
- New console: clean
- Network failures: none
- Visible change: none (silent no-op in demo mode)
- Pathname after: /measurement-batteries
- Reverted: n/a

### Skipped (destructive)
none

### Total interactive elements found: 0 (in main), 4 (outside main, outside sidebar)
### Elements clicked: 2 (header toolbar buttons only; main has 0 clickable elements)

## Accessibility
- Images without alt: 0
- Inputs without label: 0
- Buttons without accessible name: 0

## Findings

### measurement-batteries-01
- SEVERITY: P1
- DIMENSION: overflow
- VIEWPORT: all
- EVIDENCE: `div.living-background` scrollWidth=1348, clientWidth=1248 (+100px horizontal overflow). Present at all measured viewports.
- IMPACT: Background element extends 100px beyond viewport, may cause horizontal scrollbar on some browsers/OS combinations.

### measurement-batteries-02
- SEVERITY: P1
- DIMENSION: overflow
- VIEWPORT: all
- EVIDENCE: `section.holo-panel.holo-panel-mid` scrollWidth=1019, clientWidth=937 (+82px). `overflow: hidden` clips content — scrollHeight=565 vs clientHeight=353 means 212px of vertical content is silently clipped.
- IMPACT: Battery card content below the fold is permanently invisible; users cannot scroll to see clipped content within the panel.

### measurement-batteries-03
- SEVERITY: P2
- DIMENSION: console
- VIEWPORT: 1920
- EVIDENCE: Two `Error: desktop runtime unavailable` thrown at `src/api/backend.ts:17` via `cmGetBatteries()` at `src/pages/MeasurementBatteries.tsx:47`. Uncaught error in React useEffect.
- IMPACT: Expected in demo mode, but error is unhandled — no try/catch or error boundary, error propagates to console. Should be caught gracefully.

### measurement-batteries-04
- SEVERITY: P1
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: `<h1>Test Batteries</h1>` is at top=92px, inside `header.nexus-shell-header`, while `<main>` starts at top=268px. The h1 is outside the `<main>` landmark.
- IMPACT: Screen readers navigating by landmark will not find the page heading inside main; WCAG 2.4.1 bypass blocks / landmark structure violation.

### measurement-batteries-05
- SEVERITY: P1
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: 20 L1-L5 level indicator elements are rendered as plain `<div>` with no `role="button"`, no `tabindex`, no `aria-label`. They appear as interactive pill/badge UI but are not keyboard-accessible or announced to screen readers.
- IMPACT: If these are intended as interactive level selectors, they are completely inaccessible. If purely decorative, they should not look clickable.

### measurement-batteries-06
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: "No test batteries loaded yet" status message is a plain `<div>` with no `role="alert"` or `aria-live` attribute. Summary line "0 problems across 4 vectors" is a plain `<p>`.
- IMPACT: Screen readers will not announce status changes when batteries load/fail to load.

### measurement-batteries-07
- SEVERITY: P2
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: "Refresh" button (header toolbar) clicked — no console output, no visible change, no network request. Silent no-op in demo mode.
- IMPACT: Button provides zero feedback on click; user cannot tell if action was attempted or failed.

### measurement-batteries-08
- SEVERITY: P2
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: "Start Jarvis" button (header toolbar) clicked — no console output, no visible change. Silent no-op in demo mode.
- IMPACT: Button provides zero feedback on click; user cannot tell if action was attempted or failed.

### measurement-batteries-09
- SEVERITY: P2
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: 4 buttons outside `<main>` and outside sidebar (`Refresh`, `Start Jarvis`, `Open chat`, `Dismiss`) all lack `type` attribute. Without `type="button"`, they default to `type="submit"` and may trigger form submission if placed inside a form.
- IMPACT: Potential unintended form submission behavior; violates HTML best practice.

## Summary
- Gate detected: no
- Total interactive elements: 0 (in main), 4 (in header/overlay outside main)
- Elements clicked: 2
- P0: 0
- P1: 3
- P2: 4
