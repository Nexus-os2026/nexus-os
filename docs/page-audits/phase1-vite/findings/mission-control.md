# Audit: Mission Control
URL: http://localhost:1420/mission-control
Audited at: 2026-04-10T01:53:00Z
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
Measured at actual viewport: 1888x951 (browser chrome consumes difference)
- documentElement: scrollWidth=1888 clientWidth=1888 [OK]
- body: scrollWidth=1888 clientWidth=1888 [OK]
- main `main.nexus-shell-content`: scrollWidth=1627 clientWidth=1627 [OK]
- other overflowing:
  - `div.living-background`: scrollWidth=2039 clientWidth=1888 — overflow:hidden + position:fixed, not user-visible
  - `section.holo-panel.holo-panel-mid.nexus-page-panel`: scrollWidth=1716 clientWidth=1577 — overflow:hidden, not user-visible
  - `div.mc-reactor-ring`: scrollWidth=220 clientWidth=200 — overflow:visible, potential visual clip

### 1280x800
Not measured — `resize_window` MCP tool does not change CSS viewport dimensions (confirmed limitation in prior audits, see observations 1319-1321).

### 1024x768
Not measured — same limitation as above.

## Interactive elements (main content only)
| # | Label | Type | Href | Enabled |
|---|-------|------|------|---------|
| 1 | Open Agents | button[type=button] | n/a | true |
| 2 | Open Chat | button[type=button] | n/a | true |
| 3 | Inspect Cognition | button[type=button] | n/a | true |

## Click sequence
### Click 1: "Open Agents"
- Pathname before: /mission-control
- New console: clean (no new messages on mission-control page)
- Network failures: none
- Visible change: Navigated to Agents page
- Pathname after: /agents
- Reverted: yes (navigated back to /mission-control)

### Click 2: "Open Chat"
- Pathname before: /mission-control
- New console: clean
- Network failures: none
- Visible change: Navigated to Chat page
- Pathname after: /chat
- Reverted: yes (navigated back to /mission-control)

### Click 3: "Inspect Cognition"
- Pathname before: /mission-control
- New console: clean
- Network failures: none
- Visible change: Navigated to Consciousness page
- Pathname after: /consciousness
- Reverted: yes (navigated back to /mission-control)

### Skipped (destructive)
none

### Total interactive elements found: 3
### Elements clicked: 3

## Accessibility
- Images without alt: 0
- Inputs without label: 0
- Buttons without accessible name: 0
- Sections without ARIA label or role: 3
  - `section.holo-panel.holo-panel-mid.nexus-page-panel` (heading: "Runtime neural map")
  - `section.mc-hero.nx-spatial-container` (heading: "Runtime neural map")
  - `section.mc-section` (heading: "Recent Activity")

## Findings

### mission-control-01
- SEVERITY: P2
- DIMENSION: overflow
- VIEWPORT: 1920
- EVIDENCE: `div.mc-reactor-ring` has scrollWidth=220, clientWidth=200, overflow:visible. Content exceeds its container by 20px without clipping.
- IMPACT: Decorative reactor-ring element may cause minor visual overflow or clipping artifacts.

### mission-control-02
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: H1 "Mission Control" is outside the `<main>` landmark. It resides in `DIV.flex.flex-wrap.items-center.gap-2.5` within the page header area, not inside `main.nexus-shell-content`.
- IMPACT: Screen readers navigating by main landmark will miss the page heading, reducing orientation for assistive technology users.

### mission-control-03
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: 3 of 7 `<section>` elements inside `<main>` lack both `role` and `aria-label` attributes: `section.holo-panel.holo-panel-mid.nexus-page-panel`, `section.mc-hero.nx-spatial-container`, `section.mc-section` (Recent Activity).
- IMPACT: Screen readers cannot convey section purpose; users must rely on heading discovery to understand page structure.

## Summary
- Gate detected: no
- Total interactive elements: 3
- Elements clicked: 3
- P0: 0
- P1: 0
- P2: 3
