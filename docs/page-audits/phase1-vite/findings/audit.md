# Audit: Audit
URL: http://localhost:1420/audit
Audited at: 2026-04-09T21:12:00Z
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
- `Download the React DevTools for a better development experience: https://reactjs.org/link/react-devtools` — chunk-NUMECXU6.js?v=5144749d:21550:24

### Debug
- `[vite] connecting...` — @vite/client:494:8
- `[vite] connected.` — @vite/client:617:14

## Overflow

### 1920x1080
- documentElement: scrollWidth=1888 clientWidth=1888 [OK]
- body: scrollWidth=1888 clientWidth=1888 [OK]
- main `main`: scrollWidth=1627 clientWidth=1627 [OK]
- `section.holo-panel.nexus-page-panel`: scrollWidth=1670 clientWidth=1577 [OVERFLOW +93px]
- `div.living-background`: scrollWidth=2037 clientWidth=1888 [OVERFLOW +149px]
- `.holo-panel__refraction`: offsetWidth=2208 (decorative element inside overflow:hidden container)

### 1280x800
Actual viewport: 1248x615
- documentElement: scrollWidth=1248 clientWidth=1248 [OK]
- body: scrollWidth=1248 clientWidth=1248 [OK]
- main `main`: scrollWidth=987 clientWidth=987 [OK]
- `section.holo-panel.nexus-page-panel`: scrollWidth=1003 clientWidth=937 [OVERFLOW +66px]
- `div.living-background`: scrollWidth=1346 clientWidth=1248 [OVERFLOW +98px]

### 1024x768
Actual viewport: 992x583
- documentElement: scrollWidth=992 clientWidth=992 [OK]
- body: scrollWidth=992 clientWidth=992 [OK]
- main `main`: scrollWidth=731 clientWidth=731 [OK]
- `section.holo-panel.nexus-page-panel`: scrollWidth=827 clientWidth=681 [OVERFLOW +146px]
- `div.living-background`: scrollWidth=1069 clientWidth=992 [OVERFLOW +77px]

## Interactive elements (main content only)

Audit Log tab (default view):

| # | Label | Type | Href | Enabled |
|---|-------|------|------|---------|
| 1 | VERIFY CHAIN | button[type=button] | — | yes |
| 2 | REFRESH | button[type=button] | — | yes |
| 3 | Audit Log | button[type=button] (tab, active) | — | yes |
| 4 | Statistics | button[type=button] (tab) | — | yes |
| 5 | Distributed Tracing | button[type=button] (tab) | — | yes |
| 6 | Governance Verification | button[type=button] (tab) | — | yes |
| 7 | (search) | input[type=text] placeholder="Search events, payloads, hashes..." | — | yes |
| 8 | All Agents | select | — | yes |
| 9 | All Status | select | — | yes |
| 10 | All Severity | select | — | yes |
| 11 | All Time | select | — | yes |

Additional elements on other tabs (discovered during click sequence):
- Statistics tab: +1 select (All Time), +1 REFRESH button
- Distributed Tracing tab: +7 inputs, +1 select, +3 buttons (START TRACE, START SPAN, END SPAN), +1 REFRESH
- Governance Verification tab: +1 VERIFY ALL button, +1 invariant input, +1 VERIFY button, +1 EXPORT REPORT button

## Click sequence

### Click 1: "VERIFY CHAIN"
- Pathname before: /audit
- New console: clean
- Network failures: none
- Visible change: Status line changed to "CHAIN VALID (0 events)" — verification completed successfully
- Pathname after: /audit
- Reverted: n/a

### Click 2: "REFRESH"
- Pathname before: /audit
- New console: clean
- Network failures: none
- Visible change: No visible change (already showing 0 events)
- Pathname after: /audit
- Reverted: n/a

### Click 3: "Audit Log" (tab)
- Pathname before: /audit
- New console: clean
- Network failures: none
- Visible change: None — already active tab
- Pathname after: /audit
- Reverted: n/a

### Click 4: "Statistics" (tab)
- Pathname before: /audit
- New console: clean
- Network failures: none
- Visible change: Tab content switched to "Audit Statistics" view with "Click REFRESH to load statistics." message, time range select, and REFRESH button
- Pathname after: /audit
- Reverted: n/a

### Click 5: "Distributed Tracing" (tab)
- Pathname before: /audit
- New console: clean
- Network failures: none
- Visible change: Tab content switched to distributed tracing view with START TRACE, START SPAN, END SPAN forms and "desktop runtime unavailable" message in Traces section
- Pathname after: /audit
- Reverted: n/a

