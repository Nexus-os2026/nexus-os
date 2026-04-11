# Audit: Files
URL: http://localhost:1420/files
Audited at: 2026-04-09T19:13:00+01:00
Gate detected: false
Gate type: none

## Console (captured at 1920x1080, ALL messages)

### Errors
1. `[FileManager] failed to resolve initial directory Error: desktop runtime unavailable` — FileManager.tsx:313:41 (via backend.ts:17 invokeDesktop → backend.ts:2091 fileManagerHome). Fired twice (React StrictMode double-invoke).

### Warnings
none

### Logs
none

### Info
1. `%cDownload the React DevTools for a better development experience: https://reactjs.org/link/react-devtools font-weight:bold` — chunk-NUMECXU6.js:21550:24

### Debug
1. `[vite] connecting...` — @vite/client:494:8
2. `[vite] connected.` — @vite/client:617:14

## Overflow

### 1920x1080 (actual viewport 1978x951)
- documentElement: scrollWidth=1978 clientWidth=1978 [OK]
- body: scrollWidth=1978 clientWidth=1978 [OK]
- main `main.nexus-shell-content`: scrollWidth=1717 clientWidth=1717 [OK]
- other overflowing:
  - `div.living-background`: scrollWidth=2138 clientWidth=1978 [OVERFLOW +160px]
  - `section.holo-panel`: scrollWidth=1788 clientWidth=1667 [OVERFLOW +121px]
  - `span.nexus-sidebar-item-text` (×3): scrollWidth=157 clientWidth=153 [OVERFLOW +4px each]

### 1280x800 (simulated via CSS width constraint)
- documentElement: scrollWidth=1978 clientWidth=1978 [OK — outer viewport unchanged]
- body: scrollWidth=1280 clientWidth=1280 [OK]
- main `main.nexus-shell-content`: scrollWidth=1019 clientWidth=1019 [OK]
- other overflowing:
  - `div.living-background`: scrollWidth=2138 clientWidth=1978 [OVERFLOW +160px]
  - `section.holo-panel`: scrollWidth=1302 clientWidth=969 [OVERFLOW +333px]
  - `span.nexus-sidebar-item-text` (×3): scrollWidth=157 clientWidth=153 [OVERFLOW +4px each]

### 1024x768 (simulated via CSS width constraint)
- documentElement: scrollWidth=1978 clientWidth=1978 [OK — outer viewport unchanged]
- body: scrollWidth=1024 clientWidth=1024 [OK]
- main `main.nexus-shell-content`: scrollWidth=763 clientWidth=763 [OK]
- other overflowing:
  - `div.living-background`: scrollWidth=2138 clientWidth=1978 [OVERFLOW +160px]
  - `section.holo-panel`: scrollWidth=1093 clientWidth=713 [OVERFLOW +380px]
  - `span.nexus-sidebar-item-text` (×3): scrollWidth=157 clientWidth=153 [OVERFLOW +4px each]

## Interactive elements (main content only)

Initial state (list view, before search toggle):

| # | Label | Type | Href | Enabled |
|---|-------|------|------|---------|
| 1 | Grid view | button | — | yes |
| 2 | List view | button | — | yes |
| 3 | Search (Ctrl+F) | button | — | yes |
| 4 | Refresh (F5) | button | — | yes |
| 5 | Sidebar (Ctrl+B) | button | — | yes |
| 6 | × (error bar dismiss) | button | — | yes |
| 7 | Go up | button | — | **no** (disabled) |
| 8 | New file | button | — | yes |
| 9 | New folder | button | — | yes |
| 10 | Preview | button | — | yes |
| 11 | Details | button | — | yes |

After toggling search and list view, additional elements appeared:

| # | Label | Type | Href | Enabled |
|---|-------|------|------|---------|
| 12 | (search input) | input[text] | — | yes |
| 13 | × (search bar close) | button | — | yes |
| 14 | Name (column sort) | button | — | yes |
| 15 | Size (column sort) | button | — | yes |
| 16 | Modified (column sort) | button | — | yes |

## Click sequence

### Click 1: "Grid view"
- Pathname before: /files
- New console: clean
- Network failures: none
- Visible change: Grid view button gains `fm-tool-active` class (was already active — no-op)
- Pathname after: /files
- Reverted: n/a

### Click 2: "List view"
- Pathname before: /files
- New console: clean
- Network failures: none
- Visible change: List view button gains `fm-tool-active`, Grid view loses it. Content area switches to list layout. Column headers (Name, Size, Modified) appear.
- Pathname after: /files
- Reverted: n/a

### Click 3: "Search (Ctrl+F)"
- Pathname before: /files
- New console: clean
- Network failures: none
- Visible change: Search bar appears with `input.fm-search-input` (placeholder "Filter files by name...") and a × close button.
- Pathname after: /files
- Reverted: n/a

### Click 4: "Refresh (F5)"
- Pathname before: /files
- New console: clean (no new console error on refresh — original error was page-load only)
- Network failures: none
- Visible change: No visible change. Error bar still present showing "Unable to load files. Check permissions."
- Pathname after: /files
- Reverted: n/a

### Click 5: "Sidebar (Ctrl+B)"
- Pathname before: /files
- New console: clean
- Network failures: none
- Visible change: `div.fm-sidebar` element appears in DOM (sidebar panel toggled on). Preview and Details buttons disappear from the toolbar.
- Pathname after: /files
- Reverted: n/a

