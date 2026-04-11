# Audit: Policies
URL: http://localhost:1420/policies
Audited at: 2026-04-10T01:15:00Z
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
- `%cDownload the React DevTools for a better development experience: https://reactjs.org/link/react-devtools font-weight:bold` — chunk-NUMECXU6.js?v=5144749d:21550:24
### Debug
- `[vite] connecting...` — @vite/client:494:8
- `[vite] connected.` — @vite/client:617:14

## Overflow

Note: `resize_window` MCP tool reports success but viewport remains locked at 1888x951 on ultrawide display. All three requested viewport sizes measured at the same actual viewport.

### 1920x1080 (actual 1888x951)
- documentElement: scrollWidth=1888 clientWidth=1888 [OK]
- body: scrollWidth=1888 clientWidth=1888 [OK]
- main `main`: scrollWidth=1627 clientWidth=1627 [OK]
- other overflowing:
  - `div.living-background`: scrollWidth=2039 clientWidth=1888 [OVERFLOW +151px]
  - `section.holo-panel.holo-panel-mid.nexus-page-panel`: scrollWidth=1716 clientWidth=1577 [OVERFLOW +139px]
  - `span.nexus-sidebar-item-text` (x3): scrollWidth=157 clientWidth=153 [OVERFLOW +4px, sidebar]

### 1280x800 (actual 1888x951 — resize had no effect)
- documentElement: scrollWidth=1888 clientWidth=1888 [OK]
- body: scrollWidth=1888 clientWidth=1888 [OK]
- main `main`: scrollWidth=1627 clientWidth=1627 [OK]
- other overflowing: same as 1920x1080

### 1024x768 (actual 1888x951 — resize had no effect)
- Not measurable at target viewport; data identical to 1920x1080.

## Interactive elements (main content only)

Page uses a 4-tab interface (list, editor, test, conflicts). Elements differ by active tab.

| # | Label | Type | Href | Enabled | Tab |
|---|-------|------|------|---------|-----|
| 1 | list | button | — | yes | all |
| 2 | editor | button | — | yes | all |
| 3 | test | button | — | yes | all |
| 4 | Conflicts (0) | button | — | yes | all |
| 5 | Reload | button | — | yes | list |
| 6 | (textarea, no label) | textarea | — | yes | editor |
| 7 | Validate | button | — | yes | editor |
| 8 | Principal | input[text] | — | yes | test |
| 9 | Action | input[text] | — | yes | test |
| 10 | Resource | input[text] | — | yes | test |
| 11 | Evaluate | button | — | yes | test |

Shell header buttons (outside main, not audited in click sequence):
- "Refresh" button (no type attribute)
- "Start Jarvis" button (no type attribute)

## Click sequence

### Click 1: "list"
- Pathname before: /policies
- New console: clean
- Network failures: none
- Visible change: Already on list tab; no change (shows "Loaded Policies (0)" + empty state)
- Pathname after: /policies
- Reverted: n/a

### Click 2: "editor"
- Pathname before: /policies
- New console: clean
- Network failures: none
- Visible change: Switched to editor tab; shows "Policy Editor" heading, textarea with TOML template, and "Validate" button
- Pathname after: /policies
- Reverted: n/a

### Click 3: "test"
- Pathname before: /policies
- New console: clean
- Network failures: none
- Visible change: Switched to test tab; shows "Dry-Run Policy Test" heading, 3 labeled inputs (Principal, Action, Resource), and "Evaluate" button
- Pathname after: /policies
- Reverted: n/a

### Click 4: "Conflicts (0)"
- Pathname before: /policies
- New console: clean
- Network failures: none
- Visible change: Switched to conflicts tab; shows "Policy Conflicts" heading and "No conflicts detected." text
- Pathname after: /policies
- Reverted: n/a

### Click 5: "Evaluate"
- Pathname before: /policies
- New console: clean
- Network failures: none
- Visible change: Error text "Error: desktop runtime unavailable" appeared below Evaluate button
- Pathname after: /policies
- Reverted: n/a

### Click 6: "Validate"
- Pathname before: /policies
- New console: clean
- Network failures: none
- Visible change: Error text "Error: desktop runtime unavailable" appeared below Validate button
- Pathname after: /policies
- Reverted: n/a

### Click 7: "Reload"
- Pathname before: /policies
- New console: clean
- Network failures: none
- Visible change: No visible change; still shows "No policies loaded" empty state
- Pathname after: /policies
- Reverted: n/a

### Skipped (destructive)
none

### Total interactive elements found: 11
### Elements clicked: 7 (all buttons; 3 text inputs + 1 textarea are form fields — clicking only focuses)

## Accessibility
- Images without alt: 0
- Inputs without label: 1 (selectors: `main textarea` — no id, name, aria-label, or aria-labelledby)
- Buttons without accessible name: 0

## Findings

### policies-01
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: Two `<h1>` elements on the page: one in `header` (outside `main`) with text "Policy Management", and one inside `main` with identical text "Policy Management". `document.querySelectorAll('h1').length === 2`.
- IMPACT: Duplicate H1 violates heading hierarchy best practices; screen readers announce two document titles.

### policies-02
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: The `<header>` element's parent is a `<div>`, not `<body>`. `header.parentElement.tagName === 'DIV'`. No element has `role="banner"`. `document.querySelectorAll('[role="banner"]').length === 0`.
- IMPACT: Header lacks implicit banner landmark role; screen readers cannot identify the page banner region.

### policies-03
- SEVERITY: P2
- DIMENSION: overflow
- VIEWPORT: 1888 (locked)
- EVIDENCE: `div.living-background` has scrollWidth=2039 vs clientWidth=1888, overflowing by 151px.
- IMPACT: Background decorative element exceeds viewport width; may cause horizontal scroll if overflow is not hidden by parent.

### policies-04
- SEVERITY: P2
- DIMENSION: overflow
- VIEWPORT: 1888 (locked)
- EVIDENCE: `section.holo-panel.holo-panel-mid.nexus-page-panel` has scrollWidth=1716 vs clientWidth=1577, overflowing by 139px.
- IMPACT: Main content panel overflows its container; content may be clipped or cause layout issues at smaller viewports.

### policies-05
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: All 7 buttons in `main` (`list`, `editor`, `test`, `Conflicts (0)`, `Reload`, `Validate`, `Evaluate`) have `type` attribute = null. `button.getAttribute('type') === null` for all. Shell header buttons (`Refresh`, `Start Jarvis`) also missing type.
- IMPACT: Buttons default to `type="submit"`, which can cause unintended form submission if placed inside a `<form>` element.

### policies-06
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: Tab buttons (`list`, `editor`, `test`, `Conflicts (0)`) have no `role="tab"`, no `aria-selected`, no `aria-controls`. Tab active state is CSS-only. `tabBtn.getAttribute('role') === null` for all four.
- IMPACT: Screen readers cannot identify the tab interface pattern; active tab state is not programmatically exposed.

### policies-07
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: `<textarea>` in editor tab has no `id`, no `name`, no `aria-label`, no `aria-labelledby`, no associated `<label>`. Heading "Policy Editor" is the only contextual label but not programmatically linked.
- IMPACT: Screen readers announce the textarea without a label; users cannot identify its purpose.

### policies-08
- SEVERITY: P2
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: "Reload" button click on list tab produces no console output, no network request, and no visible change. Page still shows "No policies loaded" — silent no-op in demo mode.
- IMPACT: Button appears functional but does nothing; no user feedback that backend is unavailable.

## Summary
- Gate detected: no
- Total interactive elements: 11
- Elements clicked: 7
- P0: 0
- P1: 0
- P2: 8
