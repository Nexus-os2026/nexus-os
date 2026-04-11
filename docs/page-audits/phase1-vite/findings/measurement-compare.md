# Audit: Measurement Compare
URL: http://localhost:1420/measurement-compare
Audited at: 2026-04-09T22:09:00Z
Gate detected: false
Gate type: none

## Console (captured at 1920x1080, ALL messages)
### Errors
1. `Error: desktop runtime unavailable` at `invokeDesktop` (`src/api/backend.ts:17:11`) via `cmListSessions` (`src/api/backend.ts:2332:10`) called from `MeasurementCompare.tsx:53:5` — React commitHookEffectListMount
2. `Error: desktop runtime unavailable` (duplicate from React StrictMode double-invoke) — same stack as #1 via `commitDoubleInvokeEffectsInDEV`

### Warnings
none

### Logs
none

### Info
none

### Debug
none

## Overflow

Note: Browser window was maximized at 1888x951 and could not be resized via API. Measurements at 1280 and 1024 were simulated by constraining `document.documentElement` and `document.body` maxWidth.

### 1920x1080 (actual viewport 1888x951)
- documentElement: scrollWidth=1888 clientWidth=1888 [OK]
- body: scrollWidth=1888 clientWidth=1888 [OK]
- main `main.nexus-shell-content`: scrollWidth=1627 clientWidth=1627 [OK]
- other overflowing:
  - `div.living-background`: scrollWidth=2039 clientWidth=1888 delta=151 [OVERFLOW]
  - `section.holo-panel`: scrollWidth=1716 clientWidth=1577 delta=139 [OVERFLOW]
  - `div#claude-static-indicator-container`: scrollWidth=368 clientWidth=357 delta=11 [OVERFLOW]
  - `button#claude-static-chat-button`: scrollWidth=56 clientWidth=32 delta=24 [OVERFLOW]
  - `button#claude-static-close-button`: scrollWidth=50 clientWidth=32 delta=18 [OVERFLOW]

### 1280x800 (simulated)
- documentElement: scrollWidth=1888 clientWidth=1888 [OK — unconstrained outer]
- body: scrollWidth=1280 clientWidth=1280 [OK]
- main `main.nexus-shell-content`: scrollWidth=1019 clientWidth=1019 [OK]
- other overflowing:
  - `div.living-background`: scrollWidth=2039 clientWidth=1888 delta=151 [OVERFLOW]
  - `section.holo-panel`: scrollWidth=1054 clientWidth=969 delta=85 [OVERFLOW]
  - `div#claude-static-indicator-container`: scrollWidth=368 clientWidth=357 delta=11 [OVERFLOW]
  - `button#claude-static-chat-button`: scrollWidth=56 clientWidth=32 delta=24 [OVERFLOW]
  - `button#claude-static-close-button`: scrollWidth=50 clientWidth=32 delta=18 [OVERFLOW]

### 1024x768 (simulated)
- documentElement: scrollWidth=1888 clientWidth=1888 [OK — unconstrained outer]
- body: scrollWidth=1024 clientWidth=1024 [OK]
- main `main.nexus-shell-content`: scrollWidth=763 clientWidth=763 [OK]
- other overflowing:
  - `div.living-background`: scrollWidth=2039 clientWidth=1888 delta=151 [OVERFLOW]
  - `section.holo-panel`: scrollWidth=776 clientWidth=713 delta=63 [OVERFLOW]
  - `div#claude-static-indicator-container`: scrollWidth=368 clientWidth=357 delta=11 [OVERFLOW]
  - `button#claude-static-chat-button`: scrollWidth=56 clientWidth=32 delta=24 [OVERFLOW]
  - `button#claude-static-close-button`: scrollWidth=50 clientWidth=32 delta=18 [OVERFLOW]

## Interactive elements (main content only)

Elements in main content area (excluding sidebar `aside.nexus-sidebar-shell`):

| # | Label | Type | Href | Enabled |
|---|-------|------|------|---------|
| 1 | Refresh | button | — | true |
| 2 | Start Jarvis | button | — | true |
| 3 | Compare | button[type=button] | — | true |

## Click sequence

### Click 1: "Refresh"
- Pathname before: /measurement-compare
- New console: clean
- Network failures: none
- Visible change: none — silent no-op
- Pathname after: /measurement-compare
- Reverted: n/a

### Click 2: "Start Jarvis"
- Pathname before: /measurement-compare
- New console: clean
- Network failures: none
- Visible change: none — silent no-op
- Pathname after: /measurement-compare
- Reverted: n/a

