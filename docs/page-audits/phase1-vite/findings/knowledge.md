# Audit: Knowledge
URL: http://localhost:1420/knowledge
Audited at: 2026-04-10T01:30:00Z
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

NOTE: resize_window MCP tool reports success but viewport remains unchanged at 1888x951 across all resize attempts. All three viewports report identical measurements at the actual 1888x951 viewport.

### 1920x1080 (actual 1888x951)
- documentElement: scrollWidth=1888 clientWidth=1888 [OK]
- body: scrollWidth=1888 clientWidth=1888 [OK]
- main `main`: scrollWidth=1627 clientWidth=1627 [OK]
- other overflowing:
  - `div.living-background` scrollWidth=2039 clientWidth=1888 [OVERFLOW +151px]
  - `section.holo-panel.holo-panel-mid` scrollWidth=1716 clientWidth=1577 [OVERFLOW +139px]

### 1280x800 (actual 1888x951 — resize failed)
- documentElement: scrollWidth=1888 clientWidth=1888 [OK]
- body: scrollWidth=1888 clientWidth=1888 [OK]
- main `main`: scrollWidth=1627 clientWidth=1627 [OK]
- other overflowing: same as above

### 1024x768 (actual 1888x951 — resize failed)
- documentElement: scrollWidth=1888 clientWidth=1888 [OK]
- body: scrollWidth=1888 clientWidth=1888 [OK]
- main `main`: scrollWidth=1627 clientWidth=1627 [OK]
- other overflowing: same as above

## Interactive elements (main content only)
| # | Label | Type | Href | Enabled |
|---|-------|------|------|---------|
| 1 | (placeholder: Ask anything about your files...) | input[text] | — | yes |
| 2 | Search | button | — | yes |
| 3 | Index File | button | — | yes |
| 4 | Watch Directory | button | — | yes |
| 5 | Rebuild Index | button | — | yes |
| 6 | (placeholder: Enter a topic...) | input[text] | — | yes |
| 7 | Get Context | button | — | yes |
| 8 | (placeholder: File path e.g. /home/user/doc.md) | input[text] | — | yes |
| 9 | Get Entities | button | — | yes |
| 10 | (placeholder: File path e.g. /home/user/doc.md) | input[text] | — | yes |
| 11 | Get Graph | button | — | yes |
| 12 | Enable | button | — | yes |
| 13 | (placeholder: Search query...) | input[text] | — | yes |
| 14 | (placeholder: Time range start,end) | input[text] | — | yes |
| 15 | (placeholder: Source filter csv) | input[text] | — | yes |
| 16 | (placeholder: Max results) | input[text] | — | yes |
| 17 | Search Bridge | button | — | yes |
| 18 | (select: Screen/Document/Clipboard) | select | — | yes |
| 19 | (placeholder: Content to ingest...) | textarea | — | yes |
| 20 | (placeholder: Metadata JSON) | input[text] | — | yes |
| 21 | Ingest | button | — | yes |
| 22 | (placeholder: Entry ID to delete) | input[text] | — | yes |
| 23 | Delete | button | — | yes |
| 24 | (placeholder: Older than N days) | input[text] | — | yes |
| 25 | Clear Old | button | — | yes |

## Click sequence
### Click 1: "Search"
- Pathname before: /knowledge
- New console: clean
- Network failures: none
- Visible change: none — status text remains "Ready to search your file graph"
- Pathname after: /knowledge
- Reverted: n/a

### Click 2: "Index File"
- Pathname before: /knowledge
- New console: clean
- Network failures: none
- Visible change: none — silent, no file picker opened (demo mode)
- Pathname after: /knowledge
- Reverted: n/a

### Click 3: "Watch Directory"
- Pathname before: /knowledge
- New console: clean
- Network failures: none
- Visible change: none — silent, no folder picker opened (demo mode)
- Pathname after: /knowledge
- Reverted: n/a

### Click 4: "Rebuild Index"
- Pathname before: /knowledge
- New console: clean
- Network failures: none
- Visible change: none — silent
- Pathname after: /knowledge
- Reverted: n/a

### Click 5: "Get Context"
- Pathname before: /knowledge
- New console: clean
- Network failures: none
- Visible change: none — status text remains "Enter a topic to retrieve context from CogFS"
- Pathname after: /knowledge
- Reverted: n/a

### Click 6: "Get Entities"
- Pathname before: /knowledge
- New console: clean
- Network failures: none
- Visible change: none — status text remains "Enter a file path to extract entities"
- Pathname after: /knowledge
- Reverted: n/a

### Click 7: "Get Graph"
- Pathname before: /knowledge
- New console: clean
- Network failures: none
- Visible change: none — status text remains "Enter a file path to view graph links"
- Pathname after: /knowledge
- Reverted: n/a

