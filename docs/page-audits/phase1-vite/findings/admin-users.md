# Audit: Admin Users
URL: http://localhost:1420/admin-users
Audited at: 2026-04-10T00:31:00+01:00
Gate detected: false
Gate type: none

## Console (captured at 992x639 effective viewport, ALL messages)
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

> NOTE: `window.resizeTo()` did not change the actual viewport (stuck at 992x639 for all three requested sizes). Measurements below are at the single effective viewport of 992x639.

### 992x639 (actual — requested 1920x1080, 1280x800, 1024x768 all returned same)
- documentElement: scrollWidth=992 clientWidth=992 [OK]
- body: scrollWidth=992 clientWidth=992 [OK]
- main `main.nexus-shell-content`: scrollWidth=731 clientWidth=731 [OK]
- `div.living-background`: scrollWidth=1071 clientWidth=992 [OVERFLOW +79px]
- `section.holo-panel`: scrollWidth=741 clientWidth=681 [OVERFLOW +60px]

## Interactive elements (main content area, excluding sidebar nav)

| # | Label | Type | Href | Enabled |
|---|-------|------|------|---------|
| 1 | Refresh | button (no type attr) | — | yes |
| 2 | Start Jarvis | button (no type attr) | — | yes |
| 3 | Retry | button (no type attr) | — | yes |
| 4 | Filter users... | input (no type attr, placeholder only) | — | yes |
| 5 | + Add User | button (no type attr) | — | yes |

## Click sequence

### Click 1: "Refresh"
- Pathname before: /admin-users
- New console: clean
- Network failures: none
- Visible change: none
- Pathname after: /admin-users
- Reverted: n/a

### Click 2: "Start Jarvis"
- Pathname before: /admin-users
- New console: clean
- Network failures: none
- Visible change: none
- Pathname after: /admin-users
- Reverted: n/a

### Click 3: "Retry"
- Pathname before: /admin-users
- New console: clean
- Network failures: none
- Visible change: none
- Pathname after: /admin-users
- Reverted: n/a

### Click 4: "Filter users..." (input)
- Pathname before: /admin-users
- New console: clean
- Network failures: none
- Visible change: input received focus
- Pathname after: /admin-users
- Reverted: n/a

### Click 5: "+ Add User"
- Pathname before: /admin-users
- New console: clean
- Network failures: none
- Visible change: none
- Pathname after: /admin-users
- Reverted: n/a

### Skipped (destructive)
none

### Total interactive elements found: 5
### Elements clicked: 5

## Accessibility
- Images without alt: 0
- Inputs without label: 1 (selectors: `input[placeholder="Filter users..."]` — no id, no `<label>`, no aria-label, no aria-labelledby)
- Buttons without accessible name: 0
- Duplicate H1: 2 — "Admin Users" (in banner/header area) and "User Management" (in main content)

## Findings

### admin-users-01
- SEVERITY: P1
- DIMENSION: overflow
- VIEWPORT: 992 (all tested viewports returned 992)
- EVIDENCE: `div.living-background` scrollWidth=1071 > clientWidth=992 (+79px overflow)
- IMPACT: Horizontal scroll or clipped content on the background layer; may cause layout shift or scrollbar flash.

### admin-users-02
- SEVERITY: P1
- DIMENSION: overflow
- VIEWPORT: 992 (all tested viewports returned 992)
- EVIDENCE: `section.holo-panel` scrollWidth=741 > clientWidth=681 (+60px overflow)
- IMPACT: Content panel overflows its container, risking clipped interactive elements or unintended horizontal scrollbar.

### admin-users-03
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: Two `<h1>` elements on the page — "Admin Users" (banner area, outside `<main>`) and "User Management" (inside `<main>`). Both are `<h1>`.
- IMPACT: Violates WCAG heading hierarchy; screen readers announce two top-level headings, confusing document structure.

### admin-users-04
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: `input[placeholder="Filter users..."]` has no `id`, no `<label>`, no `aria-label`, and no `aria-labelledby`. Only `placeholder` provides a hint.
- IMPACT: Screen readers cannot programmatically associate the input with a label; placeholder alone is insufficient for accessibility per WCAG 1.3.1.

### admin-users-05
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: All 4 buttons ("Refresh", "Start Jarvis", "Retry", "+ Add User") lack an explicit `type` attribute. They default to `type="submit"`, which can cause unintended form submissions if wrapped in a `<form>`.
- IMPACT: Buttons may trigger form submission instead of intended action if a parent `<form>` is added during refactoring.

### admin-users-06
- SEVERITY: P2
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: "Refresh" button click produces zero console output, zero network activity, and zero visible change.
- IMPACT: Button appears inert in demo mode with no user feedback — user cannot tell if it worked or failed.

### admin-users-07
- SEVERITY: P2
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: "Start Jarvis" button click produces zero console output, zero network activity, and zero visible change.
- IMPACT: Button appears inert in demo mode with no user feedback.

### admin-users-08
- SEVERITY: P2
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: "Retry" button (desktop runtime reconnection) click produces zero console output, zero network activity, and zero visible change.
- IMPACT: Button appears inert in demo mode with no user feedback.

### admin-users-09
- SEVERITY: P2
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: "+ Add User" primary CTA button click produces zero console output, zero network activity, and zero visible change.
- IMPACT: Primary action button is a silent no-op in demo mode; no modal, toast, or disabled state to indicate why.

### admin-users-10
- SEVERITY: P2
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: User management table `<tbody>` is empty — zero rows rendered. No empty-state message, illustration, or "No users found" placeholder is displayed.
- IMPACT: Users see a table header with column names but no data and no explanation, creating confusion about whether data failed to load or simply doesn't exist.

## Summary
- Gate detected: no
- Total interactive elements: 5
- Elements clicked: 5
- P0: 0
- P1: 2
- P2: 8