### Click 3: "Compare"
- Pathname before: /measurement-compare
- New console: clean
- Network failures: none
- Visible change: none — silent no-op (0 agents selected, button enabled but does nothing)
- Pathname after: /measurement-compare
- Reverted: n/a

### Skipped (destructive)
none

### Total interactive elements found: 3
### Elements clicked: 3

## Accessibility
- Images without alt: 0
- Inputs without label: 0
- Buttons without accessible name: 0
- Additional findings:
  - h1 "Compare Agents" is positioned 116px above `<main>` element (outside main landmark)
  - "Refresh" button (`button.nx-btn`) missing `type` attribute
  - "Start Jarvis" button (`button.nx-btn`) missing `type` attribute
  - "Desktop runtime required" message div has no `role="alert"` or `aria-live` attribute
  - `section.holo-panel` has `overflow: hidden` — clips 346px of vertical content (scrollHeight=1039, clientHeight=693)
  - `.holo-panel__refraction` element is 2208px wide (decorative element causing overflow)

## Findings

### measurement-compare-01
- SEVERITY: P1
- DIMENSION: console
- VIEWPORT: all
- EVIDENCE: Two unhandled `Error: desktop runtime unavailable` thrown on page load at `MeasurementCompare.tsx:53` via `cmListSessions()` (`backend.ts:2332`). The useEffect calls `cmListSessions` without a try/catch, so the error propagates to React's error boundary mechanism. Second error is React StrictMode double-invoke.
- IMPACT: Console errors on every page load; if error boundary is strict this could crash the component tree.

### measurement-compare-02
- SEVERITY: P1
- DIMENSION: overflow
- VIEWPORT: all
- EVIDENCE: `div.living-background` overflows by 151px at 1888px viewport (scrollWidth=2039 vs clientWidth=1888). This is a decorative background element whose width exceeds the viewport at all tested sizes.
- IMPACT: Potential horizontal scrollbar or clipped content depending on parent overflow settings.

### measurement-compare-03
- SEVERITY: P1
- DIMENSION: overflow
- VIEWPORT: all
- EVIDENCE: `section.holo-panel` overflows horizontally: delta=139px at 1920, delta=85px at 1280 (sim), delta=63px at 1024 (sim). Root cause: `.holo-panel__refraction` child is 2208px wide.
- IMPACT: Content extends beyond visible panel boundary; clipped by `overflow: hidden` but represents layout miscalculation.

### measurement-compare-04
- SEVERITY: P2
- DIMENSION: overflow
- VIEWPORT: all
- EVIDENCE: `section.holo-panel` has `overflow: hidden` with scrollHeight=1039 and clientHeight=693 — 346px of vertical content is silently clipped.
- IMPACT: If any content is placed below the fold inside holo-panel, it is invisible and unreachable.

### measurement-compare-05
- SEVERITY: P2
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: "Refresh" and "Start Jarvis" buttons (`button.nx-btn`) produce no console output, no network request, and no visible change on click. Both are silent no-ops in demo mode.
- IMPACT: Buttons appear enabled and clickable but do nothing; user receives no feedback.

### measurement-compare-06
- SEVERITY: P2
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: "Compare" button (type=button) is enabled with 0 agents selected (heading reads "Select Agents (0/4)"). Clicking produces no console output and no visible change. Button should be disabled when 0 agents are selected.
- IMPACT: Misleading enabled state — user clicks expecting action but nothing happens.

### measurement-compare-07
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: `<h1>` "Compare Agents" is positioned 116px above the `<main>` element. `h1` is inside the banner/header region (ref_214 inside ref_212), not inside `<main>` (ref_221). Screen readers navigating by landmark will skip the page heading.
- IMPACT: Heading is outside the main content landmark, degrading navigation for assistive technology users.

### measurement-compare-08
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: "Refresh" and "Start Jarvis" buttons lack `type` attribute. Default type is "submit" which can cause unexpected form submission in some contexts.
- IMPACT: Missing `type="button"` is a minor HTML conformance issue that could cause unexpected behavior if buttons end up inside a form.

### measurement-compare-09
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: "Desktop runtime required" message (`<div>` with text "Desktop runtime required — launch Nexus OS from the Tauri app for full functionality.") has no `role="alert"` or `aria-live` attribute.
- IMPACT: Status/error message is not announced to screen reader users.

## Summary
- Gate detected: no
- Total interactive elements: 3
- Elements clicked: 3
- P0: 0
- P1: 3
- P2: 6
