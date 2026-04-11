# Audit: Publish
URL: http://localhost:1420/publish
Audited at: 2026-04-10T01:55:41Z
Gate detected: false
Gate type: none

## Console (captured at 1888x951, ALL messages)
### Errors
none

### Warnings
none

### Logs
none

### Info
- `%cDownload the React DevTools for a better development experience: https://reactjs.org/link/react-devtools font-weight:bold` — chunk-NUMECXU6.js?v=5144749d:21550:24 (x3 duplicate)

### Debug
- `[vite] connecting...` — @vite/client:494:8 (x3 duplicate)
- `[vite] connected.` — @vite/client:617:14 (x3 duplicate)

## Overflow

### 1920x1080
> NOTE: Browser viewport measured at 1888x951 due to OS chrome. MCP resize_window does not change CSS viewport (confirmed in prior audits). Measurements taken at native 1888x951.

- documentElement: scrollWidth=1888 clientWidth=1888 [OK]
- body: scrollWidth=1888 clientWidth=1888 [OK]
- main: scrollWidth=1627 clientWidth=1627 [OK]
- other overflowing:
  - `div.living-background`: scrollWidth=2039 clientWidth=1888 [OVERFLOW +151px]
  - `span.nexus-sidebar-item-text`: scrollWidth=157 clientWidth=153 [OVERFLOW +4px] (x3 instances)
  - `section.holo-panel.holo-panel-mid`: scrollWidth=1716 clientWidth=1577 [OVERFLOW +139px]

### 1280x800
> Not measured — MCP resize_window confirmed non-functional for CSS viewport in prior audits (obs 1319-1320). Deferred to Puppeteer screenshot pass.

### 1024x768
> Not measured — same limitation as above. Deferred to Puppeteer screenshot pass.

## Interactive elements (main content only)
| # | Label | Type | Href | Enabled |
|---|-------|------|------|---------|
| 1 | Search (placeholder: "Search by name, description, or capability...") | input[text] | n/a | yes |
| 2 | All | button[type=button] | n/a | yes |
| 3 | L1 | button[type=button] | n/a | yes |
| 4 | L2 | button[type=button] | n/a | yes |
| 5 | L3 | button[type=button] | n/a | yes |
| 6 | L4 | button[type=button] | n/a | yes |
| 7 | L5 | button[type=button] | n/a | yes |
| 8 | L6 | button[type=button] | n/a | yes |
| 9 | Pre-installed | button (no type) | n/a | yes |
| 10 | Community (GitLab) | button (no type) | n/a | yes |

**Outside main but not in sidebar:**
- "Refresh" button (type=button) — in header area
- "Start Jarvis" button (type=button) — in header area

## Click sequence
### Click 1: "Search input"
- Pathname before: /publish
- New console: clean
- Network failures: none
- Visible change: input focused
- Pathname after: /publish
- Reverted: n/a

### Click 2: "All"
- Pathname before: /publish
- New console: clean
- Network failures: none
- Visible change: "All" button gains `active` class (autonomy level filter)
- Pathname after: /publish
- Reverted: n/a

### Click 3: "L1"
- Pathname before: /publish
- New console: clean
- Network failures: none
- Visible change: "All" loses `active`, "L1" gains `active` class (filter toggles)
- Pathname after: /publish
- Reverted: n/a

### Click 4: "L2"
- Pathname before: /publish
- New console: clean
- Network failures: none
- Visible change: filter toggles to L2
- Pathname after: /publish
- Reverted: n/a

### Click 5: "L3"
- Pathname before: /publish
- New console: clean
- Network failures: none
- Visible change: filter toggles to L3
- Pathname after: /publish
- Reverted: n/a

### Click 6: "L4"
- Pathname before: /publish
- New console: clean
- Network failures: none
- Visible change: filter toggles to L4
- Pathname after: /publish
- Reverted: n/a

### Click 7: "L5"
- Pathname before: /publish
- New console: clean
- Network failures: none
- Visible change: filter toggles to L5
- Pathname after: /publish
- Reverted: n/a

### Click 8: "L6"
- Pathname before: /publish
- New console: clean
- Network failures: none
- Visible change: filter toggles to L6 (L6 gains `active` class)
- Pathname after: /publish
- Reverted: n/a

### Click 9: "Pre-installed"
- Pathname before: /publish
- New console: clean
- Network failures: none
- Visible change: section tab button clicked, class is `cursor-pointer` — no visible active state change observed
- Pathname after: /publish
- Reverted: n/a

### Click 10: "Community (GitLab)"
- Pathname before: /publish
- New console: clean
- Network failures: none
- Visible change: section tab button clicked, class is `cursor-pointer` — no visible active state change observed
- Pathname after: /publish
- Reverted: n/a

