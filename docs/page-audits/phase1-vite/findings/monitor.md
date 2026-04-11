# Audit: Monitor
URL: http://localhost:1420/monitor
Audited at: 2026-04-09T21:10:00Z
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
- main `section.holo-panel.holo-panel-mid.nexus-page-panel`: scrollWidth=1944 clientWidth=1577 [OVERFLOW +367px]
- other overflowing:
  - `div.living-background`: scrollWidth=2038 clientWidth=1888 [OVERFLOW +150px]
  - `span.nexus-sidebar-item-text` (x3): scrollWidth=157 clientWidth=153 [OVERFLOW +4px each]

### 1280x800
- documentElement: scrollWidth=1248 clientWidth=1248 [OK]
- body: scrollWidth=1248 clientWidth=1248 [OK]
- main `section.holo-panel.holo-panel-mid.nexus-page-panel`: scrollWidth=1000 clientWidth=937 [OVERFLOW +63px]
- other overflowing:
  - `div.living-background`: scrollWidth=1347 clientWidth=1248 [OVERFLOW +99px]
  - `span.nexus-sidebar-item-text` (x3): scrollWidth=157 clientWidth=153 [OVERFLOW +4px each]

### 1024x768
- documentElement: scrollWidth=992 clientWidth=992 [OK]
- body: scrollWidth=992 clientWidth=992 [OK]
- main `section.holo-panel.holo-panel-mid.nexus-page-panel`: scrollWidth=929 clientWidth=681 [OVERFLOW +248px]
- other overflowing:
  - `div.living-background`: scrollWidth=1071 clientWidth=992 [OVERFLOW +79px]
  - `span.nexus-sidebar-item-text` (x3): scrollWidth=157 clientWidth=153 [OVERFLOW +4px each]

## Interactive elements (main content only)
| # | Label | Type | Href | Enabled |
|---|-------|------|------|---------|
| 1 | Overview | button[type=button] | — | yes |
| 2 | Agents | button[type=button] | — | yes |
| 3 | Fuel | button[type=button] | — | yes |
| 4 | Alerts (0) | button[type=button] | — | yes |

## Click sequence
### Click 1: "Overview"
- Pathname before: /monitor
- New console: clean
- Network failures: none
- Visible change: Already active tab; content shows CPU & RAM usage, disk usage, system stats (CPU cores, RAM, uptime, processes all showing "—"), fuel 0/1, alerts 0
- Pathname after: /monitor
- Reverted: n/a

### Click 2: "Agents"
- Pathname before: /monitor
- New console: clean
- Network failures: none
- Visible change: Tab switches to Agents; content shows "No agents available yet. This page shows live system usage and the runtime state of every governed agent."
- Pathname after: /monitor
- Reverted: n/a

### Click 3: "Fuel"
- Pathname before: /monitor
- New console: clean
- Network failures: none
- Visible change: Tab switches to Fuel; content shows "FUEL CONSUMPTION OVER TIME" (LIVE) and "FUEL BUDGET USAGE PER AGENT"
- Pathname after: /monitor
- Reverted: n/a

### Click 4: "Alerts (0)"
- Pathname before: /monitor
- New console: clean
- Network failures: none
- Visible change: Tab switches to Alerts; content shows "GOVERNANCE ALERTS — 0 active / 0 total — No alerts — system nominal"
- Pathname after: /monitor
- Reverted: n/a

### Skipped (destructive)
none

### Total interactive elements found: 4
### Elements clicked: 4 (of 4)

## Accessibility
- Images without alt: 0
- Inputs without label: 0
- Buttons without accessible name: 0

## Findings

### monitor-01
- SEVERITY: P2
- DIMENSION: overflow
- VIEWPORT: all
- EVIDENCE: `section.holo-panel.holo-panel-mid.nexus-page-panel` overflows at all viewports: +367px at 1920x1080, +63px at 1280x800, +248px at 1024x768. Root cause: `.holo-panel__refraction` is a 2208px-wide absolutely-positioned decorative child (left: -315px). The panel has `overflow: hidden` but its parent `div.page-transition__layer` has `overflow: visible`, allowing the refraction to contribute to scrollWidth measurement.
- IMPACT: No user-visible scrollbar (parent clips), but DOM reports overflow which can confuse layout-dependent JS and automated testing.

### monitor-02
- SEVERITY: P2
- DIMENSION: overflow
- VIEWPORT: all
- EVIDENCE: `div.living-background` overflows at all viewports: +150px at 1920x1080, +99px at 1280x800, +79px at 1024x768. This is the animated background layer.
- IMPACT: Decorative element exceeds viewport; no visible scrollbar but contributes to DOM overflow measurements.

## Summary
- Gate detected: no
- Total interactive elements: 4
- Elements clicked: 4
- P0: 0
- P1: 0
- P2: 2
