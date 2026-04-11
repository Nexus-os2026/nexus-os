# Audit: Workflows
URL: http://localhost:1420/workflows
Audited at: 2026-04-09T23:42:00Z
Gate detected: false
Gate type: none

## Console (captured at 1248x671, ALL messages)

Note: resize_window MCP tool failed to change viewport from 1248x671 to 1920x1080 (known limitation). All measurements taken at actual viewport 1248x671.

### Errors
none

### Warnings
none

### Logs
none

### Info
- `%cDownload the React DevTools for a better development experience: https://reactjs.org/link/react-devtools font-weight:bold` — chunk-NUMECXU6.js?v=5144749d:21550:24 (appears twice)

### Debug
- `[vite] connecting...` — @vite/client:494:8 (appears twice)
- `[vite] connected.` — @vite/client:617:14 (appears twice)

## Overflow

### 1248x671 (actual viewport — resize to 1920x1080 failed)
- documentElement: scrollWidth=1248 clientWidth=1248 [OK]
- body: scrollWidth=1248 clientWidth=1248 [OK]
- main `main.nexus-shell-content`: scrollWidth=987 clientWidth=987 [OK]
- other overflowing:
  - `div.living-background`: scrollWidth=1348 clientWidth=1248 [OVERFLOW +100px]
  - `section.holo-panel.holo-panel-mid`: scrollWidth=1019 clientWidth=937 [OVERFLOW +82px]
  - `span.nexus-sidebar-item-text` x3: scrollWidth=157 clientWidth=153 [OVERFLOW +4px, sidebar text truncation]

### 1280x800
Not measured — resize_window tool has no effect on actual viewport (known issue, see obs 946/954).

### 1024x768
Not measured — same reason as above.

## Interactive elements (main content only)

| # | Label | Type | Href | Enabled |
|---|-------|------|------|---------|
| 1 | Refresh | button[submit] | — | yes |
| 2 | Start Jarvis | button[submit] | — | yes |
| 3 | Builder | button | — | yes |
| 4 | Scheduled | button | — | yes |
| 5 | History | button | — | yes |
| 6 | Simple Chain (template) | button | — | yes |
| 7 | Research Pipeline (template) | button | — | yes |
| 8 | Code Review (template) | button | — | yes |
| 9 | Zoom In | button | — | yes |
| 10 | Zoom Out | button | — | yes |
| 11 | Fit View | button | — | yes |
| 12 | Toggle Interactivity | button | — | yes |
| 13 | "Untitled Workflow" | input[text] | — | yes |
| 14 | Save | button | — | yes |
| 15 | Execute | button | — | yes |
| 16 | React Flow attribution | a | https://reactflow.dev | yes |

## Click sequence

### Click 1: "Refresh"
- Pathname before: /workflows
- New console: clean
- Network failures: none
- Visible change: none — silent no-op in demo mode
- Pathname after: /workflows
- Reverted: n/a

### Click 2: "Start Jarvis"
- Pathname before: /workflows
- New console: clean
- Network failures: none
- Visible change: none — silent no-op in demo mode
- Pathname after: /workflows
- Reverted: n/a

### Click 3: "Builder"
- Pathname before: /workflows
- New console: clean
- Network failures: none
- Visible change: tab opacity toggles (Builder=1, others=0.5); no content change (Builder view was already showing)
- Pathname after: /workflows
- Reverted: n/a

### Click 4: "Scheduled"
- Pathname before: /workflows
- New console: clean
- Network failures: none
- Visible change: tab opacity toggles (Scheduled=1, others=0.5); content DOES NOT change — Builder canvas, sidebar, and templates remain visible. No scheduled-workflow list or empty state shown.
- Pathname after: /workflows
- Reverted: n/a

### Click 5: "History"
- Pathname before: /workflows
- New console: clean
- Network failures: none
- Visible change: tab opacity toggles (History=1, others=0.5); content DOES NOT change — same Builder view persists. No execution history list or empty state shown.
- Pathname after: /workflows
- Reverted: n/a

### Click 6: "Simple Chain" (template)
- Pathname before: /workflows
- New console: clean
- Network failures: none
- Visible change: none — 0 React Flow nodes created on canvas. Template click is a silent no-op.
- Pathname after: /workflows
- Reverted: n/a

### Click 7: "Research Pipeline" (template)
- Pathname before: /workflows
- New console: clean
- Network failures: none
- Visible change: none — 0 React Flow nodes created on canvas. Template click is a silent no-op.
- Pathname after: /workflows
- Reverted: n/a

### Click 8: "Code Review" (template)
- Pathname before: /workflows
- New console: clean
- Network failures: none
- Visible change: none — 0 React Flow nodes created on canvas. Template click is a silent no-op.
- Pathname after: /workflows
- Reverted: n/a

