# Audit: Capability Boundaries
URL: http://localhost:1420/capability-boundaries
Audited at: 2026-04-09T22:18:00+01:00
Gate detected: false
Gate type: none

## Console (captured at 1920x1080, ALL messages)
### Errors
1. `Error: desktop runtime unavailable` at `invokeDesktop` (`src/api/backend.ts:17:11`) via `cmGetBoundaryMap` (`src/api/backend.ts:2370:10`) via `load` (`src/pages/CapabilityBoundaryMap.tsx:63:18`) — React commitHookEffectListMount
2. `Error: desktop runtime unavailable` at `invokeDesktop` (`src/api/backend.ts:17:11`) via `cmGetBoundaryMap` (`src/api/backend.ts:2370:10`) via `load` (`src/pages/CapabilityBoundaryMap.tsx:63:18`) — React StrictMode double-invoke

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
- documentElement: scrollWidth=1888 clientWidth=1888 [OK]
- body: scrollWidth=1888 clientWidth=1888 [OK]
- main `main`: scrollWidth=1627 clientWidth=1627 [OK]
- other overflowing:
  - `div.living-background`: scrollWidth=2039 clientWidth=1888 [OVERFLOW +151px]
  - `section.holo-panel.holo-panel-mid.nexus-page-panel`: scrollWidth=1716 clientWidth=1577 [OVERFLOW +139px]
  - `div` (unnamed): scrollWidth=368 clientWidth=357 [OVERFLOW +11px]
  - `button` (2x): scrollWidth=56/50 clientWidth=32 [OVERFLOW +24/+18px]

### 1280x800
- documentElement: scrollWidth=1888 clientWidth=1888 [OK]
- body: scrollWidth=1280 clientWidth=1280 [OK]
- main `main`: scrollWidth=1019 clientWidth=1019 [OK]
- other overflowing:
  - `div.living-background`: scrollWidth=2037 clientWidth=1888 [OVERFLOW +149px]
  - `section.holo-panel.holo-panel-mid.nexus-page-panel`: scrollWidth=1135 clientWidth=969 [OVERFLOW +166px]
  - `div.holo-panel__content`: scrollWidth=978 clientWidth=969 [OVERFLOW +9px]
  - `div` (unnamed): scrollWidth=368 clientWidth=357 [OVERFLOW +11px]
  - `button` (2x): scrollWidth=56/50 clientWidth=32 [OVERFLOW +24/+18px]

### 1024x768
- documentElement: scrollWidth=1888 clientWidth=1888 [OK]
- body: scrollWidth=1024 clientWidth=1024 [OK]
- main `main`: scrollWidth=763 clientWidth=763 [OK]
- other overflowing:
  - `div.living-background`: scrollWidth=2036 clientWidth=1888 [OVERFLOW +148px]
  - `section.holo-panel.holo-panel-mid.nexus-page-panel`: scrollWidth=1069 clientWidth=713 [OVERFLOW +356px]
  - `div.holo-panel__content`: scrollWidth=978 clientWidth=713 [OVERFLOW +265px]
  - `div` (unnamed): scrollWidth=368 clientWidth=357 [OVERFLOW +11px]
  - `button` (2x): scrollWidth=56/50 clientWidth=32 [OVERFLOW +24/+18px]

## Interactive elements (main content only)
| # | Label | Type | Href | Enabled |
|---|-------|------|------|---------|
| 1 | Refresh | button (no type attr) | — | yes |
| 2 | Start Jarvis | button (no type attr) | — | yes |
| 3 | Upload to Darwin | button[type=button] | — | yes |
| 4 | Open chat | button (no type attr) | — | yes |
| 5 | Dismiss | button (no type attr) | — | yes |

Note: #1-2 are in page header chrome (outside `main`). #3 is inside `main`. #4-5 are floating overlay buttons. All are outside sidebar.

## Click sequence
### Click 1: "Refresh"
- Pathname before: /capability-boundaries
- New console: clean
- Network failures: none
- Visible change: none — silent no-op
- Pathname after: /capability-boundaries
- Reverted: n/a

### Click 2: "Start Jarvis"
- Pathname before: /capability-boundaries
- New console: clean
- Network failures: none
- Visible change: none — silent no-op
- Pathname after: /capability-boundaries
- Reverted: n/a

