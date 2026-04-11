# Audit: Projects
URL: http://localhost:1420/projects
Audited at: 2026-04-10T01:36:00Z
Gate detected: false
Gate type: none

## Console (captured at 1248x671, ALL messages)
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

Note: Viewport resize via MCP tool is locked at 1248x671 on this machine. All three target viewports (1920x1080, 1280x800, 1024x768) could not be tested. Measurements below are at the locked viewport.

### 1248x671 (locked — resize failed for 1920x1080, 1280x800, 1024x768)
- documentElement: scrollWidth=1248 clientWidth=1248 [OK]
- body: scrollWidth=1248 clientWidth=1248 [OK]
- main `main.nexus-shell-content`: scrollWidth=987 clientWidth=987 [OK]
- other overflowing:
  - `div.living-background`: scrollWidth=1348 clientWidth=1248 [OVERFLOW +100px]
  - `section.holo-panel.holo-panel-mid`: scrollWidth=1019 clientWidth=937 [OVERFLOW +82px]
  - `div.pm-board`: scrollWidth=1168 clientWidth=916 [OVERFLOW +252px]
  - 3x `span.nexus-sidebar-item-text`: scrollWidth=157 clientWidth=153 [OVERFLOW +4px each, sidebar text clipping]

## Interactive elements (main content only)
| # | Label | Type | Href | Enabled |
|---|-------|------|------|---------|
| 1 | Board | submit (button) | | yes |
| 2 | List | submit (button) | | yes |
| 3 | Timeline | submit (button) | | yes |
| 4 | Metrics | submit (button) | | yes |
| 5 | New Task | submit (button) | | yes |
| 6 | Search tasks... | text (input) | | yes |
| 7 | All assignees | select-one | | yes |
| 8 | All priorities | select-one | | yes |
| 9 | All tags | select-one | | yes |

## Click sequence
### Click 1: "Board"
- Pathname before: /projects
- New console: clean
- Network failures: none
- Visible change: Board view displayed (5 kanban columns: Backlog, To Do, In Progress, Review, Done). Active class on Board button.
- Pathname after: /projects
- Reverted: n/a

### Click 2: "List"
- Pathname before: /projects
- New console: clean
- Network failures: none
- Visible change: Board removed from DOM, `.pm-list-view` rendered with block display. Active class transfers to List button.
- Pathname after: /projects
- Reverted: n/a

### Click 3: "Timeline"
- Pathname before: /projects
- New console: clean
- Network failures: none
- Visible change: `.pm-timeline-view` rendered showing "Sprint 1: 3/31/2026 - 4/14/2026, 0% complete" with status columns. Active class transfers to Timeline button.
- Pathname after: /projects
- Reverted: n/a

### Click 4: "Metrics"
- Pathname before: /projects
- New console: clean
- Network failures: none
- Visible change: `.pm-metrics-view` rendered showing "Total Tasks 0, Completed 0, In Progress 0, Story Points 0/0, Total Fuel 0, Time Tracked 0m, Burndown Chart 0/0 points". Active class transfers to Metrics button.
- Pathname after: /projects
- Reverted: n/a

### Click 5: "New Task"
- Pathname before: /projects
- New console: clean
- Network failures: none
- Visible change: Modal overlay appeared with title "New Task", close button "x", text input "Task title...", status select (Backlog/To Do/In Progress/Review/Done), and "Create Task" button.
- Pathname after: /projects
- Reverted: n/a (modal closed via x button)

### Click 6: "Search tasks..." (input)
- Pathname before: /projects
- New console: clean
- Network failures: none
- Visible change: Input receives focus. No filtering occurs (0 tasks to filter).
- Pathname after: /projects
- Reverted: n/a

### Click 7: "All assignees" (select)
- Pathname before: /projects
- New console: clean
- Network failures: none
- Visible change: Dropdown opens showing options: All assignees, You, Coder Agent, Research Agent, Planner Agent, Self-Improve Agent, Content Agent, Unassigned.
- Pathname after: /projects
- Reverted: n/a

### Click 8: "All priorities" (select)
- Pathname before: /projects
- New console: clean
- Network failures: none
- Visible change: Dropdown opens showing options: All priorities, Critical, High, Medium, Low.
- Pathname after: /projects
- Reverted: n/a

