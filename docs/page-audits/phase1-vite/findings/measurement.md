# Audit: Measurement
URL: http://localhost:1420/measurement
Audited at: 2026-04-09T22:04:00+01:00
Gate detected: false
Gate type: none

## Console (captured at 1920x1080, ALL messages)
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

### 1920x1080
- documentElement: scrollWidth=1888 clientWidth=1888 [OK]
- body: scrollWidth=1888 clientWidth=1888 [OK]
- main `main`: scrollWidth=1627 clientWidth=1627 [OK]
- other overflowing:
  - `div.living-background`: scrollWidth=2033 clientWidth=1888 [OVERFLOW +145px]
  - `section.holo-panel.holo-panel-mid.nexus-page-panel`: overflow:hidden clips 615px horizontally and 704px vertically (scrollWidth=2192 clientWidth=1577, scrollHeight=1341 clientHeight=637)

### 1280x800
- documentElement: scrollWidth=1248 clientWidth=1248 [OK]
- body: scrollWidth=1248 clientWidth=1248 [OK]
- main `main`: scrollWidth=987 clientWidth=987 [OK]
- other overflowing:
  - `section.holo-panel.holo-panel-mid.nexus-page-panel`: scrollWidth=1097 clientWidth=937 [OVERFLOW +160px]
  - `div.living-background`: scrollWidth=1341 clientWidth=1248 [OVERFLOW +93px]

### 1024x768
- documentElement: scrollWidth=992 clientWidth=992 [OK]
- body: scrollWidth=992 clientWidth=992 [OK]
- main `main`: scrollWidth=731 clientWidth=731 [OK]
- other overflowing:
  - `section.holo-panel.holo-panel-mid.nexus-page-panel`: scrollWidth=928 clientWidth=681 [OVERFLOW +247px]
  - `div.living-background`: scrollWidth=1065 clientWidth=992 [OVERFLOW +73px]

## Interactive elements (main content only)
| # | Label | Type | Href | Enabled |
|---|-------|------|------|---------|
| 1 | Refresh | button (no type attr) | n/a | true |
| 2 | Start Jarvis | button (no type attr) | n/a | true |
| 3 | Run New Measurement | button type="button" | n/a | true |
| 4 | Retry | button (no type attr) | n/a | true |

Note: "Refresh" and "Start Jarvis" are in the page header banner (outside `<main>` but inside main content area, not in sidebar).

## Click sequence
### Click 1: "Refresh"
- Pathname before: /measurement
- New console: clean
- Network failures: none
- Visible change: none observed
- Pathname after: /measurement
- Reverted: n/a

### Click 2: "Start Jarvis"
- Pathname before: /measurement
- New console: clean
- Network failures: none
- Visible change: none observed
- Pathname after: /measurement
- Reverted: n/a

### Click 3: "Run New Measurement"
- Pathname before: /measurement
- New console: `[ERROR] Error: desktop runtime unavailable` at invokeDesktop (backend.ts:17) → listAgents (backend.ts:29) → handleStartSession (MeasurementDashboard.tsx:100)
- Network failures: none
- Visible change: none (error div already present)
- Pathname after: /measurement
- Reverted: n/a

### Click 4: "Retry"
- Pathname before: /measurement
- New console: clean
- Network failures: none
- Visible change: none observed
- Pathname after: /measurement
- Reverted: n/a

### Skipped (destructive)
none

### Total interactive elements found: 4
### Elements clicked: 4 (all)

## Accessibility
- Images without alt: 0
- Inputs without label: 0
- Buttons without accessible name: 0
- Buttons missing `type` attribute (outside sidebar): 3 (`Refresh`, `Start Jarvis`, `Retry`)

## Findings

### measurement-01
- SEVERITY: P1
- DIMENSION: overflow
- VIEWPORT: all
- EVIDENCE: `div.living-background` exceeds viewport at all breakpoints — +145px at 1920, +93px at 1280, +73px at 1024. Does not cause visible scrollbar (parent clips) but is a layout bug.
- IMPACT: Background element renders wider than viewport; while clipped, it indicates incorrect sizing that could surface as a scrollbar if parent overflow changes.

### measurement-02
- SEVERITY: P1
- DIMENSION: overflow
- VIEWPORT: all
- EVIDENCE: `section.holo-panel.holo-panel-mid.nexus-page-panel` has `overflow:hidden` and clips content both horizontally (615px at 1920, 160px at 1280, 247px at 1024) and vertically (704px at 1920). Inner `div.holo-panel__refraction` is 2208px wide.
- IMPACT: Significant content may be silently clipped and unreachable by the user. The decorative refraction layer is more than double the panel width.

### measurement-03
- SEVERITY: P1
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: `<h1>Measurement</h1>` is positioned 116px above `<main>` (h1Top=92px, mainTop=208px) and is not a descendant of `<main>`. The h1 is inside the header banner element, outside the main landmark.
- IMPACT: Screen readers navigating by landmark will not find the page heading inside `<main>`, breaking expected document structure per WCAG 1.3.1.

### measurement-04
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: 3 buttons outside sidebar lack `type` attribute: `Refresh`, `Start Jarvis`, `Retry`. Without explicit `type="button"`, these default to `type="submit"` which can cause unintended form submission if ever placed inside a `<form>`.
- IMPACT: Minor — no forms currently wrap these buttons, but missing type is a best-practice violation that can cause regressions.

### measurement-05
- SEVERITY: P2
- DIMENSION: interactive
- VIEWPORT: 1920
- EVIDENCE: Clicking "Refresh" and "Start Jarvis" produces no console output, no network requests, and no visible change. Both buttons are silent no-ops in demo mode.
- IMPACT: Buttons appear clickable but give zero feedback. Users cannot tell whether the action was attempted or ignored.

### measurement-06
- SEVERITY: P2
- DIMENSION: interactive
- VIEWPORT: 1920
- EVIDENCE: Clicking "Run New Measurement" throws `Error: desktop runtime unavailable` (backend.ts:17 → MeasurementDashboard.tsx:100). The error is caught and displayed in a `<div>` with red background but the error div was already visible before the click — the error state is the default state on page load.
- IMPACT: The error message "Error: desktop runtime unavailable" is shown on initial load before the user has taken any action, which is confusing. Expected: empty state or informational message until user clicks.

### measurement-07
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: Error container `<div>` showing "Error: desktop runtime unavailable" has no `role="alert"` and no `aria-live` attribute. Tag is plain `<div>` with no semantic role.
- IMPACT: Screen readers will not announce the error when it appears or updates. Error state is invisible to assistive technology.

### measurement-08
- SEVERITY: P2
- DIMENSION: interactive
- VIEWPORT: 1920
- EVIDENCE: Clicking "Retry" produces no console output and no visible change. The error message remains. The button appears to be a no-op.
- IMPACT: Retry button gives no feedback — no loading state, no re-attempt animation, no console activity. User cannot tell if retry was attempted.

## Summary
- Gate detected: no
- Total interactive elements: 4
- Elements clicked: 4
- P0: 0
- P1: 3
- P2: 5
