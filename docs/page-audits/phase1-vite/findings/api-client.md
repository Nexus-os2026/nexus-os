# Audit: Api Client
URL: http://localhost:1420/api-client
Audited at: 2026-04-09T20:15:00+01:00
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
- main `div.nexus-main-column`: scrollWidth=1630 clientWidth=1630 [OK]
- other overflowing:
  - `div.living-background`: scrollWidth=2035 clientWidth=1888 [OVERFLOW +147px]
  - `section.holo-panel.holo-panel-mid`: scrollWidth=2087 clientWidth=1577 [OVERFLOW +510px]
  - `aside.ac-sidebar`: scrollWidth=278 clientWidth=259 [OVERFLOW +19px]
  - `div.ac-collections`: scrollWidth=278 clientWidth=259 [OVERFLOW +19px]

### 1280x800
- documentElement: scrollWidth=1888 clientWidth=1888 [OK — viewport did not shrink, simulated via maxWidth]
- body: scrollWidth=1280 clientWidth=1280 [OK]
- main `div.nexus-main-column`: scrollWidth=1022 clientWidth=1022 [OK]
- other overflowing:
  - `div.living-background`: scrollWidth=2035 clientWidth=1888 [OVERFLOW +147px]
  - `section.holo-panel.holo-panel-mid`: scrollWidth=1306 clientWidth=969 [OVERFLOW +337px]
  - `aside.ac-sidebar`: scrollWidth=278 clientWidth=259 [OVERFLOW +19px]
  - `div.ac-collections`: scrollWidth=278 clientWidth=259 [OVERFLOW +19px]

### 1024x768
- documentElement: scrollWidth=1888 clientWidth=1888 [OK — viewport did not shrink, simulated via maxWidth]
- body: scrollWidth=1024 clientWidth=1024 [OK]
- main `div.nexus-main-column`: scrollWidth=766 clientWidth=766 [OK]
- other overflowing:
  - `div.living-background`: scrollWidth=2035 clientWidth=1888 [OVERFLOW +147px]
  - `section.holo-panel.holo-panel-mid`: scrollWidth=1057 clientWidth=713 [OVERFLOW +344px]
  - `div.ac-container`: scrollWidth=718 clientWidth=692 [OVERFLOW +26px]
  - `aside.ac-sidebar`: scrollWidth=278 clientWidth=259 [OVERFLOW +19px]
  - `div.ac-collections`: scrollWidth=278 clientWidth=259 [OVERFLOW +19px]
  - `div.ac-main`: scrollWidth=166 clientWidth=0 [OVERFLOW — collapses to 0px width]

## Interactive elements (main content only)
| # | Label | Type | Href | Enabled |
|---|-------|------|------|---------|
| 1 | Refresh | button (type=submit) | — | yes |
| 2 | Start Jarvis | button (type=submit) | — | yes |
| 3 | (icon, title="New collection") | button (type=submit) | — | yes |
| 4 | (icon, title="Audit") | button (type=submit) | — | yes |
| 5 | Create your first collection | button (type=submit) | — | yes |

## Click sequence
### Click 1: "Refresh"
- Pathname before: /api-client
- New console: clean
- Network failures: none
- Visible change: none — button silently no-ops
- Pathname after: /api-client
- Reverted: n/a

### Click 2: "Start Jarvis"
- Pathname before: /api-client
- New console: clean
- Network failures: none
- Visible change: none — button silently no-ops
- Pathname after: /api-client
- Reverted: n/a

### Click 3: "New collection" (icon button)
- Pathname before: /api-client
- New console: clean
- Network failures: none
- Visible change: none — collections sidebar still shows "No collections yet"; no collection was created
- Pathname after: /api-client
- Reverted: n/a

### Click 4: "Audit" (icon button)
- Pathname before: /api-client
- New console: clean
- Network failures: none
- Visible change: sidebar switches from collections list to "AUDIT TRAIL / No requests yet"; "Audit" button gains `active` class; "Create your first collection" button disappears; "Desktop runtime required" bar also disappears
- Pathname after: /api-client
- Reverted: n/a

### Click 5: "New collection" (icon button, second click — attempting to toggle back)
- Pathname before: /api-client
- New console: clean
- Network failures: none
- Visible change: none — sidebar remains stuck on "AUDIT TRAIL" view; does NOT toggle back to collections view; "New collection" button does not gain `active` class
- Pathname after: /api-client
- Reverted: n/a

