# Audit: Memory Dashboard
URL: http://localhost:1420/memory-dashboard
Audited at: 2026-04-09T21:50:00Z
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
  - `div.living-background`: scrollWidth=2039 clientWidth=1888 [OVERFLOW +151px]
  - `section.holo-panel.holo-panel-mid`: scrollWidth=1716 clientWidth=1577 [OVERFLOW +139px horizontal, scrollHeight=1039 clientHeight=693 OVERFLOW +346px vertical]

### 1280x800 (simulated)
- documentElement: scrollWidth=1888 clientWidth=1888 [OK]
- body: scrollWidth=1280 clientWidth=1280 [OK]
- main `main.nexus-shell-content`: scrollWidth=1019 clientWidth=1019 [OK]
- other overflowing:
  - `div.living-background`: scrollWidth=2039 clientWidth=1888 [OVERFLOW +151px]
  - `section.holo-panel.holo-panel-mid`: scrollWidth=1054 clientWidth=969 [OVERFLOW +85px]

### 1024x768 (simulated)
- documentElement: scrollWidth=1888 clientWidth=1888 [OK]
- body: scrollWidth=1024 clientWidth=1024 [OK]
- main `main.nexus-shell-content`: scrollWidth=763 clientWidth=763 [OK]
- other overflowing:
  - `div.living-background`: scrollWidth=2039 clientWidth=1888 [OVERFLOW +151px]
  - `section.holo-panel.holo-panel-mid`: scrollWidth=776 clientWidth=713 [OVERFLOW +63px]

## Interactive elements (main content only)
| # | Label | Type | Href | Enabled |
|---|-------|------|------|---------|
| 1 | Select Agent | select | — | yes |
| 2 | Refresh | button | — | yes |
| 3 | All | button | — | yes |
| 4 | Working | button | — | yes |
| 5 | Episodic | button | — | yes |
| 6 | Semantic | button | — | yes |
| 7 | Procedural | button | — | yes |
| 8 | Search memories... | input[text] | — | yes |
| 9 | Planning | select | — | yes |
| 10 | Search | button | — | yes |
| 11 | Checkpoint label... | input[text] | — | yes |
| 12 | Create | button | — | yes |
| 13 | Run GC | button | — | yes |
| 14 | Clear Working Memory | button | — | yes |

## Click sequence
### Click 1: "Select Agent" (select)
- Pathname before: /memory-dashboard
- New console: clean
- Network failures: none
- Visible change: Focused select; only one placeholder option ("Select Agent") with empty value — no agents to select in demo mode
- Pathname after: /memory-dashboard
- Reverted: n/a

### Click 2: "Refresh"
- Pathname before: /memory-dashboard
- New console: clean
- Network failures: none
- Visible change: No visible change in demo mode
- Pathname after: /memory-dashboard
- Reverted: n/a

### Click 3: "All"
- Pathname before: /memory-dashboard
- New console: clean
- Network failures: none
- Visible change: Filter button activates (background changes to rgb(31, 41, 55))
- Pathname after: /memory-dashboard
- Reverted: n/a

### Click 4: "Working"
- Pathname before: /memory-dashboard
- New console: clean
- Network failures: none
- Visible change: Working button activates, All deactivates — filter state updates correctly via inline style
- Pathname after: /memory-dashboard
- Reverted: n/a

### Click 5: "Episodic"
- Pathname before: /memory-dashboard
- New console: clean
- Network failures: none
- Visible change: Episodic button activates, Working deactivates
- Pathname after: /memory-dashboard
- Reverted: n/a

### Click 6: "Semantic"
- Pathname before: /memory-dashboard
- New console: clean
- Network failures: none
- Visible change: Semantic button activates, Episodic deactivates
- Pathname after: /memory-dashboard
- Reverted: n/a

### Click 7: "Procedural"
- Pathname before: /memory-dashboard
- New console: clean
- Network failures: none
- Visible change: Procedural button activates (confirmed via inline style background: rgb(31, 41, 55))
- Pathname after: /memory-dashboard
- Reverted: n/a

