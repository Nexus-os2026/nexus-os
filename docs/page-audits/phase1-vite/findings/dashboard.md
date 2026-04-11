# Audit: Dashboard
URL: http://localhost:1420/dashboard
Audited at: 2026-04-09T17:45:00Z
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
- main `main.nexus-shell-content`: scrollWidth=1627 clientWidth=1627 [OK]
- other overflowing:
  - `div.living-background`: scrollWidth=2039 clientWidth=1888 (position:fixed; overflow:hidden — clipped, not user-visible)
  - `span.nexus-sidebar-item-text` x3: scrollWidth=157 clientWidth=153 (4px text clipping in sidebar)
  - `section.holo-panel.holo-panel-mid`: scrollWidth=1716 clientWidth=1577 (overflow:hidden — clipped)

### 1280x800
- documentElement: scrollWidth=1888 clientWidth=1888 [OK] (simulated via max-width constraint)
- body: scrollWidth=1280 clientWidth=1280 [OK]
- main `main.nexus-shell-content`: scrollWidth=1019 clientWidth=1019 [OK]
- other overflowing:
  - `div.living-background`: scrollWidth=2039 clientWidth=1888 (clipped)
  - `span.nexus-sidebar-item-text` x3: scrollWidth=157 clientWidth=153 (4px clipping)
  - `section.holo-panel.holo-panel-mid`: scrollWidth=1054 clientWidth=969 (overflow:hidden — clipped)

### 1024x768
- documentElement: scrollWidth=1888 clientWidth=1888 [OK] (simulated via max-width constraint)
- body: scrollWidth=1024 clientWidth=1024 [OK]
- main `main.nexus-shell-content`: scrollWidth=763 clientWidth=763 [OK]
- other overflowing:
  - `div.living-background`: scrollWidth=2039 clientWidth=1888 (clipped)
  - `span.nexus-sidebar-item-text` x3: scrollWidth=157 clientWidth=153 (4px clipping)
  - `section.holo-panel.holo-panel-mid`: scrollWidth=776 clientWidth=713 (overflow:hidden — clipped)

## Interactive elements (main content only)
| # | Label | Type | Href | Enabled |
|---|-------|------|------|---------|
| 1 | Refresh | button[type="submit"] | — | true |
| 2 | Start Jarvis | button[type="submit"] | — | true |
| 3 | Refresh | button[type="button"] | — | true |

## Click sequence
### Click 1: "Refresh" (header bar)
- Pathname before: /dashboard
- New console: clean
- Network failures: none
- Visible change: none — button click produced no observable feedback
- Pathname after: /dashboard
- Reverted: n/a

### Click 2: "Start Jarvis"
- Pathname before: /dashboard
- New console: clean
- Network failures: none
- Visible change: none — button click produced no observable feedback
- Pathname after: /dashboard
- Reverted: n/a

### Click 3: "Refresh" (Runtime Overview section)
- Pathname before: /dashboard
- New console: clean
- Network failures: none
- Visible change: none — button click produced no observable feedback
- Pathname after: /dashboard
- Reverted: n/a

### Skipped (destructive)
none

### Total interactive elements found: 3
### Elements clicked: 3 (of 3)

## Accessibility
- Images without alt: 0
- Inputs without label: 0
- Buttons without accessible name: 0

## Findings

### dashboard-01
- SEVERITY: P2
- DIMENSION: overflow
- VIEWPORT: all
- EVIDENCE: `div.living-background` has scrollWidth=2039 exceeding clientWidth=1888 at every viewport. Element is `position:fixed; overflow:hidden` so content is clipped, not user-visible. The background generates ~151px of unnecessary off-screen content.
- IMPACT: No visible horizontal scrollbar, but wasteful rendering of off-screen content on every frame (animated background).

### dashboard-02
- SEVERITY: P2
- DIMENSION: overflow
- VIEWPORT: all
- EVIDENCE: `span.nexus-sidebar-item-text` (3 instances) have scrollWidth=157 vs clientWidth=153 — 4px of text clipping. Affected items not identified by name (sidebar excluded from main audit scope), but overflow is present.
- IMPACT: Minor text truncation on some sidebar nav labels; last character(s) may be clipped without visible ellipsis.

### dashboard-03
- SEVERITY: P2
- DIMENSION: interactive
- VIEWPORT: 1920
- EVIDENCE: All 3 buttons ("Refresh" x2, "Start Jarvis") produce zero feedback on click — no console output, no network request, no loading spinner, no toast, no visual state change. In demo mode, buttons are silently non-functional.
- IMPACT: User has no way to tell if a click registered. Expected: at minimum a toast/banner indicating the action requires the desktop runtime.

### dashboard-04
- SEVERITY: P2
- DIMENSION: copy
- VIEWPORT: all
- EVIDENCE: "Running Agents" card (`article[ref_229]`) displays only the count "0" with no subtitle detail text. The other three cards all have subtitle lines: "Available Agents" has "0 active · 0 dormant", "Fuel Used" has "0 remaining of 0", "CPU / RAM" has "0 GB / 0 GB". The Running Agents card is visually shorter and inconsistent.
- IMPACT: Visual inconsistency across the 4 summary cards; Running Agents card appears incomplete compared to its siblings.

### dashboard-05
- SEVERITY: P2
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: Header-bar "Refresh" button has `type="submit"` instead of `type="button"`. Same for "Start Jarvis" — both are `type="submit"`. These are standalone buttons not inside a `<form>`, but `type="submit"` is semantically incorrect and could cause unexpected behavior if a parent form is ever added.
- IMPACT: No current functional issue, but incorrect button type is a latent form-submission risk.

## Summary
- Gate detected: no
- Total interactive elements: 3
- Elements clicked: 3
- P0: 0
- P1: 0
- P2: 5
