# Audit: Documents
URL: http://localhost:1420/documents
Audited at: 2026-04-09T19:30:00+01:00
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
(effective viewport: 1888x895)
- documentElement: scrollWidth=1888 clientWidth=1888 [OK]
- body: scrollWidth=1888 clientWidth=1888 [OK]
- main `main`: scrollWidth=1627 clientWidth=1627 [OK]
- other overflowing:
  - `div.living-background`: scrollWidth=2034 clientWidth=1888 [OVERFLOW +146px]
  - `section.holo-panel.holo-panel-mid.nexus-page-panel`: scrollWidth=1738 clientWidth=1577 [OVERFLOW +161px]
  - `span.nexus-sidebar-item-text` (x3): scrollWidth=157 clientWidth=153 [OVERFLOW +4px]

### 1280x800
(effective viewport: 1248x615)
- documentElement: scrollWidth=1248 clientWidth=1248 [OK]
- body: scrollWidth=1248 clientWidth=1248 [OK]
- main `main`: scrollWidth=987 clientWidth=987 [OK]
- other overflowing:
  - `div.living-background`: scrollWidth=1343 clientWidth=1248 [OVERFLOW +95px]
  - `section.holo-panel.holo-panel-mid.nexus-page-panel`: scrollWidth=1233 clientWidth=937 [OVERFLOW +296px]
  - `span.nexus-sidebar-item-text` (x3): scrollWidth=157 clientWidth=153 [OVERFLOW +4px]

### 1024x768
(effective viewport: 992x583)
- documentElement: scrollWidth=992 clientWidth=992 [OK]
- body: scrollWidth=992 clientWidth=992 [OK]
- main `main`: scrollWidth=731 clientWidth=731 [OK]
- other overflowing:
  - `div.living-background`: scrollWidth=1066 clientWidth=992 [OVERFLOW +74px]
  - Chat sidebar divs (x2): scrollWidth=283 clientWidth=273 [OVERFLOW +10px] — "Chat with Your Documents" and "Send" area
  - `span.nexus-sidebar-item-text` (x3): scrollWidth=157 clientWidth=153 [OVERFLOW +4px]
  - Note: `section.holo-panel-mid` does NOT overflow at this viewport (scrollWidth=681 clientWidth=681)

## Interactive elements (main content only)
| # | Label | Type | Href | Enabled |
|---|-------|------|------|---------|
| 1 | (drop zone / file input) | input[type=file] | — | true (display:none; wrapper div has onclick + cursor:pointer) |
| 2 | List View | button[type=submit] | — | true |
| 3 | Cluster View | button[type=submit] | — | true |
| 4 | Index documents first... | input[type=text] | — | false (disabled) |
| 5 | Send | button[type=submit] | — | false (disabled) |

Note: "Refresh" and "Start Jarvis" buttons are in the `<header>` element, not `<main>`.

## Click sequence
### Click 1: "(drop zone / file input wrapper)"
- Pathname before: /documents
- New console: clean
- Network failures: none
- Visible change: Triggers file picker dialog (expected for file input)
- Pathname after: /documents
- Reverted: n/a

### Click 2: "List View"
- Pathname before: /documents
- New console: clean
- Network failures: none
- Visible change: No visible change (List View was already the default view; button style unchanged — grey border/text remains)
- Pathname after: /documents
- Reverted: n/a

### Click 3: "Cluster View"
- Pathname before: /documents
- New console: clean
- Network failures: none
- Visible change: Cluster View button gains active state via inline styles (green border `rgba(0,255,157,0.267)`, green text `rgb(0,255,157)`, green bg `rgba(0,255,157,0.094)`). List View button switches to inactive state (grey border, grey text). No change in document list content (empty state remains).
- Pathname after: /documents
- Reverted: n/a

### Click 4: "Index documents first..." (disabled input)
- Pathname before: /documents
- New console: clean
- Network failures: none
- Visible change: none (disabled, click ignored)
- Pathname after: /documents
- Reverted: n/a

### Click 5: "Send" (disabled button)
- Pathname before: /documents
- New console: clean
- Network failures: none
- Visible change: none (disabled, click ignored)
- Pathname after: /documents
- Reverted: n/a

### Skipped (destructive)
none

### Total interactive elements found: 5
### Elements clicked: 5 (all elements)

## Accessibility
- Images without alt: 0
- Inputs without label: 2 (selectors: `input[type="file"]`, `input[type="text"]`)
- Buttons without accessible name: 0

## Findings

### documents-01
- SEVERITY: P2
- DIMENSION: overflow
- VIEWPORT: all
- EVIDENCE: `div.living-background` overflows at every viewport: +146px at 1920x1080, +95px at 1280x800, +74px at 1024x768. Element has `position: fixed; width: 1888px`. It is a decorative background layer.
- IMPACT: Fixed-position decorative element extends beyond viewport; no user-visible scrollbar because body does not overflow, but element is semantically wider than the viewport.

### documents-02
- SEVERITY: P2
- DIMENSION: overflow
- VIEWPORT: 1920|1280
- EVIDENCE: `section.holo-panel.holo-panel-mid.nexus-page-panel` has `overflow: hidden` and overflows: scrollWidth=1738 vs clientWidth=1577 (+161px) at 1920x1080; scrollWidth=1233 vs clientWidth=937 (+296px) at 1280x800. Content is silently clipped. At 1024x768, the panel fits (681/681).
- IMPACT: Up to 296px of panel content silently clipped at mid-range viewports; users cannot scroll to see it.

### documents-03
- SEVERITY: P2
- DIMENSION: overflow
- VIEWPORT: 1024
- EVIDENCE: Two anonymous `<div>` elements wrapping the "Chat with Your Documents" sidebar and Send button area overflow by +10px (scrollWidth=283, clientWidth=273) at 1024x768.
- IMPACT: Minor content clipping in the document chat sidebar at small viewports.

### documents-04
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: `input[type="file"]` (drop zone file picker) has no `id`, no `aria-label`, no associated `<label>`. It is `display: none` with a wrapper `<div onclick>` acting as the visible trigger.
- IMPACT: Screen readers cannot identify the purpose of the file input. The clickable wrapper div is not a semantic button/label.

### documents-05
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: `input[type="text"]` (chat input with placeholder "Index documents first...") has no `aria-label`, no `id`, no associated `<label>`. Placeholder text is not a substitute for an accessible label.
- IMPACT: Screen readers cannot identify the chat input's purpose.

### documents-06
- SEVERITY: P2
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: "List View" and "Cluster View" buttons use `type="submit"` but are not inside a `<form>`. They toggle view mode. Neither button has `aria-pressed` to communicate active/inactive state to assistive technology, despite having visual active state via inline styles.
- IMPACT: Assistive technology users cannot determine which view mode is currently active.

### documents-07
- SEVERITY: P2
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: The file upload drop zone is implemented as a `<div onclick>` wrapping a hidden `<input type="file">`. The div has `cursor: pointer` but no `role="button"`, no `tabindex`, no keyboard event handler visible in the DOM.
- IMPACT: Keyboard-only users may not be able to activate the file upload zone; it is not focusable or announced as interactive.

## Summary
- Gate detected: no
- Total interactive elements: 5
- Elements clicked: 5
- P0: 0
- P1: 0
- P2: 7