### Click 8: "Search memories..." (input)
- Pathname before: /memory-dashboard
- New console: clean
- Network failures: none
- Visible change: Input focused; typed "test" — no filtering occurred (demo mode, "No memories found" remains)
- Pathname after: /memory-dashboard
- Reverted: n/a

### Click 9: "Planning" (select → "Execution")
- Pathname before: /memory-dashboard
- New console: clean
- Network failures: none
- Visible change: Select value changed to "execution"; no visible content change (demo mode)
- Pathname after: /memory-dashboard
- Reverted: n/a

### Click 10: "Search"
- Pathname before: /memory-dashboard
- New console: clean
- Network failures: none
- Visible change: No visible change in demo mode
- Pathname after: /memory-dashboard
- Reverted: n/a

### Skipped (destructive)
- "Clear Working Memory" — reason: destructive keyword "clear"

### Total interactive elements found: 14
### Elements clicked: 10 (capped at 10)

## Accessibility
- Images without alt: 0
- Inputs without label: 4 (selectors: `select` (Select Agent — no id, no aria-label, no wrapping label), `input[type="text"]` (placeholder="Search memories..." — no label/aria-label), `select` (Planning — no id, no aria-label, no wrapping label), `input[type="text"]` (placeholder="Checkpoint label..." — no label/aria-label))
- Buttons without accessible name: 0

## Findings

### memory-dashboard-01
- SEVERITY: P2
- DIMENSION: overflow
- VIEWPORT: all
- EVIDENCE: `div.living-background` scrollWidth=2039 exceeds clientWidth=1888 at 1920x1080 (+151px). Persists at 1280x800 and 1024x768.
- IMPACT: Decorative background element overflows viewport at all tested sizes; hidden by parent overflow but contributes to layout width calculation.

### memory-dashboard-02
- SEVERITY: P2
- DIMENSION: overflow
- VIEWPORT: all
- EVIDENCE: `section.holo-panel.holo-panel-mid` has overflow:hidden with scrollWidth=1716 vs clientWidth=1577 (+139px horizontal) and scrollHeight=1039 vs clientHeight=693 (+346px vertical) at 1920x1080. Content is silently clipped.
- IMPACT: 346px of vertical content clipped by overflow:hidden on the holo-panel — lower portions of the memory dashboard (checkpoint/GC controls) may be cut off without scroll affordance.

### memory-dashboard-03
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: Both `<select>` elements (Select Agent, Planning) have no `id`, no `aria-label`, no `aria-labelledby`, and are not wrapped in `<label>`. Screen readers cannot identify their purpose.
- IMPACT: Two select controls are completely unlabeled for assistive technology users.

### memory-dashboard-04
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: Both `<input type="text">` elements ("Search memories...", "Checkpoint label...") rely solely on `placeholder` for identification — no `<label>`, no `aria-label`, no `aria-labelledby`.
- IMPACT: Placeholder text disappears on input and is not exposed as an accessible name by all screen readers.

### memory-dashboard-05
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: `<h1>Agent Memory</h1>` is positioned at y=92px, outside `<main>` which starts at y=208px. The h1 is a sibling or ancestor of main, not contained within it.
- IMPACT: Landmark navigation skips the page heading; users navigating via main landmark miss the h1.

### memory-dashboard-06
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: All 10 buttons in main content have no `type` attribute. Default type is "submit" which may trigger unintended form submission if buttons are inside or near a `<form>`.
- IMPACT: Buttons without explicit `type="button"` may cause unexpected form submissions.

### memory-dashboard-07
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: Filter buttons (All, Working, Episodic, Semantic, Procedural) toggle active state via inline background color but have no `aria-pressed` attribute. Active state is visual-only.
- IMPACT: Screen reader users cannot determine which memory type filter is currently selected.

### memory-dashboard-08
- SEVERITY: P2
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: "Select Agent" dropdown contains only one option: the placeholder `"Select Agent"` with empty value. No demo agents are populated.
- IMPACT: In demo mode the select is non-functional — user cannot select any agent to view memories for. The page shows "No memories found" with no way to populate data.

## Summary
- Gate detected: no
- Total interactive elements: 14
- Elements clicked: 10
- P0: 0
- P1: 0
- P2: 8
