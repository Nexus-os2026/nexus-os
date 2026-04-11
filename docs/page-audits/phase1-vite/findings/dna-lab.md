# Audit: Dna Lab
URL: http://localhost:1420/dna-lab
Audited at: 2026-04-09T21:55:00Z
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
- `%cDownload the React DevTools for a better development experience: https://reactjs.org/link/react-devtools font-weight:bold` (chunk-NUMECXU6.js?v=5144749d:21550:24)

### Debug
- `[vite] connecting...` (@vite/client:494:8)
- `[vite] connected.` (@vite/client:617:14)

## Overflow

### 1920x1080
- documentElement: scrollWidth=1888 clientWidth=1888 [OK]
- body: scrollWidth=1888 clientWidth=1888 [OK]
- main `main`: scrollWidth=1627 clientWidth=1627 [OK]
- other overflowing:
  - `section.holo-panel.holo-panel-mid.nexus-page-panel`: scrollWidth=1925 clientWidth=1577 [OVERFLOW +348px]
  - `div.living-background`: scrollWidth=2035 clientWidth=1888 [OVERFLOW +147px]

### 1280x800
- documentElement: scrollWidth=1248 clientWidth=1248 [OK]
- body: scrollWidth=1248 clientWidth=1248 [OK]
- main `main`: scrollWidth=987 clientWidth=987 [OK]
- other overflowing:
  - `section.holo-panel.holo-panel-mid.nexus-page-panel`: scrollWidth=1158 clientWidth=937 [OVERFLOW +221px]
  - `div.living-background`: scrollWidth=1344 clientWidth=1248 [OVERFLOW +96px]

### 1024x768
- documentElement: scrollWidth=992 clientWidth=992 [OK]
- body: scrollWidth=992 clientWidth=992 [OK]
- main `main`: scrollWidth=731 clientWidth=731 [OK]
- other overflowing:
  - `div.living-background`: scrollWidth=1068 clientWidth=992 [OVERFLOW +76px]
  - `section.holo-panel.holo-panel-mid.nexus-page-panel`: scrollWidth=711 clientWidth=681 [OVERFLOW +30px]

## Interactive elements (main content only)

Default view shows BREED tab. Elements vary per tab. Below is the BREED tab (default) plus header controls.

| # | Label | Type | Href | Enabled |
|---|-------|------|------|---------|
| 1 | Refresh | button | | yes |
| 2 | Start Jarvis | button | | yes |
| 3 | BREED | button (tab) | | yes |
| 4 | GENOME | button (tab) | | yes |
| 5 | EVOLVE | button (tab) | | yes |
| 6 | LINEAGE | button (tab) | | yes |
| 7 | EVOLUTION | button (tab) | | yes |
| 8 | GENESIS | button (tab) | | yes |
| 9 | GENOME TOOLS | button (tab) | | yes |
| 10 | Select agent... (Parent A) | select | | yes |
| 11 | (unnamed SVG icon) | button | | no |
| 12 | Select agent... (Parent B) | select | | yes |
| 13 | Open chat | button | | yes |
| 14 | Dismiss | button | | yes |

Additional elements appear on other tabs (not simultaneously visible):
- GENOME: 1 select
- EVOLVE: 1 button "Evolve One Generation"
- LINEAGE: 1 select + 1 button "Load Lineage"
- EVOLUTION: 2 selects, 3 inputs, 1 textarea, 6 buttons (12 total)
- GENESIS: 5 textareas, 3 inputs, 4 buttons + 1 "Delete" button + 1 "Store Pattern" button (14 total)
- GENOME TOOLS: 4 selects + 4 buttons (8 total)

## Click sequence

### Click 1: "Refresh"
- Pathname before: /dna-lab
- New console: clean
- Network failures: none
- Visible change: none observed
- Pathname after: /dna-lab
- Reverted: n/a

### Click 2: "Start Jarvis"
- Pathname before: /dna-lab
- New console: clean
- Network failures: none
- Visible change: none observed
- Pathname after: /dna-lab
- Reverted: n/a

### Click 3: "BREED" (tab)
- Pathname before: /dna-lab
- New console: clean
- Network failures: none
- Visible change: BREED tab gains `is-active` CSS class; content shows "Parent A / Parent B" breeding interface
- Pathname after: /dna-lab
- Reverted: n/a

### Click 4: "GENOME" (tab)
- Pathname before: /dna-lab
- New console: clean
- Network failures: none
- Visible change: GENOME tab gains `is-active`; content shows "Select an agent to view its full genome"
- Pathname after: /dna-lab
- Reverted: n/a

### Click 5: "EVOLVE" (tab)
- Pathname before: /dna-lab
- New console: clean
- Network failures: none
- Visible change: EVOLVE tab gains `is-active`; content shows "Evolution Playground" with "Evolve One Generation" button
- Pathname after: /dna-lab
- Reverted: n/a

### Click 6: "LINEAGE" (tab)
- Pathname before: /dna-lab
- New console: clean
- Network failures: none
- Visible change: LINEAGE tab gains `is-active`; content shows "Lineage Tree" with agent select and "Load Lineage" button
- Pathname after: /dna-lab
- Reverted: n/a

### Click 7: "EVOLUTION" (tab)
- Pathname before: /dna-lab
- New console: clean
- Network failures: none
- Visible change: EVOLUTION tab gains `is-active`; content shows "Error: desktop runtime unavailable", "Evolution Status", and agent controls
- Pathname after: /dna-lab
- Reverted: n/a

