# Audit: Notes
URL: http://localhost:1420/notes
Audited at: 2026-04-09T23:39:00Z
Gate detected: false
Gate type: none

## Console (captured at 1248x671, ALL messages)
### Errors
- `Warning: Encountered two children with the same key, 'n-1775774339404'. Keys should be unique so that components maintain their identity across updates. Non-unique keys may cause children to be duplicated and/or omitted — the behavior is unsupported and could change in a future version.` — at NotesApp (src/pages/NotesApp.tsx:139:29) via chunk-NUMECXU6.js?v=5144749d:520:37 (triggered after clicking "New note" button)
### Warnings
- none
### Logs
- none
### Info
- `%cDownload the React DevTools for a better development experience: https://reactjs.org/link/react-devtools font-weight:bold` — chunk-NUMECXU6.js?v=5144749d:21550:24
### Debug
- `[vite] connecting...` — @vite/client:494:8
- `[vite] connected.` — @vite/client:617:14

## Overflow

### 1248x671 (actual browser viewport — resize_window MCP tool has no effect)
- documentElement: scrollWidth=1248 clientWidth=1248 [OK]
- body: scrollWidth=1248 clientWidth=1248 [OK]
- main `main.nexus-shell-content`: scrollWidth=987 clientWidth=987 [OK]
- other overflowing:
  - `div.living-background`: scrollWidth=1348 clientWidth=1248 [OVERFLOW +100px]
  - `section.holo-panel.holo-panel-mid`: scrollWidth=1019 clientWidth=937 [OVERFLOW +82px, masked by overflow:hidden]
  - `span.nexus-sidebar-item-text` (×3): scrollWidth=157 clientWidth=153 [OVERFLOW +4px]
  - `div.na-agent-entry`: scrollWidth=209 clientWidth=204 [OVERFLOW +5px]

### 1920x1080
- Not measured — resize_window MCP tool consistently fails to change viewport (known issue from prior audits)

### 1280x800
- Not measured — same reason

### 1024x768
- Not measured — same reason

## Interactive elements (main content only, initial load before clicks)
| # | Label | Type | Href | Enabled |
|---|-------|------|------|---------|
| 1 | New note (aria-label) | button | — | yes |
| 2 | Hide sidebar (aria-label) | button | — | yes |
| 3 | Search notes... (placeholder) | input | — | yes |
| 4 | All Notes | button | — | yes |
| 5 | Projects | button | — | yes |
| 6 | (empty — folder toggle) | button | — | yes |
| 7 | Research | button | — | yes |
| 8 | (empty — folder toggle) | button | — | yes |
| 9 | Meetings | button | — | yes |
| 10 | (empty — folder toggle) | button | — | yes |
| 11 | Agent Notes | button | — | yes |
| 12 | (empty — folder toggle) | button | — | yes |
| 13 | Templates | button | — | yes |
| 14 | (empty — folder toggle) | button | — | yes |
| 15 | Archive | button | — | yes |
| 16 | (empty — folder toggle) | button | — | yes |
| 17 | research (tag) | button | — | yes |
| 18 | project (tag) | button | — | yes |
| 19 | meeting (tag) | button | — | yes |
| 20 | architecture (tag) | button | — | yes |
| 21 | bug (tag) | button | — | yes |
| 22 | idea (tag) | button | — | yes |
| 23 | agent-generated (tag) | button | — | yes |
| 24 | (sort select) | select | — | yes |
| 25 | Create Note | button | — | yes |
| 26 | New Note | button | — | yes |

## Click sequence
### Click 1: "New note" (+ button, aria-label)
- Pathname before: /notes
- New console: none at time of click; React duplicate key error triggered after note creation
- Network failures: none
- Visible change: "Error: desktop runtime unavailable" text appeared at top of page; template dropdown opened with options: Meeting, Research, Project, Bug Report, Blank; 4 notes created and assigned to Projects folder; note editor panel appeared with textarea, title input, move-to select
- Pathname after: /notes
- Reverted: n/a

### Click 2: "Hide sidebar" (aria-label)
- Pathname before: /notes
- New console: clean
- Network failures: none
- Visible change: Notes sidebar (folders, tags, search) collapsed; element count dropped from 26 to 4; button label changed to "Show sidebar"
- Pathname after: /notes
- Reverted: yes (clicked "Show sidebar" to restore)

### Click 3: Search input (focus)
- Pathname before: /notes
- New console: clean
- Network failures: none
- Visible change: Input received focus; no other change
- Pathname after: /notes
- Reverted: n/a

### Click 4: "All Notes"
- Pathname before: /notes
- New console: clean
- Network failures: none
- Visible change: Category filter switched to All Notes (count shows 4)
- Pathname after: /notes
- Reverted: n/a

### Click 5: "Projects"
- Pathname before: /notes
- New console: clean
- Network failures: none
- Visible change: Category filter switched to Projects (count shows 4)
- Pathname after: /notes
- Reverted: n/a

### Click 6: folder toggle (Projects)
- Pathname before: /notes
- New console: clean
- Network failures: none
- Visible change: Projects folder expanded/collapsed (toggle chevron)
- Pathname after: /notes
- Reverted: n/a

### Click 7: "Research"
- Pathname before: /notes
- New console: clean
- Network failures: none
- Visible change: Category filter switched to Research (count shows 0)
- Pathname after: /notes
- Reverted: n/a