### Click 9: "All tags" (select)
- Pathname before: /projects
- New console: clean
- Network failures: none
- Visible change: Dropdown opens showing options: All tags, feature, bug, refactor, docs, infra, agent-task, performance.
- Pathname after: /projects
- Reverted: n/a

### Skipped (destructive)
none

### Total interactive elements found: 9
### Elements clicked: 9

## Accessibility
- Images without alt: 0
- Inputs without label: 4 (selectors: `input[type="text"][placeholder="Search tasks..."]`, `select` (All assignees), `select` (All priorities), `select` (All tags))
- Buttons without accessible name: 0

### Additional a11y issues
- H1 "Project Manager" is outside `<main>` (in banner/header area); H2 "Project Manager" is inside `<main>` — duplicate heading text across heading levels
- 4 view-tab buttons (Board, List, Timeline, Metrics) lack `role="tab"`, `aria-selected`, and `aria-controls` attributes — not implementing ARIA tab pattern
- All 5 buttons in main lack explicit `type` attribute (default to `submit`)
- New Task modal lacks `role="dialog"` and `aria-label`/`aria-labelledby`
- Modal close button "x" has no `aria-label` (rendered as "x" character, not screen-reader-friendly)
- Modal input `pm-modal-input` and modal select `pm-modal-select` are unlabelled
- Section `section.holo-panel` lacks `role` and `aria-label`
- Banner buttons (Refresh, Start Jarvis) lack `type` attribute

## Findings

### projects-01
- SEVERITY: P2
- DIMENSION: overflow
- VIEWPORT: 1248 (locked)
- EVIDENCE: `div.pm-board` scrollWidth=1168 exceeds clientWidth=916 by 252px. The kanban board columns overflow their container horizontally.
- IMPACT: Board content is clipped or requires horizontal scroll at narrower viewports; may be worse at 1024x768.

### projects-02
- SEVERITY: P2
- DIMENSION: overflow
- VIEWPORT: 1248 (locked)
- EVIDENCE: `section.holo-panel.holo-panel-mid` scrollWidth=1019 exceeds clientWidth=937 by 82px.
- IMPACT: Main content panel overflows its container, potentially clipping right-side controls or content.

### projects-03
- SEVERITY: P2
- DIMENSION: overflow
- VIEWPORT: 1248 (locked)
- EVIDENCE: `div.living-background` scrollWidth=1348 exceeds clientWidth=1248 by 100px. Background decorative element extends beyond viewport.
- IMPACT: Cosmetic overflow from background layer; may cause horizontal scrollbar if overflow not hidden on parent.

### projects-04
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: 4 unlabelled form controls in main: `input[type="text"][placeholder="Search tasks..."]` and 3 `<select>` elements (assignees, priorities, tags) have no `<label>`, `aria-label`, or `aria-labelledby`.
- IMPACT: Screen readers cannot identify the purpose of these form controls; WCAG 1.3.1 / 4.1.2 violation.

### projects-05
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: View-tab buttons (Board, List, Timeline, Metrics) use `<button class="pm-view-btn">` with no `role="tab"`, no `aria-selected`, no `aria-controls`, and no `type` attribute. No parent `role="tablist"` container.
- IMPACT: Screen readers cannot convey tab selection state or tab-panel association; keyboard users cannot use standard tab-widget patterns.

### projects-06
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: H1 "Project Manager" is in the banner/header (outside `<main>`). H2 "Project Manager" is inside `<main>`. Both have identical text content, creating a duplicate heading.
- IMPACT: Screen reader users encounter the same heading twice at different levels, creating a confusing document outline.

### projects-07
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: New Task modal container has no `role="dialog"`, no `aria-label` or `aria-labelledby`. Close button renders "x" with no `aria-label="Close"`. Modal input and select are unlabelled.
- IMPACT: Screen readers cannot identify the modal as a dialog or announce its purpose; close button is not descriptive.

### projects-08
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: All 5 buttons in `<main>` and 2 banner buttons (Refresh, Start Jarvis) lack explicit `type` attribute. Buttons default to `type="submit"`, which may cause unintended form submission if placed inside a `<form>`.
- IMPACT: Buttons may trigger unexpected form behavior; best practice is explicit `type="button"`.

## Summary
- Gate detected: no
- Total interactive elements: 9
- Elements clicked: 9
- P0: 0
- P1: 0
- P2: 8
