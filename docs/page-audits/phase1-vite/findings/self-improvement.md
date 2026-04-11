# Audit: Self Improvement
URL: http://localhost:1420/self-improvement
Audited at: 2026-04-09T23:17:00+01:00
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
- `%cDownload the React DevTools for a better development experience: https://reactjs.org/link/react-devtools font-weight:bold` — chunk-NUMECXU6.js:21550:24

### Debug
- `[vite] connecting...` — @vite/client:494:8
- `[vite] connected.` — @vite/client:617:14

## Overflow

### 1920x1080
(actual viewport locked at 1888x951 — Chrome extension tab context prevents resize)
- documentElement: scrollWidth=1888 clientWidth=1888 [OK]
- body: scrollWidth=1888 clientWidth=1888 [OK]
- main `main.nexus-shell-content`: scrollWidth=1627 clientWidth=1627 [OK]
- other overflowing:
  - `div.living-background`: scrollWidth=2039 clientWidth=1888 [OVERFLOW — fixed-position decorative layer]
  - `section.holo-panel`: scrollWidth=1716 clientWidth=1577 [OVERFLOW — clipped by overflow:hidden]
  - `span.nexus-sidebar-item-text` (x3): scrollWidth=157 clientWidth=153 [OVERFLOW — sidebar text truncation]

### 1280x800
(viewport locked at 1888x951 — resize_window and window.resizeTo() have no effect in Chrome extension tab context)
- Measurements identical to 1920x1080 above.

### 1024x768
(viewport locked at 1888x951 — resize_window and window.resizeTo() have no effect in Chrome extension tab context)
- Measurements identical to 1920x1080 above.

## Interactive elements (main content only)
| # | Label | Type | Href | Enabled |
|---|-------|------|------|---------|
| 1 | Refresh | button (no type attr) | — | yes |
| 2 | Start Jarvis | button (no type attr) | — | yes |
| 3 | Run Improvement Cycle | button[type=button] | — | yes |

## Click sequence
### Click 1: "Refresh"
- Pathname before: /self-improvement
- New console: clean
- Network failures: none
- Visible change: none
- Pathname after: /self-improvement
- Reverted: n/a

### Click 2: "Start Jarvis"
- Pathname before: /self-improvement
- New console: clean
- Network failures: none
- Visible change: none
- Pathname after: /self-improvement
- Reverted: n/a

### Click 3: "Run Improvement Cycle"
- Pathname before: /self-improvement
- New console: clean
- Network failures: none
- Visible change: none
- Pathname after: /self-improvement
- Reverted: n/a

### Skipped (destructive)
none

### Total interactive elements found: 3
### Elements clicked: 3 (all)

## Accessibility
- Images without alt: 0
- Inputs without label: 0
- Buttons without accessible name: 0
- Duplicate H1: 2 (`"Self-Improvement"` in header bar, `"Governed Self-Improvement"` in main content)
- Buttons missing `type` attribute: 2 (`"Refresh"`, `"Start Jarvis"` — default to `type="submit"`)
- Sections without ARIA labels: 7 `<section>` elements in main content have no `aria-label`, `aria-labelledby`, or `role` attribute

## Findings

### self-improvement-01
- SEVERITY: P1
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: All 3 buttons ("Refresh", "Start Jarvis", "Run Improvement Cycle") produce no console output, no network requests, and no visible UI change when clicked. Zero user feedback on interaction.
- IMPACT: Users receive no indication that their action was received or that the feature requires a backend; buttons appear completely broken.

### self-improvement-02
- SEVERITY: P2
- DIMENSION: overflow
- VIEWPORT: all
- EVIDENCE: `div.living-background` (position:fixed) has scrollWidth=2039 vs clientWidth=1888, overflowing by 151px. `section.holo-panel` has scrollWidth=1716 vs clientWidth=1577, overflowing by 139px but masked by `overflow:hidden`.
- IMPACT: `living-background` overflow is cosmetic (fixed decorative layer). `holo-panel` overflow:hidden may clip content at narrower viewports.

### self-improvement-03
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: Two `<h1>` elements on the page: `"Self-Improvement"` (header bar) and `"Governed Self-Improvement"` (main content). Violates single-H1 best practice.
- IMPACT: Screen readers and SEO parsers may be confused about the primary heading of the page.

### self-improvement-04
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: "Refresh" and "Start Jarvis" buttons have no `type` attribute, defaulting to `type="submit"`. These buttons are not inside a `<form>` element but the semantic usage is incorrect.
- IMPACT: If these buttons were ever placed inside a form, they would trigger form submission unintentionally.

### self-improvement-05
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: 7 `<section>` elements in main content lack `aria-label`, `aria-labelledby`, or `role` attributes. Sections contain headed regions ("Pipeline Status", "10 Hard Invariants", "Signals", "Opportunities", "Pending Proposals", "History") but are not programmatically labelled.
- IMPACT: Assistive technology cannot identify or navigate between content regions.

### self-improvement-06
- SEVERITY: P1
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: "Pipeline Status" section permanently displays "Loading status..." and "10 Hard Invariants" section permanently displays "Loading invariants..." with no timeout, error state, or fallback content in demo mode.
- IMPACT: Users see a perpetual loading state with no indication that data will never arrive in demo mode; appears broken rather than intentionally limited.

### self-improvement-07
- SEVERITY: P1
- DIMENSION: copy
- VIEWPORT: all
- EVIDENCE: The inline text "Error: desktop runtime unavailable" is rendered as static content within the main hero section (ref_226), not as an alert, toast, or dismissible message. It appears alongside feature badges ("5-Stage Pipeline", "10 Hard Invariants", "Tier3 HITL Required") as if it were a feature descriptor.
- IMPACT: The error looks like a permanent page label rather than a transient status message, confusing the page hierarchy and alarming users unnecessarily.

## Summary
- Gate detected: no
- Total interactive elements: 3
- Elements clicked: 3
- P0: 0
- P1: 3
- P2: 4