### Click 8: "Enable" (Neural Bridge toggle)
- Pathname before: /knowledge
- New console: clean
- Network failures: none
- Visible change: "Failed to fetch status" replaced by "Toggle error: desktop runtime unavailable"
- Pathname after: /knowledge
- Reverted: n/a

### Click 9: "Search Bridge"
- Pathname before: /knowledge
- New console: clean
- Network failures: none
- Visible change: none — silent
- Pathname after: /knowledge
- Reverted: n/a

### Click 10: "Ingest"
- Pathname before: /knowledge
- New console: clean
- Network failures: none
- Visible change: none — silent
- Pathname after: /knowledge
- Reverted: n/a

### Skipped (destructive)
- "Delete" — reason: destructive keyword "delete"
- "Clear Old" — reason: destructive keyword "clear"

### Total interactive elements found: 25
### Elements clicked: 10 (capped at 10)

## Accessibility
- Images without alt: 0
- Inputs without label: 13 (selectors: `input[placeholder="Ask anything about your files..."]`, `input[placeholder="Enter a topic..."]`, `input[placeholder="File path (e.g. /home/user/doc.md)"]` x2, `input[placeholder="Search query..."]`, `input[placeholder="Time range (start,end)"]`, `input[placeholder="Source filter (csv)"]`, `input[placeholder="Max results"]`, `textarea[placeholder="Content to ingest..."]`, `input[placeholder="Metadata JSON e.g. {...}"]`, `input[placeholder="Entry ID to delete"]`, `input[placeholder="Older than N days"]`, `select` (Source type))
- Buttons without accessible name: 0

## Findings

### knowledge-01
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: Two `<h1>` elements on the page — `h1` "Knowledge Graph" inside `<header>` (banner landmark) and `h1` "Knowledge Graph" inside `<main>`. Duplicate H1 with identical text.
- IMPACT: Screen readers announce two identical top-level headings, breaking heading hierarchy and making page structure ambiguous.

### knowledge-02
- SEVERITY: P2
- DIMENSION: overflow
- VIEWPORT: 1888 (all measured)
- EVIDENCE: `div.living-background` scrollWidth=2039 > clientWidth=1888 (+151px overflow). Background decorative element extends beyond viewport.
- IMPACT: May cause horizontal scrollbar or clipped content on narrower viewports; decorative element should not overflow.

### knowledge-03
- SEVERITY: P2
- DIMENSION: overflow
- VIEWPORT: 1888 (all measured)
- EVIDENCE: `section.holo-panel.holo-panel-mid.nexus-page-panel` scrollWidth=1716 > clientWidth=1577 (+139px overflow). This is within the main content area.
- IMPACT: Content panel overflows its container, potentially causing horizontal scroll within the main content region.

### knowledge-04
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: All 13 form inputs (10 `<input>`, 2 `<textarea>`, 1 `<select>`) in main content lack accessible labels — no `<label for>`, no `aria-label`, no `aria-labelledby`. Inputs rely solely on `placeholder` text for identification.
- IMPACT: Screen readers cannot identify form fields; placeholder text disappears on focus and is not a valid accessible name per WCAG 2.1 SC 1.3.1.

### knowledge-05
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: Shell header buttons "Refresh" and "Start Jarvis" lack `type` attribute (`type=null`). Both are `<button>` elements without explicit `type="button"`.
- IMPACT: Buttons default to `type="submit"` per HTML spec, which can trigger unintended form submission if nested inside a form element.

### knowledge-06
- SEVERITY: P2
- DIMENSION: action
- VIEWPORT: all
- EVIDENCE: Neural Bridge section shows "Failed to fetch status" immediately on page load — error message rendered before any user interaction. After clicking "Enable", text changes to "Toggle error: desktop runtime unavailable". No console error emitted for either state.
- IMPACT: User sees an error state on initial page load with no context or remediation; the error is silent (no console output) making debugging difficult.

### knowledge-07
- SEVERITY: P2
- DIMENSION: action
- VIEWPORT: all
- EVIDENCE: 9 of 10 clicked buttons (Search, Index File, Watch Directory, Rebuild Index, Get Context, Get Entities, Get Graph, Search Bridge, Ingest) produce zero visible feedback, zero console output, and zero network requests in demo mode. Buttons accept click but provide no indication of action or failure.
- IMPACT: Users receive no feedback that their action was received or why it failed; violates user expectation of responsive UI elements.

### knowledge-08
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: H1 "Knowledge Graph" is located inside `<header>` element (outside `<main>`). The first heading inside `<main>` is also `<h1>`. The `<header>` H1 is in the global shell banner area, not the page content.
- IMPACT: H1 outside `<main>` is part of the shell chrome, not the page content; creates confusing document outline for assistive technology users.

## Summary
- Gate detected: no
- Total interactive elements: 25
- Elements clicked: 10
- P0: 0
- P1: 0
- P2: 8