### Skipped (destructive)
none

### Total interactive elements found: 10 (in main) + 2 (outside main, outside sidebar)
### Elements clicked: 10

## Accessibility
- Images without alt: 0
- Inputs without label: 1 (selectors: `input[type=text].as-search` — search input has placeholder but no `<label>`, `aria-label`, or `aria-labelledby`)
- Buttons without accessible name: 0

### Additional a11y findings
- h1 "Publish Agent" is outside `<main>` landmark (in `DIV.flex` ancestor)
- 4 `<section>` elements inside main lack `aria-label` or `aria-labelledby`: `section.holo-panel.holo-panel-mid`, `section.as-page`, `section.as-section` (x2)
- Filter buttons (All, L1-L6) use CSS `active` class but have no `aria-pressed` attribute — screen readers cannot detect selection state
- "Pre-installed" and "Community (GitLab)" buttons missing `type` attribute (default to `type="submit"`)
- "Refresh" and "Start Jarvis" buttons are outside `<main>` landmark, unreachable in main-landmark navigation

## Findings

### publish-01
- SEVERITY: P1
- DIMENSION: copy
- VIEWPORT: all
- EVIDENCE: The `/publish` route renders the Agent Store component (identical to `/agent-store`). h1 reads "Publish Agent" but main content shows "Agent Store — Unified runtime + community marketplace" with Pre-installed Agents and Marketplace sections. The `/agent-store` route also exists as a separate tab. Page text includes "Agent Store requires the desktop runtime" and "No pre-installed agents match the current search or autonomy filter."
- IMPACT: /publish is a duplicate route rendering Agent Store content instead of publish-specific functionality; users navigating to /publish get Agent Store with a misleading "Publish Agent" heading

### publish-02
- SEVERITY: P2
- DIMENSION: overflow
- VIEWPORT: 1920
- EVIDENCE: `div.living-background` scrollWidth=2039 > clientWidth=1888 (+151px). This is a fixed-position decorative background element wider than the viewport.
- IMPACT: Creates invisible horizontal scrollbar potential; decorative element exceeds viewport bounds

### publish-03
- SEVERITY: P2
- DIMENSION: overflow
- VIEWPORT: 1920
- EVIDENCE: `section.holo-panel.holo-panel-mid` scrollWidth=1716 > clientWidth=1577 (+139px). Main content panel overflows its container.
- IMPACT: Content panel wider than its visible area; potential clipping of content at edges

### publish-04
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: `h1.text-2xl` containing "Publish Agent" is in `DIV.flex` outside `<main class="nexus-shell-content">`. Screen readers navigating by landmark will not find the page heading inside main.
- IMPACT: Landmark navigation broken — users relying on assistive technology cannot associate the heading with the main content region

### publish-05
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: Search input `input[type=text].as-search` has `placeholder="Search by name, description, or capability..."` but no `<label>`, `aria-label`, or `aria-labelledby` attribute.
- IMPACT: Screen readers announce the input without a programmatic label; users cannot identify the field purpose without visual context

### publish-06
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: 4 `<section>` elements lack `aria-label`/`aria-labelledby`: `section.holo-panel.holo-panel-mid`, `section.as-page`, `section.as-section` (x2). These create anonymous landmark regions.
- IMPACT: Screen readers announce "region" landmarks with no distinguishing label, making navigation confusing

### publish-07
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: Filter buttons (All, L1, L2, L3, L4, L5, L6) toggle an `active` CSS class on click but have no `aria-pressed` or `aria-selected` attribute. No `role="group"` on the container.
- IMPACT: Selection state is visual-only; screen readers cannot convey which filter is currently active

### publish-08
- SEVERITY: P2
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: "Pre-installed" (class `cursor-pointer`) and "Community (GitLab)" (class `cursor-pointer`) buttons have no `type` attribute, defaulting to `type="submit"`. They also lack visible active/selected state feedback on click.
- IMPACT: Buttons may trigger form submission if placed inside a form in future; no visual confirmation of which section tab is selected

### publish-09
- SEVERITY: P2
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: "Refresh" and "Start Jarvis" buttons are positioned outside `<main>` landmark (and outside sidebar). They sit in a header flex container between the sidebar and main content area.
- IMPACT: Users navigating by landmarks will not discover these action buttons; they are functionally orphaned from the main content region

## Summary
- Gate detected: no
- Total interactive elements: 12 (10 in main + 2 outside main/sidebar)
- Elements clicked: 10
- P0: 0
- P1: 1
- P2: 8
