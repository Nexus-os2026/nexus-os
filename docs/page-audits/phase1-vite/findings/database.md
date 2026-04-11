# Audit: Database
URL: http://localhost:1420/database
Audited at: 2026-04-09T20:17:00+01:00
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

### 1920x1080 (actual viewport 1888x895)
- documentElement: scrollWidth=1888 clientWidth=1888 [OK]
- body: scrollWidth=1888 clientWidth=1888 [OK]
- main `main`: scrollWidth=1627 clientWidth=1627 [OK]
- other overflowing:
  - `div.living-background`: scrollWidth=2039 clientWidth=1888 [OVERFLOW +151px]
  - `section.holo-panel.holo-panel-mid.nexus-page-panel`: scrollWidth=1809 clientWidth=1577 [OVERFLOW +232px, overflow:hidden clips silently]
  - `span.nexus-sidebar-item-text` (x3): scrollWidth=157 clientWidth=153 [OVERFLOW +4px each]

### 1280x800 (simulated via maxWidth constraint)
- documentElement: scrollWidth=1888 clientWidth=1888 [OK — viewport unchanged]
- body: scrollWidth=1888 clientWidth=1888 [OK]
- main `main`: scrollWidth=1019 clientWidth=1019 [OK]
- other overflowing:
  - `div.living-background`: scrollWidth=2046 clientWidth=1888 [OVERFLOW]
  - `section.holo-panel.holo-panel-mid`: scrollWidth=1371 clientWidth=969 [OVERFLOW +402px, clipped]

### 1024x768 (simulated via maxWidth constraint)
- documentElement: scrollWidth=1888 clientWidth=1888 [OK — viewport unchanged]
- body: scrollWidth=1888 clientWidth=1888 [OK]
- main `main`: scrollWidth=763 clientWidth=763 [OK]
- other overflowing:
  - `div.living-background`: scrollWidth=2046 clientWidth=1888 [OVERFLOW]
  - `section.holo-panel.holo-panel-mid`: scrollWidth=1116 clientWidth=713 [OVERFLOW +403px, clipped]

## Interactive elements (main content only)
| # | Label | Type | Href | Enabled |
|---|-------|------|------|---------|
| 1 | Connection name... | input (text) | — | yes |
| 2 | SQLite path (e.g. ~/.nexus/data.db) | input (text) | — | yes |
| 3 | Connect | button | — | yes |
| 4 | Query | button (tab) | — | yes |
| 5 | Builder | button (tab) | — | yes |
| 6 | Schema | button (tab) | — | yes |
| 7 | Visualize | button (tab) | — | yes |
| 8 | History | button (tab) | — | yes |
| 9 | Run Query | button | — | yes |
| 10 | CSV | button | — | yes |
| 11 | JSON | button | — | yes |
| 12 | Clear | button | — | yes |
| 13 | Enter SQL query... (Ctrl+Enter to run) | textarea | — | yes |

## Click sequence
### Click 1: "Connection name..." (input)
- Pathname before: /database
- New console: clean
- Network failures: none
- Visible change: input receives focus
- Pathname after: /database
- Reverted: n/a

### Click 2: "SQLite path..." (input)
- Pathname before: /database
- New console: clean
- Network failures: none
- Visible change: input receives focus
- Pathname after: /database
- Reverted: n/a

### Click 3: "Connect" (button, with empty inputs)
- Pathname before: /database
- New console: clean
- Network failures: none
- Visible change: none — silently no-ops (early return on empty input)
- Pathname after: /database
- Reverted: n/a

### Click 4: "Query" (tab button)
- Pathname before: /database
- New console: clean
- Network failures: none
- Visible change: Query tab active (default state), shows SQL textarea + Run Query/CSV/JSON/Clear buttons
- Pathname after: /database
- Reverted: n/a

### Click 5: "Builder" (tab button)
- Pathname before: /database
- New console: clean
- Network failures: none
- Visible change: Builder tab becomes active; Run Query/CSV/JSON/Clear buttons disappear; shows "Connect to a database first" placeholder
- Pathname after: /database
- Reverted: n/a

### Click 6: "Schema" (tab button)
- Pathname before: /database
- New console: clean
- Network failures: none
- Visible change: Schema tab becomes active; shows "Connect to a database first" placeholder (identical to Builder)
- Pathname after: /database
- Reverted: n/a

### Click 7: "Visualize" (tab button)
- Pathname before: /database
- New console: clean
- Network failures: none
- Visible change: Visualize tab active; shows "Run a query first, then visualize the results"
- Pathname after: /database
- Reverted: n/a

### Click 8: "History" (tab button)
- Pathname before: /database
- New console: clean
- Network failures: none
- Visible change: History tab active; shows "Query History", "0 queries", "No queries executed yet"
- Pathname after: /database
- Reverted: n/a

