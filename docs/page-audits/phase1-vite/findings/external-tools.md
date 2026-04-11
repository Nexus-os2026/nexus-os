# Audit: External Tools
URL: http://localhost:1420/external-tools
Audited at: 2026-04-09T23:04:00+01:00
Gate detected: false
Gate type: none

## Console (captured at 1248x671 — viewport locked, see note)
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

NOTE: `resize_window` MCP tool does not change the actual browser viewport in this environment. All three requested sizes (1920x1080, 1280x800, 1024x768) measured at the locked viewport of 1248x671.

### 1248x671 (actual viewport for all measurements)
- documentElement: scrollWidth=1248 clientWidth=1248 [OK]
- body: scrollWidth=1248 clientWidth=1248 [OK]
- main `main`: scrollWidth=987 clientWidth=987 [OK]
- other overflowing:
  - `div.living-background`: scrollWidth=1348 clientWidth=1248 [OVERFLOW +100px] — position:fixed, overflow:hidden (not user-visible)
  - `section.holo-panel.holo-panel-mid.nexus-page-panel`: scrollWidth=1019 clientWidth=937 [OVERFLOW +82px] — overflow:hidden clips content
  - 3x `span.nexus-sidebar-item-text`: scrollWidth=157 clientWidth=153 [OVERFLOW +4px] — sidebar text truncation

### 1920x1080
(not measurable — viewport locked at 1248x671)

### 1280x800
(not measurable — viewport locked at 1248x671)

### 1024x768
(not measurable — viewport locked at 1248x671)

## Interactive elements (main content only)
| # | Label | Type | Href | Enabled |
|---|-------|------|------|---------|
| 1 | Refresh Availability | button (type=submit) | — | yes |
| 2 | Verify Audit | button (type=submit) | — | yes |
| 3 | Rate Limits | button (type=submit) | — | yes |

Header bar (outside main, outside sidebar):
| # | Label | Type | Href | Enabled |
|---|-------|------|------|---------|
| 4 | Refresh | button (type=submit) | — | yes |
| 5 | Start Jarvis | button (type=submit) | — | yes |

## Click sequence
### Click 1: "Refresh Availability"
- Pathname before: /external-tools
- New console: clean
- Network failures: none
- Visible change: none — silent no-op
- Pathname after: /external-tools
- Reverted: n/a

### Click 2: "Verify Audit"
- Pathname before: /external-tools
- New console: clean
- Network failures: none
- Visible change: none — silent no-op
- Pathname after: /external-tools
- Reverted: n/a

### Click 3: "Rate Limits"
- Pathname before: /external-tools
- New console: clean
- Network failures: none
- Visible change: none — silent no-op
- Pathname after: /external-tools
- Reverted: n/a

### Click 4: "Refresh" (header)
- Pathname before: /external-tools
- New console: clean
- Network failures: none
- Visible change: none — silent no-op
- Pathname after: /external-tools
- Reverted: n/a

### Click 5: "Start Jarvis" (header)
- Pathname before: /external-tools
- New console: clean
- Network failures: none
- Visible change: none — silent no-op
- Pathname after: /external-tools
- Reverted: n/a

### Skipped (destructive)
none

### Total interactive elements found: 5 (3 main + 2 header)
### Elements clicked: 5

## Accessibility
- Images without alt: 0
- Inputs without label: 0
- Buttons without accessible name: 0
- Duplicate h1: 2 (one in banner header "External Tools", one in main section "External Tools")
- Unlabeled section: 1 (`section.holo-panel.holo-panel-mid.nexus-page-panel` — no aria-label, aria-labelledby, or role)
- All 5 buttons use `type="submit"` outside a `<form>` context

## Findings

### external-tools-01
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: Two `<h1>` elements on the page — one in the banner/header region (ref_211) and one inside `main > section` (ref_222). Both read "External Tools".
- IMPACT: Duplicate h1 violates WCAG heading hierarchy; screen readers announce two top-level headings, confusing document structure.

### external-tools-02
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: `section.holo-panel.holo-panel-mid.nexus-page-panel` has no `aria-label`, `aria-labelledby`, or landmark `role`. It is the sole `<section>` in main content.
- IMPACT: Screen readers announce an unnamed region landmark, providing no context to assistive technology users.

### external-tools-03
- SEVERITY: P1
- DIMENSION: overflow
- VIEWPORT: 1248 (all measured)
- EVIDENCE: `section.holo-panel.holo-panel-mid.nexus-page-panel` has scrollWidth=1019 clientWidth=937 (overflow +82px) with `overflow:hidden`. scrollHeight=529 clientHeight=353 (vertical overflow +176px clipped).
- IMPACT: 176px of vertical content silently clipped by `overflow:hidden` — content below the fold is invisible and unreachable without scrolling, but scroll is suppressed.

### external-tools-04
- SEVERITY: P2
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: All 3 main-content buttons ("Refresh Availability", "Verify Audit", "Rate Limits") and both header buttons ("Refresh", "Start Jarvis") produce zero console output, zero network requests, and zero visible DOM changes on click.
- IMPACT: Buttons are completely non-functional in demo mode — no loading state, no toast, no "backend required" message. Users get no feedback that anything happened or why it didn't.

### external-tools-05
- SEVERITY: P2
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: All 5 buttons use `type="submit"` but none are inside a `<form>` element.
- IMPACT: Semantic mismatch — `type="submit"` buttons outside a form have no submit target. Should be `type="button"` to match their non-form behavior.

### external-tools-06
- SEVERITY: P2
- DIMENSION: overflow
- VIEWPORT: 1248 (all measured)
- EVIDENCE: `div.living-background` has scrollWidth=1348 vs clientWidth=1248 (overflow +100px). Element is `position:fixed` with `overflow:hidden`.
- IMPACT: Background element extends 100px beyond viewport. Hidden by overflow:hidden so not user-visible, but contributes to layout calculation anomalies.

## Summary
- Gate detected: no
- Total interactive elements: 5 (3 main + 2 header)
- Elements clicked: 5
- P0: 0
- P1: 1
- P2: 5
