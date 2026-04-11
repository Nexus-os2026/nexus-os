# Audit: Ab Validation
URL: http://localhost:1420/ab-validation
Audited at: 2026-04-09T22:27:00+01:00
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
- `%cDownload the React DevTools for a better development experience: https://reactjs.org/link/react-devtools font-weight:bold` — chunk-NUMECXU6.js?v=5144749d:21550:24

### Debug
- `[vite] connecting...` — @vite/client:494:8
- `[vite] connected.` — @vite/client:617:14

## Overflow

### 1920x1080
- documentElement: scrollWidth=1888 clientWidth=1888 [OK]
- body: scrollWidth=1888 clientWidth=1888 [OK]
- main `main`: scrollWidth=1627 clientWidth=1627 [OK]
- other overflowing:
  - `div.living-background`: scrollWidth=2039 clientWidth=1888 [OVERFLOW +151px]
  - `section.holo-panel.holo-panel-mid`: scrollWidth=1716 clientWidth=1577 [OVERFLOW +139px horizontal]; scrollHeight=1448 clientHeight=637 [OVERFLOW +811px vertical]

### 1280x800
- documentElement: scrollWidth=1248 clientWidth=1248 [OK]
- body: scrollWidth=1248 clientWidth=1248 [OK]
- main `main`: scrollWidth=987 clientWidth=987 [OK]
- other overflowing:
  - `div.living-background`: scrollWidth=1348 clientWidth=1248 [OVERFLOW +100px]
  - `section.holo-panel.holo-panel-mid`: scrollWidth=1171 clientWidth=937 [OVERFLOW +234px]

### 1024x768
- documentElement: scrollWidth=992 clientWidth=992 [OK]
- body: scrollWidth=992 clientWidth=992 [OK]
- main `main`: scrollWidth=731 clientWidth=731 [OK]
- other overflowing:
  - `div.living-background`: scrollWidth=1071 clientWidth=992 [OVERFLOW +79px]
  - `section.holo-panel.holo-panel-mid`: scrollWidth=928 clientWidth=681 [OVERFLOW +247px]

## Interactive elements (main content only)
| # | Label | Type | Href | Enabled |
|---|-------|------|------|---------|
| 1 | Refresh | button (implicit submit) | — | true |
| 2 | Start Jarvis | button (implicit submit) | — | true |
| 3 | Run A/B Validation | button[type=button] | — | true |
| 4 | Run Validation | button (implicit submit) | — | true |

Note: Elements 1–2 ("Refresh", "Start Jarvis") are in the page-level banner bar, outside `<main>` but outside the sidebar.

## Click sequence
### Click 1: "Refresh"
- Pathname before: /ab-validation
- New console: clean
- Network failures: none
- Visible change: none — button is silently inert in demo mode
- Pathname after: /ab-validation
- Reverted: n/a

### Click 2: "Start Jarvis"
- Pathname before: /ab-validation
- New console: clean
- Network failures: none
- Visible change: none — button is silently inert in demo mode
- Pathname after: /ab-validation
- Reverted: n/a

### Click 3: "Run A/B Validation"
- Pathname before: /ab-validation
- New console: clean
- Network failures: none
- Visible change: "Getting Started" empty-state section replaced by error message "Error: desktop runtime unavailable" with a "Dismiss" button
- Pathname after: /ab-validation
- Reverted: n/a

### Click 4: "Run Validation"
- Pathname before: /ab-validation
- New console: clean
- Network failures: none
- Visible change: Button no longer present — the "Getting Started" section was already replaced by the error state after Click 3. Click was effectively a no-op because the target element was gone.
- Pathname after: /ab-validation
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

### ab-validation-01
- SEVERITY: P2
- DIMENSION: overflow
- VIEWPORT: all
- EVIDENCE: `div.living-background` exceeds viewport at all three breakpoints: +151px at 1920, +100px at 1280, +79px at 1024. Hidden by parent `overflow:hidden` so no user-visible scrollbar, but element is oversized.
- IMPACT: Cosmetic — background element renders beyond viewport; no functional impact but wastes layout space.

### ab-validation-02
- SEVERITY: P1
- DIMENSION: overflow
- VIEWPORT: all
- EVIDENCE: `section.holo-panel.holo-panel-mid` overflows horizontally at all viewports (+139px at 1920, +234px at 1280, +247px at 1024) and vertically by 811px at 1920x1080 (scrollHeight=1448, clientHeight=637). Content inside the panel is silently clipped by `overflow:hidden`.
- IMPACT: Users cannot see or scroll to content below the fold inside the holo-panel; 811px of vertical content is invisible.

### ab-validation-03
- SEVERITY: P1
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: Page has two `<h1>` elements simultaneously visible: "A/B Validation" (in banner div, outside `<main>`) and "Predictive Routing Validation" (inside `<main>`). First H1 is at top=92px while `<main>` starts at top=208px.
- IMPACT: Screen readers announce two document titles, creating ambiguity about the page's primary heading. Violates WCAG 1.3.1.

### ab-validation-04
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: "Refresh" and "Start Jarvis" buttons lack explicit `type` attribute (hasAttribute('type') = false). Browser defaults to `type="submit"`, which can trigger unintended form submissions.
- IMPACT: Implicit submit type may cause unexpected behavior if buttons are ever placed inside a `<form>`. Also a code quality concern.

### ab-validation-05
- SEVERITY: P2
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: "Refresh" button click produces no console output, no network request, no visible change, and no user feedback in demo mode.
- IMPACT: User clicks a clearly labeled action button and gets zero feedback — silent failure with no indication that the feature requires the desktop runtime.

### ab-validation-06
- SEVERITY: P2
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: "Start Jarvis" button click produces no console output, no network request, no visible change, and no user feedback in demo mode.
- IMPACT: Same silent-failure pattern as Refresh — no feedback that the action cannot be performed.

### ab-validation-07
- SEVERITY: P1
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: The "Error: desktop runtime unavailable" message (shown after clicking "Run A/B Validation") renders in a plain `<div>` with no `role="alert"` or `aria-live` attribute. The "Desktop runtime required" banner text in the header also has no ARIA live region.
- IMPACT: Screen readers will not announce the error or status messages. Users relying on assistive technology will not know the action failed. Violates WCAG 4.1.3.

### ab-validation-08
- SEVERITY: P2
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: "Dismiss" button (appears after runtime error) lacks explicit `type` attribute (hasAttribute('type') = false).
- IMPACT: Same implicit-submit concern as ab-validation-04.

### ab-validation-09
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: The "Run Validation" button inside the "Getting Started" section also lacks explicit `type` attribute (hasAttribute('type') = false). Found via initial page scan before click sequence altered state.
- IMPACT: Same implicit-submit concern as ab-validation-04.

## Summary
- Gate detected: no
- Total interactive elements: 4
- Elements clicked: 4
- P0: 0
- P1: 3
- P2: 6
