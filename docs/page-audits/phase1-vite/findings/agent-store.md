# Audit: Agent Store
URL: http://localhost:1420/agent-store
Audited at: 2026-04-10T01:25:00Z
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
(resize_window requested 1920x1080; actual viewport remained 1888x951 — known MCP resize limitation)
- documentElement: scrollWidth=1888 clientWidth=1888 [OK]
- body: scrollWidth=1888 clientWidth=1888 [OK]
- main `div.nexus-main-column`: scrollWidth=1630 clientWidth=1630 [OK]
- other overflowing:
  - `div.living-background`: scrollWidth=2039 clientWidth=1888 [OVERFLOW +151px]
  - `section.holo-panel.holo-panel-mid`: scrollWidth=1716 clientWidth=1577 [OVERFLOW +139px]

### 1280x800
(resize_window tool did not change actual viewport — remained at 1888x951; measurement identical to above)
- documentElement: scrollWidth=1888 clientWidth=1888 [OK]
- body: scrollWidth=1888 clientWidth=1888 [OK]
- main `div.nexus-main-column`: scrollWidth=1630 clientWidth=1630 [OK]
- other overflowing:
  - `div.living-background`: scrollWidth=2039 clientWidth=1888 [OVERFLOW +151px]
  - `section.holo-panel.holo-panel-mid`: scrollWidth=1716 clientWidth=1577 [OVERFLOW +139px]

### 1024x768
(resize_window tool did not change actual viewport — remained at 1888x951; measurement identical to above)
- documentElement: scrollWidth=1888 clientWidth=1888 [OK]
- body: scrollWidth=1888 clientWidth=1888 [OK]
- main `div.nexus-main-column`: scrollWidth=1630 clientWidth=1630 [OK]
- other overflowing:
  - `div.living-background`: scrollWidth=2039 clientWidth=1888 [OVERFLOW +151px]
  - `section.holo-panel.holo-panel-mid`: scrollWidth=1716 clientWidth=1577 [OVERFLOW +139px]

## Interactive elements (main content only)
| # | Label | Type | Href | Enabled |
|---|-------|------|------|---------|
| 1 | Refresh | button[type=submit] | — | yes |
| 2 | Start Jarvis | button[type=submit] | — | yes |
| 3 | (placeholder: "Search by name, description, or capability...") | input[type=text] | — | yes |
| 4 | All | button[type=button] | — | yes |
| 5 | L1 | button[type=button] | — | yes |
| 6 | L2 | button[type=button] | — | yes |
| 7 | L3 | button[type=button] | — | yes |
| 8 | L4 | button[type=button] | — | yes |
| 9 | L5 | button[type=button] | — | yes |
| 10 | L6 | button[type=button] | — | yes |
| 11 | Pre-installed | button (no type) | — | yes |
| 12 | Community (GitLab) | button (no type) | — | yes |

Note: Elements 1-2 (Refresh, Start Jarvis) are in `header.nexus-shell-header` within `div.nexus-main-column`. Elements 11-12 are section tab-style buttons at the bottom of the page.

## Click sequence
### Click 1: "Refresh"
- Pathname before: /agent-store
- New console: clean
- Network failures: none
- Visible change: No visible change (silent in demo mode)
- Pathname after: /agent-store
- Reverted: n/a

### Click 2: "Start Jarvis"
- Pathname before: /agent-store
- New console: clean
- Network failures: none
- Visible change: No visible change (silent in demo mode)
- Pathname after: /agent-store
- Reverted: n/a

### Click 3: Search input (focus + typed "test")
- Pathname before: /agent-store
- New console: clean
- Network failures: none
- Visible change: Input received text "test"; agent lists remain showing "0 matching agents"
- Pathname after: /agent-store
- Reverted: n/a

### Click 4: "All"
- Pathname before: /agent-store
- New console: clean
- Network failures: none
- Visible change: Filter button "All" appears to accept click (autonomy level filter)
- Pathname after: /agent-store
- Reverted: n/a

### Click 5: "L1"
- Pathname before: /agent-store
- New console: clean
- Network failures: none
- Visible change: Filter toggles to L1 autonomy level
- Pathname after: /agent-store
- Reverted: n/a

### Click 6: "L2"
- Pathname before: /agent-store
- New console: clean
- Network failures: none
- Visible change: Filter toggles to L2 autonomy level
- Pathname after: /agent-store
- Reverted: n/a