### Click 8: "GENESIS" (tab)
- Pathname before: /dna-lab
- New console: clean
- Network failures: none
- Visible change: GENESIS tab gains `is-active`; content shows "Error: desktop runtime unavailable", "Gap Analysis" form
- Pathname after: /dna-lab
- Reverted: n/a

### Click 9: "GENOME TOOLS" (tab)
- Pathname before: /dna-lab
- New console: clean
- Network failures: none
- Visible change: GENOME TOOLS tab gains `is-active`; content shows "Error: desktop runtime unavailable", View Genome/Lineage/Breed sections and "Generate All Genomes" button
- Pathname after: /dna-lab
- Reverted: n/a

### Click 10: "Evolve One Generation" (on EVOLVE tab)
- Pathname before: /dna-lab
- New console: clean
- Network failures: none
- Visible change: none observed (button enabled, clicked successfully, no feedback)
- Pathname after: /dna-lab
- Reverted: n/a

### Skipped (destructive)
- "Delete" (GENESIS tab) -- reason: destructive keyword "delete"

### Total interactive elements found: 14 (BREED tab default view) + ~35 across other tabs
### Elements clicked: 10 (capped at 10)

## Accessibility
- Images without alt: 0
- Inputs without label: 4 (selectors: `select[type="select-one"]` x4 on GENOME TOOLS tab -- all `<select>` elements lack `id`, `aria-label`, and associated `<label>`)
- Buttons without accessible name: 1 (selector: `button` -- disabled SVG icon button on BREED tab, no text/aria-label/title)
- Additional issues:
  - h1 "DNA Lab" is outside `<main>` element (h1 top=92px, main top=208px)
  - 4 buttons missing `type` attribute: "Refresh", "Start Jarvis", "Open chat", "Dismiss"
  - 7 tab buttons (`.dna-tab`) lack `role="tab"`, `aria-selected`, and `aria-controls`
  - Tab container `.dna-tabs` lacks `role="tablist"`
  - No `role="tabpanel"` on any content panels
  - Tab state communicated via CSS class `is-active` only -- not programmatically exposed

## Findings

### dna-lab-01
- SEVERITY: P1
- DIMENSION: overflow
- VIEWPORT: all
- EVIDENCE: `section.holo-panel.holo-panel-mid.nexus-page-panel` has `overflow:hidden` and clips content both horizontally (scrollWidth=1925 vs clientWidth=1577, +348px at 1920x1080) and vertically (scrollHeight=1536 vs height=638, +897px clipped). Content below the fold is silently hidden.
- IMPACT: Users cannot access content that extends beyond the holo-panel visible area; nearly 900px of vertical content is invisible with no scroll affordance.

### dna-lab-02
- SEVERITY: P2
- DIMENSION: overflow
- VIEWPORT: all
- EVIDENCE: `div.living-background` exceeds viewport at all breakpoints: 2035px vs 1888px at 1920x1080 (+147px), 1344px vs 1248px at 1280x800 (+96px), 1068px vs 992px at 1024x768 (+76px). Contained by document overflow but contributes to layout pressure.
- IMPACT: Background element exceeds intended bounds; may cause rendering artefacts or unintended scrollbars on some browsers/OS combos.

### dna-lab-03
- SEVERITY: P1
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: 7 tab buttons (`.dna-tab`) have no `role="tab"`, no `aria-selected`, no `aria-controls`. Container `.dna-tabs` has no `role="tablist"`. No content panels have `role="tabpanel"`. Active state communicated only via CSS class `is-active`.
- IMPACT: Screen readers cannot identify tabs, their selected state, or their associated panels. Tab interface is invisible to assistive technology.

### dna-lab-04
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: 4 `<select>` elements (visible on GENOME TOOLS tab) have no `id`, no `aria-label`, no `aria-labelledby`, and no associated `<label>`. They use only placeholder option text "Select agent...", "Parent A...", "Parent B..." as their only identification.
- IMPACT: Screen readers announce these as unlabeled dropdowns; users cannot determine their purpose without visual context.

### dna-lab-05
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: 1 button on BREED tab (disabled SVG icon button at coordinates ~1046,631, 56x22px) has no text content, no `aria-label`, and no `title`. Its innerHTML is an SVG element only.
- IMPACT: Button purpose is unknown to screen readers and keyboard-only users.

### dna-lab-06
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: `<h1>DNA Lab</h1>` is positioned at top=92px, outside the `<main>` element which starts at top=208px. The h1 is not contained within any landmark.
- IMPACT: Heading hierarchy is disconnected from the main content landmark; assistive tech navigation by landmarks may miss the page title.

### dna-lab-07
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: 4 buttons lack `type` attribute: "Refresh" (`submit`), "Start Jarvis" (`submit`), "Open chat" (`submit`), "Dismiss" (`submit`). Without explicit `type="button"`, these default to `type="submit"` which can trigger unintended form submissions.
- IMPACT: Buttons inside a `<form>` would submit unexpectedly; current behaviour may be benign if no enclosing form exists but violates best practices.

### dna-lab-08
- SEVERITY: P2
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: On EVOLUTION, GENESIS, and GENOME TOOLS tabs, the text "Error: desktop runtime unavailable" is displayed in a bare `<div>` with no class. This is expected in demo mode (no Tauri backend), but the error is rendered as plain text with no styling to distinguish it as an informational banner vs. an application error.
- IMPACT: Users in demo mode see raw error text that may be mistaken for a bug rather than an expected limitation. Should be styled as a notice/banner.

## Summary
- Gate detected: no
- Total interactive elements: 14 (default BREED view); ~49 across all 7 tabs
- Elements clicked: 10
- P0: 0
- P1: 2
- P2: 6
