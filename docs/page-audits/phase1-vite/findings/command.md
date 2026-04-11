# Audit: Command
URL: http://localhost:1420/command
Audited at: 2026-04-10T01:47:00Z
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
_Note: Actual CSS viewport locked at 1888x951 (Tauri window). resize_window MCP tool does not change CSS viewport dimensions. All three viewport targets measured at 1888x951._

- documentElement: scrollWidth=1888 clientWidth=1888 [OK]
- body: scrollWidth=1888 clientWidth=1888 [OK]
- main `main.nexus-shell-content`: scrollWidth=1627 clientWidth=1627 [OK]
- other overflowing:
  - `div.living-background`: scrollWidth=2039 clientWidth=1888 [OVERFLOW +151px]
  - `section.holo-panel.holo-panel-mid`: scrollWidth=1716 clientWidth=1577 [OVERFLOW +139px]
  - `span.nexus-sidebar-item-text` (x3): scrollWidth=157 clientWidth=153 [OVERFLOW +4px, sidebar text truncation]

### 1280x800
_Could not be measured — viewport locked at 1888x951. See note above._

### 1024x768
_Could not be measured — viewport locked at 1888x951. See note above._

## Interactive elements (main content only)
| # | Label | Type | Href | Enabled |
|---|-------|------|------|---------|
| 1 | Refresh | button (no type attr) | — | yes |
| 2 | Start Jarvis | button (no type attr) | — | yes |

_Note: Both buttons are in the page header/banner area (outside `<main>`), but are part of the Command Center content area (not sidebar). Zero interactive elements exist inside `<main>` itself._

## Click sequence
### Click 1: "Refresh"
- Pathname before: /command
- New console: clean (zero new messages)
- Network failures: none
- Visible change: none — no feedback, no spinner, no toast
- Pathname after: /command
- Reverted: n/a

### Click 2: "Start Jarvis"
- Pathname before: /command
- New console: clean (zero new messages)
- Network failures: none
- Visible change: none — no feedback, no error message, no toast
- Pathname after: /command
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

### command-01
- SEVERITY: P2
- DIMENSION: overflow
- VIEWPORT: 1920
- EVIDENCE: `div.living-background` scrollWidth=2039 exceeds clientWidth=1888 by 151px. Background decoration element overflows the viewport.
- IMPACT: May cause horizontal scrollbar or clip issues on narrower viewports; cosmetic at current size but indicates missing `overflow:hidden` on the background layer.

### command-02
- SEVERITY: P2
- DIMENSION: overflow
- VIEWPORT: 1920
- EVIDENCE: `section.holo-panel.holo-panel-mid` scrollWidth=1716 exceeds clientWidth=1577 by 139px inside `<main>`.
- IMPACT: Main content panel overflows its container; content may be clipped or trigger unexpected horizontal scroll within the main area.

### command-03
- SEVERITY: P2
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: "Refresh" button (`button.nx-btn.nx-btn-ghost`) lacks `type` attribute; `button.type` resolves to `"submit"`. "Start Jarvis" button (`button.nx-btn.nx-btn-primary`) also lacks `type` attribute; resolves to `"submit"`.
- IMPACT: If either button is ever placed inside a `<form>`, clicking it will submit the form instead of executing the intended action. Both should have `type="button"`.

### command-04
- SEVERITY: P2
- DIMENSION: action
- VIEWPORT: all
- EVIDENCE: Clicking "Refresh" produces zero console output, zero network requests, and zero visible change. Clicking "Start Jarvis" also produces zero console output, zero network requests, and zero visible change. Neither button provides any user feedback in demo mode.
- IMPACT: Users clicking either CTA get no indication the click was received. At minimum, a "desktop runtime required" toast or disabled state with tooltip would communicate that the feature is unavailable.

### command-05
- SEVERITY: P2
- DIMENSION: copy
- VIEWPORT: all
- EVIDENCE: "No agents registered" text appears twice in the DOM: once as `p.cc-subtitle` inside `header[role="banner"] > header` (the panel header), and again as `<h3>` in the empty-state area of the same panel. Both are visible simultaneously.
- IMPACT: Redundant identical text confuses the visual hierarchy and is read twice by screen readers.

### command-06
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: Two `<section>` elements inside `<main>` (`section.holo-panel.holo-panel-mid.nexus-page-panel` and `section.cc-hub`) have no `role`, `aria-label`, or `aria-labelledby` attributes.
- IMPACT: Screen readers announce generic "section" landmarks with no distinguishing labels, reducing navigability for assistive technology users.

### command-07
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: `<h1>Command Center</h1>` is outside `<main>` (in the page header banner). Inside `<main>`, the heading hierarchy starts at H2 ("COMMAND CENTER // LIVE AGENT GRID") then H3 ("No agents registered"). The H1 is not inside any landmark that assistive tech can associate with the main content.
- IMPACT: Screen reader users navigating by headings may not find the H1 associated with the main content region. The page heading should be inside or programmatically linked to `<main>`.

## Summary
- Gate detected: no
- Total interactive elements: 2
- Elements clicked: 2
- P0: 0
- P1: 0
- P2: 7
