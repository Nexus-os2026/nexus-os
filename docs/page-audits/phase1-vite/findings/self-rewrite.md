# Audit: Self Rewrite
URL: http://localhost:1420/self-rewrite
Audited at: 2026-04-09T23:14:00Z
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

Note: Viewport is locked at 1248x671 across all resize attempts. The `resize_window` MCP tool and `window.resizeTo()` have no effect in this Chrome extension tab context. All three viewports report the same 1248x671 measurements.

### 1920x1080 (actual: 1248x671)
- documentElement: scrollWidth=1248 clientWidth=1248 [OK]
- body: scrollWidth=1248 clientWidth=1248 [OK]
- main `main`: scrollWidth=987 clientWidth=987 [OK]
- other overflowing:
  - `div.living-background`: scrollWidth=1348 clientWidth=1248 [OVERFLOW +100px]
  - `section.holo-panel.holo-panel-mid`: scrollWidth=1019 clientWidth=937 [OVERFLOW +82px]
  - `span.nexus-sidebar-item-text` (x3): scrollWidth=157 clientWidth=153 [OVERFLOW +4px]

### 1280x800 (actual: 1248x671)
- documentElement: scrollWidth=1248 clientWidth=1248 [OK]
- body: scrollWidth=1248 clientWidth=1248 [OK]
- main `main`: scrollWidth=987 clientWidth=987 [OK]
- other overflowing: same as above (viewport locked)

### 1024x768 (actual: 1248x671)
- documentElement: scrollWidth=1248 clientWidth=1248 [OK]
- body: scrollWidth=1248 clientWidth=1248 [OK]
- main `main`: scrollWidth=987 clientWidth=987 [OK]
- other overflowing: same as above (viewport locked)

## Interactive elements (main content only)
| # | Label | Type | Href | Enabled |
|---|-------|------|------|---------|
| 1 | Refresh | button (no type attr) | — | true |
| 2 | Start Jarvis | button (no type attr) | — | true |
| 3 | Analyze | button[type=button] | — | true |

## Click sequence
### Click 1: "Refresh"
- Pathname before: /self-rewrite
- New console: clean
- Network failures: none
- Visible change: none
- Pathname after: /self-rewrite
- Reverted: n/a

### Click 2: "Start Jarvis"
- Pathname before: /self-rewrite
- New console: clean
- Network failures: none
- Visible change: none
- Pathname after: /self-rewrite
- Reverted: n/a

### Click 3: "Analyze"
- Pathname before: /self-rewrite
- New console: clean
- Network failures: none
- Visible change: none
- Pathname after: /self-rewrite
- Reverted: n/a

### Skipped (destructive)
none

### Total interactive elements found: 3
### Elements clicked: 3

## Accessibility
- Images without alt: 0
- Inputs without label: 0
- Buttons without accessible name: 0
- Duplicate H1: 2 (both "Self-Rewrite Lab" — one in page header banner, one in main content)

## Findings

### self-rewrite-01
- SEVERITY: P2
- DIMENSION: overflow
- VIEWPORT: all (locked at 1248x671)
- EVIDENCE: `div.living-background` has scrollWidth=1348 vs clientWidth=1248 (+100px overflow). Element is `position:fixed` with `overflow:hidden`, so overflow is clipped and not user-visible, but the element is wider than the viewport.
- IMPACT: No visible scrollbar or content shift due to `overflow:hidden`, but the background div is 100px wider than the viewport, indicating incorrect sizing.

### self-rewrite-02
- SEVERITY: P1
- DIMENSION: overflow
- VIEWPORT: all (locked at 1248x671)
- EVIDENCE: `section.holo-panel.holo-panel-mid` has scrollWidth=1019 vs clientWidth=937 (+82px overflow). Computed `overflow:hidden` clips the content.
- IMPACT: Main content panel clips child content that exceeds its bounds. Any wider content (e.g., tables, code blocks, long labels) would be silently truncated with no scroll affordance.

### self-rewrite-03
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: Two `<h1>` elements on the page, both with text "Self-Rewrite Lab". First is in the page header/banner area (`[ref_211]`), second is inside `main > section.holo-panel` (`[ref_224]`).
- IMPACT: Duplicate H1 violates WCAG heading hierarchy. Screen readers announce two top-level headings with identical text, making page structure ambiguous.

### self-rewrite-04
- SEVERITY: P2
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: "Refresh" button (`[ref_217]`) and "Start Jarvis" button (`[ref_218]`) both lack a `type` attribute. Default `type` for `<button>` is `submit`, which can trigger unintended form submissions if placed inside a `<form>`.
- IMPACT: Semantic misuse — header buttons default to `type="submit"` instead of explicit `type="button"`.

### self-rewrite-05
- SEVERITY: P1
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: All 3 interactive buttons (Refresh, Start Jarvis, Analyze) produce zero console output, zero network requests, and zero visible DOM changes when clicked. No loading state, no toast, no error message, no disabled state change.
- IMPACT: Users clicking any button get no feedback whatsoever. In demo mode, buttons should either show a "backend required" toast/message or be visually disabled with a tooltip explaining unavailability.

### self-rewrite-06
- SEVERITY: P1
- DIMENSION: interactive
- VIEWPORT: all
- EVIDENCE: The "Analyze" button (`[ref_230]`) in the "Performance Analysis" section is the primary action for the page. Clicking it produces no change to the "0 bottlenecks tracked", "0 suggested patches", or "0 history events" counters, and does not populate the "No bottlenecks captured yet" empty state.
- IMPACT: The core page workflow (analyze -> get patches -> review history) is completely non-functional with no feedback. Users cannot discover what the page does.

### self-rewrite-07
- SEVERITY: P2
- DIMENSION: a11y
- VIEWPORT: all
- EVIDENCE: `main` element has no `aria-label` attribute. The 4 `<section>` elements inside main all lack `aria-label` and `role` attributes, including the primary content panel (`section.holo-panel.holo-panel-mid`) and the 3 sub-sections (Performance Analysis, Suggested Patches, Patch History).
- IMPACT: Screen readers cannot distinguish between the unnamed sections. Each section has a heading, but the `<section>` landmark itself is unlabeled.

## Summary
- Gate detected: no
- Total interactive elements: 3
- Elements clicked: 3
- P0: 0
- P1: 3
- P2: 4