Note: "Create your first collection" (button 5 from original list) was removed from DOM by click 4 and could not be clicked independently.

### Skipped (destructive)
none

### Total interactive elements found: 5
### Elements clicked: 5 (4 unique buttons; button 3 clicked twice to test toggle)

## Accessibility
- Images without alt: 0
- Inputs without label: 0
- Buttons without accessible name: 0 (two icon buttons use `title` attribute for accessible name)
- ARIA roles in main content: 0 — no `role` attributes found on any element in main content area

## Findings

### api-client-01
- SEVERITY: P1
- DIMENSION: overflow
- VIEWPORT: all
- EVIDENCE: `div.living-background` has scrollWidth=2035 vs clientWidth=1888 at all viewports. It is `position:fixed` and wider than the viewport by 147px.
- IMPACT: Decorative background element extends beyond viewport; clipped by body but structurally incorrect.

### api-client-02
- SEVERITY: P1
- DIMENSION: overflow
- VIEWPORT: all
- EVIDENCE: `section.holo-panel.holo-panel-mid` has overflow:hidden and scrollWidth exceeds clientWidth at all viewports: 2087 vs 1577 (1920), 1306 vs 969 (1280), 1057 vs 713 (1024). Content is silently clipped.
- IMPACT: The decorative holo-panel card silently clips its children; any content placed inside that exceeds the card width will be invisibly truncated.

### api-client-03
- SEVERITY: P1
- DIMENSION: overflow
- VIEWPORT: all
- EVIDENCE: `aside.ac-sidebar` (scrollWidth=278, clientWidth=259) and `div.ac-collections` (scrollWidth=278, clientWidth=259) overflow by 19px at all viewports.
- IMPACT: API client sidebar content is wider than its container; collection names or controls may be clipped or hidden.

### api-client-04
- SEVERITY: P1
- DIMENSION: overflow
- VIEWPORT: 1024
- EVIDENCE: `div.ac-main` collapses to clientWidth=0 while scrollWidth=166 at 1024px simulated width. `div.ac-container` also overflows (sw=718, cw=692).
- IMPACT: At narrow viewports the main request panel collapses to zero width, making the entire right-hand content area invisible.

### api-client-05
- SEVERITY: P2
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: All 4 buttons in main content have `type="submit"` but none have a parent `<form>` element. Selectors: `button.nx-btn.nx-btn-ghost` (Refresh), `button.nx-btn.nx-btn-primary` (Start Jarvis), `button.ac-btn-icon` (New collection), `button.ac-btn-icon` (Audit).
- IMPACT: Semantic HTML violation; `type="submit"` without a `<form>` is meaningless and misleading to assistive technologies.

### api-client-06
- SEVERITY: P2
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: "Refresh" and "Start Jarvis" buttons silently no-op on click. No console output, no network requests, no visible change. These buttons come from the App.tsx shell and have `hasDesktopRuntime()` guards.
- IMPACT: Buttons appear clickable but do nothing in demo mode; no feedback to user that action was blocked.

### api-client-07
- SEVERITY: P2
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: "New collection" icon button (title="New collection") silently no-ops on click. No collection is created, no error shown, no console output. After "Audit" tab is activated, clicking "New collection" does not toggle back to the collections view.
- IMPACT: Primary action to create an API collection does not work; user has no way to start using the API client. Once in Audit view, there is no way to return to collections.

### api-client-08
- SEVERITY: P2
- DIMENSION: copy
- VIEWPORT: all
- EVIDENCE: Two headings both read "API Client": `<h1>` in the page header area and `<h2>` in the ac-sidebar. Selectors: `h1` and `h2` inside `div.nexus-main-column`.
- IMPACT: Duplicate heading creates confusion for screen readers and heading-based navigation; the sidebar H2 should differentiate (e.g., "Collections").

### api-client-09
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: Zero `role` attributes found in the entire main content area. The sidebar/main split panel has no `role="complementary"` or `role="main"`. Icon buttons rely on `title` attribute only (not `aria-label`).
- IMPACT: Screen readers get no structural landmarks within the API client interface; `title` is less reliably announced than `aria-label` across assistive technology.

## Summary
- Gate detected: no
- Total interactive elements: 5
- Elements clicked: 5
- P0: 0
- P1: 4
- P2: 5