### Click 8: folder toggle (Research)
- Pathname before: /notes
- New console: clean
- Network failures: none
- Visible change: Research folder expanded/collapsed
- Pathname after: /notes
- Reverted: n/a

### Click 9: "Meetings"
- Pathname before: /notes
- New console: React duplicate key error (key `n-1775774339404` in NotesApp.tsx:139)
- Network failures: none
- Visible change: Category filter switched to Meetings (count 0); "No notes found" displayed with "Create Note" button
- Pathname after: /notes
- Reverted: n/a

### Click 10: folder toggle (Meetings)
- Pathname before: /notes
- New console: clean
- Network failures: none
- Visible change: Meetings folder expanded/collapsed
- Pathname after: /notes
- Reverted: n/a

### Skipped (destructive)
- none found

### Total interactive elements found: 26 (initial), 41 (after note creation added editor panel)
### Elements clicked: 10 (capped at 10)

## Accessibility
- Images without alt: 0
- Inputs without label: 4
  - `input.na-search-input` (id=na-search, no associated label element, no aria-label)
  - `select.na-sort-select` (no id, no aria-label)
  - `input.na-title-input` (no id, no aria-label)
  - `select.na-move-select` (no id, no aria-label)
  - `textarea.na-textarea` (no id, no aria-label; placeholder="Start writing... (Markdown supported)" is not a substitute)
- Buttons without accessible name: 6
  - `button.na-folder-toggle` for Projects (SVG chevron only, no aria-label)
  - `button.na-folder-toggle` for Research (SVG chevron only, no aria-label)
  - `button.na-folder-toggle` for Meetings (SVG chevron only, no aria-label)
  - `button.na-folder-toggle` for Agent Notes (SVG chevron only, no aria-label)
  - `button.na-folder-toggle` for Templates (SVG chevron only, no aria-label)
  - `button.na-folder-toggle` for Archive (SVG chevron only, no aria-label)
- Buttons without `type` attribute: 29 of 29 (all buttons in main lack `type="button"`)
- No ARIA landmarks (`role`) found in main content area
- H1 "Notes" is outside `<main>` element (in page header); only H2 "Notes" inside main
- No `<header>` element present in main content

## Findings

### notes-01
- SEVERITY: P1
- DIMENSION: action
- VIEWPORT: all
- EVIDENCE: Clicking the "New note" button (aria-label="New note") displays "Error: desktop runtime unavailable" as visible text at the top of the page content area. The error text is rendered in a `<div>` with no class. Despite the error, 4 notes are created in the Projects folder and a template dropdown appears.
- IMPACT: Users see a raw runtime error message in the UI when attempting to create a note in demo mode; confusing and unprofessional.

### notes-02
- SEVERITY: P1
- DIMENSION: console
- VIEWPORT: all
- EVIDENCE: React warning: `Encountered two children with the same key, 'n-1775774339404'` at NotesApp (src/pages/NotesApp.tsx:139:29). Triggered when switching category filters after notes are created.
- IMPACT: Duplicate React keys can cause incorrect rendering, stale state, or lost user input in the note list.

### notes-03
- SEVERITY: P2
- DIMENSION: overflow
- VIEWPORT: 1248
- EVIDENCE: `div.living-background` scrollWidth=1348 > clientWidth=1248, overflowing by 100px horizontally.
- IMPACT: Background element extends beyond viewport; may cause horizontal scrollbar in certain browser configurations.

### notes-04
- SEVERITY: P2
- DIMENSION: overflow
- VIEWPORT: 1248
- EVIDENCE: `section.holo-panel.holo-panel-mid` scrollWidth=1019 > clientWidth=937, internal overflow of 82px masked by `overflow:hidden`.
- IMPACT: Content inside holo-panel is clipped without scroll affordance; users cannot see or access overflowing content.

### notes-05
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: 6 `button.na-folder-toggle` elements (one per folder: Projects, Research, Meetings, Agent Notes, Templates, Archive) contain only an SVG chevron icon with no `aria-label`, `title`, or text content. Screen readers announce them as unlabeled buttons.
- IMPACT: Folder expand/collapse toggles are inaccessible to screen reader users.

### notes-06
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: 5 form controls lack accessible labels: `input.na-search-input` (has id=na-search but no `<label for>`), `select.na-sort-select`, `input.na-title-input`, `select.na-move-select`, `textarea.na-textarea`. None have `aria-label` or `aria-labelledby`.
- IMPACT: Screen readers cannot identify the purpose of these form fields.

### notes-07
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: All 29 `<button>` elements in main content lack `type="button"` attribute. Default type is "submit" which can cause unintended form submission.
- IMPACT: Buttons may trigger unintended form submission behavior; poor semantic markup.

### notes-08
- SEVERITY: P2
- DIMENSION: overflow
- VIEWPORT: 1248
- EVIDENCE: 3 `span.nexus-sidebar-item-text` elements overflow by 4px (scrollWidth=157, clientWidth=153). `div.na-agent-entry` overflows by 5px (scrollWidth=209, clientWidth=204).
- IMPACT: Minor text clipping in sidebar navigation items at narrow viewports.

## Summary
- Gate detected: no
- Total interactive elements: 26 (initial load)
- Elements clicked: 10
- P0: 0
- P1: 2
- P2: 6