### Click 6: "× (error bar dismiss)"
- Pathname before: /files
- New console: clean
- Network failures: none
- Visible change: Error bar (`div.fm-error-bar`) removed from DOM. Empty state text changes to "Loading your files..." (perpetual loading spinner).
- Pathname after: /files
- Reverted: n/a

### Click 7: "New file"
- Pathname before: /files
- New console: clean
- Network failures: none
- Visible change: Inline input appears (`input.fm-new-item-input`, placeholder="filename.ext"). No modal/dialog.
- Pathname after: /files
- Reverted: n/a (dismissed via Escape)

### Click 8: "New folder"
- Pathname before: /files
- New console: clean
- Network failures: none
- Visible change: Inline input appears (`input.fm-new-item-input`, placeholder="folder-name"). Replaces any prior new-item input.
- Pathname after: /files
- Reverted: n/a (dismissed via Escape)

### Click 9: "Name (column sort)"
- Pathname before: /files
- New console: clean
- Network failures: none
- Visible change: No visible change (no files to sort). No `aria-sort` attribute set on button.
- Pathname after: /files
- Reverted: n/a

### Click 10: "Size (column sort)"
- Pathname before: /files
- New console: clean
- Network failures: none
- Visible change: No visible change (no files to sort). No `aria-sort` attribute set on button.
- Pathname after: /files
- Reverted: n/a

### Skipped (destructive)
none

### Total interactive elements found: 16 (11 initial + 5 conditional: search input, search ×, Name/Size/Modified column headers)
### Elements clicked: 10 (capped at 10)

## Accessibility
- Images without alt: 0
- Inputs without label: 2 (`input.fm-search-input[type=text]`, `input.fm-new-item-input[type=text]`)
- Buttons without accessible name: 0

Note: The × buttons have text content "×" which technically provides an accessible name, but it is non-descriptive. Neither has `aria-label` to clarify purpose (e.g., "Close search" vs "Dismiss error").

## Findings

### files-01
- SEVERITY: P1
- DIMENSION: console
- VIEWPORT: all
- EVIDENCE: `[FileManager] failed to resolve initial directory Error: desktop runtime unavailable` at FileManager.tsx:313:41. Error fires twice on page load (React StrictMode double-invoke). The call chain is `fileManagerHome()` → `invokeDesktop()` which throws because Tauri runtime is absent.
- IMPACT: File manager cannot load any directory in demo mode; user sees "Unable to load files. Check permissions." error bar followed by permanent "Loading your files..." spinner after dismissing the error.

### files-02
- SEVERITY: P2
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: After dismissing the error bar via ×, the empty state text changes from "Unable to load files. Check permissions." to "Loading your files..." with no way to recover the error message or re-trigger the load. The Refresh (F5) button produces no console output and no visible change — it silently fails in demo mode.
- IMPACT: User is left with a perpetual "Loading" state after dismissing the initial error, with no actionable feedback.

### files-03
- SEVERITY: P2
- DIMENSION: overflow
- VIEWPORT: all
- EVIDENCE: `section.holo-panel` (overflowX: hidden) has scrollWidth exceeding clientWidth at all viewports: 1920→+121px, 1280→+333px, 1024→+380px. The overflow grows worse at smaller viewports. Parent is `div.page-transition__layer`.
- IMPACT: Content inside holo-panel is clipped. Any content near the right edge may be invisible to users at narrower viewports.

### files-04
- SEVERITY: P2
- DIMENSION: overflow
- VIEWPORT: all
- EVIDENCE: `div.living-background` (position: fixed, overflow: hidden) has scrollWidth=2138 vs clientWidth=1978, a +160px overflow at all viewport sizes.
- IMPACT: Fixed-position background element exceeds viewport bounds. Overflow is hidden so no horizontal scrollbar, but it is a layout anomaly.

### files-05
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: `input.fm-search-input[type=text]` has no `aria-label`, `aria-labelledby`, or associated `<label>` element. It only has `placeholder="Filter files by name..."` which is not a reliable accessible name.
- IMPACT: Screen readers cannot identify the purpose of the search input.

### files-06
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: `input.fm-new-item-input[type=text]` (created by New file / New folder buttons) has no `aria-label`, `aria-labelledby`, or associated `<label>`. It only has placeholder text.
- IMPACT: Screen readers cannot identify the purpose of the new-item inline input.

### files-07
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: Two × (close) buttons lack `aria-label`: one in `.fm-error-bar` (dismiss error) and one in `.fm-search-bar` (close search). Both use bare "×" text as their only accessible name.
- IMPACT: Screen reader users hear "times button" with no context about what the button closes or dismisses.

### files-08
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: Column sort buttons (Name, Size, Modified) in list view do not set `aria-sort` attribute when clicked. After clicking Name and Size, `aria-sort` remained `null` on both.
- IMPACT: Screen readers cannot communicate the current sort column or direction to users.

### files-09
- SEVERITY: P2
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: Breadcrumb bar (`div.fm-breadcrumbs`) is empty — no path segments are rendered. The "Go up" button is permanently disabled. The initial directory resolution failed (files-01), so there is no current path context.
- IMPACT: User has no indication of what directory they are in and cannot navigate the file tree.

## Summary
- Gate detected: no
- Total interactive elements: 16
- Elements clicked: 10
- P0: 0
- P1: 1
- P2: 8