### Click 7: "L3"
- Pathname before: /agent-store
- New console: clean
- Network failures: none
- Visible change: Filter toggles to L3 autonomy level
- Pathname after: /agent-store
- Reverted: n/a

### Click 8: "L4"
- Pathname before: /agent-store
- New console: clean
- Network failures: none
- Visible change: Filter toggles to L4 autonomy level
- Pathname after: /agent-store
- Reverted: n/a

### Click 9: "L5"
- Pathname before: /agent-store
- New console: clean
- Network failures: none
- Visible change: Filter toggles to L5 autonomy level
- Pathname after: /agent-store
- Reverted: n/a

### Click 10: "L6"
- Pathname before: /agent-store
- New console: clean
- Network failures: none
- Visible change: Filter toggles to L6 autonomy level; "L6" button gains class `active`
- Pathname after: /agent-store
- Reverted: n/a

### Skipped (destructive)
none

### Total interactive elements found: 12
### Elements clicked: 10 (capped at 10)

## Accessibility
- Images without alt: 0
- Inputs without label: 1 (selector: `input[type="text"]` in search bar — has no id, name, aria-label, or aria-labelledby; only placeholder text)
- Buttons without accessible name: 0

## Findings

### agent-store-01
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: Two `<h1>` elements on the page — `h1` "App Store" inside `header.nexus-shell-header` and `h1.as-title` "Unified runtime + community marketplace" inside `header.as-hero`. Only one H1 should exist per page.
- IMPACT: Screen readers announce two document titles, confusing heading hierarchy navigation.

### agent-store-02
- SEVERITY: P2
- DIMENSION: copy
- VIEWPORT: all
- EVIDENCE: Shell header H1 reads "App Store" but the hero kicker `p.as-kicker` reads "Agent Store" and the URL is `/agent-store`. Three different names for the same page.
- IMPACT: Naming inconsistency confuses users about the page identity.

### agent-store-03
- SEVERITY: P2
- DIMENSION: overflow
- VIEWPORT: all (measured at 1888x951)
- EVIDENCE: `div.living-background` scrollWidth=2039 > clientWidth=1888 (overflow +151px).
- IMPACT: Background element exceeds viewport width; may cause horizontal scroll on narrower viewports.

### agent-store-04
- SEVERITY: P2
- DIMENSION: overflow
- VIEWPORT: all (measured at 1888x951)
- EVIDENCE: `section.holo-panel.holo-panel-mid` scrollWidth=1716 > clientWidth=1577 (overflow +139px).
- IMPACT: Content panel overflows its container; may clip or cause horizontal scroll.

### agent-store-05
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: Search `input[type="text"]` has no `id`, `name`, `aria-label`, or `aria-labelledby`. Only `placeholder="Search by name, description, or capability..."` is present. Placeholder is not a substitute for an accessible label.
- IMPACT: Screen readers cannot announce the input's purpose.

### agent-store-06
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: No `role="banner"` landmark on the page. Two `<header>` elements exist (`header.nexus-shell-header` and `header.as-hero`) but neither has `role="banner"`.
- IMPACT: Landmark navigation does not expose a banner region.

### agent-store-07
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: Four buttons missing `type` attribute: "Refresh" (type=submit), "Start Jarvis" (type=submit) in shell header use `type="submit"` without a wrapping `<form>`, and "Pre-installed" and "Community (GitLab)" section buttons have no `type` attribute at all.
- IMPACT: Buttons without `type="button"` default to `type="submit"`, which can trigger unintended form submission. The two shell header buttons explicitly use `type="submit"` without a form context.

### agent-store-08
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: Autonomy filter buttons (All, L1-L6) with class `as-filter-btn` have no `role`, `aria-selected`, or `aria-pressed` attributes. The active button only toggles a CSS class `active`.
- IMPACT: Screen readers cannot determine which filter is currently selected.

### agent-store-09
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: "Pre-installed" and "Community (GitLab)" section toggle buttons have no `role="tab"`, `aria-selected`, or `aria-controls` attributes. They use only `class="cursor-pointer"`.
- IMPACT: Screen readers cannot identify these as tab controls or determine which section is active.

## Summary
- Gate detected: no
- Total interactive elements: 12
- Elements clicked: 10
- P0: 0
- P1: 0
- P2: 9
