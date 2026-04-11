# Audit: Collab Protocol
URL: http://localhost:1420/collab-protocol
Audited at: 2026-04-09T23:08:44Z
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
- `%cDownload the React DevTools for a better development experience: https://reactjs.org/link/react-devtools font-weight:bold` — chunk-NUMECXU6.js?v=5144749d:21550:24

### Debug
- `[vite] connecting...` — @vite/client:494:8
- `[vite] connected.` — @vite/client:617:14

## Overflow

> **Note:** Browser viewport locked at 1248x671 in this environment. `window.resizeTo()` and MCP `resize_window` have no effect. All three requested viewport sizes measured at the same actual 1248x671 viewport.

### 1920x1080 (measured at 1248x671)
- documentElement: scrollWidth=1248 clientWidth=1248 [OK]
- body: scrollWidth=1248 clientWidth=1248 [OK]
- main `main`: scrollWidth=987 clientWidth=987 [OK]
- other overflowing:
  - `div.living-background`: scrollWidth=1348 clientWidth=1248 [OVERFLOW +100px]
  - `section.holo-panel.holo-panel-mid.nexus-page-panel`: scrollWidth=1019 clientWidth=937 [OVERFLOW +82px]

### 1280x800 (measured at 1248x671)
- documentElement: scrollWidth=1248 clientWidth=1248 [OK]
- body: scrollWidth=1248 clientWidth=1248 [OK]
- main `main`: scrollWidth=987 clientWidth=987 [OK]
- other overflowing:
  - `div.living-background`: scrollWidth=1348 clientWidth=1248 [OVERFLOW +100px]
  - `section.holo-panel.holo-panel-mid.nexus-page-panel`: scrollWidth=1019 clientWidth=937 [OVERFLOW +82px]

### 1024x768 (measured at 1248x671)
- documentElement: scrollWidth=1248 clientWidth=1248 [OK]
- body: scrollWidth=1248 clientWidth=1248 [OK]
- main `main`: scrollWidth=987 clientWidth=987 [OK]
- other overflowing:
  - `div.living-background`: scrollWidth=1348 clientWidth=1248 [OVERFLOW +100px]
  - `section.holo-panel.holo-panel-mid.nexus-page-panel`: scrollWidth=1019 clientWidth=937 [OVERFLOW +82px]

## Interactive elements (main content only)
| # | Label | Type | Href | Enabled |
|---|-------|------|------|---------|
| 1 | Title (placeholder) | input text | — | yes |
| 2 | Goal (placeholder) | input text | — | yes |
| 3 | (no label) | select | — | yes |
| 4 | Lead Agent ID (placeholder) | input text | — | yes |
| 5 | Create | button (type=null) | — | yes |

## Click sequence
### Click 1: "Title" input
- Pathname before: /collab-protocol
- New console: clean
- Network failures: none
- Visible change: input receives focus
- Pathname after: /collab-protocol
- Reverted: n/a

### Click 2: "Goal" input
- Pathname before: /collab-protocol
- New console: clean
- Network failures: none
- Visible change: input receives focus
- Pathname after: /collab-protocol
- Reverted: n/a

### Click 3: select (combobox, no label)
- Pathname before: /collab-protocol
- New console: clean
- Network failures: none
- Visible change: select receives focus; dropdown is empty (0 options)
- Pathname after: /collab-protocol
- Reverted: n/a

### Click 4: "Lead Agent ID" input
- Pathname before: /collab-protocol
- New console: clean
- Network failures: none
- Visible change: input receives focus
- Pathname after: /collab-protocol
- Reverted: n/a

### Click 5: "Create" button
- Pathname before: /collab-protocol
- New console: clean
- Network failures: none
- Visible change: none — Active Sessions remains (0), no feedback to user
- Pathname after: /collab-protocol
- Reverted: n/a

### Skipped (destructive)
none

### Total interactive elements found: 5
### Elements clicked: 5 (of 5)

## Accessibility
- Images without alt: 0
- Inputs without label: 4 (selectors: `input[placeholder="Title"]`, `input[placeholder="Goal"]`, `select`, `input[placeholder="Lead Agent ID"]`)
- Buttons without accessible name: 0

## Findings

### collab-protocol-01
- SEVERITY: P1
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: 4 form inputs in `main > section` lack associated `<label>`, `aria-label`, `id`, and `name` attributes. Selectors: `input[placeholder="Title"]`, `input[placeholder="Goal"]`, `select`, `input[placeholder="Lead Agent ID"]`. The `<select>` element additionally has zero `<option>` children and no visible label text — screen readers announce it as an empty unlabeled combobox.
- IMPACT: Screen readers cannot identify form fields; form data cannot be submitted programmatically without `name` attributes.

### collab-protocol-02
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: Two `<h1>` elements on the page: "Collaboration" (in `banner` header) and "Agent Collaboration" (in `main > section`). Expected exactly one `<h1>` per page.
- IMPACT: Confuses screen reader heading navigation and violates WCAG best practices for heading hierarchy.

### collab-protocol-03
- SEVERITY: P2
- DIMENSION: overflow
- VIEWPORT: all (measured at 1248x671)
- EVIDENCE: `div.living-background` has `position: fixed; overflow: hidden` but scrollWidth=1348 > clientWidth=1248 (+100px). Content is clipped but the element is wider than the viewport.
- IMPACT: Non-scrolling decorative background — cosmetic only, no user-visible scrollbar, but indicates sizing bug in the background layer.

### collab-protocol-04
- SEVERITY: P2
- DIMENSION: overflow
- VIEWPORT: all (measured at 1248x671)
- EVIDENCE: `section.holo-panel.holo-panel-mid.nexus-page-panel` has `overflow: hidden` with scrollWidth=1019 > clientWidth=937 (+82px). Content inside the panel is clipped.
- IMPACT: Panel content may be cut off; users cannot scroll to see clipped content within the holo-panel.

### collab-protocol-05
- SEVERITY: P1
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: `<select>` element (combobox ref_228) contains 0 `<option>` elements. It has no `id`, `name`, or `aria-label`. When clicked, dropdown opens but is completely empty.
- IMPACT: User cannot select a collaboration session type; the form field serves no purpose.

### collab-protocol-06
- SEVERITY: P2
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: "Create" button (ref_230) has `type=null` (no `type` attribute set). It is inside a section but not inside a `<form>` element. Clicking it in demo mode produces zero console output, zero DOM changes, and zero user feedback. Active Sessions remains at (0).
- IMPACT: Silent no-op provides no feedback — user cannot tell if the click registered or failed. Missing `type="button"` is a semantic issue.

### collab-protocol-07
- SEVERITY: P1
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: All 4 form inputs (`Title`, `Goal`, select, `Lead Agent ID`) lack `id` and `name` attributes. Without `name`, no form data would be serialized on submission even if the form were functional.
- IMPACT: Form is structurally broken — even with a working backend, form values would not be transmitted.

### collab-protocol-08
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: `main > section` (ref_221) has `role="region"` but no `aria-label` or `aria-labelledby` attribute. WCAG requires landmark regions to have accessible names.
- IMPACT: Screen readers announce an unnamed region, reducing navigation utility.

## Summary
- Gate detected: no
- Total interactive elements: 5
- Elements clicked: 5
- P0: 0
- P1: 3
- P2: 5