### Click 3: "Upload to Darwin"
- Pathname before: /capability-boundaries
- New console: clean
- Network failures: none
- Visible change: none — silent no-op
- Pathname after: /capability-boundaries
- Reverted: n/a

### Click 4: "Open chat"
- Pathname before: /capability-boundaries
- New console: clean
- Network failures: none
- Visible change: none — silent no-op
- Pathname after: /capability-boundaries
- Reverted: n/a

### Click 5: "Dismiss"
- Pathname before: /capability-boundaries
- New console: clean
- Network failures: none
- Visible change: none — silent no-op
- Pathname after: /capability-boundaries
- Reverted: n/a

### Skipped (destructive)
none

### Total interactive elements found: 5
### Elements clicked: 5 (all)

## Accessibility
- Images without alt: 0
- Inputs without label: 0
- Buttons without accessible name: 0
- Additional: 4 buttons missing `type` attribute (Refresh, Start Jarvis, Open chat, Dismiss)

## Findings

### capability-boundaries-01
- SEVERITY: P1
- DIMENSION: console
- VIEWPORT: all
- EVIDENCE: Two `Error: desktop runtime unavailable` thrown at load from `cmGetBoundaryMap` (`src/api/backend.ts:2370:10`) called in `CapabilityBoundaryMap.tsx:63`. The error is unhandled — thrown to console as an uncaught exception. React StrictMode causes a second identical error.
- IMPACT: Console errors on every page load in demo mode; error is not caught/displayed gracefully to the user.

### capability-boundaries-02
- SEVERITY: P1
- DIMENSION: overflow
- VIEWPORT: all
- EVIDENCE: `div.living-background` overflows viewport by ~150px at all viewports (scrollWidth=2039 vs clientWidth=1888 at 1920x1080). Persistent across all three tested sizes.
- IMPACT: Decorative background element extends beyond viewport; may cause horizontal scrollbar or layout shift.

### capability-boundaries-03
- SEVERITY: P1
- DIMENSION: overflow
- VIEWPORT: all
- EVIDENCE: `section.holo-panel.holo-panel-mid.nexus-page-panel` overflows horizontally (+139px at 1920, +166px at 1280, +356px at 1024) and clips vertically (scrollHeight=1529 vs clientHeight=676 at 1920, overflow:hidden set). Content is clipped by 853px vertically.
- IMPACT: Page content is hidden/clipped inside the holo-panel; users cannot scroll to see truncated content. Worsens at smaller viewports.

### capability-boundaries-04
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: Two `<h1>` elements on page: "Boundary Map" (in page header chrome, outside `main`) and "Capability Boundary Map" (inside `main`). Document has duplicate landmark headings.
- IMPACT: Screen readers and SEO crawlers receive conflicting page title signals; violates WCAG heading hierarchy.

### capability-boundaries-05
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: The first `<h1>` ("Boundary Map") is positioned at top=92px while `<main>` starts at top=208px. The h1 is outside the `main` landmark entirely.
- IMPACT: Screen reader users navigating by landmark will miss the page heading; heading is structurally orphaned from main content.

### capability-boundaries-06
- SEVERITY: P2
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: 4 of 5 buttons (`Refresh`, `Start Jarvis`, `Open chat`, `Dismiss`) are missing the `type` attribute. Only `Upload to Darwin` has `type="button"`.
- IMPACT: Buttons without `type` default to `type="submit"`, which can trigger unintended form submissions if wrapped in a `<form>`.

### capability-boundaries-07
- SEVERITY: P2
- DIMENSION: action
- VIEWPORT: all
- EVIDENCE: All 5 buttons (Refresh, Start Jarvis, Upload to Darwin, Open chat, Dismiss) produce no visible change, no console output, and no network requests on click. All are silent no-ops in demo mode.
- IMPACT: Users clicking any button receive zero feedback; no loading indicator, toast, or error message. Indistinguishable from a broken UI.

### capability-boundaries-08
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: "INVERSIONS DETECTED" calibration status in `<p>` element has no `role="status"` or `aria-live` attribute. Parent `<div>` also has no ARIA live region.
- IMPACT: Screen readers will not announce calibration status changes; status text is invisible to assistive technology updates.

## Summary
- Gate detected: no
- Total interactive elements: 5
- Elements clicked: 5
- P0: 0
- P1: 3
- P2: 5
