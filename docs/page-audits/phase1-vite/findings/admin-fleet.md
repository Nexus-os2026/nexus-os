# Audit: Admin Fleet
URL: http://localhost:1420/admin-fleet
Audited at: 2026-04-10T00:35:00Z
Gate detected: false
Gate type: none

## Console (captured at 992x639 actual viewport, ALL messages)
### Errors
none

### Warnings
none

### Logs
none

### Info
- `%cDownload the React DevTools for a better development experience: https://reactjs.org/link/react-devtools font-weight:bold` — chunk-NUMECXU6.js:21550 (x2, duplicated)

### Debug
- `[vite] connecting...` — @vite/client:494 (x2, duplicated)
- `[vite] connected.` — @vite/client:617 (x2, duplicated)

## Overflow

> **Note:** `window.resizeTo()` is non-functional in this Chrome/Tauri environment. All three requested viewports (1920x1080, 1280x800, 1024x768) were attempted but the window remained at 992x639. Measurements are recorded at the actual viewport.

### 992x639 (actual — requested 1920x1080, 1280x800, 1024x768)
- documentElement: scrollWidth=992 clientWidth=992 [OK]
- body: scrollWidth=992 clientWidth=992 [OK]
- main `main.nexus-shell-content`: scrollWidth=731 clientWidth=731 [OK]
- other overflowing:
  - `div.living-background`: scrollWidth=1071 clientWidth=992 [OVERFLOW +79px]
  - `section.holo-panel.holo-panel-mid.nexus-page-panel`: scrollWidth=741 clientWidth=681 [OVERFLOW +60px]
  - `span.nexus-sidebar-item-text` (x3): scrollWidth=157 clientWidth=153 [OVERFLOW +4px each]

## Interactive elements (main content only)
| # | Label | Type | Href | Enabled |
|---|-------|------|------|---------|
| 1 | All Workspaces | select | — | yes |
| 2 | All Statuses | select | — | yes |
| 3 | Stop All | button | — | yes |
| 4 | (unlabeled checkbox) | input[checkbox] | — | yes |

## Click sequence
### Click 1: "All Workspaces" (select)
- Pathname before: /admin-fleet
- New console: clean
- Network failures: none
- Visible change: none — only 1 option available ("All Workspaces"), no filtering possible
- Pathname after: /admin-fleet
- Reverted: n/a

### Click 2: "All Statuses" (select)
- Pathname before: /admin-fleet
- New console: clean
- Network failures: none
- Visible change: Changed to "Running" filter value; table still shows "No agents match filter"
- Pathname after: /admin-fleet
- Reverted: n/a

### Click 3: "Stop All" (button)
- Pathname before: /admin-fleet
- New console: clean
- Network failures: none
- Visible change: none — button click produced no visible response, no confirmation dialog, no console output
- Pathname after: /admin-fleet
- Reverted: n/a

### Click 4: "(unlabeled checkbox)" (input[checkbox])
- Pathname before: /admin-fleet
- New console: clean
- Network failures: none
- Visible change: none — checkbox.checked remained false after click (click did not toggle state)
- Pathname after: /admin-fleet
- Reverted: n/a

### Skipped (destructive)
none — "Stop All" does not match the destructive keyword list (stop != delete/remove/destroy/reset/wipe/clear all/logout/sign out/uninstall/drop)

### Total interactive elements found: 4
### Elements clicked: 4 (capped at 10)

## Accessibility
- Images without alt: 0
- Inputs without label: 3 (selectors: `select.admin-select` x2, `input[type=checkbox]` in `th`)
- Buttons without accessible name: 0
- Additional: "Refresh" and "Start Jarvis" buttons outside `<main>` both lack `type` attribute

## Findings

### admin-fleet-01
- SEVERITY: P1
- DIMENSION: overflow
- VIEWPORT: 992 (all requested viewports could not be tested)
- EVIDENCE: `div.living-background` has scrollWidth=1071 vs clientWidth=992 (overflow +79px). `section.holo-panel.holo-panel-mid.nexus-page-panel` has scrollWidth=741 vs clientWidth=681 (overflow +60px).
- IMPACT: Background and main content panel overflow the viewport, potentially causing horizontal scrollbar or clipped content.

### admin-fleet-02
- SEVERITY: P1
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: Page has two `<h1>` elements: "Admin Fleet" (outside `<main>`, in `div.flex.flex-wrap`) and "Agent Fleet" (inside `main`, in `div.admin-shell`). Only one `<h1>` should exist per page for correct document outline.
- IMPACT: Screen readers announce two top-level headings, confusing document structure.

### admin-fleet-03
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: Both `select.admin-select` elements lack `id`, `aria-label`, and associated `<label>` elements. The `input[type=checkbox]` in `<th>` also lacks `aria-label`, `id`, or associated label. 3 unlabeled form controls total.
- IMPACT: Screen readers cannot announce the purpose of these filter controls or the select-all checkbox.

### admin-fleet-04
- SEVERITY: P2
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: "Stop All" `<button>` in main content lacks explicit `type` attribute (defaults to `type="submit"`). "Refresh" and "Start Jarvis" buttons outside main also lack `type` attribute.
- IMPACT: Implicit `type="submit"` may trigger unintended form submission if buttons are ever wrapped in a `<form>`.

### admin-fleet-05
- SEVERITY: P2
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: "Stop All" button click produces no visible feedback, no console output, no network request, and no confirmation dialog. Silent no-op in demo mode.
- IMPACT: User receives no indication whether the action was attempted or why it had no effect.

### admin-fleet-06
- SEVERITY: P2
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: `input[type=checkbox]` in table header `<th>` did not toggle checked state when clicked (checked remained `false`). The checkbox appears to be a select-all toggle but is non-functional.
- IMPACT: Select-all checkbox in fleet table header does not work.

### admin-fleet-07
- SEVERITY: P2
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: "All Workspaces" `<select>` contains only 1 option (`value=""`, text "All Workspaces"). No workspace options are populated in demo mode, making the filter permanently locked.
- IMPACT: Workspace filter dropdown is non-functional — user cannot filter agents by workspace.

### admin-fleet-08
- SEVERITY: P2
- DIMENSION: console
- VIEWPORT: all
- EVIDENCE: Vite HMR messages are duplicated: `[vite] connecting...` (x2 at @vite/client:494), `[vite] connected.` (x2 at @vite/client:617), React DevTools info (x2 at chunk-NUMECXU6.js:21550).
- IMPACT: Console noise from duplicated HMR initialization suggests double-mounting or duplicate script loading.

## Summary
- Gate detected: no
- Total interactive elements: 4
- Elements clicked: 4
- P0: 0
- P1: 2
- P2: 6