### Click 9: "Run Query" (button, back on Query tab)
- Pathname before: /database
- New console: clean
- Network failures: none
- Visible change: none — silently no-ops (no database connection; message remains "Connect to a database first.")
- Pathname after: /database
- Reverted: n/a

### Click 10: "CSV" (button)
- Pathname before: /database
- New console: clean
- Network failures: none
- Visible change: none — silently no-ops (no query results to export)
- Pathname after: /database
- Reverted: n/a

### Skipped (destructive)
- "Clear" — reason: destructive keyword "clear"

### Total interactive elements found: 13
### Elements clicked: 10 (capped at 10)

## Accessibility
- Images without alt: 0
- Inputs without label: 3 (selectors: `input[placeholder="Connection name..."]`, `input[placeholder="SQLite path (e.g. ~/.nexus/data.db)"]`, `textarea[placeholder="Enter SQL query... (Ctrl+Enter to run)"]`)
- Buttons without accessible name: 0

## Findings

### database-01
- SEVERITY: P1
- DIMENSION: overflow
- VIEWPORT: all
- EVIDENCE: `div.living-background` scrollWidth=2039–2046 vs clientWidth=1888 at native viewport. Fixed-position element wider than viewport by ~151px.
- IMPACT: Decorative background exceeds viewport; may cause horizontal scroll on devices where body overflow is not hidden.

### database-02
- SEVERITY: P1
- DIMENSION: overflow
- VIEWPORT: all
- EVIDENCE: `section.holo-panel.holo-panel-mid.nexus-page-panel` scrollWidth=1809 clientWidth=1577 at 1920; scrollWidth=1371 clientWidth=969 at 1280; scrollWidth=1116 clientWidth=713 at 1024. `overflow:hidden` clips content silently — up to 403px clipped at 1024.
- IMPACT: holo-panel clips page content silently at all viewports; users on smaller screens lose access to right-side content without any scroll indicator.

### database-03
- SEVERITY: P2
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: All 10 `<button>` elements in main content have no explicit `type` attribute. Source: `DatabaseManager.tsx` — every `<button>` omits `type`. No `<form>` elements exist on the page.
- IMPACT: Without `type="button"`, buttons default to `type="submit"`. While no `<form>` exists to accidentally submit, this violates semantic HTML best practice and can cause unexpected behavior if a form is ever added.

### database-04
- SEVERITY: P1
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: 3 form inputs have no associated `<label>`, no `aria-label`, no `aria-labelledby`, and no `id` for label association: `input[placeholder="Connection name..."]`, `input[placeholder="SQLite path (e.g. ~/.nexus/data.db)"]`, `textarea[placeholder="Enter SQL query... (Ctrl+Enter to run)"]`. Zero `<label>` elements exist in main content.
- IMPACT: Screen readers cannot identify these inputs; users relying on assistive technology cannot use the database connection or query features.

### database-05
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: Tab buttons (`Query`, `Builder`, `Schema`, `Visualize`, `History`) with class `.db-tab` have no `role="tab"`, no `aria-selected`, and no parent `role="tablist"`. Active state is communicated only via CSS class `active`.
- IMPACT: Screen readers cannot identify the tab pattern or announce which tab is selected; keyboard navigation patterns for tabs are not available.

### database-06
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: No ARIA landmark roles anywhere in main content. `<main>` exists but inner structure has no `role="region"`, `role="complementary"`, or other semantic roles on the sidebar or content panels.
- IMPACT: Assistive technology cannot navigate the page structure; sidebar vs content vs status bar are indistinguishable.

### database-07
- SEVERITY: P2
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: "Connect" button with filled inputs (`connectName="testdb"`, `connectInput="data.db"`) silently no-ops in demo mode. No error message, no console output, no visual feedback. Source: `connectToDb()` at DatabaseManager.tsx:100 calls Tauri `dbConnect()` command which fails silently without desktop runtime.
- IMPACT: User fills connection form and clicks Connect with no feedback that the action cannot work in demo mode; expected behavior would be an inline error or disabled state.

### database-08
- SEVERITY: P2
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: "Run Query", "CSV", and "JSON" buttons are all enabled and clickable but silently no-op when no database is connected. No error toast, no console message, no visual feedback on click.
- IMPACT: Users receive no indication why their action had no effect; the "Connect to a database first" message is passive text and not triggered by button clicks.

### database-09
- SEVERITY: P2
- DIMENSION: copy
- VIEWPORT: all
- EVIDENCE: "0 fuel" text appears twice on the page simultaneously — once in the toolbar area next to the tab row, and once in the status bar at the bottom.
- IMPACT: Duplicate information adds visual clutter and may confuse users about which fuel counter is authoritative.

## Summary
- Gate detected: no
- Total interactive elements: 13
- Elements clicked: 10
- P0: 0
- P1: 3
- P2: 6