### Click 9: "Zoom In"
- Pathname before: /workflows
- New console: clean
- Network failures: none
- Visible change: React Flow canvas zoom level increased (functional)
- Pathname after: /workflows
- Reverted: n/a

### Click 10: "Zoom Out"
- Pathname before: /workflows
- New console: clean
- Network failures: none
- Visible change: React Flow canvas zoom level decreased (functional)
- Pathname after: /workflows
- Reverted: n/a

### Skipped (destructive)
none

### Total interactive elements found: 16
### Elements clicked: 10 (capped at 10)

## Accessibility
- Images without alt: 0
- Inputs without label: 1 (selectors: `input[type=text]` — the "Untitled Workflow" name field has no id, no label element, no aria-label, no aria-labelledby)
- Buttons without accessible name: 0

### Additional ARIA/structure issues
- H1 "Workflows" is outside `<main>` element — it lives in the banner/header `header.nexus-shell-header`, not inside `main.nexus-shell-content`
- Two `<header>` elements (`header.nexus-shell-header`, `header.wf-header`) have no `role="banner"` attribute and no `aria-label`
- `<nav>` in sidebar has no `aria-label`
- Tab buttons (Builder/Scheduled/History) have no `role="tab"`, `aria-selected`, or `tablist`/`tabpanel` ARIA pattern — they use plain `<button>` with opacity toggling

## Findings

### workflows-01
- SEVERITY: P1
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: Clicking "Scheduled" tab button toggles opacity to 1 (active state) but does not change the visible content. The Builder canvas, node sidebar, and template list remain visible. No scheduled-workflows list or empty state is rendered. Same for "History" tab.
- IMPACT: Users cannot access the Scheduled or History views — the tabs are cosmetic-only with no content switching logic.

### workflows-02
- SEVERITY: P1
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: Clicking any of the 3 template buttons ("Simple Chain", "Research Pipeline", "Code Review") produces no visible change. After click, `document.querySelectorAll('.react-flow__node').length === 0`. No nodes are added to the React Flow canvas. No console errors.
- IMPACT: Template one-click workflow creation is non-functional — users cannot load a pre-built workflow template onto the canvas.

### workflows-03
- SEVERITY: P2
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: "Refresh" button (`button[type=submit]`) in the banner header produces no visible change, no console output, and no network request on click in demo mode.
- IMPACT: Button appears interactive but is a silent no-op. Users get no feedback that the action was attempted or that it requires the desktop runtime.

### workflows-04
- SEVERITY: P2
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: "Start Jarvis" button (`button[type=submit]`) in the banner header produces no visible change, no console output, and no network request on click in demo mode.
- IMPACT: Button appears interactive but is a silent no-op. Users get no feedback.

### workflows-05
- SEVERITY: P2
- DIMENSION: overflow
- VIEWPORT: 1248
- EVIDENCE: `div.living-background` scrollWidth=1348, clientWidth=1248 — overflows by 100px horizontally. `section.holo-panel.holo-panel-mid` scrollWidth=1019, clientWidth=937 — overflows by 82px horizontally. (Note: viewport was 1248x671; resize to 1920/1280/1024 failed.)
- IMPACT: Background decoration and holo-panel extend beyond viewport boundary. May cause horizontal scrollbar on some configurations.

### workflows-06
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: The "Untitled Workflow" `input[type=text]` has no `id`, no `<label>` element, no `aria-label`, and no `aria-labelledby`. Screen readers cannot identify the purpose of this field.
- IMPACT: Workflow name input is inaccessible to screen reader users.

### workflows-07
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: The 3 tab buttons (Builder, Scheduled, History) use `button.wf-create-btn` with inline `opacity` toggling. No `role="tablist"` container, no `role="tab"` on buttons, no `aria-selected` attribute, no `role="tabpanel"` on content areas.
- IMPACT: Tab navigation pattern is not communicated to assistive technology. Screen reader users cannot determine which tab is active or that these controls form a tabbed interface.

### workflows-08
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: H1 "Workflows" is in `header.nexus-shell-header` which is outside `<main>`. The `<main>` element has no `aria-label`. Two `<header>` elements lack `role` and `aria-label`. The sidebar `<nav>` has no `aria-label`.
- IMPACT: ARIA landmark structure is incomplete — screen reader users cannot efficiently navigate page regions.

### workflows-09
- SEVERITY: P2
- DIMENSION: console
- VIEWPORT: all
- EVIDENCE: All 3 unique console messages (2 vite debug + 1 React DevTools info) appear twice each on page load, totaling 6 messages. Source: `@vite/client:494`, `@vite/client:617`, `chunk-NUMECXU6.js:21550`.
- IMPACT: Duplicate initialization suggests the app or HMR client is mounting twice in development mode.

## Summary
- Gate detected: no
- Total interactive elements: 16
- Elements clicked: 10
- P0: 0
- P1: 2
- P2: 7