### Click 6: "Governance Verification" (tab)
- Pathname before: /audit
- New console: clean
- Network failures: none
- Visible change: Tab content switched to governance verification view with VERIFY ALL, specific invariant input + VERIFY, and EXPORT REPORT sections
- Pathname after: /audit
- Reverted: n/a

### Click 7: "VERIFY ALL"
- Pathname before: /audit
- New console: clean
- Network failures: none
- Visible change: "desktop runtime unavailable" message appeared — expected in demo mode
- Pathname after: /audit
- Reverted: n/a

### Click 8: "VERIFY" (specific invariant)
- Pathname before: /audit
- New console: clean
- Network failures: none
- Visible change: "desktop runtime unavailable" message — expected in demo mode
- Pathname after: /audit
- Reverted: n/a

### Click 9: "EXPORT REPORT"
- Pathname before: /audit
- New console: clean
- Network failures: none
- Visible change: No visible change — likely requires backend (expected in demo mode)
- Pathname after: /audit
- Reverted: n/a

### Click 10: "Audit Log" (tab — return)
- Pathname before: /audit
- New console: clean
- Network failures: none
- Visible change: Tab content returned to Audit Log view with search, filters, and empty table
- Pathname after: /audit
- Reverted: n/a

### Skipped (destructive)
none

### Total interactive elements found: 11 (Audit Log tab default view); ~30 across all 4 tabs
### Elements clicked: 10 (capped at 10)

## Accessibility
- Images without alt: 0
- Inputs without label: 5 on Audit Log tab (selectors: `input.audit-search[placeholder="Search events, payloads, hashes..."]`, `select.audit-select` x4); additional ~9 unlabeled inputs on Distributed Tracing tab, ~1 on Governance Verification tab, ~1 on Statistics tab — total ~16 across all tabs
- Buttons without accessible name: 0

## Findings

### audit-01
- SEVERITY: P2
- DIMENSION: overflow
- VIEWPORT: all
- EVIDENCE: `section.holo-panel.nexus-page-panel` overflows at all viewports: +93px at 1920x1080, +66px at 1280x800, +146px at 1024x768. Root cause: `.holo-panel__refraction` decorative child is 2208px wide inside the panel. Container has `overflow:hidden` so it is masked from the user, but the element's scrollWidth exceeds clientWidth.
- IMPACT: Hidden horizontal overflow in the main content panel; currently masked by `overflow:hidden` but could cause layout issues if container overflow policy changes.

### audit-02
- SEVERITY: P2
- DIMENSION: overflow
- VIEWPORT: all
- EVIDENCE: `div.living-background` overflows at all viewports: +149px at 1920x1080, +98px at 1280x800, +77px at 1024x768. Background decorative element exceeds document width.
- IMPACT: Decorative background element exceeds viewport; masked but contributes to potential scroll issues.

### audit-03
- SEVERITY: P1
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: All 5 form inputs on Audit Log tab (1 `input.audit-search`, 4 `select.audit-select`) have no `id`, no `name`, no `aria-label`, and no associated `<label>` element. Additionally, ~11 more inputs across Statistics, Distributed Tracing, and Governance Verification tabs are similarly unlabeled.
- IMPACT: Screen readers cannot identify form controls; users relying on assistive technology cannot determine purpose of search field or filter dropdowns. Total ~16 unlabeled form elements across all tabs.

### audit-04
- SEVERITY: P2
- DIMENSION: interactive
- VIEWPORT: 1920
- EVIDENCE: VERIFY ALL, VERIFY (specific invariant), and EXPORT REPORT buttons on Governance Verification tab all show "desktop runtime unavailable" or no visible feedback. No console errors, no network requests. Buttons are inert in demo mode.
- IMPACT: Expected behavior in demo mode — Tauri backend required for governance verification operations. No user feedback distinguishes between "action attempted but backend unavailable" vs "button broken."

### audit-05
- SEVERITY: P2
- DIMENSION: interactive
- VIEWPORT: 1920
- EVIDENCE: Statistics tab displays "Click REFRESH to load statistics." as initial state. The tab requires an explicit REFRESH click before showing any content, even placeholder data.
- IMPACT: Inconsistent with other tabs which render content immediately. Users may not realize they need to click REFRESH to see statistics.

## Summary
- Gate detected: no
- Total interactive elements: 11 (default Audit Log tab); ~30 across all 4 tabs
- Elements clicked: 10
- P0: 0
- P1: 1
- P2: 4
