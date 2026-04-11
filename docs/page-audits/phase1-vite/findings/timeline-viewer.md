# Audit: Timeline Viewer
URL: http://localhost:1420/timeline-viewer
Audited at: 2026-04-09T23:56:00+01:00
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
_(Viewport locked at 1888x951 — resize_window had no effect; measurements taken at actual 1888x951)_
- documentElement: scrollWidth=1888 clientWidth=1888 [OK]
- body: scrollWidth=1888 clientWidth=1888 [OK]
- main `main.nexus-shell-content`: scrollWidth=1627 clientWidth=1627 [OK]
- other overflowing:
  - `div.living-background`: scrollWidth=2039 clientWidth=1888 [OVERFLOW +151px]
  - `section.holo-panel.holo-panel-mid`: scrollWidth=1716 clientWidth=1577 [OVERFLOW +139px, masked by overflow:hidden]
  - `span.nexus-sidebar-item-text` (x3): scrollWidth=157 clientWidth=153 [OVERFLOW +4px, text truncation in sidebar]

### 1280x800
_(Viewport did not change from 1888x951 — resize_window no-op. Same measurements as above.)_

### 1024x768
_(Viewport did not change from 1888x951 — resize_window no-op. Same measurements as above.)_

## Interactive elements (main content only)
| # | Label | Type | Href | Enabled |
|---|-------|------|------|---------|
| 1 | Refresh | button[submit] | — | true |
| 2 | Start Jarvis | button[submit] | — | true |

_Note: Both buttons are in the header bar. The main content section (`<main>`) contains zero interactive elements._

## Click sequence
### Click 1: "Refresh"
- Pathname before: /timeline-viewer
- New console: clean
- Network failures: none
- Visible change: none — silent no-op
- Pathname after: /timeline-viewer
- Reverted: n/a

### Click 2: "Start Jarvis"
- Pathname before: /timeline-viewer
- New console: clean
- Network failures: none
- Visible change: none — silent no-op
- Pathname after: /timeline-viewer
- Reverted: n/a

### Skipped (destructive)
none

### Total interactive elements found: 2
### Elements clicked: 2 (of 2)

## Accessibility
- Images without alt: 0
- Inputs without label: 0
- Buttons without accessible name: 0

## Findings

### timeline-viewer-01
- SEVERITY: P1
- DIMENSION: copy
- VIEWPORT: all
- EVIDENCE: A `<div>` inside `<main>` at position (600, 359) displays "desktop runtime unavailable" in red text (color: rgb(248, 113, 113), font-size: 16px, display: block, visibility: visible, opacity: 1). The element has no class, no aria role, and no semantic wrapper. It is fully visible to users on page load.
- IMPACT: Users see a raw error string with no actionable guidance; it looks like an unhandled crash rather than an expected demo-mode limitation.

### timeline-viewer-02
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: Two `<h1>` elements exist on the page: "Timeline Viewer" in the header bar (outside `<main>`) and "TIMELINE VIEWER" inside `<main>`. Pages must have exactly one `<h1>` per WCAG best practices.
- IMPACT: Screen readers announce two document titles, confusing the page hierarchy for assistive technology users.

### timeline-viewer-03
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: The first `<h1>` ("Timeline Viewer") is in the header bar, outside the `<main>` landmark. The primary page heading should be within the main content region.
- IMPACT: Screen reader users navigating by landmarks may miss the page title when jumping to `<main>`.

### timeline-viewer-04
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: Heading hierarchy inside `<main>` skips from H1 ("TIMELINE VIEWER") directly to H3 ("Timeline Tree", "Fork Detail") with no H2 in between.
- IMPACT: Violates WCAG 1.3.1 heading hierarchy; screen reader navigation by heading level becomes unreliable.

### timeline-viewer-05
- SEVERITY: P2
- DIMENSION: overflow
- VIEWPORT: 1920x1080 (actual 1888x951)
- EVIDENCE: `div.living-background` has scrollWidth=2039, clientWidth=1888 — overflows by 151px. This is a decorative background layer.
- IMPACT: No visible scrollbar appears (likely overflow:hidden on parent), but the element exceeds viewport bounds, potentially affecting compositing performance.

### timeline-viewer-06
- SEVERITY: P2
- DIMENSION: overflow
- VIEWPORT: 1920x1080 (actual 1888x951)
- EVIDENCE: `section.holo-panel.holo-panel-mid` has scrollWidth=1716, clientWidth=1577 — overflows by 139px. CSS overflow is set to `hidden`, masking the excess content.
- IMPACT: Content inside the holo-panel may be clipped without the user's knowledge; any future dynamic content that extends rightward will be silently hidden.

### timeline-viewer-07
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: `<nav>` element in the sidebar has no `aria-label` or `aria-labelledby` attribute.
- IMPACT: Screen readers announce "navigation" without identifying which navigation region it is, reducing usability when multiple landmarks exist.

### timeline-viewer-08
- SEVERITY: P2
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: "Refresh" button (header bar) produces no console output, no network request, and no visible change when clicked. Silent no-op in demo mode.
- IMPACT: Users clicking "Refresh" receive zero feedback; no loading indicator, no toast, no error — the button appears broken.

### timeline-viewer-09
- SEVERITY: P2
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: "Start Jarvis" button (header bar) produces no console output, no network request, and no visible change when clicked. Silent no-op in demo mode.
- IMPACT: Users clicking "Start Jarvis" receive zero feedback — indistinguishable from a dead button.

## Summary
- Gate detected: no
- Total interactive elements: 2
- Elements clicked: 2
- P0: 0
- P1: 1
- P2: 8
