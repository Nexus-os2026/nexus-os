# Audit: Usage Billing
URL: http://localhost:1420/usage-billing
Audited at: 2026-04-10T00:57:00Z
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
(Actual viewport 1888x951 — resize_window MCP tool does not honor requested dimensions)
- documentElement: scrollWidth=1888 clientWidth=1888 [OK]
- body: scrollWidth=1888 clientWidth=1888 [OK]
- main `main`: scrollWidth=1627 clientWidth=1627 [OK]
- other overflowing:
  - `div.living-background`: scrollWidth=2039 clientWidth=1888 [OVERFLOW +151px]
  - `section.holo-panel.holo-panel-mid`: scrollWidth=1716 clientWidth=1577 [OVERFLOW +139px]
  - `span.nexus-sidebar-item-text` (x3): scrollWidth=157 clientWidth=153 [OVERFLOW +4px, minor text clipping]

### 1280x800
Not measurable — resize_window MCP tool locked viewport at 1888x951. Deferred to Puppeteer screenshot pass.

### 1024x768
Not measurable — resize_window MCP tool locked viewport at 1888x951. Deferred to Puppeteer screenshot pass.

## Interactive elements (main content only)
| # | Label | Type | Href | Enabled |
|---|-------|------|------|---------|
| 1 | Hour | button (no type attr) | — | yes |
| 2 | Day | button (no type attr) | — | yes |
| 3 | Week | button (no type attr) | — | yes |
| 4 | Month | button (no type attr) | — | yes |
| 5 | Export CSV | button (no type attr) | — | yes |
| 6 | (threshold input) | input[type=number] | — | yes |
| 7 | Set Alert | button (no type attr) | — | yes |

Note: "Refresh" and "Start Jarvis" buttons are rendered outside `<main>` (in header area) — listed separately below.

### Outside main (not in sidebar)
| # | Label | Type | Href | Enabled |
|---|-------|------|------|---------|
| A | Refresh | button (no type attr) | — | yes |
| B | Start Jarvis | button (no type attr) | — | yes |

## Click sequence
### Click 1: "Refresh"
- Pathname before: /usage-billing
- New console: clean
- Network failures: none
- Visible change: none
- Pathname after: /usage-billing
- Reverted: n/a

### Click 2: "Start Jarvis"
- Pathname before: /usage-billing
- New console: clean
- Network failures: none
- Visible change: none
- Pathname after: /usage-billing
- Reverted: n/a

### Click 3: "Hour"
- Pathname before: /usage-billing
- New console: clean
- Network failures: none
- Visible change: none (no visual tab switch, "Month" stays active via CSS class)
- Pathname after: /usage-billing
- Reverted: n/a

### Click 4: "Day"
- Pathname before: /usage-billing
- New console: clean
- Network failures: none
- Visible change: none
- Pathname after: /usage-billing
- Reverted: n/a

### Click 5: "Week"
- Pathname before: /usage-billing
- New console: clean
- Network failures: none
- Visible change: none
- Pathname after: /usage-billing
- Reverted: n/a

### Click 6: "Month"
- Pathname before: /usage-billing
- New console: clean
- Network failures: none
- Visible change: none (already has admin-tab--active class)
- Pathname after: /usage-billing
- Reverted: n/a

### Click 7: "Export CSV"
- Pathname before: /usage-billing
- New console: clean
- Network failures: none
- Visible change: none
- Pathname after: /usage-billing
- Reverted: n/a

### Click 8: "Set Alert"
- Pathname before: /usage-billing
- New console: clean
- Network failures: none
- Visible change: none
- Pathname after: /usage-billing
- Reverted: n/a

### Skipped (destructive)
none

### Total interactive elements found: 9 (7 in main + 2 outside main)
### Elements clicked: 8 (capped at 10)

## Accessibility
- Images without alt: 0
- Inputs without label: 1 (selectors: `input[type=number][placeholder="Threshold in USD (e.g. 50.00)"]` — no id, no name, no aria-label, no associated label element)
- Buttons without accessible name: 0
- Additional:
  - Duplicate H1: "Usage & Billing" appears twice — once outside `<main>` (in header area), once inside `<main>`
  - 8 buttons missing `type` attribute (6 in main: Hour, Day, Week, Month, Export CSV, Set Alert; 2 outside main: Refresh, Start Jarvis)
  - Time-range tabs (Hour/Day/Week/Month) use CSS class `admin-tab--active` for active state but lack `role="tab"`, `aria-selected`, or `aria-pressed`
  - `<header>` is nested inside a `<div>`, not a direct child of `<body>` — loses implicit `banner` landmark role
  - No `<footer>` or `contentinfo` landmark

## Findings

### usage-billing-01
- SEVERITY: P1
- DIMENSION: overflow
- VIEWPORT: 1920
- EVIDENCE: `div.living-background` scrollWidth=2039 exceeds clientWidth=1888 by 151px. `section.holo-panel.holo-panel-mid` scrollWidth=1716 exceeds clientWidth=1577 by 139px.
- IMPACT: Horizontal scrollbar may appear or content may be clipped; background bleeds beyond viewport bounds.

### usage-billing-02
- SEVERITY: P1
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: Two `<h1>` elements with identical text "Usage & Billing" — one in the header area outside `<main>`, one inside `<main>`. Document should have a single H1 for correct heading hierarchy.
- IMPACT: Screen readers announce duplicate page titles, confusing document structure for assistive technology users.

### usage-billing-03
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: `input[type=number]` with placeholder "Threshold in USD (e.g. 50.00)" has no `id`, no `name`, no `aria-label`, no `aria-labelledby`, and no associated `<label>` element.
- IMPACT: Screen readers cannot announce the input's purpose; form data cannot be submitted without a name attribute.

### usage-billing-04
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: Time-range buttons (Hour, Day, Week, Month) use CSS class `admin-tab` / `admin-tab--active` for visual state but have no `role="tab"`, no `aria-selected`, no `aria-pressed`. "Month" has `admin-tab--active` class but this is invisible to assistive technology.
- IMPACT: Screen readers cannot convey which time range is currently selected; tab pattern is not exposed to accessibility tree.

### usage-billing-05
- SEVERITY: P2
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: All 8 buttons (Refresh, Start Jarvis, Hour, Day, Week, Month, Export CSV, Set Alert) produce no console output, no network requests, and no visible UI change when clicked. Time-range tabs do not switch the active tab class. Export CSV does not trigger a download.
- IMPACT: Page is entirely non-functional in demo mode — no mock behavior for any interactive element.

### usage-billing-06
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: 8 buttons missing explicit `type` attribute: Hour, Day, Week, Month, Export CSV, Set Alert (in main), Refresh, Start Jarvis (outside main). All default to `type="submit"` which can cause unintended form submissions.
- IMPACT: Buttons inside or near forms may trigger unexpected submit behavior.

### usage-billing-07
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: "Refresh" and "Start Jarvis" buttons are rendered outside both `<main>` and `aside.nexus-sidebar-shell`. The `<header>` element is nested inside a `<div>` (not a direct child of `<body>`), losing its implicit `banner` landmark role.
- IMPACT: Skip-to-main-content patterns miss these controls; landmark navigation is incomplete.

## Summary
- Gate detected: no
- Total interactive elements: 9 (7 in main + 2 outside main)
- Elements clicked: 8
- P0: 0
- P1: 2
- P2: 5
